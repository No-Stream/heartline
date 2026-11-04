use std::collections::{HashSet, VecDeque};
use theme_park_sim::physics::{Circuit, CoasterRun, GRAVITY, PhysicsConfig, Train};
use theme_park_sim::track::{
    Bank, Piece, PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model,
    wooden_twister_model,
};

fn circuit_with_elements(elements: &[PieceId]) -> (Circuit, f64, f64) {
    let catalog = catalog();
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
    for &id in elements {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let element_end = pieces.iter().map(Piece::length).sum();

    // Find a return path using level track, keeping the element sequence intact.
    let mut queue = VecDeque::from([(state, Vec::<PieceId>::new())]);
    let mut visited = HashSet::from([format!("{state:?}")]);
    let return_path = loop {
        let (current, path) = queue.pop_front().expect("no return path");
        if current == start {
            break path;
        }
        if path.len() >= 80 {
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
            let piece = template(id).at(current).unwrap();
            if piece.exit.x.abs() > 300 || piece.exit.y.abs() > 300 {
                continue;
            }
            if !visited.insert(format!("{:?}", piece.exit)) {
                continue;
            }
            let mut next = path.clone();
            next.push(id);
            queue.push_back((piece.exit, next));
        }
    };
    for id in return_path {
        let piece = template(id).at(state).unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    (Circuit::new(pieces).unwrap(), element_start, element_end)
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
fn sixty_degree_crest_ratings_pass_full_train_scan() {
    use PieceId::*;
    let mut results = Vec::new();
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
            SixtyUpToLevel,
            LevelToSixtyDown,
            SixtyToSteepUp,
            SteepToSixtyDown,
        ] {
            let Some(index) = circuit.pieces().iter().position(|piece| piece.id == id) else {
                continue;
            };
            let start: f64 = circuit.pieces()[..index].iter().map(Piece::length).sum();
            let end = start + circuit.pieces()[index].length();
            let mut passing = Vec::new();
            for speed in 2..=30 {
                let mut passed = true;
                for cars in [6, 12] {
                    let mut config = hybrid_model().train;
                    config.cars = cars;
                    let mut train = Train::new(&circuit, &config, unpowered(), start, speed as f64);
                    let mut minimum_vertical = f64::INFINITY;
                    for _ in 0..100_000 {
                        train.step(&circuit);
                        for frame in train.take_readings() {
                            for car in frame {
                                if (start..end).contains(&car.distance) {
                                    minimum_vertical = minimum_vertical.min(car.g.vertical);
                                }
                            }
                        }
                        if train.distance() >= end + config.length_metres || train.speed() <= 0.0 {
                            break;
                        }
                    }
                    if train.distance() < end + config.length_metres || minimum_vertical < -1.2 {
                        if id == SixtyUpToLevel {
                            eprintln!("{id:?} {cars} cars at {speed}: minimum vertical {minimum_vertical:.3}, progress {:.2}/{:.2}, speed {:.2}", train.distance() - start, end - start + config.length_metres, train.speed());
                        }
                        passed = false;
                        break;
                    }
                }
                if passed { passing.push(speed); }
            }
            eprintln!("{id:?}: passing speeds {passing:?}");
            results.push((id, passing.last().copied(), circuit.pieces()[index].rated_speeds));
        }
    }
    for (id, maximum, rated) in results {
        let maximum = maximum.expect("no traversable speed meets the crest force target");
        assert_eq!(rated.unwrap().maximum_entry, maximum as f64, "{id:?}");
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
fn all_constructible_half_inversion_combinations_stay_within_force_limits() {
    struct ElementForces {
        id: PieceId,
        minimum_vertical: f64,
        maximum_vertical: f64,
        maximum_lateral: f64,
        maximum_longitudinal: f64,
        cars_seen: Vec<bool>,
        minimum_entry_speed: f64,
        maximum_entry_speed: f64,
    }
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
    let mut failures = Vec::new();
    for elements in combinations {
        let (circuit, element_start, element_end) = circuit_with_elements(&elements);
        let element_ranges: Vec<_> = circuit
            .pieces()
            .iter()
            .scan(0.0, |distance, piece| {
                let start = *distance;
                *distance += piece.length();
                Some((piece, start..*distance))
            })
            .filter(|(piece, _)| piece.inversion.is_some())
            .map(|(piece, range)| (piece.id, range))
            .collect();
        for model in [wooden_twister_model(), hybrid_model(), multi_launch_model()] {
            if elements.iter().any(|id| {
                let piece = catalog().into_iter().find(|piece| piece.id == *id).unwrap();
                !model.allows_piece(&piece)
            }) {
                continue;
            }
            for initial_element_speed in [27.0, 30.0, 32.0] {
                let mut train = Train::new(
                    &circuit,
                    &model.train,
                    unpowered(),
                    element_start,
                    initial_element_speed,
                );
                let mut element_forces: Vec<_> = element_ranges
                    .iter()
                    .map(|(id, _)| ElementForces {
                        id: *id,
                        minimum_vertical: f64::INFINITY,
                        maximum_vertical: f64::NEG_INFINITY,
                        maximum_lateral: 0.0,
                        maximum_longitudinal: 0.0,
                        cars_seen: vec![false; usize::from(model.train.cars)],
                        minimum_entry_speed: f64::INFINITY,
                        maximum_entry_speed: f64::NEG_INFINITY,
                    })
                    .collect();
                let mut cleared = false;
                let mut rolled_back = false;
                for _ in 0..100_000 {
                    train.step(&circuit);
                    for frame in train.take_readings() {
                        for (car_index, reading) in frame.into_iter().enumerate() {
                            if let Some(range_index) = element_ranges
                                .iter()
                                .position(|(_, range)| range.contains(&reading.distance))
                            {
                                let forces = &mut element_forces[range_index];
                                if !forces.cars_seen[car_index] {
                                    forces.minimum_entry_speed =
                                        forces.minimum_entry_speed.min(reading.speed);
                                    forces.maximum_entry_speed =
                                        forces.maximum_entry_speed.max(reading.speed);
                                    forces.cars_seen[car_index] = true;
                                }
                                forces.minimum_vertical =
                                    forces.minimum_vertical.min(reading.g.vertical);
                                forces.maximum_vertical =
                                    forces.maximum_vertical.max(reading.g.vertical);
                                forces.maximum_lateral =
                                    forces.maximum_lateral.max(reading.g.lateral.abs());
                                forces.maximum_longitudinal = forces
                                    .maximum_longitudinal
                                    .max(reading.g.longitudinal.abs());
                            }
                        }
                    }
                    if train.distance() >= element_end + model.train.length_metres {
                        cleared = true;
                        break;
                    }
                    rolled_back = !train.notes().is_empty();
                    if rolled_back {
                        break;
                    }
                }
                let limits = model.comfortable_force_limits;
                for forces in element_forces {
                    let all_cars_seen = forces.cars_seen.iter().all(|seen| *seen);
                    let within_limits = forces.minimum_vertical >= limits.minimum_vertical_g
                        && forces.maximum_vertical <= limits.maximum_vertical_g
                        && forces.maximum_lateral <= limits.maximum_lateral_g
                        && forces.maximum_longitudinal <= limits.maximum_longitudinal_g;
                    if !all_cars_seen || !cleared || rolled_back || !within_limits {
                        failures.push(format!(
                            "{:?} {} starting at {initial_element_speed} m/s: observed entry speed {:.2}..{:.2} m/s, vertical {:.2}..{:.2}, lateral {:.2}, longitudinal {:.2}, cars {}/{}, cleared {cleared}, rollback {rolled_back}; limits {limits:?}",
                            forces.id,
                            model.name,
                            forces.minimum_entry_speed,
                            forces.maximum_entry_speed,
                            forces.minimum_vertical,
                            forces.maximum_vertical,
                            forces.maximum_lateral,
                            forces.maximum_longitudinal,
                            forces.cars_seen.iter().filter(|seen| **seen).count(),
                            forces.cars_seen.len()
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn full_trains_cross_loop_profiles_within_their_rated_force_limits() {
    use PieceId::*;
    for (model, piece_id) in [
        (wooden_twister_model(), SteelLoop),
        (hybrid_model(), Loop),
        (hybrid_model(), LargeLoop),
        (multi_launch_model(), Loop),
        (multi_launch_model(), LargeLoop),
    ] {
        let piece = catalog()
            .into_iter()
            .find(|piece| piece.id == piece_id)
            .unwrap();
        let rated_speeds = piece.rated_speeds.unwrap();
        let (circuit, element_start, element_end) = circuit_with_elements(&[piece_id]);
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
fn train_completes_laps_through_combined_half_inversions() {
    use PieceId::*;
    for (name, elements) in [
        (
            "Immelmann left",
            [
                HalfLoopUp,
                HalfCorkscrewLeftDown,
                HalfCorkscrewLeftUp,
                HalfLoopDown,
            ],
        ),
        (
            "Immelmann right",
            [
                HalfLoopUp,
                HalfCorkscrewRightDown,
                HalfCorkscrewRightUp,
                HalfLoopDown,
            ],
        ),
        (
            "sea serpent left",
            [
                HalfLoopUp,
                RavenRollLeftDown,
                RavenRollRightUp,
                HalfLoopDown,
            ],
        ),
        (
            "sea serpent right",
            [
                HalfLoopUp,
                RavenRollRightDown,
                RavenRollLeftUp,
                HalfLoopDown,
            ],
        ),
        (
            "dive loop left",
            [
                HalfCorkscrewLeftUp,
                HalfLoopDown,
                HalfLoopUp,
                HalfCorkscrewLeftDown,
            ],
        ),
        (
            "dive loop right",
            [
                HalfCorkscrewRightUp,
                HalfLoopDown,
                HalfLoopUp,
                HalfCorkscrewRightDown,
            ],
        ),
    ] {
        let (circuit, element_start, element_end) = circuit_with_elements(&elements);
        for model in [hybrid_model(), multi_launch_model()] {
            let config = PhysicsConfig {
                powered_sections: false,
                ..PhysicsConfig::default()
            };
            let mut train = Train::new(&circuit, &model.train, config, 0.0, 50.0);
            let mut inverted = false;
            for _ in 0..100_000 {
                train.step(&circuit);
                for car in train.take_readings().into_iter().flatten() {
                    if (element_start..element_end).contains(&car.distance) {
                        inverted |= car.inverted;
                        assert!(
                            car.g.vertical.is_finite()
                                && car.g.lateral.is_finite()
                                && car.g.longitudinal.is_finite(),
                            "{name} {}: non-finite forces",
                            model.name
                        );
                    }
                }
                if train.distance() >= circuit.length() {
                    break;
                }
            }
            assert!(
                train.distance() >= circuit.length(),
                "{name} {}: stalled at {} of {}",
                model.name,
                train.distance(),
                circuit.length()
            );
            assert!(inverted, "{name} {}: no inverted rider", model.name);
        }
        let mut configuration = multi_launch_model().train;
        configuration.cars = 1;
        let mut train = Train::new(&circuit, &configuration, unpowered(), 0.0, 50.0);
        let initial_energy = train.mechanical_energy(&circuit);
        let mut inverted = false;
        for _ in 0..100_000 {
            train.step(&circuit);
            if (element_start..element_end).contains(&train.distance()) {
                inverted |= train.readings().last().unwrap()[0].inverted;
            }
            if train.distance() >= circuit.length() {
                break;
            }
        }
        assert!(
            train.distance() >= circuit.length(),
            "{name}: stalled at {} of {}",
            train.distance(),
            circuit.length()
        );
        assert!(inverted, "{name}: never recorded an inverted rider");
        assert!(
            (train.mechanical_energy(&circuit) - initial_energy).abs() / initial_energy.abs()
                < 0.002,
            "{name}: energy changed"
        );
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
