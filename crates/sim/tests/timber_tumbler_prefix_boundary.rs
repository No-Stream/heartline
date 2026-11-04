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
        let mut at_recorded_start = construction_with_prefix(&world, 50, 50, z, &prefix);
        assert_eq!(
            (at_recorded_start.end().x, at_recorded_start.end().y),
            (250, 2)
        );
        at_recorded_start
            .append(PieceId::HalfTileStraight, &world, &[])
            .unwrap();
        let edge_choices = at_recorded_start.available_choices(&world, &[]);
        assert!(!edge_choices.is_empty());
        assert!(
            edge_choices.iter().all(|(_, choice)| {
                choice.result
                    == Err(ConstructionError::Clearance(
                        ClearanceCollision::OutsideWorld,
                    ))
            }),
            "unexpected edge choices: {:?}",
            edge_choices
                .iter()
                .filter(|(_, choice)| choice.result
                    != Err(ConstructionError::Clearance(
                        ClearanceCollision::OutsideWorld
                    )))
                .map(|(_, choice)| choice)
                .collect::<Vec<_>>()
        );

        let mut translated_inward = construction_with_prefix(&world, 16, 64, z, &prefix);
        assert_eq!(
            (translated_inward.end().x, translated_inward.end().y),
            (216, 16)
        );
        translated_inward
            .append(PieceId::HalfTileStraight, &world, &[])
            .unwrap();
        assert!(
            translated_inward
                .available_choices(&world, &[])
                .iter()
                .any(|(_, choice)| choice.result.is_ok())
        );
    }
}

#[test]
fn a_random_walk_can_trap_its_endpoint_against_existing_track() {
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
    let mut construction = construction_with_prefix(&world, 50, 50, 2, &prefix);
    construction
        .append(PieceId::OneTileStraight, &world, &[])
        .unwrap();
    let choices = construction.available_choices(&world, &[]);
    assert!(!choices.is_empty());
    assert!(
        choices.iter().all(|(_, choice)| {
            choice.result == Err(ConstructionError::Clearance(ClearanceCollision::Track))
        }),
        "unexpected choices: {:?}",
        choices
            .iter()
            .filter(|(_, choice)| choice.result
                != Err(ConstructionError::Clearance(ClearanceCollision::Track)))
            .map(|(_, choice)| choice)
            .collect::<Vec<_>>()
    );
}
