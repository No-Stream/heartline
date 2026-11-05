//! Places guests know about and the services those places provide.
use crate::world::Position;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlaceKind {
    Ride,
    Food,
    Drink,
    Restroom,
    Bench,
    Souvenir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KnownPlace {
    pub id: u32,
    pub position: Position,
    pub kind: PlaceKind,
    pub minimum_height_cm: u8,
    pub intensity: u8,
    pub nausea: u8,
    pub ride_type: u8,
    pub theme: u8,
    pub price: u16,
}

pub(crate) fn guest_rating(score: f64) -> u8 {
    assert!(score.is_finite());
    (score * 10.0).clamp(0.0, 100.0).round() as u8
}
