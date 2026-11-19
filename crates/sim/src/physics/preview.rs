use super::{
    Circuit, ForceEvaluator, PhysicsConfig, TIME_STEP, TestRun, TestRunRecorder, TestSample, Train,
    candidate_force_peak_summary,
};
use crate::physics::ForcePeakValues;
use crate::state_hash::StateHasher;
use crate::track::{CoasterModel, Piece, PieceKind};

const PREVIEW_MAX_SECONDS: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewStatus {
    ReachedEnd,
    Stalled,
    TimedOut,
}

pub struct TrackPreview {
    pub report: TestRun,
    pub status: PreviewStatus,
    /// Front-car speed interpolated at the exact open-track endpoint when reached.
    pub end_speed_mps: Option<f64>,
    pub force_limit_exceeded: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateCarForces {
    pub car_index: usize,
    pub peaks: ForcePeakValues,
    pub sustained_minimum_vertical_g: Option<f64>,
    pub within_limits: bool,
    #[cfg(test)]
    pub(crate) minimum_vertical_sample: CandidateForceSample,
    #[cfg(test)]
    pub(crate) maximum_longitudinal_sample: CandidateForceSample,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CandidateForceSample {
    pub(crate) distance_m: f64,
    pub(crate) local_distance_m: f64,
    pub(crate) speed_mps: f64,
    pub(crate) height_m: f64,
    pub(crate) force_g: f64,
    pub(crate) pitch_degrees: f64,
    pub(crate) rail_curvature_per_m: f64,
    pub(crate) mean_train_slope_z: f64,
}

#[derive(Debug)]
pub struct CandidateCalibrationPreview {
    pub status: PreviewStatus,
    pub end_speed_mps: Option<f64>,
    pub cars: Vec<CandidateCarForces>,
}

enum PreviewOutput {
    Track(Box<TrackPreview>),
    Candidate(CandidateCalibrationPreview),
}

struct PreviewCompletion {
    status: PreviewStatus,
    end_speed_mps: Option<f64>,
    flags: Vec<bool>,
    recorder: Option<TestRunRecorder>,
    car_recorders: Option<Vec<TestRunRecorder>>,
    candidate_recorders: Option<Vec<CandidateCarRecorder>>,
    circuit_length: f64,
    completed: bool,
    rolled_back: bool,
}

#[derive(Clone)]
struct CandidateCarRecorder {
    last_reading: Option<super::CarReading>,
    next_sample_distance: f64,
    samples: Vec<TestSample>,
    within_limits: bool,
    #[cfg(test)]
    minimum_vertical_sample: Option<CandidateForceSample>,
    #[cfg(test)]
    maximum_longitudinal_sample: Option<CandidateForceSample>,
}

impl Default for CandidateCarRecorder {
    fn default() -> Self {
        Self {
            last_reading: None,
            next_sample_distance: 0.0,
            samples: Vec::new(),
            within_limits: true,
            #[cfg(test)]
            minimum_vertical_sample: None,
            #[cfg(test)]
            maximum_longitudinal_sample: None,
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct Snapshot {
    boundary: usize,
    train: Train,
    recorder: TestRunRecorder,
    evaluators: Vec<ForceEvaluator>,
    flags: Vec<bool>,
    previous_flags: Vec<Option<(f64, bool)>>,
    furthest_flag_metres: Vec<Option<usize>>,
    stationary_steps: usize,
    steps: usize,
}

/// Incremental construction preview. Snapshots are taken before a boundary-crossing step.
#[derive(Default)]
pub struct PreviewCache {
    piece_hashes: Vec<u64>,
    parameters: Vec<u8>,
    closed: bool,
    snapshots: Vec<Snapshot>,
}

impl PreviewCache {
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Serialized snapshot size; heap allocations may occupy more memory.
    pub fn snapshot_serialized_bytes(&self) -> usize {
        self.snapshots
            .iter()
            .map(|snapshot| {
                postcard::to_allocvec(snapshot)
                    .expect("preview snapshot serializes")
                    .len()
            })
            .sum()
    }

    pub fn preview(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
    ) -> Option<TrackPreview> {
        match self.preview_impl(pieces, model, config, false, None)? {
            PreviewOutput::Track(preview) => Some(*preview),
            PreviewOutput::Candidate(_) => unreachable!("lead preview returns a track report"),
        }
    }

    /// Previews an open track while measuring every car independently.
    ///
    /// The front-car samples and force-limit flags use the same path as `preview`.
    /// Per-car rating inputs start from the reusable common-prefix train snapshot.
    pub fn preview_all_cars(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
    ) -> Option<TrackPreview> {
        match self.preview_impl(pieces, model, config, true, None)? {
            PreviewOutput::Track(preview) => Some(*preview),
            PreviewOutput::Candidate(_) => unreachable!("all-car preview returns a track report"),
        }
    }

    /// Previews a candidate piece using compact per-car samples without computing ride ratings.
    pub fn preview_candidate(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
        candidate_piece_index: usize,
    ) -> Option<CandidateCalibrationPreview> {
        match self.preview_impl(pieces, model, config, false, Some(candidate_piece_index))? {
            PreviewOutput::Candidate(preview) => Some(preview),
            PreviewOutput::Track(_) => unreachable!("candidate preview returns force summaries"),
        }
    }

    fn preview_impl(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
        record_all_cars: bool,
        candidate_piece_index: Option<usize>,
    ) -> Option<PreviewOutput> {
        let configuration = &model.train;
        let limits = model.comfortable_force_limits;
        let model_key = model.name;
        let station = pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)?;
        assert!(pieces.windows(2).all(|pair| pair[0].exit == pair[1].entry));
        let closed = pieces.last().unwrap().exit == pieces[0].entry;
        let mut starts = Vec::with_capacity(pieces.len());
        let mut length = 0.0;
        for piece in pieces {
            starts.push(length);
            length += piece.length();
        }
        let circuit = Circuit {
            pieces: pieces.to_vec(),
            starts,
            length,
            open: !closed,
        };
        if let Some(candidate_piece_index) = candidate_piece_index {
            assert!(candidate_piece_index < circuit.pieces().len());
            assert!(circuit.open, "candidate calibration requires an open track");
        }
        let hashes: Vec<_> = pieces
            .iter()
            .map(|piece| {
                let mut hash = StateHasher::new();
                piece.hash_into(&mut hash);
                hash.finish()
            })
            .collect();
        let parameters =
            postcard::to_allocvec(&(configuration, config, limits, model.key, model_key))
                .expect("preview parameters serialize");
        let common = if self.parameters == parameters && self.closed == closed {
            self.piece_hashes
                .iter()
                .zip(&hashes)
                .take_while(|(left, right)| left == right)
                .count()
        } else {
            0
        };
        self.snapshots
            .retain(|snapshot| snapshot.boundary <= common);
        let saved = self.snapshots.last();
        let (
            mut train,
            mut recorder,
            mut evaluators,
            mut flags,
            mut previous_flags,
            mut furthest_flag_metres,
            mut stationary_steps,
            start_step,
        ) = if let Some(snapshot) = saved {
            let mut train = snapshot.train.clone();
            train.controls.resize(pieces.len(), None);
            let recorder = if candidate_piece_index.is_none() {
                let mut recorder = snapshot.recorder.clone();
                recorder.rebind_preview_circuit(&circuit, model);
                Some(recorder)
            } else {
                None
            };
            let mut flags = snapshot.flags.clone();
            flags.resize(length.ceil() as usize, false);
            (
                train,
                recorder,
                snapshot.evaluators.clone(),
                flags,
                snapshot.previous_flags.clone(),
                snapshot.furthest_flag_metres.clone(),
                snapshot.stationary_steps,
                snapshot.steps,
            )
        } else {
            let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
            (
                Train::new(&circuit, configuration, config, station_distance, 0.0),
                candidate_piece_index
                    .is_none()
                    .then(|| TestRunRecorder::for_circuit_with_model(&circuit, model)),
                vec![ForceEvaluator::new(limits, TIME_STEP); usize::from(configuration.cars)],
                vec![false; length.ceil() as usize],
                vec![None; usize::from(configuration.cars)],
                vec![None; usize::from(configuration.cars)],
                0,
                0,
            )
        };
        let mut car_recorders = record_all_cars.then(|| {
            evaluators
                .iter()
                .map(|evaluator| {
                    let mut recorder = TestRunRecorder::for_circuit_with_model(&circuit, model);
                    recorder.set_force_evaluator(evaluator.clone());
                    recorder
                })
                .collect::<Vec<_>>()
        });
        let mut candidate_recorders = candidate_piece_index
            .map(|_| vec![CandidateCarRecorder::default(); usize::from(configuration.cars)]);
        self.piece_hashes = hashes;
        self.parameters = parameters;
        self.closed = closed;
        let max_steps = (PREVIEW_MAX_SECONDS / TIME_STEP) as usize;
        for step in start_step..max_steps {
            let previous_train = train.clone();
            let previous_distance = previous_train.distance();
            let previous_speed = previous_train.speed();
            train.step_preview_all_cars(&circuit, TIME_STEP);
            let boundary = circuit
                .starts
                .partition_point(|start| *start <= train.distance());
            let previous_boundary = circuit
                .starts
                .partition_point(|start| *start <= previous_train.distance());
            let crossed_end = previous_train.distance() < length && train.distance() >= length;
            let snapshot_boundary = if crossed_end {
                pieces.len()
            } else {
                boundary.saturating_sub(1)
            };
            if candidate_piece_index.is_none()
                && (boundary > previous_boundary || crossed_end)
                && self
                    .snapshots
                    .last()
                    .is_none_or(|snapshot| snapshot.boundary < snapshot_boundary)
            {
                self.snapshots.push(Snapshot {
                    boundary: snapshot_boundary,
                    train: previous_train,
                    recorder: recorder
                        .as_ref()
                        .expect("track previews retain a recorder")
                        .clone(),
                    evaluators: evaluators.clone(),
                    flags: flags.clone(),
                    previous_flags: previous_flags.clone(),
                    furthest_flag_metres: furthest_flag_metres.clone(),
                    stationary_steps,
                    steps: step,
                });
            }
            let mut readings = train.take_readings();
            for frame in &mut readings {
                #[cfg(test)]
                let mean_train_slope_z = candidate_recorders
                    .as_ref()
                    .map(|_| mean_train_slope_z(&circuit, frame));
                for (car_index, (reading, evaluator)) in
                    frame.iter_mut().zip(&mut evaluators).enumerate()
                {
                    let evaluated = evaluator.update(reading.g);
                    let exceeded = !evaluated.within_limits;
                    if let Some(candidate_recorders) = &mut candidate_recorders {
                        let mut filtered_reading = *reading;
                        filtered_reading.g = evaluated.filtered;
                        filtered_reading.airtime = evaluated.filtered.vertical < 0.0;
                        candidate_recorders[car_index].record(
                            filtered_reading,
                            candidate_piece_index.expect("candidate recorder has a target piece"),
                            &circuit,
                            evaluated.within_limits,
                            #[cfg(test)]
                            mean_train_slope_z
                                .expect("candidate previews collect train slope diagnostics"),
                        );
                    } else if let Some((previous_distance, previous_exceeded)) =
                        previous_flags[car_index]
                    {
                        if reading.distance >= previous_distance {
                            let first = (previous_distance.ceil() as usize)
                                .max(furthest_flag_metres[car_index].map_or(0, |metre| metre + 1));
                            let last = (reading.distance.floor() as usize)
                                .min(flags.len().saturating_sub(1));
                            if first <= last {
                                for flag in &mut flags[first..=last] {
                                    *flag |= previous_exceeded || exceeded;
                                }
                                furthest_flag_metres[car_index] = Some(last);
                            }
                        }
                        previous_flags[car_index] = Some((reading.distance, exceeded));
                    }
                }
            }
            let reached_end = train.distance() >= length;
            if reached_end && circuit.open && !readings.is_empty() {
                readings[0][0].distance = length - 1e-6;
            }
            if !reached_end || circuit.open {
                if let Some(car_recorders) = &mut car_recorders {
                    for (car_index, recorder) in car_recorders.iter_mut().enumerate() {
                        recorder.record_train_car(&readings, car_index, model_key);
                    }
                }
                if candidate_piece_index.is_none() {
                    let front: Vec<_> = readings.iter().map(|frame| vec![frame[0]]).collect();
                    recorder
                        .as_mut()
                        .expect("track previews retain a recorder")
                        .record(&front, model_key);
                }
            }
            if reached_end {
                let end_speed_mps = Some(interpolate_speed_at_distance(
                    previous_distance,
                    train.distance(),
                    previous_speed,
                    train.speed(),
                    length,
                ));
                return Some(preview_output(PreviewCompletion {
                    status: PreviewStatus::ReachedEnd,
                    end_speed_mps,
                    flags,
                    recorder,
                    car_recorders,
                    candidate_recorders,
                    circuit_length: length,
                    completed: true,
                    rolled_back: false,
                }));
            }
            if train.speed().abs() < 0.01 {
                stationary_steps += 1;
            } else {
                stationary_steps = 0;
            }
            if !train.notes().is_empty() || stationary_steps >= (2.0 / TIME_STEP) as usize {
                return Some(preview_output(PreviewCompletion {
                    status: PreviewStatus::Stalled,
                    end_speed_mps: None,
                    flags,
                    recorder,
                    car_recorders,
                    candidate_recorders,
                    circuit_length: length,
                    completed: false,
                    rolled_back: true,
                }));
            }
        }
        Some(preview_output(PreviewCompletion {
            status: PreviewStatus::TimedOut,
            end_speed_mps: None,
            flags,
            recorder,
            car_recorders,
            candidate_recorders,
            circuit_length: length,
            completed: false,
            rolled_back: false,
        }))
    }
}

fn preview_output(completion: PreviewCompletion) -> PreviewOutput {
    if let Some(candidate_recorders) = completion.candidate_recorders {
        assert!(
            completion.recorder.is_none(),
            "candidate previews omit ride reporting"
        );
        let cars = candidate_recorders
            .into_iter()
            .enumerate()
            .filter_map(|(car_index, recorder)| {
                let summary =
                    candidate_force_peak_summary(&recorder.samples, completion.circuit_length)?;
                #[cfg(test)]
                let maximum_longitudinal_sample = recorder
                    .maximum_longitudinal_sample
                    .expect("candidate force summary has a longitudinal extremum");
                Some(CandidateCarForces {
                    car_index,
                    peaks: summary.force,
                    sustained_minimum_vertical_g: summary.sustained.minimum_vertical_g,
                    within_limits: recorder.within_limits,
                    #[cfg(test)]
                    minimum_vertical_sample: recorder
                        .minimum_vertical_sample
                        .expect("candidate force summary has a vertical extremum"),
                    #[cfg(test)]
                    maximum_longitudinal_sample,
                })
            })
            .collect();
        PreviewOutput::Candidate(CandidateCalibrationPreview {
            status: completion.status,
            end_speed_mps: completion.end_speed_mps,
            cars,
        })
    } else {
        PreviewOutput::Track(Box::new(TrackPreview {
            report: finish_preview(
                completion
                    .recorder
                    .expect("track previews retain a recorder"),
                completion.car_recorders,
                completion.completed,
                completion.rolled_back,
            ),
            status: completion.status,
            end_speed_mps: completion.end_speed_mps,
            force_limit_exceeded: completion.flags,
        }))
    }
}

impl CandidateCarRecorder {
    fn record(
        &mut self,
        reading: super::CarReading,
        candidate_piece_index: usize,
        circuit: &Circuit,
        within_limits: bool,
        #[cfg(test)] mean_train_slope_z: f64,
    ) {
        if reading.piece_index == candidate_piece_index {
            self.within_limits &= within_limits;
            #[cfg(test)]
            {
                let location = candidate_force_sample(
                    reading,
                    candidate_piece_index,
                    circuit,
                    mean_train_slope_z,
                );
                if self
                    .minimum_vertical_sample
                    .is_none_or(|current| reading.g.vertical < current.force_g)
                {
                    self.minimum_vertical_sample = Some(CandidateForceSample {
                        force_g: reading.g.vertical,
                        ..location
                    });
                }
                if self
                    .maximum_longitudinal_sample
                    .is_none_or(|current| reading.g.longitudinal.abs() > current.force_g.abs())
                {
                    self.maximum_longitudinal_sample = Some(CandidateForceSample {
                        force_g: reading.g.longitudinal,
                        ..location
                    });
                }
            }
        }
        let Some(previous) = self.last_reading else {
            self.next_sample_distance = reading.distance.ceil();
            if (self.next_sample_distance - reading.distance).abs() < 1e-9 {
                self.store_sample(
                    reading,
                    self.next_sample_distance,
                    candidate_piece_index,
                    circuit,
                );
                self.next_sample_distance += 1.0;
            }
            self.last_reading = Some(reading);
            return;
        };

        let distance_delta = reading.distance - previous.distance;
        if distance_delta > 0.0 {
            while self.next_sample_distance <= reading.distance + 1e-9 {
                let fraction = ((self.next_sample_distance - previous.distance) / distance_delta)
                    .clamp(0.0, 1.0);
                let sampled_reading = interpolate_candidate_reading(previous, reading, fraction);
                self.store_sample(
                    sampled_reading,
                    self.next_sample_distance,
                    candidate_piece_index,
                    circuit,
                );
                self.next_sample_distance += 1.0;
            }
        }
        self.last_reading = Some(reading);
    }

    fn store_sample(
        &mut self,
        reading: super::CarReading,
        sample_distance: f64,
        candidate_piece_index: usize,
        circuit: &Circuit,
    ) {
        let distance = sample_distance.rem_euclid(circuit.length());
        let piece_index = circuit.piece_index(distance);
        if piece_index != candidate_piece_index {
            return;
        }
        self.samples.push(TestSample {
            distance,
            speed: reading.speed,
            vertical_g: reading.g.vertical,
            lateral_g: reading.g.lateral,
            longitudinal_g: reading.g.longitudinal,
            airtime: reading.airtime,
            force_limit_exceeded: [false; 3],
            height_m: reading.height_m,
            inverted: reading.inverted,
            roll: reading.roll,
            piece_index,
        });
    }
}

#[cfg(test)]
fn mean_train_slope_z(circuit: &Circuit, readings: &[super::CarReading]) -> f64 {
    readings
        .iter()
        .map(|reading| {
            let (piece_index, local_distance) = circuit.locate(reading.distance);
            assert_eq!(piece_index, reading.piece_index);
            circuit.pieces[piece_index].tangent_at(local_distance).z
        })
        .sum::<f64>()
        / readings.len() as f64
}

#[cfg(test)]
fn candidate_force_sample(
    reading: super::CarReading,
    candidate_piece_index: usize,
    circuit: &Circuit,
    mean_train_slope_z: f64,
) -> CandidateForceSample {
    let local_distance_m = reading.distance - circuit.starts[candidate_piece_index];
    let sample = circuit.pieces[candidate_piece_index].sample(local_distance_m);
    CandidateForceSample {
        distance_m: reading.distance,
        local_distance_m,
        speed_mps: reading.speed,
        height_m: reading.height_m,
        force_g: reading.g.vertical,
        pitch_degrees: sample.tangent.z.asin().to_degrees(),
        rail_curvature_per_m: sample.curvature,
        mean_train_slope_z,
    }
}

fn interpolate_candidate_reading(
    first: super::CarReading,
    second: super::CarReading,
    fraction: f64,
) -> super::CarReading {
    let interpolate = |first: f64, second: f64| first + (second - first) * fraction;
    let vertical = interpolate(first.g.vertical, second.g.vertical);
    super::CarReading {
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

fn finish_preview(
    recorder: TestRunRecorder,
    car_recorders: Option<Vec<TestRunRecorder>>,
    completed: bool,
    rolled_back: bool,
) -> TestRun {
    let mut report = recorder.finish(completed, rolled_back);
    if let Some(car_recorders) = car_recorders {
        let car_reports: Vec<_> = car_recorders
            .into_iter()
            .map(|recorder| recorder.finish(completed, rolled_back))
            .collect();
        report.average_car_ratings(&car_reports);
    }
    report
}

fn interpolate_speed_at_distance(
    previous_distance: f64,
    current_distance: f64,
    previous_speed: f64,
    current_speed: f64,
    target_distance: f64,
) -> f64 {
    let distance_delta = current_distance - previous_distance;
    assert!(distance_delta > 0.0, "train crossed the endpoint forward");
    let fraction = ((target_distance - previous_distance) / distance_delta).clamp(0.0, 1.0);
    (previous_speed + fraction * (current_speed - previous_speed)).abs()
}

/// Simulate a train from the station to the end of the placed track.
/// Samples beyond a stall are omitted. A completed report means the front car
/// reached the end, rather than that the ride completed a circuit.
pub fn preview_open_track(
    pieces: &[Piece],
    model: &CoasterModel,
    config: PhysicsConfig,
) -> Option<TrackPreview> {
    PreviewCache::default().preview(pieces, model, config)
}

#[cfg(test)]
pub(super) fn preview_at_step(
    pieces: &[Piece],
    model: &CoasterModel,
    config: PhysicsConfig,
    step_limit: usize,
    time_step: f64,
) -> Option<TrackPreview> {
    assert_eq!(time_step, TIME_STEP);
    let configuration = &model.train;
    let model_key = model.name;
    let station = pieces
        .iter()
        .position(|piece| piece.kind == PieceKind::Station)?;
    assert!(pieces.windows(2).all(|pair| pair[0].exit == pair[1].entry));
    let closed = pieces.last().unwrap().exit == pieces[0].entry;
    let mut starts = Vec::with_capacity(pieces.len());
    let mut length = 0.0;
    for piece in pieces {
        starts.push(length);
        length += piece.length();
    }
    let circuit = Circuit {
        pieces: pieces.to_vec(),
        starts,
        length,
        open: !closed,
    };
    let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
    let mut train = Train::new(&circuit, configuration, config, station_distance, 0.0);
    let mut recorder = TestRunRecorder::for_circuit_with_model(&circuit, model);
    recorder.set_time_step(time_step);
    let mut stationary_steps = 0;
    for _ in 0..step_limit {
        let previous_distance = train.distance();
        let previous_speed = train.speed();
        train.step_preview(&circuit, time_step);
        if train.distance() >= length {
            if circuit.open {
                let mut readings = train.take_readings();
                readings[0][0].distance = length - 1e-6;
                recorder.record(&readings, model_key);
            }
            return Some(TrackPreview {
                report: recorder.finish(true, false),
                status: PreviewStatus::ReachedEnd,
                end_speed_mps: Some(interpolate_speed_at_distance(
                    previous_distance,
                    train.distance(),
                    previous_speed,
                    train.speed(),
                    length,
                )),
                force_limit_exceeded: Vec::new(),
            });
        }
        let readings = train.take_readings();
        recorder.record(&readings, model_key);
        if train.speed().abs() < 0.01 {
            stationary_steps += 1;
        } else {
            stationary_steps = 0;
        }
        if !train.notes().is_empty() || stationary_steps >= (2.0 / time_step) as usize {
            return Some(TrackPreview {
                report: recorder.finish(false, true),
                status: PreviewStatus::Stalled,
                end_speed_mps: None,
                force_limit_exceeded: Vec::new(),
            });
        }
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        status: PreviewStatus::TimedOut,
        end_speed_mps: None,
        force_limit_exceeded: Vec::new(),
    })
}
