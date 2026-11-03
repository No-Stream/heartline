//! Placeable buildings and their guest-facing services.
use crate::guest_behavior::{Facility, KnownPlace, PlaceKind};
use crate::world::{Direction, Position, World};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FacilityKind {
    Entrance,
    Food,
    Drink,
    Restroom,
    Souvenir,
    Carousel,
    FerrisWheel,
}

impl FacilityKind {
    pub fn footprint(self, direction: Direction) -> (u16, u16) {
        match (self, direction) {
            (Self::Entrance, Direction::East | Direction::West) => (4, 1),
            (Self::Entrance, _) => (1, 4),
            _ => (2, 2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Building {
    pub id: u32,
    pub kind: FacilityKind,
    pub position: Position,
    pub direction: Direction,
    pub price: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementError {
    OutsideWorld,
    InvalidTerrain,
    Occupied,
    EntranceExists,
    UnknownBuilding,
    InvalidPrice,
    MissingAccess,
}

#[derive(Default)]
pub struct ParkFacilities {
    buildings: Vec<Building>,
    next_id: u32,
}

impl ParkFacilities {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    /// Returns whether a path at this height would overlap a placed building footprint.
    pub fn occupies_path_position(&self, position: Position) -> bool {
        self.buildings.iter().any(|building| {
            if position.height != building.position.height {
                return false;
            }
            let (width, height) = building.kind.footprint(building.direction);
            position.x >= building.position.x
                && u32::from(position.x) < u32::from(building.position.x) + u32::from(width)
                && position.y >= building.position.y
                && u32::from(position.y) < u32::from(building.position.y) + u32::from(height)
        })
    }

    pub fn place(
        &mut self,
        world: &World,
        kind: FacilityKind,
        position: Position,
        direction: Direction,
    ) -> Result<u32, PlacementError> {
        let (width, height) = world.dimensions();
        let (building_width, building_height) = kind.footprint(direction);
        let end_x = u32::from(position.x) + u32::from(building_width);
        let end_y = u32::from(position.y) + u32::from(building_height);
        if end_x > u32::from(width) || end_y > u32::from(height) {
            return Err(PlacementError::OutsideWorld);
        }
        for y in position.y..end_y as u16 {
            for x in position.x..end_x as u16 {
                let surface = world.surface(x, y);
                if surface.corners != [position.height; 4]
                    || surface.water.is_some_and(|water| water > position.height)
                {
                    return Err(PlacementError::InvalidTerrain);
                }
                if world
                    .path(Position {
                        x,
                        y,
                        height: position.height,
                    })
                    .is_some()
                    || self.buildings.iter().any(|building| {
                        let (w, h) = building.kind.footprint(building.direction);
                        x >= building.position.x
                            && u32::from(x) < u32::from(building.position.x) + u32::from(w)
                            && y >= building.position.y
                            && u32::from(y) < u32::from(building.position.y) + u32::from(h)
                    })
                {
                    return Err(PlacementError::Occupied);
                }
            }
        }
        if kind == FacilityKind::Entrance
            && self.buildings.iter().any(|building| building.kind == kind)
        {
            return Err(PlacementError::EntranceExists);
        }
        let candidate = Building {
            id: 0,
            kind,
            position,
            direction,
            price: 0,
        };
        if Self::access(world, &candidate).is_none()
            || (matches!(kind, FacilityKind::Carousel | FacilityKind::FerrisWheel)
                && Self::access_at(world, &candidate, 1).is_none())
        {
            return Err(PlacementError::MissingAccess);
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("facility ID exhausted");
        let price = match kind {
            FacilityKind::Food | FacilityKind::Drink => 3,
            FacilityKind::Souvenir => 5,
            FacilityKind::Carousel => 4,
            FacilityKind::FerrisWheel => 5,
            _ => 0,
        };
        self.buildings.push(Building {
            id,
            kind,
            position,
            direction,
            price,
        });
        Ok(id)
    }
    pub fn remove(&mut self, id: u32) -> bool {
        let original = self.buildings.len();
        self.buildings.retain(|building| building.id != id);
        self.buildings.len() != original
    }
    pub fn set_price(&mut self, id: u32, price: u16) -> Result<(), PlacementError> {
        let building = self
            .buildings
            .iter_mut()
            .find(|building| building.id == id)
            .ok_or(PlacementError::UnknownBuilding)?;
        if building.kind == FacilityKind::Entrance && price != 0 {
            return Err(PlacementError::InvalidPrice);
        }
        building.price = price;
        Ok(())
    }
    fn access(world: &World, building: &Building) -> Option<Position> {
        Self::access_at(world, building, 0)
    }
    fn access_at(world: &World, building: &Building, offset: u16) -> Option<Position> {
        let (width, height) = building.kind.footprint(building.direction);
        let (dx, dy) = match building.direction {
            Direction::North => (i32::from(offset), -1),
            Direction::East => (i32::from(width), i32::from(offset)),
            Direction::South => (i32::from(offset), i32::from(height)),
            Direction::West => (-1, i32::from(offset)),
        };
        let x = i32::from(building.position.x) + dx;
        let y = i32::from(building.position.y) + dy;
        let (world_width, world_height) = world.dimensions();
        if x < 0 || y < 0 || x >= i32::from(world_width) || y >= i32::from(world_height) {
            return None;
        }
        for tile_y in building.position.y..building.position.y + height {
            for tile_x in building.position.x..building.position.x + width {
                let surface = world.surface(tile_x, tile_y);
                if surface.corners != [building.position.height; 4]
                    || surface
                        .water
                        .is_some_and(|water| water > building.position.height)
                    || world
                        .path(Position {
                            x: tile_x,
                            y: tile_y,
                            height: building.position.height,
                        })
                        .is_some()
                {
                    return None;
                }
            }
        }
        let position = Position {
            x: x as u16,
            y: y as u16,
            height: building.position.height,
        };
        world
            .path(position)
            .filter(|path| path.slope.is_none())
            .map(|_| position)
    }
    pub fn entrance(&self, world: &World) -> Option<Position> {
        self.buildings
            .iter()
            .find(|building| building.kind == FacilityKind::Entrance)
            .and_then(|building| Self::access(world, building))
    }
    pub fn guest_facilities(&self, world: &World) -> Vec<Facility> {
        self.buildings
            .iter()
            .filter_map(|building| {
                let position = Self::access(world, building)?;
                let (kind, capacity, duration_ticks, intensity, ride_type) = match building.kind {
                    FacilityKind::Entrance => return None,
                    FacilityKind::Food => (PlaceKind::Food, 4, 600, 0, 0),
                    FacilityKind::Drink => (PlaceKind::Drink, 4, 300, 0, 0),
                    FacilityKind::Restroom => (PlaceKind::Restroom, 4, 450, 0, 0),
                    FacilityKind::Souvenir => (PlaceKind::Souvenir, 4, 450, 0, 0),
                    FacilityKind::Carousel => (PlaceKind::Ride, 12, 1800, 15, 0),
                    FacilityKind::FerrisWheel => (PlaceKind::Ride, 16, 2400, 25, 1),
                };
                let exit_position = match building.kind {
                    FacilityKind::Carousel | FacilityKind::FerrisWheel => {
                        Some(Self::access_at(world, building, 1)?)
                    }
                    _ => None,
                };
                Some(Facility {
                    exit_position,
                    place: KnownPlace {
                        id: building.id,
                        position,
                        kind,
                        minimum_height_cm: match building.kind {
                            FacilityKind::Carousel => 95,
                            FacilityKind::FerrisWheel => 110,
                            _ => 0,
                        },
                        intensity,
                        ride_type,
                        theme: 0,
                        price: building.price,
                    },
                    capacity,
                    duration_ticks,
                })
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Path, Surface};

    fn world() -> World {
        let mut world = World::new(
            5,
            7,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: Position {
                x: 2,
                y: 1,
                height: 0,
            },
            slope: None,
        });
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 1,
                height: 0,
            },
            slope: None,
        });
        world
    }

    #[test]
    fn placement_validates_terrain_occupancy_and_entrance_uniqueness() {
        let mut world = world();
        let mut park = ParkFacilities::new();
        let p = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        assert_eq!(
            park.place(
                &world,
                FacilityKind::Entrance,
                Position {
                    x: 0,
                    y: 2,
                    height: 0
                },
                Direction::North
            ),
            Err(PlacementError::MissingAccess)
        );
        let id = park
            .place(&world, FacilityKind::Entrance, p, Direction::North)
            .unwrap();
        assert_eq!(
            park.entrance(&world),
            Some(Position {
                x: 2,
                y: 1,
                height: 0
            })
        );
        assert!(
            park.place(&world, FacilityKind::Food, p, Direction::North)
                .is_err()
        );
        assert!(
            park.place(
                &world,
                FacilityKind::Entrance,
                Position {
                    x: 3,
                    y: 2,
                    height: 0
                },
                Direction::North
            )
            .is_err()
        );
        assert!(park.remove(id));
        assert_eq!(park.entrance(&world), None);
        world.set_surface(
            2,
            2,
            Surface {
                corners: [0, 1, 1, 0],
                water: None,
            },
        );
        assert!(
            park.place(&world, FacilityKind::Food, p, Direction::North)
                .is_err()
        );
    }

    #[test]
    fn footprints_cover_every_tile_and_rotate_the_entrance() {
        let mut world = world();
        let mut park = ParkFacilities::new();
        let position = Position {
            x: 0,
            y: 2,
            height: 0,
        };
        world.place_path(Path {
            position: Position {
                x: 4,
                y: 2,
                height: 0,
            },
            slope: None,
        });
        let id = park
            .place(&world, FacilityKind::Entrance, position, Direction::East)
            .unwrap();
        assert_eq!(FacilityKind::Entrance.footprint(Direction::East), (4, 1));
        assert_eq!(park.set_price(id, 1), Err(PlacementError::InvalidPrice));
        assert_eq!(
            park.place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 2,
                    height: 0
                },
                Direction::North
            ),
            Err(PlacementError::Occupied)
        );
        assert!(park.remove(id));
        world.place_path(Path {
            position: Position {
                x: 1,
                y: 3,
                height: 0,
            },
            slope: None,
        });
        assert_eq!(
            park.place(&world, FacilityKind::Food, position, Direction::North),
            Err(PlacementError::Occupied)
        );
    }

    #[test]
    fn path_occupancy_reports_only_tiles_at_the_building_walkable_height() {
        let world = world();
        let mut park = ParkFacilities::new();
        let position = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        park.place(&world, FacilityKind::Food, position, Direction::North)
            .unwrap();

        assert!(park.occupies_path_position(position));
        assert!(park.occupies_path_position(Position {
            x: 3,
            y: 3,
            height: 0
        }));
        assert!(!park.occupies_path_position(Position {
            x: 4,
            y: 3,
            height: 0
        }));
        assert!(!park.occupies_path_position(Position {
            x: 2,
            y: 2,
            height: 1
        }));
    }

    #[test]
    fn rides_enforce_their_actual_height_limits() {
        use crate::guest_behavior::GuestBehavior;
        use crate::guests::generate_guests;
        let mut park = ParkFacilities::new();
        let world = world();
        let position = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        let generated = generate_guests(7, 1_000);
        for kind in [FacilityKind::Carousel, FacilityKind::FerrisWheel] {
            let id = park
                .place(&world, kind, position, Direction::North)
                .unwrap();
            let facility = park.guest_facilities(&world)[0];
            let short = generated
                .ids()
                .find(|&guest| generated.height_cm(guest) < facility.place.minimum_height_cm)
                .expect("generated roster contains a short guest");
            let tall = generated
                .ids()
                .find(|&guest| generated.height_cm(guest) >= facility.place.minimum_height_cm)
                .unwrap();
            let mut behavior = GuestBehavior::default();
            behavior.observe(facility.place);
            assert!(behavior.choose(&generated, short).is_none());
            assert_eq!(behavior.choose(&generated, tall).unwrap().id, id);
            park.remove(id);
        }
    }

    #[test]
    fn facilities_expose_only_valid_connected_access_and_current_prices() {
        let mut world = world();
        let mut park = ParkFacilities::new();
        let p = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        let id = park
            .place(&world, FacilityKind::Carousel, p, Direction::North)
            .unwrap();
        park.set_price(id, 7).unwrap();
        let facilities = park.guest_facilities(&world);
        assert_eq!(facilities.len(), 1);
        assert_eq!(facilities[0].place.price, 7);
        assert_eq!(facilities[0].place.kind, PlaceKind::Ride);
        world.remove_path(Position {
            x: 2,
            y: 1,
            height: 0,
        });
        assert!(park.guest_facilities(&world).is_empty());
    }
    #[test]
    fn each_placed_service_completes_the_expected_activity() {
        use crate::guest_behavior::BehaviorSimulation;
        use crate::guests::{GuestId, generate_guests};
        let world = world();
        for kind in [
            FacilityKind::Food,
            FacilityKind::Drink,
            FacilityKind::Restroom,
            FacilityKind::Souvenir,
            FacilityKind::Carousel,
            FacilityKind::FerrisWheel,
        ] {
            let mut park = ParkFacilities::new();
            let id = park
                .place(
                    &world,
                    kind,
                    Position {
                        x: 2,
                        y: 2,
                        height: 0,
                    },
                    Direction::North,
                )
                .unwrap();
            let facilities = park.guest_facilities(&world);
            let mut guests = generate_guests(31, 1);
            let mut simulation = BehaviorSimulation::new(1);
            simulation.guests[0].needs.hunger = 95;
            simulation.guests[0].needs.thirst = 95;
            simulation.guests[0].needs.restroom = 95;
            if kind == FacilityKind::Souvenir {
                simulation.guests[0].rides_completed = 1;
            }
            let start_rides = simulation.guests[0].rides_completed;
            let path = facilities[0].place.position;
            for _ in 0..3_000 {
                simulation.tick(
                    &mut guests,
                    &[Some(path)],
                    &facilities,
                    &[Some(0)],
                    &[Some(0)],
                );
                if simulation.guests[0].stops_completed > 0
                    || simulation.guests[0].rides_completed > start_rides
                {
                    break;
                }
            }
            let behavior = &simulation.guests[0];
            match kind {
                FacilityKind::Food => assert!(behavior.needs.hunger < 95),
                FacilityKind::Drink => assert!(behavior.needs.thirst < 95),
                FacilityKind::Restroom => assert!(behavior.needs.restroom < 95),
                FacilityKind::Souvenir => assert!(behavior.souvenir_purchased),
                FacilityKind::Carousel | FacilityKind::FerrisWheel => {
                    assert_eq!(behavior.rides_completed, start_rides + 1);
                    assert!(
                        guests
                            .memory(GuestId(0))
                            .iter()
                            .any(|memory| memory.ride_id == id)
                    );
                }
                FacilityKind::Entrance => unreachable!(),
            }
        }
    }

    #[test]
    fn placed_buildings_drive_visits_and_revenue() {
        use crate::park_guests::ParkGuests;
        let mut world = World::new(
            12,
            5,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..12 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        let mut park = ParkFacilities::new();
        for (x, kind) in [
            FacilityKind::Entrance,
            FacilityKind::Food,
            FacilityKind::Drink,
            FacilityKind::Restroom,
            FacilityKind::Carousel,
            FacilityKind::FerrisWheel,
        ]
        .into_iter()
        .enumerate()
        {
            park.place(
                &world,
                kind,
                Position {
                    x: if x == 0 { 0 } else { (2 * x - 1) as u16 },
                    y: 1,
                    height: 0,
                },
                Direction::North,
            )
            .unwrap();
        }
        let mut guests = ParkGuests::new(31, 1, &world, park.entrance(&world).unwrap(), 1);
        let facilities = park.guest_facilities(&world);
        for _ in 0..150_000 {
            guests.tick(&world, &facilities, 100, Some(1));
            if guests.behaviors().guests[0].rides_completed >= 2
                && guests.behaviors().guests[0].stops_completed >= 2
            {
                break;
            }
        }
        assert!(guests.behaviors().guests[0].rides_completed >= 2);
        assert!(guests.behaviors().guests[0].stops_completed >= 2);
        assert!(
            !guests
                .walking()
                .traits()
                .memory(crate::guests::GuestId(0))
                .is_empty()
        );
        assert!(guests.revenue() > 0);
        guests.set_entrance(&world, None);
        assert!(
            guests
                .tick(&world, &facilities, 100, Some(1))
                .arrived
                .is_empty()
        );
    }
}
