use super::{Circuit, PhysicsConfig, TestRun, TestRunRecorder, Train};
use crate::track::{Piece, PieceKind, TrainConfiguration};

const PREVIEW_TIME_STEP: f64 = 0.05;
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
}

/// Simulate a train from the station to the end of the placed track.
/// Samples beyond a stall are omitted. A completed report means the front car
/// reached the end, rather than that the ride completed a circuit.
pub fn preview_open_track(
    pieces: &[Piece],
    configuration: &TrainConfiguration,
    config: PhysicsConfig,
) -> Option<TrackPreview> {
    preview_at_step(
        pieces,
        configuration,
        config,
        (PREVIEW_MAX_SECONDS / PREVIEW_TIME_STEP) as usize,
        PREVIEW_TIME_STEP,
    )
}

pub(super) fn preview_at_step(
    pieces: &[Piece],
    configuration: &TrainConfiguration,
    config: PhysicsConfig,
    step_limit: usize,
    time_step: f64,
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
    let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
    let mut train = Train::new(&circuit, configuration, config, station_distance, 0.0);
    let mut recorder = TestRunRecorder::for_circuit_with_train(&circuit, configuration);
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
            });
        }
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        status: PreviewStatus::TimedOut,
    })
}
