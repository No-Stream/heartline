#[cfg(test)]
mod tests {
    use theme_park_sim::guests::{GuestId, GuestMemoryDetail, Thought};
    use theme_park_sim::park_guests::ParkGuests;
    use theme_park_sim::track::Vec3;
    use theme_park_sim::track::proximity::ProximityKind;
    use theme_park_sim::world::{Path, Position, Surface, World};

    use super::{GuestDetailReport, GuestMemory, GuestThought};

    #[test]
    fn proximity_memories_and_thoughts_serialize_with_event_type() {
        let memory = GuestMemoryDetail::Proximity {
            ride_id: 42,
            kind: ProximityKind::NearMiss,
            position: Vec3::new(1.0, 2.0, 3.0),
            strength: 0.5,
            tick: 10,
        };
        assert_eq!(
            serde_json::to_value(GuestMemory::from(memory)).unwrap(),
            serde_json::json!({"kind": "proximity", "rideId": 42, "eventKind": "nearMiss", "position": {"x": 1.0, "y": 2.0, "z": 3.0}, "strength": 0.5, "tick": 10})
        );
        assert_eq!(
            serde_json::to_value(GuestThought::from(Thought::Proximity(
                42,
                ProximityKind::NearMiss
            )))
            .unwrap(),
            serde_json::json!({"kind": "proximity", "data": {"rideId": 42, "eventKind": "nearMiss"}})
        );
    }

    #[test]
    fn report_preserves_guest_detail_wire_contract() {
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
        let park = ParkGuests::new(5, 1, &world, entrance, 1);
        let mut detail = park.guest_detail(GuestId(0)).unwrap();
        detail.name = "Guest \"quoted\"\nname".to_owned();
        detail.memories = vec![
            GuestMemoryDetail::Ride {
                ride_id: 1,
                enjoyment: -2,
                nausea: 3,
                tick: 4,
            },
            GuestMemoryDetail::Proximity {
                ride_id: 5,
                kind: ProximityKind::NearMiss,
                position: Vec3::new(1.5, 2.5, 3.5),
                strength: 0.5,
                tick: 6,
            },
            GuestMemoryDetail::Food {
                place_id: 7,
                tick: 8,
            },
            GuestMemoryDetail::Wait {
                place_id: 9,
                duration_ticks: 10,
                perceived_duration_ticks: 11,
                tick: 12,
            },
            GuestMemoryDetail::Scenery {
                theme: 1,
                enjoyment: 2,
                tick: 13,
            },
        ];
        let expected_memories = serde_json::json!([
            {"kind": "ride", "rideId": 1, "enjoyment": -2, "nausea": 3, "tick": 4},
            {"kind": "proximity", "rideId": 5, "eventKind": "nearMiss", "position": {"x": 1.5, "y": 2.5, "z": 3.5}, "strength": 0.5, "tick": 6},
            {"kind": "food", "placeId": 7, "tick": 8},
            {"kind": "wait", "placeId": 9, "durationTicks": 10, "perceivedDurationTicks": 11, "tick": 12},
            {"kind": "scenery", "theme": 1, "enjoyment": 2, "tick": 13}
        ]);
        for (thought, expected_thought) in [
            (
                Thought::Exploring,
                serde_json::json!({"kind": "exploring", "data": null}),
            ),
            (
                Thought::Hungry,
                serde_json::json!({"kind": "hungry", "data": null}),
            ),
            (
                Thought::Thirsty,
                serde_json::json!({"kind": "thirsty", "data": null}),
            ),
            (
                Thought::NeedsRestroom,
                serde_json::json!({"kind": "needsRestroom", "data": null}),
            ),
            (
                Thought::Nauseous,
                serde_json::json!({"kind": "nauseous", "data": null}),
            ),
            (
                Thought::Tired,
                serde_json::json!({"kind": "tired", "data": null}),
            ),
            (
                Thought::Unhappy,
                serde_json::json!({"kind": "unhappy", "data": null}),
            ),
            (
                Thought::EnjoyedRide(1),
                serde_json::json!({"kind": "enjoyedRide", "data": 1}),
            ),
            (
                Thought::DislikedRide(2),
                serde_json::json!({"kind": "dislikedRide", "data": 2}),
            ),
            (
                Thought::EnjoyedScenery(3),
                serde_json::json!({"kind": "enjoyedScenery", "data": 3}),
            ),
            (
                Thought::Proximity(4, ProximityKind::TerrainHugging),
                serde_json::json!({"kind": "proximity", "data": {"rideId": 4, "eventKind": "terrainHugging"}}),
            ),
        ] {
            detail.thought = thought;
            let actual = serde_json::to_value(GuestDetailReport::from(&detail)).unwrap();
            assert_eq!(
                actual,
                serde_json::json!({
                    "id": "guest-0", "name": "Guest \"quoted\"\nname", "age": "adult", "groupKind": "single",
                    "groupMembers": [{"id": "guest-0", "name": "Arjun Diallo", "relationship": "self"}],
                    "activity": {"kind": "wandering", "destination": null, "placeId": null},
                    "needs": {"hunger": 0, "thirst": 0, "restroom": 0, "nausea": 0},
                    "energy": 100, "happiness": 80,
                    "traits": {"intensity": 46, "nauseaTolerance": 55, "favouriteRideTypes": [3, 4], "themingCare": 99, "preferredTheme": 0, "spending": "budget", "heightCm": 185, "walkingSpeed": 100},
                    "memories": expected_memories, "moneySpent": 0, "lifetimeMoneySpent": 0, "visitNumber": 0,
                    "thought": expected_thought, "thoughtText": "I wonder what is around this corner."
                })
            );
        }
    }
}

use serde::Serialize;
use theme_park_sim::guests::traits::{AgeGroup, GroupKind, SpendingLevel};
use theme_park_sim::guests::{
    ActivityKind, GuestDetail, GuestMemoryDetail, RelationshipKind, Thought,
};
use theme_park_sim::track::proximity::ProximityKind;
use ts_rs::TS;

macro_rules! wire_enum {
    ($name:ident, $source:ty, {$($variant:ident $(=> $wire:literal)?),+ $(,)?}) => {
        #[derive(Serialize, TS)]
        #[serde(rename_all = "camelCase")]
        pub(crate) enum $name { $( $(#[serde(rename = $wire)])? $variant ),+ }
        impl From<$source> for $name {
            fn from(value: $source) -> Self {
                match value { $(<$source>::$variant => Self::$variant),+ }
            }
        }
    };
}
wire_enum!(GuestAgeGroup, AgeGroup, {Child, Adult, Senior});
wire_enum!(GuestGroupKind, GroupKind, {Single, Couple, Family, Friends});
wire_enum!(GuestRelationship, RelationshipKind, {SelfGuest => "self", Groupmate, Parent, Child, Partner, Friend});
wire_enum!(GuestActivityKind, ActivityKind, {Wandering, Walking, Queuing, Riding, Eating, Drinking, Restroom, Resting, Shopping, Waiting});
wire_enum!(GuestSpending, SpendingLevel, {Budget, Regular, Premium});
wire_enum!(GuestProximityKind, ProximityKind, {NearMiss, Headchopper, Dueling, Tunnel, TerrainHugging});

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuestGroupMember {
    id: String,
    name: String,
    relationship: GuestRelationship,
}
#[derive(Serialize, TS)]
pub(crate) struct GuestDestination {
    x: i32,
    y: i32,
    height: i32,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuestActivity {
    kind: GuestActivityKind,
    destination: Option<GuestDestination>,
    place_id: Option<u32>,
}
#[derive(Serialize, TS)]
pub(crate) struct GuestNeeds {
    hunger: u16,
    thirst: u16,
    restroom: u16,
    nausea: u16,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuestTraits {
    intensity: u8,
    nausea_tolerance: u8,
    favourite_ride_types: Vec<u8>,
    theming_care: u8,
    preferred_theme: u8,
    spending: GuestSpending,
    height_cm: u8,
    walking_speed: u8,
}
#[derive(Serialize, TS)]
pub(crate) struct GuestPosition {
    x: f64,
    y: f64,
    z: f64,
}
#[derive(Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum GuestMemory {
    Ride {
        ride_id: u32,
        enjoyment: i8,
        nausea: u8,
        #[ts(type = "number")]
        tick: u64,
    },
    Proximity {
        ride_id: u32,
        event_kind: GuestProximityKind,
        position: GuestPosition,
        strength: f64,
        #[ts(type = "number")]
        tick: u64,
    },
    Food {
        place_id: u32,
        #[ts(type = "number")]
        tick: u64,
    },
    Wait {
        place_id: u32,
        #[ts(type = "number")]
        duration_ticks: u64,
        #[ts(type = "number")]
        perceived_duration_ticks: u64,
        #[ts(type = "number")]
        tick: u64,
    },
    Scenery {
        theme: u8,
        enjoyment: u8,
        #[ts(type = "number")]
        tick: u64,
    },
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuestProximityThought {
    ride_id: u32,
    event_kind: GuestProximityKind,
}
#[derive(Serialize, TS)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub(crate) enum GuestThought {
    Exploring(()),
    Hungry(()),
    Thirsty(()),
    NeedsRestroom(()),
    Nauseous(()),
    Tired(()),
    Unhappy(()),
    EnjoyedRide(u32),
    DislikedRide(u32),
    EnjoyedScenery(u8),
    Proximity(GuestProximityThought),
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuestDetailReport {
    id: String,
    name: String,
    age: GuestAgeGroup,
    group_kind: GuestGroupKind,
    group_members: Vec<GuestGroupMember>,
    activity: GuestActivity,
    needs: GuestNeeds,
    energy: u16,
    happiness: u16,
    traits: GuestTraits,
    memories: Vec<GuestMemory>,
    money_spent: u32,
    lifetime_money_spent: u32,
    visit_number: u32,
    thought: GuestThought,
    thought_text: String,
}
impl From<&GuestDetail> for GuestDetailReport {
    fn from(detail: &GuestDetail) -> Self {
        Self {
            id: format!("guest-{}", detail.id.0),
            name: detail.name.clone(),
            age: detail.age.into(),
            group_kind: detail.group_kind.into(),
            group_members: detail
                .group_members
                .iter()
                .map(|member| GuestGroupMember {
                    id: format!("guest-{}", member.id.0),
                    name: member.name.clone(),
                    relationship: member.relationship.into(),
                })
                .collect(),
            activity: GuestActivity {
                kind: detail.activity.kind.into(),
                destination: detail
                    .activity
                    .destination
                    .map(|position| GuestDestination {
                        x: position.x.into(),
                        y: position.y.into(),
                        height: position.height.into(),
                    }),
                place_id: detail.activity.place_id,
            },
            needs: GuestNeeds {
                hunger: detail.needs.hunger,
                thirst: detail.needs.thirst,
                restroom: detail.needs.restroom,
                nausea: detail.needs.nausea,
            },
            energy: detail.energy,
            happiness: detail.happiness,
            traits: GuestTraits {
                intensity: detail.traits.intensity,
                nausea_tolerance: detail.traits.nausea_tolerance,
                favourite_ride_types: detail.traits.favourite_ride_types.clone(),
                theming_care: detail.traits.theming_care,
                preferred_theme: detail.traits.preferred_theme,
                spending: detail.traits.spending.into(),
                height_cm: detail.traits.height_cm,
                walking_speed: detail.traits.walking_speed,
            },
            memories: detail
                .memories
                .iter()
                .copied()
                .map(GuestMemory::from)
                .collect(),
            money_spent: detail.money_spent,
            lifetime_money_spent: detail.lifetime_money_spent,
            visit_number: detail.visit_number,
            thought: detail.thought.into(),
            thought_text: detail.thought_text.clone(),
        }
    }
}
impl From<GuestMemoryDetail> for GuestMemory {
    fn from(memory: GuestMemoryDetail) -> Self {
        match memory {
            GuestMemoryDetail::Ride {
                ride_id,
                enjoyment,
                nausea,
                tick,
            } => Self::Ride {
                ride_id,
                enjoyment,
                nausea,
                tick,
            },
            GuestMemoryDetail::Proximity {
                ride_id,
                kind,
                position,
                strength,
                tick,
            } => Self::Proximity {
                ride_id,
                event_kind: kind.into(),
                position: GuestPosition {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                strength,
                tick,
            },
            GuestMemoryDetail::Food { place_id, tick } => Self::Food { place_id, tick },
            GuestMemoryDetail::Wait {
                place_id,
                duration_ticks,
                perceived_duration_ticks,
                tick,
            } => Self::Wait {
                place_id,
                duration_ticks,
                perceived_duration_ticks,
                tick,
            },
            GuestMemoryDetail::Scenery {
                theme,
                enjoyment,
                tick,
            } => Self::Scenery {
                theme,
                enjoyment,
                tick,
            },
        }
    }
}
impl From<Thought> for GuestThought {
    fn from(thought: Thought) -> Self {
        match thought {
            Thought::Exploring => Self::Exploring(()),
            Thought::Hungry => Self::Hungry(()),
            Thought::Thirsty => Self::Thirsty(()),
            Thought::NeedsRestroom => Self::NeedsRestroom(()),
            Thought::Nauseous => Self::Nauseous(()),
            Thought::Tired => Self::Tired(()),
            Thought::Unhappy => Self::Unhappy(()),
            Thought::EnjoyedRide(id) => Self::EnjoyedRide(id),
            Thought::DislikedRide(id) => Self::DislikedRide(id),
            Thought::EnjoyedScenery(theme) => Self::EnjoyedScenery(theme),
            Thought::Proximity(ride_id, kind) => Self::Proximity(GuestProximityThought {
                ride_id,
                event_kind: kind.into(),
            }),
        }
    }
}
