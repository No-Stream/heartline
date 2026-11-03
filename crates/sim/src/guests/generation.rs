//! Deterministic guest and group generation.
use super::groups::{GuestGroup, Relationship};
use super::storage::GuestStore;
use super::traits::{AgeGroup, GroupKind};

pub fn generate_guests(seed: u64, guest_count: usize) -> GuestStore {
    let mut random = Random::new(seed);
    let mut store = GuestStore::with_capacity(seed, guest_count);
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

pub(super) struct Random(u64);

impl Random {
    pub(super) fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    pub(super) fn range(&mut self, upper: usize) -> usize {
        (self.next() % upper as u64) as usize
    }
    pub(super) fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
        self.range(denominator) < numerator
    }
    pub(super) fn adult_age(&mut self) -> AgeGroup {
        if self.chance(1, 10) {
            AgeGroup::Senior
        } else {
            AgeGroup::Adult
        }
    }
}

pub(super) fn shift(value: i16, amount: i16) -> u8 {
    (value + amount).clamp(0, 100) as u8
}
