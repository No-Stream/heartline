use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use theme_park_sim::facilities::ParkFacilities;
use theme_park_sim::guests::GuestId;
use theme_park_sim::guests::behavior::Facility;
use theme_park_sim::park::{Coaster, CoasterId, DemoCoaster, Park};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{
    BlockOperation, Circuit, ConstructionTestRunRecorder, TestRun, Train,
};
use theme_park_sim::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, ThroughputEstimator,
};
use theme_park_sim::track::clearance::TerrainCell;
use theme_park_sim::track::construction::{Completion, CompletionSearch};
use theme_park_sim::track::shaped::ElementIntent;
use theme_park_sim::track::{Piece, PieceId};
use theme_park_sim::world::{Position, World};

use crate::WasmSimulation;

/// Frozen version 8 postcard layout. Field order is the save format.
#[derive(Deserialize)]
pub(super) struct WasmSimulationV8 {
    pub(super) park: ParkV8,
    pub(super) editing: Option<CoasterId>,
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
pub(super) struct ParkV8 {
    pub(super) world: World,
    pub(super) facilities: ParkFacilities,
    pub(super) simulation: ParkGuests,
    pub(super) demo_coaster: Option<DemoCoaster>,
    pub(super) coasters: BTreeMap<CoasterId, CoasterV8>,
    pub(super) next_coaster_id: u32,
    pub(super) automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwnerV8>>,
    pub(super) manual_tunnel_cells: BTreeSet<TerrainCell>,
    pub(super) physics_remainder: u32,
    pub(super) construction_message: String,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) demo_report: Option<TestRun>,
    pub(super) pending_coaster_price: u16,
}

#[derive(Deserialize)]
pub(super) struct CoasterV8 {
    pub(super) construction: theme_park_sim::track::construction::Construction,
    pub(super) name: String,
    pub(super) construction_train: Option<(Circuit, Train)>,
    pub(super) construction_slow_steps: u32,
    pub(super) construction_outcome: Option<String>,
    pub(super) construction_message: String,
    pub(super) construction_undo_groups: Vec<usize>,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) construction_recorder: Option<ConstructionTestRunRecorder>,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) construction_report: Option<TestRun>,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, serde::Serialize)]
pub(super) struct TunnelOwnerV8 {
    pub(super) coaster_id: CoasterId,
    pub(super) piece_id: usize,
}

impl WasmSimulationV8 {
    pub(super) fn into_current(self) -> WasmSimulation {
        let Self {
            park,
            editing,
            facility_message,
            path_message,
            terrain_message,
            scenery_message,
            terrain_invalidated_paths,
            construction_model,
            selected,
            selected_shape,
            selected_element,
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

        WasmSimulation {
            park: park.into_current(),
            editing,
            facility_message,
            path_message,
            terrain_message,
            scenery_message,
            terrain_invalidated_paths,
            construction_model,
            selected,
            selected_shape,
            selected_element,
            construction_completion,
            construction_completion_available,
            construction_completion_search,
            construction_completion_checked,
            construction_completion_unavailable_reason,
            construction_completion_index,
            construction_preview_supports_cache,
            construction_placed_supports_cache,
            recovered_station,
        }
    }
}

impl ParkV8 {
    fn into_current(self) -> Park {
        let Self {
            world,
            facilities,
            simulation,
            demo_coaster,
            coasters,
            next_coaster_id,
            automatic_tunnel_owners,
            manual_tunnel_cells,
            physics_remainder,
            construction_message,
            demo_report,
            pending_coaster_price,
        } = self;

        let mut park = Park::new_empty(world, facilities, simulation);
        park.demo_coaster = demo_coaster;
        park.coasters = coasters
            .into_iter()
            .map(|(id, coaster)| (id, coaster.into_current()))
            .collect();
        park.next_coaster_id = next_coaster_id;
        park.automatic_tunnel_owners = automatic_tunnel_owners
            .into_iter()
            .map(|(cell, owners)| {
                (
                    cell,
                    owners
                        .into_iter()
                        .map(|owner| theme_park_sim::park::TunnelOwner {
                            coaster_id: owner.coaster_id,
                            piece_id: owner.piece_id,
                        })
                        .collect(),
                )
            })
            .collect();
        park.manual_tunnel_cells = manual_tunnel_cells;
        park.physics_remainder = physics_remainder;
        park.construction_message = construction_message;
        park.demo_report = demo_report;
        park.pending_coaster_price = pending_coaster_price;
        park
    }
}

impl CoasterV8 {
    fn into_current(self) -> Coaster {
        let Self {
            construction,
            name,
            construction_train,
            construction_slow_steps,
            construction_outcome,
            construction_message,
            construction_undo_groups,
            construction_recorder,
            construction_report,
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
        } = self;

        let mut coaster = Coaster::new(construction, coaster_price);
        coaster.name = name;
        coaster.construction_train = construction_train;
        coaster.construction_slow_steps = construction_slow_steps;
        coaster.construction_outcome = construction_outcome;
        coaster.construction_message = construction_message;
        coaster.construction_undo_groups = construction_undo_groups;
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
        coaster
    }
}
