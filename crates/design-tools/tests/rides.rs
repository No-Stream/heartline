use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;

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
    let output = run_cli(&arguments, "").unwrap();
    let reports: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(reports.as_array().unwrap().len(), rides.len());
    for report in reports.as_array().unwrap() {
        assert_eq!(
            report["agreementGate"],
            "informational until work item 6; agreement failures exit zero"
        );
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
                "FinishedEstimateVsFrontCar" | "PlacementEstimateVsWorstCar"
            ));
            assert!(failure["metric"].is_string());
            assert!(failure["reason"].is_string());
        }
    }
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

    assert!(reply.contains("height 16.0 m") && reply.contains("Height above ground: 8 steps"), "{reply}");
}
