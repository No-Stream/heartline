use std::collections::{BTreeSet, VecDeque};

use super::{Direction, Path, Position, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum PathKind {
    Normal = 0,
    Queue = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PathMetadata {
    pub kind: PathKind,
    pub style: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathBuildError {
    OutsideWorld,
    UnsupportedTerrain,
    EndpointHeight,
    Disconnected,
    Occupied,
    Unsupported,
    QueueConnections,
    FacilityOccupied,
    FacilityAccess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathPreviewTile {
    pub path: Path,
    pub refusal: Option<PathBuildError>,
    pub edges: u8,
}

struct RouteFailure {
    error: PathBuildError,
    prefix: Vec<Path>,
    refusal_all: bool,
}

fn recover_route(layers: &[Vec<(Path, Option<usize>)>]) -> Vec<Path> {
    let mut route = Vec::with_capacity(layers.len());
    let mut index = 0;
    for layer in layers.iter().rev() {
        let (path, previous) = layer[index];
        route.push(path);
        index = previous.unwrap_or(0);
    }
    route.reverse();
    route
}

fn route_coordinates(start: Position, end: Position) -> Vec<(u16, u16)> {
    let mut coordinates = vec![(start.x, start.y)];
    let (mut x, mut y) = (start.x, start.y);
    while x != end.x {
        x = if x < end.x { x + 1 } else { x - 1 };
        coordinates.push((x, y));
    }
    while y != end.y {
        y = if y < end.y { y + 1 } else { y - 1 };
        coordinates.push((x, y));
    }
    coordinates
}

impl World {
    pub fn path_metadata(&self, position: Position) -> Option<PathMetadata> {
        self.path(position).map(|_| {
            self.path_metadata
                .get(&position)
                .copied()
                .unwrap_or(PathMetadata {
                    kind: PathKind::Normal,
                    style: 0,
                })
        })
    }

    /// Preview an x-first cardinal route over supported terrain and excavated spaces.
    pub fn preview_path_route(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
        _style: u8,
    ) -> Result<Vec<Path>, PathBuildError> {
        self.solve_path_route(start, end, kind)
            .map(|(route, _)| route)
            .map_err(|failure| failure.error)
    }

    /// Preview the resulting path graph for validation against external facilities.
    pub fn preview_path_world(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
    ) -> Result<World, PathBuildError> {
        self.solve_path_route(start, end, kind)
            .map(|(_, world)| world)
            .map_err(|failure| failure.error)
    }

    /// Return each tile in a preview, including refused tiles after the first obstruction.
    pub fn preview_path_tiles(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
        _style: u8,
    ) -> Result<Vec<PathPreviewTile>, PathBuildError> {
        if start.x >= self.width
            || end.x >= self.width
            || start.y >= self.height
            || end.y >= self.height
        {
            return Err(PathBuildError::OutsideWorld);
        }
        match self.solve_path_route(start, end, kind) {
            Ok((route, proposed)) => Ok(route
                .into_iter()
                .map(|path| PathPreviewTile {
                    path,
                    refusal: None,
                    edges: proposed.path_edges(path.position).unwrap(),
                })
                .collect()),
            Err(failure) => {
                if failure.refusal_all {
                    return Ok(failure
                        .prefix
                        .into_iter()
                        .map(|path| PathPreviewTile {
                            path,
                            refusal: Some(failure.error),
                            edges: self.path_edges(path.position).unwrap_or(0),
                        })
                        .collect());
                }
                let prefix_len = failure.prefix.len();
                let mut preview: Vec<_> = failure
                    .prefix
                    .into_iter()
                    .map(|path| PathPreviewTile {
                        path,
                        refusal: None,
                        edges: self.path_edges(path.position).unwrap_or(0),
                    })
                    .collect();
                for (step, (x, y)) in route_coordinates(start, end)
                    .into_iter()
                    .enumerate()
                    .skip(prefix_len)
                {
                    let corners = self.surface(x, y).corners;
                    let low = *corners.iter().min().unwrap();
                    let offsets = corners.map(|height| i32::from(height) - i32::from(low));
                    let slope = match offsets {
                        [0, 0, 0, 0] => None,
                        [1, 1, 0, 0] => Some(Direction::North),
                        [0, 1, 1, 0] => Some(Direction::East),
                        [0, 0, 1, 1] => Some(Direction::South),
                        [1, 0, 0, 1] => Some(Direction::West),
                        _ => None,
                    };
                    let height = if start.height
                        < *self.surface(start.x, start.y).corners.iter().min().unwrap()
                    {
                        start.height
                    } else {
                        low
                    };
                    preview.push(PathPreviewTile {
                        path: Path {
                            position: Position { x, y, height },
                            slope,
                        },
                        edges: self.path_edges(Position { x, y, height }).unwrap_or(0),
                        refusal: Some(if step == prefix_len {
                            failure.error
                        } else {
                            PathBuildError::Disconnected
                        }),
                    });
                }
                Ok(preview)
            }
        }
    }

    fn solve_path_route(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
    ) -> Result<(Vec<Path>, World), RouteFailure> {
        if start.x >= self.width
            || end.x >= self.width
            || start.y >= self.height
            || end.y >= self.height
        {
            return Err(RouteFailure {
                error: PathBuildError::OutsideWorld,
                prefix: Vec::new(),
                refusal_all: false,
            });
        }
        let coordinates = route_coordinates(start, end);
        let mut layers: Vec<Vec<(Path, Option<usize>)>> = Vec::with_capacity(coordinates.len());
        for (step, &(x, y)) in coordinates.iter().enumerate() {
            let remaining = coordinates.len() - step - 1;
            let min_height = (i32::from(start.height) - step as i32)
                .max(i32::from(end.height) - remaining as i32)
                .max(i32::from(i16::MIN));
            let max_height = (i32::from(start.height) + step as i32)
                .min(i32::from(end.height) + remaining as i32)
                .min(i32::from(i16::MAX));
            let mut layer = Vec::new();
            let mut has_supported_candidate = false;
            let mut has_occupied_candidate = false;
            for height in min_height..=max_height {
                if step == 0 && height != i32::from(start.height)
                    || step == coordinates.len() - 1 && height != i32::from(end.height)
                {
                    continue;
                }
                for slope in [
                    None,
                    Some(Direction::North),
                    Some(Direction::East),
                    Some(Direction::South),
                    Some(Direction::West),
                ] {
                    let path = Path {
                        position: Position {
                            x,
                            y,
                            height: height as i16,
                        },
                        slope,
                    };
                    let index = self.index(x, y);
                    if !self.path_supported(index, path) {
                        continue;
                    }
                    has_supported_candidate = true;
                    if self.columns[index].as_ref().is_some_and(|column| {
                        column.paths.iter().any(|other| {
                            if *other == path
                                && self.path_metadata(path.position).is_some_and(|metadata| {
                                    metadata.kind == kind
                                        || (coordinates.len() > 1
                                            && (step == 0 || step == coordinates.len() - 1))
                                })
                            {
                                return false;
                            }
                            i32::from(path.position.height) < other.top()
                                && i32::from(other.position.height) < path.top()
                        })
                    }) {
                        has_occupied_candidate = true;
                        continue;
                    }
                    let previous = if step == 0 {
                        None
                    } else {
                        let (previous_x, previous_y) = coordinates[step - 1];
                        let (direction, opposite) = if x > previous_x {
                            (Direction::East, Direction::West)
                        } else if x < previous_x {
                            (Direction::West, Direction::East)
                        } else if y > previous_y {
                            (Direction::South, Direction::North)
                        } else {
                            (Direction::North, Direction::South)
                        };
                        let Some(index) = layers[step - 1].iter().position(|(previous, _)| {
                            previous.edge(direction) == path.edge(opposite)
                        }) else {
                            continue;
                        };
                        Some(index)
                    };
                    layer.push((path, previous));
                }
            }
            if layer.is_empty() {
                let error = if step == coordinates.len() - 1 && !has_supported_candidate {
                    PathBuildError::EndpointHeight
                } else if has_occupied_candidate {
                    PathBuildError::Occupied
                } else {
                    PathBuildError::Disconnected
                };
                return Err(RouteFailure {
                    error,
                    prefix: recover_route(&layers),
                    refusal_all: false,
                });
            }
            layers.push(layer);
        }
        let route = recover_route(&layers);
        let proposed = self
            .validate_queue_connections(&route, kind)
            .map_err(|error| RouteFailure {
                error,
                prefix: route.clone(),
                refusal_all: true,
            })?;
        Ok((route, proposed))
    }

    fn validate_queue_connections(
        &self,
        route: &[Path],
        kind: PathKind,
    ) -> Result<World, PathBuildError> {
        let mut proposed = self.clone();
        proposed.apply_path_route(route, kind, 0);
        let mut visited = BTreeSet::new();
        for path in proposed.paths() {
            if proposed.path_metadata(path.position).unwrap().kind != PathKind::Queue
                || !visited.insert(path.position)
            {
                continue;
            }
            let mut pending = VecDeque::from([path.position]);
            let mut entrances = 0;
            let mut nodes = 0;
            let mut queue_edges = 0;
            while let Some(position) = pending.pop_front() {
                nodes += 1;
                let mut queue_degree = 0;
                let mut normal_degree = 0;
                for other in proposed.neighbours(position) {
                    if proposed.path_metadata(other).unwrap().kind == PathKind::Normal {
                        normal_degree += 1;
                    } else {
                        queue_degree += 1;
                        if visited.insert(other) {
                            pending.push_back(other);
                        }
                    }
                }
                entrances += normal_degree;
                queue_edges += queue_degree;
                if queue_degree > 2 || normal_degree > 1 || (normal_degree > 0 && queue_degree > 1)
                {
                    return Err(PathBuildError::QueueConnections);
                }
            }
            if entrances > 1 || queue_edges / 2 >= nodes {
                return Err(PathBuildError::QueueConnections);
            }
        }
        Ok(proposed)
    }

    fn apply_path_route(&mut self, route: &[Path], kind: PathKind, style: u8) {
        for path in route {
            if self.path(path.position).is_none() {
                self.insert_path(*path);
                self.path_metadata
                    .insert(path.position, PathMetadata { kind, style });
                if kind == PathKind::Normal {
                    for other in self.geometric_neighbours(path.position) {
                        if self.path_metadata(other).unwrap().kind == PathKind::Normal {
                            self.connect_paths(path.position, other);
                        }
                    }
                }
            }
        }
        for pair in route.windows(2) {
            self.connect_paths(pair[0].position, pair[1].position);
        }
    }

    pub fn build_path_route(
        &mut self,
        start: Position,
        end: Position,
        kind: PathKind,
        style: u8,
    ) -> Result<Vec<Path>, PathBuildError> {
        let route = self.preview_path_route(start, end, kind, style)?;
        self.apply_path_route(&route, kind, style);
        Ok(route)
    }
}
