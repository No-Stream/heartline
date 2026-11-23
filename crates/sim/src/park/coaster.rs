use std::collections::BTreeMap;

use crate::guests::GuestId;
use crate::guests::behavior::Facility;
use crate::physics::{BlockOperation, ConstructionTestRunRecorder, TestRun, Train};
use crate::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, ThroughputEstimator,
};
use crate::track::construction::Construction;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct CoasterId(pub u32);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Coaster {
    pub construction: Construction,
    pub name: String,
    pub construction_train: Option<(crate::physics::Circuit, Train)>,
    pub construction_slow_steps: u32,
    pub construction_outcome: Option<String>,
    pub construction_message: String,
    pub construction_undo_groups: Vec<usize>,
    #[serde(deserialize_with = "super::deserialize_v6_physics_option")]
    pub construction_recorder: Option<ConstructionTestRunRecorder>,
    #[serde(deserialize_with = "super::deserialize_v6_physics_option")]
    pub construction_report: Option<TestRun>,
    pub coaster_open: bool,
    pub coaster_price: u16,
    #[serde(deserialize_with = "super::deserialize_v6_physics_option")]
    pub coaster_operation: Option<BlockOperation>,
    pub coaster_riders: BTreeMap<u64, Vec<GuestId>>,
    pub coaster_facility: Option<Facility>,
    pub coaster_ratings: Option<(u8, u8)>,
    pub block_layout: Option<BlockLayout>,
    pub desired_train_count: Option<usize>,
    pub block_placement_search: Option<BlockPlacementSearch>,
    pub block_manual_search: Option<BlockManualSearch>,
    pub block_apply_best_requested: bool,
    pub block_placement_cache: Option<BlockPlacementSearch>,
    pub block_baseline_validation: bool,
    pub block_layout_is_automatic: bool,
    pub block_message: String,
    pub block_hourly_throughput: f64,
    pub block_throughput_reason: String,
    pub block_throughput_estimator: Option<ThroughputEstimator>,
    pub block_analysis_extra_to_throughput: bool,
    pub block_throughput_cache: BTreeMap<Vec<u64>, (f64, String)>,
    #[serde(skip)]
    pub(crate) support_render_cache: BTreeMap<usize, super::tick::SupportRenderCache>,
}

impl Coaster {
    pub fn new(construction: Construction, coaster_price: u16) -> Self {
        let name = construction.model().name.to_owned();
        Self {
            construction,
            name,
            construction_train: None,
            construction_slow_steps: 0,
            construction_outcome: None,
            construction_message: String::new(),
            construction_undo_groups: Vec::new(),
            construction_recorder: None,
            construction_report: None,
            coaster_open: false,
            coaster_price,
            coaster_operation: None,
            coaster_riders: BTreeMap::new(),
            coaster_facility: None,
            coaster_ratings: None,
            block_layout: None,
            desired_train_count: None,
            block_placement_search: None,
            block_manual_search: None,
            block_apply_best_requested: false,
            block_placement_cache: None,
            block_baseline_validation: false,
            block_layout_is_automatic: false,
            block_message: String::new(),
            block_hourly_throughput: 0.0,
            block_throughput_reason: String::new(),
            block_throughput_estimator: None,
            block_analysis_extra_to_throughput: false,
            block_throughput_cache: BTreeMap::new(),
            support_render_cache: BTreeMap::new(),
        }
    }
}
