//! Times the finish-the-circuit search on content rides with their last steps removed.
//!
//! Each case replays the ride without its last `removed` ride steps (the close step counts as
//! one) and runs the search once to warm the builder's force preview of the open track, as the
//! builder has it while a player builds. It then times a second search and prints the ranked
//! options with their arrival speeds, or the reason no finish exists.
//!
//! Usage: `finish_search_bench <ride name> <removed steps> [<removed steps> ...]`, for example
//! `finish_search_bench horizon-hawk 2 3 4 5 6`.

use std::time::Instant;

use theme_park_sim::track::ride_file::RideFile;
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::BuilderSession;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let ride_name = arguments.next().expect("expected a content ride name");
    let removed_step_counts: Vec<usize> = arguments
        .map(|argument| argument.parse().expect("removed steps is a whole number"))
        .collect();
    assert!(
        !removed_step_counts.is_empty(),
        "expected at least one removed step count"
    );
    let path = format!(
        "{}/../../content/rides/{ride_name}.ride.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let ride = RideFile::from_json_slice(&source).expect("content ride parses");

    for removed_steps in removed_step_counts {
        let mut truncated = ride.clone();
        truncated.steps.truncate(ride.steps.len() - removed_steps);
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session
            .replay_ride(&truncated)
            .expect("truncated ride replays");
        session.search_finish();

        let started_at = Instant::now();
        session.search_finish();
        let elapsed = started_at.elapsed();

        let simulation = session.simulation();
        println!(
            "{ride_name} without its last {removed_steps} steps: search {:.0} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        if !simulation.construction_can_finish() {
            println!("  no finish: {}", simulation.construction_finish_reason());
            continue;
        }
        for completion in simulation.construction_finish_completions() {
            let physics = &completion.physics;
            let pieces: Vec<String> = physics
                .pieces
                .iter()
                .map(|piece| {
                    format!(
                        "{:?} ({:.1} m/s, {:+.2} g vertical, {:.2} g lateral)",
                        piece.id,
                        piece.entry_speed_mps,
                        piece.worst_car_peaks.maximum_vertical_g,
                        piece.worst_car_peaks.maximum_lateral_g
                    )
                })
                .collect();
            println!(
                "  {}. arrives at {:.1} m/s, brakes remove {:.0} kJ, headroom {:+.2} g: {}",
                completion.score,
                physics.arrival_speed_mps,
                physics.brake_energy_kj,
                physics.minimum_force_headroom_g,
                pieces.join(", ")
            );
        }
    }
}
