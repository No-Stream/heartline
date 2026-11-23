use std::fmt::{self, Debug, Display, Formatter};

use serde::{Deserialize, Serialize};
use theme_park_sim::track::blocks::BlockSectionPlacement;
use theme_park_sim::track::coaster_models;
use theme_park_sim::track::construction::{Completion, FinishForceJudge};
use theme_park_sim::track::ride_file::{RideFile, RideFileFormat, RideHeading, RideStep};
use theme_park_sim::track::shaped::{ElementIntent, ElementSpec};

use crate::WasmSimulation;
use crate::construction::{
    ConstructionCandidateForcePreviewWire, ConstructionElementWire, ConstructionForcePreviewWire,
    ConstructionPieceWire, ConstructionShapedElementWire,
};
use crate::construction_speed_match::ConstructionSpeedMatchWire;
use wasm_bindgen::prelude::*;

pub struct BuilderSession {
    simulation: WasmSimulation,
    end_speed_cache: Option<f64>,
    placed_piece_bridge_ids: Vec<Vec<usize>>,
}

const BUILDER_SESSION_SNAPSHOT_HEADER: &[u8] = b"TPBUILDER\x03";

#[derive(Serialize, Deserialize)]
struct BuilderSessionSnapshot {
    simulation: Vec<u8>,
    end_speed_cache: Option<f64>,
    placed_piece_bridge_ids: Vec<Vec<usize>>,
}

pub struct BuilderEstimates {
    pub force_preview: Option<ConstructionForcePreviewWire>,
    pub end_speed: Option<f64>,
    pub choices: Vec<ConstructionPieceWire>,
    pub elements: Vec<ConstructionElementWire>,
    pub hill_options_json: String,
    pub speed_turns_json: String,
    pub candidate_preview: Option<ConstructionCandidateForcePreviewWire>,
    pub selected_shape: Option<ConstructionShapedElementWire>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderSelection {
    pub requested_id: String,
    pub selected_id: String,
    pub speed_match: Option<ConstructionSpeedMatchWire>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuilderWorkerEstimates<'a> {
    construction_force_preview: &'a Option<ConstructionForcePreviewWire>,
    pieces: &'a Vec<ConstructionPieceWire>,
    elements: &'a Vec<ConstructionElementWire>,
    hill_options: serde_json::Value,
    speed_turns: serde_json::Value,
    candidate_force_preview: &'a Option<ConstructionCandidateForcePreviewWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_shape: &'a Option<ConstructionShapedElementWire>,
    speed_match: &'a Option<ConstructionSpeedMatchWire>,
}

impl BuilderEstimates {
    pub fn to_worker_json(&self) -> String {
        let wire = BuilderWorkerEstimates {
            construction_force_preview: &self.force_preview,
            pieces: &self.choices,
            elements: &self.elements,
            hill_options: serde_json::from_str(&self.hill_options_json)
                .expect("hill options are valid JSON"),
            speed_turns: serde_json::from_str(&self.speed_turns_json)
                .expect("speed turns are valid JSON"),
            candidate_force_preview: &self.candidate_preview,
            selected_shape: &self.selected_shape,
            speed_match: &self.speed_match,
        };
        serde_json::to_string(&wire).expect("builder worker estimates serialize")
    }
}

/// The worker keeps its simulation for rendering and other bridge calls. This adapter owns only
/// the builder session's cached estimate state and delegates the ordering to the same functions
/// used by the native BuilderSession.
#[wasm_bindgen]
pub struct BuilderSessionBridge {
    end_speed_cache: Option<f64>,
}

#[wasm_bindgen]
impl BuilderSessionBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            end_speed_cache: None,
        }
    }

    pub fn invalidate(&mut self) {
        self.end_speed_cache = None;
    }

    pub fn start(&mut self, simulation: &mut WasmSimulation, x: i32, y: i32, z: i32, heading: u8) {
        self.invalidate();
        start_construction(simulation, x, y, z, heading);
    }

    /// Selects with the worker's historical refusal behavior: the caller reads the bridge
    /// message from the simulation and displays it with the rest of construction state.
    pub fn select(&mut self, simulation: &mut WasmSimulation, id: &str, exact: bool) -> String {
        serde_json::to_string(&select_piece(simulation, self.end_speed_cache, id, exact))
            .expect("builder selection serializes")
    }

    pub fn estimates(&mut self, simulation: &mut WasmSimulation, complete: bool) -> String {
        worker_estimates_json(worker_update(
            simulation,
            &mut self.end_speed_cache,
            complete,
        ))
    }
}

impl Default for BuilderSessionBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderSessionError {
    Bridge {
        action: String,
        message: String,
    },
    BridgeRefusedWithoutMessage {
        action: String,
    },
    InvalidRide(String),
    RideStep {
        step_index: usize,
        piece_index: Option<usize>,
        message: String,
    },
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
                piece_index,
                message,
            } => {
                if let Some(piece_index) = piece_index {
                    write!(
                        formatter,
                        "ride step {step_index} (piece {piece_index}): {message}"
                    )
                } else {
                    write!(formatter, "ride step {step_index}: {message}")
                }
            }
        }
    }
}

impl std::error::Error for BuilderSessionError {}

fn start_construction(simulation: &mut WasmSimulation, x: i32, y: i32, z: i32, heading: u8) {
    simulation.construction_start(x * 2, y * 2, z / 2, heading * 2);
}

fn select_piece(
    simulation: &mut WasmSimulation,
    end_speed_cache: Option<f64>,
    id: &str,
    exact: bool,
) -> BuilderSelection {
    let speed_match = simulation.construction_speed_match(id, end_speed_cache);
    let selected_id = if exact {
        id
    } else {
        speed_match
            .as_ref()
            .and_then(|speed_match| speed_match.recommended_id)
            .unwrap_or(id)
    }
    .to_owned();
    simulation.construction_select(&selected_id);
    BuilderSelection {
        requested_id: id.to_owned(),
        selected_id,
        speed_match,
    }
}

fn worker_update(
    simulation: &mut WasmSimulation,
    end_speed_cache: &mut Option<f64>,
    complete: bool,
) -> BuilderEstimates {
    let force_preview = if complete {
        simulation.construction_force_preview()
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
    let choices = simulation.construction_choices_preview(end_speed, status);
    let elements = if complete {
        simulation.construction_elements_typed()
    } else {
        Vec::new()
    };
    if complete {
        *end_speed_cache = end_speed;
    }
    let hill_options_json = simulation.construction_hill_options();
    let speed_turns_json = simulation.construction_speed_turns();
    let candidate_preview = if complete {
        simulation.construction_candidate_force_preview()
    } else {
        None
    };
    let selected_shape = simulation.construction_selected_shape_wire(complete);
    let selected = simulation.construction_selected();
    let speed_match = if complete {
        simulation.construction_speed_match(&selected, *end_speed_cache)
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
        selected_shape,
        speed_match,
    }
}

fn worker_estimates_json(estimates: BuilderEstimates) -> String {
    estimates.to_worker_json()
}

impl BuilderSession {
    pub fn new(simulation: WasmSimulation) -> Self {
        Self {
            simulation,
            end_speed_cache: None,
            placed_piece_bridge_ids: Vec::new(),
        }
    }

    pub fn simulation(&self) -> &WasmSimulation {
        &self.simulation
    }

    pub fn placed_piece_bridge_ids(&self) -> &[Vec<usize>] {
        &self.placed_piece_bridge_ids
    }

    pub fn save_native_snapshot(&self) -> Result<Vec<u8>, String> {
        let snapshot = BuilderSessionSnapshot {
            simulation: self.simulation.save_state_native()?,
            end_speed_cache: self.end_speed_cache,
            placed_piece_bridge_ids: self.placed_piece_bridge_ids.clone(),
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
        select_piece(
            &mut self.simulation,
            self.end_speed_cache,
            &format!("model:{model_key}"),
            true,
        );
        self.check_bridge_message("select coaster model")?;
        start_construction(&mut self.simulation, x, y, z, heading);
        self.check_bridge_message("start construction")
    }

    pub fn select(
        &mut self,
        id: &str,
        exact: bool,
    ) -> Result<BuilderSelection, BuilderSessionError> {
        let selection = select_piece(&mut self.simulation, self.end_speed_cache, id, exact);
        if !self.simulation.construction_message().is_empty() {
            let message = self.simulation.construction_message();
            let message = if message == "Unknown piece" {
                format!("{message}; offered ids: {}", self.offered_ids())
            } else {
                message
            };
            return Err(self.bridge_error("select piece", message));
        }
        Ok(selection)
    }

    /// Reads the bridge estimates used by the builder panel in the worker.
    pub fn worker_update(&mut self, complete: bool) -> BuilderEstimates {
        worker_update(&mut self.simulation, &mut self.end_speed_cache, complete)
    }

    pub fn estimates(&mut self) -> BuilderEstimates {
        self.worker_update(true)
    }

    pub fn place(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.append_selected()?;
        Ok(self.estimates())
    }

    pub fn append_selected(&mut self) -> Result<(), BuilderSessionError> {
        self.end_speed_cache = None;
        let previous_piece_count = self
            .simulation
            .editing_construction()
            .map(|construction| construction.piece_ids().len());
        self.simulation.construction_finish_invalidate();
        self.simulation.construction_append();
        self.check_bridge_message("place selected piece")?;
        if let (Some(previous_piece_count), Some(construction)) =
            (previous_piece_count, self.simulation.editing_construction())
        {
            if let Some(piece_ids) = construction.piece_ids().get(previous_piece_count..) {
                if !piece_ids.is_empty() {
                    self.placed_piece_bridge_ids.push(piece_ids.to_vec());
                }
            }
        }
        Ok(())
    }

    pub fn append_resolved(
        &mut self,
        intent: &ElementIntent,
        spec: &ElementSpec,
    ) -> Result<BuilderEstimates, BuilderSessionError> {
        self.end_speed_cache = None;
        let previous_piece_count = self
            .simulation
            .editing_construction()
            .map(|construction| construction.piece_ids().len());
        self.simulation
            .construction_append_resolved_typed(intent, spec)
            .map_err(|message| self.bridge_error("place saved shaped element", message))?;
        if let (Some(previous_piece_count), Some(construction)) =
            (previous_piece_count, self.simulation.editing_construction())
        {
            if let Some(piece_ids) = construction.piece_ids().get(previous_piece_count..) {
                if !piece_ids.is_empty() {
                    self.placed_piece_bridge_ids.push(piece_ids.to_vec());
                }
            }
        }
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

    /// Places the best-ranked way back to the station and closes the circuit.
    pub fn finish(&mut self) -> Result<BuilderEstimates, BuilderSessionError> {
        self.finish_apply(1)
    }

    /// Runs the finish search to completion using the global ASTM envelope, so
    /// `construction_can_finish` and `construction_finish_reason` describe the current track
    /// instead of a pending search.
    pub fn search_finish(&mut self) {
        self.search_finish_with_judge(FinishForceJudge::Envelope);
    }

    pub fn search_finish_with_judge(&mut self, judge: FinishForceJudge) {
        self.simulation
            .construction_finish_search_start_with_judge(judge);
        while !self.simulation.construction_finish_search_advance() {}
    }

    pub fn finish_options(&mut self) -> Result<Vec<Completion>, BuilderSessionError> {
        self.finish_options_with_judge(FinishForceJudge::Envelope)
    }

    pub fn finish_options_with_judge(
        &mut self,
        judge: FinishForceJudge,
    ) -> Result<Vec<Completion>, BuilderSessionError> {
        self.search_finish_with_judge(judge);
        if !self.simulation.construction_can_finish() {
            return Err(self.bridge_error(
                "finish coaster",
                self.simulation.construction_finish_reason(),
            ));
        }
        Ok(self.simulation.construction_finish_completions().to_vec())
    }

    /// Places finish option `rank` (1 is the best) and closes the circuit.
    pub fn finish_apply(&mut self, rank: usize) -> Result<BuilderEstimates, BuilderSessionError> {
        self.finish_apply_with_judge(rank, FinishForceJudge::Envelope)
    }

    pub fn finish_apply_with_judge(
        &mut self,
        rank: usize,
        judge: FinishForceJudge,
    ) -> Result<BuilderEstimates, BuilderSessionError> {
        self.end_speed_cache = None;
        let option_count = self.finish_options_with_judge(judge)?.len();
        if rank == 0 || rank > option_count {
            return Err(self.bridge_error(
                "finish coaster",
                format!("finish option {rank} does not exist; the search found {option_count}"),
            ));
        }
        let previous_piece_count = self
            .simulation
            .editing_construction()
            .map(|construction| construction.piece_ids().len());
        self.simulation.construction_finish_preview();
        for _ in 1..rank {
            self.simulation.construction_finish_cycle(1);
        }
        self.simulation.construction_finish_confirm();
        self.check_bridge_message("confirm finished coaster")?;
        if let (Some(previous_piece_count), Some(construction)) =
            (previous_piece_count, self.simulation.editing_construction())
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
        if self
            .simulation
            .editing_construction()
            .is_some_and(|construction| construction.is_closed())
        {
            return Err(self.bridge_error(
                "close circuit",
                "The coaster circuit is already closed.".to_owned(),
            ));
        }
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

    pub fn auto_blocks_with_budget(
        &mut self,
        work_budget: u32,
    ) -> Result<(serde_json::Value, usize), BuilderSessionError> {
        if work_budget == 0 {
            return Err(BuilderSessionError::InvalidRide(
                "automatic block work budget must be positive".into(),
            ));
        }
        if !self.simulation.construction_auto_blocks() {
            return Err(self.bridge_refusal("place automatic block sections"));
        }
        let initial_work = self.block_report_value()?["placementWorkCompleted"]
            .as_u64()
            .ok_or_else(|| {
                self.bridge_error(
                    "read block report",
                    "placement work count is missing".into(),
                )
            })?;
        let mut remaining_budget = work_budget;
        while remaining_budget > 0 {
            let current_budget = remaining_budget.min(1_000);
            let complete = self
                .simulation
                .construction_advance_block_analysis(current_budget);
            remaining_budget -= current_budget;
            let message = self.simulation.construction_message();
            if !message.is_empty() {
                return Err(self.bridge_error("analyse block sections", message));
            }
            if complete {
                break;
            }
        }
        let searching_report = self.block_report_value()?;
        let work_completed = searching_report["placementWorkCompleted"]
            .as_u64()
            .ok_or_else(|| {
                self.bridge_error(
                    "read block report",
                    "placement work count is missing".into(),
                )
            })?
            .saturating_sub(initial_work) as usize;
        if work_completed > work_budget as usize {
            return Err(self.bridge_error(
                "analyse block sections",
                format!("block search used {work_completed} work units for budget {work_budget}"),
            ));
        }
        let search_status = self.block_report_value()?;
        if search_status["placementStatus"] == "searching"
            && !self.simulation.construction_apply_best_block_sections()
        {
            return Err(self.bridge_refusal("install current best block sections"));
        }
        Ok((self.block_report_value()?, work_completed))
    }

    pub fn install_block_sections(
        &mut self,
        block_sections: &[BlockSectionPlacement],
        train_capacity: usize,
        capacity_search_complete: bool,
        placement_search_complete: bool,
    ) -> Result<serde_json::Value, BuilderSessionError> {
        let block_sections_json = serde_json::to_string(block_sections).map_err(|error| {
            self.bridge_error("serialize recorded block layout", error.to_string())
        })?;
        if !self.simulation.construction_install_block_layout(
            &block_sections_json,
            train_capacity,
            capacity_search_complete,
            placement_search_complete,
        ) {
            return Err(self.bridge_refusal("install recorded block layout"));
        }
        self.block_report_value()
    }

    fn block_report_value(&self) -> Result<serde_json::Value, BuilderSessionError> {
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
        self.placed_piece_bridge_ids.clear();
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
        .map_err(|error| invalid_ride_step(1, None, error))?;
        self.set_cars_per_train(ride.cars_per_train)
            .map_err(|error| invalid_ride_step(1, None, error))?;

        for (step_offset, step) in ride.steps.iter().enumerate() {
            let step_index = step_offset + 1;
            match step {
                RideStep::Place(place) => {
                    let piece_index = self
                        .simulation
                        .editing_construction()
                        .expect("ride replay has active construction")
                        .pieces()
                        .len();
                    self.select(&place.place, true)
                        .map_err(|error| invalid_ride_step(step_index, Some(piece_index), error))?;
                    self.append_selected()
                        .map_err(|error| invalid_ride_step(step_index, Some(piece_index), error))?;
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
                                piece_index: Some(piece_index),
                                message: "placed launch has no bridge piece id".into(),
                            })?;
                        self.set_launch_speed(piece_index, target_speed)
                            .map_err(|error| {
                                invalid_ride_step(step_index, Some(piece_index), error)
                            })?;
                    }
                }
                RideStep::Shape(shape) => {
                    let piece_index = self
                        .simulation
                        .editing_construction()
                        .expect("ride replay has active construction")
                        .pieces()
                        .len();
                    self.append_resolved(&shape.intent, &shape.spec)
                        .map_err(|error| invalid_ride_step(step_index, Some(piece_index), error))?;
                }
                RideStep::Chain(chain) => {
                    self.set_chain(chain.chain)
                        .map_err(|error| invalid_ride_step(step_index, None, error))?;
                }
                RideStep::Close(close) => {
                    if !close.close {
                        return Err(BuilderSessionError::RideStep {
                            step_index,
                            piece_index: None,
                            message: "close must be true".into(),
                        });
                    }
                    self.close_track()
                        .map_err(|error| invalid_ride_step(step_index, None, error))?;
                }
                RideStep::BlockSections(blocks) => {
                    self.install_block_sections(
                        &blocks.block_sections,
                        blocks.train_capacity,
                        blocks.capacity_search_complete,
                        blocks.placement_search_complete,
                    )
                    .map_err(|error| invalid_ride_step(step_index, None, error))?;
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
                .map_err(|error| invalid_ride_step(ride.steps.len() + 1, None, error))?;
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

fn invalid_ride_step(
    step_index: usize,
    piece_index: Option<usize>,
    error: BuilderSessionError,
) -> BuilderSessionError {
    BuilderSessionError::RideStep {
        step_index,
        piece_index,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::{BuilderSession, BuilderSessionBridge};
    use crate::WasmSimulation;
    use theme_park_sim::track::blocks::BlockSectionPlacement;
    use theme_park_sim::track::ride_file::{
        RideBlockSectionsStep, RideCloseStep, RideFile, RideFileFormat, RideHeading, RidePlaceStep,
        RideStation, RideStep,
    };

    const CIRCUITS: &str = include_str!("classic-circuit-digest-fixtures.txt");

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
                .editing_construction()
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
            .editing_construction()
            .expect("construction started")
            .next_piece_id();

        session.select("Straight", true).unwrap();
        session.worker_update(true);
        session.place().unwrap();
        assert_ne!(session.simulation().construction_track(), initial_track);

        session.undo().unwrap();
        let construction = session
            .simulation()
            .editing_construction()
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
                RideStep::Shape(shape) => {
                    session.append_resolved(&shape.intent, &shape.spec).unwrap();
                    session.estimates();
                }
                RideStep::Chain(chain) => {
                    session.set_chain(chain.chain).unwrap();
                    session.estimates();
                }
                RideStep::Close(close) => {
                    assert!(close.close);
                    session.close().unwrap();
                }
                RideStep::BlockSections(blocks) => {
                    session
                        .install_block_sections(
                            &blocks.block_sections,
                            blocks.train_capacity,
                            blocks.capacity_search_complete,
                            blocks.placement_search_complete,
                        )
                        .unwrap();
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
    fn explicit_block_sections_replay_the_same_layout_and_state_hash() {
        const BLOCK_RIDE: &str = r#"{
            "format": "heartline-ride",
            "version": 1,
            "name": "Explicit block replay test",
            "model": "hybrid",
            "station": { "x": 100, "y": 100, "z": 20, "heading": "east" },
            "carsPerTrain": 4,
            "trains": 1,
            "steps": [
                { "place": "Straight" },
                { "place": "Straight" },
                { "chain": true },
                { "place": "Straight" },
                { "chain": false },
                { "place": "Straight" },
                { "place": "Straight" },
                { "place": "TurnLargeEntry" },
                { "place": "TurnLargeExit" },
                { "place": "Straight" },
                { "place": "Straight" },
                { "chain": true },
                { "place": "Straight" },
                { "chain": false },
                { "place": "Straight" },
                { "place": "Straight" },
                { "place": "Straight" },
                { "place": "TurnLargeEntry" },
                { "place": "TurnLargeExit" },
                { "close": true }
            ]
        }"#;
        const SEARCH_WORK_BUDGET: u32 = 1_000;

        let base_ride: RideFile = serde_json::from_str(BLOCK_RIDE).unwrap();
        let mut block_search = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        block_search.replay_ride(&base_ride).unwrap();
        let (search_report, work_completed) = block_search
            .auto_blocks_with_budget(SEARCH_WORK_BUDGET)
            .unwrap();
        assert!(work_completed <= SEARCH_WORK_BUDGET as usize);

        let sections = search_report["sections"]
            .as_array()
            .expect("block report sections are an array");
        let block_sections: Vec<BlockSectionPlacement> = sections
            .iter()
            .filter(|section| section["editable"] == true)
            .map(|section| {
                let brake_piece_indices = section["brakePieceIndices"]
                    .as_array()
                    .expect("editable block section has brake piece indices")
                    .iter()
                    .map(|piece_index| {
                        piece_index
                            .as_u64()
                            .expect("brake piece index is nonnegative")
                            as usize
                    })
                    .collect();
                BlockSectionPlacement {
                    boundary_metres: section["boundaryMetres"]
                        .as_f64()
                        .expect("editable block section has a boundary"),
                    brake_start_metres: section["brakeStartMetres"]
                        .as_f64()
                        .expect("editable block section has a brake start"),
                    brake_piece_indices,
                }
            })
            .collect();
        let train_capacity = search_report["trainCapacity"]
            .as_u64()
            .expect("block report train capacity") as usize;
        let capacity_search_complete = search_report["capacitySearchComplete"]
            .as_bool()
            .expect("block capacity completion status");
        let placement_search_complete = search_report["placementSearchComplete"]
            .as_bool()
            .expect("block placement completion status");
        block_search
            .install_block_sections(
                &block_sections,
                train_capacity,
                capacity_search_complete,
                placement_search_complete,
            )
            .unwrap();
        let mut explicit_ride = base_ride;
        explicit_ride
            .steps
            .push(RideStep::BlockSections(RideBlockSectionsStep {
                block_sections,
                train_capacity,
                capacity_search_complete,
                placement_search_complete,
            }));

        let mut first_replay = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut second_replay = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        first_replay.replay_ride(&explicit_ride).unwrap();
        second_replay.replay_ride(&explicit_ride).unwrap();
        let first_block_report: serde_json::Value =
            serde_json::from_str(&first_replay.simulation().construction_block_report_json())
                .unwrap();
        let second_block_report: serde_json::Value =
            serde_json::from_str(&second_replay.simulation().construction_block_report_json())
                .unwrap();
        let normalized_block_report: serde_json::Value =
            serde_json::from_str(&block_search.simulation().construction_block_report_json())
                .unwrap();

        assert_eq!(first_replay.state_hash(), second_replay.state_hash());
        assert_eq!(block_search.state_hash(), first_replay.state_hash());
        assert_ne!(first_block_report["placementStatus"], "searching");
        assert_ne!(first_block_report["placementStatus"], "validating");
        assert_eq!(
            first_block_report["sections"],
            second_block_report["sections"]
        );
        assert_eq!(
            normalized_block_report["sections"],
            first_block_report["sections"]
        );
        assert_eq!(first_block_report["sections"], search_report["sections"]);
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
        unsupported[version_index] = 1;
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
        std::thread::scope(|scope| {
            for (ride_name, model_key) in [
                ("Timber Tumbler", "hybrid"),
                ("Velocity Viper", "velocity_viper"),
                ("Briar Blitz", "wooden_twister"),
            ] {
                scope.spawn(move || {
                    let ride = fixture_ride_file(ride_name);
                    assert_eq!(ride.model, model_key, "{ride_name} ride model");
                    let commands = ride_commands(&ride);
                    let expected = benchmark_trace(ride_name, model_key, &commands);
                    let actual = session_trace(model_key, &commands);
                    assert_eq!(
                        trace_digest(&actual),
                        trace_digest(&expected),
                        "{ride_name}: {}",
                        first_trace_difference(&expected, &actual)
                    );
                });
            }
        });
    }

    #[test]
    fn classic_builder_outputs_keep_the_recorded_digests() {
        let mismatches: Vec<String> = std::thread::scope(|scope| {
            let checks: Vec<_> = [
                ("Timber Tumbler", "hybrid", 0xc4a457b50554c017),
                ("Velocity Viper", "velocity_viper", 0x31ecbb395bf3d36d),
                ("Briar Blitz", "wooden_twister", 0x1c5eaef31b4a2a0c),
            ]
            .into_iter()
            .map(|(ride_name, model_key, expected_digest)| {
                scope.spawn(move || {
                    let commands = circuit_lines(ride_name);
                    let actual = session_trace(model_key, &commands);
                    let actual_digest = trace_digest(&actual);
                    (actual_digest != expected_digest).then(|| {
                        let benchmark = benchmark_trace(ride_name, model_key, &commands);
                        format!(
                            "{ride_name}: expected {expected_digest:016x}, got {actual_digest:016x} ({})",
                            first_trace_difference(&benchmark, &actual)
                        )
                    })
                })
            })
            .collect();
            checks
                .into_iter()
                .filter_map(|check| check.join().unwrap())
                .collect()
        });
        assert!(
            mismatches.is_empty(),
            "classic builder digest mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    fn circuit_lines(ride_name: &str) -> Vec<String> {
        let header = format!("[{ride_name}]");
        let mut sections = CIRCUITS.split(&header);
        sections
            .next()
            .expect("classic circuit fixture has a preamble");
        sections
            .next()
            .unwrap_or_else(|| panic!("{ride_name} classic circuit is missing"))
            .lines()
            .map(str::trim)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn content_ride_builder_hashes_match_recorded_baselines() {
        let mut mismatches = Vec::new();
        for (ride_name, ride_json, expected_hash) in [
            (
                "Briar Blitz",
                include_str!("../../../content/rides/briar-blitz.ride.json"),
                0x2f041e98bcc4db3c,
            ),
            (
                "Browser agreement fast circuit",
                include_str!("../../../content/rides/browser-agreement-fast.ride.json"),
                0x3414060cb78a3d24,
            ),
            (
                "Daisy Dash",
                include_str!("../../../content/rides/daisy-dash.ride.json"),
                0x2b92b2bf56dbcae9,
            ),
            (
                "Horizon Hawk",
                include_str!("../../../content/rides/horizon-hawk.ride.json"),
                0xbd3e732c2585a903,
            ),
            (
                "Timber Tumbler",
                include_str!("../../../content/rides/timber-tumbler.ride.json"),
                0x25b289cb5a8d252f,
            ),
            (
                "Velocity Viper",
                include_str!("../../../content/rides/velocity-viper.ride.json"),
                0x61867397a8222f69,
            ),
        ] {
            let ride: RideFile = serde_json::from_str(ride_json).unwrap();
            assert_eq!(ride.name, ride_name);
            let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
            session.replay_ride(&ride).unwrap();
            let actual_hash = session.state_hash();
            if actual_hash != expected_hash {
                mismatches.push(format!(
                    "{ride_name}: expected {expected_hash:016x}, got {actual_hash:016x}"
                ));
            }
        }
        assert!(
            mismatches.is_empty(),
            "content ride hash mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn content_ride_builder_output_traces_match_recorded_baselines() {
        let mismatches: Vec<String> = std::thread::scope(|scope| {
            let checks: Vec<_> = [
                (
                    "Briar Blitz",
                    include_str!("../../../content/rides/briar-blitz.ride.json"),
                    0x6319ddb9332ae2bf,
                ),
                (
                    "Browser agreement fast circuit",
                    include_str!("../../../content/rides/browser-agreement-fast.ride.json"),
                    0xe5df39dec6b2014a,
                ),
                (
                    "Daisy Dash",
                    include_str!("../../../content/rides/daisy-dash.ride.json"),
                    0x424647458a464a17,
                ),
                (
                    "Horizon Hawk",
                    include_str!("../../../content/rides/horizon-hawk.ride.json"),
                    0x3df9651c78413d65,
                ),
                (
                    "Timber Tumbler",
                    include_str!("../../../content/rides/timber-tumbler.ride.json"),
                    0xf0414f79257fb302,
                ),
                (
                    "Velocity Viper",
                    include_str!("../../../content/rides/velocity-viper.ride.json"),
                    0xa85d602e7c4c1295,
                ),
            ]
            .into_iter()
            .map(|(ride_name, ride_json, expected_digest)| {
                scope.spawn(move || {
                    let ride: RideFile = serde_json::from_str(ride_json).unwrap();
                    assert_eq!(ride.name, ride_name);
                    let trace = content_ride_trace(&ride);
                    let actual_digest = trace_digest(&trace);
                    (actual_digest != expected_digest).then(|| {
                        format!(
                            "{ride_name}: expected {expected_digest:016x}, got {actual_digest:016x} ({} trace calls)",
                            trace.len()
                        )
                    })
                })
            })
            .collect();
            checks
                .into_iter()
                .filter_map(|check| check.join().unwrap())
                .collect()
        });
        assert!(
            mismatches.is_empty(),
            "content ride trace mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    fn content_ride_trace(ride: &RideFile) -> Trace {
        let heading = match ride.station.heading {
            RideHeading::East => 0,
            RideHeading::North => 1,
            RideHeading::West => 2,
            RideHeading::South => 3,
        };
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        session
            .start(
                &ride.model,
                ride.station.x,
                ride.station.y,
                ride.station.z,
                heading,
            )
            .unwrap();
        session.set_cars_per_train(ride.cars_per_train).unwrap();
        let mut trace = Vec::new();
        push_content_ride_snapshot(&session, &mut trace);

        for (step_offset, step) in ride.steps.iter().enumerate() {
            match step {
                RideStep::Place(place) => {
                    let selection = session.select(&place.place, true).unwrap();
                    trace_call(
                        &mut trace,
                        "content:selection_speed_match",
                        serde_json::to_string(&selection.speed_match).unwrap(),
                    );
                    push_content_ride_estimates(&mut session, &mut trace, true);
                    session.append_selected().unwrap();
                    if let Some(target_speed) = place.launch_speed {
                        let track: serde_json::Value =
                            serde_json::from_str(&session.simulation().construction_track())
                                .unwrap();
                        let piece_index = track.as_array().unwrap().last().unwrap()["id"]
                            .as_u64()
                            .unwrap() as usize;
                        session.set_launch_speed(piece_index, target_speed).unwrap();
                    }
                    push_content_ride_estimates(&mut session, &mut trace, false);
                    push_content_ride_estimates(&mut session, &mut trace, true);
                }
                RideStep::Shape(shape) => {
                    session.append_resolved(&shape.intent, &shape.spec).unwrap();
                    push_content_ride_estimates(&mut session, &mut trace, true);
                }
                RideStep::Chain(chain) => {
                    session.set_chain(chain.chain).unwrap();
                    push_content_ride_estimates(&mut session, &mut trace, true);
                }
                RideStep::Close(close) => {
                    assert!(close.close);
                    session.close_track().unwrap();
                    push_content_ride_estimates(&mut session, &mut trace, true);
                }
                RideStep::BlockSections(blocks) => {
                    session
                        .install_block_sections(
                            &blocks.block_sections,
                            blocks.train_capacity,
                            blocks.capacity_search_complete,
                            blocks.placement_search_complete,
                        )
                        .unwrap();
                    push_content_ride_estimates(&mut session, &mut trace, true);
                }
            }
            trace_call(&mut trace, "content:step_index", step_offset + 1);
            push_content_ride_snapshot(&session, &mut trace);
        }
        trace
    }

    fn push_content_ride_estimates(
        session: &mut BuilderSession,
        trace: &mut Trace,
        complete: bool,
    ) {
        let estimates = session.worker_update(complete);
        trace_call(
            trace,
            if complete {
                "content:complete_update"
            } else {
                "content:partial_update"
            },
            estimates.to_worker_json(),
        );
    }

    fn push_content_ride_snapshot(session: &BuilderSession, trace: &mut Trace) {
        trace_call(trace, "content:state_hash", session.state_hash());
        trace_call(
            trace,
            "content:track",
            session.simulation().construction_track(),
        );
    }

    #[test]
    fn worker_bridge_uses_the_same_selection_and_estimate_sequence() {
        let mut native_session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut worker_simulation = WasmSimulation::new(42, 0, "new".into());
        let mut worker_session = BuilderSessionBridge::new();

        native_session.start("hybrid", 100, 100, 12, 1).unwrap();
        worker_session.select(&mut worker_simulation, "model:hybrid", false);
        worker_session.start(&mut worker_simulation, 100, 100, 12, 1);
        assert_eq!(
            native_session.simulation().state_hash(),
            worker_simulation.state_hash()
        );

        let native_initial = native_session.worker_update(true);
        let worker_initial: serde_json::Value =
            serde_json::from_str(&worker_session.estimates(&mut worker_simulation, true)).unwrap();
        assert_eq!(
            worker_initial,
            serde_json::from_str::<serde_json::Value>(&native_initial.to_worker_json()).unwrap()
        );

        let native_selection = native_session.select("Straight", true).unwrap();
        let worker_selection: serde_json::Value =
            serde_json::from_str(&worker_session.select(&mut worker_simulation, "Straight", true))
                .unwrap();
        assert_eq!(
            worker_selection,
            serde_json::to_value(native_selection).unwrap()
        );
        native_session.append_selected().unwrap();
        worker_session.invalidate();
        worker_simulation.construction_finish_invalidate();
        worker_simulation.construction_append();

        let native_complete = native_session.worker_update(true);
        let worker_complete: serde_json::Value =
            serde_json::from_str(&worker_session.estimates(&mut worker_simulation, true)).unwrap();
        assert_eq!(
            worker_complete,
            serde_json::from_str::<serde_json::Value>(&native_complete.to_worker_json()).unwrap()
        );

        let native_selection = native_session.select("Loop", false).unwrap();
        let worker_selection: serde_json::Value =
            serde_json::from_str(&worker_session.select(&mut worker_simulation, "Loop", false))
                .unwrap();
        assert_eq!(
            worker_selection,
            serde_json::to_value(native_selection).unwrap()
        );
        assert_eq!(
            native_session.simulation().construction_selected(),
            worker_simulation.construction_selected()
        );

        let native_partial = native_session.worker_update(false);
        let worker_partial: serde_json::Value =
            serde_json::from_str(&worker_session.estimates(&mut worker_simulation, false)).unwrap();
        assert_eq!(
            worker_partial,
            serde_json::from_str::<serde_json::Value>(&native_partial.to_worker_json()).unwrap()
        );

        worker_session.select(&mut worker_simulation, "unknown-piece", false);
        assert_eq!(worker_simulation.construction_message(), "Unknown piece");
    }

    fn fixture_ride_file(ride_name: &str) -> RideFile {
        let source = match ride_name {
            "Timber Tumbler" => include_str!("../../../content/rides/timber-tumbler.ride.json"),
            "Velocity Viper" => include_str!("../../../content/rides/velocity-viper.ride.json"),
            "Briar Blitz" => include_str!("../../../content/rides/briar-blitz.ride.json"),
            _ => panic!("unknown journey fixture {ride_name}"),
        };
        let ride: RideFile = serde_json::from_str(source)
            .unwrap_or_else(|error| panic!("{ride_name} ride file is invalid: {error}"));
        assert_eq!(ride.name, ride_name);
        ride
    }

    fn ride_commands(ride: &RideFile) -> Vec<String> {
        ride.steps
            .iter()
            .filter_map(|step| match step {
                RideStep::Place(place) => Some(place.place.clone()),
                RideStep::Chain(chain) => {
                    Some(if chain.chain { "+chain" } else { "-chain" }.into())
                }
                RideStep::Shape(_) => panic!("classic ride command fixture contains a shaped step"),
                RideStep::Close(_) | RideStep::BlockSections(_) => None,
            })
            .collect()
    }

    fn benchmark_trace(ride_name: &str, model_key: &str, commands: &[String]) -> Trace {
        let mut simulation = WasmSimulation::new(42, 0, "new".into());
        let mut trace = Vec::new();
        simulation.construction_select(&format!("model:{model_key}"));
        simulation.construction_start(482, 498, 6, 2);
        let mut end_speed = None;
        benchmark_update(&mut simulation, true, &mut end_speed, &mut trace);

        let mut piece_count = 1;
        for command in commands {
            match command.as_str() {
                "+chain" | "-chain" => {
                    simulation.construction_set_chain(command == "+chain");
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

    fn session_trace(model_key: &str, commands: &[String]) -> Trace {
        let mut session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let mut trace = Vec::new();
        session.start(model_key, 241, 249, 12, 1).unwrap();
        session_update(&mut session, true, &mut trace);

        for command in commands {
            match command.as_str() {
                "+chain" | "-chain" => {
                    session.set_chain(command == "+chain").unwrap();
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
        if expected.len() == actual.len() {
            return "traces match".into();
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
            simulation.coaster_supports(crate::guests::DEMO_COASTER_RENDER_ID),
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
