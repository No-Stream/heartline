use std::hint::black_box;
use std::time::Instant;

use serde_json::{Value, json};
use theme_park_sim_wasm::shaped_benchmark::ShapedSolveBenchmark;

const WARMUP_RUNS: usize = 3;
const MEASURED_RUNS: usize = 30;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments
        .next()
        .expect("expected a single-solve or calibration mode");
    let kind = arguments.next().expect("expected shaped benchmark case");
    assert!(arguments.next().is_none(), "unexpected benchmark arguments");

    match mode.as_str() {
        "--single" => measure_single(&kind),
        "--warm" => measure_warm(&kind),
        "--single-calibration" => measure_single_calibration(&kind),
        "--warm-calibration" => measure_warm_calibration(&kind),
        "--single-preview-modes" => measure_single_preview_modes(&kind),
        "--warm-preview-modes" => measure_warm_preview_modes(&kind),
        _ => panic!("unknown benchmark mode: {mode}"),
    }
}

fn measure_single(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    benchmark.prepare();
    let started_at = Instant::now();
    black_box(benchmark.solve());
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
    let mut result: Value =
        serde_json::from_str(&benchmark.result_json()).expect("benchmark result is JSON");
    result["elapsed_ms"] = json!(elapsed_ms);
    println!(
        "{}",
        serde_json::to_string(&result).expect("result serializes")
    );
}

fn measure_warm(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    for _ in 0..WARMUP_RUNS {
        benchmark.prepare();
        black_box(benchmark.solve());
    }

    let mut samples = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        benchmark.prepare();
        let started_at = Instant::now();
        black_box(benchmark.solve());
        let elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
        let mut result: Value =
            serde_json::from_str(&benchmark.result_json()).expect("benchmark result is JSON");
        result["elapsed_ms"] = json!(elapsed_ms);
        samples.push(result);
    }
    println!(
        "{}",
        serde_json::to_string(&json!({"mode": "warm", "samples": samples}))
            .expect("warm samples serialize")
    );
}

fn measure_single_calibration(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    benchmark.prepare();
    benchmark.solve();
    benchmark.prepare_calibration();
    let started_at = Instant::now();
    black_box(benchmark.calibrate_once());
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
    let mut result: Value =
        serde_json::from_str(&benchmark.result_json()).expect("benchmark result is JSON");
    result["calibration"] = serde_json::from_str(&benchmark.calibration_result_json())
        .expect("calibration result is JSON");
    result["elapsed_ms"] = json!(elapsed_ms);
    println!(
        "{}",
        serde_json::to_string(&result).expect("result serializes")
    );
}

fn measure_warm_calibration(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    benchmark.prepare();
    benchmark.solve();
    for _ in 0..WARMUP_RUNS {
        benchmark.prepare_calibration();
        black_box(benchmark.calibrate_once());
    }

    let mut samples = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        benchmark.prepare_calibration();
        let started_at = Instant::now();
        black_box(benchmark.calibrate_once());
        let elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
        let mut result: Value =
            serde_json::from_str(&benchmark.result_json()).expect("benchmark result is JSON");
        result["calibration"] = serde_json::from_str(&benchmark.calibration_result_json())
            .expect("calibration result is JSON");
        result["elapsed_ms"] = json!(elapsed_ms);
        samples.push(result);
    }
    println!(
        "{}",
        serde_json::to_string(&json!({"mode": "warm-calibration", "samples": samples}))
            .expect("warm calibration samples serialize")
    );
}

fn measure_single_preview_modes(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    benchmark.prepare();
    benchmark.solve();
    let sample = preview_mode_sample(&mut benchmark, false);
    println!(
        "{}",
        serde_json::to_string(&sample).expect("preview mode sample serializes")
    );
}

fn measure_warm_preview_modes(kind: &str) {
    let mut benchmark = ShapedSolveBenchmark::new(kind);
    benchmark.prepare();
    benchmark.solve();
    for index in 0..WARMUP_RUNS {
        black_box(preview_mode_sample(&mut benchmark, index % 2 == 1));
    }

    let samples: Vec<_> = (0..MEASURED_RUNS)
        .map(|index| preview_mode_sample(&mut benchmark, index % 2 == 1))
        .collect();
    println!(
        "{}",
        serde_json::to_string(&json!({"mode": "warm-preview-modes", "samples": samples}))
            .expect("warm preview mode samples serialize")
    );
}

fn preview_mode_sample(benchmark: &mut ShapedSolveBenchmark, all_cars_first: bool) -> Value {
    benchmark.prepare_preview_modes();
    let (front_car_ms, all_cars_ms) = if all_cars_first {
        let started_at = Instant::now();
        black_box(benchmark.preview_all_cars_once());
        let all_cars_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
        let started_at = Instant::now();
        black_box(benchmark.preview_front_car_once());
        (started_at.elapsed().as_secs_f64() * 1_000.0, all_cars_ms)
    } else {
        let started_at = Instant::now();
        black_box(benchmark.preview_front_car_once());
        let front_car_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
        let started_at = Instant::now();
        black_box(benchmark.preview_all_cars_once());
        (front_car_ms, started_at.elapsed().as_secs_f64() * 1_000.0)
    };
    let mut result: Value =
        serde_json::from_str(&benchmark.result_json()).expect("benchmark result is JSON");
    result["preview_modes"] = serde_json::from_str(&benchmark.preview_modes_result_json())
        .expect("preview mode result is JSON");
    result["preview_mode_elapsed_ms"] = json!({
        "front_car": front_car_ms,
        "all_cars": all_cars_ms,
    });
    result["elapsed_ms"] = json!(front_car_ms + all_cars_ms);
    result
}
