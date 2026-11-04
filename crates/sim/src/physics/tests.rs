use super::*;
use crate::track::construction::{Construction, track_support_obstacles};
use crate::track::supports::generate_dense_wooden_lattice;
use crate::track::wooden_twister_model;
use crate::track::{PieceId, TrackState, catalog, hybrid_model};
use crate::world::{Surface, World};

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
fn wooden_twister_completes_a_lap_through_its_structure() {
    use PieceId::*;

    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let model = wooden_twister_model();
    let start = TrackState {
        x: 256,
        y: 256,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model.clone());
    let mut ids = vec![Station, LevelToGentleUp];
    ids.extend([GentleUp; 5]);
    ids.extend([GentleUpToLevel, Straight, TurnSmallStandalone]);
    ids.extend([LevelToGentleDown, GentleDown, GentleDownToLevel, Straight]);
    ids.push(TurnSmallStandalone);
    ids.extend([
        LevelToGentleDown,
        GentleDown,
        GentleDownToLevel,
        Straight,
        Straight,
        Straight,
    ]);
    ids.push(TurnSmallStandalone);
    ids.extend([Straight; 7]);
    ids.extend([Straight; 4]);
    ids.extend([
        LevelToGentleDown,
        GentleDown,
        GentleDownToLevel,
        TurnSmallRightStandalone,
    ]);
    ids.extend([Straight; 4]);
    ids.push(TurnSmallRightStandalone);
    ids.extend([Straight; 8]);
    ids.push(TurnSmallRightStandalone);
    ids.extend([Straight; 3]);
    for (index, id) in ids.into_iter().enumerate() {
        construction.set_chain((1..=8).contains(&index));
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| panic!("{id:?}: {error:?}"));
    }
    construction.close().unwrap();
    let circuit = construction.circuit().unwrap();
    let crossing_low = circuit.pieces()[1]
        .sample(circuit.pieces()[1].length())
        .position;
    let crossing_high = circuit.pieces()[26]
        .sample(circuit.pieces()[26].length())
        .position;
    assert!((crossing_low.x - crossing_high.x).hypot(crossing_low.y - crossing_high.y) < 0.01);
    assert!(crossing_high.z - crossing_low.z > model.train.height_metres);
    let obstacles = track_support_obstacles(&circuit.pieces()[1..3], &model.train);
    let supports: Vec<_> = circuit.pieces()[26..28]
        .iter()
        .flat_map(|piece| generate_dense_wooden_lattice(piece, &world, &obstacles).unwrap())
        .collect();
    for direction in [-1.0, 1.0] {
        assert!(supports.iter().any(|segment| {
            segment.start.x == segment.end.x
                && segment.start.y == segment.end.y
                && (segment.start.x - crossing_low.x).abs() < 4.0
                && (0.0..4.1).contains(&((segment.start.y - crossing_low.y) * direction))
                && segment.start.z < crossing_low.z
                && segment.end.z > crossing_low.z + model.train.height_metres
        }));
    }
    for segment in &supports {
        for step in 0..=100 {
            let point = segment.start + (segment.end - segment.start) * (f64::from(step) / 100.0);
            assert!(obstacles.iter().all(|obstacle| {
                point.x < obstacle.min.x
                    || point.x > obstacle.max.x
                    || point.y < obstacle.min.y
                    || point.y > obstacle.max.y
                    || point.z < obstacle.min.z
                    || point.z > obstacle.max.z
            }));
        }
    }

    let mut run = CoasterRun::new(&circuit, &model.train, PhysicsConfig::default());
    let outcome = (0..200_000)
        .find_map(|_| run.step(&circuit))
        .expect("train completed a lap");
    assert!(
        outcome.reports[0].completed,
        "distance {} of {}",
        run.train().distance(),
        circuit.length()
    );
    assert!(!outcome.recovered);
}

#[test]
fn constructed_launches_accelerate_twice_during_a_complete_lap() {
    use PieceId::*;
    let circuit = circuit(&[
        Station,
        LaunchLong,
        LaunchLong,
        LaunchLong,
        Launch,
        TurnLargeEntry,
        TurnLargeExit,
        LaunchLong,
        LaunchLong,
        LaunchLong,
        LaunchLong,
        TurnLargeEntry,
        TurnLargeExit,
    ]);
    let model = crate::track::multi_launch_model();
    let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), 0.0, 2.0);
    let target = model.train.launch.unwrap().target_speed;
    for (launch_index, end_index) in [(1, 5), (7, 11)] {
        for _ in 0..100_000 {
            if train.distance() >= circuit.starts[launch_index] {
                break;
            }
            train.step(&circuit);
        }
        assert!(train.distance() >= circuit.starts[launch_index]);
        let before = train.speed();
        for _ in 0..100_000 {
            if train.distance() >= circuit.starts[end_index] - 1.0 {
                break;
            }
            train.step(&circuit);
        }
        assert!(train.distance() >= circuit.starts[end_index] - 1.0);
        assert!(train.speed() > before);
        assert!(
            (train.speed() - target).abs() < 0.05,
            "speed {} target {target}",
            train.speed()
        );
    }
    for _ in 0..100_000 {
        if train.distance() >= circuit.length() {
            break;
        }
        train.step(&circuit);
    }
    assert!(train.distance() >= circuit.length());
    assert!(train.notes().is_empty());
}

#[test]
fn launch_acceleration_never_exceeds_the_technology_limit() {
    use PieceId::*;
    let circuit = circuit(&[
        Station,
        Launch,
        TurnLargeEntry,
        TurnLargeExit,
        Straight,
        Straight,
        TurnLargeEntry,
        TurnLargeExit,
    ]);
    let mut train_configuration = crate::track::multi_launch_model().train;
    train_configuration.launch.as_mut().unwrap().target_speed =
        crate::track::LAUNCH_MAX_SPEED_METRES_PER_SECOND;
    let mut train = Train::new(
        &circuit,
        &train_configuration,
        PhysicsConfig::default(),
        circuit.starts[1] + 1.0,
        2.0,
    );
    let initial_speed = train.speed();
    train.step(&circuit);
    let launch_acceleration = (train.speed() - initial_speed) / TIME_STEP;
    assert!(launch_acceleration <= crate::track::LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED);
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
fn vertical_loop_g_uses_curvature_projected_onto_rider_up() {
    let loop_piece = catalog()
        .into_iter()
        .find(|piece| piece.id == PieceId::Loop)
        .unwrap();
    for distance in [loop_piece.length() * 0.05, loop_piece.length() * 0.5] {
        let sample = loop_piece.sample(distance);
        let delta = 0.01;
        let before_distance = distance - delta;
        let after_distance = distance + delta;
        let tangent_before = loop_piece.sample(before_distance).tangent;
        let tangent_after = loop_piece.sample(after_distance).tangent;
        let curvature_along_rider_up =
            (tangent_after - tangent_before).dot(sample.up) / (after_distance - before_distance);
        assert!(curvature_along_rider_up > 0.0);
        let expected = sample.up.z + 20.0_f64.powi(2) * curvature_along_rider_up / GRAVITY;
        let g = rider_g(&loop_piece, distance, 20.0, 0.0);
        assert!(
            (g.vertical - expected).abs() < 0.01,
            "{g:?} versus {expected}"
        );
    }
}

#[test]
fn banking_reduces_lateral_g_for_left_and_right_turns_at_the_same_speed() {
    use PieceId::*;
    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let mut train_config = hybrid_model().train;
    train_config.cars = 1;
    for (direction, turn, banked_turn, bank, unbank) in [
        (
            "left",
            TurnSmallStandalone,
            BankedTurnStandalone,
            BankLeft,
            UnbankLeft,
        ),
        (
            "right",
            TurnSmallRightStandalone,
            SmallRightBankedTurnStandalone,
            BankRight,
            UnbankRight,
        ),
    ] {
        let measure = |turn_id, bank_id, unbank_id| {
            let circuit = circuit(&[
                Station, bank_id, turn_id, turn_id, unbank_id, Straight, bank_id, turn_id, turn_id,
                unbank_id,
            ]);
            let midpoint = circuit.starts[2] + circuit.pieces[2].length() / 2.0;
            let mut train = Train::new(&circuit, &train_config, config, midpoint - 1.0, 20.0);
            while train.distance() < midpoint {
                train.step(&circuit);
            }
            let reading = train.readings().last().unwrap()[0];
            assert!((reading.speed - 20.0).abs() < 1e-8);
            reading.g.lateral.abs()
        };
        let unbanked_g = measure(turn, Straight, Straight);
        let banked_g = measure(banked_turn, bank, unbank);
        assert!(
            banked_g < unbanked_g,
            "{direction}: banked {banked_g}, unbanked {unbanked_g}"
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
    use crate::executor::{Executor, SequentialExecutor, ThreadedExecutor};
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
            .any(|piece| piece.id == PieceId::GentleUp)
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

#[test]
fn coaster_run_starts_at_station_completes_and_records_forces() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let mut run = super::CoasterRun::new(
        &circuit,
        &configuration,
        super::PhysicsConfig {
            lift_speed: 5.0,
            ..super::PhysicsConfig::default()
        },
    );
    assert_eq!(
        run.capacity(),
        usize::from(configuration.cars)
            * usize::from(configuration.rows_per_car)
            * usize::from(configuration.seats_per_row)
    );
    assert_eq!(
        circuit.sample(run.train().distance()).position,
        circuit.pieces()[0]
            .sample(circuit.pieces()[0].length() / 2.0)
            .position
    );
    let mut outcome = None;
    for _ in 0..200_000 {
        if let Some(result) = run.step(&circuit) {
            outcome = Some(result);
            break;
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        panic!(
            "train did not finish: distance={} speed={}",
            run.train().distance(),
            run.train().speed()
        )
    });
    assert!(outcome.reports[0].completed);
    assert!(!outcome.recovered);
    assert!(!outcome.reports[0].samples.is_empty());
    assert!(outcome.reports[0].intensity > 0.0);
    assert_eq!(outcome.reports.len(), usize::from(configuration.cars));
    assert_ne!(outcome.reports[0].intensity, outcome.reports[1].intensity);
}

#[test]
fn coaster_run_can_recover_without_a_completed_ride() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let mut run = super::CoasterRun::new(&circuit, &configuration, super::PhysicsConfig::default());
    let outcome = run.recover();
    assert!(outcome.recovered);
    assert!(!outcome.reports[0].completed);
    assert_eq!(run.train().speed(), 0.0);
    assert_eq!(
        circuit.sample(run.train().distance()).position,
        circuit.pieces()[0]
            .sample(circuit.pieces()[0].length() / 2.0)
            .position
    );
}

#[test]
fn coaster_run_recovers_a_train_that_cannot_leave_the_station() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let mut run = super::CoasterRun::new(
        &circuit,
        &configuration,
        super::PhysicsConfig {
            powered_sections: false,
            ..super::PhysicsConfig::default()
        },
    );
    let outcome = (0..1000)
        .find_map(|_| run.step(&circuit))
        .expect("stationary train is recovered");
    assert!(outcome.recovered);
    assert!(outcome.reports.iter().all(|report| !report.completed));
}

#[test]
fn block_operation_keeps_trains_separate_and_completes_runs() {
    let circuit = circuit(&[
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ]);
    let mut pieces = circuit.pieces().to_vec();
    for index in [3, 10] {
        pieces[index].kind = PieceKind::Lift;
    }
    let circuit = Circuit::new(pieces).unwrap();
    let configuration = crate::track::hybrid_model().train;
    let boundary = |end: usize| {
        circuit.pieces()[..end]
            .iter()
            .map(|piece| piece.length())
            .sum()
    };
    let layout = crate::track::blocks::BlockLayout::new(
        &circuit,
        &configuration,
        super::PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        },
        vec![boundary(2), boundary(6), boundary(10), boundary(13)],
    )
    .unwrap();
    let mut operation = super::BlockOperation::new(layout);
    let mut dispatched = 0;
    let mut completed = 0;
    let mut maximum_concurrent = 0;
    for _ in 0..200_000 {
        if dispatched < 3 && operation.can_dispatch() {
            operation.dispatch();
            dispatched += 1;
        }
        for (_, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            completed += 1;
        }
        let positions: Vec<_> = operation
            .trains()
            .map(|(_, train)| train.distance())
            .collect();
        maximum_concurrent = maximum_concurrent.max(positions.len());
        for (index, &distance) in positions.iter().enumerate() {
            for &other in &positions[index + 1..] {
                let separation = (distance - other).rem_euclid(circuit.length());
                assert!(
                    separation >= configuration.length_metres
                        && circuit.length() - separation >= configuration.length_metres
                );
            }
        }
        if completed == 3 {
            break;
        }
    }
    assert_eq!(completed, 3);
    assert!(maximum_concurrent >= 2);
}

#[test]
fn optimized_acceleration_matches_full_track_samples() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let config = super::PhysicsConfig::default();
    let train = super::Train::new(&circuit, &configuration, config, 0.0, 0.0);
    for &start in &circuit.starts {
        for offset in [0.0, 1.0, 5.0] {
            let distance = start + offset;
            let mean_slope = (0..usize::from(configuration.cars))
                .map(|car| {
                    circuit
                        .sample(distance - car as f64 * train.spacing)
                        .tangent
                        .z
                })
                .sum::<f64>()
                / f64::from(configuration.cars);
            for speed in [-10.0_f64, 0.0, 2.0, 20.0] {
                let rolling = if speed == 0.0 {
                    0.0
                } else {
                    config.rolling_resistance * super::GRAVITY * speed.signum()
                };
                let drag = config.drag_coefficient * speed * speed.abs()
                    / (config.car_mass_kg * f64::from(configuration.cars));
                let original = -super::GRAVITY * mean_slope - rolling - drag;
                let actual = train.acceleration_with_gravity(
                    train.gravity_acceleration(&circuit, distance),
                    speed,
                );
                assert_eq!(actual.to_bits(), original.to_bits());
            }
        }
    }
}

#[test]
fn train_motion_matches_with_and_without_rider_readings() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let config = super::PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let mut recorded = super::Train::new(&circuit, &configuration, config, 2.0, 1.0);
    let mut unrecorded = recorded.clone();
    for step in 0..30_000 {
        let hold = step < 1_000;
        recorded.step_block_brake(&circuit, &[0], hold);
        unrecorded.step_block_brake_without_readings(&circuit, &[0], hold);
        assert_eq!(recorded.distance(), unrecorded.distance());
        assert_eq!(recorded.speed(), unrecorded.speed());
        assert_eq!(recorded.notes(), unrecorded.notes());
        recorded.take_readings();
        assert!(unrecorded.readings().is_empty());
    }
}

#[test]
fn block_brake_holds_and_restarts_a_train() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let mut train = super::Train::new(
        &circuit,
        &configuration,
        super::PhysicsConfig::default(),
        2.0,
        1.0,
    );
    for _ in 0..3000 {
        train.step_block_brake(&circuit, &[0], true);
        train.take_readings();
    }
    assert_eq!(train.speed(), 0.0);
    let held_distance = train.distance();
    for _ in 0..100 {
        train.step_block_brake(&circuit, &[0], false);
        train.take_readings();
    }
    assert!(train.speed() > 0.0);
    assert!(train.distance() > held_distance);
}

#[test]
fn block_brake_does_not_creep_while_held_on_a_downhill_piece() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let piece_index = circuit
        .pieces()
        .iter()
        .position(|piece| piece.id == crate::track::PieceId::GentleDown)
        .unwrap();
    let distance = circuit.starts[piece_index] + circuit.pieces()[piece_index].length() / 2.0;
    let mut train = super::Train::new(
        &circuit,
        &configuration,
        super::PhysicsConfig {
            braking_acceleration: 6.0,
            ..Default::default()
        },
        distance,
        0.0,
    );
    for _ in 0..10_000 {
        train.step_block_brake(&circuit, &[piece_index], true);
        train.take_readings();
    }
    assert_eq!(train.distance(), distance);
    assert_eq!(train.speed(), 0.0);
}

#[test]
fn downhill_brake_stops_holds_and_restarts_without_recovery() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let piece_index = circuit
        .pieces()
        .iter()
        .enumerate()
        .filter(|(_, piece)| piece.id == crate::track::PieceId::GentleDown)
        .nth(2)
        .unwrap()
        .0;
    let distance = circuit.starts[piece_index] + circuit.pieces()[piece_index].length() / 2.0;
    let config = super::PhysicsConfig {
        braking_acceleration: 6.0,
        drive_acceleration: 6.0,
        ..Default::default()
    };
    let mut train = super::Train::new(&circuit, &configuration, config, distance, 1.0);
    for _ in 0..2_000 {
        train.step_block_brake_without_readings(&circuit, &[piece_index], true);
    }
    assert_eq!(train.speed(), 0.0);
    assert!(train.notes().is_empty());
    let held_distance = train.distance();
    for _ in 0..20 {
        train.step_block_brake_without_readings(&circuit, &[piece_index], false);
    }
    assert!(train.distance() > held_distance);
    assert!(train.speed() > 0.0);
    assert!(train.notes().is_empty());
}

#[test]
fn downhill_brake_with_insufficient_holding_force_does_not_freeze_train() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let piece_index = circuit
        .pieces()
        .iter()
        .enumerate()
        .filter(|(_, piece)| piece.id == crate::track::PieceId::GentleDown)
        .nth(2)
        .unwrap()
        .0;
    let distance = circuit.starts[piece_index] + circuit.pieces()[piece_index].length() / 2.0;
    let config = super::PhysicsConfig {
        braking_acceleration: 1.0,
        ..Default::default()
    };
    let mut train = super::Train::new(&circuit, &configuration, config, distance, 0.0);
    for _ in 0..100 {
        train.step_block_brake_without_readings(&circuit, &[piece_index], true);
    }
    assert!(train.distance() > distance);
    assert!(train.speed() > 0.0);
}

#[test]
fn uphill_brake_drive_restarts_without_initial_rollback() {
    let mut pieces = super::demo_circuit().pieces().to_vec();
    for piece in &mut pieces {
        if piece.kind == crate::track::PieceKind::Lift {
            piece.kind = crate::track::PieceKind::Track;
        }
    }
    let circuit = super::Circuit::new(pieces).unwrap();
    let configuration = crate::track::hybrid_model().train;
    let piece_index = circuit
        .pieces()
        .iter()
        .enumerate()
        .filter(|(_, piece)| piece.id == crate::track::PieceId::GentleUp)
        .nth(2)
        .unwrap()
        .0;
    let distance = circuit.starts[piece_index] + circuit.pieces()[piece_index].length() / 2.0;
    let config = super::PhysicsConfig {
        braking_acceleration: 6.0,
        drive_acceleration: 6.0,
        ..Default::default()
    };
    let mut train = super::Train::new(&circuit, &configuration, config, distance, 0.0);
    for _ in 0..100 {
        train.step_block_brake_without_readings(&circuit, &[piece_index], true);
    }
    assert_eq!(train.distance(), distance);
    train.step_block_brake_without_readings(&circuit, &[piece_index], false);
    assert!(train.distance() >= distance);
    assert!(train.speed() > 0.0);
    assert!(train.notes().is_empty());
}

#[test]
fn coaster_run_waits_for_a_block_and_resumes_without_recovery() {
    let circuit = super::demo_circuit();
    let configuration = crate::track::hybrid_model().train;
    let mut run = super::CoasterRun::new(
        &circuit,
        &configuration,
        super::PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        },
    );
    for _ in 0..1000 {
        assert!(
            run.step_with_block_brake(&circuit, &[], &[0], true)
                .is_none()
        );
    }
    assert_eq!(run.train().speed(), 0.0);
    let outcome = (0..200_000)
        .find_map(|_| run.step_with_block_brake(&circuit, &[], &[0], false))
        .expect("train completes after block releases");
    assert!(outcome.reports[0].completed);
    assert!(!outcome.recovered);
}

#[test]
fn climbing_small_turn_vertical_g_matches_straight_climb() {
    let pieces = catalog();
    let straight = pieces
        .iter()
        .find(|piece| piece.id == PieceId::GentleUp)
        .unwrap();
    let baseline = rider_g(straight, straight.length() / 2.0, 15.0, 0.0).vertical;
    let piece = pieces
        .iter()
        .find(|piece| {
            piece.id == PieceId::TurnSmallStandalone
                && piece.entry.slope == crate::track::Slope::UpGentle
        })
        .expect("missing climbing small turn");
    for index in 0..=100 {
        let vertical =
            rider_g(piece, piece.length() * f64::from(index) / 100.0, 15.0, 0.0).vertical;
        assert!(
            (vertical - baseline).abs() <= 0.2,
            "{}: {vertical} vs {baseline}",
            piece.name
        );
    }
}

#[test]
fn climbing_large_turn_grid_rise_minimizes_midpoint_vertical_force() {
    let pieces = catalog();
    let straight = pieces
        .iter()
        .find(|piece| piece.id == PieceId::GentleUp)
        .unwrap();
    let baseline = rider_g(straight, straight.length() / 2.0, 15.0, 0.0).vertical;
    let turn = pieces
        .iter()
        .find(|piece| {
            piece.id == PieceId::TurnLarge && piece.entry.slope == crate::track::Slope::UpGentle
        })
        .unwrap();
    let mut alternative = turn.clone();
    alternative.exit.z = 13;
    alternative.rebuild_arc();
    let deviation =
        |piece: &Piece| (rider_g(piece, piece.length() / 2.0, 15.0, 0.0).vertical - baseline).abs();
    assert!(
        deviation(turn) <= deviation(&alternative),
        "chosen {}, alternative {}",
        deviation(turn),
        deviation(&alternative)
    );
}

#[test]
fn sixty_degree_slope_transitions_stay_below_four_vertical_g() {
    let pieces = catalog();
    for piece in pieces.iter().filter(|piece| {
        matches!(
            piece.id,
            PieceId::LevelToSixtyUp
                | PieceId::SixtyUpToLevel
                | PieceId::LevelToSixtyDown
                | PieceId::SixtyDownToLevel
                | PieceId::SteepToSixtyUp
                | PieceId::SixtyToSteepUp
                | PieceId::SteepToSixtyDown
                | PieceId::SixtyToSteepDown
        )
    }) {
        let peak_vertical_g = (0..=100)
            .map(|index| {
                rider_g(
                    piece,
                    (piece.length() * f64::from(index) / 100.0).min(piece.length()),
                    25.0,
                    0.0,
                )
                .vertical
                .abs()
            })
            .fold(0.0_f64, f64::max);
        assert!(
            peak_vertical_g <= 4.0,
            "{}: {peak_vertical_g:.3} g",
            piece.name
        );
    }
}

#[test]
fn banked_drops_complete_physics_laps_with_finite_forces() {
    use crate::track::multi_launch_model;
    use PieceId::*;
    for (model, bank, drop, unbank, first_turn, second_turn) in [
        (
            wooden_twister_model(),
            BankLeft,
            BankedDropLeft,
            Some(UnbankLeft),
            TurnLargeEntry,
            TurnLargeExit,
        ),
        (
            wooden_twister_model(),
            BankRight,
            BankedDropRight,
            Some(UnbankRight),
            TurnLargeRightEntry,
            TurnLargeRightExit,
        ),
        (
            hybrid_model(),
            BankLeft,
            BankedDropLeft,
            Some(UnbankLeft),
            TurnLargeEntry,
            TurnLargeExit,
        ),
        (
            hybrid_model(),
            BankRight,
            BankedDropRight,
            Some(UnbankRight),
            TurnLargeRightEntry,
            TurnLargeRightExit,
        ),
        (
            multi_launch_model(),
            BankLeft,
            BankedDropLeft,
            Some(UnbankLeft),
            TurnLargeEntry,
            TurnLargeExit,
        ),
        (
            multi_launch_model(),
            BankRight,
            BankedDropRight,
            Some(UnbankRight),
            TurnLargeRightEntry,
            TurnLargeRightExit,
        ),
        (
            hybrid_model(),
            BankLeft,
            TwistedDropLeft,
            None,
            TurnLargeEntry,
            TurnLargeExit,
        ),
        (
            hybrid_model(),
            BankRight,
            TwistedDropRight,
            None,
            TurnLargeRightEntry,
            TurnLargeRightExit,
        ),
        (
            hybrid_model(),
            BankLeft,
            BarrelDropLeft,
            None,
            TurnLargeEntry,
            TurnLargeExit,
        ),
        (
            hybrid_model(),
            BankRight,
            BarrelDropRight,
            None,
            TurnLargeRightEntry,
            TurnLargeRightExit,
        ),
    ] {
        let mut ids = vec![Station, LevelToGentleUp];
        if unbank.is_some() {
            ids.push(GentleUp);
        }
        ids.extend([GentleUpToLevel, first_turn, second_turn, bank, drop]);
        if let Some(unbank) = unbank {
            ids.push(unbank);
        }
        ids.push(GentleDownToLevel);
        ids.extend([first_turn, second_turn]);
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in ids {
            let piece = model
                .available_pieces(state)
                .into_iter()
                .find(|piece| piece.id == id)
                .unwrap_or_else(|| panic!("{} cannot place {id:?} at {state:?}", model.name));
            state = piece.exit;
            pieces.push(piece);
        }
        let circuit =
            Circuit::new(pieces).unwrap_or_else(|error| panic!("{drop:?} end {state:?}: {error}"));
        let config = PhysicsConfig {
            rolling_resistance: 0.0,
            drag_coefficient: 0.0,
            powered_sections: false,
            ..PhysicsConfig::default()
        };
        let mut train_config = model.train;
        train_config.cars = 1;
        let mut train = Train::new(&circuit, &train_config, config, 0.0, 20.0);
        for _ in 0..100_000 {
            train.step(&circuit);
            if train.distance() >= circuit.length() {
                break;
            }
        }
        assert!(
            train.distance() >= circuit.length(),
            "{drop:?}: {} of {}",
            train.distance(),
            circuit.length()
        );
        let piece = circuit
            .pieces()
            .iter()
            .find(|piece| piece.id == drop)
            .unwrap();
        for step in 0..=100 {
            let forces = rider_g(piece, piece.length() * f64::from(step) / 100.0, 20.0, 0.0);
            assert!(
                forces.vertical.is_finite() && forces.vertical.abs() < 6.0,
                "{drop:?}: {forces:?}"
            );
            assert!(
                forces.lateral.is_finite() && forces.lateral.abs() < 6.0,
                "{drop:?}: {forces:?}"
            );
        }
    }
}
