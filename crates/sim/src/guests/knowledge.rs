//! Places guests know about and the services those places provide.
use crate::world::Position;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceKind {
    Ride,
    Food,
    Drink,
    Restroom,
    Bench,
    Souvenir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownPlace {
    pub id: u32,
    pub position: Position,
    pub kind: PlaceKind,
    pub minimum_height_cm: u8,
    pub intensity: u8,
    pub ride_type: u8,
    pub theme: u8,
    pub price: u16,
}
