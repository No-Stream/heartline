use crate::facilities::{FacilityKind, ParkFacilities, PlacementError};
use crate::guests::park::ParkGuests;
use crate::park::{Park, commands::ParkCommandError};
use crate::physics::{PhysicsConfig, Train, demo_circuit};
use crate::track::clearance::TunnelPlan;
use crate::track::construction::Construction;
use crate::track::{PieceId, TrackState};
use crate::track::{TrainConfiguration, hybrid_model};
use crate::world::{
    Direction, Path, PathBuildError, PathKind, Position, Scenery, SceneryKind,
    SceneryPlacementError, Surface, TerrainCommand, TerrainEditError, World,
};
use std::collections::BTreeSet;

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

fn tunnel_fixture() -> (World, TunnelPlan) {
    let world = World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    let construction = Construction::new(
        TrackState {
            x: 50,
            y: 50,
            z: 11,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    let plan = construction
        .tunnel_plan(PieceId::LevelToGentleDown, &world, &[])
        .expect("descending piece fits through the hill");
    assert!(plan.would_tunnel());
    (world, plan)
}

fn empty_park(world: World) -> Park {
    let entrance = Position {
        x: 0,
        y: 0,
        height: 20,
    };
    let mut world = world;
    world.place_path(Path {
        position: entrance,
        slope: None,
    });
    let simulation = ParkGuests::new(42, 0, &world, entrance, 1);
    Park::new_empty(world, ParkFacilities::new(), simulation)
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
fn tunnel_restore_keeps_shared_cells_until_the_last_piece_is_removed() {
    let (world, plan) = tunnel_fixture();
    let mut park = empty_park(world);
    let original_hash = park.world.state_hash();

    park.carve_for_construction(&[(0, plan.clone())]).unwrap();
    let carved_hash = park.world.state_hash();
    assert_ne!(carved_hash, original_hash);
    park.carve_for_construction(&[(1, plan)]).unwrap();
    park.restore_tunnel_piece(0);
    assert_eq!(park.world.state_hash(), carved_hash);
    park.restore_tunnel_piece(1);
    assert_eq!(park.world.state_hash(), original_hash);
}

#[test]
fn placing_track_carves_exactly_the_clearance_cells_in_its_tunnel_plan() {
    let (world, plan) = tunnel_fixture();
    let mut park = empty_park(world);
    let before: BTreeSet<_> = park
        .world
        .carved_spans()
        .flat_map(|(x, y, bottom, top)| (bottom..top).map(move |height| (x, y, height)))
        .collect();

    park.carve_for_construction(&[(0, plan.clone())]).unwrap();

    let after: BTreeSet<_> = park
        .world
        .carved_spans()
        .flat_map(|(x, y, bottom, top)| (bottom..top).map(move |height| (x, y, height)))
        .collect();
    let carved_by_piece: BTreeSet<_> = after.difference(&before).copied().collect();
    let expected: BTreeSet<_> = plan
        .carve_cells
        .iter()
        .map(|cell| (cell.x, cell.y, cell.height))
        .collect();

    assert_eq!(carved_by_piece, expected);
}

#[test]
fn hand_carved_tunnel_cells_are_not_claimed_or_refilled() {
    let (mut world, original_plan) = tunnel_fixture();
    let carve_cells: Vec<_> = original_plan
        .carve_cells
        .iter()
        .map(|cell| (cell.x, cell.y, cell.height))
        .collect();
    world.carve_cells(&carve_cells).unwrap();
    let construction = Construction::new(
        TrackState {
            x: 50,
            y: 50,
            z: 11,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    let plan = construction
        .tunnel_plan(PieceId::LevelToGentleDown, &world, &[])
        .expect("hand-carved cavern remains usable");
    assert!(plan.carve_cells.is_empty());
    let mut park = empty_park(world);
    let hand_carved_hash = park.world.state_hash();

    park.carve_for_construction(&[(0, plan)]).unwrap();
    park.restore_tunnel_piece(0);

    assert_eq!(park.world.state_hash(), hand_carved_hash);
}

#[test]
fn manual_carve_inside_an_automatic_tunnel_survives_piece_deletion() {
    let (world, plan) = tunnel_fixture();
    let mut park = empty_park(world);
    let original_hash = park.world.state_hash();
    park.carve_for_construction(&[(0, plan.clone())]).unwrap();
    let min_x = plan.carve_cells.iter().map(|cell| cell.x).min().unwrap();
    let max_x = plan.carve_cells.iter().map(|cell| cell.x).max().unwrap();
    let min_y = plan.carve_cells.iter().map(|cell| cell.y).min().unwrap();
    let max_y = plan.carve_cells.iter().map(|cell| cell.y).max().unwrap();
    let bottom = plan
        .carve_cells
        .iter()
        .map(|cell| cell.height)
        .min()
        .unwrap();
    let top = plan
        .carve_cells
        .iter()
        .map(|cell| cell.height)
        .max()
        .unwrap()
        + 1;
    let area = crate::world::Brush {
        x: min_x,
        y: min_y,
        size: u8::try_from(max_x.abs_diff(min_x).max(max_y.abs_diff(min_y)) + 1).unwrap(),
    };
    park.apply_terrain(TerrainCommand::Carve { area, bottom, top }, None)
        .unwrap();
    let manual_carve_hash = park.world.state_hash();

    park.restore_tunnel_piece(0);

    assert_eq!(park.world.state_hash(), manual_carve_hash);
    park.apply_terrain(TerrainCommand::Fill { area, bottom, top }, None)
        .unwrap();
    assert_eq!(park.world.state_hash(), original_hash);

    let construction = Construction::new(
        TrackState {
            x: 50,
            y: 50,
            z: 11,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    let next_plan = construction
        .tunnel_plan(PieceId::LevelToGentleDown, &park.world, &[])
        .unwrap();
    park.carve_for_construction(&[(1, next_plan)]).unwrap();
    park.restore_tunnel_piece(1);
    assert_eq!(park.world.state_hash(), original_hash);
}

#[test]
fn identical_tunnel_commands_produce_identical_park_hashes() {
    let (world, plan) = tunnel_fixture();
    let mut first = empty_park(world.clone());
    let mut second = empty_park(world);
    for park in [&mut first, &mut second] {
        park.carve_for_construction(&[(0, plan.clone())]).unwrap();
        park.carve_for_construction(&[(1, plan.clone())]).unwrap();
        park.restore_tunnel_piece(0);
    }

    assert_eq!(first.world.state_hash(), second.world.state_hash());
    assert_eq!(first.state_hash(), second.state_hash());
}

#[test]
fn replacing_construction_transfers_shared_tunnel_ownership_before_restore() {
    let (world, plan) = tunnel_fixture();
    let mut park = empty_park(world);
    let original_hash = park.world.state_hash();
    park.carve_for_construction(&[(0, plan.clone())]).unwrap();
    let carved_hash = park.world.state_hash();

    let next_generation = park.next_construction_tunnel_generation();
    park.carve_for_construction_at(next_generation, &[(0, plan)])
        .unwrap();
    park.activate_construction_tunnel_generation(next_generation);
    assert_eq!(park.world.state_hash(), carved_hash);

    park.restore_tunnel_piece(0);
    assert_eq!(park.world.state_hash(), original_hash);
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
