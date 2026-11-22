use std::fs;
use std::path::Path;

use theme_park_sim::physics::{
    Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache, PreviewStatus, TestRun,
    TrackPreview, Train,
};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::ride_file::{
    RideFile, RideFileFormat, RideHeading, RideShapeStep, RideStation, RideStep,
};
use theme_park_sim::track::saved_shape_check::check_saved_shaped_geometry;
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, ShapedConstructionError, SolveContext, SolveRefusal,
    TurnSizeHint, default_hill_strength, solve, solve_confirm, solve_preview,
};
use theme_park_sim::track::{
    Bank, CoasterModel, Hand, HillFeel, HorizontalCurvature, Piece, PieceId, PieceKind, Slope,
    TrackState, TrainConfiguration, VerticalCurvature, catalog, hyper_model,
};
use theme_park_sim::world::{Surface, World};

const DESIGN_SPEEDS_MPS: [f64; 6] = [22.0, 24.0, 26.0, 30.0, 34.0, 36.0];
const HILL_INTENTS: [(HillFeel, f64); 2] =
    [(HillFeel::Floater, -0.5), (HillFeel::StrongFloater, -0.85)];

fn model_with_train_length(base_model: &CoasterModel, cars: u8) -> CoasterModel {
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
        .expect("catalog includes a station");
    let straight_template = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let mut prefix = vec![
        station_template
            .at(TrackState::origin())
            .expect("station starts at the origin"),
    ];
    let default_physics = PhysicsConfig::default();
    let acceleration_distance_m = (theme_park_sim::math::powi(speed_mps, 2)
        - theme_park_sim::math::powi(default_physics.station_speed, 2))
    .max(0.0)
        / (2.0 * default_physics.drive_acceleration);
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

fn content_ride_paths() -> Vec<std::path::PathBuf> {
    let rides_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/rides");
    let mut ride_paths: Vec<_> = fs::read_dir(rides_directory)
        .expect("content/rides exists")
        .map(|entry| entry.expect("ride directory entry is readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    ride_paths.sort();
    ride_paths
}

#[test]
fn every_content_ride_shaped_intent_resolves_to_its_saved_geometry_digest() {
    for path in content_ride_paths() {
        let source = fs::read(&path)
            .unwrap_or_else(|error| panic!("could not read ride {}: {error}", path.display()));
        let ride = RideFile::from_json_slice(&source)
            .unwrap_or_else(|error| panic!("could not parse ride {}: {error}", path.display()));
        if let Err(message) = check_saved_shaped_geometry(&ride) {
            panic!("{}: {message}", ride.name);
        }
    }
}

#[test]
fn saved_shaped_geometry_check_rejects_a_ride_saved_by_an_older_solver() {
    let ride = RideFile::from_json_str(include_str!(
        "../../../web/e2e/rides/fixtures/horizon-hawk-older-solver.ride.json"
    ))
    .expect("the older Horizon Hawk fixture parses");
    let message = check_saved_shaped_geometry(&ride)
        .expect_err("the older solver's first shaped turn differs from the current solver");
    assert!(
        message.starts_with("ride step 11 at ")
            && message.contains("saved shaped geometry digest 7a165ba6bd127329 differs from the current solver's 0f412105af8180cd"),
        "{message}"
    );
}

fn simulate_with_preview(
    prefix: &[Piece],
    model: &CoasterModel,
    candidate: &Piece,
    entry_speed_mps: f64,
) -> TrackPreview {
    let (_, mut cache) = entry_preview(prefix, model, entry_speed_mps, PhysicsConfig::default());
    let candidate_index = prefix.len();
    let mut pieces = prefix.to_vec();
    pieces.push(candidate.clone());
    pieces.extend(runout_from(candidate, &model.train));
    let preview = cache
        .preview_all_cars(
            &pieces,
            model,
            PhysicsConfig {
                lift_speed: entry_speed_mps,
                ..PhysicsConfig::default()
            },
        )
        .expect("candidate circuit contains a station");
    assert_eq!(
        preview.status,
        PreviewStatus::ReachedEnd,
        "candidate failed to carry the train through its runout"
    );
    assert_candidate_readings(&preview.report, candidate_index, model);
    preview
}

fn simulate_with_construction_test_physics(
    prefix: &[Piece],
    model: &CoasterModel,
    candidate: &Piece,
    entry_speed_mps: f64,
) -> TestRun {
    let candidate_index = prefix.len();
    let mut pieces = prefix.to_vec();
    pieces.push(candidate.clone());
    pieces.extend(runout_from(candidate, &model.train));
    let circuit = Circuit::open(pieces).expect("candidate track has a station and runout");
    let candidate_start_m = prefix.iter().map(Piece::length).sum::<f64>();
    let candidate_end_m = candidate_start_m + candidate.length();
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
            "construction-test train recovered before clearing candidate: {:?}",
            train.notes()
        );
        if train.distance() >= clear_candidate_distance {
            let report = recorder.finish(false, false);
            assert_candidate_readings(&report, candidate_index, model);
            return report;
        }
    }
    panic!("construction-test train did not clear the candidate and runout");
}

fn assert_candidate_readings(report: &TestRun, piece_index: usize, model: &CoasterModel) {
    let candidate_peaks: Vec<_> = report
        .car_piece_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        candidate_peaks.len(),
        usize::from(model.train.cars),
        "missing per-car forces on the shaped element"
    );
}

fn assert_preview_budget(solved: &theme_park_sim::track::shaped::SolvedElement, description: &str) {
    assert!(
        (1..=4).contains(&solved.calibration_runs),
        "{description}: solver used {} previews, expected one to four",
        solved.calibration_runs
    );
}

fn assert_ride_file_shape_round_trip(
    model: &CoasterModel,
    cars: u8,
    intent: &ElementIntent,
    spec: &ElementSpec,
) {
    let ride = RideFile {
        format: RideFileFormat::HeartlineRide,
        version: 1,
        name: "Shaped element acceptance".to_owned(),
        model: model.key.to_owned(),
        station: RideStation {
            x: 0,
            y: 0,
            z: 0,
            heading: RideHeading::East,
        },
        cars_per_train: cars,
        trains: 1,
        steps: vec![RideStep::Shape(RideShapeStep {
            intent: intent.clone(),
            spec: spec.clone(),
        })],
        expect: None,
    };
    let serialized = serde_json::to_string(&ride).expect("solved ride file serializes");
    assert!(
        serialized.len() < 100_000,
        "ride file is {} bytes",
        serialized.len()
    );
    let encoded: serde_json::Value =
        serde_json::from_str(&serialized).expect("solved ride JSON parses");
    let spec_json = &encoded["steps"][0]["spec"];
    assert!(spec_json.get("geometry").is_none());
    assert!(spec_json.get("profile").is_none());
    assert!(spec_json["geometry_digest"].as_str().is_some());
    let restored: RideFile = serde_json::from_str(&serialized).expect("ride file deserializes");
    let serialized_again = serde_json::to_string(&restored).expect("restored ride file serializes");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serialized_again)
            .expect("restored ride JSON parses"),
        serde_json::from_str::<serde_json::Value>(&serialized).expect("ride JSON parses"),
        "fixed-decimal ride-file round trip changed the shaped step"
    );
    let [RideStep::Shape(restored_shape)] = restored.steps.as_slice() else {
        panic!("ride-file round trip did not retain its shaped step")
    };
    assert_eq!(
        serde_json::to_value(&restored_shape.intent).expect("restored intent serializes"),
        serde_json::to_value(intent).expect("intent serializes"),
        "ride-file round trip changed shaped intent"
    );
    assert_eq!(
        serde_json::to_value(&restored_shape.spec).expect("restored spec serializes"),
        serde_json::to_value(spec).expect("resolved spec serializes"),
        "ride-file round trip changed canonical resolved geometry"
    );
}

#[test]
fn shaped_ride_specs_store_compact_recipes_and_reject_a_changed_geometry_digest() {
    let hill_intent = ElementIntent::Hill {
        feel: HillFeel::StrongFloater,
        strength_g: -0.85,
        size: HillSizeHint::Height(32.0),
    };
    let turn_intent = ElementIntent::Turn {
        hand: Hand::Left,
        size: TurnSizeHint::Auto,
    };

    for intent in [hill_intent, turn_intent] {
        let (_, _, solved) = solve_with_prefix(intent.clone(), &hyper_model(), 4, 30.0)
            .expect("representative shaped element solves");
        let serialized_spec = serde_json::to_value(&solved.spec).unwrap();
        assert!(serialized_spec.get("geometry").is_none());
        assert!(serialized_spec.get("profile").is_none());
        let digest_string = serialized_spec["geometry_digest"]
            .as_str()
            .expect("human-readable geometry digest is a decimal string");
        let serialized_digest = digest_string.parse::<u64>().unwrap();
        assert!(serialized_digest > (1_u64 << 53));
        assert_eq!(serialized_digest, solved.spec.resolved_geometry_digest());

        let compact_json = serde_json::to_vec(&solved.spec).unwrap();
        assert!(
            compact_json.len() < 2_000,
            "spec uses {} bytes",
            compact_json.len()
        );
        let restored: ElementSpec = serde_json::from_slice(&compact_json).unwrap();
        assert_eq!(
            restored, solved.spec,
            "compact recipe changed resolved geometry"
        );

        let compact_binary = postcard::to_allocvec(&solved.spec).unwrap();
        assert!(
            compact_binary.len() < 256,
            "binary spec uses {} bytes",
            compact_binary.len()
        );
        let restored_binary: ElementSpec = postcard::from_bytes(&compact_binary)
            .unwrap_or_else(|error| panic!("{intent:?} binary recipe failed: {error}"));
        assert_eq!(
            restored_binary, solved.spec,
            "binary recipe changed resolved geometry"
        );

        let mut changed_digest = serialized_spec;
        let changed_digest_value = changed_digest["geometry_digest"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            .wrapping_add(1);
        changed_digest["geometry_digest"] = serde_json::json!(changed_digest_value.to_string());
        let error = serde_json::from_value::<ElementSpec>(changed_digest).unwrap_err();
        assert!(
            error.to_string().contains("geometry digest mismatch"),
            "{error}"
        );

        let mut changed_spec = solved.spec.clone();
        match &mut changed_spec {
            ElementSpec::Hill {
                geometry_digest, ..
            }
            | ElementSpec::Turn {
                geometry_digest, ..
            } => *geometry_digest = geometry_digest.wrapping_add(1),
        }
        let typed_error = changed_spec.verify_geometry_digest().unwrap_err();
        assert_ne!(typed_error.expected, typed_error.actual);
    }
}

#[test]
fn ride_file_loader_preserves_typed_geometry_digest_mismatches() {
    let intent = ElementIntent::Turn {
        hand: Hand::Left,
        size: TurnSizeHint::Auto,
    };
    let (_, _, solved) = solve_with_prefix(intent.clone(), &hyper_model(), 4, 30.0)
        .expect("representative shaped element solves");
    let ride = RideFile {
        format: RideFileFormat::HeartlineRide,
        version: 1,
        name: "Typed shaped load".to_owned(),
        model: "hyper".to_owned(),
        station: RideStation {
            x: 0,
            y: 0,
            z: 0,
            heading: RideHeading::East,
        },
        cars_per_train: 4,
        trains: 1,
        steps: vec![RideStep::Shape(RideShapeStep {
            intent,
            spec: solved.spec,
        })],
        expect: None,
    };
    let mut ride_json = serde_json::to_value(&ride).expect("ride serializes");
    let changed_digest = ride_json["steps"][0]["spec"]["geometry_digest"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap()
        .wrapping_add(1);
    ride_json["steps"][0]["spec"]["geometry_digest"] =
        serde_json::json!(changed_digest.to_string());
    let ride_json = serde_json::to_string(&ride_json).expect("ride JSON serializes");

    let error = RideFile::from_json_str(&ride_json).unwrap_err();
    let theme_park_sim::track::ride_file::RideFileLoadError::ShapedGeometryDigestMismatch {
        step_index,
        mismatch,
    } = error
    else {
        panic!("expected typed shaped geometry digest mismatch, got {error}");
    };
    assert_eq!(step_index, 0);
    assert_ne!(mismatch.expected, mismatch.actual);
}

fn assert_force_limits(
    report: &TestRun,
    piece_index: usize,
    model: &CoasterModel,
    headroom_required: bool,
    description: &str,
) {
    let candidate_peaks: Vec<_> = report
        .car_piece_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        candidate_peaks.len(),
        usize::from(model.train.cars),
        "{description}: missing per-car forces"
    );
    let limits = model.design_force_targets;
    let headroom_g = if headroom_required { 0.25 } else { 0.0 };
    for car in candidate_peaks {
        let peaks = car.peaks;
        assert!(
            peaks.minimum_vertical_g >= limits.minimum_vertical_g + headroom_g,
            "{description}, car {}: minimum vertical {:.3} g, model limit {:.3} g, required headroom {:.2} g",
            car.car_index,
            peaks.minimum_vertical_g,
            limits.minimum_vertical_g,
            headroom_g
        );
        assert!(
            peaks.maximum_vertical_g <= limits.maximum_vertical_g - headroom_g,
            "{description}, car {}: maximum vertical {:.3} g, model limit {:.3} g, required headroom {:.2} g",
            car.car_index,
            peaks.maximum_vertical_g,
            limits.maximum_vertical_g,
            headroom_g
        );
        assert!(
            peaks.maximum_lateral_g <= limits.maximum_lateral_g - headroom_g,
            "{description}, car {}: lateral {:.3} g, model limit {:.3} g, required headroom {:.2} g",
            car.car_index,
            peaks.maximum_lateral_g,
            limits.maximum_lateral_g,
            headroom_g
        );
        assert!(
            peaks.maximum_longitudinal_g <= limits.maximum_longitudinal_g - headroom_g,
            "{description}, car {}: longitudinal {:.3} g, model limit {:.3} g, required headroom {:.2} g",
            car.car_index,
            peaks.maximum_longitudinal_g,
            limits.maximum_longitudinal_g,
            headroom_g
        );
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

fn assert_model_minimum_radius(piece: &Piece, model: &CoasterModel, description: &str) {
    let minimum_radius_m = piece.maximum_curvature().recip();
    assert!(
        minimum_radius_m >= model.minimum_radius_metres,
        "{description}: rail radius {minimum_radius_m:.3} m is below model minimum {:.3} m",
        model.minimum_radius_metres
    );
}

fn worst_car_sustained_airtime(report: &TestRun, piece_index: usize, model: &CoasterModel) -> f64 {
    let sustained: Vec<_> = report
        .car_piece_sustained_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        sustained.len(),
        usize::from(model.train.cars),
        "missing per-car sustained-force readings"
    );
    sustained
        .iter()
        .map(|peaks| peaks.peaks.minimum_vertical_g.unwrap_or(0.0))
        .fold(f64::INFINITY, f64::min)
}

fn worst_car_lateral(report: &TestRun, piece_index: usize, model: &CoasterModel) -> f64 {
    let candidate_peaks: Vec<_> = report
        .car_piece_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == piece_index)
        .collect();
    assert_eq!(
        candidate_peaks.len(),
        usize::from(model.train.cars),
        "missing per-car lateral readings"
    );
    candidate_peaks
        .into_iter()
        .map(|peaks| peaks.peaks.maximum_lateral_g)
        .fold(0.0, f64::max)
}

fn assert_hill_force_contract(
    prefix: &[Piece],
    model: &CoasterModel,
    piece: &Piece,
    design_speed_mps: f64,
    capped_target_g: f64,
    description: &str,
) {
    let candidate_index = prefix.len();
    for speed_offset_mps in [-0.5, 0.0, 0.5] {
        let entry_speed_mps = design_speed_mps + speed_offset_mps;
        let preview = simulate_with_preview(prefix, model, piece, entry_speed_mps);
        let construction_test =
            simulate_with_construction_test_physics(prefix, model, piece, entry_speed_mps);
        let design_speed = speed_offset_mps == 0.0;
        assert_force_limits(
            &preview.report,
            candidate_index,
            model,
            design_speed,
            &format!("{description}, preview physics at {entry_speed_mps:.1} m/s"),
        );
        assert_force_limits(
            &construction_test,
            candidate_index,
            model,
            design_speed,
            &format!("{description}, construction-test physics at {entry_speed_mps:.1} m/s"),
        );
        if design_speed {
            let runtime_airtime =
                worst_car_sustained_airtime(&preview.report, candidate_index, model);
            let construction_airtime =
                worst_car_sustained_airtime(&construction_test, candidate_index, model);
            assert!(
                (runtime_airtime - capped_target_g).abs() <= 0.10,
                "{description}: preview worst-car airtime {runtime_airtime:.3} g differs from capped target {capped_target_g:.3} g"
            );
            assert!(
                (construction_airtime - capped_target_g).abs() <= 0.15,
                "{description}: construction-test worst-car airtime {construction_airtime:.3} g differs from capped target {capped_target_g:.3} g"
            );
        }
    }
}

fn assert_turn_lateral_contract(
    prefix: &[Piece],
    model: &CoasterModel,
    piece: &Piece,
    design_speed_mps: f64,
    description: &str,
) {
    let candidate_index = prefix.len();
    for speed_offset_mps in [-0.5, 0.0, 0.5] {
        let entry_speed_mps = design_speed_mps + speed_offset_mps;
        let preview = simulate_with_preview(prefix, model, piece, entry_speed_mps);
        let construction_test =
            simulate_with_construction_test_physics(prefix, model, piece, entry_speed_mps);
        let design_speed = speed_offset_mps == 0.0;
        assert_force_limits(
            &preview.report,
            candidate_index,
            model,
            design_speed,
            &format!("{description}, preview physics at {entry_speed_mps:.1} m/s"),
        );
        assert_force_limits(
            &construction_test,
            candidate_index,
            model,
            design_speed,
            &format!("{description}, construction-test physics at {entry_speed_mps:.1} m/s"),
        );
        let lateral_limit_g = if design_speed { 0.15 } else { 0.25 };
        for (physics_name, report) in [
            ("preview", &preview.report),
            ("construction-test", &construction_test),
        ] {
            let observed_lateral = worst_car_lateral(report, candidate_index, model);
            assert!(
                observed_lateral <= lateral_limit_g,
                "{description}: {physics_name} lateral {observed_lateral:.3} g exceeds {lateral_limit_g:.2} g at {entry_speed_mps:.1} m/s"
            );
        }
    }
}

#[test]
fn solved_hill_specs_pass_the_resolved_construction_guard() {
    let intent = ElementIntent::Hill {
        feel: HillFeel::Floater,
        strength_g: -0.5,
        size: HillSizeHint::Auto,
    };
    let (model, _, solved) = solve_with_prefix(intent.clone(), &hyper_model(), 4, 22.0)
        .expect("supported floater hill solves");
    let mut construction = Construction::new(solved.piece.entry, model.clone());
    let world = World::new(
        512,
        512,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let result = construction.append_resolved(&intent, &solved.spec, &world, &[]);

    assert!(
        !matches!(result, Err(ShapedConstructionError::InvalidResolvedSpec)),
        "solver-generated hill spec was rejected before construction validation: {result:?}"
    );
}

#[test]
fn hyper_hills_meet_airtime_and_force_limits_for_four_to_eight_cars() {
    let base_model = hyper_model();
    for cars in 4..=8 {
        let mut previous_heights_m = [None, None];
        for speed_mps in DESIGN_SPEEDS_MPS {
            for (intent_index, (feel, requested_target_g)) in HILL_INTENTS.into_iter().enumerate() {
                let (model, prefix, solved) = solve_with_prefix(
                    ElementIntent::Hill {
                        feel,
                        strength_g: requested_target_g,
                        size: HillSizeHint::Auto,
                    },
                    &base_model,
                    cars,
                    speed_mps,
                )
                .unwrap_or_else(|refusal| {
                    panic!("hyper {cars}-car {feel:?} at {speed_mps:.1} m/s refused: {refusal:?}")
                });
                assert_preview_budget(
                    &solved,
                    &format!("hyper {cars}-car {feel:?} at {speed_mps:.1} m/s"),
                );
                let ElementSpec::Hill {
                    target_g,
                    delivered_g,
                    height_m,
                    design_speed_mps,
                    cars: resolved_cars,
                    ..
                } = &solved.spec
                else {
                    panic!("hill intent resolved to a turn")
                };
                assert_eq!(*resolved_cars, cars);
                assert!((design_speed_mps - speed_mps).abs() <= 0.01);
                assert!(
                    (delivered_g - target_g).abs() <= 0.05,
                    "hyper {cars}-car {feel:?} at {speed_mps:.1} m/s records {delivered_g:.3} g for target {target_g:.3} g"
                );
                assert!(*height_m > 0.0);
                if feel == HillFeel::StrongFloater && (24.0..=36.0).contains(&speed_mps) {
                    assert!(
                        *target_g <= -0.75,
                        "hyper {cars}-car strong-floater target was capped to a shallower {target_g:.3} g at {speed_mps:.1} m/s"
                    );
                }
                match solved.cap_note {
                    Some(note) => {
                        assert_eq!(note.feel, feel);
                        assert!((note.capped_target_g - target_g).abs() <= 1e-9);
                        assert!((note.delivered_g - delivered_g).abs() <= 1e-9);
                    }
                    None => assert!((target_g - requested_target_g).abs() <= 1e-9),
                }
                assert_eq!(solved.piece.entry, prefix.last().unwrap().exit);
                assert_grid_connector(solved.piece.exit);
                assert!(model.allows_piece(&solved.piece));
                assert_model_minimum_radius(&solved.piece, &model, "shaped hill");
                if let Some(previous_height_m) = previous_heights_m[intent_index] {
                    assert!(
                        *height_m >= previous_height_m,
                        "hyper {cars}-car {feel:?} height decreased from {previous_height_m:.3} m to {height_m:.3} m as speed rose to {speed_mps:.1} m/s"
                    );
                }
                previous_heights_m[intent_index] = Some(*height_m);
                assert_hill_force_contract(
                    &prefix,
                    &model,
                    &solved.piece,
                    speed_mps,
                    *target_g,
                    &format!("hyper {cars}-car {feel:?} at {speed_mps:.1} m/s"),
                );
            }
        }
    }
}

#[test]
fn hyper_turns_meet_lateral_and_force_limits_for_four_to_eight_cars() {
    let base_model = hyper_model();
    for cars in 4..=8 {
        let mut previous_radius_m = None;
        for speed_mps in DESIGN_SPEEDS_MPS {
            let (model, prefix, solved) = solve_with_prefix(
                ElementIntent::Turn {
                    hand: Hand::Left,
                    size: TurnSizeHint::Auto,
                },
                &base_model,
                cars,
                speed_mps,
            )
            .unwrap_or_else(|refusal| {
                panic!("hyper {cars}-car turn at {speed_mps:.1} m/s refused: {refusal:?}")
            });
            assert_preview_budget(
                &solved,
                &format!("hyper {cars}-car turn at {speed_mps:.1} m/s"),
            );
            let ElementSpec::Turn {
                radius_m,
                bank_degrees,
                design_speed_mps,
                cars: resolved_cars,
                ..
            } = &solved.spec
            else {
                panic!("turn intent resolved to a hill")
            };
            assert_eq!(*resolved_cars, cars);
            assert!((design_speed_mps - speed_mps).abs() <= 0.01);
            assert!(*bank_degrees <= 75.0);
            assert!(*radius_m >= model.minimum_radius_metres);
            if let Some(previous_radius_m) = previous_radius_m {
                assert!(
                    *radius_m >= previous_radius_m,
                    "hyper {cars}-car turn radius decreased from {previous_radius_m:.3} m to {radius_m:.3} m as speed rose to {speed_mps:.1} m/s"
                );
            }
            previous_radius_m = Some(*radius_m);
            assert_eq!(solved.piece.entry, prefix.last().unwrap().exit);
            assert_grid_connector(solved.piece.exit);
            assert!(model.allows_piece(&solved.piece));
            assert_model_minimum_radius(&solved.piece, &model, "shaped turn");
            assert_turn_lateral_contract(
                &prefix,
                &model,
                &solved.piece,
                speed_mps,
                &format!("hyper {cars}-car turn at {speed_mps:.1} m/s"),
            );
        }
    }
}

#[test]
fn fixed_height_hill_keeps_requested_size_and_calibrates_airtime() {
    let requested_height_m = 14.0;
    let (model, prefix, solved) = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: -0.5,
            size: HillSizeHint::Height(requested_height_m),
        },
        &hyper_model(),
        4,
        26.0,
    )
    .unwrap_or_else(|refusal| panic!("fixed-height hill refused: {refusal:?}"));
    let ElementSpec::Hill {
        height_m, target_g, ..
    } = &solved.spec
    else {
        panic!("hill intent resolved to a turn")
    };
    assert!(
        (*height_m - requested_height_m).abs() <= 0.5,
        "resolved height {height_m:.3} m differs from requested {requested_height_m:.3} m"
    );
    assert_hill_force_contract(
        &prefix,
        &model,
        &solved.piece,
        26.0,
        *target_g,
        "fixed-height hyper floater hill",
    );
}

#[test]
fn shaped_elements_below_twenty_two_metres_per_second_return_typed_refusals() {
    let base_model = hyper_model();
    for cars in 4..=8 {
        let hill_result = solve_with_prefix(
            ElementIntent::Hill {
                feel: HillFeel::Floater,
                strength_g: -0.5,
                size: HillSizeHint::Auto,
            },
            &base_model,
            cars,
            21.99,
        );
        assert!(
            matches!(
                hill_result,
                Err(SolveRefusal::BelowMinimumSpeed {
                    minimum_speed_mps: 22.0,
                    ..
                })
            ),
            "hyper {cars}-car hill at 21.99 m/s did not return the typed low-speed refusal: {hill_result:?}"
        );
        let turn_result = solve_with_prefix(
            ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Auto,
            },
            &base_model,
            cars,
            21.99,
        );
        assert!(
            matches!(
                turn_result,
                Err(SolveRefusal::BelowMinimumSpeed {
                    minimum_speed_mps: 22.0,
                    ..
                })
            ),
            "hyper {cars}-car turn at 21.99 m/s did not return the typed low-speed refusal: {turn_result:?}"
        );
    }
}

#[test]
fn shaped_elements_refuse_models_train_sizes_and_speeds_outside_supported_scope() {
    let hyper = hyper_model();
    let non_hyper = theme_park_sim::track::coaster_models()
        .into_iter()
        .find(|model| model.key != "hyper")
        .expect("another coaster model exists");
    for (model, cars, speed_mps) in [
        (&non_hyper, 4, 30.0),
        (&hyper, 3, 30.0),
        (&hyper, 9, 30.0),
        (&hyper, 4, 36.01),
    ] {
        let result = solve_with_prefix(
            ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Auto,
            },
            model,
            cars,
            speed_mps,
        );
        assert_eq!(
            result.unwrap_err(),
            SolveRefusal::UnsupportedScope,
            "{} {cars}-car turn at {speed_mps:.2} m/s did not return a scope refusal",
            model.key
        );
    }
}

#[test]
fn shaped_hill_intent_requires_an_explicit_target_g() {
    let intent =
        serde_json::from_str::<ElementIntent>(r#"{"hill":{"feel":"StrongFloater","size":"Auto"}}"#);

    assert!(
        intent.is_err(),
        "shaped hill intent accepted a missing target g"
    );
}

#[test]
fn shaped_hill_intent_uses_postcard_safe_explicit_strength() {
    let intent = ElementIntent::Hill {
        feel: HillFeel::StrongFloater,
        strength_g: -0.85,
        size: HillSizeHint::Height(24.0),
    };

    let encoded = postcard::to_allocvec(&intent).expect("intent serializes to postcard");
    let decoded = postcard::from_bytes::<ElementIntent>(&encoded)
        .expect("postcard restores the selected hill intent");

    assert_eq!(decoded, intent);
}

#[test]
fn default_hill_targets_match_the_revised_stage_one_intents() {
    let model = hyper_model();
    for (feel, expected_target_g) in [(HillFeel::Floater, -0.5), (HillFeel::StrongFloater, -0.85)] {
        let (_, _, solved) = solve_with_prefix(
            ElementIntent::Hill {
                feel,
                strength_g: default_hill_strength(feel, &model),
                size: HillSizeHint::Auto,
            },
            &model,
            4,
            30.0,
        )
        .expect("default hyper hill is feasible");
        let ElementSpec::Hill { target_g, .. } = solved.spec else {
            panic!("hill intent resolved to a turn")
        };
        assert_eq!(target_g, expected_target_g);
    }
}

#[test]
fn browse_uses_one_preview_and_confirmation_reuses_it_for_the_secant() {
    let model = model_with_train_length(&hyper_model(), 4);
    let prefix = lift_prefix(&model, 34.0);
    let physics = PhysicsConfig {
        lift_speed: 34.0,
        ..PhysicsConfig::default()
    };
    let (_, mut preview_cache) = entry_preview(&prefix, &model, 34.0, physics);
    let intent = ElementIntent::Hill {
        feel: HillFeel::StrongFloater,
        strength_g: -0.85,
        size: HillSizeHint::Auto,
    };
    let mut context = SolveContext {
        entry: prefix.last().expect("prefix has a station").exit,
        entry_speed_mps: 34.0,
        model: &model,
        train: &model.train,
        prefix: &prefix,
        preview_cache: &mut preview_cache,
        physics,
    };
    let browsed = solve_preview(&intent, &mut context).expect("browse preview resolves a hill");
    assert_eq!(browsed.calibration_runs, 1);
    let confirmed = solve_confirm(&intent, &mut context, &browsed)
        .expect("confirmation calibrates the browse candidate");
    assert!((2..=4).contains(&confirmed.calibration_runs));
    let ElementSpec::Hill {
        target_g,
        delivered_g,
        ..
    } = confirmed.spec
    else {
        panic!("hill intent resolved to a turn")
    };
    assert!((delivered_g - target_g).abs() <= 0.05);
}

#[test]
fn force_margin_caps_strong_floater_in_five_hundredths_of_a_g() {
    let model = hyper_model();
    let (_, _, solved) = solve_with_prefix(
        ElementIntent::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: -1.5,
            size: HillSizeHint::Auto,
        },
        &model,
        4,
        34.0,
    )
    .expect("strong-floater target is capped to a feasible hyper target");
    let cap = solved
        .cap_note
        .expect("capped intent reports the selected cap");
    eprintln!(
        "strong-floater cap requested={:.2} g capped={:.2} g delivered={:.2} g previews={}",
        cap.requested_target_g, cap.capped_target_g, cap.delivered_g, solved.calibration_runs
    );
    assert_eq!(cap.requested_target_g, -1.5);
    let model_headroom_floor_g = model.design_force_targets.minimum_vertical_g + 0.25;
    assert!(cap.capped_target_g >= model_headroom_floor_g);
    assert!(cap.capped_target_g <= -0.75);
    let cap_steps = (cap.capped_target_g - cap.requested_target_g) / 0.05;
    assert!((cap_steps - cap_steps.round()).abs() <= 1e-8);
    assert!((cap.delivered_g - cap.capped_target_g).abs() <= 0.05);
    assert!((1..=4).contains(&solved.calibration_runs));
}

#[test]
fn shaped_hill_and_turn_solves_are_deterministic_and_save_round_trip() {
    let base_model = hyper_model();
    for intent in [
        ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: -0.5,
            size: HillSizeHint::Auto,
        },
        ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        },
    ] {
        let first = solve_with_prefix(intent.clone(), &base_model, 8, 30.0)
            .expect("hyper shaped element is feasible")
            .2;
        let second = solve_with_prefix(intent.clone(), &base_model, 8, 30.0)
            .expect("repeated hyper shaped element is feasible")
            .2;
        assert_preview_budget(&first, "first deterministic solve");
        assert_preview_budget(&second, "repeated deterministic solve");
        let first_piece_bytes = postcard::to_allocvec(&first.piece).expect("piece serializes");
        let second_piece_bytes = postcard::to_allocvec(&second.piece).expect("piece serializes");
        assert_eq!(
            first_piece_bytes, second_piece_bytes,
            "solve changed piece bytes"
        );
        let restored_piece: Piece =
            postcard::from_bytes(&first_piece_bytes).expect("piece deserializes");
        assert_eq!(
            postcard::to_allocvec(&restored_piece).expect("restored piece serializes"),
            first_piece_bytes,
            "save round trip changed resolved geometry"
        );
        let first_spec_bytes = postcard::to_allocvec(&first.spec).expect("spec serializes");
        let second_spec_bytes = postcard::to_allocvec(&second.spec).expect("spec serializes");
        assert_eq!(
            first_spec_bytes, second_spec_bytes,
            "solve changed spec bytes"
        );
        let restored_spec: ElementSpec =
            postcard::from_bytes(&first_spec_bytes).expect("spec deserializes");
        assert_eq!(
            postcard::to_allocvec(&restored_spec).expect("restored spec serializes"),
            first_spec_bytes,
            "save round trip changed resolved spec"
        );
        assert_ride_file_shape_round_trip(&base_model, 8, &intent, &first.spec);
    }
}
