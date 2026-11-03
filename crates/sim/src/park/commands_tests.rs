use crate::facilities::{FacilityKind, ParkFacilities, PlacementError};
use crate::guests::park::ParkGuests;
use crate::park::{Park, commands::ParkCommandError};
use crate::physics::{PhysicsConfig, Train, demo_circuit};
use crate::track::{TrainConfiguration, hybrid_model};
use crate::world::{
    Direction, Path, PathBuildError, PathKind, Position, Scenery, SceneryKind,
    SceneryPlacementError, Surface, TerrainCommand, TerrainEditError, World,
};

fn park_with_small_world() -> Park {
    let mut world = World::new(
        6,
        6,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let entrance = Position {
        x: 0,
        y: 0,
        height: 0,
    };
    world.place_path(Path {
        position: entrance,
        slope: None,
    });
    let facilities = ParkFacilities::new();
    let simulation = ParkGuests::new(42, 0, &world, entrance, 1);
    let circuit = demo_circuit();
    let configuration: TrainConfiguration = hybrid_model().train;
    let train = Train::new(&circuit, &configuration, PhysicsConfig::default(), 0.0, 0.0);
    Park::new(world, facilities, simulation, circuit, train, configuration)
}

#[test]
fn facility_placement_returns_typed_error_without_mutating_the_park() {
    let mut park = park_with_small_world();
    let before = park.world.state_hash();

    let result = park.place_facility(
        FacilityKind::Food,
        Position {
            x: 5,
            y: 5,
            height: 0,
        },
        Direction::North,
    );

    assert_eq!(
        result,
        Err(ParkCommandError::Facility(PlacementError::OutsideWorld))
    );
    assert_eq!(park.world.state_hash(), before);
}

#[test]
fn terrain_edit_returns_typed_error_without_mutating_the_park() {
    let mut park = park_with_small_world();
    let before = park.world.state_hash();

    let result = park.apply_terrain(
        TerrainCommand::Flatten {
            area: crate::world::Brush {
                x: 5,
                y: 5,
                size: 2,
            },
            height: 0,
        },
        None,
    );

    assert_eq!(
        result,
        Err(ParkCommandError::Terrain(TerrainEditError::OutsideWorld))
    );
    assert_eq!(park.world.state_hash(), before);
}

#[test]
fn path_build_returns_typed_error_without_mutating_the_park() {
    let mut park = park_with_small_world();
    let before = park.world.state_hash();

    let result = park.build_path_route(
        Position {
            x: 6,
            y: 0,
            height: 0,
        },
        Position {
            x: 6,
            y: 1,
            height: 0,
        },
        PathKind::Normal,
        0,
    );

    assert_eq!(
        result,
        Err(ParkCommandError::Path(PathBuildError::OutsideWorld))
    );
    assert_eq!(park.world.state_hash(), before);
}

#[test]
fn scenery_command_returns_typed_error_for_bad_orientation_without_panicking() {
    let mut park = park_with_small_world();
    let before = park.world.state_hash();

    let result = park.place_scenery(Scenery {
        kind: SceneryKind::Bench,
        position: Position {
            x: 2,
            y: 2,
            height: 0,
        },
        rotation: 0,
        quarter: 4,
    });

    assert_eq!(
        result,
        Err(ParkCommandError::Scenery(
            SceneryPlacementError::InvalidOrientation
        ))
    );
    assert_eq!(park.world.state_hash(), before);
}
