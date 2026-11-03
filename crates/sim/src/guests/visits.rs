//! Arrival targets and activity-based visit departures for a bounded guest roster.

use crate::guest_behavior::GuestBehavior;
use crate::guests::{GuestId, GuestStore};

/// Manages visits over the fixed roster in a `GuestStore`.
///
/// Appeal is a percentage of the roster; the sandbox override is an absolute
/// population target. Targets control arrivals, never interrupt ongoing visits.
/// The caller advances behavior and records activities before processing departures.
/// Groups arrive and leave together. Groups larger than the arrival rate may
/// arrive in one tick, but never if they would exceed the population target.
/// A target may be unreachable when no inactive group fits the remaining space.
pub struct VisitManager {
    roster_size: usize,
    active_count: usize,
    next_candidate: usize,
    arrivals_per_tick: usize,
}

impl VisitManager {
    pub fn new(guests: &GuestStore, arrivals_per_tick: usize) -> Self {
        assert!(arrivals_per_tick > 0, "arrival rate must be positive");
        assert!(
            guests.groups.iter().all(|group| group
                .members
                .iter()
                .all(|&id| guests.is_active(id) == guests.is_active(group.members[0]))),
            "group members must share visit status"
        );
        let roster_size = guests.ids().count();
        let active_count = guests.ids().filter(|&id| guests.is_active(id)).count();
        Self {
            roster_size,
            active_count,
            next_candidate: 0,
            arrivals_per_tick,
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_count
    }

    pub fn arrivals_per_tick(&self) -> usize {
        self.arrivals_per_tick
    }

    /// Admits guests up to the target and returns their IDs in admission order.
    pub fn arrive(
        &mut self,
        guests: &mut GuestStore,
        behaviors: &mut [GuestBehavior],
        appeal: u8,
        population_override: Option<usize>,
    ) -> Vec<GuestId> {
        self.validate_roster(guests, behaviors.len());
        assert!(appeal <= 100, "appeal must be a percentage");
        assert!(
            population_override.is_none_or(|target| target <= self.roster_size),
            "population target exceeds roster"
        );
        let target = population_override.unwrap_or(self.roster_size * usize::from(appeal) / 100);
        let mut admitted = Vec::new();
        let group_count = guests.groups.len();
        for _ in 0..group_count {
            let group_index = self.next_candidate;
            self.next_candidate = (self.next_candidate + 1) % group_count;
            let members = &guests.groups[group_index].members;
            let group_size = members.len();
            if guests.is_active(members[0]) || self.active_count + group_size > target {
                continue;
            }
            if !admitted.is_empty() && admitted.len() + group_size > self.arrivals_per_tick {
                continue;
            }
            let members = members.clone();
            for id in members {
                guests.mark_returned(id);
                behaviors[id.0 as usize].reset_visit();
                admitted.push(id);
                self.active_count += 1;
            }
            if admitted.len() >= self.arrivals_per_tick {
                break;
            }
        }
        admitted
    }

    /// Ends completed visits; no clock or day-night boundary ends a visit.
    pub fn depart_completed(
        &mut self,
        guests: &mut GuestStore,
        behaviors: &[GuestBehavior],
    ) -> Vec<GuestId> {
        self.depart_ready(guests, behaviors, &vec![true; self.roster_size])
    }

    /// Ends complete group visits only when every member is ready to leave.
    pub fn depart_ready(
        &mut self,
        guests: &mut GuestStore,
        behaviors: &[GuestBehavior],
        ready: &[bool],
    ) -> Vec<GuestId> {
        self.validate_roster(guests, behaviors.len());
        assert_eq!(ready.len(), self.roster_size);
        let completed: Vec<_> = guests
            .groups
            .iter()
            .filter(|group| {
                guests.is_active(group.members[0])
                    && group
                        .members
                        .iter()
                        .any(|id| behaviors[id.0 as usize].visit_complete())
                    && group.members.iter().all(|id| ready[id.0 as usize])
            })
            .flat_map(|group| group.members.iter().copied())
            .collect();
        for &id in &completed {
            guests.mark_left(id);
        }
        self.active_count -= completed.len();
        completed
    }

    fn validate_roster(&self, guests: &GuestStore, behavior_count: usize) {
        assert_eq!(
            guests.ids().count(),
            self.roster_size,
            "guest roster changed"
        );
        assert_eq!(
            behavior_count, self.roster_size,
            "one behavior is required per guest"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guests::{GuestId, MemoryEntry, generate_guests};

    fn empty_park(count: usize) -> (GuestStore, Vec<GuestBehavior>, VisitManager) {
        let mut guests = generate_guests(17, count);
        for id in guests.ids().collect::<Vec<_>>() {
            guests.mark_left(id);
        }
        let behaviors = (0..count).map(|_| GuestBehavior::default()).collect();
        let manager = VisitManager::new(&guests, 3);
        (guests, behaviors, manager)
    }

    #[test]
    fn arrivals_follow_appeal_and_override_without_forcing_guests_out() {
        let (mut guests, mut behaviors, mut manager) = empty_park(20);
        for _ in 0..10 {
            manager.arrive(&mut guests, &mut behaviors, 50, None);
        }
        assert_eq!(manager.active_count(), 10);
        for _ in 0..10 {
            manager.arrive(&mut guests, &mut behaviors, 0, Some(16));
        }
        assert_eq!(manager.active_count(), 16);
        manager.arrive(&mut guests, &mut behaviors, 0, Some(0));
        assert_eq!(manager.active_count(), 16);
    }

    #[test]
    fn visits_end_after_activities_and_return_with_memory() {
        let (mut guests, mut behaviors, mut manager) = empty_park(1);
        let id = GuestId(0);
        manager.arrive(&mut guests, &mut behaviors, 100, None);
        guests.record_memory(
            id,
            MemoryEntry {
                ride_id: 7,
                enjoyment: 5,
                nausea: 1,
                tick: 9,
            },
        );
        for _ in 0..10_000 {
            manager.depart_completed(&mut guests, &behaviors);
        }
        assert!(guests.is_active(id));
        behaviors[0].rides_completed = 12;
        behaviors[0].stops_completed = 3;
        manager.depart_completed(&mut guests, &behaviors);
        assert!(!guests.is_active(id));
        manager.arrive(&mut guests, &mut behaviors, 100, None);
        assert!(guests.is_active(id));
        assert_eq!(guests.memory(id).iter().next().unwrap().ride_id, 7);
        assert_eq!(behaviors[0].rides_completed, 0);
        assert_eq!(behaviors[0].stops_completed, 0);
    }

    #[test]
    fn groups_arrive_and_depart_together() {
        let (mut guests, mut behaviors, mut manager) = empty_park(100);
        let family = guests
            .groups
            .iter()
            .find(|group| group.members.len() >= 3)
            .unwrap()
            .members
            .clone();
        for _ in 0..100 {
            manager.arrive(&mut guests, &mut behaviors, 100, None);
        }
        for &id in &family {
            assert!(guests.is_active(id));
        }
        behaviors[family[0].0 as usize].rides_completed = 12;
        behaviors[family[0].0 as usize].stops_completed = 3;
        manager.depart_completed(&mut guests, &behaviors);
        assert!(family.iter().all(|&id| !guests.is_active(id)));
    }

    #[test]
    fn admissions_are_deterministic_and_rate_limited() {
        let (mut first, mut first_behaviors, mut first_manager) = empty_park(12);
        let (mut second, mut second_behaviors, mut second_manager) = empty_park(12);
        let largest_group = first
            .groups
            .iter()
            .map(|group| group.members.len())
            .max()
            .unwrap();
        for _ in 0..12 {
            let first_admissions =
                first_manager.arrive(&mut first, &mut first_behaviors, 100, None);
            let second_admissions =
                second_manager.arrive(&mut second, &mut second_behaviors, 100, None);
            assert!(first_admissions.len() <= 3_usize.max(largest_group));
            assert_eq!(first_admissions, second_admissions);
        }
        assert_eq!(first_manager.active_count(), 12);
        assert_eq!(first, second);
    }
}
