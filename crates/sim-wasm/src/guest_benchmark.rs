use theme_park_sim::demo_world::demo_world;
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::physics::{Circuit, PhysicsConfig, Train, demo_circuit};
use theme_park_sim::track::hybrid_model;
use theme_park_sim::walking::WalkingSimulation;
use theme_park_sim::world::{Position, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmWalkingBenchmark {
    simulation: WalkingSimulation,
    world: World,
    circuit: Circuit,
    train: Train,
    physics_remainder: u32,
}

#[wasm_bindgen]
impl WasmWalkingBenchmark {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, guest_count: usize) -> Self {
        let world = demo_world();
        let simulation = WalkingSimulation::with_entrance(
            seed,
            guest_count,
            &world,
            Position {
                x: 218,
                y: 256,
                height: 8,
            },
        );
        let circuit = demo_circuit();
        let train = Train::new(
            &circuit,
            &hybrid_model().train,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            0.0,
            2.0,
        );
        Self {
            simulation,
            world,
            circuit,
            train,
            physics_remainder: 0,
        }
    }

    pub fn tick(&mut self) {
        self.simulation.tick(&self.world);
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            self.train.step(&self.circuit);
            self.physics_remainder -= 30;
        }
        self.train.take_readings();
    }

    pub fn state_hash(&self) -> u64 {
        self.simulation.state_hash()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.simulation
            .guests()
            .iter()
            .flat_map(|guest| {
                [
                    guest.x,
                    guest.y,
                    guest.height,
                    guest.heading_x,
                    guest.heading_y,
                    guest.distance,
                    if guest.moving { 1.0 } else { 0.0 },
                ]
            })
            .collect()
    }

    pub fn guest_ids(&self) -> Vec<u32> {
        self.simulation.traits().ids().map(|id| id.0).collect()
    }

    pub fn guest_appearances(&self) -> Vec<u8> {
        let traits = self.simulation.traits();
        traits
            .ids()
            .flat_map(|id: GuestId| {
                let appearance = traits.appearance(id);
                let age = match traits.age(id) {
                    AgeGroup::Child => 0,
                    AgeGroup::Adult => 1,
                    AgeGroup::Senior => 2,
                };
                [
                    appearance.skin_tone,
                    appearance.hair_style,
                    appearance.clothing_style,
                    appearance.body_shape,
                    age,
                ]
            })
            .collect()
    }
}

#[cfg(test)]
impl WasmWalkingBenchmark {
    pub fn snapshot_bytes(&self) -> usize {
        self.snapshot().len() * size_of::<f32>()
            + self.guest_ids().len() * size_of::<u32>()
            + self.guest_appearances().len()
    }
}

#[cfg(test)]
mod guest_benchmark_tests {
    use super::WasmWalkingBenchmark;

    #[test]
    fn direct_walking_matches_original_browser_workload() {
        let mut simulation = WasmWalkingBenchmark::new(42, 10_000);
        for _ in 0..220 {
            simulation.tick();
        }
        assert_eq!(simulation.state_hash(), 0xb9b7928e7c985fdb);
        assert_eq!(simulation.snapshot_bytes(), 370_000);
    }
}
