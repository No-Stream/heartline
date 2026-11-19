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
