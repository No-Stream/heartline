use std::collections::{HashSet, VecDeque};
use std::ops::Range;

use super::{Circuit, ForceEvaluator, GForces, PhysicsConfig, TIME_STEP, Train};
use crate::track::catalog::unmeasured_catalog;
use crate::track::{
    CoasterModel, ComfortableForceLimits, Piece, PieceId, TrackState, TrainConfiguration,
};

#[derive(Clone, Debug)]
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
    pub limiting_context: &'static str,
    pub context_results: Vec<PieceContextRating>,
}

#[derive(Debug)]
pub struct PieceContextRating {
    pub context: &'static str,
    pub applicable: bool,
    pub passed: bool,
    pub entry_speed: f64,
    pub minimum_speed: f64,
    pub minimum_filtered_vertical: f64,
    pub minimum_filtered_force: [f64; 3],
    pub maximum_filtered_force: [f64; 3],
    pub first_lead_filtered_force: Option<GForces>,
    pub violation: Option<ForceViolation>,
    pub failure: Option<&'static str>,
    pub inapplicable_reason: Option<&'static str>,
}

#[derive(Debug)]
pub struct PieceMeasurementContext {
    pub name: &'static str,
    pub circuit: Circuit,
    pub element_range: Range<f64>,
}

#[derive(Clone)]
struct MeasurementTrack {
    circuit: Circuit,
    element_start: f64,
    element_end: f64,
    context: &'static str,
}

struct ApproachSequence {
    name: &'static str,
    pieces: &'static [PieceId],
}

const APPROACH_SEQUENCES: &[ApproachSequence] = &[
    ApproachSequence {
        name: "HalfLoopUp then left half corkscrew",
        pieces: &[PieceId::HalfLoopUp, PieceId::HalfCorkscrewLeftDown],
    },
    ApproachSequence {
        name: "HalfLoopUp then right half corkscrew",
        pieces: &[PieceId::HalfLoopUp, PieceId::HalfCorkscrewRightDown],
    },
    ApproachSequence {
        name: "HalfLoopUp then left raven roll",
        pieces: &[PieceId::HalfLoopUp, PieceId::RavenRollLeftDown],
    },
    ApproachSequence {
        name: "HalfLoopUp then right raven roll",
        pieces: &[PieceId::HalfLoopUp, PieceId::RavenRollRightDown],
    },
    ApproachSequence {
        name: "Left half corkscrew then left half corkscrew",
        pieces: &[PieceId::HalfCorkscrewLeftUp, PieceId::HalfCorkscrewLeftDown],
    },
    ApproachSequence {
        name: "Right half corkscrew then right half corkscrew",
        pieces: &[
            PieceId::HalfCorkscrewRightUp,
            PieceId::HalfCorkscrewRightDown,
        ],
    },
    ApproachSequence {
        name: "Left raven roll then left half corkscrew",
        pieces: &[PieceId::RavenRollLeftUp, PieceId::HalfCorkscrewLeftDown],
    },
    ApproachSequence {
        name: "Right raven roll then right half corkscrew",
        pieces: &[PieceId::RavenRollRightUp, PieceId::HalfCorkscrewRightDown],
    },
    ApproachSequence {
        name: "Left half corkscrew then half loop down",
        pieces: &[PieceId::HalfCorkscrewLeftUp, PieceId::HalfLoopDown],
    },
    ApproachSequence {
        name: "Right half corkscrew then half loop down",
        pieces: &[PieceId::HalfCorkscrewRightUp, PieceId::HalfLoopDown],
    },
    ApproachSequence {
        name: "Left raven roll then half loop down",
        pieces: &[PieceId::RavenRollLeftUp, PieceId::HalfLoopDown],
    },
    ApproachSequence {
        name: "Right raven roll then half loop down",
        pieces: &[PieceId::RavenRollRightUp, PieceId::HalfLoopDown],
    },
    ApproachSequence {
        name: "Bank left then banked drop left",
        pieces: &[PieceId::BankLeft, PieceId::BankedDropLeft],
    },
    ApproachSequence {
        name: "Bank right then banked drop right",
        pieces: &[PieceId::BankRight, PieceId::BankedDropRight],
    },
    ApproachSequence {
        name: "Bank left then twisted drop left",
        pieces: &[PieceId::BankLeft, PieceId::TwistedDropLeft],
    },
    ApproachSequence {
        name: "Bank right then twisted drop right",
        pieces: &[PieceId::BankRight, PieceId::TwistedDropRight],
    },
    ApproachSequence {
        name: "Bank left then barrel drop left",
        pieces: &[PieceId::BankLeft, PieceId::BarrelDropLeft],
    },
    ApproachSequence {
        name: "Bank right then barrel drop right",
        pieces: &[PieceId::BankRight, PieceId::BarrelDropRight],
    },
    ApproachSequence {
        name: "Bank left then banked straight into left rollout",
        pieces: &[
            PieceId::BankLeft,
            PieceId::BankedStraightLeft,
            PieceId::RolloutLeft,
        ],
    },
    ApproachSequence {
        name: "Gentle up, bank left, banked straight, then left rollout",
        pieces: &[
            PieceId::LevelToGentleUp,
            PieceId::BankLeft,
            PieceId::BankedStraightLeft,
            PieceId::RolloutLeft,
        ],
    },
    ApproachSequence {
        name: "Gentle down, bank left, banked straight, then left rollout",
        pieces: &[
            PieceId::LevelToGentleDown,
            PieceId::BankLeft,
            PieceId::BankedStraightLeft,
            PieceId::RolloutLeft,
        ],
    },
    ApproachSequence {
        name: "Bank right then banked straight into right rollout",
        pieces: &[
            PieceId::BankRight,
            PieceId::BankedStraightRight,
            PieceId::RolloutRight,
        ],
    },
    ApproachSequence {
        name: "Gentle up, bank right, banked straight, then right rollout",
        pieces: &[
            PieceId::LevelToGentleUp,
            PieceId::BankRight,
            PieceId::BankedStraightRight,
            PieceId::RolloutRight,
        ],
    },
    ApproachSequence {
        name: "Gentle down, bank right, banked straight, then right rollout",
        pieces: &[
            PieceId::LevelToGentleDown,
            PieceId::BankRight,
            PieceId::BankedStraightRight,
            PieceId::RolloutRight,
        ],
    },
    ApproachSequence {
        name: "Ascending and descending sixty-degree profile",
        pieces: &[
            PieceId::LevelToSixtyUp,
            PieceId::SixtyUpToLevel,
            PieceId::LevelToSixtyDown,
            PieceId::SixtyDownToLevel,
        ],
    },
    ApproachSequence {
        name: "Gentle, steep, and sixty-degree profile",
        pieces: &[
            PieceId::LevelToGentleUp,
            PieceId::GentleToSteepUp,
            PieceId::SteepToSixtyUp,
            PieceId::SixtyToSteepUp,
            PieceId::SteepToGentleUp,
            PieceId::GentleUpToLevel,
            PieceId::LevelToGentleDown,
            PieceId::GentleToSteepDown,
            PieceId::SteepToSixtyDown,
            PieceId::SixtyToSteepDown,
            PieceId::SteepToGentleDown,
            PieceId::GentleDownToLevel,
        ],
    },
];

/// Returns a connected track context using the same approach as the rated-window physics tests.
pub fn connected_measurement_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
) -> (Circuit, Range<f64>) {
    let track = connected_track(model, cars, piece, speed);
    (track.circuit, track.element_start..track.element_end)
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

fn established_bank_straight_id(piece_id: PieceId) -> Option<PieceId> {
    match piece_id {
        PieceId::BankedStraightLeft | PieceId::UnbankLeft | PieceId::RolloutLeft => {
            Some(PieceId::BankedStraightLeft)
        }
        PieceId::BankedStraightRight | PieceId::UnbankRight | PieceId::RolloutRight => {
            Some(PieceId::BankedStraightRight)
        }
        _ => None,
    }
}

fn complete_measurement_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
    pieces: Vec<Piece>,
    state: TrackState,
    context: &'static str,
) -> MeasurementTrack {
    let (mut pieces, mut state) = ensure_measurement_runway(model, cars, speed, pieces, state);
    assert!(same_connector(state, piece.entry));
    let element_start = pieces.iter().map(Piece::length).sum();
    let element = piece.at(state).unwrap();
    state = element.exit;
    let element_end = element_start + element.length();
    pieces.push(element);

    let exit_straight = straight_for(state);
    let mut exit_length = 0.0;
    let required_exit_length = scaled_train(model, cars).length_metres + 1.5 * speed + 40.0;
    while exit_length < required_exit_length {
        let next = exit_straight.at(state).unwrap();
        assert!(
            model.allows_piece(&next),
            "{} does not allow {:?} in its measurement exit",
            model.key,
            next.id
        );
        exit_length += next.length();
        state = next.exit;
        pieces.push(next);
    }

    MeasurementTrack {
        circuit: Circuit::open(pieces).expect("measurement approach forms an open circuit"),
        element_start,
        element_end,
        context,
    }
}

fn settling_distance(speed: f64) -> f64 {
    10.0_f64.max(0.5 * speed)
}

fn ensure_measurement_runway(
    model: &CoasterModel,
    cars: u8,
    speed: f64,
    original_pieces: Vec<Piece>,
    expected_state: TrackState,
) -> (Vec<Piece>, TrackState) {
    let required_length = scaled_train(model, cars).length_metres + settling_distance(speed);
    let existing_length: f64 = original_pieces.iter().map(Piece::length).sum();
    if existing_length >= required_length {
        return (original_pieces, expected_state);
    }

    let templates = unmeasured_catalog();
    let station = original_pieces
        .first()
        .filter(|piece| piece.id == PieceId::Station)
        .expect("measurement track begins at its station")
        .clone();
    let suffix = &original_pieces[1..];
    let suffix_length: f64 = suffix.iter().map(Piece::length).sum();
    let straight = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let mut pieces = vec![station.clone()];
    let mut state = station.exit;
    let mut total_length = station.length() + suffix_length;
    while total_length < required_length {
        let next = straight.at(state).unwrap();
        assert!(
            model.allows_piece(&next),
            "{} does not allow Straight in its measurement run-up",
            model.key
        );
        total_length += next.length();
        state = next.exit;
        pieces.push(next);
    }
    for original_piece in suffix {
        let next = original_piece.at(state).unwrap_or_else(|| {
            panic!(
                "cannot reconnect {:?} after adding measurement run-up at {state:?}",
                original_piece.id
            )
        });
        state = next.exit;
        pieces.push(next);
    }
    assert!(same_connector(state, expected_state));
    (pieces, state)
}

fn connected_track(model: &CoasterModel, cars: u8, piece: &Piece, speed: f64) -> MeasurementTrack {
    assert!(speed.is_finite() && speed > 0.0);
    assert!(model.cars_per_train.contains(&cars));

    let templates = unmeasured_catalog();
    let station = templates
        .iter()
        .find(|template| template.id == PieceId::Station)
        .unwrap()
        .clone();
    assert!(
        model.allows_piece(&station),
        "{} does not allow Station",
        model.key
    );
    let mut pieces = vec![station.clone()];
    let mut state = station.exit;
    for _ in 0..2 {
        let straight = templates
            .iter()
            .find(|template| template.id == PieceId::Straight)
            .unwrap()
            .at(state)
            .unwrap();
        assert!(
            model.allows_piece(&straight),
            "{} does not allow {:?} in its measurement approach",
            model.key,
            straight.id
        );
        state = straight.exit;
        pieces.push(straight);
    }

    let mut queue = VecDeque::from([(state, Vec::<usize>::new())]);
    let mut seen = HashSet::from([connector_key(state)]);
    let path = loop {
        let (current, path) = queue.pop_front().expect("no connected approach to piece");
        if same_connector(current, piece.entry) {
            break path;
        }
        for (index, template) in templates.iter().enumerate() {
            if model.allows_piece(template)
                && let Some(next) = template.exit_at(current)
                && seen.insert(connector_key(next))
            {
                let mut next_path = path.clone();
                next_path.push(index);
                queue.push_back((next, next_path));
            }
        }
    };
    for index in path {
        let next = templates[index].at(state).unwrap();
        assert!(
            model.allows_piece(&next),
            "{} does not allow {:?} in its measurement approach",
            model.key,
            next.id
        );
        state = next.exit;
        pieces.push(next);
    }

    assert!(
        same_connector(state, piece.entry),
        "connected approach for {:?} ends at {state:?}, expected {:?}",
        piece.id,
        piece.entry
    );

    if let Some(approach_id) = established_bank_straight_id(piece.id) {
        let approach_template = templates
            .iter()
            .find(|template| {
                template.id == approach_id
                    && template.entry.slope == piece.entry.slope
                    && template.exit.slope == piece.entry.slope
            })
            .unwrap_or_else(|| {
                panic!(
                    "no {approach_id:?} approach for {:?} at {:?}",
                    piece.id, piece.entry.slope
                )
            });
        let train_length = scaled_train(model, cars).length_metres;
        let required_approach_length = train_length + 10.0_f64.max(0.5 * speed);
        let mut approach_length = 0.0;
        while approach_length < required_approach_length {
            let approach_piece = approach_template.at(state).unwrap_or_else(|| {
                panic!(
                    "{approach_id:?} does not connect to the approach for {:?} at {state:?}",
                    piece.id
                )
            });
            assert!(
                model.allows_piece(&approach_piece),
                "{} does not allow {:?} in its established-bank measurement approach",
                model.key,
                approach_piece.id
            );
            approach_length += approach_piece.length();
            state = approach_piece.exit;
            pieces.push(approach_piece);
        }
        assert!(
            same_connector(state, piece.entry),
            "established-bank approach for {:?} ends at {state:?}, expected {:?}",
            piece.id,
            piece.entry
        );
    }

    complete_measurement_track(
        model,
        cars,
        piece,
        speed,
        pieces,
        state,
        "connected model-allowed approach",
    )
}

fn entry_straight_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
    connected: &MeasurementTrack,
) -> Option<MeasurementTrack> {
    let element_index = connected.circuit.piece_index(connected.element_start);
    let mut pieces = connected.circuit.pieces()[..element_index].to_vec();
    let mut state = pieces.last().unwrap().exit;
    let straight = straight_for(state);
    if !model.allows_piece(&straight) {
        return None;
    }

    let train_length = scaled_train(model, cars).length_metres;
    let required_approach_length = train_length + 10.0_f64.max(0.5 * speed);
    let mut approach_length = 0.0;
    while approach_length < required_approach_length {
        let next = straight.at(state).unwrap();
        approach_length += next.length();
        state = next.exit;
        pieces.push(next);
    }

    Some(complete_measurement_track(
        model,
        cars,
        piece,
        speed,
        pieces,
        state,
        "entry-slope straight",
    ))
}

fn sequence_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
    sequence: &ApproachSequence,
    target_index: usize,
) -> Option<MeasurementTrack> {
    let templates = unmeasured_catalog();
    let station = templates
        .iter()
        .find(|template| template.id == PieceId::Station)?
        .clone();
    if !model.allows_piece(&station) {
        return None;
    }
    let mut pieces = vec![station.clone()];
    let mut state = station.exit;
    for _ in 0..2 {
        let straight = templates
            .iter()
            .find(|template| template.id == PieceId::Straight)?
            .at(state)?;
        if !model.allows_piece(&straight) {
            return None;
        }
        state = straight.exit;
        pieces.push(straight);
    }

    for &piece_id in &sequence.pieces[..target_index] {
        let previous = templates
            .iter()
            .find(|template| template.id == piece_id && template.exit_at(state).is_some())?
            .at(state)?;
        if !model.allows_piece(&previous) {
            return None;
        }
        state = previous.exit;
        pieces.push(previous);
    }
    if !same_connector(state, piece.entry) {
        return None;
    }

    Some(complete_measurement_track(
        model,
        cars,
        piece,
        speed,
        pieces,
        state,
        sequence.name,
    ))
}

fn measurement_tracks(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
) -> Vec<MeasurementTrack> {
    assert!(speed.is_finite() && speed > 0.0);
    assert!(model.cars_per_train.contains(&cars));
    assert!(model.allows_piece(piece));

    let connected = connected_track(model, cars, piece, speed);
    let mut tracks = vec![connected];
    if let Some(entry_straight) = entry_straight_track(model, cars, piece, speed, &tracks[0]) {
        tracks.push(entry_straight);
    }
    for sequence in APPROACH_SEQUENCES {
        for (target_index, &target_id) in sequence.pieces.iter().enumerate() {
            if target_id == piece.id
                && target_index > 0
                && let Some(track) =
                    sequence_track(model, cars, piece, speed, sequence, target_index)
            {
                tracks.push(track);
            }
        }
    }
    tracks
}

/// Returns every shared approach context used to measure a rated piece.
pub fn measurement_contexts(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    speed: f64,
) -> Vec<PieceMeasurementContext> {
    measurement_tracks(model, cars, piece, speed)
        .into_iter()
        .map(|track| PieceMeasurementContext {
            name: track.context,
            circuit: track.circuit,
            element_range: track.element_start..track.element_end,
        })
        .collect()
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
    let context_results = measurement_tracks(model, cars, piece, target)
        .into_iter()
        .map(|track| rate_piece_on_track(model, cars, piece, target, physics, track))
        .collect::<Result<Vec<_>, _>>()?;
    let applicable_results: Vec<_> = context_results
        .iter()
        .filter(|result| result.applicable)
        .collect();
    let limiting = applicable_results
        .iter()
        .find(|result| !result.passed)
        .copied()
        .or_else(|| applicable_results.first().copied())
        .unwrap_or(&context_results[0]);
    let mut minimum_filtered_force = [f64::INFINITY; 3];
    let mut maximum_filtered_force = [f64::NEG_INFINITY; 3];
    for result in &applicable_results {
        for axis in 0..3 {
            minimum_filtered_force[axis] =
                minimum_filtered_force[axis].min(result.minimum_filtered_force[axis]);
            maximum_filtered_force[axis] =
                maximum_filtered_force[axis].max(result.maximum_filtered_force[axis]);
        }
    }
    let no_applicable_context = applicable_results.is_empty();
    Ok(PieceRating {
        passed: !no_applicable_context && applicable_results.iter().all(|result| result.passed),
        entry_speed: context_results[0].entry_speed,
        minimum_speed: applicable_results
            .iter()
            .map(|result| result.minimum_speed)
            .fold(f64::INFINITY, f64::min),
        minimum_filtered_vertical: applicable_results
            .iter()
            .map(|result| result.minimum_filtered_vertical)
            .fold(f64::INFINITY, f64::min),
        minimum_filtered_force,
        maximum_filtered_force,
        violation: if no_applicable_context {
            None
        } else {
            limiting.violation.clone()
        },
        failure: if no_applicable_context {
            Some("no approach context can attain the requested entry speed")
        } else {
            limiting.failure
        },
        limiting_context: limiting.context,
        context_results,
    })
}

fn rate_piece_on_track(
    model: &CoasterModel,
    cars: u8,
    piece: &Piece,
    target: f64,
    physics: PhysicsConfig,
    track: MeasurementTrack,
) -> Result<PieceContextRating, String> {
    assert!(target.is_finite() && target > 0.0);
    assert!(
        physics.rolling_resistance == 0.0
            && physics.drag_coefficient == 0.0
            && !physics.powered_sections,
        "rating requires unpowered frictionless physics"
    );
    assert!(model.cars_per_train.contains(&cars));
    let config = scaled_train(model, cars);
    let approach_start = track.element_start - settling_distance(target);
    assert!(
        approach_start >= config.length_metres,
        "{} approach does not leave the full train on the measurement track before pre-roll",
        track.context
    );
    let initial_speed = match solve_approach_speed_inner(
        &track.circuit,
        &config,
        physics,
        approach_start,
        track.element_start,
        target,
    ) {
        Ok(speed) => speed,
        Err(ApproachSpeedError::Unreachable(_)) => {
            return Ok(inapplicable_context_rating(track.context, target));
        }
        Err(ApproachSpeedError::Convergence(error)) => {
            return Err(format!("{}: {error}", track.context));
        }
    };
    let mut evaluators =
        vec![ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP); usize::from(cars)];
    let mut train = Train::new(
        &track.circuit,
        &config,
        physics,
        approach_start,
        initial_speed,
    );
    let mut actual_entry_speed = None;
    let mut first_lead_filtered_force = None;
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
    let lead_clear_distance = track.element_end + config.length_metres;
    for _ in 0..100_000 {
        let before_distance = train.distance();
        let before_speed = train.speed();
        train.step(&track.circuit);
        if actual_entry_speed.is_none() && before_distance < track.element_start {
            let after_distance = train.distance();
            if after_distance >= track.element_start {
                let fraction =
                    (track.element_start - before_distance) / (after_distance - before_distance);
                actual_entry_speed = Some(before_speed + fraction * (train.speed() - before_speed));
            }
        }
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
                    if car == 0 && first_lead_filtered_force.is_none() {
                        first_lead_filtered_force = Some(result.filtered);
                    }
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
        if done.iter().all(|done| *done) && train.distance() >= lead_clear_distance {
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
    if (!done.iter().all(|done| *done) || train.distance() < lead_clear_distance)
        && failure.is_none()
    {
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
    let entry_speed = actual_entry_speed.unwrap_or(target);
    if failure.is_none() {
        assert!(
            (entry_speed - target).abs() <= 1e-6,
            "{} reached {:.9} m/s at the element entry for a {:.9} m/s target",
            track.context,
            entry_speed,
            target
        );
    }
    Ok(PieceContextRating {
        context: track.context,
        applicable: true,
        passed: failure.is_none() && violation.is_none(),
        entry_speed,
        minimum_speed,
        minimum_filtered_vertical,
        minimum_filtered_force,
        maximum_filtered_force,
        first_lead_filtered_force,
        violation,
        failure,
        inapplicable_reason: None,
    })
}

fn inapplicable_context_rating(context: &'static str, target: f64) -> PieceContextRating {
    PieceContextRating {
        context,
        applicable: false,
        passed: false,
        entry_speed: target,
        minimum_speed: f64::INFINITY,
        minimum_filtered_vertical: f64::INFINITY,
        minimum_filtered_force: [f64::INFINITY; 3],
        maximum_filtered_force: [f64::NEG_INFINITY; 3],
        first_lead_filtered_force: None,
        violation: None,
        failure: None,
        inapplicable_reason: Some(
            "requested entry speed cannot be reached along this unpowered approach",
        ),
    }
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
fn average_train_height(
    circuit: &Circuit,
    configuration: &TrainConfiguration,
    distance: f64,
) -> f64 {
    let cars = usize::from(configuration.cars);
    let spacing = configuration.length_metres / cars as f64;
    (0..cars)
        .map(|car| circuit.sample(distance - car as f64 * spacing).position.z)
        .sum::<f64>()
        / cars as f64
}

fn required_start_speed_squared(
    circuit: &Circuit,
    configuration: &TrainConfiguration,
    approach_start: f64,
    target_distance: f64,
    target_speed: f64,
) -> f64 {
    target_speed.powi(2)
        + 2.0
            * super::GRAVITY
            * (average_train_height(circuit, configuration, target_distance)
                - average_train_height(circuit, configuration, approach_start))
}

#[derive(Debug)]
enum ApproachSpeedError {
    Unreachable(String),
    Convergence(String),
}

impl ApproachSpeedError {
    fn message(self) -> String {
        match self {
            Self::Unreachable(message) | Self::Convergence(message) => message,
        }
    }
}

pub fn solve_approach_speed(
    circuit: &Circuit,
    configuration: &TrainConfiguration,
    physics: PhysicsConfig,
    approach_start: f64,
    target_distance: f64,
    target_speed: f64,
) -> Result<f64, String> {
    solve_approach_speed_inner(
        circuit,
        configuration,
        physics,
        approach_start,
        target_distance,
        target_speed,
    )
    .map_err(ApproachSpeedError::message)
}

fn solve_approach_speed_inner(
    circuit: &Circuit,
    configuration: &TrainConfiguration,
    physics: PhysicsConfig,
    approach_start: f64,
    target_distance: f64,
    target_speed: f64,
) -> Result<f64, ApproachSpeedError> {
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
    let start_squared = required_start_speed_squared(
        circuit,
        configuration,
        approach_start,
        target_distance,
        target_speed,
    );
    if start_squared <= 0.0 {
        return Err(ApproachSpeedError::Unreachable(format!(
            "target {target_speed} m/s is unreachable from {approach_start} m"
        )));
    }
    let speed_at_boundary = |start_squared: f64| -> Result<f64, ApproachSpeedError> {
        if start_squared < 0.0 {
            return Err(ApproachSpeedError::Convergence(format!(
                "negative proposed squared approach speed {start_squared}"
            )));
        }
        let mut train = Train::new(
            circuit,
            configuration,
            physics,
            approach_start,
            start_squared.sqrt(),
        );
        const MAX_SIMULATION_STEPS: usize = 100_000;
        for _ in 0..MAX_SIMULATION_STEPS {
            let before_distance = train.distance();
            let before_speed = train.speed();
            train.step_block_brake_without_readings(circuit, &[], false);
            if train.distance() >= target_distance {
                let fraction =
                    (target_distance - before_distance) / (train.distance() - before_distance);
                return Ok(before_speed + fraction * (train.speed() - before_speed));
            }
            if train.speed() <= 0.0 || !train.notes().is_empty() {
                return Err(ApproachSpeedError::Unreachable(
                    "train stalled before target boundary".into(),
                ));
            }
        }
        Err(ApproachSpeedError::Convergence(format!(
            "target boundary was not reached after {MAX_SIMULATION_STEPS} steps: start speed {:.9} m/s, distance {:.9} m of {:.9} m, current speed {:.12} m/s",
            start_squared.sqrt(),
            train.distance() - approach_start,
            target_distance - approach_start,
            train.speed()
        )))
    };
    let initial_error = speed_at_boundary(start_squared)? - target_speed;
    if initial_error.abs() <= 1e-6 {
        return Ok(start_squared.sqrt());
    }

    let mut bracket = None;
    let mut lower_endpoint_is_unreachable = false;
    let mut endpoint_squared = start_squared;
    let mut endpoint_error = initial_error;
    let mut step_squared = (2.0 * target_speed * initial_error.abs())
        .max(start_squared * 1e-8)
        .max(1e-12);
    for _ in 0..64 {
        let candidate_squared = if initial_error > 0.0 {
            (endpoint_squared - step_squared).max(0.0)
        } else {
            endpoint_squared + step_squared
        };
        if !candidate_squared.is_finite() || candidate_squared == endpoint_squared {
            break;
        }
        let candidate_speed = match speed_at_boundary(candidate_squared) {
            Ok(speed) => speed,
            Err(ApproachSpeedError::Unreachable(_)) if initial_error > 0.0 => {
                bracket = Some((
                    candidate_squared,
                    -target_speed,
                    endpoint_squared,
                    endpoint_error,
                ));
                lower_endpoint_is_unreachable = true;
                break;
            }
            Err(ApproachSpeedError::Unreachable(reason)) => {
                return Err(ApproachSpeedError::Convergence(format!(
                    "a faster approach trial became unreachable after a slower trial reached the target: {reason}"
                )));
            }
            Err(error @ ApproachSpeedError::Convergence(_)) => return Err(error),
        };
        let candidate_error = candidate_speed - target_speed;
        if candidate_error.abs() <= 1e-6 {
            return Ok(candidate_squared.sqrt());
        }
        if initial_error > 0.0 {
            if candidate_error < 0.0 {
                bracket = Some((
                    candidate_squared,
                    candidate_error,
                    endpoint_squared,
                    endpoint_error,
                ));
                break;
            }
            if candidate_squared == 0.0 {
                return Err(ApproachSpeedError::Unreachable(format!(
                    "target {target_speed} m/s is below the {:.9} m/s minimum from rest",
                    candidate_speed
                )));
            }
        } else if candidate_error > 0.0 {
            bracket = Some((
                endpoint_squared,
                endpoint_error,
                candidate_squared,
                candidate_error,
            ));
            break;
        }
        endpoint_squared = candidate_squared;
        endpoint_error = candidate_error;
        step_squared *= 2.0;
    }
    let (mut low_squared, mut low_error, mut high_squared, mut high_error) =
        bracket.ok_or_else(|| {
            ApproachSpeedError::Convergence(format!(
                "approach speed solver could not bracket target {target_speed} m/s around energy estimate {:.9} m/s after 64 bounded expansions",
                start_squared.sqrt()
            ))
        })?;

    for iteration in 0..64 {
        let width = high_squared - low_squared;
        let secant_squared = low_squared - low_error * width / (high_error - low_error);
        let secant_fraction = (secant_squared - low_squared) / width;
        let candidate_squared = if iteration % 2 == 0
            && secant_squared.is_finite()
            && (0.1..=0.9).contains(&secant_fraction)
        {
            secant_squared
        } else {
            (low_squared + high_squared) * 0.5
        };
        let candidate_speed = match speed_at_boundary(candidate_squared) {
            Ok(speed) => speed,
            Err(ApproachSpeedError::Unreachable(_)) if lower_endpoint_is_unreachable => {
                low_squared = candidate_squared;
                low_error = -target_speed;
                continue;
            }
            Err(ApproachSpeedError::Unreachable(reason)) => {
                return Err(ApproachSpeedError::Convergence(format!(
                    "approach trial inside a bracket with a reachable lower endpoint became unreachable: {reason}"
                )));
            }
            Err(error @ ApproachSpeedError::Convergence(_)) => return Err(error),
        };
        let candidate_error = candidate_speed - target_speed;
        if candidate_error.abs() <= 1e-6 {
            return Ok(candidate_squared.sqrt());
        }
        if candidate_error < 0.0 {
            low_squared = candidate_squared;
            low_error = candidate_error;
        } else {
            high_squared = candidate_squared;
            high_error = candidate_error;
        }
    }
    Err(ApproachSpeedError::Convergence(format!(
        "approach speed solver failed: target {target_speed}, bracket error {:.9} to {:.9}",
        low_error, high_error
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{Bank, PieceId, Slope, catalog, coaster_models, hybrid_model, rated_speeds};

    fn sequence_start(track: &MeasurementTrack) -> f64 {
        let element_index = track
            .circuit
            .starts
            .iter()
            .position(|start| *start == track.element_start)
            .unwrap();
        track.circuit.starts[element_index - 1]
    }

    #[test]
    fn raven_roll_window_ends_before_the_entry_slope_context_fails() {
        let model = hybrid_model();
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::RavenRollLeftDown)
            .unwrap();
        let maximum_entry = rated_speeds(&model, piece)
            .expect("RavenRollLeftDown has a generated rated window")
            .maximum_entry;
        let rating =
            rate_piece(&model, model.train.cars, piece, maximum_entry, unpowered()).unwrap();
        let above_maximum = rate_piece(
            &model,
            model.train.cars,
            piece,
            maximum_entry + 0.01,
            unpowered(),
        )
        .unwrap();

        assert_eq!(model.train.cars, 6);
        assert!(rating.passed, "{rating:?}");
        assert!(rating.violation.is_none(), "{rating:?}");
        assert!(!above_maximum.passed, "{above_maximum:?}");
        assert_eq!(above_maximum.limiting_context, "entry-slope straight");
        assert_eq!(above_maximum.violation.as_ref().unwrap().axis, 1);
    }

    #[test]
    fn half_loop_lead_filter_keeps_history_from_the_preceding_raven_roll() {
        let model = hybrid_model();
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HalfLoopDown)
            .unwrap();
        let target_speed = 20.47;
        let context = measurement_tracks(&model, model.train.cars, piece, target_speed)
            .into_iter()
            .find(|context| context.context == "Left raven roll then half loop down")
            .expect("HalfLoopDown has the encoded preceding-raven-roll context");
        let warmed = rate_piece_on_track(
            &model,
            model.train.cars,
            piece,
            target_speed,
            unpowered(),
            context.clone(),
        )
        .unwrap();
        assert!(warmed.applicable, "{warmed:?}");
        let warmed_lead_force = warmed
            .first_lead_filtered_force
            .expect("the lead car enters the half loop");

        let mut train = Train::new(
            &context.circuit,
            &model.train,
            unpowered(),
            context.element_start,
            target_speed,
        );
        let mut cold_filter = ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP);
        let cold_lead_force = loop {
            train.step(&context.circuit);
            if let Some(reading) = train
                .take_readings()
                .into_iter()
                .flatten()
                .find(|reading| reading.distance >= context.element_start)
            {
                break cold_filter.update(reading.g).filtered;
            }
        };
        let difference = (warmed_lead_force.vertical - cold_lead_force.vertical)
            .abs()
            .max((warmed_lead_force.lateral - cold_lead_force.lateral).abs())
            .max((warmed_lead_force.longitudinal - cold_lead_force.longitudinal).abs());
        assert!(
            difference > 0.05,
            "preceding RavenRollLeftUp should change the lead car's first filtered sample: warmed={warmed_lead_force:?}, cold={cold_lead_force:?}"
        );
    }

    #[test]
    fn approach_speed_solver_hits_the_daisy_dash_level_bank_left_boundary() {
        let model = coaster_models()
            .into_iter()
            .find(|model| model.key == "family_coaster")
            .unwrap();
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| {
                piece.id == PieceId::BankLeft
                    && piece.entry.slope == Slope::Level
                    && model.allows_piece(piece)
            })
            .unwrap();
        let target_speed = 5.408_721_923_828_125;
        let context = measurement_tracks(&model, model.train.cars, piece, target_speed)
            .into_iter()
            .find(|context| context.context == "connected model-allowed approach")
            .unwrap();
        let rating = rate_piece_on_track(
            &model,
            model.train.cars,
            piece,
            target_speed,
            unpowered(),
            context,
        )
        .unwrap();

        assert!(rating.applicable, "{rating:?}");
        assert!(
            (rating.entry_speed - target_speed).abs() <= 1e-6,
            "{rating:?}"
        );
    }

    #[test]
    fn uphill_bank_left_rating_rejects_a_context_that_cannot_clear_the_full_train() {
        let model = hybrid_model();
        let piece = unmeasured_catalog()
            .iter()
            .find(|piece| {
                piece.id == PieceId::BankLeft
                    && piece.entry.slope == Slope::UpGentle
                    && model.allows_piece(piece)
            })
            .unwrap();
        let target_speed = 17.71;
        let rating =
            rate_piece(&model, model.train.cars, piece, target_speed, unpowered()).unwrap();
        assert!(!rating.passed, "{rating:?}");
        let entry_slope_straight = rating
            .context_results
            .iter()
            .find(|context| context.context == "entry-slope straight")
            .unwrap();
        assert!(entry_slope_straight.applicable, "{entry_slope_straight:?}");
        assert_eq!(
            entry_slope_straight.failure,
            Some("stalled or rolled back"),
            "{entry_slope_straight:?}"
        );
    }

    #[test]
    fn bank_left_down_gentle_boundary_is_solved_or_marked_inapplicable() {
        for (model_key, target_speed, expected_applicable) in [
            ("velocity_viper", 5.244_726_181_030_273, false),
            ("hybrid", 5.244_726_181_030_273, false),
            ("family_coaster", 5.408_721_923_828_125, true),
        ] {
            let model = coaster_models()
                .into_iter()
                .find(|model| model.key == model_key)
                .unwrap();
            let piece = unmeasured_catalog()
                .iter()
                .find(|piece| {
                    piece.id == PieceId::BankLeft
                        && piece.entry.slope == Slope::DownGentle
                        && model.allows_piece(piece)
                })
                .unwrap_or_else(|| panic!("{model_key} must allow a gentle-down BankLeft"));
            let context = measurement_tracks(&model, model.train.cars, piece, target_speed)
                .into_iter()
                .find(|context| context.context == "connected model-allowed approach")
                .unwrap();
            let rating = rate_piece_on_track(
                &model,
                model.train.cars,
                piece,
                target_speed,
                unpowered(),
                context,
            )
            .unwrap();

            assert_eq!(
                rating.applicable, expected_applicable,
                "{model_key}: {rating:?}"
            );
            if expected_applicable {
                assert!(
                    (rating.entry_speed - target_speed).abs() <= 1e-6,
                    "{model_key}: {rating:?}"
                );
            } else {
                assert!(
                    rating.inapplicable_reason.is_some(),
                    "{model_key}: {rating:?}"
                );
            }
            assert!(rating.failure.is_none(), "{model_key}: {rating:?}");
        }
    }

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
    fn energy_unreachable_downhill_speed_is_explicitly_inapplicable() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::SixtyToSteepDown)
            .unwrap();
        let rating = rate_piece(&model, model.train.cars, piece, 1.0, unpowered()).unwrap();
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
        let speed = piece.design_speeds.unwrap().maximum_entry + 1.0;
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
    fn connected_measurement_matches_real_train_after_shared_preroll() {
        let model = hybrid_model();
        let cars = 8;
        for id in [
            PieceId::EjectorHill12x64,
            PieceId::EjectorHill12x80,
            PieceId::EjectorHill16x96,
            PieceId::LevelToSixtyDown,
        ] {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let target = piece.design_speeds.unwrap().maximum_entry;
            let context = measurement_tracks(&model, cars, piece, target)
                .into_iter()
                .find(|context| context.context == "connected model-allowed approach")
                .expect("the shared contexts include the connected model-allowed approach");
            let rating =
                rate_piece_on_track(&model, cars, piece, target, unpowered(), context.clone())
                    .unwrap();
            let config = scaled_train(&model, cars);
            let approach_start = context.element_start - settling_distance(target);
            let approach_speed = solve_approach_speed(
                &context.circuit,
                &config,
                unpowered(),
                approach_start,
                context.element_start,
                target,
            )
            .unwrap();
            let mut train = Train::new(
                &context.circuit,
                &config,
                unpowered(),
                approach_start,
                approach_speed,
            );
            let mut evaluators = vec![
                ForceEvaluator::new(model.comfortable_force_limits, TIME_STEP);
                usize::from(cars)
            ];
            let mut minimum_vertical = f64::INFINITY;
            let mut first_violation = None;
            let mut cleared_element = false;
            for _ in 0..20_000 {
                train.step(&context.circuit);
                for frame in train.take_readings() {
                    for (car, reading) in frame.into_iter().enumerate() {
                        let evaluation = evaluators[car].update(reading.g);
                        if (context.element_start..context.element_end).contains(&reading.distance)
                        {
                            minimum_vertical = minimum_vertical.min(evaluation.filtered.vertical);
                        }
                        if reading.distance >= context.element_start && first_violation.is_none() {
                            for axis in 0..3 {
                                if !evaluation.axis_within_limits[axis] {
                                    first_violation = Some((car, axis));
                                    break;
                                }
                            }
                        }
                    }
                }
                if train.distance() >= context.element_end + config.length_metres + target {
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
                "{id:?}: real={minimum_vertical}, rated={}",
                rating.minimum_filtered_vertical
            );
        }
    }

    #[test]
    fn every_rated_piece_has_a_connected_straight_approach_and_exit() {
        let model = hybrid_model();
        for piece in catalog()
            .iter()
            .filter(|piece| piece.design_speeds.is_some())
        {
            let track = connected_track(&model, model.train.cars, piece, 25.0);
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
        let track = connected_track(&model, model.train.cars, piece, 40.0);
        let mut train = model.train.clone();
        train.cars = 12;
        for (speed, passes) in [(25.0, true), (40.0, false)] {
            let rating = rate_sequence(
                &track.circuit,
                &[track.element_start..track.element_end],
                &train,
                unpowered(),
                model.comfortable_force_limits,
                sequence_start(&track),
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
    fn approach_speed_solver_refines_high_speed_downhill_energy_estimates() {
        for (model_key, piece_id, target_speed) in [
            ("hyper", PieceId::SixtyDownToLevel, 66.0),
            ("hybrid", PieceId::SixtyDownToLevel, 84.25),
            ("velocity_viper", PieceId::SixtyDownToLevel, 84.25),
            ("hyper", PieceId::WideSixtyDownToLevel, 66.0),
            ("hybrid", PieceId::SixtyToSteepDown, 84.25),
            ("hyper", PieceId::SixtyToSteepDown, 66.0),
            ("velocity_viper", PieceId::SixtyToSteepDown, 84.25),
        ] {
            let model = coaster_models()
                .into_iter()
                .find(|model| model.key == model_key)
                .unwrap();
            let piece = catalog()
                .iter()
                .find(|piece| piece.id == piece_id && model.allows_piece(piece))
                .unwrap();
            let track = connected_track(&model, model.train.cars, piece, target_speed);
            let approach_start = track.element_start - settling_distance(target_speed);
            let solved_speed = solve_approach_speed(
                &track.circuit,
                &model.train,
                unpowered(),
                approach_start,
                track.element_start,
                target_speed,
            )
            .unwrap_or_else(|error| panic!("{model_key} {piece_id:?}: {error}"));
            let mut train = Train::new(
                &track.circuit,
                &model.train,
                unpowered(),
                approach_start,
                solved_speed,
            );
            let actual_entry_speed = loop {
                let before_distance = train.distance();
                let before_speed = train.speed();
                train.step_block_brake_without_readings(&track.circuit, &[], false);
                if train.distance() >= track.element_start {
                    let fraction = (track.element_start - before_distance)
                        / (train.distance() - before_distance);
                    break before_speed + fraction * (train.speed() - before_speed);
                }
                assert!(train.speed() > 0.0, "{model_key} {piece_id:?} stalled");
            };
            assert!(
                (actual_entry_speed - target_speed).abs() <= 1e-6,
                "{model_key} {piece_id:?}: solved entry was {actual_entry_speed}, target was {target_speed}"
            );
        }
    }

    #[test]
    fn sequence_rating_reports_violating_piece_after_two_straights() {
        let model = hybrid_model();
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let track = connected_track(&model, model.train.cars, piece, 40.0);
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
        let track = connected_track(&model, model.train.cars, piece, 25.0);
        let rating = rate_sequence(
            &track.circuit,
            &[track.element_start..track.element_end],
            &model.train,
            unpowered(),
            model.comfortable_force_limits,
            sequence_start(&track),
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
            let track = connected_track(&model, model.train.cars, piece, 25.0);
            let solved = solve_approach_speed(
                &track.circuit,
                &model.train,
                unpowered(),
                sequence_start(&track),
                track.element_start,
                25.0,
            )
            .unwrap();
            let mut train = Train::new(
                &track.circuit,
                &model.train,
                unpowered(),
                sequence_start(&track),
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
        let track = connected_track(&model, model.train.cars, piece, 25.0);
        let result = solve_approach_speed(
            &track.circuit,
            &model.train,
            unpowered(),
            sequence_start(&track),
            track.element_start,
            1.0,
        );
        assert!(result.unwrap_err().contains("unreachable"));
    }

    #[test]
    fn banked_pieces_are_measured_after_a_model_allowed_established_bank_approach() {
        let templates = unmeasured_catalog();
        let model_list = coaster_models();
        let target_ids = [
            (
                PieceId::BankedStraightLeft,
                PieceId::BankedStraightLeft,
                Bank::Left,
            ),
            (
                PieceId::BankedStraightRight,
                PieceId::BankedStraightRight,
                Bank::Right,
            ),
            (PieceId::UnbankLeft, PieceId::BankedStraightLeft, Bank::Left),
            (
                PieceId::UnbankRight,
                PieceId::BankedStraightRight,
                Bank::Right,
            ),
            (
                PieceId::RolloutLeft,
                PieceId::BankedStraightLeft,
                Bank::Left,
            ),
            (
                PieceId::RolloutRight,
                PieceId::BankedStraightRight,
                Bank::Right,
            ),
        ];

        for (target_id, approach_id, bank) in target_ids {
            for target in templates.iter().filter(|piece| piece.id == target_id) {
                for model in model_list.iter().filter(|model| model.allows_piece(target)) {
                    let speed = target
                        .design_speeds
                        .map_or(20.0, |speeds| speeds.minimum_entry.max(20.0));
                    let track = connected_track(model, model.train.cars, target, speed);
                    assert!(
                        track
                            .circuit
                            .pieces
                            .iter()
                            .all(|piece| model.allows_piece(piece)),
                        "{} approach to {target_id:?} contains a disallowed piece",
                        model.key
                    );

                    let target_index = track
                        .circuit
                        .pieces
                        .iter()
                        .rposition(|piece| {
                            piece.id == target_id
                                && piece.entry.slope == target.entry.slope
                                && piece.exit.slope == target.exit.slope
                        })
                        .expect("measurement track contains its target piece");
                    let approach_pieces: Vec<_> = track.circuit.pieces[..target_index]
                        .iter()
                        .rev()
                        .take_while(|piece| {
                            piece.id == approach_id
                                && piece.entry.bank == bank
                                && piece.exit.bank == bank
                                && piece.entry.slope == target.entry.slope
                                && piece.exit.slope == target.entry.slope
                        })
                        .collect();
                    let approach_length: f64 =
                        approach_pieces.iter().map(|piece| piece.length()).sum();
                    let train_length = scaled_train(model, model.train.cars).length_metres;
                    let required_length = train_length + 10.0_f64.max(0.5 * speed);
                    assert!(
                        approach_length >= required_length,
                        "{} {target_id:?} {:?} has only {approach_length:.2} m of established bank before its target; need {required_length:.2} m",
                        model.key,
                        target.entry.slope
                    );
                    assert_eq!(target.entry.slope, target.exit.slope);
                    assert!(matches!(
                        target.entry.slope,
                        Slope::Level | Slope::UpGentle | Slope::DownGentle
                    ));
                }
            }
        }
    }

    #[test]
    fn bank_transitions_have_rated_catalog_windows_for_each_slope() {
        let templates = unmeasured_catalog();
        for id in [
            PieceId::BankLeft,
            PieceId::UnbankLeft,
            PieceId::BankRight,
            PieceId::UnbankRight,
            PieceId::RolloutLeft,
            PieceId::RolloutRight,
        ] {
            let variants: Vec<_> = templates.iter().filter(|piece| piece.id == id).collect();
            assert_eq!(
                variants.len(),
                3,
                "expected level and two gentle slopes for {id:?}"
            );
            for piece in variants {
                let speeds = piece
                    .design_speeds
                    .unwrap_or_else(|| panic!("{id:?} has no rated catalog window"));
                assert!(speeds.minimum_entry < speeds.maximum_entry);
                assert!(
                    piece.constant_speed_peak_roll_rate(speeds.maximum_entry)
                        <= Piece::MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND,
                    "{id:?} at {:?} exceeds the roll-rate limit at its catalog maximum",
                    piece.entry.slope
                );
            }
        }
    }
}
