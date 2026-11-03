use std::collections::{BTreeMap, BTreeSet};

use super::{Position, Surface, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brush {
    /// Top-left tile of the brush rectangle.
    pub x: u16,
    pub y: u16,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainCommand {
    RaiseCorner {
        x: u16,
        y: u16,
        corner: Corner,
        steps: i16,
    },
    LowerCorner {
        x: u16,
        y: u16,
        corner: Corner,
        steps: i16,
    },
    RaiseTiles {
        area: Brush,
        steps: i16,
    },
    LowerTiles {
        area: Brush,
        steps: i16,
    },
    Flatten {
        area: Brush,
        height: i16,
    },
    Smooth {
        area: Brush,
    },
    SetWater {
        area: Brush,
        level: Option<i16>,
    },
    RaiseWater {
        area: Brush,
        steps: i16,
    },
    LowerWater {
        area: Brush,
        steps: i16,
    },
    Carve {
        area: Brush,
        bottom: i16,
        top: i16,
    },
    Fill {
        area: Brush,
        bottom: i16,
        top: i16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainTile {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainEditResult {
    pub changed_tiles: Vec<TerrainTile>,
    pub invalidated_paths: Vec<Position>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainEditError {
    SceneryObstructed,
    OutsideWorld,
    InvalidBrushSize,
    InvalidStepCount,
    InvalidInterval,
    CarveAboveTerrain,
    NoWater,
    HeightOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vertex {
    x: u16,
    y: u16,
}

impl Corner {
    fn vertex(self, tile_x: u16, tile_y: u16) -> Vertex {
        match self {
            Self::NorthWest => Vertex {
                x: tile_x,
                y: tile_y,
            },
            Self::NorthEast => Vertex {
                x: tile_x + 1,
                y: tile_y,
            },
            Self::SouthEast => Vertex {
                x: tile_x + 1,
                y: tile_y + 1,
            },
            Self::SouthWest => Vertex {
                x: tile_x,
                y: tile_y + 1,
            },
        }
    }

    fn index(self) -> usize {
        match self {
            Self::NorthWest => 0,
            Self::NorthEast => 1,
            Self::SouthEast => 2,
            Self::SouthWest => 3,
        }
    }
}

impl World {
    /// Return the tiles whose world data an edit can change, without changing the world.
    pub fn terrain_edit_tiles(
        &self,
        command: &TerrainCommand,
    ) -> Result<Vec<TerrainTile>, TerrainEditError> {
        let indexes = self.terrain_edit_indexes(command)?;
        Ok(indexes
            .into_iter()
            .map(|index| TerrainTile {
                x: (index % usize::from(self.width)) as u16,
                y: (index / usize::from(self.width)) as u16,
            })
            .collect())
    }

    /// Apply one validated landscape edit and report every path it invalidated.
    pub fn apply_terrain(
        &mut self,
        command: TerrainCommand,
    ) -> Result<TerrainEditResult, TerrainEditError> {
        if !self.scenery.is_empty() {
            let mut candidate = self.clone();
            let result = candidate.apply_terrain_unchecked(command)?;
            if candidate
                .scenery
                .values()
                .any(|scenery| !candidate.scenery_supported(*scenery))
            {
                return Err(TerrainEditError::SceneryObstructed);
            }
            let applied = self.apply_terrain_unchecked(command)?;
            assert_eq!(applied, result, "terrain preflight result changed");
            return Ok(applied);
        }
        self.apply_terrain_unchecked(command)
    }

    fn apply_terrain_unchecked(
        &mut self,
        command: TerrainCommand,
    ) -> Result<TerrainEditResult, TerrainEditError> {
        self.terrain_edit_indexes(&command)?;
        let mut changed_surfaces = BTreeMap::<usize, Surface>::new();
        let mut changed_carvings = BTreeMap::<usize, Vec<(i16, i16)>>::new();

        match command {
            TerrainCommand::RaiseCorner {
                x,
                y,
                corner,
                steps,
            }
            | TerrainCommand::LowerCorner {
                x,
                y,
                corner,
                steps,
            } => {
                Self::validate_steps(steps)?;
                let vertex = corner.vertex(x, y);
                let old_height = self.surface(x, y).corners[corner.index()];
                let new_height = match command {
                    TerrainCommand::RaiseCorner { .. } => old_height.checked_add(steps),
                    _ => old_height.checked_sub(steps),
                }
                .ok_or(TerrainEditError::HeightOverflow)?;
                self.write_vertex(vertex, new_height, &mut changed_surfaces);
            }
            TerrainCommand::RaiseTiles { area, steps }
            | TerrainCommand::LowerTiles { area, steps } => {
                Self::validate_steps(steps)?;
                let raise = matches!(command, TerrainCommand::RaiseTiles { .. });
                let mut vertices = BTreeSet::new();
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        for corner in [
                            Corner::NorthWest,
                            Corner::NorthEast,
                            Corner::SouthEast,
                            Corner::SouthWest,
                        ] {
                            vertices.insert(corner.vertex(x, y));
                        }
                    }
                }
                for vertex in vertices {
                    let old_height = self.vertex_height(vertex, &changed_surfaces);
                    let new_height = if raise {
                        old_height.checked_add(steps)
                    } else {
                        old_height.checked_sub(steps)
                    }
                    .ok_or(TerrainEditError::HeightOverflow)?;
                    self.write_vertex(vertex, new_height, &mut changed_surfaces);
                }
            }
            TerrainCommand::Flatten { area, height } => {
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        for corner in [
                            Corner::NorthWest,
                            Corner::NorthEast,
                            Corner::SouthEast,
                            Corner::SouthWest,
                        ] {
                            self.write_vertex(corner.vertex(x, y), height, &mut changed_surfaces);
                        }
                    }
                }
            }
            TerrainCommand::Smooth { area } => {
                let vertices = (area.y..=area.y + u16::from(area.size))
                    .flat_map(|y| {
                        (area.x..=area.x + u16::from(area.size)).map(move |x| Vertex { x, y })
                    })
                    .collect::<Vec<_>>();
                let smoothed = vertices
                    .iter()
                    .copied()
                    .map(|vertex| {
                        let mut total = 0_i32;
                        let mut count = 0_i32;
                        for y in
                            vertex.y.saturating_sub(1)..=vertex.y.saturating_add(1).min(self.height)
                        {
                            for x in vertex.x.saturating_sub(1)
                                ..=vertex.x.saturating_add(1).min(self.width)
                            {
                                total += i32::from(
                                    self.vertex_height(Vertex { x, y }, &changed_surfaces),
                                );
                                count += 1;
                            }
                        }
                        (vertex, (total / count) as i16)
                    })
                    .collect::<Vec<_>>();
                for (vertex, height) in smoothed {
                    self.write_vertex(vertex, height, &mut changed_surfaces);
                }
            }
            TerrainCommand::SetWater { area, level } => {
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        let index = self.index(x, y);
                        changed_surfaces
                            .entry(index)
                            .or_insert(self.surfaces[index])
                            .water = level;
                    }
                }
            }
            TerrainCommand::RaiseWater { area, steps }
            | TerrainCommand::LowerWater { area, steps } => {
                Self::validate_steps(steps)?;
                let raise = matches!(command, TerrainCommand::RaiseWater { .. });
                let mut water_indexes = Vec::new();
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        water_indexes.push(self.index(x, y));
                    }
                }
                if !raise
                    && water_indexes
                        .iter()
                        .all(|&index| self.surfaces[index].water.is_none())
                {
                    return Err(TerrainEditError::NoWater);
                }
                let raised_level = if raise {
                    let existing_max = water_indexes
                        .iter()
                        .filter_map(|&index| self.surfaces[index].water)
                        .max();
                    let baseline = existing_max.unwrap_or_else(|| {
                        water_indexes
                            .iter()
                            .map(|&index| *self.surfaces[index].corners.iter().min().unwrap())
                            .min()
                            .unwrap()
                    });
                    Some(
                        baseline
                            .checked_add(steps)
                            .ok_or(TerrainEditError::HeightOverflow)?,
                    )
                } else {
                    None
                };
                for index in water_indexes {
                    let ground = *self.surfaces[index].corners.iter().min().unwrap();
                    let current = self.surfaces[index].water;
                    let level = if let Some(raised_level) = raised_level {
                        (raised_level > ground).then_some(raised_level)
                    } else if let Some(current) = current {
                        let lowered = current
                            .checked_sub(steps)
                            .ok_or(TerrainEditError::HeightOverflow)?;
                        (lowered > ground).then_some(lowered)
                    } else {
                        None
                    };
                    changed_surfaces
                        .entry(index)
                        .or_insert(self.surfaces[index])
                        .water = level;
                }
            }
            TerrainCommand::Carve { area, bottom, top }
            | TerrainCommand::Fill { area, bottom, top } => {
                if bottom >= top {
                    return Err(TerrainEditError::InvalidInterval);
                }
                let carve = matches!(command, TerrainCommand::Carve { .. });
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        let index = self.index(x, y);
                        let ceiling = *self.surfaces[index].corners.iter().max().unwrap();
                        let spans = self.columns[index]
                            .as_ref()
                            .map_or_else(Vec::new, |column| {
                                column
                                    .carved
                                    .iter()
                                    .map(|span| (span.bottom, span.top))
                                    .collect()
                            });
                        let updated = if carve {
                            let clipped_top = top.min(ceiling);
                            if bottom >= clipped_top {
                                continue;
                            }
                            merge_spans(spans, (bottom, clipped_top))
                        } else {
                            if top > ceiling {
                                return Err(TerrainEditError::CarveAboveTerrain);
                            }
                            subtract_span(spans, (bottom, top))
                        };
                        changed_carvings.insert(index, updated);
                    }
                }
            }
        }

        let edited_indexes: BTreeSet<usize> = changed_surfaces
            .keys()
            .copied()
            .chain(changed_carvings.keys().copied())
            .collect();
        for (&index, surface) in &changed_surfaces {
            self.surfaces[index] = *surface;
            if let Some(column) = &mut self.columns[index] {
                let ceiling = *surface.corners.iter().max().unwrap();
                for span in &mut column.carved {
                    span.top = span.top.min(ceiling);
                }
                column.carved.retain(|span| span.bottom < span.top);
            }
        }
        for (index, spans) in changed_carvings {
            let column =
                self.columns[index].get_or_insert_with(|| Box::new(super::Column::default()));
            column.carved = spans
                .into_iter()
                .map(|(bottom, top)| super::Span { bottom, top })
                .collect();
        }

        if edited_indexes.is_empty() {
            return Ok(TerrainEditResult {
                changed_tiles: Vec::new(),
                invalidated_paths: Vec::new(),
            });
        }
        self.revision = self.revision.wrapping_add(1);
        let mut invalidated_paths = Vec::new();
        for &index in &edited_indexes {
            invalidated_paths.extend(self.invalidate_paths(index));
        }
        invalidated_paths.sort_unstable();
        Ok(TerrainEditResult {
            changed_tiles: edited_indexes
                .into_iter()
                .map(|index| TerrainTile {
                    x: (index % usize::from(self.width)) as u16,
                    y: (index / usize::from(self.width)) as u16,
                })
                .collect(),
            invalidated_paths,
        })
    }

    fn terrain_edit_indexes(
        &self,
        command: &TerrainCommand,
    ) -> Result<Vec<usize>, TerrainEditError> {
        let mut indexes = BTreeSet::new();
        match *command {
            TerrainCommand::RaiseCorner { x, y, corner, .. }
            | TerrainCommand::LowerCorner { x, y, corner, .. } => {
                if x >= self.width || y >= self.height {
                    return Err(TerrainEditError::OutsideWorld);
                }
                let vertex = corner.vertex(x, y);
                for index in self.vertex_indexes(vertex) {
                    indexes.insert(index);
                }
            }
            TerrainCommand::RaiseTiles { area, .. }
            | TerrainCommand::LowerTiles { area, .. }
            | TerrainCommand::Flatten { area, .. }
            | TerrainCommand::Smooth { area }
            | TerrainCommand::SetWater { area, .. }
            | TerrainCommand::RaiseWater { area, .. }
            | TerrainCommand::LowerWater { area, .. }
            | TerrainCommand::Carve { area, .. }
            | TerrainCommand::Fill { area, .. } => {
                self.validate_brush(area)?;
                for y in area.y..area.y + u16::from(area.size) {
                    for x in area.x..area.x + u16::from(area.size) {
                        indexes.insert(self.index(x, y));
                    }
                }
                if matches!(
                    command,
                    TerrainCommand::RaiseTiles { .. }
                        | TerrainCommand::LowerTiles { .. }
                        | TerrainCommand::Flatten { .. }
                        | TerrainCommand::Smooth { .. }
                ) {
                    let area_tiles = indexes.iter().copied().collect::<Vec<_>>();
                    let vertices = area_tiles.iter().flat_map(|&index| {
                        let x = (index % usize::from(self.width)) as u16;
                        let y = (index / usize::from(self.width)) as u16;
                        [
                            Corner::NorthWest.vertex(x, y),
                            Corner::NorthEast.vertex(x, y),
                            Corner::SouthEast.vertex(x, y),
                            Corner::SouthWest.vertex(x, y),
                        ]
                    });
                    for vertex in vertices {
                        indexes.extend(self.vertex_indexes(vertex));
                    }
                }
            }
        }
        Ok(indexes.into_iter().collect())
    }

    fn validate_brush(&self, area: Brush) -> Result<(), TerrainEditError> {
        if !(1..=9).contains(&area.size) {
            return Err(TerrainEditError::InvalidBrushSize);
        }
        let end_x = u32::from(area.x) + u32::from(area.size);
        let end_y = u32::from(area.y) + u32::from(area.size);
        if end_x > u32::from(self.width) || end_y > u32::from(self.height) {
            return Err(TerrainEditError::OutsideWorld);
        }
        Ok(())
    }

    fn validate_steps(steps: i16) -> Result<(), TerrainEditError> {
        if steps <= 0 {
            return Err(TerrainEditError::InvalidStepCount);
        }
        Ok(())
    }

    fn vertex_indexes(&self, vertex: Vertex) -> Vec<usize> {
        let mut indexes = Vec::with_capacity(4);
        for (tile_x, tile_y, _) in [
            (vertex.x, vertex.y, Corner::NorthWest),
            (vertex.x.saturating_sub(1), vertex.y, Corner::NorthEast),
            (
                vertex.x.saturating_sub(1),
                vertex.y.saturating_sub(1),
                Corner::SouthEast,
            ),
            (vertex.x, vertex.y.saturating_sub(1), Corner::SouthWest),
        ] {
            if tile_x < self.width && tile_y < self.height {
                indexes.push(self.index(tile_x, tile_y));
            }
        }
        indexes.sort_unstable();
        indexes.dedup();
        indexes
    }

    fn vertex_height(&self, vertex: Vertex, changes: &BTreeMap<usize, Surface>) -> i16 {
        let (tile_x, tile_y, corner) = if vertex.x < self.width && vertex.y < self.height {
            (vertex.x, vertex.y, Corner::NorthWest)
        } else if vertex.x > 0 && vertex.y < self.height {
            (vertex.x - 1, vertex.y, Corner::NorthEast)
        } else if vertex.x > 0 && vertex.y > 0 {
            (vertex.x - 1, vertex.y - 1, Corner::SouthEast)
        } else {
            (vertex.x, vertex.y - 1, Corner::SouthWest)
        };
        let index = self.index(tile_x, tile_y);
        changes.get(&index).unwrap_or(&self.surfaces[index]).corners[corner.index()]
    }

    fn write_vertex(&self, vertex: Vertex, height: i16, changes: &mut BTreeMap<usize, Surface>) {
        for index in self.vertex_indexes(vertex) {
            let surface = changes.entry(index).or_insert(self.surfaces[index]);
            let tile_x = (index % usize::from(self.width)) as u16;
            let tile_y = (index / usize::from(self.width)) as u16;
            let corner = if vertex.x == tile_x && vertex.y == tile_y {
                Corner::NorthWest
            } else if vertex.x == tile_x + 1 && vertex.y == tile_y {
                Corner::NorthEast
            } else if vertex.x == tile_x + 1 && vertex.y == tile_y + 1 {
                Corner::SouthEast
            } else {
                Corner::SouthWest
            };
            surface.corners[corner.index()] = height;
        }
    }
}

fn merge_spans(mut spans: Vec<(i16, i16)>, inserted: (i16, i16)) -> Vec<(i16, i16)> {
    spans.push(inserted);
    spans.sort_unstable_by_key(|span| span.0);
    let mut merged: Vec<(i16, i16)> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(previous) = merged.last_mut() {
            if span.0 <= previous.1 {
                previous.1 = previous.1.max(span.1);
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

fn subtract_span(spans: Vec<(i16, i16)>, removed: (i16, i16)) -> Vec<(i16, i16)> {
    spans
        .into_iter()
        .flat_map(|span| {
            let mut remaining = Vec::with_capacity(2);
            if span.0 < removed.0 {
                remaining.push((span.0, span.1.min(removed.0)));
            }
            if span.1 > removed.1 {
                remaining.push((span.0.max(removed.1), span.1));
            }
            remaining
        })
        .filter(|(bottom, top)| bottom < top)
        .collect()
}
