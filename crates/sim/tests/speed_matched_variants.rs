use theme_park_sim::track::construction::select_speed_matched_variant;
use theme_park_sim::track::{Piece, PieceId, RatedSpeeds, catalog};

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
fn airtime_hills_select_the_containing_window_ridden_closest_to_its_top() {
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
    let selected = select_speed_matched_variant(&ejectors, 25.0).unwrap();
    assert_eq!(selected.id, PieceId::EjectorHill8x64);
    assert!(selected.fits);
    let selected = select_speed_matched_variant(&ejectors, 30.0).unwrap();
    assert_eq!(selected.id, PieceId::EjectorHill16x96);
    assert!(!selected.fits);
    let rolls = family(&[PieceId::ZeroGRollFastLeft, PieceId::ZeroGRollCompactLeft]);
    assert_eq!(
        select_speed_matched_variant(&rolls, 19.0).unwrap().id,
        PieceId::ZeroGRollFastLeft
    );
}
