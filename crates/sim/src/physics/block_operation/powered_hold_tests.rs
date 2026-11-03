use super::{BlockLayout, BlockOperation, Circuit, PhysicsConfig, Train};
use crate::track::{PieceId, TrackState, catalog, hybrid_model, multi_launch_model};

fn powered_oval_with(piece_id: PieceId) -> Circuit {
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        piece_id,
        piece_id,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::Straight,
        piece_id,
        piece_id,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ] {
        let piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

fn powered_oval() -> Circuit {
    powered_oval_with(PieceId::LiftFlat)
}

#[test]
fn occupied_section_brakes_and_releases_train_at_lift_start() {
    let circuit = powered_oval();
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let lift_start: f64 = circuit.pieces()[..3]
        .iter()
        .map(|piece| piece.length())
        .sum();
    let lift_end = lift_start + circuit.pieces()[3].length();
    let mut train = Train::new(
        &circuit,
        &hybrid_model().train,
        config,
        lift_start + 1.0,
        3.0,
    );
    for _ in 0..400 {
        train.step_block_brake_without_readings(&circuit, &[3], true);
    }
    assert_eq!(train.speed(), 0.0);
    assert!(train.distance() <= lift_start + 4.0);
    assert!(train.distance() < lift_end);
    assert!(train.notes().is_empty());
    for _ in 0..200 {
        train.step_block_brake_without_readings(&circuit, &[3], false);
    }
    assert!(train.speed() > 0.0);
    assert!(train.notes().is_empty());
}

#[test]
fn occupied_section_brakes_and_releases_train_at_launch_start() {
    let circuit = powered_oval_with(PieceId::LaunchShort);
    let config = PhysicsConfig::default();
    let launch_start: f64 = circuit.pieces()[..3]
        .iter()
        .map(|piece| piece.length())
        .sum();
    let launch_end = launch_start + circuit.pieces()[3].length();
    let mut train = Train::new(
        &circuit,
        &multi_launch_model().train,
        config,
        launch_start + 1.0,
        3.0,
    );
    for _ in 0..400 {
        train.step_block_brake_without_readings(&circuit, &[3], true);
    }
    assert_eq!(train.speed(), 0.0);
    assert!(train.distance() <= launch_start + 4.0);
    assert!(train.distance() < launch_end);
    assert!(train.notes().is_empty());
    for _ in 0..200 {
        train.step_block_brake_without_readings(&circuit, &[3], false);
    }
    assert!(train.speed() > 0.0);
    assert!(train.notes().is_empty());
}

#[test]
fn lift_sections_allow_two_trains_without_added_brakes() {
    let circuit = powered_oval();
    let train = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let layout = BlockLayout::new(&circuit, &train, config, Vec::new()).unwrap();
    assert!(
        layout.capacity() >= 2,
        "station and lift sections should support two trains"
    );
    let mut operation = BlockOperation::new(layout);
    operation.panic_on_interlock = true;
    let mut maximum_active = 0;
    let mut completed = 0;
    let mut held_at_lift = false;
    for _ in 0..120_000 {
        while operation.can_dispatch() {
            operation.dispatch();
        }
        maximum_active = maximum_active.max(operation.trains().count());
        for (index, operating_train) in operation.trains.iter().enumerate() {
            for other in &operation.trains[index + 1..] {
                let occupied = operation.occupied(operating_train.run.train().distance());
                let other_occupied = operation.occupied(other.run.train().distance());
                assert!(
                    occupied
                        .iter()
                        .all(|section| !other_occupied.contains(section))
                );
            }
        }
        for (_, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            completed += 1;
        }
        for (_, train) in operation.trains() {
            let piece = circuit.piece_index(train.distance());
            if train.speed() == 0.0 && [3, 11].contains(&piece) {
                assert!(
                    operation.occupied(train.distance()).len() >= 2,
                    "train rear must retain its preceding block"
                );
                held_at_lift = true;
            }
        }
        if completed >= 4 {
            break;
        }
    }
    assert!(maximum_active >= 2);
    assert!(completed >= 4);
    assert!(held_at_lift, "a blocked lift should hold a train");
}

#[test]
fn launch_sections_allow_two_trains_without_added_brakes() {
    let circuit = powered_oval_with(PieceId::LaunchShort);
    let train = multi_launch_model().train;
    let config = PhysicsConfig {
        braking_acceleration: 20.0,
        ..Default::default()
    };
    let layout = BlockLayout::new(&circuit, &train, config, Vec::new()).unwrap();
    assert!(
        layout.capacity() >= 2,
        "station and launch sections should support two trains"
    );
    let mut operation = BlockOperation::new(layout);
    operation.panic_on_interlock = true;
    let mut maximum_active = 0;
    let mut completed = 0;
    for _ in 0..120_000 {
        while operation.can_dispatch() {
            operation.dispatch();
        }
        maximum_active = maximum_active.max(operation.trains().count());
        for (_, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            completed += 1;
        }
        if completed >= 4 {
            break;
        }
    }
    assert!(maximum_active >= 2);
    assert!(completed >= 4);
}

#[test]
fn launch_approach_that_cannot_stop_safely_uses_one_train() {
    let circuit = powered_oval_with(PieceId::LaunchShort);
    let layout = BlockLayout::new(
        &circuit,
        &multi_launch_model().train,
        PhysicsConfig::default(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(layout.capacity(), 1);
    let mut operation = BlockOperation::new(layout);
    operation.panic_on_interlock = true;
    operation.dispatch();
    let outcome = (0..120_000)
        .find_map(|_| operation.step(&[]).into_iter().next())
        .expect("train completes")
        .1;
    assert!(!outcome.recovered);
}

#[test]
fn two_brakes_on_unpowered_oval_have_one_train_capacity() {
    let original = super::tests::test_circuit();
    let mut pieces = original.pieces().to_vec();
    for piece in &mut pieces {
        if piece.kind == crate::track::PieceKind::Lift {
            piece.kind = crate::track::PieceKind::Track;
        }
    }
    let circuit = Circuit::new(pieces).unwrap();
    let boundaries = [2, 6]
        .into_iter()
        .map(|end| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        })
        .collect();
    let config = PhysicsConfig {
        rolling_resistance: 0.001,
        drag_coefficient: 0.5,
        ..Default::default()
    };
    let layout = BlockLayout::new(&circuit, &hybrid_model().train, config, boundaries).unwrap();
    assert_eq!(layout.capacity(), 1);
    let mut operation = BlockOperation::new(layout);
    operation.dispatch();
    let outcome = (0..120_000)
        .find_map(|_| operation.step(&[]).into_iter().next())
        .expect("train completes")
        .1;
    assert!(!outcome.recovered);
}

#[test]
fn brake_layout_rejects_train_that_stalls_on_an_unblocked_lap() {
    let mut pieces = super::tests::test_circuit().pieces().to_vec();
    for piece in &mut pieces {
        if piece.kind == crate::track::PieceKind::Lift {
            piece.kind = crate::track::PieceKind::Track;
        }
    }
    let circuit = Circuit::new(pieces).unwrap();
    let boundaries = [2, 6]
        .into_iter()
        .map(|end| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        })
        .collect();
    let result = BlockLayout::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        boundaries,
    );
    let reason = result.unwrap_err();
    assert!(
        reason.contains("cannot complete the circuit")
            || reason.contains("did not reach the next block brake"),
        "{reason}"
    );
}

#[test]
fn two_native_lift_trains_complete_after_dispatch_stops() {
    let circuit = super::tests::test_circuit();
    let train = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let layout = BlockLayout::new(&circuit, &train, config, Vec::new()).unwrap();
    assert!(layout.capacity() >= 2);
    let mut operation = BlockOperation::new_without_reports(layout);
    operation.panic_on_interlock = true;
    let mut dispatched = 0;
    let mut completed = 0;
    let mut maximum_active = 0;
    for _ in 0..120_000 {
        if dispatched < 2 && operation.can_dispatch() {
            operation.dispatch();
            dispatched += 1;
        }
        maximum_active = maximum_active.max(operation.trains().count());
        for (_, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            completed += 1;
        }
        if dispatched == 2 && completed == 2 {
            break;
        }
    }
    assert_eq!(dispatched, 2);
    assert_eq!(maximum_active, 2);
    assert_eq!(
        completed,
        2,
        "active trains: {:?}",
        operation
            .trains()
            .map(|(id, train)| (id, train.distance(), train.speed()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn native_lift_trains_drain_after_varied_dispatch_delays() {
    let circuit = super::tests::test_circuit();
    let train = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let layout = BlockLayout::new(&circuit, &train, config, Vec::new()).unwrap();
    assert!(layout.capacity() >= 2);
    for delay in [0, 500, 1_500, 3_000, 6_000] {
        let mut operation = BlockOperation::new_without_reports(layout.clone());
        operation.panic_on_interlock = true;
        operation.dispatch();
        let mut dispatched = 1;
        let mut completed = 0;
        let mut maximum_active = 1;
        let mut second_dispatch_step = None;
        for step in 0..100_000 {
            if step >= delay && dispatched == 1 && operation.can_dispatch() {
                operation.dispatch();
                dispatched = 2;
                second_dispatch_step = Some(step);
            }
            maximum_active = maximum_active.max(operation.trains().count());
            for (_, outcome) in operation.step(&[]) {
                assert!(!outcome.recovered, "delay {delay} recovered");
                completed += 1;
            }
            if dispatched == 2 && completed == 2 {
                assert!(
                    step - second_dispatch_step.unwrap() < 66_000,
                    "delay {delay}: drain took {} steps",
                    step - second_dispatch_step.unwrap()
                );
                break;
            }
        }
        assert_eq!(maximum_active, 2, "delay {delay}");
        assert_eq!(
            completed,
            2,
            "delay {delay}: {:?}",
            operation
                .trains()
                .map(|(id, train)| (id, train.distance(), train.speed()))
                .collect::<Vec<_>>()
        );
    }
}
