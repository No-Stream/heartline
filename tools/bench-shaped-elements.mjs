import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const COLD_RUNS = 15;
const COLD_WASM_RUNS = 30;
const KINDS = [
  { kind: 'hyper-strong-floater-hill', requestedSpeed: 34 },
  { kind: 'hyper-fully-banked-turn', requestedSpeed: 28 },
];
const scriptPath = fileURLToPath(import.meta.url);

if (process.argv[2] === '--wasm-single') {
  runWasmSingle(process.argv[3], process.argv[4]);
} else if (process.argv[2] === '--check-horizon-hawk-arc-parity') {
  checkHorizonHawkArcParity(process.argv[3], process.argv[4]);
} else if (process.argv[2] === '--wasm-try-single') {
  runWasmTrySingle(process.argv[3], process.argv[4]);
} else if (process.argv[2] === '--cold-wasm') {
  runColdWasmSuite(process.argv[3]);
} else if (process.argv[2] === '--wasm-calibration-single') {
  runWasmCalibrationSingle(process.argv[3], process.argv[4]);
} else if (process.argv[2] === '--wasm-preview-modes') {
  runWasmPreviewModesSingle(process.argv[3], process.argv[4]);
} else if (process.argv[2] === '--run') {
  runSuite(process.argv[3], process.argv[4], process.argv.slice(5));
} else {
  throw new Error('Usage: bench-shaped-elements.mjs --run <wasm-module.js> <native-binary>');
}

function runWasmSingle(modulePath, kind) {
  const resolvedModulePath = path.resolve(modulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  const startedAt = performance.now();
  benchmark.solve();
  const elapsedMs = performance.now() - startedAt;
  const result = JSON.parse(benchmark.result_json());
  process.stdout.write(`${JSON.stringify({ ...result, elapsed_ms: elapsedMs })}\n`);
}

function checkHorizonHawkArcParity(wasmModulePath, nativeBinaryPath) {
  const resolvedModulePath = path.resolve(wasmModulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const wasmArcTable = wasm.horizon_hawk_first_turn_arc_table_json();
  const nativeArcTable = runProcess(nativeBinaryPath, ['--horizon-hawk-turn-arc-table']);
  const wasmDigest = createHash('sha256').update(wasmArcTable).digest('hex');
  const nativeDigest = createHash('sha256').update(nativeArcTable).digest('hex');
  if (wasmArcTable !== nativeArcTable) {
    throw new Error(
      `Horizon Hawk first shaped-turn arc table differs: native ${nativeDigest}, WASM ${wasmDigest}`,
    );
  }

  const table = JSON.parse(nativeArcTable);
  if (table.lengths.length !== 1025 || table.speeds.length !== 1025) {
    throw new Error(
      `Expected 1,025 Horizon Hawk arc rows, received ${table.lengths.length} lengths and ${table.speeds.length} speeds`,
    );
  }
  const wasmCandidatePreviewText = wasm.horizon_hawk_first_turn_candidate_preview_json();
  const nativeCandidatePreviewText = runProcess(nativeBinaryPath, [
    '--horizon-hawk-turn-candidate-preview',
  ]);
  const wasmCandidatePreview = JSON.parse(wasmCandidatePreviewText);
  const nativeCandidatePreview = JSON.parse(nativeCandidatePreviewText);
  const candidatePreviewMismatch = firstMismatch(
    nativeCandidatePreview,
    wasmCandidatePreview,
    'candidatePreview',
  );
  if (candidatePreviewMismatch) {
    throw new Error(`Horizon Hawk first-turn candidate force preview differs across targets: ${candidatePreviewMismatch}`);
  }
  const wasmHillSelection = JSON.parse(wasm.horizon_hawk_first_hill_selected_shape_json());
  const nativeHillSelection = JSON.parse(runProcess(nativeBinaryPath, ['--horizon-hawk-hill-selected-shape']));
  const nativeHillSpec = JSON.stringify(nativeHillSelection.spec);
  const wasmHillSpec = JSON.stringify(wasmHillSelection.spec);
  if (nativeHillSpec !== wasmHillSpec) {
    throw new Error(`Horizon Hawk first shaped hill solves differently across targets: native ${nativeHillSpec}, WASM ${wasmHillSpec}`);
  }
  const hillSelectionMismatch = firstMismatch(nativeHillSelection, wasmHillSelection, 'firstHillSelection');
  if (hillSelectionMismatch) {
    throw new Error(`Horizon Hawk first shaped hill selection differs across targets: ${hillSelectionMismatch}`);
  }
  const parityCases = JSON.parse(wasm.shaped_parity_cases_json());
  if (parityCases.length < 10) {
    throw new Error(`Expected at least 10 per-model parity cases (5 models × hill + turn), got ${parityCases.length}`);
  }
  const perModelParity = [];
  for (const parityCase of parityCases) {
    const benchmark = new wasm.ShapedSolveBenchmark(parityCase.id);
    benchmark.prepare();
    benchmark.solve();
    const wasmResult = JSON.parse(benchmark.result_json());
    const nativeResult = JSON.parse(runProcess(nativeBinaryPath, ['--single', parityCase.id]));
    delete nativeResult.elapsed_ms;
    const mismatch = firstMismatch(nativeResult, wasmResult, parityCase.id);
    if (mismatch) {
      throw new Error(`Native vs WASM shaped parity failed on ${parityCase.id}: ${mismatch}`);
    }
    if (nativeResult.spec_hash !== wasmResult.spec_hash) {
      throw new Error(`Spec hash mismatch on ${parityCase.id}: native ${nativeResult.spec_hash}, wasm ${wasmResult.spec_hash}`);
    }
    perModelParity.push({
      id: parityCase.id,
      spec_hash: nativeResult.spec_hash,
      identical: true,
    });
  }
  process.stdout.write(`${JSON.stringify({
    ride: 'Horizon Hawk',
    piece: 'first shaped turn',
    arc_rows: table.lengths.length,
    force_samples: JSON.parse(nativeCandidatePreviewText).forceEnvelopeSamples.length,
    native_sha256: nativeDigest,
    wasm_sha256: wasmDigest,
    force_previews_within_tolerance: true,
    first_hill_selected_shape: nativeHillSelection.spec,
    per_model_parity_cases: perModelParity.length,
    per_model_parity: perModelParity,
    identical: true,
  })}\n`);
}

function firstMismatch(expected, actual, path) {
  if (typeof expected === 'number') {
    if (typeof actual !== 'number') return `${path} must be numeric`;
    const difference = Math.abs(expected - actual);
    return difference <= 1e-6 ? null : `${path} differs by ${difference.toExponential(2)} (native ${expected}, wasm ${actual})`;
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) return `${path} must be an array`;
    if (actual.length !== expected.length) return `${path} length differs (native ${expected.length}, wasm ${actual.length})`;
    for (let index = 0; index < expected.length; index += 1) {
      const mismatch = firstMismatch(expected[index], actual[index], `${path}[${index}]`);
      if (mismatch) return mismatch;
    }
    return null;
  }
  if (expected !== null && typeof expected === 'object') {
    if (actual === null || typeof actual !== 'object' || Array.isArray(actual)) return `${path} must be an object`;
    const expectedKeys = Object.keys(expected);
    const actualKeys = Object.keys(actual);
    if (expectedKeys.length !== actualKeys.length || expectedKeys.some((key) => !(key in actual))) {
      return `${path} fields differ`;
    }
    for (const key of expectedKeys) {
      const mismatch = firstMismatch(expected[key], actual[key], `${path}.${key}`);
      if (mismatch) return mismatch;
    }
    return null;
  }
  return Object.is(expected, actual) ? null : `${path} differs (native ${expected}, wasm ${actual})`;
}

function runWasmTrySingle(modulePath, kind) {
  const resolvedModulePath = path.resolve(modulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  const startedAt = performance.now();
  benchmark.try_solve_and_commit();
  const elapsedMs = performance.now() - startedAt;
  const result = JSON.parse(benchmark.result_json());
  process.stdout.write(`${JSON.stringify({ ...result, elapsed_ms: elapsedMs })}\n`);
}

function runColdWasmSuite(wasmModulePath) {
  const resolvedModulePath = path.resolve(wasmModulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const cases = JSON.parse(wasm.shaped_benchmark_cases_json());
  if (cases.length !== 45) {
    throw new Error(`Expected 45 stage-1 hyper benchmark cases, received ${cases.length}`);
  }
  const casesRun = [];
  const allSuccessfulSamples = [];

  for (const benchmarkCase of cases) {
    const samples = Array.from({ length: COLD_WASM_RUNS }, () => {
      const output = runProcess(process.execPath, [
        scriptPath,
        '--wasm-try-single',
        resolvedModulePath,
        benchmarkCase.id,
      ]);
      return JSON.parse(output);
    });
    validateTrySamples(benchmarkCase, samples);
    const timing = summarize(samples);
    casesRun.push({
      id: benchmarkCase.id,
      model: benchmarkCase.model,
      cars: benchmarkCase.cars,
      element: benchmarkCase.element,
      requested_speed_mps: benchmarkCase.requested_speed_mps,
      cold_node_wasm: timing,
    });
    allSuccessfulSamples.push(...samples);
  }

  const worstCaseP95Ms = Math.max(...casesRun.map((item) => item.cold_node_wasm.p95_ms));
  const report = {
    mode: 'cold-node-wasm',
    samples_per_case: COLD_WASM_RUNS,
    scope: 'hyper, 4/6/8 cars, 22/26/30/34/36 m/s, floater/strong-floater/turn',
    timed_operation: 'production solve, calibration previews, corrections, and Construction::append_resolved',
    target_p95_ms: 250,
    all_successful_samples: summarize(allSuccessfulSamples),
    worst_case_cold_p95_ms: worstCaseP95Ms,
    within_target: worstCaseP95Ms <= 250,
    cases: casesRun,
  };
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (!report.within_target) {
    throw new Error(`Cold Node/WASM solve p95 ${worstCaseP95Ms.toFixed(2)} ms exceeded 250 ms`);
  }
}

function validateTrySamples(benchmarkCase, samples) {
  if (samples.length !== COLD_WASM_RUNS) {
    throw new Error(`${benchmarkCase.id} produced ${samples.length} samples`);
  }
  const first = samples[0];
  for (const sample of samples) {
    if (sample.kind !== benchmarkCase.id
      || sample.model !== benchmarkCase.model
      || sample.cars !== benchmarkCase.cars) {
      throw new Error(`Unexpected benchmark case: ${JSON.stringify(sample)}`);
    }
    if (sample.status !== 'solved') {
      throw new Error(`Unexpected solve status: ${JSON.stringify(sample)}`);
    }
    if (typeof sample.construction_append_would_tunnel !== 'boolean') {
      throw new Error(`Construction append was not validated: ${JSON.stringify(sample)}`);
    }
    if (sample.requested_entry_speed_mps !== benchmarkCase.requested_speed_mps
      || Math.abs(sample.observed_entry_speed_mps - benchmarkCase.requested_speed_mps) > 0.01) {
      throw new Error(`Observed entry speed missed target: ${JSON.stringify(sample)}`);
    }
    if (!Number.isFinite(sample.elapsed_ms) || sample.elapsed_ms <= 0) {
      throw new Error(`Invalid timing sample: ${JSON.stringify(sample)}`);
    }
    if (!Number.isInteger(sample.calibration_runs) || sample.calibration_runs > 4) {
      throw new Error(`Solve used more than four previews: ${JSON.stringify(sample)}`);
    }
    if (sample.status !== first.status
      || sample.spec_hash !== first.spec_hash) {
      throw new Error(`${benchmarkCase.id} changed solve outcome between samples`);
    }
  }
}

function runWasmCalibrationSingle(modulePath, kind) {
  const resolvedModulePath = path.resolve(modulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  benchmark.solve();
  benchmark.prepare_calibration();
  const startedAt = performance.now();
  benchmark.calibrate_once();
  const elapsedMs = performance.now() - startedAt;
  const result = JSON.parse(benchmark.result_json());
  result.calibration = JSON.parse(benchmark.calibration_result_json());
  process.stdout.write(`${JSON.stringify({ ...result, elapsed_ms: elapsedMs })}\n`);
}

function runWasmPreviewModesSingle(modulePath, kind) {
  const resolvedModulePath = path.resolve(modulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  benchmark.solve();
  process.stdout.write(`${JSON.stringify(previewModeSample(benchmark, false))}\n`);
}

function runSuite(wasmModulePath, nativeBinaryPath, requestedKinds) {
  const resolvedModulePath = path.resolve(wasmModulePath);
  const requireFromModule = createRequire(resolvedModulePath);
  const wasm = requireFromModule(resolvedModulePath);
  const selectedKinds = requestedKinds.length === 0
    ? KINDS
    : requestedKinds.map((requestedKind) => {
      const match = KINDS.find(({ kind }) => kind === requestedKind);
      if (!match) throw new Error(`Unknown benchmark case: ${requestedKind}`);
      return match;
    });
  const results = selectedKinds.map(({ kind, requestedSpeed }) => {
    const nativeCold = runColdNative(nativeBinaryPath, kind);
    const wasmCold = runColdWasm(wasmModulePath, kind);
    const nativeWarm = runWarmNative(nativeBinaryPath, kind);
    const wasmWarm = runWarmWasm(wasm, kind);
    const nativeCalibrationCold = runColdNativeCalibration(nativeBinaryPath, kind);
    const wasmCalibrationCold = runColdWasmCalibration(wasmModulePath, kind);
    const nativeCalibrationWarm = runWarmNativeCalibration(nativeBinaryPath, kind);
    const wasmCalibrationWarm = runWarmWasmCalibration(wasm, kind);
    const nativePreviewModesCold = runColdNativePreviewModes(nativeBinaryPath, kind);
    const wasmPreviewModesCold = runColdWasmPreviewModes(wasmModulePath, kind);
    const nativePreviewModesWarm = runWarmNativePreviewModes(nativeBinaryPath, kind);
    const wasmPreviewModesWarm = runWarmWasmPreviewModes(wasm, kind);
    validateSamples(kind, requestedSpeed, nativeCold);
    validateSamples(kind, requestedSpeed, wasmCold);
    validateSamples(kind, requestedSpeed, nativeWarm);
    validateSamples(kind, requestedSpeed, wasmWarm);
    validateCalibrationSamples(kind, requestedSpeed, nativeCalibrationCold);
    validateCalibrationSamples(kind, requestedSpeed, wasmCalibrationCold);
    validateCalibrationSamples(kind, requestedSpeed, nativeCalibrationWarm);
    validateCalibrationSamples(kind, requestedSpeed, wasmCalibrationWarm);
    validatePreviewModeSamples(kind, requestedSpeed, nativePreviewModesCold);
    validatePreviewModeSamples(kind, requestedSpeed, wasmPreviewModesCold);
    validatePreviewModeSamples(kind, requestedSpeed, nativePreviewModesWarm);
    validatePreviewModeSamples(kind, requestedSpeed, wasmPreviewModesWarm);
    assertEquivalent(nativeCold[0], wasmCold[0]);
    assertEquivalent(nativePreviewModesCold[0], wasmPreviewModesCold[0]);
    return {
      kind,
      requested_speed_mps: requestedSpeed,
      native: {
        cold: summarize(nativeCold),
        warm: summarize(nativeWarm),
      },
      wasm_node: {
        cold: summarize(wasmCold),
        warm: summarize(wasmWarm),
      },
      one_calibration: {
        native: {
          cold: summarize(nativeCalibrationCold),
          warm: summarize(nativeCalibrationWarm),
        },
        wasm_node: {
          cold: summarize(wasmCalibrationCold),
          warm: summarize(wasmCalibrationWarm),
        },
        full_train_cars: wasmCalibrationCold[0].calibration.candidate_cars,
      },
      preview_mode_comparison: {
        native: {
          front_car: {
            cold: summarizePreviewMode(nativePreviewModesCold, 'front_car'),
            warm: summarizePreviewMode(nativePreviewModesWarm, 'front_car'),
          },
          all_cars: {
            cold: summarizePreviewMode(nativePreviewModesCold, 'all_cars'),
            warm: summarizePreviewMode(nativePreviewModesWarm, 'all_cars'),
          },
        },
        wasm_node: {
          front_car: {
            cold: summarizePreviewMode(wasmPreviewModesCold, 'front_car'),
            warm: summarizePreviewMode(wasmPreviewModesWarm, 'front_car'),
          },
          all_cars: {
            cold: summarizePreviewMode(wasmPreviewModesCold, 'all_cars'),
            warm: summarizePreviewMode(wasmPreviewModesWarm, 'all_cars'),
          },
        },
        recorded_cars: wasmPreviewModesCold[0].preview_modes,
      },
      observed_entry_speed_mps: wasmCold[0].observed_entry_speed_mps,
      calibration_runs: wasmCold[0].calibration_runs,
      resolved: resolvedFields(wasmCold[0]),
    };
  });
  process.stdout.write(`${JSON.stringify({ cold_process_samples: COLD_RUNS, warm_samples: 30, results }, null, 2)}\n`);
}

function runColdNative(nativeBinaryPath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(nativeBinaryPath, ['--single', kind]);
    return JSON.parse(output);
  });
}

function runColdWasm(wasmModulePath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(process.execPath, [scriptPath, '--wasm-single', wasmModulePath, kind]);
    return JSON.parse(output);
  });
}

function runWarmNative(nativeBinaryPath, kind) {
  const output = runProcess(nativeBinaryPath, ['--warm', kind]);
  return JSON.parse(output).samples;
}

function runColdNativeCalibration(nativeBinaryPath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(nativeBinaryPath, ['--single-calibration', kind]);
    return JSON.parse(output);
  });
}

function runColdWasmCalibration(wasmModulePath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(process.execPath, [scriptPath, '--wasm-calibration-single', wasmModulePath, kind]);
    return JSON.parse(output);
  });
}

function runColdNativePreviewModes(nativeBinaryPath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(nativeBinaryPath, ['--single-preview-modes', kind]);
    return JSON.parse(output);
  });
}

function runColdWasmPreviewModes(wasmModulePath, kind) {
  return Array.from({ length: COLD_RUNS }, () => {
    const output = runProcess(process.execPath, [scriptPath, '--wasm-preview-modes', wasmModulePath, kind]);
    return JSON.parse(output);
  });
}

function runWarmWasm(wasm, kind) {
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  for (let index = 0; index < 3; index += 1) {
    benchmark.prepare();
    benchmark.solve();
  }
  const samples = [];
  for (let index = 0; index < 30; index += 1) {
    benchmark.prepare();
    const startedAt = performance.now();
    benchmark.solve();
    const elapsedMs = performance.now() - startedAt;
    const result = JSON.parse(benchmark.result_json());
    samples.push({ ...result, elapsed_ms: elapsedMs });
  }
  return samples;
}

function runWarmNativeCalibration(nativeBinaryPath, kind) {
  const output = runProcess(nativeBinaryPath, ['--warm-calibration', kind]);
  return JSON.parse(output).samples;
}

function runWarmWasmCalibration(wasm, kind) {
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  benchmark.solve();
  for (let index = 0; index < 3; index += 1) {
    benchmark.prepare_calibration();
    benchmark.calibrate_once();
  }
  const samples = [];
  for (let index = 0; index < 30; index += 1) {
    benchmark.prepare_calibration();
    const startedAt = performance.now();
    benchmark.calibrate_once();
    const elapsedMs = performance.now() - startedAt;
    const result = JSON.parse(benchmark.result_json());
    result.calibration = JSON.parse(benchmark.calibration_result_json());
    samples.push({ ...result, elapsed_ms: elapsedMs });
  }
  return samples;
}

function runWarmNativePreviewModes(nativeBinaryPath, kind) {
  const output = runProcess(nativeBinaryPath, ['--warm-preview-modes', kind]);
  return JSON.parse(output).samples;
}

function runWarmWasmPreviewModes(wasm, kind) {
  const benchmark = new wasm.ShapedSolveBenchmark(kind);
  benchmark.prepare();
  benchmark.solve();
  for (let index = 0; index < 3; index += 1) {
    previewModeSample(benchmark, index % 2 === 1);
  }
  return Array.from({ length: 30 }, (_, index) =>
    previewModeSample(benchmark, index % 2 === 1));
}

function previewModeSample(benchmark, allCarsFirst) {
  benchmark.prepare_preview_modes();
  let frontCarMs;
  let allCarsMs;
  if (allCarsFirst) {
    let startedAt = performance.now();
    benchmark.preview_all_cars_once();
    allCarsMs = performance.now() - startedAt;
    startedAt = performance.now();
    benchmark.preview_front_car_once();
    frontCarMs = performance.now() - startedAt;
  } else {
    let startedAt = performance.now();
    benchmark.preview_front_car_once();
    frontCarMs = performance.now() - startedAt;
    startedAt = performance.now();
    benchmark.preview_all_cars_once();
    allCarsMs = performance.now() - startedAt;
  }
  const result = JSON.parse(benchmark.result_json());
  result.preview_modes = JSON.parse(benchmark.preview_modes_result_json());
  result.preview_mode_elapsed_ms = { front_car: frontCarMs, all_cars: allCarsMs };
  result.elapsed_ms = frontCarMs + allCarsMs;
  return result;
}

function runProcess(command, arguments_) {
  const result = spawnSync(command, arguments_, { encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} ${arguments_.join(' ')} exited ${result.status}: ${result.stderr}`);
  }
  const output = result.stdout.trim();
  if (!output) throw new Error(`${command} ${arguments_.join(' ')} produced no benchmark result`);
  return output;
}

function validateSamples(kind, requestedSpeed, samples) {
  if (samples.length === 0) throw new Error(`${kind} produced no samples`);
  const first = samples[0];
  for (const sample of samples) {
    if (sample.kind !== kind || sample.model !== 'hyper') {
      throw new Error(`Unexpected benchmark case: ${JSON.stringify(sample)}`);
    }
    if (sample.requested_entry_speed_mps !== requestedSpeed) {
      throw new Error(`Requested speed changed: ${JSON.stringify(sample)}`);
    }
    if (Math.abs(sample.observed_entry_speed_mps - requestedSpeed) > 0.01) {
      throw new Error(`Observed entry speed missed target: ${JSON.stringify(sample)}`);
    }
    if (Math.abs(sample.design_speed_mps - sample.observed_entry_speed_mps) > 1e-9) {
      throw new Error(`Solver did not use observed entry speed: ${JSON.stringify(sample)}`);
    }
    if (sample.calibration_runs < 1 || sample.calibration_runs > 3) {
      throw new Error(`Unexpected calibration count: ${JSON.stringify(sample)}`);
    }
    if (sample.spec_hash !== first.spec_hash || sample.calibration_runs !== first.calibration_runs) {
      throw new Error(`${kind} changed its resolved result between samples`);
    }
    if (!Number.isFinite(sample.elapsed_ms) || sample.elapsed_ms <= 0) {
      throw new Error(`Invalid timing sample: ${JSON.stringify(sample)}`);
    }
  }
}

function validateCalibrationSamples(kind, requestedSpeed, samples) {
  validateSamples(kind, requestedSpeed, samples);
  for (const sample of samples) {
    if (sample.calibration.candidate_cars !== sample.calibration.expected_cars
      || sample.calibration.within_limits_cars !== sample.calibration.expected_cars) {
      throw new Error(`Calibration did not include every car: ${JSON.stringify(sample)}`);
    }
  }
}

function validatePreviewModeSamples(kind, requestedSpeed, samples) {
  validateSamples(kind, requestedSpeed, samples);
  for (const sample of samples) {
    if (sample.preview_modes.front_preview_cars !== 1
      || sample.preview_modes.all_cars_preview_cars !== sample.preview_modes.expected_cars) {
      throw new Error(`Preview modes recorded unexpected cars: ${JSON.stringify(sample)}`);
    }
    for (const elapsedMs of Object.values(sample.preview_mode_elapsed_ms)) {
      if (!Number.isFinite(elapsedMs) || elapsedMs <= 0) {
        throw new Error(`Invalid preview mode timing: ${JSON.stringify(sample)}`);
      }
    }
  }
}

function assertEquivalent(nativeResult, wasmResult) {
  const keys = [
    'requested_entry_speed_mps',
    'observed_entry_speed_mps',
    'design_speed_mps',
    'target_g',
    'delivered_g',
    'height_m',
    'span_m',
    'vertical_g',
    'lateral_g',
    'radius_m',
    'bank_degrees',
  ];
  for (const key of keys) {
    if (key in nativeResult && key in wasmResult
      && Math.abs(nativeResult[key] - wasmResult[key]) > 0.02) {
      throw new Error(`Native/WASM ${key} differs: ${nativeResult[key]} vs ${wasmResult[key]}`);
    }
  }
}

function resolvedFields(result) {
  const fields = {};
  for (const key of [
    'target_g',
    'delivered_g',
    'height_m',
    'span_m',
    'vertical_g',
    'lateral_g',
    'radius_m',
    'bank_degrees',
  ]) {
    if (key in result) fields[key] = result[key];
  }
  return fields;
}

function summarize(samples) {
  const elapsedMs = samples.map((sample) => sample.elapsed_ms).sort((left, right) => left - right);
  const middle = elapsedMs.length >> 1;
  const medianMs = elapsedMs.length % 2 === 0
    ? (elapsedMs[middle - 1] + elapsedMs[middle]) / 2
    : elapsedMs[middle];
  return {
    samples: elapsedMs.length,
    median_ms: medianMs,
    p95_ms: elapsedMs[Math.ceil(elapsedMs.length * 0.95) - 1],
    max_ms: elapsedMs.at(-1),
  };
}

function summarizePreviewMode(samples, mode) {
  return summarize(samples.map((sample) => ({
    elapsed_ms: sample.preview_mode_elapsed_ms[mode],
  })));
}
