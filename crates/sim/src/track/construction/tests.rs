use super::*;
use crate::track::{Heading, Slope, Vec3, hybrid_model};
use crate::world::Surface;

fn world() -> World {
    World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    )
}

fn start() -> TrackState {
    TrackState {
        x: 50,
        y: 50,
        z: 5,
        ..TrackState::origin()
    }
}

#[test]
fn choices_report_model_and_connector_rejections() {
    let mut model = hybrid_model();
    model.allowed_pieces = vec![PieceId::Straight, PieceId::GentleUp];
    let construction = Construction::new(start(), model);
    let choices = construction.choices(&world(), &[]);
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::Station)
            .unwrap()
            .result,
        Err(ConstructionError::ModelForbidden)
    );
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::GentleUp)
            .unwrap()
            .result,
        Err(ConstructionError::Unavailable)
    );
}

#[test]
fn append_and_delete_keep_connector_state_and_are_atomic_on_failure() {
    let mut construction = Construction::new(start(), hybrid_model());
    assert_eq!(
        construction.append(PieceId::GentleUp, &world(), &[]),
        Err(ConstructionError::Unavailable)
    );
    assert!(construction.pieces().is_empty());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let first_end = construction.end();
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.delete_from(1).unwrap().len(), 1);
    assert_eq!(construction.end(), first_end);
    assert!(matches!(
        construction.delete_from(1),
        Err(ConstructionError::InvalidIndex)
    ));
    assert_eq!(construction.pieces().len(), 1);
    assert_eq!(construction.end().heading, Heading::East);
    assert_eq!(construction.end().slope, Slope::Level);
}

#[test]
fn support_search_failure_does_not_mutate_track() {
    let obstacle = SupportObstacle {
        min: Vec3::new(0.0, 0.0, -1.0),
        max: Vec3::new(512.0, 512.0, 9.0),
    };
    let mut construction = Construction::new(start(), hybrid_model());
    assert_eq!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::SupportSearchExhausted)
    );
    assert!(construction.pieces().is_empty());
}

#[test]
fn closed_circuit_requires_explicit_close_and_reopens_on_deletion() {
    use PieceId::*;
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world(), &[]).unwrap();
    }
    assert_eq!(construction.end(), start());
    assert!(construction.circuit().is_err());
    construction.close().unwrap();
    assert!(construction.circuit().is_ok());
    assert_eq!(
        construction.append(Straight, &world(), &[]),
        Err(ConstructionError::Closed)
    );
    construction.delete_from(5).unwrap();
    assert!(construction.circuit().is_err());
}

#[test]
fn collision_is_reported_and_does_not_place_a_piece() {
    let obstacle = SupportObstacle {
        min: Vec3::new(106.0, 99.5, 10.0),
        max: Vec3::new(108.0, 100.5, 12.0),
    };
    let mut construction = Construction::new(start(), hybrid_model());
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::Clearance(_))
    ));
    assert!(construction.pieces().is_empty());
}

#[test]
fn supports_route_around_existing_track_and_train_space() {
    let model = hybrid_model();
    let existing = model
        .available_pieces(TrackState {
            y: 48,
            z: 3,
            ..start()
        })
        .into_iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let crossing = model
        .available_pieces(TrackState {
            x: 54,
            y: 46,
            heading: Heading::North,
            ..start()
        })
        .into_iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let obstacles = track_support_obstacles(&[existing], &model.train);
    let supports = generate_supports(&crossing, &world(), &obstacles).unwrap();
    let crossing_support = &supports[0];
    assert!(
        crossing_support.footing.x != crossing_support.anchor.x
            || crossing_support.footing.y != crossing_support.anchor.y
    );
}

#[test]
fn deleting_middle_preserves_suffix_ids_and_reconnects_when_rebuilt() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..3 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let ids = construction.piece_ids().to_vec();
    assert_eq!(construction.delete_from(ids[1]).unwrap().len(), 1);
    assert_eq!(construction.piece_ids(), &ids[..1]);
    assert_eq!(construction.disconnected_segments()[0].ids, &ids[2..]);
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.pieces().len(), 3);
    assert_eq!(construction.piece_ids()[0], ids[0]);
    assert_eq!(construction.piece_ids()[2], ids[2]);
    assert_ne!(construction.piece_ids()[1], ids[1]);
    assert!(construction.disconnected_segments().is_empty());
}

#[test]
fn deleting_disconnected_piece_preserves_both_sides() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..5 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let ids = construction.piece_ids().to_vec();
    construction.delete_from(ids[1]).unwrap();
    construction.delete_from(ids[3]).unwrap();
    let segments = construction.disconnected_segments();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].ids, [ids[2]]);
    assert_eq!(segments[1].ids, [ids[4]]);
}

#[test]
fn append_revalidates_existing_supports() {
    let mut construction = Construction::new(start(), hybrid_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(103.0, 99.0, 9.0),
        max: Vec3::new(105.0, 101.0, 11.0),
    };
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::NoSupportRoute)
    ));
    assert_eq!(construction.pieces().len(), 1);
}

#[test]
fn model_radius_and_inversion_restrictions_have_model_reason() {
    let mut model = hybrid_model();
    model.allowed_inversions.clear();
    model.minimum_radius_metres = 100.0;
    let mut construction = Construction::new(start(), model);
    for id in [PieceId::Loop, PieceId::TurnSmallStandalone] {
        assert_eq!(
            construction
                .choices(&world(), &[])
                .iter()
                .find(|choice| choice.id == id)
                .unwrap()
                .result,
            Err(ConstructionError::ModelForbidden)
        );
        assert_eq!(
            construction.append(id, &world(), &[]),
            Err(ConstructionError::ModelForbidden)
        );
    }
}

#[test]
fn reconnect_checks_retained_track_against_current_obstacles() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let first = construction.piece_ids()[0];
    construction.delete_from(first).unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(149.0, 99.5, 10.0),
        max: Vec3::new(151.0, 100.5, 12.0),
    };
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::Clearance(_))
    ));
    assert!(construction.pieces().is_empty());
    assert_eq!(construction.disconnected_segments()[0].pieces.len(), 3);
}
