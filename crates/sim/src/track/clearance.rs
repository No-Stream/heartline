//! Conservative sampled clearance envelopes in metres.
use crate::track::{Piece, TrainConfiguration, Vec3};
use crate::world::{Ground, World};

const SAMPLE_SPACING: f64 = 0.5;
const TRACK_RADIUS: f64 = 0.15;

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
    a.min.x < b.max.x
        && a.max.x > b.min.x
        && a.min.y < b.max.y
        && a.max.y > b.min.y
        && a.min.z < b.max.z
        && a.max.z > b.min.z
}

fn envelope(piece: &Piece, distance: f64, train: &TrainConfiguration) -> ClearanceBox {
    let sample = piece.sample(distance);
    let left = sample.up.cross(sample.tangent);
    let centre = sample.position + sample.up * (train.height_metres / 2.0);
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + SAMPLE_SPACING;
    let half_width = train.width_metres / 2.0 + SAMPLE_SPACING;
    let half_height = train.height_metres / 2.0 + SAMPLE_SPACING;
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
    ClearanceBox {
        min: centre - extent,
        max: centre + extent,
    }
}

fn samples(piece: &Piece) -> impl Iterator<Item = f64> + '_ {
    let steps = (piece.length() / SAMPLE_SPACING).ceil() as usize;
    (0..=steps).map(move |index| piece.length() * index as f64 / steps as f64)
}

/// Checks a piece against the world and supplied track and scenery.
/// Connected track is ignored only in the immediate vicinity of a common connector.
/// The envelope is a conservative axis-aligned bound of sampled oriented train cars.
pub fn check_clearance(
    world: &World,
    piece: &Piece,
    train: &TrainConfiguration,
    existing_track: &[Piece],
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
    let half_length = train.length_metres / f64::from(train.cars) / 2.0 + SAMPLE_SPACING;
    let half_width = train.width_metres / 2.0 + SAMPLE_SPACING;
    let envelope_height = train.height_metres + SAMPLE_SPACING;
    // The train envelope extends forward and sideways from the rail, and upward
    // by its full height. Include the sampled track box at the other end.
    let connector_allowance =
        (half_length * half_length + half_width * half_width + envelope_height * envelope_height)
            .sqrt()
            + (TRACK_RADIUS + SAMPLE_SPACING / 2.0) * 3.0_f64.sqrt();
    for distance in samples(piece) {
        let bounds = envelope(piece, distance, train);
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
                let max_height = bounds.max.z.ceil().min(f64::from(i16::MAX)) as i16;
                for z in min_height..max_height {
                    if matches!(world.ground(x, y, z), Ground::Solid | Ground::SurfaceSlope) {
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
            if intersects(bounds, path_bounds) {
                return Err(ClearanceCollision::Path);
            }
        }
        if scenery.iter().any(|obstacle| intersects(bounds, *obstacle)) {
            return Err(ClearanceCollision::Scenery);
        }
        for other in existing_track {
            for other_distance in samples(other) {
                let near_entry = piece.entry == other.exit
                    && distance + (other.length() - other_distance) <= connector_allowance;
                let near_exit = piece.exit == other.entry
                    && (piece.length() - distance) + other_distance <= connector_allowance;
                if near_entry || near_exit {
                    continue;
                }
                let position = other.sample(other_distance).position;
                let radius = TRACK_RADIUS + SAMPLE_SPACING / 2.0;
                let extent = Vec3::new(radius, radius, radius);
                if intersects(
                    bounds,
                    ClearanceBox {
                        min: position - extent,
                        max: position + extent,
                    },
                ) {
                    return Err(ClearanceCollision::Track);
                }
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
