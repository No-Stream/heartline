use std::hint::black_box;
use std::time::Instant;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::physics::{CoasterRun, PhysicsConfig, TIME_STEP, Train, demo_circuit};
use theme_park_sim::track::hybrid_model;
use theme_park_sim::track::proximity::{TrainPosition, scan_proximity};

const PHYSICS_STEPS: usize = 40_000;
const SCANS: usize = 10;
const REPORTS: usize = 1000;
const GAME_TICK_SECONDS: f64 = 1.0 / 30.0;

fn main() {
    let world = demo_world();
    let circuit = demo_circuit();
    let configuration = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };
    let mut samples = Vec::new();
    let mut times = Vec::new();
    let mut runs_with_other_train = 0;
    for include_other in [false, true] {
        let mut run = CoasterRun::new(&circuit, &configuration, config);
        let mut other = Train::new(&circuit, &configuration, config, 0.0, 2.0);
        let mut completed_runs = 0;
        let mut dueling_events = 0;
        let start = Instant::now();
        for _ in 0..PHYSICS_STEPS {
            other.step(&circuit);
            let other_positions: Vec<_> = other
                .readings()
                .last()
                .expect("step produces readings")
                .iter()
                .map(|reading| {
                    let sample = circuit.sample(reading.distance);
                    TrainPosition {
                        position: sample.position,
                        tangent: sample.tangent,
                        piece_index: circuit.piece_index(reading.distance),
                        distance: reading.distance.rem_euclid(circuit.length()),
                        speed: reading.speed,
                    }
                })
                .collect();
            other.take_readings();
            let other_trains = if include_other {
                other_positions.as_slice()
            } else {
                &[]
            };
            if let Some(outcome) = black_box(run.step_with_other_trains(
                &circuit,
                black_box(other_trains),
                "Timber Tumbler",
            )) {
                completed_runs += 1;
                dueling_events += outcome
                    .reports
                    .iter()
                    .map(|report| report.proximity_events.len())
                    .sum::<usize>();
                if !outcome.reports[0].samples.is_empty() {
                    samples = outcome.reports[0].samples.clone();
                }
                run = CoasterRun::new(&circuit, &configuration, config);
            }
        }
        if include_other {
            assert!(dueling_events > 0, "benchmark must record a dueling event");
            runs_with_other_train = completed_runs;
        }
        let elapsed = start.elapsed().as_secs_f64();
        times.push(elapsed / PHYSICS_STEPS as f64 * GAME_TICK_SECONDS / TIME_STEP * 1000.0);
        println!(
            "{}: {:.3} ms/30 Hz tick, {completed_runs} finished runs, {dueling_events} dueling events",
            if include_other {
                "One other train (all cars)"
            } else {
                "No other trains"
            },
            times.last().unwrap()
        );
    }
    assert!(!samples.is_empty(), "benchmark needs recorded ride samples");
    println!(
        "Incremental other-train checks: {:+.3} ms/30 Hz tick (baseline includes per-car detection setup)",
        times[1] - times[0]
    );
    let start = Instant::now();
    for _ in 0..SCANS {
        black_box(scan_proximity(
            &world,
            circuit.pieces(),
            &configuration,
            circuit.pieces(),
            &[],
        ));
    }
    let scan_ms = start.elapsed().as_secs_f64() / SCANS as f64 * 1000.0;
    let geometry = scan_proximity(
        &world,
        circuit.pieces(),
        &configuration,
        circuit.pieces(),
        &[],
    );
    let start = Instant::now();
    let mut event_count = 0;
    for _ in 0..REPORTS {
        event_count += black_box(geometry.events_for_run(black_box(&samples))).len();
    }
    let report_ms = start.elapsed().as_secs_f64() / REPORTS as f64 * 1000.0;
    let reports_per_run = usize::from(configuration.cars);
    let measured_ticks = PHYSICS_STEPS as f64 * TIME_STEP / GAME_TICK_SECONDS;
    let total_ms = scan_ms + report_ms * reports_per_run as f64;
    println!(
        "Static scan: {} track pieces, {} overlapping other-track pieces, {:.3} ms/scan",
        circuit.pieces().len(),
        circuit.pieces().len(),
        scan_ms
    );
    println!(
        "Per-car report: {} speed samples, {:.3} ms/report, {} events/report; {reports_per_run} cars",
        samples.len(),
        report_ms,
        event_count / REPORTS
    );
    println!(
        "One scan and {reports_per_run} reports: {:.3} ms at completion, {:.3} ms/30 Hz tick amortized over {runs_with_other_train} runs in {measured_ticks:.0} ticks",
        total_ms,
        total_ms * runs_with_other_train as f64 / measured_ticks
    );
}
