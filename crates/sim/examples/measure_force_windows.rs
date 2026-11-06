//! Measures every rated template's entry-speed window with the isolated-piece harness and, with
//! `UPDATE_RATED_WINDOWS=1`, writes `crates/sim/src/track/rated_windows.generated.rs`.
//!
//! A speed passes when every allowed model and train length passes the duration-aware evaluator and the clearance
//! checks, and its filtered force stays within the comfortable limits on every axis, so the overlay rates nothing
//! above Great at that speed. The window starts at the catalog minimum (raised to the first passing speed when the
//! catalog minimum fails) and ends at the top of the contiguous passing run above it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_sim::physics::rated_piece::{rate_piece, unpowered};
use theme_park_sim::track::catalog::unmeasured_catalog;
use theme_park_sim::track::rated_windows::RatedWindowSource;
use theme_park_sim::track::{
    AirtimeRating, CoasterModel, HillFeel, Piece, PieceId, hybrid_model, multi_launch_model,
    wooden_twister_model,
};

const GRID_STEP: f64 = 0.25;
const SCAN_THREADS: usize = 10;

/// Pieces whose forces stay comfortable far past any sensible speed keep their catalog maximum (plan section 5).
const DESIGN_CAPPED_PIECES: [PieceId; 6] = [
    PieceId::HeartlineRoll,
    PieceId::HeartlineRollRight,
    PieceId::BankedStraightLeft,
    PieceId::BankedStraightRight,
    PieceId::HalfCorkscrewLeftDown,
    PieceId::HalfCorkscrewRightDown,
];

const AXIS_NAMES: [&str; 3] = ["vertical", "lateral", "longitudinal"];
const MAXIMUM_ROLL_RATE: f64 = Piece::MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND;

/// The roll-rate design rule's peak roll rate for an entry speed, on the pieces the rule covers.
fn roll_rate_rule(piece: &'static Piece) -> Option<Box<dyn Fn(f64) -> f64>> {
    if piece
        .family
        .is_some_and(|family| matches!(family.feel, HillFeel::ZeroGRoll | HillFeel::Stall))
    {
        return Some(Box::new(|speed| piece.rolled_hill_peak_roll_rate(speed)));
    }
    matches!(
        piece.id,
        PieceId::HalfCorkscrewLeftUp
            | PieceId::HalfCorkscrewRightUp
            | PieceId::HalfCorkscrewLeftDown
            | PieceId::HalfCorkscrewRightDown
    )
    .then(|| {
        Box::new(|speed| piece.constant_speed_peak_roll_rate(speed)) as Box<dyn Fn(f64) -> f64>
    })
}

fn models() -> [CoasterModel; 3] {
    [hybrid_model(), multi_launch_model(), wooden_twister_model()]
}

fn refine_boundary(
    mut lower: f64,
    mut upper: f64,
    lower_edge: bool,
    passes: impl Fn(f64) -> bool,
) -> f64 {
    for _ in 0..12 {
        let middle = (lower + upper) / 2.0;
        if passes(middle) == lower_edge {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    if lower_edge {
        (upper * 100.0).ceil() / 100.0
    } else {
        (lower * 100.0).floor() / 100.0
    }
}

/// The first allowed configuration that fails at this speed, described for the review table.
fn failure(piece: &Piece, speed: f64) -> Option<String> {
    let mut evaluated = false;
    for model in models() {
        if !model.allows_piece(piece) {
            continue;
        }
        evaluated = true;
        let limits = model.comfortable_force_limits;
        for cars in model.cars_per_train_range() {
            let rating =
                rate_piece(&model, cars, piece, speed, unpowered()).unwrap_or_else(|error| {
                    panic!(
                        "{:?}, {} {cars} cars, {speed} m/s: {error}",
                        piece.id, model.name
                    )
                });
            if let Some(reason) = rating.failure {
                return Some(format!(
                    "{} {cars} cars: {reason} (lowest train speed {:.3} m/s)",
                    model.name, rating.minimum_speed
                ));
            }
            if let Some(violation) = &rating.violation {
                return Some(format!(
                    "{} {cars} cars: {} {:+.3} g past the duration rule",
                    model.name, AXIS_NAMES[violation.axis], violation.force
                ));
            }
            let lower = [
                limits.minimum_vertical_g,
                -limits.maximum_lateral_g,
                -limits.maximum_longitudinal_g,
            ];
            let upper = [
                limits.maximum_vertical_g,
                limits.maximum_lateral_g,
                limits.maximum_longitudinal_g,
            ];
            for axis in 0..3 {
                let low = rating.minimum_filtered_force[axis];
                let high = rating.maximum_filtered_force[axis];
                if low < lower[axis] || high > upper[axis] {
                    let force = if low < lower[axis] { low } else { high };
                    return Some(format!(
                        "{} {cars} cars: {} {force:+.3} g too much",
                        model.name, AXIS_NAMES[axis]
                    ));
                }
            }
            assert!(rating.passed && !rating.exceeds_sustained_limits(&limits));
        }
    }
    assert!(evaluated, "no allowed model for {:?}", piece.id);
    None
}

struct ScannedWindow {
    template: &'static Piece,
    minimum_entry: f64,
    maximum_entry: f64,
    minimum_source: RatedWindowSource,
    maximum_source: RatedWindowSource,
    /// The failure just above a force- or clearance-bound maximum.
    upper_failure: Option<String>,
    /// The failure at the catalog minimum, when the minimum had to rise.
    minimum_failure: Option<String>,
    /// The next grid speed above the first failure that passes again, a gap in the passing set.
    passes_again_above: Option<f64>,
    airtime: Vec<String>,
    acceptance_failures: Vec<String>,
}

fn scan(template: &'static Piece) -> ScannedWindow {
    let old = template.rated_speeds.unwrap();
    let ceiling = 100.0_f64.max(3.0 * old.maximum_entry);
    let first_index = (old.minimum_entry / GRID_STEP).floor() as usize + 1;
    let last_index = (ceiling / GRID_STEP).ceil() as usize;
    let grid: Vec<(f64, Option<String>)> = (first_index..=last_index)
        .map(|index| {
            let speed = index as f64 * GRID_STEP;
            (speed, failure(template, speed))
        })
        .collect();

    let minimum_failure = failure(template, old.minimum_entry);
    let (minimum_entry, minimum_source) = match &minimum_failure {
        None => (old.minimum_entry, RatedWindowSource::EnergyMinimum),
        Some(diagnostic) => {
            let first_pass = grid
                .iter()
                .position(|(_, failure)| failure.is_none())
                .unwrap_or_else(|| {
                    panic!(
                        "{:?}: no passing speed above the catalog minimum",
                        template.id
                    )
                });
            let below = if first_pass == 0 {
                old.minimum_entry
            } else {
                grid[first_pass - 1].0
            };
            let minimum = refine_boundary(below, grid[first_pass].0, true, |speed| {
                failure(template, speed).is_none()
            });
            let clearance = diagnostic.contains("clearance")
                || diagnostic.contains("stalled")
                || diagnostic.contains("did not clear");
            let source = if clearance {
                RatedWindowSource::ClearanceMinimum
            } else {
                RatedWindowSource::ForceBound
            };
            (minimum, source)
        }
    };
    let mut last_pass = minimum_entry;
    let mut first_failure: Option<(f64, String)> = None;
    for (speed, failure) in grid.iter().filter(|(speed, _)| *speed > minimum_entry) {
        match failure {
            None => last_pass = *speed,
            Some(diagnostic) => {
                first_failure = Some((*speed, diagnostic.clone()));
                break;
            }
        }
    }
    let passes_again_above = first_failure.as_ref().and_then(|(failing_speed, _)| {
        grid.iter()
            .find(|(speed, failure)| speed > failing_speed && failure.is_none())
            .map(|(speed, _)| *speed)
    });
    let old_maximum_in_run = old.maximum_entry > minimum_entry
        && first_failure
            .as_ref()
            .is_none_or(|(failing_speed, _)| old.maximum_entry < *failing_speed)
        && failure(template, old.maximum_entry).is_none();
    let (maximum_entry, maximum_source, upper_failure) = match first_failure {
        None => {
            assert!(
                old_maximum_in_run,
                "{:?}: catalog maximum fails below the ceiling",
                template.id
            );
            (old.maximum_entry, RatedWindowSource::DesignCap, None)
        }
        Some(_) if DESIGN_CAPPED_PIECES.contains(&template.id) && old_maximum_in_run => {
            (old.maximum_entry, RatedWindowSource::DesignCap, None)
        }
        Some((failing_speed, diagnostic)) => {
            let maximum = refine_boundary(last_pass, failing_speed, false, |speed| {
                failure(template, speed).is_none()
            });
            let source = if diagnostic.contains("clearing speed exceeds 22 m/s") {
                RatedWindowSource::ClearanceMaximum
            } else {
                RatedWindowSource::ForceBound
            };
            let boundary = failure(template, maximum + 0.01).unwrap_or(diagnostic);
            (maximum, source, Some(boundary))
        }
    };

    let (maximum_entry, maximum_source, upper_failure) = match roll_rate_rule(template) {
        Some(peak_roll_rate) if peak_roll_rate(maximum_entry) > MAXIMUM_ROLL_RATE => {
            assert!(
                peak_roll_rate(minimum_entry) <= MAXIMUM_ROLL_RATE,
                "{:?}: rolls too fast at its minimum",
                template.id
            );
            let maximum = refine_boundary(minimum_entry, maximum_entry, false, |speed| {
                peak_roll_rate(speed) <= MAXIMUM_ROLL_RATE
            });
            let above = format!("roll rate {:.1} degrees/s", peak_roll_rate(maximum + 0.01));
            (maximum, RatedWindowSource::RollRate, Some(above))
        }
        _ => (maximum_entry, maximum_source, upper_failure),
    };

    let mut acceptance_failures = Vec::new();
    for speed in [minimum_entry, maximum_entry] {
        if let Some(diagnostic) = failure(template, speed) {
            acceptance_failures.push(format!(
                "{:?}: endpoint {speed} fails: {diagnostic}",
                template.id
            ));
        }
    }
    let loop_piece = matches!(
        template.id,
        PieceId::Loop
            | PieceId::LoopRight
            | PieceId::LargeLoop
            | PieceId::LargeLoopRight
            | PieceId::SteelLoop
            | PieceId::SteelLoopRight
    );
    let required_width = if loop_piece { 1.5 } else { 2.0 };
    if maximum_entry - minimum_entry < required_width {
        acceptance_failures.push(format!(
            "{:?}: window {minimum_entry}–{maximum_entry} is narrower than {required_width} m/s",
            template.id
        ));
    }

    let mut airtime = Vec::new();
    if let Some(family) = template.family.filter(|family| {
        matches!(family.feel, HillFeel::Ejector | HillFeel::Floater) && family.twist.is_none()
    }) {
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(template))
        {
            for cars in model.cars_per_train_range() {
                let rating =
                    rate_piece(&model, cars, template, maximum_entry, unpowered()).unwrap();
                let class = model
                    .comfortable_force_limits
                    .classify_airtime(rating.minimum_filtered_vertical);
                let default_train = cars == model.train.cars;
                airtime.push(format!(
                    "{} {cars} cars{}: {:.3} g {class:?}",
                    model.name,
                    if default_train { " (default)" } else { "" },
                    rating.minimum_filtered_vertical
                ));
                if family.feel == HillFeel::Ejector
                    && default_train
                    && class != AirtimeRating::Great
                {
                    acceptance_failures.push(format!(
                        "{:?}: {} default {cars}-car train has {class:?} airtime ({:.3} g) at the {maximum_entry} m/s maximum; relabel it a floater",
                        template.id, model.name, rating.minimum_filtered_vertical
                    ));
                }
            }
        }
    }

    ScannedWindow {
        template,
        minimum_entry,
        maximum_entry,
        minimum_source,
        maximum_source,
        upper_failure,
        minimum_failure,
        passes_again_above,
        airtime,
        acceptance_failures,
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let selected_piece = arguments.next();
    assert!(
        arguments.next().is_none(),
        "usage: measure_force_windows [piece-id]"
    );
    let update = std::env::var_os("UPDATE_RATED_WINDOWS").is_some();
    assert!(
        !update || selected_piece.is_none(),
        "generation requires every template"
    );
    let templates: Vec<&'static Piece> = unmeasured_catalog()
        .iter()
        .filter(|piece| piece.rated_speeds.is_some())
        .filter(|piece| {
            selected_piece
                .as_ref()
                .is_none_or(|name| *name == format!("{:?}", piece.id))
        })
        .collect();
    assert!(!templates.is_empty(), "unknown piece or no rated pieces");

    let next_template = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<ScannedWindow>>> =
        Mutex::new((0..templates.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..SCAN_THREADS.min(templates.len()) {
            scope.spawn(|| {
                loop {
                    let index = next_template.fetch_add(1, Ordering::Relaxed);
                    let Some(template) = templates.get(index) else {
                        break;
                    };
                    let window = scan(template);
                    eprintln!(
                        "scanned {:?} ({:?} to {:?}): {:.6}–{:.6}",
                        template.id,
                        template.entry.slope,
                        template.exit.slope,
                        window.minimum_entry,
                        window.maximum_entry
                    );
                    results.lock().unwrap()[index] = Some(window);
                }
            });
        }
    });
    let windows: Vec<ScannedWindow> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect();

    println!(
        "| Template | Old window (m/s) | New window (m/s) | Minimum | Maximum | Failure just above the maximum | Failure at the old minimum | Passes again at |"
    );
    println!("| --- | ---: | ---: | --- | --- | --- | --- | ---: |");
    for window in &windows {
        let old = window.template.rated_speeds.unwrap();
        println!(
            "| `{:?}` ({:?} → {:?}) | {:.2}–{:.2} | {:.2}–{:.2} | {:?} | {:?} | {} | {} | {} |",
            window.template.id,
            window.template.entry.slope,
            window.template.exit.slope,
            old.minimum_entry,
            old.maximum_entry,
            window.minimum_entry,
            window.maximum_entry,
            window.minimum_source,
            window.maximum_source,
            window.upper_failure.as_deref().unwrap_or("—"),
            window.minimum_failure.as_deref().unwrap_or("—"),
            window
                .passes_again_above
                .map_or("—".to_string(), |speed| format!("{speed:.2}")),
        );
    }
    println!();
    for window in windows.iter().filter(|window| !window.airtime.is_empty()) {
        println!(
            "AIRTIME {:?} at {:.2} m/s: {}",
            window.template.id,
            window.maximum_entry,
            window.airtime.join("; ")
        );
    }
    let acceptance_failures: Vec<&String> = windows
        .iter()
        .flat_map(|window| &window.acceptance_failures)
        .collect();
    for failure in &acceptance_failures {
        println!("ACCEPTANCE {failure}");
    }

    if update {
        assert!(
            acceptance_failures.is_empty(),
            "rated window acceptance failed; the table was not written"
        );
        let date = std::process::Command::new("date")
            .args(["-u", "+%F"])
            .output()
            .expect("run date for table provenance");
        assert!(date.status.success(), "date command failed");
        let date = std::str::from_utf8(&date.stdout)
            .expect("date output is UTF-8")
            .trim();
        let rows: Vec<String> = windows
            .iter()
            .map(|window| {
                format!(
                    "    MeasuredRatedWindow {{ id: PieceId::{:?}, entry_slope: Slope::{:?}, exit_slope: Slope::{:?}, minimum_source: RatedWindowSource::{:?}, maximum_source: RatedWindowSource::{:?}, minimum_entry: {:?}, maximum_entry: {:?} }},",
                    window.template.id,
                    window.template.entry.slope,
                    window.template.exit.slope,
                    window.minimum_source,
                    window.maximum_source,
                    window.minimum_entry,
                    window.maximum_entry
                )
            })
            .collect();
        let contents = format!(
            "// Generated by make generate-rated-windows on {date}: 5 Hz filtered forces within the comfortable limits, 0.5 g allowance for 0.2 s.\n\
             #[rustfmt::skip]\n\
             const MEASURED_RATED_WINDOWS: &[MeasuredRatedWindow] = &[\n{}\n];\n",
            rows.join("\n")
        );
        std::fs::write("crates/sim/src/track/rated_windows.generated.rs", contents)
            .expect("write generated rated windows");
    }
}

#[cfg(test)]
mod tests {
    use super::refine_boundary;
    use theme_park_sim::physics::rated_piece::PieceRating;

    #[test]
    fn rejects_any_filtered_peak_past_a_sustained_limit() {
        let limits = theme_park_sim::track::hybrid_model().comfortable_force_limits;
        let mut rating = PieceRating {
            passed: true,
            entry_speed: 1.0,
            minimum_speed: 1.0,
            minimum_filtered_vertical: limits.minimum_vertical_g,
            minimum_filtered_force: [
                limits.minimum_vertical_g,
                -limits.maximum_lateral_g,
                -limits.maximum_longitudinal_g,
            ],
            maximum_filtered_force: [
                limits.maximum_vertical_g,
                limits.maximum_lateral_g,
                limits.maximum_longitudinal_g,
            ],
            violation: None,
            failure: None,
        };
        assert!(!rating.exceeds_sustained_limits(&limits));
        for axis in 0..3 {
            rating.minimum_filtered_force[axis] -= 0.001;
            assert!(rating.exceeds_sustained_limits(&limits));
            rating.minimum_filtered_force[axis] += 0.001;
            rating.maximum_filtered_force[axis] += 0.001;
            assert!(rating.exceeds_sustained_limits(&limits));
            rating.maximum_filtered_force[axis] -= 0.001;
        }
    }

    #[test]
    fn refines_both_edges_and_rounds_into_the_passing_window() {
        let passes = |speed: f64| (5.123..=20.987).contains(&speed);
        assert_eq!(refine_boundary(5.0, 5.25, true, passes), 5.13);
        assert_eq!(refine_boundary(20.75, 21.0, false, passes), 20.98);
    }
}
