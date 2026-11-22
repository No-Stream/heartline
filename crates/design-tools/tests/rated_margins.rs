use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;
use theme_park_sim::physics::{Circuit, ConstructionTestRunRecorder, PhysicsConfig, Train};
use theme_park_sim::track::ride_file::RideFile;
use theme_park_sim::track::{PieceKind, feel_rated_speeds, rated_speeds};
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::BuilderSession;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const TIMBER_TUMBLER: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");

struct TempRide(PathBuf);

impl TempRide {
    fn new() -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-rated-margins-{}-{sequence}.ride.json",
            std::process::id()
        )))
    }

    fn write(&self) {
        fs::write(&self.0, TIMBER_TUMBLER).unwrap();
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempRide {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let mut snapshot = self.0.as_os_str().to_owned();
        snapshot.push(".snapshot");
        let _ = fs::remove_file(snapshot);
    }
}

#[test]
fn report_shows_entry_margin_checks_and_attributes_force_peaks_to_piece_and_car() {
    let ride = TempRide::new();
    ride.write();

    let report = run_cli(&["ride".into(), ride.path().into(), "report".into()], "").unwrap();

    assert!(report.contains("Rated entry-speed margin"), "{report}");
    assert!(report.contains("default physics"), "{report}");
    assert!(report.contains("construction-test physics"), "{report}");
    assert!(report.contains("EjectorHill16x96"), "{report}");
    assert!(
        report.contains("Measured force peaks by piece and car"),
        "{report}"
    );
    assert!(report.contains("sustained by any car"), "{report}");
    assert!(report.contains("car 0"), "{report}");
    assert!(
        !report.lines().any(|line| line.starts_with("  0 ")),
        "{report}"
    );
}

#[test]
fn report_shows_mid_course_brakes_and_their_share_of_lift_energy() {
    let ride = TempRide::new();
    ride.write();

    let report = run_cli(&["ride".into(), ride.path().into(), "report".into()], "").unwrap();

    let brake_line = report
        .lines()
        .find(|line| line.starts_with("Mid-course brake runs:"))
        .expect("ride report includes its mid-course brake energy summary");
    assert!(
        brake_line.contains("Mid-course brake runs: 2"),
        "{brake_line}"
    );
    assert!(
        brake_line.contains("estimated lift energy removed by trims from lead-car samples:"),
        "{brake_line}"
    );
    let trim_energy_share = brake_line
        .split_once("from lead-car samples: ")
        .expect("summary names the lift-energy share")
        .1
        .split_whitespace()
        .next()
        .expect("summary includes a percentage")
        .trim_end_matches('%')
        .parse::<f64>()
        .expect("summary reports a numeric percentage");
    assert!(trim_energy_share > 0.0, "{brake_line}");
}

#[test]
fn historical_timber_report_preserves_its_speed_window_and_force_regressions() {
    let ride: RideFile =
        serde_json::from_str(include_str!("fixtures/historical-timber-tumbler.ride.json")).unwrap();
    let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
    session.replay_ride(&ride).unwrap();

    let construction = session.simulation().construction.as_ref().unwrap();
    let pieces = construction.pieces().to_vec();
    let model = construction.model().clone();
    let circuit = Circuit::open(pieces.clone()).unwrap();
    let mut train = Train::new(
        &circuit,
        &model.train,
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
        0.0,
        2.0,
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
        &circuit,
        &model.train,
        &model,
    );
    let mut entries = vec![vec![None; model.train.cars as usize]; pieces.len()];
    let mut maximum_lateral_g = 0.0_f64;
    let mut completed = false;
    let mut recovered = false;
    let mut stationary_steps = 0;

    for _ in 0..200_000 {
        train.step(&circuit);
        let frames = train.take_readings();
        for frame in &frames {
            for (car, reading) in frame.iter().enumerate() {
                let car_distance = train.distance()
                    - car as f64 * model.train.length_metres / f64::from(model.train.cars);
                if car_distance < 0.0 {
                    continue;
                }
                entries[reading.piece_index][car].get_or_insert(reading.speed);
                maximum_lateral_g = maximum_lateral_g.max(reading.g.lateral.abs());
            }
        }
        recorder.record(&frames, model.name);

        stationary_steps = if train.speed().abs() < 0.01 {
            stationary_steps + 1
        } else {
            0
        };
        if !train.notes().is_empty() || stationary_steps >= 400 {
            recovered = true;
            break;
        }
        if train.distance() >= circuit.length() {
            completed = true;
            break;
        }
    }
    let historical_run = recorder.finish(completed, recovered);
    let historical_report =
        theme_park_design_tools::report::render_ride_report(&ride, &historical_run, &[], false)
            .unwrap();
    assert!(
        historical_report.contains("Score breakdown"),
        "{historical_report}"
    );

    let mut outside_piece_ids = BTreeSet::new();
    for (index, (piece, piece_entries)) in pieces.iter().zip(entries).enumerate() {
        let Some(window) = feel_rated_speeds(&model, piece).or_else(|| rated_speeds(&model, piece))
        else {
            continue;
        };
        let fed_by_lift = pieces[(index + pieces.len() - 1) % pieces.len()].kind == PieceKind::Lift;
        let checked_cars = if fed_by_lift { 1 } else { piece_entries.len() };
        let worst_entry = piece_entries[..checked_cars]
            .iter()
            .flatten()
            .copied()
            .min_by(|left, right| {
                let margin =
                    |speed: f64| (speed - window.minimum_entry).min(window.maximum_entry - speed);
                margin(*left).total_cmp(&margin(*right))
            });
        if worst_entry
            .is_some_and(|speed| speed < window.minimum_entry || speed > window.maximum_entry)
        {
            outside_piece_ids.insert(format!("{:?}", piece.id));
        }
    }

    assert_eq!(
        outside_piece_ids,
        BTreeSet::from([
            "SteepToSixtyUp".to_owned(),
            "SixtyToSteepUp".to_owned(),
            "SixtyToSteepDown".to_owned(),
            "BankLeft".to_owned(),
            "UnbankLeft".to_owned(),
        ]),
        "historical rated-window flags changed"
    );
    assert!(maximum_lateral_g > 1.5, "{maximum_lateral_g}");
}
