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
            "theme-park-design-tools-{}-{sequence}-{name}",
            std::process::id()
        )))
    }

    fn write(&self, contents: &str) {
        fs::write(&self.0, contents).unwrap();
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
  "name": "CLI test ride",
  "model": "hybrid",
  "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
  "carsPerTrain": 4,
  "trains": 1,
  "steps": []
}"#;

const CLOSED_RIDE: &str = r#"{
  "format": "heartline-ride",
  "version": 1,
  "name": "CLI report ride",
  "model": "hybrid",
  "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
  "carsPerTrain": 4,
  "trains": 1,
  "steps": [
    { "place": "TurnSmallStandalone" },
    { "place": "TurnSmallStandalone" },
    { "place": "Straight" },
    { "place": "TurnSmallStandalone" },
    { "place": "TurnSmallStandalone" },
    { "close": true }
  ]
}"#;

#[test]
fn choices_estimates_and_placement_have_stable_text_replies() {
    let ride = TempFile::new("open.ride.json");
    ride.write(OPEN_RIDE);

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "choices straight".into(),
            "estimate Straight".into(),
            "place Straight --note test-piece".into(),
        ],
        "",
    )
    .unwrap();

    let expected = r#"id                         name                         group       refusal                 tunnel window     margin matched
Straight                   Straight                     straight                            no     —                 — no
OffsetLeft                 Left half-tile offset        straight                            no     —                 — no
HalfTileStraight           Half-tile straight           straight                            no     —                 — no
OneTileStraight            One-tile straight            straight                            no     —                 — no
FullOffsetLeft             Left full-tile offset        straight                            no     —                 — no
ShortOffsetLeft            Left short half-tile offset  straight                            no     —                 — no
AirtimeHill                Small airtime hill           straight                            no     —                 — no
OffsetRight                Right half-tile offset       straight                            no     —                 — no
FullOffsetRight            Right full-tile offset       straight                            no     —                 — no
ShortOffsetRight           Right short half-tile offset straight                            no     —                 — no
piece                      entry m/s window      margin  vertical g (rating)                       lateral g (rating)   long g (rating) status
Straight                        2.0 —                — 1.0 (weak) / 1.0 (weak)                    0.0 (fine)         0.0 (fine)         reachedEnd
Placed Straight
Estimate: 2.0 m/s entry, v+ 1.00 g, v- 1.00 g, lateral 0.00 g, longitudinal 0.01 g
Pieces: 1
Open end: tile (108.0, 100.0), height 10.0 m, heading East, slope Level, bank Level
Height above ground: 2 steps; offset to station entry: 32.0 m (x -8.0, y +0.0 tiles, height +0.0 m)
Chain: false; predicted end speed: 1.6 m/s (reachedEnd)
Can close: false; can finish: false (Checking for a connection to the station.); can test: false
"#;
    assert_eq!(reply, expected);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["steps"][0]["place"], "Straight");
    assert_eq!(saved["steps"][0]["note"], "test-piece");
}

#[test]
fn refused_placement_keeps_the_ride_file_unchanged() {
    let ride = TempFile::new("refused.ride.json");
    ride.write(OPEN_RIDE);
    let before = fs::read(&ride.0).unwrap();

    let error = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "place UnknownPiece".into(),
        ],
        "",
    )
    .unwrap_err();

    assert!(error.contains("Unknown piece"), "{error}");
    assert!(error.contains("offered ids"), "{error}");
    assert_eq!(fs::read(&ride.0).unwrap(), before);
}

#[test]
fn test_command_prints_the_ride_report_and_piece_table() {
    let ride = TempFile::new("closed.ride.json");
    ride.write(CLOSED_RIDE);

    let reply = run_cli(&["ride".into(), ride.path().into(), "test".into()], "").unwrap();

    assert_eq!(reply, include_str!("goldens/cli_test_report.txt"));
}

#[test]
fn exported_park_save_loads_with_the_same_construction() {
    let ride = TempFile::new("save.ride.json");
    let save = TempFile::new("save.park");
    ride.write(OPEN_RIDE);

    run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "place Straight".into(),
            "export --save".into(),
            save.path().into(),
        ],
        "",
    )
    .unwrap();

    let restored = WasmSimulation::load_state_native(&fs::read(&save.0).unwrap()).unwrap();
    let track: serde_json::Value = serde_json::from_str(&restored.construction_track()).unwrap();
    assert_eq!(
        track.as_array().unwrap().last().unwrap()["label"],
        "Straight"
    );
}

#[test]
fn new_creates_a_ride_and_state_reports_the_open_end() {
    let ride = TempFile::new("new.ride.json");

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "new --model hybrid --station 100,100,10,east --cars 4".into(),
            "state".into(),
        ],
        "",
    )
    .unwrap();

    assert_eq!(
        reply,
        "Pieces: 0\nOpen end: tile (104.0, 100.0), height 10.0 m, heading East, slope Level, bank Level\nHeight above ground: 2 steps; offset to station entry: 16.0 m (x -4.0, y +0.0 tiles, height +0.0 m)\nChain: false; predicted end speed: 2.0 m/s (reachedEnd)\nCan close: false; can finish: false (Checking for a connection to the station.); can test: false\n"
    );
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["model"], "hybrid");
    assert_eq!(saved["station"]["heading"], "east");
    assert_eq!(saved["carsPerTrain"], 4);
}
