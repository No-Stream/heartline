use super::*;
use crate::track::{PieceId, TrackState, catalog, hybrid_model};

fn circuit(ids: &[PieceId]) -> Circuit {
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in ids {
        let piece = catalog
            .iter()
            .find(|piece| piece.id == *id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

fn oval() -> Circuit {
    use PieceId::*;
    circuit(&[
        Station,
        LevelToGentleUp,
        GentleUpToLevel,
        TurnLargeEntry,
        TurnLargeExit,
        Straight,
        LevelToGentleDown,
        GentleDownToLevel,
        TurnLargeEntry,
        TurnLargeExit,
    ])
}

#[test]
fn circuit_sampling_handles_wrapped_zero_and_exact_piece_joins() {
    let circuit = oval();
    let origin = circuit.sample(0.0);
    let just_before_zero = circuit.sample(-f64::EPSILON);
    let at_circuit_end = circuit.sample(circuit.length());
    assert_eq!(just_before_zero.position, origin.position);
    assert_eq!(just_before_zero.tangent, origin.tangent);
    assert_eq!(just_before_zero.up, origin.up);
    assert_eq!(just_before_zero.curvature, origin.curvature);
    assert_eq!(at_circuit_end.position, origin.position);
    assert_eq!(at_circuit_end.tangent, origin.tangent);

    for piece_index in 0..circuit.pieces.len() {
        let next_piece = (piece_index + 1) % circuit.pieces.len();
        let join = circuit.starts[piece_index] + circuit.pieces[piece_index].length();
        let actual = circuit.sample(join);
        let expected = circuit.pieces[next_piece].sample(0.0);
        assert_eq!(
            actual.position, expected.position,
            "join after piece {piece_index}"
        );
        assert_eq!(
            actual.tangent, expected.tangent,
            "join after piece {piece_index}"
        );
        assert_eq!(actual.up, expected.up, "join after piece {piece_index}");
        assert_eq!(
            actual.curvature, expected.curvature,
            "join after piece {piece_index}"
        );
    }
}

#[test]
fn three_consecutive_large_turns_keep_lateral_g_at_piece_joins() {
    use PieceId::*;

    let circuit = circuit(&[
        TurnLargeEntry,
        TurnLarge,
        TurnLarge,
        TurnLarge,
        TurnLargeExit,
        TurnLargeEntry,
        TurnLarge,
        TurnLargeExit,
        Station,
        TurnLargeEntry,
        TurnLargeExit,
        Straight,
        TurnLargeEntry,
        TurnLargeExit,
    ]);
    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train_configuration = hybrid_model().train;
    train_configuration.cars = 1;
    train_configuration.length_metres = 2.0;
    let first_turn_end = circuit.starts[1];
    let final_join = circuit.starts[5];
    let mut train = Train::new(
        &circuit,
        &train_configuration,
        config,
        first_turn_end - 1.0,
        20.0,
    );
    while train.distance() < final_join + 1.0 {
        train.step(&circuit);
    }

    let mid_turn = rider_g(
        &circuit.pieces[2],
        circuit.pieces[2].length() / 2.0,
        20.0,
        0.0,
    )
    .lateral;
    assert!(mid_turn > 0.5, "mid-turn lateral g={mid_turn}");
    for piece_index in 1..=4 {
        let join = circuit.starts[piece_index];
        let incoming = rider_g(
            &circuit.pieces[piece_index - 1],
            circuit.pieces[piece_index - 1].length(),
            20.0,
            0.0,
        )
        .lateral;
        let outgoing = rider_g(&circuit.pieces[piece_index], 0.0, 20.0, 0.0).lateral;
        let before = train
            .readings()
            .iter()
            .flatten()
            .filter(|reading| reading.distance < join)
            .max_by(|left, right| left.distance.total_cmp(&right.distance))
            .unwrap();
        let after = train
            .readings()
            .iter()
            .flatten()
            .filter(|reading| reading.distance >= join)
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
            .unwrap();

        assert!(join - before.distance < 0.11, "{before:?} around {join}");
        assert!(after.distance - join < 0.11, "{after:?} around {join}");
        assert!(incoming > 0.2, "incoming g={incoming} at join {join}");
        assert!(outgoing > 0.2, "outgoing g={outgoing} at join {join}");
        assert!(
            (incoming - outgoing).abs() < 0.05,
            "incoming g={incoming} versus outgoing g={outgoing} at join {join}"
        );
        assert!(
            (incoming - mid_turn).abs() < 0.1 && (outgoing - mid_turn).abs() < 0.1,
            "join g {incoming}/{outgoing} differs from mid-turn g={mid_turn}"
        );
        assert!(
            (before.g.lateral - mid_turn).abs() < 0.1 && (after.g.lateral - mid_turn).abs() < 0.1,
            "sampled join g {}/{}, mid-turn g={mid_turn}",
            before.g.lateral,
            after.g.lateral
        );
        assert!(
            (before.g.lateral - after.g.lateral).abs() < 0.15,
            "{before:?} versus {after:?} at join {join}"
        );
    }
}

#[test]
fn energy_conservation_and_repeatable_full_lap() {
    let circuit = oval();
    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut first = Train::new(&circuit, &hybrid_model().train, config, 1.0, 20.0);
    let mut second = first.clone();
    let initial = first.mechanical_energy(&circuit);
    for _ in 0..5000 {
        first.step(&circuit);
        second.step(&circuit);
        if first.distance() > circuit.length() + 1.0 {
            break;
        }
    }
    assert!(first.distance() > circuit.length() + 1.0);
    assert!((first.mechanical_energy(&circuit) - initial).abs() / initial.abs() < 0.001);
    assert_eq!(first.state_hash(), second.state_hash());
}

#[test]
fn vertical_loop_g_matches_local_radius() {
    let loop_piece = catalog()
        .into_iter()
        .find(|piece| piece.id == PieceId::Loop)
        .unwrap();
    for distance in [loop_piece.length() * 0.05, loop_piece.length() * 0.5] {
        let sample = loop_piece.sample(distance);
        let g = rider_g(&loop_piece, distance, 20.0, 0.0);
        let expected = sample.up.z + 400.0 * sample.curvature / GRAVITY;
        assert!(
            (g.vertical - expected).abs() < 0.01,
            "{g:?} versus {expected}"
        );
    }
}

#[test]
fn descent_converts_height_to_speed() {
    let circuit = oval();
    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train_config = hybrid_model().train;
    train_config.cars = 1;
    let start = circuit.starts[6];
    let initial_height = circuit.sample(start).position.z;
    let initial_speed: f64 = 10.0;
    let mut train = Train::new(&circuit, &train_config, config, start, initial_speed);
    while train.distance() < circuit.starts[8] {
        train.step(&circuit);
    }
    let drop = initial_height - circuit.sample(train.distance()).position.z;
    let expected = (initial_speed.powi(2) + 2.0 * GRAVITY * drop).sqrt();
    assert!(drop > 7.9);
    assert!((train.speed() - expected).abs() < 0.01);
}

#[test]
fn stalled_train_rolls_back_and_is_recovered() {
    let circuit = oval();
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        17.0,
        1.0,
    );
    let mut reversed = false;
    for _ in 0..10000 {
        train.step(&circuit);
        reversed |= train.speed() < 0.0;
        if !train.notes().is_empty() {
            break;
        }
    }
    assert!(reversed);
    assert_eq!(train.notes(), &[Note::RecoveredAfterRollback]);
    assert!(
        train
            .readings()
            .last()
            .unwrap()
            .iter()
            .all(|reading| reading.g.longitudinal.abs() < 2.0)
    );
    assert_eq!(train.speed(), 0.0);
}

#[test]
fn station_drives_and_launch_accelerates() {
    let circuit = oval();
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        5.0,
        0.0,
    );
    for _ in 0..100 {
        train.step(&circuit);
    }
    assert!(train.speed() > 0.5);
    let mut launched = Train::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        5.0,
        0.0,
    );
    launched.set_control(
        0,
        Some(SectionControl::Launch {
            target_speed: 10.0,
            acceleration: 5.0,
        }),
    );
    for _ in 0..100 {
        launched.step(&circuit);
    }
    assert!(launched.speed() > train.speed());
}

#[test]
fn stationary_train_on_flat_track_remains_at_rest_without_power() {
    let circuit = oval();
    let config = PhysicsConfig {
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(&circuit, &hybrid_model().train, config, 10.0, 0.0);
    for _ in 0..100 {
        train.step(&circuit);
    }
    assert_eq!(train.speed(), 0.0);
    assert!(train.notes().is_empty());
}

#[cfg(feature = "threads")]
#[test]
fn independent_train_results_match_across_thread_counts() {
    use crate::{Executor, SequentialExecutor, ThreadedExecutor};
    let circuit = oval();
    let run = |index: usize| {
        let mut train = Train::new(
            &circuit,
            &hybrid_model().train,
            PhysicsConfig::default(),
            5.0,
            10.0 + index as f64,
        );
        for _ in 0..500 {
            train.step(&circuit);
        }
        train.state_hash()
    };
    let sequential = SequentialExecutor::new(3).execute(8, run);
    assert_eq!(sequential, ThreadedExecutor::new(1, 3).execute(8, run));
    assert_eq!(sequential, ThreadedExecutor::new(4, 2).execute(8, run));
}

#[test]
fn static_friction_holds_train_on_shallow_slope_without_drift() {
    let circuit = oval();
    let config = PhysicsConfig {
        rolling_resistance: 0.8,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let initial_distance = circuit.starts[2] + 5.0;
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        config,
        initial_distance,
        0.0,
    );
    for _ in 0..100 {
        train.step(&circuit);
    }
    assert_eq!(train.speed(), 0.0);
    assert_eq!(train.distance(), initial_distance);
    assert!(train.notes().is_empty());
}

#[test]
fn friction_stops_train_on_slope_and_recovers_it() {
    let circuit = oval();
    let config = PhysicsConfig {
        rolling_resistance: 0.8,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        config,
        circuit.starts[2] + 5.0,
        0.1,
    );
    for _ in 0..100 {
        train.step(&circuit);
        if !train.notes().is_empty() {
            break;
        }
    }
    assert_eq!(train.speed(), 0.0);
    assert_eq!(train.notes(), &[Note::RecoveredAfterRollback]);
}

#[test]
fn demo_circuit_is_closed_and_train_readings_can_be_cleared() {
    let circuit = demo_circuit();
    assert!(
        circuit
            .pieces()
            .iter()
            .any(|piece| piece.id == PieceId::Lift)
    );
    assert!(
        circuit
            .pieces()
            .iter()
            .any(|piece| piece.id == PieceId::Loop)
    );
    assert_eq!(circuit.pieces()[1].kind, PieceKind::Lift);
    assert_eq!(circuit.pieces()[7].kind, PieceKind::Lift);
    assert_eq!(circuit.pieces()[8].kind, PieceKind::Lift);
    assert_eq!(
        circuit.pieces().last().unwrap().exit,
        circuit.pieces()[0].entry
    );
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        0.0,
        3.0,
    );
    train.step(&circuit);
    assert!(!train.readings().is_empty());
    assert!(!train.take_readings().is_empty());
    assert!(train.readings().is_empty());
}

#[test]
fn demo_circuit_crosses_the_demo_path() {
    let circuit = demo_circuit();
    let crossing = (0..=2000)
        .map(|index| {
            circuit
                .sample(circuit.length() * f64::from(index) / 2000.0)
                .position
        })
        .find(|position| {
            (1024.0..1028.0).contains(&position.y) && (872.0..1120.0).contains(&position.x)
        });
    assert!(
        crossing.is_some(),
        "demo track must cross the path at y=256 tiles"
    );
    let crossing = crossing.unwrap();
    let world = crate::demo_world::demo_world();
    let path = world
        .paths()
        .find(|path| {
            path.position.x == (crossing.x / 4.0).floor() as u16
                && path.position.y == (crossing.y / 4.0).floor() as u16
        })
        .expect("crossing must be above a path");
    assert!(crossing.z >= f64::from(path.position.height) + 3.0);
}

#[test]
fn demo_has_track_crossing_with_vertical_clearance() {
    let circuit = demo_circuit();
    let points: Vec<_> = (0..2000)
        .map(|index| {
            circuit
                .sample(circuit.length() * f64::from(index) / 2000.0)
                .position
        })
        .collect();
    let crossing = points.iter().enumerate().find_map(|(index, point)| {
        points
            .iter()
            .enumerate()
            .skip(index + 20)
            .find(|(_, other)| {
                (point.x - other.x).hypot(point.y - other.y) < 1.0
                    && (point.z - other.z).abs() >= 4.0
            })
            .map(|(other_index, other)| (index, other_index, *point, *other))
    });
    assert!(
        crossing.is_some(),
        "demo has no track crossing with vertical clearance"
    );
}
