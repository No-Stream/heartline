// Places the three journey circuits through the real game worker (web/src/worker.ts) in
// Chrome, one exact selection and one placement per piece, and reports how long the worker
// takes to answer each placement with its full construction state.
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
    const circuits = [['Timber Tumbler', 'model:hybrid'], ['Velocity Viper', 'model:velocity_viper'], ['Briar Blitz', 'model:wooden_twister']];
    for (const [name, model] of circuits) {
      const result = await page.evaluate(placeCircuit, { name, model });
      const sorted = (values) => [...values].sort((first, second) => first - second);
      const percentile = (values, fraction) => sorted(values)[Math.max(1, Math.ceil(values.length * fraction)) - 1];
      const describe = (values) => `median ${percentile(values, 0.5).toFixed(1)} ms, p95 ${percentile(values, 0.95).toFixed(1)} ms, max ${Math.max(...values).toFixed(1)} ms`;
      console.log(`Browser worker ${name}: ${result.placements.length} placements; full state after placement ${describe(result.placements)}; first state after placement ${describe(result.firstStates)}; selection ${describe(result.selections)}`);
    }
  } finally {
    if (browser) await browser.close();
    if (server.exitCode === null) {
      const exited = new Promise((resolve) => server.once('exit', resolve));
      process.kill(-server.pid, 'SIGTERM');
      await exited;
    }
  }
}

async function placeCircuit({ name, model }) {
  const text = await (await fetch('/e2e/player/circuits.txt')).text();
  const sections = text.split(`[${name}]`);
  if (sections.length !== 2) throw new Error(`Expected one ${name} circuit`);
  const lines = [];
  for (const raw of sections[1].split('\n')) {
    const line = raw.trim();
    if (line.startsWith('[')) break;
    if (line && !line.startsWith('#')) lines.push(line);
  }
  const worker = new Worker('/src/worker.ts', { type: 'module' });
  const pending = [];
  let wake;
  let failure;
  const notify = () => { if (wake) { wake(); wake = undefined; } };
  worker.onmessage = (event) => { pending.push(event.data); notify(); };
  worker.onerror = (event) => { failure = new Error(event.message); notify(); };
  const timeout = setTimeout(() => { failure = new Error(`${name} timed out after 10 minutes`); notify(); }, 600_000);
  // The worker also sends states from background work, so each wait names the state it expects.
  const receive = async (matches) => {
    while (true) {
      if (failure) throw failure;
      const index = pending.findIndex(matches);
      if (index !== -1) return pending.splice(index, 1)[0];
      await new Promise((resolve) => { wake = resolve; });
    }
  };
  const fullState = (matches) => (message) => message.type === 'construction-state' && message.complete && matches(message);
  try {
    worker.postMessage({ type: 'init', seed: 42n, park: 'new', guestCount: 0, frozen: false, manualBlockAnalysis: false });
    await receive((message) => message.type === 'ready');
    worker.postMessage({ type: 'construction-select', piece: model, exact: true });
    await receive(fullState(() => true));
    worker.postMessage({ type: 'construction-start', x: 241, y: 249, z: 12, heading: 1 });
    let state = await receive(fullState((message) => message.track.length === 1));
    const placements = [];
    const firstStates = [];
    const selections = [];
    for (const piece of lines) {
      if (piece === '+chain' || piece === '-chain') {
        worker.postMessage({ type: 'construction-set-chain', enabled: piece === '+chain' });
        state = await receive(fullState((message) => message.chain === (piece === '+chain')));
        continue;
      }
      pending.length = 0;
      const selectionStart = performance.now();
      worker.postMessage({ type: 'construction-select', piece, exact: true });
      state = await receive(fullState((message) => message.selected === piece));
      selections.push(performance.now() - selectionStart);
      const pieceCount = state.track.length + 1;
      pending.length = 0;
      const placementStart = performance.now();
      worker.postMessage({ type: 'construction-append' });
      await receive((message) => message.type === 'construction-state' && message.track.length === pieceCount);
      firstStates.push(performance.now() - placementStart);
      state = await receive(fullState((message) => message.track.length === pieceCount));
      placements.push(performance.now() - placementStart);
    }
    return { placements, firstStates, selections };
  } finally {
    clearTimeout(timeout);
    worker.terminate();
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
