//! Proximity opportunities along track and simultaneous encounters between trains.
use std::collections::HashMap;

use crate::physics::TestSample;
use crate::track::clearance::{
    ClearanceBox, ClearanceIndex, OrientedEnvelope, connector_allowance,
};
use crate::track::supports::SupportSegment;
use crate::track::{Piece, TrainConfiguration, Vec3};
use crate::world::{Ground, World};

#[cfg(test)]
mod tests;

// Distances are metres. Sampling matches the clearance system's half-metre spacing.
const SPACING: f64 = 0.5;
const NEAR_DISTANCE: f64 = 2.0;
const CELL_SIZE: f64 = 16.0;
const HUG_DISTANCE: f64 = 2.0;
const HUG_LENGTH: f64 = 8.0;
const HUG_SPEED: f64 = 8.0;
const DUEL_DISTANCE: f64 = 10.0;

type ObstacleIndex = HashMap<(i32, i32), Vec<ClearanceBox>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProximityKind {
    NearMiss,
    Headchopper,
    Dueling,
    Tunnel,
    TerrainHugging,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProximityEvent {
    pub piece_index: usize,
    pub kind: ProximityKind,
    pub position: Vec3,
    pub distance: f64,
    pub strength: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct TrainPosition {
    pub position: Vec3,
    pub tangent: Vec3,
    pub piece_index: usize,
    pub distance: f64,
    pub speed: f64,
}

pub fn detect_dueling(primary: TrainPosition, others: &[TrainPosition]) -> Option<ProximityEvent> {
    if primary.speed < HUG_SPEED {
        return None;
    }
    others
        .iter()
        .filter(|other| other.speed >= HUG_SPEED && primary.tangent.dot(other.tangent).abs() >= 0.8)
        .filter_map(|other| {
            let delta = primary.position - other.position;
            let distance = delta.dot(delta).sqrt();
            (distance < DUEL_DISTANCE).then_some(distance)
        })
        .min_by(f64::total_cmp)
        .map(|distance| ProximityEvent {
            piece_index: primary.piece_index,
            kind: ProximityKind::Dueling,
            position: primary.position,
            distance: primary.distance,
            strength: (1.0 - distance / DUEL_DISTANCE).max(0.01),
        })
}

fn cell(value: f64) -> i32 {
    (value / CELL_SIZE).floor() as i32
}

#[derive(Clone, Debug)]
pub struct ProximityGeometry {
    events: Vec<ProximityEvent>,
    terrain: Vec<(usize, f64, Vec3, f64)>,
}

impl ProximityGeometry {
    /// Apply a particular test run's speed samples to the cached geometry.
    pub fn events_for_run(&self, speed_samples: &[TestSample]) -> Vec<ProximityEvent> {
        let mut events = self.events.clone();
        let mut hug_start: Option<f64> = None;
        let mut hug_emitted = false;
        for &(piece_index, distance, position, gap) in &self.terrain {
            let speed_index =
                speed_samples.partition_point(|sample| sample.distance < distance - 0.51);
            let speed = speed_samples
                .get(speed_index)
                .filter(|sample| (sample.distance - distance).abs() <= 0.51)
                .map_or(0.0, |sample| sample.speed);
            if (0.0..=HUG_DISTANCE).contains(&gap) && speed >= HUG_SPEED {
                let from = *hug_start.get_or_insert(distance);
                if !hug_emitted && distance - from >= HUG_LENGTH {
                    events.push(ProximityEvent {
                        piece_index,
                        kind: ProximityKind::TerrainHugging,
                        position,
                        distance,
                        strength: (1.0 - gap / HUG_DISTANCE).max(0.01),
                    });
                    hug_emitted = true;
                }
            } else {
                hug_start = None;
                hug_emitted = false;
            }
        }
        events
    }
}

/// Scans a layout after construction or a test run. `speed_samples` use circuit distances.
pub fn detect_proximity(
    world: &World,
    pieces: &[Piece],
    train: &TrainConfiguration,
    other_tracks: &[Piece],
    obstacles: &[ClearanceBox],
    speed_samples: &[TestSample],
) -> Vec<ProximityEvent> {
    scan_proximity(world, pieces, train, other_tracks, obstacles).events_for_run(speed_samples)
}

/// Precompute geometry once for all cars travelling on the same layout.
pub fn scan_proximity(
    world: &World,
    pieces: &[Piece],
    train: &TrainConfiguration,
    other_tracks: &[Piece],
    obstacles: &[ClearanceBox],
) -> ProximityGeometry {
    scan_proximity_with_supports(world, pieces, train, other_tracks, obstacles, &[])
}

pub fn scan_proximity_with_supports(
    world: &World,
    pieces: &[Piece],
    train: &TrainConfiguration,
    other_tracks: &[Piece],
    obstacles: &[ClearanceBox],
    supports: &[SupportSegment],
) -> ProximityGeometry {
    let mut index: ObstacleIndex = HashMap::new();
    let all_tracks: Vec<Piece> = pieces.iter().chain(other_tracks).cloned().collect();
    let track_index = ClearanceIndex::new(&all_tracks);
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + SPACING / 2.0;
    let margin = (half_length * half_length
        + (train.width_metres / 2.0).powi(2)
        + train.height_metres.powi(2))
    .sqrt()
        + NEAR_DISTANCE
        + SPACING;
    let connector_distance = connector_allowance(train) + NEAR_DISTANCE;
    let paths = world.paths().map(|path| {
        let x = f64::from(path.position.x) * 4.0;
        let y = f64::from(path.position.y) * 4.0;
        let z = f64::from(path.position.height);
        ClearanceBox {
            min: Vec3::new(x, y, z),
            max: Vec3::new(
                x + 4.0,
                y + 4.0,
                z + if path.slope.is_some() { 2.0 } else { 1.0 },
            ),
        }
    });
    for obstacle in obstacles.iter().copied().chain(paths) {
        for x in cell(obstacle.min.x)..=cell(obstacle.max.x) {
            for y in cell(obstacle.min.y)..=cell(obstacle.max.y) {
                index.entry((x, y)).or_default().push(obstacle);
            }
        }
    }
    let mut support_index: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (support_index_value, support) in supports.iter().enumerate() {
        for x in cell(support.start.x.min(support.end.x) - 0.2)
            ..=cell(support.start.x.max(support.end.x) + 0.2)
        {
            for y in cell(support.start.y.min(support.end.y) - 0.2)
                ..=cell(support.start.y.max(support.end.y) + 0.2)
            {
                support_index
                    .entry((x, y))
                    .or_default()
                    .push(support_index_value);
            }
        }
    }
    let mut events: Vec<ProximityEvent> = Vec::new();
    let mut start = 0.0;
    let mut terrain = Vec::new();
    for (piece_index, piece) in pieces.iter().enumerate() {
        let steps = (piece.length() / SPACING).ceil() as usize;
        for step in 0..=steps {
            let mut seen: [Option<usize>; 3] = [None; 3];
            let local = piece.length() * step as f64 / steps as f64;
            let distance = start + local;
            let sample = piece.sample(local);
            let pos = sample.position;
            let record = |kind, strength| ProximityEvent {
                piece_index,
                kind,
                position: pos,
                distance,
                strength,
            };
            let x = cell(pos.x);
            let y = cell(pos.y);
            let index_ref = &index;
            let oriented = OrientedEnvelope::for_track(piece, local, train);
            let candidates = (x - 1..=x + 1)
                .flat_map(|cx| {
                    (y - 1..=y + 1)
                        .flat_map(move |cy| index_ref.get(&(cx, cy)).into_iter().flatten())
                })
                .copied();
            let rail_candidates = track_index
                .nearby_samples(pos, margin)
                .into_iter()
                .filter_map(|(other_index, other_distance, bounds)| {
                    if other_index == piece_index {
                        return None;
                    }
                    let other = &all_tracks[other_index];
                    let near_entry = piece.entry == other.exit
                        && local + (other.length() - other_distance) <= connector_distance;
                    let near_exit = piece.exit == other.entry
                        && (piece.length() - local) + other_distance <= connector_distance;
                    (!near_entry && !near_exit).then_some(bounds)
                });
            for obstacle in candidates.chain(rail_candidates) {
                if !oriented.intersects_expanded(NEAR_DISTANCE, obstacle)
                    || oriented.intersects_expanded(0.0, obstacle)
                {
                    continue;
                }
                let mut low = 0.0;
                let mut high = NEAR_DISTANCE;
                for _ in 0..8 {
                    let middle = (low + high) / 2.0;
                    if oriented.intersects_expanded(middle, obstacle) {
                        high = middle;
                    } else {
                        low = middle;
                    }
                }
                let strength = (1.0 - high / NEAR_DISTANCE).max(0.01);
                let mut update = |slot: usize, kind| {
                    if let Some(event_index) = seen[slot] {
                        if events[event_index].strength < strength {
                            events[event_index] = record(kind, strength);
                        }
                    } else {
                        seen[slot] = Some(events.len());
                        events.push(record(kind, strength));
                    }
                };
                update(0, ProximityKind::NearMiss);
                let centre = (obstacle.min + obstacle.max) * 0.5;
                let left = sample.up.cross(sample.tangent);
                let lateral = (centre - pos).dot(left).abs();
                let obstacle_half_width = ((obstacle.max.x - obstacle.min.x) * left.x.abs()
                    + (obstacle.max.y - obstacle.min.y) * left.y.abs()
                    + (obstacle.max.z - obstacle.min.z) * left.z.abs())
                    * 0.5;
                if obstacle.min.z > pos.z + train.height_metres * 0.5
                    && lateral <= train.width_metres / 2.0 + obstacle_half_width
                {
                    update(1, ProximityKind::Headchopper);
                }
            }
            let mut visited = std::collections::HashSet::new();
            for cx in cell(pos.x - margin)..=cell(pos.x + margin) {
                for cy in cell(pos.y - margin)..=cell(pos.y + margin) {
                    for &support_id in support_index.get(&(cx, cy)).into_iter().flatten() {
                        if !visited.insert(support_id) {
                            continue;
                        }
                        let support = supports[support_id];
                        if !oriented.intersects_segment(
                            support.start,
                            support.end,
                            NEAR_DISTANCE,
                            0.2,
                        ) || oriented.intersects_segment(support.start, support.end, 0.0, 0.2)
                        {
                            continue;
                        }
                        let mut low = 0.0;
                        let mut high = NEAR_DISTANCE;
                        for _ in 0..8 {
                            let middle = (low + high) / 2.0;
                            if oriented.intersects_segment(support.start, support.end, middle, 0.2)
                            {
                                high = middle;
                            } else {
                                low = middle;
                            }
                        }
                        let strength = (1.0 - high / NEAR_DISTANCE).max(0.01);
                        let mut update = |slot: usize, kind| {
                            if let Some(event_index) = seen[slot] {
                                if events[event_index].strength < strength {
                                    events[event_index] = record(kind, strength);
                                }
                            } else {
                                seen[slot] = Some(events.len());
                                events.push(record(kind, strength));
                            }
                        };
                        update(0, ProximityKind::NearMiss);
                        let delta = support.end - support.start;
                        let length_squared = delta.dot(delta);
                        let t = if length_squared > 0.0 {
                            ((pos - support.start).dot(delta) / length_squared).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let closest = support.start + delta * t;
                        let lateral = (closest - pos).dot(sample.up.cross(sample.tangent)).abs();
                        if closest.z > pos.z + train.height_metres * 0.5
                            && lateral <= train.width_metres / 2.0 + 0.2
                        {
                            update(1, ProximityKind::Headchopper);
                        }
                    }
                }
            }
            let tx = (pos.x / 4.0).floor() as i32;
            let ty = (pos.y / 4.0).floor() as i32;
            let (width, height) = world.dimensions();
            if tx < 0 || ty < 0 || tx >= i32::from(width) || ty >= i32::from(height) {
                terrain.push((piece_index, distance, pos, f64::INFINITY));
                continue;
            }
            let tx = tx as u16;
            let ty = ty as u16;
            let corners = world.surface(tx, ty).corners.map(f64::from);
            let fx = pos.x / 4.0 - f64::from(tx);
            let fy = pos.y / 4.0 - f64::from(ty);
            let ground = if fx >= fy {
                corners[0] * (1.0 - fx) + corners[1] * (fx - fy) + corners[2] * fy
            } else {
                corners[0] * (1.0 - fy) + corners[2] * fx + corners[3] * (fy - fx)
            };
            let ground_kind = world.ground(tx, ty, pos.z.floor() as i16);
            if ground_kind == Ground::Carved || (ground_kind == Ground::Air && pos.z < ground) {
                events.push(record(ProximityKind::Tunnel, 1.0));
            }
            let gap = pos.z - ground;
            if (0.0..NEAR_DISTANCE).contains(&gap) {
                let strength = (1.0 - gap / NEAR_DISTANCE).max(0.01);
                if let Some(event_index) = seen[0] {
                    if events[event_index].strength < strength {
                        events[event_index] = record(ProximityKind::NearMiss, strength);
                    }
                } else {
                    events.push(record(ProximityKind::NearMiss, strength));
                }
            }
            terrain.push((piece_index, distance, pos, gap));
        }
        start += piece.length();
    }
    ProximityGeometry { events, terrain }
}
