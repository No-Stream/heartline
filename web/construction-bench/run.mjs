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
      try { ready = (await fetch(url)).ok; } catch (error) { if (!(error instanceof TypeError)) throw error; }
      if (ready) break;
      await delay(100);
    }
    if (!ready) throw new Error('Vite did not start within 10 seconds');
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const page = await browser.newPage();
    await page.goto(`${url}/construction-bench/`);
    const result = await page.evaluate(async () => {
      const worker = new Worker('/src/worker.ts', { type: 'module' });
      const pending = [];
      let wake;
      let failure;
      const timeout = setTimeout(() => { failure = new Error('Construction worker timed out after 10 minutes'); if (wake) { wake(); wake = undefined; } }, 600_000);
      worker.onmessage = (event) => {
        if (event.data.type !== 'ready' && event.data.type !== 'construction-state') return;
        pending.push(event.data);
        if (wake) { wake(); wake = undefined; }
      };
      worker.onerror = (event) => { failure = new Error(event.message); if (wake) { wake(); wake = undefined; } };
      const receive = async (type) => {
        while (true) {
          if (failure) throw failure;
          const index = pending.findIndex((message) => message.type === type);
          if (index !== -1) return pending.splice(index, 1)[0];
          await new Promise((resolve) => { wake = resolve; });
        }
      };
      const action = async (message) => {
        const start = performance.now();
        worker.postMessage(message);
        const state = await receive('construction-state');
        return { state, elapsedMs: performance.now() - start };
      };
      try {
        await receive('ready');
        let { state } = await action({ type: 'construction-start', x: 25, y: 25, z: 320, heading: 0 });
        if (state.track.length !== 1 || state.message) throw new Error(`Station failed: ${state.message}`);
        const pieces = [
          ...Array(72).fill('Straight'), 'TurnSmallStandalone', 'TurnSmallStandalone',
          ...Array(73).fill('Straight'), 'TurnSmallStandalone', 'TurnSmallStandalone',
        ];
        let selected = state.selected;
        for (const [index, piece] of pieces.entries()) {
          if (piece !== selected) {
            ({ state } = await action({ type: 'construction-select', piece }));
            selected = piece;
            if (!state.canAppend) throw new Error(`Piece ${index + 1} cannot be appended: ${state.message}`);
          }
          ({ state } = await action({ type: 'construction-append' }));
          if (state.track.length !== index + 2) throw new Error(`Piece ${index + 1} failed: ${state.message}`);
          selected = state.selected;
        }
        ({ state } = await action({ type: 'construction-close' }));
        if (state.canClose || !state.canTest) throw new Error(`Circuit did not close: ${state.message}`);
        const deleted = await action({ type: 'construction-delete', id: 75 });
        if (deleted.state.track.length !== 149) throw new Error(`Deletion failed: ${deleted.state.message}`);
        const refreshed = await action({ type: 'construction-select', piece: 'Straight' });
        if (!refreshed.state.pieces.length) throw new Error(`Choice refresh failed: ${refreshed.state.message}`);
        return { deletionMs: deleted.elapsedMs, refreshMs: refreshed.elapsedMs, choices: refreshed.state.pieces.length, trackPieces: refreshed.state.track.length, supportFloats: refreshed.state.placedSupports.length };
      } finally {
        clearTimeout(timeout);
        worker.terminate();
      }
    });
    const choices = await page.evaluate(() => new Promise((resolve, reject) => {
      const worker = new Worker('/construction-bench/choices-worker.js', { type: 'module' });
      const timeout = setTimeout(() => { worker.terminate(); reject(new Error('Choices worker timed out after 10 minutes')); }, 600_000);
      worker.onmessage = (event) => { clearTimeout(timeout); worker.terminate(); resolve(event.data); };
      worker.onerror = (event) => { clearTimeout(timeout); worker.terminate(); reject(new Error(event.message)); };
    }));
    if (choices.error) throw new Error(choices.error);
    console.log(`150-piece closed circuit choices: deletion and first refresh ${choices.firstMs.toFixed(1)} ms (delete ${choices.deleteMs.toFixed(1)} ms, subsequent choices call ${choices.choiceMs.toFixed(1)} ms); repeated ${choices.repeatMs.toFixed(1)} ms; ${choices.rows} catalog rows`);
    console.log(`150-piece closed circuit: deletion and worker response ${result.deletionMs.toFixed(1)} ms; repeated selection and worker response ${result.refreshMs.toFixed(1)} ms; ${result.choices} choices, ${result.trackPieces} remaining pieces, ${result.supportFloats} support floats`);
    if (choices.firstMs >= 50) throw new Error(`Deletion and first choice refresh exceeded 50 ms: ${choices.firstMs.toFixed(1)} ms`);
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null) {
      const exited = new Promise((resolve) => server.once('exit', resolve));
      process.kill(-server.pid, 'SIGTERM');
      await exited;
    }
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
