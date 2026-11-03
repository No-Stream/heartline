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
        TurnLarge,
        TurnLarge,
        Straight,
        LevelToGentleDown,
        GentleDownToLevel,
        TurnLarge,
        TurnLarge,
    ])
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
