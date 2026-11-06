//! Coordinates guest arrivals, decisions, activities and walking.

use crate::executor::{Executor, SequentialExecutor};
use crate::guests::GuestId;
use crate::guests::behavior::{BehaviorSimulation, Facility, Movement};
use crate::guests::pathfinding::Graph;
use crate::guests::visits::VisitManager;
use crate::guests::walking::{WalkControl, WalkingGuest, WalkingSimulation};
use crate::state_hash::StateHasher;
use crate::world::{Position, World};
use std::collections::BTreeMap;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct VisitChanges {
    pub arrived: Vec<GuestId>,
    pub departed: Vec<GuestId>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ParkGuests {
    pub(super) walking: WalkingSimulation,
    pub(super) behaviors: BehaviorSimulation,
    visits: VisitManager,
    entrance: Option<Position>,
    reachable: Graph,
    world_revision: u64,
    leaving: Vec<bool>,
    entrance_price: u16,
}

impl ParkGuests {
    pub(crate) fn rebind_world(&mut self, world: &World) {
        self.walking.rebind_world(world);
    }
    pub fn new(
        seed: u64,
        roster_size: usize,
        world: &World,
        entrance: Position,
        arrivals_per_tick: usize,
    ) -> Self {
        let mut walking = WalkingSimulation::with_entrance(seed, roster_size, world, entrance);
        let ids: Vec<_> = walking.traits().ids().collect();
        for id in ids {
            walking.traits_mut().mark_left(id);
        }
        let visits = VisitManager::new(walking.traits(), arrivals_per_tick);
        Self {
            walking,
            behaviors: BehaviorSimulation::new(roster_size),
            visits,
            entrance: Some(entrance),
            reachable: Graph::with_entrance(world, Some(entrance)),
            world_revision: world.revision(),
            leaving: vec![false; roster_size],
            entrance_price: 0,
        }
    }

    pub fn set_entrance_price(&mut self, price: u16) {
        self.entrance_price = price;
    }

    pub fn set_entrance(&mut self, world: &World, entrance: Option<Position>) {
        if self.entrance != entrance {
            self.entrance = entrance;
            self.reachable = Graph::with_entrance(world, entrance);
            self.world_revision = world.revision();
            self.walking.set_entrance(world, entrance);
        }
    }

    pub fn revenue(&self) -> u64 {
        self.behaviors
            .guests
            .iter()
            .map(|guest| u64::from(guest.lifetime_money_spent))
            .sum()
    }

    pub fn facility_income(&self, place_id: u32) -> u64 {
        self.behaviors.facility_income(place_id)
    }

    pub fn walking(&self) -> &WalkingSimulation {
        &self.walking
    }

    /// Stable fingerprint of the live guest simulation, excluding world revision counters.
    pub fn state_hash(&self) -> u64 {
        let mut hash = StateHasher::new();
        hash.write_tag(b"ParkGuests-v1");
        self.walking.hash_into(&mut hash);
        self.behaviors.hash_into(&mut hash);
        self.visits.hash_into(&mut hash);
        hash.write_option(self.entrance, |hash, position| {
            hash.write_u16(position.x);
            hash.write_u16(position.y);
            hash.write_i16(position.height);
        });
        self.reachable.hash_into(&mut hash);
        hash.write_len(self.leaving.len());
        for &leaving in &self.leaving {
            hash.write_bool(leaving);
        }
        hash.write_u16(self.entrance_price);
        hash.finish()
    }
    pub fn behaviors(&self) -> &BehaviorSimulation {
        &self.behaviors
    }
    pub fn set_external_rides(&mut self, ids: &[u32]) {
        self.behaviors.set_external_rides(ids);
    }

    pub fn board_coaster(&mut self, facility: &Facility) -> Vec<GuestId> {
        self.board_coaster_train(facility, 0)
    }

    pub fn board_coaster_train(&mut self, facility: &Facility, train_id: u64) -> Vec<GuestId> {
        self.behaviors
            .board_coaster_train(facility, train_id, self.walking.traits())
    }

    pub fn finish_coaster(
        &mut self,
        world: &World,
        facility: &Facility,
        ratings: &[(f64, f64, f64)],
        proximity_events: &[Vec<crate::track::proximity::ProximityEvent>],
        recovered: bool,
    ) {
        self.finish_coaster_train(world, facility, 0, ratings, proximity_events, recovered);
    }

    pub fn finish_coaster_train(
        &mut self,
        world: &World,
        facility: &Facility,
        train_id: u64,
        ratings: &[(f64, f64, f64)],
        proximity_events: &[Vec<crate::track::proximity::ProximityEvent>],
        recovered: bool,
    ) {
        let riders = self.behaviors.finish_coaster_train(
            facility,
            train_id,
            self.walking.traits_mut(),
            ratings,
            proximity_events,
            recovered,
        );
        self.walking.refresh_graph(world);
        let reference = facility.exit_position.unwrap_or(facility.place.position);
        let exit = world
            .paths()
            .filter(|path| {
                world.path_metadata(path.position).unwrap().kind == crate::world::PathKind::Normal
            })
            .min_by_key(|path| {
                let p = path.position;
                (
                    p.x.abs_diff(reference.x) as u32
                        + p.y.abs_diff(reference.y) as u32
                        + p.height.abs_diff(reference.height) as u32,
                    p,
                )
            })
            .map(|path| path.position);
        if let Some(exit) = exit {
            for rider in riders {
                self.walking.place_on_path(rider, exit);
            }
        }
    }

    pub fn close_coaster(&mut self, world: &World, facility: &Facility) {
        let guests = self.behaviors.close_coaster(facility.place.id);
        self.walking.refresh_graph(world);
        let reference = facility.exit_position.unwrap_or(facility.place.position);
        let exit = world
            .paths()
            .filter(|path| {
                world.path_metadata(path.position).unwrap().kind == crate::world::PathKind::Normal
            })
            .min_by_key(|path| {
                let p = path.position;
                (
                    p.x.abs_diff(reference.x) as u32
                        + p.y.abs_diff(reference.y) as u32
                        + p.height.abs_diff(reference.height) as u32,
                    p,
                )
            })
            .map(|path| path.position);
        if let Some(exit) = exit {
            for guest in guests {
                self.walking.place_on_path(guest, exit);
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.visits.active_count()
    }

    /// Starts the roster on seeded paths as if the park had already been open.
    pub fn open_with_guests(&mut self, facilities: &[Facility]) {
        assert_eq!(self.visits.active_count(), 0, "park is already open");
        let path_positions: Vec<_> = self.walking.path_positions().collect();
        assert!(!path_positions.is_empty(), "open park needs guest paths");
        let facility_positions: Vec<_> = facilities
            .iter()
            .map(|facility| facility.place.position)
            .filter(|position| {
                self.reachable
                    .indices
                    .get(position)
                    .is_some_and(|&node| self.reachable.normal[node])
            })
            .collect();
        let all_positions: Vec<_> = facility_positions
            .iter()
            .copied()
            .chain(path_positions.iter().copied())
            .collect();
        let groups = self.walking.traits().groups.clone();
        for (group_index, group) in groups.iter().enumerate() {
            let position = all_positions[group_index % all_positions.len()];
            for &id in &group.members {
                self.walking.traits_mut().mark_returned(id);
                self.walking.place_on_path(id, position);
                self.behaviors.guests[id.0 as usize].reset_visit();
                for facility in facilities {
                    self.behaviors.guests[id.0 as usize].observe(facility.place);
                }
                self.behaviors.guests[id.0 as usize].advance((group_index % 20 * 30) as u32);
            }
        }
        self.visits = VisitManager::new(self.walking.traits(), self.visits.arrivals_per_tick());
    }

    /// Advance one 1/30-second simulation tick.
    pub fn tick(
        &mut self,
        world: &World,
        facilities: &[Facility],
        appeal: u8,
        population_override: Option<usize>,
    ) -> VisitChanges {
        self.tick_with(
            world,
            facilities,
            appeal,
            population_override,
            &SequentialExecutor::new(256),
        )
    }

    pub fn tick_with<E: Executor<WalkingGuest>>(
        &mut self,
        world: &World,
        facilities: &[Facility],
        appeal: u8,
        population_override: Option<usize>,
        executor: &E,
    ) -> VisitChanges {
        if self.world_revision != world.revision() {
            self.reachable = Graph::with_entrance(world, self.entrance);
            self.world_revision = world.revision();
        }
        self.walking.refresh_graph(world);
        let facilities = accessible_facilities(&self.reachable, facilities);
        let component = |position: Position| {
            self.reachable
                .indices
                .get(&position)
                .map(|&node| self.reachable.components[node])
        };
        let positions: Vec<_> = self
            .walking
            .guests()
            .iter()
            .map(WalkingGuest::settled_path)
            .collect();
        let guest_components: Vec<_> = self
            .walking
            .guests()
            .iter()
            .map(|guest| guest.path.and_then(component))
            .collect();
        let facility_components: Vec<_> = facilities
            .iter()
            .map(|facility| component(facility.place.position))
            .collect();
        let queue_entrances: Vec<_> = facilities
            .iter()
            .flat_map(|facility| {
                let node = self.reachable.indices[&facility.place.position];
                self.reachable.queue_components[node]
                    .into_iter()
                    .flat_map(|queue| {
                        self.reachable.queue_entrances[queue]
                            .iter()
                            .map(|&entrance| {
                                (
                                    facility.place,
                                    self.reachable.paths[entrance].position,
                                    self.reachable.components[entrance],
                                )
                            })
                    })
            })
            .collect();
        let discovery_positions: Vec<_> = self
            .walking
            .guests()
            .iter()
            .map(|guest| guest.path)
            .collect();
        self.behaviors.observe_queue_entrances(
            self.walking.traits(),
            &discovery_positions,
            &guest_components,
            &queue_entrances,
            &self.leaving,
        );
        self.behaviors
            .observe_scenery(world, self.walking.traits(), &positions, &self.leaving);
        let movements = self.behaviors.tick_with_exiting(
            self.walking.traits_mut(),
            &positions,
            &facilities,
            &guest_components,
            &facility_components,
            &self.leaving,
        );
        for group in &self.walking.traits().groups {
            if group
                .members
                .iter()
                .any(|id| self.behaviors.guests[id.0 as usize].visit_complete())
                && !group
                    .members
                    .iter()
                    .any(|&id| self.behaviors.is_coaster_riding(id))
                && self.walking.traits().is_active(group.members[0])
            {
                for id in &group.members {
                    self.leaving[id.0 as usize] = true;
                }
            }
        }
        let ready: Vec<_> = positions
            .iter()
            .enumerate()
            .map(|(index, position)| {
                self.leaving[index] && *position == self.entrance && self.entrance.is_some()
            })
            .collect();
        let departed =
            self.visits
                .depart_ready(self.walking.traits_mut(), &self.behaviors.guests, &ready);
        for id in &departed {
            self.leaving[id.0 as usize] = false;
        }
        let arrived = if departed.is_empty() && self.entrance.and_then(component).is_some() {
            self.visits.arrive(
                self.walking.traits_mut(),
                &mut self.behaviors.guests,
                appeal,
                population_override,
                self.entrance_price,
            )
        } else {
            Vec::new()
        };
        if !arrived.is_empty() {
            self.walking.admit(&arrived);
        }
        let controls: Vec<_> = movements
            .into_iter()
            .enumerate()
            .map(|(index, movement)| {
                if !self.walking.traits().is_active(GuestId(index as u32)) {
                    return WalkControl::Stop;
                }
                if self.leaving[index] {
                    return self
                        .entrance
                        .map_or(WalkControl::Stop, WalkControl::Destination);
                }
                match movement {
                    Movement::Wander => WalkControl::Wander,
                    Movement::Destination(position) => WalkControl::Destination(position),
                    Movement::Stopped => WalkControl::Stop,
                }
            })
            .collect();
        self.walking.tick_with_controls(world, executor, &controls);
        VisitChanges { arrived, departed }
    }
}

fn accessible_facilities(graph: &Graph, facilities: &[Facility]) -> Vec<Facility> {
    let mut owners = BTreeMap::<usize, usize>::new();
    for facility in facilities {
        if let Some(&node) = graph.indices.get(&facility.place.position) {
            if let Some(component) = graph.queue_components[node] {
                *owners.entry(component).or_default() += 1;
            }
        }
    }
    facilities
        .iter()
        .copied()
        .filter(|facility| {
            let Some(&node) = graph.indices.get(&facility.place.position) else {
                return false;
            };
            match graph.queue_components[node] {
                None => true,
                Some(component) => {
                    graph.queue_has_entrance[component]
                        && owners[&component] == 1
                        && graph.queue_ends[component].contains(&node)
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guests::behavior::{KnownPlace, PlaceKind};
    use crate::world::{Path, Surface};

    #[test]
    fn save_guest_memories_survive_round_trip_and_continuation() {
        use crate::guests::memories::MemoryEntry;

        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            2,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: entrance,
            slope: None,
        });
        let mut original = ParkGuests::new(42, 2, &world, entrance, 1);
        original.walking.traits_mut().record_memory(
            GuestId(0),
            MemoryEntry {
                ride_id: 9,
                enjoyment: 7,
                nausea: 3,
                tick: 12,
            },
        );
        original.tick(&world, &[], 0, Some(1));
        let bytes = postcard::to_allocvec(&original).unwrap();
        let mut restored: ParkGuests = postcard::from_bytes(&bytes).unwrap();
        restored.rebind_world(&world);
        assert_eq!(
            original.walking.traits().memory(GuestId(0)),
            restored.walking.traits().memory(GuestId(0))
        );
        assert_eq!(original.state_hash(), restored.state_hash());
        for _ in 0..20 {
            original.tick(&world, &[], 0, None);
            restored.tick(&world, &[], 0, None);
            assert_eq!(original.state_hash(), restored.state_hash());
        }
    }

    #[test]
    fn save_riders_and_waiting_queue_continue_with_a_moving_train() {
        use crate::guests::details::ActivityKind;
        use crate::physics::{PhysicsConfig, Train, demo_circuit};
        use crate::state_hash::StateHasher;
        use crate::world::PathKind;

        let position = |x| Position { x, y: 0, height: 0 };
        let mut world = World::new(
            4,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world
            .build_path_route(position(0), position(1), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(1), position(3), PathKind::Queue, 0)
            .unwrap();
        let facility = Facility {
            place: KnownPlace {
                id: 700,
                position: position(3),
                kind: PlaceKind::Ride,
                minimum_height_cm: 0,
                intensity: 50,
                nausea: 0,
                ride_type: 2,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: Some(position(0)),
        };
        let mut original_guests = ParkGuests::new(31, 12, &world, position(0), 12);
        original_guests.open_with_guests(&[facility]);
        original_guests.set_external_rides(&[facility.place.id]);
        let mut riders = Vec::new();
        for _ in 0..20_000 {
            original_guests.tick(&world, &[facility], 100, None);
            if riders.is_empty() {
                riders = original_guests.board_coaster(&facility);
            }
            if !riders.is_empty()
                && (0..12).any(|id| {
                    original_guests.behaviors.activity_detail(GuestId(id)).kind
                        == ActivityKind::Queuing
                })
            {
                break;
            }
        }
        assert!(!riders.is_empty());
        assert!(
            riders
                .iter()
                .all(|guest| original_guests.behaviors.is_coaster_riding(*guest))
        );
        assert!((0..12).any(
            |id| original_guests.behaviors.activity_detail(GuestId(id)).kind
                == ActivityKind::Queuing
        ));
        let original_circuit = demo_circuit();
        let mut original_train = Train::new(
            &original_circuit,
            &crate::track::hybrid_model().train,
            PhysicsConfig::default(),
            0.0,
            2.0,
        );
        for _ in 0..100 {
            original_train.step(&original_circuit);
        }
        let bytes = crate::track::curves::with_snapshot_arc_tables(|| {
            postcard::to_allocvec(&(&original_guests, &original_circuit, &original_train))
        })
        .unwrap();
        let (mut restored_guests, restored_circuit, mut restored_train): (
            ParkGuests,
            crate::physics::Circuit,
            Train,
        ) = crate::track::curves::with_snapshot_arc_tables(|| postcard::from_bytes(&bytes))
            .unwrap();
        restored_guests.rebind_world(&world);
        for _ in 0..20 {
            assert_eq!(original_guests.state_hash(), restored_guests.state_hash());
            let mut first = StateHasher::new();
            let mut second = StateHasher::new();
            original_train.hash_into(&mut first);
            restored_train.hash_into(&mut second);
            assert_eq!(first.finish(), second.finish());
            original_guests.tick(&world, &[facility], 100, None);
            restored_guests.tick(&world, &[facility], 100, None);
            original_train.step(&original_circuit);
            restored_train.step(&restored_circuit);
        }
    }

    #[test]
    fn placed_scenery_changes_guest_happiness_and_details() {
        use crate::guests::{GuestMemoryDetail, Thought};
        use crate::world::{Scenery, SceneryKind};

        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut bare = World::new(
            2,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        bare.place_path(Path {
            position: entrance,
            slope: None,
        });
        let mut decorated = bare.clone();
        let seed = (0..100)
            .find(|&seed| {
                crate::guests::generate_guests(seed, 1).cares_about_theming(GuestId(0)) >= 25
            })
            .unwrap();
        let mut without = ParkGuests::new(seed, 1, &bare, entrance, 1);
        let mut with = ParkGuests::new(seed, 1, &decorated, entrance, 1);
        let guest = GuestId(0);
        let traits = with.walking.traits();
        assert!(traits.cares_about_theming(guest) >= 25);
        let theme = traits.preferred_theme(guest);
        decorated
            .place_scenery(Scenery {
                kind: if theme == 0 {
                    SceneryKind::FlowerBed
                } else {
                    SceneryKind::TreeCone
                },
                position: Position {
                    x: 1,
                    y: 0,
                    height: 0,
                },
                quarter: 0,
                rotation: 0,
            })
            .unwrap();
        without.open_with_guests(&[]);
        with.open_with_guests(&[]);
        for _ in 0..2 {
            without.tick(&bare, &[], 100, None);
            with.tick(&decorated, &[], 100, None);
        }
        let detail = with.guest_detail(guest).unwrap();
        assert!(detail.happiness > without.guest_detail(guest).unwrap().happiness);
        assert_eq!(detail.thought, Thought::EnjoyedScenery(theme));
        assert!(detail.memories.iter().any(|memory| matches!(memory, GuestMemoryDetail::Scenery { theme: remembered, .. } if *remembered == theme)));
    }

    #[test]
    fn ambiguous_queue_does_not_advertise_either_ride() {
        use crate::world::PathKind;
        let mut world = World::new(
            14,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x| Position { x, y: 0, height: 0 };
        world
            .build_path_route(position(0), position(1), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(1), position(12), PathKind::Queue, 0)
            .unwrap();
        let ride = Facility {
            place: KnownPlace {
                id: 700,
                position: position(12),
                kind: PlaceKind::Ride,
                minimum_height_cm: 0,
                intensity: 50,
                nausea: 0,
                ride_type: 2,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: Some(position(0)),
        };
        let other = Facility {
            place: KnownPlace {
                id: 701,
                ..ride.place
            },
            ..ride
        };
        let mut park = ParkGuests::new(31, 1, &world, position(0), 1);
        for _ in 0..300 {
            park.tick(&world, &[ride, other], 100, Some(1));
        }
        assert!(park.behaviors.guests[0].known_places().is_empty());
    }

    #[test]
    fn arriving_guest_discovers_and_boards_coaster_at_long_queue_entrance() {
        use crate::world::PathKind;
        let mut world = World::new(
            14,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x| Position { x, y: 0, height: 0 };
        world
            .build_path_route(position(0), position(1), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(1), position(12), PathKind::Queue, 0)
            .unwrap();
        let facility = Facility {
            place: KnownPlace {
                id: 700,
                position: position(12),
                kind: PlaceKind::Ride,
                minimum_height_cm: 0,
                intensity: 50,
                nausea: 0,
                ride_type: 2,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: Some(position(0)),
        };
        let mut park = ParkGuests::new(31, 1, &world, position(0), 1);
        park.set_external_rides(&[facility.place.id]);
        let mut boarded = Vec::new();
        for _ in 0..20_000 {
            park.tick(&world, &[facility], 100, Some(1));
            boarded = park.board_coaster(&facility);
            if !boarded.is_empty() {
                break;
            }
        }
        assert_eq!(
            boarded,
            vec![GuestId(0)],
            "active {} position {:?} known {:?} activity {:?}",
            park.walking.traits().is_active(GuestId(0)),
            park.walking.guests()[0].settled_path(),
            park.behaviors.guests[0].known_places(),
            park.behaviors.activity_detail(GuestId(0))
        );
        assert!(
            park.behaviors.guests[0]
                .known_places()
                .iter()
                .any(|place| place.id == facility.place.id)
        );
    }

    #[test]
    fn coaster_guest_queues_rides_exits_and_waits_to_depart() {
        coaster_guest_journey(false, false);
    }

    #[test]
    fn coaster_edit_recovers_rider_without_a_ride_memory() {
        coaster_guest_journey(true, false);
    }

    #[test]
    fn coaster_rejects_guest_below_minimum_height() {
        coaster_guest_journey(false, true);
    }

    fn coaster_guest_journey(recovered: bool, too_short: bool) {
        use crate::world::PathKind;
        let mut world = World::new(
            4,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x| Position { x, y: 0, height: 0 };
        world
            .build_path_route(position(0), position(1), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(1), position(3), PathKind::Queue, 0)
            .unwrap();
        let facility = Facility {
            place: KnownPlace {
                id: 700,
                position: position(3),
                kind: PlaceKind::Ride,
                minimum_height_cm: if too_short { u8::MAX } else { 0 },
                intensity: 50,
                nausea: 0,
                ride_type: 2,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: Some(position(0)),
        };
        let mut park = ParkGuests::new(31, 1, &world, position(0), 1);
        park.open_with_guests(&[facility]);
        park.set_external_rides(&[facility.place.id]);
        let mut boarded = Vec::new();
        for _ in 0..5000 {
            park.tick(&world, &[facility], 100, None);
            boarded = park.board_coaster(&facility);
            if !boarded.is_empty() {
                break;
            }
        }
        if too_short {
            assert!(boarded.is_empty());
            assert_eq!(park.walking.traits().memory(GuestId(0)).len(), 0);
            return;
        }
        assert_eq!(boarded, vec![GuestId(0)]);
        assert!(park.behaviors.is_coaster_riding(GuestId(0)));
        park.behaviors.guests[0].energy = 0;
        for _ in 0..40 {
            park.tick(&world, &[facility], 100, None);
        }
        assert!(park.walking.traits().is_active(GuestId(0)));
        assert!(park.behaviors.is_coaster_riding(GuestId(0)));
        let circuit = crate::physics::demo_circuit();
        let mut run = crate::physics::CoasterRun::new(
            &circuit,
            &crate::track::hybrid_model().train,
            crate::physics::PhysicsConfig {
                lift_speed: 5.0,
                ..Default::default()
            },
        );
        let mut outcome = if recovered {
            world.remove_path(position(3));
            run.recover()
        } else {
            (0..200_000)
                .find_map(|_| run.step(&circuit, "Timber Tumbler"))
                .expect("train finishes")
        };
        if !recovered {
            use crate::track::proximity::{ProximityEvent, ProximityKind};
            let distance = outcome.reports[0].samples[0].distance;
            outcome.reports[0].add_proximity_events(&[ProximityEvent {
                piece_index: 0,
                kind: ProximityKind::Tunnel,
                position: crate::track::Vec3::new(1.0, 2.0, 3.0),
                distance,
                strength: 0.5,
            }]);
            assert_eq!(outcome.reports[0].proximity_events.len(), 1);
        }
        let report = &outcome.reports[0];
        park.finish_coaster(
            &world,
            &facility,
            &[(report.excitement, report.intensity, report.nausea)],
            std::slice::from_ref(&report.proximity_events),
            outcome.recovered,
        );
        if recovered {
            park.close_coaster(&world, &facility);
        } else {
            assert!(park.guest_detail(GuestId(0)).unwrap().memories.iter().any(|memory| matches!(
                memory,
                crate::guests::GuestMemoryDetail::Proximity { ride_id: 700, kind: crate::track::proximity::ProximityKind::Tunnel, strength, .. } if *strength == 0.5
            )));
        }
        assert_eq!(park.walking.guests()[0].settled_path(), Some(position(0)));
        assert_eq!(
            park.walking.traits().memory(GuestId(0)).len(),
            usize::from(!recovered)
        );
        park.tick(&world, &[facility], 100, Some(0));
        assert!(!park.walking.traits().is_active(GuestId(0)));
    }

    #[test]
    fn queue_access_requires_one_owner_at_the_far_end() {
        use crate::world::PathKind;
        let mut world = World::new(
            5,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x| Position { x, y: 0, height: 0 };
        world.place_path(Path {
            position: position(0),
            slope: None,
        });
        world
            .build_path_route(position(0), position(4), PathKind::Queue, 0)
            .unwrap();
        let graph = Graph::with_entrance(&world, Some(position(0)));
        let facility = |id, x, kind| Facility {
            place: KnownPlace {
                id,
                position: position(x),
                kind,
                minimum_height_cm: 0,
                intensity: 0,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: None,
        };
        let ride = facility(1, 4, PlaceKind::Ride);
        let food = facility(2, 3, PlaceKind::Food);
        assert_eq!(accessible_facilities(&graph, &[ride]), vec![ride]);
        assert!(accessible_facilities(&graph, &[food]).is_empty());
        assert!(accessible_facilities(&graph, &[ride, food]).is_empty());
    }

    #[test]
    fn guest_walks_from_normal_path_to_the_chosen_ride_queue_end() {
        use crate::world::PathKind;
        let mut world = World::new(
            5,
            4,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x, y| Position { x, y, height: 0 };
        for x in 0..5 {
            world.place_path(Path {
                position: position(x, 0),
                slope: None,
            });
        }
        for x in [0, 4] {
            world
                .build_path_route(position(x, 0), position(x, 3), PathKind::Queue, 0)
                .unwrap();
        }
        let facility = |id, x, kind| Facility {
            place: KnownPlace {
                id,
                position: position(x, 3),
                kind,
                minimum_height_cm: 0,
                intensity: 0,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: None,
        };
        let ride = facility(1, 4, PlaceKind::Ride);
        let food = facility(2, 0, PlaceKind::Food);
        let graph = Graph::with_entrance(&world, Some(position(2, 0)));
        assert_eq!(
            accessible_facilities(&graph, &[ride, food]),
            vec![ride, food]
        );
        let mut walking = WalkingSimulation::with_entrance(3, 1, &world, position(2, 0));
        walking.place_on_path(GuestId(0), position(2, 0));
        let mut visited = Vec::new();
        for _ in 0..1000 {
            walking.tick_with_controls(
                &world,
                &SequentialExecutor::new(256),
                &[WalkControl::Destination(ride.place.position)],
            );
            if let Some(path) = walking.guests()[0].settled_path() {
                if visited.last() != Some(&path) {
                    visited.push(path);
                }
            }
            if walking.guests()[0].settled_path() == Some(ride.place.position) {
                break;
            }
        }
        assert_eq!(visited.last(), Some(&ride.place.position));
        let queue_paths: Vec<_> = visited
            .into_iter()
            .filter(|position| position.y > 0)
            .collect();
        assert_eq!(
            queue_paths,
            vec![position(4, 1), position(4, 2), position(4, 3)]
        );
        let mut park = ParkGuests::new(3, 1, &world, position(2, 0), 1);
        park.open_with_guests(&[ride, food]);
        park.walking.place_on_path(GuestId(0), position(2, 0));
        park.behaviors.guests[0].needs.hunger = 100;
        let mut queue_paths = Vec::new();
        for _ in 0..1500 {
            park.tick(&world, &[ride, food], 100, None);
            if let Some(path) = park.walking.guests()[0].settled_path() {
                if path.y > 0 && queue_paths.last() != Some(&path) {
                    queue_paths.push(path);
                }
            }
            if queue_paths.last() == Some(&food.place.position) {
                break;
            }
        }
        assert_eq!(
            queue_paths,
            vec![position(0, 1), position(0, 2), position(0, 3)]
        );
    }

    #[test]
    fn placed_ride_and_food_use_their_own_queues() {
        use crate::facilities::{FacilityKind, ParkFacilities};
        use crate::world::{Direction, PathKind};
        let mut world = World::new(
            10,
            6,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let position = |x, y| Position { x, y, height: 0 };
        world
            .build_path_route(position(0, 0), position(9, 0), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(8, 0), position(8, 3), PathKind::Normal, 0)
            .unwrap();
        world
            .build_path_route(position(8, 3), position(7, 3), PathKind::Normal, 0)
            .unwrap();
        for x in [1, 6] {
            world
                .build_path_route(position(x, 0), position(x, 3), PathKind::Queue, 0)
                .unwrap();
        }
        let mut buildings = ParkFacilities::new();
        let food_id = buildings
            .place(&world, FacilityKind::Food, position(1, 4), Direction::North)
            .unwrap();
        let ride_id = buildings
            .place(
                &world,
                FacilityKind::Carousel,
                position(6, 4),
                Direction::North,
            )
            .unwrap();
        let facilities = buildings.guest_facilities(&world);
        assert_eq!(
            facilities
                .iter()
                .map(|facility| facility.place.id)
                .collect::<Vec<_>>(),
            vec![food_id, ride_id]
        );
        for (target_id, queue_x, hungry) in [(food_id, 1, true), (ride_id, 6, false)] {
            let mut park = ParkGuests::new(3, 1, &world, position(4, 0), 1);
            park.open_with_guests(&facilities);
            park.walking.place_on_path(GuestId(0), position(4, 0));
            park.behaviors.guests[0].needs.hunger = if hungry { 100 } else { 0 };
            let mut visited = Vec::new();
            for _ in 0..2000 {
                park.tick(&world, &facilities, 100, None);
                if let Some(path) = park.walking.guests()[0].settled_path() {
                    if world.path_metadata(path).unwrap().kind == PathKind::Queue
                        && visited.last() != Some(&path)
                    {
                        visited.push(path);
                    }
                }
                if visited.last() == Some(&position(queue_x, 3)) {
                    break;
                }
            }
            assert_eq!(
                park.behaviors.activity_detail(GuestId(0)).place_id,
                Some(target_id)
            );
            assert_eq!(
                visited,
                vec![
                    position(queue_x, 1),
                    position(queue_x, 2),
                    position(queue_x, 3)
                ]
            );
        }
    }

    #[test]
    fn open_park_starts_all_guests_on_seeded_paths_and_facilities() {
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let facility_path = Position {
            x: 1,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            2,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for position in [entrance, facility_path] {
            world.place_path(Path {
                position,
                slope: None,
            });
        }
        let facility = Facility {
            exit_position: None,
            place: KnownPlace {
                id: 1,
                position: facility_path,
                kind: PlaceKind::Food,
                minimum_height_cm: 0,
                intensity: 0,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 4,
            duration_ticks: 10,
        };
        let mut first = ParkGuests::new(17, 12, &world, entrance, 2);
        let mut second = ParkGuests::new(17, 12, &world, entrance, 2);
        first.open_with_guests(&[facility]);
        second.open_with_guests(&[facility]);

        assert_eq!(first.active_count(), 12);
        assert!(
            first
                .walking()
                .traits()
                .ids()
                .all(|id| first.walking().traits().is_active(id))
        );
        assert_eq!(first.walking().state_hash(), second.walking().state_hash());
        assert_eq!(first.walking().traits(), second.walking().traits());
        assert!(
            first
                .walking()
                .guests()
                .iter()
                .all(|guest| { guest.path == Some(entrance) || guest.path == Some(facility_path) })
        );
        assert!(
            first
                .behaviors()
                .guests
                .iter()
                .all(|guest| guest.visit_number == 1)
        );
    }

    #[test]
    fn guests_arrive_complete_activities_depart_and_return_with_memories() {
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            2,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: entrance,
            slope: None,
        });
        let facilities: Vec<_> = [
            PlaceKind::Ride,
            PlaceKind::Food,
            PlaceKind::Drink,
            PlaceKind::Restroom,
        ]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Facility {
            exit_position: None,
            place: KnownPlace {
                id: id as u32,
                position: entrance,
                kind,
                minimum_height_cm: 0,
                intensity: 0,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 2,
            duration_ticks: 1,
        })
        .collect();
        let mut park = ParkGuests::new(42, 2, &world, entrance, 1);
        let initial = park.walking().guests()[1];
        let arrivals = park.tick(&world, &facilities, 0, Some(1));
        assert_eq!(arrivals.arrived, vec![GuestId(0)]);
        assert_eq!(park.active_count(), 1);
        let mut departed = false;
        for _ in 0..100_000 {
            let result = park.tick(&world, &facilities, 0, Some(0));
            if !result.departed.is_empty() {
                departed = true;
                break;
            }
        }
        assert!(
            departed,
            "guest should complete a visit using the facilities"
        );
        assert_eq!(park.walking().guests()[1], initial);
        assert_eq!(park.active_count(), 0);
        let guest = GuestId(0);
        assert!(!park.walking().traits().memory(guest).is_empty());
        let memories = park.walking().traits().memory(guest).clone();
        park.tick(&world, &facilities, 0, Some(1));
        let result = park.tick(&world, &facilities, 0, Some(2));
        assert_eq!(result.arrived, vec![guest]);
        assert_eq!(park.walking().traits().memory(guest), &memories);
        assert_eq!(park.behaviors().guests[0].rides_completed, 0);
    }
    #[cfg(feature = "threads")]
    #[test]
    fn complete_simulation_is_deterministic_across_executors() {
        use crate::executor::ThreadedExecutor;
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            2,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..2 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        let facilities = [Facility {
            exit_position: None,
            place: KnownPlace {
                id: 1,
                position: entrance,
                kind: PlaceKind::Ride,
                minimum_height_cm: 0,
                intensity: 10,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 5,
            duration_ticks: 5,
        }];
        let mut first = ParkGuests::new(42, 50, &world, entrance, 10);
        let mut second = ParkGuests::new(42, 50, &world, entrance, 10);
        let executor = ThreadedExecutor::new(4, 7);
        for _ in 0..200 {
            assert_eq!(
                first.tick(&world, &facilities, 100, None),
                second.tick_with(&world, &facilities, 100, None, &executor)
            );
        }
        assert_eq!(first.walking().state_hash(), second.walking().state_hash());
        assert_eq!(first.walking().traits(), second.walking().traits());
        assert_eq!(first.behaviors().guests, second.behaviors().guests);
    }
    #[test]
    fn completed_visit_walks_back_to_entrance_before_departing() {
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let destination = Position {
            x: 2,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            3,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..3 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        let facilities = [Facility {
            exit_position: None,
            place: KnownPlace {
                id: 1,
                position: destination,
                kind: PlaceKind::Ride,
                minimum_height_cm: 0,
                intensity: 0,
                nausea: 0,
                ride_type: 0,
                theme: 0,
                price: 0,
            },
            capacity: 1,
            duration_ticks: 100,
        }];
        let mut park = ParkGuests::new(42, 1, &world, entrance, 1);
        park.tick(&world, &facilities, 100, None);
        for _ in 0..1000 {
            park.tick(&world, &facilities, 0, Some(0));
            if park.walking().guests()[0].settled_path() == Some(destination) {
                break;
            }
        }
        assert_eq!(park.walking().guests()[0].settled_path(), Some(destination));
        park.behaviors.guests[0].rides_completed = 12;
        park.behaviors.guests[0].stops_completed = 3;
        assert!(
            park.tick(&world, &facilities, 0, Some(0))
                .departed
                .is_empty()
        );
        assert_eq!(park.active_count(), 1);
        let mut departed = false;
        for _ in 0..1000 {
            if !park
                .tick(&world, &facilities, 0, Some(0))
                .departed
                .is_empty()
            {
                departed = true;
                break;
            }
        }
        assert!(departed);
        assert_eq!(park.walking().guests()[0].settled_path(), Some(entrance));
    }

    #[test]
    fn empty_roster_advances_without_arrivals() {
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            1,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: entrance,
            slope: None,
        });
        let mut park = ParkGuests::new(1, 0, &world, entrance, 1);
        assert_eq!(park.tick(&world, &[], 100, None), VisitChanges::default());
    }

    #[test]
    fn guests_pay_the_entrance_fee_when_they_arrive() {
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            1,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: entrance,
            slope: None,
        });
        let mut park = ParkGuests::new(1, 1, &world, entrance, 1);
        park.set_entrance_price(6);

        let arrivals = park.tick(&world, &[], 100, None);

        assert_eq!(arrivals.arrived, vec![GuestId(0)]);
        assert_eq!(park.revenue(), 6);
        assert_eq!(park.behaviors().guests[0].money, 94);
    }
}

#[cfg(all(test, feature = "threads"))]
#[path = "park_determinism_tests.rs"]
mod park_determinism_tests;
