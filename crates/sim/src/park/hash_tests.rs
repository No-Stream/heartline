use super::{CoasterId, Park};
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
    let simulation = ParkGuests::new(42, 1, &world, entrance, 1);
    Park::new_empty(world, ParkFacilities::new(), simulation)
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
    train_changed.replace_coaster(
        CoasterId(0),
        Construction::new(
            TrackState {
                x: 4,
                y: 4,
                z: 0,
                ..TrackState::origin()
            },
            hybrid_model(),
        ),
    );
    let baseline_with_coaster = train_changed.state_hash();
    let circuit = demo_circuit();
    let train = Train::new(
        &circuit,
        &hybrid_model().train,
        PhysicsConfig::default(),
        0.0,
        0.0,
    );
    train_changed
        .coaster_mut(CoasterId(0))
        .unwrap()
        .construction_train = Some((circuit, train));
    assert_ne!(baseline_with_coaster, train_changed.state_hash());

    let mut construction_changed = empty_park();
    construction_changed.replace_coaster(
        CoasterId(0),
        Construction::new(
            TrackState {
                x: 4,
                y: 4,
                z: 0,
                ..TrackState::origin()
            },
            hybrid_model(),
        ),
    );
    assert_ne!(baseline_hash, construction_changed.state_hash());

    let mut nausea_rating_changed = empty_park();
    nausea_rating_changed.replace_coaster(
        CoasterId(0),
        Construction::new(
            TrackState {
                x: 4,
                y: 4,
                z: 0,
                ..TrackState::origin()
            },
            hybrid_model(),
        ),
    );
    let nausea_rating_baseline = nausea_rating_changed.state_hash();
    nausea_rating_changed
        .coaster_mut(CoasterId(0))
        .unwrap()
        .coaster_ratings = Some((50, 31));
    assert_ne!(nausea_rating_baseline, nausea_rating_changed.state_hash());
}

#[test]
fn park_without_coasters_ticks_and_has_distinct_state() {
    let mut park = empty_park();
    let initial_hash = park.state_hash();
    park.tick();
    assert!(park.coasters.is_empty());
    assert_ne!(park.state_hash(), initial_hash);
}

#[test]
fn multiple_coasters_hash_in_stable_id_order_and_keep_price_state_separate() {
    let mut first = empty_park();
    let mut second = empty_park();
    for park in [&mut first, &mut second] {
        let first_id = park.add_coaster(Construction::new(
            TrackState {
                x: 4,
                y: 4,
                z: 0,
                ..TrackState::origin()
            },
            hybrid_model(),
        ));
        let second_id = park.add_coaster(Construction::new(
            TrackState {
                x: 8,
                y: 8,
                z: 0,
                ..TrackState::origin()
            },
            crate::track::wooden_twister_model(),
        ));
        assert_eq!((first_id, second_id), (CoasterId(0), CoasterId(1)));
    }
    assert_eq!(first.state_hash(), second.state_hash());

    let first_hash = first.state_hash();
    first.rename_coaster(CoasterId(1), String::from("Family Twister"));
    assert_ne!(first_hash, first.state_hash());

    let renamed_hash = first.state_hash();
    first.set_coaster_price(Some(CoasterId(0)), 11);
    assert_ne!(renamed_hash, first.state_hash());
    assert_eq!(first.coaster_price(Some(CoasterId(1))), 6);
}
