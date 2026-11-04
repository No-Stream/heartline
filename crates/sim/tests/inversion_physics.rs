use std::collections::{HashSet, VecDeque};
use theme_park_sim::physics::{Circuit, CoasterRun, GRAVITY, PhysicsConfig, Train};
use theme_park_sim::track::{
    Piece, PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model,
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
    let mut visited = HashSet::new();
    let return_path = loop {
        let (current, path) = queue.pop_front().expect("no return path");
        if current == start {
            break path;
        }
        if path.len() >= 80 || !visited.insert(format!("{current:?}")) {
            continue;
        }
        for id in [
            PieceId::Straight,
            PieceId::TurnSmallStandalone,
            PieceId::TurnSmallRightStandalone,
        ] {
            let piece = template(id).at(current).unwrap();
            if piece.exit.x.abs() > 300 || piece.exit.y.abs() > 300 {
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
                HalfCorkscrewLeftDown,
                HalfCorkscrewRightUp,
                HalfLoopDown,
            ],
        ),
        (
            "sea serpent right",
            [
                HalfLoopUp,
                HalfCorkscrewRightDown,
                HalfCorkscrewLeftUp,
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
