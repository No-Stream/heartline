use super::*;
use crate::guest_behavior::{KnownPlace, PlaceKind};
use crate::physics::{BlockOperation, Circuit, PhysicsConfig};
use crate::track::blocks::BlockLayout;
use crate::track::{PieceId, TrackState, catalog, hybrid_model};
use crate::world::{PathKind, Surface};
use crate::{SequentialExecutor, ThreadedExecutor};

fn test_circuit() -> Circuit {
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
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
        let piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).unwrap()
}

struct Replay {
    park: ParkGuests,
    train_states: Vec<(u64, u64)>,
    events: Vec<(usize, u64, usize, bool)>,
    maximum_occupied_trains: usize,
    completed: usize,
}

fn replay<E: Executor<WalkingGuest>>(
    world: &World,
    facility: Facility,
    layout: &BlockLayout,
    executor: &E,
) -> Replay {
    let entrance = Position {
        x: 0,
        y: 0,
        height: 0,
    };
    let mut park = ParkGuests::new(42, 64, world, entrance, 64);
    park.open_with_guests(&[facility]);
    park.set_external_rides(&[facility.place.id]);
    let mut operation = BlockOperation::new(layout.clone());
    let mut riders = BTreeMap::new();
    let mut events = Vec::new();
    let mut maximum_occupied_trains = 0;
    let mut completed = 0;
    let mut physics_remainder = 0;
    for tick in 0..5000 {
        park.tick_with(world, &[facility], 100, Some(64), executor);
        physics_remainder += 200;
        while physics_remainder >= 30 {
            if let Some(train_id) = operation.next_dispatch_id() {
                let boarded = park.board_coaster_train(&facility, train_id);
                if !boarded.is_empty() {
                    assert_eq!(operation.dispatch(), train_id);
                    events.push((tick, train_id, boarded.len(), false));
                    riders.insert(train_id, boarded);
                    maximum_occupied_trains = maximum_occupied_trains.max(riders.len());
                }
            }
            for (train_id, outcome) in operation.step(&[]) {
                let boarded: Vec<GuestId> = riders.remove(&train_id).unwrap();
                let seats_per_car = usize::from(layout.train().rows_per_car)
                    * usize::from(layout.train().seats_per_row);
                let ratings: Vec<_> = (0..boarded.len())
                    .map(|seat| {
                        let report = &outcome.reports[seat / seats_per_car];
                        (report.excitement, report.intensity, report.nausea)
                    })
                    .collect();
                let proximity_events: Vec<_> = (0..boarded.len())
                    .map(|seat| {
                        outcome.reports[seat / seats_per_car]
                            .proximity_events
                            .clone()
                    })
                    .collect();
                events.push((tick, train_id, boarded.len(), outcome.recovered));
                assert!(!outcome.recovered);
                park.finish_coaster_train(
                    world,
                    &facility,
                    train_id,
                    &ratings,
                    &proximity_events,
                    false,
                );
                completed += 1;
            }
            physics_remainder -= 30;
        }
    }
    let train_states = operation
        .trains()
        .map(|(id, train)| (id, train.state_hash()))
        .collect();
    Replay {
        park,
        train_states,
        events,
        maximum_occupied_trains,
        completed,
    }
}

#[test]
fn coaster_guests_and_trains_replay_identically_across_thread_counts() {
    let entrance = Position {
        x: 0,
        y: 0,
        height: 0,
    };
    let mut world = World::new(
        4,
        1,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world
        .build_path_route(entrance, Position { x: 1, ..entrance }, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(
            Position { x: 1, ..entrance },
            Position { x: 3, ..entrance },
            PathKind::Queue,
            0,
        )
        .unwrap();
    let configuration = hybrid_model().train;
    let facility = Facility {
        place: KnownPlace {
            id: 700,
            position: Position { x: 3, ..entrance },
            kind: PlaceKind::Ride,
            minimum_height_cm: 0,
            intensity: 50,
            ride_type: 2,
            theme: 0,
            price: 0,
        },
        capacity: usize::from(configuration.cars)
            * usize::from(configuration.rows_per_car)
            * usize::from(configuration.seats_per_row),
        duration_ticks: 1,
        exit_position: Some(entrance),
    };
    let circuit = test_circuit();
    let config = PhysicsConfig {
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
        &configuration,
        config,
        vec![boundary(2), boundary(6), boundary(10), boundary(13)],
    )
    .unwrap();
    assert!(layout.capacity() >= 2);
    let baseline = replay(&world, facility, &layout, &SequentialExecutor::new(7));
    assert!(baseline.maximum_occupied_trains >= 2);
    assert!(baseline.completed > 0);
    for count in [1, 2, 4] {
        let actual = replay(&world, facility, &layout, &ThreadedExecutor::new(count, 7));
        assert_eq!(baseline.events, actual.events, "thread count {count}");
        assert_eq!(
            baseline.train_states, actual.train_states,
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.walking().state_hash(),
            actual.park.walking().state_hash(),
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.walking().traits(),
            actual.park.walking().traits(),
            "thread count {count}"
        );
        assert_eq!(
            baseline.park.behaviors().guests,
            actual.park.behaviors().guests,
            "thread count {count}"
        );
    }
}
