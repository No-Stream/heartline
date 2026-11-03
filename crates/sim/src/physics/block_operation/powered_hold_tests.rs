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
        let piece = catalog.iter().find(|piece| piece.id == id).unwrap().at(state).unwrap();
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
    let config = PhysicsConfig { lift_speed: 5.0, ..Default::default() };
    let lift_start: f64 = circuit.pieces()[..3].iter().map(|piece| piece.length()).sum();
    let lift_end = lift_start + circuit.pieces()[3].length();
    let mut train = Train::new(&circuit, &hybrid_model().train, config, lift_start + 1.0, 3.0);
    for _ in 0..400 {
        train.step_block_brake_without_readings(&circuit, &[3], true);
    }
    assert_eq!(train.speed(), 0.0);
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
    let launch_start: f64 = circuit.pieces()[..3].iter().map(|piece| piece.length()).sum();
    let launch_end = launch_start + circuit.pieces()[3].length();
    let mut train = Train::new(&circuit, &multi_launch_model().train, config, launch_start + 1.0, 3.0);
    for _ in 0..400 {
        train.step_block_brake_without_readings(&circuit, &[3], true);
    }
    assert_eq!(train.speed(), 0.0);
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
    let config = PhysicsConfig { lift_speed: 5.0, ..Default::default() };
    let layout = BlockLayout::new(&circuit, &train, config, Vec::new()).unwrap();
    assert!(layout.capacity() >= 2, "station and lift sections should support two trains");
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
