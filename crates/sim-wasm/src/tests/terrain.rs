use super::{WasmSimulation, facility_intersects_track, facility_intersects_track_sets};
use theme_park_sim::facilities::FacilityKind;
use theme_park_sim::facilities::ParkFacilities;
use theme_park_sim::track::{catalog, hybrid_model};
use theme_park_sim::world::{Direction, Ground, Path, Position, Surface, World};

#[test]
fn facility_clearance_uses_separate_demo_and_construction_trains() {
    let demo_train = hybrid_model().train;
    let mut construction_train = hybrid_model().train;
    construction_train.width_metres = 100.0;
    let construction_piece = (*catalog()
        .iter()
        .find(|piece| piece.id == theme_park_sim::track::PieceId::Straight)
        .expect("catalog contains straight track"))
    .clone();
    let demo_pieces = [construction_piece.clone()];
    let construction_pieces = [construction_piece];

    let position = (0..30)
        .flat_map(|x| (0..30).map(move |y| (x, y)))
        .map(|(x, y)| Position { x, y, height: 0 })
        .find(|position| {
            !facility_intersects_track(
                FacilityKind::Food,
                *position,
                Direction::North,
                &construction_pieces,
                &demo_train,
            ) && facility_intersects_track(
                FacilityKind::Food,
                *position,
                Direction::North,
                &construction_pieces,
                &construction_train,
            )
        })
        .expect("wide train has additional facility conflicts");

    assert!(facility_intersects_track_sets(
        FacilityKind::Food,
        position,
        Direction::North,
        &[],
        &demo_train,
        &construction_pieces,
        &construction_train,
    ));
    assert!(!facility_intersects_track_sets(
        FacilityKind::Food,
        position,
        Direction::North,
        &demo_pieces,
        &demo_train,
        &[],
        &construction_train,
    ));
}

#[test]
fn terrain_edits_refuse_buildings_and_track_without_mutating_the_world() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let before_building_edit = simulation.world_state_hash();
    assert!(!simulation.terrain_apply("raise_tiles", vec![218, 246, 1, 0, 1, 0, 0, 0]));
    assert_eq!(simulation.world_state_hash(), before_building_edit);
    assert!(simulation.terrain_message().contains("building or ride"));

    let station = simulation
        .demo_coaster()
        .expect("demo park")
        .circuit
        .pieces()[0]
        .sample(0.0)
        .position;
    let track_x = (station.x / 4.0).floor() as u16;
    let track_y = (station.y / 4.0).floor() as u16;
    let before_track_edit = simulation.world_state_hash();
    assert!(!simulation.terrain_apply(
        "flatten",
        vec![i32::from(track_x), i32::from(track_y), 1, 0, 0, 100, 0, 0,]
    ));
    assert_eq!(simulation.world_state_hash(), before_track_edit);
    assert!(simulation.terrain_message().contains("coaster track"));
}

#[test]
fn terrain_wire_rejects_missing_parameters_without_mutating_the_world() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let before = simulation.world_state_hash();
    assert!(!simulation.terrain_apply("flatten", vec![1, 2, 3]));
    assert_eq!(simulation.world_state_hash(), before);
    assert!(
        simulation
            .terrain_message()
            .contains("eight integer parameters")
    );
}

#[test]
fn tunnel_fill_reports_invalidated_paths_and_guests_recover() {
    let mut simulation = WasmSimulation::new(42, 12, String::from("demo"));
    let tunnel_path = Position {
        x: 221,
        y: 250,
        height: 5,
    };
    assert!(simulation.world.path(tunnel_path).is_some());
    assert!(simulation.terrain_apply("fill", vec![221, 250, 1, 0, 0, 0, 5, 7]));
    assert_eq!(simulation.terrain_invalidated_paths(), vec![221, 250, 5]);
    assert!(simulation.terrain_message().contains("updated"));
    assert!(simulation.world.path(tunnel_path).is_none());

    simulation.tick();
    assert!(
        simulation
            .simulation
            .walking()
            .guests()
            .iter()
            .all(|guest| guest.path.is_some())
    );
}

#[test]
fn facility_bridge_accepts_a_shop_inside_a_carved_cavern() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let mut world = World::new(
        6,
        6,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    for (x, y) in [(2, 1), (2, 2), (3, 2), (2, 3), (3, 3)] {
        world.carve(x, y, 1, 4);
    }
    world.place_path(Path {
        position: Position {
            x: 2,
            y: 1,
            height: 1,
        },
        slope: None,
    });
    simulation.world = world;
    simulation.facilities = ParkFacilities::new();

    assert!(simulation.place_facility_at("food_stall", 2, 2, 0, 1));
    assert!(
        simulation
            .facilities_json()
            .contains("\"x\":2,\"y\":2,\"z\":1")
    );
}

#[test]
fn coaster_station_can_be_constructed_inside_a_carved_cavern() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    let mut world = World::new(
        16,
        16,
        Surface {
            corners: [12; 4],
            water: None,
        },
    );
    for y in 0..16 {
        for x in 0..16 {
            world.carve(x, y, 3, 12);
        }
    }
    simulation.world = world;

    simulation.construction_start(16, 16, 4, 0);
    assert!(
        simulation.construction_active(),
        "{}",
        simulation.construction_message()
    );
    assert_eq!(simulation.world.ground(4, 4, 8), Ground::Carved);
    assert_eq!(simulation.construction_station_positions()[2], 8);
}

#[test]
fn terrain_edit_reroutes_constructed_track_supports() {
    let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
    simulation.construction_start(200, 200, 20, 0);
    simulation.construction_select("Straight");
    simulation.construction_append();
    let supports_before = simulation.construction_placed_supports();
    assert!(
        !supports_before.is_empty(),
        "{}",
        simulation.construction_message()
    );
    let first_support = supports_before.chunks_exact(7).next().unwrap();
    let x = first_support[4].floor() as u16;
    let y = first_support[5].floor() as u16;

    assert!(simulation.terrain_apply(
        "raise_tiles",
        vec![i32::from(x), i32::from(y), 1, 0, 1, 0, 0, 0]
    ));
    let supports_after = simulation.construction_placed_supports();
    assert!(
        !supports_after.is_empty(),
        "{}",
        simulation.construction_message()
    );
    assert_ne!(supports_after, supports_before);
}
