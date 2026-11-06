"""Expose the preview step and add an evaluator in a disposable source tree."""
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
module = root / "crates/sim/src/physics/mod.rs"
source = module.read_text()
source = source.replace("mod comfort;", "pub mod force_evaluation;\nmod comfort;")
source = source.replace("    fn step_preview(", "    pub fn step_preview(")
source = source.replace(
    "pub use preview::{PreviewStatus, TrackPreview, preview_open_track};",
    "pub use preview::{PreviewStatus, TrackPreview, preview_open_track, preview_at_step};",
)
module.write_text(source)
preview = root / "crates/sim/src/physics/preview.rs"
source = preview.read_text()
source = source.replace("pub(super) fn preview_at_step(", "pub fn preview_at_step(")
source = source.replace(
    "    time_step: f64,\n) -> Option<TrackPreview>",
    "    time_step: f64,\n    limits: Option<crate::track::ComfortableForceLimits>,\n) -> Option<TrackPreview>",
)
source = source.replace("        PREVIEW_TIME_STEP,\n    )", "        PREVIEW_TIME_STEP,\n        None,\n    )")
source = source.replace(
    "    let mut stationary_steps = 0;",
    "    let mut evaluators = limits.map(|limits| (0..configuration.cars).map(|_| super::force_evaluation::ForceEvaluator::new(limits, 0.005)).collect::<Vec<_>>());\n    let mut force_flags = vec![false; length.ceil() as usize + 1];\n    let mut stationary_steps = 0;",
)
source = source.replace(
    "        train.step_preview(&circuit, time_step);",
    "        let old_distance = train.distance();\n        let old_speed = train.speed();\n        train.step_preview(&circuit, time_step);",
)
source = source.replace(
    "        recorder.record(&train.take_readings());",
    """        let mut readings = train.take_readings();
        if let Some(evaluators) = evaluators.as_mut() {
            let acceleration = (train.speed() - old_speed) / time_step;
            for sample_index in 1..=10 {
                let fraction = sample_index as f64 / 10.0;
                let distance = old_distance + fraction * (train.distance() - old_distance);
                let speed = old_speed + fraction * (train.speed() - old_speed);
                for (car, evaluator) in evaluators.iter_mut().enumerate() {
                    let force = train.measurement_force(&circuit, distance, speed, acceleration, car);
                    let evaluated = evaluator.update(force);
                    let car_distance = (distance - car as f64 * configuration.length_metres / f64::from(configuration.cars)).rem_euclid(length);
                    force_flags[car_distance.floor() as usize] |= !evaluated.within_limits;
                    if car == 0 {
                        let track_sample = circuit.sample(distance);
                        let reading = &mut readings[0][0];
                        reading.distance = if circuit.open { distance.clamp(0.0, length) } else { distance.rem_euclid(length) };
                        reading.speed = speed.abs();
                        reading.g = evaluated.filtered;
                        reading.airtime = evaluated.filtered.vertical < 0.0;
                        reading.height_m = track_sample.position.z;
                        reading.inverted = track_sample.up.z < 0.0;
                        reading.roll = track_sample.roll;
                        reading.piece_index = circuit.piece_index(distance);
                    }
                }
                recorder.record(&readings);
            }
        } else {
            recorder.record(&readings);
        }
        std::hint::black_box(&force_flags);""",
)
preview.write_text(source)

# Expose the same heart-force calculation used by the simulation, without
# changing the simulation sources in the working tree.
module = root / "crates/sim/src/physics/mod.rs"
source = module.read_text()
source += '''
impl Train {
    pub fn measurement_force(&self, circuit: &Circuit, distance: f64, speed: f64, acceleration: f64, car: usize) -> GForces {
        assert!(car < self.cars);
        let (index, local) = circuit.locate(distance - car as f64 * self.spacing);
        rider_g(&circuit.pieces[index], local, speed, acceleration)
    }
}
'''
module.write_text(source)

# Benchmark the complete recorder and the fine evaluator through the same
# player-circuit harness as the baseline, using each model's actual limits.
benchmark = root / "crates/sim/examples/player_circuit_preview_bench.rs"
source = benchmark.read_text()
source = source.replace("PhysicsConfig, preview_open_track", "PhysicsConfig, preview_at_step")
source = source.replace(
    "preview_open_track(pieces, &model.train, config)",
    "preview_at_step(pieces, &model.train, config, 20_000, 0.05, Some(model.comfortable_force_limits))",
)
benchmark.write_text(source)
