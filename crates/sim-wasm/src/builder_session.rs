use std::fmt::{self, Debug, Display, Formatter};

use theme_park_sim::track::coaster_models;
use theme_park_sim::track::ride_file::{RideFile, RideFileFormat, RideHeading, RideStep};

use crate::WasmSimulation;
use crate::construction::{
    ConstructionCandidateForcePreviewWire, ConstructionElementWire, ConstructionForcePreviewWire,
    ConstructionPieceWire,
};
use crate::construction_speed_match::ConstructionSpeedMatchWire;

pub struct BuilderSession {
    simulation: WasmSimulation,
    end_speed_cache: Option<f64>,
}

pub struct BuilderEstimates {
    pub force_preview: Option<ConstructionForcePreviewWire>,
    pub end_speed: Option<f64>,
    pub choices: Vec<ConstructionPieceWire>,
    pub elements: Vec<ConstructionElementWire>,
    pub hill_options_json: String,
    pub speed_turns_json: String,
    pub candidate_preview: Option<ConstructionCandidateForcePreviewWire>,
    pub speed_match: Option<ConstructionSpeedMatchWire>,
}

impl Debug for BuilderEstimates {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BuilderEstimates")
            .field("end_speed", &self.end_speed)
            .field("choices_count", &self.choices.len())
            .field("elements_count", &self.elements.len())
            .finish_non_exhaustive()
    }
}

pub struct BuilderSelection {
    pub requested_id: String,
    pub selected_id: String,
    pub speed_match: Option<ConstructionSpeedMatchWire>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderSessionError {
    Bridge { action: String, message: String },
    BridgeRefusedWithoutMessage { action: String },
    InvalidRide(String),
    RideStep { step_index: usize, message: String },
}

impl Display for BuilderSessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge { action, message } => write!(formatter, "{action}: {message}"),
            Self::BridgeRefusedWithoutMessage { action } => {
                write!(formatter, "{action}: bridge refused without a message")
            }
            Self::InvalidRide(message) => formatter.write_str(message),
            Self::RideStep {
                step_index,
                message,
            } => {
                write!(formatter, "ride step {step_index}: {message}")
            }
        }
    }
}

impl std::error::Error for BuilderSessionError {}

impl BuilderSession {
    pub fn new(simulation: WasmSimulation) -> Self {
        Self {
            simulation,
            end_speed_cache: None,
        }
    }

    pub fn simulation(&self) -> &WasmSimulation {
        &self.simulation
    }

    pub fn into_simulation(self) -> WasmSimulation {
        self.simulation
    }

    pub fn state_hash(&self) -> u64 {
        self.simulation.state_hash()
    }

    pub fn start(
        &mut self,
        model_key: &str,
        x: i32,
        y: i32,
        z: i32,
        heading: u8,
    ) -> Result<(), BuilderSessionError> {
        if !coaster_models().iter().any(|model| model.key == model_key) {
            return Err(BuilderSessionError::InvalidRide(format!(
                "unknown coaster model {model_key:?}"
            )));
        }
        if heading > 3 {
            return Err(BuilderSessionError::InvalidRide(format!(
                "worker station heading must be between 0 and 3, got {heading}"
            )));
        }
        self.end_speed_cache = None;
        self.simulation
            .construction_select(&format!("model:{model_key}"));
        self.check_bridge_message("select coaster model")?;
        self.simulation
            .construction_start(x * 2, y * 2, z / 2, heading * 2);
        self.check_bridge_message("start construction")
    }

    pub fn select(
        &mut self,
        id: &str,
        exact: bool,
    ) -> Result<BuilderSelection, BuilderSessionError> {
        let speed_match = self
            .simulation
            .construction_speed_match(id, self.end_speed_cache);
        let selected_id = if exact {
            id
        } else {
            speed_match
                .as_ref()
                .and_then(|speed_match| speed_match.recommended_id)
                .unwrap_or(id)
        }
        .to_owned();
        self.simulation.construction_select(&selected_id);
        if !self.simulation.construction_message().is_empty() {
            let message = self.simulation.construction_message();
            let message = if message == "Unknown piece" {
                format!("{message}; offered ids: {}", self.offered_ids())
            } else {
                message
            };
            return Err(self.bridge_error("select piece", message));
        }
        Ok(BuilderSelection {
            requested_id: id.to_owned(),
            selected_id,
            speed_match,
        })
    }

    /// Reads the bridge estimates used by the builder panel in the worker.
    pub fn worker_update(&mut self, complete: bool) -> BuilderEstimates {
        let force_preview = if complete {
            self.simulation.construction_force_preview()
        } else {
            None
        };
        let end_speed = force_preview.as_ref().and_then(|preview| {
            (preview.status == "reachedEnd")
                .then(|| preview.samples.last().map(|sample| sample.speed))
                .flatten()
        });
        let status = force_preview.as_ref().map_or("", |preview| preview.status);
        let choices = self
            .simulation
            .construction_choices_preview(end_speed, status);
        let elements = if complete {
            self.simulation.construction_elements_typed()
        } else {
            Vec::new()
        };
        if complete {
            self.end_speed_cache = end_speed;
        }
        let hill_options_json = self.simulation.construction_hill_options();
        let speed_turns_json = self.simulation.construction_speed_turns();
        let candidate_preview = if complete {
            self.simulation.construction_candidate_force_preview()
        } else {
            None
        };
        let selected = self.simulation.construction_selected();
        let speed_match = if complete {
            self.simulation
                .construction_speed_match(&selected, self.end_speed_cache)
        } else {
            None
        };

        BuilderEstimates {
            force_preview,
            end_speed,
            choices,
            elements,
            hill_options_json,
            speed_turns_json,
            candidate_preview,
            speed_match,
        }
    }

    pub fn estimates(&mut self) -> BuilderEstimates {
        self.worker_update(true)
    }

    pub fn place(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_finish_invalidate();
        self.simulation.construction_append();
        self.check_bridge_message("place selected piece")?;
        Ok(self.estimates())
    }

    pub fn set_chain(&mut self, enabled: bool) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_set_chain(enabled);
        self.check_bridge_message("set chain lift")
    }

    pub fn set_launch_speed(
        &mut self,
        piece_index: usize,
        target_speed: f64,
    ) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation
            .construction_set_launch_intensity_result(piece_index, target_speed)
            .map_err(|message| self.bridge_error("set launch speed", message))
    }

    pub fn undo(&mut self) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_undo();
        self.check_bridge_message("undo placement")
    }

    pub fn delete(&mut self, piece_index: usize) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_delete(piece_index);
        self.check_bridge_message("delete placement")
    }

    pub fn set_cars_per_train(&mut self, cars: u8) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        if !self.simulation.construction_set_cars_per_train(cars) {
            return Err(self.bridge_refusal("set cars per train"));
        }
        self.check_bridge_message("set cars per train")
    }

    pub fn set_train_count(&mut self, count: usize) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation
            .construction_set_train_count_result(count)
            .map_err(|message| self.bridge_error("set train count", message))
    }

    pub fn set_colors(
        &mut self,
        track: &str,
        supports: &str,
        trains: &str,
    ) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation
            .construction_set_colors(track, supports, trains);
        self.check_bridge_message("set coaster colours")
    }

    pub fn set_price(&mut self, price: u16) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_set_coaster_price(price);
        Ok(())
    }

    pub fn finish(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_finish_search_start();
        while !self.simulation.construction_finish_search_advance() {}
        self.simulation.construction_finish_preview();
        if !self.simulation.construction_can_finish() {
            return Err(self.bridge_error(
                "finish coaster",
                self.simulation.construction_finish_reason(),
            ));
        }
        self.simulation.construction_finish_confirm();
        self.check_bridge_message("confirm finished coaster")?;
        Ok(self.estimates())
    }

    pub fn close(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_close();
        self.check_bridge_message("close circuit")?;
        Ok(self.estimates())
    }

    pub fn test(&mut self) -> Result<serde_json::Value, BuilderSessionError> {
        let message = self.simulation.ride_test("player");
        if !message.is_empty() {
            return Err(self.bridge_error("test ride", message));
        }
        serde_json::from_str(&self.simulation.ride_info_json("player"))
            .map_err(|error| self.bridge_error("read ride test report", error.to_string()))
    }

    pub fn auto_blocks(&mut self) -> Result<serde_json::Value, BuilderSessionError> {
        if !self.simulation.construction_auto_blocks() {
            return Err(self.bridge_refusal("place automatic block sections"));
        }
        while !self.simulation.construction_advance_block_analysis(1000) {
            let message = self.simulation.construction_message();
            if !message.is_empty() {
                return Err(self.bridge_error("analyse block sections", message));
            }
        }
        serde_json::from_str(&self.simulation.construction_block_report_json())
            .map_err(|error| self.bridge_error("read block report", error.to_string()))
    }

    pub fn replay_ride(
        &mut self,
        ride: &RideFile,
    ) -> Result<BuilderEstimates, BuilderSessionError> {
        if ride.format != RideFileFormat::HeartlineRide {
            return Err(BuilderSessionError::InvalidRide(
                "unsupported ride file format".into(),
            ));
        }
        if ride.version != 1 {
            return Err(BuilderSessionError::InvalidRide(format!(
                "unsupported ride file version {}",
                ride.version
            )));
        }
        if !coaster_models().iter().any(|model| model.key == ride.model) {
            return Err(BuilderSessionError::InvalidRide(format!(
                "ride step 1: unknown coaster model {:?}",
                ride.model
            )));
        }
        let heading = match ride.station.heading {
            RideHeading::East => 0,
            RideHeading::North => 1,
            RideHeading::West => 2,
            RideHeading::South => 3,
        };
        self.start(
            &ride.model,
            ride.station.x,
            ride.station.y,
            ride.station.z,
            heading,
        )
        .map_err(|error| invalid_ride_step(1, error))?;
        self.estimates();
        self.set_cars_per_train(ride.cars_per_train)
            .map_err(|error| invalid_ride_step(1, error))?;
        let mut estimates = self.estimates();

        for (step_offset, step) in ride.steps.iter().enumerate() {
            let step_index = step_offset + 1;
            match step {
                RideStep::Place(place) => {
                    self.select(&place.place, true)
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                    self.estimates();
                    estimates = self
                        .place()
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                    if let Some(target_speed) = place.launch_speed {
                        let track: serde_json::Value =
                            serde_json::from_str(&self.simulation.construction_track())
                                .expect("construction track is valid JSON");
                        let piece_index = track
                            .as_array()
                            .and_then(|pieces| pieces.last())
                            .and_then(|piece| piece["id"].as_u64())
                            .map(|id| id as usize)
                            .ok_or_else(|| BuilderSessionError::RideStep {
                                step_index,
                                message: "placed launch has no bridge piece id".into(),
                            })?;
                        self.set_launch_speed(piece_index, target_speed)
                            .map_err(|error| invalid_ride_step(step_index, error))?;
                        estimates = self.estimates();
                    }
                }
                RideStep::Chain(chain) => {
                    self.set_chain(chain.chain)
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                    estimates = self.estimates();
                }
                RideStep::Close(close) => {
                    if !close.close {
                        return Err(BuilderSessionError::RideStep {
                            step_index,
                            message: "close must be true".into(),
                        });
                    }
                    estimates = self
                        .close()
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                    if self.simulation.construction_train_count() != usize::from(ride.trains) {
                        self.set_train_count(usize::from(ride.trains))
                            .map_err(|error| invalid_ride_step(step_index, error))?;
                        estimates = self.estimates();
                    }
                }
            }
        }
        if ride.trains != 1
            && !ride
                .steps
                .iter()
                .any(|step| matches!(step, RideStep::Close(_)))
        {
            return Err(BuilderSessionError::InvalidRide(format!(
                "ride step {}: train count can only be set after the circuit is closed",
                ride.steps.len() + 1
            )));
        }
        Ok(estimates)
    }

    fn offered_ids(&self) -> String {
        let force_preview = self.simulation.construction_force_preview();
        let end_speed = force_preview.as_ref().and_then(|preview| {
            (preview.status == "reachedEnd")
                .then(|| preview.samples.last().map(|sample| sample.speed))
                .flatten()
        });
        let status = force_preview.as_ref().map_or("", |preview| preview.status);
        let mut ids: Vec<_> = self
            .simulation
            .construction_choices_preview(end_speed, status)
            .into_iter()
            .map(|choice| choice.id.to_owned())
            .chain(
                self.simulation
                    .construction_elements_typed()
                    .into_iter()
                    .map(|element| element.id),
            )
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.join(", ")
    }

    fn check_bridge_message(&self, action: &str) -> Result<(), BuilderSessionError> {
        let message = self.simulation.construction_message();
        if message.is_empty() {
            Ok(())
        } else {
            Err(self.bridge_error(action, message))
        }
    }

    fn bridge_refusal(&self, action: &str) -> BuilderSessionError {
        let message = self.simulation.construction_message();
        if message.is_empty() {
            BuilderSessionError::BridgeRefusedWithoutMessage {
                action: action.to_owned(),
            }
        } else {
            self.bridge_error(action, message)
        }
    }

    fn bridge_error(&self, action: &str, message: String) -> BuilderSessionError {
        BuilderSessionError::Bridge {
            action: action.to_owned(),
            message,
        }
    }
}

fn invalid_ride_step(step_index: usize, error: BuilderSessionError) -> BuilderSessionError {
    BuilderSessionError::RideStep {
        step_index,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::BuilderSession;
    use crate::WasmSimulation;
    use theme_park_sim::track::ride_file::{
        RideCloseStep, RideFile, RideFileFormat, RideHeading, RidePlaceStep, RideStation, RideStep,
    };

    const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");

    #[test]
    fn start_uses_the_workers_coordinate_scaling() {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session.start("hybrid", 241, 249, 12, 1).unwrap();

        let mut worker_sequence = WasmSimulation::new(42, 0, "new".into());
        worker_sequence.construction_select("model:hybrid");
        worker_sequence.construction_start(482, 498, 6, 2);

        assert_eq!(session.state_hash(), worker_sequence.state_hash());
    }

    #[test]
    fn selection_uses_the_last_reached_end_speed_and_exact_selection_bypasses_matching() {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session.start("hybrid", 241, 249, 12, 1).unwrap();
        let estimates = session.worker_update(true);
        let selection = session.select("Loop", false).unwrap();

        let mut worker_sequence = WasmSimulation::new(42, 0, "new".into());
        worker_sequence.construction_select("model:hybrid");
        worker_sequence.construction_start(482, 498, 6, 2);
        let force_preview = worker_sequence.construction_force_preview();
        let cached_speed = force_preview.as_ref().and_then(|preview| {
            (preview.status == "reachedEnd")
                .then(|| preview.samples.last().map(|sample| sample.speed))
                .flatten()
        });
        let expected_match = worker_sequence.construction_speed_match("Loop", cached_speed);
        let expected_selection = expected_match
            .as_ref()
            .and_then(|match_result| match_result.recommended_id)
            .unwrap_or("Loop");
        worker_sequence.construction_select(expected_selection);

        assert_eq!(estimates.end_speed, cached_speed);
        assert_eq!(
            selection
                .speed_match
                .map(|match_result| match_result.recommended_id),
            expected_match.map(|match_result| match_result.recommended_id)
        );
        assert_eq!(
            session.simulation().construction_selected(),
            expected_selection
        );
        session.select("LargeLoop", true).unwrap();
        assert_eq!(session.simulation().construction_selected(), "LargeLoop");
    }

    #[test]
    fn undo_restores_the_previous_state_hash_after_a_placed_piece() {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session.start("hybrid", 241, 249, 12, 1).unwrap();
        session.worker_update(true);
        let initial_hash = session.state_hash();

        session.select("Straight", true).unwrap();
        session.worker_update(true);
        session.place().unwrap();
        assert_ne!(session.state_hash(), initial_hash);

        session.undo().unwrap();
        assert_eq!(session.state_hash(), initial_hash);
    }

    #[test]
    fn ride_replay_is_deterministic_and_rejects_unknown_ids_with_step_context() {
        let ride = ride_file(
            "hybrid",
            vec![RideStep::Place(RidePlaceStep {
                place: "Straight".into(),
                launch_speed: None,
                note: None,
            })],
        );
        let mut first = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut second = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        first.replay_ride(&ride).unwrap();
        second.replay_ride(&ride).unwrap();
        assert_eq!(first.state_hash(), second.state_hash());

        let unknown_model = ride_file("not-a-model", vec![]);
        let model_error = first.replay_ride(&unknown_model).unwrap_err().to_string();
        assert!(model_error.contains("step 1"), "{model_error}");

        let unknown_piece = ride_file(
            "hybrid",
            vec![
                RideStep::Place(RidePlaceStep {
                    place: "Straight".into(),
                    launch_speed: None,
                    note: None,
                }),
                RideStep::Place(RidePlaceStep {
                    place: "not-a-piece".into(),
                    launch_speed: None,
                    note: None,
                }),
            ],
        );
        let piece_error = first.replay_ride(&unknown_piece).unwrap_err().to_string();
        assert!(piece_error.contains("step 2"), "{piece_error}");
        assert!(piece_error.contains("Straight"), "{piece_error}");
    }

    #[test]
    fn replay_rejects_a_false_close_step_with_its_index() {
        let ride = ride_file(
            "hybrid",
            vec![RideStep::Close(RideCloseStep { close: false })],
        );
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));

        let error = session.replay_ride(&ride).unwrap_err().to_string();

        assert!(error.contains("step 1"), "{error}");
        assert!(error.contains("close must be true"), "{error}");
    }

    fn ride_file(model: &str, steps: Vec<RideStep>) -> RideFile {
        RideFile {
            format: RideFileFormat::HeartlineRide,
            version: 1,
            name: "session test".into(),
            model: model.into(),
            station: RideStation {
                x: 241,
                y: 249,
                z: 12,
                heading: RideHeading::North,
            },
            cars_per_train: 8,
            trains: 1,
            steps,
            expect: None,
        }
    }

    #[test]
    fn session_outputs_match_the_worker_benchmark_digest_for_all_three_circuits() {
        for (ride_name, model_key) in [
            ("Timber Tumbler", "hybrid"),
            ("Velocity Viper", "velocity_viper"),
            ("Briar Blitz", "wooden_twister"),
        ] {
            let commands = circuit_lines(ride_name);
            let expected = benchmark_trace(ride_name, model_key, &commands);
            let actual = session_trace(model_key, &commands);
            assert_eq!(
                trace_digest(&actual),
                trace_digest(&expected),
                "{ride_name}: {}",
                first_trace_difference(&expected, &actual)
            );
        }
    }

    fn circuit_lines(ride_name: &str) -> Vec<&'static str> {
        let header = format!("[{ride_name}]");
        let mut sections = CIRCUITS.split(&header);
        sections.next().expect("circuits file has a preamble");
        sections
            .next()
            .unwrap_or_else(|| panic!("{ride_name} circuit missing"))
            .lines()
            .map(str::trim)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    fn benchmark_trace(ride_name: &str, model_key: &str, commands: &[&str]) -> Trace {
        let mut simulation = WasmSimulation::new(42, 0, "new".into());
        let mut trace = Vec::new();
        simulation.construction_select(&format!("model:{model_key}"));
        simulation.construction_start(482, 498, 6, 2);
        let mut end_speed = None;
        benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);

        let mut piece_count = 1;
        for command in commands {
            match *command {
                "+chain" | "-chain" => {
                    simulation.construction_set_chain(*command == "+chain");
                    end_speed = None;
                    benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);
                    continue;
                }
                _ => {}
            }

            trace_call(
                &mut trace,
                "select: speed match",
                simulation.construction_speed_match_json(command, end_speed),
            );
            simulation.construction_select(command);
            trace_call(&mut trace, "select: construction_select", ());
            benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);

            end_speed = None;
            simulation.construction_finish_invalidate();
            trace_call(&mut trace, "append: finish invalidate", ());
            simulation.construction_append();
            trace_call(&mut trace, "append: construction_append", ());
            if !simulation.construction_selected().starts_with("element:") {
                benchmark_update(&mut simulation, false, &mut end_speed, &mut trace);
            }
            benchmark_world_update(&simulation, &mut trace);
            benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);

            piece_count += 1;
            let track: serde_json::Value =
                serde_json::from_str(&simulation.construction_track()).unwrap();
            assert_eq!(
                track.as_array().unwrap().len(),
                piece_count,
                "{ride_name}: {command} was not placed: {}",
                simulation.construction_message()
            );
        }

        for _ in 0..3 {
            simulation.construction_undo();
            benchmark_update(&mut simulation, false, &mut end_speed, &mut trace);
            benchmark_world_update(&simulation, &mut trace);
            benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);
        }

        trace
    }

    fn session_trace(model_key: &str, commands: &[&str]) -> Trace {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut trace = Vec::new();
        session.start(model_key, 241, 249, 12, 1).unwrap();
        session_update(&mut session, true, &mut trace);

        for command in commands {
            match *command {
                "+chain" | "-chain" => {
                    session.set_chain(*command == "+chain").unwrap();
                    session_update(&mut session, true, &mut trace);
                    continue;
                }
                _ => {}
            }

            let selection = session.select(command, true).unwrap();
            trace_call(
                &mut trace,
                "select: speed match",
                serde_json::to_string(&selection.speed_match).unwrap(),
            );
            trace_call(&mut trace, "select: construction_select", ());
            session_update(&mut session, true, &mut trace);

            trace_call(&mut trace, "append: finish invalidate", ());
            trace_call(&mut trace, "append: construction_append", ());
            session.place().unwrap();
            session_update(&mut session, false, &mut trace);
            benchmark_world_update(session.simulation(), &mut trace);
            session_update(&mut session, true, &mut trace);
        }

        for _ in 0..3 {
            session.undo().unwrap();
            session_update(&mut session, false, &mut trace);
            benchmark_world_update(session.simulation(), &mut trace);
            session_update(&mut session, true, &mut trace);
        }

        trace
    }

    type Trace = Vec<(&'static str, String)>;

    fn trace_call<T: std::fmt::Debug>(trace: &mut Trace, name: &'static str, output: T) {
        trace.push((name, format!("{output:?}")));
    }

    fn trace_digest(trace: &[(&'static str, String)]) -> u64 {
        let mut digest = DefaultHasher::new();
        for (name, output) in trace {
            name.hash(&mut digest);
            output.hash(&mut digest);
        }
        digest.finish()
    }

    fn first_trace_difference(expected: &Trace, actual: &Trace) -> String {
        for (index, (expected_call, actual_call)) in expected.iter().zip(actual).enumerate() {
            if expected_call != actual_call {
                let expected_output: String = expected_call.1.chars().take(240).collect();
                let actual_output: String = actual_call.1.chars().take(240).collect();
                return format!(
                    "first difference at call {index}: expected {} {:?}, got {} {:?}",
                    expected_call.0, expected_output, actual_call.0, actual_output,
                );
            }
        }
        format!(
            "trace lengths differ: expected {}, got {}",
            expected.len(),
            actual.len()
        )
    }

    fn session_update(session: &mut BuilderSession, complete: bool, trace: &mut Trace) {
        let prefix = if complete { "full" } else { "partial" };
        let estimates = session.worker_update(complete);
        if complete {
            trace_call(
                trace,
                label(prefix, "force_preview_json"),
                serde_json::to_string(&estimates.force_preview).unwrap(),
            );
        }
        trace_call(
            trace,
            label(prefix, "choices_with_preview"),
            serde_json::to_string(&estimates.choices).unwrap(),
        );
        if complete {
            trace_call(
                trace,
                label(prefix, "elements"),
                serde_json::to_string(&estimates.elements).unwrap(),
            );
        }
        let simulation = session.simulation();
        let selected = simulation.construction_selected();
        trace_call(
            trace,
            label(prefix, "track"),
            simulation.construction_track(),
        );
        trace_call(
            trace,
            label(prefix, "preview rails, ties, supports"),
            (
                simulation.construction_rails(),
                simulation.construction_ties(),
                simulation.construction_supports(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "placed rails, distances"),
            (
                simulation.construction_placed_rails(),
                simulation.construction_rail_distances(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "block_candidates"),
            simulation.construction_block_candidates(),
        );
        trace_call(
            trace,
            label(prefix, "placed ties, supports"),
            (
                simulation.construction_placed_ties(),
                simulation.construction_placed_supports(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "block_report_json"),
            simulation.construction_block_report_json(),
        );
        trace_call(
            trace,
            label(prefix, "small getters"),
            (
                simulation.construction_message(),
                simulation.construction_coaster_price(),
                simulation.construction_coaster_income(),
                simulation.construction_colors(),
                simulation.construction_train_layout(),
                simulation.construction_active(),
                simulation.construction_chain(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "hill_options, speed_turns"),
            (
                estimates.hill_options_json.clone(),
                estimates.speed_turns_json.clone(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "can_close"),
            simulation.construction_can_close(),
        );
        trace_call(
            trace,
            label(prefix, "finish state"),
            (
                simulation.construction_finish_preview_active(),
                simulation.construction_can_finish(),
                simulation.construction_finish_reason(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "test_report_json"),
            simulation.construction_test_report_json(),
        );
        if complete {
            trace_call(
                trace,
                label(prefix, "candidate_force_preview_json"),
                serde_json::to_string(&estimates.candidate_preview).unwrap(),
            );
            trace_call(
                trace,
                label(prefix, "speed_match_json"),
                serde_json::to_string(&estimates.speed_match).unwrap(),
            );
        }
        trace_call(
            trace,
            label(prefix, "operate getters"),
            (
                simulation.construction_can_test(),
                simulation.construction_is_open(),
                simulation.construction_station_positions(),
                simulation.construction_train_count(),
                simulation.construction_min_cars_per_train(),
                simulation.construction_max_cars_per_train(),
            ),
        );
        let _ = (selected, estimates.end_speed);
    }

    fn benchmark_update(
        simulation: &mut WasmSimulation,
        complete: bool,
        end_speed: &mut Option<f64>,
        trace: &mut Trace,
    ) {
        let prefix = if complete { "full" } else { "partial" };
        let force_preview: serde_json::Value = if complete {
            let json = simulation.construction_force_preview_json();
            trace_call(trace, label(prefix, "force_preview_json"), json.clone());
            serde_json::from_str(&json).unwrap()
        } else {
            serde_json::Value::Null
        };
        let preview_end_speed = (force_preview["status"] == "reachedEnd").then(|| {
            force_preview["samples"].as_array().unwrap().last().unwrap()["speed"]
                .as_f64()
                .unwrap()
        });
        let status = force_preview["status"].as_str().unwrap_or("").to_owned();
        trace_call(
            trace,
            label(prefix, "choices_with_preview"),
            simulation.construction_choices_with_preview(preview_end_speed, &status),
        );
        if complete {
            trace_call(
                trace,
                label(prefix, "elements"),
                simulation.construction_elements(),
            );
        }
        let selected = simulation.construction_selected();
        trace_call(
            trace,
            label(prefix, "track"),
            simulation.construction_track(),
        );
        trace_call(
            trace,
            label(prefix, "preview rails, ties, supports"),
            (
                simulation.construction_rails(),
                simulation.construction_ties(),
                simulation.construction_supports(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "placed rails, distances"),
            (
                simulation.construction_placed_rails(),
                simulation.construction_rail_distances(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "block_candidates"),
            simulation.construction_block_candidates(),
        );
        trace_call(
            trace,
            label(prefix, "placed ties, supports"),
            (
                simulation.construction_placed_ties(),
                simulation.construction_placed_supports(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "block_report_json"),
            simulation.construction_block_report_json(),
        );
        trace_call(
            trace,
            label(prefix, "small getters"),
            (
                simulation.construction_message(),
                simulation.construction_coaster_price(),
                simulation.construction_coaster_income(),
                simulation.construction_colors(),
                simulation.construction_train_layout(),
                simulation.construction_active(),
                simulation.construction_chain(),
            ),
        );
        if complete {
            *end_speed = preview_end_speed;
        }
        trace_call(
            trace,
            label(prefix, "hill_options, speed_turns"),
            (
                simulation.construction_hill_options(),
                simulation.construction_speed_turns(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "can_close"),
            simulation.construction_can_close(),
        );
        trace_call(
            trace,
            label(prefix, "finish state"),
            (
                simulation.construction_finish_preview_active(),
                simulation.construction_can_finish(),
                simulation.construction_finish_reason(),
            ),
        );
        trace_call(
            trace,
            label(prefix, "test_report_json"),
            simulation.construction_test_report_json(),
        );
        if complete {
            trace_call(
                trace,
                label(prefix, "candidate_force_preview_json"),
                simulation.construction_candidate_force_preview_json(),
            );
            trace_call(
                trace,
                label(prefix, "speed_match_json"),
                simulation.construction_speed_match_json(&selected, *end_speed),
            );
        }
        trace_call(
            trace,
            label(prefix, "operate getters"),
            (
                simulation.construction_can_test(),
                simulation.construction_is_open(),
                simulation.construction_station_positions(),
                simulation.construction_train_count(),
                simulation.construction_min_cars_per_train(),
                simulation.construction_max_cars_per_train(),
            ),
        );
    }

    fn benchmark_world_update(simulation: &WasmSimulation, trace: &mut Trace) {
        trace_call(
            trace,
            "world: snapshot arrays",
            (
                simulation.world_dimensions(),
                simulation.world_surfaces(),
                simulation.world_paths(),
                simulation.world_path_metadata(),
                simulation.world_facility_openings(),
                simulation.world_carved(),
            ),
        );
        trace_call(
            trace,
            "world: coaster_supports",
            simulation.coaster_supports(),
        );
    }

    fn label(prefix: &str, call: &'static str) -> &'static str {
        match prefix {
            "full" => match call {
                "force_preview_json" => "full: force_preview_json",
                "choices_with_preview" => "full: choices_with_preview",
                "elements" => "full: elements",
                "track" => "full: track",
                "preview rails, ties, supports" => "full: preview rails, ties, supports",
                "placed rails, distances" => "full: placed rails, distances",
                "block_candidates" => "full: block_candidates",
                "placed ties, supports" => "full: placed ties, supports",
                "block_report_json" => "full: block_report_json",
                "small getters" => "full: small getters",
                "hill_options, speed_turns" => "full: hill_options, speed_turns",
                "can_close" => "full: can_close",
                "finish state" => "full: finish state",
                "test_report_json" => "full: test_report_json",
                "candidate_force_preview_json" => "full: candidate_force_preview_json",
                "speed_match_json" => "full: speed_match_json",
                "operate getters" => "full: operate getters",
                _ => panic!("unknown worker update call {call}"),
            },
            "partial" => match call {
                "force_preview_json" => "partial: force_preview_json",
                "choices_with_preview" => "partial: choices_with_preview",
                "elements" => "partial: elements",
                "track" => "partial: track",
                "preview rails, ties, supports" => "partial: preview rails, ties, supports",
                "placed rails, distances" => "partial: placed rails, distances",
                "block_candidates" => "partial: block_candidates",
                "placed ties, supports" => "partial: placed ties, supports",
                "block_report_json" => "partial: block_report_json",
                "small getters" => "partial: small getters",
                "hill_options, speed_turns" => "partial: hill_options, speed_turns",
                "can_close" => "partial: can_close",
                "finish state" => "partial: finish state",
                "test_report_json" => "partial: test_report_json",
                "candidate_force_preview_json" => "partial: candidate_force_preview_json",
                "speed_match_json" => "partial: speed_match_json",
                "operate getters" => "partial: operate getters",
                _ => panic!("unknown worker update call {call}"),
            },
            _ => panic!("unknown worker update prefix {prefix}"),
        }
    }
}
