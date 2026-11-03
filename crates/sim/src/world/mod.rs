//! Layered terrain and paths on a grid. Heights are quarter-tile steps.
//! Surface data is dense, while only columns with carved spans or paths allocate
//! extra storage; sorted half-open spans keep ordinary surface-only maps compact.

use std::collections::BTreeMap;
use std::mem::size_of;

mod construction;
pub use construction::{PathBuildError, PathKind, PathMetadata, PathPreviewTile};
mod terrain;
use std::sync::atomic::{AtomicUsize, Ordering};
pub use terrain::{
    Brush, Corner, TerrainCommand, TerrainEditError, TerrainEditResult, TerrainTile,
};

static NEXT_WORLD_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface {
    /// Corner heights in northwest, northeast, southeast, southwest order.
    pub corners: [i16; 4],
    pub water: Option<i16>,
}

/// Ground occupancy for one vertical step of a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Solid,
    Carved,
    Air,
    /// The sloping surface intersects this height step.
    SurfaceSlope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    /// Lowest height of the path, in quarter-tile steps.
    pub height: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    pub position: Position,
    /// A slope rises one height step towards this edge.
    pub slope: Option<Direction>,
}

impl Path {
    fn corners(self) -> [i32; 4] {
        let low = i32::from(self.position.height);
        match self.slope {
            None => [low; 4],
            Some(Direction::North) => [low + 1, low + 1, low, low],
            Some(Direction::East) => [low, low + 1, low + 1, low],
            Some(Direction::South) => [low, low, low + 1, low + 1],
            Some(Direction::West) => [low + 1, low, low, low + 1],
        }
    }

    fn edge(self, direction: Direction) -> [i32; 2] {
        let c = self.corners();
        match direction {
            Direction::North => [c[0], c[1]],
            Direction::East => [c[1], c[2]],
            Direction::South => [c[3], c[2]],
            Direction::West => [c[0], c[3]],
        }
    }

    fn top(self) -> i32 {
        i32::from(self.position.height) + if self.slope.is_some() { 2 } else { 1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    bottom: i16,
    top: i16,
}

#[derive(Clone, Default)]
struct Column {
    carved: Vec<Span>,
    paths: Vec<Path>,
    path_edges: Vec<u8>,
}

pub struct World {
    path_metadata: BTreeMap<Position, PathMetadata>,
    width: u16,
    height: u16,
    revision: u64,
    instance_id: usize,
    surfaces: Vec<Surface>,
    columns: Vec<Option<Box<Column>>>,
    occupied_columns: Vec<usize>,
}

impl Clone for World {
    fn clone(&self) -> Self {
        Self {
            path_metadata: self.path_metadata.clone(),
            width: self.width,
            height: self.height,
            revision: self.revision,
            instance_id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            surfaces: self.surfaces.clone(),
            columns: self.columns.clone(),
            occupied_columns: self.occupied_columns.clone(),
        }
    }
}

impl World {
    pub fn new(width: u16, height: u16, surface: Surface) -> Self {
        assert!(width > 0 && height > 0, "world dimensions must be positive");
        let count = usize::from(width) * usize::from(height);
        Self {
            path_metadata: BTreeMap::new(),
            width,
            height,
            revision: 0,
            instance_id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            surfaces: vec![surface; count],
            columns: (0..count).map(|_| None).collect(),
            occupied_columns: Vec::new(),
        }
    }

    pub(crate) fn instance_id(&self) -> usize {
        self.instance_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn index(&self, x: u16, y: u16) -> usize {
        assert!(x < self.width && y < self.height, "tile outside world");
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }

    pub fn surface(&self, x: u16, y: u16) -> Surface {
        self.surfaces[self.index(x, y)]
    }

    pub fn ground(&self, x: u16, y: u16, height: i16) -> Ground {
        let index = self.index(x, y);
        let surface = self.surfaces[index];
        let minimum = *surface.corners.iter().min().unwrap();
        let maximum = *surface.corners.iter().max().unwrap();
        if self.columns[index].as_ref().is_some_and(|column| {
            column
                .carved
                .iter()
                .any(|span| span.bottom <= height && height < span.top)
        }) {
            Ground::Carved
        } else if height >= maximum {
            Ground::Air
        } else if height >= minimum {
            Ground::SurfaceSlope
        } else {
            Ground::Solid
        }
    }

    /// Replaces the surface, clipping underground excavations and removing unsupported paths.
    pub fn set_surface(&mut self, x: u16, y: u16, surface: Surface) -> Vec<Position> {
        self.revision = self.revision.wrapping_add(1);
        let index = self.index(x, y);
        self.surfaces[index] = surface;
        if let Some(column) = &mut self.columns[index] {
            let ceiling = *surface.corners.iter().max().unwrap();
            for span in &mut column.carved {
                span.top = span.top.min(ceiling);
            }
            column.carved.retain(|span| span.bottom < span.top);
        }
        self.invalidate_paths(index)
    }

    /// Removes solid ground within [bottom, top), up to the highest surface corner.
    pub fn carve(&mut self, x: u16, y: u16, bottom: i16, top: i16) -> Vec<Position> {
        self.revision = self.revision.wrapping_add(1);
        let index = self.index(x, y);
        assert!(bottom < top, "empty or reversed interval");
        assert!(
            top <= *self.surfaces[index].corners.iter().max().unwrap(),
            "carve extends above surface"
        );
        let column = self.columns[index].get_or_insert_with(|| Box::new(Column::default()));
        column.carved.push(Span { bottom, top });
        column.carved.sort_unstable_by_key(|span| span.bottom);
        let mut merged: Vec<Span> = Vec::with_capacity(column.carved.len());
        for span in column.carved.drain(..) {
            if let Some(previous) = merged.last_mut() {
                if span.bottom <= previous.top {
                    previous.top = previous.top.max(span.top);
                    continue;
                }
            }
            merged.push(span);
        }
        column.carved = merged;
        self.invalidate_paths(index)
    }

    /// Restores solid ground within [bottom, top), removing obstructed paths.
    pub fn fill(&mut self, x: u16, y: u16, bottom: i16, top: i16) -> Vec<Position> {
        self.revision = self.revision.wrapping_add(1);
        let index = self.index(x, y);
        assert!(bottom < top, "empty or reversed interval");
        assert!(
            top <= *self.surfaces[index].corners.iter().max().unwrap(),
            "fill extends above surface"
        );
        if let Some(column) = &mut self.columns[index] {
            let mut remaining = Vec::new();
            for span in &column.carved {
                if span.bottom < bottom {
                    remaining.push(Span {
                        bottom: span.bottom,
                        top: span.top.min(bottom),
                    });
                }
                if span.top > top {
                    remaining.push(Span {
                        bottom: span.bottom.max(top),
                        top: span.top,
                    });
                }
            }
            column.carved = remaining;
        }
        self.invalidate_paths(index)
    }

    fn path_supported(&self, index: usize, path: Path) -> bool {
        let surface = self.surfaces[index];
        let corners = path.corners();
        if corners == surface.corners.map(i32::from) {
            let support_height = i32::from(path.position.height) - 1;
            return !self.columns[index].as_ref().is_some_and(|column| {
                column.carved.iter().any(|span| {
                    i32::from(span.bottom) <= support_height && support_height < i32::from(span.top)
                })
            });
        }
        let bottom = i32::from(path.position.height);
        let ceiling = i32::from(*surface.corners.iter().max().unwrap());
        if path.top() > ceiling {
            return false;
        }
        let Some(column) = &self.columns[index] else {
            return false;
        };
        let open = column
            .carved
            .iter()
            .any(|span| i32::from(span.bottom) <= bottom && i32::from(span.top) >= path.top());
        let support_open = column
            .carved
            .iter()
            .any(|span| i32::from(span.bottom) < bottom && i32::from(span.top) >= bottom);
        open && !support_open
    }

    fn invalidate_paths(&mut self, index: usize) -> Vec<Position> {
        let invalid: Vec<_> = self.columns[index]
            .as_ref()
            .map(|column| {
                column
                    .paths
                    .iter()
                    .copied()
                    .filter(|path| !self.path_supported(index, *path))
                    .map(|path| path.position)
                    .collect()
            })
            .unwrap_or_default();
        for position in &invalid {
            self.remove_path(*position);
        }
        if self.columns[index]
            .as_ref()
            .is_some_and(|column| column.paths.is_empty() && column.carved.is_empty())
        {
            self.columns[index] = None;
        }
        self.update_occupied_column(index);
        invalid
    }

    fn update_occupied_column(&mut self, index: usize) {
        match (
            self.occupied_columns.binary_search(&index),
            self.columns[index].is_some(),
        ) {
            (Err(position), true) => self.occupied_columns.insert(position, index),
            (Ok(position), false) => {
                self.occupied_columns.remove(position);
            }
            _ => {}
        }
    }

    pub fn place_path(&mut self, path: Path) {
        self.insert_path(path);
        let adjacent = self.geometric_neighbours(path.position);
        for other in adjacent {
            if self.path_metadata(other).unwrap().kind == PathKind::Normal {
                self.connect_paths(path.position, other);
            }
        }
    }

    fn insert_path(&mut self, path: Path) {
        self.revision = self.revision.wrapping_add(1);
        let index = self.index(path.position.x, path.position.y);
        assert!(
            self.path_supported(index, path),
            "path lacks support or clearance"
        );
        if let Some(column) = &self.columns[index] {
            assert!(
                column
                    .paths
                    .iter()
                    .all(|other| i32::from(path.position.height) >= other.top()
                        || i32::from(other.position.height) >= path.top()),
                "paths overlap"
            );
        }
        let column = self.columns[index].get_or_insert_with(|| Box::new(Column::default()));
        let insertion = column
            .paths
            .partition_point(|other| other.position.height < path.position.height);
        column.paths.insert(insertion, path);
        column.path_edges.insert(insertion, 0);
        self.update_occupied_column(index);
    }

    pub fn path(&self, position: Position) -> Option<Path> {
        let index = self.index(position.x, position.y);
        self.columns[index].as_ref().and_then(|column| {
            column
                .paths
                .iter()
                .copied()
                .find(|path| path.position == position)
        })
    }

    /// Iterates over paths in row-major column and ascending height order.
    pub fn paths(&self) -> impl Iterator<Item = Path> + '_ {
        self.occupied_columns
            .iter()
            .map(|&index| self.columns[index].as_ref().unwrap())
            .flat_map(|column| column.paths.iter().copied())
    }

    /// Iterates over excavations in row-major column and ascending height order.
    /// Each interval is half-open: [bottom, top).
    pub fn carved_spans(&self) -> impl Iterator<Item = (u16, u16, i16, i16)> + '_ {
        let width = usize::from(self.width);
        self.occupied_columns
            .iter()
            .copied()
            .flat_map(move |index| {
                let column = &self.columns[index];
                column.iter().flat_map(move |column| {
                    column.carved.iter().map(move |span| {
                        (
                            (index % width) as u16,
                            (index / width) as u16,
                            span.bottom,
                            span.top,
                        )
                    })
                })
            })
    }

    pub fn remove_path(&mut self, position: Position) -> Vec<Position> {
        self.revision = self.revision.wrapping_add(1);
        let index = self.index(position.x, position.y);
        let adjacent = self.neighbours(position);
        for other in adjacent {
            self.disconnect_paths(position, other);
        }
        let Some(column) = &mut self.columns[index] else {
            return Vec::new();
        };
        let found = column
            .paths
            .iter()
            .position(|path| path.position == position);
        let removed = found.is_some();
        if let Some(index) = found {
            column.paths.remove(index);
            column.path_edges.remove(index);
        }
        self.path_metadata.remove(&position);
        if column.paths.is_empty() && column.carved.is_empty() {
            self.columns[index] = None;
        }
        self.update_occupied_column(index);
        if removed { vec![position] } else { Vec::new() }
    }

    /// Cardinal edge mask: north, east, south, west are bits 0 through 3.
    pub fn path_edges(&self, position: Position) -> Option<u8> {
        let column = self.columns[self.index(position.x, position.y)].as_ref()?;
        let index = column
            .paths
            .iter()
            .position(|path| path.position == position)?;
        Some(column.path_edges[index])
    }

    fn set_path_edges(&mut self, position: Position, mask: u8) {
        let index = self.index(position.x, position.y);
        let column = self.columns[index].as_mut().unwrap();
        let index = column
            .paths
            .iter()
            .position(|path| path.position == position)
            .unwrap();
        column.path_edges[index] = mask;
    }

    fn edge_bits(first: Position, second: Position) -> (u8, u8) {
        match (
            i32::from(second.x) - i32::from(first.x),
            i32::from(second.y) - i32::from(first.y),
        ) {
            (0, -1) => (1, 4),
            (1, 0) => (2, 8),
            (0, 1) => (4, 1),
            (-1, 0) => (8, 2),
            _ => panic!("paths are not adjacent"),
        }
    }

    fn connect_paths(&mut self, first: Position, second: Position) {
        let (first_bit, second_bit) = Self::edge_bits(first, second);
        if self.path_edges(first).unwrap() & first_bit == 0 {
            self.revision = self.revision.wrapping_add(1);
        }
        self.set_path_edges(first, self.path_edges(first).unwrap() | first_bit);
        self.set_path_edges(second, self.path_edges(second).unwrap() | second_bit);
    }

    fn disconnect_paths(&mut self, first: Position, second: Position) {
        let (first_bit, second_bit) = Self::edge_bits(first, second);
        self.set_path_edges(first, self.path_edges(first).unwrap() & !first_bit);
        self.set_path_edges(second, self.path_edges(second).unwrap() & !second_bit);
    }

    /// Returns paths joined by recorded cardinal edges.
    pub fn neighbours(&self, position: Position) -> Vec<Position> {
        let Some(mask) = self.path_edges(position) else {
            return Vec::new();
        };
        self.geometric_neighbours(position)
            .into_iter()
            .filter(|other| {
                let (bit, opposite) = Self::edge_bits(position, *other);
                mask & bit != 0 && self.path_edges(*other).unwrap() & opposite != 0
            })
            .collect()
    }

    fn geometric_neighbours(&self, position: Position) -> Vec<Position> {
        let Some(path) = self.path(position) else {
            return Vec::new();
        };
        let mut neighbours = Vec::new();
        for (direction, dx, dy, opposite) in [
            (Direction::North, 0, -1, Direction::South),
            (Direction::East, 1, 0, Direction::West),
            (Direction::South, 0, 1, Direction::North),
            (Direction::West, -1, 0, Direction::East),
        ] {
            let x = i32::from(position.x) + dx;
            let y = i32::from(position.y) + dy;
            if x < 0 || y < 0 || x >= i32::from(self.width) || y >= i32::from(self.height) {
                continue;
            }
            let index = self.index(x as u16, y as u16);
            if let Some(column) = &self.columns[index] {
                neighbours.extend(
                    column
                        .paths
                        .iter()
                        .filter(|other| path.edge(direction) == other.edge(opposite))
                        .map(|other| other.position),
                );
            }
        }
        neighbours
    }

    /// Owned bytes, including reserved vector capacity, excluding allocator bookkeeping.
    pub fn allocated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.surfaces.capacity() * size_of::<Surface>()
            + self.columns.capacity() * size_of::<Option<Box<Column>>>()
            + self.occupied_columns.capacity() * size_of::<usize>()
            + self
                .columns
                .iter()
                .flatten()
                .map(|column| {
                    size_of::<Column>()
                        + column.carved.capacity() * size_of::<Span>()
                        + column.paths.capacity() * size_of::<Path>()
                        + column.path_edges.capacity() * size_of::<u8>()
                })
                .sum::<usize>()
    }

    /// Stable FNV-1a hash of the canonical world contents, independent of edit history.
    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        let mut write = |bytes: &[u8]| {
            for byte in bytes {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            }
        };
        write(&self.width.to_le_bytes());
        write(&self.height.to_le_bytes());
        for (surface, column) in self.surfaces.iter().zip(&self.columns) {
            for corner in surface.corners {
                write(&corner.to_le_bytes());
            }
            match surface.water {
                Some(height) => {
                    write(&[1]);
                    write(&height.to_le_bytes());
                }
                None => write(&[0]),
            }
            let spans = column.as_ref().map_or(&[][..], |column| &column.carved);
            write(&(spans.len() as u64).to_le_bytes());
            for span in spans {
                write(&span.bottom.to_le_bytes());
                write(&span.top.to_le_bytes());
            }
            let paths = column.as_ref().map_or(&[][..], |column| &column.paths);
            write(&(paths.len() as u64).to_le_bytes());
            for path in paths {
                let metadata = self.path_metadata(path.position).unwrap();
                write(&[
                    metadata.kind as u8,
                    metadata.style,
                    self.path_edges(path.position).unwrap(),
                ]);
                write(&path.position.height.to_le_bytes());
                write(&[match path.slope {
                    None => 0,
                    Some(Direction::North) => 1,
                    Some(Direction::East) => 2,
                    Some(Direction::South) => 3,
                    Some(Direction::West) => 4,
                }]);
            }
        }
        hash
    }
}

#[cfg(test)]
mod tests;
