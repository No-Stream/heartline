use theme_park_sim::track::clearance::ClearanceCollision;
use theme_park_sim::track::construction::{Construction, ConstructionError};
use theme_park_sim::track::{PieceId, TrackState, hybrid_model};
use theme_park_sim::world::{Surface, World};

fn flat_world() -> World {
    World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    )
}

fn construction_with_prefix(
    world: &World,
    x: i32,
    y: i32,
    z: i32,
    prefix: &[(PieceId, bool)],
) -> Construction {
    let start = TrackState {
        x,
        y,
        z,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    construction.append(PieceId::Station, world, &[]).unwrap();
    for &(piece_id, chain) in prefix {
        construction.set_chain(chain);
        construction.append(piece_id, world, &[]).unwrap();
    }
    construction.set_chain(false);
    construction
}

#[test]
fn station_backed_prefix_refusals_are_world_bounds_at_four_and_twelve_metres() {
    let world = flat_world();
    let prefix = [
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::Straight, true),
        (PieceId::LevelToGentleUp, true),
        (PieceId::GentleUpToLevel, true),
        (PieceId::Straight, true),
        (PieceId::Corkscrew, false),
        (PieceId::LevelToGentleUp, true),
        (PieceId::GentleUpToLevel, false),
        (PieceId::LevelToGentleUp, true),
        (PieceId::GentleUpToLevel, false),
        (PieceId::Corkscrew, false),
        (PieceId::LevelToGentleUp, true),
        (PieceId::GentleUp, false),
        (PieceId::GentleUpToLevel, true),
        (PieceId::LevelToGentleUp, true),
        (PieceId::GentleUpToLevel, true),
        (PieceId::TurnLargeRightStandalone, false),
        (PieceId::AirtimeHill, false),
        (PieceId::HeartlineRollRight, false),
    ];

    for z in [2, 6] {
        let at_recorded_start = construction_with_prefix(&world, 50, 50, z, &prefix);
        assert_eq!(
            (at_recorded_start.end().x, at_recorded_start.end().y),
            (250, 2)
        );
        // Half a tile from the map edge, only the half-tile straight still fits; everything else leaves the world.
        let edge_choices = at_recorded_start.available_choices(&world, &[]);
        assert!(!edge_choices.is_empty());
        for (piece, choice) in &edge_choices {
            if piece.id == PieceId::HalfTileStraight {
                assert!(
                    choice.result.is_ok(),
                    "the half-tile straight should fit before the map edge"
                );
            } else {
                assert_eq!(
                    choice.result,
                    Err(ConstructionError::Clearance(
                        ClearanceCollision::OutsideWorld
                    )),
                    "{:?}",
                    piece.id
                );
            }
        }

        let translated_inward = construction_with_prefix(&world, 16, 64, z, &prefix);
        assert_eq!(
            (translated_inward.end().x, translated_inward.end().y),
            (216, 16)
        );
        assert!(
            translated_inward
                .available_choices(&world, &[])
                .iter()
                .any(|(_, choice)| choice.result.is_ok())
        );
    }
}

#[test]
fn only_short_straights_fit_between_a_walk_endpoint_and_existing_track() {
    let world = flat_world();
    let prefix = [
        (PieceId::HeartlineRoll, false),
        (PieceId::Straight, false),
        (PieceId::TurnSmallStandalone, false),
        (PieceId::HeartlineRoll, false),
        (PieceId::TurnLargeRightEntry, false),
        (PieceId::TurnLargeRight, false),
        (PieceId::TurnLargeRightExit, false),
        (PieceId::OffsetLeft, false),
    ];
    let construction = construction_with_prefix(&world, 50, 50, 2, &prefix);
    // Only the short straights fit before the existing track; every other piece collides with it.
    let choices = construction.available_choices(&world, &[]);
    assert!(!choices.is_empty());
    for (piece, choice) in &choices {
        if matches!(
            piece.id,
            PieceId::HalfTileStraight | PieceId::OneTileStraight
        ) {
            assert!(
                choice.result.is_ok(),
                "{:?} should fit before the existing track",
                piece.id
            );
        } else {
            assert_eq!(
                choice.result,
                Err(ConstructionError::Clearance(ClearanceCollision::Track)),
                "{:?}",
                piece.id
            );
        }
    }
}
