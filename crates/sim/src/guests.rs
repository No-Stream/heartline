//! Individual guest traits and groups.
//!
//! | Trait | Distribution and correlation |
//! |---|---|
//! | Intensity taste | Integer 0–100, uniform for adults (mean 50); children are shifted down by 8 and clipped (mean about 42.3). |
//! | Nausea tolerance | Integer 0–100, uniform for adults (mean 50); children are shifted up by 10 and clipped (mean about 59.5). |
//! | Favourite ride types | One or two distinct types from keys 0–5; children choose gentle types 0–1 70% of the time. |
//! | Theming care | Integer 0–100, uniform. |
//! | Preferred theme | Uniform among the available theme keys. |
//! | Spending | Base adult rates are budget 35%, regular 45%, premium 20%; couple rolls shift up by 5 and family rolls down by 5, while children are budget. |
//! | Height | Children 90–139 cm, adults 145–204 cm. |
//! | Age | Child, adult, or senior; family groups contain one or two adults and one to three children. |
//! | Group kind | Single 20%, couple 25%, family 35%, and friends 20% of generated groups. |
//! | Walking speed | Base integer 70–130; children are shifted down by 6 and seniors by 12, with endpoints clamped. |
//! | Appearance | Uniform skin keys 0–7, hair keys 0–11, clothing keys 0–15, and body keys 0–4. |
//!
//! Values are derived from the seed and stable guest ID, so population creation is
//! reproducible and independent of executor partitioning or thread count.
//!
//! Each eight-entry memory ring occupies 136 bytes. The 2 MB budget for 10,000
//! guests allows 200 bytes per guest, leaving 64 bytes per ring for growth or padding.

pub mod walking;

use std::mem::size_of;

pub const RIDE_TYPE_COUNT: u8 = 6;
pub const THEME_COUNT: u8 = 8;
pub const SKIN_TONE_COUNT: u8 = 8;
pub const HAIR_STYLE_COUNT: u8 = 12;
pub const CLOTHING_STYLE_COUNT: u8 = 16;
pub const BODY_SHAPE_COUNT: u8 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgeGroup {
    Child,
    Adult,
    Senior,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpendingLevel {
    Budget,
    Regular,
    Premium,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupKind {
    Single,
    Couple,
    Family,
    Friends,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GuestId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Appearance {
    pub skin_tone: u8,
    pub hair_style: u8,
    pub clothing_style: u8,
    pub body_shape: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestGroup {
    pub kind: GroupKind,
    pub members: Vec<GuestId>,
    relations: Vec<Relationship>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Relationship {
    ParentChild { parent: GuestId, child: GuestId },
    Partners { first: GuestId, second: GuestId },
    Friends { first: GuestId, second: GuestId },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryEntry {
    pub ride_id: u32,
    pub enjoyment: i8,
    pub nausea: u8,
    pub tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestMemory {
    entries: [MemoryEntry; Self::CAPACITY],
    start: u8,
    len: u8,
}

impl GuestMemory {
    pub const CAPACITY: usize = 8;

    pub fn push(&mut self, entry: MemoryEntry) {
        let index = (usize::from(self.start) + usize::from(self.len)) % Self::CAPACITY;
        if usize::from(self.len) == Self::CAPACITY {
            self.entries[usize::from(self.start)] = entry;
            self.start = ((usize::from(self.start) + 1) % Self::CAPACITY) as u8;
        } else {
            self.entries[index] = entry;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryEntry> {
        (0..self.len())
            .map(|offset| &self.entries[(usize::from(self.start) + offset) % Self::CAPACITY])
    }
}

impl Default for GuestMemory {
    fn default() -> Self {
        Self {
            entries: [MemoryEntry::default(); Self::CAPACITY],
            start: 0,
            len: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestStore {
    ids: Vec<GuestId>,
    active: Vec<bool>,
    group_indices: Vec<u32>,
    ages: Vec<AgeGroup>,
    intensity: Vec<u8>,
    nausea_tolerance: Vec<u8>,
    favourite_ride_types: Vec<u8>,
    theming_care: Vec<u8>,
    preferred_theme: Vec<u8>,
    spending: Vec<SpendingLevel>,
    height_cm: Vec<u8>,
    walking_speed: Vec<u8>,
    appearance: Vec<Appearance>,
    memories: Vec<GuestMemory>,
    pub groups: Vec<GuestGroup>,
}

impl GuestStore {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            active: Vec::with_capacity(capacity),
            group_indices: Vec::with_capacity(capacity),
            ages: Vec::with_capacity(capacity),
            intensity: Vec::with_capacity(capacity),
            nausea_tolerance: Vec::with_capacity(capacity),
            favourite_ride_types: Vec::with_capacity(capacity),
            theming_care: Vec::with_capacity(capacity),
            preferred_theme: Vec::with_capacity(capacity),
            spending: Vec::with_capacity(capacity),
            height_cm: Vec::with_capacity(capacity),
            walking_speed: Vec::with_capacity(capacity),
            appearance: Vec::with_capacity(capacity),
            memories: Vec::with_capacity(capacity),
            groups: Vec::new(),
        }
    }

    fn add_guest(
        &mut self,
        age: AgeGroup,
        group: GroupKind,
        group_index: u32,
        random: &mut Random,
    ) -> GuestId {
        let id = GuestId(self.ids.len() as u32);
        let group_spending = match group {
            GroupKind::Single => 0,
            GroupKind::Couple => 1,
            GroupKind::Family => -1,
            GroupKind::Friends => 0,
        };
        let (intensity_shift, nausea_shift, speed_shift, height_range) = match age {
            AgeGroup::Child => (-8_i16, 10_i16, -6_i16, (90_u8, 139_u8)),
            AgeGroup::Adult => (0, 0, 0, (145, 204)),
            AgeGroup::Senior => (-4, 3, -12, (145, 194)),
        };
        let intensity = shift(random.range(101) as i16, intensity_shift);
        let nausea_tolerance = shift(random.range(101) as i16, nausea_shift);
        let mut favourite_types = 0_u8;
        let favourite_count = 1 + random.range(2);
        for _ in 0..favourite_count {
            let ride_type = if age == AgeGroup::Child && random.chance(70, 100) {
                random.range(2) as u8
            } else {
                random.range(usize::from(RIDE_TYPE_COUNT)) as u8
            };
            favourite_types |= 1 << ride_type;
        }
        if favourite_types.count_ones() == 1 && favourite_count == 2 {
            favourite_types |=
                1 << ((favourite_types.trailing_zeros() + 1) % u32::from(RIDE_TYPE_COUNT));
        }
        let base_spending_roll = random.range(100) as i16 + group_spending * 5;
        let spending = if age == AgeGroup::Child || base_spending_roll < 35 {
            SpendingLevel::Budget
        } else if base_spending_roll < 80 {
            SpendingLevel::Regular
        } else {
            SpendingLevel::Premium
        };
        let height_cm =
            height_range.0 + random.range(usize::from(height_range.1 - height_range.0 + 1)) as u8;
        let speed = (70 + random.range(61)) as i16;
        let speed = shift(speed, speed_shift).clamp(70, 130);
        self.ids.push(id);
        self.active.push(true);
        self.group_indices.push(group_index);
        self.ages.push(age);
        self.intensity.push(intensity);
        self.nausea_tolerance.push(nausea_tolerance);
        self.favourite_ride_types.push(favourite_types);
        self.theming_care.push(random.range(101) as u8);
        self.preferred_theme
            .push(random.range(usize::from(THEME_COUNT)) as u8);
        self.spending.push(spending);
        self.height_cm.push(height_cm);
        self.walking_speed.push(speed);
        self.appearance.push(Appearance {
            skin_tone: random.range(usize::from(SKIN_TONE_COUNT)) as u8,
            hair_style: random.range(usize::from(HAIR_STYLE_COUNT)) as u8,
            clothing_style: random.range(usize::from(CLOTHING_STYLE_COUNT)) as u8,
            body_shape: random.range(usize::from(BODY_SHAPE_COUNT)) as u8,
        });
        self.memories.push(GuestMemory::default());
        id
    }

    fn index(&self, id: GuestId) -> usize {
        let index = id.0 as usize;
        assert_eq!(self.ids[index], id, "unknown guest id");
        index
    }

    pub fn ids(&self) -> impl Iterator<Item = GuestId> + '_ {
        self.ids.iter().copied()
    }

    pub fn age(&self, id: GuestId) -> AgeGroup {
        self.ages[self.index(id)]
    }
    pub fn intensity(&self, id: GuestId) -> u8 {
        self.intensity[self.index(id)]
    }
    pub fn nausea_tolerance(&self, id: GuestId) -> u8 {
        self.nausea_tolerance[self.index(id)]
    }
    pub fn favourite_ride_types(&self, id: GuestId) -> u8 {
        self.favourite_ride_types[self.index(id)]
    }
    pub fn cares_about_theming(&self, id: GuestId) -> u8 {
        self.theming_care[self.index(id)]
    }
    pub fn preferred_theme(&self, id: GuestId) -> u8 {
        self.preferred_theme[self.index(id)]
    }
    pub fn spending(&self, id: GuestId) -> SpendingLevel {
        self.spending[self.index(id)]
    }
    pub fn height_cm(&self, id: GuestId) -> u8 {
        self.height_cm[self.index(id)]
    }
    pub fn walking_speed(&self, id: GuestId) -> u8 {
        self.walking_speed[self.index(id)]
    }
    pub fn appearance(&self, id: GuestId) -> Appearance {
        self.appearance[self.index(id)]
    }
    pub fn memory(&self, id: GuestId) -> &GuestMemory {
        &self.memories[self.index(id)]
    }

    pub fn record_memory(&mut self, id: GuestId, entry: MemoryEntry) {
        let index = self.index(id);
        self.memories[index].push(entry);
    }

    pub fn is_active(&self, id: GuestId) -> bool {
        self.active[self.index(id)]
    }

    pub fn mark_left(&mut self, id: GuestId) {
        let index = self.index(id);
        self.active[index] = false;
    }

    pub fn mark_returned(&mut self, id: GuestId) {
        let index = self.index(id);
        assert!(!self.active[index], "guest is already active");
        self.active[index] = true;
    }

    pub fn parents(&self, child: GuestId) -> Vec<GuestId> {
        self.relationships_for(child)
            .filter_map(|relation| match *relation {
                Relationship::ParentChild {
                    parent,
                    child: relation_child,
                } if relation_child == child => Some(parent),
                _ => None,
            })
            .collect()
    }

    pub fn children(&self, parent: GuestId) -> Vec<GuestId> {
        self.relationships_for(parent)
            .filter_map(|relation| match *relation {
                Relationship::ParentChild {
                    parent: relation_parent,
                    child,
                } if relation_parent == parent => Some(child),
                _ => None,
            })
            .collect()
    }

    pub fn partners(&self, guest: GuestId) -> Vec<GuestId> {
        self.relationships_for(guest)
            .filter_map(|relation| match *relation {
                Relationship::Partners { first, second } if first == guest => Some(second),
                Relationship::Partners { first, second } if second == guest => Some(first),
                _ => None,
            })
            .collect()
    }

    pub fn friends(&self, guest: GuestId) -> Vec<GuestId> {
        self.relationships_for(guest)
            .filter_map(|relation| match *relation {
                Relationship::Friends { first, second } if first == guest => Some(second),
                Relationship::Friends { first, second } if second == guest => Some(first),
                _ => None,
            })
            .collect()
    }

    fn relationships_for(&self, guest: GuestId) -> impl Iterator<Item = &Relationship> {
        let group_index = self.group_indices[self.index(guest)] as usize;
        self.groups[group_index].relations.iter()
    }

    #[cfg(test)]
    fn mean_intensity(&self, guests: &[GuestId]) -> f64 {
        guests
            .iter()
            .map(|&id| f64::from(self.intensity(id)))
            .sum::<f64>()
            / guests.len() as f64
    }

    #[cfg(test)]
    fn premium_fraction(&self, guests: &[GuestId]) -> f64 {
        guests
            .iter()
            .filter(|&&id| self.spending(id) == SpendingLevel::Premium)
            .count() as f64
            / guests.len() as f64
    }

    pub fn memory_bytes(&self) -> usize {
        self.memories.capacity() * size_of::<GuestMemory>()
    }
}

pub fn generate_guests(seed: u64, guest_count: usize) -> GuestStore {
    let mut random = Random::new(seed);
    let mut store = GuestStore::with_capacity(guest_count);
    while store.ids.len() < guest_count {
        let remaining = guest_count - store.ids.len();
        let group_roll = random.range(100);
        let requested_kind = match group_roll {
            0..20 => GroupKind::Single,
            20..45 => GroupKind::Couple,
            45..80 => GroupKind::Family,
            _ => GroupKind::Friends,
        };
        let (kind, member_ages) = match requested_kind {
            GroupKind::Single => (GroupKind::Single, vec![random.adult_age()]),
            GroupKind::Couple if remaining >= 2 => (
                GroupKind::Couple,
                vec![random.adult_age(), random.adult_age()],
            ),
            GroupKind::Family if remaining >= 3 => {
                let adults = 1 + random.range(2);
                let children = 1 + random.range(3).min(remaining.saturating_sub(adults + 1));
                if remaining < adults + children {
                    (GroupKind::Single, vec![random.adult_age()])
                } else {
                    (
                        GroupKind::Family,
                        std::iter::repeat_n(AgeGroup::Adult, adults)
                            .chain(std::iter::repeat_n(AgeGroup::Child, children))
                            .collect(),
                    )
                }
            }
            GroupKind::Friends if remaining >= 2 => {
                let count = 2 + random.range(4).min(remaining - 2);
                (
                    GroupKind::Friends,
                    (0..count).map(|_| random.adult_age()).collect(),
                )
            }
            _ => (GroupKind::Single, vec![random.adult_age()]),
        };
        let group_index = store.groups.len() as u32;
        let members: Vec<_> = member_ages
            .into_iter()
            .map(|age| store.add_guest(age, kind, group_index, &mut random))
            .collect();
        let relations = match kind {
            GroupKind::Couple => vec![Relationship::Partners {
                first: members[0],
                second: members[1],
            }],
            GroupKind::Family => {
                let child_start = members
                    .iter()
                    .position(|&member| store.age(member) == AgeGroup::Child)
                    .unwrap();
                let mut relations = Vec::new();
                for &child in &members[child_start..] {
                    for &parent in &members[..child_start] {
                        relations.push(Relationship::ParentChild { parent, child });
                    }
                }
                relations
            }
            GroupKind::Friends => members
                .iter()
                .enumerate()
                .flat_map(|(index, &first)| {
                    members[index + 1..]
                        .iter()
                        .map(move |&second| Relationship::Friends { first, second })
                })
                .collect(),
            GroupKind::Single => Vec::new(),
        };
        store.groups.push(GuestGroup {
            kind,
            members,
            relations,
        });
    }
    store
}

struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    fn range(&mut self, upper: usize) -> usize {
        (self.next() % upper as u64) as usize
    }
    fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
        self.range(denominator) < numerator
    }
    fn adult_age(&mut self) -> AgeGroup {
        if self.chance(1, 10) {
            AgeGroup::Senior
        } else {
            AgeGroup::Adult
        }
    }
}

fn shift(value: i16, amount: i16) -> u8 {
    (value + amount).clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::{AgeGroup, GroupKind, GuestMemory, GuestStore, MemoryEntry, generate_guests};
    use crate::{Executor, SequentialExecutor};

    #[test]
    fn guest_generation_is_reproducible() {
        assert_eq!(generate_guests(42, 2_000), generate_guests(42, 2_000));
        assert_ne!(generate_guests(42, 100), generate_guests(43, 100));
    }

    #[test]
    fn families_have_children_with_parent_relations() {
        let guests = generate_guests(7, 10_000);
        assert!(
            guests
                .groups
                .iter()
                .any(|group| group.kind == GroupKind::Family)
        );
        for group in guests
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Family)
        {
            let children: Vec<_> = group
                .members
                .iter()
                .filter(|&&id| guests.age(id) == AgeGroup::Child)
                .collect();
            assert!((1..=3).contains(&children.len()));
            for &&child in &children {
                let parents = guests.parents(child);
                assert!(!parents.is_empty());
                assert!(
                    parents
                        .iter()
                        .all(|&parent| guests.children(parent).contains(&child))
                );
            }
        }
        for group in guests
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Couple)
        {
            let [first, second] = group.members[..] else {
                panic!("couples must contain two guests");
            };
            assert_eq!(guests.partners(first), vec![second]);
            assert_eq!(guests.partners(second), vec![first]);
        }
        for group in guests
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Friends)
        {
            for &guest in &group.members {
                for &friend in &group.members {
                    if guest != friend {
                        assert!(guests.friends(guest).contains(&friend));
                        assert!(guests.friends(friend).contains(&guest));
                    }
                }
            }
        }
    }

    #[test]
    fn trait_distributions_and_correlations_match_documentation() {
        let guests = generate_guests(99, 100_000);
        let group_count = guests.groups.len() as f64;
        for (kind, expected_fraction) in [
            (GroupKind::Single, 0.20),
            (GroupKind::Couple, 0.25),
            (GroupKind::Family, 0.35),
            (GroupKind::Friends, 0.20),
        ] {
            let observed_fraction = guests
                .groups
                .iter()
                .filter(|group| group.kind == kind)
                .count() as f64
                / group_count;
            assert!(
                (expected_fraction - 0.02..expected_fraction + 0.02).contains(&observed_fraction),
                "{kind:?}: {observed_fraction}"
            );
        }
        let child_ids: Vec<_> = guests
            .ids()
            .filter(|&id| guests.age(id) == AgeGroup::Child)
            .collect();
        let adult_ids: Vec<_> = guests
            .ids()
            .filter(|&id| guests.age(id) == AgeGroup::Adult)
            .collect();
        let child_intensity = guests.mean_intensity(&child_ids);
        let adult_intensity = guests.mean_intensity(&adult_ids);
        assert!(
            (46.0..54.0).contains(&adult_intensity),
            "adult intensity mean: {adult_intensity}"
        );
        assert!(
            (42.0..50.0).contains(&child_intensity),
            "child intensity mean: {child_intensity}"
        );
        assert!(child_intensity < adult_intensity);
        let child_nausea = child_ids
            .iter()
            .map(|&id| f64::from(guests.nausea_tolerance(id)))
            .sum::<f64>()
            / child_ids.len() as f64;
        let adult_nausea = adult_ids
            .iter()
            .map(|&id| f64::from(guests.nausea_tolerance(id)))
            .sum::<f64>()
            / adult_ids.len() as f64;
        assert!(
            (57.0..62.0).contains(&child_nausea),
            "child nausea mean: {child_nausea}"
        );
        assert!((47.0..53.0).contains(&adult_nausea));
        let theming_mean = guests
            .ids()
            .map(|id| f64::from(guests.cares_about_theming(id)))
            .sum::<f64>()
            / 100_000.0;
        assert!((48.0..52.0).contains(&theming_mean));
        let preferred_theme_uses: Vec<_> = (0..super::THEME_COUNT)
            .map(|theme| {
                guests
                    .ids()
                    .filter(|&id| guests.preferred_theme(id) == theme)
                    .count()
            })
            .collect();
        assert!(
            preferred_theme_uses
                .iter()
                .all(|&count| (11_500..13_500).contains(&count))
        );
        assert!(guests.ids().all(|id| {
            let favourites = guests.favourite_ride_types(id).count_ones();
            (1..=2).contains(&favourites)
        }));
        let premium = guests.premium_fraction(&adult_ids);
        assert!(
            (0.16..0.24).contains(&premium),
            "adult premium fraction: {premium}"
        );
        let couple_adults: Vec<_> = guests
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Couple)
            .flat_map(|group| group.members.iter().copied())
            .filter(|&id| guests.age(id) == AgeGroup::Adult)
            .collect();
        let family_adults: Vec<_> = guests
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Family)
            .flat_map(|group| group.members.iter().copied())
            .filter(|&id| guests.age(id) == AgeGroup::Adult)
            .collect();
        assert!(guests.premium_fraction(&couple_adults) > guests.premium_fraction(&family_adults));
        let short_children = child_ids
            .iter()
            .filter(|&&id| guests.height_cm(id) < 140)
            .count();
        assert_eq!(short_children, child_ids.len());
    }

    #[test]
    fn guest_memory_is_fixed_capacity_and_population_stays_under_budget() {
        let mut memory = GuestMemory::default();
        for tick in 0..12 {
            memory.push(MemoryEntry {
                ride_id: tick,
                enjoyment: 50,
                nausea: 20,
                tick: u64::from(tick),
            });
        }
        assert_eq!(memory.len(), GuestMemory::CAPACITY);
        assert_eq!(memory.iter().next().unwrap().ride_id, 4);

        let guests = generate_guests(12, 10_000);
        let bytes = guests.memory_bytes();
        println!(
            "10,000 guests: {bytes} bytes memory storage ({:.2} bytes per guest)",
            bytes as f64 / 10_000.0
        );
        assert!(bytes < 2_000_000, "memory storage used {bytes} bytes");
    }

    #[test]
    fn guest_store_preserves_ids_after_leaving() {
        let mut guests: GuestStore = generate_guests(3, 4);
        guests.mark_left(super::GuestId(1));
        assert!(!guests.is_active(super::GuestId(1)));
        guests.mark_returned(super::GuestId(1));
        assert!(guests.is_active(super::GuestId(1)));
        assert!(matches!(
            guests.age(super::GuestId(2)),
            AgeGroup::Adult | AgeGroup::Child | AgeGroup::Senior
        ));
    }

    #[test]
    fn executor_accepts_guest_phase_output() {
        let guests = generate_guests(31, 64);
        let output = SequentialExecutor::new(11)
            .execute(64, |index| guests.intensity(super::GuestId(index as u32)));
        assert_eq!(
            output,
            (0..64)
                .map(|index| guests.intensity(super::GuestId(index)))
                .collect::<Vec<_>>()
        );
    }

    #[cfg(feature = "threads")]
    #[test]
    fn guest_phase_output_is_independent_of_thread_count() {
        use crate::ThreadedExecutor;

        let guests = generate_guests(31, 10_000);
        let sequential = SequentialExecutor::new(173).execute(guests.ids.len(), |index| {
            guests.intensity(super::GuestId(index as u32))
        });
        for thread_count in [1, 2, 4] {
            let threaded = ThreadedExecutor::new(thread_count, 257)
                .execute(guests.ids.len(), |index| {
                    guests.intensity(super::GuestId(index as u32))
                });
            assert_eq!(sequential, threaded);
        }
    }
}
