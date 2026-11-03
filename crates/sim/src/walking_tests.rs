use crate::SequentialExecutor;
use crate::walking::WalkingSimulation;
use crate::world::{Direction, Path, Position, Surface, World};

fn world() -> World {
    let mut world = World::new(
        8,
        2,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.set_surface(
        2,
        0,
        Surface {
            corners: [0, 1, 1, 0],
            water: None,
        },
    );
    for x in 3..8 {
        world.set_surface(
            x,
            0,
            Surface {
                corners: [1; 4],
                water: None,
            },
        );
    }
    for x in 0..8 {
        world.place_path(Path {
            position: Position {
                x,
                y: 0,
                height: if x <= 2 { 0 } else { 1 },
            },
            slope: if x == 2 { Some(Direction::East) } else { None },
        });
    }
    world
}

#[test]
fn guests_walk_on_connected_sloped_paths_and_replay() {
    let world = world();
    let mut first = WalkingSimulation::new(42, 100, &world);
    let mut second = WalkingSimulation::new(42, 100, &world);
    let initial = first.guests().to_vec();
    for _ in 0..300 {
        first.tick(&world);
        second.tick_with(&world, &SequentialExecutor::new(7));
    }
    assert_eq!(first.state_hash(), second.state_hash());
    assert!(
        first
            .guests()
            .iter()
            .zip(initial)
            .any(|(guest, before)| guest.distance > before.distance)
    );
    for guest in first.guests() {
        assert!(guest.path.is_some_and(|path| world.path(path).is_some()));
        assert!((0.0..=0.25).contains(&guest.height));
        assert!((0.0..1.0).contains(&guest.y));
    }
}

#[test]
fn deletion_and_disconnected_paths_do_not_strand_guests_forever() {
    let mut world = world();
    let mut simulation = WalkingSimulation::new(1, 50, &world);
    for path in world.paths().collect::<Vec<_>>() {
        world.remove_path(path.position);
    }
    simulation.tick(&world);
    assert!(simulation.guests().iter().all(|guest| !guest.moving));
    world.place_path(Path {
        position: Position {
            x: 0,
            y: 0,
            height: 0,
        },
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
    for _ in 0..200 {
        simulation.tick(&world);
    }
    assert!(simulation.guests().iter().any(|guest| guest.distance > 0.0));
}

#[test]
fn group_members_start_together_and_remain_close() {
    let world = world();
    let mut simulation = WalkingSimulation::new(7, 200, &world);
    for _ in 0..200 {
        simulation.tick(&world);
    }
    for group in &simulation.traits().groups {
        let leader = simulation.guests()[group.members[0].0 as usize];
        for member in &group.members {
            let guest = simulation.guests()[member.0 as usize];
            assert!((guest.x - leader.x).abs() < 1.1);
            assert!((guest.y - leader.y).abs() < 1.0);
        }
    }
}

#[test]
fn walking_is_continuous_and_groups_pause() {
    let world = world();
    let mut simulation = WalkingSimulation::new(7, 200, &world);
    let mut observed_pause = false;
    for _ in 0..600 {
        let before = simulation.guests().to_vec();
        simulation.tick(&world);
        for (guest, previous) in simulation.guests().iter().zip(before) {
            let delta = ((guest.x - previous.x).powi(2) + (guest.y - previous.y).powi(2)).sqrt();
            assert!(delta < 0.08, "guest jumped {delta}");
            assert!((guest.distance - previous.distance - delta).abs() < 0.00001);
            observed_pause |= guest.distance > 0.0 && !guest.moving;
        }
    }
    assert!(observed_pause);
}

#[test]
fn removing_a_path_recovers_at_the_nearest_remaining_tile() {
    let mut world = world();
    let mut simulation = WalkingSimulation::new(5, 80, &world);
    let removed = Position {
        x: 4,
        y: 0,
        height: 1,
    };
    let affected: Vec<_> = simulation
        .guests()
        .iter()
        .enumerate()
        .filter(|(_, guest)| guest.path == Some(removed))
        .map(|(index, _)| index)
        .collect();
    assert!(!affected.is_empty());
    world.remove_path(removed);
    simulation.tick(&world);
    for index in affected {
        let guest = simulation.guests()[index];
        assert!(
            guest
                .path
                .is_some_and(|position| position.x == 3 || position.x == 5)
        );
    }
}

#[test]
fn demo_flow_field_connects_surface_to_underground_loop() {
    let world = crate::demo_world::demo_world();
    let mut graph = crate::pathfinding::Graph::with_entrance(&world, None);
    let start = graph.indices[&Position {
        x: 218,
        y: 256,
        height: 8,
    }];
    let goal = graph.indices[&Position {
        x: 228,
        y: 254,
        height: 3,
    }];
    graph.ensure_field(goal);
    let mut node = start;
    let mut visited = 0;
    while node != goal {
        node = graph.fields[&goal][&node];
        visited += 1;
        assert!(visited <= graph.paths.len());
    }
    assert!(visited > 10);
}

#[test]
fn crowded_guests_separate_while_remaining_on_paths() {
    let world = world();
    let mut simulation = WalkingSimulation::new(7, 500, &world);
    for _ in 0..30 {
        simulation.tick(&world);
    }
    let positions: std::collections::BTreeSet<_> = simulation
        .guests()
        .iter()
        .map(|guest| (guest.x.to_bits(), guest.y.to_bits()))
        .collect();
    assert!(positions.len() > 40);
    assert!(
        simulation
            .guests()
            .iter()
            .all(|guest| (0.0..1.0).contains(&guest.y))
    );
}

#[cfg(feature = "threads")]
#[test]
fn walking_matches_across_thread_counts() {
    let world = world();
    let mut sequential = WalkingSimulation::new(42, 1000, &world);
    let mut parallel = [1, 2, 4].map(|_| WalkingSimulation::new(42, 1000, &world));
    let executors = [1, 2, 4].map(|threads| crate::ThreadedExecutor::new(threads, 64));
    for _ in 0..200 {
        sequential.tick(&world);
        for (simulation, executor) in parallel.iter_mut().zip(&executors) {
            simulation.tick_with(&world, executor);
        }
    }
    for simulation in parallel {
        assert_eq!(sequential.state_hash(), simulation.state_hash());
    }
}

#[test]
fn state_hash_distinguishes_seeds_even_without_guests() {
    let world = world();
    assert_ne!(
        WalkingSimulation::new(1, 0, &world).state_hash(),
        WalkingSimulation::new(2, 0, &world).state_hash()
    );
}

#[test]
fn unrelated_world_edits_preserve_movement_progress() {
    let mut world = world();
    let mut simulation = WalkingSimulation::new(7, 100, &world);
    for _ in 0..30 {
        simulation.tick(&world);
    }
    let before = simulation.guests().to_vec();
    world.set_surface(
        0,
        1,
        Surface {
            corners: [1; 4],
            water: None,
        },
    );
    simulation.tick(&world);
    for (guest, previous) in simulation.guests().iter().zip(before) {
        let delta = ((guest.x - previous.x).powi(2) + (guest.y - previous.y).powi(2)).sqrt();
        assert!(delta < 0.08, "unrelated edit moved guest {delta}");
    }
}
