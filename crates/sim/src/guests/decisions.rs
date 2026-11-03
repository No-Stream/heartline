//! Guest knowledge, needs, and activity choices.
use std::collections::{BTreeMap, VecDeque};

use super::details::GuestMemoryDetail;
use super::knowledge::{KnownPlace, PlaceKind};
use super::needs::Needs;
use super::thoughts::Thought;
use crate::guests::{AgeGroup, GuestId, GuestStore, MemoryEntry, SpendingLevel};

const RIDE_MEMORY_BONUS_LIMIT: i32 = 20;
const UNRIDDEN_RIDE_BONUS: i32 = 30;
const LAST_RIDE_PENALTY: i32 = 30;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestBehavior {
    pub needs: Needs,
    pub rides_completed: u16,
    pub stops_completed: u16,
    pub energy: u16,
    pub happiness: u16,
    pub money: u16,
    pub lifetime_money_spent: u32,
    pub visit_number: u32,
    pub thought: Thought,
    pub cannot_afford_activity: bool,
    pub souvenir_purchased: bool,
    pub(super) known_places: Vec<KnownPlace>,
    recent_memories: VecDeque<GuestMemoryDetail>,
    pub(super) elapsed_ticks: u64,
}

impl Default for GuestBehavior {
    fn default() -> Self {
        Self {
            needs: Needs::default(),
            rides_completed: 0,
            stops_completed: 0,
            energy: 100,
            happiness: 80,
            money: 100,
            lifetime_money_spent: 0,
            visit_number: 0,
            thought: Thought::Exploring,
            cannot_afford_activity: false,
            souvenir_purchased: false,
            known_places: Vec::new(),
            recent_memories: VecDeque::with_capacity(8),
            elapsed_ticks: 0,
        }
    }
}

impl GuestBehavior {
    pub fn known_places(&self) -> &[KnownPlace] {
        &self.known_places
    }
    pub fn recent_memories(&self) -> &VecDeque<GuestMemoryDetail> {
        &self.recent_memories
    }
    pub(super) fn record_memory(&mut self, memory: GuestMemoryDetail) {
        if self.recent_memories.len() == 8 {
            self.recent_memories.pop_front();
        }
        self.recent_memories.push_back(memory);
    }
    pub fn observe(&mut self, place: KnownPlace) {
        if let Some(known) = self
            .known_places
            .iter_mut()
            .find(|known| known.id == place.id)
        {
            *known = place;
        } else {
            if self.known_places.len() == 32 {
                self.known_places.remove(0);
            }
            self.known_places.push(place);
        }
    }
    pub fn hear_from(&mut self, other: &Self) {
        for &place in &other.known_places {
            self.observe(place);
        }
    }
    pub fn advance(&mut self, ticks: u32) {
        self.advance_for(ticks, AgeGroup::Adult);
    }
    pub(super) fn advance_for(&mut self, ticks: u32, age: AgeGroup) {
        let old = self.elapsed_ticks;
        self.elapsed_ticks += u64::from(ticks);
        let increase =
            |period: u64| ((self.elapsed_ticks / period - old / period).min(1000)) as u16;
        let hunger_period = if age == AgeGroup::Child { 450 } else { 540 };
        let restroom_period = if age == AgeGroup::Senior { 360 } else { 450 };
        self.needs.hunger = self
            .needs
            .hunger
            .saturating_add(increase(hunger_period))
            .min(100);
        self.needs.thirst = self.needs.thirst.saturating_add(increase(360)).min(100);
        self.needs.restroom = self
            .needs
            .restroom
            .saturating_add(increase(restroom_period))
            .min(100);
        self.needs.nausea = self.needs.nausea.saturating_sub(increase(900));
        self.energy = self.energy.saturating_sub(increase(900));
        if self.needs.hunger >= 80
            || self.needs.thirst >= 80
            || self.needs.restroom >= 80
            || self.needs.nausea >= 80
        {
            self.happiness = self.happiness.saturating_sub(increase(900));
        }
        self.thought = if self.happiness < 20 {
            Thought::Unhappy
        } else if self.energy < 25 {
            Thought::Tired
        } else if self.needs.nausea >= 70 {
            Thought::Nauseous
        } else if self.needs.restroom >= 70 {
            Thought::NeedsRestroom
        } else if self.needs.thirst >= 70 {
            Thought::Thirsty
        } else if self.needs.hunger >= 70 {
            Thought::Hungry
        } else {
            Thought::Exploring
        };
    }
    pub fn choose(&self, guests: &GuestStore, guest: GuestId) -> Option<KnownPlace> {
        self.choose_with_queues(guests, guest, &BTreeMap::new())
    }
    pub(super) fn choose_with_queues(
        &self,
        guests: &GuestStore,
        guest: GuestId,
        queues: &BTreeMap<u32, usize>,
    ) -> Option<KnownPlace> {
        self.known_places
            .iter()
            .copied()
            .filter(|place| {
                place.price <= self.money
                    && (place.kind != PlaceKind::Ride
                        || guests.height_cm(guest) >= place.minimum_height_cm)
            })
            .max_by_key(|place| {
                let score = match place.kind {
                    PlaceKind::Food => i32::from(self.needs.hunger) * 4,
                    PlaceKind::Drink => i32::from(self.needs.thirst) * 4 + 1,
                    PlaceKind::Restroom => i32::from(self.needs.restroom) * 4 + 2,
                    PlaceKind::Bench => i32::from(100 - self.energy) * 4,
                    PlaceKind::Souvenir => {
                        if self.rides_completed > 0 && !self.souvenir_purchased {
                            190
                        } else {
                            0
                        }
                    }
                    PlaceKind::Ride => {
                        180 - (i32::from(guests.intensity(guest)) - i32::from(place.intensity))
                            .abs()
                            - i32::from(self.needs.nausea) * 2
                            + if guests.favourite_ride_types(guest) & (1 << place.ride_type) != 0 {
                                20
                            } else {
                                0
                            }
                    }
                };
                let budget = match guests.spending(guest) {
                    SpendingLevel::Budget => 5,
                    SpendingLevel::Regular => 10,
                    SpendingLevel::Premium => 20,
                };
                let price_penalty = i32::from(place.price.saturating_sub(budget)) * 5;
                let theme_bonus = if place.theme == guests.preferred_theme(guest) {
                    i32::from(guests.cares_about_theming(guest)) / 5
                } else {
                    0
                };
                let queue_penalty = queues.get(&place.id).copied().unwrap_or(0).min(100) as i32 * 5;
                let ride_memory = guests
                    .memory(guest)
                    .iter()
                    .filter(|memory| memory.ride_id == place.id)
                    .last();
                let memory_bonus = ride_memory.map_or(0, |memory| {
                    i32::from(memory.enjoyment)
                        .clamp(-RIDE_MEMORY_BONUS_LIMIT, RIDE_MEMORY_BONUS_LIMIT)
                });
                let novelty_bonus = if place.kind == PlaceKind::Ride
                    && self.rides_completed > 0
                    && ride_memory.is_none()
                {
                    UNRIDDEN_RIDE_BONUS
                } else {
                    0
                };
                let repeat_penalty = if place.kind == PlaceKind::Ride
                    && guests
                        .memory(guest)
                        .iter()
                        .last()
                        .is_some_and(|memory| memory.ride_id == place.id)
                {
                    LAST_RIDE_PENALTY
                } else {
                    0
                };
                (
                    score + theme_bonus + memory_bonus + novelty_bonus
                        - repeat_penalty
                        - price_penalty
                        - queue_penalty,
                    std::cmp::Reverse(place.id),
                )
            })
    }
    pub fn complete(
        &mut self,
        place: KnownPlace,
        tick: u64,
        enjoyment: i8,
        guests: &mut GuestStore,
        guest: GuestId,
    ) {
        let money_before = self.money;
        self.money = self.money.saturating_sub(place.price);
        self.lifetime_money_spent += u32::from(money_before - self.money);
        self.complete_paid(place, tick, enjoyment, guests, guest);
    }

    pub(super) fn complete_paid(
        &mut self,
        place: KnownPlace,
        tick: u64,
        enjoyment: i8,
        guests: &mut GuestStore,
        guest: GuestId,
    ) {
        match place.kind {
            PlaceKind::Ride => {
                self.happiness = self.happiness.saturating_add(5).min(100);
                self.thought = if enjoyment >= 0 {
                    Thought::EnjoyedRide(place.id)
                } else {
                    Thought::DislikedRide(place.id)
                };
                self.rides_completed = self.rides_completed.saturating_add(1);
                let nausea = place
                    .intensity
                    .saturating_sub(guests.nausea_tolerance(guest))
                    / 4;
                self.needs.nausea = self.needs.nausea.saturating_add(u16::from(nausea)).min(100);
                guests.record_memory(
                    guest,
                    MemoryEntry {
                        ride_id: place.id,
                        enjoyment,
                        nausea,
                        tick,
                    },
                );
            }
            PlaceKind::Food => {
                self.needs.hunger = 0;
                self.stops_completed = self.stops_completed.saturating_add(1);
                self.record_memory(GuestMemoryDetail::Food {
                    place_id: place.id,
                    tick,
                });
            }
            PlaceKind::Drink => {
                self.needs.thirst = 0;
                self.stops_completed = self.stops_completed.saturating_add(1);
            }
            PlaceKind::Restroom => {
                self.needs.restroom = 0;
                self.stops_completed = self.stops_completed.saturating_add(1);
            }
            PlaceKind::Souvenir => {
                self.souvenir_purchased = true;
                self.happiness = self.happiness.saturating_add(5).min(100);
                self.stops_completed = self.stops_completed.saturating_add(1);
            }
            PlaceKind::Bench => {
                self.energy = 100;
                self.stops_completed = self.stops_completed.saturating_add(1);
            }
        }
    }
    pub fn visit_complete(&self) -> bool {
        (self.rides_completed >= 12 && self.stops_completed >= 3)
            || self.energy == 0
            || self.happiness == 0
            || self.cannot_afford_activity
    }
    pub fn reset_visit(&mut self) {
        self.needs = Needs::default();
        self.rides_completed = 0;
        self.stops_completed = 0;
        self.elapsed_ticks = 0;
        self.energy = 100;
        self.happiness = 80;
        self.money = 100;
        self.visit_number = self.visit_number.saturating_add(1);
        self.thought = Thought::Exploring;
        self.cannot_afford_activity = false;
        self.souvenir_purchased = false;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSplit {
    pub riding: Vec<GuestId>,
    pub waiting: Vec<GuestId>,
}

pub fn split_for_ride(guests: &GuestStore, members: &[GuestId], ride: KnownPlace) -> GroupSplit {
    let (mut riding, mut waiting): (Vec<_>, Vec<_>) = members
        .iter()
        .copied()
        .partition(|&guest| guests.height_cm(guest) >= ride.minimum_height_cm);
    if waiting
        .iter()
        .any(|&guest| guests.age(guest) == AgeGroup::Child)
        && !waiting
            .iter()
            .any(|&guest| guests.age(guest) != AgeGroup::Child)
    {
        if let Some(index) = riding
            .iter()
            .position(|&guest| guests.age(guest) != AgeGroup::Child)
        {
            waiting.push(riding.remove(index));
        }
    }
    GroupSplit { riding, waiting }
}
