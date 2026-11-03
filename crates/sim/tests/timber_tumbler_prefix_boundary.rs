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
    prefix: &[PieceId],
) -> Construction {
    let start = TrackState {
        x,
        y,
        z,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    construction.append(PieceId::Station, world, &[]).unwrap();
    for piece_id in prefix {
        construction.append(*piece_id, world, &[]).unwrap();
    }
    construction
}

#[test]
fn station_backed_prefix_refusals_are_world_bounds_at_four_and_twelve_metres() {
    let world = flat_world();
    let prefix = [
        PieceId::LiftFlat,
        PieceId::LiftFlat,
        PieceId::LiftFlat,
        PieceId::LiftFlat,
        PieceId::LiftFlat,
        PieceId::LiftFlat,
        PieceId::LiftLevelToGentleUp,
        PieceId::LiftGentleUpToLevel,
        PieceId::LiftFlat,
        PieceId::Corkscrew,
        PieceId::LiftLevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::LiftLevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::Corkscrew,
        PieceId::LiftLevelToGentleUp,
        PieceId::GentleUp,
        PieceId::LiftGentleUpToLevel,
        PieceId::LiftLevelToGentleUp,
        PieceId::LiftGentleUpToLevel,
        PieceId::TurnLargeRightStandalone,
        PieceId::AirtimeHill,
        PieceId::HeartlineRollRight,
    ];

    for z in [2, 6] {
        let at_recorded_start = construction_with_prefix(&world, 50, 50, z, &prefix);
        assert_eq!(
            (at_recorded_start.end().x, at_recorded_start.end().y),
            (250, 2)
        );
        let edge_choices = at_recorded_start.available_choices(&world, &[]);
        assert!(!edge_choices.is_empty());
        assert!(edge_choices.iter().all(|(_, choice)| {
            choice.result
                == Err(ConstructionError::Clearance(
                    ClearanceCollision::OutsideWorld,
                ))
        }));

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
fn a_random_walk_can_trap_its_endpoint_against_existing_track() {
    let world = flat_world();
    let prefix = [
        PieceId::HeartlineRoll,
        PieceId::Straight,
        PieceId::TurnSmallStandalone,
        PieceId::HeartlineRoll,
        PieceId::TurnLargeRightEntry,
        PieceId::TurnLargeRight,
        PieceId::TurnLargeRightExit,
        PieceId::OffsetLeft,
    ];
    let construction = construction_with_prefix(&world, 50, 50, 2, &prefix);
    let choices = construction.available_choices(&world, &[]);
    assert!(!choices.is_empty());
    assert!(choices.iter().all(|(_, choice)| {
        choice.result == Err(ConstructionError::Clearance(ClearanceCollision::Track))
    }));
}
