//! Replays the worker's call sequence for every piece placed on the three journey
//! circuits and reports where each placement update spends its time.
//!
//! The sequence mirrors `web/src/worker.ts`: the piece is selected with a speed
//! match and a full construction update, then appended and followed by a partial
//! construction update, a world snapshot and a full construction update.
//!
//! With `--digest`, every call's result is hashed instead of timed, and the run prints
//! one digest per circuit, so two builds can be checked for identical builder output.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::time::{Duration, Instant};

use theme_park_sim_wasm::WasmSimulation;

const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");

#[derive(Default)]
struct Timings {
    by_call: BTreeMap<&'static str, Vec<Duration>>,
    selection_updates: Vec<Duration>,
    placement_updates: Vec<Duration>,
    digest: Option<DefaultHasher>,
}

impl Timings {
    fn time<T: Debug>(&mut self, call: &'static str, run: impl FnOnce() -> T) -> T {
        let started_at = Instant::now();
        let result = black_box(run());
        let elapsed = started_at.elapsed();
        if let Some(digest) = &mut self.digest {
            let output = format!("{result:?}");
            call.hash(digest);
            output.hash(digest);
            if std::env::var_os("DIGEST_TRACE").is_some() {
                let mut call_digest = DefaultHasher::new();
                output.hash(&mut call_digest);
                println!(
                    "{call}: {:016x} {}",
                    call_digest.finish(),
                    &output[..output
                        .len()
                        .min(if std::env::var_os("DIGEST_FULL").is_some() {
                            usize::MAX
                        } else {
                            160
                        })]
                );
            }
        }
        self.by_call.entry(call).or_default().push(elapsed);
        result
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let digest = arguments.iter().any(|argument| argument == "--digest");
    let only_circuit = arguments.iter().find(|argument| *argument != "--digest");
    for (name, model_id) in [
        ("Timber Tumbler", "model:hybrid"),
        ("Velocity Viper", "model:velocity_viper"),
        ("Briar Blitz", "model:wooden_twister"),
    ] {
        if only_circuit.is_some_and(|only| only != name) {
            continue;
        }
        let timings = build_with_worker_updates(model_id, name, digest);
        if let Some(digest) = &timings.digest {
            println!("{name}: output digest {:016x}", digest.finish());
        } else {
            report(name, timings);
        }
    }
}

fn circuit_lines(name: &str) -> Vec<&'static str> {
    let header = format!("[{name}]");
    let mut sections = CIRCUITS.split(&header);
    sections.next().unwrap();
    let section = sections
        .next()
        .unwrap_or_else(|| panic!("{name} circuit missing"));
    section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn build_with_worker_updates(model_id: &str, name: &str, digest: bool) -> Timings {
    let mut simulation = WasmSimulation::new(42, 0, "new".into());
    let mut timings = Timings {
        digest: digest.then(DefaultHasher::new),
        ..Timings::default()
    };
    simulation.construction_select(model_id);
    simulation.construction_start(482, 498, 6, 2);
    let mut end_speed = None;
    send_construction(&mut simulation, &mut timings, true, &mut end_speed);
    let mut piece_count = 1;
    for piece_name in circuit_lines(name) {
        match piece_name {
            "+chain" | "-chain" => {
                simulation.construction_set_chain(piece_name == "+chain");
                end_speed = None;
                send_construction(&mut simulation, &mut timings, true, &mut end_speed);
                continue;
            }
            _ => {}
        }
        let selection_started_at = Instant::now();
        timings.time("select: speed match", || {
            simulation.construction_speed_match_json(piece_name, end_speed)
        });
        timings.time("select: construction_select", || {
            simulation.construction_select(piece_name)
        });
        send_construction(&mut simulation, &mut timings, true, &mut end_speed);
        timings
            .selection_updates
            .push(selection_started_at.elapsed());

        let placement_started_at = Instant::now();
        end_speed = None;
        timings.time("append: finish invalidate", || {
            simulation.construction_finish_invalidate()
        });
        timings.time("append: construction_append", || {
            simulation.construction_append()
        });
        if !simulation.construction_selected().starts_with("element:") {
            send_construction(&mut simulation, &mut timings, false, &mut end_speed);
        }
        send_world(&simulation, &mut timings);
        send_construction(&mut simulation, &mut timings, true, &mut end_speed);
        timings
            .placement_updates
            .push(placement_started_at.elapsed());
        piece_count += 1;
        let track: serde_json::Value =
            serde_json::from_str(&simulation.construction_track()).unwrap();
        assert_eq!(
            track.as_array().unwrap().len(),
            piece_count,
            "{name}: {piece_name} was not placed: {}",
            simulation.construction_message()
        );
    }
    if timings.digest.is_some() {
        for _ in 0..3 {
            simulation.construction_undo();
            send_construction(&mut simulation, &mut timings, false, &mut end_speed);
            send_world(&simulation, &mut timings);
            send_construction(&mut simulation, &mut timings, true, &mut end_speed);
        }
    }
    timings
}

fn send_world(simulation: &WasmSimulation, timings: &mut Timings) {
    timings.time("world: snapshot arrays", || {
        (
            simulation.world_dimensions(),
            simulation.world_surfaces(),
            simulation.world_paths(),
            simulation.world_path_metadata(),
            simulation.world_facility_openings(),
            simulation.world_carved(),
        )
    });
    timings.time("world: coaster_supports", || simulation.coaster_supports());
}

fn send_construction(
    simulation: &mut WasmSimulation,
    timings: &mut Timings,
    complete: bool,
    end_speed: &mut Option<f64>,
) {
    let prefix = if complete { "full" } else { "partial" };
    let label = |call: &str| -> &'static str { format!("{prefix}: {call}").leak() };
    let force_preview: serde_json::Value = if complete {
        serde_json::from_str(&timings.time(label("force_preview_json"), || {
            simulation.construction_force_preview_json()
        }))
        .unwrap()
    } else {
        serde_json::Value::Null
    };
    let preview_end_speed = (force_preview["status"] == "reachedEnd").then(|| {
        force_preview["samples"].as_array().unwrap().last().unwrap()["speed"]
            .as_f64()
            .unwrap()
    });
    let status = force_preview["status"].as_str().unwrap_or("").to_owned();
    timings.time(label("choices_with_preview"), || {
        simulation.construction_choices_with_preview(preview_end_speed, &status)
    });
    if complete {
        timings.time(label("elements"), || simulation.construction_elements());
    }
    let selected = simulation.construction_selected();
    timings.time(label("track"), || simulation.construction_track());
    timings.time(label("preview rails, ties, supports"), || {
        (
            simulation.construction_rails(),
            simulation.construction_ties(),
            simulation.construction_supports(),
        )
    });
    timings.time(label("placed rails, distances"), || {
        (
            simulation.construction_placed_rails(),
            simulation.construction_rail_distances(),
        )
    });
    timings.time(label("block_candidates"), || {
        simulation.construction_block_candidates()
    });
    timings.time(label("placed ties, supports"), || {
        (
            simulation.construction_placed_ties(),
            simulation.construction_placed_supports(),
        )
    });
    timings.time(label("block_report_json"), || {
        simulation.construction_block_report_json()
    });
    timings.time(label("small getters"), || {
        (
            simulation.construction_message(),
            simulation.construction_coaster_price(),
            simulation.construction_coaster_income(),
            simulation.construction_colors(),
            simulation.construction_train_layout(),
            simulation.construction_active(),
            simulation.construction_chain(),
        )
    });
    if complete {
        *end_speed = preview_end_speed;
    }
    timings.time(label("hill_options, speed_turns"), || {
        (
            simulation.construction_hill_options(),
            simulation.construction_speed_turns(),
        )
    });
    timings.time(label("can_close"), || simulation.construction_can_close());
    timings.time(label("finish state"), || {
        (
            simulation.construction_finish_preview_active(),
            simulation.construction_can_finish(),
            simulation.construction_finish_reason(),
        )
    });
    timings.time(label("test_report_json"), || {
        simulation.construction_test_report_json()
    });
    if complete {
        timings.time(label("candidate_force_preview_json"), || {
            simulation.construction_candidate_force_preview_json()
        });
        timings.time(label("speed_match_json"), || {
            simulation.construction_speed_match_json(&selected, *end_speed)
        });
    }
    timings.time(label("operate getters"), || {
        (
            simulation.construction_can_test(),
            simulation.construction_is_open(),
            simulation.construction_station_positions(),
            simulation.construction_train_count(),
            simulation.construction_min_cars_per_train(),
            simulation.construction_max_cars_per_train(),
        )
    });
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    sorted[((sorted.len() as f64 * fraction).ceil() as usize).max(1) - 1]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn report(name: &str, mut timings: Timings) {
    for updates in [
        &mut timings.selection_updates,
        &mut timings.placement_updates,
    ] {
        updates.sort_unstable();
    }
    println!(
        "{name}: {} placements; placement update median {:.1} ms, p95 {:.1} ms, max {:.1} ms; selection update median {:.1} ms, p95 {:.1} ms",
        timings.placement_updates.len(),
        milliseconds(percentile(&timings.placement_updates, 0.5)),
        milliseconds(percentile(&timings.placement_updates, 0.95)),
        milliseconds(*timings.placement_updates.last().unwrap()),
        milliseconds(percentile(&timings.selection_updates, 0.5)),
        milliseconds(percentile(&timings.selection_updates, 0.95)),
    );
    let mut rows: Vec<_> = timings
        .by_call
        .into_iter()
        .map(|(call, mut durations)| {
            durations.sort_unstable();
            let total: Duration = durations.iter().sum();
            (call, durations, total)
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.2));
    println!(
        "  {:<44} {:>6} {:>9} {:>9} {:>9}",
        "call", "count", "median", "p95", "total"
    );
    for (call, durations, total) in rows {
        if milliseconds(total) < 1.0 {
            continue;
        }
        println!(
            "  {:<44} {:>6} {:>9.2} {:>9.2} {:>9.1}",
            call,
            durations.len(),
            milliseconds(percentile(&durations, 0.5)),
            milliseconds(percentile(&durations, 0.95)),
            milliseconds(total),
        );
    }
}
