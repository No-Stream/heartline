use theme_park_sim::physics::rated_piece::{rate_piece, unpowered as rating_physics};
use theme_park_sim::physics::{
    Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache, PreviewStatus, TestRun,
    TrackPreview, Train,
};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, SolveContext, SolveRefusal, TurnSizeHint, solve,
};
use theme_park_sim::track::{
    Bank, CoasterModel, Hand, HillFeel, HorizontalCurvature, Piece, PieceId, PieceKind, Slope,
    TrackState, TrainConfiguration, VerticalCurvature, catalog, coaster_models, hybrid_model,
    hyper_model, wooden_twister_model,
};

fn model_with_train_length(base_model: &CoasterModel, cars: u8) -> CoasterModel {
    assert!(base_model.cars_per_train.contains(&cars));
    let mut model = base_model.clone();
    model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
    model.train.cars = cars;
    model
}

fn lift_prefix(model: &CoasterModel, speed_mps: f64) -> Vec<Piece> {
    let templates = catalog();
    let station_template = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .expect("catalog includes the station");
    let straight_template = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let mut prefix = vec![
        station_template
            .at(TrackState::origin())
            .expect("station starts at the origin"),
    ];
    let initial_speed_mps = PhysicsConfig::default().station_speed;
    let drive_acceleration = PhysicsConfig::default().drive_acceleration;
    let acceleration_distance_m =
        (speed_mps.powi(2) - initial_speed_mps.powi(2)).max(0.0) / (2.0 * drive_acceleration);
    let required_lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
    let straight_count = (required_lift_length_m / straight_template.length()).ceil() as usize;

    for _ in 0..straight_count {
        let mut lift = straight_template
            .at(prefix.last().expect("prefix has a station").exit)
            .expect("straight pieces connect");
        lift.kind = PieceKind::Lift;
        prefix.push(lift);
    }
    prefix
}

fn runout_from(piece: &Piece, train: &TrainConfiguration) -> Vec<Piece> {
    let straight_template = catalog()
        .into_iter()
        .find(|candidate| candidate.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let straight_count = (train.length_metres / straight_template.length()).ceil() as usize + 2;
    let mut runout = Vec::with_capacity(straight_count);
    let mut state = piece.exit;
    for _ in 0..straight_count {
        let straight = straight_template
            .at(state)
            .expect("straight runout pieces connect");
        state = straight.exit;
        runout.push(straight);
    }
    runout
}

fn entry_preview(
    prefix: &[Piece],
    model: &CoasterModel,
    speed_mps: f64,
    physics: PhysicsConfig,
) -> (TrackPreview, PreviewCache) {
    let mut cache = PreviewCache::default();
    let preview = cache
        .preview(
            prefix,
            model,
            PhysicsConfig {
                lift_speed: speed_mps,
                ..physics
            },
        )
        .expect("prefix contains a station");
    assert_eq!(preview.status, PreviewStatus::ReachedEnd);
    let observed_entry_speed = preview.end_speed_mps.expect("preview reaches its open end");
    assert!(
        (observed_entry_speed - speed_mps).abs() <= 0.01,
        "prefix delivers {observed_entry_speed:.4} m/s, requested {speed_mps:.4} m/s"
    );
    (preview, cache)
}

fn solve_with_prefix(
    intent: ElementIntent,
    base_model: &CoasterModel,
    cars: u8,
    speed_mps: f64,
) -> Result<
    (
        CoasterModel,
        Vec<Piece>,
        theme_park_sim::track::shaped::SolvedElement,
    ),
    SolveRefusal,
> {
    let model = model_with_train_length(base_model, cars);
    let prefix = lift_prefix(&model, speed_mps);
    let physics = PhysicsConfig {
        lift_speed: speed_mps,
        ..PhysicsConfig::default()
    };
    let (_, mut preview_cache) = entry_preview(&prefix, &model, speed_mps, physics);
    let entry = prefix.last().expect("prefix has a station").exit;
    let mut context = SolveContext {
        entry,
        entry_speed_mps: speed_mps,
        model: &model,
        train: &model.train,
        prefix: &prefix,
        preview_cache: &mut preview_cache,
        physics,
    };
    let solved = solve(&intent, &mut context)?;
    Ok((model, prefix, solved))
}

fn simulate_shape(
    prefix: &[Piece],
    model: &CoasterModel,
    solved_piece: &Piece,
    entry_speed_mps: f64,
    physics: PhysicsConfig,
) -> TrackPreview {
    let (.., mut cache) = entry_preview(prefix, model, entry_speed_mps, physics);
    let candidate_index = prefix.len();
    let mut pieces = prefix.to_vec();
    pieces.push(solved_piece.clone());
    pieces.extend(runout_from(solved_piece, &model.train));
    let preview = cache
        .preview_all_cars(
            &pieces,
            model,
            PhysicsConfig {
                lift_speed: entry_speed_mps,
                ..physics
            },
        )
        .expect("candidate circuit contains a station");
    assert_eq!(
        preview.status,
        PreviewStatus::ReachedEnd,
        "candidate failed to carry the train through its runout"
    );
    assert!(
        preview
            .report
            .car_piece_force_peaks()
            .iter()
            .any(|peaks| peaks.piece_index == candidate_index)
    );
    preview
}

fn simulate_shape_at_controlled_speed_under_construction_test_physics(
    prefix: &[Piece],
    model: &CoasterModel,
    solved_piece: &Piece,
    entry_speed_mps: f64,
) -> TestRun {
    let candidate_index = prefix.len();
    let mut pieces = prefix.to_vec();
    pieces.push(solved_piece.clone());
    pieces.extend(runout_from(solved_piece, &model.train));
    let circuit = Circuit::open(pieces).expect("candidate track has a station and a runout");
    let candidate_start_m = prefix.iter().map(Piece::length).sum::<f64>();
    let candidate_end_m = candidate_start_m + solved_piece.length();
    let physics = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };
    let mut train = Train::new(
        &circuit,
        &model.train,
        physics,
        candidate_start_m,
        entry_speed_mps,
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
        &circuit,
        &model.train,
        model,
    );
    assert!(
        (train.speed() - entry_speed_mps).abs() <= 0.01,
        "construction-test train speed {:.3} m/s differs from requested {:.3} m/s",
        train.speed(),
        entry_speed_mps
    );
    let clear_candidate_distance = candidate_end_m + model.train.length_metres;
    for _ in 0..100_000 {
        train.step(&circuit);
        recorder.record(&train.take_readings(), model.name);
        assert!(
            train.notes().is_empty(),
            "construction-test train recovered before clearing the candidate: {:?}",
            train.notes()
        );
        if train.distance() >= clear_candidate_distance {
            let report = recorder.finish(false, false);
            assert!(
                report
                    .car_piece_force_peaks()
                    .iter()
                    .any(|peaks| peaks.piece_index == candidate_index),
                "construction-test train recorded no candidate forces"
            );
            return report;
        }
    }
    panic!("construction-test train did not clear the candidate and runout");
}

fn safe_airtime_target(model: &CoasterModel) -> f64 {
    model.comfortable_force_limits.minimum_vertical_g + 0.35
}

fn assert_grid_connector(state: TrackState) {
    let position = state.position();
    assert_eq!(position.x.rem_euclid(2.0), 0.0);
    assert_eq!(position.y.rem_euclid(2.0), 0.0);
    assert_eq!(position.z.rem_euclid(2.0), 0.0);
    assert_eq!(state.slope, Slope::Level);
    assert_eq!(state.bank, Bank::Level);
    assert_eq!(state.horizontal_curvature, HorizontalCurvature::Straight);
    assert_eq!(state.vertical_curvature, VerticalCurvature::Straight);
}

fn assert_force_tolerance(
    piece: &Piece,
    model: &CoasterModel,
    entry_speed_mps: f64,
    design_speed: bool,
    description: &str,
) {
    let rating = rate_piece(
        model,
        model.train.cars,
        piece,
        entry_speed_mps,
        rating_physics(),
    )
    .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(
        rating
            .context_results
            .iter()
            .any(|context| context.applicable),
        "{description}: no model-allowed approach can reach the requested speed"
    );
    assert!(
        rating
            .context_results
            .iter()
            .filter(|context| context.applicable)
            .all(|context| context.passed),
        "{description}: a model-allowed approach exceeded a duration-aware force limit: {:?}",
        rating.context_results
    );
    if !design_speed {
        return;
    }
    let limits = model.comfortable_force_limits;
    assert!(
        rating.minimum_filtered_force[0] >= limits.minimum_vertical_g + 0.25,
        "{description}: worst car reached {:.3} g, minimum limit {:.3} g",
        rating.minimum_filtered_force[0],
        limits.minimum_vertical_g
    );
    assert!(
        rating.maximum_filtered_force[0] <= limits.maximum_vertical_g - 0.25,
        "{description}: worst car reached {:.3} g, maximum limit {:.3} g",
        rating.maximum_filtered_force[0],
        limits.maximum_vertical_g
    );
    assert!(
        rating.maximum_filtered_force[1] <= limits.maximum_lateral_g - 0.25,
        "{description}: worst car reached {:.3} lateral g",
        rating.maximum_filtered_force[1]
    );
    assert!(
        rating.maximum_filtered_force[2] <= limits.maximum_longitudinal_g - 0.25,
        "{description}: worst car reached {:.3} longitudinal g",
        rating.maximum_filtered_force[2]
    );
}

fn assert_all_car_force_headroom(
    report: &TestRun,
    piece_index: usize,
    model: &CoasterModel,
    description: &str,
) {
    let per_car_peaks: Vec<_> = report
        .car_piece_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        per_car_peaks.len(),
        usize::from(model.train.cars),
        "{description}: missing per-car forces"
    );
    let limits = model.comfortable_force_limits;
    for car in per_car_peaks {
        let peaks = car.peaks;
        assert!(
            peaks.minimum_vertical_g >= limits.minimum_vertical_g + 0.25,
            "{description}, car {}: minimum vertical force {:.3} g lacks 0.25 g headroom",
            car.car_index,
            peaks.minimum_vertical_g
        );
        assert!(
            peaks.maximum_vertical_g <= limits.maximum_vertical_g - 0.25,
            "{description}, car {}: maximum vertical force {:.3} g lacks 0.25 g headroom",
            car.car_index,
            peaks.maximum_vertical_g
        );
        assert!(
            peaks.maximum_lateral_g <= limits.maximum_lateral_g - 0.25,
            "{description}, car {}: lateral force {:.3} g lacks 0.25 g headroom",
            car.car_index,
            peaks.maximum_lateral_g
        );
        assert!(
            peaks.maximum_longitudinal_g <= limits.maximum_longitudinal_g - 0.25,
            "{description}, car {}: longitudinal force {:.3} g lacks 0.25 g headroom",
            car.car_index,
            peaks.maximum_longitudinal_g
        );
    }
}

fn assert_model_minimum_radius(piece: &Piece, model: &CoasterModel, description: &str) {
    let minimum_radius_m = piece.maximum_curvature().recip();
    assert!(
        minimum_radius_m >= model.minimum_radius_metres,
        "{description}: minimum rail radius {minimum_radius_m:.3} m is below model minimum {:.3} m",
        model.minimum_radius_metres
    );
}

fn assert_hill_intent(
    preview: &TrackPreview,
    piece_index: usize,
    model: &CoasterModel,
    target_g: f64,
    description: &str,
) {
    let sustained: Vec<_> = preview
        .report
        .car_piece_sustained_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        sustained.len(),
        usize::from(model.train.cars),
        "{description}"
    );
    let worst_airtime = sustained
        .iter()
        .map(|peaks| peaks.peaks.minimum_vertical_g.unwrap_or(0.0))
        .fold(f64::INFINITY, f64::min);
    assert!(
        (worst_airtime - target_g).abs() <= 0.1,
        "{description}: worst-car sustained airtime {worst_airtime:.3} g differs from {target_g:.3} g"
    );
}

fn assert_same_serialized_hash(left: &Piece, right: &Piece) {
    let left_bytes = postcard::to_allocvec(left).expect("resolved piece serializes");
    let right_bytes = postcard::to_allocvec(right).expect("resolved piece serializes");
    assert_eq!(
        left_bytes, right_bytes,
        "same solve inputs changed resolved bytes"
    );
    assert_eq!(stable_hash(&left_bytes), stable_hash(&right_bytes));
    let restored: Piece = postcard::from_bytes(&left_bytes).expect("resolved piece deserializes");
    assert_eq!(
        postcard::to_allocvec(&restored).expect("restored piece serializes"),
        left_bytes,
        "save round trip changed resolved geometry"
    );
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn every_model_train_length_and_integer_entry_speed_solves_hills_and_turns() {
    for base_model in coaster_models() {
        for cars in base_model.cars_per_train.clone() {
            let model = model_with_train_length(&base_model, cars);
            let target_g = safe_airtime_target(&model);
            for speed_mps in 14..=36 {
                let speed_mps = f64::from(speed_mps);
                let hill_intent = ElementIntent::Hill {
                    feel: HillFeel::Floater,
                    strength_g: Some(target_g),
                    size: HillSizeHint::Auto,
                };
                let (solved_model, prefix, hill) =
                    solve_with_prefix(hill_intent, &base_model, cars, speed_mps).unwrap_or_else(
                        |refusal| {
                            panic!(
                                "{} {cars} cars hill at {speed_mps} m/s refused: {refusal:?}",
                                base_model.key
                            )
                        },
                    );
                let ElementSpec::Hill {
                    design_speed_mps,
                    cars: solved_cars,
                    target_g: resolved_target_g,
                    height_m,
                    span_m,
                    ..
                } = hill.spec
                else {
                    panic!("hill intent resolved to a turn")
                };
                assert_eq!(solved_cars, cars);
                assert!((design_speed_mps - speed_mps).abs() <= 0.01);
                assert!((resolved_target_g - target_g).abs() <= 0.01);
                assert!(height_m > 0.0 && span_m > 0.0);
                assert_eq!(hill.piece.entry, prefix.last().unwrap().exit);
                assert_grid_connector(hill.piece.exit);
                assert!(solved_model.allows_piece(&hill.piece));
                assert_model_minimum_radius(&hill.piece, &solved_model, "shaped hill");
                let candidate_index = prefix.len();
                let design_preview = simulate_shape(
                    &prefix,
                    &solved_model,
                    &hill.piece,
                    speed_mps,
                    PhysicsConfig::default(),
                );
                assert_all_car_force_headroom(
                    &design_preview.report,
                    candidate_index,
                    &solved_model,
                    "hill under default physics",
                );
                let construction_test_report =
                    simulate_shape_at_controlled_speed_under_construction_test_physics(
                        &prefix,
                        &solved_model,
                        &hill.piece,
                        speed_mps,
                    );
                assert_all_car_force_headroom(
                    &construction_test_report,
                    candidate_index,
                    &solved_model,
                    "hill under construction-test physics",
                );
                assert_hill_intent(
                    &design_preview,
                    candidate_index,
                    &solved_model,
                    target_g,
                    &format!("{} {cars} cars hill at {speed_mps} m/s", base_model.key),
                );
                assert_force_tolerance(
                    &hill.piece,
                    &solved_model,
                    speed_mps,
                    true,
                    "hill at design speed",
                );
                for offset_mps in [-0.5, 0.5] {
                    assert_force_tolerance(
                        &hill.piece,
                        &solved_model,
                        speed_mps + offset_mps,
                        false,
                        "hill within speed tolerance",
                    );
                }

                let turn_intent = ElementIntent::Turn {
                    hand: Hand::Left,
                    size: TurnSizeHint::Auto,
                };
                let (turn_model, turn_prefix, turn) =
                    solve_with_prefix(turn_intent, &base_model, cars, speed_mps).unwrap_or_else(
                        |refusal| {
                            panic!(
                                "{} {cars} cars turn at {speed_mps} m/s refused: {refusal:?}",
                                base_model.key
                            )
                        },
                    );
                let ElementSpec::Turn {
                    design_speed_mps,
                    cars: solved_cars,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    ..
                } = turn.spec
                else {
                    panic!("turn intent resolved to a hill")
                };
                assert_eq!(solved_cars, cars);
                assert!((design_speed_mps - speed_mps).abs() <= 0.01);
                assert!(lateral_g.abs() <= 0.1);
                assert!(radius_m > 0.0);
                assert!(bank_degrees <= 75.0);
                assert_eq!(turn.piece.entry, turn_prefix.last().unwrap().exit);
                assert_grid_connector(turn.piece.exit);
                assert!(turn_model.allows_piece(&turn.piece));
                assert_model_minimum_radius(&turn.piece, &turn_model, "shaped turn");
                let turn_candidate_index = turn_prefix.len();
                let turn_design_preview = simulate_shape(
                    &turn_prefix,
                    &turn_model,
                    &turn.piece,
                    speed_mps,
                    PhysicsConfig::default(),
                );
                assert_all_car_force_headroom(
                    &turn_design_preview.report,
                    turn_candidate_index,
                    &turn_model,
                    "turn under default physics",
                );
                let construction_test_report =
                    simulate_shape_at_controlled_speed_under_construction_test_physics(
                        &turn_prefix,
                        &turn_model,
                        &turn.piece,
                        speed_mps,
                    );
                assert_all_car_force_headroom(
                    &construction_test_report,
                    turn_candidate_index,
                    &turn_model,
                    "turn under construction-test physics",
                );
                let observed_lateral = turn_design_preview
                    .report
                    .car_piece_force_peaks()
                    .into_iter()
                    .filter(|peaks| peaks.piece_index == turn_candidate_index)
                    .map(|peaks| peaks.peaks.maximum_lateral_g)
                    .fold(0.0, f64::max);
                assert!(
                    observed_lateral <= 0.1,
                    "turn lateral {observed_lateral:.3} g"
                );
                assert_force_tolerance(
                    &turn.piece,
                    &turn_model,
                    speed_mps,
                    true,
                    "turn at design speed",
                );
                for offset_mps in [-0.5, 0.5] {
                    assert_force_tolerance(
                        &turn.piece,
                        &turn_model,
                        speed_mps + offset_mps,
                        false,
                        "turn within speed tolerance",
                    );
                }
            }
        }
    }
}

#[test]
fn repeated_solve_has_the_same_hash_and_save_round_trip() {
    let model = hyper_model();
    let intent = ElementIntent::Hill {
        feel: HillFeel::Floater,
        strength_g: Some(safe_airtime_target(&model)),
        size: HillSizeHint::Auto,
    };
    let first = solve_with_prefix(intent.clone(), &model, model.train.cars, 34.0)
        .expect("hyper floater hill is feasible")
        .2;
    let second = solve_with_prefix(intent, &model, model.train.cars, 34.0)
        .expect("same hyper floater hill remains feasible")
        .2;
    assert_same_serialized_hash(&first.piece, &second.piece);
    let first_spec = postcard::to_allocvec(&first.spec).expect("resolved spec serializes");
    let restored_spec: ElementSpec =
        postcard::from_bytes(&first_spec).expect("resolved spec deserializes");
    assert_eq!(
        postcard::to_allocvec(&restored_spec).expect("restored spec serializes"),
        first_spec,
        "save round trip changed resolved spec"
    );
}

#[test]
fn auto_sizing_grows_hill_height_and_turn_radius_with_entry_speed() {
    let model = hyper_model();
    let target_g = safe_airtime_target(&model);
    let mut hill_heights = Vec::new();
    let mut turn_radii = Vec::new();
    for speed_mps in [18.0, 28.0] {
        let hill = solve_with_prefix(
            ElementIntent::Hill {
                feel: HillFeel::Floater,
                strength_g: Some(target_g),
                size: HillSizeHint::Auto,
            },
            &model,
            model.train.cars,
            speed_mps,
        )
        .expect("hyper floater hill is feasible");
        let ElementSpec::Hill { height_m, .. } = hill.2.spec else {
            panic!("hill intent resolved to a turn")
        };
        hill_heights.push(height_m);

        let turn = solve_with_prefix(
            ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Auto,
            },
            &model,
            model.train.cars,
            speed_mps,
        )
        .expect("hyper fully banked turn is feasible");
        let ElementSpec::Turn { radius_m, .. } = turn.2.spec else {
            panic!("turn intent resolved to a hill")
        };
        turn_radii.push(radius_m);
    }
    assert!(
        hill_heights[1] > hill_heights[0],
        "hill heights: {hill_heights:?}"
    );
    assert!(turn_radii[1] > turn_radii[0], "turn radii: {turn_radii:?}");
}

#[test]
fn infeasible_force_and_stall_targets_have_typed_refusals() {
    let family = coaster_models()
        .into_iter()
        .find(|model| model.key == "family_coaster")
        .expect("family coaster model exists");
    let infeasible = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: None,
            size: HillSizeHint::Auto,
        },
        &family,
        family.train.cars,
        28.0,
    );
    assert!(matches!(
        infeasible,
        Err(SolveRefusal::InfeasibleTarget { .. })
    ));

    let hyper = hyper_model();
    let stall = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: Some(safe_airtime_target(&hyper)),
            size: HillSizeHint::Height(80.0),
        },
        &hyper,
        hyper.train.cars,
        14.0,
    );
    assert!(matches!(stall, Err(SolveRefusal::Stall { .. })));
}

#[test]
fn feel_defaults_match_the_plan_and_refuse_when_the_margin_is_infeasible() {
    for model in coaster_models() {
        let floater = solve_with_prefix(
            ElementIntent::Hill {
                feel: HillFeel::Floater,
                strength_g: None,
                size: HillSizeHint::Auto,
            },
            &model,
            model.train.cars,
            28.0,
        );
        match floater {
            Ok((_, _, solved)) => {
                let ElementSpec::Hill { target_g, .. } = solved.spec else {
                    panic!("floater intent resolved to a turn")
                };
                assert_eq!(target_g, -0.5, "{} floater default", model.key);
            }
            Err(SolveRefusal::InfeasibleTarget { .. }) => {
                assert_eq!(model.key, "family_coaster");
            }
            Err(refusal) => panic!("{} floater default refused: {refusal:?}", model.key),
        }
    }

    let hyper = hyper_model();
    for (feel, expected_g) in [(HillFeel::Floater, -0.5), (HillFeel::StrongFloater, -0.9)] {
        let solved = solve_with_prefix(
            ElementIntent::Hill {
                feel,
                strength_g: None,
                size: HillSizeHint::Auto,
            },
            &hyper,
            hyper.train.cars,
            28.0,
        )
        .expect("hyper hill feel default is feasible");
        let ElementSpec::Hill { target_g, .. } = solved.2.spec else {
            panic!("hill intent resolved to a turn")
        };
        assert_eq!(target_g, expected_g);
    }

    let steel = hybrid_model();
    let steel_ejector = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Ejector,
            strength_g: None,
            size: HillSizeHint::Auto,
        },
        &steel,
        steel.train.cars,
        28.0,
    )
    .expect("steel ejector default has a feasible margin on the hybrid model");
    let ElementSpec::Hill { target_g, .. } = steel_ejector.2.spec else {
        panic!("hill intent resolved to a turn")
    };
    assert_eq!(target_g, -1.2);

    let refusal = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Ejector,
            strength_g: None,
            size: HillSizeHint::Auto,
        },
        &hyper,
        hyper.train.cars,
        28.0,
    )
    .expect_err("hyper ejector default cannot meet the target and retain 0.25 g headroom");
    assert!(matches!(
        refusal,
        SolveRefusal::InfeasibleTarget { target_g, .. } if (target_g + 1.2).abs() < 1e-9
    ));

    let wood = wooden_twister_model();
    let wood_ejector = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Ejector,
            strength_g: None,
            size: HillSizeHint::Auto,
        },
        &wood,
        wood.train.cars,
        28.0,
    )
    .expect("wood ejector default can be eased within 0.1 g to preserve the margin");
    let ElementSpec::Hill {
        target_g,
        delivered_g,
        ..
    } = wood_ejector.2.spec
    else {
        panic!("hill intent resolved to a turn")
    };
    assert_eq!(target_g, -0.8);
    assert!((delivered_g - target_g).abs() <= 0.1);
    assert_force_tolerance(
        &wood_ejector.2.piece,
        &wood_ejector.0,
        28.0,
        true,
        "wood ejector default at design speed",
    );
}
