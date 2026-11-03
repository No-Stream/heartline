use super::{Circuit, PhysicsConfig, TestRun, TestRunRecorder, Train};
use crate::track::proximity::TrainPosition;
use crate::track::{PieceKind, TrainConfiguration};

/// A single dispatched train, from the station through one circuit or recovery.
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
    pub fn new(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Self {
        Self::with_reports(circuit, configuration, config, true)
    }

    pub(super) fn new_without_reports(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Self {
        Self::with_reports(circuit, configuration, config, false)
    }

    fn with_reports(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
        record_reports: bool,
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
                    .map(|_| TestRunRecorder::new(circuit.length()))
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
    pub fn step(&mut self, circuit: &Circuit) -> Option<CoasterOutcome> {
        self.step_with_other_trains(circuit, &[])
    }

    pub fn step_with_other_trains(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
    ) -> Option<CoasterOutcome> {
        self.step_with_block_brake(circuit, others, &[], false)
    }

    pub fn step_with_block_brake(
        &mut self,
        circuit: &Circuit,
        others: &[TrainPosition],
        brake_pieces: &[usize],
        hold: bool,
    ) -> Option<CoasterOutcome> {
        let recorders = self.recorders.as_mut()?;
        if recorders.is_empty() {
            self.train
                .step_block_brake_without_readings(circuit, brake_pieces, hold);
        } else {
            self.train.step_block_brake(circuit, brake_pieces, hold);
            for frame in self.train.take_readings() {
                for (recorder, reading) in recorders.iter_mut().zip(frame) {
                    recorder.record_dueling(circuit, reading, others);
                    recorder.record(&[vec![reading]]);
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
        CoasterOutcome {
            reports: self
                .recorders
                .take()
                .expect("coaster run already ended")
                .into_iter()
                .map(|recorder| recorder.finish(completed, recovered))
                .collect(),
            recovered,
        }
    }
}
