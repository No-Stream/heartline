use super::testing::PreviewForcePeakRecorder;
use super::{
    CarReading, Circuit, ForceEvaluator, PhysicsConfig, PieceForcePeaks, PreviewForceSample,
    TIME_STEP, TestRun, TestRunRecorder, Train,
};
use crate::state_hash::StateHasher;
use crate::track::{
    Bank, CoasterModel, HorizontalCurvature, Piece, PieceId, PieceKind, Slope, TrackState,
    VerticalCurvature, catalog,
};

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
    pub force_limit_exceeded: Vec<bool>,
}

#[derive(Clone, serde::Serialize)]
struct Snapshot {
    boundary: usize,
    train: Train,
    recorder: TestRunRecorder,
    force_peak_recorder: PreviewForcePeakRecorder,
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
        let configuration = &model.train;
        let limits = model.comfortable_force_limits;
        let model_key = model.name;
        let _station = pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)?;
        assert!(pieces.windows(2).all(|pair| pair[0].exit == pair[1].entry));
        let closed = pieces.last().unwrap().exit == pieces[0].entry;
        let source_circuit = circuit_from_pieces(pieces.to_vec(), !closed);
        let source_length = source_circuit.length;
        let source_piece_count = source_circuit.pieces.len();
        let mut simulation_pieces = source_circuit.pieces.clone();
        if !closed {
            simulation_pieces.extend(virtual_level_runout(
                pieces.last().unwrap().exit,
                configuration.length_metres,
            ));
        }
        let circuit = circuit_from_pieces(simulation_pieces, !closed);
        let preview_end_distance = if closed {
            source_length
        } else {
            source_length + configuration.length_metres
        };
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
        let unchanged =
            self.parameters == parameters && self.closed == closed && self.piece_hashes == hashes;
        if !unchanged {
            self.snapshots
                .retain(|snapshot| snapshot.boundary <= common);
        }
        let saved = self.snapshots.last().cloned();
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
            let mut train = snapshot.train;
            train.controls.resize(circuit.pieces.len(), None);
            let mut recorder = snapshot.recorder;
            recorder.rebind_preview_circuit(&source_circuit, model);
            let mut force_peak_recorder = snapshot.force_peak_recorder;
            force_peak_recorder.rebind_circuit(&circuit);
            let mut flags = snapshot.flags;
            flags.resize(source_length.ceil() as usize, false);
            (
                train,
                recorder,
                force_peak_recorder,
                snapshot.evaluators,
                flags,
                snapshot.previous_flags,
                snapshot.furthest_flag_metres,
                snapshot.stationary_steps,
                snapshot.steps,
            )
        } else {
            (
                Train::new(&circuit, configuration, config, 0.0, 2.0),
                TestRunRecorder::for_circuit_with_model(&source_circuit, model),
                PreviewForcePeakRecorder::new(&circuit, configuration.cars),
                vec![ForceEvaluator::new(limits, TIME_STEP); usize::from(configuration.cars)],
                vec![false; source_length.ceil() as usize],
                vec![None; usize::from(configuration.cars)],
                vec![None; usize::from(configuration.cars)],
                0,
                0,
            )
        };
        self.piece_hashes = hashes;
        self.parameters = parameters;
        self.closed = closed;
        let max_steps = (PREVIEW_MAX_SECONDS / TIME_STEP) as usize;
        for step in start_step..max_steps {
            let previous_train = train.clone();
            train.step_preview_all_cars(&circuit, TIME_STEP);
            let boundary = circuit
                .starts
                .partition_point(|start| *start <= train.distance());
            let previous_boundary = circuit
                .starts
                .partition_point(|start| *start <= previous_train.distance());
            let crossed_end =
                previous_train.distance() < source_length && train.distance() >= source_length;
            let snapshot_boundary = if crossed_end {
                source_piece_count
            } else {
                boundary.saturating_sub(1)
            };
            if (boundary > previous_boundary || crossed_end)
                && self
                    .snapshots
                    .last()
                    .is_none_or(|snapshot| snapshot.boundary < snapshot_boundary)
            {
                self.snapshots.push(Snapshot {
                    boundary: snapshot_boundary,
                    train: previous_train,
                    recorder: recorder.clone(),
                    force_peak_recorder: force_peak_recorder.clone(),
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
                for (car_index, (reading, evaluator)) in
                    frame.iter_mut().zip(&mut evaluators).enumerate()
                {
                    let evaluated = evaluator.update(reading.g);
                    force_peak_recorder.record(
                        car_index,
                        CarReading {
                            g: evaluated.filtered,
                            ..*reading
                        },
                        evaluated.axis_within_limits,
                    );
                    let exceeded = !evaluated.within_limits;
                    if let Some((previous_distance, previous_exceeded)) = previous_flags[car_index]
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
                    }
                    previous_flags[car_index] = Some((reading.distance, exceeded));
                }
            }
            let front_car_readings: Vec<_> = readings
                .iter()
                .filter(|frame| closed || frame[0].distance < source_length)
                .map(|frame| vec![frame[0]])
                .collect();
            recorder.record(&front_car_readings, model_key);
            if train.distance() >= preview_end_distance {
                return Some(finish_preview(
                    recorder,
                    force_peak_recorder,
                    PreviewStatus::ReachedEnd,
                    true,
                    false,
                    flags,
                    &source_circuit,
                ));
            }
            if train.speed().abs() < 0.01 {
                stationary_steps += 1;
            } else {
                stationary_steps = 0;
            }
            if !train.notes().is_empty() || stationary_steps >= (2.0 / TIME_STEP) as usize {
                return Some(finish_preview(
                    recorder,
                    force_peak_recorder,
                    PreviewStatus::Stalled,
                    false,
                    true,
                    flags,
                    &source_circuit,
                ));
            }
        }
        Some(finish_preview(
            recorder,
            force_peak_recorder,
            PreviewStatus::TimedOut,
            false,
            false,
            flags,
            &source_circuit,
        ))
    }
}

fn finish_preview(
    recorder: TestRunRecorder,
    force_peak_recorder: PreviewForcePeakRecorder,
    status: PreviewStatus,
    completed: bool,
    rolled_back: bool,
    force_limit_exceeded: Vec<bool>,
    source_circuit: &Circuit,
) -> TrackPreview {
    let report = recorder.finish(completed, rolled_back);
    let force_envelope_samples = force_peak_recorder
        .force_envelope_samples()
        .into_iter()
        .filter(|sample| sample.distance < source_circuit.length)
        .collect();
    let piece_force_peaks = force_peak_recorder
        .piece_force_peaks(&report)
        .into_iter()
        .filter(|peaks| peaks.piece_index < source_circuit.pieces.len())
        .collect();
    TrackPreview {
        report,
        force_envelope_samples,
        piece_force_peaks,
        status,
        force_limit_exceeded,
    }
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

/// Builds a horizontal, unbanked extension from the open connector's position and heading.
/// The first segment resets slope and bank at the connector to model unknown track as level.
pub fn virtual_level_runout(end: TrackState, minimum_length: f64) -> Vec<Piece> {
    assert!(minimum_length.is_finite() && minimum_length > 0.0);
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog has a straight piece");
    let mut entry = TrackState {
        slope: Slope::Level,
        bank: Bank::Level,
        horizontal_curvature: HorizontalCurvature::Straight,
        vertical_curvature: VerticalCurvature::Straight,
        ..end
    };
    let mut runout = Vec::new();
    let mut length = 0.0;
    while length < minimum_length {
        let piece = straight
            .at(entry)
            .expect("level straight can be placed at its level connector");
        length += piece.length();
        entry = piece.exit;
        runout.push(piece);
    }
    runout
}

/// Simulate a train from the station across placed track and a virtual run-out.
/// Reports include only placed track. An open-track preview completes after the
/// full train has cleared the placed track; a closed preview completes after one lap.
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
                force_limit_exceeded: Vec::new(),
            });
        }
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        force_envelope_samples: Vec::new(),
        piece_force_peaks: Vec::new(),
        status: PreviewStatus::TimedOut,
        force_limit_exceeded: Vec::new(),
    })
}
