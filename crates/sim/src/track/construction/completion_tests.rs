use super::*;
use crate::physics::{ConstructionTestRunRecorder, PhysicsConfig, Train};
use crate::track::{PieceId, PieceKind, TrackState, hybrid_model, wooden_twister_model};
use crate::world::Surface;

#[test]
fn completion_uses_unpowered_connectors_and_preserves_chain_mode() {
    use PieceId::*;

    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 50,
        y: 50,
        z: 5,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let original_length = construction.pieces().len();
    construction.set_chain(true);

    let completion = construction
        .find_completion(&world, &[])
        .expect("nearby connection with chain mode enabled");

    assert!(!completion.geometry.is_empty());
    assert!(
        completion
            .geometry
            .iter()
            .all(|piece| piece.kind == PieceKind::Track)
    );
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();

    assert!(construction.is_closed());
    assert!(construction.chain());
    assert!(
        construction.pieces()[original_length..]
            .iter()
            .all(|piece| piece.kind == PieceKind::Track)
    );
}

#[test]
fn completion_finds_a_four_piece_southeast_offset_connection() {
    use PieceId::*;

    let world = World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 128,
        y: 127,
        z: 30,
        heading: crate::track::Heading::SouthEast,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, wooden_twister_model());
    for id in [
        Station,
        Straight,
        Straight,
        Straight,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        FullOffsetLeft,
        FullOffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }

    let completion = construction
        .find_completion(&world, &[])
        .expect("two-tile offset connection");

    assert_eq!(completion.pieces.len(), 4);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn constructed_reference_laps_report_ratings() {
    use PieceId::*;
    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 50,
        y: 50,
        z: 36,
        ..TrackState::origin()
    };
    let model = hybrid_model();
    let mut construction = Construction::new(start, model.clone());
    construction.append(Station, &world, &[]).unwrap();
    construction.set_chain(true);
    for id in [
        LevelToGentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUpToLevel,
        Straight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    construction.set_chain(false);
    for id in [
        OneTileStraight,
        OneTileStraight,
        OffsetLeft,
        TurnLargeEntry,
        TurnLargeExit,
        LevelToGentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDownToLevel,
        Loop,
        OffsetRight,
        TurnLargeEntry,
        TurnLargeExit,
        HalfTileStraight,
        OneTileStraight,
        OneTileStraight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let completion = construction
        .find_completion(&world, &[])
        .expect("completion");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    let circuit = construction.circuit().unwrap();
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(&circuit, &model.train, config, 0.0, 2.0);
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_limits(
        &circuit,
        &model.train,
        model.comfortable_force_limits,
    );
    let mut completed = false;
    for _ in 0..60_000 {
        train.step(&circuit);
        recorder.record(&train.take_readings(), model.name);
        if train.distance() >= circuit.length() {
            completed = true;
            break;
        }
        if !train.notes().is_empty() {
            break;
        }
    }
    let report = recorder.finish(completed, !train.notes().is_empty());
    assert!(report.completed, "train notes: {:?}", train.notes());
    assert_eq!(report.statistics.inversions, 1);
    // Excessive forces reduce excitement under the calibrated scoring model.
    assert!((2.0..3.0).contains(&report.excitement));
    assert!(report.statistics.max_positive_vertical_g > 5.0);
    assert!(report.statistics.max_lateral_g > 4.0);
    assert!(report.notes.iter().any(|note| note.contains("F2291")));
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("above its rated speed band"))
    );
}
