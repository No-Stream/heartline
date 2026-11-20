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

use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::BuilderSession;

const TIMBER_TUMBLER: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");
const VELOCITY_VIPER: &str = include_str!("../../../content/rides/velocity-viper.ride.json");
const BRIAR_BLITZ: &str = include_str!("../../../content/rides/briar-blitz.ride.json");

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
        self.record(call, &result);
        self.by_call.entry(call).or_default().push(elapsed);
        result
    }

    fn measure<T>(&mut self, call: &'static str, run: impl FnOnce() -> T) -> T {
        let started_at = Instant::now();
        let result = black_box(run());
        self.by_call
            .entry(call)
            .or_default()
            .push(started_at.elapsed());
        result
    }

    fn record<T: Debug>(&mut self, call: &'static str, result: &T) {
        let Some(digest) = &mut self.digest else {
            return;
        };
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

    fn record_with<T: Debug>(&mut self, call: &'static str, result: impl FnOnce() -> T) {
        if self.digest.is_some() {
            self.record(call, &result());
        }
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

fn build_with_worker_updates(model_id: &str, name: &str, digest: bool) -> Timings {
    let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
    let mut timings = Timings {
        digest: digest.then(DefaultHasher::new),
        ..Timings::default()
    };
    let model_key = model_id
        .strip_prefix("model:")
        .expect("benchmark model ids use the model: prefix");
    timings.measure("session: start", || {
        session.start(model_key, 241, 249, 12, 1).unwrap();
    });
    send_session_construction(&mut session, &mut timings, true);
    let mut piece_count = 1;
    let ride = ride_file(name);
    for step in &ride.steps {
        match step {
            RideStep::Chain(step) => {
                timings.measure("session: set_chain", || {
                    session.set_chain(step.chain).unwrap();
                });
                send_session_construction(&mut session, &mut timings, true);
                continue;
            }
            RideStep::Place(step) => {
                let piece_name = step.place.as_str();
                let selection_started_at = Instant::now();
                let selection = timings.measure("session: select", || {
                    session.select(piece_name, true).unwrap()
                });
                timings.record_with("select: speed match", || {
                    serde_json::to_string(&selection.speed_match).unwrap()
                });
                timings.record("select: construction_select", &());
                send_session_construction(&mut session, &mut timings, true);
                timings
                    .selection_updates
                    .push(selection_started_at.elapsed());

                let placement_started_at = Instant::now();
                timings.measure("session: append_selected", || {
                    session.append_selected().unwrap();
                });
                timings.record("append: finish invalidate", &());
                timings.record("append: construction_append", &());
                if !session
                    .simulation()
                    .construction_selected()
                    .starts_with("element:")
                {
                    send_session_construction(&mut session, &mut timings, false);
                }
                send_world(session.simulation(), &mut timings);
                send_session_construction(&mut session, &mut timings, true);
                timings
                    .placement_updates
                    .push(placement_started_at.elapsed());
                piece_count += 1;
                let track: serde_json::Value =
                    serde_json::from_str(&session.simulation().construction_track()).unwrap();
                assert_eq!(
                    track.as_array().unwrap().len(),
                    piece_count,
                    "{name}: {piece_name} was not placed: {}",
                    session.simulation().construction_message()
                );
            }
            RideStep::Close(step) => assert!(step.close, "{name} has a false close step"),
            _ => panic!("{name} benchmark does not support this ride step"),
        }
    }
    if timings.digest.is_some() {
        for _ in 0..3 {
            timings.measure("session: undo", || session.undo().unwrap());
            send_session_construction(&mut session, &mut timings, false);
            send_world(session.simulation(), &mut timings);
            send_session_construction(&mut session, &mut timings, true);
        }
    }
    timings
}

fn send_session_construction(session: &mut BuilderSession, timings: &mut Timings, complete: bool) {
    let prefix = if complete { "full" } else { "partial" };
    let label = |call: &str| -> &'static str { format!("{prefix}: {call}").leak() };
    let estimates = timings.measure(label("session estimates"), || {
        let estimates = session.worker_update(complete);
        black_box(estimates.to_worker_json());
        estimates
    });
    if complete {
        timings.record_with(label("force_preview_json"), || {
            serde_json::to_string(&estimates.force_preview).unwrap()
        });
    }
    timings.record_with(label("choices_with_preview"), || {
        serde_json::to_string(&estimates.choices).unwrap()
    });
    if complete {
        timings.record_with(label("elements"), || {
            serde_json::to_string(&estimates.elements).unwrap()
        });
    }
    let simulation = session.simulation();
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
    timings.record_with(label("hill_options, speed_turns"), || {
        (
            estimates.hill_options_json.clone(),
            estimates.speed_turns_json.clone(),
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
        timings.record_with(label("candidate_force_preview_json"), || {
            serde_json::to_string(&estimates.candidate_preview).unwrap()
        });
        timings.record_with(label("speed_match_json"), || {
            serde_json::to_string(&estimates.speed_match).unwrap()
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
