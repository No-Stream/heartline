use crate::state_hash::StateHasher;
use crate::track::proximity::{ProximityEvent, ProximityKind};
use crate::track::{Heartlining, Inversion, PieceId, PieceKind, Shape};
use std::collections::BTreeSet;

use super::TestSample;
use super::comfort::{EnvelopeExceedance, evaluate_envelope};
use super::testing::RideStatistics;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PieceSummary {
    pub(super) id: PieceId,
    pub(super) name: &'static str,
    pub(super) kind: PieceKind,
    pub(super) shape: Shape,
    pub(super) inversion: Option<Inversion>,
    pub(super) heartlining: Option<Heartlining>,
    pub(super) length: f64,
    pub(super) turn_angle_radians: f64,
}

impl PieceSummary {
    pub(super) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_u8(self.id as u8);
        hash.write_bytes(self.name.as_bytes());
        hash.write_u8(self.kind as u8);
        self.shape.hash_into(hash);
        hash.write_option(self.inversion, |hash, inversion| {
            hash.write_u8(inversion as u8);
        });
        hash.write_option(self.heartlining, |hash, heartlining| {
            hash.write_u8(heartlining as u8);
        });
        hash.write_f64(self.length);
        hash.write_f64(self.turn_angle_radians);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SampleInterval {
    pub(super) distance_m: f64,
    pub(super) seconds: f64,
}

pub(super) fn sample_interval(
    samples: &[TestSample],
    sample_index: usize,
    circuit_length: f64,
    completed: bool,
) -> SampleInterval {
    let sample = &samples[sample_index];
    let next = samples.get(sample_index + 1).unwrap_or(&samples[0]);
    let distance_m = if sample_index + 1 == samples.len() && !completed {
        0.0
    } else if next.distance > sample.distance {
        next.distance - sample.distance
    } else if sample_index + 1 == samples.len() {
        (circuit_length - sample.distance + next.distance).max(0.0)
    } else {
        1.0
    };
    let mean_speed = (sample.speed + next.speed) / 2.0;
    let seconds = if mean_speed > 0.0 {
        distance_m / mean_speed
    } else {
        0.0
    };
    SampleInterval {
        distance_m,
        seconds,
    }
}

pub(super) fn is_active_sample(sample: &TestSample, pieces: &[PieceSummary]) -> bool {
    sample.speed > 1.0
        && matches!(
            pieces[sample.piece_index].kind,
            PieceKind::Track | PieceKind::Launch | PieceKind::Brakes
        )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Location {
    pub piece_index: usize,
    pub piece_name: &'static str,
    pub distance_m: f64,
    pub seconds_from_start: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ElementClass {
    EjectorHill,
    FloaterHill,
    BigDrop,
    VerticalLoop,
    Corkscrew,
    HeartlineRoll,
    HalfLoop,
    HalfCorkscrew,
    TopHat,
    Overbank,
    BarrelDrop,
    Helix,
    BankedTurn,
    Launch,
    Tunnel,
    NearMiss,
    TerrainHugging,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProximityCounts {
    pub(super) near_miss: usize,
    pub(super) headchopper: usize,
    pub(super) dueling: usize,
    pub(super) tunnel: usize,
    pub(super) terrain_hugging: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RideFeatures {
    pub(super) statistics: RideStatistics,
    pub(super) ride_seconds: f64,
    pub(super) active_average_speed: f64,
    pub(super) inversion_passages: Vec<(Inversion, Heartlining)>,
    pub(super) element_classes: BTreeSet<ElementClass>,
    pub(super) dead_excess_seconds: f64,
    pub(super) longest_dead_run: Option<(Location, f64)>,
    pub(super) highlight_gap_seconds: f64,
    pub(super) snap_count: usize,
    pub(super) max_roll_rate_deg_s: f64,
    pub(super) roll_rate_seconds: f64,
    pub(super) reversal_count: usize,
    pub(super) proximity_strength: f64,
    pub(super) proximity_counts: ProximityCounts,
    pub(super) theming: f64,
    pub(super) envelope: EnvelopeExceedance,
}

#[derive(Clone, Copy, Debug)]
struct SampleFrame {
    index: usize,
    interval: SampleInterval,
    active: bool,
    elapsed_seconds: f64,
    active_seconds: f64,
    location: Location,
}

#[derive(Clone, Debug)]
struct DeadRun {
    end_index: usize,
    location: Location,
    seconds: f64,
}

/// Extracts ride-level features from one car's sampled run.
pub(super) fn ride_features(
    samples: &[TestSample],
    pieces: &[PieceSummary],
    proximity_events: &[ProximityEvent],
    circuit_length: f64,
    completed: bool,
) -> RideFeatures {
    let statistics = super::testing::ride_statistics(samples, circuit_length, completed);
    let frames = sample_frames(samples, pieces, circuit_length, completed);
    let ride_seconds: f64 = frames
        .iter()
        .filter(|frame| frame.active)
        .map(|frame| frame.interval.seconds)
        .sum();
    let active_distance: f64 = frames
        .iter()
        .filter(|frame| frame.active)
        .map(|frame| frame.interval.distance_m)
        .sum();
    let active_average_speed = if ride_seconds > 0.0 {
        active_distance / ride_seconds
    } else {
        0.0
    };
    let inversion_passages = inversion_passages(samples, pieces, completed);
    let mut element_classes = element_classes(samples, pieces, proximity_events, &statistics);
    if contains_helix(samples, pieces, &frames) {
        element_classes.insert(ElementClass::Helix);
    }
    let dead_runs = dead_runs(samples, &frames);
    let longest_dead_run = dead_runs
        .iter()
        .max_by(|left, right| left.seconds.total_cmp(&right.seconds))
        .map(|run| (run.location, run.seconds));
    let final_brake_run = final_dead_run_before_station(&dead_runs, &frames, pieces, completed);
    let dead_excess_seconds = dead_runs
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != final_brake_run)
        .map(|(_, run)| (run.seconds - 3.0).max(0.0))
        .sum();
    let highlight_gap_seconds = highlights(
        samples,
        pieces,
        proximity_events,
        &frames,
        completed,
        circuit_length,
        ride_seconds,
    );
    let (snap_count, max_roll_rate_deg_s, roll_rate_seconds) = whip_features(samples, &frames);
    let reversal_count = reversal_count(samples, &frames, completed);
    let (proximity_strength, proximity_counts) = proximity_features(proximity_events);
    let envelope = evaluate_envelope(samples, pieces, circuit_length, completed);

    RideFeatures {
        statistics,
        ride_seconds,
        active_average_speed,
        inversion_passages,
        element_classes,
        dead_excess_seconds,
        longest_dead_run,
        highlight_gap_seconds,
        snap_count,
        max_roll_rate_deg_s,
        roll_rate_seconds,
        reversal_count,
        proximity_strength,
        proximity_counts,
        theming: 0.0,
        envelope,
    }
}

fn sample_frames(
    samples: &[TestSample],
    pieces: &[PieceSummary],
    circuit_length: f64,
    completed: bool,
) -> Vec<SampleFrame> {
    let mut elapsed_seconds = 0.0;
    let mut active_seconds = 0.0;
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let interval = sample_interval(samples, index, circuit_length, completed);
            let location = Location {
                piece_index: sample.piece_index,
                piece_name: pieces[sample.piece_index].name,
                distance_m: sample.distance,
                seconds_from_start: elapsed_seconds,
            };
            let active = is_active_sample(sample, pieces) && interval.seconds > 0.0;
            let frame = SampleFrame {
                index,
                interval,
                active,
                elapsed_seconds,
                active_seconds,
                location,
            };
            elapsed_seconds += interval.seconds;
            if active {
                active_seconds += interval.seconds;
            }
            frame
        })
        .collect()
}

fn inversion_passages(
    samples: &[TestSample],
    pieces: &[PieceSummary],
    completed: bool,
) -> Vec<(Inversion, Heartlining)> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut previous_inverted = if completed {
        samples[samples.len() - 1].inverted
    } else {
        samples[0].inverted
    };
    let first_index = usize::from(!completed);
    let mut passages = Vec::new();
    for sample in samples
        .iter()
        .enumerate()
        .skip(first_index)
        .map(|(_, sample)| sample)
    {
        if sample.inverted && !previous_inverted {
            let piece = &pieces[sample.piece_index];
            if let Some(inversion) = piece.inversion {
                passages.push((
                    inversion,
                    piece.heartlining.unwrap_or(Heartlining::NotHeartlined),
                ));
            }
        }
        previous_inverted = sample.inverted;
    }
    passages
}

fn element_classes(
    samples: &[TestSample],
    pieces: &[PieceSummary],
    proximity_events: &[ProximityEvent],
    statistics: &RideStatistics,
) -> BTreeSet<ElementClass> {
    let mut classes = BTreeSet::new();
    let mut visited_pieces = BTreeSet::new();
    for sample in samples {
        if sample.speed <= 1.0
            || !matches!(
                pieces[sample.piece_index].kind,
                PieceKind::Track | PieceKind::Launch | PieceKind::Brakes
            )
        {
            continue;
        }
        visited_pieces.insert(sample.piece_index);
        if sample.vertical_g < -0.5 {
            classes.insert(ElementClass::EjectorHill);
        } else if sample.vertical_g <= 0.2 && sample.vertical_g >= -0.5 {
            classes.insert(ElementClass::FloaterHill);
        }
        if pieces[sample.piece_index].kind == PieceKind::Launch {
            classes.insert(ElementClass::Launch);
        }
    }
    for piece_index in visited_pieces {
        let piece = &pieces[piece_index];
        match piece.id {
            PieceId::Loop
            | PieceId::LoopRight
            | PieceId::SteelLoop
            | PieceId::SteelLoopRight
            | PieceId::LargeLoop
            | PieceId::LargeLoopRight => {
                classes.insert(ElementClass::VerticalLoop);
            }
            PieceId::Corkscrew | PieceId::CorkscrewRight => {
                classes.insert(ElementClass::Corkscrew);
            }
            PieceId::HeartlineRoll | PieceId::HeartlineRollRight => {
                classes.insert(ElementClass::HeartlineRoll);
            }
            PieceId::HalfLoopUp | PieceId::HalfLoopDown => {
                classes.insert(ElementClass::HalfLoop);
            }
            PieceId::HalfCorkscrewLeftUp
            | PieceId::HalfCorkscrewRightUp
            | PieceId::HalfCorkscrewLeftDown
            | PieceId::HalfCorkscrewRightDown => {
                classes.insert(ElementClass::HalfCorkscrew);
            }
            PieceId::BarrelDropLeft | PieceId::BarrelDropRight => {
                classes.insert(ElementClass::BarrelDrop);
            }
            PieceId::BankedTurn
            | PieceId::BankedTurnEntry
            | PieceId::BankedTurnExit
            | PieceId::BankedTurnStandalone
            | PieceId::LargeLeftBankedTurnStandalone
            | PieceId::SmallRightBankedTurnStandalone
            | PieceId::LargeRightBankedTurnStandalone
            | PieceId::LargeLeftBankedTurnEntry
            | PieceId::LargeLeftBankedTurn
            | PieceId::LargeLeftBankedTurnExit
            | PieceId::SmallRightBankedTurnEntry
            | PieceId::SmallRightBankedTurn
            | PieceId::SmallRightBankedTurnExit
            | PieceId::LargeRightBankedTurnEntry
            | PieceId::LargeRightBankedTurn
            | PieceId::LargeRightBankedTurnExit => {
                classes.insert(ElementClass::BankedTurn);
            }
            _ => {}
        }
        if piece.id == PieceId::TopHat || piece.shape == Shape::TopHat {
            classes.insert(ElementClass::TopHat);
        }
        if piece.shape == Shape::Overbank
            || matches!(
                piece.id,
                PieceId::HybridOverbankLeft | PieceId::HybridOverbankRight
            )
        {
            classes.insert(ElementClass::Overbank);
        }
    }
    if statistics.biggest_drop >= 20.0 {
        classes.insert(ElementClass::BigDrop);
    }
    for event in proximity_events {
        match event.kind {
            ProximityKind::NearMiss | ProximityKind::Headchopper | ProximityKind::Dueling => {
                classes.insert(ElementClass::NearMiss);
            }
            ProximityKind::Tunnel => {
                classes.insert(ElementClass::Tunnel);
            }
            ProximityKind::TerrainHugging => {
                classes.insert(ElementClass::TerrainHugging);
            }
        }
    }
    classes
}

fn contains_helix(samples: &[TestSample], pieces: &[PieceSummary], frames: &[SampleFrame]) -> bool {
    let mut active_distance_by_piece = vec![0.0; pieces.len()];
    let mut active_piece_order = Vec::new();
    for frame in frames {
        if !frame.active {
            active_piece_order.push(None);
            continue;
        }
        let piece_index = samples[frame.index].piece_index;
        active_distance_by_piece[piece_index] += frame.interval.distance_m;
        if active_piece_order.last().copied() != Some(Some(piece_index)) {
            active_piece_order.push(Some(piece_index));
        }
    }
    let mut signed_turn = 0.0;
    let mut direction = 0_i8;
    for piece_index in active_piece_order {
        let Some(piece_index) = piece_index else {
            signed_turn = 0.0;
            direction = 0;
            continue;
        };
        let piece = &pieces[piece_index];
        let fraction = (active_distance_by_piece[piece_index] / piece.length).min(1.0);
        let turn = piece.turn_angle_radians * fraction;
        let turn_direction = if turn > 0.0 {
            1
        } else if turn < 0.0 {
            -1
        } else {
            0
        };
        if turn_direction == 0 {
            signed_turn = 0.0;
            direction = 0;
            continue;
        }
        if direction != turn_direction {
            signed_turn = 0.0;
        }
        signed_turn += turn.abs();
        direction = turn_direction;
        if signed_turn >= 270.0_f64.to_radians() {
            return true;
        }
    }
    false
}

fn is_eventful(sample: &TestSample) -> bool {
    (sample.vertical_g - 1.0).abs() >= 0.5
        || sample.lateral_g.abs() >= 0.5
        || sample.longitudinal_g.abs() >= 0.4
        || sample.speed >= 20.0
        || sample.inverted
}

fn dead_runs(samples: &[TestSample], frames: &[SampleFrame]) -> Vec<DeadRun> {
    let mut runs = Vec::new();
    let mut current: Option<DeadRun> = None;
    for frame in frames {
        if frame.active && !is_eventful(&samples[frame.index]) {
            let run = current.get_or_insert(DeadRun {
                end_index: frame.index,
                location: frame.location,
                seconds: 0.0,
            });
            run.end_index = frame.index;
            run.seconds += frame.interval.seconds;
        } else if let Some(run) = current.take() {
            runs.push(run);
        }
    }
    if let Some(run) = current {
        runs.push(run);
    }
    runs
}

fn final_dead_run_before_station(
    runs: &[DeadRun],
    frames: &[SampleFrame],
    pieces: &[PieceSummary],
    completed: bool,
) -> Option<usize> {
    let station_index = frames
        .iter()
        .find(|frame| pieces[frame.location.piece_index].kind == PieceKind::Station)?
        .index;
    if completed {
        runs.iter()
            .enumerate()
            .map(|(index, run)| {
                (
                    index,
                    (station_index + frames.len() - run.end_index) % frames.len(),
                )
            })
            .filter(|(_, distance)| *distance > 0)
            .min_by_key(|(_, distance)| *distance)
            .map(|(index, _)| index)
    } else {
        runs.iter()
            .enumerate()
            .filter(|(_, run)| run.end_index < station_index)
            .max_by_key(|(_, run)| run.end_index)
            .map(|(index, _)| index)
    }
}

fn highlights(
    samples: &[TestSample],
    pieces: &[PieceSummary],
    proximity_events: &[ProximityEvent],
    frames: &[SampleFrame],
    completed: bool,
    circuit_length: f64,
    ride_seconds: f64,
) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    let mut highlight_times = Vec::new();
    let mut highest_height = samples[0].height_m;
    if completed && samples.len() > 1 && samples[samples.len() - 1].height_m > highest_height {
        highest_height = samples[samples.len() - 1].height_m;
    }
    let mut previous_airtime =
        completed && samples.last().is_some_and(|sample| sample.vertical_g < 0.0);
    let mut previous_inverted = completed && samples.last().is_some_and(|sample| sample.inverted);
    for frame in frames {
        if !frame.active {
            if samples[frame.index].height_m > highest_height {
                highest_height = samples[frame.index].height_m;
            }
            continue;
        }
        let sample = &samples[frame.index];
        let airtime = sample.vertical_g < 0.0;
        let inversion_entry = sample.inverted && !previous_inverted;
        let force_highlight = sample.vertical_g > 3.5 || sample.lateral_g.abs() > 1.0;
        let launch = pieces[sample.piece_index].kind == PieceKind::Launch;
        if airtime && !previous_airtime || inversion_entry || force_highlight || launch {
            highlight_times.push(frame.active_seconds);
        }
        previous_airtime = airtime;
        previous_inverted = sample.inverted;
        if highest_height - sample.height_m >= 20.0 {
            highlight_times.push(frame.active_seconds);
            highest_height = sample.height_m;
        } else if sample.height_m > highest_height {
            highest_height = sample.height_m;
        }
    }
    for event in proximity_events {
        if let Some(frame) = frames
            .iter()
            .filter(|frame| frame.active)
            .min_by(|left, right| {
                circular_distance(samples[left.index].distance, event.distance, circuit_length)
                    .total_cmp(&circular_distance(
                        samples[right.index].distance,
                        event.distance,
                        circuit_length,
                    ))
            })
        {
            highlight_times.push(frame.active_seconds);
        }
    }
    highlight_times.sort_by(f64::total_cmp);
    highlight_times.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    let mut times = Vec::with_capacity(highlight_times.len() + 2);
    times.push(0.0);
    times.extend(highlight_times);
    times.push(ride_seconds);
    times
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max)
}

fn circular_distance(distance: f64, target: f64, circuit_length: f64) -> f64 {
    let difference = (distance - target).abs();
    if circuit_length > 0.0 {
        difference.min((circuit_length - difference).abs())
    } else {
        difference
    }
}

fn whip_features(samples: &[TestSample], frames: &[SampleFrame]) -> (usize, f64, f64) {
    let mut snap_count = 0;
    let mut direction = 0_i8;
    let mut segment_counted = false;
    let mut previous_active: Option<SampleFrame> = None;
    for current in frames {
        if !current.active {
            direction = 0;
            segment_counted = false;
            previous_active = None;
            continue;
        }
        let Some(previous) = previous_active else {
            previous_active = Some(*current);
            continue;
        };
        let delta = samples[current.index].vertical_g - samples[previous.index].vertical_g;
        let next_direction = if delta > 0.0 {
            1
        } else if delta < 0.0 {
            -1
        } else {
            0
        };
        if next_direction != 0 && direction != 0 && next_direction != direction {
            segment_counted = false;
        }
        if !segment_counted {
            let fast_change = frames
                .iter()
                .take(current.index)
                .rev()
                .take_while(|frame| {
                    frame.active && current.elapsed_seconds - frame.elapsed_seconds <= 0.4
                })
                .any(|frame| {
                    (samples[current.index].vertical_g - samples[frame.index].vertical_g).abs()
                        >= 2.0
                });
            if fast_change {
                snap_count += 1;
                segment_counted = true;
            }
        }
        if next_direction != 0 {
            direction = next_direction;
        }
        previous_active = Some(*current);
    }
    let mut max_roll_rate_deg_s: f64 = 0.0;
    let mut roll_rate_seconds = 0.0;
    for pair in frames.windows(2) {
        let [first, second] = pair else {
            unreachable!();
        };
        if !first.active
            || !second.active
            || samples[first.index].piece_index != samples[second.index].piece_index
        {
            continue;
        }
        let seconds = second.elapsed_seconds - first.elapsed_seconds;
        if seconds <= 0.0 {
            continue;
        }
        let roll_rate = (samples[second.index].roll - samples[first.index].roll)
            .abs()
            .to_degrees()
            / seconds;
        if roll_rate > 90.0 {
            roll_rate_seconds += first.interval.seconds;
        }
    }
    for (end_index, end) in frames.iter().enumerate().filter(|(_, frame)| frame.active) {
        let mut start_index = end_index;
        while start_index > 0 {
            let candidate = &frames[start_index - 1];
            if !candidate.active
                || samples[candidate.index].piece_index != samples[end.index].piece_index
                || end.elapsed_seconds - candidate.elapsed_seconds > 0.5
            {
                break;
            }
            start_index -= 1;
        }
        let start = &frames[start_index];
        let window_seconds = end.elapsed_seconds - start.elapsed_seconds;
        if window_seconds > 0.0 {
            max_roll_rate_deg_s = max_roll_rate_deg_s.max(
                (samples[end.index].roll - samples[start.index].roll)
                    .abs()
                    .to_degrees()
                    / window_seconds,
            );
        }
    }
    (snap_count, max_roll_rate_deg_s, roll_rate_seconds)
}

fn reversal_count(samples: &[TestSample], frames: &[SampleFrame], completed: bool) -> usize {
    fn count_axis(
        samples: &[TestSample],
        frames: &[SampleFrame],
        value: impl Fn(&TestSample) -> f64,
        completed: bool,
    ) -> usize {
        let mut previous_sign = 0_i8;
        let mut previous_value = 0.0;
        let mut previous_time = 0.0;
        let mut count = 0;
        for frame in frames.iter().filter(|frame| frame.active) {
            let current_value = value(&samples[frame.index]);
            let current_sign = if current_value > 0.0 {
                1
            } else if current_value < 0.0 {
                -1
            } else {
                0
            };
            if current_sign != 0 {
                if previous_sign != 0
                    && current_sign != previous_sign
                    && frame.elapsed_seconds - previous_time <= 0.3
                    && (current_value - previous_value).abs() >= 1.5
                {
                    count += 1;
                }
                if current_sign != previous_sign || current_value.abs() > previous_value.abs() {
                    previous_value = current_value;
                    previous_time = frame.elapsed_seconds;
                }
                previous_sign = current_sign;
            }
        }
        if completed && frames.len() > 1 && frames[0].active && frames[frames.len() - 1].active {
            let first = &frames[0];
            let last = &frames[frames.len() - 1];
            let first_value = value(&samples[first.index]);
            let last_value = value(&samples[last.index]);
            if first_value.signum() != 0.0
                && last_value.signum() != 0.0
                && first_value.signum() != last_value.signum()
                && last.interval.seconds <= 0.3
                && (first_value - last_value).abs() >= 1.5
            {
                count += 1;
            }
        }
        count
    }
    count_axis(samples, frames, |sample| sample.vertical_g, completed)
        + count_axis(samples, frames, |sample| sample.lateral_g, completed)
}

fn proximity_features(events: &[ProximityEvent]) -> (f64, ProximityCounts) {
    let mut counts = ProximityCounts::default();
    let mut strength = 0.0;
    for event in events {
        strength += event.strength.clamp(0.0, 1.0);
        match event.kind {
            ProximityKind::NearMiss => counts.near_miss += 1,
            ProximityKind::Headchopper => counts.headchopper += 1,
            ProximityKind::Dueling => counts.dueling += 1,
            ProximityKind::Tunnel => counts.tunnel += 1,
            ProximityKind::TerrainHugging => counts.terrain_hugging += 1,
        }
    }
    (strength, counts)
}

#[cfg(test)]
mod tests {
    use crate::physics::TestSample;
    use crate::track::proximity::{ProximityEvent, ProximityKind};
    use crate::track::{Heartlining, Inversion, PieceId, PieceKind, Shape, Vec3};

    use super::{ElementClass, PieceSummary, is_active_sample, ride_features, sample_interval};

    #[test]
    fn ride_time_and_average_speed_only_include_active_track() {
        let pieces = [
            piece(PieceId::Station, PieceKind::Station),
            piece(PieceId::Straight, PieceKind::Track),
            piece(PieceId::GentleUp, PieceKind::Lift),
            piece(PieceId::Launch, PieceKind::Launch),
            piece(PieceId::Brakes, PieceKind::Brakes),
            piece(PieceId::Station, PieceKind::Station),
        ];
        let samples = [
            TestSample {
                piece_index: 0,
                ..sample(0, 10.0)
            },
            TestSample {
                piece_index: 1,
                ..sample(1, 10.0)
            },
            TestSample {
                piece_index: 2,
                ..sample(2, 10.0)
            },
            TestSample {
                piece_index: 3,
                ..sample(3, 10.0)
            },
            TestSample {
                piece_index: 4,
                ..sample(4, 10.0)
            },
            TestSample {
                piece_index: 5,
                ..sample(5, 10.0)
            },
        ];

        let features = ride_features(&samples, &pieces, &[], 6.0, true);

        assert!((features.ride_seconds - 0.3).abs() < 1e-12);
        assert!((features.active_average_speed - 10.0).abs() < 1e-12);
        assert!((features.statistics.duration_seconds - 0.6).abs() < 1e-12);
    }

    #[test]
    fn inversion_passages_keep_order_and_repeated_elements() {
        let pieces = [
            piece(PieceId::Station, PieceKind::Station),
            inversion_piece(PieceId::Loop, Inversion::VerticalLoop, None),
            inversion_piece(
                PieceId::Corkscrew,
                Inversion::Corkscrew,
                Some(Heartlining::Heartlined),
            ),
            inversion_piece(
                PieceId::HeartlineRoll,
                Inversion::HeartlineRoll,
                Some(Heartlining::Heartlined),
            ),
        ];
        let samples = [
            TestSample {
                piece_index: 0,
                ..sample(0, 10.0)
            },
            TestSample {
                piece_index: 1,
                ..inverted_sample(1, 10.0, true)
            },
            TestSample {
                piece_index: 1,
                ..sample(2, 10.0)
            },
            TestSample {
                piece_index: 2,
                ..inverted_sample(3, 10.0, true)
            },
            TestSample {
                piece_index: 2,
                ..sample(4, 10.0)
            },
            TestSample {
                piece_index: 3,
                ..inverted_sample(5, 10.0, true)
            },
            TestSample {
                piece_index: 0,
                ..sample(6, 10.0)
            },
        ];

        let features = ride_features(&samples, &pieces, &[], 7.0, true);

        assert_eq!(
            features.inversion_passages,
            [
                (Inversion::VerticalLoop, Heartlining::NotHeartlined),
                (Inversion::Corkscrew, Heartlining::Heartlined),
                (Inversion::HeartlineRoll, Heartlining::Heartlined),
            ]
        );
        assert!(
            features
                .element_classes
                .contains(&ElementClass::VerticalLoop)
        );
        assert!(features.element_classes.contains(&ElementClass::Corkscrew));
        assert!(
            features
                .element_classes
                .contains(&ElementClass::HeartlineRoll)
        );
    }

    #[test]
    fn element_classes_cover_piece_geometry_airtime_and_proximity() {
        let mut pieces = vec![
            piece(PieceId::Station, PieceKind::Station),
            piece(PieceId::AirtimeHill, PieceKind::Track),
            piece(PieceId::HalfLoopUp, PieceKind::Track),
            piece(PieceId::HalfCorkscrewRightDown, PieceKind::Track),
            piece(PieceId::TwisterHelixLeft, PieceKind::Track),
            piece(PieceId::BankedTurnStandalone, PieceKind::Track),
            piece(PieceId::BarrelDropLeft, PieceKind::Track),
            piece(PieceId::TopHat, PieceKind::Track),
            piece(PieceId::HybridOverbankRight, PieceKind::Track),
            piece(PieceId::Launch, PieceKind::Launch),
        ];
        pieces[7].shape = Shape::TopHat;
        pieces[8].shape = Shape::Overbank;
        let mut samples: Vec<_> = (0..11)
            .map(|index| TestSample {
                piece_index: index.min(pieces.len() - 1),
                ..sample(index, 10.0)
            })
            .collect();
        samples[1].vertical_g = -0.3;
        samples[2].vertical_g = -1.2;
        let events = [
            proximity_event(1, ProximityKind::NearMiss, 0.25),
            proximity_event(2, ProximityKind::Headchopper, 1.5),
            proximity_event(3, ProximityKind::Dueling, 0.5),
            proximity_event(4, ProximityKind::Tunnel, 0.3),
            proximity_event(5, ProximityKind::TerrainHugging, 0.2),
        ];

        let features = ride_features(&samples, &pieces, &events, 11.0, true);

        for class in [
            ElementClass::FloaterHill,
            ElementClass::EjectorHill,
            ElementClass::HalfLoop,
            ElementClass::HalfCorkscrew,
            ElementClass::BankedTurn,
            ElementClass::BarrelDrop,
            ElementClass::TopHat,
            ElementClass::Overbank,
            ElementClass::Launch,
            ElementClass::NearMiss,
            ElementClass::Tunnel,
            ElementClass::TerrainHugging,
        ] {
            assert!(
                features.element_classes.contains(&class),
                "missing {class:?}"
            );
        }
        assert!((features.proximity_strength - 2.25).abs() < 1e-12);
        assert_eq!(features.proximity_counts.near_miss, 1);
        assert_eq!(features.proximity_counts.headchopper, 1);
        assert_eq!(features.proximity_counts.dueling, 1);
        assert_eq!(features.proximity_counts.tunnel, 1);
        assert_eq!(features.proximity_counts.terrain_hugging, 1);
    }

    #[test]
    fn helix_class_requires_270_degrees_of_continuous_turn_at_speed() {
        let mut pieces = [
            piece(PieceId::BankedTurnStandalone, PieceKind::Track),
            piece(PieceId::BankedTurnStandalone, PieceKind::Track),
            piece(PieceId::BankedTurnStandalone, PieceKind::Track),
        ];
        for piece in &mut pieces {
            piece.turn_angle_radians = std::f64::consts::FRAC_PI_2;
        }
        let samples = [
            TestSample {
                piece_index: 0,
                ..sample(0, 10.0)
            },
            TestSample {
                piece_index: 1,
                ..sample(1, 10.0)
            },
            TestSample {
                piece_index: 2,
                ..sample(2, 10.0)
            },
        ];
        let full_turn = ride_features(&samples, &pieces, &[], 3.0, true);
        assert!(full_turn.element_classes.contains(&ElementClass::Helix));

        pieces[2].turn_angle_radians = std::f64::consts::FRAC_PI_4;
        let shorter_turn = ride_features(&samples, &pieces, &[], 3.0, true);
        assert!(!shorter_turn.element_classes.contains(&ElementClass::Helix));
    }

    #[test]
    fn pacing_excludes_the_final_dead_run_into_the_station() {
        let pieces = [
            piece(PieceId::Straight, PieceKind::Track),
            piece(PieceId::Brakes, PieceKind::Brakes),
            piece(PieceId::Station, PieceKind::Station),
        ];
        let mut samples: Vec<_> = (0..52).map(|index| sample(index, 10.0)).collect();
        samples[0].speed = 20.0;
        samples[50].piece_index = 1;
        samples[51].piece_index = 2;

        let features = ride_features(&samples, &pieces, &[], 52.0, true);

        assert_eq!(features.dead_excess_seconds, 0.0);
        let (location, seconds) = features.longest_dead_run.unwrap();
        assert_eq!(location.piece_index, 0);
        assert!(seconds > 4.5);
    }

    #[test]
    fn whip_features_count_fast_force_changes_roll_rate_and_reversals() {
        let pieces = [piece(PieceId::Straight, PieceKind::Track)];
        let mut samples: Vec<_> = (0..8).map(|index| sample(index, 10.0)).collect();
        samples[0].vertical_g = 3.0;
        samples[1].vertical_g = 3.0;
        samples[2].vertical_g = -1.0;
        samples[3].vertical_g = -1.0;
        samples[4].lateral_g = 2.0;
        samples[5].lateral_g = -2.0;
        for sample in &mut samples[4..] {
            sample.vertical_g = -1.0;
        }
        for sample in &mut samples[5..] {
            sample.roll = 0.7;
        }

        let features = ride_features(&samples, &pieces, &[], 8.0, false);

        assert_eq!(features.snap_count, 1);
        assert_eq!(features.reversal_count, 2);
        assert!((70.0..90.0).contains(&features.max_roll_rate_deg_s));
        assert!(features.roll_rate_seconds > 0.0);
    }

    #[test]
    fn long_dead_run_reports_its_start_and_excess_time() {
        let pieces = [piece(PieceId::Straight, PieceKind::Track)];
        let mut samples: Vec<_> = (0..60).map(|index| sample(index, 10.0)).collect();
        samples[0].speed = 20.0;
        samples[50].vertical_g = -0.6;
        samples[51].vertical_g = 1.0;

        let features = ride_features(&samples, &pieces, &[], 60.0, true);

        assert!(features.dead_excess_seconds > 1.5);
        let (location, seconds) = features.longest_dead_run.unwrap();
        assert_eq!(location.distance_m, 1.0);
        assert_eq!(location.piece_name, "Straight");
        assert!(seconds >= 4.0);
        assert!(features.highlight_gap_seconds > 2.0);
    }

    #[test]
    fn active_samples_exclude_lifts_stations_and_slow_track() {
        let pieces = [
            piece(PieceId::Straight, PieceKind::Track),
            piece(PieceId::GentleUp, PieceKind::Lift),
            piece(PieceId::Launch, PieceKind::Launch),
            piece(PieceId::Brakes, PieceKind::Brakes),
            piece(PieceId::Station, PieceKind::Station),
        ];
        let sample = |piece_index, speed| TestSample {
            piece_index,
            speed,
            ..sample_at(0.0, 0.0)
        };

        assert!(is_active_sample(&sample(0, 1.1), &pieces));
        assert!(is_active_sample(&sample(2, 2.0), &pieces));
        assert!(is_active_sample(&sample(3, 2.0), &pieces));
        assert!(!is_active_sample(&sample(0, 1.0), &pieces));
        assert!(!is_active_sample(&sample(1, 2.0), &pieces));
        assert!(!is_active_sample(&sample(4, 2.0), &pieces));
    }

    #[test]
    fn sample_intervals_preserve_complete_and_incomplete_end_rules() {
        let samples = [
            sample_at(0.0, 10.0),
            sample_at(1.0, 20.0),
            sample_at(2.0, 30.0),
        ];

        assert_eq!(sample_interval(&samples, 0, 10.0, true).distance_m, 1.0);
        assert!((sample_interval(&samples, 0, 10.0, true).seconds - 1.0 / 15.0).abs() < 1e-12);
        assert_eq!(sample_interval(&samples, 2, 3.0, true).distance_m, 1.0);
        assert!((sample_interval(&samples, 2, 3.0, true).seconds - 1.0 / 20.0).abs() < 1e-12);
        assert_eq!(sample_interval(&samples, 2, 10.0, false).distance_m, 0.0);
        assert_eq!(sample_interval(&samples, 2, 10.0, false).seconds, 0.0);
    }

    fn piece(id: PieceId, kind: PieceKind) -> PieceSummary {
        PieceSummary {
            id,
            name: if id == PieceId::Straight {
                "Straight"
            } else {
                "test piece"
            },
            kind,
            shape: crate::track::Shape::Plain,
            inversion: None,
            heartlining: None,
            length: 1.0,
            turn_angle_radians: 0.0,
        }
    }

    fn sample_at(distance: f64, speed: f64) -> TestSample {
        TestSample {
            distance,
            speed,
            vertical_g: 1.0,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            airtime: false,
            height_m: 0.0,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        }
    }

    fn sample(distance: usize, speed: f64) -> TestSample {
        TestSample {
            piece_index: 0,
            ..sample_at(distance as f64, speed)
        }
    }

    fn inverted_sample(distance: usize, speed: f64, inverted: bool) -> TestSample {
        TestSample {
            inverted,
            ..sample(distance, speed)
        }
    }

    fn inversion_piece(
        id: PieceId,
        inversion: Inversion,
        heartlining: Option<Heartlining>,
    ) -> PieceSummary {
        PieceSummary {
            inversion: Some(inversion),
            heartlining,
            ..piece(id, PieceKind::Track)
        }
    }

    fn proximity_event(piece_index: usize, kind: ProximityKind, strength: f64) -> ProximityEvent {
        ProximityEvent {
            piece_index,
            kind,
            position: Vec3::new(0.0, 0.0, 0.0),
            distance: piece_index as f64,
            strength,
        }
    }
}
