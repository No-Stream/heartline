import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from '@playwright/test';

const duration = 10_000;
const markerNames = ['terrain', 'person', 'track', 'tunnel', 'trackOverTrack', 'transparentEdge'];

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const port = server.address().port;
  await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  return port;
}

async function waitForServer(url, server) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (server.exitCode !== null) throw new Error(`Vite exited with code ${server.exitCode}`);
    try {
      if ((await fetch(url)).ok) return;
    } catch (error) {
      if (!(error instanceof TypeError)) throw error;
    }
    await delay(100);
  }
  throw new Error('Vite did not start within 10 seconds');
}

async function verifyMarkers(page, markers) {
  const canvas = page.locator('canvas');
  if (await canvas.count() !== 1) throw new Error('Expected exactly one benchmark canvas');
  const dimensions = await canvas.evaluate((element) => ({ width: element.width, height: element.height }));
  const screenshot = await canvas.screenshot();
  const pixels = await page.evaluate(async ({ bytes, markers, dimensions }) => {
    const bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: 'image/png' }));
    const canvas = document.createElement('canvas');
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Cannot inspect screenshot pixels');
    context.drawImage(bitmap, 0, 0);
    return Object.fromEntries(Object.entries(markers).map(([name, marker]) => {
      const x = Math.floor((marker.x + 0.5) * bitmap.width / dimensions.width);
      const y = Math.floor((marker.y + 0.5) * bitmap.height / dimensions.height);
      return [name, { color: Array.from(context.getImageData(x, y, 1, 1).data), x, y, width: bitmap.width, height: bitmap.height }];
    }));
  }, { bytes: [...screenshot], markers, dimensions });
  for (const name of markerNames) {
    const marker = markers?.[name];
    if (!marker || !Array.isArray(marker.color) || marker.color.length !== 4 ||
        !Number.isFinite(marker.x) || !Number.isFinite(marker.y) ||
        marker.x < 0 || marker.y < 0 || marker.x >= dimensions.width || marker.y >= dimensions.height) {
      throw new Error(`Invalid ${name} marker`);
    }
    if (marker.color.some((value, index) => Math.abs(value - pixels[name].color[index]) > 4)) {
      throw new Error(`${name} pixel: expected ${marker.color}, got ${JSON.stringify(pixels[name])}`);
    }
  }
}

async function runConfiguration(browser, baseUrl, renderer, backend, disableWebGPU = false) {
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  try {
    if (disableWebGPU) {
      await context.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true }));
    }
    const page = await context.newPage();
    page.setDefaultTimeout(120_000);
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    const params = new URLSearchParams({ renderer, backend, duration: String(duration) });
    await page.goto(`${baseUrl}/bench/?${params}`, { waitUntil: 'load' });
    await page.waitForFunction(() => window.benchmark?.ready || window.benchmark?.error, null, { timeout: 120_000 });
    let state = await page.evaluate(() => window.benchmark);
    if (state.error) return { error: state.error };
    await page.waitForFunction(() => window.benchmark?.result || window.benchmark?.error, null, { timeout: duration + 120_000 });
    state = await page.evaluate(() => window.benchmark);
    if (state.error) return { error: state.error };
    if (pageErrors.length) throw new Error(`Page errors: ${pageErrors.join('; ')}`);
    const result = state.result;
    if (!result || !Number.isFinite(result.meanMs) || !Number.isFinite(result.p95Ms) ||
        !Number.isFinite(result.frames) || result.frames <= 0 || result.durationMs < duration ||
        !result.backend || !result.adapter) throw new Error('Invalid benchmark result');
    if (backend !== 'auto' && result.backend.toLowerCase() !== backend) {
      throw new Error(`Requested ${backend}, received ${result.backend}`);
    }
    if (disableWebGPU && result.backend.toLowerCase() !== 'webgl2') {
      throw new Error(`WebGPU-disabled auto selected ${result.backend}, expected webgl2`);
    }
    await verifyMarkers(page, state.markers);
    return { result };
  } finally {
    await context.close();
  }
}

async function main() {
  const port = await freePort();
  const baseUrl = `http://127.0.0.1:${port}`;
  const temporaryDirectory = await mkdtemp(join(tmpdir(), 'theme-park-bench-'));
  const configPath = join(temporaryDirectory, 'vite.config.mjs');
  await writeFile(configPath, `import config from ${JSON.stringify(resolve('web/vite.config.ts'))};\nexport default { ...config, cacheDir: ${JSON.stringify(join(temporaryDirectory, 'cache'))}, server: { ...config.server, watch: { ...config.server?.watch, ignored: ['**/art/**'] } } };\n`);
  const server = spawn('npx', ['vite', '--config', configPath, '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.pipe(process.stderr);
  server.stderr.pipe(process.stderr);
  let browser;
  try {
    await waitForServer(baseUrl, server);
    const args = ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist', '--enable-features=Vulkan'];
    if (process.env.BENCH_SWIFTSHADER === '1') args.push('--use-vulkan=swiftshader', '--use-angle=swiftshader');
    console.log(`Chrome mode: ${process.env.BENCH_SWIFTSHADER === '1' ? 'explicit SwiftShader software' : 'default GPU selection'}`);
    browser = await chromium.launch({ channel: 'chrome', headless: true, args });
    const session = await browser.newBrowserCDPSession();
    const gpu = await session.send('SystemInfo.getInfo');
    console.log('Chrome GPU information:', JSON.stringify({ devices: gpu.gpu.devices, featureStatus: gpu.gpu.featureStatus }));
    const probe = await browser.newPage();
    const webgl = await probe.evaluate(() => {
      const context = document.createElement('canvas').getContext('webgl2');
      if (!context) return null;
      const extension = context.getExtension('WEBGL_debug_renderer_info');
      return {
        vendor: context.getParameter(extension ? extension.UNMASKED_VENDOR_WEBGL : context.VENDOR),
        renderer: context.getParameter(extension ? extension.UNMASKED_RENDERER_WEBGL : context.RENDERER),
      };
    });
    console.log('WebGL2 device:', JSON.stringify(webgl));
    await probe.close();
    const rows = [];
    let failed = false;
    for (const renderer of ['custom', 'pixi']) {
      for (const backend of ['webgpu', 'webgl2']) {
        try {
          console.log(`Running ${renderer} / ${backend} for at least ${duration} ms`);
          const outcome = await runConfiguration(browser, baseUrl, renderer, backend);
          if (outcome.error) {
            if (backend !== 'webgpu' || outcome.error !== 'WebGPU adapter is unavailable') {
              throw new Error(outcome.error);
            }
            console.error(`${renderer} ${backend} unavailable: ${outcome.error}`);
            rows.push({ renderer, requested: backend, status: 'unavailable', reason: outcome.error });
          } else {
            console.log(`${renderer} / ${backend}: pixel markers passed`);
            rows.push({ renderer, requested: backend, status: 'ok', ...outcome.result });
          }
        } catch (error) {
          failed = true;
          console.error(`${renderer} ${backend} failed:`, error);
          rows.push({ renderer, requested: backend, status: 'failed', reason: error.message });
        }
      }
      try {
        console.log(`Checking ${renderer} automatic fallback with WebGPU disabled`);
        const outcome = await runConfiguration(browser, baseUrl, renderer, 'auto', true);
        if (outcome.error) throw new Error(outcome.error);
        console.log(`${renderer} fallback: WebGPU disabled, selected ${outcome.result.backend}; pixel markers passed`);
      } catch (error) {
        failed = true;
        console.error(`${renderer} fallback failed:`, error);
      }
    }
    console.table(rows);
    if (failed) process.exitCode = 1;
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null) {
      const exited = new Promise((resolve) => server.once('exit', resolve));
      process.kill(-server.pid, 'SIGTERM');
      await exited;
    }
    await rm(temporaryDirectory, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
