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
