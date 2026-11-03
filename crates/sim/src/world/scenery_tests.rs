use super::{
    Brush, Path, Position, Scenery, SceneryKind, SceneryPlacementError, Surface, TerrainCommand,
    TerrainEditError, World,
};

fn item(kind: SceneryKind, x: u16, y: u16, quarter: u8) -> Scenery {
    Scenery {
        kind,
        position: Position { x, y, height: 0 },
        quarter,
        rotation: 0,
    }
}

#[test]
fn placement_checks_occupancy_and_removal() {
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let tree = item(SceneryKind::TreeOval, 2, 2, 0);
    assert_eq!(world.place_scenery(tree), Ok(()));
    assert_eq!(
        world.place_scenery(tree),
        Err(SceneryPlacementError::Occupied)
    );
    assert_eq!(
        world.place_scenery(item(SceneryKind::TreeCone, 2, 2, 1)),
        Ok(())
    );
    assert_eq!(
        world.place_scenery(item(SceneryKind::Fountain, 2, 2, 3)),
        Err(SceneryPlacementError::Occupied)
    );
    assert_eq!(world.nearby_scenery(tree.position, 1).count(), 2);
    assert!(world.remove_scenery(tree.position, 0));
    assert!(!world.remove_scenery(tree.position, 0));
}

#[test]
fn scenery_and_paths_block_each_other_and_terrain_edits_are_atomic() {
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.place_path(Path {
        position: Position {
            x: 1,
            y: 1,
            height: 0,
        },
        slope: None,
    });
    assert_eq!(
        world.place_scenery(item(SceneryKind::Lamp, 1, 1, 0)),
        Err(SceneryPlacementError::Occupied)
    );
    world
        .place_scenery(item(SceneryKind::Lamp, 3, 3, 0))
        .unwrap();
    let before = world.revision();
    assert_eq!(
        world.apply_terrain(TerrainCommand::RaiseTiles {
            area: Brush {
                x: 3,
                y: 3,
                size: 1
            },
            steps: 1
        }),
        Err(TerrainEditError::SceneryObstructed)
    );
    assert_eq!(world.surface(3, 3).corners, [0; 4]);
    assert_eq!(world.revision(), before);
}

#[test]
fn scenery_requires_flat_dry_supported_ground_and_valid_orientation() {
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.set_surface(
        2,
        2,
        Surface {
            corners: [0, 1, 1, 0],
            water: None,
        },
    );
    assert_eq!(
        world.place_scenery(item(SceneryKind::Bench, 2, 2, 0)),
        Err(SceneryPlacementError::InvalidTerrain)
    );
    world.set_surface(
        2,
        2,
        Surface {
            corners: [0; 4],
            water: Some(1),
        },
    );
    assert_eq!(
        world.place_scenery(item(SceneryKind::Bench, 2, 2, 0)),
        Err(SceneryPlacementError::InvalidTerrain)
    );
    let mut invalid = item(SceneryKind::Bench, 3, 3, 0);
    invalid.rotation = 4;
    assert_eq!(
        world.place_scenery(invalid),
        Err(SceneryPlacementError::InvalidOrientation)
    );
}

#[test]
fn path_preview_refuses_scenery() {
    use super::PathKind;
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .place_scenery(item(SceneryKind::Bench, 3, 3, 0))
        .unwrap();
    let start = Position {
        x: 2,
        y: 3,
        height: 0,
    };
    let end = Position {
        x: 3,
        y: 3,
        height: 0,
    };
    assert!(
        world
            .build_path_route(start, end, PathKind::Normal, 0)
            .is_err()
    );
}

#[test]
fn low_level_terrain_changes_cannot_invalidate_scenery() {
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .place_scenery(item(SceneryKind::Lamp, 3, 3, 0))
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        world.set_surface(
            3,
            3,
            Surface {
                corners: [1; 4],
                water: None,
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(world.surface(3, 3).corners, [0; 4]);
    assert_eq!(world.scenery().count(), 1);
}

#[test]
fn facility_placement_refuses_existing_scenery() {
    use super::Direction;
    use crate::facilities::{FacilityKind, ParkFacilities, PlacementError};
    let mut world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .place_scenery(item(SceneryKind::Lamp, 3, 3, 0))
        .unwrap();
    assert_eq!(
        ParkFacilities::new().place(
            &world,
            FacilityKind::Food,
            Position {
                x: 3,
                y: 3,
                height: 0
            },
            Direction::North
        ),
        Err(PlacementError::Occupied)
    );
}

#[test]
fn state_hash_covers_scenery_and_is_independent_of_placement_order() {
    let surface = Surface {
        corners: [0; 4],
        water: None,
    };
    let empty = World::new(8, 8, surface);
    let first_item = item(SceneryKind::Lamp, 2, 2, 0);
    let second_item = item(SceneryKind::TreeCone, 4, 4, 1);
    let mut first = empty.clone();
    let mut second = empty.clone();
    first.place_scenery(first_item).unwrap();
    first.place_scenery(second_item).unwrap();
    second.place_scenery(second_item).unwrap();
    second.place_scenery(first_item).unwrap();
    assert_eq!(first.state_hash(), second.state_hash());
    assert_ne!(first.state_hash(), empty.state_hash());
    for changed in [
        Scenery {
            kind: SceneryKind::Bench,
            ..first_item
        },
        Scenery {
            position: Position {
                x: 3,
                ..first_item.position
            },
            ..first_item
        },
        Scenery {
            quarter: 1,
            ..first_item
        },
        Scenery {
            rotation: 1,
            ..first_item
        },
    ] {
        let mut altered = empty.clone();
        altered.place_scenery(changed).unwrap();
        altered.place_scenery(second_item).unwrap();
        assert_ne!(first.state_hash(), altered.state_hash());
    }
    assert!(first.remove_scenery(first_item.position, first_item.quarter));
    assert!(first.remove_scenery(second_item.position, second_item.quarter));
    assert_eq!(first.state_hash(), empty.state_hash());
}
