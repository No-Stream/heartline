use super::*;
use crate::physics::{PhysicsConfig, PreviewStatus, Train, preview_open_track};
use crate::track::{Heading, Slope, Vec3, hybrid_model};
use crate::world::Surface;
use std::collections::{HashSet, VecDeque};

fn world() -> World {
    World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    )
}

fn start() -> TrackState {
    TrackState {
        x: 50,
        y: 50,
        z: 5,
        ..TrackState::origin()
    }
}

#[test]
fn loop_profiles_offer_mirrored_choices_for_each_model() {
    for (model, left_id, right_id) in [
        (hybrid_model(), PieceId::Loop, PieceId::LoopRight),
        (hybrid_model(), PieceId::LargeLoop, PieceId::LargeLoopRight),
        (
            crate::track::multi_launch_model(),
            PieceId::Loop,
            PieceId::LoopRight,
        ),
        (
            crate::track::multi_launch_model(),
            PieceId::LargeLoop,
            PieceId::LargeLoopRight,
        ),
        (
            crate::track::wooden_twister_model(),
            PieceId::SteelLoop,
            PieceId::SteelLoopRight,
        ),
    ] {
        let construction = Construction::new(start(), model);
        let choices = construction.choices(&world(), &[]);
        let left = choices.iter().find(|choice| choice.id == left_id).unwrap();
        let right = choices.iter().find(|choice| choice.id == right_id).unwrap();
        let left = templates()
            .iter()
            .find(|piece| piece.id == left.id)
            .unwrap();
        let right = templates()
            .iter()
            .find(|piece| piece.id == right.id)
            .unwrap();
        assert_eq!(left.exit.y, -right.exit.y);
        assert_ne!(left.exit.y, 0);
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let a = left.sample(left.length() * fraction);
            let b = right.sample(right.length() * fraction);
            assert!((a.position.x - b.position.x).abs() < 1e-6);
            assert!((a.position.y + b.position.y).abs() < 1e-6);
            assert!((a.position.z - b.position.z).abs() < 1e-6);
        }
    }
}

#[test]
fn barrel_roll_drops_are_constructible_from_level_track_in_both_directions() {
    let construction = Construction::new(start(), hybrid_model());
    let choices = construction.choices(&world(), &[]);

    for id in [PieceId::BarrelDropLeft, PieceId::BarrelDropRight] {
        assert!(
            choices.iter().any(|choice| choice.id == id),
            "missing construction choice {id:?} from a level connector"
        );
    }
}

#[test]
fn briar_blitz_choices_after_vertical_exit() {
    let world = world();
    let mut construction = Construction::new(
        TrackState { z: 15, ..start() },
        crate::track::wooden_twister_model(),
    );
    for piece in [
        PieceId::Station,
        PieceId::TightTwisterLeft,
        PieceId::BankLeft,
        PieceId::BankedTurnStandalone,
        PieceId::LargeLeftBankedTurnEntry,
        PieceId::LargeLeftBankedTurnExit,
        PieceId::UnbankLeft,
        PieceId::SteelLoop,
        PieceId::Station,
        PieceId::Brakes,
        PieceId::Station,
        PieceId::LevelToGentleDown,
        PieceId::GentleDownToLevel,
        PieceId::TurnLargeStandalone,
        PieceId::TurnLargeRightStandalone,
        PieceId::TurnSmallEntry,
        PieceId::TurnSmallExit,
        PieceId::OffsetLeft,
        PieceId::BankRight,
        PieceId::UnbankRight,
        PieceId::LevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::VerticalUpEntry,
        PieceId::VerticalUpContinue,
        PieceId::VerticalUpExit,
    ] {
        if piece == PieceId::LevelToGentleUp {
            construction.set_chain(true);
        }
        construction
            .append(piece, &world, &[])
            .unwrap_or_else(|error| panic!("{piece:?}: {error:?}"));
        if piece == PieceId::GentleUpToLevel {
            construction.set_chain(false);
        }
    }
    assert!(!construction.choices(&world, &[]).is_empty());
}

#[test]
fn construction_state_hash_covers_track_edits_but_ignores_validation_caches() {
    let world = world();
    let mut first = Construction::new(start(), hybrid_model());
    let second = Construction::new(start(), hybrid_model());
    first.choices(&world, &[]);
    assert_eq!(first.state_hash(), second.state_hash());
    first.append(PieceId::Straight, &world, &[]).unwrap();
    assert_ne!(first.state_hash(), second.state_hash());
}

#[test]
fn launch_speed_limits_follow_track_length_and_are_shared_across_models() {
    let world = world();
    let mut hybrid = Construction::new(start(), hybrid_model());
    hybrid.append(PieceId::LaunchShort, &world, &[]).unwrap();
    let short_bounds = hybrid.launch_speed_bounds(0).unwrap();
    assert!(short_bounds.minimum > 0.0);
    assert!(short_bounds.maximum <= crate::track::LAUNCH_MAX_SPEED_METRES_PER_SECOND);
    let mut long = Construction::new(start(), crate::track::multi_launch_model());
    long.append(PieceId::LaunchLong, &world, &[]).unwrap();
    let long_bounds = long.launch_speed_bounds(0).unwrap();
    assert!(long_bounds.maximum > short_bounds.maximum);
    assert_eq!(
        long_bounds.maximum,
        (2.0 * crate::track::LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED
            * long.pieces()[0].length())
        .sqrt()
    );

    let mut viper = Construction::new(start(), crate::track::multi_launch_model());
    viper.append(PieceId::LaunchLong, &world, &[]).unwrap();
    assert_eq!(viper.launch_speed_bounds(0), Some(long_bounds));
    assert!(viper.set_launch_intensity(0, 20.0).is_ok());
    assert!(
        viper
            .set_launch_intensity(0, long_bounds.maximum + 0.1)
            .is_err()
    );

    let mut high_speed_viper = Construction::new(start(), crate::track::multi_launch_model());
    for launch_index in 0..4 {
        high_speed_viper
            .append(PieceId::LaunchLong, &world, &[])
            .unwrap();
        let bounds = high_speed_viper.launch_speed_bounds(launch_index).unwrap();
        if launch_index < 3 {
            high_speed_viper
                .set_launch_intensity(launch_index, bounds.maximum)
                .unwrap();
        }
    }
    let fourth_launch_bounds = high_speed_viper.launch_speed_bounds(3).unwrap();
    assert!(fourth_launch_bounds.maximum > 58.1);
    assert!(high_speed_viper.set_launch_intensity(3, 59.0).is_ok());

    let original_hash = hybrid.state_hash();
    assert!(hybrid.set_launch_intensity(0, short_bounds.maximum).is_ok());
    assert_ne!(hybrid.state_hash(), original_hash);
}

#[test]
fn viper_can_build_a_launch_to_fifty_metres_per_second() {
    let world = world();
    let model = crate::track::multi_launch_model();
    let mut construction = Construction::new(start(), model.clone());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for (index, target) in [(1, 30.0), (2, 42.0), (3, 50.0)] {
        construction
            .append(PieceId::LaunchLong, &world, &[])
            .unwrap();
        construction.set_launch_intensity(index, target).unwrap();
    }
    let preview = preview_open_track(construction.pieces(), &model.train, PhysicsConfig::default())
        .unwrap();
    assert_eq!(preview.status, PreviewStatus::ReachedEnd);
    let last_sample = preview.report.samples.last().unwrap();
    let peak_force = preview
        .report
        .samples
        .iter()
        .map(|sample| sample.longitudinal_g.abs())
        .fold(0.0_f64, f64::max);
    eprintln!(
        "launch exit speed {} m/s, peak longitudinal force {} g",
        last_sample.speed, peak_force
    );
    assert!((45.0..=55.0).contains(&last_sample.speed));
    assert!(peak_force <= 1.5);
}

#[test]
fn wooden_twister_validates_every_dense_support_anchor() {
    let mut construction = Construction::new(start(), crate::track::wooden_twister_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let cache = construction.validation_cache.borrow();
    let supports = cache.as_ref().unwrap().supports[0]
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    let piece = &construction.pieces()[0];
    let count = (piece.length() / 4.0).ceil() as usize;
    assert_eq!(supports.len(), count);
    for (index, support) in supports.iter().enumerate() {
        assert_eq!(
            support.anchor,
            piece
                .sample(piece.length() * (index as f64 + 0.5) / count as f64)
                .position
        );
    }
}

#[test]
fn choices_report_model_and_connector_rejections() {
    let mut model = hybrid_model();
    model.allowed_pieces = vec![PieceId::Straight, PieceId::GentleUp];
    let construction = Construction::new(start(), model);
    let choices = construction.choices(&world(), &[]);
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::Station)
            .unwrap()
            .result,
        Err(ConstructionError::ModelForbidden)
    );
    assert_eq!(
        choices
            .iter()
            .find(|choice| choice.id == PieceId::GentleUp)
            .unwrap()
            .result,
        Err(ConstructionError::Unavailable)
    );
}

#[test]
fn construction_offers_bank_straights_and_rollouts_after_banked_turns() {
    use PieceId::*;

    let world = world();
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
    let models = [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ];

    for model in models {
        let mut tested_turns = 0;
        for turn in templates().iter().filter(|piece| {
            banked_turn_exits.contains(&piece.id)
                && piece.entry.heading != piece.exit.heading
                && piece.exit.bank != crate::track::Bank::Level
                && piece.exit.horizontal_curvature == crate::track::HorizontalCurvature::Straight
                && model.allows_piece(piece)
        }) {
            tested_turns += 1;
            let mut end = turn.exit;
            end.x += 50;
            end.y += 50;
            end.z += 20;
            let construction = Construction::new(end, model.clone());
            let choices = construction.choices(&world, &[]);
            let (banked_straight, rollout) = match turn.exit.bank {
                crate::track::Bank::Left => (BankedStraightLeft, RolloutLeft),
                crate::track::Bank::Right => (BankedStraightRight, RolloutRight),
                crate::track::Bank::Level | crate::track::Bank::Inverted => {
                    unreachable!("banked turn exits banked")
                }
            };

            for id in [banked_straight, rollout] {
                let choice = choices
                    .iter()
                    .find(|choice| choice.id == id)
                    .unwrap_or_else(|| panic!("missing {id:?} choice after {:?}", turn.id));
                assert_eq!(
                    choice.result,
                    Ok(()),
                    "{id:?} refused after {:?} for {} at {:?}",
                    turn.id,
                    model.name,
                    turn.exit
                );
            }
        }
        assert!(
            tested_turns > 0,
            "no banked turn exits tested for {}",
            model.name
        );
    }
}

#[test]
fn construction_offers_bank_straights_and_rollouts_after_bank_transitions() {
    use PieceId::*;

    let world = world();
    for model in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ] {
        for (transition, banked_straight, rollout) in [
            (BankLeft, BankedStraightLeft, RolloutLeft),
            (BankRight, BankedStraightRight, RolloutRight),
        ] {
            let mut construction = Construction::new(start(), model.clone());
            construction
                .append(Station, &world, &[])
                .unwrap_or_else(|error| panic!("{}: Station: {error:?}", model.name));
            construction
                .append(transition, &world, &[])
                .unwrap_or_else(|error| panic!("{}: {transition:?}: {error:?}", model.name));

            let choices = construction.choices(&world, &[]);
            for id in [banked_straight, rollout] {
                let choice = choices
                    .iter()
                    .find(|choice| choice.id == id)
                    .unwrap_or_else(|| {
                        panic!("missing {id:?} after {transition:?} for {}", model.name)
                    });
                assert_eq!(
                    choice.result,
                    Ok(()),
                    "{id:?} refused after {transition:?} for {} at {:?}",
                    model.name,
                    construction.end()
                );
            }
        }
    }
}

#[test]
fn launch_model_builds_midcourse_launch_and_chain_lifts() {
    let mut construction = Construction::new(start(), crate::track::multi_launch_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    construction
        .append(PieceId::LaunchLong, &world(), &[])
        .unwrap();
    assert_eq!(
        construction.pieces()[1].kind,
        crate::track::PieceKind::Launch
    );
    construction.set_chain(true);
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.pieces()[2].kind, crate::track::PieceKind::Lift);
    let mut hybrid = Construction::new(start(), hybrid_model());
    hybrid.append(PieceId::Launch, &world(), &[]).unwrap();
    hybrid.set_chain(true);
    hybrid.append(PieceId::LaunchShort, &world(), &[]).unwrap();
    assert_eq!(hybrid.pieces()[0].kind, crate::track::PieceKind::Launch);
    assert_eq!(hybrid.pieces()[1].kind, crate::track::PieceKind::Launch);
}

#[test]
fn chain_mode_filters_choices_and_preserves_placed_piece_kinds() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    let initial_hash = construction.state_hash();
    assert_eq!(
        construction.validate(PieceId::Straight, &world, &[]),
        Ok(())
    );
    construction.set_chain(true);
    assert!(construction.chain());
    assert_ne!(construction.state_hash(), initial_hash);
    assert_eq!(
        construction.validate(PieceId::Loop, &world, &[]),
        Err(ConstructionError::ChainNotAllowed)
    );
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Loop)
            .unwrap()
            .result,
        Err(ConstructionError::ChainNotAllowed)
    );
    assert_eq!(
        construction
            .available_choices(&world, &[])
            .iter()
            .find(|(_, choice)| choice.id == PieceId::Loop)
            .unwrap()
            .1
            .result,
        Err(ConstructionError::ChainNotAllowed)
    );
    construction
        .append(PieceId::LevelToGentleUp, &world, &[])
        .unwrap();
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    assert!(
        construction
            .pieces()
            .iter()
            .all(|piece| piece.kind == crate::track::PieceKind::Lift)
    );
    construction.set_chain(false);
    assert!(!construction.chain());
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    assert_eq!(
        construction.pieces()[2].kind,
        crate::track::PieceKind::Track
    );
}

#[test]
fn chain_rules_cover_slope_curvature_and_special_pieces() {
    let mut model = hybrid_model();
    let pieces = catalog();
    let piece = |id| pieces.iter().find(|piece| piece.id == id).unwrap();
    assert!(!model.allows_chain(piece(PieceId::Station)));
    assert!(!model.allows_chain(piece(PieceId::Brakes)));
    assert!(model.allows_chain(piece(PieceId::TurnSmallStandalone)));
    assert!(!model.allows_chain(piece(PieceId::GentleDown)));
    assert!(!model.allows_chain(piece(PieceId::Loop)));
    assert!(!model.allows_chain(piece(PieceId::AirtimeHill)));
    model.lift_rules.as_mut().unwrap().curved = false;
    assert!(!model.allows_chain(piece(PieceId::TurnSmallStandalone)));
    model.lift_rules.as_mut().unwrap().max_slope = Slope::UpGentle;
    assert!(!model.allows_chain(piece(PieceId::SteepUp)));
    model.lift_rules = None;
    assert!(!model.allows_chain(piece(PieceId::Straight)));
}

#[test]
fn chain_mode_keeps_special_pieces_placeable_and_preserves_their_kind() {
    let world = world();
    for (id, expected_kind) in [
        (PieceId::Station, PieceKind::Station),
        (PieceId::Brakes, PieceKind::Brakes),
        (PieceId::Launch, PieceKind::Launch),
    ] {
        let mut construction = Construction::new(start(), crate::track::multi_launch_model());
        construction.set_chain(true);
        assert_eq!(construction.validate(id, &world, &[]), Ok(()));
        let available = construction.available_choices(&world, &[]);
        let (piece, choice) = available
            .iter()
            .find(|(_, choice)| choice.id == id)
            .unwrap();
        assert_eq!(choice.result, Ok(()));
        assert_eq!(piece.kind, expected_kind);
        construction.append(id, &world, &[]).unwrap();
        assert_eq!(construction.pieces()[0].kind, expected_kind);
    }
}

#[test]
fn construction_places_a_curved_chain_lift() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.set_chain(true);
    construction
        .append(PieceId::TurnSmallStandalone, &world, &[])
        .unwrap();
    let placed = &construction.pieces()[0];
    assert_eq!(placed.kind, PieceKind::Lift);
    assert_ne!(placed.entry.heading, placed.exit.heading);
}

#[test]
fn append_and_delete_keep_connector_state_and_are_atomic_on_failure() {
    let mut construction = Construction::new(start(), hybrid_model());
    assert_eq!(
        construction.append(PieceId::GentleUp, &world(), &[]),
        Err(ConstructionError::Unavailable)
    );
    assert!(construction.pieces().is_empty());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let first_end = construction.end();
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.delete_from(1).unwrap().len(), 1);
    assert_eq!(construction.end(), first_end);
    assert!(matches!(
        construction.delete_from(1),
        Err(ConstructionError::InvalidIndex)
    ));
    assert_eq!(construction.pieces().len(), 1);
    assert_eq!(construction.end().heading, Heading::East);
    assert_eq!(construction.end().slope, Slope::Level);
}

#[test]
fn support_search_failure_does_not_mutate_track() {
    let obstacle = SupportObstacle {
        min: Vec3::new(0.0, 0.0, -1.0),
        max: Vec3::new(512.0, 512.0, 9.0),
    };
    let mut construction = Construction::new(start(), hybrid_model());
    assert_eq!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::NoSupportRoute)
    );
    assert!(construction.pieces().is_empty());
}

#[test]
fn closed_circuit_requires_explicit_close_and_reopens_on_deletion() {
    use PieceId::*;
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world(), &[]).unwrap();
    }
    assert_eq!(construction.end(), start());
    assert!(construction.circuit().is_err());
    construction.close().unwrap();
    assert!(construction.circuit().is_ok());
    assert_eq!(
        construction.append(Straight, &world(), &[]),
        Err(ConstructionError::Closed)
    );
    construction.delete_from(5).unwrap();
    assert!(construction.circuit().is_err());
}

#[test]
fn closed_circuit_has_no_elements_without_populating_search_cache() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    construction.close().unwrap();
    assert!(construction.element_choices(&world, &[]).is_empty());
    assert!(construction.element_choices_cache.borrow().is_none());
    construction.delete_from(5).unwrap();
    assert!(!construction.element_choices(&world, &[]).is_empty());
}

#[test]
fn every_coaster_model_can_build_a_closed_circuit() {
    use PieceId::*;

    let track_world = world();
    for model in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ] {
        let mut construction = Construction::new(start(), model);
        for piece_id in [
            Station,
            TurnSmallStandalone,
            TurnSmallStandalone,
            Straight,
            TurnSmallStandalone,
            TurnSmallStandalone,
        ] {
            construction
                .append(piece_id, &track_world, &[])
                .unwrap_or_else(|error| {
                    panic!(
                        "{} rejected {piece_id:?}: {error:?}",
                        construction.model().name
                    )
                });
        }

        assert_eq!(construction.end(), start(), "{}", construction.model().name);
        construction.close().unwrap();
        let circuit = construction.circuit().unwrap();
        assert!(construction.is_closed());
        assert!(
            circuit.length() > construction.model().train.length_metres,
            "{}",
            construction.model().name
        );
    }
}

#[test]
fn right_turn_circuit_builds_and_completes_a_lap() {
    use PieceId::*;

    let track_world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallRightStandalone,
        TurnSmallRightStandalone,
        Straight,
        TurnSmallRightStandalone,
        TurnSmallRightStandalone,
    ] {
        construction.append(id, &track_world, &[]).unwrap();
    }
    assert_eq!(construction.end(), start());
    construction.close().unwrap();
    let circuit = construction.circuit().unwrap();

    let config = PhysicsConfig {
        rolling_resistance: 0.0,
        drag_coefficient: 0.0,
        powered_sections: false,
        ..PhysicsConfig::default()
    };
    let start_distance = circuit.pieces()[0].length() / 2.0;
    let mut train = Train::new(
        &circuit,
        &construction.model().train,
        config,
        start_distance,
        12.0,
    );
    let mut completed_lap = false;
    for _ in 0..60_000 {
        train.step(&circuit);
        if train.distance() - start_distance >= circuit.length() {
            completed_lap = true;
            break;
        }
    }

    assert!(completed_lap, "the train did not complete one circuit lap");
}

#[test]
fn collision_is_reported_and_does_not_place_a_piece() {
    let obstacle = SupportObstacle {
        min: Vec3::new(106.0, 99.5, 10.0),
        max: Vec3::new(108.0, 100.5, 12.0),
    };
    let mut construction = Construction::new(start(), hybrid_model());
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::Clearance(_))
    ));
    assert!(construction.pieces().is_empty());
}

#[test]
fn piece_choices_tunnel_through_hills_and_report_the_tunnel() {
    let hill = World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    let construction = Construction::new(TrackState { z: 11, ..start() }, hybrid_model());

    let straight = construction
        .choices(&hill, &[])
        .into_iter()
        .find(|choice| choice.id == PieceId::LevelToGentleDown)
        .expect("descending choice exists");

    assert_eq!(straight.result, Ok(()));
    let plan = construction
        .tunnel_plan(PieceId::LevelToGentleDown, &hill, &[])
        .expect("tunnel is valid");
    assert!(straight.would_tunnel, "choice={straight:?} plan={plan:?}");
    assert!(!plan.carve_cells.is_empty());
    assert!(
        plan.swept_cells.iter().all(|cell| {
            hill.ground(cell.x, cell.y, cell.height) == crate::world::Ground::Solid
        })
    );
}

#[test]
fn level_track_can_rest_on_the_exact_floor_of_its_tunnel() {
    let hill = World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    let start = TrackState { z: 5, ..start() };
    let piece = template_for(PieceId::Straight, start).at(start).unwrap();
    let plan = tunnel_plan(&hill, &piece, &hybrid_model().train).unwrap();
    assert_eq!(
        plan.carve_cells.iter().map(|cell| cell.height).min(),
        Some(10),
        "the tunnel floor stays at the clearance envelope's lower boundary"
    );

    let choice = Construction::new(start, hybrid_model())
        .choices(&hill, &[])
        .into_iter()
        .find(|choice| choice.id == PieceId::Straight)
        .expect("level straight choice exists");

    assert_eq!(choice.result, Ok(()), "choice={choice:?}");
    assert!(choice.would_tunnel);
}

#[test]
fn gentle_down_continues_as_it_enters_a_hill() {
    fn append_with_carving(
        construction: &mut Construction,
        world: &mut World,
        id: PieceId,
    ) -> Result<(), ConstructionError> {
        let plan = construction.tunnel_plan(id, world, &[])?;
        let cells: Vec<_> = plan
            .carve_cells
            .iter()
            .map(|cell| (cell.x, cell.y, cell.height))
            .collect();
        if !cells.is_empty() {
            world.carve_cells(&cells).unwrap();
        }
        construction.append(id, world, &[])
    }

    let mut hill = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    for x in 66..=74 {
        for y in 63..=71 {
            hill.set_surface(
                x,
                y,
                Surface {
                    corners: [20; 4],
                    water: None,
                },
            );
        }
    }
    let start = TrackState {
        x: 100,
        y: 100,
        z: 6,
        heading: Heading::East,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, hybrid_model());
    append_with_carving(&mut construction, &mut hill, PieceId::Station).unwrap();
    construction.set_chain(true);
    for id in [
        PieceId::LevelToGentleUp,
        PieceId::GentleUp,
        PieceId::GentleUp,
        PieceId::GentleUp,
        PieceId::GentleUp,
        PieceId::GentleUp,
        PieceId::GentleUpToLevel,
        PieceId::Straight,
    ] {
        append_with_carving(&mut construction, &mut hill, id).unwrap();
    }
    construction.set_chain(false);
    for id in [
        PieceId::OneTileStraight,
        PieceId::OneTileStraight,
        PieceId::OffsetLeft,
        PieceId::TurnLargeEntry,
        PieceId::TurnLargeExit,
        PieceId::LevelToGentleDown,
        PieceId::GentleDown,
        PieceId::GentleDown,
    ] {
        append_with_carving(&mut construction, &mut hill, id).unwrap();
    }

    let choice = construction
        .choices(&hill, &[])
        .into_iter()
        .find(|choice| choice.id == PieceId::GentleDown)
        .expect("next gentle descent exists");
    assert_eq!(
        choice.result,
        Ok(()),
        "choice={choice:?}; entry={:?}",
        construction.end()
    );
}

#[test]
fn tunnel_plan_refuses_water_and_paths() {
    let construction = Construction::new(TrackState { z: 10, ..start() }, hybrid_model());
    let water = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: Some(20),
        },
    );
    assert_eq!(
        construction.tunnel_plan(PieceId::Straight, &water, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Water))
    );

    let mut path_world = World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    path_world.place_path(crate::world::Path {
        position: crate::world::Position {
            x: 25,
            y: 25,
            height: 20,
        },
        slope: None,
    });
    assert_eq!(
        construction.tunnel_plan(PieceId::Straight, &path_world, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Path))
    );
}

#[test]
fn tunnel_plan_refuses_facility_obstacles() {
    let hill = World::new(
        128,
        128,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    let construction = Construction::new(TrackState { z: 10, ..start() }, hybrid_model());
    let facility = SupportObstacle {
        min: Vec3::new(99.0, 98.0, 17.0),
        max: Vec3::new(105.0, 103.0, 24.0),
    };

    assert!(matches!(
        construction.tunnel_plan(PieceId::Straight, &hill, &[facility]),
        Err(ConstructionError::Clearance(ClearanceCollision::Scenery))
    ));
}

#[test]
fn supports_route_around_existing_track_and_train_space() {
    let model = hybrid_model();
    let existing = model
        .available_pieces(TrackState {
            y: 48,
            z: 3,
            ..start()
        })
        .into_iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let crossing = model
        .available_pieces(TrackState {
            x: 54,
            y: 46,
            heading: Heading::North,
            ..start()
        })
        .into_iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let obstacles = track_support_obstacles(&[existing], &model.train);
    let supports = generate_supports(&crossing, &world(), &obstacles).unwrap();
    let crossing_support = &supports[0];
    assert!(
        crossing_support.footing.x != crossing_support.anchor.x
            || crossing_support.footing.y != crossing_support.anchor.y
    );
}

#[test]
fn deleting_middle_preserves_suffix_ids_and_reconnects_when_rebuilt() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..3 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let ids = construction.piece_ids().to_vec();
    assert_eq!(construction.delete_from(ids[1]).unwrap().len(), 1);
    assert_eq!(construction.piece_ids(), &ids[..1]);
    assert_eq!(construction.disconnected_segments()[0].ids, &ids[2..]);
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    assert_eq!(construction.pieces().len(), 3);
    assert_eq!(construction.piece_ids()[0], ids[0]);
    assert_eq!(construction.piece_ids()[2], ids[2]);
    assert_ne!(construction.piece_ids()[1], ids[1]);
    assert!(construction.disconnected_segments().is_empty());
}

#[test]
fn deleting_disconnected_piece_preserves_both_sides() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..5 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let ids = construction.piece_ids().to_vec();
    construction.delete_from(ids[1]).unwrap();
    construction.delete_from(ids[3]).unwrap();
    let segments = construction.disconnected_segments();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].ids, [ids[2]]);
    assert_eq!(segments[1].ids, [ids[4]]);
}

#[test]
fn append_revalidates_existing_supports() {
    let mut construction = Construction::new(start(), hybrid_model());
    construction
        .append(PieceId::Straight, &world(), &[])
        .unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(103.0, 99.0, 9.0),
        max: Vec3::new(105.0, 101.0, 11.0),
    };
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::NoSupportRoute)
    ));
    assert_eq!(construction.pieces().len(), 1);
}

#[test]
fn wooden_inversion_is_rejected_even_if_added_to_allowed_ids() {
    let mut model = crate::track::wooden_twister_model();
    model.allowed_pieces.push(PieceId::Loop);
    let mut construction = Construction::new(start(), model);
    assert_eq!(
        construction.append(PieceId::Loop, &world(), &[]),
        Err(ConstructionError::ModelForbidden)
    );
}

#[test]
fn model_radius_and_inversion_restrictions_have_model_reason() {
    let mut model = hybrid_model();
    model.allowed_inversions.clear();
    model.minimum_radius_metres = 100.0;
    let mut construction = Construction::new(start(), model);
    for id in [PieceId::Loop, PieceId::TurnSmallStandalone] {
        assert_eq!(
            construction
                .choices(&world(), &[])
                .iter()
                .find(|choice| choice.id == id)
                .unwrap()
                .result,
            Err(ConstructionError::ModelForbidden)
        );
        assert_eq!(
            construction.append(id, &world(), &[]),
            Err(ConstructionError::ModelForbidden)
        );
    }
}

#[test]
fn reconnect_checks_retained_track_against_current_obstacles() {
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction
            .append(PieceId::Straight, &world(), &[])
            .unwrap();
    }
    let first = construction.piece_ids()[0];
    construction.delete_from(first).unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(149.0, 99.5, 10.0),
        max: Vec3::new(151.0, 100.5, 12.0),
    };
    assert!(matches!(
        construction.append(PieceId::Straight, &world(), &[obstacle]),
        Err(ConstructionError::Clearance(_))
    ));
    assert!(construction.pieces().is_empty());
    assert_eq!(construction.disconnected_segments()[0].pieces.len(), 3);
}

#[test]
fn choices_reflect_world_obstacle_and_track_changes() {
    let mut world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let initial = construction.choices(&world, &[]);
    assert_eq!(
        initial
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
    let obstacle = SupportObstacle {
        min: Vec3::new(119.0, 99.0, 9.0),
        max: Vec3::new(121.0, 101.0, 13.0),
    };
    assert!(
        construction
            .append_cached_choice(PieceId::Straight, &world, &[obstacle])
            .is_err()
    );
    assert_eq!(construction.pieces().len(), 1);
    assert_eq!(construction.choices(&world, &[]), initial);
    world.set_surface(
        30,
        25,
        Surface {
            corners: [20; 4],
            water: None,
        },
    );
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(()),
        "terrain alone should be tunneled through"
    );
    let water_world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: Some(20),
        },
    );
    assert_eq!(
        construction
            .choices(&water_world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Err(ConstructionError::Clearance(ClearanceCollision::Water)),
        "water in the candidate clearance should still block the choice"
    );
    assert_eq!(construction.pieces().len(), 1);
    construction.delete_from(0).unwrap();
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == PieceId::Straight)
            .unwrap()
            .result,
        Ok(())
    );
}

#[test]
fn targeted_piece_selection_matches_full_choice_priority_without_building_the_menu() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    let connector = construction.end();
    let same_orientation = |first: TrackState, second: TrackState| {
        first.heading == second.heading
            && first.slope == second.slope
            && first.bank == second.bank
            && first.horizontal_curvature == second.horizontal_curvature
            && first.vertical_curvature == second.vertical_curvature
    };

    let targeted = construction.first_valid_piece(&world, &[], |piece| {
        piece.inversion.is_none()
            && matches!(piece.kind, PieceKind::Track | PieceKind::Lift)
            && same_orientation(piece.entry, connector)
            && same_orientation(piece.exit, connector)
    });
    assert!(construction.choices_cache.borrow().is_none());

    let choices = construction.available_choices(&world, &[]);
    let expected = choices
        .iter()
        .filter(|(_, choice)| choice.result.is_ok())
        .find(|(piece, _)| {
            piece.inversion.is_none()
                && matches!(piece.kind, PieceKind::Track | PieceKind::Lift)
                && same_orientation(piece.entry, connector)
                && same_orientation(piece.exit, connector)
        })
        .or_else(|| choices.iter().find(|(_, choice)| choice.result.is_ok()))
        .map(|(piece, _)| piece.id);

    assert_eq!(targeted, expected);
}

#[test]
fn appending_a_cached_choice_without_a_menu_validates_only_that_piece() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());

    construction
        .append_cached_choice(PieceId::Straight, &world, &[])
        .unwrap();

    assert_eq!(construction.pieces().len(), 1);
    assert_eq!(construction.pieces()[0].id, PieceId::Straight);
    assert!(construction.choices_cache.borrow().is_none());
}

#[test]
fn cached_choices_match_fresh_validation_after_track_edits() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    construction.delete_from(1).unwrap();
    let cached = construction.choices(&world, &[]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[]));
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let cached = construction.choices(&world, &[]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[]));
}

#[test]
fn failed_support_search_is_rechecked_when_candidate_is_nearby() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Straight, &world, &[]).unwrap();
    let obstacle = SupportObstacle {
        min: Vec3::new(103.0, 99.0, 9.0),
        max: Vec3::new(105.0, 101.0, 11.0),
    };
    let cached = construction.choices(&world, &[obstacle]);
    *construction.validation_cache.borrow_mut() = None;
    *construction.choices_cache.borrow_mut() = None;
    assert_eq!(cached, construction.choices(&world, &[obstacle]));
}

#[test]
fn nearby_track_obstacle_filter_preserves_support_routes() {
    let world = world();
    let model = hybrid_model();
    let mut construction = Construction::new(start(), model.clone());
    for _ in 0..8 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for (index, piece) in construction.pieces().iter().enumerate() {
        let all: Vec<_> = construction
            .pieces()
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .flat_map(|(_, other)| {
                track_support_obstacles(std::slice::from_ref(other), &model.train)
            })
            .collect();
        let region = support_search_region(piece);
        let nearby: Vec<_> = all
            .iter()
            .copied()
            .filter(|obstacle| region.overlaps(obstacle))
            .collect();
        assert_eq!(
            generate_supports(piece, &world, &all),
            generate_supports(piece, &world, &nearby)
        );
    }
}

#[test]
fn available_choices_match_model_candidates_before_and_after_closing() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let expected: Vec<_> = construction
        .model()
        .available_pieces(construction.end())
        .into_iter()
        .map(|piece| (piece.id, piece.exit))
        .collect();
    for closed in [false, true] {
        if closed {
            construction.close().unwrap();
        }
        let actual = construction.available_choices(&world, &[]);
        assert_eq!(
            actual
                .iter()
                .map(|(piece, _)| (piece.id, piece.exit))
                .collect::<Vec<_>>(),
            expected
        );
        if closed {
            assert!(
                actual
                    .iter()
                    .all(|(_, choice)| choice.result == Err(ConstructionError::Closed))
            );
        }
    }
}

#[test]
fn choices_after_deleting_from_150_piece_circuit() {
    let world = World::new(
        1024,
        1024,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let mut construction = Construction::new(TrackState { z: 160, ..start() }, hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for _ in 0..72 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for _ in 0..2 {
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
    }
    for _ in 0..73 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    for _ in 0..2 {
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
    }
    assert_eq!(construction.pieces().len(), 150);
    construction.close().unwrap();
    construction.delete_from(75).unwrap();
    construction.validation_cache_rebuilds.set(0);
    construction.support_cache_misses.set(0);
    let choices = construction.choices(&world, &[]);
    assert!(choices.iter().any(|choice| choice.result.is_ok()));
    assert_eq!(construction.validation_cache_rebuilds.get(), 0);
    assert_eq!(construction.support_cache_misses.get(), 0);
}

#[test]
fn choices_match_individual_validation_for_reconnecting_candidates() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for _ in 0..4 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    construction.delete_from(1).unwrap();
    for choice in construction.choices(&world, &[]) {
        assert_eq!(
            choice.result,
            construction.validate(choice.id, &world, &[]),
            "{:?}",
            choice.id
        );
    }
}

#[test]
fn mirrored_inversion_choices_match_individual_validation_near_asymmetric_obstacles() {
    use PieceId::{Corkscrew, CorkscrewRight, HeartlineRoll, HeartlineRollRight};

    let world = world();
    let construction = Construction::new(start(), hybrid_model());
    for (left, right) in [
        (Corkscrew, CorkscrewRight),
        (HeartlineRoll, HeartlineRollRight),
    ] {
        let piece = construction
            .model_placeable(templates().iter().find(|piece| piece.id == left).unwrap())
            .unwrap();
        let obstacle = (1..20)
            .flat_map(|step| {
                let sample = piece.sample(piece.length() * f64::from(step) / 20.0);
                [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]
                    .into_iter()
                    .map(move |offset| {
                        let centre = sample.position + Vec3::new(0.0, offset, 0.0);
                        SupportObstacle {
                            min: centre - Vec3::new(0.1, 0.1, 0.1),
                            max: centre + Vec3::new(0.1, 0.1, 0.1),
                        }
                    })
            })
            .find(|obstacle| {
                let results = (
                    construction.validate(left, &world, &[*obstacle]),
                    construction.validate(right, &world, &[*obstacle]),
                );
                matches!(
                    results,
                    (
                        Ok(()),
                        Err(ConstructionError::Clearance(ClearanceCollision::Scenery))
                    ) | (
                        Err(ConstructionError::Clearance(ClearanceCollision::Scenery)),
                        Ok(())
                    )
                )
            })
            .expect("an asymmetric obstacle distinguishes both hands");
        for choice in construction.choices(&world, &[obstacle]) {
            assert_eq!(
                choice.result,
                construction.validate(choice.id, &world, &[obstacle]),
                "{:?} with obstacle for {:?}",
                choice.id,
                left
            );
        }
    }
}

#[test]
fn elevated_crossing_rejects_rail_through_existing_train_space() {
    use PieceId::*;
    let world = world();
    let mut model = hybrid_model();
    model.train.height_metres = 10.0;
    let mut construction = Construction::new(TrackState { z: 20, ..start() }, model);
    for id in [
        Straight,
        Straight,
        LevelToGentleUp,
        GentleUpToLevel,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
        Straight,
        Straight,
        TurnSmallStandalone,
        Straight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let candidate = construction
        .model_placeable(
            templates()
                .iter()
                .find(|piece| piece.id == Straight)
                .unwrap(),
        )
        .unwrap();
    let train = &construction.model().train;
    assert_eq!(
        crate::track::clearance::check_clearance(
            &world,
            &candidate,
            train,
            construction.pieces(),
            &[]
        ),
        Ok(())
    );
    assert!(construction.pieces().iter().any(|existing| {
        crate::track::clearance::check_clearance(
            &world,
            existing,
            train,
            std::slice::from_ref(&candidate),
            &[],
        ) == Err(ClearanceCollision::Track)
    }));
    let previous_count = construction.pieces().len();
    assert_eq!(
        construction
            .choices(&world, &[])
            .iter()
            .find(|choice| choice.id == Straight)
            .unwrap()
            .result,
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(
        construction.validate(Straight, &world, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(
        construction.append(Straight, &world, &[]),
        Err(ConstructionError::Clearance(ClearanceCollision::Track))
    );
    assert_eq!(construction.pieces().len(), previous_count);
}

#[test]
fn climbing_turn_choices_validate_and_append_the_matching_slope() {
    let world = world();
    for slope in [Slope::UpGentle, Slope::DownGentle] {
        let mut construction = Construction::new(
            TrackState {
                z: 30,
                slope,
                ..start()
            },
            hybrid_model(),
        );
        let choices = construction.choices(&world, &[]);
        assert_eq!(
            choices
                .iter()
                .filter(|choice| choice.id == PieceId::TurnSmallStandalone)
                .count(),
            1
        );
        assert_eq!(
            construction.validate(PieceId::TurnSmallStandalone, &world, &[]),
            Ok(())
        );
        construction
            .append(PieceId::TurnSmallStandalone, &world, &[])
            .unwrap();
        assert_eq!(construction.end().slope, slope);
    }
}

#[test]
fn closure_with_short_straights_and_full_tile_offsets() {
    use PieceId::*;
    let track_world = world();
    for model in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ] {
        let mut construction = Construction::new(start(), model);
        for (index, id) in [
            Station,
            HalfTileStraight,
            FullOffsetLeft,
            TurnSmallStandalone,
            TurnSmallStandalone,
            Straight,
            HalfTileStraight,
            FullOffsetLeft,
            TurnSmallStandalone,
            TurnSmallStandalone,
        ]
        .into_iter()
        .enumerate()
        {
            construction
                .append(id, &track_world, &[])
                .unwrap_or_else(|error| {
                    panic!(
                        "{} rejected {id:?} at {index}: {error:?}",
                        construction.model().name
                    )
                });
        }
        assert_eq!(construction.end(), start(), "{}", construction.model().name);
        construction.close().unwrap();
    }
}

#[test]
fn completion_finds_a_valid_deterministic_nearby_connection() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let original_hash = construction.state_hash();
    let completion = construction
        .find_completion(&world, &[])
        .expect("nearby connection");
    assert!(!completion.pieces.is_empty());
    assert!(completion.score.is_finite());
    let repeated = construction.find_completion(&world, &[]).unwrap();
    assert_eq!(completion.pieces, repeated.pieces);
    assert_eq!(completion.score, repeated.score);
    assert_eq!(original_hash, construction.state_hash());
    for id in completion.pieces {
        construction.append(id, &world, &[]).unwrap();
    }
    assert_eq!(construction.end(), construction.start());
    construction.close().unwrap();
}

#[test]
fn completion_rejects_a_distant_end() {
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for _ in 0..20 {
        construction.append(PieceId::Straight, &world, &[]).unwrap();
    }
    assert!(construction.find_completion(&world, &[]).is_none());
}

#[test]
fn completion_search_yields_and_application_is_atomic() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        TurnSmallStandalone,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut search = construction.completion_search().unwrap();
    assert!(matches!(
        search.advance(&world, &[]),
        CompletionSearchProgress::Pending
    ));
    let completion = loop {
        let progress = search.advance(&world, &[]);
        if let CompletionSearchProgress::Finished(result) = progress {
            break result.unwrap();
        }
    };
    let hash = construction.state_hash();
    let mut invalid = completion.clone();
    invalid.pieces.push(Straight);
    assert!(
        construction
            .apply_completion(&invalid, &world, &[])
            .is_err()
    );
    assert_eq!(construction.state_hash(), hash);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn completion_connects_a_short_turn_sequence() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [Station, TurnSmallStandalone, TurnSmallStandalone, Straight] {
        construction.append(id, &world, &[]).unwrap();
    }
    let completion = construction
        .find_completion(&world, &[])
        .expect("two-turn connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn completion_connects_a_laterally_offset_approach() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        Straight,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        OffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    expected.append(OffsetRight, &world, &[]).unwrap();
    assert_eq!(expected.end(), expected.start());
    let completion = construction
        .find_completion(&world, &[])
        .expect("offset connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_connects_an_offset_followed_by_turns() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [
        Station,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        OffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    for id in [OffsetRight, TurnSmallStandalone, TurnSmallStandalone] {
        expected.append(id, &world, &[]).unwrap();
    }
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let mut search = construction.completion_search().unwrap();
    let completion = loop {
        let progress = search.advance(&world, &[]);
        if let CompletionSearchProgress::Finished(result) = progress {
            break result.expect("offset and turns connection");
        }
    };
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_uses_short_offsets_for_a_tight_station_connection() {
    use PieceId::*;
    let world = world();
    let allowed_pieces = [
        Station,
        Straight,
        TurnSmallStandalone,
        ShortOffsetLeft,
        ShortOffsetRight,
    ];
    let mut model = hybrid_model();
    model.allowed_pieces = allowed_pieces.to_vec();
    let mut construction = Construction::new(start(), model);
    for id in [
        Station,
        Straight,
        Straight,
        TurnSmallStandalone,
        TurnSmallStandalone,
        ShortOffsetLeft,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut without_short_offsets = construction.copy_for_completion();
    without_short_offsets
        .model
        .allowed_pieces
        .retain(|id| !matches!(id, ShortOffsetLeft | ShortOffsetRight));
    assert!(without_short_offsets.find_completion(&world, &[]).is_none());
    let completion = construction
        .find_completion(&world, &[])
        .expect("short offset connection");
    assert!(
        completion
            .pieces
            .iter()
            .any(|id| matches!(id, ShortOffsetLeft | ShortOffsetRight))
    );
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
    assert!(construction.is_closed());
}

#[test]
fn completion_connects_a_sloped_approach() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(TrackState { z: 20, ..start() }, hybrid_model());
    for id in [
        Station,
        LevelToGentleUp,
        GentleUpToLevel,
        TurnSmallStandalone,
        TurnSmallStandalone,
        Straight,
        LevelToGentleDown,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let mut expected = construction.copy_for_completion();
    for id in [GentleDownToLevel, TurnSmallStandalone, TurnSmallStandalone] {
        expected.append(id, &world, &[]).unwrap();
    }
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let completion = construction
        .find_completion(&world, &[])
        .expect("sloped connection");
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn completion_finishes_the_lift_powered_circuit_after_a_loop() {
    use PieceId::*;
    let world = world();
    let mut construction = Construction::new(TrackState { z: 36, ..start() }, hybrid_model());
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
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| {
                panic!(
                    "cannot append {id:?} at {:?}: {error:?}",
                    construction.end()
                )
            });
    }
    let mut expected = construction.copy_for_completion();
    expected.append(OneTileStraight, &world, &[]).unwrap();
    assert_eq!(expected.end(), expected.start());
    expected.close().unwrap();
    let completion = construction
        .find_completion(&world, &[])
        .expect("one-tile straight");
    assert_eq!(completion.pieces, [OneTileStraight]);
    construction
        .apply_completion(&completion, &world, &[])
        .unwrap();
}

#[test]
fn construction_closes_a_corkscrew_side_exit() {
    use PieceId::*;

    let world = world();
    let mut construction = Construction::new(start(), hybrid_model());
    for id in [Station, Corkscrew] {
        construction.append(id, &world, &[]).unwrap();
    }

    let corkscrew = construction.pieces().last().unwrap();
    assert_eq!(corkscrew.exit.y - corkscrew.entry.y, 8);
    let model = construction.model();
    let return_options = [
        Straight,
        TurnSmallStandalone,
        TurnSmallRightStandalone,
        TurnLargeStandalone,
        TurnLargeRightStandalone,
        OffsetLeft,
        OffsetRight,
    ];
    let key = |state: TrackState| {
        (
            state.x,
            state.y,
            state.z,
            state.heading as u8,
            state.slope as u8,
            state.bank as u8,
            state.horizontal_curvature as u8,
            state.vertical_curvature as u8,
        )
    };
    let start_key = key(construction.start());
    let mut queue = VecDeque::from([(construction.end(), Vec::new())]);
    let mut visited = HashSet::from([key(construction.end())]);
    let return_path = loop {
        let (state, path) = queue.pop_front().expect("level return path exists");
        if key(state) == start_key {
            break path;
        }
        if path.len() >= 40 {
            continue;
        }
        let available = model.available_pieces(state);
        for id in return_options {
            let Some(piece) = available.iter().find(|piece| piece.id == id) else {
                continue;
            };
            if !(0..128).contains(&piece.exit.x) || !(0..128).contains(&piece.exit.y) {
                continue;
            }
            if !visited.insert(key(piece.exit)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((piece.exit, next_path));
        }
    };
    for id in return_path {
        construction
            .append(id, &world, &[])
            .unwrap_or_else(|error| panic!("cannot append {id:?}: {error:?}"));
    }
    assert_eq!(construction.end(), construction.start());
    construction.close().unwrap();

    assert!(construction.is_closed());
    assert_eq!(
        construction.pieces().last().unwrap().exit,
        construction.start()
    );
}

#[test]
fn raven_turns_reverse_heading_and_offset_the_exit() {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let construction = Construction::new(
        TrackState {
            x: 200,
            y: 200,
            z: 30,
            ..start()
        },
        hybrid_model(),
    );
    let choices = construction.element_choices(&world, &[]);
    for (id, lateral_sign) in [("raven-turn-left", -1), ("raven-turn-right", 1)] {
        let choice = choices
            .iter()
            .find(|choice| choice.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        let mut placed = construction.copy_for_completion();
        placed.apply_element(choice, &world, &[]).unwrap();
        assert_eq!(
            placed.end().heading.index(),
            (construction.end().heading.index() + 4) % 8
        );
        assert_eq!(
            (placed.end().y - construction.end().y).signum(),
            lateral_sign
        );
        assert_eq!(placed.end().bank, crate::track::Bank::Level);
    }
}

#[test]
fn sea_serpent_elements_reverse_heading_twice() {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let construction = Construction::new(
        TrackState {
            x: 200,
            y: 200,
            z: 30,
            ..start()
        },
        hybrid_model(),
    );
    let choices = construction.element_choices(&world, &[]);
    for id in ["sea-serpent-left", "sea-serpent-right"] {
        let choice = choices
            .iter()
            .find(|choice| choice.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(choice.geometry.len(), 4);
        for pair in choice.geometry.windows(2) {
            let end = pair[0].sample(pair[0].length());
            let start = pair[1].sample(0.0);
            assert!((end.position - start.position).length() < 1e-6);
            assert!(end.tangent.dot(start.tangent) > 0.999999);
            assert!(end.up.dot(start.up) > 0.999999);
        }
        let roll_change =
            |piece: &Piece| piece.sample(piece.length()).roll - piece.sample(0.0).roll;
        assert!(roll_change(&choice.geometry[1]) * roll_change(&choice.geometry[2]) > 0.0);
        assert_eq!(
            choice.geometry[1].exit.heading.index(),
            (construction.end().heading.index() + 4) % 8
        );
        let mut placed = construction.copy_for_completion();
        placed.apply_element(choice, &world, &[]).unwrap();
        assert_eq!(placed.end().bank, crate::track::Bank::Level);
        assert_eq!(placed.end().heading, construction.end().heading);
    }
}

#[test]
fn named_inversion_elements_compose_half_pieces() {
    let world = world();
    let construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let choices = construction.element_choices(&world, &[]);
    for (id, pieces) in [
        (
            "immelmann-left",
            [PieceId::HalfLoopUp, PieceId::HalfCorkscrewLeftDown],
        ),
        (
            "immelmann-right",
            [PieceId::HalfLoopUp, PieceId::HalfCorkscrewRightDown],
        ),
        (
            "dive-loop-left",
            [PieceId::HalfCorkscrewLeftUp, PieceId::HalfLoopDown],
        ),
        (
            "dive-loop-right",
            [PieceId::HalfCorkscrewRightUp, PieceId::HalfLoopDown],
        ),
    ] {
        let choice = choices
            .iter()
            .find(|choice| choice.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(choice.pieces, pieces);
        let validated = construction
            .append_sequence(&pieces, &world, &[])
            .expect("the uncached sequence should validate");
        let half_roll = validated
            .pieces()
            .iter()
            .find(|piece| {
                matches!(
                    piece.id,
                    PieceId::HalfCorkscrewLeftUp
                        | PieceId::HalfCorkscrewRightUp
                        | PieceId::HalfCorkscrewLeftDown
                        | PieceId::HalfCorkscrewRightDown
                )
            })
            .expect("inversion composition has one half roll");
        assert_eq!(
            (half_roll.exit.x - half_roll.entry.x).abs()
                + (half_roll.exit.y - half_roll.entry.y).abs(),
            20,
            "half-roll connector footprint grows to 20 grid units"
        );
        assert_eq!((half_roll.exit.z - half_roll.entry.z).abs(), 6);
        assert_eq!(choice.geometry.len(), validated.pieces().len());
        for (cached, revalidated) in choice.geometry.iter().zip(validated.pieces()) {
            assert_eq!(cached.id, revalidated.id);
            assert_eq!(cached.entry, revalidated.entry);
            assert_eq!(cached.exit, revalidated.exit);
            assert_eq!(cached.length(), revalidated.length());
        }
        let mut placed = construction.copy_for_completion();
        placed.apply_element(choice, &world, &[]).unwrap();
        assert_eq!(placed.end().bank, crate::track::Bank::Level);
    }
}

#[test]
fn elements_offer_complete_slopes_and_banked_turns_and_apply_atomically() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let elements = construction.element_choices(&world, &[]);
    for id in [
        "climb-60",
        "descend-60",
        "small-left-banked-turn",
        "small-right-banked-turn",
        "large-left-banked-turn",
        "large-right-banked-turn",
    ] {
        assert!(
            elements.iter().any(|element| element.id == id),
            "missing {id}"
        );
    }
    let climb = elements
        .iter()
        .find(|element| element.id == "climb-60")
        .unwrap();
    assert_eq!(climb.pieces, vec![PieceId::LevelToSixtyUp]);
    let mut stale = construction.copy_for_completion();
    stale.append(PieceId::Straight, &world, &[]).unwrap();
    let before = stale.state_hash();
    assert!(stale.apply_element(climb, &world, &[]).is_err());
    assert_eq!(stale.state_hash(), before);
    construction.apply_element(climb, &world, &[]).unwrap();
    assert_eq!(construction.end().slope, Slope::UpSixty);
    assert!(
        construction
            .element_choices(&world, &[])
            .iter()
            .any(|element| element.id == "level")
    );
}

#[test]
fn elements_support_chain_and_gentle_sloped_turns() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    construction.set_chain(true);
    let climb = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "climb-gentle")
        .unwrap();
    construction.apply_element(&climb, &world, &[]).unwrap();
    assert!(
        construction
            .pieces()
            .iter()
            .all(|piece| piece.kind == PieceKind::Lift)
    );
    construction.set_chain(false);
    let turn = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "large-left-banked-turn")
        .expect("sloped banked turn");
    construction.apply_element(&turn, &world, &[]).unwrap();
    assert_eq!(construction.end().slope, Slope::UpGentle);
    assert_eq!(construction.end().bank, crate::track::Bank::Level);
}

#[test]
fn elements_offer_climbing_and_descending_turns_from_level() {
    let world = world();
    let construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let choices = construction.element_choices(&world, &[]);
    for (id, slope) in [
        ("climbing-small-left-turn", Slope::UpGentle),
        ("climbing-large-right-turn", Slope::UpGentle),
        ("descending-small-right-turn", Slope::DownGentle),
        ("descending-large-left-turn", Slope::DownGentle),
    ] {
        let choice = choices.iter().find(|choice| choice.id == id).unwrap();
        let mut placed = construction.copy_for_completion();
        placed.apply_element(choice, &world, &[]).unwrap();
        assert_eq!(placed.end().slope, slope);
        assert_ne!(placed.end().heading, construction.end().heading);
    }
}

#[test]
fn element_apply_revalidates_world_and_keeps_original_on_failure() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    let choice = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|element| element.id == "small-left-banked-turn")
        .unwrap();
    let blocked = SupportObstacle {
        min: Vec3::new(-1000.0, -1000.0, -1000.0),
        max: Vec3::new(1000.0, 1000.0, 1000.0),
    };
    let before = construction.state_hash();
    assert!(
        construction
            .apply_element(&choice, &world, &[blocked])
            .is_err()
    );
    assert_eq!(construction.state_hash(), before);
    assert!(
        !construction
            .element_choices(&world, &[blocked])
            .iter()
            .any(|element| element.id == choice.id)
    );
}

#[test]
fn element_preview_excludes_reattached_track() {
    let world = world();
    let mut construction = Construction::new(TrackState { z: 30, ..start() }, hybrid_model());
    construction
        .append(PieceId::LevelToGentleUp, &world, &[])
        .unwrap();
    construction.append(PieceId::GentleUp, &world, &[]).unwrap();
    let first_id = construction.piece_ids()[0];
    construction.delete_from(first_id).unwrap();
    let choice = construction
        .element_choices(&world, &[])
        .into_iter()
        .find(|choice| choice.id == "climb-gentle")
        .unwrap();
    assert_eq!(choice.geometry.len(), choice.pieces.len());
    construction.apply_element(&choice, &world, &[]).unwrap();
    assert_eq!(construction.pieces().len(), 2);
    assert!(construction.disconnected_segments().is_empty());
}

#[test]
fn direct_sixty_degree_transitions_are_available_and_complete_a_lap() {
    use PieceId::*;

    let track_world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    for model in [hybrid_model(), crate::track::multi_launch_model()] {
        let origin = TrackState {
            x: 100,
            y: 100,
            z: 40,
            ..TrackState::origin()
        };
        let mut lift = Construction::new(origin, model.clone());
        lift.set_chain(true);
        let climb = lift
            .element_choices(&track_world, &[])
            .into_iter()
            .find(|choice| choice.id == "climb-60")
            .unwrap();
        assert_eq!(climb.pieces, vec![LevelToSixtyUp]);
        lift.apply_element(&climb, &track_world, &[]).unwrap();
        let level = lift
            .element_choices(&track_world, &[])
            .into_iter()
            .find(|choice| choice.id == "level")
            .unwrap();
        assert_eq!(level.pieces, vec![SixtyUpToLevel]);
        lift.apply_element(&level, &track_world, &[]).unwrap();
        assert!(
            lift.pieces()
                .iter()
                .all(|piece| piece.kind == PieceKind::Lift)
        );
        let mut construction = Construction::new(origin, model);
        for (id, element) in [
            (Station, ""),
            (LevelToSixtyUp, "climb-60"),
            (SixtyUpToLevel, "level"),
            (TurnSmallRightStandalone, ""),
            (TurnSmallRightStandalone, ""),
            (Straight, ""),
            (LevelToSixtyDown, "descend-60"),
            (SixtyDownToLevel, "level"),
            (TurnSmallRightStandalone, ""),
            (TurnSmallRightStandalone, ""),
        ] {
            if !element.is_empty() {
                let choice = construction
                    .element_choices(&track_world, &[])
                    .into_iter()
                    .find(|choice| choice.id == element)
                    .unwrap();
                assert_eq!(choice.pieces, vec![id]);
            }
            construction.append(id, &track_world, &[]).unwrap();
        }
        assert_eq!(construction.end(), origin);
        construction.close().unwrap();
        let circuit = construction.circuit().unwrap();
        let config = PhysicsConfig {
            powered_sections: false,
            ..PhysicsConfig::default()
        };
        let mut train = Train::new(&circuit, &construction.model().train, config, 0.0, 50.0);
        for _ in 0..60_000 {
            train.step(&circuit);
            if train.distance() >= circuit.length() {
                break;
            }
        }
        assert!(
            train.distance() >= circuit.length(),
            "{} did not complete a lap",
            construction.model().name
        );
    }
}

fn level_return_path(start: TrackState, target: TrackState, model: &CoasterModel) -> Vec<PieceId> {
    let key = |state: TrackState| {
        (
            state.x,
            state.y,
            state.z,
            state.heading as u8,
            state.slope as u8,
            state.bank as u8,
            state.horizontal_curvature as u8,
            state.vertical_curvature as u8,
        )
    };
    let target_key = key(target);
    let candidates = [
        PieceId::Straight,
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallRightStandalone,
        PieceId::TurnLargeStandalone,
        PieceId::TurnLargeRightStandalone,
        PieceId::OffsetLeft,
        PieceId::OffsetRight,
    ];
    let mut queue = VecDeque::from([(start, Vec::new())]);
    let mut visited = HashSet::from([key(start)]);
    loop {
        let (state, path) = queue.pop_front().expect("level return path exists");
        if key(state) == target_key {
            return path;
        }
        if path.len() >= 80 {
            continue;
        }
        let available = model.available_pieces(state);
        for id in candidates {
            let Some(piece) = available.iter().find(|piece| piece.id == id) else {
                continue;
            };
            if !(0..256).contains(&piece.exit.x) || !(0..256).contains(&piece.exit.y) {
                continue;
            }
            if !visited.insert(key(piece.exit)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((piece.exit, next_path));
        }
    }
}

#[test]
fn closed_inversion_circuit_support_positions_snapshot() {
    use PieceId::*;

    let track_world = World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let circuit_start = TrackState {
        x: 80,
        y: 80,
        z: 10,
        ..TrackState::origin()
    };
    let model = hybrid_model();
    let mut construction = Construction::new(circuit_start, model.clone());
    for id in [
        Station,
        Loop,
        Corkscrew,
        HalfCorkscrewLeftUp,
        HalfCorkscrewLeftDown,
    ] {
        construction
            .append(id, &track_world, &[])
            .unwrap_or_else(|error| panic!("construction rejected {id:?}: {error:?}"));
    }
    let climb_pieces = [LevelToGentleUp]
        .into_iter()
        .chain(std::iter::repeat_n(GentleUp, 5))
        .chain([GentleUpToLevel]);
    for id in climb_pieces {
        construction
            .append(id, &track_world, &[])
            .unwrap_or_else(|error| panic!("construction rejected climb piece {id:?}: {error:?}"));
    }
    let return_target = TrackState {
        x: circuit_start.x - 56,
        y: circuit_start.y,
        z: circuit_start.z + 24,
        ..circuit_start
    };
    for id in level_return_path(construction.end(), return_target, &model) {
        construction
            .append(id, &track_world, &[])
            .unwrap_or_else(|error| panic!("construction rejected return piece {id:?}: {error:?}"));
    }
    let descent_pieces = [LevelToGentleDown]
        .into_iter()
        .chain(std::iter::repeat_n(GentleDown, 5))
        .chain([GentleDownToLevel]);
    for id in descent_pieces {
        construction
            .append(id, &track_world, &[])
            .unwrap_or_else(|error| {
                panic!("construction rejected descent piece {id:?}: {error:?}")
            });
    }
    assert_eq!(construction.end(), circuit_start);
    construction.close().unwrap();
    let circuit = construction.circuit().unwrap();
    assert!(construction.is_closed());
    assert!(circuit.length() > construction.model().train.length_metres);

    let cache = construction.validation_cache.borrow();
    let cache = cache.as_ref().expect("construction validation cache");
    assert_eq!(cache.supports.len(), construction.pieces().len());
    let rounded = |position: Vec3| {
        Vec3::new(
            (position.x * 1e6).round() / 1e6,
            (position.y * 1e6).round() / 1e6,
            (position.z * 1e6).round() / 1e6,
        )
    };
    let snapshot: Vec<_> = construction
        .pieces()
        .iter()
        .zip(&cache.supports)
        .map(|(piece, result)| {
            let positions = result
                .as_ref()
                .expect("support validation result")
                .as_ref()
                .expect("validated support positions")
                .iter()
                .map(|support| {
                    let anchor = rounded(support.anchor);
                    let footing = rounded(support.footing);
                    format!(
                        "({:.6},{:.6},{:.6};{:.6},{:.6},{:.6})",
                        anchor.x, anchor.y, anchor.z, footing.x, footing.y, footing.z,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{:?}: [{positions}]", piece.id)
        })
        .collect();
    let snapshot = format!("{}\n", snapshot.join("\n"));
    if std::env::var_os("UPDATE_SUPPORT_POSITIONS").is_some() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/track/support_positions_snapshot.txt"
            ),
            &snapshot,
        )
        .unwrap();
        return;
    }
    assert_eq!(snapshot, include_str!("../support_positions_snapshot.txt"));
}

#[test]
fn completion_finds_known_connections_across_models_and_rotations() {
    use PieceId::*;
    let world = World::new(
        256,
        256,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let cases: &[(&str, &[PieceId], &[PieceId])] = &[
        (
            "turns",
            &[Station, TurnSmallStandalone, TurnSmallStandalone, Straight],
            &[TurnSmallStandalone, TurnSmallStandalone],
        ),
        (
            "one turn",
            &[
                Station,
                TurnSmallStandalone,
                TurnSmallStandalone,
                Straight,
                TurnSmallStandalone,
            ],
            &[TurnSmallStandalone],
        ),
        (
            "full tile offset",
            &[
                Station,
                Straight,
                Straight,
                TurnSmallStandalone,
                TurnSmallStandalone,
                FullOffsetLeft,
            ],
            &[FullOffsetRight, TurnSmallStandalone, TurnSmallStandalone],
        ),
        (
            "two tile offset",
            &[
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
            ],
            &[
                FullOffsetRight,
                FullOffsetRight,
                TurnSmallStandalone,
                TurnSmallStandalone,
            ],
        ),
        (
            "offset",
            &[
                Station,
                Straight,
                Straight,
                TurnSmallStandalone,
                TurnSmallStandalone,
                OffsetLeft,
            ],
            &[OffsetRight, TurnSmallStandalone, TurnSmallStandalone],
        ),
        (
            "slope",
            &[
                Station,
                LevelToGentleUp,
                GentleUpToLevel,
                TurnSmallStandalone,
                TurnSmallStandalone,
                Straight,
                LevelToGentleDown,
            ],
            &[GentleDownToLevel, TurnSmallStandalone, TurnSmallStandalone],
        ),
    ];
    for (model_index, model) in [
        hybrid_model(),
        crate::track::multi_launch_model(),
        crate::track::wooden_twister_model(),
    ]
    .into_iter()
    .enumerate()
    {
        for (heading_index, heading) in [
            Heading::East,
            Heading::NorthEast,
            Heading::North,
            Heading::NorthWest,
            Heading::West,
            Heading::SouthWest,
            Heading::South,
            Heading::SouthEast,
        ]
        .into_iter()
        .enumerate()
        {
            for (case_index, (case, prefix, witness)) in cases
                .iter()
                .enumerate()
                .filter(|(index, _)| *index == (model_index + heading_index) % cases.len())
            {
                let initial = TrackState {
                    x: 125 + case_index as i32,
                    y: 130 - case_index as i32,
                    z: 15 + case_index as i32 * 5,
                    heading,
                    ..start()
                };
                let mut construction = Construction::new(initial, model.clone());
                for &piece in *prefix {
                    construction
                        .append(piece, &world, &[])
                        .unwrap_or_else(|error| {
                            panic!(
                                "{} {heading:?} {case}: prefix {piece:?}: {error:?}",
                                model.name
                            )
                        });
                }
                let mut expected = construction.copy_for_completion();
                for &piece in *witness {
                    expected.append(piece, &world, &[]).unwrap_or_else(|error| {
                        panic!(
                            "{} {heading:?} {case}: witness {piece:?}: {error:?}",
                            model.name
                        )
                    });
                }
                expected.close().unwrap_or_else(|error| {
                    panic!(
                        "{} {heading:?} {case}: witness close: {error:?}",
                        model.name
                    )
                });
                let completion = construction
                    .find_completion(&world, &[])
                    .unwrap_or_else(|| {
                        panic!(
                            "{} {heading:?} {case}: search failed despite valid witness",
                            model.name
                        )
                    });
                construction
                    .apply_completion(&completion, &world, &[])
                    .unwrap();
                assert!(construction.is_closed());
            }
        }
    }
}
