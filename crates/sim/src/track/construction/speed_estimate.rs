use super::*;
use crate::physics::{GRAVITY, PhysicsConfig, PreviewStatus, TrackPreview};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndSpeedSource {
    Simulated,
    Stalled,
    TimedOut,
    Energy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndSpeedEstimate {
    pub speed: Option<f64>,
    pub source: EndSpeedSource,
}

impl Construction {
    /// Use the existing construction preview when available. A timeout does not
    /// establish whether the train can reach the end.
    pub fn estimated_end_speed(&self, preview: Option<&TrackPreview>) -> EndSpeedEstimate {
        if let Some(preview) = preview {
            return match preview.status {
                PreviewStatus::ReachedEnd => EndSpeedEstimate {
                    speed: Some(
                        preview
                            .report
                            .samples
                            .last()
                            .expect("reached end without a sample")
                            .speed,
                    ),
                    source: EndSpeedSource::Simulated,
                },
                PreviewStatus::Stalled => EndSpeedEstimate {
                    speed: Some(0.0),
                    source: EndSpeedSource::Stalled,
                },
                PreviewStatus::TimedOut => EndSpeedEstimate {
                    speed: None,
                    source: EndSpeedSource::TimedOut,
                },
            };
        }

        let config = PhysicsConfig::default();
        let mut speed_squared = 0.0_f64;
        for piece in &self.pieces {
            speed_squared = (speed_squared
                - 2.0 * GRAVITY * (piece.exit.position().z - piece.entry.position().z)
                - 2.0 * config.rolling_resistance * GRAVITY * piece.length())
            .max(0.0);
            match piece.kind {
                PieceKind::Lift => speed_squared = config.lift_speed.powi(2),
                PieceKind::Launch => {
                    let target_speed = piece.launch_target_speed.unwrap_or_else(|| {
                        self.model
                            .train
                            .launch
                            .expect("launch piece requires launch parameters")
                            .target_speed
                    });
                    speed_squared = target_speed.powi(2);
                }
                _ => {}
            }
        }
        EndSpeedEstimate {
            speed: Some(speed_squared.sqrt()),
            source: EndSpeedSource::Energy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{CoasterRun, PhysicsConfig, demo_circuit, preview_open_track};
    use crate::track::hybrid_model;

    #[test]
    fn demo_first_drop_estimate_matches_full_run_within_ten_percent() {
        let circuit = demo_circuit();
        let model = hybrid_model();
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        };
        // The first drop ends where it levels out into the first hill.
        let valley_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == PieceId::GentleDownToLevel)
            .unwrap()
            + 1;
        let pieces = &circuit.pieces()[..valley_index];
        let mut construction = Construction::new(pieces[0].entry, model.clone());
        construction.pieces = pieces.to_vec();
        let preview = preview_open_track(pieces, &model.train, config).unwrap();
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        let estimate = construction.estimated_end_speed(Some(&preview));
        let sample = preview.report.samples.last().unwrap();
        let mut run = CoasterRun::new(&circuit, &model.train, config);
        let outcome = (0..200_000).find_map(|_| run.step(&circuit)).unwrap();
        let measured = outcome.reports[0]
            .samples
            .iter()
            .find(|other| other.distance == sample.distance)
            .unwrap()
            .speed;
        let estimated = estimate.speed.unwrap();
        assert!(
            (estimated - measured).abs() / measured < 0.1,
            "estimated={estimated}, simulated={measured}"
        );
        eprintln!("demo first drop estimated={estimated:.4} m/s, full run={measured:.4} m/s");
    }

    #[test]
    fn uses_simulation_when_available_and_preserves_stall_and_timeout() {
        let circuit = demo_circuit();
        let model = hybrid_model();
        let pieces = &circuit.pieces()[..7];
        let mut construction = Construction::new(pieces[0].entry, model.clone());
        construction.pieces = pieces.to_vec();
        let mut preview =
            preview_open_track(pieces, &model.train, PhysicsConfig::default()).unwrap();
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        let speed = preview.report.samples.last().unwrap().speed;
        assert_eq!(
            construction.estimated_end_speed(Some(&preview)),
            EndSpeedEstimate {
                speed: Some(speed),
                source: EndSpeedSource::Simulated
            }
        );
        preview.status = PreviewStatus::Stalled;
        assert_eq!(
            construction.estimated_end_speed(Some(&preview)),
            EndSpeedEstimate {
                speed: Some(0.0),
                source: EndSpeedSource::Stalled
            }
        );
        preview.status = PreviewStatus::TimedOut;
        assert_eq!(
            construction.estimated_end_speed(Some(&preview)),
            EndSpeedEstimate {
                speed: None,
                source: EndSpeedSource::TimedOut
            }
        );
    }

    #[test]
    fn energy_fallback_uses_lift_launch_height_and_resistance() {
        let circuit = demo_circuit();
        let model = hybrid_model();
        let pieces = &circuit.pieces()[..7];
        let mut construction = Construction::new(pieces[0].entry, model);
        construction.pieces = pieces.to_vec();
        let config = PhysicsConfig::default();
        let mut lift = pieces[1].clone();
        lift.kind = PieceKind::Lift;
        let mut descent = pieces[2].clone();
        descent.kind = PieceKind::Track;
        descent.entry.z = 10;
        descent.exit.z = 8;
        let expected = (config.lift_speed.powi(2) + 2.0 * GRAVITY * 4.0
            - 2.0 * config.rolling_resistance * GRAVITY * descent.length())
        .sqrt();
        construction.pieces = vec![lift, descent];
        let estimate = construction.estimated_end_speed(None);
        assert_eq!(estimate.source, EndSpeedSource::Energy);
        assert!((estimate.speed.unwrap() - expected).abs() < 1e-10);

        let mut launch = pieces[0].clone();
        launch.kind = PieceKind::Launch;
        launch.launch_target_speed = Some(27.0);
        construction.pieces.push(launch);
        assert_eq!(construction.estimated_end_speed(None).speed, Some(27.0));
        construction.pieces.clear();
        assert_eq!(construction.estimated_end_speed(None).speed, Some(0.0));
    }
}
