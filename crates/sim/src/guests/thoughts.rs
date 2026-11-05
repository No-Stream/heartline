//! The current thought shown for a guest.

use crate::track::proximity::ProximityKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Thought {
    Exploring,
    Hungry,
    Thirsty,
    NeedsRestroom,
    Nauseous,
    Tired,
    Unhappy,
    EnjoyedRide(u32),
    DislikedRide(u32),
    Proximity(u32, ProximityKind),
    EnjoyedScenery(u8),
}
