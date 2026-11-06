use theme_park_sim::track::construction::{
    select_speed_matched_variant, select_speed_matched_variant_for_train,
};
use theme_park_sim::track::{Piece, PieceId, RatedSpeeds, catalog, hybrid_model};

fn candidate(id: PieceId, minimum_entry: f64, maximum_entry: f64) -> Piece {
    let mut piece = catalog()
        .iter()
        .find(|piece| piece.id == id)
        .unwrap()
        .clone();
    piece.rated_speeds = Some(RatedSpeeds {
        minimum_entry,
        maximum_entry,
    });
    piece
}

#[test]
fn selects_the_containing_window_with_the_largest_absolute_margin() {
    let candidates = [
        candidate(PieceId::Loop, 20.0, 30.0),
        candidate(PieceId::LargeLoop, 22.0, 40.0),
    ];
    let selected = select_speed_matched_variant(&candidates, 27.0).unwrap();
    assert_eq!(selected.id, PieceId::LargeLoop);
    assert!(selected.fits);
}

#[test]
fn selects_the_nearest_window_when_none_contains_the_speed() {
    let candidates = [
        candidate(PieceId::Loop, 20.0, 25.0),
        candidate(PieceId::LargeLoop, 28.0, 35.0),
    ];
    for (speed, expected) in [
        (10.0, PieceId::Loop),
        (27.0, PieceId::LargeLoop),
        (40.0, PieceId::LargeLoop),
    ] {
        let selected = select_speed_matched_variant(&candidates, speed).unwrap();
        assert_eq!(selected.id, expected);
        assert!(!selected.fits);
    }
}

#[test]
fn boundaries_fit_and_ties_keep_candidate_order() {
    let candidates = [
        candidate(PieceId::Loop, 20.0, 25.0),
        candidate(PieceId::LargeLoop, 25.0, 30.0),
    ];
    let selected = select_speed_matched_variant(&candidates, 25.0).unwrap();
    assert_eq!(selected.id, PieceId::Loop);
    assert!(selected.fits);
}

#[test]
fn ignores_unrated_candidates_and_handles_empty_input() {
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap()
        .clone();
    assert!(select_speed_matched_variant(&[], 25.0).is_none());
    assert!(select_speed_matched_variant(&[straight], 25.0).is_none());
}

#[test]
fn actual_loop_sizes_are_selected_at_their_rated_midpoints() {
    let candidates: Vec<_> = [PieceId::Loop, PieceId::LargeLoop]
        .into_iter()
        .map(|id| {
            catalog()
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .clone()
        })
        .collect();
    for candidate in &candidates {
        let window = candidate.rated_speeds.unwrap();
        let speed = (window.minimum_entry + window.maximum_entry) / 2.0;
        let selected = select_speed_matched_variant(&candidates, speed).unwrap();
        assert_eq!(selected.id, candidate.id);
        assert!(selected.fits);
    }
}

#[test]
fn airtime_hills_keep_nearest_window_fallback_and_roll_matching() {
    let family = |ids: &[PieceId]| -> Vec<Piece> {
        ids.iter()
            .map(|id| {
                catalog()
                    .iter()
                    .find(|piece| piece.id == *id)
                    .unwrap()
                    .clone()
            })
            .collect()
    };
    let ejectors = family(&[
        PieceId::EjectorHill8x48,
        PieceId::EjectorHill12x64,
        PieceId::EjectorHill8x64,
        PieceId::EjectorHill12x80,
        PieceId::EjectorHill16x96,
    ]);
    assert!(select_speed_matched_variant(&ejectors, 25.0).unwrap().fits);
    let selected = select_speed_matched_variant(&ejectors, 30.0).unwrap();
    assert_eq!(selected.id, PieceId::EjectorHill16x96);
    assert!(!selected.fits);
    let rolls = family(&[PieceId::ZeroGRollFastLeft, PieceId::ZeroGRollCompactLeft]);
    assert_eq!(
        select_speed_matched_variant(&rolls, 19.0).unwrap().id,
        PieceId::ZeroGRollFastLeft
    );
}

#[test]
fn configured_train_changes_rolled_inversion_speed_match() {
    let candidates = [catalog()
        .iter()
        .find(|piece| piece.id == PieceId::ZeroGRollCompactLeft)
        .unwrap()
        .clone()];
    let mut train = hybrid_model().train;
    train.cars = 4;
    train.length_metres = 12.0;
    assert!(
        !select_speed_matched_variant_for_train(
            &candidates,
            16.1,
            &train,
            hybrid_model().comfortable_force_limits
        )
        .unwrap()
        .fits
    );
    train.cars = 8;
    train.length_metres = 24.0;
    assert!(
        select_speed_matched_variant_for_train(
            &candidates,
            16.1,
            &train,
            hybrid_model().comfortable_force_limits
        )
        .unwrap()
        .fits
    );
}

#[test]
fn hills_choose_the_fitting_prediction_closest_to_the_feel_target() {
    use theme_park_sim::track::predicted_hill_forces;
    let candidates: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            matches!(
                piece.id,
                PieceId::EjectorHill8x48
                    | PieceId::EjectorHill12x64
                    | PieceId::EjectorHill8x64
                    | PieceId::EjectorHill12x80
                    | PieceId::EjectorHill16x96
            )
        })
        .cloned()
        .collect();
    for speed in [20.0, 22.0, 24.0, 25.0, 26.0] {
        let fitting: Vec<_> = candidates
            .iter()
            .filter(|piece| {
                let window = piece.rated_speeds.unwrap();
                window.minimum_entry <= speed && speed <= window.maximum_entry
            })
            .filter_map(|piece| {
                predicted_hill_forces(piece, speed)
                    .map(|forces| (piece.id, (forces.minimum_vertical_g + 1.2).abs()))
            })
            .collect();
        if let Some(expected) = fitting
            .iter()
            .min_by(|first, second| first.1.total_cmp(&second.1))
        {
            assert_eq!(
                select_speed_matched_variant(&candidates, speed).unwrap().id,
                expected.0,
                "speed {speed}"
            );
        }
    }
}

#[test]
fn force_target_breaks_equal_window_ties() {
    let candidates = [
        candidate(PieceId::EjectorHill16x96, 0.0, 100.0),
        candidate(PieceId::EjectorHill8x64, 0.0, 100.0),
    ];
    let forces = |piece: &Piece| {
        theme_park_sim::track::predicted_hill_forces(piece, 25.0)
            .unwrap()
            .minimum_vertical_g
    };
    assert!((forces(&candidates[1]) + 1.2).abs() < (forces(&candidates[0]) + 1.2).abs());
    assert_eq!(
        select_speed_matched_variant(&candidates, 25.0).unwrap().id,
        candidates[1].id
    );
}

#[test]
fn outer_bank_selection_uses_rider_vertical_force() {
    use theme_park_sim::track::predicted_hill_forces;
    let candidates = [
        candidate(PieceId::EjectorHill16x96OuterLeft, 0.0, 100.0),
        candidate(PieceId::EjectorHill8x64OuterLeft, 0.0, 100.0),
    ];
    let expected = candidates
        .iter()
        .min_by(|first, second| {
            let distance = |piece: &Piece| {
                (predicted_hill_forces(piece, 25.0)
                    .unwrap()
                    .minimum_vertical_g
                    + 1.2)
                    .abs()
            };
            distance(first).total_cmp(&distance(second))
        })
        .unwrap();
    assert_eq!(
        select_speed_matched_variant(&candidates, 25.0).unwrap().id,
        expected.id
    );
}

#[test]
fn ejector_selection_uses_the_models_comfort_limit() {
    use theme_park_sim::track::{predicted_hill_forces, wooden_twister_model};
    let model = wooden_twister_model();
    let candidates = [
        candidate(PieceId::EjectorHill16x96, 0.0, 100.0),
        candidate(PieceId::EjectorHill8x64, 0.0, 100.0),
    ];
    let expected = candidates
        .iter()
        .min_by(|first, second| {
            let distance = |piece: &Piece| {
                (predicted_hill_forces(piece, 25.0)
                    .unwrap()
                    .minimum_vertical_g
                    + 0.7)
                    .abs()
            };
            distance(first).total_cmp(&distance(second))
        })
        .unwrap();
    assert_eq!(
        select_speed_matched_variant_for_train(
            &candidates,
            25.0,
            &model.train,
            model.comfortable_force_limits
        )
        .unwrap()
        .id,
        expected.id
    );
}
