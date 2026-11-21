use super::features::Location;
use crate::state_hash::StateHasher;

// ASTM F2291-06a / F2291-23b acceleration limits (sections 7.1.1.2, 7.1.4–7.1.7,
// Appendix X2 isoacceleration slicing, and Figures 6–20).
// See docs/ride-score-calibration.md ("ASTM F2291 envelope check, 2026-11-21")
// for the comparison against the standard and the player-favourable choices.
// This is a game calibration model, not a safety assessment.

const MINIMUM_SUSTAINED_SECONDS: f64 = 0.2;
const DURATION_EPSILON: f64 = 1e-9;

const POSITIVE_VERTICAL_CURVE: &[(f64, f64)] = &[
    (0.2, 6.0),
    (1.0, 6.0),
    (2.0, 4.0),
    (4.0, 4.0),
    (5.0, 3.0),
    (11.8, 3.0),
    (12.0, 2.0),
];
const PUSH_PULL_VERTICAL_CURVE: &[(f64, f64)] =
    &[(0.2, 5.0), (1.5, 5.0), (2.0, 4.0), (2.5, 2.0), (6.0, 2.0)];
const NEGATIVE_VERTICAL_CURVE: &[(f64, f64)] = &[(0.2, 2.0), (0.5, 1.5), (4.0, 1.5), (7.0, 1.1)];
const LATERAL_CURVE: &[(f64, f64)] = &[(0.2, 3.0), (1.0, 3.0), (2.0, 2.0)];
const FORWARD_LONGITUDINAL_CURVE: &[(f64, f64)] = &[
    (0.2, 6.0),
    (1.0, 6.0),
    (2.0, 4.0),
    (4.0, 4.0),
    (5.0, 3.0),
    (11.8, 3.0),
    (12.0, 2.5),
];
const BRAKING_LONGITUDINAL_CURVE: &[(f64, f64)] = &[(0.2, 2.0), (0.5, 1.5)];

const CURVES: [&[(f64, f64)]; 6] = [
    POSITIVE_VERTICAL_CURVE,
    NEGATIVE_VERTICAL_CURVE,
    LATERAL_CURVE,
    LATERAL_CURVE,
    FORWARD_LONGITUDINAL_CURVE,
    BRAKING_LONGITUDINAL_CURVE,
];
const PLATEAUS: [f64; 6] = [2.0, 1.1, 2.0, 2.0, 2.5, 1.5];

/// Halved 0.2 s single-axis limits applied to rapid sustained reversals (F2291 7.1.6.1).
const REVERSAL_LATERAL_LIMIT_G: f64 = 1.5;
const REVERSAL_FORWARD_LIMIT_G: f64 = 3.0;
const REVERSAL_BRAKING_LIMIT_G: f64 = 1.0;
const REVERSAL_LOBE_THRESHOLD_G: f64 = 0.5;

fn curve_limit(curve: &[(f64, f64)], duration: f64) -> f64 {
    if duration <= curve[0].0 {
        return curve[0].1;
    }
    for endpoints in curve.windows(2) {
        let [(start_time, start_limit), (end_time, end_limit)] = [endpoints[0], endpoints[1]];
        if duration <= end_time {
            let fraction = (duration - start_time) / (end_time - start_time);
            return start_limit + fraction * (end_limit - start_limit);
        }
    }
    curve.last().unwrap().1
}

/// Maintains the exact continuous running suffix minima above `plateau`.
/// Every entry `(value, start_time)` satisfies: the signal has been continuously
/// `>= value` from `start_time` through the current step's end.
/// Entries are strictly increasing in both `value` and `start_time`.
fn push_level(stack: &mut Vec<(f64, f64)>, value: f64, start_time: f64, plateau: f64) {
    if value <= plateau {
        stack.clear();
        return;
    }
    let mut earliest_start = start_time;
    while let Some(&(top_value, top_start)) = stack.last() {
        if top_value >= value {
            earliest_start = top_start;
            stack.pop();
        } else {
            break;
        }
    }
    stack.push((value, earliest_start));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum EnvelopeAxis {
    Vertical,
    Lateral,
    Longitudinal,
    VerticalLateral,
    VerticalLongitudinal,
    LateralLongitudinal,
    Onset,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnvelopeWorst {
    pub(crate) location: Location,
    pub(crate) axis: EnvelopeAxis,
    pub(crate) value: f64,
    pub(crate) allowed: f64,
    pub(crate) ratio: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnvelopeStretch {
    pub(crate) location: Location,
    pub(crate) seconds: f64,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnvelopeExceedance {
    pub(crate) g_seconds: f64,
    pub(crate) seconds: f64,
    pub(crate) worst: Option<EnvelopeWorst>,
    pub(crate) stretches: Vec<EnvelopeStretch>,
}

/// Per-step output from the ASTM comfort envelope tracker.
/// Axis order is `[vertical, lateral, longitudinal]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EnvelopeStep {
    pub(crate) axis_exceeded: [bool; 3],
    pub(crate) axis_ratio: [f64; 3],
    pub(crate) axis_above_plateau: [bool; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ReversalLobe {
    sign: i8,
    start_time: f64,
    peak_magnitude: f64,
    peak_time: f64,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnvelopeTracker {
    result: EnvelopeExceedance,
    axis_stacks: [Vec<(f64, f64)>; 6],
    pair_stacks: [Vec<(f64, f64)>; 3],
    active_lobes: [Option<ReversalLobe>; 2],
    last_sustained_peak: [[Option<f64>; 2]; 2],
    negative_seconds: f64,
    push_pull_until: f64,
    weightless_seconds: f64,
    onset_start: Option<(f64, f64)>,
    elapsed: f64,
    stretch: Option<EnvelopeStretch>,
}

impl EnvelopeTracker {
    pub(crate) fn elapsed(&self) -> f64 {
        self.elapsed
    }

    pub(crate) fn record(
        &mut self,
        reading: [f64; 3],
        dt: f64,
        active: bool,
        mut location: Location,
    ) -> EnvelopeStep {
        assert!(dt.is_finite() && dt >= 0.0);
        assert!(reading.into_iter().all(f64::is_finite));
        if !active || dt == 0.0 {
            for stack in &mut self.axis_stacks {
                stack.clear();
            }
            for stack in &mut self.pair_stacks {
                stack.clear();
            }
            self.active_lobes = [None; 2];
            self.last_sustained_peak = [[None; 2]; 2];
            self.negative_seconds = 0.0;
            self.push_pull_until = 0.0;
            self.weightless_seconds = 0.0;
            self.onset_start = None;
            finish_stretch(&mut self.result.stretches, self.stretch.take());
            self.elapsed += dt;
            return EnvelopeStep::default();
        }

        let step_start = self.elapsed;
        let step_end = step_start + dt;
        location.seconds_from_start = step_start;

        let [vertical, lateral, longitudinal] = reading;

        // Post-airtime reduced +Gz trigger (F2291 7.1.7.1): more than 3 s of -Gz
        // arms a 6 s reduced +Gz curve on return to >= 0 g.
        if vertical >= 0.0 && self.negative_seconds > 3.0 {
            self.push_pull_until = step_start + 6.0;
        }
        self.negative_seconds = if vertical < 0.0 {
            self.negative_seconds + dt
        } else {
            0.0
        };

        let mut step_worst_ratio = 0.0_f64;
        let mut axis_ratio = [0.0_f64; 3];
        let mut candidate = |axis: EnvelopeAxis, value: f64, allowed: f64, ratio: f64| {
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

        // 1. Single-axis isoacceleration per-level duration curves (F2291 Appendix X2),
        // ignoring events <= 200 ms.
        let magnitudes = [
            vertical,
            -vertical,
            lateral,
            -lateral,
            longitudinal,
            -longitudinal,
        ];
        let envelope_axes = [
            EnvelopeAxis::Vertical,
            EnvelopeAxis::Vertical,
            EnvelopeAxis::Lateral,
            EnvelopeAxis::Lateral,
            EnvelopeAxis::Longitudinal,
            EnvelopeAxis::Longitudinal,
        ];
        let axis_indices = [0_usize, 0, 1, 1, 2, 2];
        let in_push_pull = step_start < self.push_pull_until;

        for curve_index in 0..6 {
            push_level(
                &mut self.axis_stacks[curve_index],
                magnitudes[curve_index],
                step_start,
                PLATEAUS[curve_index],
            );
            let mut worst_axis_ratio = 0.0_f64;
            let mut worst_level = 0.0_f64;
            let mut worst_limit = 0.0_f64;
            for &(level, start_time) in &self.axis_stacks[curve_index] {
                let duration = step_end - start_time;
                if duration <= MINIMUM_SUSTAINED_SECONDS + DURATION_EPSILON {
                    break;
                }
                let mut limit = curve_limit(CURVES[curve_index], duration);
                if curve_index == 0 && in_push_pull {
                    limit = limit.min(curve_limit(PUSH_PULL_VERTICAL_CURVE, duration));
                }
                let ratio = (level / limit - 1.0).max(0.0);
                if ratio > worst_axis_ratio {
                    worst_axis_ratio = ratio;
                    worst_level = level;
                    worst_limit = limit;
                }
            }
            if worst_axis_ratio > 0.0 {
                candidate(
                    envelope_axes[curve_index],
                    worst_level,
                    worst_limit,
                    worst_axis_ratio,
                );
                axis_ratio[axis_indices[curve_index]] =
                    axis_ratio[axis_indices[curve_index]].max(worst_axis_ratio);
                step_worst_ratio = step_worst_ratio.max(worst_axis_ratio);
            }
        }

        // 2. Sustained lateral and longitudinal reversals (F2291 7.1.6.1):
        // both opposite-sign lobes must be sustained (>= 200 ms), peak-to-peak < 200 ms,
        // halving the 0.2 s limit (1.5 g lateral, 3.0 g forward, 1.0 g braking).
        for (rev_index, (value, envelope_axis, axis_index)) in [
            (lateral, EnvelopeAxis::Lateral, 1_usize),
            (longitudinal, EnvelopeAxis::Longitudinal, 2_usize),
        ]
        .into_iter()
        .enumerate()
        {
            let sign: i8 = if value >= REVERSAL_LOBE_THRESHOLD_G {
                1
            } else if value <= -REVERSAL_LOBE_THRESHOLD_G {
                -1
            } else {
                0
            };
            if let Some(previous) = self.active_lobes[rev_index]
                && previous.sign != sign
            {
                if step_start - previous.start_time >= MINIMUM_SUSTAINED_SECONDS - DURATION_EPSILON
                {
                    let sign_slot = usize::from(previous.sign > 0);
                    self.last_sustained_peak[rev_index][sign_slot] = Some(previous.peak_time);
                }
                self.active_lobes[rev_index] = None;
            }
            if sign != 0 {
                let magnitude = value.abs();
                let lobe = match &mut self.active_lobes[rev_index] {
                    Some(existing) => {
                        if magnitude > existing.peak_magnitude {
                            existing.peak_magnitude = magnitude;
                            existing.peak_time = step_start;
                        }
                        *existing
                    }
                    slot => {
                        let started = ReversalLobe {
                            sign,
                            start_time: step_start,
                            peak_magnitude: magnitude,
                            peak_time: step_start,
                        };
                        *slot = Some(started);
                        started
                    }
                };
                let lobe_duration = step_end - lobe.start_time;
                if lobe_duration >= MINIMUM_SUSTAINED_SECONDS - DURATION_EPSILON {
                    let opposite_slot = usize::from(sign < 0);
                    if let Some(previous_peak_time) =
                        self.last_sustained_peak[rev_index][opposite_slot]
                        && lobe.peak_time - previous_peak_time
                            < MINIMUM_SUSTAINED_SECONDS - DURATION_EPSILON
                    {
                        let halved_limit = if rev_index == 0 {
                            REVERSAL_LATERAL_LIMIT_G
                        } else if sign > 0 {
                            REVERSAL_FORWARD_LIMIT_G
                        } else {
                            REVERSAL_BRAKING_LIMIT_G
                        };
                        let just_became_sustained = step_start - lobe.start_time
                            < MINIMUM_SUSTAINED_SECONDS - DURATION_EPSILON;
                        let checked_magnitude = if just_became_sustained {
                            magnitude.max(lobe.peak_magnitude)
                        } else {
                            magnitude
                        };
                        let ratio = (checked_magnitude / halved_limit - 1.0).max(0.0);
                        if ratio > 0.0 {
                            candidate(envelope_axis, checked_magnitude, halved_limit, ratio);
                            axis_ratio[axis_index] = axis_ratio[axis_index].max(ratio);
                            step_worst_ratio = step_worst_ratio.max(ratio);
                        }
                    }
                }
            }
        }

        // 3. Combined-axis 1.1x ellipses (F2291-06a 7.1.5.1), ignoring events <= 200 ms.
        let normalized = [
            vertical / if vertical >= 0.0 { 6.6 } else { 2.2 },
            lateral / 3.3,
            longitudinal / if longitudinal >= 0.0 { 6.6 } else { 2.2 },
        ];
        let pairs = [
            (0_usize, 1_usize, EnvelopeAxis::VerticalLateral),
            (0, 2, EnvelopeAxis::VerticalLongitudinal),
            (1, 2, EnvelopeAxis::LateralLongitudinal),
        ];
        for (pair_index, (left, right, axis)) in pairs.into_iter().enumerate() {
            let magnitude = normalized[left].hypot(normalized[right]);
            push_level(
                &mut self.pair_stacks[pair_index],
                magnitude,
                step_start,
                1.0,
            );
            let mut worst_level = 0.0_f64;
            for &(level, start_time) in &self.pair_stacks[pair_index] {
                let duration = step_end - start_time;
                if duration <= MINIMUM_SUSTAINED_SECONDS + DURATION_EPSILON {
                    break;
                }
                worst_level = level;
            }
            if worst_level > 1.0 {
                let ratio = worst_level - 1.0;
                candidate(axis, worst_level, 1.0, ratio);
                axis_ratio[left] = axis_ratio[left].max(ratio);
                axis_ratio[right] = axis_ratio[right].max(ratio);
                step_worst_ratio = step_worst_ratio.max(ratio);
            }
        }

        // 4. Onset from sustained weightlessness (F2291 7.1.7.2):
        // only checked when vertical stayed <= 0 g for at least 200 ms.
        if vertical <= 0.0 {
            self.weightless_seconds += dt;
            if self.weightless_seconds >= MINIMUM_SUSTAINED_SECONDS - DURATION_EPSILON {
                self.onset_start = Some((vertical, step_start));
            }
        } else {
            self.weightless_seconds = 0.0;
            if vertical >= 2.0
                && let Some((start_vertical, start_time)) = self.onset_start.take()
            {
                let dt_onset = step_start - start_time;
                if dt_onset > 0.0 {
                    let onset_rate = (vertical - start_vertical) / dt_onset;
                    if onset_rate > 15.0 {
                        candidate(EnvelopeAxis::Onset, onset_rate, 15.0, 0.5);
                        axis_ratio[0] = axis_ratio[0].max(0.5);
                        step_worst_ratio = step_worst_ratio.max(0.5);
                    }
                }
            }
        }

        if step_worst_ratio > 0.0 {
            self.result.g_seconds += step_worst_ratio * dt;
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
        self.elapsed = step_end;

        let axis_above_plateau = [
            !self.axis_stacks[0].is_empty()
                || !self.axis_stacks[1].is_empty()
                || !self.pair_stacks[0].is_empty()
                || !self.pair_stacks[1].is_empty(),
            !self.axis_stacks[2].is_empty()
                || !self.axis_stacks[3].is_empty()
                || !self.pair_stacks[0].is_empty()
                || !self.pair_stacks[2].is_empty(),
            !self.axis_stacks[4].is_empty()
                || !self.axis_stacks[5].is_empty()
                || !self.pair_stacks[1].is_empty()
                || !self.pair_stacks[2].is_empty(),
        ];
        EnvelopeStep {
            axis_exceeded: axis_ratio.map(|ratio| ratio > 0.0),
            axis_ratio,
            axis_above_plateau,
        }
    }

    pub(crate) fn finish(&self) -> EnvelopeExceedance {
        let mut result = self.result.clone();
        finish_stretch(&mut result.stretches, self.stretch.clone());
        result
            .stretches
            .sort_by(|left, right| right.seconds.total_cmp(&left.seconds));
        result
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"EnvelopeTracker-v2");
        for stack in &self.axis_stacks {
            hash.write_len(stack.len());
            for &(value, start_time) in stack {
                hash.write_f64(value);
                hash.write_f64(start_time);
            }
        }
        for stack in &self.pair_stacks {
            hash.write_len(stack.len());
            for &(value, start_time) in stack {
                hash.write_f64(value);
                hash.write_f64(start_time);
            }
        }
        for lobe in self.active_lobes {
            hash.write_option(lobe, |hash, lobe| {
                hash.write_i8(lobe.sign);
                hash.write_f64(lobe.start_time);
                hash.write_f64(lobe.peak_magnitude);
                hash.write_f64(lobe.peak_time);
            });
        }
        for axis in self.last_sustained_peak {
            for value in axis {
                hash.write_option(value, |hash, value| hash.write_f64(value));
            }
        }
        hash.write_f64(self.negative_seconds);
        hash.write_f64(self.push_pull_until);
        hash.write_f64(self.weightless_seconds);
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

fn hash_location(hash: &mut StateHasher, location: &Location) {
    hash.write_len(location.piece_index);
    hash.write_bytes(location.piece_name.as_bytes());
    hash.write_f64(location.distance_m);
    hash.write_f64(location.seconds_from_start);
}

fn hash_stretch(hash: &mut StateHasher, stretch: &EnvelopeStretch) {
    hash_location(hash, &stretch.location);
    hash.write_f64(stretch.seconds);
}

/// Evaluates the scoring model's ASTM comfort envelope over sampled track intervals.
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

#[cfg(test)]
mod envelope_tests {
    use super::{EnvelopeAxis, EnvelopeExceedance, EnvelopeTracker, evaluate_envelope};
    use crate::physics::features::{Location, PieceSummary};
    use crate::physics::{ForceFilter, GForces, TIME_STEP, TestSample};
    use crate::state_hash::StateHasher;
    use crate::track::{PieceId, PieceKind, Shape};

    fn location(distance_m: f64) -> Location {
        Location {
            piece_index: 0,
            piece_name: "Loop",
            distance_m,
            seconds_from_start: 0.0,
        }
    }

    fn evaluate(readings: &[(usize, f64, f64, f64)]) -> EnvelopeExceedance {
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
                    force_limit_exceeded: [false; 3],
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

    fn stream(readings: &[[f64; 3]]) -> EnvelopeExceedance {
        let mut tracker = EnvelopeTracker::default();
        for (index, &reading) in readings.iter().enumerate() {
            tracker.record(reading, TIME_STEP, true, location(index as f64));
        }
        tracker.finish()
    }

    fn filtered_stream(readings: &[[f64; 3]]) -> EnvelopeExceedance {
        let mut filter = ForceFilter::new(TIME_STEP);
        let mut tracker = EnvelopeTracker::default();
        for (index, &reading) in readings.iter().enumerate() {
            let filtered = filter.update(GForces {
                vertical: reading[0],
                lateral: reading[1],
                longitudinal: reading[2],
            });
            tracker.record(
                [filtered.vertical, filtered.lateral, filtered.longitudinal],
                TIME_STEP,
                true,
                location(index as f64),
            );
        }
        tracker.finish()
    }

    #[test]
    fn normal_forces_stay_inside_the_envelope() {
        assert_eq!(stream(&[[1.0, 0.0, 0.0]; 400]).g_seconds, 0.0);
        assert_eq!(filtered_stream(&[[1.0, 0.0, 0.0]; 400]).g_seconds, 0.0);
    }

    #[test]
    fn events_up_to_two_hundred_milliseconds_are_ignored() {
        // 40 samples at 200 Hz = 0.200 s: allowed under the 200 ms minimum event rule.
        let mut tracker = EnvelopeTracker::default();
        for step in 0..40 {
            let result = tracker.record([7.0, 0.0, 0.0], TIME_STEP, true, location(step as f64));
            assert!(!result.axis_exceeded[0]);
        }
        assert_eq!(tracker.finish().g_seconds, 0.0);
        let over = tracker.record([7.0, 0.0, 0.0], TIME_STEP, true, location(40.0));
        assert!(over.axis_exceeded[0]);
        assert!(tracker.finish().g_seconds > 0.0);
    }

    #[test]
    fn brief_high_spike_inside_sustained_force_does_not_immediately_raise_exposure() {
        let mut baseline = EnvelopeTracker::default();
        let mut spike = EnvelopeTracker::default();
        for step in 0..500 {
            baseline.record([7.2, 0.0, 0.0], TIME_STEP, true, location(step as f64));
            spike.record([7.2, 0.0, 0.0], TIME_STEP, true, location(step as f64));
        }
        for step in 0..20 {
            baseline.record(
                [7.2, 0.0, 0.0],
                TIME_STEP,
                true,
                location((500 + step) as f64),
            );
            spike.record(
                [16.0, 0.0, 0.0],
                TIME_STEP,
                true,
                location((500 + step) as f64),
            );
        }
        assert_eq!(baseline.finish().g_seconds, spike.finish().g_seconds);
    }

    #[test]
    fn smooth_valley_passes_under_per_level_isoacceleration_slicing() {
        // Hand-calculated example from docs/ride-score-calibration.md:
        // rises 2.0 -> 4.2 g over 1 s, holds 4.2 g for 1.2 s, falls 4.2 -> 2.0 g over 1 s.
        // Under plateau-clock judging this failed; under per-level slicing every level passes.
        let mut readings = Vec::new();
        for step in 0..200 {
            let fraction = step as f64 / 200.0;
            readings.push([2.0 + 2.2 * fraction, 0.0, 0.0]);
        }
        readings.extend(vec![[4.2, 0.0, 0.0]; 240]);
        for step in 0..200 {
            let fraction = step as f64 / 200.0;
            readings.push([4.2 - 2.2 * fraction, 0.0, 0.0]);
        }
        assert_eq!(stream(&readings).g_seconds, 0.0);
    }

    #[test]
    fn sustained_simultaneous_axes_exceed_pair_ellipse_only_past_two_hundred_ms() {
        // 0.05 s (10 steps) is below 200 ms -> ignored.
        assert_eq!(stream(&[[5.5, 3.2, 0.0]; 10]).g_seconds, 0.0);
        // 0.25 s (50 steps) exceeds 200 ms -> flagged on both vertical and lateral.
        let mut tracker = EnvelopeTracker::default();
        let mut last = Default::default();
        for step in 0..50 {
            last = tracker.record([5.5, 3.2, 0.0], TIME_STEP, true, location(step as f64));
        }
        assert!(last.axis_exceeded[0] && last.axis_exceeded[1] && !last.axis_exceeded[2]);
        let result = tracker.finish();
        assert!(result.g_seconds > 0.0);
        assert_eq!(result.worst.unwrap().axis, EnvelopeAxis::VerticalLateral);
    }

    #[test]
    fn separate_axis_exposures_do_not_create_a_simultaneous_pair_excess() {
        let mut readings = vec![[5.0, 0.0, 0.0]; 500];
        readings.extend(vec![[0.0, 3.0, 0.0]; 500]);
        let result = stream(&readings);
        assert!(result.g_seconds > 0.0);
        assert!(matches!(
            result.worst.unwrap().axis,
            EnvelopeAxis::Vertical | EnvelopeAxis::Lateral
        ));

        // Non-overlapping in-limit exposures stay at zero.
        let mut mild = vec![[1.0, 0.0, 0.0]; 800];
        mild[100..300].fill([5.0, 0.0, 0.0]);
        mild[500..700].fill([1.0, 2.5, 0.0]);
        assert_eq!(stream(&mild).g_seconds, 0.0);
    }

    #[test]
    fn nested_single_axis_exposure_adds_to_duration_score() {
        let baseline = stream(&[[3.2, 0.0, 0.0]; 2400]);
        let mut readings = vec![[3.2, 0.0, 0.0]; 2400];
        readings[600..1600].fill([5.0, 0.0, 0.0]);
        assert!(stream(&readings).g_seconds > baseline.g_seconds);
    }

    #[test]
    fn large_finite_readings_keep_finite_envelope_results() {
        let mild = stream(&[[1.0, 0.0, 0.0]; 400]);
        let harsh = stream(&[[1.0, 286.0, 0.0]; 400]);
        assert_eq!(mild.g_seconds, 0.0);
        assert!(harsh.g_seconds.is_finite() && harsh.g_seconds > 0.0);
    }

    #[test]
    fn hash_covers_active_envelope_history() {
        let mut first = EnvelopeTracker::default();
        let mut second = EnvelopeTracker::default();
        first.record([3.0, 0.0, 0.0], TIME_STEP, true, location(0.0));
        second.record([3.0, 2.5, 0.0], TIME_STEP, true, location(0.0));
        let mut first_hash = StateHasher::new();
        let mut second_hash = StateHasher::new();
        first.hash_into(&mut first_hash);
        second.hash_into(&mut second_hash);
        assert_ne!(first_hash.finish(), second_hash.finish());
    }

    #[test]
    fn streaming_tracks_stationary_forces_and_matches_sample_intervals() {
        let mut tracker = EnvelopeTracker::default();
        for index in 0..30 {
            tracker.record([5.0, 0.0, 0.0], 0.1, true, location(index as f64));
        }
        assert_eq!(tracker.finish(), evaluate(&[(30, 5.0, 0.0, 0.0)]));

        let mut stationary = EnvelopeTracker::default();
        for _ in 0..400 {
            stationary.record([5.0, 0.0, 0.0], TIME_STEP, true, location(12.0));
        }
        assert!(stationary.finish().g_seconds > 0.0);
        assert_eq!(stationary.finish().worst.unwrap().location.distance_m, 12.0);
    }

    #[test]
    fn onset_requires_sustained_weightlessness_and_detects_fast_ramp() {
        let ramp = |weightless_steps: usize, step_g: f64| {
            let mut tracker = EnvelopeTracker::default();
            for step in 0..weightless_steps {
                tracker.record([0.0, 0.0, 0.0], TIME_STEP, true, location(step as f64));
            }
            for step in 1..=(2.0 / step_g).ceil() as usize {
                tracker.record(
                    [step as f64 * step_g, 0.0, 0.0],
                    TIME_STEP,
                    true,
                    location((weightless_steps + step) as f64),
                );
            }
            tracker.finish()
        };
        // Brief weightlessness (< 200 ms) does not trigger the onset rule.
        assert_eq!(ramp(10, 0.1).g_seconds, 0.0);
        // Sustained weightlessness (>= 200 ms) followed by > 15 g/s triggers onset.
        let fast = ramp(45, 0.1);
        assert_eq!(fast.worst.unwrap().axis, EnvelopeAxis::Onset);
        assert!(fast.g_seconds > 0.0);
        // Slow ramp (10 g/s) stays inside the onset limit.
        assert_eq!(ramp(45, 0.05).g_seconds, 0.0);
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
    fn push_pull_uses_reduced_curve_instead_of_flat_two_g_cap() {
        // 4.0 s of -0.5 g triggers push-pull for 6 s; ramp smoothly (10 g/s <= 15 g/s) to avoid onset.
        let ramp_to_3 = [
            (40, -0.5, 0.0, 0.0),
            (1, 0.5, 0.0, 0.0),
            (1, 1.5, 0.0, 0.0),
            (1, 2.5, 0.0, 0.0),
        ];
        // 1.0 s of 3.0 g is well below the 5.0 g reduced limit (used to fail flat 2.0 g cap).
        let mut short_ok = ramp_to_3.to_vec();
        short_ok.push((10, 3.0, 0.0, 0.0));
        assert_eq!(evaluate(&short_ok).g_seconds, 0.0);
        // 0.5 s of 5.5 g exceeds the 5.0 g reduced limit (though normal limit is 6.0 g).
        let mut short_high = ramp_to_3.to_vec();
        short_high.push((5, 5.5, 0.0, 0.0));
        assert!(evaluate(&short_high).g_seconds > 0.0);
        // 3.0 s of 3.0 g exceeds the 2.0 g reduced limit from 2.5 s onward.
        let mut sustained_readings = ramp_to_3.to_vec();
        sustained_readings.push((30, 3.0, 0.0, 0.0));
        let sustained = evaluate(&sustained_readings);
        assert!(sustained.g_seconds > 0.0);
        let worst = sustained.worst.unwrap();
        assert_eq!(worst.location.piece_name, "Loop");
        assert!(worst.location.distance_m >= 40.0);
        assert!(!sustained.stretches.is_empty());
    }

    #[test]
    fn reversals_require_two_sustained_lobes_with_close_peaks_and_halve_point_two_second_limit() {
        // Brief opposite spikes (< 200 ms each) do not trigger a reversal.
        let mut brief = vec![[1.0, 2.5, 0.0]; 20];
        brief.extend(vec![[1.0, -2.5, 0.0]; 20]);
        assert_eq!(stream(&brief).g_seconds, 0.0);

        // Two sustained lobes (>= 200 ms each) whose peaks are 0.1 s apart and exceed 1.5 g lateral:
        // lobe 1 ramps up so its peak (2.0 g) is near its end (t = 0.20 s);
        // lobe 2 peaks immediately at -2.0 g (t = 0.21 s) and stays >= 0.5 g for 0.25 s.
        let mut sustained_rapid = Vec::new();
        for step in 0..42 {
            let lat = 0.6 + 1.4 * (step as f64 / 41.0);
            sustained_rapid.push([1.0, lat, 0.0]);
        }
        sustained_rapid.push([1.0, -2.0, 0.0]);
        sustained_rapid.extend(vec![[1.0, -0.8, 0.0]; 45]);
        let rapid = stream(&sustained_rapid);
        assert!(rapid.g_seconds > 0.0);
        let worst = rapid.worst.unwrap();
        assert_eq!(worst.axis, EnvelopeAxis::Lateral);
        assert_eq!(worst.allowed, 1.5);

        // Same sustained lobes staying at or below the halved 1.5 g limit pass.
        let mut within_halved = Vec::new();
        for step in 0..42 {
            let lat = 0.6 + 0.8 * (step as f64 / 41.0);
            within_halved.push([1.0, lat, 0.0]);
        }
        within_halved.push([1.0, -1.4, 0.0]);
        within_halved.extend(vec![[1.0, -0.8, 0.0]; 45]);
        assert_eq!(stream(&within_halved).g_seconds, 0.0);
    }

    #[test]
    fn envelope_tracker_throughput_stays_fast() {
        let mut tracker = EnvelopeTracker::default();
        for index in 0..500_000 {
            let lateral = if index % 5000 < 2500 { 0.2 } else { 1.0 };
            tracker.record([1.2, lateral, 0.1], TIME_STEP, true, location(0.0));
        }
        assert!(tracker.finish().g_seconds.is_finite());
    }
}
