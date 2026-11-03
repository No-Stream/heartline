use super::WasmSimulation;

#[test]
fn scenery_snapshot_place_and_remove_use_stable_ids() {
    let mut simulation = WasmSimulation::new(1, 0);
    assert_ne!(simulation.scenery_json(), "[]");
    assert!(simulation.place_scenery("bench", 210, 245, 1, 2));
    simulation.tick();
    let snapshot = simulation.scenery_json();
    assert!(
        snapshot.contains(
            "\"kind\":\"bench\",\"x\":210,\"y\":245,\"z\":8,\"rotation\":1,\"quarter\":2"
        )
    );
    let id = "210:245:8:2";
    assert!(simulation.remove_scenery(id));
    assert!(!simulation.remove_scenery(id));
    assert!(!simulation.scenery_json().contains("\"x\":210,\"y\":245"));
}

#[test]
fn scenery_and_facilities_reject_overlap_in_both_directions() {
    let mut simulation = WasmSimulation::new(1, 0);
    assert!(!simulation.place_scenery("tree_oval", 220, 254, 0, 0));
    assert!(!simulation.place_scenery("tree_oval", 218, 256, 0, 0));
    assert!(!simulation.place_scenery("tree_oval", 210, 245, 0, 4));
    assert!(simulation.place_scenery("tree_oval", 210, 245, 0, 0));
    assert!(!simulation.place_facility("food_stall", 210, 245, 0));
    assert!(!simulation.place_scenery("unknown", 210, 244, 0, 0));
    assert!(!simulation.scenery_message().is_empty());
}

#[test]
fn scenery_identity_includes_height() {
    use theme_park_sim::world::{Position, Scenery, SceneryKind};

    let lower = Scenery {
        kind: SceneryKind::Lamp,
        position: Position {
            x: 1,
            y: 2,
            height: 3,
        },
        quarter: 0,
        rotation: 0,
    };
    let upper = Scenery {
        position: Position {
            height: 10,
            ..lower.position
        },
        ..lower
    };
    assert_ne!(super::scenery_id(&lower), super::scenery_id(&upper));
}

#[test]
fn demo_scenery_does_not_intersect_track_or_facilities() {
    let simulation = WasmSimulation::new(1, 0);
    let track = super::track_support_obstacles(
        simulation.circuit.pieces(),
        &simulation.train_configuration,
    );
    for scenery in simulation.world.scenery() {
        let bounds = scenery.clearance_box();
        for obstacle in track.iter().chain(simulation.facility_obstacles().iter()) {
            assert!(
                !super::boxes_overlap(
                    bounds,
                    super::ClearanceBox {
                        min: obstacle.min,
                        max: obstacle.max
                    }
                ),
                "demo scenery intersects a ride or facility: {scenery:?}"
            );
        }
    }
}
