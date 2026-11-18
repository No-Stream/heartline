use std::collections::{HashSet, VecDeque};
use std::ops::Range;

use super::{Circuit, ForceEvaluator, GForces, PhysicsConfig, TIME_STEP, Train, rider_g};
use crate::track::catalog::unmeasured_catalog;
use crate::track::{
    CoasterModel, ComfortableForceLimits, Piece, PieceId, TrackState, TrainConfiguration,
};

#[derive(Debug)]
pub struct ForceViolation {
    pub car: usize,
    pub axis: usize,
    pub force: f64,
    pub distance: f64,
    pub in_tail: bool,
    pub excursion_seconds: f64,
}

#[derive(Debug)]
pub struct PieceRating {
    pub passed: bool,
    pub entry_speed: f64,
    pub minimum_speed: f64,
    pub minimum_filtered_vertical: f64,
    pub minimum_filtered_force: [f64; 3],
    pub maximum_filtered_force: [f64; 3],
    pub violation: Option<ForceViolation>,
    pub failure: Option<&'static str>,
}

struct MeasurementTrack {
    circuit: Circuit,
    start: f64,
    element_start: f64,
    element_end: f64,
}

pub fn unpowered() -> PhysicsConfig {
    PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    }
}

fn scaled_train(model: &CoasterModel, cars: u8) -> crate::track::TrainConfiguration {
    let mut train = model.train.clone();
    train.length_metres *= f64::from(cars) / f64::from(train.cars);
    train.cars = cars;
    train
}

fn same_connector(left: TrackState, right: TrackState) -> bool {
    left.slope == right.slope
        && left.bank == right.bank
        && left.horizontal_curvature == right.horizontal_curvature
        && left.vertical_curvature == right.vertical_curvature
}

fn connector_key(state: TrackState) -> (u8, u8, u8, u8) {
    (
        state.slope as u8,
        state.bank as u8,
        state.horizontal_curvature as u8,
        state.vertical_curvature as u8,
    )
}

fn straight_for(state: TrackState) -> Piece {
    let mut straight = unmeasured_catalog()
        .iter()
        .find(|p| {
            p.entry.slope == state.slope
                && p.exit.slope == state.slope
                && p.entry.bank == crate::track::Bank::Level
                && p.exit.bank == crate::track::Bank::Level
                && same_connector(p.entry, p.exit)
                && matches!(
                    p.id,
                    PieceId::Straight
                        | PieceId::GentleUp
                        | PieceId::GentleDown
                        | PieceId::SteepUp
                        | PieceId::SteepDown
                        | PieceId::SixtyUp
                        | PieceId::SixtyDown
                )
        })
        .expect("no straight for connector slope")
        .clone();
    straight.entry.bank = state.bank;
    straight.exit.bank = state.bank;
    assert!(
        same_connector(straight.entry, state),
        "no straight for {state:?}"
    );
    straight
}

fn measurement_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
) -> MeasurementTrack {
    let station = unmeasured_catalog()
        .iter()
        .find(|p| p.id == PieceId::Station)
        .unwrap()
        .clone();
    let mut pieces = vec![station.clone()];
    let mut state = station.exit;
    let mut queue = VecDeque::from([(state, Vec::<usize>::new())]);
    let mut seen = HashSet::from([connector_key(state)]);
    let path = loop {
        let (current, path) = queue.pop_front().expect("no connected approach to piece");
        if same_connector(current, piece.entry) {
            break path;
        }
        for (index, template) in unmeasured_catalog().iter().enumerate() {
            if let Some(next) = template.exit_at(current) {
                if seen.insert(connector_key(next)) {
                    let mut next_path = path.clone();
                    next_path.push(index);
                    queue.push_back((next, next_path));
                }
            }
        }
    };
    for index in path {
        let next = unmeasured_catalog()[index].at(state).unwrap();
        state = next.exit;
        pieces.push(next);
    }
    let approach = straight_for(state);
    let train_length = scaled_train(model, cars).length_metres;
    let needed = train_length + 10.0_f64.max(0.5 * speed);
    let mut approach_length = 0.0;
    while approach_length < needed {
        let next = approach.at(state).unwrap();
        approach_length += next.length();
        state = next.exit;
        pieces.push(next);
    }
    let element_start: f64 = pieces.iter().map(Piece::length).sum();
    let start = element_start - 10.0_f64.max(0.5 * speed);
    let element = piece.at(state).unwrap();
    state = element.exit;
    let element_end = element_start + element.length();
    pieces.push(element);
    let exit_straight = straight_for(state);
    let mut exit_length = 0.0;
    while exit_length < train_length + 1.5 * speed + 40.0 {
        let next = exit_straight.at(state).unwrap();
        exit_length += next.length();
        state = next.exit;
        pieces.push(next);
    }
    let mut starts = Vec::with_capacity(pieces.len());
    let mut length = 0.0;
    for piece in &pieces {
        starts.push(length);
        length += piece.length();
    }
    MeasurementTrack {
        circuit: Circuit {
            pieces,
            starts,
            length,
            open: true,
        },
        start,
        element_start,
        element_end,
    }
}

pub fn rate_piece(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    target: f64,
    physics: PhysicsConfig,
) -> Result<PieceRating, String> {
    assert!(target.is_finite() && target > 0.0);
    assert!(
        physics.rolling_resistance == 0.0
            && physics.drag_coefficient == 0.0
            && !physics.powered_sections,
        "rating requires unpowered frictionless physics"
    );
    assert!(model.cars_per_train.contains(&cars));
    let track = measurement_track(model, cars, piece, target);
    let config = scaled_train(model, cars);
    let spacing = config.length_metres / f64::from(cars);
    let mut evaluators =
        vec![ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP); usize::from(cars)];
    let approach_steps =
        ((track.element_start - track.start) / (target * TIME_STEP)).ceil() as usize;
    let start = track.element_start - approach_steps as f64 * target * TIME_STEP;
    for step in 1..=approach_steps {
        let distance = start + step as f64 * target * TIME_STEP;
        for (car, evaluator) in evaluators.iter_mut().enumerate() {
            let car_distance = distance - car as f64 * spacing;
            let (index, local) = track.circuit.locate(car_distance);
            evaluator.update(rider_g(&track.circuit.pieces[index], local, target, 0.0));
        }
    }
    let mut train = Train::new(
        &track.circuit,
        &config,
        physics,
        track.element_start,
        target,
    );
    let actual = train.speed();
    let mut elapsed_tail = vec![0.0; usize::from(cars)];
    let mut done = vec![false; usize::from(cars)];
    let mut excursions = vec![[0.0; 3]; usize::from(cars)];
    let mut cleared = vec![false; usize::from(cars)];
    let mut minimum_speed = f64::INFINITY;
    let mut minimum_filtered_vertical = f64::INFINITY;
    let mut minimum_filtered_force = [f64::INFINITY; 3];
    let mut maximum_filtered_force = [f64::NEG_INFINITY; 3];
    let mut violation: Option<ForceViolation> = None;
    let mut violation_excursion_active = false;
    let mut failure = None;
    for _ in 0..100_000 {
        train.step(&track.circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.into_iter().enumerate() {
                let result = evaluators[car].update(reading.g);
                for (duration, exceeding) in excursions[car]
                    .iter_mut()
                    .zip(result.axis_exceeding_sustained)
                {
                    *duration = if exceeding {
                        *duration + TIME_STEP
                    } else {
                        0.0
                    };
                }
                if reading.distance >= track.element_start && !done[car] {
                    for (axis, force) in [
                        result.filtered.vertical,
                        result.filtered.lateral,
                        result.filtered.longitudinal,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        minimum_filtered_force[axis] = minimum_filtered_force[axis].min(force);
                        maximum_filtered_force[axis] = maximum_filtered_force[axis].max(force);
                    }
                    let in_tail = reading.distance >= track.element_end;
                    if in_tail {
                        if !cleared[car] {
                            cleared[car] = true;
                            if car + 1 == usize::from(cars)
                                && matches!(
                                    piece.id,
                                    PieceId::HalfCorkscrewLeftDown
                                        | PieceId::HalfCorkscrewRightDown
                                )
                                && reading.speed > 22.0
                            {
                                failure = Some("clearing speed exceeds 22 m/s");
                            }
                        }
                        elapsed_tail[car] += TIME_STEP;
                    } else {
                        minimum_speed = minimum_speed.min(reading.speed);
                    }
                    if !in_tail {
                        minimum_filtered_vertical =
                            minimum_filtered_vertical.min(result.filtered.vertical);
                    }
                    for axis in 0..3 {
                        if !result.axis_within_limits[axis] && violation.is_none() {
                            let GForces {
                                vertical,
                                lateral,
                                longitudinal,
                            } = result.filtered;
                            violation_excursion_active = true;
                            violation = Some(ForceViolation {
                                car,
                                axis,
                                force: [vertical, lateral, longitudinal][axis],
                                distance: reading.distance - track.element_start,
                                in_tail,
                                excursion_seconds: excursions[car][axis],
                            });
                        }
                    }
                    if let Some(first_violation) = violation.as_mut() {
                        if first_violation.car == car {
                            let force = [
                                result.filtered.vertical,
                                result.filtered.lateral,
                                result.filtered.longitudinal,
                            ][first_violation.axis];
                            if violation_excursion_active
                                && result.axis_exceeding_sustained[first_violation.axis]
                                && force.is_sign_positive()
                                    == first_violation.force.is_sign_positive()
                            {
                                if force.abs() > first_violation.force.abs() {
                                    first_violation.force = force;
                                    first_violation.distance =
                                        reading.distance - track.element_start;
                                    first_violation.in_tail = in_tail;
                                }
                                first_violation.excursion_seconds = first_violation
                                    .excursion_seconds
                                    .max(excursions[car][first_violation.axis]);
                            } else {
                                violation_excursion_active = false;
                            }
                        }
                    }
                    if in_tail
                        && elapsed_tail[car] >= 0.5
                        && !result.axis_exceeding_sustained.iter().any(|v| *v)
                    {
                        done[car] = true;
                    }
                }
            }
        }
        if done.iter().all(|done| *done) {
            break;
        }
        if train.speed() <= 0.0 || !train.notes().is_empty() {
            failure = Some("stalled or rolled back");
            break;
        }
        if train.distance() >= track.circuit.length() - 1.0 {
            failure = Some("exit straight too short");
            break;
        }
    }
    if !done.iter().all(|done| *done) && failure.is_none() {
        failure = Some("train did not clear");
    }
    let required_minimum = if matches!(
        piece.id,
        PieceId::ZeroGRollFastLeft
            | PieceId::ZeroGRollFastRight
            | PieceId::ZeroGRollCompactLeft
            | PieceId::ZeroGRollCompactRight
            | PieceId::StallLeft
            | PieceId::StallRight
    ) {
        6.0
    } else if piece.exit.position().z > piece.entry.position().z
        || matches!(
            piece.id,
            PieceId::Loop | PieceId::HalfLoopUp | PieceId::TopHat | PieceId::LaunchedTopHat
        )
    {
        5.0
    } else {
        0.0
    };
    if minimum_speed < required_minimum && failure.is_none() {
        failure = Some("minimum clearance speed");
    }
    Ok(PieceRating {
        passed: failure.is_none() && violation.is_none(),
        entry_speed: actual,
        minimum_speed,
        minimum_filtered_vertical,
        minimum_filtered_force,
        maximum_filtered_force,
        violation,
        failure,
    })
}

#[derive(Debug)]
pub struct SequenceViolation {
    pub car: usize,
    pub axis: usize,
    pub piece: Option<usize>,
    pub distance: f64,
    pub value: f64,
}

#[derive(Debug)]
pub struct SequenceRating {
    pub violation: Option<SequenceViolation>,
    pub minimum_speeds: Vec<f64>,
    pub failure: Option<&'static str>,
}

pub fn rate_sequence(
    circuit: &Circuit,
    ranges: &[Range<f64>],
    configuration: &TrainConfiguration,
    physics: PhysicsConfig,
    limits: ComfortableForceLimits,
    approach_start: f64,
    start_speed: f64,
) -> SequenceRating {
    assert!(!ranges.is_empty());
    assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
    assert!(approach_start < ranges[0].start && start_speed > 0.0);
    assert!(ranges.last().unwrap().end < circuit.length());
    let mut train = Train::new(circuit, configuration, physics, approach_start, start_speed);
    let mut evaluators =
        vec![ForceEvaluator::new(limits, TIME_STEP); usize::from(configuration.cars)];
    let mut seen = vec![vec![false; ranges.len()]; usize::from(configuration.cars)];
    let mut minimum_speeds = vec![f64::INFINITY; ranges.len()];
    let mut tail_seconds = vec![0.0; usize::from(configuration.cars)];
    let mut done = vec![false; usize::from(configuration.cars)];
    let mut violation = None;
    let last_end = ranges.last().unwrap().end;
    let final_piece = circuit.piece_index(last_end - 1e-9);
    let expected_tail = straight_for(circuit.pieces()[final_piece].exit).id;
    let mut failure = None;
    for _ in 0..100_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.into_iter().enumerate() {
                let evaluated = evaluators[car].update(reading.g);
                let piece = ranges
                    .iter()
                    .position(|range| range.contains(&reading.distance));
                if let Some(piece) = piece {
                    seen[car][piece] = true;
                    minimum_speeds[piece] = minimum_speeds[piece].min(reading.speed);
                }
                if reading.distance >= ranges[0].start && !done[car] {
                    if reading.distance >= last_end {
                        if circuit.pieces()[circuit.piece_index(reading.distance)].id
                            != expected_tail
                        {
                            failure = Some("exit tail is not a matching straight");
                            continue;
                        }
                        tail_seconds[car] += TIME_STEP;
                    }
                    if violation.is_none() {
                        for axis in 0..3 {
                            if !evaluated.axis_within_limits[axis] {
                                violation = Some(SequenceViolation {
                                    car,
                                    axis,
                                    piece,
                                    distance: reading.distance,
                                    value: [
                                        evaluated.filtered.vertical,
                                        evaluated.filtered.lateral,
                                        evaluated.filtered.longitudinal,
                                    ][axis],
                                });
                                break;
                            }
                        }
                    }
                    if tail_seconds[car] >= 0.5
                        && !evaluated
                            .axis_exceeding_sustained
                            .iter()
                            .any(|exceeding| *exceeding)
                    {
                        done[car] = true;
                    }
                }
            }
        }
        if failure.is_some() || done.iter().all(|cleared| *cleared) {
            break;
        }
        if train.speed() <= 0.0 || !train.notes().is_empty() {
            failure = Some("stalled or rolled back");
            break;
        }
        if train.distance() >= circuit.length() - 1.0 {
            failure = Some("exit tail too short");
            break;
        }
    }
    if failure.is_none() && !done.iter().all(|cleared| *cleared) {
        failure = Some("train did not clear the tail");
    }
    if failure.is_none() && !seen.iter().flatten().all(|entered| *entered) {
        failure = Some("not every car entered every piece");
    }
    SequenceRating {
        violation,
        minimum_speeds,
        failure,
    }
}

/// Solve an unpowered start speed for a target speed at a later track boundary.
/// Speeds at the boundary are interpolated between the two enclosing integration steps.
pub fn solve_approach_speed(
    circuit: &Circuit,
    configuration: &TrainConfiguration,
    physics: PhysicsConfig,
    approach_start: f64,
    target_distance: f64,
    target_speed: f64,
) -> Result<f64, String> {
    assert!(
        physics.rolling_resistance == 0.0
            && physics.drag_coefficient == 0.0
            && !physics.powered_sections
    );
    assert!(
        0.0 <= approach_start
            && approach_start < target_distance
            && target_distance < circuit.length()
    );
    assert!(target_speed.is_finite() && target_speed > 0.0);
    let cars = usize::from(configuration.cars);
    let spacing = configuration.length_metres / cars as f64;
    let average_height = |distance: f64| {
        (0..cars)
            .map(|car| circuit.sample(distance - car as f64 * spacing).position.z)
            .sum::<f64>()
            / cars as f64
    };
    let start_squared = target_speed.powi(2)
        + 2.0 * super::GRAVITY * (average_height(target_distance) - average_height(approach_start));
    if start_squared <= 0.0 {
        return Err(format!(
            "target {target_speed} m/s is unreachable from {approach_start} m"
        ));
    }
    let speed_at_boundary = |start_squared: f64| -> Result<f64, String> {
        if start_squared <= 0.0 {
            return Err(format!(
                "non-positive proposed squared approach speed {start_squared}"
            ));
        }
        let mut train = Train::new(
            circuit,
            configuration,
            physics,
            approach_start,
            start_squared.sqrt(),
        );
        for _ in 0..100_000 {
            let before_distance = train.distance();
            let before_speed = train.speed();
            train.step_block_brake_without_readings(circuit, &[], false);
            if train.distance() >= target_distance {
                let fraction =
                    (target_distance - before_distance) / (train.distance() - before_distance);
                return Ok(before_speed + fraction * (train.speed() - before_speed));
            }
            if train.speed() <= 0.0 || !train.notes().is_empty() {
                return Err("train stalled before target boundary".into());
            }
        }
        Err("target boundary was not reached".into())
    };
    let mut previous_squared = start_squared;
    let mut previous_error = speed_at_boundary(previous_squared)? - target_speed;
    if previous_error.abs() <= 1e-6 {
        return Ok(previous_squared.sqrt());
    }
    let mut current_squared = start_squared + start_squared.max(1.0) * 0.01;
    let mut current_error = speed_at_boundary(current_squared)? - target_speed;
    for _ in 0..4 {
        if current_error.abs() <= 1e-6 {
            return Ok(current_squared.sqrt());
        }
        if (current_error - previous_error).abs() < f64::EPSILON {
            return Err("approach speed solver lost its secant slope".into());
        }
        let next_squared = current_squared
            - current_error * (current_squared - previous_squared)
                / (current_error - previous_error);
        previous_squared = current_squared;
        previous_error = current_error;
        current_squared = next_squared;
        current_error = speed_at_boundary(current_squared)? - target_speed;
    }
    if current_error.abs() <= 1e-6 {
        Ok(current_squared.sqrt())
    } else {
        Err(format!(
            "approach speed solver failed: target {target_speed}, error {current_error}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{PieceId, catalog, hybrid_model};

    #[test]
    fn held_approach_hits_target_on_level_sixty_down_and_inverted_straights() {
        let model = hybrid_model();
        for id in [
            PieceId::Loop,
            PieceId::SixtyToSteepDown,
            PieceId::HalfCorkscrewLeftDown,
        ] {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let rating = rate_piece(&model, model.train.cars, piece, 25.0, unpowered()).unwrap();
            assert!(
                (rating.entry_speed - 25.0).abs() <= 1e-6,
                "{id:?}: {rating:?}"
            );
        }
    }

    #[test]
    fn downhill_approach_holds_low_entry_speed() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::SixtyToSteepDown)
            .unwrap();
        let rating = rate_piece(&model, model.train.cars, piece, 1.0, unpowered()).unwrap();
        assert!((rating.entry_speed - 1.0).abs() <= 1e-6, "{rating:?}");
    }

    #[test]
    fn loop_rating_detects_force_failure_and_a_passing_speed() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let passing = rate_piece(&model, model.train.cars, piece, 25.0, unpowered()).unwrap();
        assert!(passing.passed, "{passing:?}");
        let excessive = rate_piece(&model, model.train.cars, piece, 40.0, unpowered()).unwrap();
        assert!(excessive.violation.is_some(), "{excessive:?}");
        assert!(!excessive.passed);
    }

    #[test]
    fn violation_diagnostics_preserve_the_force_side_that_failed() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::EjectorHill12x64)
            .unwrap();
        let speed = piece.rated_speeds.unwrap().maximum_entry + 1.0;
        let result = rate_piece(&model, 8, piece, speed, unpowered()).unwrap();
        let violation = result
            .violation
            .as_ref()
            .expect("1 m/s above the rated maximum the eight-car train exceeds the vertical limit");
        assert_eq!(violation.axis, 0);
        assert!(
            violation.force < model.comfortable_force_limits.minimum_vertical_g,
            "{violation:?}"
        );
        assert!(
            result.minimum_filtered_vertical < model.comfortable_force_limits.minimum_vertical_g,
            "{result:?}"
        );
    }

    #[test]
    fn held_approach_matches_real_train_at_force_boundaries() {
        let model = hybrid_model();
        let cars = 8;
        for id in [
            PieceId::EjectorHill12x64,
            PieceId::EjectorHill12x80,
            PieceId::EjectorHill16x96,
            PieceId::LevelToSixtyDown,
        ] {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let target = piece.rated_speeds.unwrap().maximum_entry;
            let rating = rate_piece(&model, cars, piece, target, unpowered()).unwrap();
            let track = measurement_track(&model, cars, piece, target);
            let config = scaled_train(&model, cars);
            let steps =
                ((track.element_start - track.start) / (target * TIME_STEP)).ceil() as usize;
            let start = track.element_start - steps as f64 * target * TIME_STEP;
            let mut train = Train::new(&track.circuit, &config, unpowered(), start, target);
            let mut evaluators = vec![
                ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP);
                usize::from(cars)
            ];
            let mut minimum_vertical = f64::INFINITY;
            let mut first_violation = None;
            let mut cleared_element = false;
            for _ in 0..20_000 {
                train.step(&track.circuit);
                for frame in train.take_readings() {
                    for (car, reading) in frame.into_iter().enumerate() {
                        let evaluation = evaluators[car].update(reading.g);
                        if (track.element_start..track.element_end).contains(&reading.distance) {
                            minimum_vertical = minimum_vertical.min(evaluation.filtered.vertical);
                        }
                        if reading.distance >= track.element_start && first_violation.is_none() {
                            for axis in 0..3 {
                                if !evaluation.axis_within_limits[axis] {
                                    first_violation = Some((car, axis));
                                    break;
                                }
                            }
                        }
                    }
                }
                if train.distance() >= track.element_end + config.length_metres + target {
                    cleared_element = true;
                    break;
                }
                assert!(train.speed() > 0.0 && train.notes().is_empty(), "{id:?}");
            }
            assert!(cleared_element && minimum_vertical.is_finite(), "{id:?}");
            assert_eq!(
                first_violation,
                rating
                    .violation
                    .as_ref()
                    .map(|violation| (violation.car, violation.axis)),
                "{id:?}"
            );
            // The independent full-trace comparison differed by at most 1.782e-6 g.
            assert!(
                (minimum_vertical - rating.minimum_filtered_vertical).abs() < 3e-6,
                "{id:?}: real={minimum_vertical}, held={}",
                rating.minimum_filtered_vertical
            );
        }
    }

    #[test]
    fn every_rated_piece_has_a_connected_straight_approach_and_exit() {
        let model = hybrid_model();
        for piece in catalog()
            .iter()
            .filter(|piece| piece.rated_speeds.is_some())
        {
            let track = measurement_track(&model, model.train.cars, piece, 25.0);
            assert!(track.element_end > track.element_start, "{:?}", piece.id);
        }
    }
    #[test]
    fn sequence_rating_keeps_per_car_history_and_detects_loop_overspeed() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let track = measurement_track(&model, model.train.cars, piece, 40.0);
        let mut train = model.train.clone();
        train.cars = 12;
        for (speed, passes) in [(25.0, true), (40.0, false)] {
            let rating = rate_sequence(
                &track.circuit,
                &[track.element_start..track.element_end],
                &train,
                unpowered(),
                model.comfortable_force_limits,
                track.start,
                speed,
            );
            assert!(rating.failure.is_none(), "{rating:?}");
            assert_eq!(rating.violation.is_none(), passes, "{rating:?}");
            assert!(rating.minimum_speeds[0].is_finite());
            if let Some(violation) = rating.violation {
                assert_eq!(violation.piece, Some(0));
                assert_eq!(violation.axis, 0);
            }
        }
    }

    #[test]
    fn sequence_rating_reports_violating_piece_after_two_straights() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let track = measurement_track(&model, model.train.cars, piece, 40.0);
        let index = track
            .circuit
            .starts
            .iter()
            .position(|start| *start == track.element_start)
            .unwrap();
        let ranges = [
            track.circuit.starts[index - 2]..track.circuit.starts[index - 1],
            track.circuit.starts[index - 1]..track.element_start,
            track.element_start..track.element_end,
        ];
        let start = track.circuit.starts[index - 3];
        let solved = solve_approach_speed(
            &track.circuit,
            &model.train,
            unpowered(),
            start,
            track.element_start,
            40.0,
        )
        .unwrap();
        let rating = rate_sequence(
            &track.circuit,
            &ranges,
            &model.train,
            unpowered(),
            model.comfortable_force_limits,
            start,
            solved,
        );
        assert!(rating.failure.is_none(), "{rating:?}");
        assert_eq!(rating.violation.as_ref().unwrap().piece, Some(2));
        assert_eq!(rating.violation.as_ref().unwrap().axis, 0);
        assert!(rating.minimum_speeds.iter().all(|speed| speed.is_finite()));
    }

    #[test]
    fn sequence_rating_reports_stall() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let track = measurement_track(&model, model.train.cars, piece, 25.0);
        let rating = rate_sequence(
            &track.circuit,
            &[track.element_start..track.element_end],
            &model.train,
            unpowered(),
            model.comfortable_force_limits,
            track.start,
            10.0,
        );
        assert_eq!(rating.failure, Some("stalled or rolled back"));
    }

    #[test]
    fn sequence_approach_solver_hits_requested_speed_on_level_up_and_down() {
        let model = hybrid_model();
        for id in [
            PieceId::Loop,
            PieceId::SixtyToSteepUp,
            PieceId::SixtyToSteepDown,
        ] {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let track = measurement_track(&model, model.train.cars, piece, 25.0);
            let solved = solve_approach_speed(
                &track.circuit,
                &model.train,
                unpowered(),
                track.start,
                track.element_start,
                25.0,
            )
            .unwrap();
            let mut train = Train::new(
                &track.circuit,
                &model.train,
                unpowered(),
                track.start,
                solved,
            );
            let actual = loop {
                let distance = train.distance();
                let speed = train.speed();
                train.step_block_brake_without_readings(&track.circuit, &[], false);
                if train.distance() >= track.element_start {
                    let fraction = (track.element_start - distance) / (train.distance() - distance);
                    break speed + fraction * (train.speed() - speed);
                }
                assert!(train.speed() > 0.0);
            };
            assert!((actual - 25.0).abs() <= 1e-6, "{id:?}: {actual}");
        }
    }

    #[test]
    fn sequence_approach_solver_reports_unreachable_speed_downhill() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::SixtyToSteepDown)
            .unwrap();
        let track = measurement_track(&model, model.train.cars, piece, 25.0);
        let result = solve_approach_speed(
            &track.circuit,
            &model.train,
            unpowered(),
            track.start,
            track.element_start,
            1.0,
        );
        assert!(result.unwrap_err().contains("unreachable"));
    }
}
