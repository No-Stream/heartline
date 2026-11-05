use super::*;
use crate::track::{PieceId, TrackState, catalog, hybrid_model};
use crate::world::{Surface, World};

fn straight(y: i32, z: i32) -> Piece {
    catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap()
        .at(TrackState {
            x: 4,
            y,
            z,
            ..TrackState::origin()
        })
        .unwrap()
}

#[test]
fn nearby_obstacles_and_tracks_generate_localized_events() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let piece = straight(4, 3);
    let overhead = ClearanceBox {
        min: Vec3::new(12.0, 7.0, 9.0),
        max: Vec3::new(14.0, 9.0, 9.5),
    };
    let events = detect_proximity(
        &world,
        &[piece],
        &hybrid_model().train,
        &[straight(5, 3)],
        &[overhead],
        &[],
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::Headchopper)
    );
    assert!(
        events
            .iter()
            .all(|event| event.strength > 0.0 && event.strength <= 1.0 && event.distance >= 0.0)
    );
}

#[test]
fn dueling_requires_simultaneous_nearby_parallel_moving_trains() {
    let primary = TrainPosition {
        position: Vec3::new(10.0, 10.0, 10.0),
        tangent: Vec3::new(1.0, 0.0, 0.0),
        piece_index: 0,
        distance: 3.0,
        speed: 10.0,
    };
    let mut other = TrainPosition {
        position: Vec3::new(10.0, 15.0, 10.0),
        ..primary
    };
    assert!(detect_dueling(primary, &[other]).is_some());
    other.speed = 0.0;
    assert!(detect_dueling(primary, &[other]).is_none());
    other.speed = 10.0;
    other.position.y = 100.0;
    assert!(detect_dueling(primary, &[other]).is_none());
}

fn speed_samples(speed: f64, length: usize) -> Vec<TestSample> {
    (0..=length)
        .map(|distance| TestSample {
            distance: distance as f64,
            speed,
            vertical_g: 1.0,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            airtime: false,
            height_m: 0.0,
            inverted: false,
        })
        .collect()
}

#[test]
fn terrain_hugging_requires_sustained_speed_and_ground_proximity() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let track = straight(4, 1);
    let fast = speed_samples(12.0, track.length().ceil() as usize);
    let slow = speed_samples(4.0, track.length().ceil() as usize);
    assert!(
        detect_proximity(
            &world,
            std::slice::from_ref(&track),
            &hybrid_model().train,
            &[],
            &[],
            &fast
        )
        .iter()
        .any(|e| e.kind == ProximityKind::TerrainHugging)
    );
    assert!(
        !detect_proximity(&world, &[track], &hybrid_model().train, &[], &[], &slow)
            .iter()
            .any(|e| e.kind == ProximityKind::TerrainHugging)
    );
}

#[test]
fn tunnel_events_follow_carved_ground() {
    use crate::world::{Brush, TerrainCommand};
    let mut world = World::new(
        32,
        32,
        Surface {
            corners: [12; 4],
            water: None,
        },
    );
    world
        .apply_terrain(TerrainCommand::Carve {
            area: Brush {
                x: 2,
                y: 2,
                size: 4,
            },
            bottom: 4,
            top: 11,
        })
        .unwrap();
    let events = detect_proximity(
        &world,
        &[straight(4, 3)],
        &hybrid_model().train,
        &[],
        &[],
        &[],
    );
    assert!(events.iter().any(|e| e.kind == ProximityKind::Tunnel));
}

#[test]
fn same_coaster_nonadjacent_track_is_detected_and_neighbors_are_excluded() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let train = hybrid_model().train;
    let pieces = [
        straight(4, 3),
        straight(40, 3),
        straight(5, 3),
        straight(50, 3),
    ];
    let events = detect_proximity(&world, &pieces, &train, &[], &[], &[]);
    assert!(
        events
            .iter()
            .any(|event| event.piece_index == 0 && event.kind == ProximityKind::NearMiss)
    );
    let first = straight(4, 3);
    let second = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap()
        .at(first.exit)
        .unwrap();
    let adjacent = [first, second];
    assert!(
        !detect_proximity(&world, &adjacent, &train, &[], &[], &[])
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
}

#[test]
fn detection_is_deterministic_across_thread_counts() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let train = hybrid_model().train;
    let pieces = [straight(4, 3), straight(5, 3)];
    let other = [straight(8, 3)];
    let samples = speed_samples(12.0, 40);
    let expected = detect_proximity(&world, &pieces, &train, &other, &[], &samples);
    for count in [1, 2, 4] {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..count)
                .map(|_| {
                    scope.spawn(|| detect_proximity(&world, &pieces, &train, &other, &[], &samples))
                })
                .collect();
            for handle in handles {
                assert_eq!(handle.join().unwrap(), expected);
            }
        });
    }
}

#[test]
fn near_miss_samples_remain_available_before_a_stronger_later_encounter() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let piece = straight(4, 3);
    let obstacle = |x: f64, y: f64| ClearanceBox {
        min: Vec3::new(x, y, 6.0),
        max: Vec3::new(x + 0.5, y + 0.5, 8.0),
    };
    let events = detect_proximity(
        &world,
        &[piece],
        &hybrid_model().train,
        &[],
        &[obstacle(9.0, 10.8), obstacle(14.0, 10.1)],
        &[],
    );
    let near: Vec<_> = events
        .iter()
        .filter(|event| event.kind == ProximityKind::NearMiss)
        .collect();
    assert!(near.iter().any(|event| event.distance < 2.0));
    assert!(near.iter().any(|event| event.distance > 4.0));
}

#[test]
fn nearby_world_path_is_included_as_an_obstacle() {
    use crate::world::{Path, Position};
    let mut world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.place_path(Path {
        position: Position {
            x: 3,
            y: 2,
            height: 0,
        },
        slope: None,
    });
    let events = detect_proximity(
        &world,
        &[straight(4, 1)],
        &hybrid_model().train,
        &[],
        &[],
        &[],
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss
                && event.position.x >= 10.0
                && event.position.x <= 18.0)
    );
}

#[test]
fn adjacent_indices_do_not_hide_nearby_unconnected_track() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let events = detect_proximity(
        &world,
        &[straight(4, 3), straight(5, 3)],
        &hybrid_model().train,
        &[],
        &[],
        &[],
    );
    assert!(
        events
            .iter()
            .any(|event| event.piece_index == 0 && event.kind == ProximityKind::NearMiss)
    );
}

#[test]
fn joined_loop_still_checks_track_away_from_the_connector() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let piece = straight(4, 3);
    let mut loop_piece = catalog()
        .iter()
        .find(|item| item.id == PieceId::Loop)
        .unwrap()
        .at(piece.entry)
        .unwrap();
    loop_piece.exit = loop_piece.entry;
    loop_piece.rebuild_arc();
    let events = detect_proximity(
        &world,
        &[loop_piece, piece],
        &hybrid_model().train,
        &[],
        &[],
        &[],
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
}

#[test]
fn support_segments_detect_nearby_and_overhead_sections_without_filling_their_bounding_box() {
    use crate::track::supports::SupportSegment;
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let piece = straight(4, 3);
    let nearby = SupportSegment {
        start: Vec3::new(12.0, 10.0, 6.0),
        end: Vec3::new(14.0, 10.0, 8.0),
    };
    let overhead = SupportSegment {
        start: Vec3::new(12.0, 6.0, 9.0),
        end: Vec3::new(12.0, 10.0, 9.0),
    };
    let events = scan_proximity_with_supports(
        &world,
        &[piece],
        &hybrid_model().train,
        &[],
        &[],
        &[
            OwnedSupportSegment {
                piece_index: None,
                segment: nearby,
            },
            OwnedSupportSegment {
                piece_index: None,
                segment: overhead,
            },
        ],
    )
    .events_for_run(&[]);
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == ProximityKind::Headchopper)
    );
}

#[test]
fn diagonal_support_does_not_fill_its_horizontal_bounding_rectangle() {
    use crate::track::supports::SupportSegment;
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let support = SupportSegment {
        start: Vec3::new(8.0, -24.0, 7.0),
        end: Vec3::new(40.0, 8.0, 7.0),
    };
    let events = scan_proximity_with_supports(
        &world,
        &[straight(4, 3)],
        &hybrid_model().train,
        &[],
        &[],
        &[OwnedSupportSegment {
            piece_index: None,
            segment: support,
        }],
    )
    .events_for_run(&[]);
    assert!(
        !events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
}

#[test]
fn a_piece_does_not_report_its_own_routed_supports() {
    use crate::track::supports::generate_steel_columns;
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let mut piece = catalog()
        .iter()
        .find(|item| item.id == PieceId::Loop)
        .unwrap()
        .at(TrackState {
            x: 8,
            y: 8,
            z: 3,
            ..TrackState::origin()
        })
        .unwrap();
    piece.exit = piece.entry;
    piece.rebuild_arc();
    let supports = generate_steel_columns(&piece, &world, &[]).unwrap();
    let owned_supports: Vec<_> = supports
        .into_iter()
        .map(|segment| OwnedSupportSegment {
            piece_index: Some(0),
            segment,
        })
        .collect();
    let events = scan_proximity_with_supports(
        &world,
        &[piece],
        &hybrid_model().train,
        &[],
        &[],
        &owned_supports,
    )
    .events_for_run(&[]);
    assert!(
        !events
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
}

#[test]
fn a_later_track_section_reports_a_near_miss_with_an_earlier_sections_support() {
    let world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let pieces = [straight(4, 3), straight(5, 3), straight(6, 3)];
    let support = SupportSegment {
        start: Vec3::new(12.0, 10.0, 6.0),
        end: Vec3::new(14.0, 10.0, 8.0),
    };
    let events = scan_proximity_with_supports(
        &world,
        &pieces,
        &hybrid_model().train,
        &[],
        &[],
        &[OwnedSupportSegment {
            piece_index: Some(0),
            segment: support,
        }],
    )
    .events_for_run(&[]);
    assert!(
        events
            .iter()
            .any(|event| { event.piece_index == 2 && event.kind == ProximityKind::NearMiss })
    );
}

#[test]
fn placed_scenery_participates_in_clearance_and_proximity() {
    use crate::track::clearance::{ClearanceCollision, check_clearance};
    use crate::world::{Position, Scenery, SceneryKind};
    let mut world = World::new(
        32,
        32,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let piece = straight(4, 3);
    let mut tree = Scenery {
        kind: SceneryKind::TreeOval,
        position: Position {
            x: 3,
            y: 2,
            height: 0,
        },
        quarter: 0,
        rotation: 0,
    };
    world.place_scenery(tree).unwrap();
    assert_eq!(
        check_clearance(&world, &piece, &hybrid_model().train, &[], &[]),
        Err(ClearanceCollision::Scenery)
    );
    world.remove_scenery(tree.position, tree.quarter);
    tree.quarter = 3;
    world.place_scenery(tree).unwrap();
    assert!(check_clearance(&world, &piece, &hybrid_model().train, &[], &[]).is_ok());
    assert!(
        detect_proximity(&world, &[piece], &hybrid_model().train, &[], &[], &[])
            .iter()
            .any(|event| event.kind == ProximityKind::NearMiss)
    );
}
