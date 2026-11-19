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
    for report in reports.as_array().unwrap() {
        assert_eq!(
            report["agreementGate"],
            "enabled; agreement failures exit nonzero"
        );
        assert!(report["expectationFailures"].as_array().unwrap().is_empty());
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
            }
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
                "FinishedEstimateVsWorstCar" | "PlacementEstimateVsWorstCar"
            ));
            assert!(failure["metric"].is_string());
            assert!(failure["reason"].is_string());
        }
    }
    assert_eq!(output.exit_code, i32::from(has_agreement_failures));
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
    assert!(output.contains("finished[min/max/lat/long g]"), "{output}");
    assert!(output.contains("front[min/max/lat/long g]"), "{output}");
    assert!(output.contains("worst[min/max/lat/long g]"), "{output}");
    assert!(output.contains("Agreement:"), "{output}");
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
