use super::{
    HILL_TAPER_ANGLE, HillCandidateMeasurement, SolveContext, SolveRefusal,
    measure_hill_candidate_with_taper,
};
use crate::physics::{PhysicsConfig, PreviewCache, PreviewStatus};
use crate::track::{CoasterModel, Piece, PieceId, PieceKind, TrackState, catalog, coaster_models};
use serde::Serialize;
use std::collections::BTreeSet;

const SPEED_STEP_MPS: u8 = 1;
const TARGET_STEP_G: f64 = 0.05;
const REQUIRED_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_TARGET_ERROR_G: f64 = 0.1;
const PROFILE_FLANK_MIN_DEGREES: f64 = 25.0;
const PROFILE_FLANK_MAX_DEGREES: f64 = 55.0;
const INITIAL_FLANK_MIN_DEGREES: f64 = 25.0;
const INITIAL_FLANK_MAX_DEGREES: f64 = 35.0;
const HYBRID_DIAGNOSTIC_PULL_G: [f64; 6] = [3.0, 3.1, 3.15, 3.2, 3.25, 3.3];
const HYBRID_DIAGNOSTIC_FLANK_DEGREES: [f64; 5] = [14.0, 16.0, 17.0, 18.0, 20.0];
const HYBRID_DIAGNOSTIC_CREST_OFFSETS_G: [f64; 4] = [-0.05, 0.0, 0.05, 0.1];
const HYBRID_DIAGNOSTIC_TAPER_RAD: [f64; 4] = [0.03, 0.04, 0.05, 0.08];
const HYBRID_DIAGNOSTIC_CANDIDATES: usize = 480;
const HYPER_DIAGNOSTIC_PULL_G: [f64; 3] = [1.5, 2.0, 2.5];
const HYPER_DIAGNOSTIC_FLANK_DEGREES: [f64; 3] = [30.0, 34.0, 38.0];
const HYPER_DIAGNOSTIC_CREST_OFFSETS_G: [f64; 4] = [-0.1, 0.0, 0.1, 0.2];
const HYPER_DIAGNOSTIC_TAPER_RAD: [f64; 3] = [0.03, 0.05, 0.08];
const HYPER_DIAGNOSTIC_CANDIDATES: usize = 108;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct CalibrationCase {
    model_index: usize,
    cars: u8,
    speed_mps: u8,
    target_g: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct HillSeed {
    pull_g: f64,
    crest_g: f64,
    flank_degrees: f64,
    taper_radians: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct CalibrationSample {
    case: CalibrationCase,
    seed: HillSeed,
    delivered_g: Option<f64>,
    headroom_g: Option<f64>,
    every_car_within_limits: Option<bool>,
    height_m: Option<f64>,
    span_m: Option<f64>,
    profile_minimum_radius_m: Option<f64>,
    piece_minimum_radius_m: Option<f64>,
    model_minimum_radius_m: Option<f64>,
    profile_radius_within_limit: Option<bool>,
    piece_radius_within_limit: Option<bool>,
    cars: Vec<CarMeasurementSample>,
    refusal: Option<SolveRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct CarMeasurementSample {
    car_index: usize,
    minimum_vertical_g: f64,
    maximum_vertical_g: f64,
    maximum_lateral_g: f64,
    maximum_longitudinal_g: f64,
    sustained_minimum_vertical_g: Option<f64>,
    within_limits: bool,
    minimum_vertical_sample: ForceSampleLocation,
    maximum_longitudinal_sample: ForceSampleLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct ForceSampleLocation {
    global_distance_m: f64,
    local_distance_m: f64,
    speed_mps: f64,
    height_m: f64,
    force_g: f64,
    pitch_degrees: f64,
    rail_curvature_per_m: f64,
    mean_train_slope_z: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct CalibrationCoefficientRow {
    model_key: String,
    cars: u8,
    speed_mps: u8,
    target_g: f64,
    seed: Option<HillSeed>,
    measured_delivery_g: Option<f64>,
    measured_headroom_g: Option<f64>,
    status: CoefficientStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum CoefficientStatus {
    Calibrated,
    TheoreticallyInfeasible,
    NoFeasibleSeedFound,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct CalibrationDataset {
    schema_version: u8,
    generator: &'static str,
    target_step_g: f64,
    minimum_headroom_g: f64,
    maximum_delivery_error_g: f64,
    rows: Vec<CalibrationCoefficientRow>,
    exploratory_samples: Vec<CalibrationSample>,
}

#[derive(Clone, Copy, Debug)]
struct CandidateResult {
    seed: HillSeed,
    delivered_g: f64,
    headroom_g: f64,
    height_m: f64,
    error_g: f64,
}

/// Offline calibration table generation is opt-in because the full train/speed/force grid is costly.
/// Run with `UPDATE_SHAPED_CALIBRATION_COEFFICIENTS=1` and set
/// `SHAPED_CALIBRATION_OUTPUT=/tmp/shaped-calibration.json` to write deterministic JSON.
#[test]
#[ignore = "offline shaped calibration data generation"]
fn generate_shaped_hill_calibration_coefficients() {
    assert_eq!(
        std::env::var("UPDATE_SHAPED_CALIBRATION_COEFFICIENTS").as_deref(),
        Ok("1"),
        "set UPDATE_SHAPED_CALIBRATION_COEFFICIENTS=1 to generate calibration data"
    );
    let output_path = std::env::var("SHAPED_CALIBRATION_OUTPUT")
        .expect("set SHAPED_CALIBRATION_OUTPUT to an explicit output path");
    let dataset = generate_dataset(&coefficient_cases(), &coaster_models(), false);
    let complete = dataset
        .rows
        .iter()
        .all(|row| row.status != CoefficientStatus::NoFeasibleSeedFound);
    let bytes = serde_json::to_vec_pretty(&dataset).expect("calibration dataset serializes");
    std::fs::write(output_path, bytes).expect("write calibration dataset");
    assert!(
        complete,
        "at least one feasible target has no measured coefficient seed; inspect the generated output"
    );
}

/// A bounded sweep for diagnosing response shape before running the full offline grid.
#[test]
#[ignore = "offline representative calibration sweep"]
fn explore_representative_hill_calibration_coefficients() {
    assert_eq!(
        std::env::var("EXPLORE_SHAPED_CALIBRATION_COEFFICIENTS").as_deref(),
        Ok("1"),
        "set EXPLORE_SHAPED_CALIBRATION_COEFFICIENTS=1 to run the exploratory sweep"
    );
    let output_path = std::env::var("SHAPED_CALIBRATION_OUTPUT")
        .expect("set SHAPED_CALIBRATION_OUTPUT to an explicit output path");
    let models = coaster_models();
    let cases = representative_cases(&models);
    let diagnostic_counts = cases
        .iter()
        .map(|case| {
            (
                models[case.model_index].key,
                diagnostic_candidate_seeds(case, &models[case.model_index]).len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        diagnostic_counts[0],
        ("hybrid", HYBRID_DIAGNOSTIC_CANDIDATES)
    );
    assert_eq!(diagnostic_counts[1], ("hyper", HYPER_DIAGNOSTIC_CANDIDATES));
    let dataset = generate_dataset(&cases, &models, true);
    let bytes = serde_json::to_vec_pretty(&dataset).expect("exploration data serializes");
    std::fs::write(output_path, bytes).expect("write exploration data");
}

fn generate_dataset(
    cases: &[CalibrationCase],
    models: &[CoasterModel],
    retain_exploration: bool,
) -> CalibrationDataset {
    let mut rows = Vec::with_capacity(cases.len());
    let mut exploratory_samples = Vec::new();
    for case in cases {
        let model = model_for_case(models, case);
        if case.target_g - model.comfortable_force_limits.minimum_vertical_g
            + MAXIMUM_TARGET_ERROR_G
            < REQUIRED_FORCE_HEADROOM_G
        {
            rows.push(CalibrationCoefficientRow {
                model_key: model.key.to_owned(),
                cars: case.cars,
                speed_mps: case.speed_mps,
                target_g: case.target_g,
                seed: None,
                measured_delivery_g: None,
                measured_headroom_g: None,
                status: CoefficientStatus::TheoreticallyInfeasible,
            });
            continue;
        }

        let mut valid_candidates = Vec::new();
        let seeds = if retain_exploration {
            diagnostic_candidate_seeds(case, model)
        } else {
            coefficient_candidate_seeds(case, &model)
        };
        for seed in seeds {
            let result = measure_case(case, &model, seed);
            let measurement = match result {
                Ok(measurement) => {
                    if retain_exploration {
                        exploratory_samples.push(CalibrationSample {
                            case: *case,
                            seed,
                            delivered_g: measurement.delivered_g,
                            headroom_g: Some(measurement.headroom_g),
                            every_car_within_limits: Some(measurement.every_car_within_limits),
                            height_m: Some(measurement.height_m),
                            span_m: Some(measurement.span_m),
                            profile_minimum_radius_m: Some(measurement.profile_minimum_radius_m),
                            piece_minimum_radius_m: Some(measurement.piece_minimum_radius_m),
                            model_minimum_radius_m: Some(measurement.model_minimum_radius_m),
                            profile_radius_within_limit: Some(
                                measurement.profile_radius_within_limit,
                            ),
                            piece_radius_within_limit: Some(measurement.piece_radius_within_limit),
                            cars: measurement
                                .cars
                                .iter()
                                .map(|car| CarMeasurementSample {
                                    car_index: car.car_index,
                                    minimum_vertical_g: car.peaks.minimum_vertical_g,
                                    maximum_vertical_g: car.peaks.maximum_vertical_g,
                                    maximum_lateral_g: car.peaks.maximum_lateral_g,
                                    maximum_longitudinal_g: car.peaks.maximum_longitudinal_g,
                                    sustained_minimum_vertical_g: car.sustained_minimum_vertical_g,
                                    within_limits: car.within_limits,
                                    minimum_vertical_sample: ForceSampleLocation {
                                        global_distance_m: car.minimum_vertical_sample.distance_m,
                                        local_distance_m: car
                                            .minimum_vertical_sample
                                            .local_distance_m,
                                        speed_mps: car.minimum_vertical_sample.speed_mps,
                                        height_m: car.minimum_vertical_sample.height_m,
                                        force_g: car.minimum_vertical_sample.force_g,
                                        pitch_degrees: car.minimum_vertical_sample.pitch_degrees,
                                        rail_curvature_per_m: car
                                            .minimum_vertical_sample
                                            .rail_curvature_per_m,
                                        mean_train_slope_z: car
                                            .minimum_vertical_sample
                                            .mean_train_slope_z,
                                    },
                                    maximum_longitudinal_sample: ForceSampleLocation {
                                        global_distance_m: car
                                            .maximum_longitudinal_sample
                                            .distance_m,
                                        local_distance_m: car
                                            .maximum_longitudinal_sample
                                            .local_distance_m,
                                        speed_mps: car.maximum_longitudinal_sample.speed_mps,
                                        height_m: car.maximum_longitudinal_sample.height_m,
                                        force_g: car.maximum_longitudinal_sample.force_g,
                                        pitch_degrees: car
                                            .maximum_longitudinal_sample
                                            .pitch_degrees,
                                        rail_curvature_per_m: car
                                            .maximum_longitudinal_sample
                                            .rail_curvature_per_m,
                                        mean_train_slope_z: car
                                            .maximum_longitudinal_sample
                                            .mean_train_slope_z,
                                    },
                                })
                                .collect(),
                            refusal: None,
                        });
                    }
                    measurement
                }
                Err(refusal) => {
                    if retain_exploration {
                        exploratory_samples.push(CalibrationSample {
                            case: *case,
                            seed,
                            delivered_g: None,
                            headroom_g: None,
                            every_car_within_limits: None,
                            height_m: None,
                            span_m: None,
                            profile_minimum_radius_m: None,
                            piece_minimum_radius_m: None,
                            model_minimum_radius_m: None,
                            profile_radius_within_limit: None,
                            piece_radius_within_limit: None,
                            cars: Vec::new(),
                            refusal: Some(refusal),
                        });
                    }
                    continue;
                }
            };
            let Some(delivered_g) = measurement.delivered_g else {
                continue;
            };
            if measurement.every_car_within_limits
                && measurement.headroom_g >= REQUIRED_FORCE_HEADROOM_G
                && (delivered_g - case.target_g).abs() <= MAXIMUM_TARGET_ERROR_G
                && measurement.piece_radius_within_limit
            {
                valid_candidates.push(CandidateResult {
                    seed,
                    delivered_g,
                    headroom_g: measurement.headroom_g,
                    height_m: measurement.height_m,
                    error_g: (delivered_g - case.target_g).abs(),
                });
            }
            if !retain_exploration && !valid_candidates.is_empty() {
                break;
            }
        }
        valid_candidates.sort_by(|left, right| {
            left.error_g
                .total_cmp(&right.error_g)
                .then_with(|| right.headroom_g.total_cmp(&left.headroom_g))
                .then_with(|| left.height_m.total_cmp(&right.height_m))
        });
        let best = valid_candidates.first();
        rows.push(CalibrationCoefficientRow {
            model_key: model.key.to_owned(),
            cars: case.cars,
            speed_mps: case.speed_mps,
            target_g: case.target_g,
            seed: best.map(|candidate| candidate.seed),
            measured_delivery_g: best.map(|candidate| candidate.delivered_g),
            measured_headroom_g: best.map(|candidate| candidate.headroom_g),
            status: if best.is_some() {
                CoefficientStatus::Calibrated
            } else {
                CoefficientStatus::NoFeasibleSeedFound
            },
        });
    }
    CalibrationDataset {
        schema_version: 2,
        generator: "track::shaped::calibration_coefficients",
        target_step_g: TARGET_STEP_G,
        minimum_headroom_g: REQUIRED_FORCE_HEADROOM_G,
        maximum_delivery_error_g: MAXIMUM_TARGET_ERROR_G,
        rows,
        exploratory_samples,
    }
}

fn coefficient_cases() -> Vec<CalibrationCase> {
    let models = coaster_models();
    let mut cases = Vec::new();
    for (model_index, model) in models.iter().enumerate() {
        let target_minimum = model.comfortable_force_limits.minimum_vertical_g
            + REQUIRED_FORCE_HEADROOM_G
            - MAXIMUM_TARGET_ERROR_G;
        let mut target_steps = BTreeSet::new();
        let mut target_step = ((target_minimum / TARGET_STEP_G) - 1e-8).ceil() as i32;
        while target_step <= 0 {
            target_steps.insert(target_step);
            target_step += 1;
        }
        for feel_target in default_feel_targets(model) {
            if feel_target <= 0.0
                && feel_target >= model.comfortable_force_limits.minimum_vertical_g
            {
                target_steps.insert((feel_target / TARGET_STEP_G).round() as i32);
            }
        }
        for cars in model.cars_per_train.clone() {
            for speed_mps in (14..=36).step_by(usize::from(SPEED_STEP_MPS)) {
                for &target_step in &target_steps {
                    cases.push(CalibrationCase {
                        model_index,
                        cars,
                        speed_mps,
                        target_g: f64::from(target_step) * TARGET_STEP_G,
                    });
                }
            }
        }
    }
    cases
}

fn representative_cases(models: &[CoasterModel]) -> Vec<CalibrationCase> {
    let (hybrid_index, hybrid) = models
        .iter()
        .enumerate()
        .find(|(_, model)| model.key == "hybrid")
        .expect("model registry contains hybrid coaster");
    let (hyper_index, hyper) = models
        .iter()
        .enumerate()
        .find(|(_, model)| model.key == "hyper")
        .expect("model registry contains hyper coaster");
    vec![
        CalibrationCase {
            model_index: hybrid_index,
            cars: 4,
            speed_mps: 14,
            target_g: hybrid.comfortable_force_limits.minimum_vertical_g + 0.35,
        },
        CalibrationCase {
            model_index: hyper_index,
            cars: hyper.train.cars,
            speed_mps: 34,
            target_g: -0.9,
        },
    ]
}

fn default_feel_targets(model: &CoasterModel) -> [f64; 3] {
    let rules = model.shaped_element_rules();
    [
        rules.default_floater_g,
        rules.default_strong_floater_g,
        if model.track_material == crate::track::Material::Wood {
            rules.wooden_ejector_g
        } else {
            rules.steel_ejector_g
        },
    ]
}

fn quantize_target(target_g: f64) -> f64 {
    (target_g / TARGET_STEP_G).round() * TARGET_STEP_G
}

fn model_for_case<'a>(models: &'a [CoasterModel], case: &CalibrationCase) -> &'a CoasterModel {
    models
        .get(case.model_index)
        .expect("case model index belongs to the model registry")
}

fn coefficient_candidate_seeds(case: &CalibrationCase, model: &CoasterModel) -> Vec<HillSeed> {
    let speed_fraction = (f64::from(case.speed_mps) - 14.0) / (36.0 - 14.0);
    let automatic_flank = INITIAL_FLANK_MIN_DEGREES
        + speed_fraction * (INITIAL_FLANK_MAX_DEGREES - INITIAL_FLANK_MIN_DEGREES);
    let rules = model.shaped_element_rules();
    let mut seeds = Vec::new();
    for pull_offset_g in [0.0, -0.25, -0.5] {
        let pull_g = rules.hill_pull_g + pull_offset_g;
        if pull_g < 1.25 {
            continue;
        }
        for flank_offset_degrees in [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0] {
            let flank_degrees = (automatic_flank + flank_offset_degrees)
                .clamp(PROFILE_FLANK_MIN_DEGREES, PROFILE_FLANK_MAX_DEGREES);
            let crest_offsets = if pull_offset_g == 0.0 && flank_offset_degrees == 0.0 {
                [0.0, -0.1, 0.1, -0.2, 0.2]
            } else {
                [-0.1, 0.0, 0.1, -0.2, 0.2]
            };
            for crest_offset_g in crest_offsets {
                let seed = HillSeed {
                    pull_g,
                    crest_g: case.target_g + crest_offset_g,
                    flank_degrees,
                    taper_radians: HILL_TAPER_ANGLE,
                };
                if !seeds.contains(&seed) {
                    seeds.push(seed);
                }
            }
        }
    }
    seeds
}

fn diagnostic_candidate_seeds(case: &CalibrationCase, model: &CoasterModel) -> Vec<HillSeed> {
    let mut seeds = Vec::new();
    if model.key == "hybrid" {
        seeds.reserve(HYBRID_DIAGNOSTIC_CANDIDATES);
        for pull_g in HYBRID_DIAGNOSTIC_PULL_G {
            for flank_degrees in HYBRID_DIAGNOSTIC_FLANK_DEGREES {
                for crest_offset_g in HYBRID_DIAGNOSTIC_CREST_OFFSETS_G {
                    for taper_radians in HYBRID_DIAGNOSTIC_TAPER_RAD {
                        seeds.push(HillSeed {
                            pull_g,
                            crest_g: case.target_g + crest_offset_g,
                            flank_degrees,
                            taper_radians,
                        });
                    }
                }
            }
        }
    } else if model.key == "hyper" {
        seeds.reserve(HYPER_DIAGNOSTIC_CANDIDATES);
        for pull_g in HYPER_DIAGNOSTIC_PULL_G {
            for flank_degrees in HYPER_DIAGNOSTIC_FLANK_DEGREES {
                for crest_offset_g in HYPER_DIAGNOSTIC_CREST_OFFSETS_G {
                    for taper_radians in HYPER_DIAGNOSTIC_TAPER_RAD {
                        seeds.push(HillSeed {
                            pull_g,
                            crest_g: case.target_g + crest_offset_g,
                            flank_degrees,
                            taper_radians,
                        });
                    }
                }
            }
        }
    } else {
        panic!("diagnostic grid only supports hybrid and hyper models");
    }
    seeds
}

fn measure_case(
    case: &CalibrationCase,
    base_model: &CoasterModel,
    seed: HillSeed,
) -> Result<HillCandidateMeasurement, SolveRefusal> {
    let model = model_with_train_length(base_model, case.cars);
    let prefix = lift_prefix(&model, f64::from(case.speed_mps));
    let entry = prefix.last().expect("prefix contains a station").exit;
    let physics = PhysicsConfig {
        lift_speed: f64::from(case.speed_mps),
        ..PhysicsConfig::default()
    };
    let mut preview_cache = PreviewCache::default();
    let prefix_preview = preview_cache
        .preview(&prefix, &model, physics)
        .expect("calibration prefix contains a station");
    assert_eq!(prefix_preview.status, PreviewStatus::ReachedEnd);
    let observed_speed = prefix_preview
        .end_speed_mps
        .expect("completed prefix preview exposes its exact endpoint speed");
    assert!(
        (observed_speed - f64::from(case.speed_mps)).abs() <= 0.01,
        "calibration prefix speed {observed_speed} differs from requested {}",
        case.speed_mps
    );
    let mut context = SolveContext {
        entry,
        entry_speed_mps: observed_speed,
        model: &model,
        train: &model.train,
        prefix: &prefix,
        preview_cache: &mut preview_cache,
        physics,
    };
    measure_hill_candidate_with_taper(
        &mut context,
        case.target_g,
        seed.pull_g,
        seed.crest_g,
        seed.flank_degrees.to_radians(),
        seed.taper_radians,
    )
}

fn model_with_train_length(base_model: &CoasterModel, cars: u8) -> CoasterModel {
    assert!(base_model.cars_per_train.contains(&cars));
    let mut model = base_model.clone();
    model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
    model.train.cars = cars;
    model
}

fn lift_prefix(model: &CoasterModel, speed_mps: f64) -> Vec<Piece> {
    let templates = catalog();
    let station = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .expect("catalog includes station");
    let straight = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let mut prefix = vec![
        station
            .at(TrackState::origin())
            .expect("station starts at the origin"),
    ];
    let physics = PhysicsConfig::default();
    let acceleration_distance_m = (speed_mps.powi(2) - physics.station_speed.powi(2)).max(0.0)
        / (2.0 * physics.drive_acceleration);
    let lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
    let straight_count = (lift_length_m / straight.length()).ceil() as usize;
    for _ in 0..straight_count {
        let mut lift = straight
            .at(prefix.last().expect("prefix contains station").exit)
            .expect("lift straight connects");
        lift.kind = PieceKind::Lift;
        prefix.push(lift);
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::{
        HYBRID_DIAGNOSTIC_CANDIDATES, HYPER_DIAGNOSTIC_CANDIDATES, coefficient_candidate_seeds,
        coefficient_cases, default_feel_targets, diagnostic_candidate_seeds, quantize_target,
        representative_cases,
    };
    use crate::track::coaster_models;

    #[test]
    fn coefficient_grid_covers_safe_range_and_default_feels() {
        let models = coaster_models();
        let cases = coefficient_cases();
        for (model_index, model) in models.iter().enumerate() {
            let model_cases: Vec<_> = cases
                .iter()
                .filter(|case| case.model_index == model_index)
                .collect();
            assert!(
                !model_cases.is_empty(),
                "{} has coefficient cases",
                model.key
            );
            let targets: Vec<_> = model_cases.iter().map(|case| case.target_g).collect();
            for target in default_feel_targets(model) {
                if target >= model.comfortable_force_limits.minimum_vertical_g && target <= 0.0 {
                    assert!(
                        targets.contains(&quantize_target(target)),
                        "{} coefficient grid omitted feel target {target}",
                        model.key
                    );
                }
            }
            let minimum_target = model.comfortable_force_limits.minimum_vertical_g + 0.15;
            assert!(
                targets
                    .iter()
                    .any(|target| (*target - minimum_target).abs() <= 0.025),
                "{} coefficient grid omits lower feasible target boundary",
                model.key
            );
            assert!(
                targets.contains(&0.0),
                "{} coefficient grid omits 0 g",
                model.key
            );
        }
        assert!(cases.iter().all(|case| (14..=36).contains(&case.speed_mps)));
    }

    #[test]
    fn candidate_grid_is_deterministic_and_keeps_all_seeds_inside_domain() {
        let models = coaster_models();
        let case = coefficient_cases()
            .into_iter()
            .next()
            .expect("grid has cases");
        let model = &models[case.model_index];
        let seeds = coefficient_candidate_seeds(&case, &model);
        assert_eq!(seeds, coefficient_candidate_seeds(&case, &model));
        assert!(!seeds.is_empty());
        assert!(seeds.iter().all(|seed| seed.pull_g >= 1.25));
        assert!(seeds.iter().all(|seed| seed.crest_g <= 0.2));
        assert!(
            seeds
                .iter()
                .all(|seed| (25.0..=55.0).contains(&seed.flank_degrees))
        );
        assert!(
            seeds
                .iter()
                .all(|seed| seed.taper_radians == super::HILL_TAPER_ANGLE)
        );
    }

    #[test]
    fn representative_grid_is_bounded_to_known_solver_failures() {
        let models = coaster_models();
        let cases = representative_cases(&models);
        assert_eq!(cases.len(), 2);
        let hybrid = cases[0];
        let hybrid_model = &models[hybrid.model_index];
        assert_eq!(hybrid_model.key, "hybrid");
        assert_eq!(hybrid.cars, 4);
        assert_eq!(hybrid.speed_mps, 14);
        assert!(
            (hybrid.target_g - (hybrid_model.comfortable_force_limits.minimum_vertical_g + 0.35))
                .abs()
                < 1e-9
        );
        let hyper = cases[1];
        let hyper_model = &models[hyper.model_index];
        assert_eq!(hyper_model.key, "hyper");
        assert_eq!(hyper.cars, hyper_model.train.cars);
        assert_eq!(hyper.speed_mps, 34);
        assert_eq!(hyper.target_g, -0.9);
        assert_eq!(
            diagnostic_candidate_seeds(&hybrid, hybrid_model).len(),
            HYBRID_DIAGNOSTIC_CANDIDATES
        );
        assert_eq!(
            diagnostic_candidate_seeds(&hyper, hyper_model).len(),
            HYPER_DIAGNOSTIC_CANDIDATES
        );
    }
}
