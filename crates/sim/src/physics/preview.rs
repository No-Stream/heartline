use super::{Circuit, ForceEvaluator, PhysicsConfig, TIME_STEP, TestRun, TestRunRecorder, Train};
use crate::state_hash::StateHasher;
use crate::track::{ComfortableForceLimits, Piece, PieceKind, TrainConfiguration};

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
    pub force_limit_exceeded: Vec<bool>,
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
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
        limits: ComfortableForceLimits,
    ) -> Option<TrackPreview> {
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
        let hashes: Vec<_> = pieces
            .iter()
            .map(|piece| {
                let mut hash = StateHasher::new();
                piece.hash_into(&mut hash);
                hash.finish()
            })
            .collect();
        let parameters = postcard::to_allocvec(&(configuration, config, limits))
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
        let saved = self.snapshots.last().cloned();
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
            let mut train = snapshot.train;
            train.controls.resize(pieces.len(), None);
            let mut recorder = snapshot.recorder;
            recorder.rebind_preview_circuit(&circuit, configuration, limits);
            let mut flags = snapshot.flags;
            flags.resize(length.ceil() as usize, false);
            (
                train,
                recorder,
                snapshot.evaluators,
                flags,
                snapshot.previous_flags,
                snapshot.furthest_flag_metres,
                snapshot.stationary_steps,
                snapshot.steps,
            )
        } else {
            let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
            (
                Train::new(&circuit, configuration, config, station_distance, 0.0),
                TestRunRecorder::for_circuit_with_train_and_limits(&circuit, configuration, limits),
                vec![ForceEvaluator::new(limits, TIME_STEP); usize::from(configuration.cars)],
                vec![false; length.ceil() as usize],
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
            let crossed_end = previous_train.distance() < length && train.distance() >= length;
            let snapshot_boundary = if crossed_end {
                pieces.len()
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
            let reached_end = train.distance() >= length;
            if reached_end && circuit.open && !readings.is_empty() {
                readings[0][0].distance = length - 1e-6;
            }
            if !reached_end || circuit.open {
                let front: Vec<_> = readings.into_iter().map(|frame| vec![frame[0]]).collect();
                recorder.record(&front);
            }
            if reached_end {
                return Some(TrackPreview {
                    report: recorder.finish(true, false),
                    status: PreviewStatus::ReachedEnd,
                    force_limit_exceeded: flags,
                });
            }
            if train.speed().abs() < 0.01 {
                stationary_steps += 1;
            } else {
                stationary_steps = 0;
            }
            if !train.notes().is_empty() || stationary_steps >= (2.0 / TIME_STEP) as usize {
                return Some(TrackPreview {
                    report: recorder.finish(false, true),
                    status: PreviewStatus::Stalled,
                    force_limit_exceeded: flags,
                });
            }
        }
        Some(TrackPreview {
            report: recorder.finish(false, false),
            status: PreviewStatus::TimedOut,
            force_limit_exceeded: flags,
        })
    }
}

/// Simulate a train from the station to the end of the placed track.
/// Samples beyond a stall are omitted. A completed report means the front car
/// reached the end, rather than that the ride completed a circuit.
pub fn preview_open_track(
    pieces: &[Piece],
    configuration: &TrainConfiguration,
    config: PhysicsConfig,
) -> Option<TrackPreview> {
    PreviewCache::default().preview(
        pieces,
        configuration,
        config,
        crate::track::hybrid_model().comfortable_force_limits,
    )
}

#[cfg(test)]
pub(super) fn preview_at_step(
    pieces: &[Piece],
    configuration: &TrainConfiguration,
    config: PhysicsConfig,
    step_limit: usize,
    time_step: f64,
) -> Option<TrackPreview> {
    assert_eq!(time_step, TIME_STEP);
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
    let mut recorder = TestRunRecorder::for_circuit_with_train(&circuit, configuration);
    recorder.set_time_step(time_step);
    let mut stationary_steps = 0;
    for _ in 0..step_limit {
        train.step_preview(&circuit, time_step);
        if train.distance() >= length {
            if circuit.open {
                let mut readings = train.take_readings();
                readings[0][0].distance = length - 1e-6;
                recorder.record(&readings);
            }
            return Some(TrackPreview {
                report: recorder.finish(true, false),
                status: PreviewStatus::ReachedEnd,
                force_limit_exceeded: Vec::new(),
            });
        }
        recorder.record(&train.take_readings());
        if train.speed().abs() < 0.01 {
            stationary_steps += 1;
        } else {
            stationary_steps = 0;
        }
        if !train.notes().is_empty() || stationary_steps >= (2.0 / time_step) as usize {
            return Some(TrackPreview {
                report: recorder.finish(false, true),
                status: PreviewStatus::Stalled,
                force_limit_exceeded: Vec::new(),
            });
        }
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        status: PreviewStatus::TimedOut,
        force_limit_exceeded: Vec::new(),
    })
}
