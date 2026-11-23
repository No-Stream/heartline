//! Headless test runs that rate a coaster without animating a builder test train.

use super::tick::{
    add_proximity_and_theming, other_coaster_train_positions, step_demo_train,
    train_positions_for_train,
};
use super::{CoasterId, Park};
use crate::physics::{Circuit, ConstructionTestRunRecorder, PhysicsConfig, TestRun, Train};
use crate::track::proximity::TrainPosition;
use crate::track::{CoasterModel, TrainConfiguration, hybrid_model};

const MAXIMUM_TEST_STEPS: u32 = 200_000;
const STOPPED_STEPS_BEFORE_GIVING_UP: u32 = 200;
const COMPLETED_MESSAGE: &str = "Train completed a lap.";
const RECOVERED_MESSAGE: &str = "Train recovered to the station after rollback.";
const STOPPED_MESSAGE: &str = "Train stopped before completing a lap.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RideKey {
    Demo,
    Player,
}

/// Runs one test train round the circuit at full physics resolution, as the builder's animated test does.
/// `demo_circuit_and_train` is the demo train other trains can duel with; it is stepped in lockstep.
fn run_test_to_outcome(
    circuit: &Circuit,
    train_configuration: &TrainConfiguration,
    model: &CoasterModel,
    model_key: &str,
    mut demo_circuit_and_train: Option<(&Circuit, Train, TrainConfiguration)>,
    other_coaster_trains: &[TrainPosition],
) -> Result<TestRun, String> {
    let mut train = Train::new(
        circuit,
        train_configuration,
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
        0.0,
        2.0,
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
        circuit,
        train_configuration,
        model,
    );
    let mut slow_steps = 0;
    for _ in 0..MAXIMUM_TEST_STEPS {
        let mut other_trains = other_coaster_trains.to_vec();
        if let Some((demo_circuit, demo_train, demo_configuration)) = &mut demo_circuit_and_train {
            other_trains.extend(train_positions_for_train(
                demo_circuit,
                demo_train,
                demo_configuration,
            ));
            step_demo_train(demo_circuit, demo_train);
        }
        let was_recovered = !train.notes().is_empty();
        let previous_distance = train.distance();
        train.step(circuit);
        let mut outcome = None;
        if !was_recovered && !train.notes().is_empty() {
            outcome = Some(RECOVERED_MESSAGE);
        } else if previous_distance < circuit.length() && train.distance() >= circuit.length() {
            outcome = Some(COMPLETED_MESSAGE);
        }
        if train.speed().abs() < 0.05 {
            slow_steps += 1;
            if slow_steps >= STOPPED_STEPS_BEFORE_GIVING_UP && train.notes().is_empty() {
                outcome = Some(STOPPED_MESSAGE);
            }
        } else {
            slow_steps = 0;
        }
        let readings = train.take_readings();
        recorder.record(&readings, model_key);
        for frame in &readings {
            if let Some(reading) = frame.first() {
                recorder.record_dueling(circuit, *reading, &other_trains);
            }
        }
        if let Some(message) = outcome {
            let (report, _front_car_report) = recorder
                .finish_with_front_car(message == COMPLETED_MESSAGE, !train.notes().is_empty());
            return Ok(report);
        }
    }
    Err(format!(
        "The test train neither finished a lap nor stopped within {MAXIMUM_TEST_STEPS} physics steps"
    ))
}

impl Park {
    /// Rates a coaster immediately and stores the report. Does not touch the builder's test train,
    /// messages, guest ratings or live operation, so it also works while the coaster is open.
    pub fn run_ride_test(&mut self, ride: RideKey, coaster_id: CoasterId) -> Result<(), String> {
        match ride {
            RideKey::Demo => self.run_demo_test(),
            RideKey::Player => self.run_player_test(coaster_id),
        }
    }

    fn run_player_test(&mut self, coaster_id: CoasterId) -> Result<(), String> {
        if !self.construction_can_test(coaster_id) {
            return Err("A closed circuit longer than the train is required".into());
        }
        let coaster = self.coaster(coaster_id).expect("checked above");
        let construction = &coaster.construction;
        let circuit = construction.circuit().expect("checked above");
        let model = construction.model();
        let mut report = run_test_to_outcome(
            &circuit,
            &model.train,
            model,
            model.name,
            self.demo_coaster.as_ref().map(|coaster| {
                (
                    &coaster.circuit,
                    coaster.train.clone(),
                    coaster.train_configuration.clone(),
                )
            }),
            &other_coaster_train_positions(self, Some(coaster_id)),
        )?;
        // The supports cache is derived from the track and excluded from the state hash.
        self.refresh_construction_supports(coaster_id)
            .map_err(|_| "Could not route supports for the placed track.".to_owned())?;
        let obstacles = self.proximity_obstacles();
        let supports = self.proximity_supports(coaster_id);
        let other_tracks = self.other_tracks(coaster_id);
        let construction = &self
            .coaster(coaster_id)
            .expect("checked above")
            .construction;
        add_proximity_and_theming(
            &self.world,
            construction.pieces(),
            &construction.model().train,
            &other_tracks,
            &obstacles,
            &supports,
            std::slice::from_mut(&mut report),
        );
        self.coaster_mut(coaster_id)
            .expect("checked above")
            .construction_report = Some(report);
        Ok(())
    }

    /// The demo track has no support geometry, but its report includes other coaster tracks.
    fn run_demo_test(&mut self) -> Result<(), String> {
        let (circuit, train_configuration) = self
            .demo_coaster
            .as_ref()
            .map(|coaster| (&coaster.circuit, coaster.train_configuration.clone()))
            .ok_or("This park has no demo coaster")?;
        let model = hybrid_model();
        let mut report = run_test_to_outcome(
            circuit,
            &train_configuration,
            &model,
            model.name,
            None,
            &other_coaster_train_positions(self, None),
        )?;
        let obstacles = self.proximity_obstacles();
        let other_tracks: Vec<_> = self
            .coasters
            .values()
            .flat_map(|coaster| {
                coaster
                    .construction
                    .pieces()
                    .iter()
                    .chain(
                        coaster
                            .construction
                            .disconnected_segments()
                            .iter()
                            .flat_map(|segment| segment.pieces.iter()),
                    )
                    .cloned()
            })
            .collect();
        add_proximity_and_theming(
            &self.world,
            circuit.pieces(),
            &model.train,
            &other_tracks,
            &obstacles,
            &[],
            std::slice::from_mut(&mut report),
        );
        self.demo_report = Some(report);
        Ok(())
    }
}
