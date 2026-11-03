//! Placeable buildings and their guest-facing services.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::guests::behavior::{Facility, KnownPlace, PlaceKind};
use crate::state_hash::StateHasher;
use crate::world::{Direction, Ground, Position, World};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathOpening {
    pub position: Position,
    /// Cardinal path-edge mask: north, east, south, west are bits 0 through 3.
    pub mask: u8,
}

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

/// Finds the first facing whose complete entrance has flat path access.
pub fn facility_access_direction(
    world: &World,
    kind: FacilityKind,
    position: Position,
    preferred: Direction,
) -> Option<Direction> {
    [
        preferred,
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ]
    .into_iter()
    .find(|&direction| {
        let building = Building {
            id: 0,
            kind,
            position,
            direction,
            price: 0,
        };
        let access_tiles = if matches!(kind, FacilityKind::Carousel | FacilityKind::FerrisWheel) {
            2
        } else {
            1
        };
        (0..access_tiles).all(|offset| {
            access_position(world, &building, offset)
                .is_some_and(|access| world.path(access).is_some_and(|path| path.slope.is_none()))
        })
    })
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
    MissingAccess,
    QueueMustEndAtFacility,
    ExitMustBeNormal,
    QueueConnectedToExit,
    QueueAlreadyOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FacilityAccessError {
    QueueMustEndAtFacility,
    ExitMustBeNormal,
    QueueConnectedToExit,
    QueueAlreadyOwned,
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
    pub fn entrance_price(&self) -> u16 {
        self.buildings
            .iter()
            .find(|building| building.kind == FacilityKind::Entrance)
            .map_or(0, |building| building.price)
    }

    pub fn state_hash(&self) -> u64 {
        let mut hash = StateHasher::new();
        hash.write_tag(b"ParkFacilities-v1");
        hash.write_u32(self.next_id);
        hash.write_len(self.buildings.len());
        for building in &self.buildings {
            hash.write_u32(building.id);
            hash.write_u8(building.kind as u8);
            hash.write_u16(building.position.x);
            hash.write_u16(building.position.y);
            hash.write_i16(building.position.height);
            hash.write_u8(building.direction as u8);
            hash.write_u16(building.price);
        }
        hash.finish()
    }

    /// Returns the path tiles and edges that face each placed building.
    /// The openings describe the building footprint even when a path is missing.
    pub fn access_openings(&self, world: &World) -> Vec<PathOpening> {
        self.buildings
            .iter()
            .flat_map(|building| {
                let direction = direction_toward_building(building.direction);
                let offsets = if matches!(
                    building.kind,
                    FacilityKind::Carousel | FacilityKind::FerrisWheel
                ) {
                    0..2
                } else {
                    0..1
                };
                offsets.filter_map(move |offset| {
                    access_position(world, building, offset).map(|position| PathOpening {
                        position,
                        mask: direction_mask(direction),
                    })
                })
            })
            .collect()
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

    fn terrain_supports_facility(world: &World, x: u16, y: u16, height: i16) -> bool {
        let surface = world.surface(x, y);
        if surface.corners == [height; 4] {
            return !surface.water.is_some_and(|water| water > height);
        }
        let Some(floor) = height.checked_sub(1) else {
            return false;
        };
        let Some(headroom) = height.checked_add(1) else {
            return false;
        };
        world.ground(x, y, floor) == Ground::Solid
            && world.ground(x, y, height) == Ground::Carved
            && world.ground(x, y, headroom) == Ground::Carved
            && !surface
                .water
                .is_some_and(|water| (height..=headroom).contains(&water))
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
                if !Self::terrain_supports_facility(world, x, y, position.height) {
                    return Err(PlacementError::InvalidTerrain);
                }
                if world.scenery().any(|scenery| {
                    scenery.position.x == x
                        && scenery.position.y == y
                        && scenery.clearance_box().max.z > f64::from(position.height)
                        && scenery.clearance_box().min.z
                            < f64::from(position.height)
                                + if matches!(
                                    kind,
                                    FacilityKind::Carousel | FacilityKind::FerrisWheel
                                ) {
                                    12.0
                                } else {
                                    4.0
                                }
                }) || world
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
        let mut proposed = Self {
            buildings: self.buildings.clone(),
            next_id: self.next_id,
        };
        proposed.buildings.push(candidate);
        validate_facility_path_access(world, &proposed, None).map_err(PlacementError::from)?;
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
        building.price = price;
        Ok(())
    }
    fn access(world: &World, building: &Building) -> Option<Position> {
        Self::access_at(world, building, 0)
    }
    fn access_at(world: &World, building: &Building, offset: u16) -> Option<Position> {
        let (width, height) = building.kind.footprint(building.direction);
        for tile_y in building.position.y..building.position.y + height {
            for tile_x in building.position.x..building.position.x + width {
                if !Self::terrain_supports_facility(world, tile_x, tile_y, building.position.height)
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
        let position = access_position(world, building, offset)?;
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
        let mut topology = QueueTopology::default();
        let queue_owners = queue_owner_counts(world, self.buildings.iter(), &mut topology);
        self.buildings
            .iter()
            .filter_map(|building| {
                let access = checked_building_access(world, building, &mut topology).ok()??;
                if access
                    .queue_component
                    .is_some_and(|component| queue_owners[&component] != 1)
                {
                    return None;
                }
                let (kind, capacity, duration_ticks, intensity, ride_type) = match building.kind {
                    FacilityKind::Entrance => return None,
                    FacilityKind::Food => (PlaceKind::Food, 4, 600, 0, 0),
                    FacilityKind::Drink => (PlaceKind::Drink, 4, 300, 0, 0),
                    FacilityKind::Restroom => (PlaceKind::Restroom, 4, 450, 0, 0),
                    FacilityKind::Souvenir => (PlaceKind::Souvenir, 4, 450, 0, 0),
                    FacilityKind::Carousel => (PlaceKind::Ride, 12, 1800, 15, 0),
                    FacilityKind::FerrisWheel => (PlaceKind::Ride, 16, 2400, 25, 1),
                };
                Some(Facility {
                    exit_position: access.exit_position,
                    place: KnownPlace {
                        id: building.id,
                        position: access.position,
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

impl From<FacilityAccessError> for PlacementError {
    fn from(error: FacilityAccessError) -> Self {
        match error {
            FacilityAccessError::QueueMustEndAtFacility => Self::QueueMustEndAtFacility,
            FacilityAccessError::ExitMustBeNormal => Self::ExitMustBeNormal,
            FacilityAccessError::QueueConnectedToExit => Self::QueueConnectedToExit,
            FacilityAccessError::QueueAlreadyOwned => Self::QueueAlreadyOwned,
        }
    }
}

pub fn validate_facility_path_access(
    world: &World,
    facilities: &ParkFacilities,
    coaster: Option<&crate::track::construction::Construction>,
) -> Result<(), FacilityAccessError> {
    let mut topology = QueueTopology::default();
    let mut owners = BTreeMap::<Position, usize>::new();
    for building in facilities
        .buildings()
        .iter()
        .filter(|building| building.kind != FacilityKind::Entrance)
    {
        if let Some(CheckedBuildingAccess {
            queue_component: Some(component),
            ..
        }) = checked_building_access(world, building, &mut topology)?
        {
            *owners.entry(component).or_default() += 1;
        }
    }
    if let Some(station) = coaster.and_then(|construction| {
        construction
            .pieces()
            .iter()
            .find(|piece| piece.id == crate::track::PieceId::Station)
    }) {
        let (queue, exit) =
            station_positions(station).ok_or(FacilityAccessError::QueueMustEndAtFacility)?;
        validate_normal_exit(world, exit)?;
        if let Some(path) = world.path(queue) {
            if world.path_metadata(path.position).unwrap().kind == crate::world::PathKind::Queue {
                let component = queue_endpoint_component(world, &mut topology, queue)?;
                *owners.entry(component).or_default() += 1;
            }
        }
    }
    if owners.values().any(|&count| count > 1) {
        return Err(FacilityAccessError::QueueAlreadyOwned);
    }
    Ok(())
}

fn checked_building_access(
    world: &World,
    building: &Building,
    topology: &mut QueueTopology,
) -> Result<Option<CheckedBuildingAccess>, FacilityAccessError> {
    let exit_position = if matches!(
        building.kind,
        FacilityKind::Carousel | FacilityKind::FerrisWheel
    ) {
        let position = access_position(world, building, 1);
        if let Some(position) = position {
            if world.path(position).is_some() {
                validate_normal_exit(world, position)?;
                if world
                    .path(position)
                    .is_some_and(|path| path.slope.is_none())
                {
                    Some(position)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let Some(entrance) = ParkFacilities::access(world, building) else {
        return Ok(None);
    };
    if matches!(
        building.kind,
        FacilityKind::Carousel | FacilityKind::FerrisWheel
    ) && exit_position.is_none()
    {
        return Ok(None);
    }
    let component = match world.path_metadata(entrance).unwrap().kind {
        crate::world::PathKind::Normal => None,
        crate::world::PathKind::Queue => Some(queue_endpoint_component(world, topology, entrance)?),
    };
    Ok(Some(CheckedBuildingAccess {
        position: entrance,
        exit_position,
        queue_component: component,
    }))
}

fn validate_normal_exit(world: &World, exit: Position) -> Result<(), FacilityAccessError> {
    if world
        .path_metadata(exit)
        .is_some_and(|metadata| metadata.kind != crate::world::PathKind::Normal)
    {
        return Err(FacilityAccessError::ExitMustBeNormal);
    }
    if world.neighbours(exit).into_iter().any(|position| {
        world
            .path_metadata(position)
            .is_some_and(|metadata| metadata.kind == crate::world::PathKind::Queue)
    }) {
        return Err(FacilityAccessError::QueueConnectedToExit);
    }
    Ok(())
}

fn queue_endpoint_component(
    world: &World,
    topology: &mut QueueTopology,
    position: Position,
) -> Result<Position, FacilityAccessError> {
    let Some(component) = topology.component(world, position) else {
        return Err(FacilityAccessError::QueueMustEndAtFacility);
    };
    topology
        .ends
        .contains(&position)
        .then_some(component)
        .ok_or(FacilityAccessError::QueueMustEndAtFacility)
}

fn queue_owner_counts<'a>(
    world: &World,
    buildings: impl Iterator<Item = &'a Building>,
    topology: &mut QueueTopology,
) -> BTreeMap<Position, usize> {
    let mut owners = BTreeMap::new();
    for building in buildings.filter(|building| building.kind != FacilityKind::Entrance) {
        if let Ok(Some(CheckedBuildingAccess {
            queue_component: Some(component),
            ..
        })) = checked_building_access(world, building, topology)
        {
            *owners.entry(component).or_default() += 1;
        }
    }
    owners
}

struct CheckedBuildingAccess {
    position: Position,
    exit_position: Option<Position>,
    queue_component: Option<Position>,
}

#[derive(Default)]
struct QueueTopology {
    component_for: BTreeMap<Position, Position>,
    ends: BTreeSet<Position>,
}

impl QueueTopology {
    fn component(&mut self, world: &World, start: Position) -> Option<Position> {
        if let Some(&component) = self.component_for.get(&start) {
            return Some(component);
        }
        if world.path_metadata(start)?.kind != crate::world::PathKind::Queue {
            return None;
        }
        let mut pending = VecDeque::from([start]);
        let mut visited = BTreeSet::from([start]);
        let mut members = Vec::new();
        let mut ends = Vec::new();
        while let Some(position) = pending.pop_front() {
            members.push(position);
            let mut queue_degree = 0;
            let mut touches_normal = false;
            for neighbour in world.neighbours(position) {
                match world.path_metadata(neighbour).unwrap().kind {
                    crate::world::PathKind::Normal => touches_normal = true,
                    crate::world::PathKind::Queue => {
                        queue_degree += 1;
                        if visited.insert(neighbour) {
                            pending.push_back(neighbour);
                        }
                    }
                }
            }
            if queue_degree <= 1 && (!touches_normal || queue_degree == 0) {
                ends.push(position);
            }
        }
        let component = *members.iter().min().unwrap();
        for position in members {
            self.component_for.insert(position, component);
        }
        self.ends.extend(ends);
        Some(component)
    }
}

fn access_position(world: &World, building: &Building, offset: u16) -> Option<Position> {
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
    Some(Position {
        x: x as u16,
        y: y as u16,
        height: building.position.height,
    })
}

fn direction_toward_building(facing: Direction) -> Direction {
    match facing {
        Direction::North => Direction::South,
        Direction::East => Direction::West,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
    }
}

fn direction_mask(direction: Direction) -> u8 {
    match direction {
        Direction::North => 1,
        Direction::East => 2,
        Direction::South => 4,
        Direction::West => 8,
    }
}

pub fn station_access_openings(station: &crate::track::Piece) -> Option<[PathOpening; 2]> {
    let (queue, exit) = station_positions(station)?;
    let start = station.entry.position();
    let end = station.exit.position();
    let access_x = (start.x + (end.x - start.x) / 4.0) / 4.0;
    let access_y = (start.y + (end.y - start.y) / 4.0) / 4.0;
    let toward_station = |position: Position| {
        let dx = access_x - (f64::from(position.x) + 0.5);
        let dy = access_y - (f64::from(position.y) + 0.5);
        if dx.abs() >= dy.abs() {
            if dx > 0.0 {
                Some(Direction::East)
            } else if dx < 0.0 {
                Some(Direction::West)
            } else {
                None
            }
        } else if dy > 0.0 {
            Some(Direction::South)
        } else if dy < 0.0 {
            Some(Direction::North)
        } else {
            None
        }
    };
    Some([
        PathOpening {
            position: queue,
            mask: direction_mask(toward_station(queue)?),
        },
        PathOpening {
            position: exit,
            mask: direction_mask(toward_station(exit)?),
        },
    ])
}

pub fn coaster_access_openings(
    construction: &crate::track::construction::Construction,
) -> Option<[PathOpening; 2]> {
    let station = construction
        .pieces()
        .iter()
        .find(|piece| piece.id == crate::track::PieceId::Station)?;
    station_access_openings(station)
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
    fn state_hash_is_reproducible_and_covers_facility_edits() {
        let world = world();
        let position = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        let mut first = ParkFacilities::new();
        let mut second = ParkFacilities::new();
        assert_eq!(first.state_hash(), second.state_hash());
        let first_id = first
            .place(&world, FacilityKind::Food, position, Direction::North)
            .unwrap();
        let second_id = second
            .place(&world, FacilityKind::Food, position, Direction::North)
            .unwrap();
        assert_eq!(first_id, second_id);
        assert_eq!(first.state_hash(), second.state_hash());
        first.set_price(first_id, 8).unwrap();
        assert_ne!(first.state_hash(), second.state_hash());
    }

    #[test]
    fn access_direction_prefers_requested_facing_then_cardinal_order() {
        let mut world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 2,
                height: 0,
            },
            slope: None,
        });
        let position = Position {
            x: 3,
            y: 3,
            height: 0,
        };
        assert_eq!(
            facility_access_direction(&world, FacilityKind::Food, position, Direction::South),
            Some(Direction::North)
        );
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 5,
                height: 0,
            },
            slope: None,
        });
        assert_eq!(
            facility_access_direction(&world, FacilityKind::Food, position, Direction::South),
            Some(Direction::South)
        );
    }

    #[test]
    fn ride_access_direction_requires_both_flat_access_tiles() {
        let mut world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 2,
                height: 0,
            },
            slope: None,
        });
        let position = Position {
            x: 3,
            y: 3,
            height: 0,
        };
        assert_eq!(
            facility_access_direction(&world, FacilityKind::Carousel, position, Direction::North),
            None
        );
        world.place_path(Path {
            position: Position {
                x: 4,
                y: 2,
                height: 0,
            },
            slope: None,
        });
        assert_eq!(
            facility_access_direction(&world, FacilityKind::Carousel, position, Direction::North),
            Some(Direction::North)
        );
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
        park.set_price(id, 1).unwrap();
        assert_eq!(
            park.buildings()
                .iter()
                .find(|building| building.id == id)
                .unwrap()
                .price,
            1
        );
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
    fn building_openings_follow_the_access_side_even_when_paths_are_removed() {
        let cases = [
            (
                Direction::North,
                Position {
                    x: 3,
                    y: 2,
                    height: 0,
                },
                4,
            ),
            (
                Direction::East,
                Position {
                    x: 5,
                    y: 3,
                    height: 0,
                },
                8,
            ),
            (
                Direction::South,
                Position {
                    x: 3,
                    y: 5,
                    height: 0,
                },
                1,
            ),
            (
                Direction::West,
                Position {
                    x: 2,
                    y: 3,
                    height: 0,
                },
                2,
            ),
        ];
        for (direction, access, mask) in cases {
            let mut world = World::new(
                8,
                8,
                Surface {
                    corners: [0; 4],
                    water: None,
                },
            );
            world.place_path(Path {
                position: access,
                slope: None,
            });
            let mut park = ParkFacilities::new();
            park.place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 3,
                    height: 0,
                },
                direction,
            )
            .unwrap();

            let expected = PathOpening {
                position: access,
                mask,
            };
            assert_eq!(park.access_openings(&world), vec![expected]);
            world.remove_path(access);
            assert_eq!(park.access_openings(&world), vec![expected]);
        }
    }

    #[test]
    fn flat_ride_reports_separate_entrance_and_exit_openings() {
        let mut world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for position in [
            Position {
                x: 3,
                y: 2,
                height: 0,
            },
            Position {
                x: 4,
                y: 2,
                height: 0,
            },
        ] {
            world.place_path(Path {
                position,
                slope: None,
            });
        }
        let mut park = ParkFacilities::new();
        park.place(
            &world,
            FacilityKind::Carousel,
            Position {
                x: 3,
                y: 3,
                height: 0,
            },
            Direction::North,
        )
        .unwrap();

        assert_eq!(
            park.access_openings(&world),
            vec![
                PathOpening {
                    position: Position {
                        x: 3,
                        y: 2,
                        height: 0
                    },
                    mask: 4,
                },
                PathOpening {
                    position: Position {
                        x: 4,
                        y: 2,
                        height: 0
                    },
                    mask: 4,
                },
            ]
        );
    }

    #[test]
    fn queue_access_requires_a_terminal_tile_but_may_be_unconnected_yet() {
        use crate::world::PathKind;
        let mut world = World::new(
            8,
            7,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        world
            .build_path_route(
                Position {
                    x: 1,
                    y: 1,
                    height: 0,
                },
                Position {
                    x: 5,
                    y: 1,
                    height: 0,
                },
                PathKind::Queue,
                0,
            )
            .unwrap();
        let mut park = ParkFacilities::new();
        assert_eq!(
            park.place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 2,
                    height: 0
                },
                Direction::North,
            ),
            Err(PlacementError::QueueMustEndAtFacility)
        );

        world.remove_path(Position {
            x: 3,
            y: 1,
            height: 0,
        });
        world
            .build_path_route(
                Position {
                    x: 3,
                    y: 2,
                    height: 0,
                },
                Position {
                    x: 3,
                    y: 2,
                    height: 0,
                },
                PathKind::Queue,
                0,
            )
            .unwrap();
        assert!(
            park.place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 3,
                    height: 0
                },
                Direction::North,
            )
            .is_ok()
        );
    }

    #[test]
    fn queue_access_is_exclusive_and_flat_ride_exit_must_be_normal() {
        use crate::world::PathKind;
        let mut world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let queue = Position {
            x: 2,
            y: 2,
            height: 0,
        };
        world
            .build_path_route(queue, queue, PathKind::Queue, 0)
            .unwrap();
        let mut park = ParkFacilities::new();
        park.place(
            &world,
            FacilityKind::Food,
            Position {
                x: 0,
                y: 2,
                height: 0,
            },
            Direction::East,
        )
        .unwrap();
        assert_eq!(
            park.place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 2,
                    height: 0
                },
                Direction::West,
            ),
            Err(PlacementError::QueueAlreadyOwned)
        );

        let mut ride_world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        ride_world
            .build_path_route(
                Position {
                    x: 2,
                    y: 0,
                    height: 0,
                },
                Position {
                    x: 2,
                    y: 1,
                    height: 0,
                },
                PathKind::Queue,
                0,
            )
            .unwrap();
        ride_world
            .build_path_route(
                Position {
                    x: 3,
                    y: 1,
                    height: 0,
                },
                Position {
                    x: 3,
                    y: 1,
                    height: 0,
                },
                PathKind::Queue,
                0,
            )
            .unwrap();
        assert_eq!(
            ParkFacilities::new().place(
                &ride_world,
                FacilityKind::Carousel,
                Position {
                    x: 2,
                    y: 2,
                    height: 0
                },
                Direction::North,
            ),
            Err(PlacementError::ExitMustBeNormal)
        );
    }

    #[test]
    fn a_ride_exit_cannot_be_the_queues_explicit_normal_entrance() {
        use crate::world::PathKind;
        let mut world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let queue_entrance = Position {
            x: 2,
            y: 1,
            height: 0,
        };
        let queue_start = Position {
            x: 2,
            y: 0,
            height: 0,
        };
        let exit = Position {
            x: 3,
            y: 1,
            height: 0,
        };
        world
            .build_path_route(queue_start, queue_entrance, PathKind::Queue, 0)
            .unwrap();
        world
            .build_path_route(exit, exit, PathKind::Normal, 0)
            .unwrap();
        let mut park = ParkFacilities::new();
        park.place(
            &world,
            FacilityKind::Carousel,
            Position {
                x: 2,
                y: 2,
                height: 0,
            },
            Direction::North,
        )
        .unwrap();

        let proposed = world
            .preview_path_world(exit, queue_entrance, PathKind::Normal)
            .unwrap();
        assert_eq!(
            validate_facility_path_access(&proposed, &park, None),
            Err(FacilityAccessError::QueueConnectedToExit)
        );
        world
            .build_path_route(exit, queue_entrance, PathKind::Normal, 0)
            .unwrap();
        assert_eq!(
            validate_facility_path_access(&world, &park, None),
            Err(FacilityAccessError::QueueConnectedToExit)
        );
        assert_eq!(
            ParkFacilities::new().place(
                &world,
                FacilityKind::Carousel,
                Position {
                    x: 2,
                    y: 2,
                    height: 0,
                },
                Direction::North,
            ),
            Err(PlacementError::QueueConnectedToExit)
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
        use crate::guests::behavior::GuestBehavior;
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
    fn cavern_shop_has_headroom_and_connects_to_the_entrance_path() {
        use crate::guests::park::ParkGuests;
        use crate::world::Ground;

        let mut world = World::new(
            6,
            7,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        for y in 1..6 {
            world.set_surface(
                1,
                y,
                Surface {
                    corners: [1; 4],
                    water: None,
                },
            );
        }
        for (x, y) in [(2, 1), (3, 1), (3, 2), (3, 3), (4, 2), (4, 3)] {
            world.carve(x, y, 1, 4);
        }
        let entrance_path = Position {
            x: 1,
            y: 1,
            height: 1,
        };
        let tunnel_path = Position {
            x: 2,
            y: 1,
            height: 1,
        };
        let shop_path = Position {
            x: 3,
            y: 1,
            height: 1,
        };
        for position in [entrance_path, tunnel_path, shop_path] {
            world.place_path(Path {
                position,
                slope: None,
            });
        }
        assert_eq!(world.ground(3, 2, 1), Ground::Carved);
        assert_eq!(world.ground(3, 2, 2), Ground::Carved);

        let mut park = ParkFacilities::new();
        let entrance = park
            .place(
                &world,
                FacilityKind::Entrance,
                Position {
                    x: 1,
                    y: 2,
                    height: 1,
                },
                Direction::North,
            )
            .unwrap();
        let shop = park
            .place(
                &world,
                FacilityKind::Food,
                Position {
                    x: 3,
                    y: 2,
                    height: 1,
                },
                Direction::North,
            )
            .unwrap();
        let entrance_position = park.entrance(&world).unwrap();
        let food = park
            .guest_facilities(&world)
            .into_iter()
            .find(|facility| facility.place.id == shop)
            .unwrap();
        let guests = ParkGuests::new(9, 12, &world, entrance_position, 1);
        assert_eq!(
            guests.walking().path_component(entrance_position),
            guests.walking().path_component(food.place.position)
        );
        assert!(
            park.buildings()
                .iter()
                .any(|building| building.id == entrance)
        );

        let mut shallow_cavern = World::new(
            4,
            4,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        shallow_cavern.carve(1, 1, 1, 2);
        shallow_cavern.carve(2, 1, 1, 2);
        shallow_cavern.carve(1, 2, 1, 2);
        shallow_cavern.carve(2, 2, 1, 2);
        shallow_cavern.carve(1, 0, 1, 2);
        shallow_cavern.place_path(Path {
            position: Position {
                x: 1,
                y: 0,
                height: 1,
            },
            slope: None,
        });
        let mut shallow_park = ParkFacilities::new();
        assert_eq!(
            shallow_park.place(
                &shallow_cavern,
                FacilityKind::Food,
                Position {
                    x: 1,
                    y: 1,
                    height: 1,
                },
                Direction::North,
            ),
            Err(PlacementError::InvalidTerrain)
        );
    }
    #[test]
    fn each_placed_service_completes_the_expected_activity() {
        use crate::guests::behavior::BehaviorSimulation;
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
        use crate::guests::park::ParkGuests;
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

#[cfg(test)]
mod coaster_access_tests {
    use super::*;
    use crate::track::{PieceId, TrackState, catalog};
    use crate::world::{Path, PathKind, Surface};

    #[test]
    fn raised_station_paths_clear_complete_circuit_and_adjacent_lift() {
        use crate::track::clearance::check_clearance;
        use crate::track::{Heading, hybrid_model};
        use crate::world::{Brush, TerrainCommand};
        use PieceId::*;
        let catalog = catalog();
        let mut state = TrackState {
            x: 482,
            y: 498,
            z: 6,
            heading: Heading::North,
            ..TrackState::origin()
        };
        let mut pieces = Vec::new();
        for id in [
            Station,
            LiftLevelToGentleUp,
            Lift,
            Lift,
            Lift,
            Lift,
            Lift,
            LiftGentleUpToLevel,
            LiftFlat,
            Straight,
            Straight,
            TurnLargeEntry,
            TurnLargeExit,
            LevelToGentleDown,
            GentleDown,
            GentleDown,
            GentleDown,
            GentleDown,
            GentleDown,
            GentleDownToLevel,
            Loop,
            TurnLargeEntry,
            TurnLargeExit,
        ] {
            let piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            state = piece.exit;
            pieces.push(piece);
        }
        let (queue, exit) = station_positions(&pieces[0]).unwrap();
        let mut world = World::new(
            512,
            512,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        for (y, height) in [
            (255, 8),
            (254, 8),
            (253, 9),
            (252, 10),
            (251, 11),
            (250, 12),
        ] {
            for x in [238, 244] {
                world
                    .apply_terrain(TerrainCommand::Flatten {
                        area: Brush { x, y, size: 1 },
                        height,
                    })
                    .unwrap();
            }
        }
        for (position, kind) in [(queue, PathKind::Queue), (exit, PathKind::Normal)] {
            world
                .apply_terrain(TerrainCommand::Flatten {
                    area: Brush {
                        x: position.x,
                        y: position.y,
                        size: 1,
                    },
                    height: position.height,
                })
                .unwrap();
            world.build_path_route(position, position, kind, 0).unwrap();
        }
        for piece in &pieces {
            assert_eq!(
                check_clearance(&world, piece, &hybrid_model().train, &[], &[]),
                Ok(()),
                "piece {:?}, queue {queue:?}, exit {exit:?}",
                piece.id
            );
        }
    }

    #[test]
    fn raised_station_paths_clear_track_for_all_headings() {
        use crate::track::clearance::check_clearance;
        use crate::track::{Heading, hybrid_model};
        use crate::world::{Brush, TerrainCommand};
        for heading in [
            Heading::East,
            Heading::NorthEast,
            Heading::North,
            Heading::NorthWest,
            Heading::West,
            Heading::SouthWest,
            Heading::South,
            Heading::SouthEast,
        ] {
            for (x, y) in [(60, 60), (60, 61), (61, 60), (61, 61)] {
                let mut world = World::new(
                    64,
                    64,
                    Surface {
                        corners: [0; 4],
                        water: None,
                    },
                );
                let station = catalog()
                    .into_iter()
                    .find(|piece| piece.id == PieceId::Station)
                    .unwrap()
                    .at(TrackState {
                        x,
                        y,
                        z: 6,
                        heading,
                        ..TrackState::origin()
                    })
                    .unwrap();
                let (queue, exit) = station_positions(&station).unwrap();
                for (position, kind) in [(queue, PathKind::Queue), (exit, PathKind::Normal)] {
                    world
                        .apply_terrain(TerrainCommand::Flatten {
                            area: Brush {
                                x: position.x,
                                y: position.y,
                                size: 1,
                            },
                            height: position.height,
                        })
                        .unwrap();
                    world.build_path_route(position, position, kind, 0).unwrap();
                }
                assert_eq!(
                    check_clearance(&world, &station, &hybrid_model().train, &[], &[]),
                    Ok(()),
                    "heading {heading:?} at ({x}, {y}): {queue:?}, {exit:?}"
                );
            }
        }
    }

    #[test]
    fn station_access_uses_nearby_queue_and_exit_at_track_height() {
        let mut world = World::new(
            10,
            5,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let queue = Position {
            x: 3,
            y: 0,
            height: 0,
        };
        let exit = Position {
            x: 3,
            y: 4,
            height: 0,
        };
        world.place_path(Path {
            position: exit,
            slope: None,
        });
        world
            .build_path_route(queue, queue, PathKind::Queue, 0)
            .unwrap();
        let station = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Station)
            .unwrap()
            .at(TrackState {
                x: 4,
                y: 4,
                ..TrackState::origin()
            })
            .unwrap();
        assert_eq!(station_access(&world, &station), Some((queue, exit)));
    }

    #[test]
    fn station_access_rejects_queue_interiors_and_queue_exits() {
        use crate::world::PathKind;
        let station = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Station)
            .unwrap()
            .at(TrackState {
                x: 4,
                y: 4,
                ..TrackState::origin()
            })
            .unwrap();
        let (queue, exit) = station_positions(&station).unwrap();
        let surface = Surface {
            corners: [0; 4],
            water: None,
        };

        let mut interior_world = World::new(10, 5, surface);
        interior_world
            .build_path_route(
                Position {
                    x: 1,
                    y: queue.y,
                    height: queue.height,
                },
                Position {
                    x: 5,
                    y: queue.y,
                    height: queue.height,
                },
                PathKind::Queue,
                0,
            )
            .unwrap();
        interior_world
            .build_path_route(exit, exit, PathKind::Normal, 0)
            .unwrap();
        assert_eq!(station_access(&interior_world, &station), None);

        let mut queue_exit_world = World::new(10, 5, surface);
        queue_exit_world
            .build_path_route(queue, queue, PathKind::Queue, 0)
            .unwrap();
        queue_exit_world
            .build_path_route(exit, exit, PathKind::Queue, 0)
            .unwrap();
        assert_eq!(station_access(&queue_exit_world, &station), None);
    }

    #[test]
    fn station_openings_point_from_each_side_toward_the_track() {
        use crate::track::{Heading, TrackState};
        for heading in [
            Heading::East,
            Heading::NorthEast,
            Heading::North,
            Heading::NorthWest,
            Heading::West,
            Heading::SouthWest,
            Heading::South,
            Heading::SouthEast,
        ] {
            let station = catalog()
                .into_iter()
                .find(|piece| piece.id == PieceId::Station)
                .unwrap()
                .at(TrackState {
                    x: 30,
                    y: 30,
                    heading,
                    ..TrackState::origin()
                })
                .unwrap();
            let openings = station_access_openings(&station).unwrap();
            assert_eq!(openings[0].mask.count_ones(), 1, "{heading:?}");
            assert_eq!(openings[1].mask.count_ones(), 1, "{heading:?}");
            assert_eq!(openings[0].mask & openings[1].mask, 0, "{heading:?}");

            let start = station.entry.position();
            let end = station.exit.position();
            let center_x = (start.x + (end.x - start.x) / 4.0) / 4.0;
            let center_y = (start.y + (end.y - start.y) / 4.0) / 4.0;
            for opening in openings {
                let (direction_x, direction_y) = match opening.mask {
                    1 => (0.0, -1.0),
                    2 => (1.0, 0.0),
                    4 => (0.0, 1.0),
                    8 => (-1.0, 0.0),
                    _ => panic!("invalid station opening mask {}", opening.mask),
                };
                let toward_track_x = center_x - (f64::from(opening.position.x) + 0.5);
                let toward_track_y = center_y - (f64::from(opening.position.y) + 0.5);
                assert!(
                    toward_track_x * direction_x + toward_track_y * direction_y > 0.0,
                    "{heading:?}, {opening:?}"
                );
            }
        }
    }
}

/// Returns the guest-facing station when both queue and exit paths are present.
pub fn coaster_facility(
    world: &World,
    construction: &crate::track::construction::Construction,
    id: u32,
    price: u16,
) -> Option<Facility> {
    if !construction.is_closed() {
        return None;
    }
    let station = construction
        .pieces()
        .iter()
        .find(|piece| piece.id == crate::track::PieceId::Station)?;
    let (position, exit) = station_access(world, station)?;
    let model = construction.model();
    Some(Facility {
        place: KnownPlace {
            id,
            position,
            kind: PlaceKind::Ride,
            minimum_height_cm: u8::try_from(model.minimum_height_cm)
                .expect("coaster height limit exceeds guest height representation"),
            intensity: 50,
            ride_type: 2,
            theme: 0,
            price,
        },
        capacity: usize::from(model.train.cars)
            * usize::from(model.train.rows_per_car)
            * usize::from(model.train.seats_per_row),
        duration_ticks: 1,
        exit_position: Some(exit),
    })
}

/// Fixed queue and exit tiles on opposite sides of the station, near its entry.
pub fn station_positions(station: &crate::track::Piece) -> Option<(Position, Position)> {
    let start = station.entry.position();
    let end = station.exit.position();
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length == 0.0 {
        return None;
    }
    let access_x = start.x + dx / 4.0;
    let access_y = start.y + dy / 4.0;
    let position = |side: f64| -> Option<Position> {
        let x = ((access_x - side * dy / length * 8.0) / 4.0).floor();
        let y = ((access_y + side * dx / length * 8.0) / 4.0).floor();
        if x < 0.0
            || y < 0.0
            || x > f64::from(u16::MAX)
            || y > f64::from(u16::MAX)
            || start.z < f64::from(i16::MIN)
            || start.z > f64::from(i16::MAX)
        {
            return None;
        }
        Some(Position {
            x: x as u16,
            y: y as u16,
            height: start.z as i16,
        })
    };
    let queue = position(-1.0)?;
    let exit = position(1.0)?;
    (queue != exit).then_some((queue, exit))
}

fn station_access(world: &World, station: &crate::track::Piece) -> Option<(Position, Position)> {
    use crate::world::PathKind;
    let (queue, exit) = station_positions(station)?;
    for (position, kind) in [(queue, PathKind::Queue), (exit, PathKind::Normal)] {
        let path = world.path(position)?;
        if path.slope.is_some() || world.path_metadata(position)?.kind != kind {
            return None;
        }
    }
    let mut topology = QueueTopology::default();
    queue_endpoint_component(world, &mut topology, queue).ok()?;
    validate_normal_exit(world, exit).ok()?;
    Some((queue, exit))
}
