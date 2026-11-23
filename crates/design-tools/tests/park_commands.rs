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
fn park_build_rejects_a_station_height_that_differs_from_the_ride_file() {
    let park = TempFile::new("station-height-mismatch.park.json");
    let save = TempFile::new("station-height-mismatch.park");
    let source_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let mut description: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(source_path).unwrap()).unwrap();
    description["coasters"][0]["station"]["z"] = serde_json::json!(12);
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

    assert!(error.contains("coasters[0]"), "{error}");
    assert!(error.contains("station height mismatch"), "{error}");
    assert!(error.contains("ride file is 10"), "{error}");
    assert!(error.contains("park description is 12"), "{error}");
    assert!(!save.0.exists());
}

#[test]
fn park_build_resolves_every_coaster_ride_file_and_writes_a_park_save() {
    let park = TempFile::new("multiple-coasters.park.json");
    let save = TempFile::new("multiple-coasters.park");
    let mut description: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repository_root().join("content/parks/horizon-hawk.park.json"))
            .unwrap(),
    )
    .unwrap();
    description["coasters"] = serde_json::json!([
        {
            "ride": "content/rides/horizon-hawk.ride.json",
            "station": { "x": 241, "y": 249, "z": 10, "heading": "north" },
            "open": true,
            "price": 6
        },
        {
            "ride": "content/rides/daisy-dash.ride.json",
            "station": { "x": 340, "y": 300, "z": 12, "heading": "north" },
            "open": false,
            "price": 8
        }
    ]);
    let terrain_edits = description["terrain"]["edits"].as_array_mut().unwrap();
    for x in 337..=344 {
        for y in 300..=302 {
            terrain_edits.push(serde_json::json!({
                "command": "flatten",
                "parameters": [x, y, 1, 0, 0, 12, 0, 0]
            }));
        }
    }
    for x in [338, 342] {
        for corner in [2, 3] {
            terrain_edits.push(serde_json::json!({
                "command": "lower_corner",
                "parameters": [x, 302, 1, corner, 1, 0, 0, 0]
            }));
        }
    }
    fs::write(&park.0, serde_json::to_vec(&description).unwrap()).unwrap();

    let reply = run_cli(
        &[
            "park".into(),
            "build".into(),
            park.path().into(),
            "--save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap();

    assert!(reply.contains("Saved park"), "{reply}");
    let restored = WasmSimulation::load_state_native(&fs::read(&save.0).unwrap()).unwrap();
    let coasters: serde_json::Value = serde_json::from_str(&restored.coasters_json()).unwrap();
    assert_eq!(coasters.as_array().unwrap().len(), 2);
    assert_eq!(coasters[0]["model"], "hyper");
    assert_eq!(coasters[1]["model"], "family_coaster");
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
