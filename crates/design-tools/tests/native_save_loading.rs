use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;
use theme_park_sim::track::ride_file::RideFile;
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::BuilderSession;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-native-save-{}-{sequence}-{name}",
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

const OPEN_RIDE: &str = r#"{
  "format": "heartline-ride",
  "version": 1,
  "name": "Native save round trip",
  "model": "hybrid",
  "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
  "carsPerTrain": 4,
  "trains": 1,
  "steps": []
}"#;

#[test]
fn exported_cli_save_loads_natively_with_the_replayed_ride_state() {
    let ride_path = TempFile::new("ride.json");
    let save_path = TempFile::new("park.save");
    fs::write(&ride_path.0, OPEN_RIDE).unwrap();

    run_cli(
        &[
            "ride".into(),
            ride_path.path().into(),
            "place Straight".into(),
            "export --save".into(),
            save_path.path().into(),
        ],
        "",
    )
    .unwrap();

    let loaded = WasmSimulation::load_state_native(&fs::read(&save_path.0).unwrap()).unwrap();
    let ride: RideFile = serde_json::from_slice(&fs::read(&ride_path.0).unwrap()).unwrap();
    let mut expected = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
    expected.replay_ride(&ride).unwrap();

    assert_eq!(loaded.state_hash(), expected.state_hash());
    assert_eq!(
        loaded.construction_track(),
        expected.simulation().construction_track()
    );
    assert_eq!(
        loaded.construction_train_layout(),
        expected.simulation().construction_train_layout()
    );
}
