use super::*;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn near_vec(a: Vec3, b: Vec3) {
    near(a.x, b.x);
    near(a.y, b.y);
    near(a.z, b.z);
}
fn near_angle(a: f64, b: f64) {
    near((a - b).sin(), 0.0);
    near((a - b).cos(), 1.0);
}

#[test]
fn all_pieces_match_their_end_states_and_have_regular_curves() {
    for piece in catalog() {
        for (distance, state) in [(0.0, piece.entry), (piece.length(), piece.exit)] {
            let sample = piece.sample(distance);
            near_vec(sample.position, state.position());
            near_vec(sample.tangent, state.tangent());
            near_angle(sample.roll, state.bank.radians());
            near(sample.up.length(), 1.0);
            near(sample.up.dot(sample.tangent), 0.0);
        }
        for index in 0..=1000 {
            let sample = piece.sample(piece.length() * f64::from(index) / 1000.0);
            assert!(sample.curvature.is_finite(), "{}", piece.name);
            near(sample.tangent.length(), 1.0);
            near(sample.up.length(), 1.0);
            near(sample.tangent.dot(sample.up), 0.0);
        }
    }
}

#[test]
fn joined_pieces_have_matching_tangents_and_roll() {
    for first in catalog() {
        for second in catalog().into_iter().filter(|p| {
            p.entry.heading == first.exit.heading
                && p.entry.slope == first.exit.slope
                && p.entry.bank == first.exit.bank
        }) {
            let joined = second.at(first.exit).unwrap();
            assert_eq!(joined.entry, first.exit);
            let a = first.sample(first.length());
            let b = joined.sample(0.0);
            near_vec(a.position, b.position);
            near_vec(a.tangent, b.tangent);
            near_angle(a.roll, b.roll);
            near_vec(a.up, b.up);
        }
    }
}

#[test]
fn model_filters_pieces_by_allowed_ids_and_entry_state() {
    let model = hybrid_model();
    for piece in model.available_pieces(TrackState::origin()) {
        assert!(model.allowed_pieces.contains(&piece.id));
        assert_eq!(piece.entry, TrackState::origin());
    }
    let mut restricted = model.clone();
    restricted.allowed_pieces = vec![PieceId::Straight];
    assert_eq!(restricted.available_pieces(TrackState::origin()).len(), 1);
    assert!(
        restricted
            .available_pieces(TrackState {
                bank: Bank::Left,
                ..TrackState::origin()
            })
            .is_empty()
    );
}

#[test]
fn four_turns_make_a_closed_circuit() {
    let mut end = TrackState::origin();
    for _ in 0..4 {
        let piece = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::TurnSmall)
            .unwrap()
            .at(end)
            .unwrap();
        end = piece.exit;
    }
    assert_eq!(end, TrackState::origin());
}

#[test]
fn player_facing_names_are_fictional() {
    let model = hybrid_model();
    let text = format!(
        "{} {} {} {}",
        model.manufacturer,
        model.name,
        model.description,
        catalog()
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    for forbidden in [
        "rocky mountain construction",
        "rmc",
        "i-box",
        "steel vengeance",
    ] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn catalog_radius_meets_model_limit() {
    let model = hybrid_model();
    for piece in catalog() {
        let maximum = (0..=2000)
            .map(|i| {
                piece
                    .sample(piece.length() * f64::from(i) / 2000.0)
                    .curvature
            })
            .fold(0.0_f64, f64::max);
        assert!(
            maximum <= 1.0 / model.minimum_radius_metres,
            "{} minimum radius {}",
            piece.name,
            1.0 / maximum
        );
    }
}

#[test]
fn all_eight_headings_place_pieces_on_grid() {
    for heading in [
        Heading::East,
        Heading::NorthEast,
        Heading::North,
        Heading::NorthWest,
        Heading::West,
        Heading::SouthWest,
        Heading::South,
        Heading::SouthEast,
    ] {
        let start = TrackState {
            x: 3,
            y: -7,
            heading,
            ..TrackState::origin()
        };
        let available = hybrid_model().available_pieces(start);
        assert!(!available.is_empty());
        for id in [PieceId::Loop, PieceId::Corkscrew, PieceId::HeartlineRoll] {
            assert!(
                available.iter().any(|piece| piece.id == id),
                "{id:?} {heading:?}"
            );
        }
        for piece in available {
            assert_eq!(piece.entry, start);
            near_vec(piece.sample(0.0).position, piece.entry.position());
            near_vec(piece.sample(piece.length()).position, piece.exit.position());
            near_vec(piece.sample(piece.length()).tangent, piece.exit.tangent());
        }
    }
}

#[test]
fn radius_limit_filters_placed_pieces() {
    let start = TrackState {
        heading: Heading::NorthEast,
        ..TrackState::origin()
    };
    let mut model = hybrid_model();
    model.allowed_pieces = vec![PieceId::Loop];
    assert_eq!(model.available_pieces(start).len(), 1);
    model.minimum_radius_metres = 100.0;
    assert!(model.available_pieces(start).is_empty());
}

#[test]
fn inversions_turn_over_and_are_regular_in_every_heading() {
    for heading in [
        Heading::East,
        Heading::NorthEast,
        Heading::North,
        Heading::NorthWest,
        Heading::West,
        Heading::SouthWest,
        Heading::South,
        Heading::SouthEast,
    ] {
        let start = TrackState {
            heading,
            ..TrackState::origin()
        };
        for id in [PieceId::Loop, PieceId::Corkscrew, PieceId::HeartlineRoll] {
            let piece = catalog()
                .into_iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(start)
                .unwrap();
            let mut previous_roll = piece.sample(0.0).roll;
            for index in 1..=1000 {
                let sample = piece.sample(piece.length() * f64::from(index) / 1000.0);
                assert!(sample.curvature.is_finite());
                near(sample.tangent.length(), 1.0);
                near(sample.up.length(), 1.0);
                assert!((sample.roll - previous_roll).abs() < 0.1);
                previous_roll = sample.roll;
            }
            let middle = piece.sample(piece.length() / 2.0);
            assert!(middle.up.z < -0.5, "{id:?} {heading:?}");
            if id == PieceId::Loop {
                assert!(middle.tangent.dot(start.tangent()) < -0.5);
            } else {
                near_angle(previous_roll, std::f64::consts::TAU);
            }
        }
    }
}
