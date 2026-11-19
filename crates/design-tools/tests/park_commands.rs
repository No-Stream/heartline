use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;
use theme_park_sim_wasm::WasmSimulation;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-park-{}-{sequence}-{name}",
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

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn park_build_resolves_the_example_ride_reference_and_writes_a_park_save() {
    let park_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let park_source = fs::read_to_string(&park_path).unwrap();
    let park: serde_json::Value = serde_json::from_str(&park_source).unwrap();
    assert_eq!(
        park["coasters"][0]["ride"],
        "content/rides/horizon-hawk.ride.json"
    );

    let save = TempFile::new("horizon-hawk.park");
    let reply = run_cli(
        &[
            "park".into(),
            "build".into(),
            park_path.to_string_lossy().into_owned(),
            "--save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap();

    assert!(reply.contains("Saved park"), "{reply}");
    let restored = WasmSimulation::load_state_native(&fs::read(&save.0).unwrap()).unwrap();
    let track: serde_json::Value = serde_json::from_str(&restored.construction_track()).unwrap();
    assert!(!track.as_array().unwrap().is_empty());
}

#[test]
fn park_build_fails_when_a_coaster_ride_reference_is_missing() {
    let park = TempFile::new("missing-ride.park.json");
    let save = TempFile::new("missing-ride.park");
    fs::write(
        &park.0,
        r#"{
          "format": "heartline-park",
          "version": 1,
          "name": "Missing ride park",
          "terrain": { "preset": "flat", "edits": [] },
          "entrance": { "x": 218, "y": 246, "height": 8, "rotation": 2 },
          "paths": [],
          "facilities": [],
          "scenery": [],
          "coasters": [{
            "ride": "content/rides/missing-ride.ride.json",
            "station": { "x": 100, "y": 100, "z": 8, "heading": "east" },
            "open": false,
            "price": 6
          }]
        }"#,
    )
    .unwrap();

    let error = run_cli(
        &[
            "park".into(),
            "build".into(),
            park.path().into(),
            "--save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap_err();

    assert!(error.contains("coaster 0"), "{error}");
    assert!(
        error.contains("content/rides/missing-ride.ride.json"),
        "{error}"
    );
    assert!(!save.0.exists());
}

#[test]
fn park_build_rejects_multiple_coasters_before_resolving_ride_files() {
    let park = TempFile::new("multiple-coasters.park.json");
    let save = TempFile::new("multiple-coasters.park");
    let coaster = serde_json::json!({
        "ride": "content/rides/missing-ride.ride.json",
        "station": { "x": 100, "y": 100, "z": 8, "heading": "east" },
        "open": false,
        "price": 6
    });
    let description = serde_json::json!({
        "format": "heartline-park",
        "version": 1,
        "name": "Multiple coasters",
        "terrain": { "preset": "flat", "edits": [] },
        "entrance": { "x": 218, "y": 246, "height": 8, "rotation": 2 },
        "paths": [],
        "facilities": [],
        "scenery": [],
        "coasters": [coaster.clone(), coaster]
    });
    fs::write(&park.0, serde_json::to_vec(&description).unwrap()).unwrap();

    let error = run_cli(
        &[
            "park".into(),
            "build".into(),
            park.path().into(),
            "--save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap_err();

    assert_eq!(error, "this build supports one player coaster per park");
    assert!(!save.0.exists());
}

#[test]
fn park_build_requires_repository_relative_content_ride_references() {
    let park = TempFile::new("nonportable-ride.park.json");
    let save = TempFile::new("nonportable-ride.park");
    let description = serde_json::json!({
        "format": "heartline-park",
        "version": 1,
        "name": "Nonportable ride reference",
        "terrain": { "preset": "flat", "edits": [] },
        "entrance": { "x": 218, "y": 246, "height": 8, "rotation": 2 },
        "paths": [],
        "facilities": [],
        "scenery": [],
        "coasters": [{
            "ride": "../rides/timber-tumbler.ride.json",
            "station": { "x": 100, "y": 100, "z": 8, "heading": "east" },
            "open": false,
            "price": 6
        }]
    });
    fs::write(&park.0, serde_json::to_vec(&description).unwrap()).unwrap();

    let error = run_cli(
        &[
            "park".into(),
            "build".into(),
            park.path().into(),
            "--save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap_err();

    assert!(
        error.contains("must use content/rides/<name>.ride.json"),
        "{error}"
    );
    assert!(!save.0.exists());
}

#[test]
fn park_build_requires_an_explicit_save_path() {
    let park = repository_root().join("content/parks/horizon-hawk.park.json");

    let error = run_cli(
        &[
            "park".into(),
            "build".into(),
            park.to_string_lossy().into_owned(),
        ],
        "",
    )
    .unwrap_err();

    assert!(error.contains("--save <path>"), "{error}");
}
