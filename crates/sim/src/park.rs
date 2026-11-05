//! State and operations for a complete park.

pub mod block_analysis;
pub mod commands;
pub mod tick;

#[cfg(test)]
mod hash_tests;
#[cfg(test)]
mod theming_tests;

use crate::facilities::ParkFacilities;
use crate::guests::GuestId;
use crate::guests::behavior::Facility;
use crate::guests::park::ParkGuests;
use crate::physics::{BlockOperation, Circuit, ConstructionTestRunRecorder, TestRun, Train};
use crate::state_hash::StateHasher;
use crate::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, ThroughputEstimator,
};
use crate::track::clearance::TerrainCell;
use crate::track::construction::Construction;
use crate::track::{CoasterColors, TrainConfiguration, hybrid_model};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

pub struct DemoCoaster {
    pub circuit: Circuit,
    pub train: Train,
    pub train_configuration: TrainConfiguration,
    pub colors: CoasterColors,
}

pub struct Park {
    pub world: World,
    pub facilities: ParkFacilities,
    pub simulation: ParkGuests,
    pub demo_coaster: Option<DemoCoaster>,
    pub construction: Option<Construction>,
    pub(crate) automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwner>>,
    pub(crate) manual_tunnel_cells: BTreeSet<TerrainCell>,
    pub(crate) construction_tunnel_generation: u64,
    pub physics_remainder: u32,
    pub construction_train: Option<(Circuit, Train)>,
    pub construction_message: String,
    pub construction_slow_steps: u32,
    pub construction_outcome: Option<String>,
    pub construction_recorder: Option<ConstructionTestRunRecorder>,
    pub construction_report: Option<TestRun>,
    pub coaster_open: bool,
    pub coaster_price: u16,
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
    pub support_render_cache: BTreeMap<usize, tick::SupportRenderCache>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TunnelOwner {
    generation: u64,
    piece_id: usize,
}

impl Park {
    pub fn new(
        world: World,
        facilities: ParkFacilities,
        simulation: ParkGuests,
        circuit: Circuit,
        train: Train,
        train_configuration: TrainConfiguration,
    ) -> Self {
        let mut park = Self::new_empty(world, facilities, simulation);
        park.demo_coaster = Some(DemoCoaster {
            circuit,
            train,
            train_configuration,
            // The demo coaster is a steel-box track on a wooden structure, so it wears the Timber Tumbler's colours.
            colors: hybrid_model().default_colors,
        });
        park
    }

    pub fn new_empty(world: World, facilities: ParkFacilities, simulation: ParkGuests) -> Self {
        Self {
            world,
            facilities,
            simulation,
            demo_coaster: None,
            construction: None,
            automatic_tunnel_owners: BTreeMap::new(),
            manual_tunnel_cells: BTreeSet::new(),
            construction_tunnel_generation: 0,
            physics_remainder: 0,
            construction_train: None,
            construction_message: String::new(),
            construction_slow_steps: 0,
            construction_outcome: None,
            construction_recorder: None,
            construction_report: None,
            coaster_open: false,
            coaster_price: 6,
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

    pub fn set_coaster_price(&mut self, price: u16) {
        self.coaster_price = price;
    }

    pub fn coaster_income(&self) -> u64 {
        self.simulation.facility_income(tick::COASTER_PLACE_ID)
    }

    /// Stable fingerprint of all live park state, excluding rebuildable render caches.
    pub fn state_hash(&self) -> u64 {
        let mut hash = StateHasher::new();
        hash.write_tag(b"Park-v2");
        hash.write_u64(self.world.state_hash());
        hash.write_u64(self.facilities.state_hash());
        hash.write_u64(self.simulation.state_hash());
        hash.write_option(self.demo_coaster.as_ref(), |hash, coaster| {
            coaster.circuit.hash_into(hash);
            coaster.train.hash_into(hash);
            coaster.train_configuration.hash_into(hash);
            coaster.colors.hash_into(hash);
        });
        hash.write_option(self.construction.as_ref(), |hash, construction| {
            hash.write_u64(construction.state_hash());
        });
        hash.write_u64(self.construction_tunnel_generation);
        hash.write_len(self.automatic_tunnel_owners.len());
        for (cell, owners) in &self.automatic_tunnel_owners {
            hash.write_u16(cell.x);
            hash.write_u16(cell.y);
            hash.write_i16(cell.height);
            hash.write_len(owners.len());
            for owner in owners {
                hash.write_u64(owner.generation);
                hash.write_len(owner.piece_id);
            }
        }
        hash.write_len(self.manual_tunnel_cells.len());
        for cell in &self.manual_tunnel_cells {
            hash.write_u16(cell.x);
            hash.write_u16(cell.y);
            hash.write_i16(cell.height);
        }
        hash.write_u32(self.physics_remainder);
        hash.write_option(
            self.construction_train.as_ref(),
            |hash, (circuit, train)| {
                circuit.hash_into(hash);
                train.hash_into(hash);
            },
        );
        hash.write_bytes(self.construction_message.as_bytes());
        hash.write_u32(self.construction_slow_steps);
        hash.write_option(self.construction_outcome.as_ref(), |hash, outcome| {
            hash.write_bytes(outcome.as_bytes());
        });
        hash.write_option(self.construction_recorder.as_ref(), |hash, recorder| {
            recorder.hash_into(hash);
        });
        hash.write_option(self.construction_report.as_ref(), |hash, report| {
            report.hash_into(hash);
        });
        hash.write_bool(self.coaster_open);
        hash.write_u16(self.coaster_price);
        hash.write_option(self.coaster_operation.as_ref(), |hash, operation| {
            operation.hash_into(hash);
        });
        hash.write_len(self.coaster_riders.len());
        for (&train_id, guests) in &self.coaster_riders {
            hash.write_u64(train_id);
            hash.write_len(guests.len());
            for guest in guests {
                hash.write_u32(guest.0);
            }
        }
        hash.write_option(self.coaster_facility, |hash, facility| {
            write_facility(hash, facility);
        });
        hash.write_option(self.coaster_ratings, |hash, (intensity, nausea)| {
            hash.write_u8(intensity);
            hash.write_u8(nausea);
        });
        hash.write_option(self.block_layout.as_ref(), |hash, layout| {
            layout.hash_into(hash);
        });
        hash.write_option(self.desired_train_count, |hash, count| {
            hash.write_len(count)
        });
        hash.write_option(self.block_placement_search.as_ref(), |hash, search| {
            search.hash_into(hash);
        });
        hash.write_option(self.block_manual_search.as_ref(), |hash, search| {
            search.hash_into(hash);
        });
        hash.write_bool(self.block_apply_best_requested);
        hash.write_option(self.block_placement_cache.as_ref(), |hash, search| {
            search.hash_into(hash)
        });
        hash.write_bool(self.block_baseline_validation);
        hash.write_bool(self.block_layout_is_automatic);
        hash.write_bytes(self.block_message.as_bytes());
        hash.write_f64(self.block_hourly_throughput);
        hash.write_bytes(self.block_throughput_reason.as_bytes());
        hash.write_option(
            self.block_throughput_estimator.as_ref(),
            |hash, estimator| estimator.hash_into(hash),
        );
        hash.write_bool(self.block_analysis_extra_to_throughput);
        hash.write_len(self.block_throughput_cache.len());
        for (fingerprint, (throughput, reason)) in &self.block_throughput_cache {
            hash.write_len(fingerprint.len());
            for &value in fingerprint {
                hash.write_u64(value);
            }
            hash.write_f64(*throughput);
            hash.write_bytes(reason.as_bytes());
        }
        // Support geometry is derived from the world and track, and only avoids recomputation.
        hash.finish()
    }
}

fn write_facility(hash: &mut StateHasher, facility: Facility) {
    hash.write_u32(facility.place.id);
    hash.write_u16(facility.place.position.x);
    hash.write_u16(facility.place.position.y);
    hash.write_i16(facility.place.position.height);
    hash.write_u8(facility.place.kind as u8);
    hash.write_u8(facility.place.minimum_height_cm);
    hash.write_u8(facility.place.intensity);
    hash.write_u8(facility.place.nausea);
    hash.write_u8(facility.place.ride_type);
    hash.write_u8(facility.place.theme);
    hash.write_u16(facility.place.price);
    hash.write_len(facility.capacity);
    hash.write_u32(facility.duration_ticks);
    hash.write_option(facility.exit_position, |hash, position| {
        hash.write_u16(position.x);
        hash.write_u16(position.y);
        hash.write_i16(position.height);
    });
}
