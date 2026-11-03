pub mod physics;

pub mod track;

pub mod world;

#[cfg(feature = "threads")]
use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    pub x: f32,
    pub y: f32,
    velocity_x: f32,
    velocity_y: f32,
}

pub trait Executor<T> {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync;
}

pub struct SequentialExecutor {
    partition_size: usize,
}

impl SequentialExecutor {
    pub fn new(partition_size: usize) -> Self {
        assert!(partition_size > 0, "partition size must be positive");
        Self { partition_size }
    }
}

impl<T> Executor<T> for SequentialExecutor {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync,
    {
        let mut output = Vec::with_capacity(agent_count);
        for start in (0..agent_count).step_by(self.partition_size) {
            let end = (start + self.partition_size).min(agent_count);
            let partition: Vec<_> = (start..end).map(&process).collect();
            output.extend(partition);
        }
        output
    }
}

#[cfg(feature = "threads")]
pub struct ThreadedExecutor {
    pool: rayon::ThreadPool,
    partition_size: usize,
}

#[cfg(feature = "threads")]
impl ThreadedExecutor {
    pub fn new(thread_count: usize, partition_size: usize) -> Self {
        assert!(thread_count > 0, "thread count must be positive");
        assert!(partition_size > 0, "partition size must be positive");
        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .expect("failed to create simulation worker pool"),
            partition_size,
        }
    }
}

#[cfg(feature = "threads")]
impl<T: Send> Executor<T> for ThreadedExecutor {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync,
    {
        self.pool.install(|| {
            (0..agent_count)
                .into_par_iter()
                .with_min_len(self.partition_size)
                .map(&process)
                .collect()
        })
    }
}

#[derive(Clone, Copy)]
enum Phase {
    AvoidNeighbours,
    Move,
}

const TICK_PHASES: [Phase; 2] = [Phase::AvoidNeighbours, Phase::Move];

pub struct Simulation {
    tick_count: u64,
    seed: u64,
    random_state: u64,
    agents: Vec<Agent>,
}

pub mod guests;

impl Simulation {
    pub fn new(seed: u64, agent_count: usize) -> Self {
        let mut simulation = Self {
            tick_count: 0,
            seed,
            random_state: seed,
            agents: Vec::with_capacity(agent_count),
        };
        for _ in 0..agent_count {
            let x = simulation.next_unit() * 100.0;
            let y = simulation.next_unit() * 100.0;
            let velocity_x = (simulation.next_unit() - 0.5) * 0.8;
            let velocity_y = (simulation.next_unit() - 0.5) * 0.8;
            simulation.agents.push(Agent {
                x,
                y,
                velocity_x,
                velocity_y,
            });
        }
        simulation
    }

    pub fn tick(&mut self) {
        self.tick_with(&SequentialExecutor::new(256));
    }

    pub fn tick_with<E: Executor<Agent>>(&mut self, executor: &E) {
        for phase in TICK_PHASES {
            let agents = &self.agents;
            self.agents = executor.execute(agents.len(), |index| {
                let mut agent = agents[index];
                match phase {
                    Phase::AvoidNeighbours => {
                        // The placeholder examines nearby indices; a spatial index can supply these later.
                        for offset in 1..=4 {
                            let neighbour = agents[(index + offset) % agents.len()];
                            let dx = agent.x - neighbour.x;
                            let dy = agent.y - neighbour.y;
                            let distance_squared = dx * dx + dy * dy;
                            if distance_squared > 0.0 && distance_squared < 4.0 {
                                agent.velocity_x += dx * 0.002;
                                agent.velocity_y += dy * 0.002;
                            }
                        }
                        let random = random_unit(self.seed, self.tick_count, index);
                        agent.velocity_x =
                            (agent.velocity_x + (random - 0.5) * 0.01).clamp(-0.4, 0.4);
                        agent.velocity_y =
                            (agent.velocity_y + (0.5 - random) * 0.01).clamp(-0.4, 0.4);
                    }
                    Phase::Move => {
                        agent.x = (agent.x + agent.velocity_x).rem_euclid(100.0);
                        agent.y = (agent.y + agent.velocity_y).rem_euclid(100.0);
                    }
                }
                agent
            });
        }
        self.tick_count += 1;
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in self
            .seed
            .to_le_bytes()
            .into_iter()
            .chain(self.tick_count.to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        for agent in &self.agents {
            for value in [agent.x, agent.y, agent.velocity_x, agent.velocity_y] {
                for byte in value.to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                }
            }
        }
        hash
    }

    fn next_unit(&mut self) -> f32 {
        self.random_state = self.random_state.wrapping_add(0x9e3779b97f4a7c15);
        (mix_random(self.random_state) >> 40) as f32 / (1_u32 << 24) as f32
    }
}

fn random_unit(seed: u64, tick: u64, index: usize) -> f32 {
    let value = seed
        .wrapping_add(tick.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add((index as u64).wrapping_mul(0xd1b54a32d192ed03));
    (mix_random(value) >> 40) as f32 / (1_u32 << 24) as f32
}

fn mix_random(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    value
}

#[cfg(test)]
mod tests {
    use super::{Agent, Simulation};

    #[test]
    fn neighbours_and_per_tick_seed_affect_agent_movement() {
        let mut nearby = Simulation::new(42, 2);
        nearby.agents = vec![
            Agent {
                x: 10.0,
                y: 10.0,
                velocity_x: 0.0,
                velocity_y: 0.0,
            },
            Agent {
                x: 11.0,
                y: 10.0,
                velocity_x: 0.0,
                velocity_y: 0.0,
            },
        ];
        let mut distant = Simulation::new(42, 2);
        distant.agents = nearby.agents.clone();
        distant.agents[1].x = 50.0;
        let mut different_seed = Simulation::new(43, 2);
        different_seed.agents = nearby.agents.clone();

        nearby.tick();
        distant.tick();
        different_seed.tick();

        assert_ne!(nearby.agents[0], distant.agents[0]);
        assert_ne!(nearby.agents[0], different_seed.agents[0]);
    }

    #[test]
    fn zero_seed_initializes_distinct_agents() {
        let simulation = Simulation::new(0, 2);
        assert_ne!(simulation.agents[0], simulation.agents[1]);
    }

    #[test]
    fn seeded_simulations_replay_identically() {
        let mut first = Simulation::new(42, 8);
        let mut second = Simulation::new(42, 8);
        for _ in 0..120 {
            first.tick();
            second.tick();
        }
        assert_eq!(first.tick_count(), 120);
        assert_eq!(first.state_hash(), second.state_hash());
    }

    #[test]
    fn agents_move_and_different_seeds_diverge() {
        let mut first = Simulation::new(42, 8);
        let mut second = Simulation::new(43, 8);
        let initial_hash = first.state_hash();
        first.tick();
        second.tick();
        assert_ne!(first.state_hash(), initial_hash);
        assert_ne!(first.state_hash(), second.state_hash());
        assert_eq!(first.agents().len(), 8);
    }
}

#[cfg(all(test, feature = "threads"))]
mod threaded_tests {
    use super::{Agent, Executor, SequentialExecutor, Simulation, ThreadedExecutor};
    use std::sync::Mutex;

    #[test]
    fn executor_reuses_worker_between_calls() {
        let executor = ThreadedExecutor::new(1, 1);
        let worker_ids = Mutex::new(Vec::new());
        for _ in 0..2 {
            executor.execute(1, |_| {
                worker_ids.lock().unwrap().push(std::thread::current().id());
                Agent {
                    x: 0.0,
                    y: 0.0,
                    velocity_x: 0.0,
                    velocity_y: 0.0,
                }
            });
        }
        let worker_ids = worker_ids.into_inner().unwrap();
        assert_eq!(worker_ids[0], worker_ids[1]);
    }
    #[test]
    fn thread_and_partition_counts_preserve_state() {
        let mut sequential = Simulation::new(42, 10_000);
        let mut one = Simulation::new(42, 10_000);
        let mut two = Simulation::new(42, 10_000);
        let mut four = Simulation::new(42, 10_000);
        let mut different_seed = Simulation::new(43, 10_000);
        let sequential_executor = SequentialExecutor::new(173);
        let one_executor = ThreadedExecutor::new(1, 257);
        let two_executor = ThreadedExecutor::new(2, 333);
        let four_executor = ThreadedExecutor::new(4, 512);
        for _ in 0..120 {
            sequential.tick_with(&sequential_executor);
            one.tick_with(&one_executor);
            two.tick_with(&two_executor);
            four.tick_with(&four_executor);
            different_seed.tick_with(&four_executor);
        }
        assert_eq!(sequential.state_hash(), one.state_hash());
        assert_eq!(one.state_hash(), two.state_hash());
        assert_eq!(one.state_hash(), four.state_hash());
        assert_ne!(one.state_hash(), different_seed.state_hash());
    }
}
