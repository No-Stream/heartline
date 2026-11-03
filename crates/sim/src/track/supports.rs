use crate::track::{Piece, Vec3};
use crate::world::World;

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

/// A bounded search found no support. This does not prove that no route exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportError {
    SearchExhausted,
}

// The renderer uses 0.045-tile half-width beams and 0.1-tile footer half-widths.
// A beam's square cross section may be rotated relative to the horizontal axes.
const BEAM_CLEARANCE: f64 = 0.18 * std::f64::consts::SQRT_2;
const FOOTER_HALF_WIDTH: f64 = 0.4;
const FOOTER_HEIGHT: f64 = 0.2;

/// Routes supports at intervals along a piece. Coordinates and obstacle bounds are in metres.
/// Paths and excavations are taken from the world; callers supply ride and scenery bounds.
/// Footings are searched on a one-metre grid within eight metres of each anchor,
/// then at obstacle boundary candidates within 32 metres. `SearchExhausted`
/// means the search was inconclusive; callers must not interpret it as proof
/// that a piece cannot be supported.
pub fn generate_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Result<Vec<Support>, SupportError> {
    generate_supports_on_structures(piece, world, obstacles, &[])
}

/// Like `generate_supports`, but allows columns to land on explicitly load-bearing structures.
/// Structure bounds are in metres; ordinary obstacles are never assumed to carry a load.
pub fn generate_supports_on_structures(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    structures: &[SupportObstacle],
) -> Result<Vec<Support>, SupportError> {
    for obstacle in obstacles.iter().chain(structures) {
        assert!(
            obstacle.min.x < obstacle.max.x
                && obstacle.min.y < obstacle.max.y
                && obstacle.min.z < obstacle.max.z,
            "invalid obstacle bounds"
        );
    }
    let path_obstacles: Vec<_> = world
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
    let tunnels: Vec<_> = world.carved_spans().collect();
    let count = (piece.length() / 8.0).ceil() as usize;
    let mut supports = Vec::with_capacity(count);
    for index in 0..count {
        let anchor = piece
            .sample(piece.length() * (index as f64 + 0.5) / count as f64)
            .position;
        let mut selected = None;
        let mut offsets: Vec<_> = (-8_i32..=8)
            .flat_map(|dx| (-8_i32..=8).map(move |dy| (dx, dy)))
            .collect();
        offsets.sort_by_key(|(dx, dy)| (dx * dx + dy * dy, *dx, *dy));
        let candidates = offsets
            .into_iter()
            .map(|(dx, dy)| (anchor.x + f64::from(dx), anchor.y + f64::from(dy)))
            .chain(
                std::iter::once_with(|| {
                    boundary_candidates(
                        anchor,
                        obstacles.iter().chain(&path_obstacles).chain(structures),
                    )
                })
                .flatten(),
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
            if obstacles.iter().chain(&path_obstacles).any(|obstacle| {
                let beam_bounds = expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE);
                let footer_bounds = expanded(obstacle, FOOTER_HALF_WIDTH, 0.0);
                intersects(footing, top, &beam_bounds)
                    || intersects(top, anchor, &beam_bounds)
                    || intersects(
                        footing,
                        footing + Vec3::new(0.0, 0.0, FOOTER_HEIGHT),
                        &footer_bounds,
                    )
            }) || structures.iter().enumerate().any(|(index, structure)| {
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
            }) {
                continue;
            }
            selected = Some(Support { anchor, footing });
            break;
        }
        supports.push(selected.ok_or(SupportError::SearchExhausted)?);
    }
    Ok(supports)
}

fn boundary_candidates<'a>(
    anchor: Vec3,
    obstacles: impl Iterator<Item = &'a SupportObstacle>,
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
        for boundary in [
            obstacle.min.x - FOOTER_HALF_WIDTH - 0.001,
            obstacle.max.x + FOOTER_HALF_WIDTH + 0.001,
        ] {
            if (boundary - anchor.x).abs() <= 32.0 {
                candidate_x.push(boundary);
            }
        }
        for boundary in [
            obstacle.min.y - FOOTER_HALF_WIDTH - 0.001,
            obstacle.max.y + FOOTER_HALF_WIDTH + 0.001,
        ] {
            if (boundary - anchor.y).abs() <= 32.0 {
                candidate_y.push(boundary);
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
    let supports = generate_supports(piece, world, obstacles)?;
    let mut occupied = obstacles.to_vec();
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
    let mut result = Vec::new();
    for support in supports {
        let top = Vec3::new(support.footing.x, support.footing.y, support.anchor.z);
        let mut frame = None;
        for (dx, dy) in [
            (2.0, 0.0),
            (-2.0, 0.0),
            (0.0, 2.0),
            (0.0, -2.0),
            (2.0, 2.0),
            (-2.0, -2.0),
            (2.0, -2.0),
            (-2.0, 2.0),
        ] {
            let x = support.footing.x + dx;
            let y = support.footing.y + dy;
            let (width, height) = world.dimensions();
            if x < 0.4
                || y < 0.4
                || x + 0.4 >= f64::from(width) * 4.0
                || y + 0.4 >= f64::from(height) * 4.0
            {
                continue;
            }
            let terrain = terrain_height(world, x, y);
            let ground = if terrain < support.anchor.z {
                if tunnels.iter().any(|&(tx, ty, _, _)| {
                    let west = f64::from(tx) * 4.0;
                    let north = f64::from(ty) * 4.0;
                    x + FOOTER_HALF_WIDTH > west
                        && x - FOOTER_HALF_WIDTH < west + 4.0
                        && y + FOOTER_HALF_WIDTH > north
                        && y - FOOTER_HALF_WIDTH < north + 4.0
                }) {
                    continue;
                }
                terrain
            } else {
                let tile_x = (x / 4.0).floor() as u16;
                let tile_y = (y / 4.0).floor() as u16;
                let Some((_, _, bottom, _)) = tunnels.iter().find(|&&(tx, ty, bottom, top)| {
                    tx == tile_x
                        && ty == tile_y
                        && f64::from(bottom) < support.anchor.z
                        && f64::from(top) > support.anchor.z
                }) else {
                    continue;
                };
                let ground = f64::from(*bottom);
                if [-FOOTER_HALF_WIDTH, FOOTER_HALF_WIDTH]
                    .into_iter()
                    .any(|dx| {
                        [-FOOTER_HALF_WIDTH, FOOTER_HALF_WIDTH]
                            .into_iter()
                            .any(|dy| {
                                !tunnels.iter().any(|&(tx, ty, bottom, top)| {
                                    tx == ((x + dx) / 4.0).floor() as u16
                                        && ty == ((y + dy) / 4.0).floor() as u16
                                        && f64::from(bottom) <= ground
                                        && f64::from(top) > support.anchor.z
                                })
                            })
                    })
                {
                    continue;
                }
                ground
            };
            let other = Vec3::new(x, y, ground);
            if top.z - support.footing.z < 0.7 || top.z - other.z < 0.7 {
                continue;
            }
            let other_top = Vec3::new(x, y, top.z);
            let first_bottom = support.footing + Vec3::new(0.0, 0.0, 0.3);
            let other_bottom = other + Vec3::new(0.0, 0.0, 0.3);
            let first_upper = top - Vec3::new(0.0, 0.0, 0.3);
            let other_upper = other_top - Vec3::new(0.0, 0.0, 0.3);
            let candidate = [
                SupportSegment {
                    start: support.footing,
                    end: top,
                },
                SupportSegment {
                    start: other,
                    end: other_top,
                },
                SupportSegment {
                    start: first_bottom,
                    end: other_upper,
                },
                SupportSegment {
                    start: other_bottom,
                    end: first_upper,
                },
                SupportSegment {
                    start: top,
                    end: other_top,
                },
            ];
            if candidate.iter().any(|segment| {
                occupied.iter().any(|obstacle| {
                    intersects(
                        segment.start,
                        segment.end,
                        &expanded(obstacle, BEAM_CLEARANCE, BEAM_CLEARANCE),
                    )
                }) || ((segment.start.x != segment.end.x || segment.start.y != segment.end.y)
                    && !brace_clears_terrain(world, &tunnels, segment.start, segment.end))
            }) || occupied.iter().any(|obstacle| {
                intersects(
                    other,
                    other + Vec3::new(0.0, 0.0, FOOTER_HEIGHT),
                    &expanded(obstacle, FOOTER_HALF_WIDTH, 0.0),
                )
            }) {
                continue;
            }
            frame = Some(candidate);
            break;
        }
        if let Some(frame) = frame {
            result.extend(frame);
        } else {
            result.push(SupportSegment {
                start: support.footing,
                end: top,
            });
        }
        if top != support.anchor {
            result.push(SupportSegment {
                start: top,
                end: support.anchor,
            });
        }
    }
    Ok(result)
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
    use crate::track::{PieceId, TrackState, catalog};
    use crate::world::{Path, Position, Surface};

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
    fn routes_beyond_eight_metres_when_a_nearby_wall_blocks_footings() {
        let wall = SupportObstacle {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(64.0, 17.0, 9.0),
        };
        let supports = generate_supports(&piece(), &world(), &[wall]).unwrap();
        assert!(
            supports
                .iter()
                .all(|support| support.footing.y > 17.0 + FOOTER_HALF_WIDTH)
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
    fn refuses_when_every_support_route_is_blocked() {
        let obstacle = SupportObstacle {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(64.0, 64.0, 11.0),
        };
        assert_eq!(
            generate_supports(&piece(), &world(), &[obstacle]),
            Err(SupportError::SearchExhausted)
        );
    }

    #[test]
    fn exhausted_search_does_not_prove_no_route_exists() {
        let wall = SupportObstacle {
            min: Vec3::new(0.0, 0.0, -1.0),
            max: Vec3::new(64.0, 50.0, 9.0),
        };
        let anchor = piece().sample(piece().length() / 2.0).position;
        let footing = Vec3::new(anchor.x, 51.0, 0.0);
        let top = Vec3::new(footing.x, footing.y, anchor.z);
        assert!(!intersects(
            footing,
            top,
            &expanded(&wall, FOOTER_HALF_WIDTH, 0.0)
        ));
        assert!(!intersects(
            top,
            anchor,
            &expanded(&wall, BEAM_CLEARANCE, BEAM_CLEARANCE)
        ));
        assert_eq!(
            generate_supports(&piece(), &world(), &[wall]),
            Err(SupportError::SearchExhausted)
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
        assert_eq!(segments.len(), 6, "{segments:?}");
        assert_eq!(segments[0].start, Vec3::new(12.0, 8.0, 0.0));
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
