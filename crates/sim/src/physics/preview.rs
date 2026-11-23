use super::features::Location;
use super::testing::PreviewForcePeakRecorder;
use super::{
    CarReading, Circuit, EnvelopeEvaluator, PhysicsConfig, PieceForcePeaks, PreviewForceSample,
    TIME_STEP, TestRun, TestRunRecorder, TestSample, Train, candidate_force_peak_summary,
};
use crate::physics::ForcePeakValues;
use crate::state_hash::StateHasher;
use crate::track::{CoasterModel, Piece, PieceKind, TrackState};

const PREVIEW_MAX_SECONDS: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewStatus {
    ReachedEnd,
    Stalled,
    TimedOut,
}

pub struct TrackPreview {
    pub report: TestRun,
    pub force_envelope_samples: Vec<PreviewForceSample>,
    pub piece_force_peaks: Vec<PieceForcePeaks>,
    pub status: PreviewStatus,
    /// Front-car speed interpolated at the exact open-track endpoint when reached.
    pub end_speed_mps: Option<f64>,
    pub force_limit_exceeded: Vec<bool>,
}

/// Height band below an inversion's highest point that counts as its crest.
const INVERTED_CREST_DEPTH_M: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateCarForces {
    pub car_index: usize,
    pub peaks: ForcePeakValues,
    pub sustained_minimum_vertical_g: Option<f64>,
    pub inverted_minimum_vertical_g: Option<f64>,
    /// Mean vertical force over the car's inverted samples within `INVERTED_CREST_DEPTH_M` of its
    /// highest inverted sample: the felt top of an inversion, read over the curved crest.
    pub inverted_crest_vertical_g: Option<f64>,
    pub peak_roll_rate_deg_s: f64,
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

#[derive(Clone, Debug)]
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
    force_peak_recorder: PreviewForcePeakRecorder,
    car_recorders: Option<Vec<TestRunRecorder>>,
    candidate_recorders: Option<Vec<CandidateCarRecorder>>,
    circuit_length: f64,
    piece_count: usize,
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Snapshot {
    boundary: usize,
    train: Train,
    recorder: TestRunRecorder,
    force_peak_recorder: PreviewForcePeakRecorder,
    evaluators: Vec<EnvelopeEvaluator>,
    flags: Vec<bool>,
    previous_flags: Vec<Option<(f64, bool)>>,
    furthest_flag_metres: Vec<Option<usize>>,
    stationary_steps: usize,
    steps: usize,
}

/// Incremental construction preview. Snapshots are taken before a boundary-crossing step.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
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
        match self.preview_impl(pieces, model, config, false, None, false)? {
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
        match self.preview_impl(pieces, model, config, true, None, false)? {
            PreviewOutput::Track(preview) => Some(*preview),
            PreviewOutput::Candidate(_) => unreachable!("all-car preview returns a track report"),
        }
    }

    /// Previews a candidate piece using compact per-car samples without computing ride ratings.
    ///
    /// Always treats the track as open so a candidate piece whose exit meets the station entry
    /// still runs through the virtual tangent runout for rear-car calibration.
    pub fn preview_candidate(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
        candidate_piece_index: usize,
    ) -> Option<CandidateCalibrationPreview> {
        match self.preview_impl(
            pieces,
            model,
            config,
            false,
            Some(candidate_piece_index),
            true,
        )? {
            PreviewOutput::Candidate(preview) => Some(preview),
            PreviewOutput::Track(_) => unreachable!("candidate preview returns force summaries"),
        }
    }

    /// A cache holding only the snapshot taken where the train crosses the end of the first
    /// `piece_count` pieces, so callers can branch several continuations from that end cheaply.
    pub fn fork_at_end(&self, piece_count: usize) -> Option<Self> {
        let snapshot = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.boundary == piece_count)?
            .clone();
        Some(Self {
            piece_hashes: self.piece_hashes.get(..piece_count)?.to_vec(),
            parameters: self.parameters.clone(),
            closed: false,
            snapshots: vec![snapshot],
        })
    }

    /// Previews `pieces` as open track even when they end where they start, so a closing
    /// sequence resumes from the open-end snapshot instead of simulating a whole lap. The
    /// train runs past the final piece onto a straight tangent runout.
    pub fn preview_open_continuation(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
    ) -> Option<TrackPreview> {
        match self.preview_impl(pieces, model, config, false, None, true)? {
            PreviewOutput::Track(preview) => Some(*preview),
            PreviewOutput::Candidate(_) => unreachable!("open continuation returns a track report"),
        }
    }

    fn preview_impl(
        &mut self,
        pieces: &[Piece],
        model: &CoasterModel,
        config: PhysicsConfig,
        record_all_cars: bool,
        candidate_piece_index: Option<usize>,
        treat_as_open: bool,
    ) -> Option<PreviewOutput> {
        let configuration = &model.train;
        let model_key = model.name;
        let _station = pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)?;
        assert!(pieces.windows(2).all(|pair| pair[0].exit == pair[1].entry));
        let closed = !treat_as_open && pieces.last().unwrap().exit == pieces[0].entry;
        let source_piece_count = pieces.len();
        let (source_circuit, source_length, circuit) = if candidate_piece_index.is_some() {
            let source_length = pieces.iter().map(Piece::length).sum();
            let mut simulation_pieces = Vec::with_capacity(pieces.len() + 1);
            simulation_pieces.extend_from_slice(pieces);
            if !closed {
                simulation_pieces.extend(virtual_tangent_runout(
                    pieces.last().unwrap().exit,
                    configuration.length_metres,
                ));
            }
            let circuit = circuit_from_pieces(simulation_pieces, !closed);
            (None, source_length, circuit)
        } else {
            let source_circuit = circuit_from_pieces(pieces.to_vec(), !closed);
            let source_length = source_circuit.length;
            let mut simulation_pieces = source_circuit.pieces.clone();
            if !closed {
                simulation_pieces.extend(virtual_tangent_runout(
                    pieces.last().unwrap().exit,
                    configuration.length_metres,
                ));
            }
            let circuit = circuit_from_pieces(simulation_pieces, !closed);
            (Some(source_circuit), source_length, circuit)
        };
        let preview_end_distance = if closed {
            source_length
        } else {
            source_length + configuration.length_metres
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
        let parameters = postcard::to_allocvec(&(configuration, config, model.key, model_key))
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
            mut force_peak_recorder,
            mut evaluators,
            mut flags,
            mut previous_flags,
            mut furthest_flag_metres,
            mut stationary_steps,
            start_step,
        ) = if let Some(snapshot) = saved {
            let mut train = snapshot.train.clone();
            train.controls.resize(circuit.pieces.len(), None);
            let recorder = if candidate_piece_index.is_none() {
                let mut recorder = snapshot.recorder.clone();
                recorder.rebind_preview_circuit(
                    source_circuit
                        .as_ref()
                        .expect("track preview builds source circuit"),
                    model,
                );
                Some(recorder)
            } else {
                None
            };
            let flags = if candidate_piece_index.is_none() {
                let mut flags = snapshot.flags.clone();
                flags.resize(source_length.ceil() as usize, false);
                flags
            } else {
                Vec::new()
            };
            let force_peak_recorder = if candidate_piece_index.is_none() {
                let mut force_peak_recorder = snapshot.force_peak_recorder.clone();
                force_peak_recorder.rebind_circuit(&circuit);
                force_peak_recorder
            } else {
                PreviewForcePeakRecorder::new(&circuit, 0)
            };
            (
                train,
                recorder,
                force_peak_recorder,
                snapshot.evaluators.clone(),
                flags,
                if candidate_piece_index.is_none() {
                    snapshot.previous_flags.clone()
                } else {
                    Vec::new()
                },
                if candidate_piece_index.is_none() {
                    snapshot.furthest_flag_metres.clone()
                } else {
                    Vec::new()
                },
                snapshot.stationary_steps,
                snapshot.steps,
            )
        } else {
            (
                Train::new(&circuit, configuration, config, 0.0, 2.0),
                candidate_piece_index.is_none().then(|| {
                    TestRunRecorder::for_circuit_with_model(
                        source_circuit
                            .as_ref()
                            .expect("track preview builds source circuit"),
                        model,
                    )
                }),
                PreviewForcePeakRecorder::new(
                    &circuit,
                    if candidate_piece_index.is_none() {
                        configuration.cars
                    } else {
                        0
                    },
                ),
                vec![EnvelopeEvaluator::new(TIME_STEP); usize::from(configuration.cars)],
                if candidate_piece_index.is_none() {
                    vec![false; source_length.ceil() as usize]
                } else {
                    Vec::new()
                },
                if candidate_piece_index.is_none() {
                    vec![None; usize::from(configuration.cars)]
                } else {
                    Vec::new()
                },
                if candidate_piece_index.is_none() {
                    vec![None; usize::from(configuration.cars)]
                } else {
                    Vec::new()
                },
                0,
                0,
            )
        };
        let mut car_recorders = record_all_cars.then(|| {
            let source_circuit = source_circuit
                .as_ref()
                .expect("all-car preview builds source circuit");
            evaluators
                .iter()
                .map(|evaluator| {
                    let mut recorder =
                        TestRunRecorder::for_circuit_with_model(source_circuit, model);
                    recorder.set_envelope_evaluator(evaluator.clone());
                    recorder
                })
                .collect::<Vec<_>>()
        });
        let mut candidate_recorders = candidate_piece_index
            .map(|_| vec![CandidateCarRecorder::default(); usize::from(configuration.cars)]);
        let mut candidate_readings = Vec::with_capacity(
            candidate_piece_index.map_or(0, |_| usize::from(configuration.cars)),
        );
        self.piece_hashes = hashes;
        self.parameters = parameters;
        self.closed = closed;
        let max_steps = (PREVIEW_MAX_SECONDS / TIME_STEP) as usize;
        let mut end_speed_mps = None;
        for step in start_step..max_steps {
            let previous_train = candidate_piece_index.is_none().then(|| train.clone());
            let previous_distance = train.distance();
            let previous_speed = train.speed();
            if candidate_piece_index.is_some() {
                train.step_preview_candidate(&circuit, TIME_STEP, &mut candidate_readings);
            } else {
                train.step_preview_all_cars(&circuit, TIME_STEP);
            }
            let boundary = circuit
                .starts
                .partition_point(|start| *start <= train.distance());
            let previous_boundary = circuit
                .starts
                .partition_point(|start| *start <= previous_distance);
            let crossed_end =
                previous_distance < source_length && train.distance() >= source_length;
            if crossed_end {
                end_speed_mps = Some(interpolate_speed_at_distance(
                    previous_distance,
                    train.distance(),
                    previous_speed,
                    train.speed(),
                    source_length,
                ));
            }
            let snapshot_boundary = if crossed_end {
                source_piece_count
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
                    train: previous_train.expect("track preview retains snapshot state"),
                    recorder: recorder
                        .as_ref()
                        .expect("track previews retain a recorder")
                        .clone(),
                    force_peak_recorder: force_peak_recorder.clone(),
                    evaluators: evaluators.clone(),
                    flags: flags.clone(),
                    previous_flags: previous_flags.clone(),
                    furthest_flag_metres: furthest_flag_metres.clone(),
                    stationary_steps,
                    steps: step,
                });
            }
            let readings = if let Some(candidate_piece_index) = candidate_piece_index {
                #[cfg(test)]
                let mean_train_slope_z = mean_train_slope_z(&circuit, &candidate_readings);
                for (car_index, reading) in candidate_readings.iter().copied().enumerate() {
                    let piece = &circuit.pieces[reading.piece_index];
                    let active = reading.speed > 1.0
                        && matches!(
                            piece.kind,
                            PieceKind::Track | PieceKind::Launch | PieceKind::Brakes
                        );
                    let evaluated = evaluators[car_index].record(
                        reading.g,
                        active,
                        Location {
                            piece_index: reading.piece_index,
                            piece_name: piece.name,
                            distance_m: reading.distance.rem_euclid(circuit.length),
                            seconds_from_start: 0.0,
                        },
                    );
                    let mut filtered_reading = reading;
                    filtered_reading.g = evaluated.filtered;
                    filtered_reading.airtime = evaluated.filtered.vertical < 0.0;
                    candidate_recorders
                        .as_mut()
                        .expect("candidate recorder has a target piece")[car_index]
                        .record(
                            filtered_reading,
                            candidate_piece_index,
                            &circuit,
                            evaluated.within_limits,
                            #[cfg(test)]
                            mean_train_slope_z,
                        );
                }
                Vec::new()
            } else {
                let mut readings = train.take_readings();
                for frame in &mut readings {
                    #[cfg(test)]
                    let mean_train_slope_z = candidate_recorders
                        .as_ref()
                        .map(|_| mean_train_slope_z(&circuit, frame));
                    for (car_index, (reading, evaluator)) in
                        frame.iter_mut().zip(&mut evaluators).enumerate()
                    {
                        let piece = &circuit.pieces[reading.piece_index];
                        let active = reading.speed > 1.0
                            && matches!(
                                piece.kind,
                                PieceKind::Track | PieceKind::Launch | PieceKind::Brakes
                            );
                        let evaluated = evaluator.record(
                            reading.g,
                            active,
                            Location {
                                piece_index: reading.piece_index,
                                piece_name: piece.name,
                                distance_m: reading.distance.rem_euclid(circuit.length),
                                seconds_from_start: 0.0,
                            },
                        );
                        force_peak_recorder.record(
                            car_index,
                            CarReading {
                                g: evaluated.filtered,
                                ..*reading
                            },
                            evaluated.axis_within_limits,
                        );
                        let exceeded = !evaluated.within_limits;
                        if let Some(candidate_recorders) = &mut candidate_recorders {
                            let mut filtered_reading = *reading;
                            filtered_reading.g = evaluated.filtered;
                            filtered_reading.airtime = evaluated.filtered.vertical < 0.0;
                            candidate_recorders[car_index].record(
                                filtered_reading,
                                candidate_piece_index
                                    .expect("candidate recorder has a target piece"),
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
                                let first = (previous_distance.ceil() as usize).max(
                                    furthest_flag_metres[car_index].map_or(0, |metre| metre + 1),
                                );
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
                readings
            };
            if !readings.is_empty() {
                if let Some(car_recorders) = &mut car_recorders {
                    for (car_index, recorder) in car_recorders.iter_mut().enumerate() {
                        let car_readings: Vec<_> = readings
                            .iter()
                            .filter(|frame| closed || frame[car_index].distance < source_length)
                            .cloned()
                            .collect();
                        recorder.record_train_car(&car_readings, car_index, model_key);
                    }
                }
                if candidate_piece_index.is_none() {
                    let front: Vec<_> = readings
                        .iter()
                        .filter(|frame| closed || frame[0].distance < source_length)
                        .map(|frame| vec![frame[0]])
                        .collect();
                    recorder
                        .as_mut()
                        .expect("track previews retain a recorder")
                        .record(&front, model_key);
                }
            }
            let reached_candidate_end =
                candidate_piece_index.is_some() && train.distance() >= source_length;
            let reached_preview_end = train.distance() >= preview_end_distance;
            if reached_candidate_end || reached_preview_end {
                return Some(preview_output(PreviewCompletion {
                    status: PreviewStatus::ReachedEnd,
                    end_speed_mps,
                    flags,
                    recorder,
                    force_peak_recorder,
                    car_recorders,
                    candidate_recorders,
                    circuit_length: source_length,
                    piece_count: source_piece_count,
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
                    force_peak_recorder,
                    car_recorders,
                    candidate_recorders,
                    circuit_length: source_length,
                    piece_count: source_piece_count,
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
            force_peak_recorder,
            car_recorders,
            candidate_recorders,
            circuit_length: source_length,
            piece_count: source_piece_count,
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
                let inverted_minimum_vertical_g = recorder
                    .samples
                    .iter()
                    .filter(|sample| sample.inverted)
                    .map(|sample| sample.vertical_g)
                    .reduce(f64::min);
                let inverted_crest_vertical_g = recorder
                    .samples
                    .iter()
                    .filter(|sample| sample.inverted)
                    .map(|sample| sample.height_m)
                    .reduce(f64::max)
                    .map(|crest_height_m| {
                        let (sum_g, count) = recorder
                            .samples
                            .iter()
                            .filter(|sample| {
                                sample.inverted
                                    && sample.height_m >= crest_height_m - INVERTED_CREST_DEPTH_M
                            })
                            .fold((0.0, 0_u32), |(sum_g, count), sample| {
                                (sum_g + sample.vertical_g, count + 1)
                            });
                        sum_g / f64::from(count)
                    });
                let peak_roll_rate_deg_s = recorder
                    .samples
                    .windows(2)
                    .filter_map(|pair| {
                        let [first, second] = pair else {
                            return None;
                        };
                        let delta_distance = second.distance - first.distance;
                        let mean_speed = 0.5 * (first.speed + second.speed);
                        if delta_distance <= 1e-9 || mean_speed <= 0.0 {
                            return None;
                        }
                        let delta_seconds = delta_distance / mean_speed;
                        Some((second.roll - first.roll).abs().to_degrees() / delta_seconds)
                    })
                    .fold(0.0_f64, f64::max);
                #[cfg(test)]
                let maximum_longitudinal_sample = recorder
                    .maximum_longitudinal_sample
                    .expect("candidate force summary has a longitudinal extremum");
                Some(CandidateCarForces {
                    car_index,
                    peaks: summary.force,
                    sustained_minimum_vertical_g: summary.sustained.minimum_vertical_g,
                    inverted_minimum_vertical_g,
                    inverted_crest_vertical_g,
                    peak_roll_rate_deg_s,
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
        let report = finish_preview(
            completion
                .recorder
                .expect("track previews retain a recorder"),
            completion.car_recorders,
            completion.completed,
            completion.rolled_back,
        );
        let force_envelope_samples = completion
            .force_peak_recorder
            .force_envelope_samples()
            .into_iter()
            .filter(|sample| sample.distance < completion.circuit_length)
            .collect();
        let piece_force_peaks = completion
            .force_peak_recorder
            .piece_force_peaks(&report)
            .into_iter()
            .filter(|peaks| peaks.piece_index < completion.piece_count)
            .collect();
        PreviewOutput::Track(Box::new(TrackPreview {
            report,
            force_envelope_samples,
            piece_force_peaks,
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
        pitch_degrees: crate::math::asin(sample.tangent.z).to_degrees(),
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
        // Each piece measures roll from its own reference, so roll only interpolates within a piece.
        roll: if first.piece_index == second.piece_index {
            interpolate(first.roll, second.roll)
        } else if fraction < 0.5 {
            first.roll
        } else {
            second.roll
        },
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

fn circuit_from_pieces(pieces: Vec<Piece>, open: bool) -> Circuit {
    let mut starts = Vec::with_capacity(pieces.len());
    let mut length = 0.0;
    for piece in &pieces {
        starts.push(length);
        length += piece.length();
    }
    Circuit {
        pieces,
        starts,
        length,
        open,
    }
}

/// Builds a straight extension that continues the open connector's tangent and bank.
pub fn virtual_tangent_runout(end: TrackState, minimum_length: f64) -> Vec<Piece> {
    assert!(minimum_length.is_finite() && minimum_length > 0.0);
    vec![Piece::virtual_tangent_runout(end, minimum_length)]
}

/// Simulate a train across placed track and a virtual run-out.
/// Reports include only placed track. An open-track preview completes after the
/// full train clears the placed track; a closed preview completes after one lap.
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
    let _station = pieces
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
    let mut train = Train::new(&circuit, configuration, config, 0.0, 2.0);
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
                force_envelope_samples: Vec::new(),
                piece_force_peaks: Vec::new(),
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
                force_envelope_samples: Vec::new(),
                piece_force_peaks: Vec::new(),
                status: PreviewStatus::Stalled,
                end_speed_mps: None,
                force_limit_exceeded: Vec::new(),
            });
        }
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        force_envelope_samples: Vec::new(),
        piece_force_peaks: Vec::new(),
        status: PreviewStatus::TimedOut,
        end_speed_mps: None,
        force_limit_exceeded: Vec::new(),
    })
}
