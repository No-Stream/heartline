use super::{Circuit, PhysicsConfig, TestRun, TestRunRecorder, Train};
use crate::state_hash::StateHasher;
use crate::track::proximity::TrainPosition;
use crate::track::{ComfortableForceLimits, PieceKind, TrainConfiguration};

/// A single dispatched train, from the station through one circuit or recovery.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CoasterRun {
    train: Train,
    recorders: Option<Vec<TestRunRecorder>>,
    station_distance: f64,
    circuit_length: f64,
    capacity: usize,
    stationary_steps: usize,
}

pub struct CoasterOutcome {
    pub reports: Vec<TestRun>,
    pub recovered: bool,
}

impl CoasterRun {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"CoasterRun-v1");
        self.train.hash_into(hash);
        hash.write_option(self.recorders.as_ref(), |hash, recorders| {
            hash.write_len(recorders.len());
            for recorder in recorders {
                recorder.hash_into(hash);
            }
        });
        hash.write_f64(self.station_distance);
        hash.write_f64(self.circuit_length);
        hash.write_len(self.capacity);
        hash.write_len(self.stationary_steps);
    }

    pub fn new(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Self {
        Self::with_reports(circuit, configuration, config, true, None)
    }

    pub fn new_with_limits(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
        limits: ComfortableForceLimits,
    ) -> Self {
        Self::with_reports(circuit, configuration, config, true, Some(limits))
    }

    pub(super) fn new_without_reports(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Self {
        Self::with_reports(circuit, configuration, config, false, None)
    }

    fn with_reports(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
        record_reports: bool,
        limits: Option<ComfortableForceLimits>,
    ) -> Self {
        let station = circuit
            .pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)
            .expect("circuit has a station");
        let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
        let capacity = usize::from(configuration.cars)
            * usize::from(configuration.rows_per_car)
            * usize::from(configuration.seats_per_row);
        assert!(capacity > 0);
        Self {
            train: Train::new(circuit, configuration, config, station_distance, 0.0),
            recorders: Some(
                (0..if record_reports {
                    configuration.cars
                } else {
                    0
                })
                    .map(|_| match limits {
                        Some(limits) => TestRunRecorder::for_circuit_with_train_and_limits(
                            circuit,
                            configuration,
                            limits,
                        ),
                        None => TestRunRecorder::for_circuit_with_train(circuit, configuration),
                    })
                    .collect(),
            ),
            station_distance,
            circuit_length: circuit.length(),
            capacity,
            stationary_steps: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn train(&self) -> &Train {
        &self.train
    }
    pub(super) fn stop_for_interlock(&mut self) {
        self.train.speed = 0.0;
    }
    #[cfg(test)]
    pub(super) fn train_mut(&mut self) -> &mut Train {
        &mut self.train
    }

    /// Advances one physics step; returns the outcome exactly once.
    pub fn step(&mut self, circuit: &Circuit, model_key: &str) -> Option<CoasterOutcome> {
        self.step_with_other_trains(circuit, &[], model_key)
    }

    pub fn step_with_other_trains(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
        model_key: &str,
    ) -> Option<CoasterOutcome> {
        self.step_with_block_brake(circuit, others, &[], false, model_key)
    }

    pub fn step_with_block_brake(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
        brake_pieces: &[usize],
        hold: bool,
        model_key: &str,
    ) -> Option<CoasterOutcome> {
        self.step_internal(circuit, others, brake_pieces, hold, Some(model_key))
    }

    pub(super) fn step_without_reports(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
        brake_pieces: &[usize],
        hold: bool,
    ) -> Option<CoasterOutcome> {
        self.step_internal(circuit, others, brake_pieces, hold, None)
    }

    fn step_internal(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
        brake_pieces: &[usize],
        hold: bool,
        model_key: Option<&str>,
    ) -> Option<CoasterOutcome> {
        let recorders = self.recorders.as_mut()?;
        assert!(
            model_key.is_some() || recorders.is_empty(),
            "recording requires a model key"
        );
        if recorders.is_empty() {
            self.train
                .step_block_brake_without_readings(circuit, brake_pieces, hold);
        } else {
            self.train.step_block_brake(circuit, brake_pieces, hold);
            for frame in self.train.take_readings() {
                for (recorder, reading) in recorders.iter_mut().zip(frame) {
                    recorder.record_dueling(circuit, reading, others);
                    recorder.record(
                        &[vec![reading]],
                        model_key.expect("recording requires a model key"),
                    );
                }
            }
        }
        if hold && brake_pieces.contains(&circuit.piece_index(self.train.distance())) {
            self.stationary_steps = 0;
        } else if self.train.speed().abs() < 0.01 {
            self.stationary_steps += 1;
        } else {
            self.stationary_steps = 0;
        }
        if self.stationary_steps >= (2.0 / super::TIME_STEP) as usize {
            return Some(self.recover());
        }
        if !self.train.notes().is_empty() {
            return Some(self.finish(false, true));
        }
        if self.train.distance() >= self.station_distance + self.circuit_length {
            self.train.distance = self.station_distance;
            self.train.speed = 0.0;
            return Some(self.finish(true, false));
        }
        None
    }

    /// Recover the train to its station when its track is edited or removed.
    pub fn recover(&mut self) -> CoasterOutcome {
        self.train.distance = self.station_distance;
        self.train.speed = 0.0;
        self.finish(false, true)
    }

    fn finish(&mut self, completed: bool, recovered: bool) -> CoasterOutcome {
        let mut reports: Vec<_> = self
            .recorders
            .take()
            .expect("coaster run already ended")
            .into_iter()
            .map(|recorder| recorder.finish(completed, recovered))
            .collect();
        if let Some(worst_car_statistics) = reports
            .iter()
            .map(|report| report.statistics)
            .reduce(merge_worst_car_statistics)
        {
            for report in &mut reports {
                report.statistics = worst_car_statistics;
            }
        }
        CoasterOutcome { reports, recovered }
    }
}

pub(super) fn merge_worst_car_statistics(
    first: super::testing::RideStatistics,
    second: super::testing::RideStatistics,
) -> super::testing::RideStatistics {
    use super::testing::RideStatistics;
    RideStatistics {
        ejector_airtime_seconds: first
            .ejector_airtime_seconds
            .max(second.ejector_airtime_seconds),
        floater_airtime_seconds: first
            .floater_airtime_seconds
            .max(second.floater_airtime_seconds),
        airtime_moments: first.airtime_moments.max(second.airtime_moments),
        strong_positive_g_seconds: first
            .strong_positive_g_seconds
            .max(second.strong_positive_g_seconds),
        longest_strong_positive_g_seconds: first
            .longest_strong_positive_g_seconds
            .max(second.longest_strong_positive_g_seconds),
        strong_lateral_g_seconds: first
            .strong_lateral_g_seconds
            .max(second.strong_lateral_g_seconds),
        max_positive_vertical_g: first
            .max_positive_vertical_g
            .max(second.max_positive_vertical_g),
        max_negative_vertical_g: first
            .max_negative_vertical_g
            .min(second.max_negative_vertical_g),
        max_lateral_g: first.max_lateral_g.max(second.max_lateral_g),
        max_forward_g: first.max_forward_g.max(second.max_forward_g),
        max_braking_g: first.max_braking_g.max(second.max_braking_g),
        max_speed: first.max_speed.max(second.max_speed),
        average_speed: first.average_speed.min(second.average_speed),
        duration_seconds: first.duration_seconds.max(second.duration_seconds),
        track_length: first.track_length.max(second.track_length),
        max_height: first.max_height.max(second.max_height),
        biggest_drop: first.biggest_drop.max(second.biggest_drop),
        steepest_drop_angle_degrees: first
            .steepest_drop_angle_degrees
            .max(second.steepest_drop_angle_degrees),
        inversions: first.inversions.max(second.inversions),
        inverted_seconds: first.inverted_seconds.max(second.inverted_seconds),
        drops: first.drops.max(second.drops),
        launches: first.launches.max(second.launches),
        launch_peak_speed: first.launch_peak_speed.max(second.launch_peak_speed),
    }
}

#[cfg(test)]
mod statistics_tests {
    use super::merge_worst_car_statistics;
    use crate::physics::testing::RideStatistics;

    #[test]
    fn run_statistics_use_the_worst_value_across_cars() {
        let first = RideStatistics {
            ejector_airtime_seconds: 1.0,
            average_speed: 20.0,
            max_positive_vertical_g: 4.0,
            max_negative_vertical_g: -0.8,
            max_lateral_g: 1.0,
            ..RideStatistics::default()
        };
        let second = RideStatistics {
            ejector_airtime_seconds: 2.0,
            average_speed: 10.0,
            max_positive_vertical_g: 3.0,
            max_negative_vertical_g: -1.2,
            max_lateral_g: 1.5,
            ..RideStatistics::default()
        };

        let worst = merge_worst_car_statistics(first, second);
        assert_eq!(worst.ejector_airtime_seconds, 2.0);
        assert_eq!(worst.average_speed, 10.0);
        assert_eq!(worst.max_positive_vertical_g, 4.0);
        assert_eq!(worst.max_negative_vertical_g, -1.2);
        assert_eq!(worst.max_lateral_g, 1.5);
    }
}
