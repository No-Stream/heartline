use super::*;

#[test]
fn catalog_returns_the_same_static_template_slice() {
    let first = catalog();
    let second = catalog();

    assert_eq!(first.as_ptr(), second.as_ptr());
    assert_eq!(first.len(), second.len());
}

#[test]
fn speed_matched_turn_catalog_banks_for_each_design_speed() {
    let expected_turns = [
        (PieceId::SpeedTurnLarge16Left, 32, Hand::Left, 16.0),
        (PieceId::SpeedTurnLarge20Left, 32, Hand::Left, 20.0),
        (PieceId::SpeedTurnLarge24Left, 32, Hand::Left, 24.0),
        (PieceId::SpeedTurnLarge16Right, 32, Hand::Right, 16.0),
        (PieceId::SpeedTurnLarge20Right, 32, Hand::Right, 20.0),
        (PieceId::SpeedTurnLarge24Right, 32, Hand::Right, 24.0),
    ];
    let templates = catalog();
    let speed_turns: Vec<_> = templates
        .iter()
        .filter(|piece| matches!(piece.family, Some(PieceFamily::SpeedTurn { .. })))
        .collect();
    assert_eq!(speed_turns.len(), expected_turns.len());
    let models = [wooden_twister_model(), hybrid_model(), multi_launch_model()];
    let mut bank_measurements = Vec::with_capacity(expected_turns.len());

    for (id, expected_radius, expected_hand, design_speed) in expected_turns {
        let piece = templates
            .iter()
            .find(|piece| piece.id == id)
            .unwrap_or_else(|| panic!("catalog is missing {id:?}"));
        let Some(PieceFamily::SpeedTurn {
            radius_metres,
            hand,
            design_speed_metres_per_second,
        }) = piece.family
        else {
            panic!("{id:?} should have speed-turn family metadata");
        };
        assert_eq!(radius_metres, expected_radius, "{id:?}");
        assert_eq!(hand, expected_hand, "{id:?}");
        near(design_speed_metres_per_second, design_speed);
        let hand_name = match hand {
            Hand::Left => "left",
            Hand::Right => "right",
        };
        let radius_name = if radius_metres == 32 {
            "Large"
        } else {
            "Small"
        };
        assert_eq!(
            piece.name,
            format!("{radius_name} {hand_name} turn banked for {design_speed:.0} m/s"),
            "{id:?} name"
        );
        assert!(
            matches!(
                piece.shape,
                Shape::SpeedBank {
                    design_speed: shape_speed
                } if shape_speed == design_speed
            ),
            "{id:?} shape"
        );
        assert_eq!(piece.entry.slope, Slope::Level, "{id:?} entry slope");
        assert_eq!(piece.entry.bank, Bank::Level, "{id:?} entry bank");
        assert_eq!(piece.exit.slope, Slope::Level, "{id:?} exit slope");
        assert_eq!(piece.exit.bank, Bank::Level, "{id:?} exit bank");
        let rated = piece.rated_speeds.expect("speed turns are rated");
        near(rated.minimum_entry, design_speed - 3.0);
        near(rated.maximum_entry, design_speed + 3.0);

        let mut peak_bank_radians: f64 = 0.0;
        let heart_curvature_step = piece.length() / 100.0;
        for index in 0..=100 {
            let distance = if index == 100 {
                piece.length()
            } else {
                piece.length() * f64::from(index) / 100.0
            };
            let sample = piece.sample(distance);
            if (1..100).contains(&index) {
                let before = piece.sample(distance - heart_curvature_step).heart;
                let after = piece.sample(distance + heart_curvature_step).heart;
                let heart_first = (after - before) / (2.0 * heart_curvature_step);
                let heart_second =
                    (before - sample.heart * 2.0 + after) / heart_curvature_step.powi(2);
                let heart_horizontal_speed = (heart_first.x.powi(2) + heart_first.y.powi(2)).sqrt();
                let heart_signed_curvature = (heart_first.x * heart_second.y
                    - heart_first.y * heart_second.x)
                    / heart_horizontal_speed.powi(3);
                let expected_bank = (design_speed.powi(2) * heart_signed_curvature
                    / crate::physics::GRAVITY)
                    .atan();
                assert!(
                    (sample.roll - expected_bank).abs() < 0.002,
                    "{id:?} bank should follow heart-path curvature at {distance:.2} m: {:.6} vs {:.6} rad",
                    sample.roll,
                    expected_bank
                );
            }
            peak_bank_radians = peak_bank_radians.max(sample.roll.abs());
        }
        let peak_bank_degrees = peak_bank_radians.to_degrees();
        let allowed_by_any_model = models.iter().any(|model| model.allows_piece(piece));
        eprintln!(
            "{id:?}: peak design bank {peak_bank_degrees:.2} degrees, allowed by any model: {allowed_by_any_model}"
        );
        bank_measurements.push((id, peak_bank_degrees, allowed_by_any_model));
    }
    let overbanked_allowed_turns: Vec<_> = bank_measurements
        .into_iter()
        .filter(|(_, peak_bank_degrees, allowed)| *allowed && *peak_bank_degrees > 75.0)
        .collect();
    assert!(
        overbanked_allowed_turns.is_empty(),
        "allowed speed turns exceed the 75-degree bank cap: {overbanked_allowed_turns:?}"
    );
}

#[test]
fn speed_matched_turns_mirror_and_join_every_compatible_piece() {
    let templates = catalog();
    for (left_id, right_id) in [
        (
            PieceId::SpeedTurnLarge16Left,
            PieceId::SpeedTurnLarge16Right,
        ),
        (
            PieceId::SpeedTurnLarge20Left,
            PieceId::SpeedTurnLarge20Right,
        ),
        (
            PieceId::SpeedTurnLarge24Left,
            PieceId::SpeedTurnLarge24Right,
        ),
    ] {
        let left = templates.iter().find(|piece| piece.id == left_id).unwrap();
        let right = templates.iter().find(|piece| piece.id == right_id).unwrap();
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let left_sample = left.sample(left.length() * fraction);
            let right_sample = right.sample(right.length() * fraction);
            near_vec(
                right_sample.position,
                Vec3::new(
                    left_sample.position.x,
                    -left_sample.position.y,
                    left_sample.position.z,
                ),
            );
            near_vec(
                right_sample.tangent,
                Vec3::new(
                    left_sample.tangent.x,
                    -left_sample.tangent.y,
                    left_sample.tangent.z,
                ),
            );
            near_vec(
                right_sample.up,
                Vec3::new(left_sample.up.x, -left_sample.up.y, left_sample.up.z),
            );
            near(right_sample.roll, -left_sample.roll);
        }

        for turn in [left, right] {
            for heading_offset in [-1, 1] {
                let diagonal_entry = TrackState {
                    heading: Heading::from_index(turn.entry.heading.index() + heading_offset),
                    ..turn.entry
                };
                let diagonal_turn = turn.at(diagonal_entry).unwrap_or_else(|| {
                    panic!("{:?} cannot be placed at {diagonal_entry:?}", turn.id)
                });
                let incoming = diagonal_turn.sample(0.0);
                let outgoing = diagonal_turn.sample(diagonal_turn.length());
                near_vec(incoming.position, diagonal_entry.position());
                near_vec(incoming.tangent, diagonal_entry.tangent());
                near_vec(outgoing.position, diagonal_turn.exit.position());
                near_vec(outgoing.tangent, diagonal_turn.exit.tangent());
            }

            let following_pieces: Vec<_> = templates
                .iter()
                .filter_map(|template| template.at(turn.exit))
                .collect();
            assert!(
                !following_pieces.is_empty(),
                "{:?} has no continuation",
                turn.id
            );
            for following_piece in following_pieces {
                let incoming = turn.sample(turn.length());
                let outgoing = following_piece.sample(0.0);
                near_vec(incoming.position, outgoing.position);
                near_vec(incoming.tangent, outgoing.tangent);
                near_vec(incoming.up, outgoing.up);
                near(incoming.roll, outgoing.roll);
                near(incoming.curvature, outgoing.curvature);
            }
        }
    }
}

#[test]
fn speed_matched_turn_roll_acceleration_is_continuous_at_connectors_and_center() {
    let turns: Vec<_> = catalog()
        .iter()
        .filter(|piece| matches!(piece.family, Some(PieceFamily::SpeedTurn { .. })))
        .collect();
    assert_eq!(turns.len(), 6);

    let connector_step_metres = 0.05;
    let center_step_metres = 0.05;
    for piece in turns {
        let roll_at = |distance: f64| piece.sample(distance).roll;
        let start_roll = roll_at(0.0);
        let start_acceleration_at_step = (roll_at(2.0 * connector_step_metres)
            - 2.0 * roll_at(connector_step_metres)
            + start_roll)
            / connector_step_metres.powi(2);
        let half_connector_step = connector_step_metres / 2.0;
        let start_acceleration_at_half_step =
            (roll_at(connector_step_metres) - 2.0 * roll_at(half_connector_step) + start_roll)
                / half_connector_step.powi(2);
        assert!(
            start_acceleration_at_half_step.abs() <= start_acceleration_at_step.abs() * 0.75 + 1e-4,
            "{:?} entry roll acceleration does not approach the level connector smoothly: {:.6} then {:.6} rad/m²",
            piece.id,
            start_acceleration_at_step,
            start_acceleration_at_half_step
        );

        let end_roll = roll_at(piece.length());
        let end_acceleration_at_step = (end_roll
            - 2.0 * roll_at(piece.length() - connector_step_metres)
            + roll_at(piece.length() - 2.0 * connector_step_metres))
            / connector_step_metres.powi(2);
        let end_acceleration_at_half_step = (end_roll
            - 2.0 * roll_at(piece.length() - half_connector_step)
            + roll_at(piece.length() - connector_step_metres))
            / half_connector_step.powi(2);
        assert!(
            end_acceleration_at_half_step.abs() <= end_acceleration_at_step.abs() * 0.75 + 1e-4,
            "{:?} exit roll acceleration does not approach the level connector smoothly: {:.6} then {:.6} rad/m²",
            piece.id,
            end_acceleration_at_step,
            end_acceleration_at_half_step
        );

        let center = piece.length() / 2.0;
        let left_center_acceleration = (roll_at(center)
            - 2.0 * roll_at(center - center_step_metres)
            + roll_at(center - 2.0 * center_step_metres))
            / center_step_metres.powi(2);
        let right_center_acceleration = (roll_at(center + 2.0 * center_step_metres)
            - 2.0 * roll_at(center + center_step_metres)
            + roll_at(center))
            / center_step_metres.powi(2);
        assert!(
            (left_center_acceleration - right_center_acceleration).abs() <= 0.01,
            "{:?} roll acceleration is discontinuous at the turn center: left {:.6}, right {:.6} rad/m²",
            piece.id,
            left_center_acceleration,
            right_center_acceleration
        );
    }
}

#[test]
fn speed_matched_turns_keep_the_heart_on_a_planar_path_with_a_steady_center() {
    let turns: Vec<_> = catalog()
        .iter()
        .filter(|piece| matches!(piece.family, Some(PieceFamily::SpeedTurn { .. })))
        .collect();
    assert!(!turns.is_empty());

    for piece in turns {
        let Some(PieceFamily::SpeedTurn { hand, .. }) = piece.family else {
            unreachable!("filtered speed-turn piece lost its family metadata");
        };
        let length = piece.length();
        let heart_height = piece.sample(0.0).heart.z;
        for index in 0..=100 {
            let distance = if index == 100 {
                length
            } else {
                length * f64::from(index) / 100.0
            };
            let heart = piece.sample(distance).heart;
            assert!(
                (heart.z - heart_height).abs() <= 1e-8,
                "{:?} rider heart leaves its horizontal path at {distance:.3} m: z {:.9} vs {:.9}",
                piece.id,
                heart.z,
                heart_height
            );
        }

        let heart_tangent_at = |distance: f64| {
            let step = length / 1000.0;
            let before_distance = (distance - step).max(0.0);
            let after_distance = (distance + step).min(length);
            let before = piece.sample(before_distance).heart;
            let after = piece.sample(after_distance).heart;
            Vec3::new(after.x - before.x, after.y - before.y, 0.0).unit()
        };
        let entry_tangent = piece.entry.tangent().unit();
        let exit_tangent = piece.exit.tangent().unit();
        let hand_sign = if hand == Hand::Left { 1.0 } else { -1.0 };
        let signed_turn = |from: Vec3, to: Vec3| {
            (from.x * to.y - from.y * to.x).atan2(from.x * to.x + from.y * to.y)
        };
        let expected_turn = hand_sign * signed_turn(entry_tangent, exit_tangent);
        assert!(
            expected_turn > 0.0,
            "{:?} connectors contradict its hand",
            piece.id
        );
        assert!(heart_tangent_at(0.0).dot(entry_tangent) > 0.9999);
        assert!(heart_tangent_at(length).dot(exit_tangent) > 0.9999);

        let mut previous_turn = 0.0;
        for index in 0..=100 {
            let distance = if index == 100 {
                length
            } else {
                length * f64::from(index) / 100.0
            };
            let directed_turn = hand_sign * signed_turn(entry_tangent, heart_tangent_at(distance));
            assert!(
                directed_turn >= previous_turn - 1e-4,
                "{:?} rider-heart heading reverses at {distance:.3} m: {:.6} after {:.6} radians",
                piece.id,
                directed_turn,
                previous_turn
            );
            assert!(
                directed_turn >= -1e-4 && directed_turn <= expected_turn + 1e-4,
                "{:?} rider-heart heading leaves the intended turn at {distance:.3} m: {:.6} of {:.6} radians",
                piece.id,
                directed_turn,
                expected_turn
            );
            previous_turn = directed_turn;
        }

        let steady_bank_samples =
            [0.45, 0.475, 0.5, 0.525, 0.55].map(|fraction| piece.sample(length * fraction).roll);
        let minimum_steady_bank = steady_bank_samples
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let maximum_steady_bank = steady_bank_samples
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            hand_sign * piece.sample(length / 2.0).roll > 0.1,
            "{:?} steady bank does not turn with the rider-heart path",
            piece.id
        );
        assert!(
            maximum_steady_bank - minimum_steady_bank <= 0.001,
            "{:?} has no steady bank across the center section: {:?}",
            piece.id,
            steady_bank_samples
        );
    }
}

#[test]
fn airtime_rating_matches_the_force_overlay_bands() {
    use super::models::AirtimeRating;

    let steel_limits = hybrid_model().comfortable_force_limits;
    assert_eq!(steel_limits.classify_airtime(-0.2), AirtimeRating::Weak);
    assert_eq!(
        steel_limits.classify_airtime(-0.200_001),
        AirtimeRating::Good
    );
    assert_eq!(steel_limits.classify_airtime(-0.9), AirtimeRating::Good);
    assert_eq!(
        steel_limits.classify_airtime(-0.900_001),
        AirtimeRating::Great
    );
    assert_eq!(steel_limits.classify_airtime(-1.5), AirtimeRating::Great);
    assert_eq!(
        steel_limits.classify_airtime(-1.500_001),
        AirtimeRating::TooMuch
    );
    assert_eq!(
        steel_limits.classify_airtime(-1.3 * -steel_limits.minimum_vertical_g),
        AirtimeRating::TooMuch
    );
    assert_eq!(
        steel_limits.classify_airtime(-1.300_001 * -steel_limits.minimum_vertical_g),
        AirtimeRating::Dangerous
    );
}

#[test]
fn outer_banks_cover_each_steel_airtime_hill_and_exclude_wood() {
    let steel_models = [hybrid_model(), multi_launch_model()];
    let wood = wooden_twister_model();
    let upright_hills: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            piece.family.is_some_and(|family| {
                matches!(family, PieceFamily::Hill(family) if
                    matches!(family.feel, HillFeel::Ejector | HillFeel::Floater)
                        && family.twist.is_none()
                )
            }) && steel_models.iter().all(|model| model.allows_piece(piece))
        })
        .collect();
    assert_eq!(upright_hills.len(), 11);

    let outer_banks: Vec<_> = catalog()
        .iter()
        .filter(|piece| {
            piece.family.is_some_and(|family| {
                matches!(family, PieceFamily::Hill(family) if
                    matches!(
                        family.twist,
                        Some(HillTwist::OuterBankLeft | HillTwist::OuterBankRight)
                    )
                )
            })
        })
        .collect();
    assert_eq!(outer_banks.len(), upright_hills.len() * 2);

    for upright in upright_hills {
        let Some(PieceFamily::Hill(family)) = upright.family else {
            panic!("{} should be a hill family", upright.name);
        };
        for twist in [HillTwist::OuterBankLeft, HillTwist::OuterBankRight] {
            let matching: Vec<_> = outer_banks
                .iter()
                .filter(|piece| {
                    let Some(PieceFamily::Hill(candidate)) = piece.family else {
                        return false;
                    };
                    candidate.feel == family.feel
                        && candidate.height_metres == family.height_metres
                        && candidate.span_metres == family.span_metres
                        && candidate.twist == Some(twist)
                })
                .collect();
            assert_eq!(matching.len(), 1, "{} {twist:?}", upright.name);
            let piece = matching[0];
            assert_eq!(piece.heartlining, Some(Heartlining::Heartlined));
            assert_eq!(piece.inversion, None);
            assert!(piece.rated_speeds.is_some(), "{}", piece.name);
            assert!(!wood.allows_piece(piece), "{}", piece.name);
            for model in &steel_models {
                assert!(model.allows_piece(piece), "{} {}", model.name, piece.name);
            }
        }
    }
}

#[test]
fn zero_g_rolls_and_stall_are_handed_steel_inversions() {
    let steel_models = [hybrid_model(), multi_launch_model()];
    let wood = wooden_twister_model();
    for (feel, size_count) in [(HillFeel::ZeroGRoll, 2), (HillFeel::Stall, 1)] {
        let pieces: Vec<_> = catalog()
            .iter()
            .filter(|piece| matches!(piece.family, Some(PieceFamily::Hill(family)) if family.feel == feel))
            .collect();
        assert_eq!(pieces.len(), 2 * size_count, "{feel:?}");
        let mut dimensions: Vec<_> = pieces
            .iter()
            .map(|piece| {
                let Some(PieceFamily::Hill(family)) = piece.family else {
                    panic!("{} should be a hill family", piece.name);
                };
                (family.height_metres, family.span_metres)
            })
            .collect();
        dimensions.sort_unstable();
        dimensions.dedup();
        assert_eq!(dimensions.len(), size_count, "{feel:?}");
        for (height, span) in dimensions {
            for twist in [HillTwist::LeftRoll, HillTwist::RightRoll] {
                let matching: Vec<_> = pieces
                    .iter()
                    .filter(|piece| {
                        let Some(PieceFamily::Hill(family)) = piece.family else {
                            return false;
                        };
                        family.height_metres == height
                            && family.span_metres == span
                            && family.twist == Some(twist)
                    })
                    .collect();
                assert_eq!(matching.len(), 1, "{feel:?} {height} x {span} {twist:?}");
                let piece = matching[0];
                assert_eq!(piece.heartlining, Some(Heartlining::Heartlined));
                assert_eq!(piece.inversion, Some(Inversion::HeartlineRoll));
                assert!(piece.rated_speeds.is_some(), "{}", piece.name);
                assert!(!wood.allows_piece(piece), "{}", piece.name);
                for model in &steel_models {
                    assert!(model.allows_piece(piece), "{} {}", model.name, piece.name);
                }
            }
        }
    }
}

#[test]
fn straight_hills_are_rated_profiles_allowed_by_expected_models_and_reachable() {
    let expected_hills: [(&str, &[&str]); 15] = [
        (
            "Ejector hill 8 × 48 m",
            &["Timber Tumbler", "Velocity Viper"],
        ),
        (
            "Ejector hill 12 × 64 m",
            &["Timber Tumbler", "Velocity Viper"],
        ),
        (
            "Ejector hill 8 × 64 m",
            &["Timber Tumbler", "Velocity Viper"],
        ),
        (
            "Ejector hill 12 × 80 m",
            &["Timber Tumbler", "Velocity Viper"],
        ),
        (
            "Ejector hill 16 × 96 m",
            &["Timber Tumbler", "Velocity Viper"],
        ),
        (
            "Floater hill 4 × 48 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Floater hill 8 × 64 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Floater hill 12 × 80 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Floater hill 6 × 64 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Floater hill 12 × 96 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Floater hill 16 × 112 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Speed hill 4 × 64 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        (
            "Speed hill 6 × 80 m",
            &["Timber Tumbler", "Velocity Viper", "Briar Blitz"],
        ),
        ("Ejector hill 6 × 48 m", &["Briar Blitz"]),
        ("Strong floater hill 8 × 64 m", &["Briar Blitz"]),
    ];
    let models = [wooden_twister_model(), hybrid_model(), multi_launch_model()];
    let hill_pieces: Vec<_> = catalog()
        .iter()
        .filter(|piece| expected_hills.iter().any(|(name, _)| piece.name == *name))
        .collect();
    assert_eq!(hill_pieces.len(), expected_hills.len());

    for (name, expected_models) in expected_hills {
        let piece = hill_pieces
            .iter()
            .find(|piece| {
                piece.name == name
                    && expected_models.iter().all(|model_name| {
                        models
                            .iter()
                            .find(|model| model.name == *model_name)
                            .unwrap()
                            .allowed_pieces
                            .contains(&piece.id)
                    })
            })
            .unwrap_or_else(|| panic!("catalog is missing {name}"));
        assert!(matches!(piece.shape, Shape::Profile(_)), "{name}");
        let rated = piece.rated_speeds.expect("hill has a rated speed window");
        assert!(rated.maximum_entry - rated.minimum_entry >= 2.0, "{name}");
        let Some(PieceFamily::Hill(family)) = piece.family else {
            panic!("{name} should have hill family metadata");
        };
        let dimensions: Vec<u16> = name
            .split_whitespace()
            .filter_map(|word| word.parse().ok())
            .collect();
        let expected_feel = if name.starts_with("Strong floater") {
            HillFeel::StrongFloater
        } else if name.starts_with("Floater") {
            HillFeel::Floater
        } else if name.starts_with("Speed") {
            HillFeel::Speed
        } else {
            HillFeel::Ejector
        };
        assert_eq!(family.feel, expected_feel, "{name}");
        assert_eq!(family.height_metres, dimensions[0] as u8, "{name}");
        assert_eq!(family.span_metres, dimensions[1], "{name}");
        assert_eq!(family.twist, None, "{name}");
        let reduction = match piece.id {
            PieceId::EjectorHill8x48 | PieceId::FloaterHill4x48 | PieceId::SpeedHill6x80 => 1.0,
            PieceId::SpeedHill4x64 => 3.0,
            _ => 0.0,
        };
        assert_eq!(
            family.design_speed_metres_per_second - reduction,
            rated.maximum_entry,
            "{name} rated maximum"
        );

        for model in &models {
            let should_be_allowed = expected_models.contains(&model.name);
            assert_eq!(
                model.allowed_pieces.contains(&piece.id),
                should_be_allowed,
                "{} availability for {name}",
                model.name
            );
            assert_eq!(
                model
                    .available_pieces(TrackState::origin())
                    .iter()
                    .any(|candidate| candidate.id == piece.id),
                should_be_allowed,
                "{} reachable entry connector for {name}",
                model.name
            );
        }
    }
}

#[test]
fn half_loop_has_grid_aligned_forward_displacement() {
    let pieces = catalog();
    for (id, expected_x) in [(PieceId::HalfLoopUp, 14), (PieceId::HalfLoopDown, -14)] {
        let piece = pieces.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(piece.exit.x - piece.entry.x, expected_x);
        for (distance, state) in [(0.0, piece.entry), (piece.length(), piece.exit)] {
            let sample = piece.sample(distance);
            assert!(
                (sample.position - state.position()).length() < 1e-8,
                "{id:?} position"
            );
            assert!(
                (sample.tangent - state.tangent()).length() < 1e-8,
                "{id:?} tangent"
            );
        }
    }
}

#[test]
fn full_loop_profiles_match_their_rated_geometry() {
    let expected = [
        (PieceId::Loop, 27.5, 24.7, 30.5, 128.0, 5.5),
        (PieceId::LargeLoop, 31.3, 27.6, 37.9, 161.0, 5.5),
        (PieceId::SteelLoop, 27.0, 25.2, 31.8, 137.0, 8.5),
    ];
    let pieces = catalog();
    for (id, maximum_entry, expected_minimum, expected_rise, expected_length, radius_floor) in
        expected
    {
        let piece = pieces.iter().find(|piece| piece.id == id).unwrap();
        let rated_speeds = piece.rated_speeds.unwrap();
        let rise = (0..=2048)
            .map(|index| {
                piece
                    .sample(piece.length() * f64::from(index) / 2048.0)
                    .position
                    .z
            })
            .fold(0.0, f64::max);
        let minimum_radius = piece.maximum_curvature().recip();
        eprintln!(
            "{id:?}: rise={rise:.3} m, length={:.3} m, rated entry={:.3}..{:.3} m/s, minimum rail radius={minimum_radius:.3} m, exit=({}, {}, {})",
            piece.length(),
            rated_speeds.minimum_entry,
            rated_speeds.maximum_entry,
            piece.exit.x,
            piece.exit.y,
            piece.exit.z
        );

        assert_eq!(rated_speeds.maximum_entry, maximum_entry, "{id:?}");
        assert!(
            (rated_speeds.minimum_entry - expected_minimum).abs() < 0.3,
            "{id:?}: minimum entry {}",
            rated_speeds.minimum_entry
        );
        assert!((rise / expected_rise - 1.0).abs() < 0.03, "{id:?}: {rise}");
        assert!(
            (piece.length() / expected_length - 1.0).abs() < 0.03,
            "{id:?}: length {}",
            piece.length()
        );
        assert!(minimum_radius >= radius_floor, "{id:?}: {minimum_radius}");
        assert_eq!(piece.exit.z, piece.entry.z, "{id:?}");
        assert_eq!(piece.exit.y - piece.entry.y, 2, "{id:?}");
        assert_eq!(piece.inversion, Some(Inversion::VerticalLoop), "{id:?}");
        assert_eq!(
            piece.heartlining,
            Some(Heartlining::NotHeartlined),
            "{id:?}"
        );
        assert!(
            (piece.sample(piece.length()).position - piece.exit.position()).length() < 1e-8,
            "{id:?}: exit position"
        );
        assert!(
            (piece.sample(piece.length()).tangent - piece.exit.tangent()).length() < 1e-8,
            "{id:?}: exit tangent"
        );
    }
}

fn end_state_after_allowed_pieces(model: &CoasterModel, ids: &[PieceId]) -> TrackState {
    let mut state = TrackState::origin();
    for id in ids {
        state = model
            .available_pieces(state)
            .into_iter()
            .find(|piece| piece.id == *id)
            .unwrap_or_else(|| panic!("{} does not offer {id:?} from {state:?}", model.name))
            .exit;
    }
    state
}

#[test]
fn inversion_pieces_identify_their_heartlining() {
    for piece in catalog() {
        if piece.inversion.is_some() {
            assert!(
                piece.heartlining.is_some(),
                "{:?}: inversions identify heartlining",
                piece.id
            );
        } else if piece.heartlining.is_some() {
            assert!(
                piece.family.is_some_and(
                    |family| matches!(family, PieceFamily::Hill(family) if matches!(
                        family.twist,
                        Some(HillTwist::OuterBankLeft | HillTwist::OuterBankRight)
                    ))
                ) && piece.heartlining == Some(Heartlining::Heartlined),
                "{:?}: unexpected heartlining on a non-inversion",
                piece.id
            );
        }
        assert!(
            !piece.name.contains("(heartlined)"),
            "{:?}: {}",
            piece.id,
            piece.name
        );
        assert!(
            !piece.name.contains("(not heartlined)"),
            "{:?}: {}",
            piece.id,
            piece.name
        );
    }
}

#[test]
fn half_corkscrew_ids_display_as_up_and_down_half_rolls() {
    let expected_names = [
        (PieceId::HalfCorkscrewLeftUp, "Left half roll up"),
        (PieceId::HalfCorkscrewLeftDown, "Left half roll down"),
        (PieceId::HalfCorkscrewRightUp, "Right half roll up"),
        (PieceId::HalfCorkscrewRightDown, "Right half roll down"),
    ];
    let pieces = catalog();

    for (id, expected_name) in expected_names {
        let piece = pieces.iter().find(|piece| piece.id == id).unwrap();
        assert_eq!(piece.name, expected_name, "{id:?}");
    }
}

#[test]
fn half_inversions_connect_on_inverting_models() {
    for model in [hybrid_model(), multi_launch_model()] {
        for half_corkscrew in [
            PieceId::HalfCorkscrewLeftDown,
            PieceId::HalfCorkscrewRightDown,
        ] {
            let end =
                end_state_after_allowed_pieces(&model, &[PieceId::HalfLoopUp, half_corkscrew]);
            assert_eq!(end.bank, Bank::Level);
        }
        for half_corkscrew in [PieceId::HalfCorkscrewLeftUp, PieceId::HalfCorkscrewRightUp] {
            let end =
                end_state_after_allowed_pieces(&model, &[half_corkscrew, PieceId::HalfLoopDown]);
            assert_eq!(end.bank, Bank::Level);
        }
    }
}

#[test]
fn wooden_twister_offers_top_hat() {
    let model = wooden_twister_model();
    assert!(
        model
            .available_pieces(TrackState::origin())
            .iter()
            .any(|piece| piece.id == PieceId::TopHat)
    );
}

#[test]
fn wooden_twister_offers_vertical_curve_continuations() {
    let model = wooden_twister_model();
    let upward_end = end_state_after_allowed_pieces(
        &model,
        &[
            PieceId::VerticalUpEntry,
            PieceId::VerticalUpContinue,
            PieceId::VerticalUpExit,
        ],
    );
    let downward_end = end_state_after_allowed_pieces(
        &model,
        &[
            PieceId::VerticalDownEntry,
            PieceId::VerticalDownContinue,
            PieceId::VerticalDownExit,
        ],
    );
    assert_eq!(upward_end.vertical_curvature, VerticalCurvature::Straight);
    assert_eq!(downward_end.vertical_curvature, VerticalCurvature::Straight);
}

#[test]
fn tangent_only_matches_full_sample_for_every_piece() {
    for piece in catalog() {
        for index in 0..=16 {
            let distance = piece.length() * f64::from(index) / 16.0;
            assert_eq!(
                piece.tangent_at(distance),
                piece.sample(distance).tangent,
                "{:?} at {distance}",
                piece.id
            );
        }
    }
}

#[test]
fn wooden_twister_offers_only_steel_structure_inversions() {
    let model = wooden_twister_model();
    assert!(model.lift_rules.is_some());
    assert!(model.launch_types.is_empty());
    assert_eq!(model.support_style, SupportStyle::DenseWoodenLattice);
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
    assert!(model.lift_rules.is_some());
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
    }
    for id in [PieceId::TerrainRise, PieceId::TerrainFall] {
        assert!(choices.iter().any(|piece| piece.id == id));
    }
}

#[test]
fn hybrid_model_offers_linear_motor_launches() {
    let model = hybrid_model();
    assert_eq!(model.launch_types, vec![LaunchType::LinearMotor]);
    let launch = model.train.launch.expect("launch parameters");
    assert!(launch.target_speed > 0.0);
    assert!(launch.acceleration > 0.0);

    let choices = model.available_pieces(TrackState::origin());
    for id in [PieceId::LaunchShort, PieceId::Launch, PieceId::LaunchLong] {
        assert!(model.allowed_pieces.contains(&id), "{id:?}");
        let piece = choices
            .iter()
            .find(|piece| piece.id == id)
            .unwrap_or_else(|| panic!("{} does not offer {id:?} from level", model.name));
        assert_eq!(piece.kind, PieceKind::Launch);
    }
}

#[test]
fn signature_pieces_are_available_to_their_coaster_models() {
    let hybrid = hybrid_model();
    let multi_launch = multi_launch_model();
    let wooden_twister = wooden_twister_model();

    for piece in [PieceId::HybridOverbankLeft, PieceId::HybridOverbankRight] {
        assert!(hybrid.allowed_pieces.contains(&piece));
        assert!(multi_launch.allowed_pieces.contains(&piece));
        assert!(!wooden_twister.allowed_pieces.contains(&piece));
    }

    for piece in [PieceId::TwisterFanLeft, PieceId::TwisterFanRight] {
        assert!(wooden_twister.allowed_pieces.contains(&piece));
        assert!(!hybrid.allowed_pieces.contains(&piece));
        assert!(!multi_launch.allowed_pieces.contains(&piece));
    }

    for piece in [PieceId::LaunchShort, PieceId::Launch, PieceId::LaunchLong] {
        assert!(multi_launch.allowed_pieces.contains(&piece));
        assert!(hybrid.allowed_pieces.contains(&piece));
        assert!(!wooden_twister.allowed_pieces.contains(&piece));
    }

    assert!(wooden_twister.allowed_pieces.contains(&PieceId::SteelLoop));
    assert!(!hybrid.allowed_pieces.contains(&PieceId::SteelLoop));
    assert!(!multi_launch.allowed_pieces.contains(&PieceId::SteelLoop));
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
    for piece in catalog().iter().filter(|piece| {
        piece.entry.heading != piece.exit.heading
            && matches!(piece.entry.bank, Bank::Left | Bank::Right)
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
    for piece in pieces.iter().filter(|piece| {
        (piece.entry.heading != piece.exit.heading
            && (piece.exit.heading.index() - piece.entry.heading.index()).rem_euclid(8) != 4)
            || piece.exit.y != 0
    }) {
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
            near_angle(b.roll, -a.roll);
        }
    }
}

fn mirrored_bank(bank: Bank) -> Bank {
    match bank {
        Bank::Left => Bank::Right,
        Bank::Right => Bank::Left,
        Bank::Level => Bank::Level,
        Bank::Inverted => Bank::Inverted,
    }
}

fn mirrored_geometry_matches(piece: &Piece, candidate: &Piece) -> bool {
    if candidate.entry.x != piece.entry.x
        || candidate.entry.y != -piece.entry.y
        || candidate.entry.z != piece.entry.z
        || candidate.entry.heading.index() != (-piece.entry.heading.index()).rem_euclid(8)
        || candidate.entry.slope != piece.entry.slope
        || candidate.entry.bank != mirrored_bank(piece.entry.bank)
        || candidate.exit.x != piece.exit.x
        || candidate.exit.y != -piece.exit.y
        || candidate.exit.z != piece.exit.z
        || candidate.exit.heading.index() != (-piece.exit.heading.index()).rem_euclid(8)
        || candidate.exit.slope != piece.exit.slope
        || candidate.exit.bank != mirrored_bank(piece.exit.bank)
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
        let mut reflected_exit = left.exit;
        reflected_exit.y = -reflected_exit.y;
        assert_eq!(reflected_exit, right.exit);
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

fn assert_track_frame_continuous_at_join(first: &Piece, second: &Piece) {
    let join_distance = 0.05;
    let incoming = first.sample(first.length());
    let outgoing = second.sample(0.0);
    near_vec(incoming.position, outgoing.position);
    near_vec(incoming.tangent, outgoing.tangent);
    near_vec(incoming.up, outgoing.up);
    near_angle(incoming.roll, outgoing.roll);
    near(incoming.curvature, outgoing.curvature);

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
            let sample = piece.sample(piece.length() * (f64::from(index) / 1000.0));
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
    for first in templates {
        for template in templates {
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
            assert!(
                (a.curvature - b.curvature).abs() < 1e-8,
                "{:?} -> {:?}: curvature {} != {}",
                first.id,
                second.id,
                a.curvature,
                b.curvature
            );
        }
    }
}

#[test]
fn banked_turns_join_banked_straights_and_rollouts_continuously() {
    use PieceId::*;

    let templates = catalog();
    let banked_turn_exits = [
        BankedTurnExit,
        BankedTurnStandalone,
        SmallRightBankedTurnExit,
        SmallRightBankedTurnStandalone,
        LargeLeftBankedTurnExit,
        LargeLeftBankedTurnStandalone,
        LargeRightBankedTurnExit,
        LargeRightBankedTurnStandalone,
        TwisterFanLeft,
        TwisterFanRight,
        TwisterHelixLeft,
        TwisterHelixRight,
    ];

    let mut tested_turns = 0;
    for turn in templates.iter().filter(|piece| {
        banked_turn_exits.contains(&piece.id)
            && piece.entry.heading != piece.exit.heading
            && piece.exit.bank != Bank::Level
            && piece.exit.horizontal_curvature == HorizontalCurvature::Straight
    }) {
        tested_turns += 1;
        let (straight_id, rollout_id) = match turn.exit.bank {
            Bank::Left => (BankedStraightLeft, RolloutLeft),
            Bank::Right => (BankedStraightRight, RolloutRight),
            Bank::Level | Bank::Inverted => unreachable!("banked turn exits banked"),
        };
        let banked_straight = templates
            .iter()
            .filter(|piece| piece.id == straight_id)
            .find_map(|template| template.at(turn.exit))
            .unwrap_or_else(|| panic!("{straight_id:?} does not follow {:?}", turn.id));
        let rollout = templates
            .iter()
            .filter(|piece| piece.id == rollout_id)
            .find_map(|template| template.at(banked_straight.exit))
            .unwrap_or_else(|| panic!("{rollout_id:?} does not follow {:?}", turn.id));
        let direct_rollout = templates
            .iter()
            .filter(|piece| piece.id == rollout_id)
            .find_map(|template| template.at(turn.exit))
            .unwrap_or_else(|| panic!("{rollout_id:?} does not follow {:?} directly", turn.id));

        assert_track_frame_continuous_at_join(turn, &banked_straight);
        assert_track_frame_continuous_at_join(&banked_straight, &rollout);
        assert_track_frame_continuous_at_join(turn, &direct_rollout);
    }
    assert!(tested_turns > 0, "no banked turn exits tested");
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
fn piece_filters_preserve_model_state_and_radius_rules() {
    let origin = TrackState::origin();
    let mut restricted = hybrid_model();
    restricted.allowed_pieces = vec![PieceId::Straight];
    let mut loop_only = hybrid_model();
    loop_only.allowed_pieces = vec![PieceId::Loop];
    let mut radius_limited = loop_only.clone();
    radius_limited.minimum_radius_metres = 100.0;

    let cases = [
        (
            "hybrid model allow-list and entry state",
            hybrid_model(),
            origin,
            None,
        ),
        (
            "restricted model allows only straight track",
            restricted.clone(),
            origin,
            Some(vec![PieceId::Straight]),
        ),
        (
            "restricted model rejects a left-banked entry",
            restricted,
            TrackState {
                bank: Bank::Left,
                ..origin
            },
            Some(Vec::new()),
        ),
        (
            "loop-only model allows its loop at the northeast heading",
            loop_only,
            TrackState {
                heading: Heading::NorthEast,
                ..origin
            },
            Some(vec![PieceId::Loop]),
        ),
        (
            "minimum radius rejects the loop",
            radius_limited,
            TrackState {
                heading: Heading::NorthEast,
                ..origin
            },
            Some(Vec::new()),
        ),
    ];

    for (case_name, model, entry, expected_piece_ids) in cases {
        let pieces = model.available_pieces(entry);
        for piece in &pieces {
            assert!(model.allowed_pieces.contains(&piece.id), "{case_name}");
            assert_eq!(piece.entry, entry, "{case_name}");
        }
        if let Some(expected_piece_ids) = expected_piece_ids {
            assert_eq!(
                pieces.iter().map(|piece| piece.id).collect::<Vec<_>>(),
                expected_piece_ids,
                "{case_name}"
            );
        }
    }

    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let incompatible_entries = [
        (
            "upward slope",
            TrackState {
                slope: Slope::UpGentle,
                ..origin
            },
        ),
        (
            "left bank",
            TrackState {
                bank: Bank::Left,
                ..origin
            },
        ),
    ];
    for (case_name, entry) in incompatible_entries {
        assert!(straight.at(entry).is_none(), "{case_name}");
    }
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
                .iter()
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
                    .sample(piece.length() * (f64::from(i) / 2000.0))
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
fn short_offsets_have_grid_aligned_smooth_mirrored_geometry() {
    let pieces = catalog();
    let left = pieces
        .iter()
        .find(|piece| piece.id == PieceId::ShortOffsetLeft)
        .expect("short left offset");
    let right = pieces
        .iter()
        .find(|piece| piece.id == PieceId::ShortOffsetRight)
        .expect("short right offset");

    assert_eq!(left.exit.x, 6);
    assert_eq!(left.exit.y, 1);
    assert_eq!(left.exit.heading, left.entry.heading);
    near_vec(left.sample(0.0).position, left.entry.position());
    near_vec(left.sample(left.length()).position, left.exit.position());
    near_vec(left.sample(0.0).tangent, left.entry.tangent());
    near_vec(left.sample(left.length()).tangent, left.exit.tangent());
    near(left.sample(0.0).curvature, 0.0);
    near(left.sample(left.length()).curvature, 0.0);
    let minimum_radius = 1.0 / left.maximum_curvature();
    assert!(minimum_radius >= 8.0, "minimum radius {minimum_radius}");
    assert!(
        (minimum_radius - 9.89).abs() < 0.02,
        "minimum radius {minimum_radius}"
    );

    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let left_sample = left.sample(left.length() * fraction);
        let right_sample = right.sample(right.length() * fraction);
        near_vec(
            right_sample.position,
            Vec3::new(
                left_sample.position.x,
                -left_sample.position.y,
                left_sample.position.z,
            ),
        );
        near_vec(
            right_sample.tangent,
            Vec3::new(
                left_sample.tangent.x,
                -left_sample.tangent.y,
                left_sample.tangent.z,
            ),
        );
        near(left_sample.curvature.abs(), right_sample.curvature.abs());
    }
}

#[test]
fn each_coaster_model_offers_short_offsets_when_its_radius_allows_them() {
    for model in [wooden_twister_model(), hybrid_model(), multi_launch_model()] {
        let available_ids = model
            .available_pieces(TrackState::origin())
            .into_iter()
            .map(|piece| piece.id)
            .collect::<Vec<_>>();
        assert!(
            available_ids.contains(&PieceId::ShortOffsetLeft),
            "{}",
            model.name
        );
        assert!(
            available_ids.contains(&PieceId::ShortOffsetRight),
            "{}",
            model.name
        );
    }
}

#[test]
fn sixty_degree_slope_templates_have_grid_aligned_geometry() {
    use crate::track::Slope;
    use PieceId::*;

    let pieces = catalog();
    let expected = [
        (LevelToSixtyUp, Slope::Level, Slope::UpSixty),
        (SixtyUpToLevel, Slope::UpSixty, Slope::Level),
        (LevelToSixtyDown, Slope::Level, Slope::DownSixty),
        (SixtyDownToLevel, Slope::DownSixty, Slope::Level),
        (SteepToSixtyUp, Slope::UpSteep, Slope::UpSixty),
        (SixtyUp, Slope::UpSixty, Slope::UpSixty),
        (SixtyToSteepUp, Slope::UpSixty, Slope::UpSteep),
        (SteepToSixtyDown, Slope::DownSteep, Slope::DownSixty),
        (SixtyDown, Slope::DownSixty, Slope::DownSixty),
        (SixtyToSteepDown, Slope::DownSixty, Slope::DownSteep),
    ];
    for (id, entry_slope, exit_slope) in expected {
        let piece = pieces
            .iter()
            .find(|piece| piece.id == id)
            .expect("60-degree template");
        assert_eq!(
            (piece.entry.slope, piece.exit.slope),
            (entry_slope, exit_slope)
        );
        assert!((piece.exit.position().z - piece.entry.position().z).abs() > 0.0);
        if matches!(id, SixtyUp | SixtyDown) {
            let horizontal_distance = (piece.exit.position() - piece.entry.position())
                .x
                .hypot((piece.exit.position() - piece.entry.position()).y);
            let vertical_distance = (piece.exit.position().z - piece.entry.position().z).abs();
            let grid_angle = vertical_distance.atan2(horizontal_distance).to_degrees();
            assert!(
                (grid_angle - 60.0).abs() < 1.0,
                "{id:?}: {grid_angle:.3} degrees"
            );
        }
        for state in [piece.entry, piece.exit] {
            let angle = state
                .tangent()
                .z
                .atan2(state.tangent().x.hypot(state.tangent().y))
                .to_degrees()
                .abs();
            if state.slope == Slope::UpSixty || state.slope == Slope::DownSixty {
                assert!((angle - 60.0).abs() < 1.0, "{id:?}: {angle:.3} degrees");
            }
        }
    }
}

#[test]
fn sixty_degree_pull_profiles_match_connectors_and_reverse() {
    use PieceId::*;
    let pieces = catalog();
    let piece = |id| pieces.iter().find(|piece| piece.id == id).unwrap();
    for (up_id, down_id) in [
        (LevelToSixtyUp, SixtyDownToLevel),
        (SteepToSixtyUp, SixtyToSteepDown),
    ] {
        let up = piece(up_id);
        let down = piece(down_id);
        assert!(matches!(up.shape, Shape::Profile(_)));
        assert!(matches!(down.shape, Shape::Profile(_)));
        assert!((up.length() - down.length()).abs() < 1e-8);
        for item in [up, down] {
            assert!((item.sample(0.0).position - item.entry.position()).length() < 1e-8);
            assert!((item.sample(item.length()).position - item.exit.position()).length() < 1e-8);
            assert!(item.rated_speeds.is_some());
        }
        for index in 0..=20 {
            let fraction = index as f64 / 20.0;
            let up_position = up.sample(up.length() * (1.0 - fraction)).position;
            let down_position = down.sample(down.length() * fraction).position;
            let expected_x = up.exit.position().x - up_position.x;
            let expected_z = up_position.z - up.exit.position().z;
            assert!((down_position.x - down.entry.position().x - expected_x).abs() < 1e-7);
            assert!((down_position.z - down.entry.position().z - expected_z).abs() < 1e-7);
        }
    }
    for id in [
        SixtyUpToLevel,
        LevelToSixtyDown,
        SixtyToSteepUp,
        SteepToSixtyDown,
    ] {
        assert!(piece(id).rated_speeds.is_some(), "{id:?}");
    }
}

#[test]
fn coaster_models_use_peak_rider_force_limits() {
    for (model, minimum_vertical, maximum_vertical) in [
        (wooden_twister_model(), -1.0, 4.0),
        (hybrid_model(), -1.5, 4.5),
        (multi_launch_model(), -1.5, 5.0),
    ] {
        let limits = model.comfortable_force_limits;
        assert_eq!(limits.minimum_vertical_g, minimum_vertical);
        assert_eq!(limits.maximum_vertical_g, maximum_vertical);
        assert_eq!(limits.maximum_lateral_g, 1.5);
        assert_eq!(limits.maximum_longitudinal_g, 1.5);
    }
}

#[test]
fn sixty_degree_track_is_allowed_only_by_models_with_sixty_degree_limits() {
    use PieceId::{
        LevelToSixtyDown, LevelToSixtyUp, SixtyDown, SixtyDownToLevel, SixtyToSteepDown,
        SixtyToSteepUp, SixtyUp, SixtyUpToLevel, SteepToSixtyDown, SteepToSixtyUp,
    };

    let sixty_degree_pieces = [
        LevelToSixtyUp,
        SixtyUpToLevel,
        LevelToSixtyDown,
        SixtyDownToLevel,
        SteepToSixtyUp,
        SixtyUp,
        SixtyToSteepUp,
        SteepToSixtyDown,
        SixtyDown,
        SixtyToSteepDown,
    ];
    for model in [hybrid_model(), multi_launch_model()] {
        assert!(
            sixty_degree_pieces
                .iter()
                .all(|piece| model.allowed_pieces.contains(piece))
        );
        assert_eq!(
            model.lift_rules.unwrap().max_slope,
            crate::track::Slope::UpSixty
        );
    }
    let wooden_twister = wooden_twister_model();
    assert!(
        sixty_degree_pieces
            .iter()
            .all(|piece| !wooden_twister.allowed_pieces.contains(piece))
    );
    assert_eq!(
        wooden_twister.lift_rules.unwrap().max_slope,
        crate::track::Slope::UpSteep
    );
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
        for id in [
            PieceId::Loop,
            PieceId::LargeLoop,
            PieceId::Corkscrew,
            PieceId::HeartlineRoll,
        ] {
            assert!(
                available.iter().any(|piece| piece.id == id),
                "{id:?} {heading:?}"
            );
        }
        for piece in available {
            assert_eq!(piece.entry, start);
            assert_eq!(piece.exit_at(start), Some(piece.exit));
            near_vec(piece.sample(0.0).position, piece.entry.position());
            assert!(
                (piece.sample(piece.length()).position - piece.exit.position()).length() < 1e-8,
                "{:?} {heading:?}: endpoint {:?} != {:?}",
                piece.id,
                piece.sample(piece.length()).position,
                piece.exit.position()
            );
            near_vec(piece.sample(piece.length()).tangent, piece.exit.tangent());
        }
    }
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
        for id in [
            PieceId::Loop,
            PieceId::LargeLoop,
            PieceId::SteelLoop,
            PieceId::Corkscrew,
            PieceId::HeartlineRoll,
        ] {
            let piece = catalog()
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(start)
                .unwrap();
            let mut previous_roll = piece.sample(0.0).roll;
            for index in 1..=1000 {
                let sample = piece.sample(piece.length() * (f64::from(index) / 1000.0));
                assert!(sample.curvature.is_finite());
                near(sample.tangent.length(), 1.0);
                near(sample.up.length(), 1.0);
                assert!((sample.roll - previous_roll).abs() < 0.1);
                previous_roll = sample.roll;
            }
            let middle = piece.sample(piece.length() / 2.0);
            assert!(middle.up.z < -0.5, "{id:?} {heading:?}");
            if matches!(id, PieceId::Loop | PieceId::LargeLoop | PieceId::SteelLoop) {
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
    use crate::track::construction::Construction;
    use crate::world::{Surface, World};

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
    let mut powered_construction = Construction::new(start, hybrid_model());
    let mut unpowered_construction = Construction::new(start, hybrid_model());
    powered_construction.set_chain(true);
    for id in [
        PieceId::Straight,
        PieceId::LevelToGentleUp,
        PieceId::GentleUp,
        PieceId::GentleUpToLevel,
    ] {
        powered_construction.append(id, &world, &[]).unwrap();
        unpowered_construction.append(id, &world, &[]).unwrap();
        let powered = powered_construction.pieces().last().unwrap();
        let unpowered = unpowered_construction.pieces().last().unwrap();
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
    }
}

#[test]
fn coaster_models_have_a_minimum_rider_height() {
    assert_eq!(super::hybrid_model().minimum_height_cm, 122);
    assert_eq!(super::multi_launch_model().minimum_height_cm, 132);
}

#[test]
fn sloped_turns_obey_model_capabilities_and_radius() {
    let mut model = hybrid_model();
    for slope in [Slope::UpGentle, Slope::DownGentle] {
        let state = TrackState {
            slope,
            ..TrackState::origin()
        };
        let turns: Vec<_> = model
            .available_pieces(state)
            .into_iter()
            .filter(|piece| piece.entry.heading != piece.exit.heading)
            .collect();
        assert!(!turns.is_empty());
        assert!(
            turns
                .iter()
                .all(|piece| piece.maximum_curvature() <= 1.0 / model.minimum_radius_metres)
        );
        let mut restricted = model.clone();
        restricted.minimum_radius_metres = 1000.0;
        assert!(
            restricted
                .available_pieces(state)
                .iter()
                .all(|piece| piece.entry.heading == piece.exit.heading)
        );
    }
    model.sloped_turn_slopes.clear();
    for slope in [Slope::UpGentle, Slope::DownGentle] {
        let state = TrackState {
            slope,
            ..TrackState::origin()
        };
        assert!(
            model
                .available_pieces(state)
                .iter()
                .all(|piece| piece.entry.heading == piece.exit.heading)
        );
    }
}

#[test]
fn full_loops_separate_entry_and_exit_and_clear_themselves() {
    let pieces = catalog();
    for id in [PieceId::Loop, PieceId::LargeLoop] {
        let template = pieces.iter().find(|piece| piece.id == id).unwrap();
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
            let piece = template.at(start).unwrap();
            let forward = start.tangent();
            let displacement = piece.exit.position() - start.position();
            let sideways = Vec3::new(-forward.y, forward.x, 0.0);
            assert!(displacement.dot(sideways) >= 4.0 - 1e-6, "{heading:?}");
            let samples: Vec<_> = (0..=400)
                .map(|index| {
                    let distance = piece.length() * (f64::from(index) / 400.0);
                    (distance, piece.sample(distance).position)
                })
                .collect();
            for (index, (distance, position)) in samples.iter().enumerate() {
                for (other_distance, other_position) in &samples[index + 1..] {
                    if other_distance - distance > 12.0 {
                        assert!(
                            (*other_position - *position).length() >= 2.0,
                            "{heading:?}: {distance} and {other_distance}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn half_inversions_join_with_matching_frames() {
    use PieceId::*;
    for (first, second) in [
        (HalfLoopUp, HalfCorkscrewLeftDown),
        (HalfLoopUp, HalfCorkscrewRightDown),
        (HalfCorkscrewLeftUp, HalfLoopDown),
        (HalfCorkscrewRightUp, HalfLoopDown),
    ] {
        let pieces = catalog();
        let first = pieces.iter().find(|piece| piece.id == first).unwrap();
        let second = pieces
            .iter()
            .find(|piece| piece.id == second)
            .unwrap()
            .at(first.exit)
            .unwrap();
        let end = first.sample(first.length());
        let start = second.sample(0.0);
        near_vec(end.position, start.position);
        near_vec(end.tangent, start.tangent);
        near_vec(end.up, start.up);
        assert!(end.up.z < -0.99);
        assert!(second.sample(second.length()).up.z > 0.99);
    }
}

#[test]
fn classic_corkscrew_has_a_larger_track_path_than_heartline_roll() {
    let pieces = catalog();
    let corkscrew = pieces
        .iter()
        .find(|piece| piece.id == PieceId::Corkscrew)
        .unwrap();
    let heartline = pieces
        .iter()
        .find(|piece| piece.id == PieceId::HeartlineRoll)
        .unwrap();
    let corkscrew_midpoint = corkscrew.sample(corkscrew.length() / 2.0);
    let heartline_midpoint = heartline.sample(heartline.length() / 2.0);
    assert!(corkscrew_midpoint.position.z > heartline_midpoint.position.z + 5.0);
}

#[test]
fn top_hat_has_a_tall_summit_and_level_connectors() {
    for (id, height) in [(PieceId::TopHat, 30.0), (PieceId::LaunchedTopHat, 38.0)] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let midpoint = piece.sample(piece.length() / 2.0);
        assert!((midpoint.position.z - height).abs() < 0.01);
        let slopes: Vec<_> = (0..=1000)
            .map(|index| {
                piece
                    .sample(piece.length() * (f64::from(index) / 1000.0))
                    .tangent
                    .z
            })
            .collect();
        assert!(slopes.iter().copied().fold(0.0, f64::max) > 0.65);
        assert!(slopes.iter().copied().fold(0.0, f64::min) < -0.65);
        assert!(piece.rated_speeds.is_some());
        near_vec(piece.sample(0.0).tangent, piece.entry.tangent());
        near_vec(piece.sample(piece.length()).tangent, piece.exit.tangent());
    }
}

#[test]
fn half_corkscrews_have_mirrored_track_paths() {
    let pieces = catalog();
    let left = pieces
        .iter()
        .find(|piece| piece.id == PieceId::HalfCorkscrewLeftUp)
        .unwrap();
    let right = pieces
        .iter()
        .find(|piece| piece.id == PieceId::HalfCorkscrewRightUp)
        .unwrap();
    let left_middle = left.sample(left.length() / 2.0);
    let right_middle = right.sample(right.length() / 2.0);
    assert!(left_middle.position.y.abs() > 1.0);
    near(left_middle.position.y, -right_middle.position.y);
}

#[test]
fn new_elements_have_regular_frames_in_every_heading() {
    use PieceId::*;
    let pieces = catalog();
    for id in [
        HalfLoopUp,
        HalfLoopDown,
        HalfCorkscrewLeftUp,
        HalfCorkscrewRightUp,
        HalfCorkscrewLeftDown,
        HalfCorkscrewRightDown,
        TopHat,
    ] {
        let template = pieces.iter().find(|piece| piece.id == id).unwrap();
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
            let state = TrackState {
                heading,
                ..template.entry
            };
            let piece = template.at(state).unwrap();
            for index in 0..=100 {
                let sample = piece.sample(piece.length() * (f64::from(index) / 100.0));
                assert!(sample.curvature.is_finite(), "{id:?} {heading:?} {index}");
                near(sample.tangent.length(), 1.0);
                near(sample.up.length(), 1.0);
                near(sample.up.dot(sample.tangent), 0.0);
            }
        }
    }
}

#[test]
fn hybrid_banked_drops_connect_and_roll_smoothly() {
    let model = hybrid_model();
    for (bank_piece, drop_ids) in [
        (
            PieceId::BankLeft,
            [
                PieceId::BankedDropLeft,
                PieceId::TwistedDropLeft,
                PieceId::BarrelDropLeft,
            ],
        ),
        (
            PieceId::BankRight,
            [
                PieceId::BankedDropRight,
                PieceId::TwistedDropRight,
                PieceId::BarrelDropRight,
            ],
        ),
    ] {
        let banked = end_state_after_allowed_pieces(&model, &[bank_piece]);
        for id in drop_ids {
            let piece = model
                .available_pieces(banked)
                .into_iter()
                .find(|piece| piece.id == id)
                .unwrap();
            assert_eq!(piece.exit.slope, Slope::DownGentle);
            if matches!(id, PieceId::BankedDropLeft | PieceId::BankedDropRight) {
                assert_eq!(piece.exit.bank, banked.bank);
            } else {
                assert_eq!(piece.exit.bank, Bank::Level);
            }
            assert!(piece.exit.z < piece.entry.z);
            assert!((piece.sample(0.0).roll - banked.bank.radians()).abs() < 1e-8);
            assert!(piece.maximum_curvature() <= 1.0 / model.minimum_radius_metres);
            assert!(!model.available_pieces(piece.exit).is_empty());
        }
    }
}

#[test]
fn barrel_drops_make_a_full_roll_and_mirror() {
    let pieces = catalog();
    let left = pieces
        .iter()
        .find(|piece| piece.id == PieceId::BarrelDropLeft)
        .unwrap();
    let right = pieces
        .iter()
        .find(|piece| piece.id == PieceId::BarrelDropRight)
        .unwrap();
    for step in 0..=100 {
        let fraction = f64::from(step) / 100.0;
        let left_sample = left.sample(left.length() * fraction);
        let right_sample = right.sample(right.length() * fraction);
        assert!((left_sample.roll + right_sample.roll).abs() < 1e-8);
        assert!(
            left_sample.tangent.z <= 1e-8,
            "fraction {fraction}: tangent {:?}",
            left_sample.tangent
        );
    }
    assert!(left.sample(left.length() * 0.5).roll > std::f64::consts::PI);
    assert!((left.sample(left.length()).roll - std::f64::consts::TAU).abs() < 1e-8);
}

#[test]
fn barrel_drop_supports_stay_outside_inverted_sections() {
    use crate::track::supports::generate_supports;
    use crate::world::{Surface, World};
    let world = World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for id in [PieceId::BarrelDropLeft, PieceId::BarrelDropRight] {
        let template = catalog().iter().find(|piece| piece.id == id).unwrap();
        let state = TrackState {
            x: 100,
            y: 100,
            z: 20,
            bank: template.entry.bank,
            ..TrackState::origin()
        };
        let piece = template.at(state).unwrap();
        let supports = generate_supports(&piece, &world, &[]).unwrap();
        let inverted = piece.sample(piece.length() / 2.0);
        assert!(inverted.up.z < 0.0);
        assert!(
            supports.iter().any(|support| {
                (support.anchor.x - inverted.position.x)
                    .hypot(support.anchor.y - inverted.position.y)
                    < 8.0
                    && support.footing.z < support.anchor.z
            }),
            "{id:?}: nearby grounded support is still required"
        );
        let count = (piece.length() / 8.0).ceil() as usize;
        for support in supports {
            let distance = (0..count)
                .map(|index| piece.length() * (index as f64 + 0.5) / count as f64)
                .find(|&distance| piece.sample(distance).position == support.anchor)
                .expect("support anchors come from piece samples");
            let sample = piece.sample(distance);
            assert!(sample.up.z >= 0.0, "{id:?}: {support:?} at {distance} m");
            assert!(
                sample.tangent.z.abs() <= 0.8,
                "{id:?}: {support:?} at {distance} m is near vertical"
            );
        }
    }
}

#[test]
fn configured_train_rolled_inversion_speed_windows_are_physically_valid() {
    const SAMPLE_SPACING: f64 = 0.05;
    for id in [
        PieceId::ZeroGRollFastLeft,
        PieceId::ZeroGRollCompactLeft,
        PieceId::StallLeft,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let rated = piece.rated_speeds.unwrap();
        for (cars, car_length) in (3..=8)
            .map(|cars| (cars, 3.0))
            .chain((4..=12).map(|cars| (cars, 1.5)))
        {
            let length = car_length * f64::from(cars);
            let steps = ((piece.length() + length) / SAMPLE_SPACING).ceil() as usize;
            let heights: Vec<_> = (0..=steps)
                .map(|index| {
                    piece
                        .sample((index as f64 * SAMPLE_SPACING).min(piece.length()))
                        .position
                        .z
                })
                .collect();
            let initial_height = heights[0];
            let spacing_steps = (length / f64::from(cars) / SAMPLE_SPACING).round() as usize;
            let maximum_mean_height = (0..=steps)
                .map(|front| {
                    (0..cars)
                        .map(|car| heights[front.saturating_sub(car as usize * spacing_steps)])
                        .sum::<f64>()
                        / f64::from(cars)
                })
                .fold(f64::NEG_INFINITY, f64::max);
            let minimum = (2.0 * crate::physics::GRAVITY * (maximum_mean_height - initial_height)
                + 36.0)
                .sqrt();
            eprintln!(
                "{id:?} {cars} cars: minimum {minimum:.4}, maximum {:.4}, catalog minimum {:.4}",
                rated.maximum_entry, rated.minimum_entry
            );
            assert!(
                minimum < rated.maximum_entry,
                "{id:?} {cars} cars: {minimum} >= {}",
                rated.maximum_entry
            );
            let mut train = hybrid_model().train;
            train.cars = cars;
            train.length_metres = length;
            let actual = super::effective_rated_speeds(piece, &train).unwrap();
            assert!((actual.minimum_entry - minimum).abs() < 1e-9);
            assert_eq!(actual.maximum_entry, rated.maximum_entry);
        }
    }
}

#[test]
fn default_train_uses_exact_catalog_rating_at_any_elevation() {
    let train = hybrid_model().train;
    for id in [
        PieceId::ZeroGRollFastLeft,
        PieceId::ZeroGRollCompactLeft,
        PieceId::StallLeft,
    ] {
        let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
        let placed = piece
            .at(TrackState {
                z: 100,
                ..piece.entry
            })
            .unwrap();
        assert_eq!(
            super::effective_rated_speeds(&placed, &train)
                .unwrap()
                .minimum_entry
                .to_bits(),
            piece.rated_speeds.unwrap().minimum_entry.to_bits()
        );
    }
}
