use std::hint::black_box;
use std::time::Instant;

use theme_park_sim::ThreadedExecutor;
use theme_park_sim::demo_world::demo_world;
use theme_park_sim::guest_behavior::{Facility, KnownPlace, PlaceKind};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::walking::WalkingSimulation;
use theme_park_sim::world::Position;

const GUEST_COUNT: usize = 10_000;
const WARMUP_TICKS: usize = 20;
const MEASURED_TICKS: usize = 200;
const SEED: u64 = 42;

fn main() {
    let world = demo_world();
    println!(
        "Guests: {GUEST_COUNT}; warmup ticks: {WARMUP_TICKS}; measured ticks: {MEASURED_TICKS}; seed: {SEED}"
    );
    println!("Native walking simulation, including avoidance:");
    let mut expected_hash = None;
    for thread_count in [1, 2, 4, 8] {
        let executor = ThreadedExecutor::new(thread_count, 256);
        let mut simulation = WalkingSimulation::with_entrance(
            SEED,
            GUEST_COUNT,
            &world,
            Position {
                x: 218,
                y: 256,
                height: 8,
            },
        );
        for _ in 0..WARMUP_TICKS {
            simulation.tick_with(&world, &executor);
        }
        let start = Instant::now();
        for _ in 0..MEASURED_TICKS {
            simulation.tick_with(&world, &executor);
        }
        let elapsed = start.elapsed();
        let hash = black_box(simulation.state_hash());
        if let Some(expected_hash) = expected_hash {
            assert_eq!(
                hash, expected_hash,
                "guest state differs across thread counts"
            );
        } else {
            expected_hash = Some(hash);
        }
        println!(
            "{thread_count} threads: {:.2} ticks/s, {:.3} ms/tick, state hash {hash:016x}",
            MEASURED_TICKS as f64 / elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1000.0 / MEASURED_TICKS as f64
        );
    }
    println!("Native complete guest simulation, including decisions and avoidance:");
    let entrance = Position {
        x: 218,
        y: 256,
        height: 8,
    };
    let facilities: Vec<_> = [
        PlaceKind::Ride,
        PlaceKind::Food,
        PlaceKind::Drink,
        PlaceKind::Restroom,
        PlaceKind::Bench,
    ]
    .into_iter()
    .enumerate()
    .map(|(id, kind)| Facility {
        place: KnownPlace {
            id: id as u32,
            position: Position {
                x: 219 + id as u16,
                y: 256,
                height: 8,
            },
            kind,
            minimum_height_cm: 0,
            intensity: 20,
            ride_type: 0,
            theme: 0,
            price: 0,
        },
        capacity: 24,
        duration_ticks: 900,
    })
    .collect();
    let mut expected_state = None;
    for thread_count in [1, 2, 4, 8] {
        let executor = ThreadedExecutor::new(thread_count, 256);
        let mut simulation = ParkGuests::new(SEED, GUEST_COUNT, &world, entrance, GUEST_COUNT);
        for _ in 0..WARMUP_TICKS {
            simulation.tick_with(&world, &facilities, 100, None, &executor);
        }
        assert_eq!(
            simulation.active_count(),
            GUEST_COUNT,
            "benchmark must simulate 10,000 active guests"
        );
        let initial_distance: f32 = simulation
            .walking()
            .guests()
            .iter()
            .map(|guest| guest.distance)
            .sum();
        let start = Instant::now();
        for _ in 0..MEASURED_TICKS {
            simulation.tick_with(&world, &facilities, 100, None, &executor);
        }
        let elapsed = start.elapsed();
        let total_distance: f32 = simulation
            .walking()
            .guests()
            .iter()
            .map(|guest| guest.distance)
            .sum();
        assert!(
            total_distance > initial_distance,
            "guests must walk during the measured period"
        );
        let hash = black_box(simulation.walking().state_hash());
        let state = (
            hash,
            simulation.walking().traits().clone(),
            simulation.behaviors().guests.clone(),
        );
        if let Some(expected) = &expected_state {
            assert_eq!(
                &state, expected,
                "complete guest simulation differs across thread counts"
            );
        } else {
            expected_state = Some(state);
        }
        println!(
            "{thread_count} threads: {:.2} ticks/s, {:.3} ms/tick, walking state hash {hash:016x}",
            MEASURED_TICKS as f64 / elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1000.0 / MEASURED_TICKS as f64
        );
    }
}
