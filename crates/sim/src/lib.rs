#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    pub x: f32,
    pub y: f32,
    velocity_x: f32,
    velocity_y: f32,
}

pub struct Simulation {
    tick_count: u64,
    random_state: u64,
    agents: Vec<Agent>,
}

impl Simulation {
    pub fn new(seed: u64, agent_count: usize) -> Self {
        let mut simulation = Self {
            tick_count: 0,
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
        self.tick_count += 1;
        for agent in &mut self.agents {
            agent.x = (agent.x + agent.velocity_x).rem_euclid(100.0);
            agent.y = (agent.y + agent.velocity_y).rem_euclid(100.0);
        }
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in self.tick_count.to_le_bytes() {
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
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 7;
        self.random_state ^= self.random_state << 17;
        (self.random_state >> 40) as f32 / (1_u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::Simulation;

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
