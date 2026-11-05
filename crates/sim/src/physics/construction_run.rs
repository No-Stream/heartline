use super::coaster_run::merge_worst_car_statistics;
use super::{CarReading, Circuit, TestRun, TestRunRecorder};
use crate::state_hash::StateHasher;
use crate::track::proximity::TrainPosition;

/// Records each car independently for a construction test.
pub struct ConstructionTestRunRecorder {
    cars: Vec<TestRunRecorder>,
}

impl ConstructionTestRunRecorder {
    pub fn for_circuit(circuit: &Circuit, car_count: usize) -> Self {
        Self::for_circuit_with_limits(
            circuit,
            car_count,
            crate::track::hybrid_model().comfortable_force_limits,
        )
    }

    pub fn for_circuit_with_limits(
        circuit: &Circuit,
        car_count: usize,
        limits: crate::track::ComfortableForceLimits,
    ) -> Self {
        assert!(car_count > 0);
        Self {
            cars: (0..car_count)
                .map(|_| TestRunRecorder::for_circuit_with_limits(circuit, limits))
                .collect(),
        }
    }

    #[cfg(test)]
    fn new(circuit_length: f64, car_count: usize) -> Self {
        assert!(car_count > 0);
        Self {
            cars: (0..car_count)
                .map(|_| TestRunRecorder::new(circuit_length))
                .collect(),
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"ConstructionTestRunRecorder-v1");
        hash.write_len(self.cars.len());
        for car in &self.cars {
            car.hash_into(hash);
        }
    }

    pub fn record(&mut self, frames: &[Vec<CarReading>]) {
        for frame in frames {
            if frame.is_empty() {
                continue;
            }
            assert_eq!(
                frame.len(),
                self.cars.len(),
                "train car count changed during test"
            );
            for (recorder, &reading) in self.cars.iter_mut().zip(frame) {
                recorder.record(&[vec![reading]]);
            }
        }
    }

    pub fn record_dueling(
        &mut self,
        circuit: &Circuit,
        reading: CarReading,
        others: &[TrainPosition],
    ) {
        self.cars[0].record_dueling(circuit, reading, others);
    }

    pub fn finish(self, completed: bool, rolled_back: bool) -> TestRun {
        self.finish_with_front_car(completed, rolled_back).0
    }

    pub fn finish_with_front_car(self, completed: bool, rolled_back: bool) -> (TestRun, TestRun) {
        let reports: Vec<_> = self
            .cars
            .into_iter()
            .map(|car| car.finish(completed, rolled_back))
            .collect();
        let front_car_report = reports[0].clone();
        let mut report = front_car_report.clone();
        report.average_car_ratings(&reports);
        report.statistics = reports
            .iter()
            .map(|car| car.statistics)
            .reduce(merge_worst_car_statistics)
            .expect("train has cars");
        report.notes.push("Ratings average all cars. The track ribbon, proximity events and ride advice describe the front car; statistics report the most extreme value across cars.".to_owned());
        (report, front_car_report)
    }
}

#[cfg(test)]
mod tests {
    use super::ConstructionTestRunRecorder;
    use crate::physics::{CarReading, GForces, TestRunRecorder};

    fn reading(distance: f64, vertical: f64, lateral: f64) -> CarReading {
        CarReading {
            distance,
            speed: 10.0,
            g: GForces {
                vertical,
                lateral,
                longitudinal: 0.0,
            },
            airtime: vertical < 0.0,
            height_m: 0.0,
            inverted: false,
            roll: 0.0,
            piece_index: 0,
        }
    }

    fn scoring_reading(distance: f64, vertical: f64, lateral: f64) -> CarReading {
        CarReading {
            speed: 30.0,
            ..reading(distance, vertical, lateral)
        }
    }

    #[test]
    fn construction_score_includes_sustained_force_from_rear_car() {
        let mut quiet = ConstructionTestRunRecorder::new(10.0, 2);
        let mut force = ConstructionTestRunRecorder::new(10.0, 2);
        for recorder in [&mut quiet, &mut force] {
            recorder.record(&[vec![
                scoring_reading(0.0, 1.0, 0.0),
                scoring_reading(0.0, 1.0, 0.0),
            ]]);
        }
        for _ in 0..400 {
            quiet.record(&[vec![
                scoring_reading(0.0, 1.0, 0.0),
                scoring_reading(0.0, 1.0, 0.0),
            ]]);
            force.record(&[vec![
                scoring_reading(0.0, 1.0, 0.0),
                scoring_reading(0.0, 8.0, 3.0),
            ]]);
        }
        for distance in 1..10 {
            for recorder in [&mut quiet, &mut force] {
                recorder.record(&[vec![
                    scoring_reading(distance as f64, 1.0, 0.0),
                    scoring_reading(distance as f64, 1.0, 0.0),
                ]]);
            }
        }
        let quiet = quiet.finish(true, false);
        let force = force.finish(true, false);
        assert!(force.excitement < quiet.excitement);
        assert!(force.notes.iter().any(|note| note.contains("F2291")));
        assert!(!quiet.notes.iter().any(|note| note.contains("F2291")));
    }

    #[test]
    fn hash_includes_rear_car_history() {
        use crate::state_hash::StateHasher;
        let mut first = ConstructionTestRunRecorder::new(10.0, 2);
        let mut second = ConstructionTestRunRecorder::new(10.0, 2);
        first.record(&[vec![reading(0.0, 1.0, 0.0), reading(0.0, 1.0, 0.0)]]);
        second.record(&[vec![reading(0.0, 1.0, 0.0), reading(0.0, 2.0, 0.0)]]);
        let mut first_hash = StateHasher::new();
        let mut second_hash = StateHasher::new();
        first.hash_into(&mut first_hash);
        second.hash_into(&mut second_hash);
        assert_ne!(first_hash.finish(), second_hash.finish());
    }

    #[test]
    fn proximity_update_preserves_average_base_ratings() {
        use crate::track::Vec3;
        use crate::track::proximity::{ProximityEvent, ProximityKind};
        let mut recorder = ConstructionTestRunRecorder::new(10.0, 2);
        for distance in 0..10 {
            recorder.record(&[vec![
                scoring_reading(distance as f64, 4.0, 0.0),
                scoring_reading(distance as f64, 1.0, 3.0),
            ]]);
        }
        let mut report = recorder.finish(true, false);
        let excitement = report.excitement;
        let intensity = report.intensity;
        report.add_proximity_events(&[ProximityEvent {
            piece_index: 0,
            kind: ProximityKind::NearMiss,
            position: Vec3::new(0.0, 0.0, 0.0),
            strength: 1.0,
            distance: 1.0,
        }]);
        assert!((report.excitement - excitement - (1.0 - (-0.5_f64).exp())).abs() < 1e-10);
        assert!((report.intensity - intensity).abs() < 1e-10);
    }

    #[test]
    fn recorder_reports_force_extremes_across_the_whole_train() {
        let mut recorder = ConstructionTestRunRecorder::new(10.0, 2);
        let front = reading(0.0, 1.0, 0.1);
        let mut rear = reading(0.0, -1.2, -1.8);
        rear.speed = 12.0;
        rear.g.longitudinal = -0.4;
        recorder.record(&[vec![front, rear]]);
        recorder.record(&[vec![reading(1.0, 1.0, 0.0), reading(1.0, 1.0, 0.0)]]);
        let report = recorder.finish(false, false);
        assert_eq!(report.statistics.max_negative_vertical_g, -1.2);
        assert_eq!(report.statistics.max_lateral_g, 1.8);
        assert_eq!(report.statistics.max_speed, 12.0);
    }

    #[test]
    fn construction_report_averages_real_cars_and_keeps_train_extremes() {
        let mut construction = ConstructionTestRunRecorder::new(10.0, 2);
        let mut cars = [TestRunRecorder::new(10.0), TestRunRecorder::new(10.0)];
        for distance in 0..10 {
            let frame = vec![
                reading(distance as f64, 4.0, 0.0),
                reading(distance as f64, 1.0, 3.0),
            ];
            construction.record(std::slice::from_ref(&frame));
            for (recorder, car) in cars.iter_mut().zip(frame) {
                recorder.record(&[vec![car]]);
            }
        }
        let report = construction.finish(true, false);
        let [front, rear] = cars.map(|recorder| recorder.finish(true, false));
        assert_eq!(report.samples, front.samples);
        assert!((report.excitement - (front.excitement + rear.excitement) / 2.0).abs() < 1e-10);
        assert!((report.intensity - (front.intensity + rear.intensity) / 2.0).abs() < 1e-10);
        assert!((report.nausea - (front.nausea + rear.nausea) / 2.0).abs() < 1e-10);
        assert_eq!(report.statistics.max_positive_vertical_g, 4.0);
        assert_eq!(report.statistics.max_lateral_g, 3.0);
        assert!(report.statistics.strong_lateral_g_seconds > 0.0);
    }
}
