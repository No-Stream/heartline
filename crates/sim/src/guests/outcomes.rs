use super::needs::Needs;
use super::traits::GuestId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RideOutcome {
    pub rides_taken: u64,
    pub unique_riders: BTreeSet<GuestId>,
    pub guest_rating_total: i64,
    pub guest_rating_count: u64,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct GuestDestination {
    pub guest_id: GuestId,
    pub facility_id: u32,
    pub position: crate::world::Position,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParkOutcomes {
    pub rides: BTreeMap<u32, RideOutcome>,
    /// Active guests whose current needs meet the game-thought threshold.
    pub unmet_need_guests: BTreeSet<GuestId>,
    /// Active guests whose path component cannot reach a listed facility.
    pub unreachable_destinations: BTreeSet<GuestDestination>,
    /// Chosen facility destinations that became unreachable after a path change.
    pub failed_routes: BTreeSet<GuestDestination>,
}

impl ParkOutcomes {
    pub(super) fn record_ride(&mut self, ride_id: u32, guest_id: GuestId, rating: i8) {
        let ride = self.rides.entry(ride_id).or_default();
        ride.rides_taken += 1;
        ride.unique_riders.insert(guest_id);
        ride.guest_rating_total += i64::from(rating);
        ride.guest_rating_count += 1;
    }

    pub(super) fn refresh_unmet_need_guests(
        &mut self,
        active_needs: impl IntoIterator<Item = (GuestId, Needs)>,
    ) {
        self.unmet_need_guests = active_needs
            .into_iter()
            .filter_map(|(guest_id, needs)| {
                (needs.hunger >= 70
                    || needs.thirst >= 70
                    || needs.restroom >= 70
                    || needs.nausea >= 70)
                    .then_some(guest_id)
            })
            .collect();
    }

    pub(super) fn record_failed_route(&mut self, destination: GuestDestination) {
        self.failed_routes.insert(destination);
    }

    pub(super) fn refresh_unreachable_destinations(
        &mut self,
        guest_components: impl IntoIterator<Item = (GuestId, Option<usize>)>,
        facilities: impl IntoIterator<Item = (u32, crate::world::Position, Option<usize>)>,
    ) {
        self.unreachable_destinations.clear();
        let facilities: Vec<_> = facilities.into_iter().collect();
        for (guest_id, guest_component) in guest_components {
            for &(facility_id, position, facility_component) in &facilities {
                if guest_component.is_none()
                    || facility_component.is_none()
                    || guest_component != facility_component
                {
                    self.unreachable_destinations.insert(GuestDestination {
                        guest_id,
                        facility_id,
                        position,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GuestDestination, ParkOutcomes};
    use crate::guests::needs::Needs;
    use crate::guests::traits::GuestId;
    use crate::world::Position;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn ride_outcomes_count_each_rating_and_distinct_riders() {
        let mut outcomes = ParkOutcomes::default();

        outcomes.record_ride(17, GuestId(2), 24);
        outcomes.record_ride(17, GuestId(2), -6);
        outcomes.record_ride(17, GuestId(5), 12);

        assert_eq!(
            outcomes.rides,
            BTreeMap::from([(
                17,
                super::RideOutcome {
                    rides_taken: 3,
                    unique_riders: BTreeSet::from([GuestId(2), GuestId(5)]),
                    guest_rating_total: 30,
                    guest_rating_count: 3,
                }
            )])
        );
    }

    #[test]
    fn unmet_need_snapshot_includes_only_active_needs_at_the_guest_threshold() {
        let mut outcomes = ParkOutcomes::default();
        outcomes.refresh_unmet_need_guests([
            (GuestId(0), Needs::default()),
            (
                GuestId(1),
                Needs {
                    hunger: 69,
                    ..Needs::default()
                },
            ),
            (
                GuestId(2),
                Needs {
                    nausea: 70,
                    ..Needs::default()
                },
            ),
        ]);

        assert_eq!(outcomes.unmet_need_guests, BTreeSet::from([GuestId(2)]));

        outcomes.refresh_unmet_need_guests([(GuestId(0), Needs::default())]);
        assert!(outcomes.unmet_need_guests.is_empty());
    }

    #[test]
    fn unreachable_destinations_preserve_guest_and_facility_identity() {
        let mut outcomes = ParkOutcomes::default();
        let destination = GuestDestination {
            guest_id: GuestId(3),
            facility_id: 41,
            position: Position {
                x: 9,
                y: 12,
                height: 2,
            },
        };

        outcomes.record_failed_route(destination);

        assert_eq!(outcomes.failed_routes, BTreeSet::from([destination]));
    }

    #[test]
    fn reachability_snapshot_names_only_guests_without_a_path_to_a_facility() {
        let mut outcomes = ParkOutcomes::default();
        let facility_position = Position {
            x: 9,
            y: 12,
            height: 2,
        };

        outcomes.refresh_unreachable_destinations(
            [(GuestId(0), Some(1)), (GuestId(1), Some(2))],
            [(41, facility_position, Some(2))],
        );

        assert_eq!(
            outcomes.unreachable_destinations,
            BTreeSet::from([GuestDestination {
                guest_id: GuestId(0),
                facility_id: 41,
                position: facility_position,
            }])
        );
    }
}
