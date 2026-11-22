use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use theme_park_sim::facilities::ParkFacilities;
use theme_park_sim::guests::GuestId;
use theme_park_sim::guests::behavior::Facility;
use theme_park_sim::park::{Coaster, CoasterId, Park};
use theme_park_sim::park::{DemoCoaster, TunnelOwner};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{
    BlockOperation, Circuit, ConstructionTestRunRecorder, TestRun, Train,
};
use theme_park_sim::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, ThroughputEstimator,
};
use theme_park_sim::track::clearance::TerrainCell;
use theme_park_sim::track::construction::{Completion, CompletionSearch, Construction};
use theme_park_sim::track::shaped::ElementIntent;
use theme_park_sim::track::{Piece, PieceId};
use theme_park_sim::world::{Position, World};

use crate::WasmSimulation;

/// Version 6 of the postcard save layout. Keep field order aligned with the
/// version 6 `WasmSimulation` and `Park` declarations.
#[derive(Deserialize)]
pub(super) struct WasmSimulationV6 {
    pub(super) park: ParkV6,
    pub(super) facility_message: String,
    pub(super) path_message: String,
    pub(super) terrain_message: String,
    pub(super) scenery_message: String,
    pub(super) terrain_invalidated_paths: Vec<Position>,
    #[serde(with = "crate::construction_model_serde")]
    pub(super) construction_model: String,
    pub(super) selected: Option<PieceId>,
    #[serde(default)]
    pub(super) selected_shape: Option<ElementIntent>,
    pub(super) selected_element: Option<String>,
    pub(super) construction_undo_groups: Vec<usize>,
    pub(super) construction_completion: Option<Completion>,
    pub(super) construction_completion_available: Vec<Completion>,
    pub(super) construction_completion_search: Option<CompletionSearch>,
    pub(super) construction_completion_checked: bool,
    pub(super) construction_completion_unavailable_reason: Option<String>,
    #[serde(skip)]
    pub(super) construction_completion_index: usize,
    #[serde(skip)]
    pub(super) construction_preview_supports_cache: Vec<f32>,
    #[serde(skip)]
    pub(super) construction_placed_supports_cache: Vec<f32>,
    pub(super) recovered_station: Option<Piece>,
}

#[derive(Deserialize)]
pub(super) struct ParkV6 {
    pub(super) world: World,
    pub(super) facilities: ParkFacilities,
    pub(super) simulation: ParkGuests,
    pub(super) demo_coaster: Option<DemoCoaster>,
    pub(super) construction: Option<Construction>,
    pub(super) automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwner>>,
    pub(super) manual_tunnel_cells: BTreeSet<TerrainCell>,
    pub(super) construction_tunnel_generation: u64,
    pub(super) physics_remainder: u32,
    pub(super) construction_train: Option<(Circuit, Train)>,
    pub(super) construction_message: String,
    pub(super) construction_slow_steps: u32,
    pub(super) construction_outcome: Option<String>,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) construction_recorder: Option<ConstructionTestRunRecorder>,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) construction_report: Option<TestRun>,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) demo_report: Option<TestRun>,
    pub(super) coaster_open: bool,
    pub(super) coaster_price: u16,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) coaster_operation: Option<BlockOperation>,
    pub(super) coaster_riders: BTreeMap<u64, Vec<GuestId>>,
    pub(super) coaster_facility: Option<Facility>,
    pub(super) coaster_ratings: Option<(u8, u8)>,
    pub(super) block_layout: Option<BlockLayout>,
    pub(super) desired_train_count: Option<usize>,
    pub(super) block_placement_search: Option<BlockPlacementSearch>,
    pub(super) block_manual_search: Option<BlockManualSearch>,
    pub(super) block_apply_best_requested: bool,
    pub(super) block_placement_cache: Option<BlockPlacementSearch>,
    pub(super) block_baseline_validation: bool,
    pub(super) block_layout_is_automatic: bool,
    pub(super) block_message: String,
    pub(super) block_hourly_throughput: f64,
    pub(super) block_throughput_reason: String,
    pub(super) block_throughput_estimator: Option<ThroughputEstimator>,
    pub(super) block_analysis_extra_to_throughput: bool,
    pub(super) block_throughput_cache: BTreeMap<Vec<u64>, (f64, String)>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub(super) support_render_cache:
        BTreeMap<usize, theme_park_sim::park::tick::SupportRenderCache>,
}

impl WasmSimulationV6 {
    pub(super) fn into_current(self) -> Result<WasmSimulation, &'static str> {
        let Self {
            park,
            facility_message,
            path_message,
            terrain_message,
            scenery_message,
            terrain_invalidated_paths,
            construction_model,
            selected,
            selected_shape,
            selected_element,
            construction_undo_groups,
            construction_completion,
            construction_completion_available,
            construction_completion_search,
            construction_completion_checked,
            construction_completion_unavailable_reason,
            construction_completion_index,
            construction_preview_supports_cache,
            construction_placed_supports_cache,
            recovered_station,
        } = self;
        let ParkV6 {
            world,
            facilities,
            simulation,
            demo_coaster,
            construction,
            automatic_tunnel_owners,
            manual_tunnel_cells,
            construction_tunnel_generation,
            physics_remainder,
            construction_train,
            construction_message,
            construction_slow_steps,
            construction_outcome,
            construction_recorder,
            construction_report,
            demo_report,
            coaster_open,
            coaster_price,
            coaster_operation,
            coaster_riders,
            coaster_facility,
            coaster_ratings,
            block_layout,
            desired_train_count,
            block_placement_search,
            block_manual_search,
            block_apply_best_requested,
            block_placement_cache,
            block_baseline_validation,
            block_layout_is_automatic,
            block_message,
            block_hourly_throughput,
            block_throughput_reason,
            block_throughput_estimator,
            block_analysis_extra_to_throughput,
            block_throughput_cache,
            ..
        } = park;

        let has_construction = construction.is_some();
        if !has_construction
            && !(construction_train.is_none()
                && construction_slow_steps == 0
                && construction_outcome.is_none()
                && construction_recorder.is_none()
                && construction_report.is_none()
                && !coaster_open
                && coaster_operation.is_none()
                && coaster_riders.is_empty()
                && coaster_facility.is_none()
                && coaster_ratings.is_none()
                && block_layout.is_none()
                && desired_train_count.is_none()
                && block_placement_search.is_none()
                && block_manual_search.is_none()
                && !block_apply_best_requested
                && block_placement_cache.is_none()
                && !block_baseline_validation
                && !block_layout_is_automatic
                && block_message.is_empty()
                && block_hourly_throughput == 0.0
                && block_throughput_reason.is_empty()
                && block_throughput_estimator.is_none()
                && !block_analysis_extra_to_throughput
                && block_throughput_cache.is_empty())
        {
            return Err("version 6 save has per-coaster state without a construction");
        }

        let mut park = Park::new_empty(world, facilities, simulation);
        park.demo_coaster = demo_coaster;
        park.automatic_tunnel_owners = automatic_tunnel_owners;
        park.manual_tunnel_cells = manual_tunnel_cells;
        park.construction_tunnel_generation = construction_tunnel_generation;
        park.physics_remainder = physics_remainder;
        park.construction_message = construction_message;
        park.demo_report = demo_report;
        park.pending_coaster_price = coaster_price;

        if let Some(construction) = construction {
            let mut coaster = Coaster::new(construction, coaster_price);
            coaster.construction_train = construction_train;
            coaster.construction_slow_steps = construction_slow_steps;
            coaster.construction_outcome = construction_outcome;
            coaster.construction_recorder = construction_recorder;
            coaster.construction_report = construction_report;
            coaster.coaster_open = coaster_open;
            coaster.coaster_operation = coaster_operation;
            coaster.coaster_riders = coaster_riders;
            coaster.coaster_facility = coaster_facility;
            coaster.coaster_ratings = coaster_ratings;
            coaster.block_layout = block_layout;
            coaster.desired_train_count = desired_train_count;
            coaster.block_placement_search = block_placement_search;
            coaster.block_manual_search = block_manual_search;
            coaster.block_apply_best_requested = block_apply_best_requested;
            coaster.block_placement_cache = block_placement_cache;
            coaster.block_baseline_validation = block_baseline_validation;
            coaster.block_layout_is_automatic = block_layout_is_automatic;
            coaster.block_message = block_message;
            coaster.block_hourly_throughput = block_hourly_throughput;
            coaster.block_throughput_reason = block_throughput_reason;
            coaster.block_throughput_estimator = block_throughput_estimator;
            coaster.block_analysis_extra_to_throughput = block_analysis_extra_to_throughput;
            coaster.block_throughput_cache = block_throughput_cache;
            park.coasters.insert(CoasterId(0), coaster);
            park.next_coaster_id = 1;
        }

        Ok(WasmSimulation {
            park,
            editing: has_construction.then_some(CoasterId(0)),
            facility_message,
            path_message,
            terrain_message,
            scenery_message,
            terrain_invalidated_paths,
            construction_model,
            selected,
            selected_shape,
            selected_element,
            construction_undo_groups,
            construction_completion,
            construction_completion_available,
            construction_completion_search,
            construction_completion_checked,
            construction_completion_unavailable_reason,
            construction_completion_index,
            construction_preview_supports_cache,
            construction_placed_supports_cache,
            recovered_station,
        })
    }
}

#[cfg(test)]
pub(super) fn serialize_v6_for_test(simulation: &WasmSimulation) -> Vec<u8> {
    let park = &simulation.park;
    let coaster = simulation
        .editing_coaster()
        .expect("version 6 test save requires an edited coaster");
    let snapshot = WasmSimulationV6Ref {
        park: ParkV6Ref {
            world: &park.world,
            facilities: &park.facilities,
            simulation: &park.simulation,
            demo_coaster: &park.demo_coaster,
            construction: Some(&coaster.construction),
            automatic_tunnel_owners: &park.automatic_tunnel_owners,
            manual_tunnel_cells: &park.manual_tunnel_cells,
            construction_tunnel_generation: &park.construction_tunnel_generation,
            physics_remainder: &park.physics_remainder,
            construction_train: coaster.construction_train.as_ref(),
            construction_message: &park.construction_message,
            construction_slow_steps: &coaster.construction_slow_steps,
            construction_outcome: &coaster.construction_outcome,
            construction_recorder: &coaster.construction_recorder,
            construction_report: &coaster.construction_report,
            demo_report: &park.demo_report,
            coaster_open: &coaster.coaster_open,
            coaster_price: &coaster.coaster_price,
            coaster_operation: &coaster.coaster_operation,
            coaster_riders: &coaster.coaster_riders,
            coaster_facility: &coaster.coaster_facility,
            coaster_ratings: &coaster.coaster_ratings,
            block_layout: &coaster.block_layout,
            desired_train_count: &coaster.desired_train_count,
            block_placement_search: &coaster.block_placement_search,
            block_manual_search: &coaster.block_manual_search,
            block_apply_best_requested: &coaster.block_apply_best_requested,
            block_placement_cache: &coaster.block_placement_cache,
            block_baseline_validation: &coaster.block_baseline_validation,
            block_layout_is_automatic: &coaster.block_layout_is_automatic,
            block_message: &coaster.block_message,
            block_hourly_throughput: &coaster.block_hourly_throughput,
            block_throughput_reason: &coaster.block_throughput_reason,
            block_throughput_estimator: &coaster.block_throughput_estimator,
            block_analysis_extra_to_throughput: &coaster.block_analysis_extra_to_throughput,
            block_throughput_cache: &coaster.block_throughput_cache,
        },
        facility_message: &simulation.facility_message,
        path_message: &simulation.path_message,
        terrain_message: &simulation.terrain_message,
        scenery_message: &simulation.scenery_message,
        terrain_invalidated_paths: &simulation.terrain_invalidated_paths,
        construction_model: &simulation.construction_model,
        selected: &simulation.selected,
        selected_shape: &simulation.selected_shape,
        selected_element: &simulation.selected_element,
        construction_undo_groups: &simulation.construction_undo_groups,
        construction_completion: &simulation.construction_completion,
        construction_completion_available: &simulation.construction_completion_available,
        construction_completion_search: &simulation.construction_completion_search,
        construction_completion_checked: &simulation.construction_completion_checked,
        construction_completion_unavailable_reason: &simulation
            .construction_completion_unavailable_reason,
        recovered_station: &simulation.recovered_station,
    };
    postcard::to_allocvec(&snapshot).expect("version 6 test save serializes")
}

#[cfg(test)]
#[derive(serde::Serialize)]
struct WasmSimulationV6Ref<'a> {
    park: ParkV6Ref<'a>,
    facility_message: &'a String,
    path_message: &'a String,
    terrain_message: &'a String,
    scenery_message: &'a String,
    terrain_invalidated_paths: &'a Vec<Position>,
    #[serde(with = "crate::construction_model_serde")]
    construction_model: &'a str,
    selected: &'a Option<PieceId>,
    selected_shape: &'a Option<ElementIntent>,
    selected_element: &'a Option<String>,
    construction_undo_groups: &'a Vec<usize>,
    construction_completion: &'a Option<Completion>,
    construction_completion_available: &'a Vec<Completion>,
    construction_completion_search: &'a Option<CompletionSearch>,
    construction_completion_checked: &'a bool,
    construction_completion_unavailable_reason: &'a Option<String>,
    recovered_station: &'a Option<Piece>,
}

#[cfg(test)]
#[derive(serde::Serialize)]
struct ParkV6Ref<'a> {
    world: &'a World,
    facilities: &'a ParkFacilities,
    simulation: &'a ParkGuests,
    demo_coaster: &'a Option<DemoCoaster>,
    construction: Option<&'a Construction>,
    automatic_tunnel_owners: &'a BTreeMap<TerrainCell, BTreeSet<TunnelOwner>>,
    manual_tunnel_cells: &'a BTreeSet<TerrainCell>,
    construction_tunnel_generation: &'a u64,
    physics_remainder: &'a u32,
    construction_train: Option<&'a (Circuit, Train)>,
    construction_message: &'a String,
    construction_slow_steps: &'a u32,
    construction_outcome: &'a Option<String>,
    construction_recorder: &'a Option<ConstructionTestRunRecorder>,
    construction_report: &'a Option<TestRun>,
    demo_report: &'a Option<TestRun>,
    coaster_open: &'a bool,
    coaster_price: &'a u16,
    coaster_operation: &'a Option<BlockOperation>,
    coaster_riders: &'a BTreeMap<u64, Vec<GuestId>>,
    coaster_facility: &'a Option<Facility>,
    coaster_ratings: &'a Option<(u8, u8)>,
    block_layout: &'a Option<BlockLayout>,
    desired_train_count: &'a Option<usize>,
    block_placement_search: &'a Option<BlockPlacementSearch>,
    block_manual_search: &'a Option<BlockManualSearch>,
    block_apply_best_requested: &'a bool,
    block_placement_cache: &'a Option<BlockPlacementSearch>,
    block_baseline_validation: &'a bool,
    block_layout_is_automatic: &'a bool,
    block_message: &'a String,
    block_hourly_throughput: &'a f64,
    block_throughput_reason: &'a String,
    block_throughput_estimator: &'a Option<ThroughputEstimator>,
    block_analysis_extra_to_throughput: &'a bool,
    block_throughput_cache: &'a BTreeMap<Vec<u64>, (f64, String)>,
}
