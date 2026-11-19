use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const TIMBER_TUMBLER: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");

struct TempRide(PathBuf);

impl TempRide {
    fn new() -> Self {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-rated-margins-{}-{sequence}.ride.json",
            std::process::id()
        )))
    }

    fn write(&self) {
        fs::write(&self.0, TIMBER_TUMBLER).unwrap();
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

#[test]
fn report_shows_entry_margin_checks_and_attributes_force_peaks_to_piece_and_car() {
    let ride = TempRide::new();
    ride.write();

    let report = run_cli(&["ride".into(), ride.path().into(), "report".into()], "").unwrap();

    assert!(report.contains("Rated entry-speed margin"), "{report}");
    assert!(report.contains("default physics"), "{report}");
    assert!(report.contains("construction-test physics"), "{report}");
    assert!(report.contains("EjectorHill16x96"), "{report}");
    assert!(
        report.contains("Measured force peaks by piece and car"),
        "{report}"
    );
    assert!(report.contains("sustained by any car"), "{report}");
    assert!(report.contains("car 0"), "{report}");
    assert!(
        !report.lines().any(|line| line.starts_with("  0 ")),
        "{report}"
    );
}

#[test]
fn report_shows_mid_course_brakes_and_their_share_of_lift_energy() {
    let ride = TempRide::new();
    ride.write();

    let report = run_cli(&["ride".into(), ride.path().into(), "report".into()], "").unwrap();

    let brake_line = report
        .lines()
        .find(|line| line.starts_with("Mid-course brake runs:"))
        .expect("ride report includes its mid-course brake energy summary");
    assert!(
        brake_line.contains("Mid-course brake runs: 2"),
        "{brake_line}"
    );
    assert!(
        brake_line.contains("estimated lift energy removed by trims from lead-car samples:"),
        "{brake_line}"
    );
    let trim_energy_share = brake_line
        .split_once("from lead-car samples: ")
        .expect("summary names the lift-energy share")
        .1
        .split_whitespace()
        .next()
        .expect("summary includes a percentage")
        .trim_end_matches('%')
        .parse::<f64>()
        .expect("summary reports a numeric percentage");
    assert!(trim_energy_share > 0.0, "{brake_line}");
}
