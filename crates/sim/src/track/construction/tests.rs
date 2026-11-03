use super::*;
use crate::physics::{PhysicsConfig, Train};
use crate::track::{Heading, Slope, Vec3, hybrid_model};
use crate::world::Surface;
use std::time::{Duration, Instant};

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
fn wooden_twister_validates_every_dense_support_anchor() {
    let mut construction = Construction::new(start(), crate::track::wooden_twister_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let cache = construction.validation_cache.borrow();
    let supports = cache.as_ref().unwrap().supports[0]
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    let piece = &construction.pieces()[0];
    let count = (piece.length() / 4.0).ceil() as usize;
    assert_eq!(supports.len(), count);
    for (index, support) in supports.iter().enumerate() {
        assert_eq!(
            support.anchor,
            piece
                .sample(piece.length() * (index as f64 + 0.5) / count as f64)
                .position
        );
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
fn launch_model_builds_midcourse_launch_and_rejects_chain_lifts() {
    let mut construction = Construction::new(start(), crate::track::multi_launch_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    construction
        .append(PieceId::LaunchLong, &world(), &[])
        .unwrap();
    assert_eq!(
        construction.pieces()[1].kind,
        crate::track::PieceKind::Launch
    );
    assert_eq!(
        construction.append(PieceId::LiftFlat, &world(), &[]),
        Err(ConstructionError::ModelForbidden)
    );
    let mut hybrid = Construction::new(start(), hybrid_model());
    assert_eq!(
        hybrid.append(PieceId::Launch, &world(), &[]),
        Err(ConstructionError::ModelForbidden)
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
        Err(ConstructionError::NoSupportRoute)
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
fn every_coaster_model_can_build_a_closed_circuit() {
    use PieceId::*;

    let track_world = world();
    for model in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ] {
        let mut construction = Construction::new(start(), model);
        for piece_id in [
            Station,
            TurnSmallStandalone,
            TurnSmallStandalone,
            Straight,
            TurnSmallStandalone,
            TurnSmallStandalone,
        ] {
            construction
                .append(piece_id, &track_world, &[])
                .unwrap_or_else(|error| {
                    panic!(
                        "{} rejected {piece_id:?}: {error:?}",
                        construction.model().name
                    )
                });
        }

        assert_eq!(construction.end(), start(), "{}", construction.model().name);
        construction.close().unwrap();
        let circuit = construction.circuit().unwrap();
        assert!(construction.is_closed());
        assert!(
            circuit.length() > construction.model().train.length_metres,
            "{}",
            construction.model().name
        );
    }
}

#[test]
fn right_turn_circuit_builds_and_completes_a_lap() {
    use PieceId::*;

    let track_world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallRightStandalone,
        TurnSmallRightStandalone,
        Straight,
        TurnSmallRightStandalone,
        TurnSmallRightStandalone,
    ] {
        construction.append(id, &track_world, &[]).unwrap();
    }
    assert_eq!(construction.end(), start());
    construction.close().unwrap();
    let circuit = construction.circuit().unwrap();

    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let start_distance = circuit.pieces()[0].length() / 2.0;
    let mut train = Train::new(
        &circuit,
        &construction.model().train,
        config,
        start_distance,
        12.0,
    );
    let mut completed_lap = false;
    for _ in 0..60_000 {
        train.step(&circuit);
        if train.distance() - start_distance >= circuit.length() {
            completed_lap = true;
            break;
        }
    }

    assert!(completed_lap, "the train did not complete one circuit lap");
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
fn wooden_inversion_is_rejected_even_if_added_to_allowed_ids() {
    let mut model = crate::track::wooden_twister_model();
    model.allowed_pieces.push(PieceId::Loop);
    let mut construction = Construction::new(start(), model);
    assert_eq!(
        construction.append(PieceId::Loop, &world(), &[]),
        Err(ConstructionError::ModelForbidden)
    );
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

#[test]
fn choices_reflect_world_obstacle_and_track_changes() {
    let mut world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let initial = construction.choices(&world, &[]);
    assert_eq!(
        initial
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
    let obstacle = SupportObstacle {
        min: Vec3::new(119.0, 99.0, 9.0),
        max: Vec3::new(121.0, 101.0, 13.0),
    };
    assert!(
        construction
            .choices(&world, &[obstacle])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result
            .is_err()
    );
    assert_eq!(construction.choices(&world, &[]), initial);
    world.set_surface(
        30,
        25,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    assert!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result
            .is_err()
    );
    construction.delete_from(0).unwrap();
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
}

#[test]
fn cached_choices_match_fresh_validation_after_track_edits() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    construction.delete_from(1).unwrap();
    let cached = construction.choices(&world, &[]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[]));
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let cached = construction.choices(&world, &[]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[]));
}

#[test]
fn failed_support_search_is_rechecked_when_candidate_is_nearby() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(103.0, 99.0, 9.0),
        max: Vec3::new(105.0, 101.0, 11.0),
    };
    let cached = construction.choices(&world, &[obstacle]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[obstacle]));
}

#[test]
fn nearby_track_obstacle_filter_preserves_support_routes() {
    let world = world();
    let model = hybrid_model();
    let mut construction = Construction::new(start(), model.clone());
    for _ in 0..8 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for (index, piece) in construction.pieces().iter().enumerate() {
        let all: Vec<_> = construction
            .pieces()
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .flat_map(|(_, other)| {
                track_support_obstacles(std::slice::from_ref(other), &model.train)
            })
            .collect();
        let region = support_search_region(piece);
        let nearby: Vec<_> = all
            .iter()
            .copied()
            .filter(|obstacle| region.overlaps(obstacle))
            .collect();
        assert_eq!(
            generate_supports(piece, &world, &all),
            generate_supports(piece, &world, &nearby)
        );
    }
}

#[test]
fn available_choices_match_model_candidates_before_and_after_closing() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let expected: Vec<_> = construction
        .model()
        .available_pieces(construction.end())
        .into_iter()
        .map(|piece| (piece.id, piece.exit))
        .collect();
    for closed in [false, true] {
        if closed {
            construction.close().unwrap();
        }
        let actual = construction.available_choices(&world, &[]);
        assert_eq!(
            actual
                .iter()
                .map(|(piece, _)| (piece.id, piece.exit))
                .collect::<Vec<_>>(),
            expected
        );
        if closed {
            assert!(
                actual
                    .iter()
                    .all(|(_, choice)| choice.result == Err(ConstructionError::Closed))
            );
        }
    }
}

#[test]
fn choices_after_deleting_from_150_piece_circuit() {
    let world = World::new(
        1024,
        1024,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let mut construction = Construction::new(TrackState { z: 160, ..start() }, hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for _ in 0..72 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for _ in 0..2 {
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
    }
    for _ in 0..73 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for _ in 0..2 {
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
    }
    assert_eq!(construction.pieces().len(), 150);
    construction.close().unwrap();
    construction.delete_from(75).unwrap();
    let started = Instant::now();
    let choices = construction.choices(&world, &[]);
    let elapsed = started.elapsed();
    assert!(choices.iter().any(|choice| choice.result.is_ok()));
    if !cfg!(debug_assertions) {
        assert!(
            elapsed < Duration::from_millis(50),
            "choices took {elapsed:?}"
        );
    }
}

#[test]
fn choices_match_individual_validation_for_reconnecting_candidates() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    construction.delete_from(1).unwrap();
    for choice in construction.choices(&world, &[]) {
        assert_eq!(
            choice.result,
            construction.validate(choice.id, &world, &[]),
            "{:?}",
            choice.id
        );
    }
}

#[test]
fn elevated_crossing_rejects_rail_through_existing_train_space() {
    use PieceId::*;
    let world = world();
    let mut model = hybrid_model();
    model.train.height_metres = 10.0;
    let mut construction = Construction::new(TrackState { z: 20, ..start() }, model);
    for id in [
        Straight,
        Straight,
        LevelToGentleUp,
        GentleUpToLevel,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        Straight,
        Straight,
        TurnSmallStandalone,
        Straight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let candidate = construction
        .model_placeable(
            templates()
                .iter()
                .find(|piece| piece.id == Straight)
                .unwrap(),
        )
        .unwrap();
    let train = &construction.model().train;
    assert_eq!(
        crate::track::clearance::check_clearance(
            &world,
            &candidate,
            train,
            construction.pieces(),
            &[]
        ),
        Ok(())
    );
    assert!(construction.pieces().iter().any(|existing| {
        crate::track::clearance::check_clearance(
            &world,
            existing,
            train,
            std::slice::from_ref(&candidate),
            &[],
        ) == Err(ClearanceCollision::Track)
    }));
    let previous_count = construction.pieces().len();
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == Straight)
            .unwrap()
            .result,
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(
        construction.validate(Straight, &world, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(
        construction.append(Straight, &world, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(construction.pieces().len(), previous_count);
}
