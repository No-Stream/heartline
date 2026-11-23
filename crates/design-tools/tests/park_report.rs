use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-park-report-{}-{sequence}-{name}",
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

fn write_park_fixture(description: &serde_json::Value, name: &str) -> TempFile {
    let park = TempFile::new(name);
    fs::write(&park.0, serde_json::to_vec(description).unwrap()).unwrap();
    park
}

fn report(park_path: &str) -> String {
    run_cli(
        &[
            "park".into(),
            "report".into(),
            park_path.into(),
            "--guests".into(),
            "16".into(),
            "--minutes".into(),
            "20".into(),
            "--seed".into(),
            "42".into(),
        ],
        "",
    )
    .unwrap()
}

fn json_report(park_path: &str) -> serde_json::Value {
    let output = run_cli(
        &[
            "park".into(),
            "report".into(),
            park_path.into(),
            "--guests".into(),
            "16".into(),
            "--minutes".into(),
            "1".into(),
            "--seed".into(),
            "42".into(),
            "--json".into(),
        ],
        "",
    )
    .unwrap();
    serde_json::from_str(&output).unwrap()
}

fn riders_per_hour(output: &str) -> (f64, f64) {
    let line = output
        .lines()
        .find(|line| line.starts_with("Riders/hour:"))
        .expect("report includes completed rides per hour and the throughput estimate");
    let (actual, estimate) = line
        .split_once(':')
        .unwrap()
        .1
        .trim()
        .split_once('/')
        .expect("report separates actual riders/hour from the throughput estimate");
    (
        actual.trim().parse().unwrap(),
        estimate.trim().parse().unwrap(),
    )
}

#[test]
fn horizon_hawk_park_report_is_deterministic_and_matches_the_golden() {
    let park_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let park_path = park_path.to_string_lossy();
    let first = report(&park_path);
    let second = report(&park_path);

    assert_eq!(first, second);
    assert_eq!(first, include_str!("goldens/horizon_hawk_park_report.txt"));
    assert!(first.contains("Force targets:"), "{first}");
    assert!(
        first.contains("Native agreement: pass") || first.contains("Native agreement: fail"),
        "report must show the native agreement verdict when its code is available"
    );
    let (actual, estimate) = riders_per_hour(&first);
    assert!(actual > 0.0, "one-coaster park should complete rides");
    assert!(
        actual <= estimate,
        "{actual} riders/hour exceeds {estimate}"
    );
}

#[test]
fn park_report_names_guests_who_cannot_reach_the_coaster() {
    let source_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let mut description: serde_json::Value =
        serde_json::from_slice(&fs::read(source_path).unwrap()).unwrap();
    let paths = description["paths"].as_array().unwrap();
    description["paths"] = serde_json::json!([paths[0].clone(), paths[2].clone()]);

    let disconnected_park = write_park_fixture(&description, "disconnected-queue.park.json");
    let output = report(disconnected_park.path());

    assert!(output.contains("Coaster: Horizon Hawk (hyper)"), "{output}");
    let unreachable_section = output
        .lines()
        .position(|line| line == "Guests who could not reach a ride at park start:")
        .expect("report lists guests who cannot reach the ride at park start");
    let unreachable_guests: Vec<_> = output
        .lines()
        .skip(unreachable_section + 2)
        .take_while(|line| line.starts_with("  "))
        .collect();
    assert!(
        !unreachable_guests.is_empty(),
        "fixture must contain unreachable guests"
    );
    for guest in unreachable_guests {
        let guest_details = guest.trim();
        let (name, destination) = guest_details
            .split_once(" (guest-")
            .expect("unreachable guest row includes the guest name and stable id");
        assert!(!name.is_empty(), "report must name unreachable guests");
        assert!(
            destination.contains("could not reach Horizon Hawk"),
            "{guest}"
        );
    }
}

#[test]
fn park_report_has_a_guest_and_revenue_section_for_every_coaster() {
    let source_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let mut description: serde_json::Value =
        serde_json::from_slice(&fs::read(source_path).unwrap()).unwrap();
    description["coasters"] = serde_json::json!([
        {
            "ride": "content/rides/horizon-hawk.ride.json",
            "station": { "x": 241, "y": 249, "z": 10, "heading": "north" },
            "open": true,
            "price": 6
        },
        {
            "ride": "content/rides/horizon-hawk.ride.json",
            "station": { "x": 340, "y": 300, "z": 10, "heading": "north" },
            "open": false,
            "price": 8
        }
    ]);
    for x in 337..=344 {
        for y in 300..=302 {
            description["terrain"]["edits"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "command": "flatten",
                    "parameters": [x, y, 1, 0, 0, 10, 0, 0]
                }));
        }
    }
    for x in [338, 342] {
        for corner in [2, 3] {
            description["terrain"]["edits"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "command": "lower_corner",
                    "parameters": [x, 302, 1, corner, 1, 0, 0, 0]
                }));
        }
    }
    description["paths"].as_array_mut().unwrap().extend([
        serde_json::json!({
            "kind": "normal",
            "style": 0,
            "route": [[239, 260, 8], [239, 320, 8], [338, 320, 8]]
        }),
        serde_json::json!({
            "kind": "normal",
            "style": 0,
            "route": [[338, 301, 10], [338, 320, 8], [343, 320, 8]]
        }),
        serde_json::json!({
            "kind": "queue",
            "style": 0,
            "route": [[342, 301, 10], [342, 320, 8]]
        }),
    ]);
    let park = write_park_fixture(&description, "two-coaster-report.park.json");

    let output = json_report(park.path());
    let coasters = output["coasters"].as_array().unwrap();

    assert_eq!(coasters.len(), 2, "{output}");
    assert_eq!(coasters[0]["facilityId"], serde_json::json!(u32::MAX));
    assert_eq!(coasters[1]["facilityId"], serde_json::json!(u32::MAX - 1));
    let guest_report = &output["guests"];
    for coaster in coasters {
        assert_eq!(coaster["name"], "Horizon Hawk");
        assert_eq!(coaster["open"], true);
        assert!(coaster["ridesTaken"].is_number(), "{coaster}");
        assert!(coaster["ridersPerHour"].is_number(), "{coaster}");
        assert!(coaster["queueLength"].is_number(), "{coaster}");
        assert!(coaster["revenue"].is_number(), "{coaster}");
        assert!(coaster["unreachableGuestsAtStart"].is_number(), "{coaster}");
        assert!(coaster["unreachableGuestsAtEnd"].is_number(), "{coaster}");
        let facility_id = coaster["facilityId"].as_u64().unwrap();
        let rides_taken = coaster["ridesTaken"].as_u64().unwrap();
        let completed_rides = guest_report["completedRidesByCoaster"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["facilityId"] == facility_id)
            .unwrap();
        assert_eq!(completed_rides["ridesTaken"], rides_taken);
        assert!(
            (coaster["ridersPerHour"].as_f64().unwrap() - rides_taken as f64 * 60.0).abs() < 0.001
        );
        let revenue = guest_report["revenueByFacility"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["facilityId"] == facility_id)
            .unwrap();
        assert_eq!(coaster["revenue"], revenue["revenue"]);
        let queue = guest_report["queueLengths"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["facilityId"] == facility_id)
            .unwrap();
        assert_eq!(coaster["queueLength"], queue["guests"]);
        for (property, outcome_property) in [
            ("unreachableGuestsAtStart", "initialUnreachableDestinations"),
            ("unreachableGuestsAtEnd", "unreachableDestinations"),
        ] {
            let unreachable_guests: BTreeSet<_> = guest_report[outcome_property]
                .as_array()
                .unwrap()
                .iter()
                .filter(|destination| destination["facilityId"] == facility_id)
                .map(|destination| destination["guestId"].as_u64().unwrap())
                .collect();
            assert_eq!(
                coaster[property].as_u64().unwrap() as usize,
                unreachable_guests.len()
            );
        }
    }
    assert_eq!(output["parkFacts"]["facilitiesByKind"]["coaster"], 2);
    for property in [
        "completedRidesByCoaster",
        "uniqueRidersByCoaster",
        "guestRatingsByCoaster",
    ] {
        assert_eq!(output["guests"][property].as_array().unwrap().len(), 2);
    }
}

#[test]
fn park_report_rejects_zero_guests_before_loading_the_park() {
    let park_path = repository_root().join("content/parks/horizon-hawk.park.json");
    let error = run_cli(
        &[
            "park".into(),
            "report".into(),
            park_path.to_string_lossy().into_owned(),
            "--guests".into(),
            "0".into(),
        ],
        "",
    )
    .unwrap_err();

    assert_eq!(error, "--guests must be greater than zero");
}
