use std::fmt::{self, Debug, Display, Formatter};

use serde::{Deserialize, Serialize};
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
    placed_piece_bridge_ids: Vec<Vec<usize>>,
    auto_block_advance_counts: Vec<usize>,
}

const BUILDER_SESSION_SNAPSHOT_HEADER: &[u8] = b"TPBUILDER\x01";

#[derive(Serialize, Deserialize)]
struct BuilderSessionSnapshot {
    simulation: Vec<u8>,
    end_speed_cache: Option<f64>,
    placed_piece_bridge_ids: Vec<Vec<usize>>,
    auto_block_advance_counts: Vec<usize>,
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
            placed_piece_bridge_ids: Vec::new(),
            auto_block_advance_counts: Vec::new(),
        }
    }

    pub fn simulation(&self) -> &WasmSimulation {
        &self.simulation
    }

    pub fn placed_piece_bridge_ids(&self) -> &[Vec<usize>] {
        &self.placed_piece_bridge_ids
    }

    pub fn auto_block_advance_counts(&self) -> &[usize] {
        &self.auto_block_advance_counts
    }

    pub fn save_native_snapshot(&self) -> Result<Vec<u8>, String> {
        let snapshot = BuilderSessionSnapshot {
            simulation: self.simulation.save_state_native()?,
            end_speed_cache: self.end_speed_cache,
            placed_piece_bridge_ids: self.placed_piece_bridge_ids.clone(),
            auto_block_advance_counts: self.auto_block_advance_counts.clone(),
        };
        let payload = postcard::to_allocvec(&snapshot)
            .map_err(|error| format!("Could not save builder session: {error}"))?;
        let mut bytes = BUILDER_SESSION_SNAPSHOT_HEADER.to_vec();
        bytes.extend(snapshot_checksum(&payload).to_le_bytes());
        bytes.extend(payload);
        Ok(bytes)
    }

    pub fn load_native_snapshot(bytes: &[u8]) -> Result<Self, String> {
        let data = bytes
            .strip_prefix(BUILDER_SESSION_SNAPSHOT_HEADER)
            .ok_or("Unsupported or invalid builder session snapshot version")?;
        let (stored_checksum, payload) = data
            .split_at_checked(8)
            .ok_or("Builder session snapshot is incomplete")?;
        let stored_checksum = u64::from_le_bytes(
            stored_checksum
                .try_into()
                .expect("builder session snapshot checksum length"),
        );
        if snapshot_checksum(payload) != stored_checksum {
            return Err("Builder session snapshot is corrupt".into());
        }
        let (snapshot, remaining): (BuilderSessionSnapshot, &[u8]) =
            postcard::take_from_bytes(payload)
                .map_err(|error| format!("Could not load builder session snapshot: {error}"))?;
        if !remaining.is_empty() {
            return Err("Builder session snapshot has trailing data".into());
        }
        Ok(Self {
            simulation: WasmSimulation::load_state_native(&snapshot.simulation)?,
            end_speed_cache: snapshot.end_speed_cache,
            placed_piece_bridge_ids: snapshot.placed_piece_bridge_ids,
            auto_block_advance_counts: snapshot.auto_block_advance_counts,
        })
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
            if preview.status != "reachedEnd" {
                return None;
            }
            let last_sample = preview
                .samples
                .last()
                .expect("reached-end force preview has at least one sample");
            let speed_json = serde_json::to_string(&last_sample.speed)
                .expect("force preview sample speed serializes");
            Some(serde_json::from_str(&speed_json).expect("force preview speed JSON is valid"))
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
        self.append_selected()?;
        Ok(self.estimates())
    }

    fn append_selected(&mut self) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        let previous_piece_count = self
            .simulation
            .construction
            .as_ref()
            .map(|construction| construction.piece_ids().len());
        self.simulation.construction_finish_invalidate();
        self.simulation.construction_append();
        self.check_bridge_message("place selected piece")?;
        if let (Some(previous_piece_count), Some(construction)) =
            (previous_piece_count, self.simulation.construction.as_ref())
        {
            if let Some(piece_ids) = construction.piece_ids().get(previous_piece_count..) {
                if !piece_ids.is_empty() {
                    self.placed_piece_bridge_ids.push(piece_ids.to_vec());
                }
            }
        }
        Ok(())
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
        self.check_bridge_message("undo placement")?;
        self.placed_piece_bridge_ids.pop();
        Ok(())
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
        let previous_piece_count = self
            .simulation
            .construction
            .as_ref()
            .map(|construction| construction.piece_ids().len());
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
        if let (Some(previous_piece_count), Some(construction)) =
            (previous_piece_count, self.simulation.construction.as_ref())
        {
            if let Some(new_piece_ids) = construction.piece_ids().get(previous_piece_count..) {
                self.placed_piece_bridge_ids
                    .extend(new_piece_ids.iter().map(|piece_id| vec![*piece_id]));
            }
        }
        Ok(self.estimates())
    }

    pub fn close(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.close_track()?;
        Ok(self.estimates())
    }

    fn close_track(&mut self) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        self.simulation.construction_close();
        self.check_bridge_message("close circuit")
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
        self.auto_blocks_with_advance_count()
            .map(|(report, _)| report)
    }

    pub fn auto_blocks_with_advance_count(
        &mut self,
    ) -> Result<(serde_json::Value, usize), BuilderSessionError> {
        if !self.simulation.construction_auto_blocks() {
            return Err(self.bridge_refusal("place automatic block sections"));
        }
        let mut advance_count = 0;
        loop {
            advance_count += 1;
            if self.simulation.construction_advance_block_analysis(1000) {
                break;
            }
            let message = self.simulation.construction_message();
            if !message.is_empty() {
                return Err(self.bridge_error("analyse block sections", message));
            }
        }
        let report = serde_json::from_str(&self.simulation.construction_block_report_json())
            .map_err(|error| self.bridge_error("read block report", error.to_string()))?;
        self.auto_block_advance_counts.push(advance_count);
        Ok((report, advance_count))
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
        self.placed_piece_bridge_ids.clear();
        self.auto_block_advance_counts.clear();
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
        self.set_cars_per_train(ride.cars_per_train)
            .map_err(|error| invalid_ride_step(1, error))?;

        for (step_offset, step) in ride.steps.iter().enumerate() {
            let step_index = step_offset + 1;
            match step {
                RideStep::Place(place) => {
                    self.select(&place.place, true)
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                    self.append_selected()
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
                    }
                }
                RideStep::Chain(chain) => {
                    self.set_chain(chain.chain)
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                }
                RideStep::Close(close) => {
                    if !close.close {
                        return Err(BuilderSessionError::RideStep {
                            step_index,
                            message: "close must be true".into(),
                        });
                    }
                    self.close_track()
                        .map_err(|error| invalid_ride_step(step_index, error))?;
                }
                RideStep::AutoBlocks(blocks) => {
                    if !blocks.auto_blocks {
                        return Err(BuilderSessionError::RideStep {
                            step_index,
                            message: "autoBlocks must be true".into(),
                        });
                    }
                    self.auto_blocks()
                        .map_err(|error| invalid_ride_step(step_index, error))?;
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
        if ride.trains != 1
            && ride
                .steps
                .iter()
                .any(|step| matches!(step, RideStep::Close(_)))
            && self.simulation.construction_train_count() != usize::from(ride.trains)
        {
            self.set_train_count(usize::from(ride.trains))
                .map_err(|error| invalid_ride_step(ride.steps.len() + 1, error))?;
        }
        Ok(self.estimates())
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

fn snapshot_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
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
    fn undo_restores_the_previous_construction_state_after_a_placed_piece() {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session.start("hybrid", 241, 249, 12, 1).unwrap();
        session.worker_update(true);
        let initial_track = session.simulation().construction_track();
        let mut initial_construction = serde_json::to_value(
            session
                .simulation()
                .construction
                .as_ref()
                .expect("construction started"),
        )
        .unwrap();
        initial_construction
            .as_object_mut()
            .expect("construction serializes as an object")
            .remove("next_id")
            .expect("construction serializes its next piece id");
        let initial_next_piece_id = session
            .simulation()
            .construction
            .as_ref()
            .expect("construction started")
            .next_piece_id();

        session.select("Straight", true).unwrap();
        session.worker_update(true);
        session.place().unwrap();
        assert_ne!(session.simulation().construction_track(), initial_track);

        session.undo().unwrap();
        let construction = session
            .simulation()
            .construction
            .as_ref()
            .expect("construction remains active");
        assert_eq!(session.simulation().construction_track(), initial_track);
        assert_eq!(construction.next_piece_id(), initial_next_piece_id + 1);
        let mut restored_construction = serde_json::to_value(construction).unwrap();
        restored_construction
            .as_object_mut()
            .expect("construction serializes as an object")
            .remove("next_id")
            .expect("construction serializes its next piece id");
        assert_eq!(restored_construction, initial_construction);
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
    fn ride_replay_matches_the_worker_sequence_and_leaves_speed_matching_ready() {
        let ride = ride_file(
            "hybrid",
            vec![
                RideStep::Chain(theme_park_sim::track::ride_file::RideChainStep { chain: true }),
                RideStep::Place(RidePlaceStep {
                    place: "Straight".into(),
                    launch_speed: None,
                    note: None,
                }),
                RideStep::Chain(theme_park_sim::track::ride_file::RideChainStep { chain: false }),
                RideStep::Place(RidePlaceStep {
                    place: "Straight".into(),
                    launch_speed: None,
                    note: None,
                }),
            ],
        );
        let mut replayed = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut worker_sequence = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));

        replayed.replay_ride(&ride).unwrap();
        replay_ride_with_worker_estimates(&mut worker_sequence, &ride);

        assert_eq!(replayed.state_hash(), worker_sequence.state_hash());
        assert_eq!(
            replayed.simulation().construction_track(),
            worker_sequence.simulation().construction_track()
        );
        let replayed_selection = replayed.select("Straight", false).unwrap();
        let worker_selection = worker_sequence.select("Straight", false).unwrap();
        assert_eq!(replayed_selection.selected_id, worker_selection.selected_id);
        assert_eq!(
            serde_json::to_value(replayed_selection.speed_match).unwrap(),
            serde_json::to_value(worker_selection.speed_match).unwrap()
        );
    }

    fn replay_ride_with_worker_estimates(session: &mut BuilderSession, ride: &RideFile) {
        let heading = match ride.station.heading {
            RideHeading::East => 0,
            RideHeading::North => 1,
            RideHeading::West => 2,
            RideHeading::South => 3,
        };
        session
            .start(
                &ride.model,
                ride.station.x,
                ride.station.y,
                ride.station.z,
                heading,
            )
            .unwrap();
        session.estimates();
        session.set_cars_per_train(ride.cars_per_train).unwrap();
        session.estimates();
        for step in &ride.steps {
            match step {
                RideStep::Place(place) => {
                    session.select(&place.place, true).unwrap();
                    session.estimates();
                    session.place().unwrap();
                    if let Some(target_speed) = place.launch_speed {
                        let track: serde_json::Value =
                            serde_json::from_str(&session.simulation().construction_track())
                                .unwrap();
                        let piece_id = track.as_array().unwrap().last().unwrap()["id"]
                            .as_u64()
                            .unwrap() as usize;
                        session.set_launch_speed(piece_id, target_speed).unwrap();
                        session.estimates();
                    }
                }
                RideStep::Chain(chain) => {
                    session.set_chain(chain.chain).unwrap();
                    session.estimates();
                }
                RideStep::Close(close) => {
                    assert!(close.close);
                    session.close().unwrap();
                }
                RideStep::AutoBlocks(blocks) => {
                    assert!(blocks.auto_blocks);
                    session.auto_blocks().unwrap();
                }
            }
        }
        session.estimates();
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

    #[test]
    fn replay_rejects_a_false_auto_blocks_step_with_its_index() {
        let ride = ride_file(
            "hybrid",
            vec![RideStep::AutoBlocks(
                theme_park_sim::track::ride_file::RideAutoBlocksStep { auto_blocks: false },
            )],
        );
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let error = session.replay_ride(&ride).unwrap_err().to_string();
        assert!(error.contains("ride step 1"), "{error}");
        assert!(error.contains("autoBlocks must be true"), "{error}");
    }

    #[test]
    fn native_snapshot_preserves_simulation_speed_cache_and_replay_metadata() {
        let ride = ride_file(
            "hybrid",
            vec![RideStep::Place(RidePlaceStep {
                place: "HalfTileStraight".into(),
                launch_speed: None,
                note: None,
            })],
        );
        let mut original = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        original.replay_ride(&ride).unwrap();

        let snapshot = original.save_native_snapshot().unwrap();
        let mut restored = BuilderSession::load_native_snapshot(&snapshot).unwrap();

        assert_eq!(restored.state_hash(), original.state_hash());
        assert_eq!(
            restored.simulation().construction_track(),
            original.simulation().construction_track()
        );
        assert_eq!(
            restored.placed_piece_bridge_ids(),
            original.placed_piece_bridge_ids()
        );
        assert_eq!(
            restored.auto_block_advance_counts(),
            original.auto_block_advance_counts()
        );
        assert_eq!(
            restored.select("Loop", false).unwrap().selected_id,
            original.select("Loop", false).unwrap().selected_id
        );
    }

    #[test]
    fn native_snapshot_rejects_corruption_and_unknown_versions() {
        let session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let snapshot = session.save_native_snapshot().unwrap();
        let mut corrupt = snapshot.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        let corruption_error = match BuilderSession::load_native_snapshot(&corrupt) {
            Ok(_) => panic!("corrupt builder session snapshot loaded"),
            Err(error) => error,
        };
        assert_eq!(corruption_error, "Builder session snapshot is corrupt");
        let mut unsupported = snapshot;
        let version_index = super::BUILDER_SESSION_SNAPSHOT_HEADER.len() - 1;
        unsupported[version_index] = 2;
        let version_error = match BuilderSession::load_native_snapshot(&unsupported) {
            Ok(_) => panic!("unsupported builder session snapshot loaded"),
            Err(error) => error,
        };
        assert_eq!(
            version_error,
            "Unsupported or invalid builder session snapshot version"
        );
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
                let expected_chars: Vec<_> = expected_call.1.chars().collect();
                let actual_chars: Vec<_> = actual_call.1.chars().collect();
                let common_prefix = expected_chars
                    .iter()
                    .zip(&actual_chars)
                    .take_while(|(expected, actual)| expected == actual)
                    .count();
                let context_start = common_prefix.saturating_sub(80);
                let expected_output: String = expected_chars
                    .iter()
                    .skip(context_start)
                    .take(200)
                    .collect();
                let actual_output: String =
                    actual_chars.iter().skip(context_start).take(200).collect();
                return format!(
                    "first difference at call {index}: expected {} ({} chars) near {context_start} {:?}, got {} ({} chars) near {context_start} {:?}",
                    expected_call.0,
                    expected_chars.len(),
                    expected_output,
                    actual_call.0,
                    actual_chars.len(),
                    actual_output,
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
