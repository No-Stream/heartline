//! Coordinates guest activities and per-tick behavior.
use std::collections::{BTreeMap, BTreeSet};

pub use super::decisions::{GroupSplit, GuestBehavior, split_for_ride};
use super::details::{ActivityDetail, ActivityKind, GuestMemoryDetail};
pub use super::knowledge::{KnownPlace, PlaceKind};
pub use super::needs::Needs;
pub use super::thoughts::Thought;
use crate::guests::{AgeGroup, GuestId, GuestStore};
use crate::state_hash::StateHasher;
use crate::track::proximity::ProximityEvent;
use crate::world::Position;
use crate::world::World;

const QUEUE_ENTRANCE_VISIBILITY_TILES: u16 = 12;

/// A facility's path position, service capacity, and duration in 1/30-second ticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Facility {
    pub place: KnownPlace,
    pub capacity: usize,
    pub duration_ticks: u32,
    pub exit_position: Option<Position>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RidePhase {
    Idle,
    Running,
    Unloading,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RideState {
    pub phase: RidePhase,
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Activity {
    Walking(u32),
    Queuing(u32, u64),
    Using(u32, u32),
    CoasterRiding(u32),
    Unloading(u32, Position),
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
    income: BTreeMap<u32, u64>,
    external_rides: BTreeSet<u32>,
    coaster_riders: BTreeMap<(u32, u64), Vec<GuestId>>,
    scenery_scores: Vec<u8>,
    queue_scenery: Vec<u64>,
    tick: u64,
}

impl BehaviorSimulation {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"BehaviorSimulation-v1");
        hash.write_len(self.guests.len());
        for guest in &self.guests {
            guest.hash_into(hash);
        }
        hash.write_len(self.activities.len());
        for activity in &self.activities {
            hash.write_option(*activity, write_activity);
        }
        hash.write_len(self.queues.len());
        for (&place_id, guests) in &self.queues {
            hash.write_u32(place_id);
            hash.write_len(guests.len());
            for guest in guests {
                hash.write_u32(guest.0);
            }
        }
        hash.write_len(self.income.len());
        for (&place_id, &income) in &self.income {
            hash.write_u32(place_id);
            hash.write_u64(income);
        }
        hash.write_len(self.external_rides.len());
        for &ride in &self.external_rides {
            hash.write_u32(ride);
        }
        hash.write_len(self.coaster_riders.len());
        for (&(facility_id, train_id), guests) in &self.coaster_riders {
            hash.write_u32(facility_id);
            hash.write_u64(train_id);
            hash.write_len(guests.len());
            for guest in guests {
                hash.write_u32(guest.0);
            }
        }
        hash.write_len(self.scenery_scores.len());
        for &score in &self.scenery_scores {
            hash.write_u8(score);
        }
        hash.write_len(self.queue_scenery.len());
        for &score in &self.queue_scenery {
            hash.write_u64(score);
        }
        hash.write_u64(self.tick);
    }

    pub(super) fn observe_scenery(
        &mut self,
        world: &World,
        guests: &GuestStore,
        positions: &[Option<Position>],
        exiting: &[bool],
    ) {
        let has_scenery = world.scenery().next().is_some();
        for (index, position) in positions.iter().enumerate() {
            let guest = GuestId(index as u32);
            if !guests.is_active(guest) || exiting[index] {
                continue;
            }
            if matches!(self.activities[index], Some(Activity::CoasterRiding(_))) {
                continue;
            }
            if !has_scenery {
                self.scenery_scores[index] = 0;
                continue;
            }
            let Some(position) = position else {
                self.scenery_scores[index] = 0;
                continue;
            };
            let occupied = matches!(
                self.activities[index],
                Some(Activity::Walking(_) | Activity::Queuing(_, _) | Activity::Using(_, _))
            );
            if !occupied && index as u64 % 30 != self.tick % 30 {
                continue;
            }
            let preferred = guests.preferred_theme(guest);
            let mut theme = None;
            let mut weight = 0_u32;
            for scenery in world.nearby_scenery(*position, 3) {
                let candidate = scenery.kind.theme() as u8;
                weight = (weight + if candidate == preferred { 4 } else { 2 }).min(10_000);
                if theme.is_none() || candidate == preferred {
                    theme = Some(candidate);
                }
            }
            self.scenery_scores[index] = (20 * weight / (weight + 8)
                * u32::from(guests.cares_about_theming(guest))
                / 100) as u8;
            if matches!(self.activities[index], Some(Activity::Queuing(_, _))) {
                self.queue_scenery[index] += u64::from(self.scenery_scores[index]);
            }
            if let Some(theme) = theme.filter(|_| index as u64 % 1800 == self.tick % 1800) {
                self.guests[index].enjoy_scenery(
                    theme,
                    preferred,
                    guests.cares_about_theming(guest),
                    matches!(self.activities[index], Some(Activity::Queuing(_, _))),
                    self.tick,
                );
            }
        }
    }
    pub fn new(guest_count: usize) -> Self {
        Self {
            guests: vec![GuestBehavior::default(); guest_count],
            activities: vec![None; guest_count],
            queues: BTreeMap::new(),
            income: BTreeMap::new(),
            external_rides: BTreeSet::new(),
            coaster_riders: BTreeMap::new(),
            scenery_scores: vec![0; guest_count],
            queue_scenery: vec![0; guest_count],
            tick: 0,
        }
    }

    pub fn is_coaster_riding(&self, guest: GuestId) -> bool {
        matches!(
            self.activities[guest.0 as usize],
            Some(Activity::CoasterRiding(_))
        )
    }

    pub fn set_external_rides(&mut self, ids: &[u32]) {
        self.external_rides = ids.iter().copied().collect();
    }

    /// Admit one train in queue order and charge each rider at boarding.
    pub fn board_coaster(&mut self, facility: &Facility, guests: &GuestStore) -> Vec<GuestId> {
        self.board_coaster_train(facility, 0, guests)
    }

    /// Admit one identified train in queue order and charge each rider at boarding.
    pub fn board_coaster_train(
        &mut self,
        facility: &Facility,
        train_id: u64,
        guests: &GuestStore,
    ) -> Vec<GuestId> {
        let id = facility.place.id;
        assert!(self.external_rides.contains(&id));
        assert!(!self.coaster_riders.contains_key(&(id, train_id)));
        let queue = self.queues.entry(id).or_default();
        let mut selected: Vec<GuestId> = Vec::new();
        while let Some(&first) = queue.iter().find(|guest| !selected.contains(*guest)) {
            let group = guests
                .groups
                .iter()
                .find(|group| group.members.contains(&first))
                .expect("guest has a group");
            if group.members.iter().any(|member| matches!(self.activities[member.0 as usize], Some(Activity::Walking(place_id)) if place_id == id)) {
                break;
            }
            let queued: Vec<_> = queue
                .iter()
                .copied()
                .filter(|member| group.members.contains(member) && !selected.contains(member))
                .collect();
            if selected.len() + queued.len() > facility.capacity {
                if selected.is_empty() && queued.len() > facility.capacity {
                    selected.extend(queued.into_iter().take(facility.capacity));
                }
                break;
            }
            selected.extend(queued);
        }
        queue.retain(|guest| !selected.contains(guest));
        let mut riders = Vec::new();
        for guest in selected {
            let index = guest.0 as usize;
            if self.guests[index].money < facility.place.price {
                self.activities[index] = None;
                continue;
            }
            let queued_at = match self.activities[index] {
                Some(Activity::Queuing(place_id, tick)) if place_id == id => tick,
                _ => unreachable!("only queued guests are admitted"),
            };
            let behavior = &mut self.guests[index];
            behavior.money -= facility.place.price;
            behavior.lifetime_money_spent += u32::from(facility.place.price);
            behavior.record_memory(GuestMemoryDetail::Wait {
                place_id: id,
                duration_ticks: self.tick.saturating_sub(queued_at),
                perceived_duration_ticks: self
                    .tick
                    .saturating_sub(queued_at)
                    .saturating_sub(self.queue_scenery[index] / 40),
                tick: self.tick,
            });
            *self.income.entry(id).or_default() += u64::from(facility.place.price);
            self.activities[index] = Some(Activity::CoasterRiding(id));
            riders.push(guest);
        }
        if !riders.is_empty() {
            self.coaster_riders.insert((id, train_id), riders.clone());
        }
        riders
    }

    /// Cancel guests approaching or waiting for a closed coaster.
    pub fn close_coaster(&mut self, id: u32) -> Vec<GuestId> {
        self.queues.remove(&id);
        let mut displaced = Vec::new();
        for (index, activity) in self.activities.iter_mut().enumerate() {
            if matches!(*activity, Some(Activity::Walking(place) | Activity::Queuing(place, _) | Activity::Waiting(place) | Activity::Unloading(place, _)) if place == id)
            {
                *activity = None;
                displaced.push(GuestId(index as u32));
            }
        }
        displaced
    }

    /// Complete or recover a train. Ratings are in boarded guest order.
    pub fn finish_coaster(
        &mut self,
        facility: &Facility,
        guests: &mut GuestStore,
        ratings: &[(f64, f64, f64)],
        proximity_events: &[Vec<ProximityEvent>],
        recovered: bool,
    ) -> Vec<GuestId> {
        self.finish_coaster_train(facility, 0, guests, ratings, proximity_events, recovered)
    }

    /// Complete or recover one identified train. Ratings are in boarded guest order.
    pub fn finish_coaster_train(
        &mut self,
        facility: &Facility,
        train_id: u64,
        guests: &mut GuestStore,
        ratings: &[(f64, f64, f64)],
        proximity_events: &[Vec<ProximityEvent>],
        recovered: bool,
    ) -> Vec<GuestId> {
        let riders = self
            .coaster_riders
            .remove(&(facility.place.id, train_id))
            .unwrap_or_default();
        assert!(
            recovered || (ratings.len() == riders.len() && proximity_events.len() == riders.len())
        );
        for (seat, guest) in riders.iter().copied().enumerate() {
            let index = guest.0 as usize;
            if !recovered {
                let (excitement, intensity, nausea) = ratings[seat];
                assert!(excitement.is_finite() && intensity.is_finite() && nausea.is_finite());
                let enjoyment = (excitement
                    - (intensity - f64::from(guests.intensity(guest))).abs()
                    - f64::from(self.guests[index].needs.nausea)
                    + f64::from(self.scenery_scores[index]))
                .clamp(-100.0, 100.0) as i8;
                let mut place = facility.place;
                place.intensity = nausea.clamp(0.0, 100.0) as u8;
                self.guests[index].complete_paid(place, self.tick, enjoyment, guests, guest);
                let mut kinds = Vec::new();
                for event in &proximity_events[seat] {
                    assert!(
                        event.position.x.is_finite()
                            && event.position.y.is_finite()
                            && event.position.z.is_finite()
                            && event.strength.is_finite()
                    );
                    if !kinds.contains(&event.kind) {
                        kinds.push(event.kind);
                        self.guests[index].record_memory(GuestMemoryDetail::Proximity {
                            ride_id: place.id,
                            kind: event.kind,
                            position: event.position,
                            strength: event.strength,
                            tick: self.tick,
                        });
                    }
                }
                if let Some(&kind) = kinds.last() {
                    self.guests[index].thought = Thought::Proximity(place.id, kind);
                }
            }
            self.activities[index] = facility
                .exit_position
                .map(|exit| Activity::Unloading(facility.place.id, exit));
        }
        riders
    }

    pub fn ride_state(&self, facility: &Facility) -> RideState {
        assert_eq!(facility.place.kind, PlaceKind::Ride);
        assert!(facility.duration_ticks > 0);
        let mut unloading = false;
        for activity in &self.activities {
            match activity {
                Some(Activity::Using(id, remaining)) if *id == facility.place.id => {
                    return RideState {
                        phase: RidePhase::Running,
                        progress: 1.0 - (*remaining as f32 / facility.duration_ticks as f32),
                    };
                }
                Some(Activity::Unloading(id, _)) if *id == facility.place.id => unloading = true,
                _ => {}
            }
        }
        if unloading {
            RideState {
                phase: RidePhase::Unloading,
                progress: 1.0,
            }
        } else {
            RideState {
                phase: RidePhase::Idle,
                progress: 0.0,
            }
        }
    }

    pub fn facility_income(&self, place_id: u32) -> u64 {
        self.income.get(&place_id).copied().unwrap_or(0)
    }

    pub fn activity_detail(&self, guest: GuestId) -> ActivityDetail {
        let index = guest.0 as usize;
        let activity = self.activities[index];
        let (kind, place_id) = match activity {
            Some(Activity::Walking(place_id)) => (ActivityKind::Walking, Some(place_id)),
            Some(Activity::Queuing(place_id, _)) => (ActivityKind::Queuing, Some(place_id)),
            Some(Activity::Using(place_id, _) | Activity::CoasterRiding(place_id)) => {
                let place_kind = self.guests[index]
                    .known_places()
                    .iter()
                    .find(|place| place.id == place_id)
                    .map(|place| place.kind);
                let kind = match place_kind {
                    Some(PlaceKind::Ride) => ActivityKind::Riding,
                    Some(PlaceKind::Food) => ActivityKind::Eating,
                    Some(PlaceKind::Drink) => ActivityKind::Drinking,
                    Some(PlaceKind::Restroom) => ActivityKind::Restroom,
                    Some(PlaceKind::Bench) => ActivityKind::Resting,
                    Some(PlaceKind::Souvenir) => ActivityKind::Shopping,
                    None => ActivityKind::Walking,
                };
                (kind, Some(place_id))
            }
            Some(Activity::Waiting(place_id)) => (ActivityKind::Waiting, Some(place_id)),
            Some(Activity::Unloading(place_id, _)) => (ActivityKind::Walking, Some(place_id)),
            None => (ActivityKind::Wandering, None),
        };
        let destination = if let Some(Activity::Unloading(_, exit)) = activity {
            Some(exit)
        } else {
            place_id.and_then(|place_id| {
                self.guests[index]
                    .known_places()
                    .iter()
                    .find(|place| place.id == place_id)
                    .map(|place| place.position)
            })
        };
        ActivityDetail {
            kind,
            destination,
            place_id,
        }
    }
    pub(super) fn observe_queue_entrances(
        &mut self,
        guests: &GuestStore,
        positions: &[Option<Position>],
        guest_components: &[Option<usize>],
        entrances: &[(KnownPlace, Position, usize)],
        exiting: &[bool],
    ) {
        if entrances.is_empty() {
            return;
        }
        for (index, position) in positions.iter().copied().enumerate() {
            let guest = GuestId(index as u32);
            if !guests.is_active(guest)
                || exiting[index]
                || index as u64 % 30 != (self.tick + 1) % 30
            {
                continue;
            }
            let Some(position) = position else {
                continue;
            };
            for &(place, entrance, component) in entrances {
                if guest_components[index] == Some(component)
                    && position.x.abs_diff(entrance.x) + position.y.abs_diff(entrance.y)
                        <= QUEUE_ENTRANCE_VISIBILITY_TILES
                    && (position.height - entrance.height).abs() <= 2
                {
                    self.guests[index].observe(place);
                }
            }
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
                let scenery = self.guests[index].recent_memories().back();
                let ride = guests.memory(id).iter().last();
                match (scenery, ride) {
                    (Some(GuestMemoryDetail::Scenery { theme, tick, .. }), ride)
                        if ride.is_none_or(|memory| *tick >= memory.tick) =>
                    {
                        self.guests[index].thought = Thought::EnjoyedScenery(*theme);
                    }
                    (_, Some(memory)) => {
                        self.guests[index].thought = if memory.enjoyment >= 0 {
                            Thought::EnjoyedRide(memory.ride_id)
                        } else {
                            Thought::DislikedRide(memory.ride_id)
                        }
                    }
                    _ => {}
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
                Some(
                    Activity::Using(place_id, _)
                    | Activity::Waiting(place_id)
                    | Activity::Unloading(place_id, _),
                ) if !by_id.contains_key(&place_id) => {
                    self.activities[index] = None;
                }
                Some(Activity::Using(place_id, remaining)) if remaining <= 1 => {
                    if let Some(facility) = by_id.get(&place_id) {
                        let enjoyment = (80_i16
                            - (i16::from(guests.intensity(id))
                                - i16::from(facility.place.intensity))
                            .abs()
                                * 2
                            - self.guests[index].needs.nausea as i16
                            + if facility.place.kind == PlaceKind::Ride {
                                i16::from(self.scenery_scores[index])
                            } else {
                                0
                            })
                        .clamp(-100, 100) as i8;
                        self.guests[index].complete_paid(
                            facility.place,
                            self.tick,
                            enjoyment,
                            guests,
                            id,
                        );
                    }
                    self.activities[index] = by_id[&place_id]
                        .exit_position
                        .map(|exit| Activity::Unloading(place_id, exit));
                }
                Some(Activity::Unloading(_, exit)) if positions[index] == Some(exit) => {
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
                    let facility = by_id[&place_id];
                    let queue = self.queues.entry(place_id).or_default();
                    if facility.place.kind != PlaceKind::Ride
                        && queue.len() >= facility.capacity * 2
                    {
                        self.activities[index] = None;
                    } else {
                        queue.push(id);
                        self.queue_scenery[index] = 0;
                        self.activities[index] = Some(Activity::Queuing(place_id, self.tick));
                    }
                }
                Some(Activity::Queuing(place_id, _))
                    if !by_id.contains_key(&place_id)
                        || guest_components[index].is_none()
                        || guest_components[index] != components[&place_id] =>
                {
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
                    let riders_remain = active.iter().any(|other| matches!(self.activities[other.0 as usize], Some(Activity::Walking(id) | Activity::Queuing(id, _) | Activity::Using(id, _) | Activity::CoasterRiding(id) | Activity::Unloading(id, _)) if id == place_id));
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
            for place in &mut available.known_places {
                if let Some(facility) = by_id.get(&place.id) {
                    *place = facility.place;
                }
            }
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
            queue.retain(|id| {
                matches!(
                    self.activities[id.0 as usize],
                    Some(Activity::Queuing(activity_place_id, _)) if activity_place_id == place_id
                )
            });
            if let Some(facility) = by_id.get(&place_id) {
                if self.external_rides.contains(&place_id) {
                    continue;
                }
                let occupied = self.activities.iter().filter(|activity| matches!(activity, Some(Activity::Using(id, _)) if *id == place_id)).count();
                let take = if facility.place.kind == PlaceKind::Ride && occupied > 0 {
                    0
                } else {
                    facility.capacity.saturating_sub(occupied).min(queue.len())
                };
                for id in queue.drain(..take) {
                    let behavior = &mut self.guests[id.0 as usize];
                    if behavior.money < facility.place.price {
                        self.activities[id.0 as usize] = None;
                        continue;
                    }
                    behavior.money -= facility.place.price;
                    behavior.lifetime_money_spent += u32::from(facility.place.price);
                    *self.income.entry(place_id).or_default() += u64::from(facility.place.price);
                    let queued_at = match self.activities[id.0 as usize] {
                        Some(Activity::Queuing(_, queued_at)) => queued_at,
                        _ => unreachable!("only queued guests are admitted"),
                    };
                    self.guests[id.0 as usize].record_memory(GuestMemoryDetail::Wait {
                        place_id,
                        duration_ticks: self.tick.saturating_sub(queued_at),
                        perceived_duration_ticks: self
                            .tick
                            .saturating_sub(queued_at)
                            .saturating_sub(self.queue_scenery[id.0 as usize] / 40),
                        tick: self.tick,
                    });
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
                Some(Activity::Unloading(_, exit)) => Movement::Destination(*exit),
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

fn write_activity(hash: &mut StateHasher, activity: Activity) {
    match activity {
        Activity::Walking(place_id) => {
            hash.write_u8(0);
            hash.write_u32(place_id);
        }
        Activity::Queuing(place_id, tick) => {
            hash.write_u8(1);
            hash.write_u32(place_id);
            hash.write_u64(tick);
        }
        Activity::Using(place_id, remaining) => {
            hash.write_u8(2);
            hash.write_u32(place_id);
            hash.write_u32(remaining);
        }
        Activity::CoasterRiding(place_id) => {
            hash.write_u8(3);
            hash.write_u32(place_id);
        }
        Activity::Unloading(place_id, position) => {
            hash.write_u8(4);
            hash.write_u32(place_id);
            write_position(hash, position);
        }
        Activity::Waiting(place_id) => {
            hash.write_u8(5);
            hash.write_u32(place_id);
        }
    }
}

fn write_position(hash: &mut StateHasher, position: Position) {
    hash.write_u16(position.x);
    hash.write_u16(position.y);
    hash.write_i16(position.height);
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
    fn scenery_changes_queue_perception_and_flat_and_coaster_enjoyment() {
        use crate::world::{Scenery, SceneryKind, Surface, World};
        let seed = (0..100)
            .find(|&seed| generate_guests(seed, 1).cares_about_theming(GuestId(0)) >= 80)
            .unwrap();
        let guest = GuestId(0);
        let mut guests = generate_guests(seed, 1);
        let position = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        let mut world = World::new(
            4,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let kind = if guests.preferred_theme(guest) == 0 {
            SceneryKind::FlowerBed
        } else {
            SceneryKind::TreeCone
        };
        let mut simulation = BehaviorSimulation::new(1);
        simulation.activities[0] = Some(Activity::Queuing(1, 0));
        world
            .place_scenery(Scenery {
                kind,
                position: Position {
                    x: 1,
                    y: 0,
                    height: 0,
                },
                quarter: 0,
                rotation: 0,
            })
            .unwrap();
        simulation.observe_scenery(&world, &guests, &[Some(position)], &[false]);
        let first_score = simulation.scenery_scores[0];
        for x in 2..=3 {
            world
                .place_scenery(Scenery {
                    kind,
                    position: Position { x, y: 0, height: 0 },
                    quarter: 0,
                    rotation: 0,
                })
                .unwrap();
        }
        for tick in 0..100 {
            simulation.tick = tick;
            simulation.observe_scenery(&world, &guests, &[Some(position)], &[false]);
        }
        let score = simulation.scenery_scores[0];
        assert!(score > first_score && score < first_score * 3);
        let facility = Facility {
            place: KnownPlace {
                position,
                intensity: guests.intensity(guest),
                ..place(1, PlaceKind::Ride)
            },
            capacity: 1,
            duration_ticks: 1,
            exit_position: None,
        };
        simulation.tick = 100;
        simulation.queues.insert(1, vec![guest]);
        simulation.set_external_rides(&[1]);
        assert_eq!(simulation.board_coaster(&facility, &guests), vec![guest]);
        assert!(
            matches!(simulation.guests[0].recent_memories().back(), Some(GuestMemoryDetail::Wait { duration_ticks: 100, perceived_duration_ticks, .. }) if *perceived_duration_ticks < 100)
        );
        simulation.observe_scenery(&world, &guests, &[None], &[false]);
        assert_eq!(simulation.scenery_scores[0], score);
        simulation.finish_coaster(
            &facility,
            &mut guests,
            &[(50.0, f64::from(facility.place.intensity), 0.0)],
            &[vec![]],
            false,
        );
        assert_eq!(
            guests.memory(guest).iter().last().unwrap().enjoyment,
            50 + score as i8
        );

        let mut flat_guests = generate_guests(seed, 1);
        let mut flat = BehaviorSimulation::new(1);
        flat.activities[0] = Some(Activity::Using(1, 1));
        flat.observe_scenery(&world, &flat_guests, &[Some(position)], &[false]);
        flat.tick(
            &mut flat_guests,
            &[Some(position)],
            &[facility],
            &[Some(0)],
            &[Some(0)],
        );
        assert_eq!(
            flat_guests.memory(guest).iter().last().unwrap().enjoyment,
            80 + score as i8
        );
    }

    #[test]
    fn guest_explores_a_similar_unridden_ride_after_an_enjoyable_ride() {
        let mut guests = generate_guests(31, 1);
        let guest = GuestId(0);
        let mut behavior = GuestBehavior::default();
        let first = KnownPlace {
            intensity: guests.intensity(guest),
            ..place(1, PlaceKind::Ride)
        };
        let second = KnownPlace { id: 2, ..first };
        behavior.observe(first);
        behavior.observe(second);
        behavior.complete(first, 0, 80, &mut guests, guest);
        assert_eq!(behavior.choose(&guests, guest), Some(second));
    }

    #[test]
    fn guest_can_repeat_a_ride_and_prioritizes_urgent_needs() {
        let mut guests = generate_guests(31, 1);
        let guest = GuestId(0);
        let mut behavior = GuestBehavior::default();
        let ride = KnownPlace {
            intensity: guests.intensity(guest),
            ..place(1, PlaceKind::Ride)
        };
        behavior.observe(ride);
        behavior.complete(ride, 0, 80, &mut guests, guest);
        assert_eq!(behavior.choose(&guests, guest), Some(ride));
        let food = place(2, PlaceKind::Food);
        behavior.observe(food);
        behavior.needs.hunger = 100;
        assert_eq!(behavior.choose(&guests, guest), Some(food));
    }

    #[test]
    fn queue_entrance_is_visible_from_nearby_paths_only() {
        let guests = generate_guests(31, 1);
        let ride = place(1, PlaceKind::Ride);
        let entrance = Position {
            x: 12,
            y: 0,
            height: 0,
        };
        let mut simulation = BehaviorSimulation::new(1);
        simulation.tick = 29;
        simulation.observe_queue_entrances(
            &guests,
            &[Some(Position {
                x: 0,
                y: 0,
                height: 0,
            })],
            &[Some(0)],
            &[(ride, entrance, 0)],
            &[false],
        );
        assert_eq!(simulation.guests[0].known_places(), &[ride]);
        let mut far = BehaviorSimulation::new(1);
        far.tick = 29;
        far.observe_queue_entrances(
            &guests,
            &[Some(Position {
                x: 0,
                y: 0,
                height: 0,
            })],
            &[Some(0)],
            &[(ride, Position { x: 13, ..entrance }, 0)],
            &[false],
        );
        assert!(far.guests[0].known_places().is_empty());
    }

    #[test]
    fn externally_driven_coaster_boards_and_records_force_outcomes() {
        let mut guests = generate_guests(31, 2);
        let mut simulation = BehaviorSimulation::new(2);
        let ride = KnownPlace {
            price: 6,
            ..place(1, PlaceKind::Ride)
        };
        let facility = Facility {
            place: ride,
            capacity: 2,
            duration_ticks: 3,
            exit_position: Some(ride.position),
        };
        simulation.set_external_rides(&[ride.id]);
        for index in 0..2 {
            simulation.guests[index].observe(ride);
            simulation.activities[index] = Some(Activity::Queuing(ride.id, 0));
        }
        simulation
            .queues
            .insert(ride.id, vec![GuestId(0), GuestId(1)]);
        simulation.tick(
            &mut guests,
            &[Some(ride.position); 2],
            &[facility],
            &[Some(0); 2],
            &[Some(0)],
        );
        assert_eq!(
            simulation.board_coaster(&facility, &guests),
            vec![GuestId(0), GuestId(1)]
        );
        assert_eq!(simulation.guests[0].money, 94);
        assert_eq!(simulation.facility_income(ride.id), 12);
        assert_eq!(
            simulation.activity_detail(GuestId(0)).kind,
            ActivityKind::Riding
        );
        use crate::track::Vec3;
        use crate::track::proximity::ProximityKind;
        let event = ProximityEvent {
            piece_index: 0,
            kind: ProximityKind::Tunnel,
            position: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            distance: 0.0,
            strength: 1.0,
        };
        simulation.finish_coaster(
            &facility,
            &mut guests,
            &[(100.0, 10.0, 0.0), (10.0, 100.0, 100.0)],
            &[vec![event, event], vec![]],
            false,
        );
        assert_eq!(
            simulation.guests[0]
                .recent_memories()
                .iter()
                .filter(|memory| matches!(
                    memory,
                    GuestMemoryDetail::Proximity {
                        ride_id: 1,
                        kind: ProximityKind::Tunnel,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert!(
            simulation.guests[1]
                .recent_memories()
                .iter()
                .all(|memory| !matches!(memory, GuestMemoryDetail::Proximity { .. }))
        );
        assert_eq!(
            simulation.guests[0].thought,
            Thought::Proximity(1, ProximityKind::Tunnel)
        );
        let first = *guests.memory(GuestId(0)).iter().last().unwrap();
        let second = *guests.memory(GuestId(1)).iter().last().unwrap();
        assert_ne!(first.enjoyment, second.enjoyment);
        assert!(second.nausea > first.nausea);
        assert_eq!(
            simulation.activity_detail(GuestId(0)).kind,
            ActivityKind::Walking
        );
    }

    #[test]
    fn coaster_train_cohorts_finish_independently() {
        let mut guests = generate_guests(31, 2);
        let mut simulation = BehaviorSimulation::new(2);
        let ride = place(1, PlaceKind::Ride);
        let facility = Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 3,
            exit_position: Some(ride.position),
        };
        simulation.set_external_rides(&[ride.id]);
        for index in 0..2 {
            simulation.guests[index].observe(ride);
            simulation.activities[index] = Some(Activity::Queuing(ride.id, 0));
        }
        simulation
            .queues
            .insert(ride.id, vec![GuestId(0), GuestId(1)]);

        assert_eq!(
            simulation.board_coaster_train(&facility, 11, &guests),
            vec![GuestId(0)]
        );
        assert_eq!(
            simulation.board_coaster_train(&facility, 22, &guests),
            vec![GuestId(1)]
        );

        assert_eq!(
            simulation.finish_coaster_train(&facility, 11, &mut guests, &[], &[], true),
            vec![GuestId(0)]
        );
        assert!(!simulation.is_coaster_riding(GuestId(0)));
        assert!(simulation.is_coaster_riding(GuestId(1)));
        assert!(guests.memory(GuestId(0)).is_empty());
        assert!(guests.memory(GuestId(1)).is_empty());

        assert_eq!(
            simulation.finish_coaster_train(
                &facility,
                22,
                &mut guests,
                &[(50.0, 20.0, 0.0)],
                &[vec![]],
                false,
            ),
            vec![GuestId(1)]
        );
        assert!(!simulation.is_coaster_riding(GuestId(1)));
        assert_eq!(guests.memory(GuestId(1)).len(), 1);
    }

    #[test]
    fn zero_price_coaster_boards_riders_without_income() {
        let guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let ride = place(1, PlaceKind::Ride);
        let facility = Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 3,
            exit_position: None,
        };
        simulation.set_external_rides(&[ride.id]);
        simulation.activities[0] = Some(Activity::Queuing(ride.id, 0));
        simulation.queues.insert(ride.id, vec![GuestId(0)]);

        assert_eq!(
            simulation.board_coaster(&facility, &guests),
            vec![GuestId(0)]
        );
        assert_eq!(simulation.guests[0].money, 100);
        assert_eq!(simulation.facility_income(ride.id), 0);
    }

    #[test]
    fn proximity_memories_remain_bounded() {
        use crate::track::Vec3;
        use crate::track::proximity::ProximityKind;

        let mut behavior = GuestBehavior::default();
        for tick in 0..12 {
            behavior.record_memory(GuestMemoryDetail::Proximity {
                ride_id: 1,
                kind: ProximityKind::Tunnel,
                position: Vec3::new(1.0, 2.0, 3.0),
                strength: 0.5,
                tick,
            });
        }
        assert_eq!(behavior.recent_memories().len(), 8);
        assert_eq!(behavior.recent_memories().front().unwrap().tick(), 4);
    }

    #[test]
    fn group_larger_than_train_can_board_without_starving() {
        let guests = generate_guests(31, 40);
        let group = guests
            .groups
            .iter()
            .find(|group| group.members.len() > 1)
            .unwrap();
        let ride = place(1, PlaceKind::Ride);
        let facility = Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 3,
            exit_position: None,
        };
        let mut simulation = BehaviorSimulation::new(40);
        simulation.set_external_rides(&[ride.id]);
        for &guest in &group.members {
            simulation.activities[guest.0 as usize] = Some(Activity::Queuing(ride.id, 0));
        }
        simulation.queues.insert(ride.id, group.members.clone());
        let riders = simulation.board_coaster(&facility, &guests);
        assert_eq!(riders, vec![group.members[0]]);
        assert_eq!(simulation.queues[&ride.id].len(), group.members.len() - 1);
    }

    #[test]
    fn ride_admits_a_new_cohort_only_after_the_current_cycle_finishes() {
        let mut guests = generate_guests(31, 2);
        let mut simulation = BehaviorSimulation::new(2);
        let ride = KnownPlace {
            price: 7,
            ..place(1, PlaceKind::Ride)
        };
        let facilities = [Facility {
            place: ride,
            capacity: 2,
            exit_position: None,
            duration_ticks: 3,
        }];
        simulation.activities[0] = Some(Activity::Using(ride.id, 2));
        simulation.activities[1] = Some(Activity::Queuing(ride.id, 0));
        simulation.queues.insert(ride.id, vec![GuestId(1)]);
        simulation.tick(
            &mut guests,
            &[Some(ride.position); 2],
            &facilities,
            &[Some(0); 2],
            &[Some(0)],
        );
        assert!(matches!(
            simulation.activities[1],
            Some(Activity::Queuing(_, _))
        ));
        simulation.tick(
            &mut guests,
            &[Some(ride.position); 2],
            &facilities,
            &[Some(0); 2],
            &[Some(0)],
        );
        assert!(matches!(
            simulation.activities[1],
            Some(Activity::Using(_, 3))
        ));
        assert_eq!(simulation.facility_income(ride.id), 7);
    }

    #[test]
    fn current_price_controls_choice_and_admission() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let old = place(1, PlaceKind::Ride);
        simulation.guests[0].observe(old);
        simulation.guests[0].money = 5;
        let ride = KnownPlace { price: 8, ..old };
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            exit_position: None,
            duration_ticks: 2,
        }];
        simulation.tick = 29;
        simulation.tick(
            &mut guests,
            &[Some(ride.position)],
            &facilities,
            &[Some(0)],
            &[Some(0)],
        );
        assert!(simulation.guests[0].cannot_afford_activity);
        assert_eq!(simulation.facility_income(ride.id), 0);
    }

    #[test]
    fn souvenir_purchase_records_income_and_improves_happiness() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let shop = KnownPlace {
            price: 7,
            ..place(1, PlaceKind::Souvenir)
        };
        let facilities = [Facility {
            place: shop,
            capacity: 1,
            exit_position: None,
            duration_ticks: 1,
        }];
        simulation.guests[0].rides_completed = 1;
        simulation.guests[0].happiness = 50;
        simulation.tick = 29;
        for _ in 0..3 {
            simulation.tick(
                &mut guests,
                &[Some(shop.position)],
                &facilities,
                &[Some(0)],
                &[Some(0)],
            );
        }
        assert_eq!(simulation.guests[0].money, 93);
        assert_eq!(simulation.facility_income(shop.id), 7);
        assert_eq!(simulation.guests[0].happiness, 55);
    }

    #[test]
    fn ride_state_reports_running_progress_and_unloading() {
        let ride = place(1, PlaceKind::Ride);
        let exit = Position {
            x: 2,
            y: 0,
            height: 0,
        };
        let facility = Facility {
            place: ride,
            capacity: 2,
            duration_ticks: 4,
            exit_position: Some(exit),
        };
        let mut simulation = BehaviorSimulation::new(1);
        assert_eq!(
            simulation.ride_state(&facility),
            RideState {
                phase: RidePhase::Idle,
                progress: 0.0
            }
        );
        simulation.activities[0] = Some(Activity::Using(ride.id, 3));
        assert_eq!(
            simulation.ride_state(&facility),
            RideState {
                phase: RidePhase::Running,
                progress: 0.25
            }
        );
        simulation.activities[0] = Some(Activity::Unloading(ride.id, exit));
        assert_eq!(
            simulation.ride_state(&facility),
            RideState {
                phase: RidePhase::Unloading,
                progress: 1.0
            }
        );
    }

    #[test]
    fn completed_ride_moves_guest_to_exit_before_releasing_them() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let ride = place(1, PlaceKind::Ride);
        let exit = Position {
            x: 2,
            y: 0,
            height: 0,
        };
        let facilities = [Facility {
            place: ride,
            capacity: 1,
            duration_ticks: 1,
            exit_position: Some(exit),
        }];
        simulation.activities[0] = Some(Activity::Using(ride.id, 1));
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(ride.position)],
                &facilities,
                &[Some(0)],
                &[Some(0)]
            )[0],
            Movement::Destination(exit)
        );
        assert_eq!(
            simulation.tick(
                &mut guests,
                &[Some(exit)],
                &facilities,
                &[Some(0)],
                &[Some(0)]
            )[0],
            Movement::Wander
        );
        assert_eq!(simulation.guests[0].rides_completed, 1);
    }

    #[test]
    fn removing_queued_ride_clears_the_queue() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let ride = place(1, PlaceKind::Ride);
        simulation.activities[0] = Some(Activity::Queuing(ride.id, 0));
        simulation.queues.insert(ride.id, vec![GuestId(0)]);
        assert_eq!(
            simulation.tick(&mut guests, &[Some(ride.position)], &[], &[Some(0)], &[])[0],
            Movement::Wander
        );
        assert!(simulation.queues[&ride.id].is_empty());
    }

    #[test]
    fn removing_occupied_ride_cancels_without_sending_guest_to_missing_exit() {
        let mut guests = generate_guests(31, 1);
        let mut simulation = BehaviorSimulation::new(1);
        let ride = place(1, PlaceKind::Ride);
        simulation.activities[0] = Some(Activity::Using(ride.id, 1));
        assert_eq!(
            simulation.tick(&mut guests, &[Some(ride.position)], &[], &[Some(0)], &[])[0],
            Movement::Wander
        );
        assert_eq!(simulation.guests[0].rides_completed, 0);
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
            exit_position: None,
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
