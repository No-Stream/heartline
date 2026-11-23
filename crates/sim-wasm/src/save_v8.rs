use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use theme_park_sim::facilities::ParkFacilities;
use theme_park_sim::guests::GuestId;
use theme_park_sim::guests::behavior::Facility;
#[cfg(test)]
use theme_park_sim::park::TunnelOwner;
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
use theme_park_sim::track::ride_file::RideFile;
use theme_park_sim::track::shaped::ElementIntent;
use theme_park_sim::track::{CoasterColors, Piece, PieceId, TrainConfiguration};
use theme_park_sim::world::{Position, World};

use crate::WasmSimulation;
use crate::builder_session::BuilderSession;

const MIGRATED_DEMO_COASTER_PRICE: u16 = 6;

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
    pub(super) demo_coaster: Option<LegacyDemoCoaster>,
    pub(super) coasters: BTreeMap<CoasterId, CoasterV8>,
    pub(super) next_coaster_id: u32,
    pub(super) automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwnerV8>>,
    pub(super) manual_tunnel_cells: BTreeSet<TerrainCell>,
    pub(super) physics_remainder: u32,
    pub(super) construction_message: String,
    pub(super) demo_report: Option<TestRun>,
    pub(super) pending_coaster_price: u16,
}

#[derive(Deserialize, Serialize)]
pub(super) struct LegacyDemoCoaster {
    #[allow(dead_code)]
    pub(super) circuit: Circuit,
    #[allow(dead_code)]
    pub(super) train: Train,
    #[allow(dead_code)]
    pub(super) train_configuration: TrainConfiguration,
    #[allow(dead_code)]
    pub(super) colors: CoasterColors,
}

#[derive(Deserialize)]
pub(super) struct CoasterV8 {
    pub(super) construction: Construction,
    pub(super) name: String,
    pub(super) construction_train: Option<(Circuit, Train)>,
    pub(super) construction_slow_steps: u32,
    pub(super) construction_outcome: Option<String>,
    pub(super) construction_message: String,
    pub(super) construction_undo_groups: Vec<usize>,
    pub(super) construction_recorder: Option<ConstructionTestRunRecorder>,
    pub(super) construction_report: Option<TestRun>,
    pub(super) coaster_open: bool,
    pub(super) coaster_price: u16,
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

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TunnelOwnerV8 {
    coaster_id: CoasterId,
    piece_id: usize,
}

impl WasmSimulationV8 {
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
        let ParkV8 {
            world,
            facilities,
            simulation,
            demo_coaster,
            coasters: old_coasters,
            next_coaster_id,
            automatic_tunnel_owners: old_tunnel_owners,
            manual_tunnel_cells,
            physics_remainder,
            construction_message,
            demo_report,
            pending_coaster_price,
        } = park;

        if editing.is_some_and(|id| !old_coasters.contains_key(&id)) {
            return Err("version 8 save edits a coaster that does not exist");
        }
        if old_coasters.keys().any(|id| id.0 >= next_coaster_id) {
            return Err("version 8 save has an invalid next coaster id");
        }
        if demo_coaster.is_none() && demo_report.is_some() {
            return Err("version 8 save has a demo report without a demo coaster");
        }

        let coasters: BTreeMap<CoasterId, Coaster> = old_coasters
            .into_iter()
            .map(|(id, old_coaster)| (id, old_coaster.into_current()))
            .collect();
        let automatic_tunnel_owners = old_tunnel_owners
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
        migrate_legacy_demo(simulation, demo_coaster, demo_report)
    }
}

impl CoasterV8 {
    fn into_current(self) -> Coaster {
        let mut coaster = Coaster::new(self.construction, self.coaster_price);
        coaster.name = self.name;
        coaster.construction_train = self.construction_train;
        coaster.construction_slow_steps = self.construction_slow_steps;
        coaster.construction_outcome = self.construction_outcome;
        coaster.construction_message = self.construction_message;
        coaster.construction_undo_groups = self.construction_undo_groups;
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

pub(super) fn migrate_legacy_demo(
    mut simulation: WasmSimulation,
    legacy_demo: Option<LegacyDemoCoaster>,
    demo_report: Option<TestRun>,
) -> Result<WasmSimulation, &'static str> {
    if legacy_demo.is_none() {
        if demo_report.is_some() {
            return Err("legacy save has a demo report without a demo coaster");
        }
        return Ok(simulation);
    }

    let editing = simulation.editing;
    let construction_model = std::mem::take(&mut simulation.construction_model);
    let selected = simulation.selected.take();
    let selected_shape = simulation.selected_shape.take();
    let selected_element = simulation.selected_element.take();
    let recovered_station = simulation.recovered_station.take();
    let construction_message = std::mem::take(&mut simulation.park.construction_message);

    simulation.coaster_stop_editing();
    let ride = RideFile::from_json_str(include_str!("../../../content/rides/demo.ride.json"))
        .map_err(|_| "could not parse the built-in demo ride during save migration")?;
    let mut session = BuilderSession::new(simulation);
    session
        .replay_ride(&ride)
        .map_err(|_| "could not rebuild the built-in demo ride during save migration")?;
    let mut simulation = session.into_simulation();
    let migrated_demo_id = simulation
        .editing_coaster_id()
        .ok_or("rebuilt demo ride did not create a coaster")?;
    let migrated_demo = simulation
        .park
        .coaster_mut(migrated_demo_id)
        .expect("rebuilt demo coaster exists");
    migrated_demo.coaster_price = MIGRATED_DEMO_COASTER_PRICE;
    if let Some(report) = demo_report {
        migrated_demo.construction_report = Some(report);
    }

    simulation.editing = editing;
    simulation.construction_model = construction_model;
    simulation.selected = selected;
    simulation.selected_shape = selected_shape;
    simulation.selected_element = selected_element;
    simulation.recovered_station = recovered_station;
    simulation.park.construction_message = construction_message;
    simulation.construction_preview_supports_cache.clear();
    simulation.construction_placed_supports_cache.clear();
    Ok(simulation)
}

#[cfg(test)]
pub(super) fn serialize_v8_for_test(
    simulation: &WasmSimulation,
    demo_coaster: &Option<LegacyDemoCoaster>,
    demo_report: &Option<TestRun>,
) -> Vec<u8> {
    let park = &simulation.park;
    let payload = WasmSimulationV8Ref {
        park: ParkV8Ref {
            world: &park.world,
            facilities: &park.facilities,
            simulation: &park.simulation,
            demo_coaster,
            coasters: &park.coasters,
            next_coaster_id: &park.next_coaster_id,
            automatic_tunnel_owners: &park.automatic_tunnel_owners,
            manual_tunnel_cells: &park.manual_tunnel_cells,
            physics_remainder: &park.physics_remainder,
            construction_message: &park.construction_message,
            demo_report,
            pending_coaster_price: &park.pending_coaster_price,
        },
        editing: &simulation.editing,
        facility_message: &simulation.facility_message,
        path_message: &simulation.path_message,
        terrain_message: &simulation.terrain_message,
        scenery_message: &simulation.scenery_message,
        terrain_invalidated_paths: &simulation.terrain_invalidated_paths,
        construction_model: &simulation.construction_model,
        selected: &simulation.selected,
        selected_shape: &simulation.selected_shape,
        selected_element: &simulation.selected_element,
        construction_completion: &simulation.construction_completion,
        construction_completion_available: &simulation.construction_completion_available,
        construction_completion_search: &simulation.construction_completion_search,
        construction_completion_checked: &simulation.construction_completion_checked,
        construction_completion_unavailable_reason: &simulation
            .construction_completion_unavailable_reason,
        recovered_station: &simulation.recovered_station,
    };
    postcard::to_allocvec(&payload).expect("version 8 test save serializes")
}

#[cfg(test)]
#[derive(Serialize)]
struct WasmSimulationV8Ref<'a> {
    park: ParkV8Ref<'a>,
    editing: &'a Option<CoasterId>,
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
    construction_completion: &'a Option<Completion>,
    construction_completion_available: &'a Vec<Completion>,
    construction_completion_search: &'a Option<CompletionSearch>,
    construction_completion_checked: &'a bool,
    construction_completion_unavailable_reason: &'a Option<String>,
    recovered_station: &'a Option<Piece>,
}

#[cfg(test)]
#[derive(Serialize)]
struct ParkV8Ref<'a> {
    world: &'a World,
    facilities: &'a ParkFacilities,
    simulation: &'a ParkGuests,
    demo_coaster: &'a Option<LegacyDemoCoaster>,
    coasters: &'a BTreeMap<CoasterId, Coaster>,
    next_coaster_id: &'a u32,
    automatic_tunnel_owners: &'a BTreeMap<TerrainCell, BTreeSet<TunnelOwner>>,
    manual_tunnel_cells: &'a BTreeSet<TerrainCell>,
    physics_remainder: &'a u32,
    construction_message: &'a String,
    demo_report: &'a Option<TestRun>,
    pending_coaster_price: &'a u16,
}
