use super::{Surface, WasmSimulation};
use theme_park_sim::world::{Position, World};

#[test]
fn tunnel_path_commands_refresh_the_guest_graph() {
    let mut simulation = WasmSimulation::new(42, 12);
    let entrance = Position {
        x: 218,
        y: 250,
        height: 8,
    };
    let tunnel = Position {
        x: 224,
        y: 250,
        height: 3,
    };
    assert_eq!(
        simulation.simulation.walking().path_component(entrance),
        simulation.simulation.walking().path_component(tunnel)
    );

    assert!(simulation.path_remove(221, 250, 5));
    simulation.tick();
    assert_ne!(
        simulation.simulation.walking().path_component(entrance),
        simulation.simulation.walking().path_component(tunnel)
    );
    let before_preview = simulation.world_state_hash();
    let coordinates = vec![218, 250, 8, 224, 250, 3];
    let preview = simulation.path_preview(coordinates.clone(), "normal", 0);
    assert_eq!(preview.len(), 7 * 9);
    assert!(preview.chunks_exact(9).all(|tile| tile[6] == 0));
    assert!(
        preview
            .chunks_exact(9)
            .any(|tile| tile[..4] == [221, 250, 5, 4])
    );
    assert_eq!(simulation.world_state_hash(), before_preview);

    assert!(simulation.path_build(coordinates, "normal", 0));
    assert!(
        simulation
            .world_paths()
            .chunks_exact(4)
            .any(|path| path == [221, 250, 5, 4])
    );
    simulation.tick();
    assert_eq!(
        simulation.simulation.walking().path_component(entrance),
        simulation.simulation.walking().path_component(tunnel)
    );
}

#[test]
fn path_preview_build_snapshot_and_remove_share_the_path_wire_contract() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();

    let before_preview = simulation.world_state_hash();
    let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
    assert_eq!(
        preview,
        vec![
            0, 0, 0, 0, 1, 2, 0, 2,
            0, // x, y, height, slope, kind, style, refusal, edges, openings
            1, 0, 0, 0, 1, 2, 0, 10, 0, 2, 0, 0, 0, 1, 2, 0, 8, 0,
        ]
    );
    assert_eq!(simulation.world_state_hash(), before_preview);
    assert!(simulation.path_message().is_empty());

    assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
    assert_eq!(
        simulation.world_paths(),
        vec![0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]
    );
    assert_eq!(
        simulation.world_path_metadata(),
        vec![1, 2, 2, 1, 2, 10, 1, 2, 8]
    );

    assert!(simulation.path_remove(1, 0, 0));
    assert_eq!(simulation.world_paths(), vec![0, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(simulation.world_path_metadata(), vec![1, 2, 0, 1, 2, 0]);
    let revision = simulation.world.revision();
    assert!(!simulation.path_remove(1, 0, 0));
    assert_eq!(simulation.world.revision(), revision);
}

#[test]
fn path_preview_reports_explicit_edges_for_adjacent_queue_paths_and_connections() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 0));
    assert!(simulation.path_build(vec![2, 1, 0, 2, 1, 0], "queue", 0));
    assert_eq!(
        simulation.world_path_metadata(),
        vec![1, 0, 2, 1, 0, 10, 1, 0, 8, 1, 0, 0]
    );

    let preview = simulation.path_preview(vec![2, 1, 0, 2, 0, 0], "queue", 0);
    assert_eq!(
        preview,
        vec![2, 1, 0, 0, 1, 0, 0, 1, 0, 2, 0, 0, 0, 1, 0, 0, 12, 0]
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![2, 1, 0, 2, 0, 0], "queue", 0));
    assert_eq!(
        simulation.world_path_metadata(),
        vec![1, 0, 2, 1, 0, 10, 1, 0, 12, 1, 0, 1]
    );
}

#[test]
fn facility_openings_follow_ride_placement_and_access_paths() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        8,
        8,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![4, 3, 0, 4, 3, 0], "queue", 0));
    assert!(simulation.path_build(vec![5, 3, 0, 5, 3, 0], "normal", 0));
    let id = simulation
        .facilities
        .place(
            &simulation.world,
            super::FacilityKind::Carousel,
            Position {
                x: 4,
                y: 4,
                height: 0,
            },
            super::Direction::North,
        )
        .unwrap();
    assert_eq!(
        simulation.world_facility_openings(),
        vec![4, 3, 0, 4, 5, 3, 0, 4]
    );
    assert!(simulation.path_remove(4, 3, 0));
    assert_eq!(
        simulation.world_facility_openings(),
        vec![4, 3, 0, 4, 5, 3, 0, 4]
    );
    let preview = simulation.path_preview(vec![4, 3, 0, 4, 3, 0], "queue", 0);
    assert_eq!(preview[8], 4);
    assert!(simulation.path_build(vec![3, 3, 0, 4, 3, 0], "queue", 0));
    let before = simulation.world_state_hash();
    let invalid = simulation.path_preview(vec![4, 2, 0, 4, 3, 0], "queue", 0);
    assert!(
        invalid
            .chunks_exact(9)
            .all(|tile| tile[6] == 9 && tile[8] == 0)
    );
    assert!(!simulation.path_build(vec![4, 2, 0, 4, 3, 0], "queue", 0));
    assert_eq!(simulation.world_state_hash(), before);
    assert!(simulation.remove_facility(id));
    assert!(simulation.world_facility_openings().is_empty());
}

#[test]
fn path_preview_reports_existing_tile_metadata_that_build_preserves() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

    let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "normal", 3);
    assert_eq!(
        preview,
        vec![
            0, 0, 0, 0, 0, 1, 0, 2, 0, 1, 0, 0, 0, 0, 3, 0, 10, 0, 2, 0, 0, 0, 0, 3, 0, 8, 0
        ]
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "normal", 3));
    assert_eq!(
        simulation.world_path_metadata(),
        vec![0, 1, 2, 0, 3, 10, 0, 3, 8]
    );
}

#[test]
fn queue_preview_keeps_a_reused_normal_path_endpoint_normal() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

    let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
    assert_eq!(
        preview,
        vec![
            0, 0, 0, 0, 0, 1, 0, 2, 0, 1, 0, 0, 0, 1, 2, 0, 10, 0, 2, 0, 0, 0, 1, 2, 0, 8, 0
        ]
    );
    simulation.facilities = super::ParkFacilities::new();
    assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
    assert_eq!(
        simulation.world_path_metadata(),
        vec![0, 1, 2, 1, 2, 10, 1, 2, 8]
    );
}

#[test]
fn invalid_path_preview_returns_a_message_without_mutating_the_world() {
    let mut simulation = WasmSimulation::new(42, 0);
    simulation.world = World::new(
        4,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    simulation.facilities = super::ParkFacilities::new();

    let before_preview = simulation.world_state_hash();
    let preview = simulation.path_preview(vec![0, 0, 0, 3, 0, 1], "normal", 0);
    assert_eq!(preview.len(), 4 * 9);
    assert!(preview.chunks_exact(9).any(|tile| tile[6] != 0));
    assert!(!simulation.path_message().is_empty());
    assert_eq!(simulation.world_state_hash(), before_preview);
    assert!(!simulation.path_build(vec![0, 0, 0, 3, 0, 1], "unknown", 0));
    assert_eq!(simulation.world_state_hash(), before_preview);
    assert!(
        simulation
            .path_preview(vec![0, 0, 0], "normal", 0)
            .is_empty()
    );
    assert!(simulation.path_message().contains("six coordinates"));
    assert_eq!(simulation.world_state_hash(), before_preview);
}

#[test]
fn path_preview_and_build_reject_a_facility_footprint() {
    let mut simulation = WasmSimulation::new(42, 0);
    let building_position = simulation.facilities.buildings()[0].position;
    let coordinates = vec![
        i32::from(building_position.x),
        i32::from(building_position.y),
        i32::from(building_position.height),
        i32::from(building_position.x),
        i32::from(building_position.y),
        i32::from(building_position.height),
    ];
    let revision = simulation.world.revision();
    let preview = simulation.path_preview(coordinates.clone(), "normal", 0);
    assert_eq!(preview.len(), 9);
    assert_eq!(preview[6], 8);
    assert!(simulation.path_message().contains("facility"));
    assert!(!simulation.path_build(coordinates, "normal", 0));
    assert_eq!(simulation.world.revision(), revision);
}
