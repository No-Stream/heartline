use super::Park;
use crate::facilities::ParkFacilities;
use crate::guests::park::ParkGuests;
use crate::physics::{PhysicsConfig, Train, demo_circuit};
use crate::track::construction::Construction;
use crate::track::{TrackState, hybrid_model};
use crate::world::{Path, Position, Surface, World};

fn empty_park() -> Park {
    let mut world = World::new(
        3,
        3,
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
    world.place_path(Path {
        position: Position {
            x: 1,
            y: 0,
            height: 0,
        },
        slope: None,
    });
    world.place_path(Path {
        position: Position {
            x: 2,
            y: 0,
            height: 0,
        },
        slope: None,
    });
    let train_configuration = hybrid_model().train;
    let circuit = demo_circuit();
    let train = Train::new(
        &circuit,
        &train_configuration,
        PhysicsConfig::default(),
        0.0,
        0.0,
    );
    let simulation = ParkGuests::new(42, 1, &world, entrance, 1);
    Park::new(
        world,
        ParkFacilities::new(),
        simulation,
        circuit,
        train,
        train_configuration,
    )
}

#[test]
fn state_hash_is_reproducible_and_covers_live_park_state() {
    let first = empty_park();
    let mut second = empty_park();
    assert_eq!(first.state_hash(), second.state_hash());
    second.physics_remainder = 1;
    assert_ne!(first.state_hash(), second.state_hash());

    let baseline_hash = first.state_hash();
    let mut world_changed = empty_park();
    world_changed.world.set_surface(
        0,
        0,
        Surface {
            corners: [1; 4],
            water: None,
        },
    );
    assert_ne!(baseline_hash, world_changed.state_hash());

    let mut facilities_changed = empty_park();
    facilities_changed
        .facilities
        .place(
            &facilities_changed.world,
            crate::facilities::FacilityKind::Food,
            Position {
                x: 1,
                y: 1,
                height: 0,
            },
            crate::world::Direction::North,
        )
        .unwrap();
    assert_ne!(baseline_hash, facilities_changed.state_hash());

    let mut behavior_changed = empty_park();
    behavior_changed
        .simulation
        .tick(&behavior_changed.world, &[], 100, None);
    assert_ne!(baseline_hash, behavior_changed.state_hash());

    let mut train_changed = empty_park();
    train_changed.train.step(&train_changed.circuit);
    assert_ne!(baseline_hash, train_changed.state_hash());

    let mut construction_changed = empty_park();
    construction_changed.construction = Some(Construction::new(
        TrackState {
            x: 4,
            y: 4,
            z: 0,
            ..TrackState::origin()
        },
        hybrid_model(),
    ));
    assert_ne!(baseline_hash, construction_changed.state_hash());
}
