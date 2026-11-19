use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;

static NEXT_TEMP_RIDE: AtomicUsize = AtomicUsize::new(0);

struct TempRide(PathBuf);

impl TempRide {
    fn new() -> Self {
        let sequence = NEXT_TEMP_RIDE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-cli-diagnostics-{}-{sequence}.ride.json",
            std::process::id()
        )))
    }

    fn path(&self) -> String {
        self.0.display().to_string()
    }
}

impl Drop for TempRide {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let mut snapshot_path = self.0.as_os_str().to_owned();
        snapshot_path.push(".snapshot");
        let _ = fs::remove_file(snapshot_path);
    }
}

#[test]
fn help_lists_commands_and_choice_groups() {
    let help = run_cli(&["help".into()], "").unwrap();

    for command in [
        "ride",
        "check",
        "park build",
        "park report",
        "new",
        "state",
        "choices",
        "estimate",
        "place",
        "finish",
        "close",
        "test",
        "report",
        "agreement",
        "map",
        "export",
        "chain",
        "launch-speed",
        "cars",
        "trains",
        "auto-blocks",
        "undo",
        "truncate",
        "probe",
    ] {
        assert!(help.contains(command), "help omits {command}:\n{help}");
    }
    for group in [
        "bank",
        "element",
        "hill",
        "inversion",
        "slope",
        "special",
        "straight",
        "turn",
    ] {
        assert!(
            help.contains(group),
            "help omits choice group {group}:\n{help}"
        );
    }
}

#[test]
fn failed_close_shows_remaining_station_offset_and_heading_mismatch() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/rides/timber-tumbler.ride.json");
    let ride = TempRide::new();
    let mut ride_file: serde_json::Value =
        serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
    let steps = ride_file["steps"].as_array_mut().unwrap();
    steps.pop().expect("catalog ride ends with a close step");
    let final_turn_index = steps.len() - 1;
    assert_eq!(steps[final_turn_index]["place"], "TurnSmallStandalone");
    steps.remove(final_turn_index - 1);
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_file).unwrap()).unwrap();

    let state_json = run_cli(
        &["ride".into(), ride.path(), "state".into(), "--json".into()],
        "",
    )
    .unwrap();
    let state_json: serde_json::Value = serde_json::from_str(&state_json).unwrap();
    assert!(
        state_json["openEnd"]["offsetToStationEntryMetres"]
            .as_f64()
            .unwrap()
            < 10.0,
        "fixture should end near the station: {state_json}"
    );
    assert!(!state_json["canClose"].as_bool().unwrap());

    let state = run_cli(&["ride".into(), ride.path(), "state".into()], "").unwrap();

    assert!(state.contains("remaining offset:"), "{state}");
    assert!(state.contains("tiles"), "{state}");
    assert!(state.contains("m"), "{state}");
    assert!(state.contains("heading mismatch:"), "{state}");

    let error = run_cli(&["ride".into(), ride.path(), "close".into()], "").unwrap_err();
    assert!(error.contains("remaining offset:"), "{error}");
    assert!(error.contains("heading mismatch:"), "{error}");
}

#[test]
fn failed_finish_shows_remaining_station_offset_and_heading_mismatch() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/rides/timber-tumbler.ride.json");
    let ride = TempRide::new();
    fs::copy(source, &ride.0).unwrap();

    let error = run_cli(&["ride".into(), ride.path(), "finish".into()], "").unwrap_err();

    assert!(error.contains("remaining offset:"), "{error}");
    assert!(error.contains("heading mismatch:"), "{error}");
}

#[test]
fn replay_errors_label_one_based_step_and_piece_indices() {
    let ride = TempRide::new();
    fs::write(
        &ride.0,
        r#"{
            "format": "heartline-ride",
            "version": 1,
            "name": "Bad second step",
            "model": "hyper",
            "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
            "carsPerTrain": 4,
            "trains": 1,
            "steps": [
                { "chain": true },
                { "place": "MissingPiece" }
            ]
        }"#,
    )
    .unwrap();

    let error = run_cli(&["ride".into(), ride.path(), "state".into()], "").unwrap_err();

    assert!(
        error.contains("ride step 2 (piece 1):"),
        "replay error should use one-based ride-step and placement numbering: {error}"
    );
}

#[test]
fn report_piece_table_starts_at_the_first_placed_piece() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/rides/timber-tumbler.ride.json");
    let ride = TempRide::new();
    fs::copy(source, &ride.0).unwrap();

    let report = run_cli(&["ride".into(), ride.path(), "report".into()], "").unwrap();
    let table_header_index = report.find("# piece ").expect("report has a piece table");
    let first_piece_row = report[table_header_index..]
        .lines()
        .nth(1)
        .expect("piece table has a row");

    assert!(
        first_piece_row.starts_with("  1 ") || first_piece_row.starts_with("1 "),
        "first report row should use the first placed piece index: {first_piece_row}"
    );
}
