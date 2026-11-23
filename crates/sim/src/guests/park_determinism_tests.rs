use super::coaster_boarding_tests::{construction_for, open_station_paths};
use super::*;
use crate::executor::{Executor, SequentialExecutor, ThreadedExecutor};
use crate::facilities::{ParkFacilities, coaster_facility};
use crate::guests::GuestId;
use crate::guests::park::ParkGuests;
use crate::park::tick::COASTER_PLACE_ID;
use crate::park::{CoasterId, Park};
use crate::physics::{BlockOperation, PhysicsConfig};
use crate::track::blocks::BlockLayout;
use crate::track::hybrid_model;
use crate::world::Surface;

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
    train_states: Vec<(u64, u64)>,
    events: Vec<CoasterEvent>,
    boarded_guest_count: usize,
    completed_train_count: usize,
    completed_guest_count: usize,
    maximum_occupied_trains: usize,
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
    let construction = construction_for(&world, hybrid_model());
    let circuit = construction.circuit().unwrap();
    let coaster_model = construction.model().clone();
    let train_configuration = coaster_model.train.clone();
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

    let queue_entry = open_station_paths(&mut world, &construction);

    let facility = coaster_facility(&world, &construction, COASTER_PLACE_ID, 0, None).unwrap();
    let roster_size = 64;
    let mut simulation = ParkGuests::new(42, roster_size, &world, queue_entry, roster_size);
    simulation.open_with_guests(&[facility]);
    for guest_index in 0..roster_size {
        simulation
            .walking
            .place_on_path(GuestId(guest_index as u32), queue_entry);
    }

    let mut park = Park::new_empty(world, ParkFacilities::new(), simulation);
    let coaster_id = CoasterId(0);
    park.replace_coaster(coaster_id, construction);
    let coaster = park.coaster_mut(coaster_id).expect("coaster was inserted");
    coaster.coaster_open = true;
    coaster.coaster_operation = Some(BlockOperation::new(layout.clone(), &coaster_model));
    coaster.coaster_facility = Some(facility);
    coaster.block_layout = Some(layout);
    coaster.desired_train_count = Some(2);

    let mut events = Vec::new();
    let mut boarded_guest_count = 0;
    let mut completed_train_count = 0;
    let mut completed_guest_count = 0;
    let mut maximum_occupied_trains = 0;
    for tick in 0..5000 {
        let riders_before = park
            .coaster(coaster_id)
            .expect("coaster remains present")
            .coaster_riders
            .clone();
        let rides_completed_before: Vec<_> = park
            .simulation
            .behaviors()
            .guests
            .iter()
            .map(|guest| guest.rides_completed)
            .collect();
        park.tick_with(executor);

        for (&train_id, riders) in &park
            .coaster(coaster_id)
            .expect("coaster remains present")
            .coaster_riders
        {
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
            if park
                .coaster(coaster_id)
                .expect("coaster remains present")
                .coaster_riders
                .contains_key(&train_id)
            {
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
        maximum_occupied_trains = maximum_occupied_trains.max(
            park.coaster(coaster_id)
                .expect("coaster remains present")
                .coaster_riders
                .len(),
        );
    }

    let train_states = park
        .coaster(coaster_id)
        .expect("coaster remains present")
        .coaster_operation
        .as_ref()
        .unwrap()
        .trains()
        .map(|(id, train)| (id, train.state_hash()))
        .collect();
    Replay {
        park_state_hash: park.state_hash(),
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
