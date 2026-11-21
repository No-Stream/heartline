use super::GForces;
use crate::state_hash::StateHasher;
use crate::track::DesignForceTargets;

const CUTOFF_HZ: f64 = 5.0;
const BRIEF_ALLOWANCE_G: f64 = 0.5;
const BRIEF_DURATION_SECONDS: f64 = 0.2;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Biquad {
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn new(time_step: f64, quality: f64) -> Self {
        let k = (std::f64::consts::PI * CUTOFF_HZ * time_step).tan();
        let norm = 1.0 / (1.0 + k / quality + k * k);
        let b0 = k * k * norm;
        Self {
            b1: 2.0 * b0,
            b2: b0,
            a1: 2.0 * (k * k - 1.0) * norm,
            a2: (1.0 - k / quality + k * k) * norm,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn hash_into(&self, hash: &mut StateHasher) {
        for value in [
            self.b1, self.b2, self.a1, self.a2, self.x1, self.x2, self.y1, self.y2,
        ] {
            hash.write_f64(value);
        }
    }

    fn initialize(&mut self, value: f64) {
        self.x1 = value;
        self.x2 = value;
        self.y1 = value;
        self.y2 = value;
    }

    fn update(&mut self, input: f64) -> f64 {
        // Unity gain in difference form keeps a constant input exact at the force cap.
        let output = input + self.b1 * (self.x1 - input) + self.b2 * (self.x2 - input)
            - self.a1 * (self.y1 - input)
            - self.a2 * (self.y2 - input);
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// A causal four-pole 5 Hz Butterworth filter for one rider's three force axes.
/// The initial sample initializes the filter at equilibrium, avoiding a startup spike.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ForceFilter {
    sections: [[Biquad; 2]; 3],
    initialized: bool,
}

impl ForceFilter {
    pub fn new(time_step: f64) -> Self {
        assert!(time_step.is_finite() && time_step > 0.0 && time_step * CUTOFF_HZ < 0.5);
        let sections = [
            Biquad::new(time_step, 0.541196100146197),
            Biquad::new(time_step, 1.306562964876377),
        ];
        Self {
            sections: [sections; 3],
            initialized: false,
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_bool(self.initialized);
        for axis in &self.sections {
            for section in axis {
                section.hash_into(hash);
            }
        }
    }

    pub fn update(&mut self, force: GForces) -> GForces {
        let values = [force.vertical, force.lateral, force.longitudinal];
        assert!(values.iter().all(|value| value.is_finite()));
        let mut output = [0.0; 3];
        for (axis, &value) in values.iter().enumerate() {
            if !self.initialized {
                for section in &mut self.sections[axis] {
                    section.initialize(value);
                }
            }
            output[axis] = self.sections[axis]
                .iter_mut()
                .fold(value, |input, section| section.update(input));
        }
        self.initialized = true;
        GForces {
            vertical: output[0],
            lateral: output[1],
            longitudinal: output[2],
        }
    }
}

/// The result for one car at one physics step. Axis arrays are vertical, lateral, longitudinal.
#[derive(Clone, Copy, Debug)]
pub struct EvaluatedForce {
    pub filtered: GForces,
    pub within_limits: bool,
    pub axis_within_limits: [bool; 3],
    pub axis_exceeding_sustained: [bool; 3],
}

/// Applies provisional game limits to filtered force, independently for each axis.
/// An excursion can exceed its sustained limit by at most 0.5 g for at most 0.2 s.
/// These game parameters are not a statement of ASTM limits or ride safety.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ForceEvaluator {
    filter: ForceFilter,
    limits: DesignForceTargets,
    time_step: f64,
    durations: [f64; 3],
    excursion_side: [i8; 3],
}

impl ForceEvaluator {
    pub fn new(limits: DesignForceTargets, time_step: f64) -> Self {
        assert!(
            limits.minimum_vertical_g.is_finite()
                && limits.maximum_vertical_g.is_finite()
                && limits.minimum_vertical_g < limits.maximum_vertical_g
                && limits.maximum_lateral_g.is_finite()
                && limits.maximum_lateral_g > 0.0
                && limits.maximum_longitudinal_g.is_finite()
                && limits.maximum_longitudinal_g > 0.0
        );
        Self {
            filter: ForceFilter::new(time_step),
            limits,
            time_step,
            durations: [0.0; 3],
            excursion_side: [0; 3],
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        self.filter.hash_into(hash);
        hash.write_f64(self.limits.minimum_vertical_g);
        hash.write_f64(self.limits.maximum_vertical_g);
        hash.write_f64(self.limits.maximum_lateral_g);
        hash.write_f64(self.limits.maximum_longitudinal_g);
        hash.write_f64(self.time_step);
        for value in self.durations {
            hash.write_f64(value);
        }
        for value in self.excursion_side {
            hash.write_i8(value);
        }
    }

    pub fn update(&mut self, force: GForces) -> EvaluatedForce {
        let filtered = self.filter.update(force);
        let values = [filtered.vertical, filtered.lateral, filtered.longitudinal];
        let minima = [
            self.limits.minimum_vertical_g,
            -self.limits.maximum_lateral_g,
            -self.limits.maximum_longitudinal_g,
        ];
        let maxima = [
            self.limits.maximum_vertical_g,
            self.limits.maximum_lateral_g,
            self.limits.maximum_longitudinal_g,
        ];
        let mut exceeding = [false; 3];
        let mut valid = [true; 3];
        for axis in 0..3 {
            let side = if values[axis] < minima[axis] {
                -1
            } else if values[axis] > maxima[axis] {
                1
            } else {
                0
            };
            exceeding[axis] = side != 0;
            self.durations[axis] = if side == 0 {
                0.0
            } else if side == self.excursion_side[axis] {
                self.durations[axis] + self.time_step
            } else {
                self.time_step
            };
            self.excursion_side[axis] = side;
            valid[axis] = values[axis] >= minima[axis] - BRIEF_ALLOWANCE_G
                && values[axis] <= maxima[axis] + BRIEF_ALLOWANCE_G
                && self.durations[axis] <= BRIEF_DURATION_SECONDS + 1e-12;
        }
        EvaluatedForce {
            filtered,
            within_limits: valid.iter().all(|valid| *valid),
            axis_within_limits: valid,
            axis_exceeding_sustained: exceeding,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::TIME_STEP;
    use crate::track::wooden_twister_model;

    fn force(vertical: f64) -> GForces {
        GForces {
            vertical,
            lateral: 0.0,
            longitudinal: 0.0,
        }
    }

    #[test]
    fn constant_force_has_no_startup_transient() {
        let mut filter = ForceFilter::new(TIME_STEP);
        for _ in 0..200 {
            assert!((filter.update(force(1.0)).vertical - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn filter_has_butterworth_five_hertz_response() {
        let mut filter = ForceFilter::new(TIME_STEP);
        let mut squared_input = 0.0;
        let mut squared_output = 0.0;
        for i in 0..4000 {
            let value = (2.0 * std::f64::consts::PI * 5.0 * i as f64 * TIME_STEP).sin();
            let filtered = filter.update(force(value)).vertical;
            if i >= 1000 {
                squared_input += value * value;
                squared_output += filtered * filtered;
            }
        }
        let gain = (squared_output / squared_input).sqrt();
        assert!(
            (gain - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.001,
            "gain {gain}"
        );
    }

    #[test]
    fn brief_four_and_a_half_g_peak_is_allowed_but_sustained_is_not() {
        let mut evaluator =
            ForceEvaluator::new(wooden_twister_model().design_force_targets, TIME_STEP);
        for _ in 0..200 {
            assert!(evaluator.update(force(1.0)).within_limits);
        }
        for _ in 0..20 {
            assert!(evaluator.update(force(4.5)).within_limits);
        }
        for _ in 0..200 {
            assert!(evaluator.update(force(1.0)).within_limits);
        }
        let mut failed = false;
        for _ in 0..200 {
            failed |= !evaluator.update(force(4.5)).within_limits;
        }
        assert!(failed);
    }

    #[test]
    fn caps_and_duration_apply_on_each_axis() {
        let limits = wooden_twister_model().design_force_targets;
        for axis in 0..3 {
            let reading = match axis {
                0 => force(4.4),
                1 => GForces {
                    vertical: 1.0,
                    lateral: 1.9,
                    longitudinal: 0.0,
                },
                _ => GForces {
                    vertical: 1.0,
                    lateral: 0.0,
                    longitudinal: -1.9,
                },
            };
            let mut evaluator = ForceEvaluator::new(limits, TIME_STEP);
            for _ in 0..40 {
                assert!(evaluator.update(reading).axis_within_limits[axis]);
            }
            assert!(!evaluator.update(reading).axis_within_limits[axis]);
        }
        let mut at_cap = ForceEvaluator::new(limits, TIME_STEP);
        for _ in 0..40 {
            assert!(at_cap.update(force(4.5)).within_limits);
        }
        assert!(!at_cap.update(force(4.5)).within_limits);
        let mut evaluator = ForceEvaluator::new(limits, TIME_STEP);
        assert!(!evaluator.update(force(4.6)).within_limits);
    }

    #[test]
    fn serialized_filter_continues_the_same_trace() {
        let mut original = ForceFilter::new(TIME_STEP);
        for _ in 0..30 {
            original.update(force(2.5));
        }
        let saved = postcard::to_allocvec(&original).unwrap();
        let mut restored: ForceFilter = postcard::from_bytes(&saved).unwrap();
        for i in 0..100 {
            let sample = force((i as f64 * 0.1).sin());
            assert_eq!(original.update(sample), restored.update(sample));
        }
    }
}
