use super::comfort::{EnvelopeExceedance, EnvelopeTracker};
use super::features::{PieceSummary, sample_interval};
use super::scoring::{ForceAxis, ForceBand, ForceBandDurations, ScoreBreakdown, force_band, score};
use super::{CarReading, Circuit};
use crate::state_hash::StateHasher;
use crate::track::proximity::{ProximityEvent, TrainPosition, detect_dueling};
use crate::track::{ComfortableForceLimits, Piece};

// Report the requested ejector and floater bands in rider-frame vertical g.
const EJECTOR_AIRTIME_MAX_VERTICAL_G: f64 = -1.0;
const FLOATER_AIRTIME_MAX_VERTICAL_G: f64 = 0.0;
// 3.5 g is a published peak example for a modern coaster, used as a reporting threshold, not a limit.
// https://time.com/2906000/roller-coaster-goliath-science/
const STRONG_POSITIVE_G_THRESHOLD: f64 = 3.5;
// 1.0 g is a rounded reporting threshold above the 0.7 g sustained lateral case discussed by Rohde.
// It is not a design or safety limit.
// https://www.vdv-freizeittechnologie.de/files/bilder-vdv/pdf/Rohde-Accelerationlimits-2nd-edition-2024.pdf
const STRONG_LATERAL_G_THRESHOLD: f64 = 1.0;
/// One fixed-distance force and speed sample from a coaster test run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestSample {
    pub distance: f64,
    pub speed: f64,
    pub vertical_g: f64,
    pub lateral_g: f64,
    pub longitudinal_g: f64,
    pub airtime: bool,
    pub height_m: f64,
    pub inverted: bool,
    pub roll: f64,
    pub piece_index: usize,
}

/// A completed or interrupted test run, including advisory ride ratings.
#[derive(Clone, Debug, PartialEq)]
pub struct TestRun {
    pub samples: Vec<TestSample>,
    pub proximity_events: Vec<ProximityEvent>,
    pub excitement: f64,
    pub intensity: f64,
    pub nausea: f64,
    pub breakdown: ScoreBreakdown,
    pub statistics: RideStatistics,
    pub notes: Vec<String>,
    pub completed: bool,
    pub furthest_distance: f64,
    visited_ranges: Vec<(f64, f64)>,
    rating_inputs: Vec<RatingInput>,
    rating_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct RatingInput {
    samples: Vec<TestSample>,
    pieces: Vec<PieceSummary>,
    proximity_events: Vec<ProximityEvent>,
    theming: f64,
    visited_ranges: Vec<(f64, f64)>,
    circuit_length: f64,
    completed: bool,
    force_limits: ComfortableForceLimits,
    force_bands: ForceBandDurations,
    statistics: RideStatistics,
    envelope: EnvelopeExceedance,
}

impl RatingInput {
    fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_len(self.samples.len());
        for &sample in &self.samples {
            write_sample(hash, sample);
        }
        hash.write_len(self.pieces.len());
        for piece in &self.pieces {
            piece.hash_into(hash);
        }
        hash.write_len(self.proximity_events.len());
        for &event in &self.proximity_events {
            write_proximity_event(hash, event);
        }
        hash.write_f64(self.theming);
        write_ranges(hash, &self.visited_ranges);
        hash.write_f64(self.circuit_length);
        hash.write_bool(self.completed);
        for value in [
            self.force_limits.minimum_vertical_g,
            self.force_limits.maximum_vertical_g,
            self.force_limits.maximum_lateral_g,
            self.force_limits.maximum_longitudinal_g,
        ] {
            hash.write_f64(value);
        }
        hash_force_bands(hash, self.force_bands);
        write_ride_statistics(hash, self.statistics);
        hash.write_f64(self.envelope.g_seconds);
        hash.write_f64(self.envelope.seconds);
        hash.write_option(self.envelope.worst.as_ref(), |hash, worst| {
            write_location(hash, worst.location);
            hash.write_u8(worst.axis as u8);
            hash.write_f64(worst.value);
            hash.write_f64(worst.allowed);
            hash.write_f64(worst.ratio);
        });
        hash.write_len(self.envelope.stretches.len());
        for stretch in &self.envelope.stretches {
            write_location(hash, stretch.location);
            hash.write_f64(stretch.seconds);
        }
    }
}

fn hash_force_bands(hash: &mut StateHasher, bands: ForceBandDurations) {
    for value in [
        bands.airtime_good,
        bands.airtime_great,
        bands.vertical_good,
        bands.vertical_great,
        bands.lateral_strong,
        bands.longitudinal_strong,
        bands.too_much,
        bands.dangerous,
    ] {
        hash.write_f64(value);
    }
}

fn write_location(hash: &mut StateHasher, location: super::features::Location) {
    hash.write_len(location.piece_index);
    hash.write_bytes(location.piece_name.as_bytes());
    hash.write_f64(location.distance_m);
    hash.write_f64(location.seconds_from_start);
}

fn write_ride_statistics(hash: &mut StateHasher, stats: RideStatistics) {
    for value in [
        stats.ejector_airtime_seconds,
        stats.floater_airtime_seconds,
        stats.strong_positive_g_seconds,
        stats.longest_strong_positive_g_seconds,
        stats.strong_lateral_g_seconds,
        stats.max_positive_vertical_g,
        stats.max_negative_vertical_g,
        stats.max_lateral_g,
        stats.max_forward_g,
        stats.max_braking_g,
        stats.max_speed,
        stats.average_speed,
        stats.duration_seconds,
        stats.track_length,
        stats.max_height,
        stats.biggest_drop,
        stats.steepest_drop_angle_degrees,
        stats.inverted_seconds,
        stats.launch_peak_speed,
    ] {
        hash.write_f64(value);
    }
    for value in [
        stats.airtime_moments,
        stats.inversions,
        stats.drops,
        stats.launches,
    ] {
        hash.write_len(value);
    }
}

fn record_force_bands(
    durations: &mut ForceBandDurations,
    reading: CarReading,
    limits: ComfortableForceLimits,
    seconds: f64,
) {
    let vertical_axis = if reading.g.vertical < 0.3 {
        ForceAxis::Airtime
    } else {
        ForceAxis::Vertical
    };
    for (axis, value) in [
        (vertical_axis, reading.g.vertical),
        (ForceAxis::Lateral, reading.g.lateral),
        (ForceAxis::Longitudinal, reading.g.longitudinal),
    ] {
        match (axis, force_band(axis, value, limits)) {
            (ForceAxis::Airtime, ForceBand::Good) => durations.airtime_good += seconds,
            (ForceAxis::Airtime, ForceBand::Great) => durations.airtime_great += seconds,
            (ForceAxis::Vertical, ForceBand::Good) => durations.vertical_good += seconds,
            (ForceAxis::Vertical, ForceBand::Great) => durations.vertical_great += seconds,
            (ForceAxis::Lateral, ForceBand::Strong) => durations.lateral_strong += seconds,
            (ForceAxis::Longitudinal, ForceBand::Strong) => {
                durations.longitudinal_strong += seconds
            }
            (axis, ForceBand::TooMuch) => {
                durations.too_much += seconds;
                if axis == ForceAxis::Lateral {
                    durations.lateral_strong += seconds;
                }
            }
            (axis, ForceBand::Dangerous) => {
                durations.dangerous += seconds;
                if axis == ForceAxis::Lateral {
                    durations.lateral_strong += seconds;
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RideStatistics {
    pub ejector_airtime_seconds: f64,
    pub floater_airtime_seconds: f64,
    pub airtime_moments: usize,
    pub strong_positive_g_seconds: f64,
    pub longest_strong_positive_g_seconds: f64,
    pub strong_lateral_g_seconds: f64,
    pub max_positive_vertical_g: f64,
    pub max_negative_vertical_g: f64,
    pub max_lateral_g: f64,
    pub max_forward_g: f64,
    pub max_braking_g: f64,
    pub max_speed: f64,
    pub average_speed: f64,
    pub duration_seconds: f64,
    pub track_length: f64,
    pub max_height: f64,
    pub biggest_drop: f64,
    pub steepest_drop_angle_degrees: f64,
    pub inversions: usize,
    pub inverted_seconds: f64,
    pub drops: usize,
    pub launches: usize,
    pub launch_peak_speed: f64,
}

impl TestRun {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"TestRun-v5");
        hash.write_len(self.samples.len());
        for sample in &self.samples {
            write_sample(hash, *sample);
        }
        hash.write_len(self.proximity_events.len());
        for &event in &self.proximity_events {
            write_proximity_event(hash, event);
        }
        hash.write_f64(self.excitement);
        hash.write_f64(self.intensity);
        hash.write_f64(self.nausea);
        let stats = self.statistics;
        for value in [
            stats.ejector_airtime_seconds,
            stats.floater_airtime_seconds,
            stats.strong_positive_g_seconds,
            stats.longest_strong_positive_g_seconds,
            stats.strong_lateral_g_seconds,
            stats.max_positive_vertical_g,
            stats.max_negative_vertical_g,
            stats.max_lateral_g,
            stats.max_forward_g,
            stats.max_braking_g,
            stats.max_speed,
            stats.average_speed,
            stats.duration_seconds,
            stats.track_length,
            stats.max_height,
            stats.biggest_drop,
            stats.steepest_drop_angle_degrees,
            stats.inverted_seconds,
            stats.launch_peak_speed,
        ] {
            hash.write_f64(value);
        }
        for value in [
            stats.airtime_moments,
            stats.inversions,
            stats.drops,
            stats.launches,
        ] {
            hash.write_len(value);
        }
        hash.write_len(self.notes.len());
        for note in &self.notes {
            hash.write_bytes(note.as_bytes());
        }
        hash.write_bool(self.completed);
        hash.write_f64(self.furthest_distance);
        write_ranges(hash, &self.visited_ranges);
        for contributions in [
            &self.breakdown.excitement,
            &self.breakdown.intensity,
            &self.breakdown.nausea,
        ] {
            hash.write_len(contributions.len());
            for contribution in contributions {
                hash.write_u8(contribution.term as u8);
                hash.write_f64(contribution.value);
                hash.write_f64(contribution.measure);
                hash.write_option(contribution.location, |hash, location| {
                    hash.write_len(location.piece_index);
                    hash.write_bytes(location.piece_name.as_bytes());
                    hash.write_f64(location.distance_m);
                    hash.write_f64(location.seconds_from_start);
                });
            }
        }
        hash.write_len(self.rating_inputs.len());
        for input in &self.rating_inputs {
            input.hash_into(hash);
        }
        hash.write_len(self.rating_notes.len());
        for note in &self.rating_notes {
            hash.write_bytes(note.as_bytes());
        }
    }

    pub(super) fn average_car_ratings(&mut self, reports: &[TestRun]) {
        assert!(!reports.is_empty());
        self.rating_inputs = reports
            .iter()
            .flat_map(|report| report.rating_inputs.iter().cloned())
            .collect();
        self.recompute_ratings();
    }

    pub fn unfinished_circuit() -> Self {
        Self {
            samples: Vec::new(),
            proximity_events: Vec::new(),
            excitement: 0.0,
            intensity: 0.0,
            nausea: 0.0,
            breakdown: ScoreBreakdown::default(),
            statistics: RideStatistics::default(),
            notes: vec![
                "The circuit is unfinished. Connect the track into a complete loop before testing the train."
                    .to_owned(),
            ],
            completed: false,
            furthest_distance: 0.0,
            visited_ranges: Vec::new(),
            rating_inputs: Vec::new(),
            rating_notes: Vec::new(),
        }
    }

    /// Add events at positions actually sampled by this car.
    pub fn add_proximity_events(&mut self, events: &[ProximityEvent]) {
        for event in events {
            for input in &mut self.rating_inputs {
                if input
                    .visited_ranges
                    .iter()
                    .any(|(start, end)| event.distance >= *start && event.distance <= *end)
                {
                    insert_event(&mut input.proximity_events, *event);
                }
            }
            if self
                .visited_ranges
                .iter()
                .any(|(start, end)| event.distance >= *start && event.distance <= *end)
            {
                self.insert_proximity_event(*event);
            }
        }
        self.recompute_ratings();
    }

    pub fn set_theming(&mut self, theming: f64) {
        assert!(
            (0.0..=1.0).contains(&theming),
            "theming score must be in [0, 1]"
        );
        for input in &mut self.rating_inputs {
            input.theming = theming;
        }
        self.recompute_ratings();
    }

    fn insert_proximity_event(&mut self, event: ProximityEvent) {
        insert_event(&mut self.proximity_events, event);
    }

    fn recompute_ratings(&mut self) {
        if self.rating_inputs.is_empty() {
            return;
        }
        self.notes.retain(|note| !self.rating_notes.contains(note));
        self.rating_notes.clear();
        self.breakdown = ScoreBreakdown::default();
        let count = self.rating_inputs.len() as f64;
        for input in &self.rating_inputs {
            let mut features = super::features::ride_features(
                &input.samples,
                &input.pieces,
                &input.proximity_events,
                input.circuit_length,
                input.completed,
            );
            features.statistics = input.statistics;
            features.envelope = input.envelope.clone();
            features.theming = input.theming;
            let scores = score(&features, &input.force_bands);
            for note in scoring_notes(&features, scores.intensity, &scores.breakdown.nausea) {
                if !self.rating_notes.contains(&note) {
                    self.rating_notes.push(note);
                }
            }
            for (target, source) in [
                (&mut self.breakdown.excitement, scores.breakdown.excitement),
                (&mut self.breakdown.intensity, scores.breakdown.intensity),
                (&mut self.breakdown.nausea, scores.breakdown.nausea),
            ] {
                if target.is_empty() {
                    target.extend(source.into_iter().map(|mut contribution| {
                        contribution.value /= count;
                        contribution.measure /= count;
                        contribution
                    }));
                } else {
                    assert_eq!(target.len(), source.len());
                    for (total, contribution) in target.iter_mut().zip(source) {
                        assert_eq!(total.term, contribution.term);
                        total.value += contribution.value / count;
                        total.measure += contribution.measure / count;
                        if total.location.is_none() {
                            total.location = contribution.location;
                        }
                    }
                }
            }
        }
        self.excitement = self
            .breakdown
            .excitement
            .iter()
            .map(|item| item.value)
            .sum();
        self.intensity = self.breakdown.intensity.iter().map(|item| item.value).sum();
        self.nausea = self.breakdown.nausea.iter().map(|item| item.value).sum();
        self.notes.extend(self.rating_notes.iter().cloned());
    }
}

fn insert_event(events: &mut Vec<ProximityEvent>, event: ProximityEvent) {
    if let Some(existing) = events
        .iter_mut()
        .find(|existing| existing.kind == event.kind && existing.piece_index == event.piece_index)
    {
        if event.strength > existing.strength {
            *existing = event;
        }
    } else {
        events.push(event);
    }
}

fn scoring_notes(
    features: &super::features::RideFeatures,
    intensity: f64,
    nausea: &[super::scoring::Contribution],
) -> Vec<String> {
    let mut notes = Vec::new();
    if intensity >= 7.0 {
        notes.push(
            "This is a very intense ride. A few lower-force sections can make it more comfortable."
                .to_owned(),
        );
    }
    if nausea
        .iter()
        .any(|item| item.term == super::scoring::Term::Laterals && item.value >= 1.5)
    {
        notes.push("Some turns pull riders sideways strongly. More banking or wider turns can improve comfort.".to_owned());
    }
    if features.dead_excess_seconds >= 8.0 {
        notes.push(
            "Long slow stretches reduce excitement. Consider shortening them or adding elements."
                .to_owned(),
        );
    }
    if let Some(worst) = &features.envelope.worst {
        notes.push(format!("High forces exceed the ASTM F2291 comfort envelope for {:.1} s, worst at {} ({:?} {:.1} g where {:.1} g is allowed). The scores are reduced; a gentler valley or a slower entry would recover them.", features.envelope.seconds, worst.location.piece_name, worst.axis, worst.value, worst.allowed));
    }
    notes
}

/// Turns per-tick train readings into one interpolated sample per track metre.
pub struct TestRunRecorder {
    circuit_length: f64,
    force_limits: ComfortableForceLimits,
    force_bands: ForceBandDurations,
    comfort: EnvelopeTracker,
    piece_starts: Vec<f64>,
    piece_summaries: Vec<PieceSummary>,
    rated_pieces: Vec<Option<RatedPieceSpeedWindow>>,
    launch_ranges: Vec<(f64, f64)>,
    maximum_positive_vertical_g: f64,
    maximum_negative_vertical_g: f64,
    maximum_lateral_g: f64,
    maximum_forward_g: f64,
    maximum_braking_g: f64,
    maximum_speed: f64,
    maximum_height: f64,
    last_piece_index: Option<usize>,
    noted_rated_pieces: Vec<bool>,
    speed_band_notes: Vec<String>,
    samples: Vec<Option<TestSample>>,
    started: bool,
    start_unwrapped_distance: f64,
    last_wrapped_distance: f64,
    last_unwrapped_distance: f64,
    last_reading: Option<CarReading>,
    next_sample_distance: f64,
    furthest_distance: f64,
    dueling_events: Vec<ProximityEvent>,
    visited_ranges: Vec<(f64, f64)>,
}

#[derive(Clone, Copy, Debug)]
struct RatedPieceSpeedWindow {
    name: &'static str,
    minimum_entry: f64,
    maximum_entry: f64,
}

impl TestRunRecorder {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"TestRunRecorder-v6");
        hash.write_f64(self.circuit_length);
        for value in [
            self.force_limits.minimum_vertical_g,
            self.force_limits.maximum_vertical_g,
            self.force_limits.maximum_lateral_g,
            self.force_limits.maximum_longitudinal_g,
        ] {
            hash.write_f64(value);
        }
        hash_force_bands(hash, self.force_bands);
        self.comfort.hash_into(hash);
        hash.write_len(self.piece_starts.len());
        for &piece_start in &self.piece_starts {
            hash.write_f64(piece_start);
        }
        hash.write_len(self.piece_summaries.len());
        for piece in &self.piece_summaries {
            piece.hash_into(hash);
        }
        hash.write_len(self.rated_pieces.len());
        for rated_piece in &self.rated_pieces {
            hash.write_option(*rated_piece, |hash, window| {
                hash.write_bytes(window.name.as_bytes());
                hash.write_f64(window.minimum_entry);
                hash.write_f64(window.maximum_entry);
            });
        }
        write_ranges(hash, &self.launch_ranges);
        for value in [
            self.maximum_positive_vertical_g,
            self.maximum_negative_vertical_g,
            self.maximum_lateral_g,
            self.maximum_forward_g,
            self.maximum_braking_g,
            self.maximum_speed,
            self.maximum_height,
        ] {
            hash.write_f64(value);
        }
        hash.write_option(self.last_piece_index, |hash, index| hash.write_len(index));
        hash.write_len(self.noted_rated_pieces.len());
        for &noted in &self.noted_rated_pieces {
            hash.write_bool(noted);
        }
        hash.write_len(self.speed_band_notes.len());
        for note in &self.speed_band_notes {
            hash.write_bytes(note.as_bytes());
        }
        hash.write_len(self.samples.len());
        for sample in &self.samples {
            hash.write_option(*sample, write_sample);
        }
        hash.write_bool(self.started);
        hash.write_f64(self.start_unwrapped_distance);
        hash.write_f64(self.last_wrapped_distance);
        hash.write_f64(self.last_unwrapped_distance);
        hash.write_option(self.last_reading, write_reading);
        hash.write_f64(self.next_sample_distance);
        hash.write_f64(self.furthest_distance);
        hash.write_len(self.dueling_events.len());
        for &event in &self.dueling_events {
            write_proximity_event(hash, event);
        }
        write_ranges(hash, &self.visited_ranges);
    }

    pub fn new(circuit_length: f64) -> Self {
        assert!(circuit_length.is_finite() && circuit_length > 0.0);
        Self {
            circuit_length,
            force_limits: crate::track::hybrid_model().comfortable_force_limits,
            force_bands: ForceBandDurations::default(),
            comfort: EnvelopeTracker::default(),
            piece_starts: Vec::new(),
            piece_summaries: Vec::new(),
            rated_pieces: Vec::new(),
            launch_ranges: Vec::new(),
            maximum_positive_vertical_g: 0.0,
            maximum_negative_vertical_g: 0.0,
            maximum_lateral_g: 0.0,
            maximum_forward_g: 0.0,
            maximum_braking_g: 0.0,
            maximum_speed: 0.0,
            maximum_height: 0.0,
            last_piece_index: None,
            noted_rated_pieces: Vec::new(),
            speed_band_notes: Vec::new(),
            samples: vec![None; circuit_length.ceil() as usize],
            started: false,
            start_unwrapped_distance: 0.0,
            last_wrapped_distance: 0.0,
            last_unwrapped_distance: 0.0,
            last_reading: None,
            next_sample_distance: 0.0,
            furthest_distance: 0.0,
            dueling_events: Vec::new(),
            visited_ranges: Vec::new(),
        }
    }

    pub fn for_circuit(circuit: &Circuit) -> Self {
        Self::for_circuit_with_limits(
            circuit,
            crate::track::hybrid_model().comfortable_force_limits,
        )
    }

    pub fn for_circuit_with_limits(circuit: &Circuit, limits: ComfortableForceLimits) -> Self {
        let mut recorder = Self::new(circuit.length());
        recorder.force_limits = limits;
        recorder.piece_starts.clone_from(&circuit.starts);
        recorder.piece_summaries = circuit
            .pieces
            .iter()
            .map(|piece| PieceSummary {
                id: piece.id,
                name: piece.name,
                kind: piece.kind,
                shape: piece.shape.clone(),
                inversion: piece.inversion,
                heartlining: piece.heartlining,
                length: piece.length(),
                turn_angle_radians: piece_turn_angle_radians(piece),
            })
            .collect();
        recorder.rated_pieces = circuit
            .pieces
            .iter()
            .map(|piece| {
                piece.rated_speeds.map(|speeds| RatedPieceSpeedWindow {
                    name: piece.name,
                    minimum_entry: speeds.minimum_entry,
                    maximum_entry: speeds.maximum_entry,
                })
            })
            .collect();
        for (index, piece) in circuit.pieces.iter().enumerate() {
            if piece.kind != crate::track::PieceKind::Launch {
                continue;
            }
            let start = circuit.starts[index];
            let end = start + piece.length();
            if let Some((_, previous_end)) = recorder.launch_ranges.last_mut()
                && (*previous_end - start).abs() < 1e-9
            {
                *previous_end = end;
            } else {
                recorder.launch_ranges.push((start, end));
            }
        }
        recorder.noted_rated_pieces = vec![false; circuit.pieces.len()];
        recorder
    }

    /// Records readings for one car at each physics step.
    ///
    /// Readings are sampled at 200 Hz, so interpolation at crossed metre marks
    /// stays close to the simulated motion while keeping the overlay compact.
    pub fn record(&mut self, frames: &[Vec<CarReading>]) {
        for frame in frames {
            if frame.is_empty() {
                continue;
            }
            assert_eq!(frame.len(), 1, "a test recorder tracks one car");
            let reading = frame[0];
            let active = reading.speed > 1.0
                && (self.piece_summaries.is_empty()
                    || matches!(
                        self.piece_summaries[reading.piece_index].kind,
                        crate::track::PieceKind::Track
                            | crate::track::PieceKind::Launch
                            | crate::track::PieceKind::Brakes
                    ));
            if active {
                record_force_bands(
                    &mut self.force_bands,
                    reading,
                    self.force_limits,
                    super::TIME_STEP,
                );
            }
            let piece_name = self
                .piece_summaries
                .get(reading.piece_index)
                .map_or("Track", |piece| piece.name);
            self.comfort.record(
                [
                    reading.g.vertical,
                    reading.g.lateral,
                    reading.g.longitudinal,
                ],
                super::TIME_STEP,
                active,
                super::features::Location {
                    piece_index: reading.piece_index,
                    piece_name,
                    distance_m: reading.distance.rem_euclid(self.circuit_length),
                    seconds_from_start: 0.0,
                },
            );
            self.maximum_positive_vertical_g =
                self.maximum_positive_vertical_g.max(reading.g.vertical);
            self.maximum_negative_vertical_g =
                self.maximum_negative_vertical_g.min(reading.g.vertical);
            self.maximum_lateral_g = self.maximum_lateral_g.max(reading.g.lateral.abs());
            self.maximum_forward_g = self.maximum_forward_g.max(reading.g.longitudinal.max(0.0));
            self.maximum_braking_g = self
                .maximum_braking_g
                .max((-reading.g.longitudinal).max(0.0));
            self.maximum_speed = self.maximum_speed.max(reading.speed);
            self.maximum_height = self.maximum_height.max(reading.height_m);
            let wrapped_distance = reading.distance.rem_euclid(self.circuit_length);
            if !self.piece_starts.is_empty() {
                let piece_index = self
                    .piece_starts
                    .partition_point(|start| *start <= wrapped_distance)
                    - 1;
                if self.last_piece_index != Some(piece_index) {
                    let entry_speed =
                        self.rated_piece_entry_speed(piece_index, wrapped_distance, reading.speed);
                    self.record_rated_speed_band(piece_index, entry_speed);
                    self.last_piece_index = Some(piece_index);
                }
            }
            if !self.started {
                self.visited_ranges
                    .push((wrapped_distance, wrapped_distance));
                self.started = true;
                self.start_unwrapped_distance = wrapped_distance;
                self.last_wrapped_distance = wrapped_distance;
                self.last_unwrapped_distance = wrapped_distance;
                self.next_sample_distance =
                    next_sample_at_or_after(wrapped_distance, self.circuit_length);
                if (self.next_sample_distance - wrapped_distance).abs() < 1e-9 {
                    self.store_sample(wrapped_distance, reading);
                    self.next_sample_distance =
                        next_sample_after(self.next_sample_distance, self.circuit_length);
                }
                self.last_reading = Some(reading);
                continue;
            }

            let mut delta = wrapped_distance - self.last_wrapped_distance;
            if delta < -self.circuit_length / 2.0 {
                delta += self.circuit_length;
            } else if delta > self.circuit_length / 2.0 {
                delta -= self.circuit_length;
            }
            let endpoint = self.last_wrapped_distance + delta;
            if endpoint < 0.0 {
                self.add_visited_range(0.0, self.last_wrapped_distance);
                self.add_visited_range(endpoint + self.circuit_length, self.circuit_length);
            } else if endpoint >= self.circuit_length {
                self.add_visited_range(self.last_wrapped_distance, self.circuit_length);
                self.add_visited_range(0.0, endpoint - self.circuit_length);
            } else {
                self.add_visited_range(
                    self.last_wrapped_distance.min(endpoint),
                    self.last_wrapped_distance.max(endpoint),
                );
            }
            let unwrapped_distance = self.last_unwrapped_distance + delta;
            self.furthest_distance = self
                .furthest_distance
                .max(unwrapped_distance - self.start_unwrapped_distance);
            if delta > 0.0 {
                let previous = self.last_reading.expect("started recorder has a reading");
                while self.next_sample_distance <= unwrapped_distance + 1e-9 {
                    let fraction = ((self.next_sample_distance - self.last_unwrapped_distance)
                        / delta)
                        .clamp(0.0, 1.0);
                    self.store_sample(
                        self.next_sample_distance.rem_euclid(self.circuit_length),
                        interpolate_reading(previous, reading, fraction),
                    );
                    self.next_sample_distance =
                        next_sample_after(self.next_sample_distance, self.circuit_length);
                }
            }
            self.last_wrapped_distance = wrapped_distance;
            self.last_unwrapped_distance = unwrapped_distance;
            self.last_reading = Some(reading);
        }
    }

    pub fn record_dueling(
        &mut self,
        circuit: &Circuit,
        reading: CarReading,
        others: &[TrainPosition],
    ) {
        let sample = circuit.sample(reading.distance);
        let primary = TrainPosition {
            position: sample.position,
            tangent: sample.tangent,
            piece_index: circuit.piece_index(reading.distance),
            distance: reading.distance.rem_euclid(circuit.length()),
            speed: reading.speed,
        };
        if let Some(event) = detect_dueling(primary, others) {
            if let Some(existing) = self
                .dueling_events
                .iter_mut()
                .find(|existing| existing.piece_index == event.piece_index)
            {
                if event.strength > existing.strength {
                    *existing = event;
                }
            } else {
                self.dueling_events.push(event);
            }
        }
    }

    /// Finalizes the report. An incomplete run includes a distance note; a
    /// caller-reported rollback adds a separate recovery note.
    pub fn finish(self, completed: bool, rolled_back: bool) -> TestRun {
        let circuit_length = self.circuit_length;
        let samples: Vec<_> = self.samples.into_iter().flatten().collect();
        let mut notes = Vec::new();
        let mut statistics = ride_statistics(&samples, circuit_length, completed);
        statistics.max_positive_vertical_g = self.maximum_positive_vertical_g;
        statistics.max_negative_vertical_g = self.maximum_negative_vertical_g;
        statistics.max_lateral_g = self.maximum_lateral_g;
        statistics.max_forward_g = self.maximum_forward_g;
        statistics.max_braking_g = self.maximum_braking_g;
        statistics.max_speed = self.maximum_speed.max(statistics.max_speed);
        statistics.max_height = self.maximum_height.max(statistics.max_height);
        statistics.launches = self
            .launch_ranges
            .iter()
            .filter(|(start, end)| {
                samples
                    .iter()
                    .any(|sample| sample.distance >= *start && sample.distance <= *end)
            })
            .count();
        statistics.launch_peak_speed = samples
            .iter()
            .filter(|sample| {
                self.launch_ranges
                    .iter()
                    .any(|(start, end)| sample.distance >= *start && sample.distance <= *end)
            })
            .map(|sample| sample.speed)
            .fold(0.0, f64::max);
        notes.extend(self.speed_band_notes);
        if !completed {
            notes.push(format!(
                "The train could not finish the circuit after travelling {:.1} m. Add a lift or launch, or shorten the circuit.",
                self.furthest_distance
            ));
        }
        if rolled_back {
            notes.push("The train rolled back and was recovered safely.".to_owned());
        }
        let mut scoring_samples = samples.clone();
        let pieces = if self.piece_summaries.is_empty() {
            for sample in &mut scoring_samples {
                sample.piece_index = 0;
            }
            vec![PieceSummary {
                id: crate::track::PieceId::Straight,
                name: "Track",
                kind: crate::track::PieceKind::Track,
                shape: crate::track::Shape::Plain,
                inversion: None,
                heartlining: None,
                length: circuit_length,
                turn_angle_radians: 0.0,
            }]
        } else {
            self.piece_summaries
        };
        let input = RatingInput {
            samples: scoring_samples,
            pieces,
            proximity_events: self.dueling_events.clone(),
            theming: 0.0,
            visited_ranges: self.visited_ranges.clone(),
            circuit_length,
            completed,
            force_limits: self.force_limits,
            force_bands: self.force_bands,
            statistics,
            envelope: self.comfort.finish(),
        };
        let mut report = TestRun {
            samples,
            proximity_events: self.dueling_events,
            excitement: 0.0,
            intensity: 0.0,
            nausea: 0.0,
            breakdown: ScoreBreakdown::default(),
            statistics,
            notes,
            completed,
            furthest_distance: self.furthest_distance,
            visited_ranges: self.visited_ranges,
            rating_inputs: vec![input],
            rating_notes: Vec::new(),
        };
        report.recompute_ratings();
        report
    }

    fn record_rated_speed_band(&mut self, piece_index: usize, entry_speed: f64) {
        let Some(window) = self.rated_pieces[piece_index] else {
            return;
        };
        if self.noted_rated_pieces[piece_index] {
            return;
        }
        let condition = if entry_speed < window.minimum_entry {
            "below"
        } else if entry_speed > window.maximum_entry {
            "above"
        } else {
            return;
        };
        self.noted_rated_pieces[piece_index] = true;
        self.speed_band_notes.push(format!(
            "The train entered {} at {entry_speed:.1} m/s, {condition} its rated speed band of {:.1}–{:.1} m/s.",
            window.name, window.minimum_entry, window.maximum_entry
        ));
    }

    fn rated_piece_entry_speed(
        &self,
        piece_index: usize,
        current_distance: f64,
        current_speed: f64,
    ) -> f64 {
        let Some(previous) = self.last_reading else {
            return current_speed;
        };
        let previous_distance = previous.distance.rem_euclid(self.circuit_length);
        let mut distance_delta = current_distance - previous_distance;
        if distance_delta < -self.circuit_length / 2.0 {
            distance_delta += self.circuit_length;
        } else if distance_delta > self.circuit_length / 2.0 {
            distance_delta -= self.circuit_length;
        }
        if distance_delta == 0.0 {
            return current_speed;
        }
        let mut distance_to_entry = self.piece_starts[piece_index] - previous_distance;
        if distance_delta > 0.0 && distance_to_entry < 0.0 {
            distance_to_entry += self.circuit_length;
        } else if distance_delta < 0.0 && distance_to_entry > 0.0 {
            distance_to_entry -= self.circuit_length;
        }
        let fraction = distance_to_entry / distance_delta;
        if !(0.0..=1.0).contains(&fraction) {
            return current_speed;
        }
        previous.speed + fraction * (current_speed - previous.speed)
    }

    fn store_sample(&mut self, distance: f64, reading: CarReading) {
        let distance = distance.rem_euclid(self.circuit_length);
        let index = distance.round() as usize % self.samples.len();
        if self.samples[index].is_none() {
            let piece_index = if self.piece_starts.is_empty() {
                reading.piece_index
            } else {
                self.piece_starts
                    .partition_point(|start| *start <= distance)
                    - 1
            };
            self.samples[index] = Some(TestSample {
                distance,
                speed: reading.speed,
                vertical_g: reading.g.vertical,
                lateral_g: reading.g.lateral,
                longitudinal_g: reading.g.longitudinal,
                airtime: reading.g.vertical < 0.0,
                height_m: reading.height_m,
                inverted: reading.inverted,
                roll: reading.roll,
                piece_index,
            });
        }
    }

    fn add_visited_range(&mut self, start: f64, end: f64) {
        if let Some(last) = self.visited_ranges.last_mut() {
            if start <= last.1 && end >= last.0 {
                last.0 = last.0.min(start);
                last.1 = last.1.max(end);
                return;
            }
        }
        self.visited_ranges.push((start, end));
    }
}

fn write_sample(hash: &mut StateHasher, sample: TestSample) {
    hash.write_f64(sample.distance);
    hash.write_f64(sample.speed);
    hash.write_f64(sample.vertical_g);
    hash.write_f64(sample.lateral_g);
    hash.write_f64(sample.longitudinal_g);
    hash.write_bool(sample.airtime);
    hash.write_f64(sample.height_m);
    hash.write_bool(sample.inverted);
    hash.write_f64(sample.roll);
    hash.write_len(sample.piece_index);
}

fn piece_turn_angle_radians(piece: &Piece) -> f64 {
    const GEOMETRY_STEPS: usize = 64;
    const MINIMUM_HORIZONTAL_TANGENT: f64 = 0.1;
    let mut previous_yaw: Option<f64> = None;
    let mut turn_angle = 0.0;
    for step in 0..=GEOMETRY_STEPS {
        let distance = piece.length() * step as f64 / GEOMETRY_STEPS as f64;
        let tangent = piece.sample(distance).tangent;
        if tangent.x.hypot(tangent.y) < MINIMUM_HORIZONTAL_TANGENT {
            continue;
        }
        let yaw = tangent.y.atan2(tangent.x);
        if let Some(previous_yaw) = previous_yaw {
            let difference = (yaw - previous_yaw + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            turn_angle += difference;
        }
        previous_yaw = Some(yaw);
    }
    turn_angle
}

fn write_reading(hash: &mut StateHasher, reading: CarReading) {
    hash.write_f64(reading.distance);
    hash.write_f64(reading.speed);
    hash.write_f64(reading.g.vertical);
    hash.write_f64(reading.g.lateral);
    hash.write_f64(reading.g.longitudinal);
    hash.write_bool(reading.airtime);
    hash.write_f64(reading.height_m);
    hash.write_bool(reading.inverted);
    hash.write_f64(reading.roll);
    hash.write_len(reading.piece_index);
}

fn write_proximity_event(hash: &mut StateHasher, event: crate::track::proximity::ProximityEvent) {
    hash.write_len(event.piece_index);
    hash.write_u8(event.kind as u8);
    hash.write_f64(event.position.x);
    hash.write_f64(event.position.y);
    hash.write_f64(event.position.z);
    hash.write_f64(event.distance);
    hash.write_f64(event.strength);
}

fn write_ranges(hash: &mut StateHasher, ranges: &[(f64, f64)]) {
    hash.write_len(ranges.len());
    for &(start, end) in ranges {
        hash.write_f64(start);
        hash.write_f64(end);
    }
}

fn next_sample_at_or_after(distance: f64, circuit_length: f64) -> f64 {
    let lap_start = (distance / circuit_length).floor() * circuit_length;
    let within_lap = distance - lap_start;
    let target = within_lap.ceil();
    if target < circuit_length {
        lap_start + target
    } else {
        lap_start + circuit_length
    }
}

fn next_sample_after(distance: f64, circuit_length: f64) -> f64 {
    let lap_start = (distance / circuit_length).floor() * circuit_length;
    let within_lap = distance - lap_start;
    let target = within_lap + 1.0;
    if target < circuit_length {
        lap_start + target
    } else {
        lap_start + circuit_length
    }
}

fn interpolate_reading(first: CarReading, second: CarReading, fraction: f64) -> CarReading {
    let interpolate = |first: f64, second: f64| first + (second - first) * fraction;
    let vertical = interpolate(first.g.vertical, second.g.vertical);
    CarReading {
        distance: interpolate(first.distance, second.distance),
        speed: interpolate(first.speed, second.speed),
        g: super::GForces {
            vertical,
            lateral: interpolate(first.g.lateral, second.g.lateral),
            longitudinal: interpolate(first.g.longitudinal, second.g.longitudinal),
        },
        airtime: vertical < 0.0,
        height_m: interpolate(first.height_m, second.height_m),
        inverted: if fraction < 0.5 {
            first.inverted
        } else {
            second.inverted
        },
        roll: interpolate(first.roll, second.roll),
        piece_index: if fraction < 0.5 {
            first.piece_index
        } else {
            second.piece_index
        },
    }
}

pub(super) fn ride_statistics(
    samples: &[TestSample],
    circuit_length: f64,
    completed: bool,
) -> RideStatistics {
    if samples.is_empty() {
        return RideStatistics {
            track_length: circuit_length,
            ..RideStatistics::default()
        };
    }
    let mut stats = RideStatistics {
        track_length: circuit_length,
        max_positive_vertical_g: samples
            .iter()
            .map(|sample| sample.vertical_g)
            .fold(0.0, f64::max),
        max_negative_vertical_g: samples
            .iter()
            .map(|sample| sample.vertical_g)
            .fold(0.0, f64::min),
        max_lateral_g: samples
            .iter()
            .map(|sample| sample.lateral_g.abs())
            .fold(0.0, f64::max),
        max_forward_g: samples
            .iter()
            .map(|sample| sample.longitudinal_g.max(0.0))
            .fold(0.0, f64::max),
        max_braking_g: samples
            .iter()
            .map(|sample| (-sample.longitudinal_g).max(0.0))
            .fold(0.0, f64::max),
        max_speed: samples
            .iter()
            .map(|sample| sample.speed)
            .fold(0.0, f64::max),
        average_speed: samples.iter().map(|sample| sample.speed).sum::<f64>()
            / samples.len() as f64,
        max_height: samples
            .iter()
            .map(|sample| sample.height_m)
            .fold(f64::NEG_INFINITY, f64::max),
        biggest_drop: if completed {
            largest_downhill_drop(samples)
        } else {
            largest_downhill_drop_on_sampled_run(samples)
        },
        inversions: if completed {
            count_inversion_passages(samples)
        } else {
            count_inversion_passages_on_sampled_run(samples)
        },
        ..RideStatistics::default()
    };
    let mut previous_airtime = if completed {
        samples
            .last()
            .is_some_and(|sample| sample.vertical_g < FLOATER_AIRTIME_MAX_VERTICAL_G)
    } else {
        samples[0].vertical_g < FLOATER_AIRTIME_MAX_VERTICAL_G
    };
    let mut positive_run = 0.0;
    let mut previous_height_delta = if completed {
        samples[0].height_m - samples[samples.len() - 1].height_m
    } else {
        0.0
    };
    let mut total_time = 0.0;
    let mut total_distance = 0.0;
    let mut leading_positive_run = 0.0;
    let mut trailing_positive_run = 0.0;
    let mut still_at_start = true;
    for (index, sample) in samples.iter().enumerate() {
        let next = samples.get(index + 1).unwrap_or(&samples[0]);
        let interval = sample_interval(samples, index, circuit_length, completed);
        let distance = interval.distance_m;
        let seconds = interval.seconds;
        total_time += seconds;
        total_distance += distance;
        let ejector = sample.vertical_g < EJECTOR_AIRTIME_MAX_VERTICAL_G;
        let floater = sample.vertical_g >= EJECTOR_AIRTIME_MAX_VERTICAL_G
            && sample.vertical_g < FLOATER_AIRTIME_MAX_VERTICAL_G;
        stats.ejector_airtime_seconds += if ejector { seconds } else { 0.0 };
        stats.floater_airtime_seconds += if floater { seconds } else { 0.0 };
        let airtime = sample.vertical_g < FLOATER_AIRTIME_MAX_VERTICAL_G;
        if airtime && !previous_airtime {
            stats.airtime_moments += 1;
        }
        previous_airtime = airtime;
        if sample.vertical_g > STRONG_POSITIVE_G_THRESHOLD {
            stats.strong_positive_g_seconds += seconds;
            positive_run += seconds;
            trailing_positive_run += seconds;
            if still_at_start {
                leading_positive_run += seconds;
            }
            stats.longest_strong_positive_g_seconds =
                stats.longest_strong_positive_g_seconds.max(positive_run);
        } else {
            positive_run = 0.0;
            trailing_positive_run = 0.0;
            still_at_start = false;
        }
        if sample.lateral_g.abs() > STRONG_LATERAL_G_THRESHOLD {
            stats.strong_lateral_g_seconds += seconds;
        }
        if sample.inverted {
            stats.inverted_seconds += seconds;
        }
        if index + 1 == samples.len() && !completed {
            continue;
        }
        let height_delta = next.height_m - sample.height_m;
        if previous_height_delta > 0.0 && height_delta <= 0.0 {
            stats.drops += 1;
        }
        if height_delta < 0.0 {
            let angle = (-height_delta / distance.max(1e-9))
                .clamp(-1.0, 1.0)
                .asin()
                .to_degrees();
            stats.steepest_drop_angle_degrees = stats.steepest_drop_angle_degrees.max(angle);
        }
        previous_height_delta = height_delta;
    }
    stats.duration_seconds = total_time;
    if total_time > 0.0 {
        stats.average_speed = total_distance / total_time;
    }
    if completed {
        let all_intervals_are_strong = samples
            .iter()
            .all(|sample| sample.vertical_g > STRONG_POSITIVE_G_THRESHOLD);
        if all_intervals_are_strong {
            stats.longest_strong_positive_g_seconds = total_time;
        } else {
            stats.longest_strong_positive_g_seconds = stats
                .longest_strong_positive_g_seconds
                .max(leading_positive_run + trailing_positive_run);
        }
    }
    stats
}

fn largest_downhill_drop(samples: &[TestSample]) -> f64 {
    let Some(first) = samples.first() else {
        return 0.0;
    };
    let mut highest_point = first.height_m;
    let mut largest_drop: f64 = 0.0;
    for sample in samples.iter().chain(samples) {
        largest_drop = largest_drop.max(highest_point - sample.height_m);
        highest_point = highest_point.max(sample.height_m);
    }
    largest_drop
}

fn largest_downhill_drop_on_sampled_run(samples: &[TestSample]) -> f64 {
    let Some(first) = samples.first() else {
        return 0.0;
    };
    let mut highest_point = first.height_m;
    let mut largest_drop: f64 = 0.0;
    for sample in samples {
        largest_drop = largest_drop.max(highest_point - sample.height_m);
        highest_point = highest_point.max(sample.height_m);
    }
    largest_drop
}

fn count_inversion_passages(samples: &[TestSample]) -> usize {
    if samples.is_empty() {
        return 0;
    }
    let mut previous_inverted = samples.last().expect("nonempty samples").inverted;
    let mut passages = 0;
    for sample in samples {
        if sample.inverted && !previous_inverted {
            passages += 1;
        }
        previous_inverted = sample.inverted;
    }
    passages
}

fn count_inversion_passages_on_sampled_run(samples: &[TestSample]) -> usize {
    let Some(first) = samples.first() else {
        return 0;
    };
    let mut previous_inverted = first.inverted;
    let mut passages = 0;
    for sample in samples.iter().skip(1) {
        if sample.inverted && !previous_inverted {
            passages += 1;
        }
        previous_inverted = sample.inverted;
    }
    passages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{CarReading, GForces, PhysicsConfig, Term, Train, demo_circuit};
    use crate::track::{PieceId, PieceKind, TrackState, catalog, hybrid_model};

    fn ratings(samples: &[TestSample], circuit_length: f64) -> (f64, f64, f64, Vec<String>) {
        let pieces = [PieceSummary {
            id: PieceId::Straight,
            name: "Track",
            kind: PieceKind::Track,
            shape: crate::track::Shape::Plain,
            inversion: None,
            heartlining: None,
            length: circuit_length,
            turn_angle_radians: 0.0,
        }];
        let features =
            super::super::features::ride_features(samples, &pieces, &[], circuit_length, true);
        let limits = hybrid_model().comfortable_force_limits;
        let mut bands = ForceBandDurations::default();
        for (index, sample) in samples.iter().enumerate() {
            if !super::super::features::is_active_sample(sample, &pieces) {
                continue;
            }
            let reading = CarReading {
                distance: sample.distance,
                speed: sample.speed,
                g: GForces {
                    vertical: sample.vertical_g,
                    lateral: sample.lateral_g,
                    longitudinal: sample.longitudinal_g,
                },
                airtime: sample.airtime,
                height_m: sample.height_m,
                inverted: sample.inverted,
                roll: sample.roll,
                piece_index: sample.piece_index,
            };
            record_force_bands(
                &mut bands,
                reading,
                limits,
                sample_interval(samples, index, circuit_length, true).seconds,
            );
        }
        let result = score(&features, &bands);
        let notes = scoring_notes(&features, result.intensity, &result.breakdown.nausea);
        (result.excitement, result.intensity, result.nausea, notes)
    }

    #[test]
    fn score_reference_profiles_and_extreme_force_saturation() {
        let straight: Vec<_> = (0..100)
            .map(|distance| TestSample {
                distance: distance as f64,
                speed: 10.0,
                vertical_g: 1.0,
                lateral_g: 0.0,
                longitudinal_g: 0.0,
                airtime: false,
                height_m: 0.0,
                inverted: false,
                roll: 0.0,
                piece_index: 0,
            })
            .collect();
        let (straight_excitement, straight_intensity, straight_nausea, _) =
            ratings(&straight, 100.0);
        assert!((2.0..=3.0).contains(&straight_excitement));
        assert!((1.0..=3.0).contains(&straight_intensity));
        assert_eq!(straight_nausea, 0.0);

        let mut moderate = straight.clone();
        for sample in &mut moderate[20..40] {
            sample.speed = 20.0;
            sample.vertical_g = 3.5;
            sample.lateral_g = 1.0;
        }
        let (moderate_excitement, moderate_intensity, moderate_nausea, _) =
            ratings(&moderate, 100.0);
        assert!((3.0..=4.0).contains(&moderate_excitement));
        assert!((3.0..=5.0).contains(&moderate_intensity));
        assert_eq!(moderate_nausea, 0.0);

        let mut extreme = moderate.clone();
        for sample in &mut extreme[20..40] {
            sample.vertical_g = 5.0;
            sample.lateral_g = 3.0;
        }
        let (extreme_excitement, _, extreme_nausea, _) = ratings(&extreme, 100.0);
        assert!(extreme_excitement < moderate_excitement);
        assert!(extreme_nausea > moderate_nausea);

        let mut thrilling = straight.clone();
        for (index, sample) in thrilling.iter_mut().enumerate() {
            sample.speed = 30.0;
            sample.vertical_g = if index < 20 { -0.1 } else { 1.6 };
            sample.airtime = index < 20;
            sample.height_m = (50.0 - index as f64).max(0.0);
            sample.inverted =
                (20..30).contains(&index) || (40..50).contains(&index) || (60..70).contains(&index);
        }
        let (thrilling_excitement, _, thrilling_nausea, _) = ratings(&thrilling, 100.0);
        assert!((9.0..=10.0).contains(&thrilling_excitement));
        assert_eq!(thrilling_nausea, 0.0);

        let mut higher_force = extreme.clone();
        for sample in &mut extreme[20..40] {
            sample.vertical_g = 8.0;
            sample.lateral_g = 0.0;
        }
        for sample in &mut higher_force[20..40] {
            sample.vertical_g = 9.0;
            sample.lateral_g = 0.0;
        }
        let (excitement, _, nausea, _) = ratings(&extreme, 100.0);
        let (higher_excitement, _, higher_nausea, _) = ratings(&higher_force, 100.0);
        assert!(higher_nausea > nausea);
        assert!(
            (excitement + 0.45 * nausea - higher_excitement - 0.45 * higher_nausea).abs() < 1e-9
        );
    }

    #[test]
    fn airtime_statistics_use_requested_force_boundaries() {
        let samples: Vec<_> = [-1.2, -1.0, -0.8, 0.0, 0.1]
            .into_iter()
            .enumerate()
            .map(|(distance, vertical)| sample_for_statistics(distance as f64, 10.0, vertical))
            .collect();
        let statistics = ride_statistics(&samples, 5.0, true);
        assert!((statistics.ejector_airtime_seconds - 0.1).abs() < 1e-9);
        assert!((statistics.floater_airtime_seconds - 0.2).abs() < 1e-9);
        assert_eq!(statistics.airtime_moments, 1);
    }

    #[test]
    fn statistics_measure_airtime_forces_speed_and_inversions() {
        let samples = [
            TestSample {
                distance: 0.0,
                speed: 10.0,
                vertical_g: -1.2,
                lateral_g: 1.2,
                longitudinal_g: 0.4,
                airtime: true,
                height_m: 10.0,
                inverted: false,
                roll: 0.0,
                piece_index: 0,
            },
            TestSample {
                distance: 1.0,
                speed: 10.0,
                vertical_g: -0.2,
                lateral_g: 0.0,
                longitudinal_g: -0.7,
                airtime: true,
                height_m: 11.0,
                inverted: true,
                roll: 0.0,
                piece_index: 0,
            },
            TestSample {
                distance: 2.0,
                speed: 10.0,
                vertical_g: 4.0,
                lateral_g: -1.5,
                longitudinal_g: 1.1,
                airtime: false,
                height_m: 9.0,
                inverted: true,
                roll: 0.0,
                piece_index: 0,
            },
            TestSample {
                distance: 3.0,
                speed: 10.0,
                vertical_g: 4.2,
                lateral_g: 0.2,
                longitudinal_g: -1.4,
                airtime: false,
                height_m: 7.0,
                inverted: false,
                roll: 0.0,
                piece_index: 0,
            },
        ];
        let stats = ride_statistics(&samples, 4.0, true);
        assert!((stats.ejector_airtime_seconds - 0.1).abs() < 1e-9);
        assert!((stats.floater_airtime_seconds - 0.1).abs() < 1e-9);
        assert_eq!(stats.airtime_moments, 1);
        assert!((stats.strong_positive_g_seconds - 0.2).abs() < 1e-9);
        assert!((stats.longest_strong_positive_g_seconds - 0.2).abs() < 1e-9);
        assert!((stats.strong_lateral_g_seconds - 0.2).abs() < 1e-9);
        assert_eq!(stats.max_positive_vertical_g, 4.2);
        assert_eq!(stats.max_negative_vertical_g, -1.2);
        assert_eq!(stats.max_lateral_g, 1.5);
        assert_eq!(stats.max_forward_g, 1.1);
        assert_eq!(stats.max_braking_g, 1.4);
        assert_eq!(stats.max_speed, 10.0);
        assert_eq!(stats.average_speed, 10.0);
        assert!((stats.duration_seconds - 0.4).abs() < 1e-9);
        assert_eq!(stats.track_length, 4.0);
        assert_eq!(stats.max_height, 11.0);
        assert_eq!(stats.biggest_drop, 4.0);
        assert!((stats.steepest_drop_angle_degrees - 90.0).abs() < 1e-9);
        assert_eq!(stats.drops, 1);
        assert_eq!(stats.inversions, 1);
        assert!((stats.inverted_seconds - 0.2).abs() < 1e-9);
    }

    #[test]
    fn complete_run_counts_a_drop_at_the_circuit_seam() {
        let samples = [
            TestSample {
                distance: 0.0,
                height_m: 10.0,
                ..sample_for_statistics(10.0, 1.0, 1.0)
            },
            TestSample {
                distance: 1.0,
                height_m: 8.0,
                ..sample_for_statistics(10.0, 1.0, 1.0)
            },
            TestSample {
                distance: 2.0,
                height_m: 9.0,
                ..sample_for_statistics(10.0, 1.0, 1.0)
            },
        ];

        assert_eq!(ride_statistics(&samples, 3.0, true).drops, 1);
    }

    #[test]
    fn longest_strong_positive_run_includes_the_circuit_seam() {
        let samples = [
            TestSample {
                distance: 0.0,
                ..sample_for_statistics(0.0, 10.0, 4.0)
            },
            TestSample {
                distance: 1.0,
                ..sample_for_statistics(0.0, 10.0, 1.0)
            },
            TestSample {
                distance: 2.0,
                ..sample_for_statistics(0.0, 10.0, 4.0)
            },
        ];

        let stats = ride_statistics(&samples, 3.0, true);
        assert!((stats.strong_positive_g_seconds - 0.2).abs() < 1e-9);
        assert!((stats.longest_strong_positive_g_seconds - 0.2).abs() < 1e-9);
    }

    #[test]
    fn average_speed_is_distance_divided_by_elapsed_time() {
        let samples = [
            sample_for_statistics(0.0, 5.0, 1.0),
            sample_for_statistics(1.0, 10.0, 1.0),
            sample_for_statistics(2.0, 20.0, 1.0),
        ];

        let stats = ride_statistics(&samples, 3.0, true);
        assert!((stats.average_speed - 3.0 / 0.28).abs() < 1e-9);
    }

    fn sample_for_statistics(distance: f64, speed: f64, vertical_g: f64) -> TestSample {
        TestSample {
            distance,
            speed,
            vertical_g,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            airtime: false,
            height_m: 0.0,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        }
    }

    #[test]
    fn ratings_use_a_soft_ten_point_scale_without_an_upper_cap() {
        let sample = TestSample {
            distance: 0.0,
            speed: 30.0,
            vertical_g: 8.0,
            lateral_g: 2.0,
            longitudinal_g: 1.0,
            airtime: false,
            height_m: 0.0,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        };
        let (_, intensity, _, _) = ratings(&[sample], 10.0);
        assert!(intensity > 10.0);
        let speed_record = TestSample {
            speed: 100.0,
            vertical_g: 1.0,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            ..sample
        };
        let (excitement, _, _, _) = ratings(&[speed_record], 10.0);
        assert!(excitement > 10.0);
    }

    #[test]
    fn proximity_events_only_reward_sections_the_car_reached() {
        use crate::track::Vec3;
        use crate::track::proximity::{ProximityEvent, ProximityKind};
        let mut recorder = TestRunRecorder::new(100.0);
        record(&mut recorder, reading(0.0, 10.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(10.0, 10.0, 1.0, 0.0, 0.0));
        let mut report = recorder.finish(false, false);
        let baseline = report.excitement;
        let event = |distance| ProximityEvent {
            piece_index: 0,
            kind: ProximityKind::NearMiss,
            position: Vec3::new(distance, 0.0, 0.0),
            strength: 1.0,
            distance,
        };
        report.add_proximity_events(&[event(5.0), event(10.5), event(50.0)]);
        assert_eq!(report.proximity_events.len(), 1);
        assert!(report.excitement > baseline);
    }

    #[test]
    fn proximity_ratings_have_diminishing_returns_and_ignore_event_order() {
        use crate::track::proximity::{ProximityEvent, ProximityKind};
        let mut recorder = TestRunRecorder::new(100.0);
        record(&mut recorder, reading(0.0, 10.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(20.0, 10.0, 1.0, 0.0, 0.0));
        let baseline = recorder.finish(false, false);
        let events: Vec<_> = (1..=10)
            .map(|index| ProximityEvent {
                piece_index: index,
                kind: ProximityKind::NearMiss,
                position: crate::track::Vec3::new(index as f64, 0.0, 0.0),
                distance: index as f64,
                strength: 1.0,
            })
            .collect();
        let mut one = baseline.clone();
        one.add_proximity_events(&events[..1]);
        let mut two = one.clone();
        two.add_proximity_events(&events[1..2]);
        let mut all = baseline.clone();
        all.add_proximity_events(&events);
        let mut reversed = baseline.clone();
        reversed.add_proximity_events(&events.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(one.intensity, baseline.intensity);
        assert!(one.excitement - baseline.excitement > two.excitement - one.excitement);
        assert!((all.excitement - baseline.excitement - (1.0 - (-5.0_f64).exp())).abs() < 1e-10);
        assert_eq!(all.intensity, baseline.intensity);
        assert!((all.excitement - reversed.excitement).abs() < 1e-10);
        assert!((all.intensity - reversed.intensity).abs() < 1e-10);
    }

    #[test]
    fn simultaneous_other_train_is_recorded_for_each_car_separately() {
        use crate::track::proximity::ProximityKind;
        let circuit = demo_circuit();
        let first = reading(0.0, 12.0, 1.0, 0.0, 0.0);
        let second = reading(circuit.length() / 2.0, 12.0, 1.0, 0.0, 0.0);
        let sample = circuit.sample(first.distance);
        let other = TrainPosition {
            position: sample.position + crate::track::Vec3::new(0.0, 0.0, 3.0),
            tangent: sample.tangent,
            piece_index: 0,
            distance: 0.0,
            speed: 12.0,
        };
        let mut first_car = TestRunRecorder::new(circuit.length());
        let mut second_car = TestRunRecorder::new(circuit.length());
        first_car.record_dueling(&circuit, first, &[other]);
        second_car.record_dueling(&circuit, second, &[other]);
        assert!(
            first_car
                .finish(false, false)
                .proximity_events
                .iter()
                .any(|event| event.kind == ProximityKind::Dueling)
        );
        assert!(second_car.finish(false, false).proximity_events.is_empty());
        let mut no_other_train = TestRunRecorder::new(circuit.length());
        no_other_train.record_dueling(&circuit, first, &[]);
        assert!(
            no_other_train
                .finish(false, false)
                .proximity_events
                .is_empty()
        );
    }

    fn reading(
        distance: f64,
        speed: f64,
        vertical: f64,
        lateral: f64,
        longitudinal: f64,
    ) -> CarReading {
        CarReading {
            distance,
            speed,
            g: GForces {
                vertical,
                lateral,
                longitudinal,
            },
            airtime: vertical < 0.0,
            height_m: 0.0,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        }
    }

    fn reading_with_track(
        distance: f64,
        speed: f64,
        vertical: f64,
        lateral: f64,
        longitudinal: f64,
        height_m: f64,
        inverted: bool,
    ) -> CarReading {
        CarReading {
            height_m,
            inverted,
            ..reading(distance, speed, vertical, lateral, longitudinal)
        }
    }

    fn record(recorder: &mut TestRunRecorder, car_reading: CarReading) {
        recorder.record(&[vec![car_reading]]);
    }

    fn circuit_from_ids(ids: &[PieceId]) -> crate::physics::Circuit {
        let catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::with_capacity(ids.len());
        for id in ids {
            let piece = catalog
                .iter()
                .find(|piece| piece.id == *id)
                .expect("reference piece exists")
                .at(state)
                .expect("reference pieces connect");
            state = piece.exit;
            pieces.push(piece);
        }
        crate::physics::Circuit::new(pieces).expect("reference circuit closes")
    }

    fn simulate_reference_lap(
        circuit: crate::physics::Circuit,
        train_configuration: &crate::track::TrainConfiguration,
        config: PhysicsConfig,
        initial_speed: f64,
    ) -> TestRun {
        let mut start_distance = 0.0;
        for piece in circuit.pieces() {
            if piece.kind == PieceKind::Station {
                start_distance += piece.length() / 2.0;
                break;
            }
            start_distance += piece.length();
        }
        let mut train = Train::new(
            &circuit,
            train_configuration,
            config,
            start_distance,
            initial_speed,
        );
        let mut recorder = super::super::ConstructionTestRunRecorder::for_circuit(
            &circuit,
            usize::from(train_configuration.cars),
        );
        let mut completed = false;
        for _ in 0..60_000 {
            train.step(&circuit);
            recorder.record(&train.take_readings());
            if train.distance() - start_distance >= circuit.length() {
                completed = true;
                break;
            }
            if !train.notes().is_empty() {
                break;
            }
        }
        recorder.finish(completed, !train.notes().is_empty())
    }

    #[test]
    fn recorder_interpolates_at_each_fixed_metre() {
        let mut recorder = TestRunRecorder::new(10.0);
        record(&mut recorder, reading(0.2, 10.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(1.2, 20.0, 2.0, 1.0, 0.5));
        record(&mut recorder, reading(3.2, 30.0, 4.0, 3.0, 1.5));

        let result = recorder.finish(true, false);
        assert!(result.completed);
        assert_eq!(
            result
                .samples
                .iter()
                .map(|sample| sample.distance)
                .collect::<Vec<_>>(),
            [1.0, 2.0, 3.0]
        );
        assert_eq!(result.samples[0].speed, 18.0);
        assert_eq!(result.samples[0].vertical_g, 1.8);
        assert_eq!(result.samples[1].speed, 24.0);
        assert_eq!(result.samples[1].lateral_g, 1.8);
        assert_eq!(result.samples[2].longitudinal_g, 1.4);
    }

    #[test]
    fn recorder_interpolates_roll_and_uses_the_nearer_piece() {
        let first = CarReading {
            roll: 0.0,
            piece_index: 2,
            ..reading(0.0, 10.0, 1.0, 0.0, 0.0)
        };
        let second = CarReading {
            roll: 2.0,
            piece_index: 3,
            ..reading(2.0, 10.0, 1.0, 0.0, 0.0)
        };

        let before_boundary = interpolate_reading(first, second, 0.25);
        let after_boundary = interpolate_reading(first, second, 0.75);

        assert_eq!(before_boundary.roll, 0.5);
        assert_eq!(before_boundary.piece_index, 2);
        assert_eq!(after_boundary.roll, 1.5);
        assert_eq!(after_boundary.piece_index, 3);
    }

    #[test]
    fn recorder_interpolates_across_the_circuit_seam() {
        let mut recorder = TestRunRecorder::new(10.0);
        record(&mut recorder, reading(9.5, 10.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(0.5, 20.0, -1.0, 2.0, 0.0));

        let result = recorder.finish(true, false);
        let seam = result
            .samples
            .iter()
            .find(|sample| sample.distance == 0.0)
            .unwrap();
        assert_eq!(seam.speed, 15.0);
        assert_eq!(seam.vertical_g, 0.0);
        assert_eq!(seam.lateral_g, 1.0);
        assert!(!seam.airtime);
    }

    #[test]
    fn uncomfortable_forces_raise_intensity_and_nausea_and_reduce_excitement() {
        fn report(vertical: f64, lateral: f64, longitudinal: f64) -> TestRun {
            let mut recorder = TestRunRecorder::new(10.0);
            record(
                &mut recorder,
                reading(0.0, 20.0, vertical, lateral, longitudinal),
            );
            record(
                &mut recorder,
                reading(1.0, 20.0, vertical, lateral, longitudinal),
            );
            recorder.finish(true, false)
        }

        let comfortable = report(1.0, 0.1, 0.0);
        let uncomfortable = report(5.0, 3.0, 2.0);
        assert!(uncomfortable.intensity > comfortable.intensity);
        assert!(uncomfortable.nausea > comfortable.nausea);
        assert!(uncomfortable.excitement < comfortable.excitement);
        assert!(uncomfortable.excitement.is_finite());
        assert!(uncomfortable.intensity.is_finite());
        assert!(uncomfortable.nausea.is_finite());
    }

    #[test]
    fn unfinished_circuits_and_stalls_return_friendly_notes() {
        let unfinished = TestRun::unfinished_circuit();
        assert!(!unfinished.completed);
        assert!(unfinished.samples.is_empty());
        assert_eq!(unfinished.notes.len(), 1);
        assert!(unfinished.notes[0].to_lowercase().contains("connect"));

        let mut recorder = TestRunRecorder::new(10.0);
        record(&mut recorder, reading(4.0, 10.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(6.25, 0.0, 1.0, 0.0, 0.0));
        let stalled = recorder.finish(false, true);
        assert!(!stalled.completed);
        assert_eq!(stalled.furthest_distance, 2.25);
        assert!(stalled.notes.iter().any(|note| note.contains("2.2 m")));
        assert!(
            stalled
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("rolled back"))
        );
    }

    #[test]
    fn drops_and_inversions_raise_excitement() {
        fn excitement(readings: &[CarReading]) -> f64 {
            let mut recorder = TestRunRecorder::new(10.0);
            for reading in readings {
                record(&mut recorder, *reading);
            }
            recorder.finish(true, false).excitement
        }

        let flat = [
            reading_with_track(0.0, 20.0, 1.0, 0.1, 0.0, 10.0, false),
            reading_with_track(1.0, 20.0, 1.0, 0.1, 0.0, 10.0, false),
            reading_with_track(2.0, 20.0, 1.0, 0.1, 0.0, 10.0, false),
        ];
        let drop_and_inversion = [
            reading_with_track(0.0, 20.0, 1.0, 0.1, 0.0, 30.0, false),
            reading_with_track(1.0, 20.0, 1.0, 0.1, 0.0, 0.0, true),
            reading_with_track(2.0, 20.0, 1.0, 0.1, 0.0, 0.0, true),
            reading_with_track(3.0, 20.0, 1.0, 0.1, 0.0, 10.0, false),
        ];
        assert!(excitement(&drop_and_inversion) > excitement(&flat));
    }

    #[test]
    fn strong_lateral_and_sustained_high_g_produce_advisory_notes() {
        let mut recorder = TestRunRecorder::new(72.0);
        for tick in 0..800 {
            record(
                &mut recorder,
                reading_with_track(
                    f64::from(tick) * 18.0 * super::super::TIME_STEP,
                    18.0,
                    5.0,
                    2.5,
                    1.0,
                    0.0,
                    false,
                ),
            );
        }
        let report = recorder.finish(true, false);
        assert!(report.intensity >= 7.0);
        assert!(report.nausea > 0.0);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("sideways"))
        );
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("high forces"))
        );

        let mut dispersed = TestRunRecorder::new(100.0);
        for distance in 0..=100 {
            let is_force_peak = distance % 10 == 0;
            record(
                &mut dispersed,
                reading(
                    f64::from(distance),
                    18.0,
                    if is_force_peak { 5.0 } else { 1.0 },
                    if is_force_peak { 2.5 } else { 0.0 },
                    0.0,
                ),
            );
        }
        let dispersed_report = dispersed.finish(true, false);
        assert!(
            !dispersed_report
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("in a row under high forces"))
        );
    }

    #[test]
    fn long_slow_layout_gets_a_friendly_note() {
        let mut recorder = TestRunRecorder::new(600.0);
        for distance in 0..=600 {
            record(
                &mut recorder,
                reading(
                    f64::from(distance),
                    if distance < 120 { 2.0 } else { 20.0 },
                    1.0,
                    0.0,
                    0.0,
                ),
            );
        }
        let report = recorder.finish(true, false);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("long slow stretches"))
        );
    }

    #[test]
    fn reference_oval_and_drop_loop_produce_distinct_ratings() {
        let mut train_configuration = hybrid_model().train;
        train_configuration.cars = 1;
        train_configuration.length_metres = 4.0;
        let gentle_circuit = circuit_from_ids(&[
            PieceId::Station,
            PieceId::LevelToGentleUp,
            PieceId::GentleUpToLevel,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::LevelToGentleDown,
            PieceId::GentleDownToLevel,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ]);
        let gentle = simulate_reference_lap(
            gentle_circuit,
            &train_configuration,
            PhysicsConfig {
                rolling_resistance: 0.0,
                drag_coefficient: 0.0,
                powered_sections: false,
                ..PhysicsConfig::default()
            },
            20.0,
        );
        let drop_and_loop = simulate_reference_lap(
            demo_circuit(),
            &train_configuration,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            2.0,
        );

        assert!(gentle.completed, "gentle notes={:?}", gentle.notes);
        assert!(
            drop_and_loop.completed,
            "drop and loop notes={:?}",
            drop_and_loop.notes
        );
        assert!(!gentle.samples.iter().any(|sample| sample.inverted));
        assert!(drop_and_loop.samples.iter().any(|sample| sample.inverted));
        assert!(
            (5.0..=7.0).contains(&drop_and_loop.excitement),
            "demo excitement was {}",
            drop_and_loop.excitement
        );
        assert!((3.0..=6.0).contains(&drop_and_loop.intensity));
        assert!(drop_and_loop.excitement > gentle.excitement);
        assert!(drop_and_loop.samples.len() > gentle.samples.len());
    }

    #[test]
    fn demo_lap_records_piece_indices_matching_the_circuit() {
        let circuit = demo_circuit();
        let mut train_configuration = hybrid_model().train;
        train_configuration.cars = 1;
        train_configuration.length_metres = 4.0;
        let report = simulate_reference_lap(
            circuit.clone(),
            &train_configuration,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            2.0,
        );

        assert!(report.completed, "demo lap notes={:?}", report.notes);
        for sample in &report.samples {
            assert_eq!(sample.piece_index, circuit.piece_index(sample.distance));
        }
    }

    #[test]
    fn recorder_preserves_unwrapped_roll_through_a_heartline_roll() {
        let roll_piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HeartlineRoll)
            .expect("catalog has a heartline roll");
        let mut recorder = TestRunRecorder::new(roll_piece.length());
        for step in 0..=200 {
            let distance = roll_piece.length() * f64::from(step) / 200.0;
            let track_sample = roll_piece.sample(distance);
            record(
                &mut recorder,
                CarReading {
                    height_m: track_sample.position.z,
                    inverted: track_sample.up.z < 0.0,
                    roll: track_sample.roll,
                    piece_index: 0,
                    ..reading(distance, 10.0, 1.0, 0.0, 0.0)
                },
            );
        }

        let report = recorder.finish(true, false);
        let roll_samples: Vec<_> = report.samples.iter().map(|sample| sample.roll).collect();
        assert!(roll_samples.len() > 2);
        assert!(roll_samples.windows(2).all(|pair| pair[1] >= pair[0]));
        assert!(roll_samples.last().unwrap() - roll_samples[0] > std::f64::consts::PI);
    }

    #[test]
    fn hybrid_launches_complete_and_produce_ride_ratings() {
        use PieceId::*;

        let circuit = circuit_from_ids(&[
            Station,
            Straight,
            Straight,
            LaunchShort,
            LaunchShort,
            Straight,
            Straight,
            TurnLargeEntry,
            TurnLargeExit,
            Straight,
            Straight,
            LaunchShort,
            LaunchShort,
            Straight,
            Straight,
            Straight,
            TurnLargeEntry,
            TurnLargeExit,
        ]);
        let mut model = hybrid_model();
        model.train.cars = 1;
        model.train.length_metres = 4.0;
        let report = simulate_reference_lap(circuit, &model.train, PhysicsConfig::default(), 0.0);

        assert!(report.completed, "launch ride notes={:?}", report.notes);
        assert!(!report.samples.is_empty());
        assert!(report.samples.iter().any(|sample| sample.speed > 4.0));
        assert_eq!(report.statistics.launches, 2);
        assert!(report.statistics.launch_peak_speed > 4.0);
        for rating in [report.excitement, report.intensity, report.nausea] {
            assert!(rating.is_finite() && rating >= 0.0);
        }
        assert!(
            report
                .notes
                .iter()
                .all(|note| !note.contains("could not finish the circuit")),
            "launch ride notes={:?}",
            report.notes
        );
    }

    #[test]
    fn reference_lap_records_a_multi_car_train() {
        let configuration = hybrid_model().train;
        assert!(configuration.cars > 1);
        let report = simulate_reference_lap(
            demo_circuit(),
            &configuration,
            PhysicsConfig::default(),
            2.0,
        );
        assert!(!report.samples.is_empty());
        assert!(report.statistics.max_speed > 0.0);
    }

    #[test]
    fn stronger_hybrid_launches_raise_speed_and_rescue_a_rollback() {
        let base_circuit = super::super::demo_circuit();
        let train = hybrid_model().train;
        let with_launch_speed = |speed: f64| {
            let mut pieces = base_circuit.pieces().to_vec();
            for piece in &mut pieces {
                if piece.kind == PieceKind::Lift {
                    piece.kind = PieceKind::Launch;
                    piece.launch_target_speed = Some(speed);
                }
            }
            crate::physics::Circuit::new(pieces).unwrap()
        };
        let low = simulate_reference_lap(
            with_launch_speed(0.0),
            &train,
            PhysicsConfig::default(),
            0.0,
        );
        let high = simulate_reference_lap(
            with_launch_speed(35.0),
            &train,
            PhysicsConfig::default(),
            0.0,
        );

        let low_peak_speed = low
            .samples
            .iter()
            .map(|sample| sample.speed)
            .fold(0.0, f64::max);
        let high_peak_speed = high
            .samples
            .iter()
            .map(|sample| sample.speed)
            .fold(0.0, f64::max);
        assert!(
            high_peak_speed > low_peak_speed,
            "low={low_peak_speed} high={high_peak_speed}"
        );
        assert!(!low.completed, "low speed notes={:?}", low.notes);
        assert!(
            low.notes.iter().any(|note| note.contains("rolled back")),
            "low speed notes={:?}",
            low.notes
        );
        assert!(high.completed, "high speed notes={:?}", high.notes);
    }

    #[test]
    fn simulated_stall_note_reports_the_forward_distance_before_recovery() {
        let circuit = circuit_from_ids(&[
            PieceId::Station,
            PieceId::LevelToGentleUp,
            PieceId::GentleUpToLevel,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::LevelToGentleDown,
            PieceId::GentleDownToLevel,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ]);
        let mut train_configuration = hybrid_model().train;
        train_configuration.cars = 1;
        train_configuration.length_metres = 4.0;
        let start_distance = circuit.pieces[..2]
            .iter()
            .map(|piece| piece.length())
            .sum::<f64>()
            + 5.0;
        let mut train = Train::new(
            &circuit,
            &train_configuration,
            PhysicsConfig {
                rolling_resistance: 0.8,
                powered_sections: false,
                ..PhysicsConfig::default()
            },
            start_distance,
            0.1,
        );
        let mut recorder = TestRunRecorder::for_circuit(&circuit);
        for _ in 0..100 {
            train.step(&circuit);
            recorder.record(&train.take_readings());
            if !train.notes().is_empty() {
                break;
            }
        }

        let report = recorder.finish(false, !train.notes().is_empty());
        assert!(!report.completed);
        assert!(report.furthest_distance < 2.0);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("after travelling") && note.contains("m"))
        );
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.to_lowercase().contains("rolled back"))
        );
    }

    #[test]
    fn a_successful_run_without_rollback_has_no_notes() {
        let mut recorder = TestRunRecorder::new(10.0);
        record(&mut recorder, reading(0.0, 5.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(1.0, 6.0, 1.0, 0.0, 0.0));
        let result = recorder.finish(true, false);
        assert!(result.notes.is_empty());
    }

    #[test]
    fn rated_piece_entry_outside_speed_window_is_reported_once() {
        let mut recorder = TestRunRecorder::new(20.0);
        recorder.piece_starts = vec![0.0, 10.0];
        recorder.rated_pieces = vec![
            None,
            Some(RatedPieceSpeedWindow {
                name: "Loop",
                minimum_entry: 24.7,
                maximum_entry: 27.5,
            }),
        ];
        recorder.noted_rated_pieces = vec![false; 2];
        record(&mut recorder, reading(9.9, 35.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(10.1, 35.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(19.9, 35.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(0.1, 35.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(10.1, 35.0, 1.0, 0.0, 0.0));

        let report = recorder.finish(true, false);
        assert_eq!(report.notes.len(), 1, "notes: {:?}", report.notes);
        assert_eq!(
            report.notes[0],
            "The train entered Loop at 35.0 m/s, above its rated speed band of 24.7–27.5 m/s."
        );
    }

    #[test]
    fn rated_piece_entry_below_speed_window_is_reported() {
        let mut recorder = TestRunRecorder::new(20.0);
        recorder.piece_starts = vec![0.0, 10.0];
        recorder.rated_pieces = vec![
            None,
            Some(RatedPieceSpeedWindow {
                name: "Loop",
                minimum_entry: 24.7,
                maximum_entry: 27.5,
            }),
        ];
        recorder.noted_rated_pieces = vec![false; 2];
        record(&mut recorder, reading(9.9, 2.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(10.1, 2.0, 1.0, 0.0, 0.0));

        let report = recorder.finish(false, false);
        assert_eq!(
            report.notes[0],
            "The train entered Loop at 2.0 m/s, below its rated speed band of 24.7–27.5 m/s."
        );
    }

    #[test]
    fn rated_piece_note_uses_interpolated_speed_at_the_entry_boundary() {
        let mut recorder = TestRunRecorder::new(20.0);
        recorder.piece_starts = vec![0.0, 10.0];
        recorder.rated_pieces = vec![
            None,
            Some(RatedPieceSpeedWindow {
                name: "Loop",
                minimum_entry: 24.7,
                maximum_entry: 27.5,
            }),
        ];
        recorder.noted_rated_pieces = vec![false; 2];
        record(&mut recorder, reading(9.9, 20.0, 1.0, 0.0, 0.0));
        record(&mut recorder, reading(10.3, 30.0, 1.0, 0.0, 0.0));

        let report = recorder.finish(true, false);
        assert_eq!(
            report.notes[0],
            "The train entered Loop at 22.5 m/s, below its rated speed band of 24.7–27.5 m/s."
        );
    }

    #[test]
    fn score_breakdown_sums_to_report_after_proximity_update() {
        let mut recorder = TestRunRecorder::new(10.0);
        for distance in 0..10 {
            record(
                &mut recorder,
                reading(distance as f64, 10.0, -1.0, 0.0, 0.0),
            );
        }
        let mut report = recorder.finish(true, false);
        report.add_proximity_events(&[crate::track::proximity::ProximityEvent {
            piece_index: 0,
            kind: crate::track::proximity::ProximityKind::NearMiss,
            position: crate::track::Vec3::new(0.0, 0.0, 0.0),
            strength: 1.0,
            distance: 1.0,
        }]);
        for (score, contributions) in [
            (report.excitement, &report.breakdown.excitement),
            (report.intensity, &report.breakdown.intensity),
            (report.nausea, &report.breakdown.nausea),
        ] {
            assert!(
                (score - contributions.iter().map(|term| term.value).sum::<f64>()).abs() < 1e-10
            );
        }
    }

    #[test]
    fn theming_recomputes_every_car_before_averaging_the_ride_score() {
        fn report(vertical_g: f64) -> TestRun {
            let mut recorder = TestRunRecorder::new(10.0);
            record(&mut recorder, reading(0.0, 20.0, vertical_g, 0.0, 0.0));
            record(&mut recorder, reading(1.0, 20.0, vertical_g, 0.0, 0.0));
            recorder.finish(true, false)
        }

        let first_car = report(1.0);
        let second_car = report(3.0);
        let baseline_average = (first_car.excitement + second_car.excitement) / 2.0;
        let mut ride_report = first_car.clone();
        ride_report.average_car_ratings(&[first_car, second_car]);
        assert!((ride_report.excitement - baseline_average).abs() < 1e-10);

        ride_report.set_theming(0.5);

        let theming_contribution = ride_report
            .breakdown
            .excitement
            .iter()
            .find(|contribution| contribution.term == Term::Theming)
            .expect("theming contribution exists")
            .value;
        assert!((ride_report.excitement - baseline_average - 0.4).abs() < 1e-10);
        assert!((theming_contribution - 0.4).abs() < 1e-10);
    }

    #[test]
    fn recorder_rates_force_against_its_model_limits() {
        let mut lenient = TestRunRecorder::new(100.0);
        let mut strict = TestRunRecorder::new(100.0);
        strict.force_limits.minimum_vertical_g = -1.0;
        for tick in 0..800 {
            let reading = reading(tick as f64 * 0.05, 10.0, -1.2, 0.0, 0.0);
            record(&mut lenient, reading);
            record(&mut strict, reading);
        }
        let lenient = lenient.finish(false, false);
        let strict = strict.finish(false, false);
        assert!(lenient.excitement > strict.excitement);
        assert!(strict.nausea > lenient.nausea);
    }

    #[test]
    fn report_hash_covers_breakdown_and_recomputation_inputs() {
        let mut recorder = TestRunRecorder::new(10.0);
        for distance in 0..10 {
            record(&mut recorder, reading(distance as f64, 10.0, 1.0, 0.0, 0.0));
        }
        let report = recorder.finish(true, false);
        let digest = |report: &TestRun| {
            let mut hash = StateHasher::new();
            report.hash_into(&mut hash);
            hash.finish()
        };
        let mut changed_breakdown = report.clone();
        changed_breakdown.breakdown.excitement[0].measure += 1.0;
        assert_ne!(digest(&report), digest(&changed_breakdown));
        let mut changed_input = report.clone();
        changed_input.rating_inputs[0]
            .force_limits
            .minimum_vertical_g -= 0.1;
        assert_ne!(digest(&report), digest(&changed_input));
        let mut changed_theming = report.clone();
        changed_theming.set_theming(0.5);
        assert_ne!(digest(&report), digest(&changed_theming));
    }
}
