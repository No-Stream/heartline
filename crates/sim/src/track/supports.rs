use crate::track::{Piece, Vec3};
use crate::world::World;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Support {
    pub anchor: Vec3,
    pub footing: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SupportSegment {
    pub start: Vec3,
    pub end: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct SupportObstacle {
    pub min: Vec3,
    pub max: Vec3,
}

/// The outcome of attempting to route supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportError {
    NoRoute,
}

// The renderer draws support members at most 0.045 tiles from their centre line, with 0.1-tile footer half-widths.
// A beam's square cross section may be rotated relative to the horizontal axes.
const BEAM_CLEARANCE: f64 = 0.18 * std::f64::consts::SQRT_2;
const FOOTER_HALF_WIDTH: f64 = 0.4;
const FOOTER_HEIGHT: f64 = 0.2;
const TRACK_RAIL_HALF_SPACING: f64 = 1.0;
const SUPPORT_SEARCH_RADIUS_METRES: i32 = 32;
/// Maximum distance at which world changes can affect generated support geometry.
pub const SUPPORT_INFLUENCE_RADIUS_METRES: f64 =
    SUPPORT_SEARCH_RADIUS_METRES as f64 + 2.0 + FOOTER_HALF_WIDTH;

/// Checks the same beam, brace, and footer clearances used when routing a support.
pub fn support_intersects_obstacle(support: &Support, obstacle: &SupportObstacle) -> bool {
    let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
    let beam_bounds = expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE);
    let footer_bounds = expanded(obstacle, FOOTER_HALF_WIDTH, 0.0);
    intersects(support.footing, top, &beam_bounds)
        || intersects(top, support.anchor, &beam_bounds)
        || intersects(
            support.footing,
            support.footing + Vec3::new(0.0, 0.0, FOOTER_HEIGHT),
            &footer_bounds,
        )
}

/// Routes supports at intervals along a piece. Coordinates and obstacle bounds are in metres.
/// Paths and excavations are taken from the world; callers supply ride and scenery bounds.
/// Footings are searched on a one-metre grid within 32 metres of each anchor,
/// and at obstacle-boundary candidates within a 32-metre square around it.
/// The complete candidate set is checked before returning `NoRoute`.
pub fn generate_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<Support>, SupportError> {
    generate_supports_on_structures(piece, world, obstacles, &[])
}

/// Routes closely spaced supports for a wooden twister coaster.
pub fn generate_dense_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<Support>, SupportError> {
    generate_supports_at_spacing(piece, world, obstacles, &[], 4.0)
}

/// Generate routed steel columns and their horizontal connections to the track.
/// Tall columns gain a braced partner column across the track where one fits.
pub fn generate_steel_columns(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<SupportSegment>, SupportError> {
    generate_framed_supports(piece, world, obstacles, &STEEL_TOWER)
}

fn steel_column_segments(supports: Vec<Support>) -> Vec<SupportSegment> {
    let mut segments = Vec::with_capacity(supports.len() * 2);
    for support in supports {
        let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
        segments.push(SupportSegment {
            start: support.footing,
            end: top,
        });
        if top != support.anchor {
            segments.push(SupportSegment {
                start: top,
                end: support.anchor,
            });
        }
    }
    segments
}

/// Like `generate_supports`, but allows columns to land on explicitly load-bearing structures.
/// Structure bounds are in metres; ordinary obstacles are never assumed to carry a load.
pub fn generate_supports_on_structures(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    structures: &[SupportObstacle],
) -> Result<Vec<Support>, SupportError> {
    generate_supports_at_spacing(piece, world, obstacles, structures, 8.0)
}

fn generate_supports_at_spacing(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    structures: &[SupportObstacle],
    spacing: f64,
) -> Result<Vec<Support>, SupportError> {
    Ok(
        route_supports(piece, world, obstacles, structures, spacing)?
            .into_iter()
            .map(|(support, _)| support)
            .collect(),
    )
}

/// Routes supports as `generate_supports_at_spacing` does, with the track tangent at each anchor.
fn route_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    structures: &[SupportObstacle],
    spacing: f64,
) -> Result<Vec<(Support, Vec3)>, SupportError> {
    for obstacle in obstacles.iter().chain(structures) {
        assert!(
            obstacle.min.x < obstacle.max.x
                && obstacle.min.y < obstacle.max.y
                && obstacle.min.z < obstacle.max.z,
            "invalid obstacle bounds"
        );
    }
    let mut path_obstacles: Vec<_> = world
        .paths()
        .map(|path| SupportObstacle {
            min: Vec3::new(
                f64::from(path.position.x) * 4.0,
                f64::from(path.position.y) * 4.0,
                f64::from(path.position.height),
            ),
            max: Vec3::new(
                (f64::from(path.position.x) + 1.0) * 4.0,
                (f64::from(path.position.y) + 1.0) * 4.0,
                f64::from(path.position.height) + if path.slope.is_some() { 3.0 } else { 2.0 },
            ),
        })
        .collect();
    path_obstacles.extend(world.scenery_boxes().map(|bounds| SupportObstacle {
        min: bounds.min,
        max: bounds.max,
    }));
    let tunnels: Vec<_> = world.carved_spans().collect();
    let inversion_track_segments = piece.inversion.is_some().then(|| {
        let steps = (piece.length() / 0.25).ceil() as usize;
        (0..steps)
            .flat_map(|index| {
                let start_distance = piece.length() * index as f64 / steps as f64;
                let end_distance = piece.length() * (index + 1) as f64 / steps as f64;
                let start = piece.sample(start_distance);
                let end = piece.sample(end_distance);
                let start_lateral = start.up.cross(start.tangent).unit();
                let end_lateral = end.up.cross(end.tangent).unit();
                [-TRACK_RAIL_HALF_SPACING, 0.0, TRACK_RAIL_HALF_SPACING].map(|offset| {
                    (
                        start_distance,
                        end_distance,
                        start.position + start_lateral * offset,
                        end.position + end_lateral * offset,
                    )
                })
            })
            .collect::<Vec<_>>()
    });
    let inverted_track_segments = inverted_track_rail_segments(piece);
    let count = (piece.length() / spacing).ceil() as usize;
    let mut supports = Vec::with_capacity(count);
    for index in 0..count {
        let anchor_distance = piece.length() * (index as f64 + 0.5) / count as f64;
        let sample = piece.sample(anchor_distance);
        if piece.inversion.is_some() && (sample.up.z < 0.0 || sample.tangent.z.abs() > 0.8) {
            // Columns beneath inverted or near-vertical track would cross the ride structure.
            continue;
        }
        let anchor = sample.position;
        if obstacles
            .iter()
            .chain(&path_obstacles)
            .chain(structures)
            .any(|obstacle| {
                intersects(
                    anchor,
                    anchor,
                    &expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE),
                )
            })
        {
            return Err(SupportError::NoRoute);
        }
        let mut selected = None;
        let candidates = footing_candidates(
            anchor,
            obstacles.iter().chain(&path_obstacles).chain(structures),
        );
        for (x, y) in candidates {
            let (width, height) = world.dimensions();
            if x < 0.0 || y < 0.0 || x >= f64::from(width) * 4.0 || y >= f64::from(height) * 4.0 {
                continue;
            }
            let tile_x = (x / 4.0).floor() as u16;
            let tile_y = (y / 4.0).floor() as u16;
            let terrain = terrain_height(world, x, y);
            let tile_tunnels: Vec<_> = tunnels
                .iter()
                .filter(|&&(tx, ty, _, _)| tx == tile_x && ty == tile_y)
                .collect();
            let ground = if anchor.z > terrain {
                // A surface footer must not cover an excavation underneath it.
                if tunnels.iter().any(|&(tunnel_x, tunnel_y, _, _)| {
                    let west = f64::from(tunnel_x) * 4.0;
                    let north = f64::from(tunnel_y) * 4.0;
                    x + FOOTER_HALF_WIDTH > west
                        && x - FOOTER_HALF_WIDTH < west + 4.0
                        && y + FOOTER_HALF_WIDTH > north
                        && y - FOOTER_HALF_WIDTH < north + 4.0
                }) {
                    continue;
                }
                Some(terrain)
            } else {
                tile_tunnels.iter().find_map(|&&(_, _, bottom, top)| {
                    (f64::from(bottom) < anchor.z && anchor.z < f64::from(top))
                        .then_some(f64::from(bottom))
                })
            };
            let Some(ground) = ground else {
                continue;
            };
            let supporting_structure = structures
                .iter()
                .enumerate()
                .filter(|(_, structure)| {
                    x - FOOTER_HALF_WIDTH >= structure.min.x
                        && x + FOOTER_HALF_WIDTH <= structure.max.x
                        && y - FOOTER_HALF_WIDTH >= structure.min.y
                        && y + FOOTER_HALF_WIDTH <= structure.max.y
                        && structure.max.z < anchor.z
                        && structure.max.z >= ground
                })
                .max_by(|(_, first), (_, second)| first.max.z.total_cmp(&second.max.z));
            let z = supporting_structure.map_or(ground, |(_, structure)| structure.max.z);
            let footing = Vec3::new(x, y, z);
            if !brace_clears_terrain(world, &tunnels, Vec3::new(x, y, anchor.z), anchor) {
                continue;
            }
            let top = Vec3::new(x, y, anchor.z);
            if obstacles
                .iter()
                .chain(&path_obstacles)
                .any(|obstacle| support_intersects_obstacle(&Support { anchor, footing }, obstacle))
                || structures.iter().enumerate().any(|(index, structure)| {
                    let beam_bounds = expanded(structure, BEAM_CLEARANCE, BEAM_CLEARANCE);
                    let footer_bounds = expanded(structure, FOOTER_HALF_WIDTH, 0.0);
                    intersects(top, anchor, &beam_bounds)
                        || (supporting_structure
                            .is_none_or(|(support_index, _)| support_index != index)
                            && (intersects(footing, top, &beam_bounds)
                                || intersects(
                                    footing,
                                    footing + Vec3::new(0.0, 0.0, FOOTER_HEIGHT),
                                    &footer_bounds,
                                )))
                })
            {
                continue;
            }
            let support = Support { anchor, footing };
            if inversion_track_segments.as_ref().is_some_and(|segments| {
                !support_clears_inversion_track(&support, anchor_distance, segments)
            }) || inverted_track_segments.as_ref().is_some_and(|segments| {
                !support_clears_inverted_track_projection(&support, segments)
            }) {
                continue;
            }
            selected = Some((support, sample.tangent));
            break;
        }
        let Some(selected) = selected else {
            if inversion_track_segments.is_some() {
                continue;
            }
            return Err(SupportError::NoRoute);
        };
        supports.push(selected);
    }
    Ok(supports)
}

fn support_clears_inversion_track(
    support: &Support,
    anchor_distance: f64,
    track_segments: &[(f64, f64, Vec3, Vec3)],
) -> bool {
    let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
    let support_segments = if top == support.anchor {
        [(support.footing, top), (top, top)]
    } else {
        [(support.footing, top), (top, support.anchor)]
    };
    track_segments
        .iter()
        .all(|(start_distance, end_distance, start, end)| {
            if *start_distance >= anchor_distance - 1.0 && *end_distance <= anchor_distance + 1.0 {
                return true;
            }
            support_segments.iter().all(|(support_start, support_end)| {
                segment_distance(*support_start, *support_end, *start, *end) > BEAM_CLEARANCE
            })
        })
}

fn support_clears_inverted_track_projection(
    support: &Support,
    inverted_track_segments: &[(Vec3, Vec3)],
) -> bool {
    let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
    let support_segments = [(support.footing, top), (top, support.anchor)];
    inverted_track_segments
        .iter()
        .all(|(track_start, track_end)| {
            support_segments.iter().all(|(support_start, support_end)| {
                segment_distance_xy(*support_start, *support_end, *track_start, *track_end)
                    > BEAM_CLEARANCE
            })
        })
}

fn inverted_track_rail_segments(piece: &Piece) -> Option<Vec<(Vec3, Vec3)>> {
    piece.inversion.map(|_| {
        let steps = (piece.length() / 0.25).ceil() as usize;
        (0..steps)
            .filter_map(|index| {
                let start_distance = piece.length() * index as f64 / steps as f64;
                let end_distance = piece.length() * (index + 1) as f64 / steps as f64;
                let start = piece.sample(start_distance);
                let end = piece.sample(end_distance);
                let middle = piece.sample((start_distance + end_distance) / 2.0);
                (middle.up.z < 0.0).then_some((start, end))
            })
            .flat_map(|(start, end)| {
                let start_lateral = start.up.cross(start.tangent).unit();
                let end_lateral = end.up.cross(end.tangent).unit();
                [-TRACK_RAIL_HALF_SPACING, 0.0, TRACK_RAIL_HALF_SPACING].map(|offset| {
                    (
                        start.position + start_lateral * offset,
                        end.position + end_lateral * offset,
                    )
                })
            })
            .collect()
    })
}

fn segment_distance_xy(
    first_start: Vec3,
    first_end: Vec3,
    second_start: Vec3,
    second_end: Vec3,
) -> f64 {
    segment_distance(
        Vec3::new(first_start.x, first_start.y, 0.0),
        Vec3::new(first_end.x, first_end.y, 0.0),
        Vec3::new(second_start.x, second_start.y, 0.0),
        Vec3::new(second_end.x, second_end.y, 0.0),
    )
}

fn segment_distance(
    first_start: Vec3,
    first_end: Vec3,
    second_start: Vec3,
    second_end: Vec3,
) -> f64 {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let between_starts = first_start - second_start;
    let first_length_squared = first_direction.dot(first_direction);
    let second_length_squared = second_direction.dot(second_direction);
    let direction_dot = first_direction.dot(second_direction);
    let first_offset = first_direction.dot(between_starts);
    let second_offset = second_direction.dot(between_starts);
    let denominator = first_length_squared * second_length_squared - direction_dot * direction_dot;

    let mut first_numerator;
    let mut first_denominator = denominator;
    let mut second_numerator;
    let mut second_denominator = denominator;
    if denominator < 1e-12 {
        first_numerator = 0.0;
        first_denominator = 1.0;
        second_numerator = second_offset;
        second_denominator = second_length_squared;
    } else {
        first_numerator = direction_dot * second_offset - second_length_squared * first_offset;
        second_numerator = first_length_squared * second_offset - direction_dot * first_offset;
        if first_numerator < 0.0 {
            first_numerator = 0.0;
            second_numerator = second_offset;
            second_denominator = second_length_squared;
        } else if first_numerator > first_denominator {
            first_numerator = first_denominator;
            second_numerator = second_offset + direction_dot;
            second_denominator = second_length_squared;
        }
    }
    if second_numerator < 0.0 {
        second_numerator = 0.0;
        if -first_offset < 0.0 {
            first_numerator = 0.0;
        } else if -first_offset > first_length_squared {
            first_numerator = first_denominator;
        } else {
            first_numerator = -first_offset;
            first_denominator = first_length_squared;
        }
    } else if second_numerator > second_denominator {
        second_numerator = second_denominator;
        if -first_offset + direction_dot < 0.0 {
            first_numerator = 0.0;
        } else if -first_offset + direction_dot > first_length_squared {
            first_numerator = first_denominator;
        } else {
            first_numerator = -first_offset + direction_dot;
            first_denominator = first_length_squared;
        }
    }
    let first_fraction = if first_numerator.abs() < 1e-12 {
        0.0
    } else {
        first_numerator / first_denominator
    };
    let second_fraction = if second_numerator.abs() < 1e-12 {
        0.0
    } else {
        second_numerator / second_denominator
    };
    (between_starts + first_direction * first_fraction - second_direction * second_fraction)
        .length()
}

fn boundary_candidates<'a>(
    anchor: Vec3,
    obstacles: impl Iterator<Item = &'a SupportObstacle>,
    clearances: &[f64],
) -> Vec<(f64, f64)> {
    let mut candidate_x = vec![anchor.x];
    let mut candidate_y = vec![anchor.y];
    for obstacle in obstacles {
        if obstacle.max.x < anchor.x - 32.0
            || obstacle.min.x > anchor.x + 32.0
            || obstacle.max.y < anchor.y - 32.0
            || obstacle.min.y > anchor.y + 32.0
        {
            continue;
        }
        for &clearance in clearances {
            for boundary in [
                obstacle.min.x - clearance - 0.001,
                obstacle.max.x + clearance + 0.001,
            ] {
                if (boundary - anchor.x).abs() <= f64::from(SUPPORT_SEARCH_RADIUS_METRES) {
                    candidate_x.push(boundary);
                }
            }
            for boundary in [
                obstacle.min.y - clearance - 0.001,
                obstacle.max.y + clearance + 0.001,
            ] {
                if (boundary - anchor.y).abs() <= f64::from(SUPPORT_SEARCH_RADIUS_METRES) {
                    candidate_y.push(boundary);
                }
            }
        }
    }
    candidate_x.sort_by(f64::total_cmp);
    candidate_x.dedup();
    candidate_y.sort_by(f64::total_cmp);
    candidate_y.dedup();
    let mut candidates: Vec<_> = candidate_x
        .iter()
        .flat_map(|&x| candidate_y.iter().map(move |&y| (x, y)))
        .collect();
    candidates.sort_by(|first, second| {
        ((first.0 - anchor.x).powi(2) + (first.1 - anchor.y).powi(2))
            .total_cmp(&((second.0 - anchor.x).powi(2) + (second.1 - anchor.y).powi(2)))
    });
    candidates
}

fn footing_candidates<'a>(
    anchor: Vec3,
    obstacles: impl Iterator<Item = &'a SupportObstacle>,
) -> impl Iterator<Item = (f64, f64)> + 'a {
    let obstacles: Vec<_> = obstacles.collect();
    let boundary_obstacles = obstacles.clone();
    static NEARBY_OFFSETS: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    let offsets = NEARBY_OFFSETS.get_or_init(|| {
        let mut offsets: Vec<_> = (-8_i32..=8)
            .flat_map(|dx| (-8_i32..=8).map(move |dy| (dx, dy)))
            .collect();
        offsets.sort_by_key(|(dx, dy)| (dx * dx + dy * dy, *dx, *dy));
        offsets
    });
    offsets
        .iter()
        .map(move |&(dx, dy)| (anchor.x + f64::from(dx), anchor.y + f64::from(dy)))
        .chain(
            std::iter::once_with(move || {
                boundary_candidates(anchor, boundary_obstacles.into_iter(), &[FOOTER_HALF_WIDTH])
            })
            .flatten(),
        )
        .chain(
            std::iter::once_with(move || additional_footing_candidates(anchor, &obstacles))
                .flatten(),
        )
}

fn additional_footing_candidates(anchor: Vec3, obstacles: &[&SupportObstacle]) -> Vec<(f64, f64)> {
    let radius_squared = f64::from(SUPPORT_SEARCH_RADIUS_METRES).powi(2);
    let mut additional: Vec<_> = (-SUPPORT_SEARCH_RADIUS_METRES..=SUPPORT_SEARCH_RADIUS_METRES)
        .flat_map(|dx| {
            (-SUPPORT_SEARCH_RADIUS_METRES..=SUPPORT_SEARCH_RADIUS_METRES).map(move |dy| (dx, dy))
        })
        .filter(|(dx, dy)| f64::from(dx * dx + dy * dy) <= radius_squared)
        .map(|(dx, dy)| (anchor.x + f64::from(dx), anchor.y + f64::from(dy)))
        .collect();
    additional.extend(boundary_candidates(
        anchor,
        obstacles.iter().copied(),
        &[BEAM_CLEARANCE, FOOTER_HALF_WIDTH],
    ));
    additional.retain(|(x, y)| (x - anchor.x).powi(2) + (y - anchor.y).powi(2) <= radius_squared);
    additional.sort_by(|first, second| {
        ((first.0 - anchor.x).powi(2) + (first.1 - anchor.y).powi(2))
            .total_cmp(&((second.0 - anchor.x).powi(2) + (second.1 - anchor.y).powi(2)))
            .then_with(|| first.0.total_cmp(&second.0))
            .then_with(|| first.1.total_cmp(&second.1))
    });
    additional.dedup_by(|first, second| first.0 == second.0 && first.1 == second.1);
    additional
}

fn expanded(obstacle: &SupportObstacle, horizontal: f64, vertical: f64) -> SupportObstacle {
    let padding = Vec3::new(horizontal, horizontal, vertical);
    SupportObstacle {
        min: obstacle.min - padding,
        max: obstacle.max + padding,
    }
}

/// Generate checked wooden frames around each routed support, in metres.
pub fn generate_wooden_lattice(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<SupportSegment>, SupportError> {
    generate_framed_supports(piece, world, obstacles, &WOODEN_BENT)
}

/// Generate closely spaced wooden frames for a wooden twister coaster.
pub fn generate_dense_wooden_lattice(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<SupportSegment>, SupportError> {
    generate_framed_supports(piece, world, obstacles, &DENSE_WOODEN_BENT)
}

/// Routes supports at the shape's spacing and builds a checked frame around each.
fn generate_framed_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    shape: &FrameShape,
) -> Result<Vec<SupportSegment>, SupportError> {
    let supports = route_supports(piece, world, obstacles, &[], shape.spacing)?;
    Ok(framed_support_segments(
        piece, world, obstacles, supports, shape,
    ))
}

/// How diagonal members brace each storey of a support frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bracing {
    /// Two crossing diagonals per storey, as in a timber bent.
    Cross,
    /// One diagonal per storey, alternating direction, as in a steel ladder tower.
    Zigzag,
}

/// The shape of the frames built around routed supports. Lengths are in metres.
struct FrameShape {
    /// Distance between routed supports along the track.
    spacing: f64,
    /// Frames are built only under track at least this far above the footing; lower supports stay single columns.
    minimum_height: f64,
    /// Target height of each braced storey; ledgers separate the storeys.
    storey_height: f64,
    bracing: Bracing,
    /// Leg positions to try, each a pair of offsets from the routed footing across the track (positive to its left).
    across_track_legs: &'static [(f64, f64)],
    /// Whether to fall back to `FALLBACK_LEG_OFFSETS` when no leg pair across the track fits.
    fixed_leg_fallback: bool,
}

/// Timber bents straddle the track on legs either side of it; failing that, a leg stands at the routed footing.
const WOODEN_BENT: FrameShape = FrameShape {
    spacing: 8.0,
    minimum_height: 0.0,
    storey_height: 5.0,
    bracing: Bracing::Cross,
    across_track_legs: &[(-1.2, 1.2), (0.0, 2.0), (0.0, -2.0)],
    fixed_leg_fallback: true,
};
const DENSE_WOODEN_BENT: FrameShape = FrameShape {
    spacing: 4.0,
    storey_height: 3.5,
    ..WOODEN_BENT
};
/// Tall steel columns get a partner column and zigzag bracing; the routed column always stands.
const STEEL_TOWER: FrameShape = FrameShape {
    spacing: 8.0,
    minimum_height: 10.0,
    storey_height: 6.0,
    bracing: Bracing::Zigzag,
    across_track_legs: &[(0.0, 1.8), (0.0, -1.8)],
    fixed_leg_fallback: false,
};
/// Fixed leg positions, in metres from the routed footing, tried when no leg pair across the track fits.
const FALLBACK_LEG_OFFSETS: [(f64, f64); 8] = [
    (2.0, 0.0),
    (-2.0, 0.0),
    (0.0, 2.0),
    (0.0, -2.0),
    (2.0, 2.0),
    (-2.0, -2.0),
    (2.0, -2.0),
    (-2.0, 2.0),
];

/// The members of a two-legged frame from its footings up to `centre`, the top of the routed column under the track.
fn frame_segments(
    first: Vec3,
    second: Vec3,
    centre: Vec3,
    storey_height: f64,
    bracing: Bracing,
) -> Vec<SupportSegment> {
    let at = |leg: Vec3, z: f64| Vec3::new(leg.x, leg.y, z);
    let segment = |start: Vec3, end: Vec3| SupportSegment { start, end };
    let mut segments = vec![
        segment(first, at(first, centre.z)),
        segment(second, at(second, centre.z)),
    ];
    for leg in [first, second] {
        if at(leg, centre.z) != centre {
            segments.push(segment(at(leg, centre.z), centre));
        }
    }
    let bottom = first.z.max(second.z) + 0.3;
    let upper = centre.z - 0.3;
    let storeys = ((upper - bottom) / storey_height).round().max(1.0) as usize;
    let height = (upper - bottom) / storeys as f64;
    for storey in 0..storeys {
        let low = bottom + height * storey as f64;
        let high = low + height;
        if bracing == Bracing::Cross || storey % 2 == 0 {
            segments.push(segment(at(first, low), at(second, high)));
        }
        if bracing == Bracing::Cross || storey % 2 == 1 {
            segments.push(segment(at(second, low), at(first, high)));
        }
        if storey > 0 {
            segments.push(segment(at(first, low), at(second, low)));
        }
    }
    segments
}

/// Builds the checked frame for each routed support, or keeps its single column when no frame fits.
fn framed_support_segments(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    supports: Vec<(Support, Vec3)>,
    shape: &FrameShape,
) -> Vec<SupportSegment> {
    if piece.inversion.is_some() {
        return steel_column_segments(supports.into_iter().map(|(support, _)| support).collect());
    }
    let mut occupied = obstacles.to_vec();
    occupied.extend(world.scenery_boxes().map(|bounds| SupportObstacle {
        min: bounds.min,
        max: bounds.max,
    }));
    occupied.extend(world.paths().map(|path| SupportObstacle {
        min: Vec3::new(
            f64::from(path.position.x) * 4.0,
            f64::from(path.position.y) * 4.0,
            f64::from(path.position.height),
        ),
        max: Vec3::new(
            (f64::from(path.position.x) + 1.0) * 4.0,
            (f64::from(path.position.y) + 1.0) * 4.0,
            f64::from(path.position.height) + if path.slope.is_some() { 3.0 } else { 2.0 },
        ),
    }));
    let tunnels: Vec<_> = world.carved_spans().collect();
    // The footing of a new leg at (x, y), on terrain or a tunnel floor below the track at `track_z`.
    let leg_footing = |x: f64, y: f64, track_z: f64| -> Option<Vec3> {
        let (width, height) = world.dimensions();
        if x < 0.4
            || y < 0.4
            || x + 0.4 >= f64::from(width) * 4.0
            || y + 0.4 >= f64::from(height) * 4.0
        {
            return None;
        }
        let terrain = terrain_height(world, x, y);
        let ground = if terrain < track_z {
            if tunnels.iter().any(|&(tx, ty, _, _)| {
                let west = f64::from(tx) * 4.0;
                let north = f64::from(ty) * 4.0;
                x + FOOTER_HALF_WIDTH > west
                    && x - FOOTER_HALF_WIDTH < west + 4.0
                    && y + FOOTER_HALF_WIDTH > north
                    && y - FOOTER_HALF_WIDTH < north + 4.0
            }) {
                return None;
            }
            terrain
        } else {
            let tile_x = (x / 4.0).floor() as u16;
            let tile_y = (y / 4.0).floor() as u16;
            let (_, _, bottom, _) = tunnels.iter().find(|&&(tx, ty, bottom, top)| {
                tx == tile_x
                    && ty == tile_y
                    && f64::from(bottom) < track_z
                    && f64::from(top) > track_z
            })?;
            let ground = f64::from(*bottom);
            let footer_fits = [-FOOTER_HALF_WIDTH, FOOTER_HALF_WIDTH]
                .into_iter()
                .all(|dx| {
                    [-FOOTER_HALF_WIDTH, FOOTER_HALF_WIDTH]
                        .into_iter()
                        .all(|dy| {
                            tunnels.iter().any(|&(tx, ty, bottom, top)| {
                                tx == ((x + dx) / 4.0).floor() as u16
                                    && ty == ((y + dy) / 4.0).floor() as u16
                                    && f64::from(bottom) <= ground
                                    && f64::from(top) > track_z
                            })
                        })
                });
            if !footer_fits {
                return None;
            }
            ground
        };
        Some(Vec3::new(x, y, ground))
    };
    let footer_clear = |leg: Vec3| {
        !occupied.iter().any(|obstacle| {
            intersects(
                leg,
                leg + Vec3::new(0.0, 0.0, FOOTER_HEIGHT),
                &expanded(obstacle, FOOTER_HALF_WIDTH, 0.0),
            )
        })
    };
    let members_clear = |segments: &[SupportSegment]| {
        segments.iter().all(|segment| {
            !occupied.iter().any(|obstacle| {
                intersects(
                    segment.start,
                    segment.end,
                    &expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE),
                )
            }) && ((segment.start.x == segment.end.x && segment.start.y == segment.end.y)
                || brace_clears_terrain(world, &tunnels, segment.start, segment.end))
        })
    };
    let mut result = Vec::new();
    for (support, tangent) in supports {
        let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
        let horizontal = Vec3::new(tangent.x, tangent.y, 0.0);
        let across =
            (horizontal.length() > 0.3).then(|| Vec3::new(-horizontal.y, horizontal.x, 0.0).unit());
        let across_track = across.into_iter().flat_map(|across| {
            shape
                .across_track_legs
                .iter()
                .map(move |&(first, second)| (across * first, across * second))
        });
        let fallback = shape
            .fixed_leg_fallback
            .then_some(FALLBACK_LEG_OFFSETS)
            .into_iter()
            .flatten()
            .map(|(dx, dy)| (Vec3::new(0.0, 0.0, 0.0), Vec3::new(dx, dy, 0.0)));
        let frame = (top.z - support.footing.z >= shape.minimum_height)
            .then(|| {
                across_track
                    .chain(fallback)
                    .find_map(|(first_offset, second_offset)| {
                        let leg = |offset: Vec3| {
                            if offset == Vec3::new(0.0, 0.0, 0.0) {
                                Some(support.footing)
                            } else {
                                let footing = leg_footing(
                                    support.footing.x + offset.x,
                                    support.footing.y + offset.y,
                                    support.anchor.z,
                                )?;
                                footer_clear(footing).then_some(footing)
                            }
                        };
                        let (first, second) = (leg(first_offset)?, leg(second_offset)?);
                        if top.z - first.z < 0.7 || top.z - second.z < 0.7 {
                            return None;
                        }
                        let segments =
                            frame_segments(first, second, top, shape.storey_height, shape.bracing);
                        members_clear(&segments).then_some(segments)
                    })
            })
            .flatten();
        result.extend(frame.unwrap_or_else(|| {
            vec![SupportSegment {
                start: support.footing,
                end: top,
            }]
        }));
        if top != support.anchor {
            result.push(SupportSegment {
                start: top,
                end: support.anchor,
            });
        }
    }
    result
}

fn terrain_height(world: &World, x: f64, y: f64) -> f64 {
    let tile_x = (x / 4.0).floor() as u16;
    let tile_y = (y / 4.0).floor() as u16;
    let corners = world.surface(tile_x, tile_y).corners.map(f64::from);
    let fx = x / 4.0 - f64::from(tile_x);
    let fy = y / 4.0 - f64::from(tile_y);
    if fx >= fy {
        corners[0] * (1.0 - fx) + corners[1] * (fx - fy) + corners[2] * fy
    } else {
        corners[0] * (1.0 - fy) + corners[2] * fx + corners[3] * (fy - fx)
    }
}

fn brace_clears_terrain(
    world: &World,
    tunnels: &[(u16, u16, i16, i16)],
    start: Vec3,
    end: Vec3,
) -> bool {
    let delta = end - start;
    let steps = (delta.length() / 0.25).ceil() as usize;
    [-BEAM_CLEARANCE, BEAM_CLEARANCE].into_iter().all(|dx| {
        [-BEAM_CLEARANCE, BEAM_CLEARANCE].into_iter().all(|dy| {
            let ray_start = start + Vec3::new(dx, dy, 0.0);
            let mut checkpoints: Vec<f64> = (0..=steps)
                .map(|index| index as f64 / steps.max(1) as f64)
                .collect();
            for (origin, change) in [
                (ray_start.x, delta.x),
                (ray_start.y, delta.y),
                (ray_start.x - ray_start.y, delta.x - delta.y),
            ] {
                if change != 0.0 {
                    let first = (origin.min(origin + change) / 4.0).ceil() as i32;
                    let last = (origin.max(origin + change) / 4.0).floor() as i32;
                    for boundary in first..=last {
                        let t = (f64::from(boundary) * 4.0 - origin) / change;
                        if t > 0.0 && t < 1.0 {
                            checkpoints.push(t);
                        }
                    }
                }
            }
            checkpoints.sort_by(f64::total_cmp);
            // Inside each interval the selected terrain triangle is a plane, and each
            // tunnel boundary is horizontal. Split where the ray crosses either.
            let mut crossings = Vec::new();
            for pair in checkpoints.windows(2) {
                let midpoint = ray_start + delta * ((pair[0] + pair[1]) / 2.0);
                let (width, height) = world.dimensions();
                if midpoint.x < 0.0
                    || midpoint.y < 0.0
                    || midpoint.x >= f64::from(width) * 4.0
                    || midpoint.y >= f64::from(height) * 4.0
                {
                    return false;
                }
                let tile_x = (midpoint.x / 4.0).floor() as u16;
                let tile_y = (midpoint.y / 4.0).floor() as u16;
                let corners = world.surface(tile_x, tile_y).corners.map(f64::from);
                let upper =
                    midpoint.x / 4.0 - f64::from(tile_x) >= midpoint.y / 4.0 - f64::from(tile_y);
                let clearance = |t: f64| {
                    let point = ray_start + delta * t;
                    let x = point.x / 4.0 - f64::from(tile_x);
                    let y = point.y / 4.0 - f64::from(tile_y);
                    let terrain = if upper {
                        corners[0] * (1.0 - x) + corners[1] * (x - y) + corners[2] * y
                    } else {
                        corners[0] * (1.0 - y) + corners[2] * x + corners[3] * (y - x)
                    };
                    point.z - terrain
                };
                for dz in [-BEAM_CLEARANCE, BEAM_CLEARANCE] {
                    let first = clearance(pair[0]) + dz;
                    let last = clearance(pair[1]) + dz;
                    if first * last < 0.0 {
                        crossings.push(pair[0] + (pair[1] - pair[0]) * first / (first - last));
                    }
                    if delta.z != 0.0 {
                        for &(_, _, bottom, top) in tunnels
                            .iter()
                            .filter(|&&(x, y, _, _)| x == tile_x && y == tile_y)
                        {
                            for level in [bottom, top] {
                                let t = (f64::from(level) - ray_start.z - dz) / delta.z;
                                if t > pair[0] && t < pair[1] {
                                    crossings.push(t);
                                }
                            }
                        }
                    }
                }
            }
            checkpoints.extend(crossings);
            checkpoints.sort_by(f64::total_cmp);
            let midpoints: Vec<_> = checkpoints
                .windows(2)
                .map(|pair| (pair[0] + pair[1]) / 2.0)
                .collect();
            checkpoints.extend(midpoints);
            checkpoints.into_iter().all(|t| {
                [-BEAM_CLEARANCE, BEAM_CLEARANCE].into_iter().all(|dz| {
                    let point = ray_start + delta * t + Vec3::new(0.0, 0.0, dz);
                    let (width, height) = world.dimensions();
                    point.x >= 0.0
                        && point.y >= 0.0
                        && point.x < f64::from(width) * 4.0
                        && point.y < f64::from(height) * 4.0
                        && (point.z > terrain_height(world, point.x, point.y)
                            || tunnels.iter().any(|&(x, y, bottom, top)| {
                                x == (point.x / 4.0).floor() as u16
                                    && y == (point.y / 4.0).floor() as u16
                                    && point.z > f64::from(bottom)
                                    && point.z < f64::from(top)
                            }))
                })
            })
        })
    })
}

fn intersects(start: Vec3, end: Vec3, bounds: &SupportObstacle) -> bool {
    let mut enter: f64 = 0.0;
    let mut exit: f64 = 1.0;
    for (origin, direction, min, max) in [
        (start.x, end.x - start.x, bounds.min.x, bounds.max.x),
        (start.y, end.y - start.y, bounds.min.y, bounds.max.y),
        (start.z, end.z - start.z, bounds.min.z, bounds.max.z),
    ] {
        if direction.abs() < 1e-12 {
            if origin < min || origin > max {
                return false;
            }
        } else {
            let first = (min - origin) / direction;
            let second = (max - origin) / direction;
            enter = enter.max(first.min(second));
            exit = exit.min(first.max(second));
            if enter > exit {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{Heading, PieceId, TrackState, catalog, hybrid_model};
    use crate::world::{Path, Position, Surface};

    const SUPPORT_ATTACHMENT_NEIGHBORHOOD_METRES: f64 = 1.0;

    fn piece() -> Piece {
        let state = TrackState {
            x: 4,
            y: 4,
            z: 5,
            ..TrackState::origin()
        };
        catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(state)
            .unwrap()
    }

    fn world() -> World {
        World::new(
            16,
            16,
            Surface {
                corners: [0; 4],
                water: None,
            },
        )
    }

    fn assert_support_segments_clear_of_track(
        piece: &Piece,
        supports: &[Support],
        segments: &[SupportSegment],
        spacing: f64,
        track_half_width: f64,
        context: &str,
    ) {
        let support_anchors: Vec<_> = supports
            .iter()
            .map(|support| {
                let count = (piece.length() / spacing).ceil() as usize;
                let (_, distance) = (0..count)
                    .map(|index| {
                        let distance = piece.length() * (index as f64 + 0.5) / count as f64;
                        (piece.sample(distance).position, distance)
                    })
                    .find(|(anchor, _)| *anchor == support.anchor)
                    .expect("each support anchor is sampled from its piece");
                (
                    support.anchor,
                    support.footing,
                    Vec3::new(support.footing.x, support.footing.y, support.anchor.z),
                    distance,
                )
            })
            .collect();
        let steps = (piece.length() / 0.25).ceil() as usize;
        for segment in segments {
            let nearest_attachment = support_anchors
                .iter()
                .map(|(anchor, footing, top, distance)| {
                    let endpoint_distance = [segment.start, segment.end]
                        .into_iter()
                        .flat_map(|endpoint| {
                            [*anchor, *footing, *top].map(|point| (endpoint - point).length())
                        })
                        .fold(f64::INFINITY, f64::min);
                    (endpoint_distance, *distance)
                })
                .min_by(|first, second| first.0.total_cmp(&second.0));
            let attachment_distance = nearest_attachment
                .filter(|(endpoint_distance, _)| *endpoint_distance < 0.01)
                .map(|(_, distance)| distance);
            for index in 0..steps {
                let start_distance = piece.length() * index as f64 / steps as f64;
                let end_distance = piece.length() * (index + 1) as f64 / steps as f64;
                if attachment_distance.is_some_and(|distance| {
                    start_distance >= distance - SUPPORT_ATTACHMENT_NEIGHBORHOOD_METRES
                        && end_distance <= distance + SUPPORT_ATTACHMENT_NEIGHBORHOOD_METRES
                }) {
                    continue;
                }
                let first_sample = piece.sample(start_distance);
                let second_sample = piece.sample(end_distance);
                let first_lateral = first_sample.up.cross(first_sample.tangent).unit();
                let second_lateral = second_sample.up.cross(second_sample.tangent).unit();
                for offset in [-track_half_width, 0.0, track_half_width] {
                    let first_rail = first_sample.position + first_lateral * offset;
                    let second_rail = second_sample.position + second_lateral * offset;
                    assert!(
                        segment_distance(segment.start, segment.end, first_rail, second_rail,)
                            > BEAM_CLEARANCE,
                        "{context}: support segment {segment:?} intersects the loop track near distance {start_distance:.2} m; attachment at {attachment_distance:?} m (nearest support point {nearest_attachment:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn dense_lattice_follows_the_curve_and_clears_crossing_track() {
        let piece = piece();
        let world = world();
        let obstacle = SupportObstacle {
            min: Vec3::new(18.0, 15.0, 1.0),
            max: Vec3::new(20.0, 17.0, 3.0),
        };
        let dense = generate_dense_wooden_lattice(&piece, &world, &[obstacle]).unwrap();
        let standard = generate_wooden_lattice(&piece, &world, &[obstacle]).unwrap();
        assert!(dense.len() > standard.len());
        for segment in &dense {
            assert!(!intersects(
                segment.start,
                segment.end,
                &expanded(&obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE),
            ));
        }
        for index in 0..(piece.length() / 4.0).ceil() as usize {
            let count = (piece.length() / 4.0).ceil() as usize;
            let anchor = piece
                .sample(piece.length() * (index as f64 + 0.5) / count as f64)
                .position;
            assert!(dense.iter().any(|segment| segment.end == anchor));
        }
    }

    fn straight_at_height(z: i32) -> Piece {
        catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .at(TrackState {
                x: 4,
                y: 4,
                z,
                ..TrackState::origin()
            })
            .unwrap()
    }

    fn is_ledger(segment: &SupportSegment) -> bool {
        segment.start.z == segment.end.z
            && (segment.start.x != segment.end.x || segment.start.y != segment.end.y)
    }

    #[test]
    fn wooden_bents_straddle_the_track_with_braced_storeys() {
        let piece = straight_at_height(10);
        let anchors: Vec<_> = generate_supports(&piece, &world(), &[])
            .unwrap()
            .into_iter()
            .map(|support| support.anchor)
            .collect();
        let lattice = generate_wooden_lattice(&piece, &world(), &[]).unwrap();
        let dense = generate_dense_wooden_lattice(&piece, &world(), &[]).unwrap();
        for anchor in &anchors {
            for side in [-1.2, 1.2] {
                assert!(
                    lattice.iter().any(|segment| segment.start
                        == Vec3::new(anchor.x, anchor.y + side, 0.0)
                        && segment.end == Vec3::new(anchor.x, anchor.y + side, anchor.z)),
                    "a leg stands {side} m across the track from {anchor:?}"
                );
            }
        }
        let ledgers_per_bent = |segments: &[SupportSegment], bents: usize| {
            segments.iter().filter(|segment| is_ledger(segment)).count() as f64 / bents as f64
        };
        let dense_bents = dense
            .iter()
            .filter(|segment| segment.start.z == 0.0 && segment.end.z > 0.0)
            .count()
            / 2;
        assert!(ledgers_per_bent(&dense, dense_bents) > ledgers_per_bent(&lattice, anchors.len()));
    }

    #[test]
    fn tall_steel_columns_gain_a_braced_partner_and_short_ones_stay_single() {
        let short = generate_steel_columns(&straight_at_height(3), &world(), &[]).unwrap();
        assert!(short.iter().all(|segment| segment.start.x == segment.end.x
            && segment.start.y == segment.end.y));
        let piece = straight_at_height(12);
        let supports = generate_supports(&piece, &world(), &[]).unwrap();
        let tall = generate_steel_columns(&piece, &world(), &[]).unwrap();
        for support in &supports {
            let partner = tall.iter().find(|segment| {
                segment.start.z == 0.0
                    && segment.start.x == support.footing.x
                    && ((segment.start.y - support.footing.y).abs() - 1.8).abs() < 1e-9
            });
            assert!(
                partner.is_some(),
                "a partner column stands beside {support:?}"
            );
        }
        let diagonals = tall
            .iter()
            .filter(|segment| segment.start.z != segment.end.z && segment.start.y != segment.end.y)
            .count();
        assert!(diagonals >= supports.len() * 2, "{tall:?}");
    }

    #[test]
    fn steel_columns_follow_the_checked_support_route() {
        let piece = piece();
        let world = world();
        let supports = generate_supports(&piece, &world, &[]).unwrap();
        let segments = generate_steel_columns(&piece, &world, &[]).unwrap();
        assert!(!segments.is_empty());
        for support in supports {
            assert!(
                segments
                    .iter()
                    .any(|segment| segment.start == support.footing)
            );
            assert!(segments.iter().any(|segment| segment.end == support.anchor));
        }
    }

    #[test]
    fn loop_supports_do_not_intersect_other_parts_of_the_track() {
        let world = World::new(
            32,
            32,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let model = hybrid_model();
        for piece_id in [
            PieceId::Loop,
            PieceId::SteelLoop,
            PieceId::LoopRight,
            PieceId::SteelLoopRight,
        ] {
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
                let loop_piece = catalog()
                    .into_iter()
                    .find(|piece| piece.id == piece_id)
                    .unwrap()
                    .at(TrackState {
                        x: 32,
                        y: 32,
                        z: 5,
                        heading,
                        ..TrackState::origin()
                    })
                    .unwrap();
                for (style, spacing) in [("steel", 8.0), ("wooden", 8.0), ("dense wooden", 4.0)] {
                    let segments = match style {
                        "steel" => generate_steel_columns(&loop_piece, &world, &[]).unwrap(),
                        "wooden" => generate_wooden_lattice(&loop_piece, &world, &[]).unwrap(),
                        "dense wooden" => {
                            generate_dense_wooden_lattice(&loop_piece, &world, &[]).unwrap()
                        }
                        _ => unreachable!(),
                    };
                    assert!(!segments.is_empty(), "{piece_id:?}, {heading:?}, {style}");
                    let supports =
                        generate_supports_at_spacing(&loop_piece, &world, &[], &[], spacing)
                            .unwrap();
                    assert_support_segments_clear_of_track(
                        &loop_piece,
                        &supports,
                        &segments,
                        spacing,
                        model.train.width_metres / 2.0,
                        &format!("{piece_id:?}, {heading:?}, {style}, {spacing} m spacing"),
                    );
                }
            }
        }
    }

    #[test]
    fn inversion_supports_skip_inverted_sections_and_clear_the_track() {
        let world = World::new(
            64,
            64,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let model = hybrid_model();
        for template in catalog()
            .into_iter()
            .filter(|piece| piece.inversion.is_some())
        {
            let piece = template
                .at(TrackState {
                    x: 32,
                    y: 32,
                    z: 32,
                    ..template.entry
                })
                .unwrap();
            for (style, spacing) in [("steel", 8.0), ("wooden", 8.0), ("dense wooden", 4.0)] {
                let segments = match style {
                    "steel" => generate_steel_columns(&piece, &world, &[]).unwrap(),
                    "wooden" => generate_wooden_lattice(&piece, &world, &[]).unwrap(),
                    "dense wooden" => generate_dense_wooden_lattice(&piece, &world, &[]).unwrap(),
                    _ => unreachable!(),
                };
                let supports =
                    generate_supports_at_spacing(&piece, &world, &[], &[], spacing).unwrap();
                let inverted_segments = inverted_track_rail_segments(&piece).unwrap();
                assert!(
                    !supports.is_empty(),
                    "{:?}, {style}: no routed supports remain",
                    piece.id
                );
                let count = (piece.length() / spacing).ceil() as usize;
                for support in &supports {
                    let anchor_distance = (0..count)
                        .map(|index| piece.length() * (index as f64 + 0.5) / count as f64)
                        .find(|&distance| piece.sample(distance).position == support.anchor)
                        .expect("each support anchor is sampled from its piece");
                    let sample = piece.sample(anchor_distance);
                    assert!(
                        sample.up.z >= 0.0 && sample.tangent.z.abs() <= 0.8,
                        "{:?}, {style}: support {support:?} is beneath an inverted or near-vertical section",
                        piece.id
                    );
                    assert!(
                        support_clears_inverted_track_projection(support, &inverted_segments),
                        "{:?}, {style}: support {support:?} passes beneath inverted track",
                        piece.id
                    );
                }
                assert_support_segments_clear_of_track(
                    &piece,
                    &supports,
                    &segments,
                    spacing,
                    model.train.width_metres / 2.0,
                    &format!("{:?}, {style}, {spacing} m spacing", piece.id),
                );
            }
        }
    }

    #[test]
    fn support_obstacle_collision_checks_beam_brace_and_footer() {
        let support = Support {
            footing: Vec3::new(0.0, 0.0, 0.0),
            anchor: Vec3::new(4.0, 4.0, 8.0),
        };
        for (min, max, expected) in [
            (Vec3::new(0.1, 0.1, 3.0), Vec3::new(0.2, 0.2, 4.0), true),
            (Vec3::new(1.9, 1.9, 7.9), Vec3::new(2.1, 2.1, 8.1), true),
            (Vec3::new(0.3, 0.3, 0.0), Vec3::new(0.4, 0.4, 0.1), true),
            (Vec3::new(0.0, 4.0, 4.0), Vec3::new(0.1, 4.1, 5.0), false),
        ] {
            assert_eq!(
                support_intersects_obstacle(&support, &SupportObstacle { min, max }),
                expected
            );
        }
    }

    #[test]
    fn grid_candidates_precede_closer_obstacle_boundaries() {
        let anchor = Vec3::new(20.0, 20.0, 10.0);
        let obstacle = SupportObstacle {
            min: Vec3::new(20.1, 20.1, 0.0),
            max: Vec3::new(20.2, 20.2, 1.0),
        };
        let candidates: Vec<_> = footing_candidates(anchor, std::iter::once(&obstacle)).collect();
        let grid_index = candidates
            .iter()
            .position(|&point| point == (21.0, 20.0))
            .unwrap();
        let boundary_index = candidates
            .iter()
            .position(|&(x, y)| x.fract() != 0.0 || y.fract() != 0.0)
            .unwrap();
        assert!(grid_index < boundary_index);
    }

    #[test]
    fn detects_a_short_intersection_at_a_terrain_tile_corner() {
        let mut world = world();
        world.set_surface(
            1,
            1,
            Surface {
                corners: [10; 4],
                water: None,
            },
        );
        assert!(!brace_clears_terrain(
            &world,
            &[],
            Vec3::new(4.528552533580271, 3.295223549163879, 5.0),
            Vec3::new(2.3427120019267593, 4.616541837062011, 5.0),
        ));
    }

    #[test]
    fn supports_follow_track_and_sloping_terrain() {
        let mut world = world();
        world.set_surface(
            3,
            2,
            Surface {
                corners: [0, 0, 4, 0],
                water: None,
            },
        );
        let elevated = piece()
            .at(TrackState {
                x: 5,
                y: 5,
                z: 5,
                ..TrackState::origin()
            })
            .unwrap();
        let supports = generate_supports(&elevated, &world, &[]).unwrap();
        assert!(!supports.is_empty());
        assert_eq!(supports[0].footing.z, 2.0);
        assert!(
            supports.iter().any(|support| support.footing.z > 1.0),
            "{supports:?}"
        );
        assert!(
            supports
                .iter()
                .all(|support| support.anchor.z == 10.0 && support.footing.z < support.anchor.z)
        );
    }

    #[test]
    fn supports_reroute_around_paths_and_ride_obstacles() {
        let mut world = world();
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 2,
                height: 0,
            },
            slope: None,
        });
        let obstacle = SupportObstacle {
            min: Vec3::new(12.0, 8.0, 0.0),
            max: Vec3::new(16.0, 12.0, 9.0),
        };
        let supports = generate_supports(&piece(), &world, &[obstacle]).unwrap();
        assert!(!supports.is_empty());
        assert!(
            supports
                .iter()
                .any(|support| support.footing.y != support.anchor.y
                    || support.footing.x != support.anchor.x)
        );
    }

    #[test]
    fn support_footers_do_not_overlap_nearby_ride_obstacles() {
        // The column centre is clear, but a 0.4 m half-width footer would overlap.
        let obstacle = SupportObstacle {
            min: Vec3::new(11.0, 7.0, -1.0),
            max: Vec3::new(11.75, 9.0, 1.0),
        };
        let supports = generate_supports(&piece(), &world(), &[obstacle]).unwrap();
        for support in supports {
            let footing = support.footing;
            let overlaps_footer = footing.x + 0.4 >= obstacle.min.x
                && footing.x - 0.4 <= obstacle.max.x
                && footing.y + 0.4 >= obstacle.min.y
                && footing.y - 0.4 <= obstacle.max.y
                && footing.z + 0.2 >= obstacle.min.z
                && footing.z <= obstacle.max.z;
            assert!(!overlaps_footer, "footer overlaps obstacle: {support:?}");
        }
    }

    #[test]
    fn support_columns_do_not_clip_nearby_ride_obstacles() {
        let obstacle = SupportObstacle {
            min: Vec3::new(11.0, 7.0, 4.0),
            max: Vec3::new(11.9, 9.0, 6.0),
        };
        let supports = generate_supports(&piece(), &world(), &[obstacle]).unwrap();
        for support in supports {
            let footing = support.footing;
            assert!(
                footing.x + 0.18 < obstacle.min.x
                    || footing.x - 0.18 > obstacle.max.x
                    || footing.y + 0.18 < obstacle.min.y
                    || footing.y - 0.18 > obstacle.max.y,
                "column clips obstacle: {support:?}"
            );
        }
    }

    #[test]
    fn footer_must_fit_entirely_on_structure() {
        let narrow = SupportObstacle {
            min: Vec3::new(11.8, 7.8, 0.0),
            max: Vec3::new(12.2, 8.2, 4.0),
        };
        let supports = generate_supports_on_structures(&piece(), &world(), &[], &[narrow]).unwrap();
        assert!(supports.iter().all(|support| support.footing.z == 0.0));
    }

    #[test]
    fn support_columns_do_not_clip_adjacent_structure() {
        let structure = SupportObstacle {
            min: Vec3::new(11.0, 7.0, 4.0),
            max: Vec3::new(11.9, 9.0, 6.0),
        };
        let supports =
            generate_supports_on_structures(&piece(), &world(), &[], &[structure]).unwrap();
        assert!(supports.iter().all(|support| {
            let footing = support.footing;
            footing.x + 0.18 < structure.min.x
                || footing.x - 0.18 > structure.max.x
                || footing.y + 0.18 < structure.min.y
                || footing.y - 0.18 > structure.max.y
        }));
    }

    #[test]
    fn underground_beam_must_clear_ceiling_with_its_full_width() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [20; 4],
                water: None,
            },
        );
        world.carve(2, 2, 2, 14);
        let tunnels: Vec<_> = world.carved_spans().collect();
        assert!(!brace_clears_terrain(
            &world,
            &tunnels,
            Vec3::new(9.0, 9.0, 13.9),
            Vec3::new(11.0, 9.0, 13.9)
        ));
        assert!(brace_clears_terrain(
            &world,
            &tunnels,
            Vec3::new(9.0, 9.0, 13.0),
            Vec3::new(11.0, 9.0, 13.0)
        ));
    }

    #[test]
    fn brace_cannot_cross_a_rising_terrain_tile() {
        let mut world = world();
        world.set_surface(
            3,
            2,
            Surface {
                corners: [12; 4],
                water: None,
            },
        );
        assert!(!brace_clears_terrain(
            &world,
            &[],
            Vec3::new(10.0, 9.0, 10.0),
            Vec3::new(18.0, 9.0, 10.0)
        ));
        assert!(brace_clears_terrain(
            &world,
            &[],
            Vec3::new(10.0, 9.0, 13.0),
            Vec3::new(18.0, 9.0, 13.0)
        ));
    }

    #[test]
    fn routes_to_diagonal_footing_when_cardinal_routes_are_blocked() {
        let blockers = [
            SupportObstacle {
                min: Vec3::new(0.0, 7.5, 0.0),
                max: Vec3::new(64.0, 8.5, 9.0),
            },
            SupportObstacle {
                min: Vec3::new(11.5, 0.0, 0.0),
                max: Vec3::new(12.5, 64.0, 9.0),
            },
        ];
        let supports = generate_supports(&piece(), &world(), &blockers).unwrap();
        assert_ne!(supports[0].footing.x, supports[0].anchor.x);
        assert_ne!(supports[0].footing.y, supports[0].anchor.y);
    }

    #[test]
    fn routes_beyond_eight_metres_within_the_bounded_search_radius() {
        let wall = SupportObstacle {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(64.0, 24.0, 9.0),
        };
        let supports = generate_supports(&piece(), &world(), &[wall]).unwrap();
        assert!(
            supports
                .iter()
                .all(|support| support.footing.y > 24.0 + FOOTER_HALF_WIDTH)
        );
    }

    #[test]
    fn routes_through_a_corridor_between_grid_points() {
        let walls = [
            SupportObstacle {
                min: Vec3::new(0.0, 0.0, -1.0),
                max: Vec3::new(64.0, 8.2, 9.0),
            },
            SupportObstacle {
                min: Vec3::new(0.0, 9.1, -1.0),
                max: Vec3::new(64.0, 64.0, 9.0),
            },
        ];
        let supports = generate_supports(&piece(), &world(), &walls).unwrap();
        assert!(
            supports
                .iter()
                .all(|support| support.footing.y - FOOTER_HALF_WIDTH > 8.2
                    && support.footing.y + FOOTER_HALF_WIDTH < 9.1)
        );
    }

    #[test]
    fn reports_no_route_when_obstacle_contains_anchor() {
        let obstacle = SupportObstacle {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(64.0, 64.0, 11.0),
        };
        assert_eq!(
            generate_supports(&piece(), &world(), &[obstacle]),
            Err(SupportError::NoRoute)
        );
    }

    #[test]
    fn reports_no_route_when_structure_intersects_anchor_beam_clearance() {
        let anchor = piece().sample(piece().length() / 4.0).position;
        let structure = SupportObstacle {
            min: Vec3::new(
                anchor.x + BEAM_CLEARANCE / 2.0,
                anchor.y - 1.0,
                anchor.z - 1.0,
            ),
            max: Vec3::new(anchor.x + 1.0, anchor.y + 1.0, anchor.z + 1.0),
        };
        assert_eq!(
            generate_supports_on_structures(&piece(), &world(), &[], &[structure]),
            Err(SupportError::NoRoute)
        );
    }

    #[test]
    fn reports_no_route_after_exhausting_the_bounded_search_domain() {
        let world = world();
        let anchor = piece().sample(piece().length() / 2.0).position;
        let obstruction = SupportObstacle {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(65.0, 65.0, anchor.z - BEAM_CLEARANCE - 0.01),
        };
        assert!(!intersects(
            anchor,
            anchor,
            &expanded(&obstruction, BEAM_CLEARANCE, BEAM_CLEARANCE)
        ));
        assert_eq!(
            generate_supports(&piece(), &world, &[obstruction]),
            Err(SupportError::NoRoute)
        );
    }

    #[test]
    fn reroutes_when_footing_would_cover_a_buried_tunnel() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [5; 4],
                water: None,
            },
        );
        world.carve(3, 2, 1, 4);
        let supports = generate_supports(&piece(), &world, &[]).unwrap();
        assert!(supports.iter().all(|support| !(support.footing.x >= 12.0
            && support.footing.x < 16.0
            && support.footing.y >= 8.0
            && support.footing.y < 12.0)));
    }

    #[test]
    fn footer_does_not_overlap_an_adjacent_buried_tunnel() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [5; 4],
                water: None,
            },
        );
        world.carve(2, 2, 1, 4);
        let supports = generate_supports(&piece(), &world, &[]).unwrap();
        for support in supports {
            let footing = support.footing;
            assert!(
                footing.x + 0.4 <= 8.0
                    || footing.x - 0.4 >= 12.0
                    || footing.y + 0.4 <= 8.0
                    || footing.y - 0.4 >= 12.0,
                "footer covers tunnel: {support:?}"
            );
        }
    }

    #[test]
    fn supports_inside_tunnel_reach_its_floor() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [20; 4],
                water: None,
            },
        );
        for x in 0..16 {
            for y in 0..16 {
                world.carve(x, y, 2, 14);
            }
        }
        let supports = generate_supports(&piece(), &world, &[]).unwrap();
        assert!(supports.iter().all(|support| support.footing.z == 2.0));
    }

    #[test]
    fn underground_brace_cannot_cross_solid_wall() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [20; 4],
                water: None,
            },
        );
        world.carve(2, 2, 2, 14);
        world.carve(4, 2, 2, 14);
        let tunnels: Vec<_> = world.carved_spans().collect();
        assert!(!brace_clears_terrain(
            &world,
            &tunnels,
            Vec3::new(9.0, 9.0, 10.0),
            Vec3::new(17.0, 9.0, 10.0)
        ));
    }

    #[test]
    fn supports_can_land_on_explicit_load_bearing_structure() {
        let platform = SupportObstacle {
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(64.0, 64.0, 4.0),
        };
        let supports =
            generate_supports_on_structures(&piece(), &world(), &[], &[platform]).unwrap();
        assert!(supports.iter().all(|support| support.footing.z == 4.0));
    }

    #[test]
    fn lattice_contains_cross_braces_and_avoids_ride_obstacles() {
        let obstacle = SupportObstacle {
            min: Vec3::new(9.0, 7.0, 0.0),
            max: Vec3::new(10.5, 9.0, 9.0),
        };
        let lattice = generate_wooden_lattice(&piece(), &world(), &[obstacle]).unwrap();
        assert!(
            lattice
                .iter()
                .filter(|segment| {
                    segment.start.z != segment.end.z
                        && (segment.start.x != segment.end.x || segment.start.y != segment.end.y)
                })
                .count()
                >= 2
        );
        let bounds = expanded(&obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE);
        assert!(
            lattice
                .iter()
                .all(|segment| !intersects(segment.start, segment.end, &bounds))
        );
    }

    #[test]
    fn lattice_reroutes_around_path_and_buried_tunnel() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [5; 4],
                water: None,
            },
        );
        world.place_path(Path {
            position: Position {
                x: 3,
                y: 2,
                height: 5,
            },
            slope: None,
        });
        world.carve(2, 2, 1, 4);
        let lattice = generate_wooden_lattice(&piece(), &world, &[]).unwrap();
        let path = SupportObstacle {
            min: Vec3::new(12.0, 8.0, 5.0),
            max: Vec3::new(16.0, 12.0, 7.0),
        };
        let bounds = expanded(&path, BEAM_CLEARANCE, BEAM_CLEARANCE);
        assert!(
            lattice
                .iter()
                .all(|segment| !intersects(segment.start, segment.end, &bounds))
        );
        for segment in lattice
            .iter()
            .filter(|segment| segment.start.z == 5.0 && segment.end.z > 5.0)
        {
            assert!(
                segment.start.x - FOOTER_HALF_WIDTH >= 12.0
                    || segment.start.x + FOOTER_HALF_WIDTH <= 8.0
                    || segment.start.y - FOOTER_HALF_WIDTH >= 12.0
                    || segment.start.y + FOOTER_HALF_WIDTH <= 8.0
            );
        }
    }

    #[test]
    fn lattice_can_be_built_inside_a_wide_tunnel() {
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [20; 4],
                water: None,
            },
        );
        for x in 0..16 {
            for y in 0..16 {
                world.carve(x, y, 2, 14);
            }
        }
        let lattice = generate_wooden_lattice(&piece(), &world, &[]).unwrap();
        assert!(
            lattice
                .iter()
                .any(|segment| segment.start.z == 2.0 && segment.end.z > 2.0)
        );
    }

    #[test]
    fn lattice_retains_safe_single_column_when_no_frame_fits() {
        let obstacles = [
            SupportObstacle {
                min: Vec3::new(9.0, 5.0, 0.0),
                max: Vec3::new(11.0, 11.0, 9.0),
            },
            SupportObstacle {
                min: Vec3::new(13.0, 5.0, 0.0),
                max: Vec3::new(15.0, 11.0, 9.0),
            },
            SupportObstacle {
                min: Vec3::new(11.0, 5.0, 0.0),
                max: Vec3::new(13.0, 7.0, 9.0),
            },
            SupportObstacle {
                min: Vec3::new(11.0, 9.0, 0.0),
                max: Vec3::new(13.0, 11.0, 9.0),
            },
        ];
        let segments = generate_wooden_lattice(&piece(), &world(), &obstacles).unwrap();
        assert_eq!(segments[0].start, Vec3::new(12.0, 8.0, 0.0));
        assert!(
            segments[1..]
                .iter()
                .all(|segment| segment.start.x > 15.0 && segment.end.x > 15.0),
            "only the single column stands at the boxed-in support: {segments:?}"
        );
        assert_eq!(segments[0].start.x, segments[0].end.x);
        assert_eq!(segments[0].start.y, segments[0].end.y);
        assert!(obstacles.iter().all(|obstacle| !intersects(
            segments[0].start,
            segments[0].end,
            &expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE)
        )));
    }

    #[test]
    fn lattice_can_support_every_piece_in_demo_circuit() {
        let world = crate::demo_world::demo_world();
        for piece in crate::physics::demo_circuit().pieces() {
            assert!(
                generate_wooden_lattice(piece, &world, &[]).is_ok(),
                "{:?}",
                piece.id
            );
        }
    }
}
