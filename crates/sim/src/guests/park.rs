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
    walking: WalkingSimulation,
    behaviors: BehaviorSimulation,
    visits: VisitManager,
    entrance: Position,
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
            entrance,
            reachable: Graph::with_entrance(world, Some(entrance)),
            world_revision: world.revision(),
            leaving: vec![false; roster_size],
        }
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
            self.reachable = Graph::with_entrance(world, Some(self.entrance));
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
            .map(|(index, position)| self.leaving[index] && *position == Some(self.entrance))
            .collect();
        let departed =
            self.visits
                .depart_ready(self.walking.traits_mut(), &self.behaviors.guests, &ready);
        for id in &departed {
            self.leaving[id.0 as usize] = false;
        }
        let arrived = if departed.is_empty() && component(self.entrance).is_some() {
            self.visits.arrive(
                self.walking.traits_mut(),
                &mut self.behaviors.guests,
                appeal,
                population_override,
            )
        } else {
            Vec::new()
        };
        self.walking.admit(&arrived);
        let controls: Vec<_> = movements
            .into_iter()
            .enumerate()
            .map(|(index, movement)| {
                if !self.walking.traits().is_active(GuestId(index as u32)) {
                    return WalkControl::Stop;
                }
                if self.leaving[index] {
                    return WalkControl::Destination(self.entrance);
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
