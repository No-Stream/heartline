//! Checks that the construction caches return what a construction without caches computes.

use super::*;
use crate::track::{Heading, hybrid_model};
use crate::world::{Path, Position, Scenery, SceneryKind, Surface};
use std::collections::{BTreeSet, HashSet};

fn flat_world() -> World {
    World::new(
        128,
        128,
        Surface {
            corners: [8; 4],
            water: None,
        },
    )
}

fn station_construction() -> Construction {
    let mut construction = Construction::new(
        TrackState {
            x: 120,
            y: 200,
            z: 10,
            heading: Heading::North,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    construction
        .append(PieceId::Station, &flat_world(), &[])
        .unwrap();
    construction
}

#[test]
fn choices_after_hill_variant_changes_match_a_fresh_computation() {
    let world = flat_world();
    let mut construction = station_construction();
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    construction.choices(&world, &[]);
    let hill_ids: Vec<_> = templates()
        .iter()
        .filter(|piece| matches!(piece.family, Some(PieceFamily::Hill(_))))
        .map(|piece| piece.id)
        .take(4)
        .collect();
    assert_eq!(hill_ids.len(), 4);
    for id in hill_ids {
        construction.select_hill_variant(id, &world, &[]).unwrap();
        let fresh = construction.copy_for_completion();
        assert_eq!(
            construction.choices(&world, &[]),
            fresh.choices(&world, &[])
        );
        let ids = |choices: Vec<(Piece, PieceChoice)>| -> Vec<_> {
            choices
                .into_iter()
                .map(|(piece, choice)| (piece.id, choice))
                .collect()
        };
        assert_eq!(
            ids(construction.available_choices(&world, &[])),
            ids(fresh.available_choices(&world, &[]))
        );
    }
}

#[test]
fn existing_supports_are_shared_across_candidate_validations() {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let mut construction = station_construction();
    for _ in 0..12 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    construction.choices(&world, &[]);

    let validation_cache = construction.validation_cache.borrow();
    let baseline_supports = validation_cache.as_ref().unwrap().supports[0]
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    let choices_cache = construction.choices_cache.borrow();
    let first_piece_region = support_search_region(&construction.pieces[0]);
    let unaffected_candidates: Vec<_> = choices_cache
        .as_ref()
        .unwrap()
        .available_choices
        .iter()
        .filter(|candidate| candidate.choice.result.is_ok())
        .filter_map(|candidate| {
            let validation = candidate.validation.as_ref()?;
            (!validation
                .track_obstacles
                .iter()
                .any(|bounds| first_piece_region.overlaps(bounds)))
            .then_some(candidate)
        })
        .collect();
    let pair = unaffected_candidates
        .iter()
        .enumerate()
        .find_map(|(first_index, first)| {
            unaffected_candidates
                .iter()
                .skip(first_index + 1)
                .find(|second| !same_geometry(&first.piece, &second.piece))
                .map(|second| (*first, *second))
        })
        .expect("two valid candidate geometries outside the first piece's support region");
    let (first, second) = pair;
    let first_supports = &first.validation.as_ref().unwrap().supports[0];
    let second_supports = &second.validation.as_ref().unwrap().supports[0];
    assert!(Arc::ptr_eq(first_supports, baseline_supports));
    assert!(Arc::ptr_eq(first_supports, second_supports));

    for candidate in [first, second] {
        let cached_validation = candidate.validation.as_ref().unwrap();
        let fresh_construction = construction.copy_for_completion();
        let fresh_validation = fresh_construction
            .validate_piece(&candidate.piece, &world, &[])
            .unwrap();
        let support_values = |validation: &PieceValidation| {
            validation
                .supports
                .iter()
                .map(|supports| supports.as_ref().clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            support_values(cached_validation),
            support_values(&fresh_validation)
        );
    }
}

#[test]
fn support_world_obstacles_are_reused_and_rebuilt_after_world_changes() {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    world.place_path(Path {
        position: Position {
            x: 100,
            y: 100,
            height: 0,
        },
        slope: None,
    });
    world
        .place_scenery(Scenery {
            kind: SceneryKind::BushFlowering,
            position: Position {
                x: 102,
                y: 100,
                height: 0,
            },
            quarter: 0,
            rotation: 0,
        })
        .unwrap();
    crate::track::supports::reset_support_world_obstacle_snapshot_builds();

    let mut construction = Construction::new(
        TrackState {
            x: 120,
            y: 200,
            z: 10,
            heading: Heading::North,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    construction.append(PieceId::Station, &world, &[]).unwrap();
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    construction.choices(&world, &[]);
    assert_eq!(
        crate::track::supports::support_world_obstacle_snapshot_builds(),
        1,
        "one world snapshot should serve all choice validations"
    );

    world.place_path(Path {
        position: Position {
            x: 200,
            y: 200,
            height: 0,
        },
        slope: None,
    });
    construction.choices(&world, &[]);
    assert_eq!(
        crate::track::supports::support_world_obstacle_snapshot_builds(),
        2,
        "a world revision should rebuild the support obstacle snapshot once"
    );
}

fn hill_world() -> World {
    World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    )
}

/// Routes every piece on the world carved for the existing track and the candidate, as
/// validation did before it reused supports the candidate cannot reach.
fn supports_routed_from_scratch(
    construction: &Construction,
    candidate: &Piece,
    world: &World,
) -> Result<Vec<Vec<Support>>, ConstructionError> {
    let train = &construction.model.train;
    let all_pieces: Vec<_> = construction
        .pieces
        .iter()
        .chain(std::iter::once(candidate))
        .collect();
    let mut carve_cells = BTreeSet::new();
    for piece in &all_pieces {
        carve_cells.extend(
            tunnel_plan(world, piece, train)
                .map_err(ConstructionError::Clearance)?
                .carve_cells,
        );
    }
    let carved = carved_world(world, &carve_cells)?;
    let bounds: Vec<_> = all_pieces
        .iter()
        .map(|piece| track_support_obstacles(std::slice::from_ref(*piece), train))
        .collect();
    let world_obstacles = SupportWorldObstacles::new(world);
    all_pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| {
            let region = support_search_region(piece);
            let obstacles: Vec<_> = bounds
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != index)
                .flat_map(|(_, bounds)| bounds.iter().copied())
                .filter(|bounds| region.overlaps(bounds))
                .collect();
            let route_supports = if construction.model.support_style_for_piece(piece)
                == SupportStyle::DenseWoodenLattice
            {
                generate_dense_supports_with_world_obstacles
            } else {
                generate_supports_with_world_obstacles
            };
            route_supports(piece, &carved, &obstacles, &world_obstacles)
                .map_err(construction_support_error)
        })
        .collect()
}

/// Builds a track that dips into a hill and back out, checking every tunnelling candidate's
/// validated supports against routing from scratch. With `carve_placed_track`, the world is
/// carved after each placement as the game does; otherwise the track's tunnels stay uncarved.
fn assert_tunnelling_supports_match_routing_from_scratch(carve_placed_track: bool) {
    let mut world = hill_world();
    let mut construction = Construction::new(
        TrackState {
            x: 50,
            y: 50,
            z: 11,
            ..TrackState::origin()
        },
        hybrid_model(),
    );
    let route = [
        PieceId::Station,
        PieceId::LevelToGentleDown,
        PieceId::GentleDownToLevel,
        PieceId::Straight,
        PieceId::TurnLargeStandalone,
        PieceId::Straight,
        PieceId::LevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::Straight,
    ];
    let mut compared = 0;
    for id in route {
        let mut seen_ids = HashSet::new();
        for template in templates().iter().filter(|piece| seen_ids.insert(piece.id)) {
            let Some(candidate) = construction.candidate_piece(template.id) else {
                continue;
            };
            let validation = construction.validate_piece(&candidate, &world, &[]);
            let tunnelled = construction.pieces.iter().chain([&candidate]).any(|piece| {
                tunnel_plan(&world, piece, &construction.model.train)
                    .is_ok_and(|plan| plan.would_tunnel())
            });
            if !tunnelled {
                continue;
            }
            match validation {
                Ok(validation) => {
                    assert_eq!(
                        Ok(validation
                            .supports
                            .iter()
                            .map(|supports| supports.as_ref().clone())
                            .collect::<Vec<_>>()),
                        supports_routed_from_scratch(&construction, &candidate, &world),
                        "{:?} after {} pieces",
                        template.id,
                        construction.pieces.len()
                    );
                    compared += 1;
                }
                Err(ConstructionError::NoSupportRoute) => {
                    assert_eq!(
                        Err(ConstructionError::NoSupportRoute),
                        supports_routed_from_scratch(&construction, &candidate, &world)
                    );
                    compared += 1;
                }
                Err(_) => {}
            }
        }
        let plan = construction.tunnel_plan(id, &world, &[]).unwrap();
        if carve_placed_track && plan.would_tunnel() {
            let cells: Vec<_> = plan
                .carve_cells
                .iter()
                .map(|cell| (cell.x, cell.y, cell.height))
                .collect();
            world.carve_cells(&cells).unwrap();
        }
        construction.append(id, &world, &[]).unwrap();
    }
    assert!(
        compared > 100,
        "only {compared} tunnelling candidates compared"
    );
}

#[test]
fn tunnelling_supports_match_routing_from_scratch_on_an_uncarved_world() {
    assert_tunnelling_supports_match_routing_from_scratch(false);
}

#[test]
fn tunnelling_supports_match_routing_from_scratch_as_the_game_carves() {
    assert_tunnelling_supports_match_routing_from_scratch(true);
}

#[test]
fn support_regions_reach_terrain_tiles_they_touch() {
    let region = SupportSearchRegion {
        min_x: 10.0,
        max_x: 20.0,
        min_y: 30.0,
        max_y: 40.0,
    };
    let tile = |x, y| TerrainCell { x, y, height: 0 };
    assert!(
        region.overlaps_tile(&tile(2, 7)),
        "tile spanning 8-12 m by 28-32 m overlaps"
    );
    assert!(
        region.overlaps_tile(&tile(5, 10)),
        "tile starting at the far corner touches"
    );
    assert!(
        !region.overlaps_tile(&tile(2, 6)),
        "tile ending 2 m short in y"
    );
    assert!(
        !region.overlaps_tile(&tile(6, 8)),
        "tile starting 4 m past in x"
    );
}
