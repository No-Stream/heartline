use theme_park_sim::physics::rated_piece::{rate_piece, unpowered as rating_physics};
use theme_park_sim::physics::{
    Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache, PreviewStatus, TestRun,
    TrackPreview, Train,
};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, FeelCapNote, HillSizeHint, SolveContext, SolveRefusal,
    TurnSizeHint, solve,
};
use theme_park_sim::track::shaped_generator::{
    CellRefusal, GENERATED_SHAPED_ELEMENT_GRID, GeneratedSpec, GeneratedTableOutcome, GridElement,
};
use theme_park_sim::track::{
    Bank, CoasterModel, Hand, Heading, HillFeel, HorizontalCurvature, Piece, PieceId, PieceKind,
    Slope, TrackState, TrainConfiguration, VerticalCurvature, catalog, coaster_models, hyper_model,
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
        .iter()
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
    model.comfortable_force_limits.minimum_vertical_g + 0.4
}

fn requested_hill_strengths(model: &CoasterModel) -> Vec<f64> {
    let first_tenth =
        ((model.comfortable_force_limits.minimum_vertical_g + 0.15) * 10.0).ceil() as i16;
    let mut strengths = (first_tenth..=0)
        .rev()
        .map(|tenths| f64::from(tenths) / 10.0)
        .collect::<Vec<_>>();
    let rules = model.shaped_element_rules();
    for feel in [
        HillFeel::Floater,
        HillFeel::StrongFloater,
        HillFeel::Ejector,
    ] {
        let default_target = match feel {
            HillFeel::Floater => rules.default_floater_g,
            HillFeel::StrongFloater => rules.default_strong_floater_g,
            HillFeel::Ejector if model.track_material == theme_park_sim::track::Material::Wood => {
                rules.wooden_ejector_g
            }
            HillFeel::Ejector => rules.steel_ejector_g,
            _ => unreachable!(),
        };
        if !strengths.contains(&default_target) {
            strengths.push(default_target);
        }
    }
    strengths.sort_by(f64::total_cmp);
    strengths.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    strengths
}

fn generated_hill_outcome(
    model: &CoasterModel,
    cars: u8,
    feel: HillFeel,
    strength_g: f64,
    speed_mps: f64,
) -> &'static GeneratedTableOutcome {
    GENERATED_SHAPED_ELEMENT_GRID
        .binary_search_by(|row| {
            row.model_key
                .cmp(model.key)
                .then_with(|| row.cars.cmp(&cars))
                .then_with(|| match row.element {
                    GridElement::Hill {
                        feel: row_feel,
                        strength_g: row_strength,
                    } => {
                        hill_feel_order(row_feel)
                            .cmp(&hill_feel_order(feel))
                            .then_with(|| row_strength.total_cmp(&strength_g))
                    }
                    GridElement::FullyBankedTurn => std::cmp::Ordering::Greater,
                })
                .then_with(|| row.entry_speed_mps.total_cmp(&speed_mps))
        })
        .ok()
        .map(|index| &GENERATED_SHAPED_ELEMENT_GRID[index].outcome)
        .unwrap_or_else(|| {
            panic!(
                "generated hill table is missing {} {cars} cars {feel:?} at {strength_g:.1} g and {speed_mps:.2} m/s",
                model.key
            )
        })
}

fn generated_turn_outcome(
    model: &CoasterModel,
    cars: u8,
    speed_mps: f64,
) -> &'static GeneratedTableOutcome {
    GENERATED_SHAPED_ELEMENT_GRID
        .binary_search_by(|row| {
            row.model_key
                .cmp(model.key)
                .then_with(|| row.cars.cmp(&cars))
                .then_with(|| match row.element {
                    GridElement::Hill { .. } => std::cmp::Ordering::Less,
                    GridElement::FullyBankedTurn => row.entry_speed_mps.total_cmp(&speed_mps),
                })
        })
        .ok()
        .map(|index| &GENERATED_SHAPED_ELEMENT_GRID[index].outcome)
        .unwrap_or_else(|| {
            panic!(
                "generated turn table is missing {} {cars} cars at {speed_mps:.2} m/s",
                model.key
            )
        })
}

fn generated_row_is_solved(outcome: &GeneratedTableOutcome) -> bool {
    matches!(outcome, GeneratedTableOutcome::Solved(_))
}

fn measured_table_row_is_solved(outcome: &GeneratedTableOutcome) -> bool {
    match outcome {
        GeneratedTableOutcome::Solved(_) => true,
        GeneratedTableOutcome::Infeasible {
            best_measurements, ..
        } => {
            assert_eq!(
                best_measurements.len(),
                6,
                "infeasible table rows retain all six measured physics/speed cases"
            );
            false
        }
        GeneratedTableOutcome::Refused(CellRefusal::InvalidGeometry { .. }) => false,
        GeneratedTableOutcome::Refused(refusal) => panic!(
            "in-range hill/turn grid cell was refused without a measured best case: {refusal:?}"
        ),
    }
}

fn hill_feel_order(feel: HillFeel) -> u8 {
    match feel {
        HillFeel::Floater => 0,
        HillFeel::StrongFloater => 1,
        HillFeel::Ejector => 2,
        _ => unreachable!("the property matrix covers supported hill feels only"),
    }
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

fn assert_all_car_force_limits(
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
            peaks.minimum_vertical_g >= limits.minimum_vertical_g,
            "{description}, car {}: minimum vertical force {:.3} g exceeds the limit {:.3} g",
            car.car_index,
            peaks.minimum_vertical_g,
            limits.minimum_vertical_g
        );
        assert!(
            peaks.maximum_vertical_g <= limits.maximum_vertical_g,
            "{description}, car {}: maximum vertical force {:.3} g exceeds the limit {:.3} g",
            car.car_index,
            peaks.maximum_vertical_g,
            limits.maximum_vertical_g
        );
        assert!(
            peaks.maximum_lateral_g <= limits.maximum_lateral_g,
            "{description}, car {}: lateral force {:.3} g exceeds the limit {:.3} g",
            car.car_index,
            peaks.maximum_lateral_g,
            limits.maximum_lateral_g
        );
        assert!(
            peaks.maximum_longitudinal_g <= limits.maximum_longitudinal_g,
            "{description}, car {}: longitudinal force {:.3} g exceeds the limit {:.3} g",
            car.car_index,
            peaks.maximum_longitudinal_g,
            limits.maximum_longitudinal_g
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
    assert_hill_delivery(&preview.report, piece_index, model, target_g, description);
}

fn assert_hill_delivery(
    report: &TestRun,
    piece_index: usize,
    model: &CoasterModel,
    target_g: f64,
    description: &str,
) {
    let sustained: Vec<_> = report
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
                let hill_cell = generated_hill_outcome(
                    &base_model,
                    cars,
                    HillFeel::Floater,
                    target_g,
                    speed_mps,
                );
                if !generated_row_is_solved(hill_cell) {
                    let refusal = solve_with_prefix(hill_intent, &base_model, cars, speed_mps);
                    assert!(table_failure_has_typed_refusal(&refusal), "{refusal:?}");
                } else {
                    let (solved_model, prefix, hill) =
                        solve_with_prefix(hill_intent, &base_model, cars, speed_mps)
                            .unwrap_or_else(|refusal| {
                                panic!(
                                    "{} {cars} cars hill at {speed_mps} m/s refused: {refusal:?}",
                                    base_model.key
                                )
                            });
                    let hill_cap_note = hill.cap_note;
                    let ElementSpec::Hill {
                        design_speed_mps,
                        cars: solved_cars,
                        target_g: resolved_target_g,
                        delivered_g,
                        height_m,
                        span_m,
                        ..
                    } = hill.spec
                    else {
                        panic!("hill intent resolved to a turn")
                    };
                    assert_eq!(solved_cars, cars);
                    assert!((design_speed_mps - speed_mps).abs() <= 0.01);
                    assert!(
                        (delivered_g - resolved_target_g).abs() <= 0.1,
                        "delivered target {delivered_g:.3} g differs from capped target {resolved_target_g:.3} g"
                    );
                    if (resolved_target_g - target_g).abs() > 1e-9 {
                        let note: FeelCapNote = hill_cap_note
                            .expect("a shallower capped target exposes a typed cap note");
                        assert_eq!(note.feel, HillFeel::Floater);
                        assert!((note.requested_target_g - target_g).abs() < 1e-9);
                        assert!((note.capped_target_g - resolved_target_g).abs() < 1e-9);
                        assert!((note.delivered_g - delivered_g).abs() < 1e-9);
                    } else {
                        assert!(hill_cap_note.is_none());
                    }
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
                        resolved_target_g,
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
                }

                let turn_intent = ElementIntent::Turn {
                    hand: Hand::Left,
                    size: TurnSizeHint::Auto,
                };
                let turn_cell = generated_turn_outcome(&base_model, cars, speed_mps);
                if !generated_row_is_solved(turn_cell) {
                    let refusal = solve_with_prefix(turn_intent, &base_model, cars, speed_mps);
                    assert!(table_failure_has_typed_refusal(&refusal), "{refusal:?}");
                    continue;
                }
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

fn generated_hill_feasible_at_speed(
    model: &CoasterModel,
    cars: u8,
    feel: HillFeel,
    target_g: f64,
    speed_mps: f64,
) -> bool {
    let half_speed = speed_mps * 2.0;
    let nearest_half_speed = half_speed.round();
    if (half_speed - nearest_half_speed).abs() < 1e-9 {
        return measured_table_row_is_solved(generated_hill_outcome(
            model,
            cars,
            feel,
            target_g,
            nearest_half_speed / 2.0,
        ));
    }
    let lower_half_speed = half_speed.floor();
    let lower_speed_mps = lower_half_speed / 2.0;
    let upper_speed_mps = (lower_half_speed + 1.0) / 2.0;
    measured_table_row_is_solved(generated_hill_outcome(
        model,
        cars,
        feel,
        target_g,
        lower_speed_mps,
    )) && measured_table_row_is_solved(generated_hill_outcome(
        model,
        cars,
        feel,
        target_g,
        upper_speed_mps,
    ))
}

fn generated_turn_feasible_at_speed(model: &CoasterModel, cars: u8, speed_mps: f64) -> bool {
    let half_speed = speed_mps * 2.0;
    let nearest_half_speed = half_speed.round();
    if (half_speed - nearest_half_speed).abs() < 1e-9 {
        return measured_table_row_is_solved(generated_turn_outcome(
            model,
            cars,
            nearest_half_speed / 2.0,
        ));
    }
    let lower_half_speed = half_speed.floor();
    measured_table_row_is_solved(generated_turn_outcome(model, cars, lower_half_speed / 2.0))
        && measured_table_row_is_solved(generated_turn_outcome(
            model,
            cars,
            (lower_half_speed + 1.0) / 2.0,
        ))
}

fn assert_hill_scenarios(
    prefix: &[Piece],
    model: &CoasterModel,
    piece: &Piece,
    design_speed_mps: f64,
    requested_target_g: f64,
) {
    let piece_index = prefix.len();
    for offset_mps in [-0.5, 0.0, 0.5] {
        let entry_speed_mps = design_speed_mps + offset_mps;
        let runtime_preview = simulate_shape(
            prefix,
            model,
            piece,
            entry_speed_mps,
            PhysicsConfig::default(),
        );
        let construction_report =
            simulate_shape_at_controlled_speed_under_construction_test_physics(
                prefix,
                model,
                piece,
                entry_speed_mps,
            );
        assert_all_car_force_headroom(
            &runtime_preview.report,
            piece_index,
            model,
            "hill under runtime-preview physics across the speed margin",
        );
        assert_all_car_force_headroom(
            &construction_report,
            piece_index,
            model,
            "hill under construction-test physics across the speed margin",
        );
        if offset_mps == 0.0 {
            assert_hill_intent(
                &runtime_preview,
                piece_index,
                model,
                requested_target_g,
                "hill under runtime-preview physics at design speed",
            );
            assert_hill_delivery(
                &construction_report,
                piece_index,
                model,
                requested_target_g,
                "hill under construction-test physics at design speed",
            );
        }
    }
}

fn assert_turn_scenarios(
    prefix: &[Piece],
    model: &CoasterModel,
    piece: &Piece,
    design_speed_mps: f64,
) {
    let piece_index = prefix.len();
    for offset_mps in [-0.5, 0.0, 0.5] {
        let entry_speed_mps = design_speed_mps + offset_mps;
        let runtime_preview = simulate_shape(
            prefix,
            model,
            piece,
            entry_speed_mps,
            PhysicsConfig::default(),
        );
        let construction_report =
            simulate_shape_at_controlled_speed_under_construction_test_physics(
                prefix,
                model,
                piece,
                entry_speed_mps,
            );
        if offset_mps == 0.0 {
            assert_all_car_force_headroom(
                &runtime_preview.report,
                piece_index,
                model,
                "turn under runtime-preview physics at design speed",
            );
            assert_all_car_force_headroom(
                &construction_report,
                piece_index,
                model,
                "turn under construction-test physics at design speed",
            );
            for (description, report) in [
                ("runtime preview", &runtime_preview.report),
                ("construction test", &construction_report),
            ] {
                let observed_lateral = report
                    .car_piece_force_peaks()
                    .into_iter()
                    .filter(|peaks| peaks.piece_index == piece_index)
                    .map(|peaks| peaks.peaks.maximum_lateral_g)
                    .fold(0.0, f64::max);
                assert!(
                    observed_lateral <= 0.1,
                    "{} {}-car turn at {design_speed_mps:.2} m/s with radius {:.2} m has lateral {observed_lateral:.3} g under {description} physics",
                    model.key,
                    model.train.cars,
                    piece.maximum_curvature().recip()
                );
            }
        } else {
            assert_all_car_force_limits(
                &runtime_preview.report,
                piece_index,
                model,
                "turn under runtime-preview physics around design speed",
            );
            assert_all_car_force_limits(
                &construction_report,
                piece_index,
                model,
                "turn under construction-test physics around design speed",
            );
        }
    }
}

#[test]
fn every_feasible_hill_grid_point_and_speed_midpoint_meets_properties() {
    for base_model in coaster_models() {
        for cars in base_model.cars_per_train.clone() {
            let model = model_with_train_length(&base_model, cars);
            for feel in [
                HillFeel::Floater,
                HillFeel::StrongFloater,
                HillFeel::Ejector,
            ] {
                for requested_target_g in requested_hill_strengths(&model) {
                    for half_speed in 28..=72 {
                        let speed_mps = f64::from(half_speed) / 2.0;
                        check_hill_grid_point(
                            &base_model,
                            cars,
                            feel,
                            requested_target_g,
                            speed_mps,
                        );
                        if half_speed < 72 {
                            check_hill_grid_point(
                                &base_model,
                                cars,
                                feel,
                                requested_target_g,
                                speed_mps + 0.25,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn check_hill_grid_point(
    base_model: &CoasterModel,
    cars: u8,
    feel: HillFeel,
    requested_target_g: f64,
    speed_mps: f64,
) {
    let expected_feasible =
        generated_hill_feasible_at_speed(base_model, cars, feel, requested_target_g, speed_mps);
    let intent = ElementIntent::Hill {
        feel,
        strength_g: Some(requested_target_g),
        size: HillSizeHint::Auto,
    };
    let result = solve_with_prefix(intent, base_model, cars, speed_mps);
    if !expected_feasible {
        assert!(
            table_failure_has_typed_refusal(&result),
            "{} {cars} cars {feel:?} at {speed_mps:.2} m/s should return a typed refusal for infeasible table rows, got {result:?}",
            base_model.key
        );
        return;
    }
    let (model, prefix, solved) = result.unwrap_or_else(|refusal| {
        panic!(
            "{} {cars} cars {feel:?} at {speed_mps:.2} m/s failed despite feasible table brackets: {refusal:?}",
            base_model.key
        )
    });
    assert_eq!(solved.calibration_runs, 1);
    let ElementSpec::Hill {
        target_g,
        delivered_g,
        height_m,
        span_m,
        ..
    } = &solved.spec
    else {
        panic!("hill intent resolved to a turn")
    };
    assert_eq!(solved.spec.cars(), cars);
    assert!((solved.spec.design_speed_mps() - speed_mps).abs() < 1e-6);
    assert!(height_m.is_finite() && *height_m > 0.0);
    assert!(span_m.is_finite() && *span_m > 0.0);
    if (speed_mps * 2.0 - (speed_mps * 2.0).round()).abs() < 1e-9 {
        let GeneratedTableOutcome::Solved(GeneratedSpec::Hill {
            height_m: table_height_m,
            span_m: table_span_m,
            ..
        }) = generated_hill_outcome(base_model, cars, feel, requested_target_g, speed_mps)
        else {
            panic!("feasible exact hill row has no solved generated specification")
        };
        assert!(
            (*height_m - table_height_m).abs() <= 0.01,
            "runtime hill height {height_m:.4} m differs from generated height {table_height_m:.4} m"
        );
        assert!(
            (*span_m - table_span_m).abs() <= 0.01,
            "runtime hill span {span_m:.4} m differs from generated span {table_span_m:.4} m"
        );
    }
    if (*target_g - requested_target_g).abs() > 1e-9 {
        let note = solved
            .cap_note
            .expect("a capped target exposes a typed feel-cap note");
        assert_eq!(note.feel, feel);
        assert!((note.requested_target_g - requested_target_g).abs() < 1e-9);
        assert!((note.capped_target_g - *target_g).abs() < 1e-9);
        assert!((note.delivered_g - *delivered_g).abs() < 1e-9);
    } else {
        assert!(solved.cap_note.is_none());
    }
    assert!((*delivered_g - *target_g).abs() <= 0.1);
    assert_grid_connector(solved.piece.exit);
    assert!(model.allows_piece(&solved.piece));
    assert_model_minimum_radius(&solved.piece, &model, "shaped hill");
    assert_hill_scenarios(&prefix, &model, &solved.piece, speed_mps, *target_g);
}

#[test]
fn every_feasible_turn_grid_point_and_speed_midpoint_meets_properties() {
    for base_model in coaster_models() {
        for cars in base_model.cars_per_train.clone() {
            for half_speed in 28..=72 {
                let speed_mps = f64::from(half_speed) / 2.0;
                check_turn_grid_point(&base_model, cars, speed_mps);
                if half_speed < 72 {
                    check_turn_grid_point(&base_model, cars, speed_mps + 0.25);
                }
            }
        }
    }
}

#[test]
fn every_feasible_strength_midpoint_interpolates_between_measured_rows() {
    let entry_speed_mps = 28.25;
    for base_model in coaster_models() {
        for cars in base_model.cars_per_train.clone() {
            let model = model_with_train_length(&base_model, cars);
            let strengths = requested_hill_strengths(&model);
            for feel in [
                HillFeel::Floater,
                HillFeel::StrongFloater,
                HillFeel::Ejector,
            ] {
                for strength_pair in strengths.windows(2) {
                    let [lower_strength_g, upper_strength_g] = strength_pair else {
                        unreachable!("strength window always contains a pair")
                    };
                    let requested_strength_g = (*lower_strength_g + *upper_strength_g) / 2.0;
                    let expected_feasible = generated_hill_feasible_at_speed(
                        &base_model,
                        cars,
                        feel,
                        *lower_strength_g,
                        entry_speed_mps,
                    ) && generated_hill_feasible_at_speed(
                        &base_model,
                        cars,
                        feel,
                        *upper_strength_g,
                        entry_speed_mps,
                    );
                    let result = solve_with_prefix(
                        ElementIntent::Hill {
                            feel,
                            strength_g: Some(requested_strength_g),
                            size: HillSizeHint::Auto,
                        },
                        &base_model,
                        cars,
                        entry_speed_mps,
                    );
                    if !expected_feasible {
                        assert!(
                            table_failure_has_typed_refusal(&result),
                            "{} {cars} cars {feel:?} at {requested_strength_g:.3} g and {entry_speed_mps:.2} m/s should refuse because a measured neighbor is infeasible, got {result:?}",
                            base_model.key
                        );
                        continue;
                    }

                    let (solved_model, prefix, solved) = result.unwrap_or_else(|refusal| {
                        panic!(
                            "{} {cars} cars {feel:?} midpoint {requested_strength_g:.3} g at {entry_speed_mps:.2} m/s failed despite feasible measured neighbors: {refusal:?}",
                            base_model.key
                        )
                    });
                    assert_eq!(solved.calibration_runs, 1);
                    let ElementSpec::Hill {
                        target_g,
                        delivered_g,
                        ..
                    } = &solved.spec
                    else {
                        panic!("hill intent resolved to a turn")
                    };
                    assert_eq!(solved.spec.cars(), cars);
                    assert!((solved.spec.design_speed_mps() - entry_speed_mps).abs() < 1e-6);
                    assert!((delivered_g - target_g).abs() <= 0.1);
                    match solved.cap_note {
                        Some(note) => {
                            assert_eq!(note.feel, feel);
                            assert!((note.requested_target_g - requested_strength_g).abs() < 1e-12);
                            assert!((note.capped_target_g - target_g).abs() < 1e-12);
                            assert!((note.delivered_g - delivered_g).abs() < 1e-12);
                        }
                        None => assert!((target_g - requested_strength_g).abs() < 1e-12),
                    }
                    assert_grid_connector(solved.piece.exit);
                    assert!(solved_model.allows_piece(&solved.piece));
                    assert_model_minimum_radius(&solved.piece, &solved_model, "interpolated hill");
                    assert_hill_scenarios(
                        &prefix,
                        &solved_model,
                        &solved.piece,
                        entry_speed_mps,
                        *target_g,
                    );
                }
            }
        }
    }
}

fn check_turn_grid_point(base_model: &CoasterModel, cars: u8, speed_mps: f64) {
    let expected_feasible = generated_turn_feasible_at_speed(base_model, cars, speed_mps);
    let intent = ElementIntent::Turn {
        hand: Hand::Left,
        size: TurnSizeHint::Auto,
    };
    let result = solve_with_prefix(intent, base_model, cars, speed_mps);
    if !expected_feasible {
        assert!(
            table_failure_has_typed_refusal(&result),
            "{} {cars} cars turn at {speed_mps:.2} m/s should return a typed refusal for infeasible table rows, got {result:?}",
            base_model.key
        );
        return;
    }
    let (model, prefix, solved) = result.unwrap_or_else(|refusal| {
        panic!(
            "{} {cars} cars turn at {speed_mps:.2} m/s failed despite feasible table brackets: {refusal:?}",
            base_model.key
        )
    });
    assert_eq!(solved.calibration_runs, 1);
    let ElementSpec::Turn {
        lateral_g,
        radius_m,
        bank_degrees,
        ..
    } = &solved.spec
    else {
        panic!("turn intent resolved to a hill")
    };
    assert_eq!(solved.spec.cars(), cars);
    assert!((solved.spec.design_speed_mps() - speed_mps).abs() < 1e-6);
    assert!(lateral_g.abs() <= 0.1);
    assert!(*radius_m > 0.0);
    assert!(*bank_degrees <= 75.0);
    assert_grid_connector(solved.piece.exit);
    assert!(model.allows_piece(&solved.piece));
    assert_model_minimum_radius(&solved.piece, &model, "shaped turn");
    let table_radius_m = if (speed_mps * 2.0 - (speed_mps * 2.0).round()).abs() < 1e-9 {
        match generated_turn_outcome(base_model, cars, speed_mps) {
            GeneratedTableOutcome::Solved(GeneratedSpec::Turn { radius_m, .. }) => *radius_m,
            _ => panic!("feasible exact turn row has no solved generated specification"),
        }
    } else {
        let lower_half_speed = (speed_mps * 2.0).floor();
        let lower_radius_m = match generated_turn_outcome(base_model, cars, lower_half_speed / 2.0)
        {
            GeneratedTableOutcome::Solved(GeneratedSpec::Turn { radius_m, .. }) => *radius_m,
            _ => panic!("feasible lower turn bracket has no solved specification"),
        };
        let upper_radius_m =
            match generated_turn_outcome(base_model, cars, (lower_half_speed + 1.0) / 2.0) {
                GeneratedTableOutcome::Solved(GeneratedSpec::Turn { radius_m, .. }) => *radius_m,
                _ => panic!("feasible upper turn bracket has no solved specification"),
            };
        let fraction = speed_mps * 2.0 - lower_half_speed;
        lower_radius_m + (upper_radius_m - lower_radius_m) * fraction
    };
    let radius_step_m = match prefix.last().unwrap().exit.heading {
        Heading::East | Heading::North | Heading::West | Heading::South => 2.0,
        Heading::NorthEast | Heading::NorthWest | Heading::SouthWest | Heading::SouthEast => {
            2.0 * std::f64::consts::SQRT_2
        }
    };
    let snapped_table_radius_m = (table_radius_m / radius_step_m).ceil() * radius_step_m;
    assert!(
        (*radius_m - snapped_table_radius_m).abs() <= 1e-6,
        "runtime turn radius {radius_m:.4} m differs from snapped generated radius {snapped_table_radius_m:.4} m"
    );
    assert_turn_scenarios(&prefix, &model, &solved.piece, speed_mps);
}

fn table_failure_has_typed_refusal(
    result: &Result<
        (
            CoasterModel,
            Vec<Piece>,
            theme_park_sim::track::shaped::SolvedElement,
        ),
        SolveRefusal,
    >,
) -> bool {
    matches!(
        result,
        Err(SolveRefusal::InfeasibleTarget { .. }
            | SolveRefusal::UnmeasuredTarget { .. }
            | SolveRefusal::InvalidGeometry
            | SolveRefusal::Stall { .. })
    )
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
    let feasible_speeds: Vec<_> = (28..=72)
        .map(|half_speed| f64::from(half_speed) / 2.0)
        .filter(|speed_mps| {
            generated_hill_feasible_at_speed(
                &model,
                model.train.cars,
                HillFeel::Floater,
                target_g,
                *speed_mps,
            ) && generated_turn_feasible_at_speed(&model, model.train.cars, *speed_mps)
        })
        .collect();
    assert!(
        feasible_speeds.len() >= 2,
        "hyper model has fewer than two speeds where both auto-sized intents are feasible: {feasible_speeds:?}"
    );
    let speed_points = [feasible_speeds[0], *feasible_speeds.last().unwrap()];
    let mut hill_heights = Vec::new();
    let mut turn_radii = Vec::new();
    for speed_mps in speed_points {
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
        .unwrap_or_else(|refusal| {
            panic!("hyper fully banked turn at {speed_mps:.2} m/s was table-feasible but refused: {refusal:?}")
        });
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
fn extreme_hill_strength_is_capped_and_unsupported_sizes_have_typed_refusals() {
    let family = coaster_models()
        .into_iter()
        .find(|model| model.key == "family_coaster")
        .expect("family coaster model exists");
    let capped = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: Some(-4.0),
            size: HillSizeHint::Auto,
        },
        &family,
        family.train.cars,
        28.0,
    )
    .expect("below-floor hill strengths use the lowest measured target");
    let ElementSpec::Hill {
        target_g,
        delivered_g,
        ..
    } = capped.2.spec
    else {
        panic!("hill intent resolved to a turn")
    };
    assert!(target_g > -4.0);
    assert!((delivered_g - target_g).abs() <= 0.1);
    let cap_note = capped
        .2
        .cap_note
        .expect("below-floor request exposes its measured cap");
    assert_eq!(cap_note.feel, HillFeel::Floater);
    assert_eq!(cap_note.requested_target_g, -4.0);
    assert_eq!(cap_note.capped_target_g, target_g);
    assert_eq!(cap_note.delivered_g, delivered_g);
}

#[test]
fn fixed_height_overrides_return_typed_refusals_without_table_fallback() {
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

    let unsupported_size = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: Some(safe_airtime_target(&hyper)),
            size: HillSizeHint::Height(20.0),
        },
        &hyper,
        hyper.train.cars,
        28.0,
    );
    assert!(matches!(
        unsupported_size,
        Err(SolveRefusal::UnsupportedSizeOverride)
    ));
}

#[test]
fn unsupported_speeds_refuse_and_radius_override_uses_table_backed_solve() {
    let velocity_viper = coaster_models()
        .into_iter()
        .find(|model| model.key == "velocity_viper")
        .expect("Velocity Viper model exists");
    let below_range = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: Some(-0.5),
            size: HillSizeHint::Auto,
        },
        &velocity_viper,
        velocity_viper.train.cars,
        13.5,
    );
    assert!(matches!(
        below_range,
        Err(SolveRefusal::BelowVerifiedSpeed {
            minimum_speed_mps: 14.0,
            ..
        })
    ));

    let above_range = solve_with_prefix(
        ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        },
        &velocity_viper,
        velocity_viper.train.cars,
        36.5,
    );
    assert!(matches!(
        above_range,
        Err(SolveRefusal::AboveVerifiedSpeed {
            maximum_speed_mps: 36.0,
            ..
        })
    ));

    let radius_override = solve_with_prefix(
        ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Radius(64.0),
        },
        &velocity_viper,
        velocity_viper.train.cars,
        15.8,
    );
    let (model, prefix, solved) =
        radius_override.expect("a finite radius override uses the table-backed turn solve");
    assert_eq!(solved.calibration_runs, 1);
    let ElementSpec::Turn { radius_m, .. } = solved.spec else {
        panic!("turn intent resolved to a hill")
    };
    assert!((radius_m - 64.0).abs() < 1e-6);
    assert_grid_connector(solved.piece.exit);
    assert_turn_scenarios(&prefix, &model, &solved.piece, 15.8);
}

#[test]
fn unsupported_hill_feels_return_table_unavailable() {
    let hyper = hyper_model();
    let result = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Speed,
            strength_g: None,
            size: HillSizeHint::Auto,
        },
        &hyper,
        hyper.train.cars,
        28.0,
    );
    assert_eq!(result.unwrap_err(), SolveRefusal::TableUnavailable);
}

#[test]
fn feel_defaults_match_requested_targets_or_refuse() {
    for base_model in coaster_models() {
        for cars in base_model.cars_per_train.clone() {
            let model = model_with_train_length(&base_model, cars);
            let rules = model.shaped_element_rules();
            for (feel, requested_target_g) in [
                (HillFeel::Floater, rules.default_floater_g),
                (HillFeel::StrongFloater, rules.default_strong_floater_g),
                (
                    HillFeel::Ejector,
                    if model.track_material == theme_park_sim::track::Material::Wood {
                        rules.wooden_ejector_g
                    } else {
                        rules.steel_ejector_g
                    },
                ),
            ] {
                let row_outcome =
                    generated_hill_outcome(&base_model, cars, feel, requested_target_g, 28.0);
                let result = solve_with_prefix(
                    ElementIntent::Hill {
                        feel,
                        strength_g: None,
                        size: HillSizeHint::Auto,
                    },
                    &base_model,
                    cars,
                    28.0,
                );
                match (generated_row_is_solved(row_outcome), result) {
                    (true, Ok((_, _, solved))) => {
                        let ElementSpec::Hill {
                            target_g,
                            delivered_g,
                            ..
                        } = solved.spec
                        else {
                            panic!("hill intent resolved to a turn")
                        };
                        assert!(
                            (delivered_g - target_g).abs() <= 0.1,
                            "{} {feel:?} delivered {delivered_g:.3} g for capped target {target_g:.3} g",
                            model.key
                        );
                        if (requested_target_g - target_g).abs() > 1e-9 {
                            let note: FeelCapNote = solved
                                .cap_note
                                .expect("capped feel default returns a typed note");
                            assert_eq!(note.feel, feel);
                            assert!((note.requested_target_g - requested_target_g).abs() < 1e-9);
                            assert!((note.capped_target_g - target_g).abs() < 1e-9);
                            assert!((note.delivered_g - delivered_g).abs() < 1e-9);
                        } else {
                            assert!(solved.cap_note.is_none());
                        }
                    }
                    (false, refusal) if table_failure_has_typed_refusal(&refusal) => {}
                    (expected_solved, actual) => panic!(
                        "{} {feel:?} table solved={expected_solved}, runtime result={actual:?}",
                        model.key
                    ),
                }
            }
        }
    }
}
