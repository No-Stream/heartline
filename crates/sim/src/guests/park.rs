//! Coordinates guest arrivals, decisions, activities and walking.

use crate::guest_behavior::{BehaviorSimulation, Facility, Movement};
use crate::guests::GuestId;
use crate::guests::walking::{WalkControl, WalkingGuest, WalkingSimulation};
use crate::pathfinding::Graph;
use crate::visits::VisitManager;
use crate::world::{Position, World};
use crate::{Executor, SequentialExecutor};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct VisitChanges {
    pub arrived: Vec<GuestId>,
    pub departed: Vec<GuestId>,
}

pub struct ParkGuests {
    pub(super) walking: WalkingSimulation,
    pub(super) behaviors: BehaviorSimulation,
    visits: VisitManager,
    entrance: Option<Position>,
    reachable: Graph,
    world_revision: u64,
    leaving: Vec<bool>,
}

impl ParkGuests {
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
        }
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

    pub fn walking(&self) -> &WalkingSimulation {
        &self.walking
    }
    pub fn behaviors(&self) -> &BehaviorSimulation {
        &self.behaviors
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
            .filter(|position| self.walking.path_component(*position).is_some())
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
        let movements = self.behaviors.tick_with_exiting(
            self.walking.traits_mut(),
            &positions,
            facilities,
            &guest_components,
            &facility_components,
            &self.leaving,
        );
        for group in &self.walking.traits().groups {
            if group
                .members
                .iter()
                .any(|id| self.behaviors.guests[id.0 as usize].visit_complete())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guest_behavior::{KnownPlace, PlaceKind};
    use crate::world::{Path, Surface};

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
        use crate::ThreadedExecutor;
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
}
