#[cfg(test)]
mod tests {
    use super::combined_axis_excess;

    #[test]
    fn steady_normal_forces_have_no_excess() {
        assert_eq!(combined_axis_excess(&vec![[1.0, 0.0, 0.0]; 400]), 0.0);
    }

    #[test]
    fn sustained_combination_exceeds_ellipse() {
        assert!(combined_axis_excess(&vec![[5.0, 2.5, 0.0]; 400]) > 0.0);
    }

    #[test]
    fn short_pulse_does_not_count_as_sustained_excess() {
        let mut history = vec![[1.0, 0.0, 0.0]; 400];
        for sample in &mut history[100..110] {
            *sample = [1.0, 5.0, 0.0];
        }
        assert_eq!(combined_axis_excess(&history), 0.0);
    }

    #[test]
    fn separate_axes_at_different_times_do_not_combine() {
        let mut history = vec![[1.0, 0.0, 0.0]; 800];
        for sample in &mut history[100..300] {
            *sample = [5.0, 0.0, 0.0];
        }
        for sample in &mut history[500..700] {
            *sample = [1.0, 2.5, 0.0];
        }
        assert_eq!(combined_axis_excess(&history), 0.0);
    }
}

// F2291-06a, sections 7.1.1.2 and 7.1.5.1 and figures 6–10.
// The base-case longitudinal curve assumes a headrest and backrest.
// This partial game estimate is not an evaluation of the full standard.
// https://ia800607.us.archive.org/34/items/gov.law.astm.f2291.2006/astm.f2291.2006.pdf
const SAMPLE_RATE_HZ: f64 = 200.0;
const MINIMUM_SUSTAINED_INTERVALS: usize = 40;
type DurationCurve = (usize, f64, &'static [(f64, f64)]);

struct LowPass {
    stages: [Biquad; 2],
}

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    state1: f64,
    state2: f64,
}

impl Biquad {
    fn new(quality: f64, initial: f64) -> Self {
        let k = (std::f64::consts::PI * 5.0 / SAMPLE_RATE_HZ).tan();
        let norm = 1.0 / (1.0 + k / quality + k * k);
        let b0 = k * k * norm;
        let b1 = 2.0 * b0;
        let b2 = b0;
        let a1 = 2.0 * (k * k - 1.0) * norm;
        let a2 = (1.0 - k / quality + k * k) * norm;
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            state1: initial * (1.0 - b0),
            state2: initial * (b2 - a2),
        }
    }

    fn apply(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.state1;
        self.state1 = self.b1 * input - self.a1 * output + self.state2;
        self.state2 = self.b2 * input - self.a2 * output;
        output
    }
}

impl LowPass {
    fn new(initial: f64) -> Self {
        Self {
            stages: [
                Biquad::new(0.541196100146197, initial),
                Biquad::new(1.306562964876377, initial),
            ],
        }
    }

    fn apply(&mut self, input: f64) -> f64 {
        self.stages
            .iter_mut()
            .fold(input, |value, stage| stage.apply(value))
    }
}

/// Streaming approximation of F2291-06a single-axis and combined-axis limits.
/// The input is a 200 Hz history of vertical, lateral and longitudinal g for one car.
/// Events under 200 ms, transitions, and restraint differences are not assessed.
/// Single-axis exposure starts accumulating after 200 ms. Force slices are
/// 0.1 g through 6 g and 1 g through 32 g, where axis scoring saturates.
/// Pair slices use normalized increments of 0.01, 0.1, then 1, up to 16.
/// The maximum across summed axis and pair exposures approximates the penalty.
/// The positive vertical plateau is extrapolated beyond the chart's 40 s marker.
pub(super) struct ComfortTracker {
    filters: Option<[LowPass; 3]>,
    durations: [[u32; 87]; 6],
    pair_durations: [[u32; 141]; 3],
    active_axes: [usize; 6],
    active_pairs: [usize; 3],
    pair_total: [f64; 3],
    axis_total: f64,
}

impl Default for ComfortTracker {
    fn default() -> Self {
        Self {
            filters: None,
            durations: [[0; 87]; 6],
            pair_durations: [[0; 141]; 3],
            active_axes: [0; 6],
            active_pairs: [0; 3],
            pair_total: [0.0; 3],
            axis_total: 0.0,
        }
    }
}

impl ComfortTracker {
    pub(super) fn record(&mut self, reading: [f64; 3]) {
        let filters = self
            .filters
            .get_or_insert_with(|| reading.map(LowPass::new));
        let filtered: [f64; 3] = std::array::from_fn(|index| filters[index].apply(reading[index]));
        let [vertical, lateral, longitudinal] = filtered;
        let normalised = [
            vertical / if vertical >= 0.0 { 6.6 } else { 2.2 },
            lateral / 3.3,
            longitudinal / if longitudinal >= 0.0 { 6.6 } else { 2.2 },
        ];
        for (pair, (left, right)) in [(0, 1), (0, 2), (1, 2)].into_iter().enumerate() {
            let magnitude = normalised[left].hypot(normalised[right]);
            let mut worst = 0.0_f64;
            let mut active = 0;
            for (index, duration) in self.pair_durations[pair].iter_mut().enumerate() {
                let level = if index < 100 {
                    1.01 + index as f64 / 100.0
                } else if index < 130 {
                    2.1 + (index - 100) as f64 / 10.0
                } else {
                    6.0 + (index - 130) as f64
                };
                if magnitude.partial_cmp(&level) != Some(std::cmp::Ordering::Greater)
                    && magnitude != level
                {
                    break;
                }
                active += 1;
                *duration = duration.saturating_add(1);
                if *duration > MINIMUM_SUSTAINED_INTERVALS as u32 {
                    worst = level - 1.0;
                }
            }
            if active < self.active_pairs[pair] {
                self.pair_durations[pair][active..self.active_pairs[pair]].fill(0);
            }
            self.active_pairs[pair] = active;
            self.pair_total[pair] += worst / SAMPLE_RATE_HZ;
        }

        let curves: &[DurationCurve] = &[
            (
                0,
                1.0,
                &[
                    (0.2, 6.0),
                    (1.0, 6.0),
                    (2.0, 4.0),
                    (4.0, 4.0),
                    (5.0, 3.0),
                    (11.8, 3.0),
                    (12.0, 2.0),
                ],
            ),
            (0, -1.0, &[(0.2, 2.0), (0.5, 1.5), (4.0, 1.5), (7.0, 1.1)]),
            (1, 1.0, &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)]),
            (1, -1.0, &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)]),
            (
                2,
                1.0,
                &[
                    (0.2, 6.0),
                    (1.0, 6.0),
                    (2.0, 4.0),
                    (4.0, 4.0),
                    (5.0, 3.0),
                    (11.8, 3.0),
                    (12.0, 2.5),
                ],
            ),
            (2, -1.0, &[(0.2, 2.0), (0.5, 1.5)]),
        ];

        let mut worst = 0.0_f64;
        for (curve_index, &(axis, sign, curve)) in curves.iter().enumerate() {
            let magnitude = sign * filtered[axis];
            let mut active = 0;
            for level_index in 1..=86 {
                let level = if level_index <= 60 {
                    level_index as f64 / 10.0
                } else {
                    (level_index - 54) as f64
                };
                if magnitude.partial_cmp(&level) != Some(std::cmp::Ordering::Greater)
                    && magnitude != level
                {
                    break;
                }
                active = level_index;
                let duration = &mut self.durations[curve_index][level_index];
                *duration = duration.saturating_add(1);
                if *duration > MINIMUM_SUSTAINED_INTERVALS as u32 {
                    let seconds = (*duration - 1) as f64 / SAMPLE_RATE_HZ;
                    worst = worst.max(level / curve_limit(curve, seconds) - 1.0);
                }
            }
            if active < self.active_axes[curve_index] {
                self.durations[curve_index][active + 1..=self.active_axes[curve_index]].fill(0);
            }
            self.active_axes[curve_index] = active;
        }
        self.axis_total += worst.max(0.0) / SAMPLE_RATE_HZ;
    }

    #[cfg(test)]
    fn record_reference(&mut self, reading: [f64; 3]) {
        let filters = self
            .filters
            .get_or_insert_with(|| reading.map(LowPass::new));
        let filtered: [f64; 3] = std::array::from_fn(|index| filters[index].apply(reading[index]));
        let [vertical, lateral, longitudinal] = filtered;
        let normalised = [
            vertical / if vertical >= 0.0 { 6.6 } else { 2.2 },
            lateral / 3.3,
            longitudinal / if longitudinal >= 0.0 { 6.6 } else { 2.2 },
        ];
        for (pair, (left, right)) in [(0, 1), (0, 2), (1, 2)].into_iter().enumerate() {
            let magnitude = normalised[left].hypot(normalised[right]);
            let mut worst = 0.0_f64;
            for (index, duration) in self.pair_durations[pair].iter_mut().enumerate() {
                let level = if index < 100 {
                    1.01 + index as f64 / 100.0
                } else if index < 130 {
                    2.1 + (index - 100) as f64 / 10.0
                } else {
                    6.0 + (index - 130) as f64
                };
                if magnitude >= level {
                    *duration = duration.saturating_add(1);
                    if *duration > MINIMUM_SUSTAINED_INTERVALS as u32 {
                        worst = level - 1.0;
                    }
                } else {
                    *duration = 0;
                }
            }
            self.pair_total[pair] += worst / SAMPLE_RATE_HZ;
        }

        let curves: &[DurationCurve] = &[
            (
                0,
                1.0,
                &[
                    (0.2, 6.0),
                    (1.0, 6.0),
                    (2.0, 4.0),
                    (4.0, 4.0),
                    (5.0, 3.0),
                    (11.8, 3.0),
                    (12.0, 2.0),
                ],
            ),
            (0, -1.0, &[(0.2, 2.0), (0.5, 1.5), (4.0, 1.5), (7.0, 1.1)]),
            (1, 1.0, &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)]),
            (1, -1.0, &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)]),
            (
                2,
                1.0,
                &[
                    (0.2, 6.0),
                    (1.0, 6.0),
                    (2.0, 4.0),
                    (4.0, 4.0),
                    (5.0, 3.0),
                    (11.8, 3.0),
                    (12.0, 2.5),
                ],
            ),
            (2, -1.0, &[(0.2, 2.0), (0.5, 1.5)]),
        ];

        let mut worst = 0.0_f64;
        for (curve_index, &(axis, sign, curve)) in curves.iter().enumerate() {
            let magnitude = sign * filtered[axis];
            for level_index in 1..=86 {
                let level = if level_index <= 60 {
                    level_index as f64 / 10.0
                } else {
                    (level_index - 54) as f64
                };
                let duration = &mut self.durations[curve_index][level_index];
                if magnitude >= level {
                    *duration = duration.saturating_add(1);
                    if *duration > MINIMUM_SUSTAINED_INTERVALS as u32 {
                        let seconds = (*duration - 1) as f64 / SAMPLE_RATE_HZ;
                        worst = worst.max(level / curve_limit(curve, seconds) - 1.0);
                    }
                } else {
                    *duration = 0;
                }
            }
        }
        self.axis_total += worst.max(0.0) / SAMPLE_RATE_HZ;
    }

    pub(super) fn finish(&self) -> f64 {
        self.pair_total.into_iter().fold(self.axis_total, f64::max)
    }

    pub(super) fn hash_into(&self, hash: &mut crate::state_hash::StateHasher) {
        hash.write_bool(self.filters.is_some());
        if let Some(filters) = &self.filters {
            for filter in filters {
                for stage in &filter.stages {
                    hash.write_f64(stage.state1);
                    hash.write_f64(stage.state2);
                }
            }
        }
        for axis in &self.durations {
            for duration in axis {
                hash.write_u32(*duration);
            }
        }
        for pair in &self.pair_durations {
            for duration in pair {
                hash.write_u32(*duration);
            }
        }
        for value in self.pair_total.into_iter().chain([self.axis_total]) {
            hash.write_f64(value);
        }
    }
}

#[cfg(test)]
fn combined_axis_excess(history: &[[f64; 3]]) -> f64 {
    let mut tracker = ComfortTracker::default();
    for &reading in history {
        tracker.record(reading);
    }
    tracker.finish()
}

fn curve_limit(curve: &[(f64, f64)], duration: f64) -> f64 {
    for endpoints in curve.windows(2) {
        let [(start_time, start_limit), (end_time, end_limit)] = [endpoints[0], endpoints[1]];
        if duration <= end_time {
            let fraction = (duration - start_time) / (end_time - start_time);
            return start_limit + fraction * (end_limit - start_limit);
        }
    }
    curve.last().unwrap().1
}

#[cfg(test)]
mod sustained_axis_tests {
    use super::combined_axis_excess;

    #[test]
    fn duration_changes_the_single_axis_limit() {
        let short = vec![[4.5, 0.0, 0.0]; 200];
        let long = vec![[4.5, 0.0, 0.0]; 800];
        assert_eq!(combined_axis_excess(&short), 0.0);
        assert!(combined_axis_excess(&long) > 0.0);
    }

    #[test]
    fn nested_single_axis_events_are_detected() {
        let mut history = vec![[3.2, 0.0, 0.0]; 2400];
        for sample in &mut history[600..1600] {
            sample[0] = 5.0;
        }
        assert!(combined_axis_excess(&history) > 0.0);
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::ComfortTracker;

    #[test]
    fn brief_high_force_within_sustained_pair_exceedance_does_not_immediately_raise_pair_exposure()
    {
        let mut baseline = ComfortTracker::default();
        let mut spike = ComfortTracker::default();
        for _ in 0..500 {
            baseline.record([7.2, 0.0, 0.0]);
            spike.record([7.2, 0.0, 0.0]);
        }
        for _ in 0..20 {
            baseline.record([7.2, 0.0, 0.0]);
            spike.record([16.0, 0.0, 0.0]);
        }
        assert_eq!(baseline.pair_total, spike.pair_total);
    }

    #[test]
    fn brief_high_force_within_sustained_force_does_not_immediately_raise_axis_exposure() {
        let mut baseline = ComfortTracker::default();
        let mut spike = ComfortTracker::default();
        for _ in 0..500 {
            baseline.record([7.2, 0.0, 0.0]);
            spike.record([7.2, 0.0, 0.0]);
        }
        for _ in 0..20 {
            baseline.record([7.2, 0.0, 0.0]);
            spike.record([16.0, 0.0, 0.0]);
        }
        assert_eq!(baseline.axis_total, spike.axis_total);
    }

    #[test]
    fn streaming_waits_for_two_hundred_milliseconds() {
        let mut tracker = ComfortTracker::default();
        for _ in 0..40 {
            tracker.record([7.0, 0.0, 0.0]);
        }
        assert_eq!(tracker.finish(), 0.0);
        tracker.record([7.0, 0.0, 0.0]);
        assert!(tracker.finish() > 0.0);
    }

    #[test]
    fn streaming_hash_includes_active_history() {
        let mut first = ComfortTracker::default();
        let mut second = ComfortTracker::default();
        first.record([1.0, 0.0, 0.0]);
        second.record([1.0, 1.0, 0.0]);
        let mut first_hash = crate::state_hash::StateHasher::new();
        let mut second_hash = crate::state_hash::StateHasher::new();
        first.hash_into(&mut first_hash);
        second.hash_into(&mut second_hash);
        assert_ne!(first_hash.finish(), second_hash.finish());
    }

    #[test]
    fn sustained_axis_streams_are_independent_and_bounded_for_large_spikes() {
        let mut mild = ComfortTracker::default();
        let mut harsh = ComfortTracker::default();
        for _ in 0..12_000 {
            mild.record([1.0, 0.0, 0.0]);
            harsh.record([1.0, 286.0, 0.0]);
        }
        assert_eq!(mild.finish(), 0.0);
        assert!(harsh.finish().is_finite() && harsh.finish() > 0.0);
    }
}

#[cfg(test)]
mod optimization_tests {
    use super::ComfortTracker;
    use crate::state_hash::StateHasher;

    #[test]
    fn optimized_force_history_throughput() {
        let mut tracker = ComfortTracker::default();
        for index in 0..500_000 {
            let lateral = if index % 5000 < 2500 { 0.2 } else { 1.0 };
            tracker.record([1.2, lateral, 0.1]);
        }
        assert!(tracker.finish().is_finite());
    }

    #[test]
    fn reference_force_history_throughput() {
        let mut tracker = ComfortTracker::default();
        for index in 0..500_000 {
            let lateral = if index % 5000 < 2500 { 0.2 } else { 1.0 };
            tracker.record_reference([1.2, lateral, 0.1]);
        }
        assert!(tracker.finish().is_finite());
    }

    #[test]
    fn threshold_updates_preserve_state_across_rising_and_falling_forces() {
        let mut tracker = ComfortTracker::default();
        let mut reference = ComfortTracker::default();
        for reading in [
            [1.0, 0.0, 0.0],
            [8.0, 5.0, -3.0],
            [25.0, -10.0, 4.0],
            [1.0, 0.0, 0.0],
        ] {
            for _ in 0..400 {
                tracker.record(reading);
                reference.record_reference(reading);
            }
            let mut actual = StateHasher::new();
            let mut expected = StateHasher::new();
            tracker.hash_into(&mut actual);
            reference.hash_into(&mut expected);
            assert_eq!(actual.finish(), expected.finish());
        }
    }
}
