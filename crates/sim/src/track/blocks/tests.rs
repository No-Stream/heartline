use crate::physics::demo_circuit;
use crate::track::blocks::brake_runs;

#[test]
fn automatic_search_finds_two_train_layouts_on_distinct_circuits() {
    use crate::track::{PieceId, TrackState, catalog};
    for extra_straights in [0, 1] {
        let mut ids = vec![
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
        ];
        ids.extend(std::iter::repeat_n(PieceId::Straight, extra_straights));
        ids.extend([
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
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
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::Straight,
        PieceId::LiftFlat,
        PieceId::Straight,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
    ] {
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
    search.bootstrap.clear();
    search.search_size = 0;
    search.unverified_upper_bound = 1;
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
        piece_indices,
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
    let circuit = demo_circuit();
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
    for run in &runs {
        assert!(run.length_metres >= 18.0);
        for &index in &run.piece_indices {
            let piece = &circuit.pieces()[index];
            assert_eq!(piece.entry.slope, crate::track::Slope::Level);
            assert_eq!(piece.exit.slope, crate::track::Slope::Level);
        }
    }
}

#[test]
fn candidates_that_are_too_short_are_excluded() {
    let circuit = demo_circuit();
    assert!(brake_runs(&circuit, circuit.length()).is_empty());
}

#[test]
fn missing_brake_drive_rejects_brakes_and_falls_back_to_one_train() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let config = crate::physics::PhysicsConfig {
        drive_acceleration: 0.0,
        ..Default::default()
    };
    let run = &brake_runs(&circuit, train.length_metres)[0];
    assert!(super::BlockLayout::new(&circuit, &train, config, vec![run.boundary_metres]).is_err());
    let layout = super::BlockLayout::automatic(&circuit, &train, config).unwrap();
    assert_eq!(layout.capacity(), 1);
    assert!(layout.brake_pieces(0).is_empty());
}

#[test]
fn candidates_include_alternative_piece_ends_and_reject_overlap() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let runs = brake_runs(&circuit, train.length_metres);
    assert!(runs.len() >= 2);
    assert_ne!(runs[0].piece_indices, runs[1].piece_indices);
    let boundaries: Vec<_> = runs
        .iter()
        .take(2)
        .map(|run| (run.start_metres + run.length_metres).rem_euclid(circuit.length()))
        .collect();
    assert_eq!(
        super::BlockLayout::new(&circuit, &train, Default::default(), boundaries).unwrap_err(),
        "block brake runs cannot overlap"
    );
}

#[test]
fn powered_sections_get_a_candidate_at_their_end() {
    use crate::track::{PieceId, TrackState, catalog};
    let catalog = catalog();
    for powered in [PieceId::LiftFlat, PieceId::Launch] {
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
        let train = crate::track::hybrid_model().train;
        let runs = brake_runs(&circuit, train.length_metres);
        let layout =
            super::BlockLayout::new(&circuit, &train, Default::default(), Vec::new()).unwrap();
        assert_eq!(layout.capacity(), 1);
        let mut distance = 0.0;
        for piece in circuit.pieces() {
            distance += piece.length();
            if piece.id == powered {
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
}

#[test]
fn rejects_brakes_that_cannot_stop_an_approaching_train() {
    let circuit = demo_circuit();
    let train = crate::track::hybrid_model().train;
    let runs = brake_runs(&circuit, train.length_metres);
    let boundary = runs[0].boundary_metres;
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
