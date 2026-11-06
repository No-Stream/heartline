use std::collections::{HashSet, VecDeque};

use theme_park_sim::physics::{Circuit, PhysicsConfig, Train};
use theme_park_sim::track::{
    AirtimeRating, CoasterModel, HillFeel, Piece, PieceFamily, PieceId, TrackState, catalog,
    hybrid_model, multi_launch_model, wooden_twister_model,
};

const RETURN_PIECES: [PieceId; 9] = [
    PieceId::Straight,
    PieceId::HalfTileStraight,
    PieceId::OneTileStraight,
    PieceId::TurnSmallStandalone,
    PieceId::TurnSmallRightStandalone,
    PieceId::TurnLargeStandalone,
    PieceId::TurnLargeRightStandalone,
    PieceId::OffsetLeft,
    PieceId::OffsetRight,
];

fn connector_key(state: TrackState) -> (i32, i32, i32, u8, u8, u8, u8, u8) {
    (
        state.x,
        state.y,
        state.z,
        state.heading as u8,
        state.slope as u8,
        state.bank as u8,
        state.horizontal_curvature as u8,
        state.vertical_curvature as u8,
    )
}

fn next_connector(piece: &Piece, state: TrackState) -> TrackState {
    assert_eq!(state.slope, piece.entry.slope);
    assert_eq!(state.bank, piece.entry.bank);
    assert_eq!(state.horizontal_curvature, piece.entry.horizontal_curvature);
    assert_eq!(state.vertical_curvature, piece.entry.vertical_curvature);

    let rotation = (state.heading as i32 - piece.entry.heading as i32).rem_euclid(8);
    let delta_x = piece.exit.x - piece.entry.x;
    let delta_y = piece.exit.y - piece.entry.y;
    let (rotated_x, rotated_y) = match rotation {
        0 => (delta_x, delta_y),
        1 => (delta_x - delta_y, delta_x + delta_y),
        2 => (-delta_y, delta_x),
        3 => (-delta_x - delta_y, delta_x - delta_y),
        4 => (-delta_x, -delta_y),
        5 => (delta_y - delta_x, -delta_x - delta_y),
        6 => (delta_y, -delta_x),
        7 => (delta_x + delta_y, delta_y - delta_x),
        _ => unreachable!(),
    };
    TrackState {
        x: state.x + rotated_x,
        y: state.y + rotated_y,
        z: state.z
            + if rotation % 2 == 0 {
                piece.exit.z - piece.entry.z
            } else {
                (f64::from(piece.exit.z - piece.entry.z) * std::f64::consts::SQRT_2).round() as i32
            },
        heading: match (piece.exit.heading as i32 + rotation).rem_euclid(8) {
            0 => theme_park_sim::track::Heading::East,
            1 => theme_park_sim::track::Heading::NorthEast,
            2 => theme_park_sim::track::Heading::North,
            3 => theme_park_sim::track::Heading::NorthWest,
            4 => theme_park_sim::track::Heading::West,
            5 => theme_park_sim::track::Heading::SouthWest,
            6 => theme_park_sim::track::Heading::South,
            7 => theme_park_sim::track::Heading::SouthEast,
            _ => unreachable!(),
        },
        slope: piece.exit.slope,
        bank: piece.exit.bank,
        horizontal_curvature: piece.exit.horizontal_curvature,
        vertical_curvature: piece.exit.vertical_curvature,
    }
}

fn level_return_path(start: TrackState, templates: &[Piece]) -> Vec<PieceId> {
    let margin = 64;
    let x_bounds = start.x.min(0) - margin..=start.x.max(0) + margin;
    let y_bounds = start.y.min(0) - margin..=start.y.max(0) + margin;
    let mut queue = VecDeque::from([(start, Vec::new())]);
    let mut visited = HashSet::from([connector_key(start)]);
    while let Some((state, path)) = queue.pop_front() {
        if state == TrackState::origin() {
            return path;
        }
        if path.len() == 48 {
            continue;
        }
        for id in RETURN_PIECES {
            let template = templates.iter().find(|piece| piece.id == id).unwrap();
            if state.slope != template.entry.slope
                || state.bank != template.entry.bank
                || state.horizontal_curvature != template.entry.horizontal_curvature
                || state.vertical_curvature != template.entry.vertical_curvature
            {
                continue;
            }
            let next = next_connector(template, state);
            if !x_bounds.contains(&next.x) || !y_bounds.contains(&next.y) || next.z != 0 {
                continue;
            }
            if !visited.insert(connector_key(next)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((next, next_path));
        }
    }
    panic!("no bounded level return path from {start:?}");
}

fn circuit_for_level_piece(piece_id: PieceId) -> (Circuit, f64, f64) {
    let templates = catalog();
    let template = |id| templates.iter().find(|piece| piece.id == id).unwrap();
    let element_template = template(piece_id);
    assert_eq!(
        element_template.entry,
        TrackState::origin(),
        "{piece_id:?} entry"
    );
    assert_eq!(
        element_template.exit.slope,
        theme_park_sim::track::Slope::Level
    );
    assert_eq!(
        element_template.exit.bank,
        theme_park_sim::track::Bank::Level
    );

    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight] {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let element_start = pieces.iter().map(Piece::length).sum();
    let element = element_template.at(state).unwrap();
    state = element.exit;
    pieces.push(element);
    let element_end = pieces.iter().map(Piece::length).sum();

    let mut return_path = level_return_path(state, templates);
    for id in return_path.drain(..) {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(state, TrackState::origin());
    (Circuit::new(pieces).unwrap(), element_start, element_end)
}

#[test]
fn rolled_inversion_minimum_speeds_clear_with_energy_margin() {
    use PieceId::*;
    for id in [
        ZeroGRollFastLeft,
        ZeroGRollFastRight,
        ZeroGRollCompactLeft,
        ZeroGRollCompactRight,
        StallLeft,
        StallRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated = piece.rated_speeds.expect("rolled inversion must be rated");
        let (circuit, start, end) = circuit_for_level_piece(id);
        for model in [hybrid_model(), multi_launch_model()] {
            for cars in [6, 12] {
                assert_train_crosses_element(
                    &model,
                    &circuit,
                    id,
                    start..end,
                    cars,
                    rated.minimum_entry,
                    true,
                );
            }
        }
    }
}

#[test]
fn level_elements_clear_from_their_minimum_after_a_steep_crest() {
    use PieceId::*;
    for piece in catalog().iter().filter(|piece| {
        piece.rated_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        let (circuit, ranges) = circuit_for_sequence(&[LevelToSixtyUp, SixtyUpToLevel, piece.id]);
        let (_, start, end) = ranges[2];
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            for cars in [6, 12] {
                assert_train_crosses_element(
                    &model,
                    &circuit,
                    piece.id,
                    start..end,
                    cars,
                    piece.rated_speeds.unwrap().minimum_entry,
                    false,
                );
            }
        }
    }
}

#[test]
fn half_inversion_windows_hold_after_preceding_inversion() {
    use PieceId::*;
    for sequence in [
        [HalfLoopUp, HalfCorkscrewLeftDown],
        [HalfLoopUp, HalfCorkscrewRightDown],
        [HalfLoopUp, RavenRollLeftDown],
        [HalfLoopUp, RavenRollRightDown],
        [HalfCorkscrewLeftUp, HalfLoopDown],
        [HalfCorkscrewRightUp, HalfLoopDown],
        [RavenRollLeftUp, HalfLoopDown],
        [RavenRollRightUp, HalfLoopDown],
    ] {
        let (circuit, ranges) = circuit_for_sequence(&sequence);
        let (id, start, end) = ranges[1];
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated = piece.rated_speeds.unwrap();
        for model in [hybrid_model(), multi_launch_model()] {
            for cars in [6, 12] {
                for speed in [rated.minimum_entry, rated.maximum_entry] {
                    assert_train_crosses_element(
                        &model,
                        &circuit,
                        id,
                        start..end,
                        cars,
                        speed,
                        false,
                    );
                }
            }
        }
    }
}

#[test]
fn sixty_degree_windows_hold_with_connected_approaches() {
    use PieceId::*;
    let sequences: [&[PieceId]; 2] = [
        &[
            LevelToSixtyUp,
            SixtyUpToLevel,
            LevelToSixtyDown,
            SixtyDownToLevel,
        ],
        &[
            LevelToGentleUp,
            GentleToSteepUp,
            SteepToSixtyUp,
            SixtyToSteepUp,
            SteepToGentleUp,
            GentleUpToLevel,
            LevelToGentleDown,
            GentleToSteepDown,
            SteepToSixtyDown,
            SixtyToSteepDown,
            SteepToGentleDown,
            GentleDownToLevel,
        ],
    ];
    for sequence in sequences {
        let (circuit, ranges) = circuit_for_sequence(sequence);
        for (id, start, end) in ranges {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let Some(rated) = piece.rated_speeds else {
                continue;
            };
            for model in [hybrid_model(), multi_launch_model()] {
                for cars in [6, 12] {
                    for speed in [rated.minimum_entry, rated.maximum_entry] {
                        assert_train_crosses_element(
                            &model,
                            &circuit,
                            id,
                            start..end,
                            cars,
                            speed,
                            false,
                        );
                    }
                }
            }
        }
    }
}

fn circuit_for_sequence(elements: &[PieceId]) -> (Circuit, Vec<(PieceId, f64, f64)>) {
    let templates = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight] {
        let piece = templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }

    let mut element_ranges = Vec::with_capacity(elements.len());
    for &id in elements {
        let element_start = pieces.iter().map(Piece::length).sum();
        let template = templates
            .iter()
            .find(|piece| {
                piece.id == id
                    && piece.entry.slope == state.slope
                    && piece.entry.bank == state.bank
                    && piece.entry.horizontal_curvature == state.horizontal_curvature
                    && piece.entry.vertical_curvature == state.vertical_curvature
            })
            .unwrap_or_else(|| panic!("no {id:?} template joins from {state:?}"));
        let element = template.at(state).unwrap();
        state = element.exit;
        pieces.push(element);
        element_ranges.push((id, element_start, pieces.iter().map(Piece::length).sum()));
    }
    while state.z != 0 {
        let id = if state.z > 0 {
            PieceId::ShortFall
        } else {
            PieceId::ShortRise
        };
        let piece = templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(
        (state.z, state.slope, state.bank),
        (
            0,
            theme_park_sim::track::Slope::Level,
            theme_park_sim::track::Bank::Level
        ),
        "sequence does not return to level height and bank: {elements:?}"
    );

    let return_path = level_return_path(state, templates);
    for id in return_path {
        let template = templates.iter().find(|piece| piece.id == id).unwrap();
        let piece = template.at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(state, TrackState::origin());
    (Circuit::new(pieces).unwrap(), element_ranges)
}

fn unpowered() -> PhysicsConfig {
    PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    }
}

#[derive(Clone, Copy, Debug)]
struct ElementForces {
    minimum_vertical: f64,
    maximum_vertical: f64,
    maximum_lateral: f64,
    maximum_longitudinal: f64,
    peak_lateral_distance: f64,
    peak_lateral_speed: f64,
}

#[derive(Clone, Copy, Debug)]
struct ElementScan {
    forces: ElementForces,
    all_cars_seen: bool,
    train_cleared: bool,
    stalled: bool,
    rolled_back: bool,
    minimum_speed: f64,
}

fn scan_train_element(
    model: &CoasterModel,
    circuit: &Circuit,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
) -> ElementScan {
    let mut train_config = model.train.clone();
    train_config.cars = cars;
    let mut train = Train::new(
        circuit,
        &train_config,
        unpowered(),
        element_range.start,
        entry_speed,
    );
    let mut cars_seen = vec![false; usize::from(cars)];
    let mut minimum_vertical = f64::INFINITY;
    let mut maximum_vertical = f64::NEG_INFINITY;
    let mut maximum_lateral: f64 = 0.0;
    let mut maximum_longitudinal: f64 = 0.0;
    let mut peak_lateral_distance = element_range.start;
    let mut peak_lateral_speed = entry_speed;
    let mut minimum_speed = f64::INFINITY;
    let mut stalled = false;
    for _ in 0..100_000 {
        train.step(circuit);
        for frame in train.take_readings() {
            for (index, reading) in frame.into_iter().enumerate() {
                if element_range.contains(&reading.distance) {
                    cars_seen[index] = true;
                    minimum_vertical = minimum_vertical.min(reading.g.vertical);
                    maximum_vertical = maximum_vertical.max(reading.g.vertical);
                    if reading.g.lateral.abs() > maximum_lateral {
                        maximum_lateral = reading.g.lateral.abs();
                        peak_lateral_distance = reading.distance;
                        peak_lateral_speed = reading.speed;
                    }
                    maximum_longitudinal = maximum_longitudinal.max(reading.g.longitudinal.abs());
                    minimum_speed = minimum_speed.min(reading.speed);
                }
            }
        }
        if train.distance() >= element_range.end + train_config.length_metres {
            break;
        }
        if train.speed() <= 0.0 {
            stalled = true;
            break;
        }
        if !train.notes().is_empty() {
            break;
        }
    }

    ElementScan {
        forces: ElementForces {
            minimum_vertical,
            maximum_vertical,
            maximum_lateral,
            maximum_longitudinal,
            peak_lateral_distance,
            peak_lateral_speed,
        },
        all_cars_seen: cars_seen.into_iter().all(|seen| seen),
        train_cleared: train.distance() >= element_range.end + train_config.length_metres,
        stalled,
        rolled_back: !train.notes().is_empty(),
        minimum_speed,
    }
}

fn assert_train_crosses_element(
    model: &CoasterModel,
    circuit: &Circuit,
    piece_id: PieceId,
    element_range: std::ops::Range<f64>,
    cars: u8,
    entry_speed: f64,
    minimum_speed_required: bool,
) -> ElementForces {
    let scan = scan_train_element(model, circuit, element_range, cars, entry_speed);
    let forces = scan.forces;
    let limits = model.comfortable_force_limits;
    assert!(
        scan.all_cars_seen,
        "{piece_id:?} {} {cars} cars at {entry_speed}: not every car entered",
        model.name
    );
    assert!(
        scan.train_cleared,
        "{piece_id:?} {} {cars} cars at {entry_speed}: train did not clear",
        model.name
    );
    assert!(
        !scan.rolled_back,
        "{piece_id:?} {} {cars} cars at {entry_speed}: rollback",
        model.name
    );
    assert!(
        !scan.stalled,
        "{piece_id:?} {} {cars} cars at {entry_speed}: stalled",
        model.name
    );
    assert!(
        forces.minimum_vertical >= limits.minimum_vertical_g
            && forces.maximum_vertical <= limits.maximum_vertical_g
            && forces.maximum_lateral <= limits.maximum_lateral_g
            && forces.maximum_longitudinal <= limits.maximum_longitudinal_g,
        "{piece_id:?} {} {cars} cars at {entry_speed}: vertical {}..{}, lateral {}, longitudinal {}; limits {limits:?}",
        model.name,
        forces.minimum_vertical,
        forces.maximum_vertical,
        forces.maximum_lateral,
        forces.maximum_longitudinal
    );
    if matches!(piece_id, PieceId::Corkscrew | PieceId::CorkscrewRight) {
        assert!(
            forces.maximum_lateral <= 1.0,
            "{piece_id:?} {} {cars} cars at {entry_speed}: lateral {} exceeds 1 g",
            model.name,
            forces.maximum_lateral
        );
    }
    if minimum_speed_required {
        assert!(
            scan.minimum_speed >= 5.0,
            "{piece_id:?}: minimum speed {}",
            scan.minimum_speed
        );
    }
    eprintln!(
        "{piece_id:?} {} {cars} cars at {entry_speed:.3}: vertical {:.3}..{:.3}, lateral {:.3}, longitudinal {:.3}",
        model.name,
        forces.minimum_vertical,
        forces.maximum_vertical,
        forces.maximum_lateral,
        forces.maximum_longitudinal
    );
    forces
}

fn peak_roll_rate_degrees_per_second(piece: &Piece, speed: f64) -> f64 {
    let sample_count = (piece.length() / 0.05).ceil() as usize;
    let mut previous_distance = 0.0;
    let mut previous_roll = piece.sample(0.0).roll;
    let mut peak_rate: f64 = 0.0;
    for index in 1..=sample_count {
        let distance = if index == sample_count {
            piece.length()
        } else {
            piece.length() * index as f64 / sample_count as f64
        };
        let roll = piece.sample(distance).roll;
        let roll_rate = (roll - previous_roll).abs() / (distance - previous_distance) * speed;
        peak_rate = peak_rate.max(roll_rate.to_degrees());
        previous_distance = distance;
        previous_roll = roll;
    }
    peak_rate
}

fn models() -> [CoasterModel; 3] {
    [wooden_twister_model(), hybrid_model(), multi_launch_model()]
}

#[test]
fn every_buildable_inversion_drop_and_sixty_transition_has_a_rated_window() {
    use PieceId::*;

    let sixty_transitions = [
        LevelToSixtyUp,
        SixtyUpToLevel,
        LevelToSixtyDown,
        SixtyDownToLevel,
        SteepToSixtyUp,
        SixtyToSteepUp,
        SteepToSixtyDown,
        SixtyToSteepDown,
    ];
    let drops = [
        BankedDropLeft,
        BankedDropRight,
        TwistedDropLeft,
        TwistedDropRight,
        BarrelDropLeft,
        BarrelDropRight,
    ];
    let allowed_models = models();
    for piece in catalog().iter().filter(|piece| {
        matches!(piece.id, TopHat | LaunchedTopHat)
            || piece.inversion.is_some()
            || sixty_transitions.contains(&piece.id)
            || drops.contains(&piece.id)
    }) {
        if allowed_models.iter().any(|model| model.allows_piece(piece)) {
            let rated = piece
                .rated_speeds
                .unwrap_or_else(|| panic!("buildable {:?} has no rated speed window", piece.id));
            let minimum_width =
                if piece.inversion == Some(theme_park_sim::track::Inversion::VerticalLoop) {
                    1.5
                } else {
                    2.0
                };
            assert!(
                rated.minimum_entry >= 0.0
                    && rated.maximum_entry - rated.minimum_entry >= minimum_width,
                "{:?}: invalid rated window {rated:?}",
                piece.id
            );
        }
    }
}

#[test]
fn every_level_to_level_rated_element_passes_full_train_at_both_window_ends() {
    let mut failures = Vec::new();
    for template in catalog().iter().filter(|piece| {
        piece.rated_speeds.is_some()
            && piece.entry == TrackState::origin()
            && piece.exit.slope == theme_park_sim::track::Slope::Level
            && piece.exit.bank == theme_park_sim::track::Bank::Level
    }) {
        let (circuit, element_start, element_end) = circuit_for_level_piece(template.id);
        let rated = template.rated_speeds.unwrap();
        eprintln!(
            "{:?}: catalog length {:.3} m, midpoint {:?}",
            template.id,
            template.length(),
            template.sample(template.length() / 2.0).position
        );
        for model in models()
            .into_iter()
            .filter(|model| model.allows_piece(template))
        {
            for cars in [6, 12] {
                for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                    let is_ascending = template.exit.position().z > template.entry.position().z
                        || matches!(
                            template.id,
                            PieceId::TopHat
                                | PieceId::LaunchedTopHat
                                | PieceId::HalfLoopUp
                                | PieceId::Loop
                                | PieceId::LoopRight
                                | PieceId::LargeLoop
                                | PieceId::LargeLoopRight
                                | PieceId::SteelLoop
                                | PieceId::SteelLoopRight
                        );
                    if std::panic::catch_unwind(|| {
                        assert_train_crosses_element(
                            &model,
                            &circuit,
                            template.id,
                            element_start..element_end,
                            cars,
                            entry_speed,
                            entry_speed == rated.minimum_entry && is_ascending,
                        );
                    })
                    .is_err()
                    {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {entry_speed}",
                            template.id, model.name
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "rated physics failures: {}",
        failures.join("; ")
    );
}

#[test]
fn wooden_strong_floater_is_rated_and_available_on_briar_blitz() {
    let piece = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::WoodenEjectorHill8x64)
        .expect("wooden strong floater is in the catalog");
    assert!(piece.name.contains("floater"));
    assert!(piece.rated_speeds.is_some());
    assert!(wooden_twister_model().allows_piece(piece));
    assert!(!hybrid_model().allows_piece(piece));
    assert!(!multi_launch_model().allows_piece(piece));
    let rated = piece.rated_speeds.unwrap();
    let model = wooden_twister_model();
    let (circuit, element_start, element_end) = circuit_for_level_piece(piece.id);
    for cars in [6, 12] {
        let mut speeds = vec![rated.minimum_entry];
        let mut speed = rated.maximum_entry;
        while speed > rated.minimum_entry {
            speeds.push(speed);
            speed -= 1.0;
        }
        for entry_speed in speeds {
            assert_train_crosses_element(
                &model,
                &circuit,
                piece.id,
                element_start..element_end,
                cars,
                entry_speed,
                false,
            );
        }
        let mut train_config = model.train.clone();
        train_config.cars = cars;
        let mut train = Train::new(
            &circuit,
            &train_config,
            unpowered(),
            element_start,
            rated.maximum_entry,
        );
        let mut minimum_vertical_g = f64::INFINITY;
        for _ in 0..100_000 {
            train.step(&circuit);
            for frame in train.take_readings() {
                for reading in frame {
                    if (element_start..element_end).contains(&reading.distance) {
                        minimum_vertical_g = minimum_vertical_g.min(reading.g.vertical);
                    }
                }
            }
            if train.distance() >= element_end + train_config.length_metres {
                break;
            }
        }
        assert_eq!(
            model
                .comfortable_force_limits
                .classify_airtime(minimum_vertical_g),
            AirtimeRating::Good,
            "{cars} cars: {minimum_vertical_g:.3} g"
        );
    }
}

#[test]
fn every_ejector_hill_is_great_at_rated_maximum_for_each_allowed_model_and_train_layout() {
    let allowed_models = models();
    let ejector_hills: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            matches!(piece.family, Some(PieceFamily::Hill(family))
                if family.feel == HillFeel::Ejector && family.twist.is_none())
                && allowed_models
                    .iter()
                    .any(|model| model.allowed_pieces.contains(&piece.id))
        })
        .collect();
    assert_eq!(ejector_hills.len(), 6, "ejector hill family size");

    let mut failures = Vec::new();
    for piece in ejector_hills {
        let rated = piece
            .rated_speeds
            .expect("ejector hill has a rated speed window");
        let (circuit, element_start, element_end) = circuit_for_level_piece(piece.id);

        for model in allowed_models
            .iter()
            .filter(|model| model.allows_piece(piece))
        {
            for cars in [6, 12] {
                let mut train_config = model.train.clone();
                train_config.cars = cars;
                let mut train = Train::new(
                    &circuit,
                    &train_config,
                    unpowered(),
                    element_start,
                    rated.maximum_entry,
                );
                let mut cars_seen = vec![false; usize::from(cars)];
                let mut minimum_vertical_g = f64::INFINITY;

                for _ in 0..100_000 {
                    train.step(&circuit);
                    for frame in train.take_readings() {
                        for (index, reading) in frame.into_iter().enumerate() {
                            if (element_start..element_end).contains(&reading.distance) {
                                cars_seen[index] = true;
                                minimum_vertical_g = minimum_vertical_g.min(reading.g.vertical);
                            }
                        }
                    }
                    if train.distance() >= element_end + train_config.length_metres {
                        break;
                    }
                    assert!(train.speed() > 0.0, "{} {cars} cars stalled", piece.name);
                    assert!(
                        train.notes().is_empty(),
                        "{} {cars} cars rolled back",
                        piece.name
                    );
                }

                let limits = model.comfortable_force_limits;
                eprintln!(
                    "{} {} {cars} cars / {:.1} m at rated maximum {:.3} m/s: minimum vertical force {minimum_vertical_g:.3} g",
                    piece.name, model.name, train_config.length_metres, rated.maximum_entry
                );
                assert!(
                    cars_seen.into_iter().all(|seen| seen),
                    "{} {} {cars} cars: not every car entered the hill",
                    piece.name,
                    model.name
                );
                assert!(
                    train.distance() >= element_end + train_config.length_metres,
                    "{} {} {cars} cars: train did not clear the hill",
                    piece.name,
                    model.name
                );
                assert!(
                    train.notes().is_empty(),
                    "{} {} {cars} cars: train rolled back",
                    piece.name,
                    model.name
                );
                if minimum_vertical_g < limits.minimum_vertical_g {
                    failures.push(format!(
                        "{} {} {cars} cars: {minimum_vertical_g:.3} g below floor {:.3} g",
                        piece.name, model.name, limits.minimum_vertical_g
                    ));
                }
                if limits.classify_airtime(minimum_vertical_g) != AirtimeRating::Great {
                    failures.push(format!(
                        "{} {} {cars} cars: {minimum_vertical_g:.3} g is not Great",
                        piece.name, model.name
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_tilted_roll_and_drop_variant_passes_full_train_at_both_window_ends() {
    use PieceId::*;

    let sequences: [&[PieceId]; 10] = [
        &[HalfCorkscrewLeftUp, HalfCorkscrewLeftDown],
        &[HalfCorkscrewRightUp, HalfCorkscrewRightDown],
        &[RavenRollLeftUp, RavenRollLeftDown],
        &[RavenRollRightUp, RavenRollRightDown],
        &[
            BankLeft,
            BankedDropLeft,
            UnbankLeft,
            GentleDownToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
        ],
        &[
            BankRight,
            BankedDropRight,
            UnbankRight,
            GentleDownToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
        ],
        &[
            BankLeft,
            TwistedDropLeft,
            GentleDownToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
        ],
        &[
            BankRight,
            TwistedDropRight,
            GentleDownToLevel,
            LevelToGentleUp,
            GentleUpToLevel,
        ],
        &[
            BankLeft,
            BarrelDropLeft,
            GentleDownToLevel,
            LevelToGentleUp,
            OneTileGentleUp,
            OneTileGentleUp,
            GentleUpToLevel,
        ],
        &[
            BankRight,
            BarrelDropRight,
            GentleDownToLevel,
            LevelToGentleUp,
            OneTileGentleUp,
            OneTileGentleUp,
            GentleUpToLevel,
        ],
    ];
    let rated_ids = [
        HalfCorkscrewLeftUp,
        HalfCorkscrewLeftDown,
        HalfCorkscrewRightUp,
        HalfCorkscrewRightDown,
        RavenRollLeftUp,
        RavenRollLeftDown,
        RavenRollRightUp,
        RavenRollRightDown,
        BankedDropLeft,
        BankedDropRight,
        TwistedDropLeft,
        TwistedDropRight,
        BarrelDropLeft,
        BarrelDropRight,
    ];

    for sequence in sequences {
        let (circuit, element_ranges) = circuit_for_sequence(sequence);
        for (piece_id, element_start, element_end) in element_ranges {
            if !rated_ids.contains(&piece_id) {
                continue;
            }
            let piece = catalog().iter().find(|piece| piece.id == piece_id).unwrap();
            let rated = piece
                .rated_speeds
                .unwrap_or_else(|| panic!("{piece_id:?} has no rated speeds"));
            for model in models()
                .into_iter()
                .filter(|model| model.allows_piece(piece))
            {
                for cars in [6, 12] {
                    for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                        let minimum_speed_required = entry_speed == rated.minimum_entry
                            && piece.exit.position().z > piece.entry.position().z;
                        assert_train_crosses_element(
                            &model,
                            &circuit,
                            piece_id,
                            element_start..element_end,
                            cars,
                            entry_speed,
                            minimum_speed_required,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn barrel_roll_drop_rail_descends_monotonically_for_both_mirrors() {
    use PieceId::{BarrelDropLeft, BarrelDropRight};

    for piece_id in [BarrelDropLeft, BarrelDropRight] {
        let piece = catalog().iter().find(|piece| piece.id == piece_id).unwrap();
        let maximum_upward_tangent = (0..=2048)
            .map(|step| {
                piece
                    .sample(piece.length() * f64::from(step) / 2048.0)
                    .tangent
                    .z
            })
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            maximum_upward_tangent <= 1e-8,
            "{piece_id:?} has upward rail tangent {maximum_upward_tangent}"
        );
    }
}

#[test]
fn banked_straights_and_rollouts_pass_full_train_at_rated_window_ends() {
    use PieceId::*;
    for (bank, straight, rollout) in [
        (BankLeft, BankedStraightLeft, RolloutLeft),
        (BankRight, BankedStraightRight, RolloutRight),
    ] {
        for (entry, exit) in [
            (None, None),
            (Some(LevelToGentleUp), Some(GentleUpToLevel)),
            (Some(LevelToGentleDown), Some(GentleDownToLevel)),
        ] {
            let mut sequence = Vec::new();
            sequence.extend(entry);
            sequence.extend([bank, straight, rollout]);
            sequence.extend(exit);
            let (circuit, ranges) = circuit_for_sequence(&sequence);
            for &(id, start, end) in &ranges {
                if id != straight && id != rollout {
                    continue;
                }
                let slope = if entry == Some(LevelToGentleUp) {
                    theme_park_sim::track::Slope::UpGentle
                } else if entry == Some(LevelToGentleDown) {
                    theme_park_sim::track::Slope::DownGentle
                } else {
                    theme_park_sim::track::Slope::Level
                };
                let piece = catalog()
                    .iter()
                    .find(|piece| piece.id == id && piece.entry.slope == slope)
                    .unwrap();
                let rated = piece
                    .rated_speeds
                    .expect("banked continuation needs a rated window");
                for model in models()
                    .into_iter()
                    .filter(|model| model.allows_piece(piece))
                {
                    for cars in [6, 12] {
                        for speed in [rated.minimum_entry, rated.maximum_entry] {
                            assert_train_crosses_element(
                                &model,
                                &circuit,
                                id,
                                start..end,
                                cars,
                                speed,
                                false,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn wave_and_overbank_turns_have_rated_windows_for_both_steel_models() {
    for id in [
        PieceId::WaveTurnLeft,
        PieceId::WaveTurnRight,
        PieceId::HybridOverbankLeft,
        PieceId::HybridOverbankRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated = piece.rated_speeds.expect("turn must have a rated window");
        assert!(rated.maximum_entry - rated.minimum_entry >= 2.0);
        for model in [hybrid_model(), multi_launch_model()] {
            assert!(
                model.allows_piece(piece),
                "{} does not allow {id:?}",
                model.name
            );
        }
    }
}

#[test]
fn wave_and_overbank_turns_have_contiguous_rated_windows() {
    for id in [
        PieceId::WaveTurnLeft,
        PieceId::WaveTurnRight,
        PieceId::HybridOverbankLeft,
        PieceId::HybridOverbankRight,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated = piece.rated_speeds.unwrap();
        let (circuit, start, end) = circuit_for_level_piece(id);
        let mut speed = rated.minimum_entry;
        while speed <= rated.maximum_entry {
            for model in [hybrid_model(), multi_launch_model()] {
                for cars in [6, 12] {
                    assert_train_crosses_element(
                        &model,
                        &circuit,
                        id,
                        start..end,
                        cars,
                        speed,
                        false,
                    );
                }
            }
            speed += 1.0;
        }
    }
}

#[test]
fn speed_matched_turns_stay_inside_rated_force_limits_and_roll_rate() {
    const MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND: f64 = 95.0;
    const MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND: f64 = 200.0;

    let speed_turns: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            matches!(
                piece.family,
                Some(theme_park_sim::track::PieceFamily::SpeedTurn { .. })
            )
        })
        .collect();
    assert_eq!(speed_turns.len(), 6);
    let mut failures = Vec::new();
    let all_models = models();
    let mut scanned_candidates = 0;

    for piece in speed_turns {
        let Some(theme_park_sim::track::PieceFamily::SpeedTurn {
            design_speed_metres_per_second: design_speed,
            ..
        }) = piece.family
        else {
            unreachable!("filtered speed-turn piece lost its family metadata");
        };
        let rated = piece.rated_speeds.expect("speed turn has a rated window");
        let (circuit, start, end) = circuit_for_level_piece(piece.id);
        let allowed_by_any_model = all_models.iter().any(|model| model.allows_piece(piece));
        let mut maximum_design_speed_lateral: f64 = 0.0;
        let mut design_lateral_context = String::from("no model scanned");
        let mut window_forces = ElementForces {
            minimum_vertical: f64::INFINITY,
            maximum_vertical: f64::NEG_INFINITY,
            maximum_lateral: 0.0,
            maximum_longitudinal: 0.0,
            peak_lateral_distance: start,
            peak_lateral_speed: rated.minimum_entry,
        };
        let mut model_window_forces = vec![window_forces; all_models.len()];
        let mut window_lateral_context = String::from("no model scanned");
        let mut speed = rated.minimum_entry;
        while speed <= rated.maximum_entry {
            for (model_index, model) in all_models.iter().enumerate() {
                let model_allows_piece = model.allows_piece(piece);
                for cars in [6, 12] {
                    let scan = scan_train_element(model, &circuit, start..end, cars, speed);
                    let forces = scan.forces;
                    if !scan.all_cars_seen {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {speed:.1} m/s: not every car entered in the candidate scan",
                            piece.id, model.name
                        ));
                    }
                    if !scan.train_cleared {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {speed:.1} m/s: train did not clear in the candidate scan",
                            piece.id, model.name
                        ));
                    }
                    if scan.stalled {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {speed:.1} m/s: train stalled in the candidate scan",
                            piece.id, model.name
                        ));
                    }
                    if scan.rolled_back {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {speed:.1} m/s: train rolled back in the candidate scan",
                            piece.id, model.name
                        ));
                    }
                    let limits = model.comfortable_force_limits;
                    if model_allows_piece
                        && (forces.minimum_vertical < limits.minimum_vertical_g
                            || forces.maximum_vertical > limits.maximum_vertical_g
                            || forces.maximum_lateral > limits.maximum_lateral_g
                            || forces.maximum_longitudinal > limits.maximum_longitudinal_g)
                    {
                        failures.push(format!(
                            "{:?} {} {cars} cars at {speed:.1} m/s: vertical {:.3}..{:.3} g, lateral {:.3} g at {:.2} m (car speed {:.2} m/s), longitudinal {:.3} g; limits {limits:?}",
                            piece.id,
                            model.name,
                            forces.minimum_vertical,
                            forces.maximum_vertical,
                            forces.maximum_lateral,
                            forces.peak_lateral_distance,
                            forces.peak_lateral_speed,
                            forces.maximum_longitudinal
                        ));
                    }
                    window_forces.minimum_vertical =
                        window_forces.minimum_vertical.min(forces.minimum_vertical);
                    window_forces.maximum_vertical =
                        window_forces.maximum_vertical.max(forces.maximum_vertical);
                    if forces.maximum_lateral > window_forces.maximum_lateral {
                        window_forces.maximum_lateral = forces.maximum_lateral;
                        window_forces.peak_lateral_distance = forces.peak_lateral_distance;
                        window_forces.peak_lateral_speed = forces.peak_lateral_speed;
                        window_lateral_context =
                            format!("{} {cars} cars at {speed:.1} m/s", model.name);
                    }
                    window_forces.maximum_longitudinal = window_forces
                        .maximum_longitudinal
                        .max(forces.maximum_longitudinal);
                    let model_window = &mut model_window_forces[model_index];
                    model_window.minimum_vertical =
                        model_window.minimum_vertical.min(forces.minimum_vertical);
                    model_window.maximum_vertical =
                        model_window.maximum_vertical.max(forces.maximum_vertical);
                    if forces.maximum_lateral > model_window.maximum_lateral {
                        model_window.maximum_lateral = forces.maximum_lateral;
                        model_window.peak_lateral_distance = forces.peak_lateral_distance;
                        model_window.peak_lateral_speed = forces.peak_lateral_speed;
                    }
                    model_window.maximum_longitudinal = model_window
                        .maximum_longitudinal
                        .max(forces.maximum_longitudinal);
                    if (speed - design_speed).abs() < 1e-8
                        && forces.maximum_lateral > maximum_design_speed_lateral
                    {
                        maximum_design_speed_lateral = forces.maximum_lateral;
                        design_lateral_context = format!(
                            "{} {cars} cars at {:.2} m, car speed {:.2} m/s",
                            model.name, forces.peak_lateral_distance, forces.peak_lateral_speed
                        );
                    }
                }
            }
            speed += 1.0;
        }
        scanned_candidates += 1;
        let design_speed_peak_roll_rate_degrees_per_second =
            peak_roll_rate_degrees_per_second(piece, design_speed);
        let window_peak_roll_rate_degrees_per_second =
            peak_roll_rate_degrees_per_second(piece, rated.maximum_entry);
        eprintln!(
            "{:?}: scanned across all models; allowed by any model: {allowed_by_any_model}; full-train vertical {:.3}..{:.3} g, lateral {:.3} g across window at {window_lateral_context}, {:.3} g at design speed at {design_lateral_context}, longitudinal {:.3} g, design-speed roll rate {design_speed_peak_roll_rate_degrees_per_second:.1} degrees/s, peak rated-window roll rate {window_peak_roll_rate_degrees_per_second:.1} degrees/s",
            piece.id,
            window_forces.minimum_vertical,
            window_forces.maximum_vertical,
            window_forces.maximum_lateral,
            maximum_design_speed_lateral,
            window_forces.maximum_longitudinal
        );
        for (model, forces) in all_models.iter().zip(model_window_forces) {
            eprintln!(
                "{:?} {} allowed={}: full-train vertical {:.4}..{:.4} g, lateral {:.4} g at {:.2} m (car speed {:.2} m/s), longitudinal {:.4} g",
                piece.id,
                model.name,
                model.allows_piece(piece),
                forces.minimum_vertical,
                forces.maximum_vertical,
                forces.maximum_lateral,
                forces.peak_lateral_distance,
                forces.peak_lateral_speed,
                forces.maximum_longitudinal
            );
        }
        if maximum_design_speed_lateral > 0.35 {
            failures.push(format!(
                "{} design-speed worst-car lateral {maximum_design_speed_lateral:.3} g at {design_lateral_context} exceeds 0.35 g",
                piece.name
            ));
        }
        if design_speed_peak_roll_rate_degrees_per_second
            > MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND
        {
            failures.push(format!(
                "{} design-speed roll rate {design_speed_peak_roll_rate_degrees_per_second:.1} degrees/s exceeds {MAXIMUM_DESIGN_SPEED_ROLL_RATE_DEGREES_PER_SECOND:.1} degrees/s",
                piece.name
            ));
        }
        if window_peak_roll_rate_degrees_per_second
            > MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND
        {
            failures.push(format!(
                "{} peak rated-window roll rate {window_peak_roll_rate_degrees_per_second:.1} degrees/s exceeds {MAXIMUM_RATED_WINDOW_ROLL_RATE_DEGREES_PER_SECOND:.1} degrees/s",
                piece.name
            ));
        }
    }
    assert_eq!(
        scanned_candidates, 6,
        "full-train scan covers every speed-turn candidate"
    );
    assert!(
        failures.is_empty(),
        "speed-turn force scan failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn floater_point_mass_forecast_compared_with_full_train() {
    use theme_park_sim::track::predicted_hill_forces;

    let piece = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::FloaterHill8x64)
        .unwrap();
    let entry_speed = piece.rated_speeds.unwrap().maximum_entry;
    let forecast = predicted_hill_forces(piece, entry_speed).unwrap();
    let (circuit, start, end) = circuit_for_level_piece(piece.id);
    for cars in [6, 12] {
        let mut config = hybrid_model().train;
        config.cars = cars;
        let mut train = Train::new(&circuit, &config, unpowered(), start, entry_speed);
        let mut minimum = f64::INFINITY;
        for _ in 0..100_000 {
            train.step(&circuit);
            for frame in train.take_readings() {
                for reading in frame {
                    if (start..end).contains(&reading.distance) {
                        minimum = minimum.min(reading.g.vertical);
                    }
                }
            }
            if train.distance() >= end + config.length_metres {
                break;
            }
            assert!(train.speed() > 0.0 && train.notes().is_empty());
        }
        assert!(minimum.is_finite());
        assert!(train.distance() >= end + config.length_metres);
        eprintln!(
            "FloaterHill8x64 {cars} cars at {entry_speed:.3} m/s: forecast {:.3} g, full train {minimum:.3} g, difference {:+.3} g",
            forecast.minimum_vertical_g,
            minimum - forecast.minimum_vertical_g
        );
    }
}
