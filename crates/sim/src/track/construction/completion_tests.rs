use super::*;
use crate::physics::{ConstructionTestRunRecorder, PhysicsConfig, Train};
use crate::track::{PieceId, TrackState, hybrid_model, wooden_twister_model};
use crate::world::Surface;

fn flat_world() -> World {
    World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    )
}

#[test]
fn unpowered_track_explains_that_the_train_stops_before_the_end() {
    use PieceId::*;
    let world = flat_world();
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

    let reason = construction.find_completions(&world, &[]).unwrap_err();

    assert!(
        reason.contains("The train stops before the end of the track"),
        "{reason}"
    );
}

#[test]
fn distant_end_explains_how_far_it_is_from_the_station() {
    use PieceId::*;
    let world = flat_world();
    let start = TrackState {
        x: 128,
        y: 127,
        z: 30,
        heading: crate::track::Heading::SouthEast,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, wooden_twister_model());
    construction.append(Station, &world, &[]).unwrap();
    for _ in 0..10 {
        construction.append(Straight, &world, &[]).unwrap();
    }

    let reason = construction.find_completions(&world, &[]).unwrap_err();

    assert!(
        reason.contains("from the station entry; bring it within 120 m"),
        "{reason}"
    );
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
    let completions = construction
        .find_completions(&world, &[])
        .expect("completion");
    let completion = &completions[0];
    construction
        .apply_completion(completion, &world, &[])
        .unwrap();
    let circuit = construction.circuit().unwrap();
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(&circuit, &model.train, config, 0.0, 2.0);
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
        &circuit,
        &model.train,
        &model,
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
    // Forces past the ASTM comfort envelope still reduce excitement without double-penalizing.
    assert!(
        (3.5..4.5).contains(&report.excitement),
        "excitement={:.3}",
        report.excitement
    );
    assert!(report.statistics.max_positive_vertical_g > 5.0);
    assert!(report.statistics.max_lateral_g > 4.0);
    assert!(report.notes.iter().any(|note| note.contains("F2291")));
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("Lateral force exceeded the duration-aware limit")),
        "report notes: {:?}",
        report.notes
    );
}

#[test]
fn finish_search_places_shaped_turns_for_non_hyper_profiled_models() {
    use PieceId::*;
    let world = flat_world();
    for (model, expected_min_speed) in [
        (crate::track::multi_launch_model(), 20.0),
        (crate::track::hybrid_model(), 20.0),
        (crate::track::wooden_twister_model(), 18.0),
        (crate::track::family_coaster_model(), 14.0),
    ] {
        let start = TrackState {
            x: 80,
            y: 80,
            z: 8,
            heading: crate::track::Heading::East,
            ..TrackState::origin()
        };
        let mut construction = Construction::new(start, model.clone());
        construction.append(Station, &world, &[]).unwrap();
        construction.set_chain(true);
        construction.append(LevelToGentleUp, &world, &[]).unwrap();
        construction.append(GentleUpToLevel, &world, &[]).unwrap();
        construction.set_chain(false);
        construction.append(LevelToGentleDown, &world, &[]).unwrap();
        construction.append(GentleDownToLevel, &world, &[]).unwrap();
        let profile = model
            .shaped_profile
            .expect("tested model has a shaped profile");
        assert!(
            profile.minimum_speed_mps <= expected_min_speed,
            "{} profile floor {}",
            model.key,
            profile.minimum_speed_mps
        );
        let mut search = construction
            .completion_search_with_judge(FinishForceJudge::DesignTargets)
            .expect("completion search starts");
        let speed_squared = expected_min_speed * expected_min_speed + 25.0;
        search.beam[0].speed_squared_estimate = speed_squared;
        let options = search.shaped_turn_options(&search.beam[0]);
        assert!(
            !options.is_empty(),
            "{} finish search offered no shaped turn options at {:.1} m/s",
            model.key,
            speed_squared.sqrt()
        );
    }
}
