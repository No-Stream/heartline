//! The current thought shown for a guest.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
}
