use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use theme_park_design_tools::run_cli;
use theme_park_sim_wasm::WasmSimulation;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-{name}-{}-{sequence}",
            std::process::id()
        )))
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn session_export_replays_element_expansion_before_assigning_launch_speed() {
    let ride = json!({
        "format": "heartline-ride",
        "version": 1,
        "name": "Expanded element launch",
        "model": "velocity_viper",
        "station": { "x": 100, "y": 100, "z": 12, "heading": "north" },
        "carsPerTrain": 6,
        "trains": 1,
        "steps": [
            { "place": "element:small-left-banked-turn" },
            { "place": "LaunchShort", "launchSpeed": 5.0 }
        ]
    });
    let save_file = TempFile::new("ride.park");
    let session_file = TempFile::new("ride.session.json");
    let ride_file = TempFile::new("ride.json");
    fs::write(&ride_file.0, serde_json::to_vec(&ride).unwrap()).unwrap();

    run_cli(
        &[
            "ride".to_owned(),
            ride_file.path().to_owned(),
            "export".to_owned(),
            "--save".to_owned(),
            save_file.path().to_owned(),
            "export".to_owned(),
            "--session".to_owned(),
            session_file.path().to_owned(),
        ],
        "",
    )
    .unwrap();

    let track: Vec<Value> = serde_json::from_str(
        &WasmSimulation::load_state_native(&fs::read(&save_file.0).unwrap())
            .unwrap()
            .construction_track(),
    )
    .unwrap();
    let session: Value = serde_json::from_slice(&fs::read(&session_file.0).unwrap()).unwrap();
    let launch_id = track
        .iter()
        .find(|piece| piece["label"] == "LaunchShort")
        .unwrap()["id"]
        .clone();
    let exported_launch_id = session["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["command"]["type"] == "construction-set-launch-intensity")
        .unwrap()["command"]["id"]
        .clone();

    assert_eq!(exported_launch_id, launch_id);
}

#[test]
fn session_export_tracks_a_placement_after_undoing_an_element_group() {
    let ride = json!({
        "format": "heartline-ride",
        "version": 1,
        "name": "Undo before session export",
        "model": "hybrid",
        "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
        "carsPerTrain": 4,
        "trains": 1,
        "steps": [{ "place": "LaunchLong" }]
    });
    let ride_file = TempFile::new("undo.ride.json");
    let session_file = TempFile::new("undo.session.json");
    fs::write(&ride_file.0, serde_json::to_vec(&ride).unwrap()).unwrap();

    run_cli(
        &[
            "ride".to_owned(),
            ride_file.path().to_owned(),
            "place element:small-left-banked-turn".to_owned(),
            "undo".to_owned(),
            "export".to_owned(),
            "--session".to_owned(),
            session_file.path().to_owned(),
        ],
        "",
    )
    .unwrap();

    let session: Value = serde_json::from_slice(&fs::read(&session_file.0).unwrap()).unwrap();
    let commands = session["commands"].as_array().unwrap();
    let append_count = commands
        .iter()
        .filter(|entry| entry["command"]["type"] == "construction-append")
        .count();
    assert_eq!(append_count, 1);
}

#[test]
fn session_export_replays_explicit_block_sections_without_running_search() {
    let ride = json!({
        "format": "heartline-ride",
        "version": 1,
        "name": "Automatic block two-train circuit",
        "model": "hybrid",
        "station": { "x": 50, "y": 50, "z": 80, "heading": "east" },
        "carsPerTrain": 4,
        "trains": 1,
        "steps": [
            { "place": "Straight" },
            { "place": "Straight" },
            { "chain": true },
            { "place": "Straight" },
            { "chain": false },
            { "place": "Straight" },
            { "place": "Straight" },
            { "place": "TurnLargeEntry" },
            { "place": "TurnLargeExit" },
            { "place": "Straight" },
            { "place": "Straight" },
            { "chain": true },
            { "place": "Straight" },
            { "chain": false },
            { "place": "Straight" },
            { "place": "Straight" },
            { "place": "Straight" },
            { "place": "TurnLargeEntry" },
            { "place": "TurnLargeExit" },
            { "close": true },
            {
                "blockSections": [],
                "trainCapacity": 1,
                "capacitySearchComplete": false,
                "placementSearchComplete": true
            }
        ]
    });

    let ride_file = TempFile::new("blocks.ride.json");
    let session_file = TempFile::new("blocks.session.json");
    fs::write(&ride_file.0, serde_json::to_vec(&ride).unwrap()).unwrap();
    run_cli(
        &[
            "ride".to_owned(),
            ride_file.path().to_owned(),
            "export".to_owned(),
            "--session".to_owned(),
            session_file.path().to_owned(),
        ],
        "",
    )
    .unwrap();
    let session: Value = serde_json::from_slice(&fs::read(&session_file.0).unwrap()).unwrap();
    let commands = session["commands"].as_array().unwrap();
    let block_sections_index = commands
        .iter()
        .position(|entry| entry["command"]["type"] == "construction-set-block-sections")
        .unwrap();
    let close_index = commands
        .iter()
        .position(|entry| entry["command"]["type"] == "construction-close")
        .unwrap();
    assert!(close_index < block_sections_index);
    assert_eq!(
        commands[block_sections_index]["command"],
        json!({
            "type": "construction-set-block-sections",
            "blockSections": [],
            "trainCapacity": 1,
            "capacitySearchComplete": false,
            "placementSearchComplete": true,
        })
    );
    assert!(
        !commands
            .iter()
            .any(|entry| entry["command"]["type"] == "construction-auto-blocks")
    );
    assert!(
        !commands
            .iter()
            .any(|entry| entry["command"]["type"] == "test-advance-block-analysis")
    );
}
