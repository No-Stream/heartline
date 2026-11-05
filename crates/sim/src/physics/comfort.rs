use super::features::Location;

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
#[cfg(test)]
const SAMPLE_RATE_HZ: f64 = 200.0;
#[cfg(test)]
const MINIMUM_SUSTAINED_INTERVALS: usize = 40;
#[cfg(test)]
type DurationCurve = (usize, f64, &'static [(f64, f64)]);

#[cfg(test)]
struct LowPass {
    stages: [Biquad; 2],
}

#[cfg(test)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    state1: f64,
    state2: f64,
}

#[cfg(test)]
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

#[cfg(test)]
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
#[cfg(test)]
pub(super) struct ComfortTracker {
    filters: Option<[LowPass; 3]>,
    durations: [[u32; 87]; 6],
    pair_durations: [[u32; 141]; 3],
    active_axes: [usize; 6],
    active_pairs: [usize; 3],
    pair_total: [f64; 3],
    axis_total: f64,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
mod envelope_tests {
    use super::evaluate_envelope;
    use crate::physics::TestSample;
    use crate::physics::features::PieceSummary;
    use crate::track::{PieceId, PieceKind, Shape};

    fn evaluate(readings: &[(usize, f64, f64, f64)]) -> super::EnvelopeExceedance {
        let mut samples = Vec::new();
        for &(count, vertical_g, lateral_g, longitudinal_g) in readings {
            for _ in 0..count {
                samples.push(TestSample {
                    distance: samples.len() as f64,
                    speed: 10.0,
                    vertical_g,
                    lateral_g,
                    longitudinal_g,
                    airtime: false,
                    height_m: 0.0,
                    inverted: false,
                    roll: 0.0,
                    piece_index: 0,
                });
            }
        }
        let piece = PieceSummary {
            id: PieceId::Loop,
            name: "Loop",
            kind: PieceKind::Track,
            shape: Shape::Plain,
            inversion: None,
            heartlining: None,
            length: samples.len() as f64,
            turn_angle_radians: 0.0,
        };
        evaluate_envelope(&samples, &[piece], samples.len() as f64, true)
    }

    #[test]
    fn streaming_tracks_stationary_forces_and_matches_sample_intervals() {
        let mut tracker = super::EnvelopeTracker::default();
        for index in 0..30 {
            tracker.record(
                [5.0, 0.0, 0.0],
                0.1,
                true,
                super::Location {
                    piece_index: 0,
                    piece_name: "Loop",
                    distance_m: index as f64,
                    seconds_from_start: 0.0,
                },
            );
        }
        assert_eq!(tracker.finish(), evaluate(&[(30, 5.0, 0.0, 0.0)]));
        let mut stationary = super::EnvelopeTracker::default();
        for _ in 0..400 {
            stationary.record(
                [5.0, 0.0, 0.0],
                0.005,
                true,
                super::Location {
                    piece_index: 0,
                    piece_name: "Loop",
                    distance_m: 12.0,
                    seconds_from_start: 0.0,
                },
            );
        }
        assert!(stationary.finish().g_seconds > 0.0);
        assert_eq!(stationary.finish().worst.unwrap().location.distance_m, 12.0);
    }

    #[test]
    fn onset_detects_smooth_fast_ramp_but_not_slow_ramp() {
        let ramp = |step_g: f64| {
            let mut tracker = super::EnvelopeTracker::default();
            for step in 0..=(2.0 / step_g).ceil() as usize {
                tracker.record(
                    [step as f64 * step_g, 0.0, 0.0],
                    0.005,
                    true,
                    super::Location {
                        piece_index: 0,
                        piece_name: "Loop",
                        distance_m: step as f64,
                        seconds_from_start: 0.0,
                    },
                );
            }
            tracker.finish()
        };
        let fast = ramp(0.1);
        assert_eq!(fast.worst.unwrap().axis, super::EnvelopeAxis::Onset);
        assert!(fast.g_seconds > 0.0);
        assert_eq!(ramp(0.05).g_seconds, 0.0);
    }

    #[test]
    fn duration_curves_and_combined_axes() {
        assert_eq!(evaluate(&[(8, 5.0, 0.0, 0.0)]).g_seconds, 0.0);
        assert!(evaluate(&[(30, 5.0, 0.0, 0.0)]).g_seconds > 0.0);
        assert_eq!(evaluate(&[(50, -1.3, 0.0, 0.0)]).g_seconds, 0.0);
        assert!(evaluate(&[(80, -1.3, 0.0, 0.0)]).g_seconds > 0.0);
        assert_eq!(evaluate(&[(15, 1.0, 2.5, 0.0)]).g_seconds, 0.0);
        assert!(evaluate(&[(30, 1.0, 2.5, 0.0)]).g_seconds > 0.0);
        assert_eq!(evaluate(&[(10, 4.0, 2.5, 0.0)]).g_seconds, 0.0);
        assert!(evaluate(&[(10, 5.0, 2.5, 0.0)]).g_seconds > 0.0);
    }

    #[test]
    fn reversal_push_pull_onset_and_locations() {
        let reversal = evaluate(&[(3, 1.0, 2.5, 0.0), (3, 1.0, -2.5, 0.0)]);
        assert!(reversal.g_seconds > 0.0);
        let push_pull = evaluate(&[(40, -0.5, 0.0, 0.0), (10, 3.0, 0.0, 0.0)]);
        assert!(push_pull.g_seconds > 0.0);
        let worst = push_pull.worst.unwrap();
        assert_eq!(worst.location.piece_name, "Loop");
        assert!(worst.location.distance_m >= 40.0);
        assert!(!push_pull.stretches.is_empty());
        assert!(evaluate(&[(1, 0.0, 0.0, 0.0), (1, 3.0, 0.0, 0.0)]).g_seconds > 0.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnvelopeAxis {
    Vertical,
    Lateral,
    Longitudinal,
    VerticalLateral,
    VerticalLongitudinal,
    LateralLongitudinal,
    Onset,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EnvelopeWorst {
    pub(super) location: Location,
    pub(super) axis: EnvelopeAxis,
    pub(super) value: f64,
    pub(super) allowed: f64,
    pub(super) ratio: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EnvelopeStretch {
    pub(super) location: Location,
    pub(super) seconds: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct EnvelopeExceedance {
    pub(super) g_seconds: f64,
    pub(super) seconds: f64,
    pub(super) worst: Option<EnvelopeWorst>,
    pub(super) stretches: Vec<EnvelopeStretch>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct EnvelopeTracker {
    result: EnvelopeExceedance,
    clocks: [f64; 6],
    last_sign_times: [[Option<f64>; 2]; 2],
    negative_seconds: f64,
    push_pull_until: f64,
    onset_start: Option<(f64, f64)>,
    elapsed: f64,
    stretch: Option<EnvelopeStretch>,
}

impl EnvelopeTracker {
    pub(super) fn record(
        &mut self,
        reading: [f64; 3],
        dt: f64,
        active: bool,
        mut location: Location,
    ) {
        assert!(dt.is_finite() && dt >= 0.0);
        assert!(reading.into_iter().all(f64::is_finite));
        if !active || dt == 0.0 {
            self.clocks = [0.0; 6];
            self.last_sign_times = [[None; 2]; 2];
            self.negative_seconds = 0.0;
            self.push_pull_until = 0.0;
            self.onset_start = None;
            finish_stretch(&mut self.result.stretches, self.stretch.take());
            self.elapsed += dt;
            return;
        }
        location.seconds_from_start = self.elapsed;
        const CURVES: [&[(f64, f64)]; 6] = [
            &[
                (0.2, 6.0),
                (1.0, 6.0),
                (2.0, 4.0),
                (4.0, 4.0),
                (5.0, 3.0),
                (11.8, 3.0),
                (12.0, 2.0),
            ],
            &[(0.2, 2.0), (0.5, 1.5), (4.0, 1.5), (7.0, 1.1)],
            &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)],
            &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)],
            &[
                (0.2, 6.0),
                (1.0, 6.0),
                (2.0, 4.0),
                (4.0, 4.0),
                (5.0, 3.0),
                (11.8, 3.0),
                (12.0, 2.5),
            ],
            &[(0.2, 2.0), (0.5, 1.5)],
        ];
        let plateaus = [2.0, 1.1, 2.0, 2.0, 2.5, 1.5];
        let axes = [
            EnvelopeAxis::Vertical,
            EnvelopeAxis::Vertical,
            EnvelopeAxis::Lateral,
            EnvelopeAxis::Lateral,
            EnvelopeAxis::Longitudinal,
            EnvelopeAxis::Longitudinal,
        ];
        let pairs = [
            (0, 1, EnvelopeAxis::VerticalLateral),
            (0, 2, EnvelopeAxis::VerticalLongitudinal),
            (1, 2, EnvelopeAxis::LateralLongitudinal),
        ];
        let [vertical, lateral, longitudinal] = reading;
        if vertical >= 0.0 && self.negative_seconds > 3.0 {
            self.push_pull_until = self.elapsed + 6.0;
        }
        self.negative_seconds = if vertical < 0.0 {
            self.negative_seconds + dt
        } else {
            0.0
        };
        let mut reversed = [false; 2];
        for (axis, value) in [lateral, longitudinal].into_iter().enumerate() {
            if value.abs() >= 0.5 {
                let sign = usize::from(value >= 0.0);
                reversed[axis] = self.last_sign_times[axis][1 - sign]
                    .is_some_and(|time| self.elapsed - time < 0.2);
                self.last_sign_times[axis][sign] = Some(self.elapsed);
            }
        }
        let magnitudes = [
            vertical,
            -vertical,
            lateral,
            -lateral,
            longitudinal,
            -longitudinal,
        ];
        let mut candidate = |axis, value: f64, allowed: f64, ratio: f64| {
            if ratio > 0.0
                && self
                    .result
                    .worst
                    .as_ref()
                    .is_none_or(|worst| ratio > worst.ratio)
            {
                self.result.worst = Some(EnvelopeWorst {
                    location,
                    axis,
                    value,
                    allowed,
                    ratio,
                });
            }
        };
        let mut max_ratio: f64 = 0.0;
        for axis in 0..6 {
            if magnitudes[axis] > plateaus[axis] {
                self.clocks[axis] += dt;
            } else {
                self.clocks[axis] = 0.0;
            }
            let mut limit = curve_limit(CURVES[axis], self.clocks[axis].max(0.2));
            if axis == 0 && self.elapsed < self.push_pull_until {
                limit = limit.min(2.0);
            }
            if (axis == 2 || axis == 3) && reversed[0] || (axis == 4 || axis == 5) && reversed[1] {
                limit *= 0.5;
            }
            let ratio = (magnitudes[axis] / limit - 1.0).max(0.0);
            candidate(axes[axis], magnitudes[axis], limit, ratio);
            max_ratio = max_ratio.max(ratio);
        }
        let normalized = [
            vertical / if vertical >= 0.0 { 6.6 } else { 2.2 },
            lateral / 3.3,
            longitudinal / if longitudinal >= 0.0 { 6.6 } else { 2.2 },
        ];
        for (left, right, axis) in pairs {
            let value = normalized[left].hypot(normalized[right]);
            let ratio = (value - 1.0).max(0.0);
            candidate(axis, value, 1.0, ratio);
            max_ratio = max_ratio.max(ratio);
        }
        if vertical <= 0.0 {
            self.onset_start = Some((vertical, self.elapsed));
        } else if vertical >= 2.0
            && let Some((start_vertical, start_time)) = self.onset_start.take()
        {
            let onset_rate = (vertical - start_vertical) / (self.elapsed - start_time);
            if onset_rate > 15.0 {
                candidate(EnvelopeAxis::Onset, onset_rate, 15.0, 0.5);
                max_ratio = max_ratio.max(0.5);
            }
        }
        if max_ratio > 0.0 {
            self.result.g_seconds += max_ratio * dt;
            self.result.seconds += dt;
            self.stretch
                .get_or_insert(EnvelopeStretch {
                    location,
                    seconds: 0.0,
                })
                .seconds += dt;
        } else {
            finish_stretch(&mut self.result.stretches, self.stretch.take());
        }
        self.elapsed += dt;
    }

    pub(super) fn finish(&self) -> EnvelopeExceedance {
        let mut result = self.result.clone();
        finish_stretch(&mut result.stretches, self.stretch.clone());
        result
            .stretches
            .sort_by(|left, right| right.seconds.total_cmp(&left.seconds));
        result
    }

    pub(super) fn hash_into(&self, hash: &mut crate::state_hash::StateHasher) {
        for value in self.clocks {
            hash.write_f64(value);
        }
        for axis in self.last_sign_times {
            for value in axis {
                hash.write_option(value, |hash, value| hash.write_f64(value));
            }
        }
        hash.write_f64(self.negative_seconds);
        hash.write_f64(self.push_pull_until);
        hash.write_option(self.onset_start, |hash, (value, time)| {
            hash.write_f64(value);
            hash.write_f64(time);
        });
        hash.write_f64(self.elapsed);
        hash.write_f64(self.result.g_seconds);
        hash.write_f64(self.result.seconds);
        hash.write_option(self.result.worst.as_ref(), |hash, worst| {
            hash_location(hash, &worst.location);
            hash.write_u8(worst.axis as u8);
            hash.write_f64(worst.value);
            hash.write_f64(worst.allowed);
            hash.write_f64(worst.ratio);
        });
        hash.write_len(self.result.stretches.len());
        for stretch in &self.result.stretches {
            hash_stretch(hash, stretch);
        }
        hash.write_option(self.stretch.as_ref(), hash_stretch);
    }
}

fn hash_location(hash: &mut crate::state_hash::StateHasher, location: &Location) {
    hash.write_len(location.piece_index);
    hash.write_bytes(location.piece_name.as_bytes());
    hash.write_f64(location.distance_m);
    hash.write_f64(location.seconds_from_start);
}

fn hash_stretch(hash: &mut crate::state_hash::StateHasher, stretch: &EnvelopeStretch) {
    hash_location(hash, &stretch.location);
    hash.write_f64(stretch.seconds);
}

/// Evaluates the scoring model's approximate comfort envelope over sampled track intervals.
#[allow(dead_code)]
pub(super) fn evaluate_envelope(
    samples: &[super::TestSample],
    pieces: &[super::features::PieceSummary],
    circuit_length: f64,
    completed: bool,
) -> EnvelopeExceedance {
    use super::features::{is_active_sample, sample_interval};
    let mut tracker = EnvelopeTracker::default();
    for (index, sample) in samples.iter().enumerate() {
        let dt = sample_interval(samples, index, circuit_length, completed).seconds;
        let location = Location {
            piece_index: sample.piece_index,
            piece_name: pieces[sample.piece_index].name,
            distance_m: sample.distance,
            seconds_from_start: 0.0,
        };
        tracker.record(
            [sample.vertical_g, sample.lateral_g, sample.longitudinal_g],
            dt,
            is_active_sample(sample, pieces),
            location,
        );
    }
    tracker.finish()
}

fn finish_stretch(stretches: &mut Vec<EnvelopeStretch>, stretch: Option<EnvelopeStretch>) {
    if let Some(stretch) = stretch
        && stretch.seconds > 0.2
    {
        stretches.push(stretch);
    }
}
