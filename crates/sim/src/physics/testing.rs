use super::{CarReading, Circuit};
use crate::state_hash::StateHasher;
use crate::track::proximity::{ProximityEvent, TrainPosition, detect_dueling};

const HIGH_FORCE_SECTION_METRES: f64 = 8.0;
const LONG_SLOW_SECTION_METRES: f64 = 100.0;
const HIGH_FORCE_VERTICAL_G: f64 = 3.0;
const HIGH_FORCE_NEGATIVE_VERTICAL_G: f64 = -1.0;
const HIGH_FORCE_LATERAL_G: f64 = 2.0;
const HIGH_FORCE_LONGITUDINAL_G: f64 = 1.5;
const SLOW_SPEED_METRES_PER_SECOND: f64 = 8.0;
const DROP_SCORE_CALIBRATION_METRES: f64 = 30.0;
const INVERSION_SCORE_CALIBRATION_PASSAGES: f64 = 3.0;
const VERTICAL_DISCOMFORT_THRESHOLD_G: f64 = 3.5;
const NEGATIVE_VERTICAL_DISCOMFORT_THRESHOLD_G: f64 = 0.5;
const LATERAL_DISCOMFORT_THRESHOLD_G: f64 = 1.5;
const LONGITUDINAL_DISCOMFORT_THRESHOLD_G: f64 = 1.5;
const DISCOMFORT_VERTICAL_SCALE_G: f64 = 2.0;
const DISCOMFORT_NEGATIVE_VERTICAL_SCALE_G: f64 = 1.5;
const DISCOMFORT_LATERAL_SCALE_G: f64 = 1.5;
const DISCOMFORT_LONGITUDINAL_SCALE_G: f64 = 1.5;
const MODERATE_FORCE_CALIBRATION: f64 = 7.0;
const INTENSITY_BASE_SCORE: f64 = 20.0;
const INTENSITY_VERTICAL_WEIGHT: f64 = 14.0;
const INTENSITY_LATERAL_WEIGHT: f64 = 18.0;
const INTENSITY_LONGITUDINAL_WEIGHT: f64 = 10.0;
const INTENSITY_AIRTIME_WEIGHT: f64 = 8.0;
const INTENSITY_HIGH_FORCE_WEIGHT: f64 = 10.0;
const HIGH_FORCE_SCORE_CALIBRATION_METRES: f64 = 20.0;
const NAUSEA_SCORE_SCALE: f64 = 100.0;
const EXCITEMENT_BASE_SCORE: f64 = 10.0;
const EXCITEMENT_SPEED_WEIGHT: f64 = 1.6;
const EXCITEMENT_AIRTIME_WEIGHT: f64 = 25.0;
const EXCITEMENT_MODERATE_FORCE_WEIGHT: f64 = 24.0;
const EXCITEMENT_DROP_WEIGHT: f64 = 15.0;
const EXCITEMENT_INVERSION_WEIGHT: f64 = 15.0;
const EXCITEMENT_NAUSEA_WEIGHT: f64 = 0.45;

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
}

/// A completed or interrupted test run, including advisory ride ratings.
#[derive(Clone, Debug, PartialEq)]
pub struct TestRun {
    pub samples: Vec<TestSample>,
    pub proximity_events: Vec<ProximityEvent>,
    pub excitement: f64,
    pub intensity: f64,
    pub nausea: f64,
    pub notes: Vec<String>,
    pub completed: bool,
    pub furthest_distance: f64,
    visited_ranges: Vec<(f64, f64)>,
    base_excitement: f64,
    base_intensity: f64,
}

impl TestRun {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"TestRun-v1");
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
        hash.write_len(self.notes.len());
        for note in &self.notes {
            hash.write_bytes(note.as_bytes());
        }
        hash.write_bool(self.completed);
        hash.write_f64(self.furthest_distance);
        write_ranges(hash, &self.visited_ranges);
        hash.write_f64(self.base_excitement);
        hash.write_f64(self.base_intensity);
    }

    pub fn unfinished_circuit() -> Self {
        Self {
            samples: Vec::new(),
            proximity_events: Vec::new(),
            excitement: 0.0,
            intensity: 0.0,
            nausea: 0.0,
            notes: vec![
                "The circuit is unfinished. Connect the track into a complete loop before testing the train."
                    .to_owned(),
            ],
            completed: false,
            furthest_distance: 0.0,
            visited_ranges: Vec::new(),
            base_excitement: 0.0,
            base_intensity: 0.0,
        }
    }

    /// Add events at positions actually sampled by this car.
    pub fn add_proximity_events(&mut self, events: &[ProximityEvent]) {
        for event in events {
            if self
                .visited_ranges
                .iter()
                .any(|(start, end)| event.distance >= *start && event.distance <= *end)
            {
                self.insert_proximity_event(*event);
            }
        }
        self.update_proximity_ratings();
    }

    fn insert_proximity_event(&mut self, event: ProximityEvent) {
        if let Some(existing) = self.proximity_events.iter_mut().find(|existing| {
            existing.kind == event.kind && existing.piece_index == event.piece_index
        }) {
            if event.strength > existing.strength {
                *existing = event;
            }
        } else {
            self.proximity_events.push(event);
        }
    }

    fn update_proximity_ratings(&mut self) {
        let mut strengths: Vec<_> = self
            .proximity_events
            .iter()
            .map(|event| event.strength.clamp(0.0, 1.0))
            .collect();
        strengths.sort_by(f64::total_cmp);
        let total: f64 = strengths.iter().sum();
        let contribution = total / (total + 4.0);
        self.excitement = (self.base_excitement + 8.0 * contribution).min(100.0);
        self.intensity = (self.base_intensity + 2.0 * contribution).min(100.0);
    }
}

/// Turns per-tick train readings into one interpolated sample per track metre.
pub struct TestRunRecorder {
    circuit_length: f64,
    piece_starts: Vec<f64>,
    rated_pieces: Vec<Option<RatedPieceSpeedWindow>>,
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
        hash.write_tag(b"TestRunRecorder-v2");
        hash.write_f64(self.circuit_length);
        hash.write_len(self.piece_starts.len());
        for &piece_start in &self.piece_starts {
            hash.write_f64(piece_start);
        }
        hash.write_len(self.rated_pieces.len());
        for rated_piece in &self.rated_pieces {
            hash.write_option(*rated_piece, |hash, window| {
                hash.write_bytes(window.name.as_bytes());
                hash.write_f64(window.minimum_entry);
                hash.write_f64(window.maximum_entry);
            });
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
            piece_starts: Vec::new(),
            rated_pieces: Vec::new(),
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
        let mut recorder = Self::new(circuit.length());
        recorder.piece_starts.clone_from(&circuit.starts);
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
        recorder.noted_rated_pieces = vec![false; circuit.pieces.len()];
        recorder
    }

    /// Records the front car from each physics step in a batch.
    ///
    /// Readings are sampled at 200 Hz, so interpolation at crossed metre marks
    /// stays close to the simulated motion while keeping the overlay compact.
    pub fn record(&mut self, frames: &[Vec<CarReading>]) {
        for frame in frames {
            let Some(reading) = frame.first().copied() else {
                continue;
            };
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
        let (excitement, intensity, nausea, mut notes) = ratings(&samples, circuit_length);
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
        let mut report = TestRun {
            samples,
            proximity_events: Vec::new(),
            excitement,
            intensity,
            nausea,
            notes,
            completed,
            furthest_distance: self.furthest_distance,
            visited_ranges: self.visited_ranges,
            base_excitement: excitement,
            base_intensity: intensity,
        };
        for event in self.dueling_events {
            report.insert_proximity_event(event);
        }
        report.update_proximity_ratings();
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
            self.samples[index] = Some(TestSample {
                distance,
                speed: reading.speed,
                vertical_g: reading.g.vertical,
                lateral_g: reading.g.lateral,
                longitudinal_g: reading.g.longitudinal,
                airtime: reading.g.vertical < 0.0,
                height_m: reading.height_m,
                inverted: reading.inverted,
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
    }
}

/// Scores use distance-weighted averages from the one-metre record and clamp to
/// 0–100. Intensity is `20 + 14*mean(|vertical_g-1|) + 18*mean(|lateral_g|) +
/// 10*mean(|longitudinal_g|) + 8*airtime_fraction + 10*min(high_force_run/20m,1)`.
/// Per-sample nausea exposure is `max(vertical_g-3.5,0)/2 +
/// max(-vertical_g-0.5,0)/1.5 + max(|lateral_g|-1.5,0)/1.5 +
/// max(|longitudinal_g|-1.5,0)/1.5`; nausea is 100 times its mean. Excitement is
/// `10 + 1.6*mean(speed_m/s) + 25*airtime_fraction + 24*moderate_force +
/// 15*min(max_downhill_drop/30m,1) + 15*min(inversion_passages/3,1) - 0.45*nausea`,
/// where `moderate_force = min(mean(|vertical_g-1| + |lateral_g| +
/// |longitudinal_g|)/7,1)`. These are feedback scores, not safety limits.
/// Notes require a contiguous 8m high-force section or 100m below 8m/s.
// Proximity adds 8 * strength / (strength + 4) excitement and 2 * strength /
// (strength + 4) intensity, where strength is the sum of event strengths.
fn ratings(samples: &[TestSample], circuit_length: f64) -> (f64, f64, f64, Vec<String>) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0, Vec::new());
    }
    let count = samples.len() as f64;
    let mean = |value: fn(&TestSample) -> f64| samples.iter().map(value).sum::<f64>() / count;
    let mean_speed = mean(|sample| sample.speed);
    let airtime_fraction = samples.iter().filter(|sample| sample.airtime).count() as f64 / count;
    let mean_vertical_deviation = mean(|sample| (sample.vertical_g - 1.0).abs());
    let mean_lateral_load = mean(|sample| sample.lateral_g.abs());
    let mean_longitudinal_load = mean(|sample| sample.longitudinal_g.abs());
    let maximum_lateral_load = samples
        .iter()
        .map(|sample| sample.lateral_g.abs())
        .fold(0.0, f64::max);
    let high_force_run = longest_contiguous_section(samples, circuit_length, |sample| {
        sample.vertical_g > HIGH_FORCE_VERTICAL_G
            || sample.vertical_g < HIGH_FORCE_NEGATIVE_VERTICAL_G
            || sample.lateral_g.abs() > HIGH_FORCE_LATERAL_G
            || sample.longitudinal_g.abs() > HIGH_FORCE_LONGITUDINAL_G
    });
    let long_slow_run = longest_contiguous_section(samples, circuit_length, |sample| {
        sample.speed < SLOW_SPEED_METRES_PER_SECOND
    });
    let maximum_drop = largest_downhill_drop(samples);
    let inversion_passages = count_inversion_passages(samples);
    let drop_score = (maximum_drop / DROP_SCORE_CALIBRATION_METRES).clamp(0.0, 1.0);
    let inversion_score =
        (inversion_passages as f64 / INVERSION_SCORE_CALIBRATION_PASSAGES).clamp(0.0, 1.0);
    let discomfort = mean(|sample| {
        (sample.vertical_g - VERTICAL_DISCOMFORT_THRESHOLD_G).max(0.0) / DISCOMFORT_VERTICAL_SCALE_G
            + (-sample.vertical_g - NEGATIVE_VERTICAL_DISCOMFORT_THRESHOLD_G).max(0.0)
                / DISCOMFORT_NEGATIVE_VERTICAL_SCALE_G
            + (sample.lateral_g.abs() - LATERAL_DISCOMFORT_THRESHOLD_G).max(0.0)
                / DISCOMFORT_LATERAL_SCALE_G
            + (sample.longitudinal_g.abs() - LONGITUDINAL_DISCOMFORT_THRESHOLD_G).max(0.0)
                / DISCOMFORT_LONGITUDINAL_SCALE_G
    });
    let moderate_force = (mean(|sample| {
        (sample.vertical_g - 1.0).abs() + sample.lateral_g.abs() + sample.longitudinal_g.abs()
    }) / MODERATE_FORCE_CALIBRATION)
        .clamp(0.0, 1.0);
    let intensity = (INTENSITY_BASE_SCORE
        + INTENSITY_VERTICAL_WEIGHT * mean_vertical_deviation
        + INTENSITY_LATERAL_WEIGHT * mean_lateral_load
        + INTENSITY_LONGITUDINAL_WEIGHT * mean_longitudinal_load
        + INTENSITY_AIRTIME_WEIGHT * airtime_fraction
        + INTENSITY_HIGH_FORCE_WEIGHT
            * (high_force_run / HIGH_FORCE_SCORE_CALIBRATION_METRES).clamp(0.0, 1.0))
    .clamp(0.0, 100.0);
    let nausea = (NAUSEA_SCORE_SCALE * discomfort).clamp(0.0, 100.0);
    let excitement = (EXCITEMENT_BASE_SCORE
        + EXCITEMENT_SPEED_WEIGHT * mean_speed
        + EXCITEMENT_AIRTIME_WEIGHT * airtime_fraction
        + EXCITEMENT_MODERATE_FORCE_WEIGHT * moderate_force
        + EXCITEMENT_DROP_WEIGHT * drop_score
        + EXCITEMENT_INVERSION_WEIGHT * inversion_score
        - EXCITEMENT_NAUSEA_WEIGHT * nausea)
        .clamp(0.0, 100.0);
    let mut notes = Vec::new();
    if mean_lateral_load >= 1.5 || maximum_lateral_load >= 2.5 {
        notes.push(
            "Some turns pull riders sideways strongly. More banking or wider turns can improve comfort."
                .to_owned(),
        );
    }
    if high_force_run >= HIGH_FORCE_SECTION_METRES {
        notes.push(
            "Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements."
                .to_owned(),
        );
    }
    if intensity >= 70.0 {
        notes.push(
            "This is a very intense ride. A few lower-force sections can make it more comfortable."
                .to_owned(),
        );
    }
    if long_slow_run >= LONG_SLOW_SECTION_METRES {
        notes.push(
            "This layout has at least 100 m in a row below 8 m/s. A taller first drop or a faster section could make it feel livelier."
                .to_owned(),
        );
    }
    (excitement, intensity, nausea, notes)
}

fn longest_contiguous_section(
    samples: &[TestSample],
    circuit_length: f64,
    matches: impl Fn(&TestSample) -> bool,
) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut previous_distance = 0.0;
    let mut previous_matches = false;
    let mut current_length = 0.0;
    let mut longest_length: f64 = 0.0;
    for index in 0..samples.len() * 2 {
        let sample = &samples[index % samples.len()];
        let distance_delta = if sample.distance > previous_distance || index == 0 {
            sample.distance - previous_distance
        } else {
            circuit_length - previous_distance + sample.distance
        };
        let is_contiguous = index > 0 && distance_delta > 0.0 && distance_delta <= 1.000_001;
        let is_match = matches(sample);
        if is_match {
            current_length = if previous_matches && is_contiguous {
                current_length + distance_delta
            } else {
                1.0
            };
            longest_length = longest_length.max(current_length);
        } else {
            current_length = 0.0;
        }
        previous_distance = sample.distance;
        previous_matches = is_match;
    }
    longest_length.min(samples.len() as f64)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{CarReading, GForces, PhysicsConfig, Train, demo_circuit};
    use crate::track::{PieceId, PieceKind, TrackState, catalog, hybrid_model};

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
        assert!(one.intensity > baseline.intensity);
        assert!(one.excitement - baseline.excitement > two.excitement - one.excitement);
        assert!(all.excitement - baseline.excitement < 8.0);
        assert!(all.intensity - baseline.intensity < 2.0);
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
        let mut recorder = TestRunRecorder::for_circuit(&circuit);
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
        assert!(
            (0.0..=100.0).contains(&uncomfortable.excitement)
                && (0.0..=100.0).contains(&uncomfortable.intensity)
                && (0.0..=100.0).contains(&uncomfortable.nausea)
        );
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
        let mut recorder = TestRunRecorder::new(10.0);
        for distance in 0..=10 {
            record(
                &mut recorder,
                reading_with_track(f64::from(distance), 18.0, 5.0, 2.5, 1.0, 0.0, false),
            );
        }
        let report = recorder.finish(true, false);
        assert!(report.intensity >= 70.0);
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
                .any(|note| note.to_lowercase().contains("at least 100 m in a row"))
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
        assert!(drop_and_loop.excitement > gentle.excitement);
        assert!(drop_and_loop.samples.len() > gentle.samples.len());
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
        for rating in [report.excitement, report.intensity, report.nausea] {
            assert!(rating.is_finite() && (0.0..=100.0).contains(&rating));
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
}
