use super::{Ground, Path, Position, World};
use crate::track::Vec3;
use crate::track::clearance::ClearanceBox;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SceneryTheme {
    Garden = 0,
    Woodland = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum SceneryKind {
    TreeOval = 0,
    BushFlowering = 1,
    FlowerBed = 2,
    Bench = 3,
    Lamp = 4,
    Fountain = 5,
    Planter = 6,
    TreeCone = 7,
    TreeConeOchre = 8,
    TreeCypress = 9,
    TreeCypressShort = 10,
    TreeOvalCoral = 11,
    TreeTiers = 12,
    Fence = 13,
    Signpost = 14,
    Bin = 15,
    BalloonCart = 16,
}

impl SceneryKind {
    pub fn theme(self) -> SceneryTheme {
        match self {
            Self::TreeOval
            | Self::BushFlowering
            | Self::FlowerBed
            | Self::Bench
            | Self::Lamp
            | Self::Fountain
            | Self::Planter => SceneryTheme::Garden,
            _ => SceneryTheme::Woodland,
        }
    }

    fn height(self) -> i16 {
        match self {
            Self::TreeOval
            | Self::TreeCone
            | Self::TreeConeOchre
            | Self::TreeCypress
            | Self::TreeOvalCoral
            | Self::TreeTiers => 6,
            Self::TreeCypressShort | Self::Lamp | Self::Signpost | Self::BalloonCart => 3,
            _ => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Scenery {
    pub kind: SceneryKind,
    pub position: Position,
    /// Northwest, northeast, southeast, southwest.
    pub quarter: u8,
    pub rotation: u8,
}

impl Scenery {
    pub fn clearance_box(self) -> ClearanceBox {
        let whole_tile = self.kind == SceneryKind::Fountain;
        let (dx, dy) = match self.quarter {
            0 => (0.0, 0.0),
            1 => (2.0, 0.0),
            2 => (2.0, 2.0),
            3 => (0.0, 2.0),
            _ => panic!("invalid scenery quarter"),
        };
        let x = f64::from(self.position.x) * 4.0 + if whole_tile { 0.0 } else { dx };
        let y = f64::from(self.position.y) * 4.0 + if whole_tile { 0.0 } else { dy };
        let z = f64::from(self.position.height);
        let width = if whole_tile { 4.0 } else { 2.0 };
        ClearanceBox {
            min: Vec3::new(x, y, z),
            max: Vec3::new(x + width, y + width, z + f64::from(self.kind.height())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneryPlacementError {
    OutsideWorld,
    InvalidOrientation,
    InvalidTerrain,
    Occupied,
}

fn overlaps(a: ClearanceBox, b: ClearanceBox) -> bool {
    a.min.x < b.max.x
        && b.min.x < a.max.x
        && a.min.y < b.max.y
        && b.min.y < a.max.y
        && a.min.z < b.max.z
        && b.min.z < a.max.z
}

impl World {
    pub fn scenery(&self) -> impl Iterator<Item = &Scenery> {
        self.scenery.values()
    }

    pub fn nearby_scenery(
        &self,
        position: Position,
        radius: u16,
    ) -> impl Iterator<Item = &Scenery> {
        let min_x = position.x.saturating_sub(radius);
        let max_x = position.x.saturating_add(radius);
        let min_y = position.y.saturating_sub(radius);
        let max_y = position.y.saturating_add(radius);
        (min_x..=max_x)
            .flat_map(move |x| {
                self.scenery
                    .range((x, min_y, i16::MIN, 0)..=(x, max_y, i16::MAX, u8::MAX))
                    .map(|(_, scenery)| scenery)
            })
            .filter(move |scenery| {
                (i32::from(scenery.position.height) - i32::from(position.height)).abs() <= 4
            })
    }

    pub fn scenery_boxes(&self) -> impl Iterator<Item = ClearanceBox> + '_ {
        self.scenery().map(|scenery| scenery.clearance_box())
    }

    pub fn place_scenery(&mut self, scenery: Scenery) -> Result<(), SceneryPlacementError> {
        let p = scenery.position;
        if p.x >= self.width || p.y >= self.height {
            return Err(SceneryPlacementError::OutsideWorld);
        }
        if scenery.quarter > 3 || scenery.rotation > 3 {
            return Err(SceneryPlacementError::InvalidOrientation);
        }
        if !self.scenery_supported(scenery) {
            return Err(SceneryPlacementError::InvalidTerrain);
        }
        let bounds = scenery.clearance_box();
        if self
            .scenery
            .values()
            .any(|other| overlaps(bounds, other.clearance_box()))
            || self.paths().any(|path| {
                path.position.x == p.x
                    && path.position.y == p.y
                    && i32::from(path.position.height)
                        < i32::from(p.height) + i32::from(scenery.kind.height())
                    && i32::from(p.height) < path.top()
            })
        {
            return Err(SceneryPlacementError::Occupied);
        }
        self.scenery
            .insert((p.x, p.y, p.height, scenery.quarter), scenery);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub fn remove_scenery(&mut self, position: Position, quarter: u8) -> bool {
        let removed = self
            .scenery
            .remove(&(position.x, position.y, position.height, quarter))
            .is_some();
        if removed {
            self.revision = self.revision.wrapping_add(1);
        }
        removed
    }

    pub(super) fn scenery_supported(&self, scenery: Scenery) -> bool {
        let p = scenery.position;
        let Some(top) = p.height.checked_add(scenery.kind.height()) else {
            return false;
        };
        let Some(below) = p.height.checked_sub(1) else {
            return false;
        };
        let surface = self.surface(p.x, p.y);
        if surface.water.is_some_and(|water| water > p.height) {
            return false;
        }
        matches!(self.ground(p.x, p.y, below), Ground::Solid)
            && (p.height..top)
                .all(|height| matches!(self.ground(p.x, p.y, height), Ground::Air | Ground::Carved))
    }

    pub(super) fn scenery_blocks_path(&self, path: Path) -> bool {
        self.scenery.values().any(|scenery| {
            scenery.position.x == path.position.x
                && scenery.position.y == path.position.y
                && i32::from(path.position.height)
                    < i32::from(scenery.position.height) + i32::from(scenery.kind.height())
                && i32::from(scenery.position.height) < path.top()
        })
    }
}
