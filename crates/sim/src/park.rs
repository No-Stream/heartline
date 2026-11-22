//! State and operations for a complete park.

pub mod block_analysis;
pub mod coaster;
pub mod commands;
pub mod ride_test;
pub mod tick;

pub use coaster::{Coaster, CoasterId};

#[cfg(test)]
mod hash_tests;
#[cfg(test)]
mod theming_tests;

use crate::facilities::ParkFacilities;
use crate::guests::behavior::Facility;
use crate::guests::park::ParkGuests;
use crate::physics::{Circuit, TestRun, Train};
use crate::state_hash::StateHasher;
use crate::track::clearance::TerrainCell;
use crate::track::construction::Construction;
use crate::track::{CoasterColors, TrainConfiguration, hybrid_model};
use crate::world::World;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

thread_local! {
    static LEGACY_PRE_V6_SAVE_DECODE: Cell<bool> = const { Cell::new(false) };
    static LEGACY_FORCE_LIMITS_SEEN: Cell<bool> = const { Cell::new(false) };
}

/// Runs `decode` while marking V4/V5 saves so any stored pre-V6 test report, recorder, or
/// operating coaster triggers an explicit legacy-format error instead of silent mis-decoding.
pub fn with_legacy_pre_v6_save_decode_context<T>(decode: impl FnOnce() -> T) -> (T, bool) {
    let previous_mode = LEGACY_PRE_V6_SAVE_DECODE.replace(true);
    let previous_seen = LEGACY_FORCE_LIMITS_SEEN.replace(false);
    let result = decode();
    let seen = LEGACY_FORCE_LIMITS_SEEN.replace(previous_seen);
    LEGACY_PRE_V6_SAVE_DECODE.set(previous_mode);
    (result, seen)
}

pub fn deserialize_v6_physics_option<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    if !LEGACY_PRE_V6_SAVE_DECODE.get() {
        return serde::Deserialize::deserialize(deserializer);
    }
    struct LegacyOptionGuard<T>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for LegacyOptionGuard<T> {
        type Value = Option<T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an optional pre-v6 physics record")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            _deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            LEGACY_FORCE_LIMITS_SEEN.set(true);
            Err(serde::de::Error::custom(
                "legacy pre-v6 save contains stored per-model evaluator limits",
            ))
        }
    }
    deserializer.deserialize_option(LegacyOptionGuard(std::marker::PhantomData))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DemoCoaster {
    pub circuit: Circuit,
    pub train: Train,
    pub train_configuration: TrainConfiguration,
    pub colors: CoasterColors,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Park {
    pub world: World,
    pub facilities: ParkFacilities,
    pub simulation: ParkGuests,
    pub demo_coaster: Option<DemoCoaster>,
    pub coasters: BTreeMap<CoasterId, Coaster>,
    pub next_coaster_id: u32,
    pub automatic_tunnel_owners: BTreeMap<TerrainCell, BTreeSet<TunnelOwner>>,
    pub manual_tunnel_cells: BTreeSet<TerrainCell>,
    pub construction_tunnel_generation: u64,
    pub physics_remainder: u32,
    pub construction_message: String,
    /// Latest test of the demo coaster; its track never changes, so it is never invalidated.
    #[serde(deserialize_with = "deserialize_v6_physics_option")]
    pub demo_report: Option<TestRun>,
    pub pending_coaster_price: u16,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TunnelOwner {
    pub generation: u64,
    pub piece_id: usize,
}

impl Park {
    pub fn rebind_world_after_loading(&mut self) {
        self.simulation.rebind_world(&self.world);
    }
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
            coasters: BTreeMap::new(),
            next_coaster_id: 0,
            automatic_tunnel_owners: BTreeMap::new(),
            manual_tunnel_cells: BTreeSet::new(),
            construction_tunnel_generation: 0,
            physics_remainder: 0,
            construction_message: String::new(),
            demo_report: None,
            pending_coaster_price: 6,
        }
    }

    pub fn coaster(&self, id: CoasterId) -> Option<&Coaster> {
        self.coasters.get(&id)
    }

    pub fn coaster_mut(&mut self, id: CoasterId) -> Option<&mut Coaster> {
        self.coasters.get_mut(&id)
    }

    pub fn replace_coaster(&mut self, id: CoasterId, construction: Construction) {
        assert!(
            id == CoasterId(0) && (self.coasters.is_empty() || self.coasters.contains_key(&id)),
            "the first coaster grouping slice only replaces coaster id 0"
        );
        let previous = self.coasters.remove(&id);
        let coaster_price = previous
            .as_ref()
            .map_or(self.pending_coaster_price, |coaster| coaster.coaster_price);
        let coaster_ratings = previous
            .as_ref()
            .and_then(|coaster| coaster.coaster_ratings);
        let construction_slow_steps = previous
            .as_ref()
            .map_or(0, |coaster| coaster.construction_slow_steps);
        let mut coaster = Coaster::new(construction, coaster_price);
        coaster.coaster_ratings = coaster_ratings;
        coaster.construction_slow_steps = construction_slow_steps;
        self.coasters.insert(id, coaster);
        self.pending_coaster_price = coaster_price;
        if id.0 >= self.next_coaster_id {
            self.next_coaster_id = id.0.checked_add(1).expect("coaster id space exhausted");
        }
    }

    pub fn set_coaster_price(&mut self, id: Option<CoasterId>, price: u16) {
        if let Some(id) = id {
            let coaster = self.coaster_mut(id).expect("edited coaster exists");
            coaster.coaster_price = price;
        }
        self.pending_coaster_price = price;
    }

    pub fn coaster_price(&self, id: Option<CoasterId>) -> u16 {
        id.map_or(self.pending_coaster_price, |id| {
            self.coaster(id)
                .expect("edited coaster exists")
                .coaster_price
        })
    }

    pub fn coaster_income(&self, id: CoasterId) -> u64 {
        self.simulation.facility_income(u32::MAX - id.0)
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
        assert!(
            self.coasters.len() <= 1 && self.coasters.keys().all(|id| *id == CoasterId(0)),
            "the first coaster grouping slice supports at most coaster id 0"
        );
        let coaster = self.coaster(CoasterId(0));
        hash.write_option(
            coaster.map(|coaster| &coaster.construction),
            |hash, construction| {
                hash.write_u64(construction.state_hash());
            },
        );
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
            coaster.and_then(|coaster| coaster.construction_train.as_ref()),
            |hash, (circuit, train)| {
                circuit.hash_into(hash);
                train.hash_into(hash);
            },
        );
        hash.write_bytes(self.construction_message.as_bytes());
        hash.write_u32(coaster.map_or(0, |coaster| coaster.construction_slow_steps));
        hash.write_option(
            coaster.and_then(|coaster| coaster.construction_outcome.as_ref()),
            |hash, outcome| {
                hash.write_bytes(outcome.as_bytes());
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.construction_recorder.as_ref()),
            |hash, recorder| {
                recorder.hash_into(hash);
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.construction_report.as_ref()),
            |hash, report| {
                report.hash_into(hash);
            },
        );
        // Only written when present so parks without a demo report keep their earlier hashes.
        if let Some(report) = &self.demo_report {
            hash.write_tag(b"DemoReport");
            report.hash_into(&mut hash);
        }
        hash.write_bool(coaster.is_some_and(|coaster| coaster.coaster_open));
        hash.write_u16(coaster.map_or(self.pending_coaster_price, |coaster| coaster.coaster_price));
        hash.write_option(
            coaster.and_then(|coaster| coaster.coaster_operation.as_ref()),
            |hash, operation| {
                operation.hash_into(hash);
            },
        );
        hash.write_len(coaster.map_or(0, |coaster| coaster.coaster_riders.len()));
        if let Some(coaster) = coaster {
            for (&train_id, guests) in &coaster.coaster_riders {
                hash.write_u64(train_id);
                hash.write_len(guests.len());
                for guest in guests {
                    hash.write_u32(guest.0);
                }
            }
        }
        hash.write_option(
            coaster.and_then(|coaster| coaster.coaster_facility),
            |hash, facility| {
                write_facility(hash, facility);
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.coaster_ratings),
            |hash, (intensity, nausea)| {
                hash.write_u8(intensity);
                hash.write_u8(nausea);
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.block_layout.as_ref()),
            |hash, layout| {
                layout.hash_into(hash);
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.desired_train_count),
            |hash, count| hash.write_len(count),
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.block_placement_search.as_ref()),
            |hash, search| {
                search.hash_into(hash);
            },
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.block_manual_search.as_ref()),
            |hash, search| {
                search.hash_into(hash);
            },
        );
        hash.write_bool(coaster.is_some_and(|coaster| coaster.block_apply_best_requested));
        hash.write_option(
            coaster.and_then(|coaster| coaster.block_placement_cache.as_ref()),
            |hash, search| search.hash_into(hash),
        );
        hash.write_bool(coaster.is_some_and(|coaster| coaster.block_baseline_validation));
        hash.write_bool(coaster.is_some_and(|coaster| coaster.block_layout_is_automatic));
        hash.write_bytes(
            coaster
                .map_or("", |coaster| coaster.block_message.as_str())
                .as_bytes(),
        );
        hash.write_f64(coaster.map_or(0.0, |coaster| coaster.block_hourly_throughput));
        hash.write_bytes(
            coaster
                .map_or("", |coaster| coaster.block_throughput_reason.as_str())
                .as_bytes(),
        );
        hash.write_option(
            coaster.and_then(|coaster| coaster.block_throughput_estimator.as_ref()),
            |hash, estimator| estimator.hash_into(hash),
        );
        hash.write_bool(coaster.is_some_and(|coaster| coaster.block_analysis_extra_to_throughput));
        hash.write_len(coaster.map_or(0, |coaster| coaster.block_throughput_cache.len()));
        if let Some(coaster) = coaster {
            for (fingerprint, (throughput, reason)) in &coaster.block_throughput_cache {
                hash.write_len(fingerprint.len());
                for &value in fingerprint {
                    hash.write_u64(value);
                }
                hash.write_f64(*throughput);
                hash.write_bytes(reason.as_bytes());
            }
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
