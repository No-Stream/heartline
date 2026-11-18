use theme_park_sim::track::construction::select_speed_matched_variant;
use theme_park_sim::track::{
    Piece, PieceId, catalog, hybrid_model, predicted_hill_forces, rated_speeds,
    wooden_twister_model,
};

fn pieces(ids: &[PieceId]) -> Vec<Piece> {
    ids.iter()
        .map(|id| {
            catalog()
                .iter()
                .find(|piece| piece.id == *id)
                .unwrap()
                .clone()
        })
        .collect()
}

#[test]
fn selects_the_fitting_loop_with_the_largest_window_margin() {
    let model = hybrid_model();
    let candidates = pieces(&[PieceId::Loop, PieceId::LargeLoop]);
    let loop_window = rated_speeds(&model, &candidates[0]).unwrap();
    let large_loop_window = rated_speeds(&model, &candidates[1]).unwrap();
    let overlap_minimum = loop_window.minimum_entry.max(large_loop_window.minimum_entry);
    let overlap_maximum = loop_window.maximum_entry.min(large_loop_window.maximum_entry);
    assert!(overlap_minimum < overlap_maximum);
    let entry_speed = (overlap_minimum + overlap_maximum) / 2.0;
    let expected = candidates
        .iter()
        .max_by(|first, second| {
            let margin = |piece: &Piece| {
                let window = rated_speeds(&model, piece).unwrap();
                (entry_speed - window.minimum_entry).min(window.maximum_entry - entry_speed)
            };
            margin(first).total_cmp(&margin(second))
        })
        .unwrap();

    let selected = select_speed_matched_variant(&candidates, entry_speed, &model).unwrap();

    assert_eq!(selected.id, expected.id);
    assert!(selected.fits);
}

#[test]
fn selects_the_nearest_window_when_the_speed_fits_none() {
    let model = hybrid_model();
    let candidates = pieces(&[PieceId::Loop, PieceId::LargeLoop]);
    for (entry_speed, expected_id) in [
        (0.0, PieceId::Loop),
        (60.0, PieceId::LargeLoop),
    ] {
        let selected = select_speed_matched_variant(&candidates, entry_speed, &model).unwrap();
        assert_eq!(selected.id, expected_id);
        assert!(!selected.fits);
    }
}

#[test]
fn accepts_each_loop_window_boundary() {
    let model = hybrid_model();
    let candidate = pieces(&[PieceId::Loop]);
    let window = rated_speeds(&model, &candidate[0]).unwrap();
    for entry_speed in [window.minimum_entry, window.maximum_entry] {
        let selected = select_speed_matched_variant(&candidate, entry_speed, &model).unwrap();
        assert_eq!(selected.id, PieceId::Loop);
        assert!(selected.fits);
    }
}

#[test]
fn ignores_unrated_candidates_and_handles_empty_input() {
    let model = hybrid_model();
    let straight = pieces(&[PieceId::Straight]);
    assert!(select_speed_matched_variant(&[], 25.0, &model).is_none());
    assert!(select_speed_matched_variant(&straight, 25.0, &model).is_none());
}

#[test]
fn actual_loop_windows_are_used_for_speed_matching() {
    let model = hybrid_model();
    let candidates = pieces(&[PieceId::Loop, PieceId::LargeLoop]);
    for candidate in &candidates {
        let window = rated_speeds(&model, candidate).unwrap();
        let entry_speed = (window.minimum_entry + window.maximum_entry) / 2.0;
        let selected = select_speed_matched_variant(&candidates, entry_speed, &model).unwrap();
        let selected_piece = candidates
            .iter()
            .find(|piece| piece.id == selected.id)
            .unwrap();
        let selected_window = rated_speeds(&model, selected_piece).unwrap();
        assert!(selected.fits);
        assert!((selected_window.minimum_entry..=selected_window.maximum_entry).contains(&entry_speed));
    }
}

#[test]
fn hill_selection_uses_the_fitting_window_closest_to_its_force_target() {
    let model = hybrid_model();
    let candidates = pieces(&[
        PieceId::EjectorHill8x48,
        PieceId::EjectorHill12x64,
        PieceId::EjectorHill8x64,
        PieceId::EjectorHill12x80,
        PieceId::EjectorHill16x96,
    ]);
    for entry_speed in [20.0, 22.0, 24.0, 25.0, 26.0] {
        let fitting: Vec<_> = candidates
            .iter()
            .filter(|piece| {
                let window = rated_speeds(&model, piece).unwrap();
                window.minimum_entry <= entry_speed && entry_speed <= window.maximum_entry
            })
            .filter_map(|piece| {
                predicted_hill_forces(piece, entry_speed)
                    .map(|forces| (piece.id, (forces.minimum_vertical_g + 1.2).abs()))
            })
            .collect();
        if let Some(expected) = fitting
            .iter()
            .min_by(|first, second| first.1.total_cmp(&second.1))
        {
            let selected = select_speed_matched_variant(&candidates, entry_speed, &model).unwrap();
            assert_eq!(selected.id, expected.0, "speed {entry_speed}");
            assert!(selected.fits, "speed {entry_speed}");
        }
    }
}

#[test]
fn ejector_target_uses_the_selected_models_force_limit() {
    let model = wooden_twister_model();
    let candidates = pieces(&[
        PieceId::WoodenEjectorHill6x48,
        PieceId::WoodenEjectorHill8x64,
    ]);
    let entry_speed = 18.0;
    let fitting: Vec<_> = candidates
        .iter()
        .filter(|piece| {
            let window = rated_speeds(&model, piece).unwrap();
            window.minimum_entry <= entry_speed && entry_speed <= window.maximum_entry
        })
        .collect();
    assert_eq!(fitting.len(), 2, "both ejector candidates should fit");
    let expected = fitting
        .iter()
        .min_by(|first, second| {
            let distance = |piece: &Piece| {
                (predicted_hill_forces(piece, entry_speed)
                    .unwrap()
                    .minimum_vertical_g
                    + 0.7)
                    .abs()
            };
            distance(first).total_cmp(&distance(second))
        })
        .unwrap();

    let selected = select_speed_matched_variant(&candidates, entry_speed, &model).unwrap();

    assert_eq!(selected.id, expected.id);
    assert!(selected.fits);
}
