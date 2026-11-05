use std::hint::black_box;
use std::time::{Duration, Instant};

use theme_park_sim_wasm::WasmSimulation;

const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");
const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 100;

fn main() {
    for (name, model_id) in [
        ("Timber Tumbler", "model:hybrid"),
        ("Velocity Viper", "model:velocity_viper"),
        ("Briar Blitz", "model:wooden_twister"),
    ] {
        let mut simulation = build_player_circuit(model_id, name);
        measure_preview(&mut simulation, name);
    }
}

fn build_player_circuit(model_id: &str, name: &str) -> WasmSimulation {
    let mut simulation = WasmSimulation::new(42, 0, "new".into());
    simulation.construction_select(model_id);
    simulation.construction_start(482, 498, 6, 2);

    let header = format!("[{name}]");
    let mut sections = CIRCUITS.split(&header);
    sections.next().unwrap();
    let section = sections
        .next()
        .unwrap_or_else(|| panic!("{name} circuit missing"));
    assert!(sections.next().is_none(), "duplicate {name} circuit");
    let mut piece_count = 1;
    for piece_name in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        match piece_name {
            "+chain" => {
                simulation.construction_set_chain(true);
                continue;
            }
            "-chain" => {
                simulation.construction_set_chain(false);
                continue;
            }
            _ => {}
        }
        simulation.construction_select(piece_name);
        simulation.construction_append();
        let track: serde_json::Value =
            serde_json::from_str(&simulation.construction_track()).unwrap();
        piece_count += 1;
        assert_eq!(track.as_array().unwrap().len(), piece_count);
    }
    let track: serde_json::Value = serde_json::from_str(&simulation.construction_track()).unwrap();
    let expected_piece_count = section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| {
            !line.is_empty() && !line.starts_with('#') && *line != "+chain" && *line != "-chain"
        })
        .count()
        + 1;
    assert_eq!(track.as_array().unwrap().len(), expected_piece_count);
    assert!(simulation.construction_active());
    simulation
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
