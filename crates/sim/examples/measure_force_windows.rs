//! Measures every rated template's entry-speed window with the isolated-piece harness and, with
//! `UPDATE_RATED_WINDOWS=1`, writes `crates/sim/src/track/rated_windows.generated.rs`.
//!
//! A speed passes when every allowed model and train length passes the duration-aware force evaluator and clearance
//! checks. The evaluator permits short, bounded force excursions while rejecting sustained or larger violations. The
//! window starts at the catalog minimum (raised to the first passing speed when the catalog minimum fails) and ends at
//! the top of the contiguous passing run above it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_sim::physics::rated_piece::{rate_connected_piece, unpowered};
use theme_park_sim::track::catalog::unmeasured_catalog;
use theme_park_sim::track::rated_windows::RatedWindowSource;
use theme_park_sim::track::{
    CoasterModel, ForceRating, ForceRatingAxis, HillFamily, HillFeel, Piece, PieceFamily, PieceId,
    coaster_models,
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

/// Whether each allowed model's default train stays below the hill's airtime ceiling.
fn rated_hill_airtime_within_feel_ceiling(
    piece: &Piece,
    model: &CoasterModel,
    speed: f64,
) -> bool {
    let Some(family) = hill_family(piece).filter(|family| {
        matches!(
            family.feel,
            HillFeel::Ejector | HillFeel::Floater | HillFeel::StrongFloater
        ) && family.twist.is_none()
    }) else {
        return true;
    };

    assert!(model.allows_piece(piece));
    let rating = rate_connected_piece(model, model.train.cars, piece, speed, unpowered()).unwrap();
    let airtime = model.comfortable_force_limits.classify_force(
        ForceRatingAxis::Airtime,
        rating.minimum_filtered_vertical,
        model.name,
    );
    match family.feel {
        HillFeel::Ejector => !matches!(airtime, ForceRating::TooMuch | ForceRating::Dangerous),
        HillFeel::Floater | HillFeel::StrongFloater => {
            matches!(airtime, ForceRating::Weak | ForceRating::Good)
        }
        _ => unreachable!("only airtime hill feels are checked"),
    }
}

/// The maximum entry speed at which each allowed default train keeps the hill's intended airtime feel.
fn cap_maximum_for_hill_feel(
    piece: &Piece,
    model: &CoasterModel,
    minimum_entry: f64,
    maximum_entry: f64,
) -> Option<f64> {
    let family = hill_family(piece).filter(|family| {
        matches!(
            family.feel,
            HillFeel::Ejector | HillFeel::Floater | HillFeel::StrongFloater
        ) && family.twist.is_none()
    })?;
    if rated_hill_airtime_within_feel_ceiling(piece, model, maximum_entry) {
        return None;
    }
    assert!(
        rated_hill_airtime_within_feel_ceiling(piece, model, minimum_entry),
        "{:?}: default train airtime exceeds the {:?} ceiling at its minimum",
        piece.id,
        family.feel
    );
    Some(refine_boundary(
        minimum_entry,
        maximum_entry,
        false,
        |speed| rated_hill_airtime_within_feel_ceiling(piece, model, speed),
    ))
}

fn hill_family(piece: &Piece) -> Option<HillFamily> {
    match piece.family {
        Some(PieceFamily::Hill(family)) => Some(family),
        _ => None,
    }
}

/// The roll-rate design rule's peak roll rate for an entry speed, on the pieces the rule covers.
fn roll_rate_rule(piece: &'static Piece) -> Option<Box<dyn Fn(f64) -> f64>> {
    if hill_family(piece)
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

fn models() -> Vec<CoasterModel> {
    coaster_models()
}

fn refine_boundary(
    mut lower: f64,
    mut upper: f64,
    lower_edge: bool,
    passes: impl Fn(f64) -> bool,
) -> f64 {
    for _ in 0..24 {
        let middle = (lower + upper) / 2.0;
        if passes(middle) == lower_edge {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    if lower_edge {
        return (upper * 100.0).ceil() / 100.0;
    }
    let mut passing_cent = (lower * 100.0).floor() as i64;
    loop {
        let next_speed = (passing_cent + 1) as f64 / 100.0;
        if next_speed >= upper || !passes(next_speed) {
            return passing_cent as f64 / 100.0;
        }
        passing_cent += 1;
    }
}

/// The first allowed configuration that fails at this speed, described for the review table.
fn failure(piece: &Piece, model: &CoasterModel, speed: f64) -> Option<String> {
    assert!(model.allows_piece(piece), "{} does not allow {:?}", model.key, piece.id);
    let cars = model.train.cars;
    let rating = rate_connected_piece(model, cars, piece, speed, unpowered()).unwrap_or_else(|error| {
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
    assert!(
        rating.passed,
        "{} {cars} cars at {speed} m/s has no recorded failure but did not pass",
        model.name
    );
    None
}

struct ScannedWindow {
    template: &'static Piece,
    model_key: &'static str,
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

fn scan(template: &'static Piece, model: &CoasterModel) -> ScannedWindow {
    assert!(model.allows_piece(template));
    let old = template.design_speeds.unwrap();
    let ceiling = 100.0_f64.max(3.0 * old.maximum_entry);
    let first_index = (old.minimum_entry / GRID_STEP).floor() as usize + 1;
    let last_index = (ceiling / GRID_STEP).ceil() as usize;
    let grid: Vec<(f64, Option<String>)> = (first_index..=last_index)
        .map(|index| {
            let speed = index as f64 * GRID_STEP;
            (speed, failure(template, model, speed))
        })
        .collect();

    let minimum_failure = failure(template, model, old.minimum_entry);
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
                failure(template, model, speed).is_none()
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
        && failure(template, model, old.maximum_entry).is_none();
    let (maximum_entry, maximum_source, upper_failure) = match first_failure {
        None => {
            assert!(
                old_maximum_in_run,
                "{:?}: catalog maximum fails below the ceiling",
                template.id
            );
            (old.maximum_entry, RatedWindowSource::DesignCap, None)
        }
        Some(_)
            if (DESIGN_CAPPED_PIECES.contains(&template.id)
                || matches!(template.family, Some(PieceFamily::SpeedTurn { .. })))
                && old_maximum_in_run =>
        {
            (old.maximum_entry, RatedWindowSource::DesignCap, None)
        }
        Some((failing_speed, diagnostic)) => {
            let maximum = refine_boundary(last_pass, failing_speed, false, |speed| {
                failure(template, model, speed).is_none()
            });
            let source = if diagnostic.contains("clearing speed exceeds 22 m/s") {
                RatedWindowSource::ClearanceMaximum
            } else {
                RatedWindowSource::ForceBound
            };
            let boundary = failure(template, model, maximum + 0.01).unwrap_or(diagnostic);
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

    let (maximum_entry, maximum_source, upper_failure) =
        match cap_maximum_for_hill_feel(template, model, minimum_entry, maximum_entry) {
            Some(maximum) => {
                let feel = hill_family(template).unwrap().feel;
                let above = format!(
                    "default-train airtime leaves {feel:?} feel above {:.2} m/s",
                    maximum + 0.01
                );
                (maximum, RatedWindowSource::AirtimeFeel, Some(above))
            }
            None => (maximum_entry, maximum_source, upper_failure),
        };

    let mut acceptance_failures = Vec::new();
    for speed in [minimum_entry, maximum_entry] {
        if let Some(diagnostic) = failure(template, model, speed) {
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
    if let Some(family) = hill_family(template).filter(|family| {
        matches!(
            family.feel,
            HillFeel::Ejector | HillFeel::Floater | HillFeel::StrongFloater
        ) && family.twist.is_none()
    }) {
        let cars = model.train.cars;
        let rating = rate_connected_piece(model, cars, template, maximum_entry, unpowered()).unwrap();
        let class = model.comfortable_force_limits.classify_force(
            ForceRatingAxis::Airtime,
            rating.minimum_filtered_vertical,
            model.name,
        );
        airtime.push(format!(
            "{} {cars} cars (default): {:.3} g {class:?}",
            model.name, rating.minimum_filtered_vertical
        ));
        if family.feel == HillFeel::Ejector && class != ForceRating::Great {
            acceptance_failures.push(format!(
                "{:?}: {} default {cars}-car train has {class:?} airtime ({:.3} g) at the {maximum_entry} m/s maximum; expected Great",
                template.id, model.name, rating.minimum_filtered_vertical
            ));
        }
    }

    ScannedWindow {
        template,
        model_key: model.key,
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
        .filter(|piece| piece.design_speeds.is_some())
        .filter(|piece| {
            selected_piece
                .as_ref()
                .is_none_or(|name| *name == format!("{:?}", piece.id))
        })
        .collect();
    assert!(!templates.is_empty(), "unknown piece or no rated pieces");

    let work: Vec<(&'static Piece, CoasterModel)> = templates
        .iter()
        .flat_map(|piece| {
            models()
                .into_iter()
                .filter(|model| model.allows_piece(piece))
                .map(|model| (*piece, model))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(!work.is_empty(), "no allowed models for rated templates");

    let next_template = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<ScannedWindow>>> =
        Mutex::new((0..work.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..SCAN_THREADS.min(work.len()) {
            scope.spawn(|| {
                loop {
                    let index = next_template.fetch_add(1, Ordering::Relaxed);
                    let Some((template, model)) = work.get(index) else {
                        break;
                    };
                    let window = scan(template, model);
                    eprintln!(
                        "scanned {} {:?} ({:?} to {:?}): {:.6}–{:.6}",
                        model.key,
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
        "| Model | Template | Old window (m/s) | New window (m/s) | Minimum | Maximum | Failure just above the maximum | Failure at the old minimum | Passes again at |"
    );
    println!("| --- | --- | ---: | ---: | --- | --- | --- | --- | ---: |");
    for window in &windows {
        let old = window.template.design_speeds.unwrap();
        println!(
            "| `{}` | `{:?}` ({:?} → {:?}) | {:.2}–{:.2} | {:.2}–{:.2} | {:?} | {:?} | {} | {} | {} |",
            window.model_key,
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
            "AIRTIME {} {:?} at {:.2} m/s: {}",
            window.model_key,
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
                    "    MeasuredRatedWindow {{ model_key: {:?}, id: PieceId::{:?}, entry_slope: Slope::{:?}, exit_slope: Slope::{:?}, minimum_source: RatedWindowSource::{:?}, maximum_source: RatedWindowSource::{:?}, minimum_entry: {:?}, maximum_entry: {:?} }},",
                    window.model_key,
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
            "// Generated by make generate-rated-windows on {date}: 5 Hz filtered forces use sustained limits, 0.5 g allowance for 0.2 s.\n\
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
    use super::{
        ScannedWindow, cap_maximum_for_hill_feel, failure, refine_boundary, scan,
        unmeasured_catalog,
    };
    use theme_park_sim::physics::rated_piece::{rate_connected_piece, rate_piece, unpowered};
    use theme_park_sim::track::{
        CoasterModel, ForceRating, ForceRatingAxis, PieceId, coaster_models, hybrid_model,
    };

    fn scan_models(
        template: &'static theme_park_sim::track::Piece,
        models: &[CoasterModel],
    ) -> Vec<ScannedWindow> {
        models
            .iter()
            .filter(|model| model.allows_piece(template))
            .map(|model| scan(template, model))
            .collect()
    }

    #[test]
    fn accepts_evaluator_approved_brief_excursion_at_corkscrew_speed() {
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .expect("catalog contains the corkscrew");
        let model = hybrid_model();
        let limits = model.comfortable_force_limits;
        let rating = rate_piece(&model, 8, piece, 19.83, unpowered()).unwrap();

        assert!(rating.passed, "{rating:?}");
        assert!(
            rating.maximum_filtered_force[0] > limits.maximum_vertical_g,
            "expected a brief filtered overshoot above {:.3} g, saw {:.3} g",
            limits.maximum_vertical_g,
            rating.maximum_filtered_force[0]
        );
        assert_eq!(failure(piece, &model, 19.83), None);
    }

    #[test]
    fn refines_both_edges_and_rounds_into_the_passing_window() {
        let passes = |speed: f64| (5.123..=20.987).contains(&speed);
        assert_eq!(refine_boundary(5.0, 5.25, true, passes), 5.13);
        assert_eq!(refine_boundary(20.75, 21.0, false, passes), 20.98);
    }

    #[test]
    fn upper_boundary_returns_last_passing_cent() {
        let passes = |speed: f64| speed <= 10.24;
        let maximum = refine_boundary(5.0, 20.0, false, passes);

        assert_eq!(maximum, 10.24);
        assert!(passes(maximum));
        assert!(!passes(maximum + 0.01));
    }

    #[test]
    fn caps_ejector_and_floater_families_for_every_default_train() {
        for (piece_id, upper_speed, ejector) in [
            (PieceId::EjectorHill8x48, 25.0, true),
            (PieceId::WoodenEjectorHill6x48, 20.01, true),
            (PieceId::FloaterHill12x80, 30.0, false),
        ] {
            let piece = unmeasured_catalog()
                .iter()
                .find(|piece| piece.id == piece_id)
                .unwrap();
            let minimum = piece.design_speeds.unwrap().minimum_entry;
            let allowed_models: Vec<_> = coaster_models()
                .into_iter()
                .filter(|model| model.allows_piece(piece))
                .collect();
            assert!(!allowed_models.is_empty());
            let allowed_rating = |rating| {
                if ejector {
                    rating == ForceRating::Great
                } else {
                    matches!(rating, ForceRating::Weak | ForceRating::Good)
                }
            };
            for model in &allowed_models {
                let airtime = |speed| {
                    let rating = rate_connected_piece(
                        model,
                        model.train.cars,
                        piece,
                        speed,
                        unpowered(),
                    )
                    .unwrap();
                    model.comfortable_force_limits.classify_force(
                        ForceRatingAxis::Airtime,
                        rating.minimum_filtered_vertical,
                        model.name,
                    )
                };
                match cap_maximum_for_hill_feel(piece, model, minimum, upper_speed) {
                    Some(maximum) => {
                        assert!(
                            allowed_rating(airtime(maximum)),
                            "{piece_id:?} {}",
                            model.name
                        );
                        assert!(
                            !allowed_rating(airtime(maximum + 0.01)),
                            "{piece_id:?} {}",
                            model.name
                        );
                    }
                    None => assert!(
                        allowed_rating(airtime(upper_speed)),
                        "{piece_id:?} {} at {upper_speed}",
                        model.name
                    ),
                }
            }
        }
    }

    #[test]
    fn caps_wooden_ejector_window_at_briar_blitz_great_airtime() {
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::WoodenEjectorHill6x48)
            .expect("catalog contains the wooden ejector hill");
        let rated_speeds = piece.design_speeds.expect("the hill has rated speeds");
        let model = coaster_models()
            .into_iter()
            .find(|model| model.name == "Briar Blitz")
            .expect("catalog contains Briar Blitz");
        let capped_maximum =
            cap_maximum_for_hill_feel(piece, &model, rated_speeds.minimum_entry, 20.01)
                .expect("the computed maximum exceeds the intended airtime feel");

        let classify_at = |speed| {
            let rating = rate_connected_piece(
                &model,
                model.train.cars,
                piece,
                speed,
                unpowered(),
            )
            .unwrap();
            model.comfortable_force_limits.classify_force(
                ForceRatingAxis::Airtime,
                rating.minimum_filtered_vertical,
                model.name,
            )
        };

        assert_eq!(model.train.cars, 12);
        assert!(capped_maximum < 20.01);
        assert_eq!(classify_at(capped_maximum), ForceRating::Great);
        assert_ne!(classify_at(capped_maximum + 0.01), ForceRating::Great);
    }

    #[test]
    fn adding_a_restrictive_model_does_not_change_existing_model_windows() {
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .expect("catalog contains the corkscrew");
        let model = hybrid_model();
        let baseline = scan_models(piece, std::slice::from_ref(&model));
        let mut restrictive_model = model.clone();
        restrictive_model.key = "restrictive_test_model";
        restrictive_model.comfortable_force_limits.maximum_vertical_g = 3.2;
        let expanded = scan_models(piece, &[model, restrictive_model]);

        let baseline = &baseline[0];
        let existing = expanded
            .iter()
            .find(|window| window.model_key == baseline.model_key)
            .expect("the original model's window remains generated");
        let restrictive = expanded
            .iter()
            .find(|window| window.model_key == "restrictive_test_model")
            .expect("the restrictive model gets its own window");

        assert_eq!(existing.minimum_entry, baseline.minimum_entry);
        assert_eq!(existing.maximum_entry, baseline.maximum_entry);
        assert!(
            restrictive.maximum_entry < existing.maximum_entry,
            "restrictive model window {} should be narrower than {}",
            restrictive.maximum_entry,
            existing.maximum_entry
        );
    }
}
