use std::collections::{HashMap, HashSet, VecDeque};
use theme_park_sim::physics::{Circuit, CoasterRun, GRAVITY, PhysicsConfig, Train};
use theme_park_sim::track::{
    Bank, Heading, Piece, PieceId, PieceKind, TrackState, catalog, hybrid_model,
    multi_launch_model, wooden_twister_model,
};

fn circuit_with_elements(elements: &[PieceId]) -> (Circuit, f64, f64) {
    let catalog = catalog();
    circuit_with_elements_from_catalog(&catalog, elements)
}

type ConnectorKey = (i32, i32, i32, u8, u8, u8, u8, u8);

fn connector_key(state: TrackState) -> ConnectorKey {
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

fn heading_index(heading: Heading) -> i32 {
    match heading {
        Heading::East => 0,
        Heading::NorthEast => 1,
        Heading::North => 2,
        Heading::NorthWest => 3,
        Heading::West => 4,
        Heading::SouthWest => 5,
        Heading::South => 6,
        Heading::SouthEast => 7,
    }
}

fn heading_from_index(index: i32) -> Heading {
    [
        Heading::East,
        Heading::NorthEast,
        Heading::North,
        Heading::NorthWest,
        Heading::West,
        Heading::SouthWest,
        Heading::South,
        Heading::SouthEast,
    ][index.rem_euclid(8) as usize]
}

fn connector_exit_at(piece: &Piece, state: TrackState) -> Option<TrackState> {
    if state.slope != piece.entry.slope
        || state.bank != piece.entry.bank
        || state.horizontal_curvature != piece.entry.horizontal_curvature
        || state.vertical_curvature != piece.entry.vertical_curvature
    {
        return None;
    }
    let rotation =
        (heading_index(state.heading) - heading_index(piece.entry.heading)).rem_euclid(8);
    let dx = piece.exit.x - piece.entry.x;
    let dy = piece.exit.y - piece.entry.y;
    let (x, y) = match rotation {
        0 => (dx, dy),
        1 => (dx - dy, dx + dy),
        2 => (-dy, dx),
        3 => (-dx - dy, dx - dy),
        4 => (-dx, -dy),
        5 => (dy - dx, -dx - dy),
        6 => (dy, -dx),
        7 => (dx + dy, dy - dx),
        _ => unreachable!(),
    };
    Some(TrackState {
        x: state.x + x,
        y: state.y + y,
        z: state.z
            + if rotation % 2 == 0 {
                piece.exit.z - piece.entry.z
            } else {
                (f64::from(piece.exit.z - piece.entry.z) * std::f64::consts::SQRT_2).round() as i32
            },
        heading: heading_from_index(heading_index(piece.exit.heading) + rotation),
        slope: piece.exit.slope,
        bank: piece.exit.bank,
        horizontal_curvature: piece.exit.horizontal_curvature,
        vertical_curvature: piece.exit.vertical_curvature,
    })
}

fn circuit_with_elements_from_catalog(
    catalog: &[Piece],
    elements: &[PieceId],
) -> (Circuit, f64, f64) {
    circuit_with_lifted_elements_from_catalog(catalog, elements, 0)
}

fn circuit_with_lifted_elements_from_catalog(
    catalog: &[Piece],
    elements: &[PieceId],
    lifted_element_count: usize,
) -> (Circuit, f64, f64) {
    assert!(lifted_element_count <= elements.len());
    let template = |id| catalog.iter().find(|piece| piece.id == id).unwrap();
    let start = TrackState::origin();
    let mut state = start;
    let mut pieces: Vec<Piece> = Vec::new();
    let mut append = |id| {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    };
    append(PieceId::Station);
    let element_start = pieces.iter().map(Piece::length).sum();
    for (element_index, &id) in elements.iter().enumerate() {
        let mut piece = template(id).at(state).unwrap();
        if element_index < lifted_element_count {
            piece.kind = PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let element_end = pieces.iter().map(Piece::length).sum();

    // Find a return path using level track, keeping the element sequence intact.
    let search_start_key = connector_key(state);
    let start_key = connector_key(start);
    let mut queue = VecDeque::from([(state, search_start_key, 0)]);
    let mut visited = HashSet::from([search_start_key]);
    let mut predecessors: HashMap<ConnectorKey, (ConnectorKey, PieceId)> = HashMap::new();
    let return_path = loop {
        let (current, current_key, path_length) = queue.pop_front().expect("no return path");
        if current_key == start_key {
            let mut path = Vec::new();
            let mut key = current_key;
            while key != search_start_key {
                let (previous_key, piece_id) = predecessors[&key];
                path.push(piece_id);
                key = previous_key;
            }
            path.reverse();
            break path;
        }
        if path_length >= 80 {
            continue;
        }
        for id in [
            PieceId::Straight,
            PieceId::TurnSmallStandalone,
            PieceId::TurnSmallRightStandalone,
            PieceId::TurnLargeStandalone,
            PieceId::TurnLargeRightStandalone,
            PieceId::OffsetLeft,
            PieceId::OffsetRight,
        ] {
            let next = connector_exit_at(template(id), current).unwrap();
            if next.x.abs() > 300 || next.y.abs() > 300 {
                continue;
            }
            let next_key = connector_key(next);
            if !visited.insert(next_key) {
                continue;
            }
            predecessors.insert(next_key, (current_key, id));
            queue.push_back((next, next_key, path_length + 1));
        }
    };
    for id in return_path {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    (Circuit::new(pieces).unwrap(), element_start, element_end)
}

fn circuit_with_gentle_lift_drop_and_loop(
    loop_id: PieceId,
    gentle_up_piece_count: usize,
) -> Circuit {
    // The catalog's gentle lift steps snap to 8 m increments: seven and three
    // climbs give the plan's roughly 60 m and 30 m drops.
    let mut elements = vec![PieceId::LevelToGentleUp];
    elements.extend(std::iter::repeat_n(
        PieceId::GentleUp,
        gentle_up_piece_count,
    ));
    elements.push(PieceId::GentleUpToLevel);
    let lifted_element_count = elements.len();
    elements.push(PieceId::LevelToGentleDown);
    elements.extend(std::iter::repeat_n(
        PieceId::GentleDown,
        gentle_up_piece_count,
    ));
    elements.push(PieceId::GentleDownToLevel);
    elements.push(loop_id);

    circuit_with_lifted_elements_from_catalog(&catalog(), &elements, lifted_element_count).0
}

fn run_one_car_coaster(
    circuit: &Circuit,
    rolling_resistance: f64,
) -> theme_park_sim::physics::CoasterOutcome {
    let mut train_configuration = hybrid_model().train;
    train_configuration.cars = 1;
    train_configuration.length_metres = 4.0;
    let mut run = CoasterRun::new(
        circuit,
        &train_configuration,
        PhysicsConfig {
            lift_speed: 5.0,
            rolling_resistance,
            powered_sections: true,
            ..PhysicsConfig::default()
        },
    );
    for _ in 0..100_000 {
        if let Some(outcome) = run.step(circuit) {
            return outcome;
        }
    }
    panic!("one-car test run did not complete or recover");
}

fn unpowered() -> PhysicsConfig {
    PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    }
}

fn circuit_with_straight_return(elements: &[PieceId]) -> Circuit {
    let catalog = catalog();
    let template = |id| catalog.iter().find(|piece| piece.id == id).unwrap();
    let start = TrackState::origin();
    let mut state = start;
    let mut pieces = Vec::new();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight]
        .into_iter()
        .chain(elements.iter().copied())
        .chain([
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnSmallStandalone,
            PieceId::TurnSmallStandalone,
        ])
    {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    while state.x > start.x {
        let piece = template(PieceId::HalfTileStraight).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    for _ in 0..2 {
        let piece = template(PieceId::TurnSmallStandalone).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    assert_eq!(state, start);
    Circuit::new(pieces).unwrap()
}

#[test]
fn sixty_degree_crests_pass_full_trains_at_both_rated_endpoints() {
    use PieceId::*;
    let sequences: [&[PieceId]; 2] = [
        &[
            LevelToSixtyUp,
            SixtyUpToLevel,
            Straight,
            Straight,
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
    let models = [wooden_twister_model(), hybrid_model(), multi_launch_model()];
    for sequence in sequences {
        let (circuit, _, _) = circuit_with_elements(sequence);
        for id in [
            SixtyUpToLevel,
            LevelToSixtyDown,
            SixtyToSteepUp,
            SteepToSixtyDown,
        ] {
            let Some(index) = circuit.pieces().iter().position(|piece| piece.id == id) else {
                continue;
            };
            let piece = &circuit.pieces()[index];
            let rated = piece
                .rated_speeds
                .expect("crest needs a rated speed window");
            assert!(
                rated.minimum_entry >= 3.0 && rated.maximum_entry <= 30.0,
                "{id:?}: {rated:?} must stay within 3–30 m/s"
            );
            assert!(
                rated.maximum_entry - rated.minimum_entry >= 2.0,
                "{id:?}: {rated:?} is narrower than 2 m/s"
            );
            let start: f64 = circuit.pieces()[..index].iter().map(Piece::length).sum();
            let end = start + piece.length();

            for model in models.iter().filter(|model| model.allows_piece(piece)) {
                for cars in [6, 12] {
                    let mut configuration = model.train.clone();
                    configuration.cars = cars;
                    for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                        let mut train =
                            Train::new(&circuit, &configuration, unpowered(), start, entry_speed);
                        let mut cars_seen = vec![false; usize::from(cars)];
                        let mut minimum_vertical = f64::INFINITY;
                        let mut maximum_vertical = f64::NEG_INFINITY;
                        let mut maximum_lateral: f64 = 0.0;
                        let mut maximum_longitudinal: f64 = 0.0;
                        let mut minimum_speed = f64::INFINITY;
                        for _ in 0..100_000 {
                            train.step(&circuit);
                            for frame in train.take_readings() {
                                for (car_index, car) in frame.into_iter().enumerate() {
                                    if (start..end).contains(&car.distance) {
                                        cars_seen[car_index] = true;
                                        minimum_vertical = minimum_vertical.min(car.g.vertical);
                                        maximum_vertical = maximum_vertical.max(car.g.vertical);
                                        maximum_lateral = maximum_lateral.max(car.g.lateral.abs());
                                        maximum_longitudinal =
                                            maximum_longitudinal.max(car.g.longitudinal.abs());
                                        minimum_speed = minimum_speed.min(car.speed);
                                    }
                                }
                            }
                            if train.distance() >= end + configuration.length_metres
                                || train.speed() <= 0.0
                            {
                                break;
                            }
                        }
                        let limits = model.comfortable_force_limits;
                        assert!(
                            cars_seen.iter().all(|seen| *seen)
                                && train.distance() >= end + configuration.length_metres
                                && train.notes().is_empty(),
                            "{id:?} {} {cars} cars at {entry_speed:.3} m/s did not clear",
                            model.name
                        );
                        assert!(
                            minimum_vertical >= limits.minimum_vertical_g
                                && maximum_vertical <= limits.maximum_vertical_g
                                && maximum_lateral <= limits.maximum_lateral_g
                                && maximum_longitudinal <= limits.maximum_longitudinal_g,
                            "{id:?} {} {cars} cars at {entry_speed:.3} m/s: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}; limits {limits:?}",
                            model.name
                        );
                        if piece.exit.position().z > piece.entry.position().z
                            && entry_speed == rated.minimum_entry
                        {
                            assert!(minimum_speed >= 5.0, "{id:?}: {minimum_speed}");
                        }
                        eprintln!(
                            "{id:?} {} {cars} cars at {entry_speed:.3}: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}",
                            model.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sixty_degree_ascending_crest_has_a_contiguous_rated_window() {
    use PieceId::*;
    let sequence = [
        LevelToSixtyUp,
        SixtyUpToLevel,
        Straight,
        Straight,
        LevelToSixtyDown,
        SixtyDownToLevel,
    ];
    let (circuit, _, _) = circuit_with_elements(&sequence);
    let piece_index = circuit
        .pieces()
        .iter()
        .position(|piece| piece.id == SixtyUpToLevel)
        .unwrap();
    let piece = &circuit.pieces()[piece_index];
    let rated = piece.rated_speeds.unwrap();
    let element_start: f64 = circuit.pieces()[..piece_index]
        .iter()
        .map(Piece::length)
        .sum();
    let element_end = element_start + piece.length();
    let model = hybrid_model();
    let limits = model.comfortable_force_limits;
    let sample_count = (rated.maximum_entry - rated.minimum_entry).ceil() as usize;

    for sample_index in 0..=sample_count {
        let entry_speed = (rated.minimum_entry + sample_index as f64).min(rated.maximum_entry);
        for cars in [6, 12] {
            let mut configuration = model.train.clone();
            configuration.cars = cars;
            let mut train = Train::new(
                &circuit,
                &configuration,
                unpowered(),
                element_start,
                entry_speed,
            );
            let mut cars_seen = vec![false; usize::from(cars)];
            let mut minimum_speed = f64::INFINITY;
            let mut minimum_vertical = f64::INFINITY;
            let mut maximum_vertical = f64::NEG_INFINITY;
            let mut maximum_lateral: f64 = 0.0;
            let mut maximum_longitudinal: f64 = 0.0;
            for _ in 0..100_000 {
                train.step(&circuit);
                for frame in train.take_readings() {
                    for (car_index, car) in frame.into_iter().enumerate() {
                        if (element_start..element_end).contains(&car.distance) {
                            cars_seen[car_index] = true;
                            minimum_speed = minimum_speed.min(car.speed);
                            minimum_vertical = minimum_vertical.min(car.g.vertical);
                            maximum_vertical = maximum_vertical.max(car.g.vertical);
                            maximum_lateral = maximum_lateral.max(car.g.lateral.abs());
                            maximum_longitudinal =
                                maximum_longitudinal.max(car.g.longitudinal.abs());
                        }
                    }
                }
                if train.distance() >= element_end + configuration.length_metres
                    || train.speed() <= 0.0
                {
                    break;
                }
            }
            assert!(
                cars_seen.iter().all(|seen| *seen)
                    && train.distance() >= element_end + configuration.length_metres
                    && train.notes().is_empty()
                    && minimum_speed >= 5.0
                    && minimum_vertical >= limits.minimum_vertical_g
                    && maximum_vertical <= limits.maximum_vertical_g
                    && maximum_lateral <= limits.maximum_lateral_g
                    && maximum_longitudinal <= limits.maximum_longitudinal_g,
                "SixtyUpToLevel {cars} cars at {entry_speed:.3} m/s: vertical {minimum_vertical}..{maximum_vertical}, lateral {maximum_lateral}, longitudinal {maximum_longitudinal}, minimum speed {minimum_speed}"
            );
        }
    }
}

#[test]
fn sixty_degree_pull_profiles_keep_trains_within_force_limits() {
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
        let circuit = circuit_with_straight_return(sequence);
        for id in [
            LevelToSixtyUp,
            SixtyDownToLevel,
            SteepToSixtyUp,
            SixtyToSteepDown,
        ] {
            let Some(index) = circuit.pieces().iter().position(|piece| piece.id == id) else {
                continue;
            };
            let piece = &circuit.pieces()[index];
            let start: f64 = circuit.pieces()[..index].iter().map(Piece::length).sum();
            let end = start + piece.length();
            let rated = piece.rated_speeds.unwrap();
            for model in [hybrid_model(), multi_launch_model()] {
                for cars in [6, 12] {
                    let mut config = model.train.clone();
                    config.cars = cars;
                    for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                        let mut train =
                            Train::new(&circuit, &config, unpowered(), start, entry_speed);
                        let mut seen = vec![false; cars as usize];
                        let mut minimum_vertical = f64::INFINITY;
                        let mut maximum_vertical = f64::NEG_INFINITY;
                        let mut maximum_lateral: f64 = 0.0;
                        let mut maximum_longitudinal: f64 = 0.0;
                        let mut minimum_speed = f64::INFINITY;
                        for _ in 0..100_000 {
                            train.step(&circuit);
                            for frame in train.take_readings() {
                                for (index, car) in frame.iter().enumerate() {
                                    if (start..end).contains(&car.distance) {
                                        seen[index] = true;
                                        minimum_speed = minimum_speed.min(car.speed);
                                        minimum_vertical = minimum_vertical.min(car.g.vertical);
                                        maximum_vertical = maximum_vertical.max(car.g.vertical);
                                        maximum_lateral = maximum_lateral.max(car.g.lateral.abs());
                                        maximum_longitudinal =
                                            maximum_longitudinal.max(car.g.longitudinal.abs());
                                    }
                                }
                            }
                            if train.distance() >= end + config.length_metres
                                || train.speed() <= 0.0
                            {
                                break;
                            }
                        }
                        eprintln!(
                            "{id:?} {} {cars} cars at {entry_speed:.3}: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}, minimum speed {minimum_speed:.3}",
                            model.name
                        );
                        let limits = model.comfortable_force_limits;
                        assert!(
                            seen.iter().all(|seen| *seen)
                                && train.distance() >= end + config.length_metres
                                && train.notes().is_empty(),
                            "{id:?} {cars} cars at {entry_speed}: incomplete, progress {} required {}, speed {} seen {:?}",
                            train.distance() - start,
                            end - start + config.length_metres,
                            train.speed(),
                            seen
                        );
                        assert!(
                            minimum_vertical >= limits.minimum_vertical_g
                                && maximum_vertical <= limits.maximum_vertical_g
                                && maximum_lateral <= limits.maximum_lateral_g
                                && maximum_longitudinal <= limits.maximum_longitudinal_g,
                            "{id:?} {cars} cars at {entry_speed}"
                        );
                        if matches!(id, LevelToSixtyUp | SteepToSixtyUp)
                            && entry_speed == rated.minimum_entry
                        {
                            assert!(minimum_speed >= 5.0, "{id:?}: {minimum_speed}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn descending_fifteen_degree_pull_maximum_respects_longitudinal_limit() {
    use PieceId::*;
    let circuit = circuit_with_straight_return(&[
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
    ]);
    let index = circuit
        .pieces()
        .iter()
        .position(|piece| piece.id == SixtyToSteepDown)
        .unwrap();
    let start: f64 = circuit.pieces()[..index].iter().map(Piece::length).sum();
    let end = start + circuit.pieces()[index].length();
    let maximum = circuit.pieces()[index].rated_speeds.unwrap().maximum_entry;
    let mut config = hybrid_model().train;
    config.cars = 12;
    for (speed, should_pass) in [(maximum, true), (maximum + 1.0, false)] {
        let mut train = Train::new(&circuit, &config, unpowered(), start, speed);
        let mut maximum_longitudinal: f64 = 0.0;
        for _ in 0..100_000 {
            train.step(&circuit);
            for frame in train.take_readings() {
                for car in frame {
                    if (start..end).contains(&car.distance) {
                        maximum_longitudinal = maximum_longitudinal.max(car.g.longitudinal.abs());
                    }
                }
            }
            if train.distance() >= end + config.length_metres || train.speed() <= 0.0 {
                break;
            }
        }
        assert!(train.distance() >= end + config.length_metres);
        assert_eq!(
            maximum_longitudinal <= 1.5,
            should_pass,
            "speed {speed}, longitudinal {maximum_longitudinal}"
        );
    }
}

#[test]
fn trains_cross_half_loops_within_model_force_limits() {
    let catalog = catalog();
    let template = |id| catalog.iter().find(|piece| piece.id == id).unwrap();
    let mut inverted_straight = template(PieceId::Straight).clone();
    inverted_straight.entry.bank = Bank::Inverted;
    inverted_straight.exit.bank = Bank::Inverted;
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    let mut ranges = Vec::new();
    for piece in [
        template(PieceId::Station),
        template(PieceId::Straight),
        template(PieceId::Straight),
        template(PieceId::HalfLoopUp),
        &inverted_straight,
        &inverted_straight,
        template(PieceId::HalfLoopDown),
        template(PieceId::Straight),
        template(PieceId::Straight),
    ] {
        let piece = piece.at(state).unwrap();
        let start: f64 = pieces.iter().map(Piece::length).sum();
        if matches!(piece.id, PieceId::HalfLoopUp | PieceId::HalfLoopDown) {
            ranges.push((
                piece.id,
                start..start + piece.length(),
                piece.rated_speeds.unwrap(),
            ));
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let return_path = [
        PieceId::TurnSmallStandalone,
        PieceId::OffsetLeft,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
        PieceId::OffsetRight,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
        PieceId::OffsetLeft,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
        PieceId::OffsetRight,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallStandalone,
    ];
    for id in return_path {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = Circuit::new(pieces).unwrap();
    for (id, range, rated) in ranges {
        for model in [hybrid_model(), multi_launch_model()] {
            for cars in [6, 12] {
                let mut configuration = model.train.clone();
                configuration.cars = cars;
                for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                    let mut train = Train::new(
                        &circuit,
                        &configuration,
                        unpowered(),
                        range.start,
                        entry_speed,
                    );
                    let mut seen = vec![false; cars as usize];
                    let mut minimum_speed = f64::INFINITY;
                    let mut minimum_vertical = f64::INFINITY;
                    let mut maximum_vertical = f64::NEG_INFINITY;
                    let mut maximum_lateral: f64 = 0.0;
                    let mut maximum_longitudinal: f64 = 0.0;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for (index, car) in frame.iter().enumerate() {
                                if range.contains(&car.distance) {
                                    seen[index] = true;
                                    minimum_speed = minimum_speed.min(car.speed);
                                    minimum_vertical = minimum_vertical.min(car.g.vertical);
                                    maximum_vertical = maximum_vertical.max(car.g.vertical);
                                    maximum_lateral = maximum_lateral.max(car.g.lateral.abs());
                                    maximum_longitudinal =
                                        maximum_longitudinal.max(car.g.longitudinal.abs());
                                }
                            }
                        }
                        if train.distance() >= range.end + configuration.length_metres {
                            break;
                        }
                    }
                    eprintln!(
                        "{id:?} {} {cars} cars at {entry_speed:.3} m/s: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}, minimum speed {minimum_speed:.3}",
                        model.name
                    );
                    let limits = model.comfortable_force_limits;
                    assert!(
                        seen.iter().all(|seen| *seen)
                            && train.distance() >= range.end + configuration.length_metres
                            && train.notes().is_empty(),
                        "{id:?} {} {cars} cars at {entry_speed}: incomplete or rollback",
                        model.name
                    );
                    assert!(
                        minimum_vertical >= limits.minimum_vertical_g
                            && maximum_vertical <= limits.maximum_vertical_g
                            && maximum_lateral <= limits.maximum_lateral_g
                            && maximum_longitudinal <= limits.maximum_longitudinal_g,
                        "{id:?} {} {cars} cars at {entry_speed}: vertical {minimum_vertical}..{maximum_vertical}, lateral {maximum_lateral}, longitudinal {maximum_longitudinal}",
                        model.name
                    );
                    if id == PieceId::HalfLoopUp && entry_speed == rated.minimum_entry {
                        assert!(
                            minimum_speed >= 5.0,
                            "{id:?}: minimum speed {minimum_speed}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn all_constructible_half_inversion_combinations_pass_at_rated_element_bands() {
    use PieceId::*;
    let combinations = [
        [
            HalfLoopUp,
            HalfCorkscrewLeftDown,
            HalfCorkscrewLeftUp,
            HalfLoopDown,
        ],
        [
            HalfLoopUp,
            HalfCorkscrewRightDown,
            HalfCorkscrewRightUp,
            HalfLoopDown,
        ],
        [
            HalfLoopUp,
            RavenRollLeftDown,
            RavenRollRightUp,
            HalfLoopDown,
        ],
        [
            HalfLoopUp,
            RavenRollRightDown,
            RavenRollLeftUp,
            HalfLoopDown,
        ],
        [
            HalfCorkscrewLeftUp,
            HalfLoopDown,
            HalfLoopUp,
            HalfCorkscrewLeftDown,
        ],
        [
            HalfCorkscrewRightUp,
            HalfLoopDown,
            HalfLoopUp,
            HalfCorkscrewRightDown,
        ],
    ];
    let templates = catalog();
    for elements in combinations {
        let (circuit, _, _) = circuit_with_elements(&elements);
        let mut element_ranges = Vec::with_capacity(elements.len());
        let mut start = circuit.pieces()[0].length();
        for (index, id) in elements.iter().enumerate() {
            let piece = &circuit.pieces()[index + 1];
            assert_eq!(
                piece.id, *id,
                "element sequence changed while building circuit"
            );
            let end = start + piece.length();
            let rated = templates
                .iter()
                .find(|template| template.id == *id)
                .unwrap()
                .rated_speeds
                .unwrap_or_else(|| panic!("{id:?} has no rated entry band"));
            element_ranges.push((
                *id,
                start..end,
                piece.exit.position().z > piece.entry.position().z,
                rated,
            ));
            start = end;
        }

        let mut supported_model_count = 0;
        for model in [hybrid_model(), multi_launch_model()] {
            if elements.iter().any(|id| {
                let piece = templates.iter().find(|piece| piece.id == *id).unwrap();
                !model.allows_piece(piece)
            }) {
                continue;
            }
            supported_model_count += 1;
            let configuration = model.train.clone();
            let limits = model.comfortable_force_limits;
            for (id, range, ascending, rated) in &element_ranges {
                for entry_speed in [rated.minimum_entry, rated.maximum_entry] {
                    let mut train = Train::new(
                        &circuit,
                        &configuration,
                        unpowered(),
                        range.start,
                        entry_speed,
                    );
                    let mut cars_seen = vec![false; usize::from(configuration.cars)];
                    let mut minimum_vertical = f64::INFINITY;
                    let mut maximum_vertical = f64::NEG_INFINITY;
                    let mut maximum_lateral: f64 = 0.0;
                    let mut maximum_longitudinal: f64 = 0.0;
                    let mut minimum_speed = f64::INFINITY;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for (car_index, reading) in frame.into_iter().enumerate() {
                                if range.contains(&reading.distance) {
                                    cars_seen[car_index] = true;
                                    minimum_vertical = minimum_vertical.min(reading.g.vertical);
                                    maximum_vertical = maximum_vertical.max(reading.g.vertical);
                                    maximum_lateral = maximum_lateral.max(reading.g.lateral.abs());
                                    maximum_longitudinal =
                                        maximum_longitudinal.max(reading.g.longitudinal.abs());
                                    minimum_speed = minimum_speed.min(reading.speed);
                                }
                            }
                        }
                        if train.distance() >= range.end + configuration.length_metres
                            || train.speed() <= 0.0
                            || !train.notes().is_empty()
                        {
                            break;
                        }
                    }
                    eprintln!(
                        "{id:?} {} at rated {entry_speed:.3} m/s: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}",
                        model.name
                    );
                    assert!(
                        cars_seen.iter().all(|seen| *seen)
                            && train.distance() >= range.end + configuration.length_metres
                            && train.notes().is_empty(),
                        "{id:?} {} at rated {entry_speed} m/s did not clear without rollback",
                        model.name
                    );
                    assert!(
                        minimum_vertical >= limits.minimum_vertical_g
                            && maximum_vertical <= limits.maximum_vertical_g
                            && maximum_lateral <= limits.maximum_lateral_g
                            && maximum_longitudinal <= limits.maximum_longitudinal_g,
                        "{id:?} {} at rated {entry_speed} m/s: vertical {minimum_vertical}..{maximum_vertical}, lateral {maximum_lateral}, longitudinal {maximum_longitudinal}; limits {limits:?}",
                        model.name
                    );
                    if *ascending && entry_speed == rated.minimum_entry {
                        assert!(
                            minimum_speed >= 5.0,
                            "{id:?} {} at rated minimum {entry_speed} m/s slowed to {minimum_speed} m/s",
                            model.name
                        );
                    }
                }
            }
        }
        assert!(supported_model_count > 0, "no model supports {elements:?}");
    }
}

#[test]
fn full_trains_cross_loop_profiles_within_their_rated_force_limits() {
    use PieceId::*;
    let piece_catalog = catalog();
    for (model, piece_id) in [
        (wooden_twister_model(), SteelLoop),
        (hybrid_model(), Loop),
        (hybrid_model(), LargeLoop),
        (multi_launch_model(), Loop),
        (multi_launch_model(), LargeLoop),
    ] {
        let piece = piece_catalog
            .iter()
            .find(|piece| piece.id == piece_id)
            .unwrap();
        let rated_speeds = piece.rated_speeds.unwrap();
        let (circuit, element_start, element_end) =
            circuit_with_elements_from_catalog(&piece_catalog, &[piece_id]);
        for cars in [6, 12] {
            let mut train_configuration = model.train.clone();
            train_configuration.cars = cars;
            for entry_speed in [rated_speeds.minimum_entry, rated_speeds.maximum_entry] {
                let mut train = Train::new(
                    &circuit,
                    &train_configuration,
                    unpowered(),
                    element_start,
                    entry_speed,
                );
                let mut cars_seen = vec![false; cars as usize];
                let mut minimum_speed = f64::INFINITY;
                let mut minimum_vertical = f64::INFINITY;
                let mut maximum_vertical = f64::NEG_INFINITY;
                let mut maximum_lateral: f64 = 0.0;
                let mut maximum_longitudinal: f64 = 0.0;
                let mut cleared = false;
                for _ in 0..100_000 {
                    train.step(&circuit);
                    for frame in train.take_readings() {
                        for (car_index, reading) in frame.into_iter().enumerate() {
                            if (element_start..element_end).contains(&reading.distance) {
                                cars_seen[car_index] = true;
                                minimum_speed = minimum_speed.min(reading.speed);
                                minimum_vertical = minimum_vertical.min(reading.g.vertical);
                                maximum_vertical = maximum_vertical.max(reading.g.vertical);
                                maximum_lateral = maximum_lateral.max(reading.g.lateral.abs());
                                maximum_longitudinal =
                                    maximum_longitudinal.max(reading.g.longitudinal.abs());
                            }
                        }
                    }
                    if train.distance() >= element_end + train_configuration.length_metres {
                        cleared = true;
                        break;
                    }
                    assert!(train.notes().is_empty(), "{piece_id:?}: train rolled back");
                }
                let limits = model.comfortable_force_limits;
                eprintln!(
                    "{piece_id:?} {} {cars} cars at {entry_speed:.3} m/s: vertical {minimum_vertical:.3}..{maximum_vertical:.3}, lateral {maximum_lateral:.3}, longitudinal {maximum_longitudinal:.3}, minimum speed {minimum_speed:.3}",
                    model.name
                );
                assert!(
                    cars_seen.iter().all(|seen| *seen) && cleared && train.notes().is_empty(),
                    "{piece_id:?} {} {cars} cars at {entry_speed}: incomplete crossing or rollback",
                    model.name
                );
                assert!(
                    minimum_vertical >= limits.minimum_vertical_g
                        && maximum_vertical <= limits.maximum_vertical_g
                        && maximum_lateral <= limits.maximum_lateral_g
                        && maximum_longitudinal <= limits.maximum_longitudinal_g,
                    "{piece_id:?} {} {cars} cars at {entry_speed}: vertical {minimum_vertical}..{maximum_vertical}, lateral {maximum_lateral}, longitudinal {maximum_longitudinal}; limits {limits:?}",
                    model.name
                );
                if entry_speed == rated_speeds.minimum_entry {
                    assert!(
                        minimum_speed >= 5.0,
                        "{piece_id:?}: minimum speed {minimum_speed}"
                    );
                }
            }
        }
    }
}

#[test]
fn top_hat_slows_on_the_climb_and_recovers_speed_on_the_descent() {
    let (circuit, start, end) = circuit_with_elements(&[PieceId::TopHat]);
    let mut configuration = multi_launch_model().train;
    configuration.cars = 1;
    let initial_speed: f64 = 35.0;
    let mut train = Train::new(&circuit, &configuration, unpowered(), start, initial_speed);
    let start_height = circuit.sample(start).position.z;
    let mut minimum_speed = initial_speed;
    let mut peak_height = start_height;
    for _ in 0..20_000 {
        train.step(&circuit);
        if train.distance() >= end {
            break;
        }
        minimum_speed = minimum_speed.min(train.speed());
        peak_height = peak_height.max(circuit.sample(train.distance()).position.z);
    }
    assert!(
        train.distance() >= end,
        "fast train did not clear the top hat"
    );
    let expected_peak_speed =
        (initial_speed.powi(2) - 2.0 * GRAVITY * (peak_height - start_height)).sqrt();
    assert!(peak_height - start_height > 25.0);
    assert!((minimum_speed - expected_peak_speed).abs() < 0.1);
    assert!((train.speed() - initial_speed).abs() < 0.1);

    let mut slow = Train::new(&circuit, &configuration, unpowered(), start, 10.0);
    let mut reversed = false;
    for _ in 0..20_000 {
        slow.step(&circuit);
        if slow.speed() < 0.0 {
            reversed = true;
            break;
        }
        assert!(slow.distance() < end, "slow train cleared the top hat");
    }
    assert!(reversed, "slow train did not roll back");
}

#[test]
fn wooden_twister_train_traverses_top_hat_with_resistance() {
    let (circuit, start, end) = circuit_with_elements(&[PieceId::TopHat]);
    let model = wooden_twister_model();
    let config = PhysicsConfig {
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let initial_speed: f64 = 35.0;
    let mut train = Train::new(&circuit, &model.train, config, start, initial_speed);
    let mut minimum_speed = initial_speed;
    for _ in 0..20_000 {
        train.step(&circuit);
        for car in train.take_readings().into_iter().flatten() {
            if (start..end).contains(&car.distance) {
                assert!(
                    car.g.vertical.is_finite()
                        && car.g.lateral.is_finite()
                        && car.g.longitudinal.is_finite()
                );
            }
        }
        minimum_speed = minimum_speed.min(train.speed());
        if train.distance() >= end {
            break;
        }
    }
    assert!(
        train.distance() >= end,
        "wooden twister train stalled on top hat"
    );
    assert!(minimum_speed < initial_speed);
    assert!(train.speed() > minimum_speed);
    assert!(train.speed() < initial_speed);
}

#[test]
fn ride_test_reports_whether_the_train_clears_the_top_hat() {
    use PieceId::*;
    let mut lifted_elements = vec![LevelToGentleUp];
    lifted_elements.extend([GentleUp; 8]);
    lifted_elements.extend([GentleUpToLevel, Straight, Straight, LevelToGentleDown]);
    lifted_elements.extend([GentleDown; 8]);
    lifted_elements.extend([GentleDownToLevel, TopHat]);
    for (elements, should_complete) in [(vec![TopHat], false), (lifted_elements, true)] {
        let (circuit, _, top_hat_end) = circuit_with_elements(&elements);
        let circuit = if should_complete {
            let mut pieces = circuit.pieces().to_vec();
            for piece in &mut pieces[1..=12] {
                piece.kind = PieceKind::Lift;
            }
            Circuit::new(pieces).unwrap()
        } else {
            circuit
        };
        let top_hat = circuit
            .pieces()
            .iter()
            .find(|piece| piece.id == TopHat)
            .unwrap();
        let top_hat_start = top_hat_end - top_hat.length();
        let mut run = CoasterRun::new(
            &circuit,
            &wooden_twister_model().train,
            PhysicsConfig::default(),
        );
        let outcome = (0..200_000)
            .find_map(|_| run.step(&circuit))
            .expect("ride test did not finish");
        let report = &outcome.reports[0];
        assert_eq!(
            report.completed, should_complete,
            "distance {}, top hat {top_hat_start}..{top_hat_end}, notes {:?}",
            report.furthest_distance, report.notes
        );
        assert_eq!(outcome.recovered, !should_complete);
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.distance >= top_hat_start && sample.distance < top_hat_end),
            "ride test did not reach top hat"
        );
        if should_complete {
            assert!(
                report
                    .samples
                    .iter()
                    .any(|sample| sample.distance >= top_hat_end),
                "ride test did not clear top hat"
            );
        } else {
            assert!(
                report
                    .samples
                    .iter()
                    .all(|sample| sample.distance < top_hat_end),
                "failed ride test reached the top hat exit"
            );
            assert!(
                report
                    .notes
                    .iter()
                    .any(|note| note.contains("could not finish"))
            );
        }
    }
}

#[test]
fn coaster_run_reports_large_loop_overspeed_once() {
    let circuit = circuit_with_gentle_lift_drop_and_loop(PieceId::LargeLoop, 7);
    let outcome = run_one_car_coaster(&circuit, 0.005);

    assert!(!outcome.recovered);
    assert!(outcome.reports[0].completed);
    let speed_band_notes: Vec<_> = outcome.reports[0]
        .notes
        .iter()
        .filter(|note| note.contains("rated speed band"))
        .collect();
    assert_eq!(
        speed_band_notes.len(),
        1,
        "notes={:?}",
        outcome.reports[0].notes
    );
    assert!(speed_band_notes[0].contains("Large vertical loop"));
    assert!(speed_band_notes[0].contains("above its rated speed band"));
    assert!(speed_band_notes[0].contains("31.3 m/s"));
}

#[test]
fn coaster_run_reports_loop_underspeed_when_the_train_stalls_once() {
    let circuit = circuit_with_gentle_lift_drop_and_loop(PieceId::Loop, 3);
    let outcome = run_one_car_coaster(&circuit, 0.06);

    assert!(outcome.recovered, "the train should stall in the loop");
    let speed_band_notes: Vec<_> = outcome.reports[0]
        .notes
        .iter()
        .filter(|note| note.contains("rated speed band"))
        .collect();
    assert_eq!(
        speed_band_notes.len(),
        1,
        "notes={:?}",
        outcome.reports[0].notes
    );
    assert!(speed_band_notes[0].contains("Vertical loop"));
    assert!(speed_band_notes[0].contains("below its rated speed band"));
    assert!(speed_band_notes[0].contains("24.7–27.5 m/s"));
}

fn half_roll_test_circuit(elements: &[PieceId]) -> Circuit {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let templates = catalog();
    let template = |id| templates.iter().find(|piece| piece.id == id).unwrap();
    let mut pieces = Vec::new();
    let mut state = TrackState::origin();
    for id in [PieceId::Station, PieceId::Straight, PieceId::Straight]
        .into_iter()
        .chain(elements.iter().copied())
    {
        let piece = templates
            .iter()
            .filter(|piece| piece.id == id)
            .find_map(|piece| piece.at(state))
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let mut nodes = vec![(state, Vec::<PieceId>::new())];
    let mut queue = BinaryHeap::from([Reverse((0_usize, 0_usize))]);
    let mut visited = HashSet::from([connector_key(state)]);
    let path = loop {
        let Reverse((_, index)) = queue.pop().expect("no return path");
        let (state, path) = nodes[index].clone();
        if state == TrackState::origin() {
            break path;
        }
        if path.len() >= 60 {
            continue;
        }
        for id in [
            PieceId::Straight,
            PieceId::HalfTileStraight,
            PieceId::OneTileStraight,
            PieceId::TurnSmallStandalone,
            PieceId::TurnSmallRightStandalone,
            PieceId::TurnLargeStandalone,
            PieceId::TurnLargeRightStandalone,
            PieceId::OffsetLeft,
            PieceId::OffsetRight,
        ] {
            let next = connector_exit_at(template(id), state).unwrap();
            if next.x.abs() > 300 || next.y.abs() > 300 || !visited.insert(connector_key(next)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            let priority =
                (next.x.unsigned_abs() + next.y.unsigned_abs()) as usize * 100 + next_path.len();
            nodes.push((next, next_path));
            queue.push(Reverse((priority, nodes.len() - 1)));
        }
    };
    for id in path {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

#[test]
fn half_rolls_pass_full_trains_at_both_rated_endpoints() {
    use PieceId::*;
    for (target, elements) in [
        (
            HalfCorkscrewLeftUp,
            [
                HalfLoopUp,
                HalfCorkscrewLeftDown,
                HalfCorkscrewLeftUp,
                HalfLoopDown,
            ],
        ),
        (
            HalfCorkscrewRightUp,
            [
                HalfLoopUp,
                HalfCorkscrewRightDown,
                HalfCorkscrewRightUp,
                HalfLoopDown,
            ],
        ),
        (
            HalfCorkscrewLeftDown,
            [
                HalfLoopUp,
                HalfCorkscrewLeftDown,
                HalfCorkscrewLeftUp,
                HalfLoopDown,
            ],
        ),
        (
            HalfCorkscrewRightDown,
            [
                HalfLoopUp,
                HalfCorkscrewRightDown,
                HalfCorkscrewRightUp,
                HalfLoopDown,
            ],
        ),
        (
            RavenRollLeftUp,
            [HalfLoopUp, RavenRollLeftDown, RavenRollLeftUp, HalfLoopDown],
        ),
        (
            RavenRollRightUp,
            [
                HalfLoopUp,
                RavenRollRightDown,
                RavenRollRightUp,
                HalfLoopDown,
            ],
        ),
        (
            RavenRollLeftDown,
            [HalfLoopUp, RavenRollLeftDown, RavenRollLeftUp, HalfLoopDown],
        ),
        (
            RavenRollRightDown,
            [
                HalfLoopUp,
                RavenRollRightDown,
                RavenRollRightUp,
                HalfLoopDown,
            ],
        ),
    ] {
        let circuit = half_roll_test_circuit(&elements);
        let mut start = 0.0;
        let piece = circuit
            .pieces()
            .iter()
            .find(|piece| {
                if piece.id == target {
                    true
                } else {
                    start += piece.length();
                    false
                }
            })
            .unwrap();
        let end = start + piece.length();
        let rated = piece.rated_speeds.unwrap();
        for model in [wooden_twister_model(), hybrid_model(), multi_launch_model()]
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            for cars in [6, 12] {
                let mut config = model.train.clone();
                config.cars = cars;
                for speed in [rated.minimum_entry, rated.maximum_entry] {
                    let mut train = Train::new(&circuit, &config, unpowered(), start, speed);
                    let mut seen = vec![false; usize::from(cars)];
                    let mut min_vertical = f64::INFINITY;
                    let mut max_vertical = f64::NEG_INFINITY;
                    let mut max_lateral: f64 = 0.0;
                    let mut max_longitudinal: f64 = 0.0;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for (index, reading) in frame.into_iter().enumerate() {
                                if (start..end).contains(&reading.distance) {
                                    seen[index] = true;
                                    min_vertical = min_vertical.min(reading.g.vertical);
                                    max_vertical = max_vertical.max(reading.g.vertical);
                                    max_lateral = max_lateral.max(reading.g.lateral.abs());
                                    max_longitudinal =
                                        max_longitudinal.max(reading.g.longitudinal.abs());
                                }
                            }
                        }
                        if train.distance() >= end + config.length_metres
                            || !train.notes().is_empty()
                        {
                            break;
                        }
                    }
                    if matches!(target, HalfCorkscrewLeftDown | HalfCorkscrewRightDown)
                        && speed == rated.maximum_entry
                    {
                        assert!(
                            train.speed() <= 22.0,
                            "{target:?} {} {cars} cars: {:.6} m/s after clearing",
                            model.name,
                            train.speed()
                        );
                    }
                    let limits = model.comfortable_force_limits;
                    assert!(
                        seen.iter().all(|seen| *seen)
                            && train.distance() >= end + config.length_metres
                            && train.notes().is_empty(),
                        "{target:?} {} {cars} cars at {speed}: incomplete or rollback",
                        model.name
                    );
                    assert!(
                        min_vertical >= limits.minimum_vertical_g
                            && max_vertical <= limits.maximum_vertical_g
                            && max_lateral <= limits.maximum_lateral_g
                            && max_longitudinal <= limits.maximum_longitudinal_g,
                        "{target:?} {} {cars} cars at {speed}: vertical {min_vertical}..{max_vertical}, lateral {max_lateral}, longitudinal {max_longitudinal}; limits {limits:?}",
                        model.name
                    );
                }
            }
        }
    }
}

#[test]
fn banked_drops_pass_full_trains_at_both_rated_endpoints() {
    use PieceId::*;
    for (target, elements) in [
        (
            BankedDropLeft,
            vec![
                BankLeft,
                BankedDropLeft,
                UnbankLeft,
                GentleDownToLevel,
                LevelToGentleUp,
                GentleUp,
                GentleUpToLevel,
            ],
        ),
        (
            BankedDropRight,
            vec![
                BankRight,
                BankedDropRight,
                UnbankRight,
                GentleDownToLevel,
                LevelToGentleUp,
                GentleUp,
                GentleUpToLevel,
            ],
        ),
        (
            TwistedDropLeft,
            vec![
                BankLeft,
                TwistedDropLeft,
                GentleDownToLevel,
                LevelToGentleUp,
                GentleUpToLevel,
            ],
        ),
        (
            TwistedDropRight,
            vec![
                BankRight,
                TwistedDropRight,
                GentleDownToLevel,
                LevelToGentleUp,
                GentleUpToLevel,
            ],
        ),
        (
            BarrelDropLeft,
            vec![
                BankLeft,
                BarrelDropLeft,
                GentleDownToLevel,
                LevelToGentleUp,
                OneTileGentleUp,
                OneTileGentleUp,
                GentleUpToLevel,
            ],
        ),
        (
            BarrelDropRight,
            vec![
                BankRight,
                BarrelDropRight,
                GentleDownToLevel,
                LevelToGentleUp,
                OneTileGentleUp,
                OneTileGentleUp,
                GentleUpToLevel,
            ],
        ),
    ] {
        let circuit = half_roll_test_circuit(&elements);
        let mut start = 0.0;
        let piece = circuit
            .pieces()
            .iter()
            .find(|piece| {
                if piece.id == target {
                    true
                } else {
                    start += piece.length();
                    false
                }
            })
            .unwrap();
        let end = start + piece.length();
        let rated = piece.rated_speeds.unwrap();
        for model in [wooden_twister_model(), hybrid_model(), multi_launch_model()]
            .into_iter()
            .filter(|model| model.allows_piece(piece))
        {
            for cars in [6, 12] {
                let mut config = model.train.clone();
                config.cars = cars;
                for speed in [rated.minimum_entry, rated.maximum_entry] {
                    let mut train = Train::new(&circuit, &config, unpowered(), start, speed);
                    let mut seen = vec![false; usize::from(cars)];
                    let mut min_vertical = f64::INFINITY;
                    let mut max_vertical = f64::NEG_INFINITY;
                    let mut max_lateral: f64 = 0.0;
                    let mut max_longitudinal: f64 = 0.0;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for (index, reading) in frame.into_iter().enumerate() {
                                if (start..end).contains(&reading.distance) {
                                    seen[index] = true;
                                    min_vertical = min_vertical.min(reading.g.vertical);
                                    max_vertical = max_vertical.max(reading.g.vertical);
                                    max_lateral = max_lateral.max(reading.g.lateral.abs());
                                    max_longitudinal =
                                        max_longitudinal.max(reading.g.longitudinal.abs());
                                }
                            }
                        }
                        if train.distance() >= end + config.length_metres
                            || !train.notes().is_empty()
                        {
                            break;
                        }
                    }
                    let limits = model.comfortable_force_limits;
                    eprintln!(
                        "{target:?} {} {cars} cars at {speed}: vertical {min_vertical}..{max_vertical}, lateral {max_lateral}, longitudinal {max_longitudinal}",
                        model.name
                    );
                    assert!(
                        seen.iter().all(|seen| *seen)
                            && train.distance() >= end + config.length_metres
                            && train.notes().is_empty(),
                        "{target:?} {} {cars} cars at {speed}: incomplete or rollback",
                        model.name
                    );
                    assert!(
                        min_vertical >= limits.minimum_vertical_g
                            && max_vertical <= limits.maximum_vertical_g
                            && max_lateral <= limits.maximum_lateral_g
                            && max_longitudinal <= limits.maximum_longitudinal_g,
                        "{target:?} {} {cars} cars at {speed}: vertical {min_vertical}..{max_vertical}, lateral {max_lateral}, longitudinal {max_longitudinal}; limits {limits:?}",
                        model.name
                    );
                }
            }
        }
    }
}

#[test]
fn corkscrews_have_contiguous_rated_windows() {
    use PieceId::*;
    for (target, sequence) in [
        (Corkscrew, vec![Corkscrew]),
        (
            HalfCorkscrewLeftUp,
            vec![HalfCorkscrewLeftUp, HalfCorkscrewLeftDown],
        ),
    ] {
        let (circuit, _, _) = circuit_with_elements(&sequence);
        let index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == target)
            .unwrap();
        let piece = &circuit.pieces()[index];
        let rated = piece.rated_speeds.unwrap();
        let start: f64 = circuit.pieces()[..index].iter().map(Piece::length).sum();
        let end = start + piece.length();
        let passes = |speed: f64| {
            [hybrid_model(), multi_launch_model()].iter().all(|model| {
                [6, 12].into_iter().all(|cars| {
                    let mut configuration = model.train.clone();
                    configuration.cars = cars;
                    let mut train = Train::new(&circuit, &configuration, unpowered(), start, speed);
                    let mut seen = vec![false; usize::from(cars)];
                    let mut minimum_vertical = f64::INFINITY;
                    let mut maximum_vertical = f64::NEG_INFINITY;
                    let mut maximum_lateral: f64 = 0.0;
                    let mut maximum_longitudinal: f64 = 0.0;
                    let mut minimum_speed = f64::INFINITY;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for (car_index, car) in frame.into_iter().enumerate() {
                                if (start..end).contains(&car.distance) {
                                    seen[car_index] = true;
                                    minimum_speed = minimum_speed.min(car.speed);
                                    minimum_vertical = minimum_vertical.min(car.g.vertical);
                                    maximum_vertical = maximum_vertical.max(car.g.vertical);
                                    maximum_lateral = maximum_lateral.max(car.g.lateral.abs());
                                    maximum_longitudinal =
                                        maximum_longitudinal.max(car.g.longitudinal.abs());
                                }
                            }
                        }
                        if train.distance() >= end + configuration.length_metres
                            || train.speed() <= 0.0
                        {
                            break;
                        }
                    }
                    let limits = model.comfortable_force_limits;
                    seen.iter().all(|seen| *seen)
                        && train.distance() >= end + configuration.length_metres
                        && train.notes().is_empty()
                        && (target != HalfCorkscrewLeftUp || minimum_speed >= 5.0)
                        && minimum_vertical >= limits.minimum_vertical_g
                        && maximum_vertical <= limits.maximum_vertical_g
                        && maximum_lateral <= limits.maximum_lateral_g
                        && maximum_longitudinal <= limits.maximum_longitudinal_g
                })
            })
        };
        let passing: Vec<f64> = (4..=60)
            .map(|i| f64::from(i) * 0.5)
            .filter(|speed| passes(*speed))
            .collect();
        eprintln!("{target:?} measured passing speeds at 0.5 m/s intervals: {passing:?}");
        assert!(!passing.is_empty(), "{target:?} has no passing speeds");
        assert!(
            passing
                .windows(2)
                .all(|pair| (pair[1] - pair[0] - 0.5).abs() < 1e-9),
            "{target:?}: {passing:?}"
        );
        assert!(
            rated.maximum_entry - rated.minimum_entry >= 2.0,
            "{target:?}: {rated:?}"
        );
        let sample_count = (rated.maximum_entry - rated.minimum_entry).ceil() as usize;
        for index in 0..=sample_count {
            let speed = (rated.minimum_entry + index as f64).min(rated.maximum_entry);
            assert!(passes(speed), "{target:?} fails at {speed} m/s");
        }
    }
}
