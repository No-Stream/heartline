use super::{Direction, Ground, Path, Position, Surface, World};

fn flat_surface(height: i16) -> Surface {
    Surface {
        corners: [height; 4],
        water: None,
    }
}

fn position(x: u16, y: u16, height: i16) -> Position {
    Position { x, y, height }
}

#[test]
fn surface_initialization_is_queryable_and_part_of_the_state_hash() {
    let flat = flat_surface(8);
    let world = World::new(2, 2, flat);

    let first_tile = world.surface(0, 0);
    assert_eq!(first_tile.corners, [8; 4]);
    assert_eq!(first_tile.water, None);
    let flat_hash = world.state_hash();

    let varied_surface = Surface {
        corners: [6, 8, 10, 8],
        water: Some(5),
    };
    let varied_world = World::new(2, 2, varied_surface);
    let last_tile = varied_world.surface(1, 1);
    assert_eq!(last_tile.corners, varied_surface.corners);
    assert_eq!(last_tile.water, varied_surface.water);
    assert_ne!(flat_hash, varied_world.state_hash());
}

#[test]
fn ground_queries_distinguish_solid_carved_air_and_surface_slopes() {
    let mut world = World::new(2, 2, flat_surface(8));

    assert_eq!(world.ground(0, 0, 7), Ground::Solid);
    assert_eq!(world.ground(0, 0, 8), Ground::Air);

    world.carve(0, 0, 3, 5);
    assert_eq!(world.ground(0, 0, 3), Ground::Carved);
    assert_eq!(world.ground(0, 0, 4), Ground::Carved);
    assert_eq!(world.ground(0, 0, 5), Ground::Solid);

    world.set_surface(
        1,
        0,
        Surface {
            corners: [8, 9, 9, 8],
            water: None,
        },
    );
    assert_eq!(world.ground(1, 0, 8), Ground::SurfaceSlope);
}

#[test]
fn changing_the_surface_clips_excavation_above_its_new_minimum_corner() {
    let mut world = World::new(1, 1, flat_surface(8));
    world.carve(0, 0, 3, 7);
    assert_eq!(world.ground(0, 0, 6), Ground::Carved);

    world.set_surface(0, 0, flat_surface(5));

    assert_eq!(world.ground(0, 0, 3), Ground::Carved);
    assert_eq!(world.ground(0, 0, 4), Ground::Carved);
    assert_eq!(world.ground(0, 0, 5), Ground::Air);
    assert_eq!(world.ground(0, 0, 6), Ground::Air);
}

#[test]
fn terrain_edits_remove_paths_when_the_path_or_its_support_is_filled() {
    let mut world = World::new(3, 3, flat_surface(8));
    let underground_path = position(1, 1, 3);

    world.carve(1, 1, 3, 5);
    world.place_path(Path {
        position: underground_path,
        slope: None,
    });
    assert!(world.path(underground_path).is_some());

    let removed_by_fill = world.fill(1, 1, 3, 4);
    assert_eq!(removed_by_fill, vec![underground_path]);
    assert!(world.path(underground_path).is_none());

    let surface_path = position(2, 1, 8);
    world.place_path(Path {
        position: surface_path,
        slope: None,
    });
    let removed_by_carve = world.carve(2, 1, 7, 8);
    assert_eq!(removed_by_carve, vec![surface_path]);
    assert!(world.path(surface_path).is_none());
}

#[test]
fn flat_and_sloped_paths_connect_across_levels() {
    let mut world = World::new(5, 3, flat_surface(8));
    let lower_flat = position(1, 1, 8);
    let slope = position(2, 1, 8);
    let upper_flat = position(3, 1, 9);

    world.set_surface(
        2,
        1,
        Surface {
            corners: [8, 9, 9, 8],
            water: None,
        },
    );
    world.set_surface(3, 1, flat_surface(9));
    world.place_path(Path {
        position: lower_flat,
        slope: None,
    });
    world.place_path(Path {
        position: slope,
        slope: Some(Direction::East),
    });
    world.place_path(Path {
        position: upper_flat,
        slope: None,
    });

    let lower_neighbours = world.neighbours(lower_flat);
    assert_eq!(lower_neighbours.len(), 1);
    assert!(lower_neighbours.contains(&slope));

    let slope_neighbours = world.neighbours(slope);
    assert_eq!(slope_neighbours.len(), 2);
    assert!(slope_neighbours.contains(&lower_flat));
    assert!(slope_neighbours.contains(&upper_flat));

    let removed_path = world.remove_path(slope);
    assert_eq!(removed_path, vec![slope]);
    assert!(world.neighbours(lower_flat).is_empty());
    assert!(world.neighbours(upper_flat).is_empty());
}

#[test]
fn underground_path_loop_remains_connected_under_a_surface_path() {
    let mut world = World::new(4, 4, flat_surface(8));
    let overhead_path = position(1, 1, 8);
    world.place_path(Path {
        position: overhead_path,
        slope: None,
    });

    let loop_paths = [
        position(1, 1, 3),
        position(2, 1, 3),
        position(2, 2, 3),
        position(1, 2, 3),
    ];
    for underground_path in loop_paths {
        world.carve(underground_path.x, underground_path.y, 3, 5);
        world.place_path(Path {
            position: underground_path,
            slope: None,
        });
    }

    assert!(world.path(overhead_path).is_some());
    assert_eq!(world.ground(1, 1, 3), Ground::Carved);
    for underground_path in loop_paths {
        assert_eq!(world.neighbours(underground_path).len(), 2);
    }
}

#[test]
fn state_hash_is_reproducible_and_changes_with_world_edits() {
    let surface = flat_surface(8);
    let mut first = World::new(4, 4, surface);
    let mut second = World::new(4, 4, surface);
    assert_eq!(first.state_hash(), second.state_hash());

    let first_path = position(1, 1, 3);
    let second_path = position(2, 1, 3);
    first.carve(1, 1, 3, 5);
    first.carve(2, 1, 3, 5);
    second.carve(2, 1, 3, 5);
    second.carve(1, 1, 3, 5);
    first.place_path(Path {
        position: first_path,
        slope: None,
    });
    second.place_path(Path {
        position: second_path,
        slope: None,
    });
    first.place_path(Path {
        position: second_path,
        slope: None,
    });
    second.place_path(Path {
        position: first_path,
        slope: None,
    });
    assert_eq!(first.state_hash(), second.state_hash());

    first.remove_path(first_path);
    assert_ne!(first.state_hash(), second.state_hash());
}

#[test]
fn a_512_by_512_world_with_4608_developed_tiles_fits_in_8_mib() {
    let mut world = World::new(512, 512, flat_surface(8));

    for x in 0..512 {
        world.place_path(Path {
            position: position(x, 256, 8),
            slope: None,
        });
    }
    for x in 128..192 {
        for y in 128..192 {
            world.carve(x, y, 3, 5);
            world.place_path(Path {
                position: position(x, y, 3),
                slope: None,
            });
        }
    }

    assert_eq!(world.surface(0, 0).corners, [8; 4]);
    assert_eq!(world.surface(511, 511).corners, [8; 4]);
    assert_eq!(world.ground(128, 128, 3), Ground::Carved);
    let allocated_bytes = world.allocated_bytes();
    let budget = 8 * 1024 * 1024;
    println!("allocated world bytes: {allocated_bytes}; budget bytes: {budget}");
    assert!(allocated_bytes <= budget);
}

#[test]
#[should_panic]
fn world_dimensions_must_be_positive() {
    World::new(0, 3, flat_surface(8));
}

#[test]
#[should_panic]
fn terrain_edits_must_stay_inside_the_world() {
    let mut world = World::new(2, 2, flat_surface(8));
    world.carve(2, 0, 1, 2);
}

#[test]
#[should_panic]
fn terrain_intervals_must_be_nonempty_and_ordered() {
    let mut world = World::new(2, 2, flat_surface(8));
    world.fill(0, 0, 4, 4);
}

#[test]
#[should_panic]
fn path_positions_must_stay_inside_the_world() {
    let mut world = World::new(2, 2, flat_surface(8));
    world.place_path(Path {
        position: position(2, 0, 8),
        slope: None,
    });
}

#[test]
#[should_panic(expected = "paths overlap")]
fn paths_cannot_overlap_the_same_occupied_space() {
    let mut world = World::new(2, 2, flat_surface(8));
    world.place_path(Path {
        position: position(0, 0, 8),
        slope: None,
    });
    world.place_path(Path {
        position: position(0, 0, 8),
        slope: None,
    });
}

#[test]
fn carved_span_iteration_is_canonical_and_reflects_edits() {
    let mut world = World::new(3, 2, flat_surface(8));
    world.carve(2, 1, 1, 3);
    world.carve(0, 0, 4, 6);
    world.carve(0, 0, 1, 3);
    assert_eq!(
        world.carved_spans().collect::<Vec<_>>(),
        vec![(0, 0, 1, 3), (0, 0, 4, 6), (2, 1, 1, 3)]
    );
    world.fill(0, 0, 2, 5);
    assert_eq!(
        world.carved_spans().collect::<Vec<_>>(),
        vec![(0, 0, 1, 2), (0, 0, 5, 6), (2, 1, 1, 3)]
    );
}

#[test]
fn occupied_column_index_tracks_paths_and_excavations_through_edits() {
    let mut world = World::new(100, 100, flat_surface(8));
    let path = Path {
        position: position(90, 90, 8),
        slope: None,
    };
    world.place_path(path);
    world.carve(1, 2, 1, 4);
    world.carve(90, 90, 1, 4);
    assert_eq!(world.occupied_columns, vec![201, 9090]);
    assert_eq!(world.paths().collect::<Vec<_>>(), vec![path]);
    world.remove_path(path.position);
    assert_eq!(world.occupied_columns, vec![201, 9090]);
    world.fill(90, 90, 1, 4);
    assert_eq!(world.occupied_columns, vec![201]);
    world.set_surface(1, 2, flat_surface(1));
    assert!(world.occupied_columns.is_empty());
    assert!(world.paths().next().is_none());
    assert!(world.carved_spans().next().is_none());
}

#[test]
fn path_route_follows_terrain_and_is_atomic_on_invalid_end() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.set_surface(
        1,
        0,
        Surface {
            corners: [0, 1, 1, 0],
            water: None,
        },
    );
    world.set_surface(
        2,
        0,
        Surface {
            corners: [1; 4],
            water: None,
        },
    );
    let start = position(0, 0, 0);
    let end = position(2, 0, 1);
    let route = world
        .build_path_route(start, end, PathKind::Queue, 2)
        .unwrap();
    assert_eq!(route.len(), 3);
    assert_eq!(route[1].slope, Some(Direction::East));
    assert_eq!(world.neighbours(start), vec![route[1].position]);
    assert_eq!(world.path_metadata(end).unwrap().kind, PathKind::Queue);
    let revision = world.revision();
    assert_eq!(
        world.build_path_route(position(0, 1, 0), position(3, 1, 1), PathKind::Normal, 0),
        Err(PathBuildError::EndpointHeight)
    );
    assert_eq!(world.revision(), revision);
    assert!(world.path(position(0, 1, 0)).is_none());
    world.remove_path(end);
    assert!(world.path_metadata(end).is_none());
}

#[test]
fn path_route_reuses_existing_paths_of_the_same_kind_with_a_different_style() {
    use super::{PathKind, PathMetadata};
    let mut world = World::new(
        3,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(position(0, 0, 0), position(0, 0, 0), PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(position(0, 0, 0), position(2, 0, 0), PathKind::Normal, 1)
        .unwrap();
    assert_eq!(
        world.path_metadata(position(0, 0, 0)),
        Some(PathMetadata {
            kind: PathKind::Normal,
            style: 0
        })
    );
    assert_eq!(
        world.path_metadata(position(1, 0, 0)),
        Some(PathMetadata {
            kind: PathKind::Normal,
            style: 1
        })
    );
    assert_eq!(
        world.path_metadata(position(2, 0, 0)),
        Some(PathMetadata {
            kind: PathKind::Normal,
            style: 1
        })
    );
}

#[test]
fn path_route_supports_excavated_tunnels_and_rejects_disconnected_terrain() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        3,
        2,
        Surface {
            corners: [5; 4],
            water: None,
        },
    );
    for x in 0..3 {
        world.carve(x, 0, 2, 4);
    }
    let route = world
        .build_path_route(position(0, 0, 2), position(2, 0, 2), PathKind::Normal, 0)
        .unwrap();
    assert_eq!(world.neighbours(route[1].position).len(), 2);
    world.set_surface(
        1,
        1,
        Surface {
            corners: [4; 4],
            water: None,
        },
    );
    assert_eq!(
        world.preview_path_route(position(0, 1, 5), position(2, 1, 5), PathKind::Normal, 0),
        Err(PathBuildError::Disconnected)
    );
}

#[test]
fn queue_route_rejects_multiple_connections_to_regular_paths() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        3,
        3,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for (x, y) in [(0, 1), (2, 1)] {
        world.place_path(Path {
            position: position(x, y, 0),
            slope: None,
        });
    }
    assert_eq!(
        world.build_path_route(position(1, 1, 0), position(1, 1, 0), PathKind::Queue, 0),
        Err(PathBuildError::QueueConnections)
    );
    assert!(world.path(position(1, 1, 0)).is_none());
}

#[test]
fn queue_route_can_start_or_end_on_its_normal_path_entrance() {
    use super::{PathKind, PathMetadata};
    for (start, end) in [
        (position(0, 0, 0), position(2, 0, 0)),
        (position(2, 0, 0), position(0, 0, 0)),
    ] {
        let mut world = World::new(
            3,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world
            .build_path_route(position(0, 0, 0), position(0, 0, 0), PathKind::Normal, 3)
            .unwrap();
        world
            .build_path_route(start, end, PathKind::Queue, 1)
            .unwrap();
        assert_eq!(
            world.path_metadata(position(0, 0, 0)),
            Some(PathMetadata {
                kind: PathKind::Normal,
                style: 3
            })
        );
        for x in [1, 2] {
            assert_eq!(
                world.path_metadata(position(x, 0, 0)),
                Some(PathMetadata {
                    kind: PathKind::Queue,
                    style: 1
                })
            );
        }
        assert_eq!(world.neighbours(position(0, 0, 0)), vec![position(1, 0, 0)]);
    }
}

#[test]
fn queue_route_cannot_cross_a_normal_path_or_connect_two_normal_paths() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        5,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for x in [1, 3] {
        world
            .build_path_route(position(x, 0, 0), position(x, 0, 0), PathKind::Normal, 0)
            .unwrap();
    }
    assert_eq!(
        world.build_path_route(position(0, 0, 0), position(2, 0, 0), PathKind::Queue, 0),
        Err(PathBuildError::Occupied)
    );
    assert_eq!(
        world.build_path_route(position(1, 0, 0), position(3, 0, 0), PathKind::Queue, 0),
        Err(PathBuildError::QueueConnections)
    );
    assert!(world.path(position(2, 0, 0)).is_none());
}

#[test]
fn path_preview_checks_existing_queue_components_away_from_the_route() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(7, 3, Surface { corners: [0; 4], water: None });
    world.build_path_route(position(1, 1, 0), position(1, 1, 0), PathKind::Queue, 0).unwrap();
    for x in [0, 2] {
        world.place_path(Path { position: position(x, 1, 0), slope: None });
    }
    assert_eq!(
        world.preview_path_route(position(5, 1, 0), position(6, 1, 0), PathKind::Normal, 0),
        Err(PathBuildError::QueueConnections)
    );
    world.remove_path(position(0, 1, 0));
    assert!(world.preview_path_route(position(5, 1, 0), position(6, 1, 0), PathKind::Normal, 0).is_ok());
}

#[test]
fn queue_preview_checks_merged_components_and_adjacent_normal_runs() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(7, 2, Surface { corners: [0; 4], water: None });
    for x in [0, 6] {
        world.place_path(Path { position: position(x, 0, 0), slope: None });
    }
    for x in [1, 5] {
        world.build_path_route(position(x, 0, 0), position(x, 0, 0), PathKind::Queue, 0).unwrap();
    }
    assert_eq!(
        world.preview_path_route(position(1, 0, 0), position(5, 0, 0), PathKind::Queue, 0),
        Err(PathBuildError::QueueConnections)
    );
    assert_eq!(
        world.preview_path_route(position(1, 1, 0), position(5, 1, 0), PathKind::Normal, 0),
        Err(PathBuildError::QueueConnections)
    );
}

#[test]
fn path_preview_preserves_valid_prefix_and_marks_refused_tiles_without_editing() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        4,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.set_surface(
        2,
        0,
        Surface {
            corners: [3; 4],
            water: None,
        },
    );
    let revision = world.revision();
    let preview = world
        .preview_path_tiles(position(0, 0, 0), position(3, 0, 0), PathKind::Normal, 0)
        .unwrap();
    assert_eq!(
        preview
            .iter()
            .map(|tile| tile.path.position.x)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(preview[0].refusal, None);
    assert_eq!(preview[1].refusal, None);
    assert!(matches!(
        preview[2].refusal,
        Some(PathBuildError::Disconnected | PathBuildError::Unsupported)
    ));
    assert_eq!(preview[3].refusal, Some(PathBuildError::Disconnected));
    assert_eq!(world.revision(), revision);
    assert!(world.paths().next().is_none());
    assert!(
        world
            .build_path_route(position(0, 0, 0), position(3, 0, 0), PathKind::Normal, 0)
            .is_err()
    );
}

#[test]
fn path_preview_marks_queue_connection_refusal_on_the_route() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        3,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for x in [0, 2] {
        world.place_path(Path {
            position: position(x, 0, 0),
            slope: None,
        });
    }
    let preview = world
        .preview_path_tiles(position(1, 0, 0), position(1, 0, 0), PathKind::Queue, 0)
        .unwrap();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].refusal, Some(PathBuildError::QueueConnections));
}

#[test]
fn route_turns_and_descends_into_an_excavated_tunnel() {
    use super::PathKind;
    let mut world = World::new(
        3,
        2,
        Surface {
            corners: [4; 4],
            water: None,
        },
    );
    world.set_surface(
        0,
        0,
        Surface {
            corners: [2; 4],
            water: None,
        },
    );
    world.carve(1, 0, 1, 4);
    world.carve(2, 0, 1, 3);
    world.carve(2, 1, 1, 3);
    let start = position(0, 0, 2);
    let end = position(2, 1, 1);
    let route = world
        .build_path_route(start, end, PathKind::Normal, 0)
        .unwrap();
    assert_eq!(
        route
            .iter()
            .map(|path| (path.position.x, path.position.y))
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 0), (2, 0), (2, 1)]
    );
    assert_eq!(route[1].slope, Some(Direction::West));
    for pair in route.windows(2) {
        assert!(
            world
                .neighbours(pair[0].position)
                .contains(&pair[1].position)
        );
    }
}

#[test]
fn guests_recover_after_removing_a_built_path_and_use_a_rebuilt_network() {
    use super::PathKind;
    use crate::walking::WalkingSimulation;
    let mut world = World::new(
        4,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = position(0, 0, 0);
    let end = position(3, 0, 0);
    world
        .build_path_route(start, end, PathKind::Normal, 0)
        .unwrap();
    let mut guests = WalkingSimulation::new(5, 80, &world);
    let removed = position(1, 0, 0);
    let affected: Vec<_> = guests
        .guests()
        .iter()
        .enumerate()
        .filter(|(_, guest)| guest.path == Some(removed))
        .map(|(index, _)| index)
        .collect();
    assert!(!affected.is_empty());
    world.remove_path(removed);
    guests.tick(&world);
    assert!(affected.iter().all(|&index| {
        guests.guests()[index]
            .path
            .is_some_and(|path| world.path(path).is_some())
    }));
    assert_ne!(guests.path_component(start), guests.path_component(end));
    world
        .build_path_route(removed, removed, PathKind::Normal, 0)
        .unwrap();
    guests.tick(&world);
    assert_eq!(guests.path_component(start), guests.path_component(end));
    let distance: f32 = guests.guests().iter().map(|guest| guest.distance).sum();
    for _ in 0..200 {
        guests.tick(&world);
    }
    assert!(
        guests
            .guests()
            .iter()
            .map(|guest| guest.distance)
            .sum::<f32>()
            > distance
    );
}
