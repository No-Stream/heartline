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
fn briar_blitz_choices_after_vertical_exit() {
    let world = world();
    let mut construction = Construction::new(
        TrackState { z: 15, ..start() },
        crate::track::wooden_twister_model(),
    );
    for piece in [
        PieceId::Station,
        PieceId::TightTwisterLeft,
        PieceId::BankLeft,
        PieceId::BankedTurnStandalone,
        PieceId::LargeLeftBankedTurnEntry,
        PieceId::LargeLeftBankedTurnExit,
        PieceId::UnbankLeft,
        PieceId::SteelLoop,
        PieceId::Station,
        PieceId::Brakes,
        PieceId::Station,
        PieceId::LevelToGentleDown,
        PieceId::GentleDownToLevel,
        PieceId::TurnLargeStandalone,
        PieceId::TurnLargeRightStandalone,
        PieceId::TurnSmallEntry,
        PieceId::TurnSmallExit,
        PieceId::OffsetLeft,
        PieceId::BankRight,
        PieceId::UnbankRight,
        PieceId::LevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::VerticalUpEntry,
        PieceId::VerticalUpContinue,
        PieceId::VerticalUpExit,
    ] {
        if piece == PieceId::LevelToGentleUp {
            construction.set_chain(true);
        }
        construction
            .append(piece, &world, &[])
            .unwrap_or_else(|error| panic!("{piece:?}: {error:?}"));
        if piece == PieceId::GentleUpToLevel {
            construction.set_chain(false);
        }
    }
    assert!(!construction.choices(&world, &[]).is_empty());
}

#[test]
fn construction_state_hash_covers_track_edits_but_ignores_validation_caches() {
    let world = world();
    let mut first = Construction::new(start(), hybrid_model());
    let second = Construction::new(start(), hybrid_model());
    first.choices(&world, &[]);
    assert_eq!(first.state_hash(), second.state_hash());
    first.append(PieceId::Straight, &world, &[]).unwrap();
    assert_ne!(first.state_hash(), second.state_hash());
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
fn launch_model_builds_midcourse_launch_and_chain_lifts() {
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
    construction.set_chain(true);
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.pieces()[2].kind, crate::track::PieceKind::Lift);
    let mut hybrid = Construction::new(start(), hybrid_model());
    assert_eq!(
        hybrid.append(PieceId::Launch, &world(), &[]),
        Err(ConstructionError::ModelForbidden)
    );
}

#[test]
fn chain_mode_filters_choices_and_preserves_placed_piece_kinds() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    let initial_hash = construction.state_hash();
    assert_eq!(
        construction.validate(PieceId::Straight, &world, &[]),
        Ok(())
    );
    construction.set_chain(true);
    assert!(construction.chain());
    assert_ne!(construction.state_hash(), initial_hash);
    assert_eq!(
        construction.validate(PieceId::Loop, &world, &[]),
        Err(ConstructionError::ChainNotAllowed)
    );
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Loop)
            .unwrap()
            .result,
        Err(ConstructionError::ChainNotAllowed)
    );
    assert_eq!(
        construction
            .available_choices(&world, &[])
            .iter()
            .find(|(_, choice)| choice.id == PieceId::Loop)
            .unwrap()
            .1
            .result,
        Err(ConstructionError::ChainNotAllowed)
    );
    construction
        .append(PieceId::LevelToGentleUp, &world, &[])
        .unwrap();
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    assert!(
        construction
            .pieces()
            .iter()
            .all(|piece| piece.kind == crate::track::PieceKind::Lift)
    );
    construction.set_chain(false);
    assert!(!construction.chain());
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    assert_eq!(
        construction.pieces()[2].kind,
        crate::track::PieceKind::Track
    );
}

#[test]
fn chain_rules_cover_slope_curvature_and_special_pieces() {
    let mut model = hybrid_model();
    let pieces = catalog();
    let piece = |id| pieces.iter().find(|piece| piece.id == id).unwrap();
    assert!(!model.allows_chain(piece(PieceId::Station)));
    assert!(!model.allows_chain(piece(PieceId::Brakes)));
    assert!(model.allows_chain(piece(PieceId::TurnSmallStandalone)));
    assert!(!model.allows_chain(piece(PieceId::GentleDown)));
    assert!(!model.allows_chain(piece(PieceId::Loop)));
    assert!(!model.allows_chain(piece(PieceId::AirtimeHill)));
    model.lift_rules.as_mut().unwrap().curved = false;
    assert!(!model.allows_chain(piece(PieceId::TurnSmallStandalone)));
    model.lift_rules.as_mut().unwrap().max_slope = Slope::UpGentle;
    assert!(!model.allows_chain(piece(PieceId::SteepUp)));
    model.lift_rules = None;
    assert!(!model.allows_chain(piece(PieceId::Straight)));
}

#[test]
fn chain_mode_keeps_special_pieces_placeable_and_preserves_their_kind() {
    let world = world();
    for (id, expected_kind) in [
        (PieceId::Station, PieceKind::Station),
        (PieceId::Brakes, PieceKind::Brakes),
        (PieceId::Launch, PieceKind::Launch),
    ] {
        let mut construction = Construction::new(start(), crate::track::multi_launch_model());
        construction.set_chain(true);
        assert_eq!(construction.validate(id, &world, &[]), Ok(()));
        let available = construction.available_choices(&world, &[]);
        let (piece, choice) = available
            .iter()
            .find(|(_, choice)| choice.id == id)
            .unwrap();
        assert_eq!(choice.result, Ok(()));
        assert_eq!(piece.kind, expected_kind);
        construction.append(id, &world, &[]).unwrap();
        assert_eq!(construction.pieces()[0].kind, expected_kind);
    }
}

#[test]
fn construction_places_a_curved_chain_lift() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.set_chain(true);
    construction
        .append(PieceId::TurnSmallStandalone, &world, &[])
        .unwrap();
    let placed = &construction.pieces()[0];
    assert_eq!(placed.kind, PieceKind::Lift);
    assert_ne!(placed.entry.heading, placed.exit.heading);
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
fn closed_circuit_has_no_elements_without_populating_search_cache() {
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
    construction.close().unwrap();
    assert!(construction.element_choices(&world, &[]).is_empty());
    assert!(construction.element_choices_cache.borrow().is_none());
    construction.delete_from(5).unwrap();
    assert!(!construction.element_choices(&world, &[]).is_empty());
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
fn mirrored_inversion_choices_match_individual_validation_near_asymmetric_obstacles() {
    use PieceId::{Corkscrew, CorkscrewRight, HeartlineRoll, HeartlineRollRight};

    let world = world();
    let construction = Construction::new(start(), hybrid_model());
    for (left, right) in [
        (Corkscrew, CorkscrewRight),
        (HeartlineRoll, HeartlineRollRight),
    ] {
        let piece = construction
            .model_placeable(templates().iter().find(|piece| piece.id == left).unwrap())
            .unwrap();
        let obstacle = (1..20)
            .flat_map(|step| {
                let sample = piece.sample(piece.length() * f64::from(step) / 20.0);
                [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]
                    .into_iter()
                    .map(move |offset| {
                        let centre = sample.position + Vec3::new(0.0, offset, 0.0);
                        SupportObstacle {
                            min: centre - Vec3::new(0.1, 0.1, 0.1),
                            max: centre + Vec3::new(0.1, 0.1, 0.1),
                        }
                    })
            })
            .find(|obstacle| {
                let results = (
                    construction.validate(left, &world, &[*obstacle]),
                    construction.validate(right, &world, &[*obstacle]),
                );
                matches!(
                    results,
                    (
                        Ok(()),
                        Err(ConstructionError::Clearance(ClearanceCollision::Scenery))
                    ) | (
                        Err(ConstructionError::Clearance(ClearanceCollision::Scenery)),
                        Ok(())
                    )
                )
            })
            .expect("an asymmetric obstacle distinguishes both hands");
        for choice in construction.choices(&world, &[obstacle]) {
            assert_eq!(
                choice.result,
                construction.validate(choice.id, &world, &[obstacle]),
                "{:?} with obstacle for {:?}",
                choice.id,
                left
            );
        }
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

#[test]
fn climbing_turn_choices_validate_and_append_the_matching_slope() {
    let world = world();
    for slope in [Slope::UpGentle, Slope::DownGentle] {
        let mut construction = Construction::new(
            TrackState {
                z: 30,
                slope,
                ..start()
            },
            hybrid_model(),
        );
        let choices = construction.choices(&world, &[]);
        assert_eq!(
            choices
                .iter()
                .filter(|choice| choice.id == PieceId::TurnSmallStandalone)
                .count(),
            1
        );
        assert_eq!(
            construction.validate(PieceId::TurnSmallStandalone, &world, &[]),
            Ok(())
        );
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
        assert_eq!(construction.end().slope, slope);
    }
}

#[test]
fn closure_with_short_straights_and_full_tile_offsets() {
    use PieceId::*;
    let track_world = world();
    for model in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ] {
        let mut construction = Construction::new(start(), model);
        for (index, id) in [
            Station,
            HalfTileStraight,
            FullOffsetLeft,
            TurnSmallStandalone,
            TurnSmallStandalone,
            Straight,
            HalfTileStraight,
            FullOffsetLeft,
            TurnSmallStandalone,
            TurnSmallStandalone,
        ]
        .into_iter()
        .enumerate()
        {
            construction
                .append(id, &track_world, &[])
                .unwrap_or_else(|error| {
                    panic!(
                        "{} rejected {id:?} at {index}: {error:?}",
                        construction.model().name
                    )
                });
        }
        assert_eq!(construction.end(), start(), "{}", construction.model().name);
        construction.close().unwrap();
    }
}

#[test]
fn completion_finds_a_valid_deterministic_nearby_connection() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let original_hash = construction.state_hash();
    let completion = construction
        .find_completion(&world, &[])
        .expect("nearby connection");
    assert!(!completion.pieces.is_empty());
    assert!(completion.score.is_finite());
    let repeated = construction.find_completion(&world, &[]).unwrap();
    assert_eq!(completion.pieces, repeated.pieces);
    assert_eq!(completion.score, repeated.score);
    assert_eq!(original_hash, construction.state_hash());
    for id in completion.pieces {
        construction.append(id, &world, &[]).unwrap();
    }
    assert_eq!(construction.end(), construction.start());
    construction.close().unwrap();
}

#[test]
fn completion_rejects_a_distant_end() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for _ in 0..20 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    assert!(construction.find_completion(&world, &[]).is_none());
}

#[test]
fn completion_search_yields_and_application_is_atomic() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut search = construction.completion_search().unwrap();
    assert!(matches!(
        search.advance(&world, &[]),
        CompletionSearchProgress::Pending
    ));
    let completion = loop {
        let progress = search.advance(&world, &[]);
        if let CompletionSearchProgress::Finished(result) = progress {
            break result.unwrap();
        }
    };
    let hash = construction.state_hash();
    let mut invalid = completion.clone();
    invalid.pieces.push(Straight);
    assert!(
        construction
            .apply_completion(&invalid, &world, &[])
            .is_err()
    );
    assert_eq!(construction.state_hash(), hash);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn completion_connects_a_short_turn_sequence() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [Station, TurnSmallStandalone, TurnSmallStandalone, Straight] {
        construction.append(id, &world, &[]).unwrap();
    }
    let completion = construction
        .find_completion(&world, &[])
        .expect("two-turn connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn completion_connects_a_laterally_offset_approach() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        Straight,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        OffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    expected.append(OffsetRight, &world, &[]).unwrap();
    assert_eq!(expected.end(), expected.start());
    let completion = construction
        .find_completion(&world, &[])
        .expect("offset connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_connects_an_offset_followed_by_turns() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        OffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    for id in [OffsetRight, TurnSmallStandalone, TurnSmallStandalone] {
        expected.append(id, &world, &[]).unwrap();
    }
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let mut search = construction.completion_search().unwrap();
    let completion = loop {
        let progress = search.advance(&world, &[]);
        if let CompletionSearchProgress::Finished(result) = progress {
            break result.expect("offset and turns connection");
        }
    };
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_connects_a_sloped_approach() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(TrackState { z: 20, ..start() }, hybrid_model());
    for id in [
        Station,
        LevelToGentleUp,
        GentleUpToLevel,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        LevelToGentleDown,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    for id in [GentleDownToLevel, TurnSmallStandalone, TurnSmallStandalone] {
        expected.append(id, &world, &[]).unwrap();
    }
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let completion = construction
        .find_completion(&world, &[])
        .expect("sloped connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_finishes_the_lift_powered_circuit_after_large_turn_entry() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(TrackState { z: 36, ..start() }, hybrid_model());
    construction.append(Station, &world, &[]).unwrap();
    construction.set_chain(true);
    for id in [
        LevelToGentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUpToLevel,
        Straight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    construction.set_chain(false);
    for id in [
        Straight,
        Straight,
        TurnLargeEntry,
        TurnLargeExit,
        LevelToGentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDownToLevel,
        Loop,
        TurnLargeEntry,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    expected.append(TurnLargeExit, &world, &[]).unwrap();
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let completion = construction
        .find_completion(&world, &[])
        .expect("large turn exit");
    assert_eq!(completion.pieces, [TurnLargeExit]);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn elements_offer_complete_slopes_and_banked_turns_and_apply_atomically() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let elements = construction.element_choices(&world, &[]);
    for id in [
        "climb-60",
        "descend-60",
        "small-left-banked-turn",
        "small-right-banked-turn",
        "large-left-banked-turn",
        "large-right-banked-turn",
    ] {
        assert!(
            elements.iter().any(|element| element.id == id),
            "missing {id}"
        );
    }
    let climb = elements
        .iter()
        .find(|element| element.id == "climb-60")
        .unwrap();
    assert!(climb.pieces.len() > 1);
    let mut stale = construction.copy_for_completion();
    stale.append(PieceId::Straight, &world, &[]).unwrap();
    let before = stale.state_hash();
    assert!(stale.apply_element(climb, &world, &[]).is_err());
    assert_eq!(stale.state_hash(), before);
    construction.apply_element(climb, &world, &[]).unwrap();
    assert_eq!(construction.end().slope, Slope::UpSixty);
    assert!(
        construction
            .element_choices(&world, &[])
            .iter()
            .any(|element| element.id == "level")
    );
}

#[test]
fn elements_support_chain_and_gentle_sloped_turns() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    construction.set_chain(true);
    let climb = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "climb-gentle")
        .unwrap();
    construction.apply_element(&climb, &world, &[]).unwrap();
    assert!(
        construction
            .pieces()
            .iter()
            .all(|piece| piece.kind == PieceKind::Lift)
    );
    construction.set_chain(false);
    let turn = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "large-left-banked-turn")
        .expect("sloped banked turn");
    construction.apply_element(&turn, &world, &[]).unwrap();
    assert_eq!(construction.end().slope, Slope::UpGentle);
    assert_eq!(construction.end().bank, crate::track::Bank::Level);
}

#[test]
fn elements_offer_climbing_and_descending_turns_from_level() {
    let world = world();
    let construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let choices = construction.element_choices(&world, &[]);
    for (id, slope) in [
        ("climbing-small-left-turn", Slope::UpGentle),
        ("climbing-large-right-turn", Slope::UpGentle),
        ("descending-small-right-turn", Slope::DownGentle),
        ("descending-large-left-turn", Slope::DownGentle),
    ] {
        let choice = choices.iter().find(|choice| choice.id == id).unwrap();
        let mut placed = construction.copy_for_completion();
        placed.apply_element(choice, &world, &[]).unwrap();
        assert_eq!(placed.end().slope, slope);
        assert_ne!(placed.end().heading, construction.end().heading);
    }
}

#[test]
fn element_apply_revalidates_world_and_keeps_original_on_failure() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let choice = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "small-left-banked-turn")
        .unwrap();
    let blocked = SupportObstacle {
        min: Vec3::new(-1000.0, -1000.0, -1000.0),
        max: Vec3::new(1000.0, 1000.0, 1000.0),
    };
    let before = construction.state_hash();
    assert!(
        construction
            .apply_element(&choice, &world, &[blocked])
            .is_err()
    );
    assert_eq!(construction.state_hash(), before);
    assert!(
        !construction
            .element_choices(&world, &[blocked])
            .iter()
            .any(|element| element.id == choice.id)
    );
}

#[test]
fn element_preview_excludes_reattached_track() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    construction
        .append(PieceId::LevelToGentleUp, &world, &[])
        .unwrap();
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    let first_id = construction.piece_ids()[0];
    construction.delete_from(first_id).unwrap();
    let choice = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|choice| choice.id == "climb-gentle")
        .unwrap();
    assert_eq!(choice.geometry.len(), choice.pieces.len());
    construction.apply_element(&choice, &world, &[]).unwrap();
    assert_eq!(construction.pieces().len(), 2);
    assert!(construction.disconnected_segments().is_empty());
}
