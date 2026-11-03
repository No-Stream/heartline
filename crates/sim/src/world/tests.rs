use super::{
    Brush, Corner, Direction, Ground, Path, Position, Surface, TerrainCommand, TerrainEditError,
    World,
};

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
fn corner_sculpting_updates_all_tiles_sharing_the_corner() {
    let mut world = World::new(2, 2, flat_surface(4));

    let result = world
        .apply_terrain(TerrainCommand::RaiseCorner {
            x: 1,
            y: 1,
            corner: Corner::NorthWest,
            steps: 2,
        })
        .unwrap();

    assert_eq!(world.surface(0, 0).corners[2], 6);
    assert_eq!(world.surface(1, 0).corners[3], 6);
    assert_eq!(world.surface(0, 1).corners[1], 6);
    assert_eq!(world.surface(1, 1).corners[0], 6);
    assert_eq!(result.changed_tiles.len(), 4);
}

#[test]
fn tile_brush_raise_lower_flatten_and_smooth_use_quarter_steps() {
    let mut world = World::new(5, 5, flat_surface(4));
    let area = Brush {
        x: 1,
        y: 1,
        size: 3,
    };

    world
        .apply_terrain(TerrainCommand::RaiseTiles { area, steps: 4 })
        .unwrap();
    assert_eq!(world.surface(2, 2).corners, [8; 4]);
    assert_eq!(world.surface(0, 0).corners[0], 4);
    world
        .apply_terrain(TerrainCommand::LowerTiles {
            area: Brush {
                x: 2,
                y: 2,
                size: 1,
            },
            steps: 2,
        })
        .unwrap();
    assert_eq!(world.surface(2, 2).corners, [6; 4]);

    world
        .apply_terrain(TerrainCommand::Flatten { area, height: 5 })
        .unwrap();
    assert_eq!(world.surface(1, 1).corners, [5; 4]);
    assert_eq!(world.surface(3, 3).corners, [5; 4]);

    world
        .apply_terrain(TerrainCommand::RaiseCorner {
            x: 2,
            y: 2,
            corner: Corner::NorthWest,
            steps: 4,
        })
        .unwrap();
    world
        .apply_terrain(TerrainCommand::Smooth {
            area: Brush {
                x: 2,
                y: 2,
                size: 1,
            },
        })
        .unwrap();
    assert!(world.surface(2, 2).corners.iter().any(|height| *height < 9));
}

#[test]
fn water_commands_fill_a_basin_and_lower_its_level() {
    let mut world = World::new(3, 3, flat_surface(8));
    let basin = Brush {
        x: 1,
        y: 1,
        size: 2,
    };
    for x in 1..3 {
        world.set_surface(x, 1, flat_surface(6));
        world.set_surface(x, 2, flat_surface(4));
    }

    world
        .apply_terrain(TerrainCommand::RaiseWater {
            area: basin,
            steps: 3,
        })
        .unwrap();
    for y in 1..3 {
        for x in 1..3 {
            assert_eq!(world.surface(x, y).water, Some(7));
        }
    }
    world
        .apply_terrain(TerrainCommand::LowerWater {
            area: basin,
            steps: 2,
        })
        .unwrap();
    assert_eq!(world.surface(1, 1).water, None);
    assert_eq!(world.surface(1, 2).water, Some(5));
    world
        .apply_terrain(TerrainCommand::SetWater {
            area: basin,
            level: None,
        })
        .unwrap();
    assert_eq!(world.surface(1, 1).water, None);
}

#[test]
fn hillside_tunnel_can_cross_the_low_surface_corner_and_hold_a_path() {
    let mut world = World::new(3, 1, flat_surface(4));
    world
        .apply_terrain(TerrainCommand::RaiseCorner {
            x: 1,
            y: 0,
            corner: Corner::NorthEast,
            steps: 4,
        })
        .unwrap();
    world
        .apply_terrain(TerrainCommand::RaiseCorner {
            x: 1,
            y: 0,
            corner: Corner::SouthEast,
            steps: 4,
        })
        .unwrap();

    let hillside = world.surface(1, 0);
    assert_eq!(hillside.corners, [4, 8, 8, 4]);
    let path_position = position(1, 0, 5);
    let carve_result = world
        .apply_terrain(TerrainCommand::Carve {
            area: Brush {
                x: 1,
                y: 0,
                size: 1,
            },
            bottom: 5,
            top: 7,
        })
        .unwrap();
    assert!(carve_result.invalidated_paths.is_empty());
    assert_eq!(world.ground(1, 0, 6), Ground::Carved);
    world.place_path(Path {
        position: path_position,
        slope: None,
    });
    assert!(world.path(path_position).is_some());
}

#[test]
fn tunnel_brush_clips_its_ceiling_to_each_tile_surface() {
    let mut world = World::new(3, 3, flat_surface(8));
    for corner in [Corner::NorthEast, Corner::SouthEast] {
        world
            .apply_terrain(TerrainCommand::RaiseCorner {
                x: 1,
                y: 0,
                corner,
                steps: 4,
            })
            .unwrap();
    }

    world
        .apply_terrain(TerrainCommand::Carve {
            area: Brush {
                x: 0,
                y: 0,
                size: 3,
            },
            bottom: 4,
            top: 10,
        })
        .unwrap();

    assert_eq!(
        world
            .carved_spans()
            .filter(|(_, y, _, _)| *y == 0)
            .collect::<Vec<_>>(),
        vec![(0, 0, 4, 8), (1, 0, 4, 10), (2, 0, 4, 10)]
    );
    assert_eq!(world.ground(0, 0, 8), Ground::Air);
    assert_eq!(world.ground(1, 0, 9), Ground::Carved);
}

#[test]
fn lowering_a_hillside_ceiling_clips_the_tunnel_and_reports_its_path() {
    let mut world = World::new(3, 1, flat_surface(4));
    for corner in [Corner::NorthEast, Corner::SouthEast] {
        world
            .apply_terrain(TerrainCommand::RaiseCorner {
                x: 1,
                y: 0,
                corner,
                steps: 4,
            })
            .unwrap();
    }
    world
        .apply_terrain(TerrainCommand::Carve {
            area: Brush {
                x: 1,
                y: 0,
                size: 1,
            },
            bottom: 5,
            top: 7,
        })
        .unwrap();
    let path_position = position(1, 0, 5);
    world.place_path(Path {
        position: path_position,
        slope: None,
    });

    world
        .apply_terrain(TerrainCommand::LowerCorner {
            x: 1,
            y: 0,
            corner: Corner::NorthEast,
            steps: 4,
        })
        .unwrap();
    assert!(world.path(path_position).is_some());
    let lowered_ceiling = world
        .apply_terrain(TerrainCommand::LowerCorner {
            x: 1,
            y: 0,
            corner: Corner::SouthEast,
            steps: 4,
        })
        .unwrap();

    assert_eq!(lowered_ceiling.invalidated_paths, vec![path_position]);
    assert!(world.path(path_position).is_none());
    assert_eq!(world.ground(1, 0, 5), Ground::Air);
}

#[test]
fn cavern_carving_spans_multiple_tiles_and_supports_a_path_network() {
    let mut world = World::new(5, 5, flat_surface(12));
    let cavern = Brush {
        x: 1,
        y: 1,
        size: 3,
    };
    world
        .apply_terrain(TerrainCommand::Carve {
            area: cavern,
            bottom: 3,
            top: 8,
        })
        .unwrap();
    for y in 1..4 {
        for x in 1..4 {
            assert_eq!(world.ground(x, y, 6), Ground::Carved);
        }
    }
    let path_position = position(2, 2, 3);
    world.place_path(Path {
        position: path_position,
        slope: None,
    });
    assert!(world.path(path_position).is_some());
    let filled_path = world
        .apply_terrain(TerrainCommand::Fill {
            area: Brush {
                x: 2,
                y: 2,
                size: 1,
            },
            bottom: 3,
            top: 4,
        })
        .unwrap();
    assert_eq!(filled_path.invalidated_paths, vec![path_position]);
    assert!(world.path(path_position).is_none());
    assert_eq!(world.ground(2, 2, 3), Ground::Solid);
}

#[test]
fn invalid_terrain_commands_fail_without_changing_world_state() {
    let mut world = World::new(2, 2, flat_surface(8));
    let original_hash = world.state_hash();
    let error = world
        .apply_terrain(TerrainCommand::Carve {
            area: Brush {
                x: 1,
                y: 1,
                size: 2,
            },
            bottom: 2,
            top: 4,
        })
        .unwrap_err();
    assert_eq!(error, TerrainEditError::OutsideWorld);
    assert_eq!(world.state_hash(), original_hash);

    world.set_surface(1, 0, flat_surface(3));
    world.carve(0, 0, 1, 5);
    let partially_valid_hash = world.state_hash();
    let error = world
        .apply_terrain(TerrainCommand::Fill {
            area: Brush {
                x: 0,
                y: 0,
                size: 2,
            },
            bottom: 1,
            top: 5,
        })
        .unwrap_err();
    assert_eq!(error, TerrainEditError::CarveAboveTerrain);
    assert_eq!(world.state_hash(), partially_valid_hash);

    let error = world
        .apply_terrain(TerrainCommand::RaiseTiles {
            area: Brush {
                x: 0,
                y: 0,
                size: 0,
            },
            steps: 1,
        })
        .unwrap_err();
    assert_eq!(error, TerrainEditError::InvalidBrushSize);
    assert_eq!(world.state_hash(), partially_valid_hash);
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
fn cloned_world_preserves_state_without_reusing_cache_identity() {
    let world = World::new(2, 2, flat_surface(8));
    let cloned = world.clone();

    assert_eq!(cloned.state_hash(), world.state_hash());
    assert_ne!(cloned.instance_id(), world.instance_id());
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
        world.build_path_route(position(0, 1, 0), position(2, 1, 0), PathKind::Queue, 0),
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
fn normal_route_can_start_or_end_on_its_queue_entrance() {
    use super::{PathBuildError, PathKind, PathMetadata};
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
            .build_path_route(position(0, 0, 0), position(0, 0, 0), PathKind::Queue, 3)
            .unwrap();
        world
            .build_path_route(start, end, PathKind::Normal, 1)
            .unwrap();
        assert_eq!(
            world.path_metadata(position(0, 0, 0)),
            Some(PathMetadata {
                kind: PathKind::Queue,
                style: 3
            })
        );
        for x in [1, 2] {
            assert_eq!(
                world.path_metadata(position(x, 0, 0)),
                Some(PathMetadata {
                    kind: PathKind::Normal,
                    style: 1
                })
            );
        }
    }
    let mut world = World::new(
        5,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(position(2, 0, 0), position(2, 0, 0), PathKind::Queue, 0)
        .unwrap();
    assert_eq!(
        world.build_path_route(position(1, 0, 0), position(3, 0, 0), PathKind::Normal, 0),
        Err(PathBuildError::Occupied)
    );
    world
        .build_path_route(position(0, 0, 0), position(2, 0, 0), PathKind::Normal, 0)
        .unwrap();
    assert_eq!(
        world.build_path_route(position(2, 0, 0), position(4, 0, 0), PathKind::Normal, 0),
        Err(PathBuildError::QueueConnections)
    );
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
    let mut world = World::new(
        7,
        3,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(position(1, 1, 0), position(1, 1, 0), PathKind::Queue, 0)
        .unwrap();
    for x in [0, 2] {
        world.place_path(Path {
            position: position(x, 1, 0),
            slope: None,
        });
    }
    world.connect_paths(position(1, 1, 0), position(0, 1, 0));
    world.connect_paths(position(1, 1, 0), position(2, 1, 0));
    assert_eq!(
        world.preview_path_route(position(5, 1, 0), position(6, 1, 0), PathKind::Normal, 0),
        Err(PathBuildError::QueueConnections)
    );
    world.remove_path(position(0, 1, 0));
    assert!(
        world
            .preview_path_route(position(5, 1, 0), position(6, 1, 0), PathKind::Normal, 0)
            .is_ok()
    );
}

#[test]
fn queue_preview_checks_merged_components_and_adjacent_normal_runs() {
    use super::{PathBuildError, PathKind};
    let mut world = World::new(
        7,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for x in [0, 6] {
        world.place_path(Path {
            position: position(x, 0, 0),
            slope: None,
        });
    }
    for (normal, queue) in [(0, 1), (6, 5)] {
        world
            .build_path_route(
                position(normal, 0, 0),
                position(queue, 0, 0),
                PathKind::Queue,
                0,
            )
            .unwrap();
    }
    assert_eq!(
        world.preview_path_route(position(1, 0, 0), position(5, 0, 0), PathKind::Queue, 0),
        Err(PathBuildError::QueueConnections)
    );
    assert_eq!(
        world.preview_path_route(position(1, 1, 0), position(1, 0, 0), PathKind::Normal, 0),
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
        .preview_path_tiles(position(0, 0, 0), position(2, 0, 0), PathKind::Queue, 0)
        .unwrap();
    assert_eq!(preview.len(), 3);
    assert!(
        preview
            .iter()
            .all(|tile| tile.refusal == Some(PathBuildError::QueueConnections))
    );
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
    use crate::guests::walking::WalkingSimulation;
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

#[test]
fn queue_rejects_a_branch_and_a_normal_connection_in_the_middle() {
    use crate::world::{PathBuildError, PathKind};
    let mut world = World::new(
        5,
        5,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let position = |x, y| Position { x, y, height: 0 };
    world
        .build_path_route(position(1, 2), position(3, 2), PathKind::Queue, 0)
        .unwrap();
    assert_eq!(
        world.build_path_route(position(2, 2), position(2, 1), PathKind::Queue, 0),
        Err(PathBuildError::QueueConnections)
    );
    assert_eq!(
        world.build_path_route(position(2, 2), position(2, 1), PathKind::Normal, 0),
        Err(PathBuildError::QueueConnections)
    );
}

#[test]
fn queue_switchback_only_connects_drawn_edges() {
    use super::PathKind;
    let mut world = World::new(
        4,
        3,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(position(0, 0, 0), position(3, 0, 0), PathKind::Queue, 0)
        .unwrap();
    world
        .build_path_route(position(3, 0, 0), position(3, 1, 0), PathKind::Queue, 0)
        .unwrap();
    world
        .build_path_route(position(3, 1, 0), position(0, 1, 0), PathKind::Queue, 0)
        .unwrap();
    assert_eq!(
        world.neighbours(position(1, 0, 0)),
        vec![position(2, 0, 0), position(0, 0, 0)]
    );
    assert_eq!(
        world.neighbours(position(1, 1, 0)),
        vec![position(2, 1, 0), position(0, 1, 0)]
    );
}

#[test]
fn removing_and_rebuilding_a_queue_tile_updates_edges_and_preview() {
    use super::PathKind;
    let mut world = World::new(3, 1, flat_surface(0));
    let left = position(0, 0, 0);
    let middle = position(1, 0, 0);
    let right = position(2, 0, 0);
    world
        .build_path_route(left, right, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(world.path_edges(middle), Some(10));
    world.remove_path(middle);
    assert_eq!(world.path_edges(left), Some(0));
    assert_eq!(world.path_edges(right), Some(0));
    let preview = world
        .preview_path_tiles(left, right, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(
        preview.iter().map(|tile| tile.edges).collect::<Vec<_>>(),
        vec![2, 10, 8]
    );
    world
        .build_path_route(left, right, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(world.path_edges(middle), Some(10));
}

#[test]
fn queue_end_does_not_connect_to_adjacent_exit_path() {
    use super::PathKind;
    let mut world = World::new(4, 2, flat_surface(0));
    let entrance = position(0, 0, 0);
    let exit = position(3, 1, 0);
    let queue_end = position(3, 0, 0);
    for path in [entrance, exit] {
        world
            .build_path_route(path, path, PathKind::Normal, 0)
            .unwrap();
    }
    world
        .build_path_route(entrance, queue_end, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(world.path_edges(queue_end), Some(8));
    assert_eq!(world.path_edges(exit), Some(0));
    let graph = crate::guests::pathfinding::Graph::with_entrance(&world, Some(entrance));
    let queue_node = graph.indices[&queue_end];
    let exit_node = graph.indices[&exit];
    assert!(!graph.neighbours[queue_node].contains(&exit_node));
    assert_ne!(graph.components[queue_node], graph.components[exit_node]);
}

#[test]
fn queue_placed_next_to_normal_path_stays_disconnected_until_drawn() {
    use super::PathKind;
    let mut world = World::new(2, 1, flat_surface(0));
    let normal = position(0, 0, 0);
    let queue = position(1, 0, 0);
    world
        .build_path_route(normal, normal, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(queue, queue, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(world.path_edges(queue), Some(0));
    assert_eq!(world.path_edges(normal), Some(0));
    world
        .build_path_route(normal, queue, PathKind::Queue, 0)
        .unwrap();
    assert_eq!(world.path_edges(queue), Some(8));
    assert_eq!(world.path_edges(normal), Some(2));
}

#[test]
fn state_hash_includes_queue_edges_independent_of_route_direction() {
    use super::PathKind;
    let mut connected = World::new(3, 2, flat_surface(0));
    for (start, end) in [((0, 0), (2, 0)), ((2, 0), (2, 1)), ((2, 1), (0, 1))] {
        connected
            .build_path_route(
                position(start.0, start.1, 0),
                position(end.0, end.1, 0),
                PathKind::Queue,
                0,
            )
            .unwrap();
    }
    let mut reversed = World::new(3, 2, flat_surface(0));
    for (start, end) in [((0, 1), (2, 1)), ((2, 1), (2, 0)), ((2, 0), (0, 0))] {
        reversed
            .build_path_route(
                position(start.0, start.1, 0),
                position(end.0, end.1, 0),
                PathKind::Queue,
                0,
            )
            .unwrap();
    }
    let mut separated = World::new(3, 2, flat_surface(0));
    for y in 0..=1 {
        separated
            .build_path_route(position(0, y, 0), position(2, y, 0), PathKind::Queue, 0)
            .unwrap();
    }
    assert_eq!(
        connected.paths().collect::<Vec<_>>(),
        separated.paths().collect::<Vec<_>>()
    );
    assert_eq!(connected.state_hash(), reversed.state_hash());
    assert_ne!(connected.state_hash(), separated.state_hash());
}
