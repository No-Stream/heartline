use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;
use theme_park_design_tools::run_cli_with_status;

static NEXT_TEMP_RIDE: AtomicUsize = AtomicUsize::new(0);

struct TempRide(PathBuf);

impl TempRide {
    fn new() -> Self {
        let sequence = NEXT_TEMP_RIDE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-agreement-{}-{sequence}.ride.json",
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

fn ride_files() -> Vec<PathBuf> {
    let rides_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/rides");
    let mut paths: Vec<_> = fs::read_dir(rides_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    paths
}

fn write_expectation_failure_ride(ride: &TempRide, expectations: serde_json::Value) {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/rides/browser-agreement-fast.ride.json"),
    )
    .unwrap();
    let mut ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    ride_json["expect"] = expectations;
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();
}

#[test]
fn short_straight_and_hill_ride_runs_through_native_agreement() {
    let ride = TempRide::new();
    let ride_path = ride.path();
    fs::write(
        &ride.0,
        r#"{
            "format": "heartline-ride",
            "version": 1,
            "name": "Short straight hill ride",
            "model": "hybrid",
            "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
            "carsPerTrain": 4,
            "trains": 1,
            "steps": [
                { "place": "Launch", "launchSpeed": 14.0 },
                { "place": "AirtimeHill" },
                { "place": "TurnSmallStandalone" },
                { "place": "TurnSmallStandalone" },
                { "place": "FullOffsetLeft" },
                { "place": "Straight" },
                { "place": "FullOffsetRight" },
                { "place": "TurnSmallStandalone" },
                { "place": "TurnSmallStandalone" },
                { "close": true }
            ]
        }"#,
    )
    .unwrap();

    let agreement = run_cli(&["ride".into(), ride_path, "agreement".into()], "").unwrap();

    assert!(agreement.contains("AirtimeHill"), "{agreement}");
    assert!(agreement.contains("Agreement:"), "{agreement}");
}

#[test]
fn ride_report_json_includes_both_physics_element_forces_and_brake_speeds() {
    let ride_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/rides/timber-tumbler.ride.json")
        .display()
        .to_string();
    let output = run_cli(
        &["ride".into(), ride_path, "report".into(), "--json".into()],
        "",
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    let element_physics = report["elementPhysics"].as_array().unwrap();
    assert_eq!(
        element_physics.len(),
        report["estimates"].as_array().unwrap().len()
    );
    assert!(element_physics.iter().any(|element| {
        element["default_physics"]["sampled_entry_speed_mps"].is_number()
            && element["construction_test_physics"]["sampled_entry_speed_mps"].is_number()
            && element["default_physics"]["minimum_vertical_sustained"]["car_index"].is_number()
            && element["construction_test_physics"]["minimum_vertical_sustained"]["car_index"]
                .is_number()
            && element["default_physics"]["maximum_lateral"]["car_index"].is_number()
            && element["construction_test_physics"]["maximum_lateral"]["car_index"].is_number()
    }));

    let brake_run = report["brakeRuns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| {
            run["default_physics"]["sampled_speed_removed_mps"].is_number()
                && run["construction_test_physics"]["sampled_speed_removed_mps"].is_number()
        })
        .expect("a measured brake run is reported under both physics configurations");
    for configuration in ["default_physics", "construction_test_physics"] {
        let entry_speed = brake_run[configuration]["sampled_entry_speed_mps"]
            .as_f64()
            .unwrap();
        let exit_speed = brake_run[configuration]["sampled_exit_speed_mps"]
            .as_f64()
            .unwrap();
        let speed_removed = brake_run[configuration]["sampled_speed_removed_mps"]
            .as_f64()
            .unwrap();
        assert!(entry_speed > exit_speed);
        assert!((speed_removed - (entry_speed - exit_speed)).abs() < 1e-9);
    }
}

#[test]
fn native_agreement_check_reports_every_ride_and_attributes_each_failure() {
    let rides = ride_files();
    let ride_names: Vec<_> = rides
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    for catalog_ride in [
        "briar-blitz.ride.json",
        "daisy-dash.ride.json",
        "horizon-hawk.ride.json",
        "timber-tumbler.ride.json",
        "velocity-viper.ride.json",
    ] {
        assert!(
            ride_names.iter().any(|name| name == catalog_ride),
            "missing catalog ride {catalog_ride} in {ride_names:?}"
        );
    }

    let mut arguments = vec!["check".to_owned(), "--json".to_owned()];
    arguments.extend(rides.iter().map(|path| path.display().to_string()));
    let output = run_cli_with_status(&arguments, "");
    assert!(output.stderr.is_empty(), "{}", output.stderr);
    let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(reports.as_array().unwrap().len(), rides.len());
    let mut has_agreement_failures = false;
    let mut has_design_margin_opt_in = false;
    for (report, path) in reports.as_array().unwrap().iter().zip(&rides) {
        let ride_json: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let design_margins_required = ride_json["expect"]["designMargins"] == true;
        assert_eq!(
            report["agreementGate"],
            "enabled; finished-track agreement failures exit nonzero"
        );
        assert!(report["expectationFailures"].as_array().unwrap().is_empty());
        assert!(
            report["defaultPhysicsExpectationFailures"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        has_agreement_failures |= !report["agreement"]["passed"].as_bool().unwrap();
        let rated_speed_margins = report["ratedSpeedMargins"].as_array().unwrap();
        if !rated_speed_margins.is_empty() {
            let physics_configurations: std::collections::BTreeSet<_> = rated_speed_margins
                .iter()
                .map(|margin| margin["physicsConfiguration"].as_str().unwrap())
                .collect();
            assert_eq!(
                physics_configurations,
                ["construction-test", "default"].into_iter().collect()
            );
            for margin in rated_speed_margins {
                assert!(margin["pieceId"].is_string());
                assert!(margin["entrySpeed"].is_number() || margin["entrySpeed"].is_null());
                assert!(margin["ratedWindow"].is_array());
                assert_eq!(
                    margin["status"],
                    if !margin["unavailableReason"].is_null() {
                        "UNAVAILABLE"
                    } else {
                        match margin["ruleMargin"].as_f64() {
                            Some(rule_margin) if rule_margin >= 0.0 => "PASS",
                            Some(_) => "FAIL",
                            None => "UNAVAILABLE",
                        }
                    },
                );
                if design_margins_required {
                    assert_eq!(margin["status"], "PASS", "{path:?}: {margin}");
                }
            }
        }
        if design_margins_required {
            has_design_margin_opt_in = true;
            assert!(
                report["ratedSpeedMarginFailures"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                report["forceHeadroomFailures"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                report["forceHeadroom"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|result| result["passed"] == true)
            );
        }
        let agreement = &report["agreement"];
        let rows = agreement["rows"].as_array().unwrap();
        assert!(!rows.is_empty(), "{} has no per-piece rows", report["path"]);
        for row in rows {
            assert!(row["pieceName"].is_string());
            for field in [
                "placementEntrySpeedEstimate",
                "finishedEntrySpeedEstimate",
                "entrySpeedMeasuredFront",
                "placementEstimate",
                "finishedTrackEstimate",
                "measuredFront",
                "measuredWorst",
                "check2",
                "check3",
            ] {
                assert!(row.get(field).is_some(), "missing {field} in {row}");
            }
        }
        for failure in agreement["failures"].as_array().unwrap() {
            assert!(matches!(
                failure["check"].as_str().unwrap(),
                "FinishedEstimateVsFrontCar"
                    | "FinishedEstimateVsWorstCar"
                    | "PlacementEstimateVsWorstCar"
            ));
            assert!(failure["metric"].is_string());
            assert!(failure["reason"].is_string());
        }
    }
    assert!(
        has_design_margin_opt_in,
        "no ride opted into design margins"
    );
    assert_eq!(output.exit_code, i32::from(has_agreement_failures));
}

#[test]
fn ride_expect_checks_the_compiled_heartlined_inversion_count() {
    let ride = TempRide::new();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/rides/timber-tumbler.ride.json");
    let source = fs::read_to_string(source).unwrap();
    let mut ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    ride_json["expect"]["features"]["heartlinedInversionsAtLeast"] = 2.into();
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let output = run_cli(&["check".into(), "--json".into(), ride.path()], "").unwrap();
    let reports: serde_json::Value = serde_json::from_str(&output).unwrap();
    let expectations = reports[0]["expectations"].as_array().unwrap();
    let heartlined_inversions = expectations
        .iter()
        .find(|result| result["metric"] == "heartlined inversion pieces")
        .unwrap();
    assert!(heartlined_inversions["observed"].as_u64().unwrap() >= 2);
    assert_eq!(heartlined_inversions["passed"], true);

    ride_json["expect"]["features"]["heartlinedInversionsAtLeast"] = 100.into();
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();
    let failed = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");
    assert_eq!(failed.exit_code, 1);
    let failed_reports: serde_json::Value = serde_json::from_str(&failed.stdout).unwrap();
    assert_eq!(
        failed_reports[0]["expectations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|result| result["metric"] == "heartlined inversion pieces")
            .unwrap()["passed"],
        false
    );
}

#[test]
fn agreement_command_prints_the_force_values_for_a_closed_ride() {
    let timber_tumbler = ride_files()
        .into_iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "timber-tumbler.ride.json")
        })
        .unwrap();
    let output = run_cli(
        &[
            "ride".into(),
            timber_tumbler.display().to_string(),
            "agreement".into(),
        ],
        "",
    )
    .unwrap();

    assert!(
        output.contains("entry[placement/finished/test m/s]"),
        "{output}"
    );
    assert!(output.contains("placement[min/max/lat/long g]"), "{output}");
    assert!(
        output.contains("finished-front[min/max/lat/long g]"),
        "{output}"
    );
    assert!(
        output.contains("finished-worst[min/max/lat/long g]"),
        "{output}"
    );
    assert!(
        output.contains("measured-front[min/max/lat/long g]"),
        "{output}"
    );
    assert!(
        output.contains("measured-worst[min/max/lat/long g]"),
        "{output}"
    );
    assert!(output.contains("Agreement:"), "{output}");
}

#[test]
fn timber_tumbler_steepening_descent_placement_matches_the_closed_run() {
    let timber_tumbler = ride_files()
        .into_iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "timber-tumbler.ride.json")
        })
        .unwrap();
    let output = run_cli(
        &[
            "ride".into(),
            timber_tumbler.display().to_string(),
            "agreement".into(),
            "--json".into(),
        ],
        "",
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    let piece = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["pieceName"] == "GentleToSteepDown")
        .unwrap();
    let placement_longitudinal = piece["placementEstimate"]["maximumLongitudinalG"]
        .as_f64()
        .unwrap();
    let measured_longitudinal = piece["measuredWorst"]["maximumLongitudinalG"]
        .as_f64()
        .unwrap();

    assert!(
        (placement_longitudinal - measured_longitudinal).abs() <= 0.3,
        "Timber Tumbler GentleToSteepDown placement estimate {placement_longitudinal:.3} g differs from its closed-run measurement {measured_longitudinal:.3} g"
    );
}

#[test]
fn check_prints_observed_values_for_passing_and_failing_expectations() {
    let ride = TempRide::new();
    write_expectation_failure_ride(
        &ride,
        serde_json::json!({
            "excitement": [100.0, 101.0],
            "sustainedMinVerticalG": [-1.2, -0.8],
            "inversions": 0
        }),
    );

    let text = run_cli(&["check".into(), ride.path()], "").unwrap_err();

    assert!(
        text.contains("Rated entry-speed margin (default physics"),
        "{text}"
    );
    assert!(
        text.contains("Rated entry-speed margin (construction-test physics"),
        "{text}"
    );
    assert!(
        text.contains("expectation excitement expected 100.00 to 101.00, observed "),
        "{text}"
    );
    assert!(
        text.contains("expectation inversions expected 0, observed 0 (pass)"),
        "{text}"
    );
    assert!(
        text.contains(
            "expectation sustained minimum vertical g expected -1.20 to -0.80, observed "
        ),
        "{text}"
    );
}

#[test]
fn check_fails_when_default_physics_expectations_fail() {
    let ride = TempRide::new();
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/rides/browser-agreement-fast.ride.json"),
    )
    .unwrap();
    let mut ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    ride_json["expect"] = serde_json::json!({"excitement": [0.0, 100.0]});
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let baseline = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");
    let baseline_reports: serde_json::Value = serde_json::from_str(&baseline.stdout).unwrap();
    let construction_excitement = baseline_reports[0]["expectations"][0]["observed"]
        .as_f64()
        .unwrap();
    let default_excitement = baseline_reports[0]["defaultPhysicsExpectations"][0]["observed"]
        .as_f64()
        .unwrap();
    assert_ne!(construction_excitement, default_excitement);

    ride_json["expect"]["excitement"] =
        serde_json::json!([construction_excitement, construction_excitement]);
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let output = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");

    assert_eq!(output.exit_code, 1);
    let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(
        reports[0]["defaultPhysicsExpectations"][0]["metric"],
        "excitement"
    );
    assert_eq!(reports[0]["defaultPhysicsExpectations"][0]["passed"], false);
    assert_eq!(reports[0]["expectations"][0]["passed"], true);
    assert!(
        reports[0]["expectationFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !reports[0]["defaultPhysicsExpectationFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn check_reports_design_margin_failures_without_gating_legacy_rides() {
    let ride = TempRide::new();
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/rides/browser-agreement-fast.ride.json"),
    )
    .unwrap();
    let ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let output = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");

    assert_eq!(output.exit_code, 0, "{}", output.stderr);
    let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(reports[0]["agreement"]["passed"], true);
    assert!(
        reports[0]["expectationFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        reports[0]["ratedSpeedMarginFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        reports[0]["forceHeadroomFailures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure.as_str().unwrap().contains("maximum lateral g"))
    );
}

#[test]
fn check_gates_design_margin_failures_when_opted_in() {
    let ride = TempRide::new();
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/rides/browser-agreement-fast.ride.json"),
    )
    .unwrap();
    let mut ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    ride_json["expect"]["designMargins"] = true.into();
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let output = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");

    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.is_empty(), "{}", output.stderr);
    let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(reports[0]["agreement"]["passed"], true);
    assert!(
        reports[0]["expectationFailures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        reports[0]["forceHeadroomFailures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure.as_str().unwrap().contains("maximum lateral g"))
    );
}

#[test]
fn check_supports_comfortable_force_and_ride_feature_expectations() {
    let ride = TempRide::new();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/rides/velocity-viper.ride.json");
    let source = fs::read_to_string(source).unwrap();
    let mut ride_json: serde_json::Value = serde_json::from_str(&source).unwrap();
    ride_json["expect"] = serde_json::json!({
        "withinComfortableForceLimits": true,
        "inversionsAtLeast": 0,
        "launches": 2,
        "biggestDropAtLeast": 0.0,
        "maxSpeedAtLeast": 0.0,
        "features": {
            "piecePrefixesAtLeast": {"LaunchLong": 2},
            "piecePrefixGroupsAtLeast": [
                {"prefixes": ["LaunchLong", "LaunchedTopHat"], "minimum": 3}
            ],
            "orderedSteps": [[{"placePrefix": "LaunchLong"}, {"place": "LaunchedTopHat"}]],
            "adjacentSteps": [[{"place": "Launch"}, {"place": "LaunchedTopHat"}]]
        }
    });
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();

    let output = run_cli(&["check".into(), "--json".into(), ride.path()], "").unwrap();
    let reports: serde_json::Value = serde_json::from_str(&output).unwrap();
    let expectations = reports[0]["expectations"].as_array().unwrap();

    assert_eq!(expectations.len(), 9);
    assert!(expectations.iter().all(|result| result["passed"] == true));
    assert_eq!(expectations[0]["metric"], "comfortable force limits");
    assert_eq!(expectations[1]["metric"], "inversions");
    assert_eq!(expectations[2]["metric"], "launches");
    assert_eq!(expectations[3]["metric"], "biggest drop");
    assert_eq!(expectations[4]["metric"], "maximum speed");
    assert_eq!(expectations[5]["metric"], "ride feature count");
    assert_eq!(expectations[6]["metric"], "grouped ride feature count");
    assert_eq!(expectations[6]["observed"], 3);
    assert_eq!(expectations[7]["metric"], "ordered ride feature steps");
    assert_eq!(expectations[8]["metric"], "adjacent ride feature steps");

    ride_json["expect"]["features"]["piecePrefixGroupsAtLeast"][0]["minimum"] = 4.into();
    fs::write(&ride.0, serde_json::to_vec_pretty(&ride_json).unwrap()).unwrap();
    let failed = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");
    assert_eq!(failed.exit_code, 1);
    let failed_reports: serde_json::Value = serde_json::from_str(&failed.stdout).unwrap();
    let grouped_expectation = failed_reports[0]["expectations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["metric"] == "grouped ride feature count")
        .unwrap();
    assert_eq!(grouped_expectation["observed"], 3);
    assert_eq!(grouped_expectation["passed"], false);
}

#[test]
fn failed_check_json_is_written_to_stdout_with_a_failure_exit_code() {
    let ride = TempRide::new();
    write_expectation_failure_ride(
        &ride,
        serde_json::json!({"excitement": [100.0, 101.0], "inversions": 0}),
    );

    let output = run_cli_with_status(&["check".into(), "--json".into(), ride.path()], "");

    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.is_empty(), "{}", output.stderr);
    let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(
        reports[0]["expectationFailures"].as_array().unwrap().len(),
        1
    );
    assert_eq!(reports[0]["expectations"][0]["passed"], false);
    assert!(reports[0]["expectations"][0]["observed"].is_number());
    assert_eq!(reports[0]["expectations"][1]["passed"], true);
    assert!(reports[0]["expectations"][1]["observed"].is_number());
}

#[test]
fn failed_text_check_prints_its_report_on_stdout_and_a_summary_on_stderr() {
    let ride = TempRide::new();
    write_expectation_failure_ride(&ride, serde_json::json!({"excitement": [100.0, 101.0]}));

    let output = Command::new(env!("CARGO_BIN_EXE_heartline-design"))
        .args(["check", &ride.path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stdout.contains("expectation excitement expected 100.00 to 101.00, observed "),
        "{stdout}"
    );
    assert!(stdout.contains("Rated entry-speed margin"), "{stdout}");
    assert_eq!(
        stderr,
        "check failed for 1 of 1 ride(s); the report is on stdout:\n  Browser agreement fast circuit: expectations, default physics expectations\n"
    );
}

#[test]
fn binary_check_json_failure_uses_stdout_and_exits_nonzero() {
    let ride = TempRide::new();
    write_expectation_failure_ride(&ride, serde_json::json!({"excitement": [100.0, 101.0]}));

    let output = Command::new(env!("CARGO_BIN_EXE_heartline-design"))
        .args(["check", "--json", &ride.path()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reports: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reports[0]["expectations"][0]["passed"], false);
}

#[test]
fn map_draws_every_closed_catalog_ride() {
    for path in ride_files() {
        let ride = TempRide::new();
        fs::copy(&path, &ride.0).unwrap();

        let map = run_cli(&["ride".into(), ride.path(), "map".into()], "")
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));

        assert!(map.contains("Footprint (each cell"), "{}", path.display());
    }
}

#[test]
fn state_reads_the_ground_under_an_open_end_far_from_the_origin() {
    let ride = TempRide::new();
    let reply = run_cli(
        &[
            "ride".into(),
            ride.path(),
            "new --model hyper --station 241,249,12,north".into(),
            "chain on".into(),
            "place LevelToGentleUp".into(),
            "state".into(),
        ],
        "",
    )
    .unwrap();

    assert!(
        reply.contains("height 16.0 m") && reply.contains("Height above ground: 8 steps"),
        "{reply}"
    );
}
