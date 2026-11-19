//! Offline generation and train-preview verification for shaped-element rows.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::physics::{
    CandidateCalibrationPreview, Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache,
    PreviewStatus, Train,
};
use crate::track::curves::{speed_turn_geometry, speed_turn_radius_for_peak_bank};
use crate::track::profile::{
    OfflineHillSpec, OfflineSolveError, OfflineTrain, integrate_hill_offline_once_profile,
    solve_hill_offline_profile,
};
use crate::track::shaped::{ElementSpec, SolveRefusal, piece_from_spec};
use crate::track::{
    CoasterModel, Hand, HillFeel, Piece, PieceId, PieceKind, TrackState, TrainConfiguration,
    catalog, coaster_models,
};

const MINIMUM_SPEED_HALF_MPS: u8 = 28;
const MAXIMUM_SPEED_HALF_MPS: u8 = 72;
const OFFLINE_STEPS: usize = 2048;
const RUNTIME_STEPS: usize = 512;
const MINIMUM_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_AIRTIME_ERROR_G: f64 = 0.1;
const MAXIMUM_TURN_LATERAL_G: f64 = 0.1;
const MINIMUM_AUTO_FLANK_DEGREES: f64 = 25.0;
const MAXIMUM_AUTO_FLANK_DEGREES: f64 = 35.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsVariant {
    RuntimePreview,
    ConstructionTest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridElement {
    Hill { feel: HillFeel, strength_g: f64 },
    FullyBankedTurn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeneratedSpec {
    Hill {
        target_g: f64,
        pull_g: f64,
        crest_g: f64,
        flank_angle_radians: f64,
        height_m: f64,
        span_m: f64,
        minimum_radius_m: f64,
        connector_insert_m: f64,
        fixed_point_iterations: u8,
    },
    Turn {
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellRefusal {
    BelowFirstVerifiedSpeed { minimum_speed_mps: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellFailure {
    OfflineSolver(OfflineSolveError),
    PieceConstruction(SolveRefusal),
    TurnRadiusOutOfRange {
        radius_m: f64,
    },
    PreviewUnavailable,
    PreviewDidNotReachEnd(PreviewStatus),
    WrongCarCount {
        expected: u8,
        observed: usize,
    },
    MissingSustainedAirtime,
    AirtimeIntentMiss {
        target_g: f64,
        observed_g: f64,
    },
    ForceHeadroomMiss {
        required_g: f64,
        observed_g: f64,
    },
    TurnLateralMiss {
        maximum_g: f64,
        observed_g: f64,
    },
    TurnBankMiss {
        maximum_degrees: f64,
        observed_degrees: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum GridCellOutcome {
    Solved {
        spec: GeneratedSpec,
        measurements: Vec<ScenarioMeasurement>,
    },
    Refused(CellRefusal),
    Unresolved {
        failure: CellFailure,
        measurements: Vec<ScenarioMeasurement>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedGridCell {
    pub model_key: &'static str,
    pub cars: u8,
    pub element: GridElement,
    pub entry_speed_mps: f64,
    pub outcome: GridCellOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneratedTableCell {
    pub model_key: &'static str,
    pub cars: u8,
    pub element: GridElement,
    pub entry_speed_mps: f64,
    pub outcome: GeneratedTableOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeneratedTableOutcome {
    Solved(GeneratedSpec),
    Refused(CellRefusal),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenarioMeasurement {
    pub entry_speed_mps: f64,
    pub physics: PhysicsVariant,
    pub status: Option<PreviewStatus>,
    pub car_count: usize,
    pub worst_sustained_minimum_vertical_g: Option<f64>,
    pub minimum_force_headroom_g: Option<f64>,
    pub maximum_lateral_g: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationReport {
    pub cells: Vec<GeneratedGridCell>,
    pub elapsed: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateSearchAttempt {
    pub spec: Option<GeneratedSpec>,
    pub failure: Option<CellFailure>,
    pub measurements: Vec<ScenarioMeasurement>,
}

impl GenerationReport {
    pub fn unresolved_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| matches!(cell.outcome, GridCellOutcome::Unresolved { .. }))
            .count()
    }

    pub fn solved_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| matches!(cell.outcome, GridCellOutcome::Solved { .. }))
            .count()
    }

    pub fn refused_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| matches!(cell.outcome, GridCellOutcome::Refused(_)))
            .count()
    }

    pub fn unresolved_diagnostics(&self) -> Vec<String> {
        self.cells
            .iter()
            .filter_map(|cell| match &cell.outcome {
                GridCellOutcome::Unresolved {
                    failure,
                    measurements,
                } => {
                    let mut line = format!(
                        "{} {} cars {:?} at {:.1} m/s: {failure:?}",
                        cell.model_key, cell.cars, cell.element, cell.entry_speed_mps
                    );
                    if !measurements.is_empty() {
                        let _ = write!(line, " measurements={measurements:?}");
                    }
                    Some(line)
                }
                _ => None,
            })
            .collect()
    }
}

/// Generate and verify every configured hill feel and fully banked turn.
///
/// This diagnostic evaluates one schedule per cell. Failed cells remain unresolved
/// because failure of one schedule does not establish infeasibility.
pub fn generate_grid() -> GenerationReport {
    generate_grid_for_models(&coaster_models())
}

pub fn generate_grid_for_models(models: &[CoasterModel]) -> GenerationReport {
    let started = Instant::now();
    let mut cells = Vec::new();
    for base_model in models {
        for cars in base_model.cars_per_train.clone() {
            let model = model_for_train_length(base_model, cars);
            let mut elements = hill_elements(&model);
            elements.push(GridElement::FullyBankedTurn);
            for element in elements {
                for speed_half_mps in MINIMUM_SPEED_HALF_MPS..=MAXIMUM_SPEED_HALF_MPS {
                    let entry_speed_mps = f64::from(speed_half_mps) / 2.0;
                    cells.push(generate_cell(&model, element, entry_speed_mps));
                }
            }
        }
    }
    GenerationReport {
        cells,
        elapsed: started.elapsed(),
    }
}

pub fn render_generated_table(report: &GenerationReport) -> Result<String, Vec<String>> {
    let unresolved = report.unresolved_diagnostics();
    if !unresolved.is_empty() {
        return Err(unresolved);
    }

    let mut source = String::from(
        "// Generated by `make generate-shaped-elements`. Do not edit by hand.\n\
         pub const GENERATED_SHAPED_ELEMENT_GRID: &[GeneratedTableCell] = &[\n",
    );
    for cell in &report.cells {
        write!(
            source,
            "    GeneratedTableCell {{ model_key: {:?}, cars: {}, element: ",
            cell.model_key, cell.cars
        )
        .unwrap();
        match cell.element {
            GridElement::Hill { feel, strength_g } => {
                write!(source, "GridElement::Hill {{ feel: HillFeel::{feel:?}, strength_g: {strength_g:.17} }}, ").unwrap();
            }
            GridElement::FullyBankedTurn => source.push_str("GridElement::FullyBankedTurn, "),
        }
        write!(
            source,
            "entry_speed_mps: {:.1}, outcome: ",
            cell.entry_speed_mps
        )
        .unwrap();
        match &cell.outcome {
            GridCellOutcome::Solved { spec, .. } => {
                source.push_str("GeneratedTableOutcome::Solved(");
                write_spec(&mut source, *spec);
                source.push_str("),\n");
            }
            GridCellOutcome::Refused(CellRefusal::BelowFirstVerifiedSpeed {
                minimum_speed_mps,
            }) => {
                writeln!(
                    source,
                    "GeneratedTableOutcome::Refused(CellRefusal::BelowFirstVerifiedSpeed {{ minimum_speed_mps: {minimum_speed_mps:.1} }}),"
                )
                .unwrap();
            }
            GridCellOutcome::Unresolved { .. } => {
                unreachable!("unresolved rows were rejected above")
            }
        }
        source.push_str("    },\n");
    }
    source.push_str("];\n");
    Ok(source)
}

fn write_spec(source: &mut String, spec: GeneratedSpec) {
    match spec {
        GeneratedSpec::Hill {
            target_g,
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
        } => {
            write!(
                source,
                "GeneratedSpec::Hill {{ target_g: {target_g:.17}, pull_g: {pull_g:.17}, crest_g: {crest_g:.17}, flank_angle_radians: {flank_angle_radians:.17}, height_m: {height_m:.17}, span_m: {span_m:.17}, minimum_radius_m: {minimum_radius_m:.17}, connector_insert_m: {connector_insert_m:.17}, fixed_point_iterations: {fixed_point_iterations} }}"
            )
            .unwrap();
        }
        GeneratedSpec::Turn {
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
        } => {
            write!(
                source,
                "GeneratedSpec::Turn {{ vertical_g: {vertical_g:.17}, lateral_g: {lateral_g:.17}, radius_m: {radius_m:.17}, bank_degrees: {bank_degrees:.17} }}"
            )
            .unwrap();
        }
    }
}

pub fn default_output_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/track/shaped/generated_grid.rs"
    )
}

pub fn generate_cell_for_model(
    base_model: &CoasterModel,
    cars: u8,
    element: GridElement,
    entry_speed_mps: f64,
) -> GeneratedGridCell {
    assert!(base_model.cars_per_train.contains(&cars));
    assert!(entry_speed_mps.is_finite() && (14.0..=36.0).contains(&entry_speed_mps));
    let model = model_for_train_length(base_model, cars);
    generate_cell(&model, element, entry_speed_mps)
}

/// Run a bounded offline schedule search for one train, feel, and speed.
///
/// The search changes only the offline force schedule. Runtime still consumes the
/// selected parameters and performs a single integration pass.
pub fn search_hill_candidates_for_model(
    base_model: &CoasterModel,
    cars: u8,
    feel: HillFeel,
    entry_speed_mps: f64,
) -> Vec<CandidateSearchAttempt> {
    assert!(base_model.cars_per_train.contains(&cars));
    assert!(entry_speed_mps.is_finite() && (14.0..=36.0).contains(&entry_speed_mps));
    let model = model_for_train_length(base_model, cars);
    let target_g = hill_strength(&model, feel);
    let rules = model.shaped_element_rules();
    let speed_fraction = ((entry_speed_mps - 14.0) / 22.0).clamp(0.0, 1.0);
    let initial_flank_degrees = MINIMUM_AUTO_FLANK_DEGREES
        + speed_fraction * (MAXIMUM_AUTO_FLANK_DEGREES - MINIMUM_AUTO_FLANK_DEGREES);
    let mut schedules = vec![(rules.hill_pull_g, target_g, initial_flank_degrees)];
    for pull_g in [1.5, 2.0, 2.5] {
        if pull_g > 1.0 && pull_g <= model.comfortable_force_limits.maximum_vertical_g - 0.25 {
            for flank_degrees in [30.0, 34.0, 38.0] {
                for crest_offset_g in [0.0, -0.05, 0.05, -0.1, 0.1] {
                    let schedule = (pull_g, target_g + crest_offset_g, flank_degrees);
                    if !schedules.contains(&schedule) {
                        schedules.push(schedule);
                    }
                }
            }
        }
    }

    let mut attempts = Vec::new();
    for (pull_g, crest_g, flank_degrees) in schedules {
        let candidate = generate_hill_candidate_with_parameters(
            &model,
            target_g,
            pull_g,
            crest_g,
            flank_degrees.to_radians(),
            entry_speed_mps,
        );
        let (spec, piece) = match candidate {
            Ok(candidate) => candidate,
            Err(failure) => {
                attempts.push(CandidateSearchAttempt {
                    spec: None,
                    failure: Some(failure),
                    measurements: Vec::new(),
                });
                continue;
            }
        };
        let measurements = verify_candidate(&model, &piece, entry_speed_mps);
        let failure = measurements.iter().find_map(|measurement| {
            measurement_failure(
                &model,
                GridElement::Hill {
                    feel,
                    strength_g: target_g,
                },
                entry_speed_mps,
                measurement,
            )
        });
        let passed = failure.is_none();
        attempts.push(CandidateSearchAttempt {
            spec: Some(spec),
            failure,
            measurements,
        });
        if passed {
            break;
        }
    }
    attempts
}

fn hill_elements(model: &CoasterModel) -> Vec<GridElement> {
    vec![
        GridElement::Hill {
            feel: HillFeel::Floater,
            strength_g: hill_strength(model, HillFeel::Floater),
        },
        GridElement::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: hill_strength(model, HillFeel::StrongFloater),
        },
        GridElement::Hill {
            feel: HillFeel::Ejector,
            strength_g: hill_strength(model, HillFeel::Ejector),
        },
    ]
}

fn hill_strength(model: &CoasterModel, feel: HillFeel) -> f64 {
    let rules = model.shaped_element_rules();
    match feel {
        HillFeel::Floater => rules.default_floater_g,
        HillFeel::StrongFloater => rules.default_strong_floater_g,
        HillFeel::Ejector if model.track_material == crate::track::Material::Wood => {
            rules.wooden_ejector_g
        }
        HillFeel::Ejector => rules.steel_ejector_g,
        _ => unreachable!("generator only handles the three planned airtime feels"),
    }
}

fn generate_cell(model: &CoasterModel, element: GridElement, speed_mps: f64) -> GeneratedGridCell {
    let candidate = match element {
        GridElement::Hill {
            feel: _,
            strength_g,
        } => generate_hill_candidate(model, strength_g, speed_mps),
        GridElement::FullyBankedTurn => generate_turn_candidate(model, speed_mps),
    };
    let (spec, piece) = match candidate {
        Ok(candidate) => candidate,
        Err(failure) => {
            return GeneratedGridCell {
                model_key: model.key,
                cars: model.train.cars,
                element,
                entry_speed_mps: speed_mps,
                outcome: GridCellOutcome::Unresolved {
                    failure,
                    measurements: Vec::new(),
                },
            };
        }
    };
    if let GeneratedSpec::Turn { bank_degrees, .. } = spec
        && bank_degrees > model.shaped_element_rules().maximum_bank_degrees
    {
        return GeneratedGridCell {
            model_key: model.key,
            cars: model.train.cars,
            element,
            entry_speed_mps: speed_mps,
            outcome: GridCellOutcome::Unresolved {
                failure: CellFailure::TurnBankMiss {
                    maximum_degrees: model.shaped_element_rules().maximum_bank_degrees,
                    observed_degrees: bank_degrees,
                },
                measurements: Vec::new(),
            },
        };
    }
    let measurements = verify_candidate(model, &piece, speed_mps);
    let failure = measurements
        .iter()
        .find_map(|measurement| measurement_failure(model, element, speed_mps, measurement));
    GeneratedGridCell {
        model_key: model.key,
        cars: model.train.cars,
        element,
        entry_speed_mps: speed_mps,
        outcome: match failure {
            Some(failure) => GridCellOutcome::Unresolved {
                failure,
                measurements,
            },
            None => GridCellOutcome::Solved { spec, measurements },
        },
    }
}

fn generate_hill_candidate(
    model: &CoasterModel,
    strength_g: f64,
    speed_mps: f64,
) -> Result<(GeneratedSpec, Piece), CellFailure> {
    let rules = model.shaped_element_rules();
    let speed_fraction = ((speed_mps - 14.0) / (36.0 - 14.0)).clamp(0.0, 1.0);
    let flank_angle_radians = (MINIMUM_AUTO_FLANK_DEGREES
        + speed_fraction * (MAXIMUM_AUTO_FLANK_DEGREES - MINIMUM_AUTO_FLANK_DEGREES))
        .to_radians();
    generate_hill_candidate_with_parameters(
        model,
        strength_g,
        rules.hill_pull_g,
        strength_g,
        flank_angle_radians,
        speed_mps,
    )
}

fn generate_hill_candidate_with_parameters(
    model: &CoasterModel,
    target_g: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle_radians: f64,
    speed_mps: f64,
) -> Result<(GeneratedSpec, Piece), CellFailure> {
    let offline_spec = OfflineHillSpec {
        entry_speed_mps: speed_mps,
        pull_g,
        crest_g,
        flank_angle_radians,
        taper_angle_radians: None,
    };
    let offline_train = OfflineTrain::Coaster {
        cars: model.train.cars,
        length_metres: model.train.length_metres,
    };
    let (_, fixed_point_iterations, _) =
        solve_hill_offline_profile(offline_spec, offline_train, OFFLINE_STEPS)
            .map_err(CellFailure::OfflineSolver)?;
    let (profile, _, connector_insert_m) =
        integrate_hill_offline_once_profile(offline_spec, offline_train, RUNTIME_STEPS)
            .map_err(CellFailure::OfflineSolver)?;
    let height_m = profile
        .resolved_points()
        .iter()
        .map(|point| point[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_m = profile.endpoint()[0];
    let minimum_radius_m = profile.minimum_radius();
    let spec = ElementSpec::Hill {
        design_speed_mps: speed_mps,
        cars: model.train.cars,
        target_g,
        delivered_g: target_g,
        height_m,
        span_m,
        profile: profile.clone(),
    };
    let piece =
        piece_from_spec(TrackState::origin(), &spec).map_err(CellFailure::PieceConstruction)?;
    Ok((
        GeneratedSpec::Hill {
            target_g,
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
        },
        piece,
    ))
}

fn generate_turn_candidate(
    model: &CoasterModel,
    speed_mps: f64,
) -> Result<(GeneratedSpec, Piece), CellFailure> {
    let rules = model.shaped_element_rules();
    let target_vertical_g = rules.turn_vertical_g;
    let target_bank = (1.0 / target_vertical_g).acos();
    let ideal_radius = speed_turn_radius_for_peak_bank(target_bank, speed_mps);
    let radius_m = (ideal_radius.max(rules.minimum_turn_radius_m) / 2.0).ceil() * 2.0;
    if radius_m > rules.maximum_turn_radius_m {
        return Err(CellFailure::TurnRadiusOutOfRange { radius_m });
    }
    let geometry = (*speed_turn_geometry(radius_m, speed_mps)).clone();
    let bank_degrees = geometry.peak_bank.to_degrees();
    let spec = ElementSpec::Turn {
        design_speed_mps: speed_mps,
        cars: model.train.cars,
        hand: Hand::Left,
        vertical_g: 1.0 / geometry.peak_bank.cos(),
        lateral_g: 0.0,
        radius_m,
        bank_degrees,
        geometry,
    };
    let piece =
        piece_from_spec(TrackState::origin(), &spec).map_err(CellFailure::PieceConstruction)?;
    Ok((
        GeneratedSpec::Turn {
            vertical_g: match &spec {
                ElementSpec::Turn { vertical_g, .. } => *vertical_g,
                ElementSpec::Hill { .. } => unreachable!(),
            },
            lateral_g: 0.0,
            radius_m,
            bank_degrees,
        },
        piece,
    ))
}

fn verify_candidate(
    model: &CoasterModel,
    piece: &Piece,
    design_speed_mps: f64,
) -> Vec<ScenarioMeasurement> {
    let mut measurements = Vec::with_capacity(6);
    for physics in [
        PhysicsVariant::RuntimePreview,
        PhysicsVariant::ConstructionTest,
    ] {
        for speed_offset_mps in [-0.5, 0.0, 0.5] {
            let entry_speed_mps = design_speed_mps + speed_offset_mps;
            let measurement = preview_candidate(model, piece, entry_speed_mps, physics);
            measurements.push(measurement);
        }
    }
    measurements
}

fn preview_candidate(
    model: &CoasterModel,
    piece: &Piece,
    entry_speed_mps: f64,
    physics_variant: PhysicsVariant,
) -> ScenarioMeasurement {
    let physics = match physics_variant {
        PhysicsVariant::RuntimePreview => PhysicsConfig {
            lift_speed: entry_speed_mps,
            ..PhysicsConfig::default()
        },
        PhysicsVariant::ConstructionTest => PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
    };
    let prefix = lift_prefix(model, entry_speed_mps, PhysicsConfig::default());
    let candidate_piece_index = prefix.len();
    let candidate_entry = prefix.last().expect("lift prefix contains a station").exit;
    let candidate_piece = piece
        .at(candidate_entry)
        .expect("candidate supports the level lift connector");
    let candidate_start_m = prefix.iter().map(Piece::length).sum::<f64>();
    let mut pieces = prefix.clone();
    pieces.push(candidate_piece.clone());
    pieces.extend(runout_from(&candidate_piece, &model.train));
    if physics_variant == PhysicsVariant::RuntimePreview {
        let mut preview_cache = PreviewCache::default();
        let prefix_preview = preview_cache
            .preview(&prefix, model, physics)
            .expect("lift prefix includes the station");
        if prefix_preview.status != PreviewStatus::ReachedEnd {
            return ScenarioMeasurement {
                entry_speed_mps,
                physics: physics_variant,
                status: Some(prefix_preview.status),
                car_count: 0,
                worst_sustained_minimum_vertical_g: None,
                minimum_force_headroom_g: None,
                maximum_lateral_g: None,
            };
        }
        let observed_speed = prefix_preview
            .end_speed_mps
            .expect("open lift prefix reaches its end");
        assert!(
            (observed_speed - entry_speed_mps).abs() <= 0.01,
            "prefix speed {observed_speed} differs from requested {entry_speed_mps}"
        );
        let preview =
            preview_cache.preview_candidate(&pieces, model, physics, candidate_piece_index);
        return match preview {
            Some(preview) => {
                measurement_from_candidate_preview(model, entry_speed_mps, physics_variant, preview)
            }
            None => ScenarioMeasurement {
                entry_speed_mps,
                physics: physics_variant,
                status: None,
                car_count: 0,
                worst_sustained_minimum_vertical_g: None,
                minimum_force_headroom_g: None,
                maximum_lateral_g: None,
            },
        };
    }
    let circuit = Circuit::open(pieces).expect("generated candidate circuit is connected");
    let candidate_end_m = candidate_start_m + candidate_piece.length();
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
    let clear_candidate_distance_m = candidate_end_m + model.train.length_metres;
    for _ in 0..100_000 {
        train.step(&circuit);
        recorder.record(&train.take_readings(), model.name);
        if !train.notes().is_empty() {
            return ScenarioMeasurement {
                entry_speed_mps,
                physics: physics_variant,
                status: Some(PreviewStatus::Stalled),
                car_count: 0,
                worst_sustained_minimum_vertical_g: None,
                minimum_force_headroom_g: None,
                maximum_lateral_g: None,
            };
        }
        if train.distance() >= clear_candidate_distance_m {
            let report = recorder.finish(false, false);
            let force_peaks = report
                .car_piece_force_peaks()
                .into_iter()
                .filter(|peaks| peaks.piece_index == candidate_piece_index)
                .collect::<Vec<_>>();
            let sustained_peaks = report
                .car_piece_sustained_force_peaks()
                .into_iter()
                .filter(|peaks| peaks.piece_index == candidate_piece_index)
                .collect::<Vec<_>>();
            return ScenarioMeasurement {
                entry_speed_mps,
                physics: physics_variant,
                status: Some(PreviewStatus::ReachedEnd),
                car_count: force_peaks.len(),
                worst_sustained_minimum_vertical_g: sustained_peaks
                    .iter()
                    .filter_map(|peaks| peaks.peaks.minimum_vertical_g)
                    .reduce(f64::min),
                minimum_force_headroom_g: force_peaks
                    .iter()
                    .map(|peaks| force_headroom(peaks.peaks, model.comfortable_force_limits))
                    .reduce(f64::min),
                maximum_lateral_g: force_peaks
                    .iter()
                    .map(|peaks| peaks.peaks.maximum_lateral_g)
                    .reduce(f64::max),
            };
        }
    }
    ScenarioMeasurement {
        entry_speed_mps,
        physics: physics_variant,
        status: Some(PreviewStatus::TimedOut),
        car_count: 0,
        worst_sustained_minimum_vertical_g: None,
        minimum_force_headroom_g: None,
        maximum_lateral_g: None,
    }
}

fn measurement_from_candidate_preview(
    model: &CoasterModel,
    entry_speed_mps: f64,
    physics: PhysicsVariant,
    preview: CandidateCalibrationPreview,
) -> ScenarioMeasurement {
    let force_headroom = preview
        .cars
        .iter()
        .map(|car| force_headroom(car.peaks, model.comfortable_force_limits))
        .reduce(f64::min);
    let maximum_lateral_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.maximum_lateral_g)
        .reduce(f64::max);
    ScenarioMeasurement {
        entry_speed_mps,
        physics,
        status: Some(preview.status),
        car_count: preview.cars.len(),
        worst_sustained_minimum_vertical_g: preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min),
        minimum_force_headroom_g: force_headroom,
        maximum_lateral_g,
    }
}

fn measurement_failure(
    model: &CoasterModel,
    element: GridElement,
    design_speed_mps: f64,
    measurement: &ScenarioMeasurement,
) -> Option<CellFailure> {
    let Some(status) = measurement.status else {
        return Some(CellFailure::PreviewUnavailable);
    };
    if status != PreviewStatus::ReachedEnd {
        return Some(CellFailure::PreviewDidNotReachEnd(status));
    }
    if measurement.car_count != usize::from(model.train.cars) {
        return Some(CellFailure::WrongCarCount {
            expected: model.train.cars,
            observed: measurement.car_count,
        });
    }
    let Some(headroom) = measurement.minimum_force_headroom_g else {
        return Some(CellFailure::PreviewUnavailable);
    };
    if headroom < MINIMUM_FORCE_HEADROOM_G {
        return Some(CellFailure::ForceHeadroomMiss {
            required_g: MINIMUM_FORCE_HEADROOM_G,
            observed_g: headroom,
        });
    }
    if (measurement.entry_speed_mps - design_speed_mps).abs() < 0.01 {
        match element {
            GridElement::Hill { strength_g, .. } => {
                let Some(observed_g) = measurement.worst_sustained_minimum_vertical_g else {
                    return Some(CellFailure::MissingSustainedAirtime);
                };
                if (observed_g - strength_g).abs() > MAXIMUM_AIRTIME_ERROR_G {
                    return Some(CellFailure::AirtimeIntentMiss {
                        target_g: strength_g,
                        observed_g,
                    });
                }
            }
            GridElement::FullyBankedTurn => {
                let Some(observed_g) = measurement.maximum_lateral_g else {
                    return Some(CellFailure::PreviewUnavailable);
                };
                if observed_g > MAXIMUM_TURN_LATERAL_G {
                    return Some(CellFailure::TurnLateralMiss {
                        maximum_g: MAXIMUM_TURN_LATERAL_G,
                        observed_g,
                    });
                }
            }
        }
    }
    None
}

fn force_headroom(
    peaks: crate::physics::ForcePeakValues,
    limits: crate::track::ComfortableForceLimits,
) -> f64 {
    (peaks.minimum_vertical_g - limits.minimum_vertical_g)
        .min(limits.maximum_vertical_g - peaks.maximum_vertical_g)
        .min(limits.maximum_lateral_g - peaks.maximum_lateral_g)
        .min(limits.maximum_longitudinal_g - peaks.maximum_longitudinal_g)
}

fn model_for_train_length(base_model: &CoasterModel, cars: u8) -> CoasterModel {
    assert!(base_model.cars_per_train.contains(&cars));
    let mut model = base_model.clone();
    model.train = train_configuration_for_cars(&base_model.train, cars);
    model
}

fn train_configuration_for_cars(base_train: &TrainConfiguration, cars: u8) -> TrainConfiguration {
    let mut train = base_train.clone();
    train.length_metres *= f64::from(cars) / f64::from(train.cars);
    train.cars = cars;
    train
}

fn lift_prefix(model: &CoasterModel, speed_mps: f64, physics: PhysicsConfig) -> Vec<Piece> {
    let templates = catalog();
    let station = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .expect("catalog includes station");
    let straight = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight");
    let mut pieces = vec![
        station
            .at(TrackState::origin())
            .expect("station starts at origin"),
    ];
    let acceleration_distance_m = (speed_mps.powi(2) - physics.station_speed.powi(2)).max(0.0)
        / (2.0 * physics.drive_acceleration);
    let required_lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
    let straight_count = (required_lift_length_m / straight.length()).ceil() as usize;
    for _ in 0..straight_count {
        let mut lift = straight
            .at(pieces.last().expect("prefix contains station").exit)
            .expect("lift pieces connect");
        lift.kind = PieceKind::Lift;
        pieces.push(lift);
    }
    pieces
}

fn runout_from(piece: &Piece, train: &TrainConfiguration) -> Vec<Piece> {
    let straight = catalog()
        .iter()
        .find(|candidate| candidate.id == PieceId::Straight)
        .expect("catalog includes straight");
    let straight_count = (train.length_metres / straight.length()).ceil() as usize + 2;
    let mut runout = Vec::with_capacity(straight_count);
    let mut state = piece.exit;
    for _ in 0..straight_count {
        let piece = straight.at(state).expect("runout pieces connect");
        state = piece.exit;
        runout.push(piece);
    }
    runout
}

pub fn report_summary(report: &GenerationReport) -> String {
    format!(
        "grid rows: {}; solved: {}; typed refusals: {}; unresolved: {}; generation time: {:.3}s",
        report.cells.len(),
        report.solved_count(),
        report.refused_count(),
        report.unresolved_count(),
        report.elapsed.as_secs_f64()
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CellRefusal, GeneratedGridCell, GeneratedSpec, GridCellOutcome, GridElement,
        MAXIMUM_SPEED_HALF_MPS, MINIMUM_SPEED_HALF_MPS, model_for_train_length,
        render_generated_table,
    };
    use crate::track::{HillFeel, coaster_models};

    #[test]
    fn speed_grid_is_half_metres_per_second_from_14_through_36() {
        assert_eq!(MINIMUM_SPEED_HALF_MPS, 28);
        assert_eq!(MAXIMUM_SPEED_HALF_MPS, 72);
        assert_eq!(
            usize::from(MAXIMUM_SPEED_HALF_MPS - MINIMUM_SPEED_HALF_MPS) + 1,
            45
        );
    }

    #[test]
    fn train_configuration_uses_the_model_car_spacing() {
        let hyper = coaster_models()
            .into_iter()
            .find(|model| model.key == "hyper")
            .expect("hyper model exists");
        let one_car_length = hyper.train.length_metres / f64::from(hyper.train.cars);
        let four_cars = model_for_train_length(&hyper, 4);
        assert_eq!(four_cars.train.cars, 4);
        assert!((four_cars.train.length_metres - 4.0 * one_car_length).abs() < 1e-12);
    }

    #[test]
    fn table_rendering_is_deterministic_and_preserves_explicit_typed_refusals() {
        let report = super::GenerationReport {
            cells: vec![
                GeneratedGridCell {
                    model_key: "hyper",
                    cars: 8,
                    element: GridElement::Hill {
                        feel: HillFeel::Floater,
                        strength_g: -0.5,
                    },
                    entry_speed_mps: 14.0,
                    outcome: GridCellOutcome::Refused(CellRefusal::BelowFirstVerifiedSpeed {
                        minimum_speed_mps: 15.0,
                    }),
                },
                GeneratedGridCell {
                    model_key: "hyper",
                    cars: 8,
                    element: GridElement::FullyBankedTurn,
                    entry_speed_mps: 15.0,
                    outcome: GridCellOutcome::Solved {
                        spec: GeneratedSpec::Turn {
                            vertical_g: 2.0,
                            lateral_g: 0.0,
                            radius_m: 16.0,
                            bank_degrees: 60.0,
                        },
                        measurements: Vec::new(),
                    },
                },
            ],
            elapsed: std::time::Duration::ZERO,
        };
        let first = render_generated_table(&report).expect("complete grid renders");
        let second = render_generated_table(&report).expect("same grid renders");
        assert_eq!(first, second);
        assert!(first.contains("BelowFirstVerifiedSpeed"));
        assert!(first.contains("GeneratedSpec::Turn"));
    }

    #[test]
    fn table_rendering_rejects_unresolved_candidate_failures() {
        let report = super::GenerationReport {
            cells: vec![GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -0.5,
                },
                entry_speed_mps: 14.0,
                outcome: GridCellOutcome::Unresolved {
                    failure: super::CellFailure::MissingSustainedAirtime,
                    measurements: Vec::new(),
                },
            }],
            elapsed: std::time::Duration::ZERO,
        };
        assert!(render_generated_table(&report).is_err());
    }
}
