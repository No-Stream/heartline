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
    let circuit = if closed {
        Circuit::new(pieces.to_vec()).expect("connected closed track")
    } else {
        Circuit::open(pieces.to_vec()).expect("connected open track")
    };
    let length = circuit.length();
    let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
    let mut train = Train::new(&circuit, configuration, config, station_distance, 0.0);
    let mut recorder = TestRunRecorder::for_circuit_with_train(&circuit, configuration);
    let mut accelerations = Vec::new();
    let mut stationary_steps = 0;
    for _ in 0..step_limit {
        train.step_preview(&circuit, time_step);
        let distance = train.distance().min(length);
        if accelerations
            .last()
            .is_none_or(|(previous, _)| distance > *previous)
        {
            accelerations.push((distance, train.last_acceleration));
        }
        if train.distance() >= length {
            if circuit.open {
                let mut readings = train.take_readings();
                readings[0][0].distance = length - 1e-6;
                recorder.record(&readings);
            }
            if time_step != super::TIME_STEP {
                correct_sample_forces(recorder.samples_mut(), &circuit, &accelerations);
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
            if time_step != super::TIME_STEP {
                correct_sample_forces(recorder.samples_mut(), &circuit, &accelerations);
            }
            return Some(TrackPreview {
                report: recorder.finish(false, true),
                status: PreviewStatus::Stalled,
            });
        }
    }
    if time_step != super::TIME_STEP {
        correct_sample_forces(recorder.samples_mut(), &circuit, &accelerations);
    }
    Some(TrackPreview {
        report: recorder.finish(false, false),
        status: PreviewStatus::TimedOut,
    })
}

fn correct_sample_forces<'a>(
    samples: impl Iterator<Item = &'a mut super::TestSample>,
    circuit: &Circuit,
    accelerations: &[(f64, f64)],
) {
    for sample in samples {
        let upper = accelerations.partition_point(|(distance, _)| *distance < sample.distance);
        let acceleration = if upper == 0 {
            accelerations[0].1
        } else if upper == accelerations.len() {
            accelerations[upper - 1].1
        } else {
            let (first_distance, first_acceleration) = accelerations[upper - 1];
            let (second_distance, second_acceleration) = accelerations[upper];
            let fraction = (sample.distance - first_distance) / (second_distance - first_distance);
            first_acceleration + fraction * (second_acceleration - first_acceleration)
        };
        let (index, local) = circuit.locate(sample.distance);
        let force = super::rider_g(&circuit.pieces[index], local, sample.speed, acceleration);
        sample.vertical_g = force.vertical;
        sample.lateral_g = force.lateral;
        sample.longitudinal_g = force.longitudinal;
        sample.airtime = force.vertical < 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{demo_circuit, rider_g};
    use crate::track::hybrid_model;

    #[test]
    fn preview_samples_evaluate_force_at_the_reported_position() {
        let circuit = demo_circuit();
        let configuration = hybrid_model().train;
        let mut report =
            preview_open_track(circuit.pieces(), &configuration, PhysicsConfig::default())
                .unwrap()
                .report;
        let before = report.samples.clone();
        correct_sample_forces(
            report.samples.iter_mut(),
            &circuit,
            &[(0.0, 0.0), (circuit.length(), 0.0)],
        );
        let mut changed = false;
        for (sample, original) in report.samples.iter().zip(&before) {
            let (index, local) = circuit.locate(sample.distance);
            let expected = rider_g(&circuit.pieces[index], local, sample.speed, 0.0);
            assert!((sample.longitudinal_g - expected.longitudinal).abs() < 1e-9);
            changed |= (sample.longitudinal_g - original.longitudinal_g).abs() > 1e-6;
        }
        assert!(changed);
    }
}
