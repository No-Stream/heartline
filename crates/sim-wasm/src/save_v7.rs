use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use theme_park_sim::facilities::ParkFacilities;
use theme_park_sim::guests::GuestId;
use theme_park_sim::guests::behavior::Facility;
use theme_park_sim::park::{Coaster, CoasterId, Park};
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
use crate::save_v8::LegacyDemoCoaster;

/// Frozen version 7 postcard layout. Field order is the save format.
#[derive(Deserialize)]
pub(super) struct WasmSimulationV7 {
    pub(super) park: ParkV7,
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
pub(super) struct ParkV7 {
    pub(super) world: World,
    pub(super) facilities: ParkFacilities,
    pub(super) simulation: ParkGuests,
    pub(super) demo_coaster: Option<LegacyDemoCoaster>,
    pub(super) coasters: BTreeMap<CoasterId, CoasterV7>,
    pub(super) next_coaster_id: u32,
    pub(super) automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwnerV7>>,
    pub(super) manual_tunnel_cells: BTreeSet<TerrainCell>,
    #[allow(dead_code)]
    pub(super) construction_tunnel_generation: u64,
    pub(super) physics_remainder: u32,
    pub(super) construction_message: String,
    #[serde(deserialize_with = "theme_park_sim::park::deserialize_v6_physics_option")]
    pub(super) demo_report: Option<TestRun>,
    pub(super) pending_coaster_price: u16,
}

#[derive(Deserialize)]
pub(super) struct CoasterV7 {
    pub(super) construction: Construction,
    pub(super) construction_train: Option<(Circuit, Train)>,
    pub(super) construction_slow_steps: u32,
    pub(super) construction_outcome: Option<String>,
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
    #[serde(skip)]
    #[allow(dead_code)]
    pub(super) support_render_cache:
        BTreeMap<usize, theme_park_sim::park::tick::SupportRenderCache>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, serde::Serialize)]
pub(super) struct TunnelOwnerV7 {
    pub(super) generation: u64,
    pub(super) piece_id: usize,
}

impl WasmSimulationV7 {
    pub(super) fn into_current(self) -> Result<WasmSimulation, &'static str> {
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
        let ParkV7 {
            world,
            facilities,
            simulation,
            demo_coaster,
            coasters: old_coasters,
            next_coaster_id,
            automatic_tunnel_owners: old_tunnel_owners,
            manual_tunnel_cells,
            construction_tunnel_generation: _,
            physics_remainder,
            construction_message,
            demo_report,
            pending_coaster_price,
        } = park;

        if old_coasters.keys().any(|id| *id != CoasterId(0)) {
            return Err("version 7 save contains a coaster other than id 0");
        }
        if editing.is_some_and(|id| id != CoasterId(0)) {
            return Err("version 7 save edits a coaster other than id 0");
        }

        let mut coasters: BTreeMap<CoasterId, Coaster> = old_coasters
            .into_iter()
            .map(|(id, old_coaster)| (id, old_coaster.into_current()))
            .collect();
        if let Some(coaster) = coasters.get_mut(&CoasterId(0)) {
            // Version 7 used this string for both builder errors and the test train.
            coaster
                .construction_message
                .clone_from(&construction_message);
            coaster.construction_undo_groups = construction_undo_groups;
        } else if editing.is_some() || !construction_undo_groups.is_empty() {
            return Err("version 7 save has editing state without a coaster");
        }

        let mut automatic_tunnel_owners = BTreeMap::new();
        for (cell, owners) in old_tunnel_owners {
            if !owners.is_empty() && !coasters.contains_key(&CoasterId(0)) {
                return Err("version 7 save has tunnel owners without a coaster");
            }
            let owners = owners
                .into_iter()
                .map(|owner| theme_park_sim::park::TunnelOwner {
                    coaster_id: CoasterId(0),
                    piece_id: owner.piece_id,
                })
                .collect();
            automatic_tunnel_owners.insert(cell, owners);
        }

        let mut park = Park::new_empty(world, facilities, simulation);
        park.coasters = coasters;
        park.next_coaster_id = next_coaster_id;
        park.automatic_tunnel_owners = automatic_tunnel_owners;
        park.manual_tunnel_cells = manual_tunnel_cells;
        park.physics_remainder = physics_remainder;
        park.construction_message = construction_message;
        park.pending_coaster_price = pending_coaster_price;

        let simulation = WasmSimulation {
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
        };
        crate::save_v8::migrate_legacy_demo(simulation, demo_coaster, demo_report)
    }
}

impl CoasterV7 {
    fn into_current(self) -> Coaster {
        let name = self.construction.model().name.to_owned();
        let mut coaster = Coaster::new(self.construction, self.coaster_price);
        coaster.name = name;
        coaster.construction_train = self.construction_train;
        coaster.construction_slow_steps = self.construction_slow_steps;
        coaster.construction_outcome = self.construction_outcome;
        coaster.construction_recorder = self.construction_recorder;
        coaster.construction_report = self.construction_report;
        coaster.coaster_open = self.coaster_open;
        coaster.coaster_operation = self.coaster_operation;
        coaster.coaster_riders = self.coaster_riders;
        coaster.coaster_facility = self.coaster_facility;
        coaster.coaster_ratings = self.coaster_ratings;
        coaster.block_layout = self.block_layout;
        coaster.desired_train_count = self.desired_train_count;
        coaster.block_placement_search = self.block_placement_search;
        coaster.block_manual_search = self.block_manual_search;
        coaster.block_apply_best_requested = self.block_apply_best_requested;
        coaster.block_placement_cache = self.block_placement_cache;
        coaster.block_baseline_validation = self.block_baseline_validation;
        coaster.block_layout_is_automatic = self.block_layout_is_automatic;
        coaster.block_message = self.block_message;
        coaster.block_hourly_throughput = self.block_hourly_throughput;
        coaster.block_throughput_reason = self.block_throughput_reason;
        coaster.block_throughput_estimator = self.block_throughput_estimator;
        coaster.block_analysis_extra_to_throughput = self.block_analysis_extra_to_throughput;
        coaster.block_throughput_cache = self.block_throughput_cache;
        coaster
    }
}
