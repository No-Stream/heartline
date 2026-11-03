//! Coordinates guest activities and per-tick behavior.
use std::collections::BTreeMap;

pub use super::decisions::{GroupSplit, GuestBehavior, split_for_ride};
pub use super::knowledge::{KnownPlace, PlaceKind};
pub use super::needs::Needs;
pub use super::thoughts::Thought;
use crate::guests::{AgeGroup, GuestId, GuestStore};
use crate::world::Position;

/// A facility's path position, service capacity, and duration in 1/30-second ticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Facility {
    pub place: KnownPlace,
    pub capacity: usize,
    pub duration_ticks: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Activity {
    Walking(u32),
    Queuing(u32),
    Using(u32, u32),
    Waiting(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Movement {
    Wander,
    Destination(Position),
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct BehaviorSimulation {
    pub guests: Vec<GuestBehavior>,
    activities: Vec<Option<Activity>>,
    queues: BTreeMap<u32, Vec<GuestId>>,
    tick: u64,
}

impl BehaviorSimulation {
    pub fn new(guest_count: usize) -> Self {
        Self {
            guests: vec![GuestBehavior::default(); guest_count],
            activities: vec![None; guest_count],
            queues: BTreeMap::new(),
            tick: 0,
        }
    }
    /// Returns movement commands indexed by guest ID.
    /// Call with one current path per guest and currently reachable, open facilities only.
    pub fn tick(
        &mut self,
        guests: &mut GuestStore,
        positions: &[Option<Position>],
        facilities: &[Facility],
        guest_components: &[Option<usize>],
        facility_components: &[Option<usize>],
    ) -> Vec<Movement> {
        self.tick_with_exiting(
            guests,
            positions,
            facilities,
            guest_components,
            facility_components,
            &vec![false; self.guests.len()],
        )
    }

    pub fn tick_with_exiting(
        &mut self,
        guests: &mut GuestStore,
        positions: &[Option<Position>],
        facilities: &[Facility],
        guest_components: &[Option<usize>],
        facility_components: &[Option<usize>],
        exiting: &[bool],
    ) -> Vec<Movement> {
        assert_eq!(self.guests.len(), exiting.len());
        assert_eq!(self.guests.len(), positions.len());
        assert_eq!(self.guests.len(), guest_components.len());
        assert_eq!(facilities.len(), facility_components.len());
        assert!(facilities.iter().all(|facility| facility.capacity > 0
            && facility.duration_ticks > 0
            && facility.place.ride_type < 8));
        self.tick += 1;
        let by_id: BTreeMap<_, _> = facilities
            .iter()
            .map(|facility| (facility.place.id, *facility))
            .collect();
        assert_eq!(by_id.len(), facilities.len(), "duplicate facility id");
        let components: BTreeMap<_, _> = facilities
            .iter()
            .zip(facility_components)
            .map(|(facility, &component)| (facility.place.id, component))
            .collect();
        for (index, position) in positions.iter().copied().enumerate() {
            let id = GuestId(index as u32);
            if !guests.is_active(id) || exiting[index] {
                continue;
            }
            self.guests[index].advance_for(1, guests.age(id));
            if self.guests[index].thought == Thought::Exploring {
                if let Some(memory) = guests.memory(id).iter().last() {
                    self.guests[index].thought = if memory.enjoyment >= 0 {
                        Thought::EnjoyedRide(memory.ride_id)
                    } else {
                        Thought::DislikedRide(memory.ride_id)
                    };
                }
            }
            // Night is the second half of a 25-minute day; mood shifts gently.
            if self.tick % 45_000 >= 22_500 && self.tick % 3_600 == 0 {
                self.guests[index].happiness = self.guests[index].happiness.saturating_sub(1);
            }
            if index as u64 % 30 != self.tick % 30 {
                continue;
            }
            if let Some(position) = position {
                for (facility, component) in facilities.iter().zip(facility_components) {
                    if guest_components[index].is_none() || guest_components[index] != *component {
                        continue;
                    }
                    let p = facility.place.position;
                    if position.x.abs_diff(p.x) + position.y.abs_diff(p.y) <= 4
                        && (position.height - p.height).abs() <= 2
                    {
                        self.guests[index].observe(facility.place);
                    }
                }
            }
        }
        for (group_index, group) in guests.groups.iter().enumerate() {
            if group_index as u64 % 30 != self.tick % 30 {
                continue;
            }
            for &member in &group.members {
                let index = member.0 as usize;
                if !guests.is_active(member) || exiting[index] {
                    continue;
                }
                let Some(position) = positions[index] else {
                    continue;
                };
                let shared: Vec<_> = group
                    .members
                    .iter()
                    .filter(|&&other| {
                        guests.is_active(other)
                            && !exiting[other.0 as usize]
                            && positions[other.0 as usize].is_some_and(|other_position| {
                                position.x.abs_diff(other_position.x)
                                    + position.y.abs_diff(other_position.y)
                                    <= 1
                                    && position.height == other_position.height
                            })
                    })
                    .flat_map(|other| self.guests[other.0 as usize].known_places().iter().copied())
                    .collect();
                for place in shared {
                    self.guests[index].observe(place);
                }
            }
        }
        for index in 0..self.guests.len() {
            let id = GuestId(index as u32);
            if !guests.is_active(id) || exiting[index] {
                self.activities[index] = None;
                continue;
            }
            match self.activities[index] {
                Some(Activity::Using(place_id, remaining)) if remaining <= 1 => {
                    if let Some(facility) = by_id.get(&place_id) {
                        let enjoyment = (80_i16
                            - (i16::from(guests.intensity(id))
                                - i16::from(facility.place.intensity))
                            .abs()
                                * 2
                            - self.guests[index].needs.nausea as i16)
                            .clamp(-100, 100) as i8;
                        self.guests[index].complete(
                            facility.place,
                            self.tick,
                            enjoyment,
                            guests,
                            id,
                        );
                    }
                    self.activities[index] = None;
                }
                Some(Activity::Using(place_id, remaining)) => {
                    self.activities[index] = Some(Activity::Using(place_id, remaining - 1))
                }
                Some(Activity::Walking(place_id))
                    if !by_id.contains_key(&place_id)
                        || guest_components[index].is_none()
                        || guest_components[index] != components[&place_id] =>
                {
                    self.activities[index] = None
                }
                Some(Activity::Walking(place_id))
                    if positions[index] == Some(by_id[&place_id].place.position) =>
                {
                    self.queues.entry(place_id).or_default().push(id);
                    self.activities[index] = Some(Activity::Queuing(place_id));
                }
                Some(Activity::Queuing(place_id)) if !by_id.contains_key(&place_id) => {
                    self.activities[index] = None
                }
                _ => {}
            }
        }
        for (group_index, group) in guests.groups.iter().enumerate() {
            if group_index as u64 % 30 != self.tick % 30 {
                continue;
            }
            let active: Vec<_> = group
                .members
                .iter()
                .copied()
                .filter(|&id| guests.is_active(id) && !exiting[id.0 as usize])
                .collect();
            for &id in &active {
                if let Some(Activity::Waiting(place_id)) = self.activities[id.0 as usize] {
                    let riders_remain = active.iter().any(|other| matches!(self.activities[other.0 as usize], Some(Activity::Walking(id) | Activity::Queuing(id) | Activity::Using(id, _)) if id == place_id));
                    if !riders_remain {
                        self.activities[id.0 as usize] = None;
                    }
                }
            }
            if active.is_empty()
                || active
                    .iter()
                    .any(|id| self.activities[id.0 as usize].is_some())
            {
                continue;
            }
            let Some(leader) = active
                .iter()
                .copied()
                .find(|&id| guests.age(id) != AgeGroup::Child)
                .or_else(|| active.first().copied())
            else {
                continue;
            };
            let index = leader.0 as usize;
            if self.guests[index].visit_complete() {
                continue;
            }
            let mut available = self.guests[index].clone();
            available.known_places.retain(|place| {
                by_id.contains_key(&place.id)
                    && guest_components[index].is_some()
                    && guest_components[index] == components[&place.id]
            });
            let queue_lengths = self
                .queues
                .iter()
                .map(|(&place_id, queue)| (place_id, queue.len()))
                .collect();
            let has_reachable_place = !available.known_places.is_empty();
            available.known_places.retain(|place| {
                active
                    .iter()
                    .all(|id| self.guests[id.0 as usize].money >= place.price)
            });
            let choice = available.choose_with_queues(guests, leader, &queue_lengths);
            if choice.is_none() && has_reachable_place && available.known_places.is_empty() {
                for id in &active {
                    self.guests[id.0 as usize].cannot_afford_activity = true;
                }
            }
            if let Some(place) = choice {
                let split = if place.kind == PlaceKind::Ride {
                    split_for_ride(guests, &active, place)
                } else {
                    GroupSplit {
                        riding: active.clone(),
                        waiting: Vec::new(),
                    }
                };
                if !split.riding.is_empty() {
                    for id in split.riding {
                        self.activities[id.0 as usize] = Some(Activity::Walking(place.id));
                    }
                    for id in split.waiting {
                        self.activities[id.0 as usize] = Some(Activity::Waiting(place.id));
                    }
                }
            }
        }
        for (&place_id, queue) in &mut self.queues {
            queue.retain(|id| self.activities[id.0 as usize] == Some(Activity::Queuing(place_id)));
            if let Some(facility) = by_id.get(&place_id) {
                let occupied = self.activities.iter().filter(|activity| matches!(activity, Some(Activity::Using(id, _)) if *id == place_id)).count();
                let take = facility.capacity.saturating_sub(occupied).min(queue.len());
                for id in queue.drain(..take) {
                    self.activities[id.0 as usize] =
                        Some(Activity::Using(place_id, facility.duration_ticks));
                }
            }
        }
        self.activities
            .iter()
            .enumerate()
            .map(|(index, activity)| match activity {
                Some(Activity::Walking(id)) => Movement::Destination(by_id[id].place.position),
                Some(Activity::Waiting(id))
                    if by_id.get(id).is_some_and(|facility| {
                        positions[index] != Some(facility.place.position)
                    }) =>
                {
                    Movement::Destination(by_id[id].place.position)
                }
                Some(_) => Movement::Stopped,
                None => Movement::Wander,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guests::{GuestId, generate_guests};
    use crate::world::Position;

    fn place(id: u32, kind: PlaceKind) -> KnownPlace {
        KnownPlace {
            id,
            position: Position {
                x: id as u16,
                y: 0,
                height: 0,
            },
            kind,
            minimum_height_cm: 0,
            intensity: 50,
            ride_type: 0,
            theme: 0,
            price: 0,
        }
    }

    #[test]
    fn choices_use_only_individual_knowledge_and_share_with_group() {
        let guests = generate_guests(12, 2);
        let mut first = GuestBehavior::default();
        let mut second = GuestBehavior::default();
        first.observe(place(1, PlaceKind::Ride));
        second.observe(place(2, PlaceKind::Ride));
        assert_eq!(first.choose(&guests, GuestId(0)).unwrap().id, 1);
        assert_eq!(second.choose(&guests, GuestId(1)).unwrap().id, 2);
        second.hear_from(&first);
        assert_eq!(second.known_places().len(), 2);
    }

    #[test]
    fn needs_accumulate_and_completed_stops_relieve_them() {
        let guests = generate_guests(12, 1);
        let mut behavior = GuestBehavior::default();
        behavior.observe(place(1, PlaceKind::Food));
        behavior.observe(place(2, PlaceKind::Drink));
        behavior.observe(place(3, PlaceKind::Restroom));
        behavior.advance(54_000);
        assert_eq!(
            behavior.choose(&guests, GuestId(0)).unwrap().kind,
            PlaceKind::Restroom
        );
        behavior.complete(
            place(3, PlaceKind::Restroom),
            1000,
            0,
            &mut generate_guests(12, 1),
            GuestId(0),
        );
        assert_eq!(
            behavior.choose(&guests, GuestId(0)).unwrap().kind,
            PlaceKind::Drink
        );
    }

    #[test]
    fn ride_memory_is_bounded_and_visit_counts_activities() {
        let mut guests = generate_guests(12, 1);
        let mut behavior = GuestBehavior::default();
        for tick in 0..12 {
            behavior.complete(
                place(tick, PlaceKind::Ride),
                u64::from(tick),
                50,
                &mut guests,
                GuestId(0),
            );
        }
        assert_eq!(guests.memory(GuestId(0)).len(), 8);
        assert_eq!(behavior.rides_completed, 12);
        assert!(!behavior.visit_complete());
        behavior.complete(place(20, PlaceKind::Food), 20, 0, &mut guests, GuestId(0));
        behavior.complete(place(21, PlaceKind::Drink), 21, 0, &mut guests, GuestId(0));
        behavior.complete(
            place(22, PlaceKind::Restroom),
            22,
            0,
            &mut guests,
            GuestId(0),
        );
        assert!(behavior.visit_complete());
    }

    #[test]
    fn short_children_stay_with_an_adult_when_group_splits() {
        let guests = generate_guests(7, 100);
        let family = guests
            .groups
            .iter()
            .find(|group| {
                group
                    .members
                    .iter()
                    .any(|&id| guests.age(id) == crate::guests::AgeGroup::Child)
                    && group
                        .members
                        .iter()
                        .any(|&id| guests.age(id) != crate::guests::AgeGroup::Child)
            })
            .unwrap();
        let mut ride = place(1, PlaceKind::Ride);
        ride.minimum_height_cm = 140;
        let split = split_for_ride(&guests, &family.members, ride);
        assert!(
            split
                .waiting
                .iter()
                .any(|&id| guests.age(id) == crate::guests::AgeGroup::Child)
        );
        assert!(
            split
                .waiting
                .iter()
                .any(|&id| guests.age(id) != crate::guests::AgeGroup::Child)
        );
        assert_eq!(
            split.riding.len() + split.waiting.len(),
            family.members.len()
        );
    }

    #[test]
    fn tick_moves_queues_and_completes_a_ride() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        let ride = place(1, PlaceKind::Ride);
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 2,
        }];
        let destinations = simulation.tick(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
        );
        assert_eq!(destinations[0], Movement::Destination(ride.position));
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(ride.position)],
                &facilities,
                &[Some(0)],
                &[Some(0)]
            )[0],
            Movement::Stopped
        );
        simulation.tick(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
        );
        simulation.tick(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
        );
        assert_eq!(simulation.guests[0].rides_completed, 1);
        assert_eq!(guests.memory(GuestId(0)).len(), 1);
    }

    #[test]
    fn family_decides_together_and_short_children_wait_with_an_adult() {
        let mut guests = generate_guests(7, 100);
        let family = guests
            .groups
            .iter()
            .find(|group| {
                group
                    .members
                    .iter()
                    .any(|&id| guests.age(id) == AgeGroup::Child)
                    && group
                        .members
                        .iter()
                        .filter(|&&id| guests.age(id) != AgeGroup::Child)
                        .count()
                        >= 2
            })
            .unwrap()
            .members
            .clone();
        let mut simulation = BehaviorSimulation::new(100);
        let mut positions = vec![None; 100];
        let ride = KnownPlace {
            minimum_height_cm: 140,
            ..place(1, PlaceKind::Ride)
        };
        for &id in &family {
            positions[id.0 as usize] = Some(ride.position);
        }
        let facilities = [Facility {
            place: ride,
            capacity: 4,
            duration_ticks: 2,
        }];
        for _ in 0..100 {
            simulation.tick(
                &mut guests,
                &positions,
                &facilities,
                &[Some(0); 100],
                &[Some(0)],
            );
            if family.iter().any(|id| {
                matches!(
                    simulation.activities[id.0 as usize],
                    Some(Activity::Using(_, _))
                )
            }) {
                break;
            }
        }
        assert!(family.iter().any(|id| matches!(
            simulation.activities[id.0 as usize],
            Some(Activity::Waiting(_))
        )));
        assert!(family.iter().any(|id| matches!(
            simulation.activities[id.0 as usize],
            Some(Activity::Using(_, _))
        )));
        for &child in family
            .iter()
            .filter(|&&id| guests.age(id) == AgeGroup::Child)
        {
            assert!(matches!(
                simulation.activities[child.0 as usize],
                Some(Activity::Waiting(_))
            ));
        }
    }

    #[test]
    fn realistic_visit_completes_after_a_day_cycle() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let position = place(0, PlaceKind::Ride).position;
        let facilities: Vec<_> = [
            PlaceKind::Ride,
            PlaceKind::Food,
            PlaceKind::Drink,
            PlaceKind::Restroom,
            PlaceKind::Bench,
        ]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Facility {
            place: KnownPlace {
                position,
                ..place(id as u32, kind)
            },
            capacity: 1,
            duration_ticks: if kind == PlaceKind::Ride { 5_000 } else { 600 },
        })
        .collect();
        for tick in 0..150_000 {
            simulation.tick(
                &mut guests,
                &[Some(position)],
                &facilities,
                &[Some(0)],
                &[Some(0); 5],
            );
            if tick == 45_000 {
                assert!(!simulation.guests[0].visit_complete());
            }
            if simulation.guests[0].visit_complete() {
                break;
            }
        }
        assert!(simulation.guests[0].rides_completed >= 12);
        assert!(simulation.guests[0].stops_completed >= 3);
    }

    #[test]
    fn disconnected_facility_is_not_seen_or_selected() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        let ride = place(1, PlaceKind::Ride);
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 2,
        }];
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(ride.position)],
                &facilities,
                &[Some(0)],
                &[Some(1)]
            ),
            vec![Movement::Wander]
        );
        assert!(simulation.guests[0].known_places().is_empty());
    }

    #[test]
    fn guest_leaves_when_no_known_activity_is_affordable() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        simulation.guests[0].money = 0;
        let ride = KnownPlace {
            price: 1,
            ..place(1, PlaceKind::Ride)
        };
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 2,
        }];
        simulation.tick(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
        );
        assert!(simulation.guests[0].visit_complete());
        assert!(simulation.guests[0].cannot_afford_activity);
    }

    #[test]
    fn uninformed_guest_does_not_choose_unseen_ride() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        let ride = place(10, PlaceKind::Ride);
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 2,
        }];
        let origin = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(origin)],
                &facilities,
                &[Some(0)],
                &[Some(0)]
            ),
            vec![Movement::Wander]
        );
    }

    #[test]
    fn hungry_guest_travels_to_known_food() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        simulation.guests[0].needs.hunger = 100;
        let food = place(1, PlaceKind::Food);
        let facilities = [Facility {
            place: food,
            capacity: 1,
            duration_ticks: 2,
        }];
        let origin = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(origin)],
                &facilities,
                &[Some(0)],
                &[Some(0)]
            ),
            vec![Movement::Destination(food.position)]
        );
    }
    #[test]
    fn exiting_cancels_service_without_recording_a_ride() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let ride = place(1, PlaceKind::Ride);
        simulation.guests[0].observe(ride);
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 2,
        }];
        for _ in 0..32 {
            simulation.tick(
                &mut guests,
                &[Some(ride.position)],
                &facilities,
                &[Some(0)],
                &[Some(0)],
            );
        }
        simulation.tick_with_exiting(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
            &[true],
        );
        for _ in 0..5 {
            simulation.tick_with_exiting(
                &mut guests,
                &[Some(ride.position)],
                &facilities,
                &[Some(0)],
                &[Some(0)],
                &[true],
            );
        }
        assert!(guests.memory(GuestId(0)).is_empty());
        assert_eq!(simulation.guests[0].rides_completed, 0);
    }
}
