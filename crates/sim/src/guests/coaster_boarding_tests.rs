//! A guest boarding check for coaster height limits, plus the closed test coaster and station paths that
//! the park determinism tests share.

use super::*;
use crate::facilities::{coaster_access_openings, coaster_facility};
use crate::guests::behavior::GuestBehavior;
use crate::guests::generate_guests;
use crate::park::tick::COASTER_PLACE_ID;
use crate::track::construction::Construction;
use crate::track::{CoasterModel, PieceId, TrackState, family_coaster_model, hybrid_model};
use crate::world::{PathKind, Surface};

pub(super) fn construction_for(world: &World, model: CoasterModel) -> Construction {
    let start = TrackState {
        x: 256,
        y: 256,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model);
    for (index, id) in [
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ]
    .into_iter()
    .enumerate()
    {
        construction.set_chain(matches!(index, 3 | 10));
        construction
            .append(id, world, &[])
            .unwrap_or_else(|error| panic!("could not append {id:?}: {error:?}"));
    }
    construction.close().unwrap();
    construction
}

/// Lays a one-tile queue and exit at the coaster's station and returns the path tile guests enter the queue from.
pub(super) fn open_station_paths(world: &mut World, construction: &Construction) -> Position {
    let [queue, exit] = coaster_access_openings(construction).unwrap();
    let queue_entry = [
        Position {
            x: queue.position.x.saturating_sub(1),
            ..queue.position
        },
        Position {
            x: queue.position.x.saturating_add(1),
            ..queue.position
        },
        Position {
            y: queue.position.y.saturating_sub(1),
            ..queue.position
        },
        Position {
            y: queue.position.y.saturating_add(1),
            ..queue.position
        },
    ]
    .into_iter()
    .find(|position| *position != exit.position)
    .unwrap();
    for position in [queue.position, exit.position, queue_entry] {
        assert!(
            world
                .set_surface(
                    position.x,
                    position.y,
                    Surface {
                        corners: [position.height; 4],
                        water: None,
                    },
                )
                .is_empty()
        );
    }
    world
        .build_path_route(queue_entry, queue_entry, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(exit.position, exit.position, PathKind::Normal, 0)
        .unwrap();
    world
        .build_path_route(queue_entry, queue.position, PathKind::Queue, 0)
        .unwrap();
    queue_entry
}

#[test]
fn children_from_100_cm_choose_the_family_coaster_but_not_the_hybrid() {
    let guests = generate_guests(42, 2_000);
    let child = guests
        .ids()
        .find(|&guest| (100..122).contains(&guests.height_cm(guest)))
        .expect("the generated roster contains a child between 100 and 121 cm");
    for (model, child_may_ride) in [(family_coaster_model(), true), (hybrid_model(), false)] {
        let mut world = World::new(
            512,
            512,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let construction = construction_for(&world, model.clone());
        open_station_paths(&mut world, &construction);
        let facility =
            coaster_facility(&world, &construction, COASTER_PLACE_ID, 0, Some((20, 10))).unwrap();
        assert_eq!(
            u16::from(facility.place.minimum_height_cm),
            model.minimum_height_cm
        );
        let mut behavior = GuestBehavior::default();
        behavior.observe(facility.place);
        assert_eq!(
            behavior.choose(&guests, child).is_some(),
            child_may_ride,
            "{}",
            model.name
        );
    }
}
