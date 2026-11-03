use super::*;

#[test]
fn wooden_twister_offers_only_steel_structure_inversions() {
    let model = wooden_twister_model();
    assert_eq!(model.lift_types, vec![LiftType::Chain]);
    assert!(model.launch_types.is_empty());
    assert_eq!(model.track_material, Material::Wood);
    assert_eq!(model.support_style, SupportStyle::DenseWoodenLattice);
    assert_eq!(
        (
            model.train.cars,
            model.train.rows_per_car,
            model.train.seats_per_row
        ),
        (12, 1, 2)
    );
    let choices = model.available_pieces(TrackState::origin());
    assert!(!choices.iter().any(|piece| piece.id == PieceId::Loop));
    let steel_loop = choices
        .iter()
        .find(|piece| piece.id == PieceId::SteelLoop)
        .unwrap();
    assert_eq!(steel_loop.structure_material, Some(Material::Steel));
    assert_eq!(
        model.support_style_for_piece(steel_loop),
        SupportStyle::SteelColumns
    );
    for id in [
        PieceId::TwisterFanLeft,
        PieceId::TwisterFanRight,
        PieceId::AirtimeHill,
    ] {
        assert!(choices.iter().any(|piece| piece.id == id), "{id:?}");
    }
    for name in [model.name, model.manufacturer, model.description] {
        for real_name in [
            "GCI",
            "Great Coasters International",
            "Mystic Timbers",
            "Millennium Flyer",
            "Son of Beast",
        ] {
            assert!(!name.to_lowercase().contains(&real_name.to_lowercase()));
        }
    }
}

#[test]
fn wooden_twister_has_tighter_mirrored_turns_within_radius_limit() {
    let model = wooden_twister_model();
    let choices = model.available_pieces(TrackState::origin());
    let left = choices
        .iter()
        .find(|piece| piece.id == PieceId::TightTwisterLeft)
        .unwrap();
    let right = choices
        .iter()
        .find(|piece| piece.id == PieceId::TightTwisterRight)
        .unwrap();
    assert!(
        left.exit.x
            < catalog()
                .iter()
                .find(|piece| piece.id == PieceId::TurnSmallStandalone)
                .unwrap()
                .exit
                .x
    );
    for piece in [left, right] {
        assert!(1.0 / piece.maximum_curvature() >= model.minimum_radius_metres);
    }
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let a = left.sample(left.length() * fraction);
        let b = right.sample(right.length() * fraction);
        near_vec(
            b.position,
            Vec3::new(a.position.x, -a.position.y, a.position.z),
        );
    }
}

#[test]
fn wooden_twister_banked_helixes_have_mirrored_geometry() {
    let model = wooden_twister_model();
    let pieces = catalog();
    let left = pieces
        .iter()
        .find(|piece| piece.id == PieceId::TwisterHelixLeft)
        .unwrap();
    let right = pieces
        .iter()
        .find(|piece| piece.id == PieceId::TwisterHelixRight)
        .unwrap();
    assert!(
        model
            .available_pieces(left.entry)
            .iter()
            .any(|piece| piece.id == left.id)
    );
    assert!(
        model
            .available_pieces(right.entry)
            .iter()
            .any(|piece| piece.id == right.id)
    );
    assert!(left.exit.z < left.entry.z);
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let a = left.sample(left.length() * fraction);
        let b = right.sample(right.length() * fraction);
        near_vec(
            b.position,
            Vec3::new(a.position.x, -a.position.y, a.position.z),
        );
        near_vec(b.up, Vec3::new(a.up.x, -a.up.y, a.up.z));
    }
}

#[test]
fn multi_launch_model_offers_powered_straights_and_terrain_transitions() {
    let model = multi_launch_model();
    assert_eq!(model.launch_types, vec![LaunchType::LinearMotor]);
    assert!(model.lift_types.is_empty());
    assert_eq!(
        model.train.cars * model.train.rows_per_car * model.train.seats_per_row,
        24
    );
    let choices = model.available_pieces(TrackState::origin());
    for (id, length) in [
        (PieceId::LaunchShort, 8.0),
        (PieceId::Launch, 16.0),
        (PieceId::LaunchLong, 32.0),
    ] {
        let piece = choices.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(piece.kind, PieceKind::Launch);
        assert!((piece.length() - length).abs() < 1e-6);
        assert!(!hybrid_model().allowed_pieces.contains(&id));
    }
    for id in [PieceId::TerrainRise, PieceId::TerrainFall] {
        assert!(choices.iter().any(|piece| piece.id == id));
    }
    assert!(!model.allowed_pieces.contains(&PieceId::Lift));
}

#[test]
fn signature_pieces_are_exclusive_to_their_coaster_models() {
    let hybrid = hybrid_model();
    let multi_launch = multi_launch_model();
    let wooden_twister = wooden_twister_model();

    for piece in [PieceId::HybridOverbankLeft, PieceId::HybridOverbankRight] {
        assert!(hybrid.allowed_pieces.contains(&piece));
        assert!(!multi_launch.allowed_pieces.contains(&piece));
        assert!(!wooden_twister.allowed_pieces.contains(&piece));
    }

    for piece in [PieceId::TwisterFanLeft, PieceId::TwisterFanRight] {
        assert!(wooden_twister.allowed_pieces.contains(&piece));
        assert!(!hybrid.allowed_pieces.contains(&piece));
        assert!(!multi_launch.allowed_pieces.contains(&piece));
    }

    for piece in [PieceId::LaunchShort, PieceId::Launch, PieceId::LaunchLong] {
        assert!(multi_launch.allowed_pieces.contains(&piece));
        assert!(!hybrid.allowed_pieces.contains(&piece));
        assert!(!wooden_twister.allowed_pieces.contains(&piece));
    }

    assert!(wooden_twister.allowed_pieces.contains(&PieceId::SteelLoop));
    assert!(!hybrid.allowed_pieces.contains(&PieceId::SteelLoop));
    assert!(!multi_launch.allowed_pieces.contains(&PieceId::SteelLoop));

    for piece in [
        PieceId::Lift,
        PieceId::LiftFlat,
        PieceId::LiftLevelToGentleUp,
        PieceId::LiftGentleUpToLevel,
    ] {
        assert!(!multi_launch.allowed_pieces.contains(&piece));
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn near_vec(a: Vec3, b: Vec3) {
    near(a.x, b.x);
    near(a.y, b.y);
    near(a.z, b.z);
}
fn near_vec_with_tolerance(a: Vec3, b: Vec3, tolerance: f64) {
    assert!((a.x - b.x).abs() < tolerance, "{a:?} != {b:?}");
    assert!((a.y - b.y).abs() < tolerance, "{a:?} != {b:?}");
    assert!((a.z - b.z).abs() < tolerance, "{a:?} != {b:?}");
}
fn near_angle(a: f64, b: f64) {
    near((a - b).sin(), 0.0);
    near((a - b).cos(), 1.0);
}

#[test]
fn banked_turns_lean_into_their_curves() {
    for piece in catalog().into_iter().filter(|piece| {
        piece.entry.heading != piece.exit.heading && piece.entry.bank != Bank::Level
    }) {
        let distance = piece.length() / 2.0;
        let sample = piece.sample(distance);
        let before = piece.sample(distance - 0.01).tangent;
        let after = piece.sample(distance + 0.01).tangent;
        let signed_curvature =
            sample.tangent.x * (after.y - before.y) - sample.tangent.y * (after.x - before.x);
        let left = Vec3::new(-sample.tangent.y, sample.tangent.x, 0.0).unit();
        assert!(
            sample.up.dot(left) * signed_curvature > 0.0,
            "{}",
            piece.name
        );
        assert!(sample.roll * signed_curvature > 0.0, "{}", piece.name);
    }
}

#[test]
fn every_turn_and_offset_has_a_mirrored_catalog_piece() {
    let pieces = catalog();
    for piece in pieces
        .iter()
        .filter(|piece| piece.entry.heading != piece.exit.heading || piece.exit.y != 0)
    {
        let mirrored = pieces
            .iter()
            .find(|candidate| mirrored_geometry_matches(piece, candidate))
            .unwrap_or_else(|| panic!("missing mirror for {}", piece.name));
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let a = piece.sample(piece.length() * fraction);
            let b = mirrored.sample(mirrored.length() * fraction);
            near_vec(
                b.position,
                Vec3::new(a.position.x, -a.position.y, a.position.z),
            );
            near_vec(b.up, Vec3::new(a.up.x, -a.up.y, a.up.z));
            near(b.roll, -a.roll);
        }
    }
}

fn mirrored_geometry_matches(piece: &Piece, candidate: &Piece) -> bool {
    if candidate.entry.x != piece.entry.x
        || candidate.entry.y != -piece.entry.y
        || candidate.entry.z != piece.entry.z
        || candidate.entry.heading.index() != (-piece.entry.heading.index()).rem_euclid(8)
        || candidate.entry.slope != piece.entry.slope
        || candidate.entry.bank.radians() != -piece.entry.bank.radians()
        || candidate.exit.x != piece.exit.x
        || candidate.exit.y != -piece.exit.y
        || candidate.exit.z != piece.exit.z
        || candidate.exit.heading.index() != (-piece.exit.heading.index()).rem_euclid(8)
        || candidate.exit.slope != piece.exit.slope
        || candidate.exit.bank.radians() != -piece.exit.bank.radians()
        || (candidate.entry.curvature_vector().y + piece.entry.curvature_vector().y).abs() >= 1e-10
        || (candidate.exit.curvature_vector().x - piece.exit.curvature_vector().x).abs() >= 1e-10
        || (candidate.exit.curvature_vector().y + piece.exit.curvature_vector().y).abs() >= 1e-10
    {
        return false;
    }

    [0.0, 0.25, 0.5, 0.75, 1.0].into_iter().all(|fraction| {
        let original = piece.sample(piece.length() * fraction);
        let mirrored = candidate.sample(candidate.length() * fraction);
        let expected_position = Vec3::new(
            original.position.x,
            -original.position.y,
            original.position.z,
        );
        let expected_up = Vec3::new(original.up.x, -original.up.y, original.up.z);
        (mirrored.position - expected_position).length() < 1e-8
            && (mirrored.up - expected_up).length() < 1e-8
            && (mirrored.roll + original.roll).sin().abs() < 1e-8
            && (mirrored.roll + original.roll).cos() > 1.0 - 1e-8
    })
}

#[test]
fn hybrid_overbanks_exceed_ninety_degrees_mirror_and_join_smoothly() {
    let pieces = catalog();
    let left = pieces
        .iter()
        .find(|piece| piece.id == PieceId::HybridOverbankLeft)
        .unwrap();
    let right = pieces
        .iter()
        .find(|piece| piece.id == PieceId::HybridOverbankRight)
        .unwrap();
    let largest_roll = (0..=100)
        .map(|index| {
            left.sample(left.length() * (f64::from(index) / 100.0))
                .roll
                .abs()
        })
        .fold(0.0, f64::max);
    assert!(largest_roll > std::f64::consts::FRAC_PI_2);

    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let a = left.sample(left.length() * fraction);
        let b = right.sample(right.length() * fraction);
        near_vec(
            b.position,
            Vec3::new(a.position.x, -a.position.y, a.position.z),
        );
        near_vec(b.up, Vec3::new(a.up.x, -a.up.y, a.up.z));
        near(b.roll, -a.roll);
    }

    let following_pieces: Vec<_> = pieces
        .iter()
        .filter_map(|template| template.at(left.exit))
        .collect();
    assert!(!following_pieces.is_empty());
    for following_piece in &following_pieces {
        let incoming = left.sample(left.length());
        let outgoing = following_piece.sample(0.0);
        near_vec(incoming.position, outgoing.position);
        near_vec(incoming.tangent, outgoing.tangent);
        near_vec(incoming.up, outgoing.up);
        near_angle(incoming.roll, outgoing.roll);
        near(incoming.curvature, outgoing.curvature);
    }
}

#[test]
fn handed_inversions_are_mirrored() {
    let pieces = catalog();
    for (left_id, right_id) in [
        (PieceId::Corkscrew, PieceId::CorkscrewRight),
        (PieceId::HeartlineRoll, PieceId::HeartlineRollRight),
    ] {
        let left = pieces.iter().find(|piece| piece.id == left_id).unwrap();
        let right = pieces.iter().find(|piece| piece.id == right_id).unwrap();
        assert_eq!(left.entry, right.entry);
        assert_eq!(left.exit, right.exit);
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let a = left.sample(left.length() * fraction);
            let b = right.sample(right.length() * fraction);
            near_vec(
                b.position,
                Vec3::new(a.position.x, -a.position.y, a.position.z),
            );
            near_vec(b.up, Vec3::new(a.up.x, -a.up.y, a.up.z));
            near(b.roll, -a.roll);
        }
    }
}

fn assert_curvature_continuous_at_join(first: &Piece, second: &Piece) {
    let join_distance = 0.05;
    let incoming = first.sample(first.length());
    let outgoing = second.sample(0.0);
    near_vec(incoming.position, outgoing.position);
    near_vec(incoming.tangent, outgoing.tangent);
    near_vec(incoming.up, outgoing.up);
    near_angle(incoming.roll, outgoing.roll);
    near(incoming.curvature, outgoing.curvature);
    assert!(
        incoming.curvature > 1e-6,
        "join curvature should be nonzero"
    );

    let incoming_curvature_vector =
        (incoming.tangent - first.sample(first.length() - join_distance).tangent) / join_distance;
    let outgoing_curvature_vector =
        (second.sample(join_distance).tangent - outgoing.tangent) / join_distance;
    near_vec_with_tolerance(incoming_curvature_vector, outgoing_curvature_vector, 1e-2);
}

fn place_curvature_sequence(ids: &[PieceId], initial_state: TrackState) -> Vec<Piece> {
    let templates = catalog();
    let mut end = initial_state;
    let mut pieces = Vec::with_capacity(ids.len());

    for id in ids {
        let template = templates
            .iter()
            .find(|piece| piece.id == *id)
            .unwrap_or_else(|| panic!("missing piece {id:?}"));
        let piece = template
            .at(end)
            .unwrap_or_else(|| panic!("{id:?} rejected compatible state {end:?}"));
        end = piece.exit;
        pieces.push(piece);
    }

    pieces
}

fn assert_curvature_sequence(pieces: &[Piece]) -> TrackState {
    for pair in pieces.windows(2) {
        assert_curvature_continuous_at_join(&pair[0], &pair[1]);
    }
    pieces.last().expect("nonempty piece sequence").exit
}

fn assert_vertical_curvature_sign(pieces: &[Piece], expected_sign: f64) {
    let mut total_tangent_z_change = 0.0;
    for piece in pieces {
        let mut previous_tangent_z = piece.sample(0.0).tangent.z;
        for index in 1..=100 {
            let distance = piece.length() * (f64::from(index) / 100.0);
            let tangent_z = piece.sample(distance).tangent.z;
            let tangent_z_change = tangent_z - previous_tangent_z;
            assert!(
                tangent_z_change * expected_sign >= -1e-4,
                "{} {:?} has vertical curvature change {tangent_z_change} with the wrong sign at distance {distance}",
                piece.name,
                piece.entry.heading
            );
            total_tangent_z_change += tangent_z_change;
            previous_tangent_z = tangent_z;
        }
    }
    assert!(total_tangent_z_change * expected_sign > 1e-3);
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
fn joined_pieces_have_matching_position_tangent_roll_and_curvature() {
    let templates = catalog();
    for first in &templates {
        for template in &templates {
            let Some(second) = template.at(first.exit) else {
                continue;
            };
            assert_eq!(second.entry, first.exit);
            let a = first.sample(first.length());
            let b = second.sample(0.0);
            near_vec(a.position, b.position);
            near_vec(a.tangent, b.tangent);
            near_angle(a.roll, b.roll);
            near_vec(a.up, b.up);
            near(a.curvature, b.curvature);
        }
    }
}

#[test]
fn large_turn_segments_match_curvature_vectors_at_rotated_joins() {
    let templates = catalog();
    let find_template = |id| {
        templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap_or_else(|| panic!("missing piece {id:?}"))
    };
    let entry_template = find_template(PieceId::TurnLargeEntry);
    let turn_ids = [
        PieceId::TurnLargeEntry,
        PieceId::TurnLarge,
        PieceId::TurnLargeExit,
    ];

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
        let initial_state = TrackState {
            x: 3,
            y: -7,
            z: 2,
            heading,
            slope: entry_template.entry.slope,
            bank: entry_template.entry.bank,
            ..TrackState::origin()
        };
        let pieces = place_curvature_sequence(&turn_ids, initial_state);
        assert_curvature_sequence(&pieces);
    }
}

#[test]
fn vertical_curve_segments_match_curvature_vectors_at_rotated_joins() {
    let templates = catalog();
    let find_template = |id| {
        templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap_or_else(|| panic!("missing piece {id:?}"))
    };
    let upward_entry = find_template(PieceId::VerticalUpEntry);
    let downward_entry = find_template(PieceId::VerticalDownEntry);

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
        let upward_pieces = place_curvature_sequence(
            &[
                PieceId::VerticalUpEntry,
                PieceId::VerticalUpContinue,
                PieceId::VerticalUpExit,
            ],
            TrackState {
                x: 3,
                y: -7,
                z: 2,
                heading,
                ..upward_entry.entry
            },
        );
        let upward_end = assert_curvature_sequence(&upward_pieces);
        assert_eq!(upward_end.vertical_curvature, VerticalCurvature::Straight);
        assert_vertical_curvature_sign(&upward_pieces, 1.0);

        let downward_pieces = place_curvature_sequence(
            &[
                PieceId::VerticalDownEntry,
                PieceId::VerticalDownContinue,
                PieceId::VerticalDownExit,
            ],
            TrackState {
                x: 3,
                y: -7,
                z: 2,
                heading,
                ..downward_entry.entry
            },
        );
        let downward_end = assert_curvature_sequence(&downward_pieces);
        assert_eq!(downward_end.vertical_curvature, VerticalCurvature::Straight);
        assert_vertical_curvature_sign(&downward_pieces, -1.0);
    }
}

#[test]
fn pieces_reject_states_with_incompatible_slope_or_bank() {
    let straight = catalog()
        .into_iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    assert!(
        straight
            .at(TrackState {
                slope: Slope::UpGentle,
                ..TrackState::origin()
            })
            .is_none()
    );
    assert!(
        straight
            .at(TrackState {
                bank: Bank::Left,
                ..TrackState::origin()
            })
            .is_none()
    );
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
        for id in [
            PieceId::TurnSmallEntry,
            PieceId::TurnSmall,
            PieceId::TurnSmall,
            PieceId::TurnSmallExit,
        ] {
            let piece = catalog()
                .into_iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(end)
                .unwrap();
            end = piece.exit;
        }
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

#[test]
fn model_offers_connected_turns_for_each_radius_and_bank() {
    use Bank::{Left, Level, Right};
    use PieceId::*;

    let model = hybrid_model();
    for (bank, radius, ids) in [
        (Level, 16.0, [TurnSmallEntry, TurnSmall, TurnSmallExit]),
        (Level, 32.0, [TurnLargeEntry, TurnLarge, TurnLargeExit]),
        (Left, 16.0, [BankedTurnEntry, BankedTurn, BankedTurnExit]),
        (
            Left,
            32.0,
            [
                LargeLeftBankedTurnEntry,
                LargeLeftBankedTurn,
                LargeLeftBankedTurnExit,
            ],
        ),
        (
            Right,
            16.0,
            [
                SmallRightBankedTurnEntry,
                SmallRightBankedTurn,
                SmallRightBankedTurnExit,
            ],
        ),
        (
            Right,
            32.0,
            [
                LargeRightBankedTurnEntry,
                LargeRightBankedTurn,
                LargeRightBankedTurnExit,
            ],
        ),
    ] {
        let mut state = TrackState {
            bank,
            ..TrackState::origin()
        };
        let mut placed = Vec::new();
        for id in ids {
            let piece = model
                .available_pieces(state)
                .into_iter()
                .find(|piece| piece.id == id)
                .unwrap_or_else(|| panic!("model did not offer {id:?} at {state:?}"));
            assert_eq!(piece.entry, state);
            assert_eq!(piece.exit.bank, bank);
            state = piece.exit;
            placed.push(piece);
        }
        assert_eq!(state.horizontal_curvature, HorizontalCurvature::Straight);
        let curvature = placed[1].sample(placed[1].length() / 2.0).curvature;
        assert!((curvature - 1.0 / radius).abs() < 1e-4);
        assert_curvature_sequence(&placed);
    }
}

#[test]
fn model_offers_right_bank_and_level_transition() {
    let model = hybrid_model();
    let bank = model
        .available_pieces(TrackState::origin())
        .into_iter()
        .find(|piece| piece.id == PieceId::BankRight)
        .expect("right bank should be available from level track");
    assert_eq!(bank.exit.bank, Bank::Right);
    let unbank = model
        .available_pieces(bank.exit)
        .into_iter()
        .find(|piece| piece.id == PieceId::UnbankRight)
        .expect("level track should be available from right bank");
    assert_eq!(unbank.exit.bank, Bank::Level);
}

#[test]
fn curvature_rate_approaches_zero_at_representative_joins() {
    use PieceId::*;
    for ids in [
        [Straight, Straight, Straight],
        [TurnSmallEntry, TurnSmall, TurnSmallExit],
        [VerticalUpEntry, VerticalUpContinue, VerticalUpExit],
        [Straight, Loop, Straight],
    ] {
        let pieces = place_curvature_sequence(&ids, TrackState::origin());
        for pair in pieces.windows(2) {
            let distance = 0.01;
            let incoming_end = pair[0].sample(pair[0].length()).curvature;
            let incoming_before = pair[0].sample(pair[0].length() - distance).curvature;
            let outgoing_start = pair[1].sample(0.0).curvature;
            let outgoing_after = pair[1].sample(distance).curvature;
            let incoming_rate = (incoming_end - incoming_before) / distance;
            let outgoing_rate = (outgoing_after - outgoing_start) / distance;
            assert!(
                incoming_rate.abs() < 1e-3,
                "{:?} incoming rate {incoming_rate}",
                pair[0].id
            );
            assert!(
                outgoing_rate.abs() < 1e-3,
                "{:?} outgoing rate {outgoing_rate}",
                pair[1].id
            );
        }
    }
}

#[test]
fn standalone_turns_connect_straight_curvature_at_both_ends() {
    use PieceId::*;
    let model = hybrid_model();
    for (bank, id) in [
        (Bank::Level, TurnSmallStandalone),
        (Bank::Level, TurnLargeStandalone),
        (Bank::Left, BankedTurnStandalone),
        (Bank::Left, LargeLeftBankedTurnStandalone),
        (Bank::Right, SmallRightBankedTurnStandalone),
        (Bank::Right, LargeRightBankedTurnStandalone),
    ] {
        let start = TrackState {
            bank,
            ..TrackState::origin()
        };
        let piece = model
            .available_pieces(start)
            .into_iter()
            .find(|piece| piece.id == id)
            .unwrap_or_else(|| panic!("model did not offer {id:?}"));
        assert_eq!(
            piece.exit.heading,
            if bank == Bank::Right {
                Heading::South
            } else {
                Heading::North
            }
        );
        assert_eq!(piece.exit.bank, bank);
        assert_eq!(
            piece.exit.horizontal_curvature,
            HorizontalCurvature::Straight
        );
        near(piece.sample(0.0).curvature, 0.0);
        near(piece.sample(piece.length()).curvature, 0.0);
    }
}

#[test]
fn powered_lift_pieces_match_unpowered_geometry_and_are_available() {
    let model = hybrid_model();
    let templates = catalog();
    for (powered_id, unpowered_id) in [
        (PieceId::LiftFlat, PieceId::Straight),
        (PieceId::LiftLevelToGentleUp, PieceId::LevelToGentleUp),
        (PieceId::LiftGentleUpToLevel, PieceId::GentleUpToLevel),
        (PieceId::Lift, PieceId::GentleUp),
    ] {
        let powered = templates
            .iter()
            .find(|piece| piece.id == powered_id)
            .unwrap();
        let unpowered = templates
            .iter()
            .find(|piece| piece.id == unpowered_id)
            .unwrap();
        assert_eq!(powered.kind, PieceKind::Lift);
        assert_eq!(powered.entry, unpowered.entry);
        assert_eq!(powered.exit, unpowered.exit);
        near(powered.length(), unpowered.length());
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let powered_sample = powered.sample(powered.length() * fraction);
            let unpowered_sample = unpowered.sample(unpowered.length() * fraction);
            near_vec(powered_sample.position, unpowered_sample.position);
            near_vec(powered_sample.tangent, unpowered_sample.tangent);
        }
        assert!(
            model
                .available_pieces(powered.entry)
                .iter()
                .any(|piece| piece.id == powered_id)
        );
    }
}

#[test]
fn coaster_models_have_a_minimum_rider_height() {
    assert_eq!(super::hybrid_model().minimum_height_cm, 122);
    assert_eq!(super::multi_launch_model().minimum_height_cm, 132);
}
