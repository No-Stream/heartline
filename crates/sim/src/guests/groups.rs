//! Guest groups and their family or social relationships.
use super::traits::{GroupKind, GuestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestGroup {
    pub kind: GroupKind,
    pub members: Vec<GuestId>,
    pub(super) relations: Vec<Relationship>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Relationship {
    ParentChild { parent: GuestId, child: GuestId },
    Partners { first: GuestId, second: GuestId },
    Friends { first: GuestId, second: GuestId },
}
