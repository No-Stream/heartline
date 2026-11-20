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

fn assert_six_decimal_precision(value: &serde_json::Value) {
    match value {
        serde_json::Value::Number(number) => {
            let value = number.as_f64().expect("ride spec numbers are finite");
            let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
            assert!(
                (value - rounded).abs() <= 1e-9,
                "ride spec number {value} has more than six decimal places"
            );
        }
        serde_json::Value::Array(values) => {
            for value in values {
                assert_six_decimal_precision(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                assert_six_decimal_precision(value);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {}
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

    let expected = r#"id                         name                         group       refusal                 tunnel window     margin matched end tile        height m length m
Straight                   Straight                     straight                            no     —                 — no      (108.0, 100.0)      10.0     16.0
OffsetLeft                 Left half-tile offset        straight                            no     —                 — no      (110.0, 100.5)      10.0     24.1
HalfTileStraight           Half-tile straight           straight                            no     —                 — no      (104.5, 100.0)      10.0      2.0
OneTileStraight            One-tile straight            straight                            no     —                 — no      (105.0, 100.0)      10.0      4.0
FullOffsetLeft             Left full-tile offset        straight                            no     —                 — no      (110.0, 101.0)      10.0     24.5
ShortOffsetLeft            Left short half-tile offset  straight                            no     —                 — no      (107.0, 100.5)      10.0     12.3
AirtimeHill                Small airtime hill           straight                            no     —                 — no      (112.0, 100.0)      10.0     32.7
OffsetRight                Right half-tile offset       straight                            no     —                 — no      (110.0, 99.5)       10.0     24.1
FullOffsetRight            Right full-tile offset       straight                            no     —                 — no      (110.0, 99.0)       10.0     24.5
ShortOffsetRight           Right short half-tile offset straight                            no     —                 — no      (107.0, 99.5)       10.0     12.3
piece                      entry m/s window      margin  candidate preview vertical g (rating)     lateral g (rating)   long g (rating) status
Straight                        2.0 —                — 1.0 (weak) / 1.0 (weak)                    0.0 (fine)         0.0 (fine)         reachedEnd
Placed Straight
Estimate: 2.0 m/s entry, v+ 1.00 g, v- 1.00 g, lateral 0.00 g, longitudinal 0.01 g
Pieces: 1
Open end: tile (108.0, 100.0), height 10.0 m, heading East, slope Level, bank Level
Height above ground: 2 steps; offset to station entry: 32.0 m (x -8.0, y +0.0 tiles, height +0.0 m)
Closure approach: remaining offset: 8.0 tiles (32.0 m); heading mismatch: 0 degrees
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
fn estimate_uses_the_exact_requested_piece_after_a_high_speed_launch() {
    let ride = TempFile::new("exact-estimate.ride.json");
    ride.write(OPEN_RIDE);

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "place LaunchLong --launch-speed 30".into(),
            "estimate Loop".into(),
        ],
        "",
    )
    .unwrap();
    let replies: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(replies[1][0]["id"], "Loop");
    assert!(replies[1][0].get("requestedId").is_none());
}

#[test]
fn matched_speed_turn_uses_a_concrete_variant_inside_its_rated_window() {
    let ride = TempFile::new("matched-speed-turn.ride.json");
    ride.write(OPEN_RIDE);

    let mut arguments = vec!["ride".into(), ride.path().into(), "--json".into()];
    arguments.extend(speed_matched_prefix_commands());
    arguments.push("place SpeedTurnLarge32Left --matched".into());
    let output = run_cli(&arguments, "").unwrap();
    let replies: serde_json::Value = serde_json::from_str(&output).unwrap();
    let matched_turn = replies.as_array().unwrap().last().unwrap();
    let entry_speed = matched_turn["estimate"]["entrySpeed"].as_f64().unwrap();
    let window = matched_turn["estimate"]["ratedWindow"].as_array().unwrap();

    assert!(
        matched_turn["placed"]
            .as_str()
            .unwrap()
            .starts_with("SpeedTurnLarge")
    );
    assert_eq!(matched_turn["requested"], "SpeedTurnLarge32Left");
    assert!(window[0].as_f64().unwrap() <= entry_speed);
    assert!(entry_speed <= window[1].as_f64().unwrap());
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(
        saved["steps"].as_array().unwrap().last().unwrap()["place"],
        matched_turn["placed"]
    );
}

#[test]
fn matched_placement_fails_when_no_variant_window_fits_the_entry_speed() {
    let ride = TempFile::new("unmatched-speed-turn.ride.json");
    ride.write(OPEN_RIDE);

    let error = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "place SpeedTurnLarge32Left --matched".into(),
        ],
        "",
    )
    .unwrap_err();

    assert!(error.contains("no speed-matched variant"), "{error}");
    assert!(error.contains("2.00 m/s"), "{error}");
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert!(saved["steps"].as_array().unwrap().is_empty());
}

#[test]
fn matched_hill_uses_a_concrete_variant_inside_its_rated_window() {
    let ride = TempFile::new("matched-hill.ride.json");
    ride.write(OPEN_RIDE);

    let mut arguments = vec!["ride".into(), ride.path().into(), "--json".into()];
    arguments.extend(speed_matched_prefix_commands());
    arguments.push("place FloaterHill12x96 --matched".into());
    let output = run_cli(&arguments, "").unwrap();
    let replies: serde_json::Value = serde_json::from_str(&output).unwrap();
    let matched_hill = replies.as_array().unwrap().last().unwrap();
    let entry_speed = matched_hill["estimate"]["entrySpeed"].as_f64().unwrap();
    let window = matched_hill["estimate"]["ratedWindow"].as_array().unwrap();

    assert!(
        matched_hill["placed"]
            .as_str()
            .unwrap()
            .starts_with("FloaterHill")
    );
    assert_eq!(matched_hill["requested"], "FloaterHill12x96");
    assert!(window[0].as_f64().unwrap() <= entry_speed);
    assert!(entry_speed <= window[1].as_f64().unwrap());
}

fn speed_matched_prefix_commands() -> Vec<String> {
    [
        "chain on",
        "place LevelToGentleUp",
        "place GentleUp",
        "place GentleUp",
        "place GentleUpToLevel",
        "chain off",
        "place LevelToGentleDown",
        "place GentleDown",
        "place GentleDown",
        "place GentleDown",
        "place GentleDownToLevel",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn hill_estimate_labels_the_open_track_estimate_and_closed_form_prediction() {
    let ride = TempFile::new("hill-estimate.ride.json");
    ride.write(OPEN_RIDE);

    let json_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "place LaunchLong --launch-speed 30".into(),
            "estimate EjectorHill8x48".into(),
        ],
        "",
    )
    .unwrap();
    let estimates: serde_json::Value = serde_json::from_str(&json_reply).unwrap();
    let estimate = &estimates[1][0];
    assert_eq!(
        estimate["forceEstimateMethod"],
        "open-track candidate simulation"
    );
    assert_eq!(
        estimate["hillPrediction"]["method"],
        "closed-form hill inspector prediction"
    );
    assert!(
        estimate["peaks"]["minimumVerticalG"].as_f64().is_some(),
        "{estimate}"
    );
    assert!(
        estimate["hillPrediction"]["minimumVerticalG"]
            .as_f64()
            .is_some(),
        "{estimate}"
    );

    let text_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "estimate EjectorHill8x48".into(),
        ],
        "",
    )
    .unwrap();
    assert!(
        text_reply.contains("candidate preview vertical g"),
        "{text_reply}"
    );
    assert!(
        text_reply.contains("hill inspector prediction (closed-form)"),
        "{text_reply}"
    );
}

#[test]
fn choices_include_every_model_hill_and_speed_turn_variant_with_rated_windows() {
    let ride = TempFile::new("all-choices.ride.json");
    ride.write(OPEN_RIDE);
    let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
    simulation.construction_select("model:hybrid");
    simulation.construction_start(200, 200, 20, 0);
    let hill_options: serde_json::Value =
        serde_json::from_str(&simulation.construction_hill_options()).unwrap();
    let speed_turns: serde_json::Value =
        serde_json::from_str(&simulation.construction_speed_turns()).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "choices".into(),
        ],
        "",
    )
    .unwrap();
    let rows: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let rows = rows.as_array().unwrap();

    for variant in hill_options
        .as_array()
        .unwrap()
        .iter()
        .chain(speed_turns.as_array().unwrap())
    {
        let id = variant["id"].as_str().unwrap();
        let row = rows
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("choices omitted offered variant {id}"));
        assert!(row["ratedWindow"].as_array().is_some(), "{id}: {row}");
        assert_eq!(row["ratedWindow"].as_array().unwrap().len(), 2, "{id}");
        assert!(row["endTile"]["x"].as_f64().is_some(), "{id}: {row}");
        assert!(row["endTile"]["y"].as_f64().is_some(), "{id}: {row}");
        assert!(row["endHeight"].as_f64().is_some(), "{id}: {row}");
        assert!(row["length"].as_f64().is_some(), "{id}: {row}");
    }
}

#[test]
fn shaped_placement_persists_intent_and_resolved_geometry_for_replay() {
    let ride = TempFile::new("shaped-placement.ride.json");
    let session = TempFile::new("shaped-placement.session.json");

    run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "new --model hyper --station 241,249,10,north --cars 4".into(),
            "chain on".into(),
            "place LevelToGentleUp".into(),
            "place GentleToSteepUp".into(),
            "place SteepUp".into(),
            "place SteepUp".into(),
            "place SteepToGentleUp".into(),
            "place GentleUpToLevel".into(),
            "chain off".into(),
            "place WideLevelToSixtyDown".into(),
            "place WideSixtyDownToLevel".into(),
            "place turn left feel=banked radius=64".into(),
        ],
        "",
    )
    .unwrap();

    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    let shaped_turn = saved["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["intent"]["kind"] == "turn")
        .unwrap();
    assert_eq!(shaped_turn["intent"]["kind"], "turn");
    assert_eq!(shaped_turn["intent"]["hand"], "Left");
    assert_eq!(shaped_turn["spec"]["kind"], "turn");
    assert!(shaped_turn["spec"]["design_speed_mps"].as_f64().unwrap() >= 22.0);
    assert!(shaped_turn["spec"]["geometry"]["knots"].is_array());
    assert_six_decimal_precision(&shaped_turn["spec"]);

    let replay = run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    assert!(replay.contains("Pieces: 9"), "{replay}");

    run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "export".into(),
            "--session".into(),
            session.path().into(),
        ],
        "",
    )
    .unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&fs::read(&session.0).unwrap()).unwrap();
    let resolved_append = exported["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["command"]["type"] == "construction-append-resolved")
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            resolved_append["command"]["intentJson"].as_str().unwrap()
        )
        .unwrap(),
        shaped_turn["intent"]
    );
    let exported_spec: serde_json::Value =
        serde_json::from_str(resolved_append["command"]["specJson"].as_str().unwrap()).unwrap();
    let saved_spec = &shaped_turn["spec"];
    for field in [
        "kind",
        "design_speed_mps",
        "cars",
        "hand",
        "vertical_g",
        "lateral_g",
        "radius_m",
        "bank_degrees",
    ] {
        assert_eq!(
            exported_spec[field], saved_spec[field],
            "spec field {field}"
        );
    }
    assert_eq!(
        exported_spec["geometry"]["length"],
        saved_spec["geometry"]["length"]
    );
    assert_eq!(
        exported_spec["geometry"]["peak_bank"],
        saved_spec["geometry"]["peak_bank"]
    );
    assert_eq!(
        exported_spec["geometry"]["knots"].as_array().unwrap().len(),
        saved_spec["geometry"]["knots"].as_array().unwrap().len()
    );
    for (knot_index, (exported, saved)) in exported_spec["geometry"]["knots"]
        .as_array()
        .unwrap()
        .iter()
        .zip(saved_spec["geometry"]["knots"].as_array().unwrap())
        .enumerate()
    {
        assert_eq!(exported, saved, "resolved geometry knot {knot_index}");
    }
}

#[test]
fn shaped_hill_and_fully_banked_turn_place_on_a_hyper_and_replay() {
    let ride = TempFile::new("shaped-hyper.ride.json");

    let placement_output = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "new --model hyper --station 241,249,10,north --cars 4".into(),
            "chain on".into(),
            "place LevelToGentleUp".into(),
            "place GentleToSteepUp".into(),
            "place SteepUp".into(),
            "place SteepUp".into(),
            "place SteepToGentleUp".into(),
            "place GentleUpToLevel".into(),
            "chain off".into(),
            "place WideLevelToSixtyDown".into(),
            "place WideSixtyDownToLevel".into(),
            "place hill feel=floater".into(),
            "place turn left feel=banked".into(),
        ],
        "",
    )
    .unwrap();
    assert!(
        placement_output.contains("Shaped hill: floater"),
        "{placement_output}"
    );
    assert!(
        placement_output.contains("fully banked turn"),
        "{placement_output}"
    );

    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["model"], "hyper");
    let shape_steps: Vec<_> = saved["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|step| step.get("intent").is_some())
        .collect();
    assert_eq!(shape_steps.len(), 2);
    assert_eq!(shape_steps[0]["intent"]["kind"], "hill");
    assert_eq!(shape_steps[0]["intent"]["feel"], "Floater");
    assert_eq!(shape_steps[0]["spec"]["kind"], "hill");
    assert!(shape_steps[0]["spec"]["design_speed_mps"].as_f64().unwrap() >= 22.0);
    assert!(shape_steps[0]["spec"]["profile"]["points"].is_array());
    assert_eq!(shape_steps[1]["intent"]["kind"], "turn");
    assert_eq!(shape_steps[1]["intent"]["hand"], "Left");
    assert_eq!(shape_steps[1]["spec"]["kind"], "turn");
    assert!(shape_steps[1]["spec"]["design_speed_mps"].as_f64().unwrap() >= 22.0);
    assert!(shape_steps[1]["spec"]["geometry"]["knots"].is_array());

    let replay = run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    assert!(replay.contains("Pieces: 10"), "{replay}");
}

#[test]
fn probe_reports_candidate_geometry_and_does_not_change_the_ride_file() {
    let ride = TempFile::new("probe.ride.json");
    ride.write(OPEN_RIDE);
    let before = fs::read(&ride.0).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "probe Straight".into(),
        ],
        "",
    )
    .unwrap();
    let probe: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(probe["id"], "Straight");
    assert!(probe["entrySpeed"].as_f64().is_some());
    assert!(probe["peaks"].is_object());
    assert!(probe["endTile"]["x"].as_f64().is_some());
    assert!(probe["endTile"]["y"].as_f64().is_some());
    assert!(probe["endHeight"].as_f64().is_some());
    assert!(probe["length"].as_f64().is_some());
    assert_eq!(probe["endTile"]["x"], 108.0);
    assert_eq!(probe["endTile"]["y"], 100.0);
    assert_eq!(probe["endHeight"], 10.0);
    assert_eq!(probe["length"], 16.0);
    assert_eq!(fs::read(&ride.0).unwrap(), before);
}

#[test]
fn known_piece_disallowed_by_the_model_reports_the_refusal_reason() {
    let ride = TempFile::new("unoffered.ride.json");
    ride.write(&OPEN_RIDE.replace("\"hybrid\"", "\"wooden_twister\""));

    let error = run_cli(
        &["ride".into(), ride.path().into(), "probe Loop".into()],
        "",
    )
    .unwrap_err();

    assert!(error.contains("not offered at this end"), "{error}");
    assert!(error.contains("cannot use this piece"), "{error}");
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
        "Pieces: 0\nOpen end: tile (104.0, 100.0), height 10.0 m, heading East, slope Level, bank Level\nHeight above ground: 2 steps; offset to station entry: 16.0 m (x -4.0, y +0.0 tiles, height +0.0 m)\nClosure approach: remaining offset: 4.0 tiles (16.0 m); heading mismatch: 0 degrees\nChain: false; predicted end speed: 2.0 m/s (reachedEnd)\nCan close: false; can finish: false (Checking for a connection to the station.); can test: false\n"
    );
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["model"], "hybrid");
    assert_eq!(saved["station"]["heading"], "east");
    assert_eq!(saved["carsPerTrain"], 4);
}
