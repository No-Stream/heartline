use super::*;
use crate::executor::{Executor, SequentialExecutor, ThreadedExecutor};
use crate::facilities::{ParkFacilities, coaster_access_openings, coaster_facility};
use crate::guests::GuestId;
use crate::guests::park::ParkGuests;
use crate::park::Park;
use crate::park::tick::COASTER_PLACE_ID;
use crate::physics::{BlockOperation, PhysicsConfig, Train};
use crate::track::blocks::BlockLayout;
use crate::track::construction::Construction;
use crate::track::{PieceId, TrackState, hybrid_model};
use crate::world::{PathKind, Surface};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoasterEventKind {
    Boarded,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CoasterEvent {
    tick: usize,
    train_id: u64,
    guest_count: usize,
    kind: CoasterEventKind,
}

struct Replay {
    park: Park,
    park_state_hash: u64,
    demo_train_state: u64,
    train_states: Vec<(u64, u64)>,
    events: Vec<CoasterEvent>,
    boarded_guest_count: usize,
    completed_train_count: usize,
    completed_guest_count: usize,
    maximum_occupied_trains: usize,
}

fn construction_for(world: &World) -> Construction {
    let start = TrackState {
        x: 256,
        y: 256,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    for id in [
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::LiftFlat,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::LiftFlat,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ] {
        construction
            .append(id, world, &[])
            .unwrap_or_else(|error| panic!("could not append {id:?}: {error:?}"));
    }
    construction.close().unwrap();
    construction
}

fn replay<E: Executor<WalkingGuest>>(executor: &E) -> Replay {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let construction = construction_for(&world);
    let circuit = construction.circuit().unwrap();
    let train_configuration = construction.model().train.clone();
    let physics_config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let boundary = |end: usize| {
        circuit.pieces()[..end]
            .iter()
            .map(|piece| piece.length())
            .sum()
    };
    let layout = BlockLayout::new(
        &circuit,
        &train_configuration,
        physics_config,
        vec![boundary(2), boundary(6), boundary(10), boundary(13)],
    )
    .unwrap();
    assert!(layout.capacity() >= 2);

    let [queue, exit] = coaster_access_openings(&construction).unwrap();
    let queue_entry = [
        Position {
            x: queue.position.x.saturating_sub(1),
            ..queue.position
        },
        Position {
            x: queue.position.x.saturating_add(1),
            ..queue.position
        },
        Position {
            y: queue.position.y.saturating_sub(1),
            ..queue.position
        },
        Position {
            y: queue.position.y.saturating_add(1),
            ..queue.position
        },
    ]
    .into_iter()
    .find(|position| *position != exit.position)
    .unwrap();
    for position in [queue.position, exit.position, queue_entry] {
        assert!(
            world
                .set_surface(
                    position.x,
                    position.y,
                    Surface {
                        corners: [position.height; 4],
                        water: None,
                    },
                )
                .is_empty()
        );
    }
    world
        .build_path_route(queue_entry, queue_entry, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(exit.position, exit.position, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(queue_entry, queue.position, PathKind::Queue, 0)
        .unwrap();

    let facility = coaster_facility(&world, &construction, COASTER_PLACE_ID, 0).unwrap();
    let roster_size = 64;
    let mut simulation = ParkGuests::new(42, roster_size, &world, queue_entry, roster_size);
    simulation.open_with_guests(&[facility]);
    for guest_index in 0..roster_size {
        simulation
            .walking
            .place_on_path(GuestId(guest_index as u32), queue_entry);
    }

    let demo_train = Train::new(&circuit, &train_configuration, physics_config, 0.0, 0.0);
    let mut park = Park::new(
        world,
        ParkFacilities::new(),
        simulation,
        circuit,
        demo_train,
        train_configuration,
    );
    park.construction = Some(construction);
    park.coaster_open = true;
    park.coaster_operation = Some(BlockOperation::new(layout.clone()));
    park.coaster_facility = Some(facility);
    park.block_layout = Some(layout);
    park.desired_train_count = Some(2);

    let mut events = Vec::new();
    let mut boarded_guest_count = 0;
    let mut completed_train_count = 0;
    let mut completed_guest_count = 0;
    let mut maximum_occupied_trains = 0;
    for tick in 0..5000 {
        let riders_before = park.coaster_riders.clone();
        let rides_completed_before: Vec<_> = park
            .simulation
            .behaviors()
            .guests
            .iter()
            .map(|guest| guest.rides_completed)
            .collect();
        park.tick_with(executor);

        for (&train_id, riders) in &park.coaster_riders {
            if !riders_before.contains_key(&train_id) {
                events.push(CoasterEvent {
                    tick,
                    train_id,
                    guest_count: riders.len(),
                    kind: CoasterEventKind::Boarded,
                });
                boarded_guest_count += riders.len();
            }
        }
        for (&train_id, riders) in &riders_before {
            if park.coaster_riders.contains_key(&train_id) {
                continue;
            }
            let completed_riders = riders
                .iter()
                .filter(|guest| {
                    park.simulation.behaviors().guests[guest.0 as usize].rides_completed
                        > rides_completed_before[guest.0 as usize]
                })
                .count();
            assert_eq!(
                completed_riders,
                riders.len(),
                "train {train_id} must finish for every boarded guest"
            );
            events.push(CoasterEvent {
                tick,
                train_id,
                guest_count: completed_riders,
                kind: CoasterEventKind::Completed,
            });
            completed_train_count += 1;
            completed_guest_count += completed_riders;
        }
        maximum_occupied_trains = maximum_occupied_trains.max(park.coaster_riders.len());
    }

    let train_states = park
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(id, train)| (id, train.state_hash()))
        .collect();
    Replay {
        park_state_hash: park.state_hash(),
        demo_train_state: park.demo_coaster.as_ref().unwrap().train.state_hash(),
        park,
        train_states,
        events,
        boarded_guest_count,
        completed_train_count,
        completed_guest_count,
        maximum_occupied_trains,
    }
}

#[test]
fn coaster_guests_and_trains_replay_identically_across_thread_counts() {
    let baseline = replay(&SequentialExecutor::new(7));
    assert!(baseline.boarded_guest_count > 0);
    assert!(baseline.completed_train_count > 0);
    assert!(baseline.completed_guest_count > 0);
    assert!(baseline.maximum_occupied_trains >= 2);
    assert_eq!(
        baseline.completed_guest_count,
        baseline
            .park
            .simulation
            .behaviors()
            .guests
            .iter()
            .map(|guest| guest.rides_completed as usize)
            .sum::<usize>()
    );

    for count in [1, 2, 4] {
        let actual = replay(&ThreadedExecutor::new(count, 7));
        assert_eq!(baseline.events, actual.events, "thread count {count}");
        assert_eq!(
            baseline.boarded_guest_count, actual.boarded_guest_count,
            "thread count {count}"
        );
        assert_eq!(
            baseline.completed_train_count, actual.completed_train_count,
            "thread count {count}"
        );
        assert_eq!(
            baseline.completed_guest_count, actual.completed_guest_count,
            "thread count {count}"
        );
        assert_eq!(
            baseline.maximum_occupied_trains, actual.maximum_occupied_trains,
            "thread count {count}"
        );
        assert_eq!(
            baseline.demo_train_state, actual.demo_train_state,
            "thread count {count}"
        );
        assert_eq!(
            baseline.park_state_hash, actual.park_state_hash,
            "thread count {count}"
        );
        assert_eq!(
            baseline.train_states, actual.train_states,
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.simulation.walking().state_hash(),
            actual.park.simulation.walking().state_hash(),
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.simulation.walking().traits(),
            actual.park.simulation.walking().traits(),
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.simulation.behaviors().guests,
            actual.park.simulation.behaviors().guests,
            "thread count {count}"
        );
    }
}
