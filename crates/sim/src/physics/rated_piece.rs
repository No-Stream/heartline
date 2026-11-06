use std::collections::{HashSet, VecDeque};

use super::{Circuit, ForceEvaluator, GForces, PhysicsConfig, TIME_STEP, Train, rider_g};
use crate::track::{CoasterModel, Piece, PieceId, TrackState, catalog};

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
    let mut straight = catalog()
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
    let station = catalog()
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
        for (index, template) in catalog().iter().enumerate() {
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
        let next = catalog()[index].at(state).unwrap();
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
    assert!(model.cars_per_train_range().contains(&cars));
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
    let mut violation: Option<ForceViolation> = None;
    let mut failure = None;
    for _ in 0..100_000 {
        train.step(&track.circuit);
        for frame in train.take_readings() {
            for (car, reading) in frame.into_iter().enumerate() {
                let result = evaluators[car].update(reading.g);
                for axis in 0..3 {
                    excursions[car][axis] = if result.axis_exceeding_sustained[axis] {
                        excursions[car][axis] + TIME_STEP
                    } else {
                        0.0
                    };
                }
                if reading.distance >= track.element_start && !done[car] {
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
                    for axis in 0..3 {
                        if !result.axis_within_limits[axis] && violation.is_none() {
                            let GForces {
                                vertical,
                                lateral,
                                longitudinal,
                            } = result.filtered;
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
                            if force.abs() > first_violation.force.abs() {
                                first_violation.force = force;
                                first_violation.distance = reading.distance - track.element_start;
                                first_violation.in_tail = in_tail;
                            }
                            first_violation.excursion_seconds = first_violation
                                .excursion_seconds
                                .max(excursions[car][first_violation.axis]);
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
        violation,
        failure,
    })
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
}
