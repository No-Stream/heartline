use super::*;
use crate::track::{PieceId, TrackState, catalog, hybrid_model};

fn two_brake_layout() -> (Circuit, TrainConfiguration, PhysicsConfig, BlockLayout) {
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
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
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Station,
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
    let circuit = Circuit::new(pieces).unwrap();
    let train = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let boundary = |end: usize| {
        circuit.pieces()[..end]
            .iter()
            .map(|piece| piece.length())
            .sum()
    };
    let layout = BlockLayout::new(&circuit, &train, config, vec![boundary(3), boundary(15)])
        .expect("the two brake runs must validate");
    assert!(layout.boundaries().iter().enumerate().all(|(index, _)| {
        layout
            .brake_pieces(index)
            .iter()
            .all(|&piece| circuit.pieces()[piece].kind != crate::track::PieceKind::Station)
    }));
    (circuit, train, config, layout)
}

#[test]
fn station_and_two_separate_brakes_support_two_trains() {
    let (circuit, train, _, layout) = two_brake_layout();
    assert_eq!(
        layout.capacity(),
        2,
        "station and two brakes should support two trains"
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
            assert!(outcome.reports[0].completed);
            completed += 1;
        }
        let distances: Vec<_> = operation
            .trains()
            .map(|(_, train)| train.distance())
            .collect();
        for (index, &distance) in distances.iter().enumerate() {
            for &other in &distances[index + 1..] {
                let separation = (distance - other).rem_euclid(circuit.length());
                assert!(
                    separation >= train.length_metres
                        && circuit.length() - separation >= train.length_metres
                );
            }
        }
        if completed >= 4 {
            break;
        }
    }
    assert_eq!(maximum_active, 2);
    assert!(
        completed >= 4,
        "trains stopped making progress after {completed} completions"
    );
}

#[test]
fn station_two_brakes_recover_both_active_trains() {
    let (_, _, _, layout) = two_brake_layout();
    let mut operation = BlockOperation::new(layout);
    operation.panic_on_interlock = true;
    let mut dispatched = Vec::new();
    for _ in 0..120_000 {
        if operation.can_dispatch() {
            dispatched.push(operation.dispatch());
        }
        assert!(operation.step(&[]).is_empty());
        if dispatched.len() == 2 {
            break;
        }
    }
    assert_eq!(dispatched.len(), 2);
    let outcomes = operation.recover_all();
    assert_eq!(
        outcomes.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        dispatched
    );
    assert!(outcomes.iter().all(|(_, outcome)| outcome.recovered));
    assert_eq!(operation.trains().count(), 0);
}

#[test]
fn long_station_fixture_can_be_built_with_construction_tool() {
    let world = crate::world::World::new(
        512,
        512,
        crate::world::Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 256,
        y: 256,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = crate::track::construction::Construction::new(start, hybrid_model());
    for id in [
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
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Station,
    ] {
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| panic!("{id:?}: {error:?}"));
    }
    construction.close().unwrap();
}
