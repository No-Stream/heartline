//! Guest-facing identity and detail data assembled from live simulation state.

use super::behavior::Thought;
use super::groups::Relationship;
use super::needs::Needs;
use super::park::ParkGuests;
use super::storage::GuestStore;
use super::traits::{AgeGroup, GroupKind, GuestId, SpendingLevel};
use crate::track::Vec3;
use crate::track::proximity::ProximityKind;
use crate::world::Position;

const GIVEN_NAMES: [&str; 32] = [
    "Amina", "Mateo", "Mei", "Amara", "Luca", "Sana", "Ravi", "Ines", "Noah", "Yara", "Hana",
    "Diego", "Nia", "Theo", "Farah", "Min-jun", "Leila", "Tomas", "Priya", "Emil", "Zuri", "Ana",
    "Kenji", "Mariam", "Felix", "Sofia", "Ibrahim", "Mila", "Arjun", "Esme", "Lin", "Samira",
];

const FAMILY_NAMES: [&str; 24] = [
    "Okafor",
    "Santos",
    "Chen",
    "Patel",
    "Rossi",
    "Haddad",
    "Kim",
    "Silva",
    "Ndlovu",
    "Garcia",
    "Nguyen",
    "Dubois",
    "Kowalski",
    "Ali",
    "Andersson",
    "Yamamoto",
    "Costa",
    "Mensah",
    "Ivanov",
    "Kaur",
    "Mori",
    "Diallo",
    "Novak",
    "Reyes",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipKind {
    SelfGuest,
    Groupmate,
    Parent,
    Child,
    Partner,
    Friend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityKind {
    Wandering,
    Walking,
    Queuing,
    Riding,
    Eating,
    Drinking,
    Restroom,
    Resting,
    Shopping,
    Waiting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMemberDetail {
    pub id: GuestId,
    pub name: String,
    pub relationship: RelationshipKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivityDetail {
    pub kind: ActivityKind,
    pub destination: Option<Position>,
    pub place_id: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestTraitsDetail {
    pub intensity: u8,
    pub nausea_tolerance: u8,
    pub favourite_ride_types: Vec<u8>,
    pub theming_care: u8,
    pub preferred_theme: u8,
    pub spending: SpendingLevel,
    pub height_cm: u8,
    pub walking_speed: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuestMemoryDetail {
    Ride {
        ride_id: u32,
        enjoyment: i8,
        nausea: u8,
        tick: u64,
    },
    Proximity {
        ride_id: u32,
        kind: ProximityKind,
        position: Vec3,
        strength: f64,
        tick: u64,
    },
    Food {
        place_id: u32,
        tick: u64,
    },
    Wait {
        place_id: u32,
        duration_ticks: u64,
        perceived_duration_ticks: u64,
        tick: u64,
    },
    Scenery {
        theme: u8,
        enjoyment: u8,
        tick: u64,
    },
}

impl GuestMemoryDetail {
    pub const fn tick(self) -> u64 {
        match self {
            Self::Ride { tick, .. }
            | Self::Proximity { tick, .. }
            | Self::Food { tick, .. }
            | Self::Wait { tick, .. }
            | Self::Scenery { tick, .. } => tick,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuestDetail {
    pub id: GuestId,
    pub name: String,
    pub age: AgeGroup,
    pub group_kind: GroupKind,
    pub group_members: Vec<GroupMemberDetail>,
    pub activity: ActivityDetail,
    pub needs: Needs,
    pub energy: u16,
    pub happiness: u16,
    pub traits: GuestTraitsDetail,
    pub memories: Vec<GuestMemoryDetail>,
    pub money_spent: u32,
    pub lifetime_money_spent: u32,
    pub visit_number: u32,
    pub thought: Thought,
    pub thought_text: String,
}

impl GuestDetail {
    /// Encode the panel record as JSON without adding a runtime serialization dependency.
    pub fn to_json(&self) -> String {
        let members = self
            .group_members
            .iter()
            .map(|member| {
                format!(
                    "{{\"id\":{},\"name\":{},\"relationship\":{}}}",
                    json_string(&format!("guest-{}", member.id.0)),
                    json_string(&member.name),
                    json_string(relationship_name(member.relationship))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let favourite_ride_types = self
            .traits
            .favourite_ride_types
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let memories = self
            .memories
            .iter()
            .map(memory_json)
            .collect::<Vec<_>>()
            .join(",");
        let (thought_kind, thought_data) = thought_json(self.thought);
        let destination = self.activity.destination.map_or_else(
            || "null".to_owned(),
            |position| {
                format!(
                    "{{\"x\":{},\"y\":{},\"height\":{}}}",
                    position.x, position.y, position.height
                )
            },
        );
        format!(
            concat!(
                "{{\"id\":{},\"name\":{},\"age\":{},\"groupKind\":{},",
                "\"groupMembers\":[{}],\"activity\":{{\"kind\":{},\"destination\":{},\"placeId\":{}}},",
                "\"needs\":{{\"hunger\":{},\"thirst\":{},\"restroom\":{},\"nausea\":{}}},",
                "\"energy\":{},\"happiness\":{},\"traits\":{{\"intensity\":{},\"nauseaTolerance\":{},",
                "\"favouriteRideTypes\":[{}],\"themingCare\":{},\"preferredTheme\":{},\"spending\":{},",
                "\"heightCm\":{},\"walkingSpeed\":{}}},\"memories\":[{}],",
                "\"moneySpent\":{},\"lifetimeMoneySpent\":{},\"visitNumber\":{},",
                "\"thought\":{{\"kind\":{},\"data\":{}}},\"thoughtText\":{}}}"
            ),
            json_string(&format!("guest-{}", self.id.0)),
            json_string(&self.name),
            json_string(age_name(self.age)),
            json_string(group_name(self.group_kind)),
            members,
            json_string(activity_name(self.activity.kind)),
            destination,
            self.activity
                .place_id
                .map_or_else(|| "null".to_owned(), |id| id.to_string()),
            self.needs.hunger,
            self.needs.thirst,
            self.needs.restroom,
            self.needs.nausea,
            self.energy,
            self.happiness,
            self.traits.intensity,
            self.traits.nausea_tolerance,
            favourite_ride_types,
            self.traits.theming_care,
            self.traits.preferred_theme,
            json_string(spending_name(self.traits.spending)),
            self.traits.height_cm,
            self.traits.walking_speed,
            memories,
            self.money_spent,
            self.lifetime_money_spent,
            self.visit_number,
            json_string(thought_kind),
            thought_data,
            json_string(&self.thought_text),
        )
    }
}

impl GuestStore {
    pub fn name(&self, id: GuestId) -> String {
        self.index(id);
        let value = mix_name_seed(self.seed ^ u64::from(id.0).wrapping_mul(0x9e3779b97f4a7c15));
        let given = GIVEN_NAMES[(value % GIVEN_NAMES.len() as u64) as usize];
        let family = FAMILY_NAMES[((value >> 32) % FAMILY_NAMES.len() as u64) as usize];
        format!("{given} {family}")
    }

    pub fn contains(&self, id: GuestId) -> bool {
        self.ids.get(id.0 as usize) == Some(&id)
    }
}

impl ParkGuests {
    pub fn guest_detail(&self, id: GuestId) -> Option<GuestDetail> {
        if !self.walking.traits().contains(id) {
            return None;
        }
        let store = self.walking.traits();
        let behavior = &self.behaviors.guests[id.0 as usize];
        let group = store
            .groups
            .iter()
            .find(|group| group.members.contains(&id))
            .expect("every guest belongs to a group");
        let group_members = std::iter::once(GroupMemberDetail {
            id,
            name: store.name(id),
            relationship: RelationshipKind::SelfGuest,
        })
        .chain(
            group
                .members
                .iter()
                .copied()
                .filter(|&member| member != id)
                .map(|member| GroupMemberDetail {
                    id: member,
                    name: store.name(member),
                    relationship: relationship_between(store, id, member),
                }),
        )
        .collect();
        let mut memories: Vec<_> = store
            .memory(id)
            .iter()
            .map(|entry| GuestMemoryDetail::Ride {
                ride_id: entry.ride_id,
                enjoyment: entry.enjoyment,
                nausea: entry.nausea,
                tick: entry.tick,
            })
            .chain(behavior.recent_memories().iter().copied())
            .collect();
        memories.sort_by_key(|memory| memory.tick());
        let memories = memories.into_iter().rev().take(8).collect::<Vec<_>>();
        let activity = self.behaviors.activity_detail(id);
        let thought = behavior.thought;
        Some(GuestDetail {
            id,
            name: store.name(id),
            age: store.age(id),
            group_kind: group.kind,
            group_members,
            activity,
            needs: behavior.needs,
            energy: behavior.energy,
            happiness: behavior.happiness,
            traits: GuestTraitsDetail {
                intensity: store.intensity(id),
                nausea_tolerance: store.nausea_tolerance(id),
                favourite_ride_types: (0..8)
                    .filter(|ride_type| store.favourite_ride_types(id) & (1 << ride_type) != 0)
                    .collect(),
                theming_care: store.cares_about_theming(id),
                preferred_theme: store.preferred_theme(id),
                spending: store.spending(id),
                height_cm: store.height_cm(id),
                walking_speed: store.walking_speed(id),
            },
            memories,
            money_spent: u32::from(100_u16.saturating_sub(behavior.money)),
            lifetime_money_spent: behavior.lifetime_money_spent,
            visit_number: behavior.visit_number,
            thought,
            thought_text: thought_text(id, thought),
        })
    }

    pub fn tick_count(&self) -> u64 {
        self.walking.tick_count()
    }
}

pub fn thought_text(guest: GuestId, thought: Thought) -> String {
    let (phrases, variant) = match thought {
        Thought::Exploring => (
            &[
                "I wonder what is around this corner.",
                "There is so much to see here.",
                "I should see where this path goes.",
            ][..],
            0,
        ),
        Thought::Hungry => (
            &[
                "I could really go for a snack.",
                "My stomach is starting to complain.",
                "A little something to eat would hit the spot.",
            ][..],
            1,
        ),
        Thought::Thirsty => (
            &[
                "I could use a cold drink.",
                "Time for a drink before the next adventure.",
                "I am getting thirsty out here.",
            ][..],
            2,
        ),
        Thought::NeedsRestroom => (
            &[
                "I need to find a restroom soon.",
                "I should make a quick restroom stop.",
                "A restroom would be very welcome right now.",
            ][..],
            3,
        ),
        Thought::Nauseous => (
            &[
                "I need a moment to settle my stomach.",
                "That ride left me feeling a bit wobbly.",
                "I should take it easy for a little while.",
            ][..],
            4,
        ),
        Thought::Tired => (
            &[
                "A quiet sit-down sounds lovely.",
                "My feet could use a rest.",
                "I need a little breather.",
            ][..],
            5,
        ),
        Thought::Unhappy => (
            &[
                "This visit is not going quite to plan.",
                "I could use something to cheer me up.",
                "I hope the next stop is more fun.",
            ][..],
            6,
        ),
        Thought::EnjoyedRide(ride_id) => (
            &[
                "That ride was a real treat.",
                "I would happily ride that again.",
                "That was worth the wait.",
            ][..],
            7 ^ ride_id,
        ),
        Thought::Proximity(ride_id, kind) => (&[proximity_text(kind)][..], 9 ^ ride_id),
        Thought::EnjoyedScenery(theme) => (&["The scenery here is lovely."][..], u32::from(theme)),
        Thought::DislikedRide(ride_id) => (
            &[
                "That ride was a bit much for me.",
                "I think I will skip that one next time.",
                "I am glad that ride is over.",
            ][..],
            8 ^ ride_id,
        ),
    };
    let hash =
        mix_name_seed(u64::from(guest.0) ^ u64::from(variant).wrapping_mul(0x517cc1b727220a95));
    phrases[(hash % phrases.len() as u64) as usize].to_owned()
}

fn relationship_between(store: &GuestStore, guest: GuestId, member: GuestId) -> RelationshipKind {
    let group = store
        .groups
        .iter()
        .find(|group| group.members.contains(&guest))
        .expect("every guest belongs to a group");
    if group.relations.iter().any(|relation| {
        matches!(
            relation,
            Relationship::ParentChild { parent, child }
                if (*parent == guest && *child == member) || (*parent == member && *child == guest)
        )
    }) {
        return if store.children(guest).contains(&member) {
            RelationshipKind::Child
        } else {
            RelationshipKind::Parent
        };
    }
    if store.partners(guest).contains(&member) {
        RelationshipKind::Partner
    } else if store.friends(guest).contains(&member) {
        RelationshipKind::Friend
    } else {
        RelationshipKind::Groupmate
    }
}

fn memory_json(memory: &GuestMemoryDetail) -> String {
    match *memory {
        GuestMemoryDetail::Ride {
            ride_id,
            enjoyment,
            nausea,
            tick,
        } => format!(
            "{{\"kind\":\"ride\",\"rideId\":{ride_id},\"enjoyment\":{enjoyment},\"nausea\":{nausea},\"tick\":{tick}}}"
        ),
        GuestMemoryDetail::Proximity {
            ride_id,
            kind,
            position,
            strength,
            tick,
        } => format!(
            "{{\"kind\":\"proximity\",\"rideId\":{ride_id},\"eventKind\":\"{}\",\"position\":{{\"x\":{},\"y\":{},\"z\":{}}},\"strength\":{strength},\"tick\":{tick}}}",
            proximity_name(kind),
            position.x,
            position.y,
            position.z
        ),
        GuestMemoryDetail::Food { place_id, tick } => {
            format!("{{\"kind\":\"food\",\"placeId\":{place_id},\"tick\":{tick}}}")
        }
        GuestMemoryDetail::Wait {
            place_id,
            duration_ticks,
            perceived_duration_ticks,
            tick,
        } => format!(
            "{{\"kind\":\"wait\",\"placeId\":{place_id},\"durationTicks\":{duration_ticks},\"perceivedDurationTicks\":{perceived_duration_ticks},\"tick\":{tick}}}"
        ),
        GuestMemoryDetail::Scenery {
            theme,
            enjoyment,
            tick,
        } => format!(
            "{{\"kind\":\"scenery\",\"theme\":{theme},\"enjoyment\":{enjoyment},\"tick\":{tick}}}"
        ),
    }
}

fn thought_json(thought: Thought) -> (&'static str, String) {
    match thought {
        Thought::Exploring => ("exploring", "null".into()),
        Thought::Hungry => ("hungry", "null".into()),
        Thought::Thirsty => ("thirsty", "null".into()),
        Thought::NeedsRestroom => ("needsRestroom", "null".into()),
        Thought::Nauseous => ("nauseous", "null".into()),
        Thought::Tired => ("tired", "null".into()),
        Thought::Unhappy => ("unhappy", "null".into()),
        Thought::EnjoyedRide(ride_id) => ("enjoyedRide", ride_id.to_string()),
        Thought::DislikedRide(ride_id) => ("dislikedRide", ride_id.to_string()),
        Thought::EnjoyedScenery(theme) => ("enjoyedScenery", theme.to_string()),
        Thought::Proximity(ride_id, kind) => (
            "proximity",
            format!(
                "{{\"rideId\":{ride_id},\"eventKind\":\"{}\"}}",
                proximity_name(kind)
            ),
        ),
    }
}

fn proximity_name(kind: ProximityKind) -> &'static str {
    match kind {
        ProximityKind::NearMiss => "nearMiss",
        ProximityKind::Headchopper => "headchopper",
        ProximityKind::Dueling => "dueling",
        ProximityKind::Tunnel => "tunnel",
        ProximityKind::TerrainHugging => "terrainHugging",
    }
}

fn proximity_text(kind: ProximityKind) -> &'static str {
    match kind {
        ProximityKind::NearMiss => "That was a close pass!",
        ProximityKind::Headchopper => "That went right over my head!",
        ProximityKind::Dueling => "The other train came so close!",
        ProximityKind::Tunnel => "The tunnel was exciting!",
        ProximityKind::TerrainHugging => "We came so close to the ground!",
    }
}

fn json_string(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() + 2);
    encoded.push('"');
    for character in value.chars() {
        match character {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            character if character <= '\u{1f}' => {
                encoded.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

const fn age_name(age: AgeGroup) -> &'static str {
    match age {
        AgeGroup::Child => "child",
        AgeGroup::Adult => "adult",
        AgeGroup::Senior => "senior",
    }
}

const fn group_name(group: GroupKind) -> &'static str {
    match group {
        GroupKind::Single => "single",
        GroupKind::Couple => "couple",
        GroupKind::Family => "family",
        GroupKind::Friends => "friends",
    }
}

const fn relationship_name(relationship: RelationshipKind) -> &'static str {
    match relationship {
        RelationshipKind::SelfGuest => "self",
        RelationshipKind::Groupmate => "groupmate",
        RelationshipKind::Parent => "parent",
        RelationshipKind::Child => "child",
        RelationshipKind::Partner => "partner",
        RelationshipKind::Friend => "friend",
    }
}

const fn activity_name(activity: ActivityKind) -> &'static str {
    match activity {
        ActivityKind::Wandering => "wandering",
        ActivityKind::Walking => "walking",
        ActivityKind::Queuing => "queuing",
        ActivityKind::Riding => "riding",
        ActivityKind::Eating => "eating",
        ActivityKind::Drinking => "drinking",
        ActivityKind::Restroom => "restroom",
        ActivityKind::Resting => "resting",
        ActivityKind::Shopping => "shopping",
        ActivityKind::Waiting => "waiting",
    }
}

const fn spending_name(spending: SpendingLevel) -> &'static str {
    match spending {
        SpendingLevel::Budget => "budget",
        SpendingLevel::Regular => "regular",
        SpendingLevel::Premium => "premium",
    }
}

fn mix_name_seed(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use crate::guest_behavior::{Facility, GuestBehavior, KnownPlace, PlaceKind, Thought};
    use crate::guests::{GuestId, generate_guests};
    use crate::world::{Path, Position, Surface, World};

    use super::{GuestMemoryDetail, ParkGuests, Vec3};

    fn park(seed: u64, guest_count: usize) -> (ParkGuests, World) {
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
        (
            ParkGuests::new(seed, guest_count, &world, entrance, 1),
            world,
        )
    }

    #[test]
    fn names_are_deterministic_and_vary_across_guest_ids() {
        let first = generate_guests(19, 40);
        let second = generate_guests(19, 40);
        let other_seed = generate_guests(20, 40);
        let names: Vec<_> = first.ids().map(|id| first.name(id)).collect();

        assert_eq!(
            names,
            second.ids().map(|id| second.name(id)).collect::<Vec<_>>()
        );
        assert_ne!(
            names,
            other_seed
                .ids()
                .map(|id| other_seed.name(id))
                .collect::<Vec<_>>()
        );
        assert!(names.iter().collect::<std::collections::HashSet<_>>().len() > 20);
    }

    #[test]
    fn detail_shows_ride_memory_and_matching_thought() {
        let (mut park, world) = park(5, 1);
        let id = GuestId(0);
        let ride = KnownPlace {
            id: 42,
            position: Position {
                x: 0,
                y: 0,
                height: 0,
            },
            kind: PlaceKind::Ride,
            minimum_height_cm: 0,
            intensity: 50,
            ride_type: 1,
            theme: 0,
            price: 12,
        };
        let facilities = [Facility {
            exit_position: None,
            place: ride,
            capacity: 1,
            duration_ticks: 1,
        }];
        for _ in 0..100 {
            park.tick(&world, &facilities, 0, Some(1));
        }

        let detail = park.guest_detail(id).unwrap();
        assert!(detail.memories.iter().any(|memory| matches!(
            memory,
            GuestMemoryDetail::Ride {
                ride_id: 42,
                enjoyment,
                ..
            } if *enjoyment >= 0
        )));
        assert!(matches!(detail.thought, Thought::EnjoyedRide(42)));
    }

    #[test]
    fn proximity_memories_and_thoughts_serialize_with_event_type() {
        use crate::track::proximity::ProximityKind;

        let memory = GuestMemoryDetail::Proximity {
            ride_id: 42,
            kind: ProximityKind::NearMiss,
            position: Vec3::new(1.0, 2.0, 3.0),
            strength: 0.5,
            tick: 10,
        };
        assert_eq!(
            super::memory_json(&memory),
            r#"{"kind":"proximity","rideId":42,"eventKind":"nearMiss","position":{"x":1,"y":2,"z":3},"strength":0.5,"tick":10}"#
        );
        assert_eq!(
            super::thought_json(Thought::Proximity(42, ProximityKind::NearMiss)),
            (
                "proximity",
                r#"{"rideId":42,"eventKind":"nearMiss"}"#.to_owned()
            )
        );
        assert!(
            !super::thought_text(GuestId(0), Thought::Proximity(42, ProximityKind::NearMiss))
                .is_empty()
        );
    }

    #[test]
    fn detail_shows_hunger_thought_for_a_hungry_guest() {
        let mut behavior = GuestBehavior::default();
        behavior.needs.hunger = 80;
        behavior.advance(0);

        assert_eq!(behavior.thought, Thought::Hungry);
    }
}
