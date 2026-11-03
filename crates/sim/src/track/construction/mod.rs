use crate::physics::Circuit;
use crate::track::clearance::{ClearanceBox, ClearanceCollision, check_clearance};
use crate::track::supports::{SupportError, SupportObstacle, generate_supports};
use crate::track::{CoasterModel, Piece, PieceId, TrackState, TrainConfiguration, Vec3, catalog};
use crate::world::World;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionError {
    Closed,
    Clearance(ClearanceCollision),
    Unavailable,
    ModelForbidden,
    AmbiguousConnection,
    SupportSearchExhausted,
    NoSupportRoute,
    InvalidIndex,
    NotClosed,
    InvalidCircuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceChoice {
    pub id: PieceId,
    pub result: Result<(), ConstructionError>,
}

pub struct TrackSegment {
    pub ids: Vec<usize>,
    pub pieces: Vec<Piece>,
}

pub struct Construction {
    start: TrackState,
    model: CoasterModel,
    pieces: Vec<Piece>,
    ids: Vec<usize>,
    disconnected: Vec<TrackSegment>,
    next_id: usize,
    closed: bool,
}

impl Construction {
    pub fn new(start: TrackState, model: CoasterModel) -> Self {
        Self {
            start,
            model,
            pieces: Vec::new(),
            ids: Vec::new(),
            disconnected: Vec::new(),
            next_id: 0,
            closed: false,
        }
    }

    pub fn start(&self) -> TrackState {
        self.start
    }
    pub fn end(&self) -> TrackState {
        self.pieces.last().map_or(self.start, |piece| piece.exit)
    }
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn piece_ids(&self) -> &[usize] {
        &self.ids
    }
    pub fn disconnected_segments(&self) -> &[TrackSegment] {
        &self.disconnected
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn model(&self) -> &CoasterModel {
        &self.model
    }

    pub fn choices(&self, world: &World, obstacles: &[SupportObstacle]) -> Vec<PieceChoice> {
        templates()
            .iter()
            .map(|template| {
                let result = self
                    .placeable(template)
                    .and_then(|piece| self.validate_piece(&piece, world, obstacles));
                PieceChoice {
                    id: template.id,
                    result,
                }
            })
            .collect()
    }

    pub fn validate(
        &self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let template = templates()
            .iter()
            .find(|piece| piece.id == id)
            .expect("catalog contains every piece id");
        let piece = self.placeable(template)?;
        self.validate_piece(&piece, world, obstacles)
    }

    fn placeable(&self, template: &Piece) -> Result<Piece, ConstructionError> {
        if self.closed {
            return Err(ConstructionError::Closed);
        }
        if !self.model.allowed_pieces.contains(&template.id)
            || template
                .inversion
                .is_some_and(|inversion| !self.model.allowed_inversions.contains(&inversion))
        {
            return Err(ConstructionError::ModelForbidden);
        }
        let piece = template
            .at(self.end())
            .ok_or(ConstructionError::Unavailable)?;
        if piece.maximum_curvature() > 1.0 / self.model.minimum_radius_metres {
            return Err(ConstructionError::ModelForbidden);
        }
        Ok(piece)
    }

    fn validate_piece(
        &self,
        piece: &Piece,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let scenery: Vec<_> = obstacles
            .iter()
            .map(|obstacle| ClearanceBox {
                min: obstacle.min,
                max: obstacle.max,
            })
            .collect();
        let existing: Vec<_> = self
            .pieces
            .iter()
            .chain(self.disconnected.iter().flat_map(|segment| &segment.pieces))
            .cloned()
            .collect();
        check_clearance(world, piece, &self.model.train, &existing, &scenery)
            .map_err(ConstructionError::Clearance)?;
        let mut all_pieces = existing;
        all_pieces.push(piece.clone());
        let mut end = piece.exit;
        let mut connected = Vec::new();
        loop {
            let matches: Vec<_> = self
                .disconnected
                .iter()
                .enumerate()
                .filter(|(index, segment)| {
                    !connected.contains(index) && segment.pieces[0].entry == end
                })
                .collect();
            if matches.len() > 1 {
                return Err(ConstructionError::AmbiguousConnection);
            }
            let Some((index, segment)) = matches.first() else {
                break;
            };
            connected.push(*index);
            end = segment.pieces.last().expect("nonempty segment").exit;
        }
        if !connected.is_empty() {
            for (index, retained_piece) in all_pieces.iter().enumerate() {
                let other_pieces: Vec<_> = all_pieces
                    .iter()
                    .enumerate()
                    .filter(|(other_index, _)| *other_index != index)
                    .map(|(_, other)| other.clone())
                    .collect();
                check_clearance(
                    world,
                    retained_piece,
                    &self.model.train,
                    &other_pieces,
                    &scenery,
                )
                .map_err(ConstructionError::Clearance)?;
            }
        }
        let track_obstacles: Vec<_> = all_pieces
            .iter()
            .map(|other_piece| {
                track_support_obstacles(std::slice::from_ref(other_piece), &self.model.train)
            })
            .collect();
        for (index, supported_piece) in all_pieces.iter().enumerate() {
            let mut support_obstacles = obstacles.to_vec();
            for (other_index, bounds) in track_obstacles.iter().enumerate() {
                if index != other_index {
                    support_obstacles.extend_from_slice(bounds);
                }
            }
            generate_supports(supported_piece, world, &support_obstacles).map_err(|error| {
                match error {
                    SupportError::SearchExhausted => ConstructionError::SupportSearchExhausted,
                    SupportError::NoRoute => ConstructionError::NoSupportRoute,
                }
            })?;
        }
        Ok(())
    }

    pub fn append(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let template = templates()
            .iter()
            .find(|piece| piece.id == id)
            .expect("catalog contains every piece id");
        let piece = self.placeable(template)?;
        self.validate_piece(&piece, world, obstacles)?;
        self.pieces.push(piece);
        self.ids.push(self.next_id);
        self.next_id += 1;
        while let Some(index) = self
            .disconnected
            .iter()
            .position(|segment| segment.pieces[0].entry == self.end())
        {
            let segment = self.disconnected.remove(index);
            self.ids.extend(segment.ids);
            self.pieces.extend(segment.pieces);
        }
        Ok(())
    }

    pub fn delete_from(&mut self, id: usize) -> Result<Vec<Piece>, ConstructionError> {
        if let Some(index) = self.ids.iter().position(|&piece_id| piece_id == id) {
            let suffix_pieces = self.pieces.split_off(index + 1);
            let suffix_ids = self.ids.split_off(index + 1);
            let removed = self.pieces.pop().expect("piece matching id");
            self.ids.pop();
            if !suffix_pieces.is_empty() {
                self.disconnected.push(TrackSegment {
                    ids: suffix_ids,
                    pieces: suffix_pieces,
                });
            }
            self.closed = false;
            return Ok(vec![removed]);
        }
        let segment_index = self
            .disconnected
            .iter()
            .position(|segment| segment.ids.contains(&id))
            .ok_or(ConstructionError::InvalidIndex)?;
        let segment = &mut self.disconnected[segment_index];
        let index = segment
            .ids
            .iter()
            .position(|&piece_id| piece_id == id)
            .expect("piece id exists");
        let suffix_pieces = segment.pieces.split_off(index + 1);
        let suffix_ids = segment.ids.split_off(index + 1);
        let removed = segment.pieces.pop().expect("piece matching id");
        segment.ids.pop();
        if segment.pieces.is_empty() {
            self.disconnected.remove(segment_index);
        }
        if !suffix_pieces.is_empty() {
            self.disconnected.push(TrackSegment {
                ids: suffix_ids,
                pieces: suffix_pieces,
            });
        }
        Ok(vec![removed])
    }

    pub fn close(&mut self) -> Result<(), ConstructionError> {
        Circuit::new(self.pieces.clone()).map_err(|_| ConstructionError::InvalidCircuit)?;
        self.closed = true;
        Ok(())
    }

    pub fn circuit(&self) -> Result<Circuit, ConstructionError> {
        if !self.closed {
            return Err(ConstructionError::NotClosed);
        }
        Circuit::new(self.pieces.clone()).map_err(|_| ConstructionError::InvalidCircuit)
    }
}

fn track_support_obstacles(pieces: &[Piece], train: &TrainConfiguration) -> Vec<SupportObstacle> {
    let mut obstacles = Vec::new();
    for piece in pieces {
        let segments = (piece.length() / 2.0).ceil() as usize;
        for segment in 0..segments {
            let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut max = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for step in 0..=4 {
                let distance =
                    piece.length() * (segment as f64 + f64::from(step) / 4.0) / segments as f64;
                let sample = piece.sample(distance);
                let left = sample.up.cross(sample.tangent);
                for side in [-1.0, 1.0] {
                    for height in [0.0, train.height_metres] {
                        let point = sample.position
                            + left * (side * train.width_metres / 2.0)
                            + sample.up * height;
                        min = Vec3::new(min.x.min(point.x), min.y.min(point.y), min.z.min(point.z));
                        max = Vec3::new(max.x.max(point.x), max.y.max(point.y), max.z.max(point.z));
                    }
                }
            }
            let padding = Vec3::new(0.2, 0.2, 0.2);
            obstacles.push(SupportObstacle {
                min: min - padding,
                max: max + padding,
            });
        }
    }
    obstacles
}

fn templates() -> &'static [Piece] {
    static CATALOG: OnceLock<Vec<Piece>> = OnceLock::new();
    CATALOG.get_or_init(catalog)
}
