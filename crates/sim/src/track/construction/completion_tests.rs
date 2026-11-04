use super::*;
use crate::track::{PieceId, PieceKind, TrackState, hybrid_model, wooden_twister_model};
use crate::world::Surface;

#[test]
fn completion_uses_unpowered_connectors_and_preserves_chain_mode() {
    use PieceId::*;

    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 50,
        y: 50,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let original_length = construction.pieces().len();
    construction.set_chain(true);

    let completion = construction
        .find_completion(&world, &[])
        .expect("nearby connection with chain mode enabled");

    assert!(!completion.geometry.is_empty());
    assert!(
        completion
            .geometry
            .iter()
            .all(|piece| piece.kind == PieceKind::Track)
    );
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();

    assert!(construction.is_closed());
    assert!(construction.chain());
    assert!(
        construction.pieces()[original_length..]
            .iter()
            .all(|piece| piece.kind == PieceKind::Track)
    );
}

#[test]
fn completion_finds_a_four_piece_southeast_offset_connection() {
    use PieceId::*;

    let world = World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 128,
        y: 127,
        z: 30,
        heading: crate::track::Heading::SouthEast,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, wooden_twister_model());
    for id in [
        Station,
        Straight,
        Straight,
        Straight,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        FullOffsetLeft,
        FullOffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }

    let completion = construction
        .find_completion(&world, &[])
        .expect("two-tile offset connection");

    assert_eq!(completion.pieces.len(), 4);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}
