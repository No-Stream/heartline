import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from '@playwright/test';

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

async function main() {
  const port = await freePort();
  const url = `http://127.0.0.1:${port}`;
  const server = spawn('npx', ['vite', '--config', 'web/vite.config.ts', '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { detached: true, stdio: 'ignore' });
  let browser;
  try {
    let ready = false;
    for (let attempt = 0; attempt < 100; attempt++) {
      if (server.exitCode !== null) throw new Error(`Vite exited with code ${server.exitCode}`);
      try {
        ready = (await fetch(url)).ok;
      } catch (error) {
        if (!(error instanceof TypeError)) throw error;
      }
      if (ready) break;
      await delay(100);
    }
    if (!ready) throw new Error('Vite did not start within 10 seconds');
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const page = await browser.newPage();
    await page.goto(`${url}/guest-bench/`);
    const result = await page.evaluate(() => new Promise((resolve, reject) => {
      const worker = new Worker('/guest-bench/worker.js', { type: 'module' });
      const timeout = setTimeout(() => { worker.terminate(); reject(new Error('Worker timed out after 120 seconds')); }, 120_000);
      worker.onmessage = (event) => { clearTimeout(timeout); worker.terminate(); resolve(event.data); };
      worker.onerror = (event) => { clearTimeout(timeout); worker.terminate(); reject(new Error(event.message)); };
    }));
    if (result.error) throw new Error(result.error);
    if (!(result.elapsedMs > 0) || result.guestCount !== 10_000 || result.measuredTicks !== 200) {
      throw new Error(`Invalid benchmark result: ${JSON.stringify(result)}`);
    }
    console.log(`Browser worker (single thread, includes coaster physics): ${(result.measuredTicks * 1000 / result.elapsedMs).toFixed(2)} ticks/s, ${(result.elapsedMs / result.measuredTicks).toFixed(3)} ms/tick, state hash ${result.stateHash}`);
    console.log(`Guest snapshot transfer: ${result.snapshotBytes} bytes/snapshot (${result.snapshotBytes / result.guestCount} bytes/guest)`);
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null) {
      const exited = new Promise((resolve) => server.once('exit', resolve));
      process.kill(-server.pid, 'SIGTERM');
      await exited;
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
