use std::hint::black_box;
use std::time::{Duration, Instant};

use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim_wasm::WasmSimulation;

const TIMBER_TUMBLER: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");
const VELOCITY_VIPER: &str = include_str!("../../../content/rides/velocity-viper.ride.json");
const BRIAR_BLITZ: &str = include_str!("../../../content/rides/briar-blitz.ride.json");
const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 100;

fn main() {
    let circuits = [
        ("Timber Tumbler", "model:hybrid", "Loop", 21),
        ("Velocity Viper", "model:velocity_viper", "Loop", 9),
        ("Briar Blitz", "model:wooden_twister", "SteelLoop", 11),
    ];
    for (name, model_id, _, _) in circuits {
        measure_append_previews(name, model_id);
    }
    if std::env::args().any(|argument| argument == "--append-only") {
        return;
    }

    for (name, model_id, candidate, prefix_length) in circuits {
        let mut complete = build_player_circuit(model_id, name, None);
        measure_preview(&mut complete, name);
        let mut simulation = build_player_circuit(model_id, name, Some(prefix_length));
        println!("{name}: measuring added work after {prefix_length} circuit pieces plus station");
        let preview: serde_json::Value =
            serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
        let entry_speed = (preview["status"] == "reachedEnd").then(|| {
            preview["samples"].as_array().unwrap().last().unwrap()["speed"]
                .as_f64()
                .unwrap()
        });
        let speed_match: serde_json::Value =
            serde_json::from_str(&simulation.construction_speed_match_json(candidate, entry_speed))
                .unwrap();
        let requested_candidate = speed_match["recommendedId"].as_str().unwrap_or(candidate);
        let available_choices: serde_json::Value =
            serde_json::from_str(&simulation.construction_choices()).unwrap();
        let is_available = |id: &str| {
            available_choices.as_array().is_some_and(|choices| {
                choices
                    .iter()
                    .any(|choice| choice["id"] == id && choice["refusal"] == "")
            })
        };
        let selected = if is_available(requested_candidate) {
            requested_candidate.to_owned()
        } else {
            available_choices
                .as_array()
                .and_then(|choices| {
                    choices
                        .iter()
                        .find(|choice| choice["refusal"] == "")
                        .and_then(|choice| choice["id"].as_str())
                })
                .unwrap_or_else(|| {
                    panic!("{name} has no valid candidate after the measured prefix")
                })
                .to_owned()
        };
        if selected != requested_candidate {
            println!(
                "{name}: speed-matched candidate {requested_candidate} is unavailable after the prefix; measuring available {selected} geometry"
            );
        }
        simulation.construction_select(&selected);
        assert!(
            simulation.construction_preview_valid(),
            "{name} {selected} must be a valid candidate at the measured prefix"
        );
        measure_candidate_preview(&simulation, name, &selected);
        measure_speed_match(&simulation, name, candidate, entry_speed);
        measure_added_action_work(&simulation, name, candidate, entry_speed, 1, "placement");
        measure_added_action_work(&simulation, name, candidate, entry_speed, 2, "selection");
    }
}

fn measure_append_previews(name: &str, model_id: &str) {
    let mut simulation = WasmSimulation::new(42, 0, "new".into());
    simulation.construction_select(model_id);
    simulation.construction_start(482, 498, 6, 2);

    let ride = ride_file(name);
    let mut append_latencies = Vec::new();
    let mut placed_preview_latencies = Vec::new();
    let mut candidate_preview_latencies = Vec::new();
    let mut combined_preview_latencies = Vec::new();
    let mut append_update_latencies = Vec::new();
    let mut piece_count = 1;

    for step in &ride.steps {
        let piece_name = match step {
            RideStep::Chain(step) => {
                simulation.construction_set_chain(step.chain);
                black_box(simulation.construction_force_preview_json());
                black_box(simulation.construction_candidate_force_preview_json());
                continue;
            }
            RideStep::Place(step) => step.place.as_str(),
            RideStep::Close(step) => {
                assert!(step.close, "{name} has a false close step");
                continue;
            }
            _ => panic!("{name} benchmark does not support this ride step"),
        };

        simulation.construction_select(piece_name);
        black_box(simulation.construction_force_preview_json());
        black_box(simulation.construction_candidate_force_preview_json());

        let update_started_at = Instant::now();
        let append_started_at = Instant::now();
        simulation.construction_append();
        let append_elapsed = append_started_at.elapsed();

        let placed_preview_started_at = Instant::now();
        let placed_preview = simulation.construction_force_preview_json();
        let placed_preview_elapsed = placed_preview_started_at.elapsed();

        let candidate_preview_started_at = Instant::now();
        let candidate_preview = simulation.construction_candidate_force_preview_json();
        let candidate_preview_elapsed = candidate_preview_started_at.elapsed();
        let update_elapsed = update_started_at.elapsed();

        piece_count += 1;
        let track: serde_json::Value =
            serde_json::from_str(&simulation.construction_track()).unwrap();
        assert_eq!(track.as_array().unwrap().len(), piece_count);
        assert!(
            !serde_json::from_str::<serde_json::Value>(&placed_preview)
                .unwrap()
                .is_null()
        );
        assert!(
            !serde_json::from_str::<serde_json::Value>(&candidate_preview)
                .unwrap()
                .is_null()
        );
        black_box((placed_preview, candidate_preview));
        append_latencies.push(append_elapsed);
        placed_preview_latencies.push(placed_preview_elapsed);
        candidate_preview_latencies.push(candidate_preview_elapsed);
        combined_preview_latencies.push(placed_preview_elapsed + candidate_preview_elapsed);
        append_update_latencies.push(update_elapsed);
    }

    assert!(
        !append_latencies.is_empty(),
        "{name} circuit has no append actions"
    );
    append_latencies.sort_unstable();
    placed_preview_latencies.sort_unstable();
    candidate_preview_latencies.sort_unstable();
    combined_preview_latencies.sort_unstable();
    append_update_latencies.sort_unstable();
    println!(
        "{name}: {} appends; construction p95 {:.3} ms; placed preview p95 {:.3} ms; candidate preview p95 {:.3} ms; both previews p95 {:.3} ms; append update p95 {:.3} ms, max {:.3} ms",
        append_latencies.len(),
        milliseconds(percentile_95(&append_latencies)),
        milliseconds(percentile_95(&placed_preview_latencies)),
        milliseconds(percentile_95(&candidate_preview_latencies)),
        milliseconds(percentile_95(&combined_preview_latencies)),
        milliseconds(percentile_95(&append_update_latencies)),
        milliseconds(append_update_latencies[append_update_latencies.len() - 1]),
    );
}

fn percentile_95(sorted_elapsed: &[Duration]) -> Duration {
    sorted_elapsed[(sorted_elapsed.len() * 95).div_ceil(100) - 1]
}

fn measure_added_action_work(
    simulation: &WasmSimulation,
    name: &str,
    candidate: &str,
    entry_speed: Option<f64>,
    match_calls: usize,
    action: &str,
) {
    let run = || {
        for _ in 0..match_calls {
            black_box(simulation.construction_speed_match_json(candidate, entry_speed));
        }
        black_box(simulation.construction_candidate_force_preview_json());
    };
    for _ in 0..WARMUP_RUNS {
        run();
    }
    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let started_at = Instant::now();
        run();
        elapsed.push(started_at.elapsed());
    }
    elapsed.sort_unstable();
    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    println!(
        "{name}: added {action} work median {:.3} ms, p95 {:.3} ms",
        milliseconds(median),
        milliseconds(p95)
    );
}

fn measure_speed_match(
    simulation: &WasmSimulation,
    name: &str,
    candidate: &str,
    entry_speed: Option<f64>,
) {
    let expected = simulation.construction_speed_match_json(candidate, entry_speed);
    let preview: serde_json::Value = serde_json::from_str(&expected).unwrap();
    assert!(
        !preview.is_null(),
        "{name} {candidate} has no available rated candidate"
    );
    for _ in 1..WARMUP_RUNS {
        assert_eq!(
            simulation.construction_speed_match_json(candidate, entry_speed),
            expected
        );
    }
    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let started_at = Instant::now();
        let result = simulation.construction_speed_match_json(candidate, entry_speed);
        elapsed.push(started_at.elapsed());
        assert_eq!(result, expected);
        black_box(result);
    }
    elapsed.sort_unstable();
    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    println!(
        "{name}: {candidate} speed match median {:.3} ms, p95 {:.3} ms",
        milliseconds(median),
        milliseconds(p95)
    );
}

fn measure_candidate_preview(simulation: &WasmSimulation, name: &str, candidate: &str) {
    let expected = simulation.construction_candidate_force_preview_json();
    let preview: serde_json::Value = serde_json::from_str(&expected).unwrap();
    assert!(
        !preview.is_null(),
        "{name} {candidate} has no candidate geometry"
    );
    for _ in 1..WARMUP_RUNS {
        assert_eq!(
            simulation.construction_candidate_force_preview_json(),
            expected
        );
    }
    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let started_at = Instant::now();
        let result = simulation.construction_candidate_force_preview_json();
        elapsed.push(started_at.elapsed());
        assert_eq!(result, expected);
        black_box(result);
    }
    elapsed.sort_unstable();
    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    println!(
        "{name}: {candidate} candidate endpoint ({}) median {:.3} ms, p95 {:.3} ms",
        preview["status"].as_str().unwrap(),
        milliseconds(median),
        milliseconds(p95)
    );
}

fn build_player_circuit(
    model_id: &str,
    name: &str,
    prefix_length: Option<usize>,
) -> WasmSimulation {
    let mut simulation = WasmSimulation::new(42, 0, "new".into());
    simulation.construction_select(model_id);
    simulation.construction_start(482, 498, 6, 2);

    let ride = ride_file(name);
    let mut piece_count = 1;
    for step in &ride.steps {
        let piece_name = match step {
            RideStep::Chain(step) => {
                simulation.construction_set_chain(step.chain);
                continue;
            }
            RideStep::Place(step) => step.place.as_str(),
            RideStep::Close(step) => {
                assert!(step.close, "{name} has a false close step");
                continue;
            }
            _ => panic!("{name} benchmark does not support this ride step"),
        };
        simulation.construction_select(piece_name);
        simulation.construction_append();
        let track: serde_json::Value =
            serde_json::from_str(&simulation.construction_track()).unwrap();
        piece_count += 1;
        assert_eq!(track.as_array().unwrap().len(), piece_count);
        if prefix_length.is_some_and(|length| piece_count == length + 1) {
            break;
        }
    }
    let track: serde_json::Value = serde_json::from_str(&simulation.construction_track()).unwrap();
    let expected_piece_count = prefix_length.unwrap_or_else(|| {
        ride.steps
            .iter()
            .filter(|step| matches!(step, RideStep::Place(_)))
            .count()
    }) + 1;
    assert_eq!(track.as_array().unwrap().len(), expected_piece_count);
    assert!(simulation.construction_active());
    simulation
}

fn ride_file(name: &str) -> RideFile {
    let json = match name {
        "Timber Tumbler" => TIMBER_TUMBLER,
        "Velocity Viper" => VELOCITY_VIPER,
        "Briar Blitz" => BRIAR_BLITZ,
        _ => panic!("unknown benchmark ride {name}"),
    };
    serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid ride file for {name}: {error}"))
}

fn measure_preview(simulation: &mut WasmSimulation, name: &str) {
    let first_preview: serde_json::Value =
        serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
    let status = first_preview["status"].as_str().unwrap().to_owned();
    let sample_count = first_preview["samples"].as_array().unwrap().len();
    let furthest_distance_metres =
        first_preview["samples"].as_array().unwrap().last().unwrap()["distance"]
            .as_f64()
            .unwrap();
    black_box(first_preview);
    for _ in 1..WARMUP_RUNS {
        let preview: serde_json::Value =
            serde_json::from_str(&simulation.construction_force_preview_json()).unwrap();
        assert_eq!(preview["status"], status);
        assert_eq!(preview["samples"].as_array().unwrap().len(), sample_count);
        assert_eq!(
            preview["samples"].as_array().unwrap().last().unwrap()["distance"],
            furthest_distance_metres
        );
        black_box(preview);
    }

    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let started_at = Instant::now();
        let preview_json = simulation.construction_force_preview_json();
        let duration = started_at.elapsed();
        let preview: serde_json::Value = serde_json::from_str(&preview_json).unwrap();
        assert_eq!(preview["status"], status);
        assert_eq!(preview["samples"].as_array().unwrap().len(), sample_count);
        black_box((preview_json, preview));
        elapsed.push(duration);
    }
    elapsed.sort_unstable();

    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    let maximum = elapsed[MEASURED_RUNS - 1];
    println!(
        "{name}: {status} at {:.1} m with {sample_count} samples; JSON endpoint median {:.3} ms, p95 {:.3} ms, max {:.3} ms",
        furthest_distance_metres,
        milliseconds(median),
        milliseconds(p95),
        milliseconds(maximum),
    );
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
