//! `ride <file> finish`: ranked ways back to the station on real rides with their last pieces removed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_design_tools::run_cli;
use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim::track::saved_shape_check::check_saved_shaped_geometry;
use theme_park_sim::track::shaped::{ElementIntent, TurnSizeHint};

static NEXT_TEMP_RIDE: AtomicUsize = AtomicUsize::new(0);

struct TempRide(PathBuf);

impl TempRide {
    /// A copy of a content ride, keeping all but its last `removed_steps` steps.
    fn truncated(ride_name: &str, removed_steps: usize) -> Self {
        let sequence = NEXT_TEMP_RIDE.fetch_add(1, Ordering::Relaxed);
        let ride = Self(std::env::temp_dir().join(format!(
            "theme-park-design-tools-finish-{}-{sequence}.ride.json",
            std::process::id()
        )));
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../content/rides/{ride_name}.ride.json"));
        fs::copy(source, &ride.0).unwrap();
        let step_count = serde_json::from_slice::<serde_json::Value>(&fs::read(&ride.0).unwrap())
            .unwrap()["steps"]
            .as_array()
            .unwrap()
            .len();
        ride.run(&[&format!("truncate {}", step_count - removed_steps)])
            .unwrap();
        ride
    }

    fn path(&self) -> String {
        self.0.display().to_string()
    }

    fn run(&self, commands: &[&str]) -> Result<String, String> {
        let mut arguments = vec!["ride".to_owned(), self.path()];
        arguments.extend(commands.iter().map(|command| (*command).to_owned()));
        run_cli(&arguments, "")
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

fn finish_options(ride: &TempRide) -> Vec<serde_json::Value> {
    let reply = ride.run(&["finish", "--json"]).unwrap();
    let json: serde_json::Value = serde_json::from_str(&reply).unwrap();
    json["replies"][0]["completions"]
        .as_array()
        .or_else(|| json["completions"].as_array())
        .unwrap_or_else(|| panic!("finish JSON has no completions: {reply}"))
        .clone()
}

/// The best option passes the closed-lap rated-margin and force-headroom rules of `check`
/// under both physics configurations, and applying it closes a ride that replays closed.
fn assert_best_option_applies(ride: &TempRide) -> serde_json::Value {
    let options = finish_options(ride);
    let best = options[0].clone();
    assert_eq!(best["rank"], 1.0, "{best}");
    assert_eq!(best["rideCheck"]["passed"], true, "{best}");
    assert!(
        best["physics"]["arrivalSpeedMps"].as_f64().unwrap() > 0.0,
        "{best}"
    );
    assert!(
        best["physics"]["minimumForceHeadroomG"].as_f64().unwrap() >= 0.0,
        "{best}"
    );
    let applied = ride.run(&["finish apply 1"]).unwrap();
    assert!(applied.contains("and closed the circuit."), "{applied}");
    let replayed = ride.run(&["state"]).unwrap();
    assert!(replayed.contains("can test: true"), "{replayed}");
    best
}

#[test]
fn nearly_finished_classic_ride_gets_a_checked_completion() {
    let ride = TempRide::truncated("velocity-viper", 4);

    let listing = ride.run(&["finish"]).unwrap();
    assert!(listing.starts_with("Found "), "{listing}");
    assert!(listing.contains("1. "), "{listing}");
    assert!(listing.contains("ride check pass"), "{listing}");

    let best = assert_best_option_applies(&ride);
    assert_eq!(best["pieces"].as_array().unwrap().len(), 3, "{best}");
}

#[test]
fn horizon_hawk_without_its_last_straights_gets_a_checked_completion() {
    let ride = TempRide::truncated("horizon-hawk", 3);

    let best = assert_best_option_applies(&ride);

    assert_eq!(
        best["pieces"],
        serde_json::json!(["OneTileStraight", "OneTileStraight"])
    );
}

/// Every shaped turn in the best option is solved for its entry speed within the owner's force
/// targets, saved as the builder saves it (intent plus solved recipe with its geometry digest),
/// and re-solves to the same geometry when the ride is replayed, as the browser does.
fn assert_best_option_uses_shaped_turns(ride: &TempRide) -> serde_json::Value {
    let best = assert_best_option_applies(ride);
    let shaped_reports: Vec<_> = best["physics"]["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|piece| piece["id"].as_str().unwrap().starts_with("ShapedTurn"))
        .collect();
    assert!(!shaped_reports.is_empty(), "{best}");
    for report in &shaped_reports {
        let peaks = &report["worstCarPeaks"];
        assert!(
            peaks["maximum_vertical_g"].as_f64().unwrap() <= 3.5,
            "{report}"
        );
        assert!(
            peaks["maximum_lateral_g"].as_f64().unwrap() <= 0.6,
            "{report}"
        );
    }

    let saved = RideFile::from_json_slice(&fs::read(&ride.0).unwrap()).unwrap();
    let shaped_steps: Vec<_> = saved
        .steps
        .iter()
        .filter_map(|step| match step {
            RideStep::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();
    let finish_turns = &shaped_steps[shaped_steps.len() - shaped_reports.len()..];
    for shape in finish_turns {
        assert!(
            matches!(
                shape.intent,
                ElementIntent::Turn {
                    size: TurnSizeHint::Radius(radius_m),
                    ..
                } if radius_m % 2.0 == 0.0
            ),
            "finish turns use a fixed even radius the browser stepper can select: {:?}",
            shape.intent
        );
    }
    check_saved_shaped_geometry(&saved).unwrap();
    let check = run_cli(&["check".to_owned(), ride.path()], "");
    assert!(check.is_ok(), "{check:?}");
    best
}

#[test]
fn horizon_hawk_before_its_final_banked_turn_finishes_with_a_shaped_turn() {
    let ride = TempRide::truncated("horizon-hawk", 4);

    let best = assert_best_option_uses_shaped_turns(&ride);

    assert_eq!(
        best["pieces"],
        serde_json::json!(["ShapedTurnRight", "OneTileStraight", "OneTileStraight"])
    );
}

#[test]
fn horizon_hawk_five_steps_from_closed_finishes_with_a_straight_and_a_shaped_turn() {
    let ride = TempRide::truncated("horizon-hawk", 5);

    let best = assert_best_option_uses_shaped_turns(&ride);

    assert_eq!(
        best["pieces"],
        serde_json::json!([
            "OneTileStraight",
            "ShapedTurnRight",
            "OneTileStraight",
            "OneTileStraight"
        ])
    );
}

/// Before its brakes, Horizon Hawk is fast and 108 m from the station: classic turns are over
/// the force limits and no shaped turn the search tries leads back.
#[test]
fn horizon_hawk_before_its_brakes_explains_it_is_too_fast() {
    let ride = TempRide::truncated("horizon-hawk", 28);

    let error = ride.run(&["finish"]).unwrap_err();

    assert!(error.contains("No safe finish found"), "{error}");
    assert!(error.contains("over a force limit"), "{error}");
    assert!(error.contains("The train is too fast here"), "{error}");
    assert!(error.contains("90 degree heading mismatch"), "{error}");
    assert!(
        error.contains("shaped turn(s) could not be solved"),
        "{error}"
    );
}

#[test]
fn horizon_hawk_one_step_from_closed_says_to_close_the_circuit() {
    let ride = TempRide::truncated("horizon-hawk", 1);

    let error = ride.run(&["finish"]).unwrap_err();

    assert!(
        error.contains("The track already reaches the station entry; close the circuit instead."),
        "{error}"
    );
}

#[test]
fn finish_apply_refuses_an_option_that_does_not_exist() {
    let ride = TempRide::truncated("velocity-viper", 4);

    let error = ride.run(&["finish apply 9"]).unwrap_err();

    assert!(error.contains("finish option 9 does not exist"), "{error}");
}

#[test]
fn state_reports_the_searched_finish_result_instead_of_a_pending_search() {
    let finishable = TempRide::truncated("velocity-viper", 4);
    let state = finishable.run(&["state"]).unwrap();
    assert!(
        state.contains("Can close: false; can finish: true; can test: false"),
        "{state}"
    );

    let too_fast = TempRide::truncated("horizon-hawk", 28);
    let state = too_fast.run(&["state"]).unwrap();
    assert!(
        state.contains("can finish: false (No safe finish found"),
        "{state}"
    );
    assert!(state.contains("The train is too fast here"), "{state}");

    let at_station = TempRide::truncated("horizon-hawk", 1);
    let state = at_station.run(&["state"]).unwrap();
    assert!(
        state.contains(
            "Can close: true; can finish: false (The track already reaches the station entry; close the circuit instead.)"
        ),
        "{state}"
    );

    for state in [
        finishable.run(&["state"]).unwrap(),
        too_fast.run(&["state"]).unwrap(),
        at_station.run(&["state"]).unwrap(),
    ] {
        assert!(!state.contains("Checking for a connection"), "{state}");
    }
}

#[test]
fn a_finished_circuit_reports_closed_and_refuses_a_second_close() {
    let ride = TempRide::truncated("velocity-viper", 4);
    let applied = ride.run(&["finish apply 1"]).unwrap();
    assert!(
        applied.contains(
            "Can close: false; can finish: false (The coaster circuit is already closed.); can test: true"
        ),
        "{applied}"
    );
    let saved_after_finish = fs::read(&ride.0).unwrap();

    let error = ride.run(&["close"]).unwrap_err();

    assert!(
        error.contains("The coaster circuit is already closed."),
        "{error}"
    );
    assert_eq!(fs::read(&ride.0).unwrap(), saved_after_finish);
}
