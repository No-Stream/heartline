use crate::physics::demo_circuit;
use crate::track::blocks::brake_runs;

#[test]
fn automatic_search_finds_two_trains_on_browser_circuit() {
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};
    let ids = [
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
    ];
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in ids {
        let mut piece = catalog()
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 3 | 10) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let config = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let mut search =
        super::BlockPlacementSearch::new(circuit, hybrid_model().train, config).unwrap();
    search.advance(300_000);
    assert!(search.best().capacity() >= 2);
}

#[test]
fn manual_search_selects_best_real_powered_hold_combination() {
    use crate::physics::{Circuit, PhysicsConfig};
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};

    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
        PieceId::Station,
        PieceId::Straight,
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
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 3 | 4 | 11 | 12) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = Circuit::new(pieces).unwrap();
    let train = hybrid_model().train;
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let mut search =
        super::BlockManualSearch::new(circuit.clone(), train.clone(), config, Vec::new()).unwrap();
    assert_eq!(search.choices.len(), 2);
    for alternatives in &mut search.choices {
        alternatives.reverse();
    }
    let first_runs = search
        .choices
        .iter()
        .map(|alternatives| alternatives[0].clone())
        .collect();
    let mut budget = 1_000_000;
    let first = super::BlockLayout::from_runs_with_budget(
        &circuit,
        &train,
        config,
        first_runs,
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        first.capacity(),
        1,
        "first real combination must be a lower-capacity valid layout"
    );

    let mut bounded =
        super::BlockManualSearch::new(circuit.clone(), train.clone(), config, Vec::new()).unwrap();
    for alternatives in &mut bounded.choices {
        alternatives.reverse();
    }
    let fallback = bounded.finish_with_work_limit(0).unwrap();
    assert_eq!(fallback.capacity(), 1);
    assert!(fallback.capacity_search_complete());
    assert!(!fallback.placement_search_complete());

    while search.result().is_none() && search.work_completed() < 1_000_000 {
        let previous = search.work_completed();
        search.advance(1_000);
        assert!(search.work_completed() - previous <= 1_000);
    }
    let best = search
        .result()
        .expect("manual search must finish")
        .as_ref()
        .unwrap();
    assert_eq!(best.capacity(), 2);
    assert!(best.capacity_search_complete());
}

#[test]
fn capacity_verification_waits_for_two_distinct_peak_cohorts_to_finish() {
    let mut cohorts = super::SustainedCohorts::new(2);
    cohorts.observe_active(&[0, 1]);
    cohorts.complete(0);
    cohorts.observe_active(&[1, 2]);
    cohorts.complete(2);
    cohorts.observe_active(&[1, 3]);
    cohorts.complete(3);
    cohorts.observe_active(&[1, 4]);
    cohorts.complete(1);
    assert!(
        !cohorts.verified(),
        "one train must not stand in for a second cohort"
    );

    cohorts.observe_active(&[4, 5]);
    cohorts.complete(4);
    assert!(!cohorts.verified());
    cohorts.complete(5);
    assert!(cohorts.verified());
}

#[test]
fn capacity_verification_observes_two_real_train_cohorts() {
    use crate::physics::{BlockOperation, Circuit, PhysicsConfig};
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};

    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
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
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 3 | 10) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = Circuit::new(pieces).unwrap();
    let boundaries = [2, 6, 10, 13]
        .into_iter()
        .map(|end| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        })
        .collect();
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let layout =
        super::BlockLayout::new(&circuit, &hybrid_model().train, config, boundaries).unwrap();
    assert!(layout.capacity() >= 2);
    let mut operation = BlockOperation::new_without_reports(layout.clone());
    let mut cohorts = super::SustainedCohorts::new(layout.capacity());
    let mut captured_cohorts = Vec::new();
    for _ in 0..(600.0 / crate::physics::TIME_STEP) as usize {
        while operation.can_dispatch() {
            operation.dispatch();
        }
        let active: Vec<_> = operation.trains().map(|(id, _)| id).collect();
        if cohorts.remaining.is_empty() && !cohorts.verified() && active.len() == layout.capacity()
        {
            captured_cohorts.push(active.clone());
        }
        cohorts.observe_active(&active);
        for (id, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            cohorts.complete(id);
        }
        assert!(!operation.interlock_triggered());
        if cohorts.verified() {
            break;
        }
    }
    assert!(cohorts.verified());
    assert_eq!(captured_cohorts.len(), 2);
    assert!(
        captured_cohorts[0]
            .iter()
            .all(|id| !captured_cohorts[1].contains(id))
    );
}

#[test]
fn level_unbanked_turn_can_hold_and_restart_a_train() {
    use crate::physics::{Circuit, PhysicsConfig};
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
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
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 3 | 10) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = Circuit::new(pieces).unwrap();
    let train = hybrid_model().train;
    let boundary: f64 = circuit.pieces()[..8]
        .iter()
        .map(|piece| piece.length())
        .sum();
    let candidates = brake_runs(&circuit, train.length_metres);
    assert!(candidates.iter().any(|run| {
        (run.boundary_metres - boundary).abs() < 1e-7
            && run
                .piece_indices
                .iter()
                .any(|&index| circuit.pieces()[index].id == PieceId::TurnLargeExit)
    }));
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let layout = super::BlockLayout::new(&circuit, &train, config, vec![boundary]).unwrap();
    let index = layout
        .boundaries()
        .iter()
        .position(|distance| (*distance - boundary).abs() < 1e-7)
        .unwrap();
    assert!(
        layout
            .brake_pieces(index)
            .iter()
            .any(|&piece| circuit.pieces()[piece].id == PieceId::TurnLargeExit)
    );
    assert!(layout.capacity() >= 1);
}

#[test]
fn gentle_downhill_brake_requires_holding_force_and_validates_restart() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let candidates = brake_runs(&circuit, train.length_metres);
    let downhill: Vec<_> = candidates
        .iter()
        .filter(|run| {
            run.piece_indices
                .iter()
                .all(|&index| circuit.pieces()[index].id == crate::track::PieceId::GentleDown)
        })
        .collect();
    assert!(
        !downhill.is_empty(),
        "gentle downhill track should have brake candidates"
    );
    let weak = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let strong = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        braking_acceleration: 6.0,
        drive_acceleration: 6.0,
        ..Default::default()
    };
    let (boundary, layout) = downhill
        .iter()
        .find_map(|run| {
            super::BlockLayout::new(&circuit, &train, strong, vec![run.boundary_metres])
                .ok()
                .map(|layout| (run.boundary_metres, layout))
        })
        .expect("a physically restartable downhill brake should validate");
    let error = super::BlockLayout::new(&circuit, &train, weak, vec![boundary]).unwrap_err();
    assert!(error.contains("holding force"), "{error}");
    assert!(layout.capacity() >= 1);
}

#[test]
fn gentle_uphill_brake_requires_drive_force_and_validates_restart() {
    let mut pieces = demo_circuit().pieces().to_vec();
    for piece in pieces
        .iter_mut()
        .filter(|piece| piece.id == crate::track::PieceId::GentleUp)
        .skip(1)
    {
        piece.kind = crate::track::PieceKind::Track;
    }
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let train = crate::track::hybrid_model().train;
    let candidates = brake_runs(&circuit, train.length_metres);
    let uphill: Vec<_> = candidates
        .iter()
        .filter(|run| {
            run.piece_indices.iter().all(|&index| {
                circuit.pieces()[index].id == crate::track::PieceId::GentleUp
                    && circuit.pieces()[index].kind == crate::track::PieceKind::Track
            })
        })
        .collect();
    assert!(
        !uphill.is_empty(),
        "gentle uphill track should have brake candidates"
    );
    let weak = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        braking_acceleration: 6.0,
        drive_acceleration: 2.0,
        ..Default::default()
    };
    let strong = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        braking_acceleration: 6.0,
        drive_acceleration: 6.0,
        ..Default::default()
    };
    let (boundary, layout) = uphill
        .iter()
        .find_map(|run| {
            super::BlockLayout::new(&circuit, &train, strong, vec![run.boundary_metres])
                .ok()
                .map(|layout| (run.boundary_metres, layout))
        })
        .expect("a physically restartable uphill brake should validate");
    let error = super::BlockLayout::new(&circuit, &train, weak, vec![boundary]).unwrap_err();
    assert!(error.contains("drive force"), "{error}");
    assert!(layout.capacity() >= 1);
}

#[test]
fn hills_inversions_and_overbanks_are_not_brake_sites() {
    use crate::track::{PieceId, catalog};
    let pieces = catalog();
    for id in [
        PieceId::AirtimeHill,
        PieceId::Loop,
        PieceId::HybridOverbankLeft,
    ] {
        let piece = pieces.iter().find(|piece| piece.id == id).unwrap();
        assert!(
            !super::eligible(piece),
            "{id:?} must not be a stopping site"
        );
    }
}

#[test]
fn stalled_circuit_reports_zero_capacity_after_asynchronous_fallback_trial() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let config = crate::physics::PhysicsConfig {
        powered_sections: false,
        ..Default::default()
    };
    let mut search =
        super::BlockPlacementSearch::new(circuit.clone(), train.clone(), config).unwrap();
    assert!(!search.is_complete());
    assert!(!search.best().capacity_search_complete());
    search.advance(2_000);
    assert!(search.is_complete());
    assert_eq!(search.best().capacity(), 0);
    assert!(search.best().capacity_search_complete());

    let mut test_train = crate::physics::CoasterRun::new(&circuit, &train, config);
    let outcome = (0..1_000)
        .find_map(|_| test_train.step(&circuit))
        .expect("stalled train is recovered");
    assert!(outcome.recovered);
}

#[test]
fn complete_single_train_circuit_is_verified_before_reporting_capacity() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let config = crate::physics::PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let baseline = super::BlockLayout::unverified_baseline(&circuit, &train, config).unwrap();
    assert_eq!(baseline.capacity(), 1);
    assert!(!baseline.capacity_search_complete());
    let mut search = super::BlockManualSearch::new(circuit, train, config, Vec::new()).unwrap();
    assert!(search.result().is_none());
    for _ in 0..120 {
        search.advance(1_000);
        if search.result().is_some() {
            break;
        }
    }
    let layout = search
        .result()
        .expect("fallback trial finishes")
        .as_ref()
        .unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(layout.capacity_search_complete());
}

#[test]
fn long_straights_share_brake_piece_storage() {
    use crate::track::{PieceId, TrackState, catalog};
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    let mut ids = vec![PieceId::Station];
    ids.extend(std::iter::repeat_n(PieceId::Straight, 255));
    ids.extend([PieceId::TurnLargeEntry, PieceId::TurnLargeExit]);
    ids.extend(std::iter::repeat_n(PieceId::Straight, 256));
    ids.extend([PieceId::TurnLargeEntry, PieceId::TurnLargeExit]);
    for id in ids {
        let piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let runs = brake_runs(&circuit, 18.0);
    assert!(runs.len() > 50_000);
    let storage = &runs[0].piece_indices.storage;
    assert_eq!(storage.len(), circuit.pieces().len() * 2);
    assert!(
        runs.iter()
            .all(|run| std::sync::Arc::ptr_eq(storage, &run.piece_indices.storage))
    );
    for run in &runs {
        let actual_length: f64 = run
            .piece_indices
            .iter()
            .map(|&index| circuit.pieces()[index].length())
            .sum();
        assert!((actual_length - run.length_metres).abs() < 1e-7);
        assert!(
            run.piece_indices
                .windows(2)
                .all(|pair| (pair[0] + 1) % circuit.pieces().len() == pair[1])
        );
    }
}

#[test]
fn overcrowded_trial_keeps_a_safe_lower_train_capacity() {
    use crate::physics::{BlockOperation, Circuit, PhysicsConfig};
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
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
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 3 | 10) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = Circuit::new(pieces).unwrap();
    let boundary = |end: usize| {
        circuit.pieces()[..end]
            .iter()
            .map(|piece| piece.length())
            .sum()
    };
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..Default::default()
    };
    let boundaries = vec![boundary(3), boundary(6), boundary(11)];
    let candidates = brake_runs(&circuit, hybrid_model().train.length_metres);
    let runs: Vec<_> = boundaries
        .iter()
        .map(|boundary| {
            candidates
                .iter()
                .find(|run| (run.boundary_metres - boundary).abs() < 1e-7)
                .unwrap()
                .clone()
        })
        .collect();
    for attempted_capacity in [3, 2] {
        let mut trial = super::BlockLayout::assemble_runs(
            &circuit,
            &hybrid_model().train,
            config,
            runs.clone(),
        )
        .unwrap();
        trial.capacity = attempted_capacity;
        let mut operation = BlockOperation::new_without_reports(trial);
        let mut completed = 0;
        for _ in 0..120_000 {
            while operation.can_dispatch() {
                operation.dispatch();
            }
            for (_, outcome) in operation.step(&[]) {
                assert!(!outcome.recovered);
                completed += 1;
            }
            if operation.interlock_triggered() {
                break;
            }
        }
        assert!(
            operation.interlock_triggered(),
            "{attempted_capacity}-train trial should reach the emergency interlock"
        );
        assert!(completed > 0);
    }
    let layout =
        super::BlockLayout::new(&circuit, &hybrid_model().train, config, boundaries).unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(!layout.capacity_search_complete());
    let capacity = layout.capacity();
    let mut operation = BlockOperation::new_without_reports(layout);
    let mut completed = 0;
    let mut maximum_active = 0;
    for _ in 0..120_000 {
        while operation.can_dispatch() {
            operation.dispatch();
        }
        maximum_active = maximum_active.max(operation.trains().count());
        for (_, outcome) in operation.step(&[]) {
            assert!(!outcome.recovered);
            completed += 1;
        }
        assert!(!operation.interlock_triggered());
        if completed >= capacity * 3 {
            break;
        }
    }
    assert_eq!(maximum_active, capacity);
    assert!(
        completed >= capacity * 3,
        "only {completed} trains completed"
    );
}

#[test]
fn automatic_search_finds_two_train_layouts_on_distinct_circuits() {
    use crate::track::{PieceId, TrackState, catalog};
    for extra_straights in [0, 1] {
        let mut ids = vec![
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
        ];
        ids.extend(std::iter::repeat_n(PieceId::Straight, extra_straights));
        ids.extend([
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
        ]);
        ids.extend(std::iter::repeat_n(PieceId::Straight, extra_straights));
        ids.extend([
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Station,
        ]);
        let catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in ids {
            let mut piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            if pieces.len() == 3 || pieces.len() == 10 + extra_straights {
                piece.kind = crate::track::PieceKind::Lift;
            }
            state = piece.exit;
            pieces.push(piece);
        }
        let circuit = crate::physics::Circuit::new(pieces).unwrap();
        let config = crate::physics::PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let mut search =
            super::BlockPlacementSearch::new(circuit, crate::track::hybrid_model().train, config)
                .unwrap();
        while !search.is_complete()
            && search.best().capacity() < 2
            && search.work_completed() < 300_000
        {
            search.advance(5_000);
        }
        assert!(
            search.best().capacity() >= 2,
            "extra_straights={extra_straights}: searched {} steps",
            search.work_completed()
        );
        assert!(
            search
                .best()
                .boundaries()
                .iter()
                .enumerate()
                .filter(|(index, _)| !search.best().brake_pieces(*index).is_empty())
                .count()
                >= 2
        );
    }
}

#[test]
fn automatic_placement_completes_on_a_small_valid_circuit() {
    use crate::track::{PieceId, TrackState, catalog};
    let catalog = catalog();
    let mut state = TrackState::origin();
    let mut pieces = Vec::new();
    for id in [
        PieceId::Station,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::BankLeft,
        PieceId::LargeLeftBankedTurnEntry,
        PieceId::LargeLeftBankedTurnExit,
        PieceId::UnbankLeft,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::BankLeft,
        PieceId::LargeLeftBankedTurnEntry,
        PieceId::LargeLeftBankedTurnExit,
        PieceId::UnbankLeft,
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .unwrap()
            .at(state)
            .unwrap();
        if matches!(pieces.len(), 8) {
            piece.kind = crate::track::PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let mut train = crate::track::hybrid_model().train;
    train.length_metres = 33.0;
    let config = crate::physics::PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        ..Default::default()
    };
    let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
    let mut polls = 0;
    while !search.is_complete() && polls < 1_000_000 {
        search.advance(1_000);
        polls += 1_000;
    }
    assert!(
        search.is_complete(),
        "placement failed to finish after {polls} work steps"
    );
    assert!(search.best().placement_search_complete());
    assert_eq!(search.best().capacity(), 1);
    assert!(
        search
            .best()
            .boundaries()
            .iter()
            .enumerate()
            .any(|(index, _)| !search.best().brake_pieces(index).is_empty())
    );
}

#[test]
fn automatic_search_resumes_with_a_bounded_number_of_steps() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let mut chunked =
        super::BlockPlacementSearch::new(circuit.clone(), train.clone(), Default::default())
            .unwrap();
    let mut continuous =
        super::BlockPlacementSearch::new(circuit, train, Default::default()).unwrap();
    chunked.advance(400);
    chunked.advance(600);
    continuous.advance(1000);
    assert_eq!(chunked.work_completed(), 1000);
    assert_eq!(chunked.work_completed(), continuous.work_completed());
    assert_eq!(chunked.is_complete(), continuous.is_complete());
    assert_eq!(chunked.best().capacity(), continuous.best().capacity());
    assert_eq!(chunked.best().boundaries(), continuous.best().boundaries());
}

#[test]
fn unresolved_candidates_cannot_hide_a_capacity_they_cannot_exceed() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let mut search = super::BlockPlacementSearch::new(circuit, train, Default::default()).unwrap();
    while search.active_baseline {
        search.advance(1_000);
    }
    search.active = None;
    search.bootstrap.clear();
    search.search_size = 0;
    search.unverified_upper_bound = search.best().capacity();
    search.advance(1);
    assert!(search.is_complete());
    assert!(search.best().placement_search_complete());
}

#[test]
fn manual_boundary_validation_can_resume_without_changing_the_result() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let boundary = brake_runs(&circuit, train.length_metres)[0].boundary_metres;
    let expected = super::BlockLayout::new(&circuit, &train, Default::default(), vec![boundary]);
    let mut search =
        super::BlockManualSearch::new(circuit, train, Default::default(), vec![boundary]).unwrap();
    search.advance(1);
    assert!(search.result().is_none());
    while search.result().is_none() {
        search.advance(1000);
    }
    match (expected, search.result().unwrap()) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(expected.boundaries(), actual.boundaries());
            assert_eq!(expected.capacity(), actual.capacity());
        }
        (Err(expected), Err(actual)) => assert_eq!(expected, *actual),
        _ => panic!("resumed validation disagrees with synchronous validation"),
    }
}

#[test]
fn placement_search_reuses_validation_for_the_same_candidate_set() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let mut search = super::BlockPlacementSearch::new(circuit, train, Default::default()).unwrap();
    while search.active.is_none() && !search.is_complete() {
        search.advance(1);
    }
    let original = std::ptr::from_ref(search.active.as_ref().unwrap().as_ref().get_ref());
    search.advance(10);
    let resumed = std::ptr::from_ref(search.active.as_ref().unwrap().as_ref().get_ref());
    assert!(
        std::ptr::eq(original, resumed),
        "advancing should resume the same candidate validation"
    );
}

#[test]
fn circular_nonoverlap_bound_matches_exhaustive_selection() {
    use crate::track::blocks::BrakeRun;
    let runs: Vec<_> = [
        vec![6, 7, 0],
        vec![0, 1],
        vec![1, 2],
        vec![3, 4],
        vec![5, 6],
        vec![7, 0, 1],
    ]
    .into_iter()
    .map(|piece_indices| BrakeRun {
        start_metres: 0.0,
        boundary_metres: 0.0,
        length_metres: 1.0,
        piece_indices: piece_indices.into(),
    })
    .collect();
    for excluded_mask in 0..1_usize << runs.len() {
        let available: Vec<_> = runs
            .iter()
            .enumerate()
            .filter(|(index, _)| excluded_mask & (1 << index) == 0)
            .map(|(_, run)| run)
            .collect();
        let brute_max = (0..1_usize << available.len())
            .filter_map(|mask| {
                let mut used = [false; 8];
                let mut count = 0;
                for (index, run) in available.iter().enumerate() {
                    if mask & (1 << index) == 0 {
                        continue;
                    }
                    if run.piece_indices.iter().any(|piece| used[*piece]) {
                        return None;
                    }
                    for piece in &run.piece_indices {
                        used[*piece] = true;
                    }
                    count += 1;
                }
                Some(count)
            })
            .max()
            .unwrap();
        assert_eq!(super::maximum_nonoverlapping_runs(&available), brute_max);
    }
}

#[test]
fn bounded_automatic_search_returns_verified_fallback_and_reports_incomplete() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let layout =
        super::BlockLayout::automatic_with_budget(&circuit, &train, Default::default(), 0, 0, 0)
            .unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(!layout.placement_search_complete());
}

#[test]
fn automatic_search_reports_incomplete_when_physics_work_budget_is_exhausted() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let layout = super::BlockLayout::automatic_with_budget(
        &circuit,
        &train,
        Default::default(),
        100_000,
        64,
        0,
    )
    .unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(!layout.placement_search_complete());
}

#[test]
fn station_start_and_end_are_implicit_boundaries() {
    let circuit = demo_circuit();
    let layout = super::BlockLayout::new(
        &circuit,
        &crate::track::hybrid_model().train,
        Default::default(),
        Vec::new(),
    )
    .unwrap();
    let station = circuit
        .pieces()
        .iter()
        .find(|piece| piece.kind == crate::track::PieceKind::Station)
        .unwrap();
    assert!(layout.boundaries().contains(&0.0));
    assert!(
        layout
            .boundaries()
            .iter()
            .any(|boundary| (*boundary - station.length()).abs() < 1e-7)
    );
}

#[test]
fn candidate_includes_longer_brake_run_for_same_boundary() {
    let circuit = demo_circuit();
    let runs = brake_runs(&circuit, 18.0);
    assert!(runs.iter().any(|short| runs.iter().any(|long| {
        short.boundary_metres == long.boundary_metres
            && short.piece_indices.len() < long.piece_indices.len()
            && short
                .piece_indices
                .iter()
                .all(|piece| long.piece_indices.contains(piece))
    })));
}

#[test]
fn existing_brakes_remain_boundaries_without_installed_stopping_runs() {
    use crate::track::{PieceId, PieceKind};
    let mut pieces = demo_circuit().pieces().to_vec();
    let piece = pieces
        .iter_mut()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    piece.kind = PieceKind::Brakes;
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let layout = super::BlockLayout::new(
        &circuit,
        &crate::track::hybrid_model().train,
        Default::default(),
        Vec::new(),
    )
    .unwrap();
    let mut distance = 0.0;
    for piece in circuit.pieces() {
        distance += piece.length();
        if piece.kind == PieceKind::Brakes {
            let end = distance.rem_euclid(circuit.length());
            let index = layout
                .boundaries()
                .iter()
                .position(|boundary| (*boundary - end).abs() < 1e-7)
                .unwrap();
            assert!(layout.brake_pieces(index).is_empty());
        }
    }
}

#[test]
fn manual_boundary_uses_a_longer_run_when_short_run_cannot_stop() {
    let mut pieces = demo_circuit().pieces().to_vec();
    let first_straight = pieces
        .iter_mut()
        .find(|piece| piece.id == crate::track::PieceId::Straight)
        .unwrap();
    first_straight.kind = crate::track::PieceKind::Track;
    let circuit = crate::physics::Circuit::new(pieces).unwrap();
    let train = crate::track::hybrid_model().train;
    let candidates = brake_runs(&circuit, train.length_metres);
    let config = crate::physics::PhysicsConfig {
        braking_acceleration: 0.5,
        ..Default::default()
    };
    for short in &candidates {
        if super::BlockLayout::from_runs(&circuit, &train, config, vec![short.clone()]).is_ok() {
            continue;
        }
        if let Some(long) = candidates.iter().find(|candidate| {
            candidate.boundary_metres == short.boundary_metres
                && candidate.length_metres > short.length_metres
                && super::BlockLayout::from_runs(
                    &circuit,
                    &train,
                    config,
                    vec![(*candidate).clone()],
                )
                .is_ok()
        }) {
            let layout =
                super::BlockLayout::new(&circuit, &train, config, vec![short.boundary_metres])
                    .unwrap();
            assert!(
                layout
                    .runs
                    .iter()
                    .any(|run| run.boundary_metres == long.boundary_metres
                        && run.length_metres > short.length_metres)
            );
            return;
        }
    }
    panic!("test circuit should have a boundary requiring a longer brake run");
}

#[test]
fn automatic_candidates_group_consecutive_level_straight_pieces() {
    let circuit = demo_circuit();
    let runs = brake_runs(&circuit, 18.0);
    assert!(runs.iter().any(|run| run.piece_indices.len() >= 2));
    assert!(
        runs.iter()
            .any(|run| run.piece_indices.iter().all(|&index| {
                let piece = &circuit.pieces()[index];
                piece.entry.slope == crate::track::Slope::Level
                    && piece.exit.slope == crate::track::Slope::Level
            }))
    );
    for run in &runs {
        assert!(run.length_metres >= 18.0);
        for &index in &run.piece_indices {
            let piece = &circuit.pieces()[index];
            for sample in 0..=32 {
                let point = piece.sample(piece.length() * sample as f64 / 32.0);
                assert!(point.tangent.z.abs() <= 0.447_214);
                assert!(point.roll.abs() <= 1e-6);
            }
        }
    }
}

#[test]
fn candidates_that_are_too_short_are_excluded() {
    let circuit = demo_circuit();
    assert!(brake_runs(&circuit, circuit.length()).is_empty());
}

#[test]
fn missing_brake_drive_rejects_brakes_and_reports_a_stalled_train() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let config = crate::physics::PhysicsConfig {
        drive_acceleration: 0.0,
        ..Default::default()
    };
    let run = &brake_runs(&circuit, train.length_metres)[0];
    assert!(super::BlockLayout::new(&circuit, &train, config, vec![run.boundary_metres]).is_err());
    let layout = super::BlockLayout::automatic(&circuit, &train, config).unwrap();
    assert_eq!(layout.capacity(), 0);
    assert!(layout.capacity_search_complete());
    assert!(layout.brake_pieces(0).is_empty());
}

#[test]
fn candidates_include_alternative_piece_ends_and_reject_overlap() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let runs = brake_runs(&circuit, train.length_metres);
    assert!(runs.len() >= 2);
    assert_ne!(runs[0].piece_indices, runs[1].piece_indices);
    let boundaries = runs
        .iter()
        .find_map(|first| {
            runs.iter().find_map(|second| {
                if (first.boundary_metres - second.boundary_metres).abs() < 1e-7 {
                    return None;
                }
                let every_pair_overlaps = runs
                    .iter()
                    .filter(|run| (run.boundary_metres - first.boundary_metres).abs() < 1e-7)
                    .all(|left| {
                        runs.iter()
                            .filter(|run| {
                                (run.boundary_metres - second.boundary_metres).abs() < 1e-7
                            })
                            .all(|right| super::brake_runs_overlap(left, right))
                    });
                every_pair_overlaps.then_some(vec![first.boundary_metres, second.boundary_metres])
            })
        })
        .expect("the circuit has two boundaries whose possible runs overlap");
    assert_eq!(
        super::BlockLayout::new(&circuit, &train, Default::default(), boundaries).unwrap_err(),
        "block brake runs cannot overlap"
    );
}

#[test]
fn powered_sections_get_a_candidate_at_their_end() {
    use crate::track::{PieceId, TrackState, catalog};
    let catalog = catalog();
    let mut saw_incomplete_placement_search = false;
    for powered in [PieceId::Straight, PieceId::Launch] {
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in [
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            powered,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            powered,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ] {
            let mut piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            if powered == PieceId::Straight && matches!(pieces.len(), 3 | 10) {
                piece.kind = crate::track::PieceKind::Lift;
            }
            state = piece.exit;
            pieces.push(piece);
        }
        let circuit = crate::physics::Circuit::new(pieces).unwrap();
        let train = if powered == PieceId::Launch {
            crate::track::multi_launch_model().train
        } else {
            crate::track::hybrid_model().train
        };
        let runs = brake_runs(&circuit, train.length_metres);
        let layout =
            super::BlockLayout::new(&circuit, &train, Default::default(), Vec::new()).unwrap();
        let mut test_train = crate::physics::CoasterRun::new(&circuit, &train, Default::default());
        let outcome = (0..120_000)
            .find_map(|_| test_train.step(&circuit))
            .expect("train finishes or is recovered");
        assert_eq!(layout.capacity(), usize::from(!outcome.recovered));
        assert!(layout.capacity_search_complete());
        saw_incomplete_placement_search |= !layout.placement_search_complete();
        let mut distance = 0.0;
        for piece in circuit.pieces() {
            distance += piece.length();
            if (powered == PieceId::Straight && piece.kind == crate::track::PieceKind::Lift)
                || (powered == PieceId::Launch && piece.id == powered)
            {
                let end = distance.rem_euclid(circuit.length());
                assert!(
                    runs.iter()
                        .any(|run| (run.boundary_metres - end).abs() < 1e-7)
                );
                let index = layout
                    .boundaries()
                    .iter()
                    .position(|boundary| (*boundary - end).abs() < 1e-7)
                    .unwrap();
                assert!(layout.brake_pieces(index).is_empty());
            }
            if piece.id == PieceId::Station {
                assert!(layout.boundaries().iter().any(|boundary| {
                    (*boundary - distance.rem_euclid(circuit.length())).abs() < 1e-7
                }));
            }
        }
    }
    assert!(saw_incomplete_placement_search);
}

#[test]
fn rejects_brakes_that_cannot_stop_an_approaching_train() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let runs = brake_runs(&circuit, train.length_metres);
    let boundary = runs
        .iter()
        .find(|run| {
            run.piece_indices.iter().all(|&index| {
                let piece = &circuit.pieces()[index];
                piece.id == crate::track::PieceId::Straight
                    && piece.kind == crate::track::PieceKind::Track
                    || matches!(
                        piece.kind,
                        crate::track::PieceKind::Station | crate::track::PieceKind::Brakes
                    )
            })
        })
        .expect("circuit has a straight brake candidate")
        .boundary_metres;
    let config = crate::physics::PhysicsConfig {
        braking_acceleration: 0.0001,
        ..Default::default()
    };
    let result = super::BlockLayout::new(&circuit, &train, config, vec![boundary]);
    assert_eq!(
        result.unwrap_err(),
        "train cannot safely reach and stop on the next block brake"
    );
}
