//! Checks that the construction caches return what a construction without caches computes.

use super::*;
use crate::track::{Heading, hybrid_model};
use crate::world::Surface;

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
