//! Measures every rated template's entry-speed window with the connected model-allowed approach and, with
//! `UPDATE_RATED_WINDOWS=1`, writes `crates/sim/src/track/rated_windows.generated.rs`.
//!
//! A speed passes when every allowed model's default train passes the duration-aware force evaluator and clearance
//! checks. The evaluator permits short, bounded force excursions while rejecting sustained or larger violations. The
//! window starts at the catalog minimum (raised to the first passing speed when the catalog minimum fails) and ends at
//! the top of the contiguous passing run above it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use theme_park_sim::physics::rated_piece::{
    ForceJudge, connected_measurement_track, rate_piece, rate_piece_with_judge, unpowered,
};
use theme_park_sim::physics::{ForceEvaluator, TIME_STEP, Train};
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
fn rated_hill_airtime_within_feel_ceiling(piece: &Piece, model: &CoasterModel, speed: f64) -> bool {
    let Some(family) = hill_family(piece).filter(|family| {
        matches!(
            family.feel,
            HillFeel::Ejector | HillFeel::Floater | HillFeel::StrongFloater
        ) && family.twist.is_none()
    }) else {
        return true;
    };

    assert!(model.allows_piece(piece));
    let rating = rate_piece(model, model.train.cars, piece, speed, unpowered()).unwrap();
    let airtime = model.design_force_targets.classify_force(
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
    if matches!(piece.family, Some(PieceFamily::SpeedTurn { .. }))
        || matches!(
            piece.id,
            PieceId::BankLeft
                | PieceId::UnbankLeft
                | PieceId::BankRight
                | PieceId::UnbankRight
                | PieceId::RolloutLeft
                | PieceId::RolloutRight
                | PieceId::HalfCorkscrewLeftUp
                | PieceId::HalfCorkscrewRightUp
                | PieceId::HalfCorkscrewLeftDown
                | PieceId::HalfCorkscrewRightDown
        )
    {
        return Some(Box::new(|speed| piece.constant_speed_peak_roll_rate(speed)));
    }
    None
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
        let mut first_passing_cent = (upper * 100.0).ceil() as i64;
        while !passes(first_passing_cent as f64 / 100.0) {
            first_passing_cent += 1;
        }
        while passes((first_passing_cent - 1) as f64 / 100.0) {
            first_passing_cent -= 1;
        }
        return first_passing_cent as f64 / 100.0;
    }
    let mut passing_cent = (lower * 100.0).floor() as i64;
    while !passes(passing_cent as f64 / 100.0) {
        passing_cent -= 1;
    }
    loop {
        let next_speed = (passing_cent + 1) as f64 / 100.0;
        if next_speed >= upper || !passes(next_speed) {
            return passing_cent as f64 / 100.0;
        }
        passing_cent += 1;
    }
}

/// The first allowed configuration that fails at this speed, described for the review table.
fn failure(piece: &Piece, model: &CoasterModel, speed: f64) -> Option<ScanFailure> {
    failure_with_judge(piece, model, speed, ForceJudge::Envelope)
}

fn failure_with_judge(
    piece: &Piece,
    model: &CoasterModel,
    speed: f64,
    judge: ForceJudge,
) -> Option<ScanFailure> {
    assert!(
        model.allows_piece(piece),
        "{} does not allow {:?}",
        model.key,
        piece.id
    );
    let cars = model.train.cars;
    let rating = rate_piece_with_judge(model, cars, piece, speed, unpowered(), judge)
        .unwrap_or_else(|error| {
            panic!(
                "{:?}, {} {cars} cars, {speed} m/s: {error}",
                piece.id, model.name
            )
        });
    if let Some(reason) = rating.failure {
        let kind = if rating
            .context_results
            .iter()
            .all(|context| !context.applicable)
        {
            FailureKind::ApproachReachability
        } else {
            FailureKind::Other
        };
        return Some(ScanFailure {
            kind,
            diagnostic: format!(
                "{} {cars} cars: {reason} (lowest train speed {:.3} m/s)",
                model.name, rating.minimum_speed
            ),
        });
    }
    if let Some(violation) = &rating.violation {
        let rule_name = match judge {
            ForceJudge::Envelope => "the ASTM comfort envelope",
            ForceJudge::DesignTargets(_) => "the model design targets",
        };
        return Some(ScanFailure {
            kind: FailureKind::Other,
            diagnostic: format!(
                "{} {cars} cars: {} {:+.3} g past {rule_name}",
                model.name, AXIS_NAMES[violation.axis], violation.force
            ),
        });
    }
    assert!(
        rating.passed,
        "{} {cars} cars at {speed} m/s has no recorded failure but did not pass",
        model.name
    );
    None
}

fn diagnose_rating(
    context: &str,
    piece: &Piece,
    model: &CoasterModel,
    speed: f64,
    rating: &theme_park_sim::physics::rated_piece::PieceRating,
) {
    println!(
        "{context}: {} {:?} {:?}->{:?}, {}-car default train, {speed:.2} m/s, passed={}, minimum speed={:.3} m/s",
        model.key,
        piece.id,
        piece.entry.slope,
        piece.exit.slope,
        model.train.cars,
        rating.passed,
        rating.minimum_speed
    );
    if let Some(reason) = rating.failure {
        println!("  failure: {reason}");
    }
    if let Some(violation) = &rating.violation {
        println!(
            "  violation: car {}, {} {:+.6} g for {:.3} s at {:.3} m from piece entry (in_tail={})",
            violation.car,
            AXIS_NAMES[violation.axis],
            violation.force,
            violation.excursion_seconds,
            violation.distance,
            violation.in_tail
        );
    } else {
        println!(
            "  filtered range: vertical {:.6}..{:.6} g, lateral {:.6}..{:.6} g, longitudinal {:.6}..{:.6} g",
            rating.minimum_filtered_force[0],
            rating.maximum_filtered_force[0],
            rating.minimum_filtered_force[1],
            rating.maximum_filtered_force[1],
            rating.minimum_filtered_force[2],
            rating.maximum_filtered_force[2]
        );
    }
}

#[derive(Clone, Copy)]
struct ForceSample {
    force: f64,
    distance: f64,
    in_tail: bool,
    measured: bool,
}

#[derive(Clone, Copy)]
struct PeakForce {
    car: usize,
    force: f64,
    distance: f64,
    in_tail: bool,
    excursion_seconds: f64,
}

fn force_side(force: f64, limits: (f64, f64)) -> i8 {
    if force < limits.0 {
        -1
    } else if force > limits.1 {
        1
    } else {
        0
    }
}

fn connected_peak_forces(piece: &Piece, model: &CoasterModel, speed: f64) -> [PeakForce; 3] {
    let (circuit, element_range) =
        connected_measurement_track(model, model.train.cars, piece, speed);
    let limits = model.design_force_targets;
    let axis_limits = [
        (limits.minimum_vertical_g, limits.maximum_vertical_g),
        (-limits.maximum_lateral_g, limits.maximum_lateral_g),
        (
            -limits.maximum_longitudinal_g,
            limits.maximum_longitudinal_g,
        ),
    ];
    let mut evaluators =
        vec![ForceEvaluator::new(limits, TIME_STEP); usize::from(model.train.cars)];
    let mut samples: Vec<[Vec<ForceSample>; 3]> = (0..usize::from(model.train.cars))
        .map(|_| std::array::from_fn(|_| Vec::new()))
        .collect();
    let mut elapsed_tail = vec![0.0; usize::from(model.train.cars)];
    let mut done = vec![false; usize::from(model.train.cars)];
    let mut train = Train::new(
        &circuit,
        &model.train,
        unpowered(),
        element_range.start,
        speed,
    );

    for _ in 0..100_000 {
        train.step(&circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.into_iter().enumerate() {
                let result = evaluators[car].update(reading.g);
                if done[car] {
                    continue;
                }
                let in_rating_segment = reading.distance >= element_range.start;
                let in_tail = reading.distance >= element_range.end;
                for (axis, force) in [
                    result.filtered.vertical,
                    result.filtered.lateral,
                    result.filtered.longitudinal,
                ]
                .into_iter()
                .enumerate()
                {
                    samples[car][axis].push(ForceSample {
                        force,
                        distance: reading.distance - element_range.start,
                        in_tail,
                        measured: in_rating_segment,
                    });
                }
                if in_tail {
                    elapsed_tail[car] += TIME_STEP;
                }
                if in_tail
                    && elapsed_tail[car] >= 0.5
                    && !result
                        .axis_exceeding_sustained
                        .iter()
                        .any(|exceeding| *exceeding)
                {
                    done[car] = true;
                }
            }
        }
        if done.iter().all(|finished| *finished) {
            break;
        }
    }
    assert!(
        done.iter().all(|finished| *finished),
        "train did not clear {piece:?}"
    );

    std::array::from_fn(|axis| {
        let (car, sample_index, peak_sample) = samples
            .iter()
            .enumerate()
            .flat_map(|(car, car_samples)| {
                car_samples[axis]
                    .iter()
                    .enumerate()
                    .filter(|(_, sample)| sample.measured)
                    .map(move |(index, sample)| (car, index, sample))
            })
            .max_by(|left, right| left.2.force.abs().total_cmp(&right.2.force.abs()))
            .expect("rating interval has no force samples");
        let side = force_side(peak_sample.force, axis_limits[axis]);
        let mut first = sample_index;
        let mut last = sample_index;
        if side != 0 {
            while first > 0
                && force_side(samples[car][axis][first - 1].force, axis_limits[axis]) == side
            {
                first -= 1;
            }
            while last + 1 < samples[car][axis].len()
                && force_side(samples[car][axis][last + 1].force, axis_limits[axis]) == side
            {
                last += 1;
            }
        }
        PeakForce {
            car,
            force: peak_sample.force,
            distance: peak_sample.distance,
            in_tail: peak_sample.in_tail,
            excursion_seconds: if side == 0 {
                0.0
            } else {
                (last - first + 1) as f64 * TIME_STEP
            },
        }
    })
}

fn diagnose(
    piece_name: &str,
    model_key: &str,
    entry_slope: &str,
    exit_slope: &str,
    requested_speeds: &[f64],
) {
    let piece = unmeasured_catalog()
        .iter()
        .find(|piece| {
            format!("{:?}", piece.id) == piece_name
                && format!("{:?}", piece.entry.slope) == entry_slope
                && format!("{:?}", piece.exit.slope) == exit_slope
                && piece.design_speeds.is_some()
        })
        .unwrap_or_else(|| {
            panic!("unknown rated piece variant {piece_name} ({entry_slope} to {exit_slope})")
        });
    let model = models()
        .into_iter()
        .find(|model| model.key == model_key)
        .unwrap_or_else(|| panic!("unknown coaster model {model_key}"));
    assert!(
        model.allows_piece(piece),
        "{model_key} does not allow {piece_name}"
    );
    let old = piece.design_speeds.unwrap();
    println!(
        "limits: {:?}; catalog window {:.2}–{:.2} m/s",
        model.design_force_targets, old.minimum_entry, old.maximum_entry
    );
    let mut speeds = vec![
        ("catalog minimum", old.minimum_entry),
        ("old maximum", old.maximum_entry),
        ("first cent above old maximum", old.maximum_entry + 0.01),
    ];
    speeds.extend(
        requested_speeds
            .iter()
            .map(|speed| ("requested speed", *speed)),
    );
    for (label, speed) in speeds {
        let rating = rate_piece(&model, model.train.cars, piece, speed, unpowered())
            .unwrap_or_else(|error| panic!("{label} at {speed:.2} m/s: {error}"));
        diagnose_rating("shared measurement", piece, &model, speed, &rating);
        if label == "requested speed" && speed == requested_speeds[0] && rating.passed {
            for (axis, peak) in ["vertical", "lateral", "longitudinal"]
                .into_iter()
                .zip(connected_peak_forces(piece, &model, speed))
            {
                println!(
                    "  connected peak at this speed: car {}, {axis} {:+.6} g, {:.3} s sustained-limit excursion, {:.3} m from piece entry (in_tail={})",
                    peak.car, peak.force, peak.excursion_seconds, peak.distance, peak.in_tail
                );
            }
        }
    }
}

struct ScannedWindow {
    template: &'static Piece,
    model_key: &'static str,
    minimum_entry: Option<f64>,
    maximum_entry: Option<f64>,
    measured_from: Option<f64>,
    minimum_source: Option<RatedWindowSource>,
    maximum_source: Option<RatedWindowSource>,
    feel_minimum_entry: Option<f64>,
    feel_maximum_entry: Option<f64>,
    feel_maximum_source: Option<RatedWindowSource>,
    unavailable_reason: Option<String>,
    /// The failure just above a force- or clearance-bound maximum.
    upper_failure: Option<String>,
    /// The failure at the catalog minimum, when the minimum had to rise.
    minimum_failure: Option<String>,
    /// The next grid speed above the first failure that passes again, a gap in the passing set.
    passes_again_above: Option<f64>,
    airtime: Vec<String>,
    acceptance_failures: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureKind {
    ApproachReachability,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScanFailure {
    kind: FailureKind,
    diagnostic: String,
}

fn minimum_source(failure: &ScanFailure) -> RatedWindowSource {
    assert_ne!(
        failure.kind,
        FailureKind::ApproachReachability,
        "approach reachability does not define a rated-window minimum"
    );
    if failure.diagnostic.contains("clearance")
        || failure.diagnostic.contains("stalled")
        || failure.diagnostic.contains("did not clear")
    {
        RatedWindowSource::ClearanceMinimum
    } else {
        RatedWindowSource::ForceBound
    }
}

fn scan(template: &'static Piece, model: &CoasterModel) -> ScannedWindow {
    assert!(model.allows_piece(template));
    let old = template.design_speeds.unwrap();
    let ceiling = 100.0_f64.max(3.0 * old.maximum_entry);
    let first_index = (old.minimum_entry / GRID_STEP).floor() as usize + 1;
    let last_index = (ceiling / GRID_STEP).ceil() as usize;
    let grid: Vec<(f64, Option<ScanFailure>)> = (first_index..=last_index)
        .map(|index| {
            let speed = index as f64 * GRID_STEP;
            (speed, failure(template, model, speed))
        })
        .collect();

    let minimum_failure = failure(template, model, old.minimum_entry);
    let (minimum_entry, minimum_source) = match &minimum_failure {
        None => (old.minimum_entry, RatedWindowSource::EnergyMinimum),
        Some(diagnostic) => {
            let Some(first_pass) = grid.iter().position(|(_, failure)| failure.is_none()) else {
                return ScannedWindow {
                    template,
                    model_key: model.key,
                    minimum_entry: None,
                    maximum_entry: None,
                    measured_from: None,
                    minimum_source: None,
                    maximum_source: None,
                    feel_minimum_entry: None,
                    feel_maximum_entry: None,
                    feel_maximum_source: None,
                    unavailable_reason: Some(format!(
                        "no passing speed above catalog minimum {:.2} m/s: {}",
                        old.minimum_entry, diagnostic.diagnostic
                    )),
                    upper_failure: None,
                    minimum_failure: minimum_failure
                        .as_ref()
                        .map(|failure| failure.diagnostic.clone()),
                    passes_again_above: None,
                    airtime: Vec::new(),
                    acceptance_failures: Vec::new(),
                };
            };
            let below = if first_pass == 0 {
                old.minimum_entry
            } else {
                grid[first_pass - 1].0
            };
            let minimum = refine_boundary(below, grid[first_pass].0, true, |speed| {
                failure(template, model, speed).is_none()
            });
            let boundary_failure = if first_pass == 0 {
                diagnostic
            } else {
                grid[first_pass - 1]
                    .1
                    .as_ref()
                    .expect("the sample before the first pass must fail")
            };
            if boundary_failure.kind == FailureKind::ApproachReachability {
                (old.minimum_entry, RatedWindowSource::EnergyMinimum)
            } else {
                (minimum, minimum_source(boundary_failure))
            }
        }
    };
    let measured_from = match minimum_failure.as_ref() {
        Some(minimum_failure) if minimum_failure.kind == FailureKind::ApproachReachability => {
            match grid.iter().position(|(_, failure)| {
                failure
                    .as_ref()
                    .is_none_or(|failure| failure.kind != FailureKind::ApproachReachability)
            }) {
                Some(first_reachable) => {
                    let below = if first_reachable == 0 {
                        old.minimum_entry
                    } else {
                        grid[first_reachable - 1].0
                    };
                    refine_boundary(below, grid[first_reachable].0, true, |speed| {
                        failure(template, model, speed)
                            .is_none_or(|failure| failure.kind != FailureKind::ApproachReachability)
                    })
                }
                None => unreachable!("a rated window requires a reachable measurement speed"),
            }
        }
        _ => old.minimum_entry,
    };
    let mut last_pass = minimum_entry;
    let mut first_failure: Option<(f64, ScanFailure)> = None;
    for (speed, failure) in grid.iter().filter(|(speed, _)| *speed > minimum_entry) {
        match failure {
            None => last_pass = *speed,
            Some(diagnostic) if diagnostic.kind == FailureKind::ApproachReachability => {}
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
        Some(_) if DESIGN_CAPPED_PIECES.contains(&template.id) && old_maximum_in_run => {
            (old.maximum_entry, RatedWindowSource::DesignCap, None)
        }
        Some((failing_speed, diagnostic)) => {
            let maximum = refine_boundary(last_pass, failing_speed, false, |speed| {
                failure(template, model, speed).is_none()
            });
            let source = if diagnostic
                .diagnostic
                .contains("clearing speed exceeds 22 m/s")
            {
                RatedWindowSource::ClearanceMaximum
            } else {
                RatedWindowSource::ForceBound
            };
            let boundary = failure(template, model, maximum + 0.01)
                .unwrap_or(diagnostic)
                .diagnostic;
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

    let (feel_minimum_entry, feel_maximum_entry, feel_maximum_source) = scan_feel(
        template,
        model,
        minimum_entry,
        maximum_entry,
        maximum_source,
    );

    assert!(
        measured_from <= maximum_entry,
        "{:?}: measured_from {measured_from} is above maximum {maximum_entry}",
        template.id
    );
    let mut acceptance_failures = Vec::new();
    for speed in [minimum_entry, maximum_entry] {
        if let Some(diagnostic) = failure(template, model, speed)
            .filter(|failure| failure.kind != FailureKind::ApproachReachability)
        {
            acceptance_failures.push(format!(
                "{:?}: endpoint {speed} fails: {}",
                template.id, diagnostic.diagnostic
            ));
        }
    }
    if let (Some(feel_min), Some(feel_max)) = (feel_minimum_entry, feel_maximum_entry) {
        let feel_judge = ForceJudge::DesignTargets(model.design_force_targets);
        for speed in [feel_min, feel_max] {
            if let Some(diagnostic) = failure_with_judge(template, model, speed, feel_judge)
                .filter(|failure| failure.kind != FailureKind::ApproachReachability)
            {
                acceptance_failures.push(format!(
                    "{:?}: feel endpoint {speed} fails design targets: {}",
                    template.id, diagnostic.diagnostic
                ));
            }
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
        let check_speed = feel_maximum_entry.unwrap_or(maximum_entry);
        let rating = rate_piece(model, cars, template, check_speed, unpowered()).unwrap();
        let class = model.design_force_targets.classify_force(
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
                "{:?}: {} default {cars}-car train has {class:?} airtime ({:.3} g) at the {check_speed} m/s feel maximum; expected Great",
                template.id, model.name, rating.minimum_filtered_vertical
            ));
        }
    }

    ScannedWindow {
        template,
        model_key: model.key,
        minimum_entry: Some(minimum_entry),
        maximum_entry: Some(maximum_entry),
        measured_from: Some(measured_from),
        minimum_source: Some(minimum_source),
        maximum_source: Some(maximum_source),
        feel_minimum_entry,
        feel_maximum_entry,
        feel_maximum_source,
        unavailable_reason: None,
        upper_failure,
        minimum_failure: minimum_failure.map(|failure| failure.diagnostic),
        passes_again_above,
        airtime,
        acceptance_failures,
    }
}

/// Scans the inner feel window inside `[hard_min, hard_max]` using the model's `DesignForceTargets`,
/// hill airtime feel ceiling, and speed-turn catalog design caps.
fn scan_feel(
    template: &'static Piece,
    model: &CoasterModel,
    hard_min: f64,
    hard_max: f64,
    hard_max_source: RatedWindowSource,
) -> (Option<f64>, Option<f64>, Option<RatedWindowSource>) {
    let old = template.design_speeds.unwrap();
    let judge = ForceJudge::DesignTargets(model.design_force_targets);
    let first_index = (hard_min / GRID_STEP).floor() as usize + 1;
    let last_index = (hard_max / GRID_STEP).floor() as usize;
    let mut grid: Vec<(f64, Option<ScanFailure>)> = Vec::new();
    for index in first_index..=last_index {
        let speed = index as f64 * GRID_STEP;
        if speed > hard_min && speed < hard_max {
            grid.push((speed, failure_with_judge(template, model, speed, judge)));
        }
    }
    grid.push((
        hard_max,
        failure_with_judge(template, model, hard_max, judge),
    ));

    let min_failure = failure_with_judge(template, model, hard_min, judge);
    let feel_min = match &min_failure {
        None => hard_min,
        Some(diagnostic) => {
            let Some(first_pass) = grid.iter().position(|(_, failure)| failure.is_none()) else {
                return (None, None, None);
            };
            let boundary_failure = if first_pass == 0 {
                diagnostic
            } else {
                grid[first_pass - 1]
                    .1
                    .as_ref()
                    .expect("the sample before the first feel pass must fail")
            };
            if boundary_failure.kind == FailureKind::ApproachReachability {
                hard_min
            } else {
                let below = if first_pass == 0 {
                    hard_min
                } else {
                    grid[first_pass - 1].0
                };
                refine_boundary(below, grid[first_pass].0, true, |speed| {
                    failure_with_judge(template, model, speed, judge).is_none()
                })
                .max(hard_min)
            }
        }
    };

    let mut last_pass = feel_min;
    let mut first_failure: Option<f64> = None;
    for (speed, failure) in grid.iter().filter(|(speed, _)| *speed > feel_min) {
        match failure {
            None => last_pass = *speed,
            Some(diagnostic) if diagnostic.kind == FailureKind::ApproachReachability => {}
            Some(_) => {
                first_failure = Some(*speed);
                break;
            }
        }
    }

    let speed_turn_cap =
        matches!(template.family, Some(PieceFamily::SpeedTurn { .. })).then_some(old.maximum_entry);
    let (mut feel_max, mut feel_max_source) = match first_failure {
        None => (hard_max, hard_max_source),
        Some(failing_speed) => {
            let refined = refine_boundary(last_pass, failing_speed, false, |speed| {
                failure_with_judge(template, model, speed, judge).is_none()
            })
            .min(hard_max);
            (refined, RatedWindowSource::DesignTarget)
        }
    };
    if let Some(cap) = speed_turn_cap
        && cap >= feel_min
        && cap < feel_max
        && failure_with_judge(template, model, cap, judge).is_none()
    {
        feel_max = cap;
        feel_max_source = RatedWindowSource::DesignCap;
    }

    if feel_max < feel_min {
        return (None, None, None);
    }

    if !rated_hill_airtime_within_feel_ceiling(template, model, feel_min) {
        return (None, None, None);
    }
    if let Some(capped) = cap_maximum_for_hill_feel(template, model, feel_min, feel_max)
        && capped < feel_max
    {
        feel_max = capped;
        feel_max_source = RatedWindowSource::AirtimeFeel;
    }

    (Some(feel_min), Some(feel_max), Some(feel_max_source))
}

fn generated_rated_windows_contents(date: &str, windows: &[ScannedWindow]) -> String {
    let rows: Vec<String> = windows
        .iter()
        .filter(|window| window.minimum_entry.is_some())
        .map(|window| {
            let feel_source = match window.feel_maximum_source {
                Some(source) => format!("Some(RatedWindowSource::{source:?})"),
                None => "None".to_owned(),
            };
            format!(
                "    MeasuredRatedWindow {{ model_key: {:?}, id: PieceId::{:?}, entry_slope: Slope::{:?}, exit_slope: Slope::{:?}, minimum_source: RatedWindowSource::{:?}, maximum_source: RatedWindowSource::{:?}, minimum_entry: {:?}, maximum_entry: {:?}, measured_from: {:?}, feel_minimum_entry: {:?}, feel_maximum_entry: {:?}, feel_maximum_source: {feel_source} }},",
                window.model_key,
                window.template.id,
                window.template.entry.slope,
                window.template.exit.slope,
                window.minimum_source.unwrap(),
                window.maximum_source.unwrap(),
                window.minimum_entry.unwrap(),
                window.maximum_entry.unwrap(),
                window.measured_from.unwrap(),
                window.feel_minimum_entry,
                window.feel_maximum_entry,
            )
        })
        .collect();
    let unavailable_rows: Vec<String> = windows
        .iter()
        .filter(|window| window.minimum_entry.is_none())
        .map(|window| {
            assert!(window.maximum_entry.is_none());
            format!(
                "    UnavailableRatedWindow {{ model_key: {:?}, id: PieceId::{:?}, entry_slope: Slope::{:?}, exit_slope: Slope::{:?} }},",
                window.model_key,
                window.template.id,
                window.template.entry.slope,
                window.template.exit.slope
            )
        })
        .collect();
    let contents = format!(
        "// Generated by make generate-rated-windows on {date}: hard edge uses the global ASTM comfort envelope; feel edge uses model DesignForceTargets and hill feel.\n\
         #[rustfmt::skip]\n\
         const MEASURED_RATED_WINDOWS: &[MeasuredRatedWindow] = &[\n{}\n];\n",
        rows.join("\n")
    );
    format!(
        "{contents}#[rustfmt::skip]\nconst UNAVAILABLE_RATED_WINDOWS: &[UnavailableRatedWindow] = &[\n{}\n];\n",
        unavailable_rows.join("\n")
    )
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let first_argument = arguments.next();
    if first_argument.as_deref() == Some("--diagnose") {
        let piece = arguments.next().expect("--diagnose requires a piece ID");
        let model = arguments.next().expect("--diagnose requires a model key");
        let entry_slope = arguments
            .next()
            .expect("--diagnose requires an entry slope");
        let exit_slope = arguments.next().expect("--diagnose requires an exit slope");
        let requested_speeds: Vec<f64> = arguments
            .map(|speed| speed.parse().expect("diagnostic speeds must be numbers"))
            .collect();
        diagnose(&piece, &model, &entry_slope, &exit_slope, &requested_speeds);
        return;
    }
    let selected_piece = first_argument;
    assert!(
        arguments.next().is_none(),
        "usage: measure_force_windows [piece-id] | --diagnose <piece-id> <model-key> <entry-slope> <exit-slope> [speed ...]"
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
                    match (window.minimum_entry, window.maximum_entry) {
                        (Some(minimum), Some(maximum)) => eprintln!(
                            "scanned {} {:?} ({:?} to {:?}): {:.6}–{:.6}",
                            model.key,
                            template.id,
                            template.entry.slope,
                            template.exit.slope,
                            minimum,
                            maximum
                        ),
                        (None, None) => eprintln!(
                            "scanned {} {:?} ({:?} to {:?}): unavailable ({})",
                            model.key,
                            template.id,
                            template.entry.slope,
                            template.exit.slope,
                            window
                                .unavailable_reason
                                .as_deref()
                                .expect("unavailable scan has a reason")
                        ),
                        _ => panic!("scan returned an incomplete rated window"),
                    }
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
        "| Model | Template | Old window (m/s) | New window (m/s) | Measured from (m/s) | Minimum | Maximum | Failure just above the maximum | Failure at the old minimum | Passes again at |"
    );
    println!("| --- | --- | ---: | ---: | ---: | --- | --- | --- | --- | ---: |");
    for window in &windows {
        let old = window.template.design_speeds.unwrap();
        let new_window = match (window.minimum_entry, window.maximum_entry) {
            (Some(minimum), Some(maximum)) => format!("{minimum:.2}–{maximum:.2}"),
            (None, None) => "unavailable".into(),
            _ => panic!("scan returned an incomplete rated window"),
        };
        println!(
            "| `{}` | `{:?}` ({:?} → {:?}) | {:.2}–{:.2} | {} | {} | {:?} | {:?} | {} | {} | {} |",
            window.model_key,
            window.template.id,
            window.template.entry.slope,
            window.template.exit.slope,
            old.minimum_entry,
            old.maximum_entry,
            new_window,
            window
                .measured_from
                .map_or("—".to_string(), |speed| format!("{speed:.2}")),
            window.minimum_source,
            window.maximum_source,
            window
                .unavailable_reason
                .as_deref()
                .or(window.upper_failure.as_deref())
                .unwrap_or("—"),
            window.minimum_failure.as_deref().unwrap_or("—"),
            window
                .passes_again_above
                .map_or("—".to_string(), |speed| format!("{speed:.2}")),
        );
    }
    println!();
    for window in windows
        .iter()
        .filter(|window| window.maximum_entry.is_some() && !window.airtime.is_empty())
    {
        println!(
            "AIRTIME {} {:?} at {:.2} m/s: {}",
            window.model_key,
            window.template.id,
            window.maximum_entry.unwrap(),
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
        let contents = generated_rated_windows_contents(date, &windows);
        std::fs::write("crates/sim/src/track/rated_windows.generated.rs", contents)
            .expect("write generated rated windows");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FailureKind, MAXIMUM_ROLL_RATE, RatedWindowSource, ScannedWindow,
        cap_maximum_for_hill_feel, failure, generated_rated_windows_contents, minimum_source,
        refine_boundary, roll_rate_rule, scan, unmeasured_catalog,
    };
    use theme_park_sim::physics::rated_piece::{rate_piece, unpowered};
    use theme_park_sim::track::{
        CoasterModel, ForceRating, ForceRatingAxis, PieceId, Slope, coaster_models,
        family_coaster_model, hybrid_model, hyper_model,
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
        let limits = model.design_force_targets;
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
    fn unreachable_approach_remains_a_diagnostic_failure() {
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| {
                piece.id == PieceId::BankedStraightLeft && piece.entry.slope == Slope::DownGentle
            })
            .expect("catalog contains the downhill banked straight");
        let model = family_coaster_model();
        let speed = 1.0;
        let rating = rate_piece(&model, model.train.cars, piece, speed, unpowered()).unwrap();
        assert!(!rating.passed, "{rating:?}");
        assert_eq!(
            rating.failure,
            Some("no approach context can attain the requested entry speed"),
            "{rating:?}"
        );
        assert!(
            rating.context_results.iter().all(|context| {
                !context.applicable
                    && context.inapplicable_reason.is_some()
                    && context.failure.is_none()
            }),
            "{rating:?}"
        );
        let failure = failure(piece, &model, speed).expect("entry speed is not reachable");

        assert_eq!(failure.kind, FailureKind::ApproachReachability);
    }

    #[test]
    fn hybrid_sixty_down_to_level_uses_energy_minimum_and_records_measured_from() {
        let template = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::SixtyDownToLevel)
            .expect("catalog contains SixtyDownToLevel");
        let result = scan(template, &hybrid_model());

        assert_eq!(
            result.minimum_source,
            Some(RatedWindowSource::EnergyMinimum)
        );
        assert!((result.minimum_entry.unwrap() - 2.0).abs() < 0.01);
        assert!((result.measured_from.unwrap() - 12.56).abs() < 0.02);
        assert!(
            result.acceptance_failures.is_empty(),
            "{:?}",
            result.acceptance_failures
        );
    }

    #[test]
    fn passing_catalog_minimum_is_the_measured_from_speed() {
        let template = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::SixtyUpToLevel)
            .expect("catalog contains SixtyUpToLevel");
        let result = scan(template, &hybrid_model());

        assert_eq!(
            result.minimum_source,
            Some(RatedWindowSource::EnergyMinimum)
        );
        assert_eq!(result.measured_from, result.minimum_entry);
    }

    #[test]
    fn reachable_catalog_minimum_is_measured_even_when_clearance_raises_the_rating_minimum() {
        let model = hybrid_model();
        let template = unmeasured_catalog()
            .iter()
            .find(|piece| {
                piece.id == PieceId::BankLeft
                    && piece.entry.slope == Slope::UpGentle
                    && model.allows_piece(piece)
            })
            .expect("hybrid model allows a gentle-up BankLeft");
        let catalog_minimum = template.design_speeds.unwrap().minimum_entry;
        let result = scan(template, &model);

        assert_eq!(
            result.minimum_source,
            Some(RatedWindowSource::ClearanceMinimum)
        );
        assert!(result.minimum_entry.unwrap() > catalog_minimum);
        assert_eq!(result.measured_from, Some(catalog_minimum));
        assert_eq!(
            failure(template, &model, catalog_minimum)
                .expect("catalog minimum fails clearance")
                .kind,
            FailureKind::Other
        );
    }

    #[test]
    fn measured_force_failure_keeps_the_force_bound_source() {
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .expect("catalog contains the corkscrew");
        let model = hybrid_model();
        let failure = failure(piece, &model, 40.0).expect("40 m/s exceeds the force window");

        assert_eq!(failure.kind, FailureKind::Other);
        assert_eq!(minimum_source(&failure), RatedWindowSource::ForceBound);
    }

    #[test]
    fn refines_both_edges_and_rounds_into_the_passing_window() {
        let passes = |speed: f64| (5.123..=20.987).contains(&speed);
        assert_eq!(refine_boundary(5.0, 5.25, true, passes), 5.13);
        assert_eq!(refine_boundary(20.75, 21.0, false, passes), 20.98);
    }

    #[test]
    fn generated_table_records_a_scanned_combination_without_a_passing_window() {
        let template = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::BankLeft && piece.entry.slope == Slope::Level)
            .expect("catalog contains level BankLeft");
        let mut model = hyper_model();
        model.key = "no_passing_window_test_model";
        model.design_force_targets.maximum_vertical_g = 0.01;
        let scan_result = scan(template, &model);
        assert!(scan_result.minimum_entry.is_some());
        assert!(scan_result.maximum_entry.is_some());
        assert_eq!(scan_result.feel_minimum_entry, None);
        assert_eq!(scan_result.feel_maximum_entry, None);
        assert_eq!(scan_result.feel_maximum_source, None);

        let unavailable = ScannedWindow {
            template,
            model_key: "no_passing_window_test_model",
            minimum_entry: None,
            maximum_entry: None,
            measured_from: None,
            minimum_source: None,
            maximum_source: None,
            feel_minimum_entry: None,
            feel_maximum_entry: None,
            feel_maximum_source: None,
            unavailable_reason: Some("synthetic unavailable window".into()),
            upper_failure: None,
            minimum_failure: None,
            passes_again_above: None,
            airtime: Vec::new(),
            acceptance_failures: Vec::new(),
        };
        let generated = generated_rated_windows_contents("2026-11-21", &[unavailable]);

        assert!(generated.contains(
            "UnavailableRatedWindow { model_key: \"no_passing_window_test_model\", id: PieceId::BankLeft, entry_slope: Slope::Level, exit_slope: Slope::Level }"
        ));
        assert!(!generated.contains("MeasuredRatedWindow { model_key:"));
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
                    let rating =
                        rate_piece(model, model.train.cars, piece, speed, unpowered()).unwrap();
                    model.design_force_targets.classify_force(
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
            let rating = rate_piece(&model, model.train.cars, piece, speed, unpowered()).unwrap();
            model.design_force_targets.classify_force(
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
        restrictive_model.design_force_targets.maximum_vertical_g = 3.2;
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
        assert_eq!(restrictive.minimum_entry, existing.minimum_entry);
        assert_eq!(restrictive.maximum_entry, existing.maximum_entry);
        assert!(
            restrictive.feel_maximum_entry.unwrap() < existing.feel_maximum_entry.unwrap(),
            "restrictive model feel window {} should be narrower than {}",
            restrictive.feel_maximum_entry.unwrap(),
            existing.feel_maximum_entry.unwrap()
        );
    }

    #[test]
    fn scans_each_bank_transition_with_its_own_roll_rate_rule() {
        let templates = unmeasured_catalog();
        let model = hybrid_model();
        let transition_ids = [
            PieceId::BankLeft,
            PieceId::BankRight,
            PieceId::UnbankLeft,
            PieceId::UnbankRight,
            PieceId::RolloutLeft,
            PieceId::RolloutRight,
        ];
        let mut available_windows = 0;

        for id in transition_ids {
            let template = templates
                .iter()
                .find(|piece| piece.id == id && piece.entry.slope == Slope::Level)
                .unwrap_or_else(|| panic!("catalog has no level {id:?}"));
            assert!(
                template.design_speeds.is_some(),
                "{id:?} has no rated window"
            );
            assert!(
                model.allows_piece(template),
                "{} does not allow {id:?}",
                model.key
            );
            let roll_rate = roll_rate_rule(template)
                .unwrap_or_else(|| panic!("{id:?} is missing the roll-rate rule"));
            let result = scan(template, &model);

            match (result.minimum_entry, result.maximum_entry) {
                (Some(minimum), Some(maximum)) => {
                    available_windows += 1;
                    assert!(minimum <= maximum, "{id:?} has an inverted window");
                    assert!(
                        result.acceptance_failures.is_empty(),
                        "{id:?} has acceptance failures: {:?}",
                        result.acceptance_failures
                    );
                    assert!(
                        roll_rate(maximum) <= MAXIMUM_ROLL_RATE,
                        "{id:?} reaches {:.3} degrees/s at {maximum:.2} m/s",
                        roll_rate(maximum)
                    );
                }
                (None, None) => assert!(
                    result.unavailable_reason.is_some(),
                    "{id:?} has no passing window but no unavailable reason"
                ),
                _ => panic!("{id:?} has an incomplete measured window"),
            }
        }

        assert!(
            available_windows > 0,
            "no bank transition has a rated window"
        );
    }

    #[test]
    fn bank_transition_maximum_endpoint_passes_for_the_hyper_model() {
        let template = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::BankLeft && piece.entry.slope == Slope::Level)
            .expect("catalog has level BankLeft");
        let model = hyper_model();

        let result = scan(template, &model);

        assert!(
            result.acceptance_failures.is_empty(),
            "BankLeft measured maximum {:?} fails: {:?}",
            result.maximum_entry,
            result.acceptance_failures
        );
        let maximum = result
            .maximum_entry
            .expect("hyper BankLeft has a passing window");
        assert_eq!(failure(template, &model, maximum), None);
    }
}
