use theme_park_sim::Simulation;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmSimulation {
    simulation: Simulation,
}

#[wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, agent_count: usize) -> Self {
        Self {
            simulation: Simulation::new(seed, agent_count),
        }
    }

    pub fn tick(&mut self) {
        self.simulation.tick();
    }

    pub fn tick_count(&self) -> u64 {
        self.simulation.tick_count()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.simulation
            .agents()
            .iter()
            .flat_map(|agent| [agent.x, agent.y])
            .collect()
    }
}
