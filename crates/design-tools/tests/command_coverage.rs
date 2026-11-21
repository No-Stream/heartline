use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::{run_cli, run_cli_with_status};

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempRide(PathBuf);

impl TempRide {
    fn new() -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-command-{}-{sequence}.ride.json",
            std::process::id()
        )))
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

const OPEN_RIDE: &str = r#"{
  "format": "heartline-ride",
  "version": 1,
  "name": "CLI command coverage ride",
  "model": "hybrid",
  "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
  "carsPerTrain": 4,
  "trains": 1,
  "steps": []
}"#;

const CLOSED_RIDE: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");
const LAUNCH_RIDE: &str = include_str!("../../../content/rides/velocity-viper.ride.json");
const BLOCK_RIDE: &str = r#"{
  "format": "heartline-ride",
  "version": 1,
  "name": "Two-train block test ride",
  "model": "hybrid",
  "station": { "x": 100, "y": 100, "z": 20, "heading": "east" },
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
    { "place": "TurnLargeExit" }
  ]
}"#;

#[test]
fn commands_can_be_read_from_standard_input() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();

    let reply = run_cli(
        &["ride".into(), ride.path().into()],
        "state\nchoices straight\nestimate Straight\n",
    )
    .unwrap();

    assert!(reply.starts_with("Pieces: 0\nOpen end:"), "{reply}");
    assert!(reply.contains("Straight"), "{reply}");
    assert!(reply.contains("entry m/s"), "{reply}");
}

#[test]
fn state_reuses_replay_estimates_without_changing_the_reply() {
    let ride = TempRide::new();
    let mut ride_json: serde_json::Value = serde_json::from_str(OPEN_RIDE).unwrap();
    ride_json["steps"] = serde_json::json!([{ "place": "Straight" }]);
    fs::write(&ride.0, serde_json::to_vec(&ride_json).unwrap()).unwrap();

    let replay_estimate_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "state".into(),
        ],
        "",
    )
    .unwrap();
    let replay_state: serde_json::Value = serde_json::from_str(&replay_estimate_reply).unwrap();

    let recomputed_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "choices straight".into(),
            "state".into(),
        ],
        "",
    )
    .unwrap();
    let recomputed_replies: serde_json::Value = serde_json::from_str(&recomputed_reply).unwrap();

    assert_eq!(replay_state, recomputed_replies[1]);
}

#[test]
fn ride_replay_snapshot_reuses_state_and_invalidates_exact_source_changes() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();
    let mut snapshot_path = ride.0.as_os_str().to_owned();
    snapshot_path.push(".snapshot");
    let snapshot_path = PathBuf::from(snapshot_path);

    let first_reply = run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    let first_snapshot = fs::read(&snapshot_path).unwrap();
    let warm_reply = run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    assert_eq!(warm_reply, first_reply);

    fs::write(&ride.0, format!("{OPEN_RIDE}\n")).unwrap();
    let invalidated_reply =
        run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    assert_eq!(invalidated_reply, first_reply);
    let mut source_updated_snapshot = fs::read(&snapshot_path).unwrap();
    assert_ne!(source_updated_snapshot, first_snapshot);

    source_updated_snapshot[4] = 2;
    fs::write(&snapshot_path, source_updated_snapshot).unwrap();
    let version_invalidated_reply =
        run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();
    assert_eq!(version_invalidated_reply, first_reply);
    assert_eq!(fs::read(snapshot_path).unwrap()[4], 1);
}

#[test]
fn corrupt_matching_ride_replay_snapshot_fails_loudly() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();
    run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap();

    let mut snapshot_path = ride.0.as_os_str().to_owned();
    snapshot_path.push(".snapshot");
    let snapshot_path = PathBuf::from(snapshot_path);
    let mut snapshot = fs::read(&snapshot_path).unwrap();
    *snapshot.last_mut().unwrap() ^= 1;
    fs::write(snapshot_path, snapshot).unwrap();

    let error = run_cli(&["ride".into(), ride.path().into(), "state".into()], "").unwrap_err();
    assert!(error.contains("replay snapshot"), "{error}");
    assert!(error.contains("is corrupt"), "{error}");
}

#[test]
fn estimate_uses_the_palette_selection_and_prints_force_ratings() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();

    let json_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "estimate Loop".into(),
            "place Loop".into(),
        ],
        "",
    )
    .unwrap();
    let replies: serde_json::Value = serde_json::from_str(&json_reply).unwrap();
    let estimate = &replies[0][0];
    assert_eq!(estimate["id"], replies[1]["placed"]);
    let ratings = estimate["ratings"].as_object().unwrap();
    assert_eq!(ratings.len(), 4);

    let text_ride = TempRide::new();
    fs::write(&text_ride.0, OPEN_RIDE).unwrap();
    let text_reply = run_cli(
        &[
            "ride".into(),
            text_ride.path().into(),
            "estimate Loop".into(),
        ],
        "",
    )
    .unwrap();
    assert!(text_reply.contains("vertical g (rating)"), "{text_reply}");
    assert!(text_reply.contains("lateral g (rating)"), "{text_reply}");
    assert!(text_reply.contains("long g (rating)"), "{text_reply}");
    for rating in ratings.values() {
        assert!(
            text_reply.contains(&format!("({})", rating.as_str().unwrap())),
            "{text_reply}"
        );
    }
}

#[test]
fn chain_and_track_controls_update_the_saved_ride() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "chain on".into(),
            "place Straight".into(),
            "chain off".into(),
            "cars 5".into(),
            "map".into(),
        ],
        "",
    )
    .unwrap();

    assert!(reply.contains("Chain lift: on"), "{reply}");
    assert!(reply.contains("Chain lift: off"), "{reply}");
    assert!(reply.contains("Cars per train: 5"), "{reply}");
    assert!(reply.contains("Footprint (each cell"), "{reply}");
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["carsPerTrain"], 5);
    assert_eq!(saved["trains"], 1);
    assert_eq!(saved["steps"][0]["chain"], true);
    assert_eq!(saved["steps"][1]["place"], "Straight");
    assert_eq!(saved["steps"][2]["chain"], false);
}

#[test]
fn train_count_refusal_preserves_the_ride_file() {
    let ride = TempRide::new();
    fs::write(&ride.0, OPEN_RIDE).unwrap();
    let before = fs::read(&ride.0).unwrap();

    let error = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "place Straight".into(),
            "trains 2".into(),
        ],
        "",
    )
    .unwrap_err();

    assert!(
        error.contains("within the available block capacity"),
        "{error}"
    );
    assert_eq!(fs::read(&ride.0).unwrap(), before);
}

#[test]
fn launch_speed_targets_a_placed_launch_and_persists_it() {
    let ride = TempRide::new();
    let mut launch_ride: serde_json::Value = serde_json::from_str(LAUNCH_RIDE).unwrap();
    launch_ride["station"]["x"] = serde_json::json!(100);
    launch_ride["station"]["y"] = serde_json::json!(100);
    launch_ride["steps"] = serde_json::json!([{ "place": "LaunchLong" }]);
    fs::write(&ride.0, serde_json::to_vec(&launch_ride).unwrap()).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "launch-speed 1 25".into(),
        ],
        "",
    )
    .unwrap();

    assert!(
        reply.contains("Launch speed for piece 1: 25.0 m/s"),
        "{reply}"
    );
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["steps"][0]["launchSpeed"], 25.0);
}

#[test]
fn launch_speed_uses_the_placement_index_after_a_multi_piece_element() {
    let ride = TempRide::new();
    let mut launch_ride: serde_json::Value = serde_json::from_str(LAUNCH_RIDE).unwrap();
    launch_ride["station"]["x"] = serde_json::json!(100);
    launch_ride["station"]["y"] = serde_json::json!(100);
    launch_ride["steps"] = serde_json::json!([
        { "place": "LaunchLong" },
        { "place": "element:small-left-banked-turn" },
        { "place": "LaunchLong" }
    ]);
    fs::write(&ride.0, serde_json::to_vec(&launch_ride).unwrap()).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "launch-speed 3 25".into(),
        ],
        "",
    )
    .unwrap();

    assert!(
        reply.contains("Launch speed for piece 3: 25.0 m/s"),
        "{reply}"
    );
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["steps"][2]["launchSpeed"], 25.0);
    let state_reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "state".into(),
        ],
        "",
    )
    .unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_reply).unwrap();
    assert_eq!(state["track"].as_array().unwrap().len(), 6);
}

#[test]
fn undo_removes_a_multi_piece_element_as_one_ride_step() {
    let ride = TempRide::new();
    let mut launch_ride: serde_json::Value = serde_json::from_str(LAUNCH_RIDE).unwrap();
    launch_ride["station"]["x"] = serde_json::json!(100);
    launch_ride["station"]["y"] = serde_json::json!(100);
    launch_ride["steps"] = serde_json::json!([{ "place": "LaunchLong" }]);
    fs::write(&ride.0, serde_json::to_vec(&launch_ride).unwrap()).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "place element:small-left-banked-turn".into(),
            "undo".into(),
        ],
        "",
    )
    .unwrap();

    let replies: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert!(replies[0]["state"]["pieceCount"].as_u64().unwrap() > 1);
    assert_eq!(replies[1]["pieceCount"], 1);
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["steps"].as_array().unwrap().len(), 1);
    assert_eq!(saved["steps"][0]["place"], "LaunchLong");
}

#[test]
fn auto_blocks_persists_the_best_layout_within_the_work_budget() {
    let ride = TempRide::new();
    fs::write(&ride.0, BLOCK_RIDE).unwrap();

    let reply = run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            "--json".into(),
            "close".into(),
            "auto-blocks".into(),
        ],
        "",
    )
    .unwrap();

    let replies: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(replies[1]["searchWorkBudget"], 1_000);
    assert!(replies[1]["searchWorkCompleted"].as_u64().unwrap() <= 1_000);
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    assert_eq!(saved["steps"].as_array().unwrap()[19]["close"], true);
    assert!(saved["steps"].as_array().unwrap()[20]["blockSections"].is_array());
    assert_eq!(
        saved["steps"][20]["blockSections"],
        replies[1]["blockSections"]
    );
    assert_eq!(
        saved["steps"][20]["trainCapacity"],
        replies[1]["trainCapacity"]
    );
    assert_eq!(
        saved["steps"][20]["capacitySearchComplete"],
        replies[1]["capacitySearchComplete"]
    );
    assert_eq!(
        saved["steps"][20]["placementSearchComplete"],
        replies[1]["placementSearchComplete"]
    );
    assert!(saved["steps"][20].get("autoBlocks").is_none());
    assert_eq!(saved["trains"], 1);
    let fresh_ride = TempRide::new();
    fs::copy(&ride.0, &fresh_ride.0).unwrap();
    let replayed = run_cli(
        &["ride".into(), fresh_ride.path().into(), "state".into()],
        "",
    )
    .unwrap();
    assert!(replayed.contains("Pieces:"), "{replayed}");
}

#[test]
fn truncate_removes_steps_and_finish_apply_persists_the_completion() {
    let ride = TempRide::new();
    fs::write(&ride.0, LAUNCH_RIDE).unwrap();
    let source: serde_json::Value = serde_json::from_str(LAUNCH_RIDE).unwrap();
    let kept_steps = source["steps"].as_array().unwrap().len() - 4;
    run_cli(
        &[
            "ride".into(),
            ride.path().into(),
            format!("truncate {kept_steps}"),
        ],
        "",
    )
    .unwrap();

    let reply = run_cli(
        &["ride".into(), ride.path().into(), "finish apply 1".into()],
        "",
    )
    .unwrap();

    assert!(
        reply.starts_with("Placed finish option 1 (3 piece(s)) and closed the circuit.\n"),
        "{reply}"
    );
    assert!(reply.contains("can test: true"), "{reply}");
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&ride.0).unwrap()).unwrap();
    let steps = saved["steps"].as_array().unwrap();
    assert_eq!(steps.len(), kept_steps + 4);
    assert_eq!(steps.last().unwrap(), &serde_json::json!({ "close": true }));
    let fresh_ride = TempRide::new();
    fs::copy(&ride.0, &fresh_ride.0).unwrap();
    let replayed = run_cli(
        &["ride".into(), fresh_ride.path().into(), "state".into()],
        "",
    )
    .unwrap();
    assert!(replayed.contains("can test: true"), "{replayed}");
}

#[test]
fn report_samples_and_agreement_run_on_the_closed_ride() {
    let ride = TempRide::new();
    fs::write(&ride.0, CLOSED_RIDE).unwrap();

    let samples = run_cli(
        &["ride".into(), ride.path().into(), "report --samples".into()],
        "",
    )
    .unwrap();
    assert!(samples.contains("Ride: Timber Tumbler"), "{samples}");
    assert!(
        samples.contains("Samples (front car, one metre spacing):"),
        "{samples}"
    );

    let agreement = run_cli(&["ride".into(), ride.path().into(), "agreement".into()], "").unwrap();
    assert!(agreement.contains("Agreement:"), "{agreement}");
}

#[test]
fn check_json_exposes_worker_estimates_and_raw_agreement_values() {
    let ride = TempRide::new();
    fs::write(
        &ride.0,
        r#"{
      "format": "heartline-ride",
      "version": 1,
      "name": "Browser agreement fixture",
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
    }"#,
    )
    .unwrap();

    let output = run_cli_with_status(&["check".into(), ride.path().into(), "--json".into()], "");
    assert_eq!(
        output.exit_code, 1,
        "stdout: {}\nstderr: {}",
        output.stdout, output.stderr
    );
    assert!(output.stderr.is_empty(), "{}", output.stderr);
    let checks: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(
        checks[0]["agreementGate"],
        "enabled; finished-track agreement failures exit nonzero"
    );
    assert!(
        checks[0]["expectationFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let force_headroom = checks[0]["forceHeadroom"].as_array().unwrap();
    assert_eq!(force_headroom.len(), 10);
    assert!(force_headroom.iter().any(|result| {
        result["physics_configuration"] == "default physics"
            && result["unavailable_reason"].is_string()
            && result["observed_g"].is_null()
            && result["headroom_g"].is_null()
            && result["passed"] == false
    }));
    assert!(
        checks[0]["forceHeadroomFailures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure.as_str().unwrap().contains("unavailable"))
    );
    assert_eq!(checks[0]["agreement"]["passed"], false);
    let agreement = &checks[0]["agreement"];
    assert!(
        agreement["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| {
                failure["check"] == "FinishedEstimateVsFrontCar"
                    && failure["reason"].as_str().unwrap().contains("unavailable")
            })
    );
    let browser_estimates = checks[0]["browserEstimates"].as_array().unwrap();
    assert_eq!(browser_estimates.len(), 5);
    assert_eq!(browser_estimates[0]["stepIndex"], 1);
    assert_eq!(
        browser_estimates.last().unwrap()["selection"]["candidateForcePreview"]["status"],
        "stalled"
    );
    let estimate_error_failures = agreement["failures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|failure| {
            failure["check"] == "PlacementEstimateVsWorstCar"
                && failure["classification"] == "estimate error"
        })
        .collect::<Vec<_>>();
    assert!(!estimate_error_failures.is_empty());
    assert!(estimate_error_failures.iter().all(|failure| {
        failure["cause"]
            .as_str()
            .is_some_and(|cause| !cause.is_empty())
    }));
    for stage in ["selection", "placement"] {
        let snapshot = &browser_estimates[0][stage];
        assert_eq!(snapshot["selected"], "TurnSmallStandalone");
        assert!(snapshot["constructionForcePreview"].is_object());
        assert!(snapshot["candidateForcePreview"].is_object());
        assert!(snapshot["speedMatch"].is_null() || snapshot["speedMatch"].is_object());
        assert!(snapshot["hillEstimate"].is_null() || snapshot["hillEstimate"].is_object());
    }

    let agreement_row = &checks[0]["agreement"]["rows"][0];
    assert!(agreement_row["placementEntrySpeedEstimate"].is_number());
    assert!(agreement_row["finishedEntrySpeedEstimate"].is_number());
    assert!(agreement_row["entrySpeedMeasuredFront"].is_number());
    for metric in [
        "placementEstimate",
        "finishedTrackEstimate",
        "measuredFront",
        "measuredWorst",
    ] {
        assert!(
            agreement_row[metric].is_object(),
            "missing {metric}: {agreement_row}"
        );
    }
}

#[test]
fn check_fails_fast_on_shaped_geometry_saved_by_an_older_solver() {
    let ride_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/e2e/rides/fixtures/horizon-hawk-older-solver.ride.json"
    );
    let output = run_cli_with_status(&["check".into(), ride_path.into()], "");
    assert_eq!(output.exit_code, 1, "stdout: {}", output.stdout);
    assert!(output.stdout.is_empty(), "{}", output.stdout);
    assert!(
        output.stderr.contains("stale shaped geometry: ride step 11 at ")
            && output.stderr.contains("saved shaped geometry digest 7a165ba6bd127329 differs from the current solver's 0f412105af8180cd"),
        "{}",
        output.stderr
    );
}

#[test]
fn shaped_hill_text_uses_two_decimals_and_estimate_prints_the_shaped_forces() {
    let ride = TempRide::new();
    fs::write(
        &ride.0,
        include_str!("../../../content/rides/horizon-hawk.ride.json"),
    )
    .unwrap();
    let hill = "shape:hill:strongFloater:default:auto";
    run_cli(&["ride".into(), ride.path().into(), "truncate 10".into()], "").unwrap();
    let probe_json: serde_json::Value = serde_json::from_str(
        &run_cli(
            &[
                "ride".into(),
                ride.path().into(),
                "--json".into(),
                format!("probe {hill}"),
            ],
            "",
        )
        .unwrap(),
    )
    .unwrap();
    let spec = &probe_json["shape"]["spec"];
    let target = spec["targetG"].as_f64().unwrap();
    let delivered = spec["deliveredG"].as_f64().unwrap();
    assert_eq!(target, -0.85);
    let shape_line = format!("target {target:.2} g, delivered {delivered:.2} g");

    let probe = run_cli(
        &["ride".into(), ride.path().into(), format!("probe {hill}")],
        "",
    )
    .unwrap();
    assert!(probe.contains(&shape_line), "{probe}");

    let estimate = run_cli(
        &["ride".into(), ride.path().into(), format!("estimate {hill}")],
        "",
    )
    .unwrap();
    assert!(
        estimate.contains("  Shaped hill: strongFloater at "),
        "{estimate}"
    );
    assert!(estimate.contains(&shape_line), "{estimate}");

    let shapes = run_cli(
        &["ride".into(), ride.path().into(), "options shapes".into()],
        "",
    )
    .unwrap();
    let strong_floater_row = shapes
        .lines()
        .find(|line| line.starts_with(hill))
        .unwrap_or_else(|| panic!("options shapes omits {hill}:\n{shapes}"));
    assert!(
        strong_floater_row.contains(&format!(" {delivered:.2} ")),
        "{shapes}"
    );
}
