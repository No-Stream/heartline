//! Conservative sampled clearance envelopes in metres.
use crate::track::{Piece, TrackState, TrainConfiguration, Vec3};
use crate::world::{Ground, World};
use std::collections::{HashMap, HashSet};

const SAMPLE_SPACING: f64 = 0.5;
const TRACK_RADIUS: f64 = 0.15;
const INDEX_CELL_SIZE: f64 = 16.0;

fn index_cell(coordinate: f64) -> i32 {
    (coordinate / INDEX_CELL_SIZE).floor() as i32
}

#[derive(Clone, Copy, Debug)]
pub struct ClearanceBox {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearanceCollision {
    Terrain,
    Path,
    Scenery,
    Track,
    OutsideWorld,
}

fn intersects(a: ClearanceBox, b: ClearanceBox) -> bool {
    a.min.x <= b.max.x
        && a.max.x >= b.min.x
        && a.min.y <= b.max.y
        && a.max.y >= b.min.y
        && a.min.z <= b.max.z
        && a.max.z >= b.min.z
}

#[derive(Clone, Copy)]
pub(crate) struct OrientedEnvelope {
    centre: Vec3,
    axes: [Vec3; 3],
    half: [f64; 3],
    bounds: ClearanceBox,
}

fn intersects_envelope(envelope: &OrientedEnvelope, obstacle: ClearanceBox) -> bool {
    if !intersects(envelope.bounds, obstacle) {
        return false;
    }
    let centre = (obstacle.min + obstacle.max) * 0.5;
    let half = (obstacle.max - obstacle.min) * 0.5;
    let world_axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    let delta = centre - envelope.centre;
    let separated = |axis: Vec3| {
        let radius = envelope
            .axes
            .iter()
            .zip(envelope.half)
            .map(|(basis, extent)| basis.dot(axis).abs() * extent)
            .sum::<f64>();
        let other = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
        axis.dot(delta).abs() > radius + other && axis.dot(axis) > 1e-20
    };
    !envelope
        .axes
        .iter()
        .chain(world_axes.iter())
        .copied()
        .any(separated)
        && !envelope
            .axes
            .iter()
            .any(|a| world_axes.iter().any(|b| separated(a.cross(*b))))
}

fn envelope(
    piece: &Piece,
    distance: f64,
    train: &TrainConfiguration,
    spacing: f64,
) -> OrientedEnvelope {
    let sample = piece.sample(distance);
    let left = sample.up.cross(sample.tangent);
    let centre = sample.position + sample.up * (train.height_metres / 2.0);
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + spacing / 2.0;
    let half_width = train.width_metres / 2.0;
    let half_height = train.height_metres / 2.0;
    let extent = Vec3::new(
        sample.tangent.x.abs() * half_length
            + left.x.abs() * half_width
            + sample.up.x.abs() * half_height,
        sample.tangent.y.abs() * half_length
            + left.y.abs() * half_width
            + sample.up.y.abs() * half_height,
        sample.tangent.z.abs() * half_length
            + left.z.abs() * half_width
            + sample.up.z.abs() * half_height,
    );
    OrientedEnvelope {
        centre,
        axes: [sample.tangent, left, sample.up],
        half: [half_length, half_width, half_height],
        bounds: ClearanceBox {
            min: centre - extent,
            max: centre + extent,
        },
    }
}

pub(crate) fn connector_allowance(train: &TrainConfiguration) -> f64 {
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + SAMPLE_SPACING;
    let half_width = train.width_metres / 2.0 + SAMPLE_SPACING;
    let height = train.height_metres + SAMPLE_SPACING;
    (half_length * half_length + half_width * half_width + height * height).sqrt()
        + (TRACK_RADIUS + SAMPLE_SPACING / 2.0) * 3.0_f64.sqrt()
}

fn connected_distance(
    start: TrackState,
    end: TrackState,
    pieces: &[Piece],
    excluded_piece: Option<usize>,
    second_excluded_piece: Option<usize>,
    limit: f64,
) -> Option<f64> {
    let mut reachable = vec![(start, 0.0)];
    let mut index = 0;
    while index < reachable.len() {
        let (state, distance) = reachable[index];
        index += 1;
        for (piece_index, piece) in pieces.iter().enumerate() {
            if Some(piece_index) == excluded_piece
                || Some(piece_index) == second_excluded_piece
                || piece.entry != state
            {
                continue;
            }
            let next_distance = distance + piece.length();
            if next_distance > limit {
                continue;
            }
            if let Some((_, best)) = reachable.iter_mut().find(|(next, _)| *next == piece.exit) {
                if next_distance < *best {
                    *best = next_distance;
                    reachable.push((piece.exit, next_distance));
                }
            } else {
                reachable.push((piece.exit, next_distance));
            }
        }
    }
    reachable
        .iter()
        .filter(|(state, _)| *state == end)
        .map(|(_, distance)| *distance)
        .min_by(f64::total_cmp)
}

fn train_sample_spacing(train: &TrainConfiguration) -> f64 {
    SAMPLE_SPACING.min(train.length_metres / f64::from(train.cars) / 2.0)
}

fn samples(piece: &Piece, spacing: f64) -> impl Iterator<Item = f64> + '_ {
    let steps = (piece.length() / spacing).ceil() as usize;
    (0..=steps).map(move |index| piece.length() * (index as f64 / steps as f64))
}

/// Cached rail samples for repeated clearance checks against the same track.
#[derive(Clone)]
pub struct ClearanceIndex {
    pieces: Vec<Piece>,
    samples: Vec<Vec<(f64, ClearanceBox)>>,
    cells: HashMap<(i32, i32), Vec<(usize, usize)>>,
}

impl ClearanceIndex {
    pub fn new(pieces: &[Piece]) -> Self {
        let mut index = Self {
            pieces: Vec::new(),
            samples: Vec::new(),
            cells: HashMap::new(),
        };
        for piece in pieces {
            index.push(piece);
        }
        index
    }

    pub fn push(&mut self, piece: &Piece) {
        let radius = TRACK_RADIUS + SAMPLE_SPACING / 2.0;
        let extent = Vec3::new(radius, radius, radius);
        let piece_samples: Vec<_> = samples(piece, SAMPLE_SPACING)
            .map(|distance| {
                let position = piece.sample(distance).position;
                (
                    distance,
                    ClearanceBox {
                        min: position - extent,
                        max: position + extent,
                    },
                )
            })
            .collect();
        let piece_index = self.pieces.len();
        for (sample_index, (_, bounds)) in piece_samples.iter().enumerate() {
            let centre = (bounds.min + bounds.max) * 0.5;
            self.cells
                .entry((index_cell(centre.x), index_cell(centre.y)))
                .or_default()
                .push((piece_index, sample_index));
        }
        self.pieces.push(piece.clone());
        self.samples.push(piece_samples);
    }

    /// Returns indexed rail samples within the requested horizontal bounds.
    pub(crate) fn nearby_samples(
        &self,
        position: Vec3,
        margin: f64,
    ) -> Vec<(usize, f64, ClearanceBox)> {
        let mut nearby = Vec::new();
        for x in index_cell(position.x - margin)..=index_cell(position.x + margin) {
            for y in index_cell(position.y - margin)..=index_cell(position.y + margin) {
                if let Some(entries) = self.cells.get(&(x, y)) {
                    for &(piece_index, sample_index) in entries {
                        let (distance, bounds) = self.samples[piece_index][sample_index];
                        nearby.push((piece_index, distance, bounds));
                    }
                }
            }
        }
        nearby
    }

    /// Retains and orders entries according to their current indices.
    pub fn reorder(&mut self, indices: &[usize]) {
        let mut seen = vec![false; self.pieces.len()];
        for &index in indices {
            assert!(!seen[index], "duplicate clearance index");
            seen[index] = true;
        }
        self.pieces = indices
            .iter()
            .map(|&index| self.pieces[index].clone())
            .collect();
        self.samples = indices
            .iter()
            .map(|&index| self.samples[index].clone())
            .collect();
        self.cells.clear();
        for (piece_index, piece_samples) in self.samples.iter().enumerate() {
            for (sample_index, (_, bounds)) in piece_samples.iter().enumerate() {
                let centre = (bounds.min + bounds.max) * 0.5;
                self.cells
                    .entry((index_cell(centre.x), index_cell(centre.y)))
                    .or_default()
                    .push((piece_index, sample_index));
            }
        }
    }
}

/// Checks retained train envelopes against a candidate rail without checking the world.
/// Rail samples select nearby retained pieces; the margin covers every oriented
/// car corner and the maximum distance to the nearest retained rail sample.
pub fn check_reverse_track_clearance(
    index: &ClearanceIndex,
    candidate: &Piece,
    train: &TrainConfiguration,
) -> Result<(), ClearanceCollision> {
    assert!(
        train.cars > 0
            && train.length_metres.is_finite()
            && train.length_metres > 0.0
            && train.width_metres.is_finite()
            && train.width_metres > 0.0
            && train.height_metres.is_finite()
            && train.height_metres > 0.0,
        "invalid train dimensions"
    );
    let spacing = train_sample_spacing(train);
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + spacing / 2.0;
    let half_width = train.width_metres / 2.0;
    let half_height = train.height_metres / 2.0;
    let margin = (half_length * half_length + half_width * half_width + half_height * half_height)
        .sqrt()
        + half_height
        + SAMPLE_SPACING / 2.0;
    let candidate_index = ClearanceIndex::new(std::slice::from_ref(candidate));
    let mut nearby = HashSet::new();
    for (_, bounds) in &candidate_index.samples[0] {
        for x in index_cell(bounds.min.x - margin)..=index_cell(bounds.max.x + margin) {
            for y in index_cell(bounds.min.y - margin)..=index_cell(bounds.max.y + margin) {
                if let Some(entries) = index.cells.get(&(x, y)) {
                    nearby.extend(entries.iter().map(|(piece_index, _)| *piece_index));
                }
            }
        }
    }
    let connector_allowance = connector_allowance(train);
    for piece_index in nearby {
        let piece = &index.pieces[piece_index];
        let entry_gap = connected_distance(
            candidate.exit,
            piece.entry,
            &index.pieces,
            Some(piece_index),
            None,
            connector_allowance,
        );
        let exit_gap = connected_distance(
            piece.exit,
            candidate.entry,
            &index.pieces,
            Some(piece_index),
            None,
            connector_allowance,
        );
        for distance in samples(piece, spacing) {
            let oriented = envelope(piece, distance, train, spacing);
            for (other_distance, bounds) in &candidate_index.samples[0] {
                let near_entry = entry_gap.is_some_and(|gap| {
                    distance + (candidate.length() - other_distance) + gap <= connector_allowance
                });
                let near_exit = exit_gap.is_some_and(|gap| {
                    (piece.length() - distance) + other_distance + gap <= connector_allowance
                });
                if !near_entry && !near_exit && intersects_envelope(&oriented, *bounds) {
                    return Err(ClearanceCollision::Track);
                }
            }
        }
    }
    Ok(())
}

/// Checks a piece against the world and supplied track and scenery.
/// Connected track is ignored only in the immediate vicinity of a common connector.
/// The envelope uses sampled oriented train cars and conservative rail bounds.
pub fn check_clearance(
    world: &World,
    piece: &Piece,
    train: &TrainConfiguration,
    existing_track: &[Piece],
    scenery: &[ClearanceBox],
) -> Result<(), ClearanceCollision> {
    check_clearance_indexed(
        world,
        piece,
        train,
        &ClearanceIndex::new(existing_track),
        None,
        scenery,
    )
}

/// Checks a piece using cached track samples. `excluded_piece` omits one entry
/// when checking an existing piece against the rest of the index.
pub fn check_clearance_indexed(
    world: &World,
    piece: &Piece,
    train: &TrainConfiguration,
    index: &ClearanceIndex,
    excluded_piece: Option<usize>,
    scenery: &[ClearanceBox],
) -> Result<(), ClearanceCollision> {
    assert!(
        train.cars > 0
            && train.length_metres.is_finite()
            && train.length_metres > 0.0
            && train.width_metres.is_finite()
            && train.width_metres > 0.0
            && train.height_metres.is_finite()
            && train.height_metres > 0.0,
        "invalid train dimensions"
    );
    for obstacle in scenery {
        assert!(
            obstacle.min.x < obstacle.max.x
                && obstacle.min.y < obstacle.max.y
                && obstacle.min.z < obstacle.max.z,
            "invalid scenery bounds"
        );
    }
    let (width, height) = world.dimensions();
    let connector_allowance = connector_allowance(train);
    // Adjacent samples are at most half a car length apart, including for
    // non-default trains. The longitudinal padding covers translation between
    // samples; orientation between samples remains an approximation.
    let spacing = train_sample_spacing(train);
    let envelopes: Vec<_> = samples(piece, spacing)
        .map(|distance| (distance, envelope(piece, distance, train, spacing)))
        .collect();
    let mut overall = envelopes[0].1.bounds;
    for (_, envelope) in &envelopes[1..] {
        overall.min = Vec3::new(
            overall.min.x.min(envelope.bounds.min.x),
            overall.min.y.min(envelope.bounds.min.y),
            overall.min.z.min(envelope.bounds.min.z),
        );
        overall.max = Vec3::new(
            overall.max.x.max(envelope.bounds.max.x),
            overall.max.y.max(envelope.bounds.max.y),
            overall.max.z.max(envelope.bounds.max.z),
        );
    }
    let nearby_scenery: Vec<_> = world
        .scenery_boxes()
        .filter(|bounds| intersects(overall, *bounds))
        .collect();
    let radius = TRACK_RADIUS + SAMPLE_SPACING / 2.0;
    let mut nearby_track = Vec::new();
    for x in index_cell(overall.min.x - radius)..=index_cell(overall.max.x + radius) {
        for y in index_cell(overall.min.y - radius)..=index_cell(overall.max.y + radius) {
            if let Some(entries) = index.cells.get(&(x, y)) {
                for &(piece_index, sample_index) in entries {
                    if Some(piece_index) == excluded_piece {
                        continue;
                    }
                    let (distance, bounds) = index.samples[piece_index][sample_index];
                    if intersects(overall, bounds) {
                        nearby_track.push((piece_index, distance, bounds));
                    }
                }
            }
        }
    }
    let mut connection_gaps = HashMap::new();
    for (index_to_check, _, _) in &nearby_track {
        connection_gaps.entry(*index_to_check).or_insert_with(|| {
            let other = &index.pieces[*index_to_check];
            let entry_gap = connected_distance(
                other.exit,
                piece.entry,
                &index.pieces,
                Some(*index_to_check),
                excluded_piece,
                connector_allowance,
            );
            let exit_gap = connected_distance(
                piece.exit,
                other.entry,
                &index.pieces,
                Some(*index_to_check),
                excluded_piece,
                connector_allowance,
            );
            (entry_gap, exit_gap)
        });
    }
    for (distance, oriented) in envelopes {
        let bounds = oriented.bounds;
        if bounds.min.x < 0.0
            || bounds.min.y < 0.0
            || bounds.max.x > f64::from(width) * 4.0
            || bounds.max.y > f64::from(height) * 4.0
        {
            return Err(ClearanceCollision::OutsideWorld);
        }
        for x in (bounds.min.x / 4.0).floor() as u16
            ..=(bounds.max.x / 4.0).floor().min(f64::from(width - 1)) as u16
        {
            for y in (bounds.min.y / 4.0).floor() as u16
                ..=(bounds.max.y / 4.0).floor().min(f64::from(height - 1)) as u16
            {
                let min_height = bounds.min.z.floor().max(f64::from(i16::MIN)) as i16;
                if world.surface(x, y).corners.into_iter().max().unwrap() <= min_height {
                    continue;
                }
                let max_height = bounds.max.z.ceil().min(f64::from(i16::MAX)) as i16;
                for z in min_height..max_height {
                    if matches!(world.ground(x, y, z), Ground::Solid | Ground::SurfaceSlope)
                        && intersects_envelope(
                            &oriented,
                            ClearanceBox {
                                min: Vec3::new(
                                    f64::from(x) * 4.0,
                                    f64::from(y) * 4.0,
                                    f64::from(z),
                                ),
                                max: Vec3::new(
                                    f64::from(x) * 4.0 + 4.0,
                                    f64::from(y) * 4.0 + 4.0,
                                    f64::from(z) + 1.0,
                                ),
                            },
                        )
                    {
                        return Err(ClearanceCollision::Terrain);
                    }
                }
            }
        }
        for path in world.paths() {
            let x = f64::from(path.position.x) * 4.0;
            let y = f64::from(path.position.y) * 4.0;
            let z = f64::from(path.position.height);
            let path_bounds = ClearanceBox {
                min: Vec3::new(x, y, z),
                max: Vec3::new(
                    x + 4.0,
                    y + 4.0,
                    z + if path.slope.is_some() { 2.0 } else { 1.0 },
                ),
            };
            if intersects_envelope(&oriented, path_bounds) {
                return Err(ClearanceCollision::Path);
            }
        }
        if scenery
            .iter()
            .copied()
            .chain(nearby_scenery.iter().copied())
            .any(|obstacle| intersects_envelope(&oriented, obstacle))
        {
            return Err(ClearanceCollision::Scenery);
        }
        for (other_index, other_distance, other_bounds) in &nearby_track {
            let other = &index.pieces[*other_index];
            let (entry_gap, exit_gap) = connection_gaps[other_index];
            let near_entry = entry_gap.is_some_and(|gap| {
                distance + (other.length() - other_distance) + gap <= connector_allowance
            });
            let near_exit = exit_gap.is_some_and(|gap| {
                (piece.length() - distance) + other_distance + gap <= connector_allowance
            });
            if !near_entry && !near_exit && intersects_envelope(&oriented, *other_bounds) {
                return Err(ClearanceCollision::Track);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};
    use crate::world::{Path, Position, Surface};

    fn straight(x: i32, y: i32, z: i32) -> Piece {
        let state = TrackState {
            x,
            y,
            z,
            ..TrackState::origin()
        };
        catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(state)
            .unwrap()
    }

    fn world(height: i16) -> World {
        World::new(
            32,
            32,
            Surface {
                corners: [height; 4],
                water: None,
            },
        )
    }

    #[test]
    fn scenery_overlap_fails_and_near_miss_passes() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        let obstacle = |y| ClearanceBox {
            min: Vec3::new(12.0, y, 6.5),
            max: Vec3::new(13.0, y + 0.5, 7.5),
        };
        assert!(check_clearance(&world(0), &piece, &train, &[], &[obstacle(8.0)]).is_err());
        assert!(check_clearance(&world(0), &piece, &train, &[], &[obstacle(12.0)]).is_ok());
    }

    #[test]
    fn terrain_and_paths_obstruct_train() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        assert!(matches!(
            check_clearance(&world(7), &piece, &train, &[], &[]),
            Err(ClearanceCollision::Terrain)
        ));
        let mut map = world(5);
        map.place_path(Path {
            position: Position {
                x: 3,
                y: 2,
                height: 5,
            },
            slope: None,
        });
        assert!(matches!(
            check_clearance(&map, &piece, &train, &[], &[]),
            Err(ClearanceCollision::Path)
        ));
        assert!(check_clearance(&world(0), &piece, &train, &[], &[]).is_ok());
    }

    #[test]
    fn sloped_terrain_blocks_below_highest_corner_but_not_above_it() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        let mut map = world(0);
        map.set_surface(
            3,
            2,
            Surface {
                corners: [0, 0, 7, 7],
                water: None,
            },
        );
        assert_eq!(
            check_clearance(&map, &piece, &train, &[], &[]),
            Err(ClearanceCollision::Terrain)
        );
        map.set_surface(
            3,
            2,
            Surface {
                corners: [0, 0, 6, 6],
                water: None,
            },
        );
        assert_eq!(check_clearance(&map, &piece, &train, &[], &[]), Ok(()));
    }

    #[test]
    fn clearance_uses_train_width() {
        let piece = straight(4, 4, 3);
        let mut train = hybrid_model().train;
        let obstacle = ClearanceBox {
            min: Vec3::new(12.0, 10.0, 6.5),
            max: Vec3::new(13.0, 10.5, 7.5),
        };
        assert!(check_clearance(&world(0), &piece, &train, &[], &[obstacle]).is_ok());
        train.width_metres = 4.0;
        assert!(matches!(
            check_clearance(&world(0), &piece, &train, &[], &[obstacle]),
            Err(ClearanceCollision::Scenery)
        ));
    }

    #[test]
    fn carved_tunnel_provides_clearance() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        let mut map = world(20);
        for x in 1..=7 {
            for y in 1..=2 {
                map.carve(x, y, 5, 10);
            }
        }
        assert!(check_clearance(&map, &piece, &train, &[], &[]).is_ok());
    }

    #[test]
    fn short_car_samples_are_closer_than_its_length_and_detect_overlap() {
        let piece = straight(4, 4, 3);
        let mut train = hybrid_model().train;
        train.length_metres = 0.2;
        train.cars = 1;
        let distances: Vec<_> = samples(&piece, train_sample_spacing(&train)).collect();
        assert!(
            distances
                .windows(2)
                .all(|pair| pair[1] - pair[0] <= train.length_metres / 2.0 + 1e-10)
        );
        let position = piece.sample(0.25).position + Vec3::new(0.0, 0.0, 0.5);
        let extent = Vec3::new(0.01, 0.01, 0.01);
        let obstacle = ClearanceBox {
            min: position - extent,
            max: position + extent,
        };
        assert_eq!(
            check_clearance(&world(0), &piece, &train, &[], &[obstacle]),
            Err(ClearanceCollision::Scenery)
        );
    }

    #[test]
    fn cached_track_checks_match_uncached_and_can_exclude_a_piece() {
        let piece = straight(4, 4, 3);
        let track = vec![piece.clone(), straight(12, 4, 3)];
        let index = ClearanceIndex::new(&track);
        let map = world(0);
        let train = hybrid_model().train;
        assert_eq!(
            check_clearance_indexed(&map, &piece, &train, &index, None, &[]),
            check_clearance(&map, &piece, &train, &track, &[])
        );
        assert_eq!(
            check_clearance_indexed(&map, &piece, &train, &index, Some(0), &[]),
            check_clearance(&map, &piece, &train, &track[1..], &[])
        );
    }

    #[test]
    fn cached_track_can_be_reordered_and_extended() {
        let first = straight(4, 4, 3);
        let second = straight(12, 4, 3);
        let mut index = ClearanceIndex::new(&[first.clone(), second.clone()]);
        index.reorder(&[1]);
        index.push(&first);
        let map = world(0);
        let train = hybrid_model().train;
        assert_eq!(
            check_clearance_indexed(&map, &first, &train, &index, Some(1), &[]),
            check_clearance(&map, &first, &train, &[second], &[])
        );
        assert_eq!(
            check_clearance_indexed(&map, &first, &train, &index, Some(0), &[]),
            Err(ClearanceCollision::Track)
        );
    }

    #[test]
    fn cached_track_finds_collisions_across_spatial_bin_boundaries() {
        let piece = straight(8, 8, 3);
        let mut track = vec![straight(4, 8, 3), straight(8, 8, 3), straight(12, 8, 3)];
        let mut index = ClearanceIndex::new(&track);
        let map = world(0);
        let train = hybrid_model().train;
        assert_eq!(
            check_clearance_indexed(&map, &piece, &train, &index, Some(0), &[]),
            Err(ClearanceCollision::Track)
        );
        index.reorder(&[2, 0]);
        track.remove(1);
        assert_eq!(
            check_clearance_indexed(&map, &piece, &train, &index, None, &[]),
            check_clearance(&map, &piece, &train, &track, &[])
        );
    }

    #[test]
    fn reverse_track_check_matches_full_check_for_long_cars_and_banked_track() {
        let mut train = hybrid_model().train;
        train.length_metres = 60.0;
        train.cars = 1;
        let map = World::new(
            128,
            128,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let retained = straight(40, 40, 10);
        let index = ClearanceIndex::new(std::slice::from_ref(&retained));
        for y in [20, 32, 38, 40, 42, 48, 60] {
            let candidate = catalog()
                .into_iter()
                .find(|piece| piece.id == PieceId::BankedTurnStandalone)
                .unwrap()
                .at(TrackState {
                    x: 40,
                    y,
                    z: 10,
                    bank: crate::track::Bank::Left,
                    ..TrackState::origin()
                })
                .unwrap();
            assert_eq!(
                check_reverse_track_clearance(&index, &candidate, &train),
                check_clearance(&map, &retained, &train, &[candidate], &[]),
                "y={y}"
            );
        }
    }

    #[test]
    fn reverse_track_check_ignores_world_and_respects_connectors() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        let index = ClearanceIndex::new(std::slice::from_ref(&piece));
        assert_eq!(
            check_clearance(
                &world(20),
                &piece,
                &train,
                std::slice::from_ref(&piece),
                &[]
            ),
            Err(ClearanceCollision::Terrain)
        );
        assert_eq!(
            check_reverse_track_clearance(&index, &piece, &train),
            Err(ClearanceCollision::Track)
        );
        assert_eq!(
            check_reverse_track_clearance(&index, &straight(12, 4, 3), &train),
            Ok(())
        );
    }

    #[test]
    fn existing_track_and_connected_neighbour() {
        let piece = straight(4, 4, 3);
        let train = hybrid_model().train;
        assert!(matches!(
            check_clearance(&world(0), &piece, &train, &[straight(4, 4, 3)], &[]),
            Err(ClearanceCollision::Track)
        ));
        assert!(check_clearance(&world(0), &piece, &train, &[straight(12, 4, 3)], &[]).is_ok());
        assert!(check_clearance(&world(0), &piece, &train, &[straight(4, 8, 3)], &[]).is_ok());
    }

    #[test]
    fn short_connected_piece_does_not_hide_overlapping_track() {
        let train = hybrid_model().train;
        let first = straight(20, 20, 5);
        let short = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::HalfTileStraight)
            .unwrap()
            .at(first.exit)
            .unwrap();
        let candidate = straight(short.exit.x, short.exit.y, short.exit.z);
        let index = ClearanceIndex::new(&[first.clone(), short.clone()]);
        assert_eq!(
            check_clearance(
                &world(0),
                &candidate,
                &train,
                &[first.clone(), short.clone()],
                &[]
            ),
            Ok(())
        );
        assert_eq!(
            check_reverse_track_clearance(&index, &candidate, &train),
            Ok(())
        );
        let overlap = candidate.clone();
        let index = ClearanceIndex::new(&[first.clone(), short.clone(), overlap.clone()]);
        assert_eq!(
            check_clearance(&world(0), &candidate, &train, &[first, short, overlap], &[]),
            Err(ClearanceCollision::Track)
        );
        assert_eq!(
            check_reverse_track_clearance(&index, &candidate, &train),
            Err(ClearanceCollision::Track)
        );
    }

    #[test]
    fn ascending_connected_pieces_clear_their_predecessors() {
        let pieces = crate::physics::demo_circuit();
        let pieces = pieces.pieces();
        let map = World::new(
            512,
            512,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let train = hybrid_model().train;
        for index in 1..3 {
            assert_eq!(
                check_clearance(&map, &pieces[index], &train, &pieces[..index], &[]),
                Ok(()),
                "piece {index}"
            );
        }
        let mut obstacles = pieces[..2].to_vec();
        obstacles.push(pieces[2].clone());
        assert_eq!(
            check_clearance(&map, &pieces[2], &train, &obstacles, &[]),
            Err(ClearanceCollision::Track)
        );
    }

    #[test]
    fn connected_loop_still_checks_track_far_from_the_join() {
        let piece = straight(40, 40, 20);
        let mut connected_loop = catalog()
            .into_iter()
            .find(|item| item.id == PieceId::Loop)
            .unwrap()
            .at(piece.entry)
            .unwrap();
        connected_loop.exit = connected_loop.entry;
        connected_loop.rebuild_arc();
        let map = world(0);
        assert_eq!(
            check_clearance(&map, &piece, &hybrid_model().train, &[connected_loop], &[]),
            Err(ClearanceCollision::Track)
        );
    }
}

#[cfg(test)]
mod tight_clearance_tests {
    use super::*;
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};
    use crate::world::Surface;

    #[test]
    fn half_metre_near_miss_passes_and_overlap_fails() {
        let piece = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(TrackState {
                x: 20,
                y: 20,
                z: 5,
                ..TrackState::origin()
            })
            .unwrap();
        let train = hybrid_model().train;
        let map = World::new(
            64,
            64,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let centre = piece.sample(piece.length() / 2.0).position;
        let obstacle = |gap: f64| ClearanceBox {
            min: Vec3::new(
                centre.x - 0.2,
                centre.y + train.width_metres / 2.0 + gap,
                centre.z + 0.5,
            ),
            max: Vec3::new(
                centre.x + 0.2,
                centre.y + train.width_metres / 2.0 + gap + 0.2,
                centre.z + 1.0,
            ),
        };
        assert_eq!(
            check_clearance(&map, &piece, &train, &[], &[obstacle(0.5)]),
            Ok(())
        );
        assert_eq!(
            check_clearance(&map, &piece, &train, &[], &[obstacle(-0.1)]),
            Err(ClearanceCollision::Scenery)
        );
    }
    #[test]
    fn diagonal_train_allows_half_metre_near_miss() {
        let piece = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(TrackState {
                x: 20,
                y: 20,
                z: 5,
                heading: crate::track::Heading::NorthEast,
                ..TrackState::origin()
            })
            .unwrap();
        let train = hybrid_model().train;
        let map = World::new(
            64,
            64,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let sample = piece.sample(piece.length() / 2.0);
        let left = sample.up.cross(sample.tangent);
        let obstacle = |gap: f64| {
            let centre =
                sample.position + left * (train.width_metres / 2.0 + gap) + sample.up * 1.0;
            let extent = Vec3::new(0.05, 0.05, 0.05);
            ClearanceBox {
                min: centre - extent,
                max: centre + extent,
            }
        };
        assert_eq!(
            check_clearance(&map, &piece, &train, &[], &[obstacle(0.5)]),
            Ok(())
        );
        assert_eq!(
            check_clearance(&map, &piece, &train, &[], &[obstacle(-0.1)]),
            Err(ClearanceCollision::Scenery)
        );
    }
}

impl OrientedEnvelope {
    pub(crate) fn for_track(piece: &Piece, distance: f64, train: &TrainConfiguration) -> Self {
        envelope(piece, distance, train, SAMPLE_SPACING)
    }

    pub(crate) fn intersects_expanded(&self, expansion: f64, obstacle: ClearanceBox) -> bool {
        let mut expanded = *self;
        for half in &mut expanded.half {
            *half += expansion;
        }
        let extent = Vec3::new(
            expanded
                .axes
                .iter()
                .zip(expanded.half)
                .map(|(axis, half)| axis.x.abs() * half)
                .sum(),
            expanded
                .axes
                .iter()
                .zip(expanded.half)
                .map(|(axis, half)| axis.y.abs() * half)
                .sum(),
            expanded
                .axes
                .iter()
                .zip(expanded.half)
                .map(|(axis, half)| axis.z.abs() * half)
                .sum(),
        );
        expanded.bounds = ClearanceBox {
            min: expanded.centre - extent,
            max: expanded.centre + extent,
        };
        intersects_envelope(&expanded, obstacle)
    }

    /// Intersect a support centreline with the envelope, inflated by the support radius.
    pub(crate) fn intersects_segment(
        &self,
        start: Vec3,
        end: Vec3,
        expansion: f64,
        radius: f64,
    ) -> bool {
        let delta = end - start;
        let offset = start - self.centre;
        let mut lower: f64 = 0.0;
        let mut upper: f64 = 1.0;
        for (axis, half) in self.axes.iter().zip(self.half) {
            let extent = half + expansion + radius * (axis.x.abs() + axis.y.abs() + axis.z.abs());
            let origin = offset.dot(*axis);
            let direction = delta.dot(*axis);
            if direction.abs() < 1e-12 {
                if origin.abs() > extent {
                    return false;
                }
            } else {
                let first = (-extent - origin) / direction;
                let second = (extent - origin) / direction;
                lower = lower.max(first.min(second));
                upper = upper.min(first.max(second));
                if lower > upper {
                    return false;
                }
            }
        }
        true
    }
}
