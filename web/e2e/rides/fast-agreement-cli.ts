import { execFile } from "node:child_process";
import { availableParallelism } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkRideAgreement } from "./fast-agreement.ts";

/** Usage: node --experimental-strip-types web/e2e/rides/fast-agreement-cli.ts <design binary> <ride file> ... */
const [designBinary, ...ridePaths] = process.argv.slice(2);
if (!designBinary || ridePaths.length === 0) {
  throw new Error("usage: fast-agreement-cli.ts <design binary> <ride file> ...");
}

const maximumFailuresShown = 20;

async function checkOneRide(ridePath: string): Promise<boolean> {
  const result = await checkRideAgreement(resolve(designBinary), resolve(ridePath));
  const timing = `${result.seconds.total.toFixed(1)} s, of which the Node/WASM browser path took ${result.seconds.wasm.toFixed(1)} s alongside the native check`;
  if (result.failures.length === 0) {
    console.log(`${result.ride}: browser-path agreement passed (${result.estimateComparisons} estimate comparisons and the test report; ${timing})`);
    return true;
  }
  console.log(`${result.ride}: browser-path agreement FAILED (${timing})`);
  for (const failure of result.failures.slice(0, maximumFailuresShown)) console.log(`  - ${failure}`);
  if (result.failures.length > maximumFailuresShown) console.log(`  … and ${result.failures.length - maximumFailuresShown} more`);
  return false;
}

/** Each ride runs in its own process, so rides check in parallel; output stays in the order given. */
function checkRideInChildProcess(ridePath: string): Promise<{ readonly passed: boolean; readonly output: string }> {
  const script = fileURLToPath(import.meta.url);
  return new Promise((resolvePromise, reject) => {
    execFile(process.execPath, [...process.execArgv, script, designBinary, ridePath], { maxBuffer: 1 << 24, encoding: "utf8" }, (error, stdout, stderr) => {
      const status = error ? error.code : 0;
      if (status !== 0 && status !== 1) {
        reject(new Error(`agreement check of ${ridePath} crashed (${String(status)}):\n${stdout}${stderr}`));
        return;
      }
      resolvePromise({ passed: status === 0, output: stdout + stderr });
    });
  });
}

const start = performance.now();
if (ridePaths.length === 1) {
  process.exitCode = (await checkOneRide(ridePaths[0])) ? 0 : 1;
} else {
  // The native check and the WASM replay each use one core per ride.
  const concurrency = Math.max(1, Math.floor(availableParallelism() / 2));
  const results: Promise<{ readonly passed: boolean; readonly output: string }>[] = [];
  const running = new Set<Promise<unknown>>();
  for (const ridePath of ridePaths) {
    while (running.size >= concurrency) await Promise.race(running);
    const result = checkRideInChildProcess(ridePath);
    const tracked: Promise<unknown> = result.finally(() => running.delete(tracked));
    running.add(tracked);
    results.push(result);
  }
  let passedRides = 0;
  for (const result of results) {
    const { passed, output } = await result;
    process.stdout.write(output);
    if (passed) passedRides += 1;
  }
  console.log(`Browser-path agreement: ${passedRides}/${ridePaths.length} rides passed in ${((performance.now() - start) / 1000).toFixed(1)} s`);
  process.exitCode = passedRides === ridePaths.length ? 0 : 1;
}
