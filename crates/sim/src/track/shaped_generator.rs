//! Offline generation and train-preview verification for shaped-element rows.

mod intent_feasibility;

#[cfg(test)]
#[path = "shaped_generator/intent_feasibility_tests.rs"]
mod intent_feasibility_tests;

#[cfg(test)]
#[path = "shaped_generator/intent_headroom_guard_tests.rs"]
mod intent_headroom_guard_tests;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::physics::{
    CandidateCalibrationPreview, Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache,
    PreviewStatus, Train,
};
use crate::track::curves::{speed_turn_geometry, speed_turn_radius_for_peak_bank};
use crate::track::profile::{
    OfflineHillSpec, OfflineSolveError, OfflineTrain, SHAPED_HILL_TAPER_ANGLE, SolvedProfile,
    integrate_shaped_hill_once_profile, solve_hill_offline_profile,
};
use crate::track::shaped::{ElementSpec, SolveRefusal, piece_from_spec, synthesize_hill_profile};
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
const MAXIMUM_HILL_DELIVERY_CALIBRATION_ATTEMPTS: usize = 2;

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
        requested_target_g: f64,
        target_g: f64,
        delivered_g: f64,
        pull_g: f64,
        crest_g: f64,
        flank_angle_radians: f64,
        height_m: f64,
        span_m: f64,
        minimum_radius_m: f64,
        connector_insert_m: f64,
        fixed_point_iterations: u8,
        measurement_attempts: u8,
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
    BelowFirstVerifiedSpeed {
        minimum_speed_mps: f64,
    },
    AboveLastVerifiedSpeed {
        maximum_speed_mps: f64,
    },
    InfeasibleTarget {
        target_g: f64,
        delivered_g: Option<f64>,
        available_headroom_g: Option<f64>,
        failure: CellFailure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellFailure {
    OfflineSolver(OfflineSolveError),
    PieceConstruction(SolveRefusal),
    TurnRadiusOutOfRange {
        radius_m: f64,
    },
    MinimumRadiusMiss {
        minimum_m: f64,
        observed_m: f64,
    },
    IntentHeadroomConflict {
        required_minimum_vertical_g: f64,
        maximum_intent_vertical_g: f64,
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
    Infeasible {
        failure: CellFailure,
        best_spec: GeneratedSpec,
        attempts: u8,
        best_measurements: Vec<ScenarioMeasurement>,
    },
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

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedTableCell {
    pub model_key: &'static str,
    pub cars: u8,
    pub element: GridElement,
    pub entry_speed_mps: f64,
    pub outcome: GeneratedTableOutcome,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeneratedTableOutcome {
    Solved(GeneratedSpec),
    Refused(CellRefusal),
    Infeasible {
        failure: CellFailure,
        best_spec: GeneratedSpec,
        attempts: u8,
        best_measurements: &'static [ScenarioMeasurement],
    },
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

    pub fn infeasible_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| matches!(cell.outcome, GridCellOutcome::Infeasible { .. }))
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
            let targets = hill_strength_targets(&model);
            for speed_half_mps in MINIMUM_SPEED_HALF_MPS..=MAXIMUM_SPEED_HALF_MPS {
                let entry_speed_mps = f64::from(speed_half_mps) / 2.0;
                let mut measured_hills = Vec::with_capacity(targets.len());
                for &strength_g in &targets {
                    measured_hills.push(generate_cell(
                        &model,
                        GridElement::Hill {
                            feel: HillFeel::Floater,
                            strength_g,
                        },
                        entry_speed_mps,
                    ));
                }
                cap_measured_hill_rows(&mut measured_hills, entry_speed_mps);
                for feel in [
                    HillFeel::Floater,
                    HillFeel::StrongFloater,
                    HillFeel::Ejector,
                ] {
                    for measured_hill in &measured_hills {
                        let GridElement::Hill { strength_g, .. } = measured_hill.element else {
                            unreachable!("cached hill row contains a turn")
                        };
                        cells.push(GeneratedGridCell {
                            model_key: measured_hill.model_key,
                            cars: measured_hill.cars,
                            element: GridElement::Hill { feel, strength_g },
                            entry_speed_mps,
                            outcome: measured_hill.outcome.clone(),
                        });
                    }
                }
                cells.push(generate_cell(
                    &model,
                    GridElement::FullyBankedTurn,
                    entry_speed_mps,
                ));
                let infeasible_strength_count = measured_hills
                    .iter()
                    .filter(|cell| matches!(cell.outcome, GridCellOutcome::Infeasible { .. }))
                    .count();
                eprintln!(
                    "shaped generation progress: model={} cars={} speed={entry_speed_mps:.1} m/s rows={} infeasible-strengths={} elapsed={:.1}s",
                    model.key,
                    cars,
                    cells.len(),
                    infeasible_strength_count,
                    started.elapsed().as_secs_f64(),
                );
                for row in &measured_hills {
                    let GridCellOutcome::Infeasible {
                        failure,
                        best_spec,
                        attempts,
                        best_measurements,
                    } = &row.outcome
                    else {
                        continue;
                    };
                    eprintln!(
                        "shaped generation infeasible: model={} cars={} speed={entry_speed_mps:.1} m/s element={:?} failure={failure:?} attempts={attempts} best_spec={best_spec:?} best_measurements={best_measurements:?}",
                        model.key, cars, row.element,
                    );
                }
            }
        }
    }
    cells.sort_by(compare_grid_cells);
    GenerationReport {
        cells,
        elapsed: started.elapsed(),
    }
}

fn compare_grid_cells(left: &GeneratedGridCell, right: &GeneratedGridCell) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let prefix = left
        .model_key
        .cmp(right.model_key)
        .then_with(|| left.cars.cmp(&right.cars));
    if prefix != Ordering::Equal {
        return prefix;
    }
    match (left.element, right.element) {
        (
            GridElement::Hill {
                feel: left_feel,
                strength_g: left_strength,
            },
            GridElement::Hill {
                feel: right_feel,
                strength_g: right_strength,
            },
        ) => hill_feel_order(left_feel)
            .cmp(&hill_feel_order(right_feel))
            .then_with(|| left_strength.total_cmp(&right_strength))
            .then_with(|| left.entry_speed_mps.total_cmp(&right.entry_speed_mps)),
        (GridElement::Hill { .. }, GridElement::FullyBankedTurn) => Ordering::Less,
        (GridElement::FullyBankedTurn, GridElement::Hill { .. }) => Ordering::Greater,
        (GridElement::FullyBankedTurn, GridElement::FullyBankedTurn) => {
            left.entry_speed_mps.total_cmp(&right.entry_speed_mps)
        }
    }
}

fn hill_feel_order(feel: HillFeel) -> u8 {
    match feel {
        HillFeel::Floater => 0,
        HillFeel::StrongFloater => 1,
        HillFeel::Ejector => 2,
        _ => unreachable!("the generated grid only includes supported hill feels"),
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
            GridCellOutcome::Refused(CellRefusal::AboveLastVerifiedSpeed { maximum_speed_mps }) => {
                writeln!(
                    source,
                    "GeneratedTableOutcome::Refused(CellRefusal::AboveLastVerifiedSpeed {{ maximum_speed_mps: {maximum_speed_mps:.1} }}),"
                )
                .unwrap();
            }
            GridCellOutcome::Refused(CellRefusal::InfeasibleTarget {
                target_g,
                delivered_g,
                available_headroom_g,
                failure,
            }) => {
                writeln!(
                    source,
                    "GeneratedTableOutcome::Refused(CellRefusal::InfeasibleTarget {{ target_g: {target_g:.17}, delivered_g: {}, available_headroom_g: {}, failure: {failure:?} }}),",
                    write_option_f64(*delivered_g),
                    write_option_f64(*available_headroom_g),
                )
                .unwrap();
            }
            GridCellOutcome::Infeasible {
                failure,
                best_spec,
                attempts,
                best_measurements,
            } => {
                write!(source, "GeneratedTableOutcome::Infeasible {{ failure: {failure:?}, attempts: {attempts}, best_spec: ").unwrap();
                write_spec(&mut source, *best_spec);
                source.push_str(", best_measurements: &[");
                for measurement in best_measurements {
                    write_measurement(&mut source, measurement);
                    source.push_str(", ");
                }
                source.push_str("] },\n");
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
            requested_target_g,
            target_g,
            delivered_g,
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
            measurement_attempts,
        } => {
            write!(
                source,
                "GeneratedSpec::Hill {{ requested_target_g: {requested_target_g:.1}, target_g: {target_g:.17}, delivered_g: {delivered_g:.17}, pull_g: {pull_g:.17}, crest_g: {crest_g:.17}, flank_angle_radians: {flank_angle_radians:.17}, height_m: {height_m:.17}, span_m: {span_m:.17}, minimum_radius_m: {minimum_radius_m:.17}, connector_insert_m: {connector_insert_m:.17}, fixed_point_iterations: {fixed_point_iterations}, measurement_attempts: {measurement_attempts} }}"
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

fn write_measurement(source: &mut String, measurement: &ScenarioMeasurement) {
    write!(
        source,
        "ScenarioMeasurement {{ entry_speed_mps: {:.3}, physics: PhysicsVariant::{:?}, status: {:?}, car_count: {}, worst_sustained_minimum_vertical_g: {}, minimum_force_headroom_g: {}, maximum_lateral_g: {} }}",
        measurement.entry_speed_mps,
        measurement.physics,
        measurement.status,
        measurement.car_count,
        write_option_f64(measurement.worst_sustained_minimum_vertical_g),
        write_option_f64(measurement.minimum_force_headroom_g),
        write_option_f64(measurement.maximum_lateral_g),
    )
    .unwrap();
}

fn write_option_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "None".to_owned(), |value| format!("Some({value:.17})"))
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
    match element {
        GridElement::FullyBankedTurn => generate_cell(&model, element, entry_speed_mps),
        GridElement::Hill { feel, strength_g } => {
            let mut targets = hill_strength_targets(&model);
            if !targets.contains(&strength_g) {
                targets.push(strength_g);
                targets.sort_by(f64::total_cmp);
            }
            let mut rows = targets
                .into_iter()
                .map(|target_g| {
                    generate_cell(
                        &model,
                        GridElement::Hill {
                            feel: HillFeel::Floater,
                            strength_g: target_g,
                        },
                        entry_speed_mps,
                    )
                })
                .collect::<Vec<_>>();
            cap_measured_hill_rows(&mut rows, entry_speed_mps);
            rows.into_iter()
                .find(|row| {
                    matches!(row.element, GridElement::Hill { strength_g: row_g, .. } if (row_g - strength_g).abs() < 1e-9)
                })
                .map(|mut row| {
                    row.element = GridElement::Hill { feel, strength_g };
                    row
                })
                .expect("requested hill strength is included in the measurement grid")
        }
    }
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
        if let Some(failure) = minimum_radius_failure(&model, &piece) {
            attempts.push(CandidateSearchAttempt {
                spec: Some(spec),
                failure: Some(failure),
                measurements: Vec::new(),
            });
            continue;
        }
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

fn hill_strength_targets(model: &CoasterModel) -> Vec<f64> {
    let minimum_target = model.comfortable_force_limits.minimum_vertical_g
        + MINIMUM_FORCE_HEADROOM_G
        - MAXIMUM_AIRTIME_ERROR_G;
    let first_tenth = (minimum_target * 10.0).ceil() as i16;
    let mut targets = (first_tenth..=0)
        .rev()
        .map(|target_tenths| f64::from(target_tenths) / 10.0)
        .collect::<Vec<_>>();
    for feel in [
        HillFeel::Floater,
        HillFeel::StrongFloater,
        HillFeel::Ejector,
    ] {
        let default_target = hill_strength(model, feel);
        if !targets.contains(&default_target) {
            targets.push(default_target);
        }
    }
    targets.sort_by(f64::total_cmp);
    targets.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    targets
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
    match element {
        GridElement::Hill { feel, strength_g } => {
            generate_hill_cell(model, feel, strength_g, speed_mps)
        }
        GridElement::FullyBankedTurn => generate_turn_cell(model, element, speed_mps),
    }
}

fn generate_hill_cell(
    model: &CoasterModel,
    feel: HillFeel,
    requested_target_g: f64,
    speed_mps: f64,
) -> GeneratedGridCell {
    let element = GridElement::Hill {
        feel,
        strength_g: requested_target_g,
    };
    if let Some((required_minimum_vertical_g, maximum_intent_vertical_g)) =
        intent_feasibility::conflict(
            model.comfortable_force_limits.minimum_vertical_g,
            requested_target_g,
            MINIMUM_FORCE_HEADROOM_G,
            MAXIMUM_AIRTIME_ERROR_G,
        )
    {
        let available_headroom_g =
            maximum_intent_vertical_g - model.comfortable_force_limits.minimum_vertical_g;
        return GeneratedGridCell {
            model_key: model.key,
            cars: model.train.cars,
            element,
            entry_speed_mps: speed_mps,
            outcome: GridCellOutcome::Refused(CellRefusal::InfeasibleTarget {
                target_g: requested_target_g,
                delivered_g: None,
                available_headroom_g: Some(available_headroom_g),
                failure: CellFailure::IntentHeadroomConflict {
                    required_minimum_vertical_g,
                    maximum_intent_vertical_g,
                },
            }),
        };
    }

    let rules = model.shaped_element_rules();
    let speed_fraction = ((speed_mps - 14.0) / (36.0 - 14.0)).clamp(0.0, 1.0);
    let flank_angle_radians = (MINIMUM_AUTO_FLANK_DEGREES
        + speed_fraction * (MAXIMUM_AUTO_FLANK_DEGREES - MINIMUM_AUTO_FLANK_DEGREES))
        .to_radians();
    let mut crest_g = requested_target_g;
    let mut best_attempt: Option<(
        f64,
        GeneratedSpec,
        Vec<ScenarioMeasurement>,
        CellFailure,
        u8,
    )> = None;
    let mut attempts = 0_u8;

    for _ in 0..=MAXIMUM_HILL_DELIVERY_CALIBRATION_ATTEMPTS {
        attempts += 1;
        let candidate = generate_hill_candidate_with_parameters(
            model,
            requested_target_g,
            rules.hill_pull_g,
            crest_g,
            flank_angle_radians,
            speed_mps,
        );
        let (mut spec, piece) = match candidate {
            Ok(candidate) => candidate,
            Err(failure) => {
                return unresolved_cell(model, element, speed_mps, failure, Vec::new());
            }
        };
        let radius_failure = minimum_radius_failure(model, &piece);

        let measurements = verify_candidate(model, &piece, speed_mps);
        let failure = radius_failure.or_else(|| {
            measurements.iter().find_map(|measurement| {
                measurement_failure(
                    model,
                    GridElement::Hill {
                        feel,
                        strength_g: requested_target_g,
                    },
                    speed_mps,
                    measurement,
                )
            })
        });
        let Some(failure) = failure else {
            let delivered_g = measured_design_delivery(speed_mps, &measurements)
                .expect("passing hill cell has design-speed delivery measurements");
            spec = hill_spec_with_delivery(
                spec,
                requested_target_g,
                requested_target_g,
                delivered_g,
                attempts,
            );
            return GeneratedGridCell {
                model_key: model.key,
                cars: model.train.cars,
                element,
                entry_speed_mps: speed_mps,
                outcome: GridCellOutcome::Solved { spec, measurements },
            };
        };

        let score = measured_attempt_score(requested_target_g, speed_mps, &measurements);
        if best_attempt
            .as_ref()
            .is_none_or(|(best_score, ..)| score < *best_score)
        {
            let delivered_g =
                measured_design_delivery(speed_mps, &measurements).unwrap_or(requested_target_g);
            best_attempt = Some((
                score,
                hill_spec_with_delivery(
                    spec,
                    requested_target_g,
                    requested_target_g,
                    delivered_g,
                    attempts,
                ),
                measurements.clone(),
                failure,
                attempts,
            ));
        }

        let observed_delivery_g = measured_design_delivery(speed_mps, &measurements);
        let mut next_crest_g = crest_g;
        if let Some(delivered_g) = observed_delivery_g
            && (delivered_g - requested_target_g).abs() > MAXIMUM_AIRTIME_ERROR_G
        {
            next_crest_g += requested_target_g - delivered_g;
        }
        if (next_crest_g - crest_g).abs() < 0.001 {
            break;
        }
        crest_g = next_crest_g;
    }

    let (_, best_spec, best_measurements, failure, _) =
        best_attempt.expect("delivery calibration measures a candidate before refusing");
    GeneratedGridCell {
        model_key: model.key,
        cars: model.train.cars,
        element,
        entry_speed_mps: speed_mps,
        outcome: GridCellOutcome::Infeasible {
            failure,
            best_spec,
            attempts,
            best_measurements,
        },
    }
}

fn cap_measured_hill_rows(rows: &mut [GeneratedGridCell], speed_mps: f64) {
    let measured_candidates = rows
        .iter()
        .filter_map(|row| {
            let GridElement::Hill { strength_g, .. } = row.element else {
                unreachable!("hill cap candidates contain only hill rows")
            };
            match &row.outcome {
                GridCellOutcome::Solved { spec, measurements } => Some((
                    spec.target_g(),
                    *spec,
                    measurements.clone(),
                    None,
                    spec.measurement_attempts(),
                    measured_attempt_score(spec.target_g(), speed_mps, measurements),
                )),
                GridCellOutcome::Infeasible {
                    failure,
                    best_spec,
                    attempts,
                    best_measurements,
                } => Some((
                    strength_g,
                    *best_spec,
                    best_measurements.clone(),
                    Some(*failure),
                    *attempts,
                    measured_attempt_score(best_spec.target_g(), speed_mps, best_measurements),
                )),
                GridCellOutcome::Refused(_) | GridCellOutcome::Unresolved { .. } => None,
            }
        })
        .collect::<Vec<_>>();

    for row in rows {
        let GridElement::Hill { strength_g, .. } = row.element else {
            unreachable!("hill cap candidates contain only hill rows")
        };
        if !hill_row_needs_force_cap(&row.outcome) {
            continue;
        }

        let passing_candidate = measured_candidates
            .iter()
            .filter(|(candidate_target_g, _, _, failure, _, _)| {
                failure.is_none() && *candidate_target_g >= strength_g
            })
            .min_by(|left, right| left.0.total_cmp(&right.0));
        if let Some((target_g, spec, measurements, _, attempts, _)) = passing_candidate {
            let delivered_g = measured_design_delivery(speed_mps, measurements)
                .expect("passing cap candidate records design-speed delivery");
            row.outcome = GridCellOutcome::Solved {
                spec: hill_spec_with_delivery(*spec, strength_g, *target_g, delivered_g, *attempts),
                measurements: measurements.clone(),
            };
            continue;
        }

        let best_candidate = measured_candidates
            .iter()
            .filter(|(candidate_target_g, _, _, _, _, _)| *candidate_target_g >= strength_g)
            .min_by(|left, right| left.5.total_cmp(&right.5));
        if let Some((target_g, spec, measurements, Some(failure), attempts, _)) = best_candidate {
            let delivered_g =
                measured_design_delivery(speed_mps, measurements).unwrap_or(spec.target_g());
            row.outcome = GridCellOutcome::Infeasible {
                failure: *failure,
                best_spec: hill_spec_with_delivery(
                    *spec,
                    strength_g,
                    *target_g,
                    delivered_g,
                    *attempts,
                ),
                attempts: *attempts,
                best_measurements: measurements.clone(),
            };
        }
    }
}

fn hill_row_needs_force_cap(outcome: &GridCellOutcome) -> bool {
    match outcome {
        GridCellOutcome::Infeasible {
            failure,
            best_measurements,
            ..
        } => {
            matches!(failure, CellFailure::ForceHeadroomMiss { .. })
                || best_measurements.iter().any(|measurement| {
                    measurement
                        .minimum_force_headroom_g
                        .is_some_and(|headroom_g| headroom_g < MINIMUM_FORCE_HEADROOM_G)
                })
        }
        GridCellOutcome::Refused(CellRefusal::InfeasibleTarget {
            failure: CellFailure::IntentHeadroomConflict { .. },
            ..
        })
        | GridCellOutcome::Unresolved {
            failure: CellFailure::IntentHeadroomConflict { .. },
            ..
        } => true,
        GridCellOutcome::Solved { .. }
        | GridCellOutcome::Refused(_)
        | GridCellOutcome::Unresolved { .. } => false,
    }
}

trait GeneratedSpecHillFields {
    fn target_g(self) -> f64;
    fn measurement_attempts(self) -> u8;
}

impl GeneratedSpecHillFields for GeneratedSpec {
    fn target_g(self) -> f64 {
        match self {
            Self::Hill { target_g, .. } => target_g,
            Self::Turn { .. } => unreachable!("hill cap candidate resolved to a turn"),
        }
    }

    fn measurement_attempts(self) -> u8 {
        match self {
            Self::Hill {
                measurement_attempts,
                ..
            } => measurement_attempts,
            Self::Turn { .. } => unreachable!("hill cap candidate resolved to a turn"),
        }
    }
}

fn generate_turn_cell(
    model: &CoasterModel,
    element: GridElement,
    speed_mps: f64,
) -> GeneratedGridCell {
    let candidate = generate_turn_candidate(model, speed_mps);
    let (spec, piece) = match candidate {
        Ok(candidate) => candidate,
        Err(CellFailure::TurnRadiusOutOfRange { radius_m }) => {
            let maximum_radius_m = model.shaped_element_rules().maximum_turn_radius_m;
            let (spec, piece) =
                match generate_turn_candidate_at_radius(model, speed_mps, maximum_radius_m) {
                    Ok(candidate) => candidate,
                    Err(failure) => {
                        return unresolved_cell(model, element, speed_mps, failure, Vec::new());
                    }
                };
            let measurements = verify_candidate(model, &piece, speed_mps);
            return GeneratedGridCell {
                model_key: model.key,
                cars: model.train.cars,
                element,
                entry_speed_mps: speed_mps,
                outcome: GridCellOutcome::Infeasible {
                    failure: CellFailure::TurnRadiusOutOfRange { radius_m },
                    best_spec: spec,
                    attempts: 1,
                    best_measurements: measurements,
                },
            };
        }
        Err(failure) => {
            return unresolved_cell(model, element, speed_mps, failure, Vec::new());
        }
    };
    if let Some(failure) = minimum_radius_failure(model, &piece) {
        return unresolved_cell(model, element, speed_mps, failure, Vec::new());
    }
    if let GeneratedSpec::Turn { bank_degrees, .. } = spec
        && bank_degrees > model.shaped_element_rules().maximum_bank_degrees
    {
        return unresolved_cell(
            model,
            element,
            speed_mps,
            CellFailure::TurnBankMiss {
                maximum_degrees: model.shaped_element_rules().maximum_bank_degrees,
                observed_degrees: bank_degrees,
            },
            Vec::new(),
        );
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
            Some(failure) => GridCellOutcome::Infeasible {
                failure,
                best_spec: spec,
                attempts: 1,
                best_measurements: measurements,
            },
            None => GridCellOutcome::Solved { spec, measurements },
        },
    }
}

fn unresolved_cell(
    model: &CoasterModel,
    element: GridElement,
    speed_mps: f64,
    failure: CellFailure,
    measurements: Vec<ScenarioMeasurement>,
) -> GeneratedGridCell {
    GeneratedGridCell {
        model_key: model.key,
        cars: model.train.cars,
        element,
        entry_speed_mps: speed_mps,
        outcome: GridCellOutcome::Unresolved {
            failure,
            measurements,
        },
    }
}

fn measured_design_delivery(
    design_speed_mps: f64,
    measurements: &[ScenarioMeasurement],
) -> Option<f64> {
    measurements
        .iter()
        .filter(|measurement| (measurement.entry_speed_mps - design_speed_mps).abs() < 0.01)
        .filter_map(|measurement| measurement.worst_sustained_minimum_vertical_g)
        .reduce(f64::min)
}

fn measured_attempt_score(
    target_g: f64,
    design_speed_mps: f64,
    measurements: &[ScenarioMeasurement],
) -> f64 {
    let minimum_headroom_g = measurements
        .iter()
        .filter_map(|measurement| measurement.minimum_force_headroom_g)
        .reduce(f64::min)
        .unwrap_or(f64::NEG_INFINITY);
    let delivery_error_g = measured_design_delivery(design_speed_mps, measurements)
        .map_or(f64::INFINITY, |delivered_g| (delivered_g - target_g).abs());
    (MINIMUM_FORCE_HEADROOM_G - minimum_headroom_g).max(0.0)
        + (delivery_error_g - MAXIMUM_AIRTIME_ERROR_G).max(0.0)
}

fn hill_spec_with_delivery(
    spec: GeneratedSpec,
    requested_target_g: f64,
    target_g: f64,
    delivered_g: f64,
    measurement_attempts: u8,
) -> GeneratedSpec {
    match spec {
        GeneratedSpec::Hill {
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
            ..
        } => GeneratedSpec::Hill {
            requested_target_g,
            target_g,
            delivered_g,
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
            measurement_attempts,
        },
        GeneratedSpec::Turn { .. } => unreachable!("hill candidate resolved to a turn"),
    }
}

fn minimum_radius_failure(model: &CoasterModel, piece: &Piece) -> Option<CellFailure> {
    let observed_m = piece.maximum_curvature().recip();
    (observed_m < model.minimum_radius_metres).then_some(CellFailure::MinimumRadiusMiss {
        minimum_m: model.minimum_radius_metres,
        observed_m,
    })
}

#[cfg(test)]
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
        taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
    };
    let offline_train = OfflineTrain::Coaster {
        cars: model.train.cars,
        length_metres: model.train.length_metres,
    };
    let (_, fixed_point_iterations, _) =
        solve_hill_offline_profile(offline_spec, offline_train, OFFLINE_STEPS)
            .map_err(CellFailure::OfflineSolver)?;
    let (profile, snapped_span_m, connector_insert_m) =
        generate_hill_profile_once(offline_spec, offline_train)
            .map_err(CellFailure::OfflineSolver)?;
    let height_m = profile
        .resolved_points()
        .iter()
        .map(|point| point[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_m = snapped_span_m;
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
            requested_target_g: target_g,
            target_g,
            delivered_g: target_g,
            pull_g,
            crest_g,
            flank_angle_radians,
            height_m,
            span_m,
            minimum_radius_m,
            connector_insert_m,
            fixed_point_iterations,
            measurement_attempts: 0,
        },
        piece,
    ))
}

fn generate_hill_profile_once(
    spec: OfflineHillSpec,
    train: OfflineTrain,
) -> Result<(SolvedProfile, f64, f64), OfflineSolveError> {
    integrate_shaped_hill_once_profile(spec, train, RUNTIME_STEPS, 1.0, 2.0)
        .map_err(|height_metres| OfflineSolveError::NonPositiveSpeedSquared { height_metres })
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
    generate_turn_candidate_at_radius(model, speed_mps, radius_m)
}

fn generate_turn_candidate_at_radius(
    model: &CoasterModel,
    speed_mps: f64,
    radius_m: f64,
) -> Result<(GeneratedSpec, Piece), CellFailure> {
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
        "grid rows: {}; solved: {}; typed refusals: {}; infeasible: {}; unresolved: {}; generation time: {:.3}s",
        report.cells.len(),
        report.solved_count(),
        report.refused_count(),
        report.infeasible_count(),
        report.unresolved_count(),
        report.elapsed.as_secs_f64()
    )
}

pub(crate) fn lookup_generated_spec(
    model_key: &str,
    cars: u8,
    element: GridElement,
    entry_speed_mps: f64,
) -> Result<GeneratedSpec, CellRefusal> {
    if entry_speed_mps < 14.0 {
        return Err(CellRefusal::BelowFirstVerifiedSpeed {
            minimum_speed_mps: 14.0,
        });
    }
    if entry_speed_mps > 36.0 {
        return Err(CellRefusal::AboveLastVerifiedSpeed {
            maximum_speed_mps: 36.0,
        });
    }
    lookup_generated_spec_in_table(
        GENERATED_SHAPED_ELEMENT_GRID,
        model_key,
        cars,
        element,
        entry_speed_mps,
    )
}

fn lookup_generated_spec_in_table(
    table: &[GeneratedTableCell],
    model_key: &str,
    cars: u8,
    element: GridElement,
    entry_speed_mps: f64,
) -> Result<GeneratedSpec, CellRefusal> {
    let lookup_element =
        lookup_element_with_lowest_measured_strength(table, model_key, cars, element);
    let first = table.partition_point(|candidate| {
        compare_table_key(candidate, model_key, cars, lookup_element) == std::cmp::Ordering::Less
    });
    let after_last = table.partition_point(|candidate| {
        compare_table_key(candidate, model_key, cars, lookup_element) != std::cmp::Ordering::Greater
    });
    if first == after_last {
        return Err(missing_table_row_refusal(element));
    }
    let rows = &table[first..after_last];
    let upper_index = rows.partition_point(|row| row.entry_speed_mps < entry_speed_mps);
    let requested_target_g = match element {
        GridElement::Hill { strength_g, .. } => Some(strength_g),
        GridElement::FullyBankedTurn => None,
    };
    let apply_requested_target = |spec| match requested_target_g {
        Some(target_g) => hill_spec_with_requested_target(spec, target_g),
        None => spec,
    };
    if upper_index < rows.len()
        && (rows[upper_index].entry_speed_mps - entry_speed_mps).abs() < 1e-9
    {
        return table_row_spec(&rows[upper_index], element).map(apply_requested_target);
    }
    if upper_index == 0 || upper_index == rows.len() {
        return Err(missing_table_row_refusal(element));
    }
    let lower = &rows[upper_index - 1];
    let upper = &rows[upper_index];
    let lower_spec = table_row_spec(lower, element)?;
    let upper_spec = table_row_spec(upper, element)?;
    let fraction =
        (entry_speed_mps - lower.entry_speed_mps) / (upper.entry_speed_mps - lower.entry_speed_mps);
    Ok(apply_requested_target(interpolate_spec(
        lower_spec, upper_spec, fraction,
    )))
}

fn lookup_element_with_lowest_measured_strength(
    table: &[GeneratedTableCell],
    model_key: &str,
    cars: u8,
    element: GridElement,
) -> GridElement {
    let GridElement::Hill { feel, strength_g } = element else {
        return element;
    };
    let first_feel_row = table.partition_point(|row| {
        compare_table_key(
            row,
            model_key,
            cars,
            GridElement::Hill {
                feel,
                strength_g: f64::NEG_INFINITY,
            },
        ) == std::cmp::Ordering::Less
    });
    let Some(row) = table.get(first_feel_row) else {
        return element;
    };
    let GridElement::Hill {
        feel: row_feel,
        strength_g: first_strength_g,
    } = row.element
    else {
        return element;
    };
    if row.model_key == model_key
        && row.cars == cars
        && row_feel == feel
        && strength_g < first_strength_g
    {
        GridElement::Hill {
            feel,
            strength_g: first_strength_g,
        }
    } else {
        element
    }
}

fn hill_spec_with_requested_target(spec: GeneratedSpec, requested_target_g: f64) -> GeneratedSpec {
    let GeneratedSpec::Hill {
        target_g,
        delivered_g,
        measurement_attempts,
        ..
    } = spec
    else {
        unreachable!("a hill table key resolved to a turn")
    };
    hill_spec_with_delivery(
        spec,
        requested_target_g,
        target_g,
        delivered_g,
        measurement_attempts,
    )
}

fn compare_table_key(
    row: &GeneratedTableCell,
    model_key: &str,
    cars: u8,
    element: GridElement,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let prefix = row
        .model_key
        .cmp(model_key)
        .then_with(|| row.cars.cmp(&cars));
    if prefix != Ordering::Equal {
        return prefix;
    }
    match (row.element, element) {
        (
            GridElement::Hill {
                feel: row_feel,
                strength_g: row_strength,
            },
            GridElement::Hill {
                feel: target_feel,
                strength_g: target_strength,
            },
        ) => hill_feel_order(row_feel)
            .cmp(&hill_feel_order(target_feel))
            .then_with(|| row_strength.total_cmp(&target_strength)),
        (GridElement::Hill { .. }, GridElement::FullyBankedTurn) => Ordering::Less,
        (GridElement::FullyBankedTurn, GridElement::Hill { .. }) => Ordering::Greater,
        (GridElement::FullyBankedTurn, GridElement::FullyBankedTurn) => Ordering::Equal,
    }
}

fn missing_table_row_refusal(element: GridElement) -> CellRefusal {
    let target_g = match element {
        GridElement::Hill { strength_g, .. } => strength_g,
        GridElement::FullyBankedTurn => 0.0,
    };
    CellRefusal::InfeasibleTarget {
        target_g,
        delivered_g: None,
        available_headroom_g: None,
        failure: CellFailure::PreviewUnavailable,
    }
}

fn table_row_spec(
    row: &GeneratedTableCell,
    element: GridElement,
) -> Result<GeneratedSpec, CellRefusal> {
    match &row.outcome {
        GeneratedTableOutcome::Solved(spec) => Ok(*spec),
        GeneratedTableOutcome::Refused(refusal) => Err(*refusal),
        GeneratedTableOutcome::Infeasible {
            failure,
            best_measurements,
            ..
        } => {
            let target_g = match element {
                GridElement::Hill { strength_g, .. } => strength_g,
                GridElement::FullyBankedTurn => 0.0,
            };
            Err(CellRefusal::InfeasibleTarget {
                target_g,
                delivered_g: measured_design_delivery(row.entry_speed_mps, best_measurements),
                available_headroom_g: best_measurements
                    .iter()
                    .filter_map(|measurement| measurement.minimum_force_headroom_g)
                    .reduce(f64::min),
                failure: *failure,
            })
        }
    }
}

fn interpolate_spec(lower: GeneratedSpec, upper: GeneratedSpec, fraction: f64) -> GeneratedSpec {
    let interpolate = |start: f64, end: f64| start + (end - start) * fraction;
    match (lower, upper) {
        (
            GeneratedSpec::Hill {
                requested_target_g,
                target_g: lower_target_g,
                delivered_g: lower_delivered_g,
                pull_g: lower_pull_g,
                crest_g: lower_crest_g,
                flank_angle_radians: lower_flank_angle,
                height_m: lower_height_m,
                span_m: lower_span_m,
                minimum_radius_m: lower_radius_m,
                connector_insert_m: lower_insert_m,
                fixed_point_iterations: lower_iterations,
                measurement_attempts: lower_attempts,
            },
            GeneratedSpec::Hill {
                target_g: upper_target_g,
                delivered_g: upper_delivered_g,
                pull_g: upper_pull_g,
                crest_g: upper_crest_g,
                flank_angle_radians: upper_flank_angle,
                height_m: upper_height_m,
                span_m: upper_span_m,
                minimum_radius_m: upper_radius_m,
                connector_insert_m: upper_insert_m,
                fixed_point_iterations: upper_iterations,
                measurement_attempts: upper_attempts,
                ..
            },
        ) => GeneratedSpec::Hill {
            requested_target_g,
            target_g: interpolate(lower_target_g, upper_target_g),
            delivered_g: interpolate(lower_delivered_g, upper_delivered_g),
            pull_g: interpolate(lower_pull_g, upper_pull_g),
            crest_g: interpolate(lower_crest_g, upper_crest_g),
            flank_angle_radians: interpolate(lower_flank_angle, upper_flank_angle),
            height_m: interpolate(lower_height_m, upper_height_m),
            span_m: interpolate(lower_span_m, upper_span_m),
            minimum_radius_m: interpolate(lower_radius_m, upper_radius_m),
            connector_insert_m: interpolate(lower_insert_m, upper_insert_m),
            fixed_point_iterations: if fraction < 0.5 {
                lower_iterations
            } else {
                upper_iterations
            },
            measurement_attempts: lower_attempts.max(upper_attempts),
        },
        (
            GeneratedSpec::Turn {
                vertical_g: lower_vertical_g,
                lateral_g: lower_lateral_g,
                radius_m: lower_radius_m,
                bank_degrees: lower_bank_degrees,
            },
            GeneratedSpec::Turn {
                vertical_g: upper_vertical_g,
                lateral_g: upper_lateral_g,
                radius_m: upper_radius_m,
                bank_degrees: upper_bank_degrees,
            },
        ) => GeneratedSpec::Turn {
            vertical_g: interpolate(lower_vertical_g, upper_vertical_g),
            lateral_g: interpolate(lower_lateral_g, upper_lateral_g),
            radius_m: interpolate(lower_radius_m, upper_radius_m),
            bank_degrees: interpolate(lower_bank_degrees, upper_bank_degrees),
        },
        _ => unreachable!("a table key cannot change element kind across speed anchors"),
    }
}

include!("shaped/generated_grid.rs");

#[cfg(test)]
mod tests {
    use super::{
        CellRefusal, GeneratedGridCell, GeneratedSpec, GridCellOutcome, GridElement,
        MAXIMUM_SPEED_HALF_MPS, MINIMUM_SPEED_HALF_MPS, generate_hill_profile_once,
        hill_strength_targets, model_for_train_length, render_generated_table,
    };
    use crate::track::profile::{OfflineHillSpec, OfflineTrain, SHAPED_HILL_TAPER_ANGLE};
    use crate::track::shaped::synthesize_hill_profile;
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
    fn generated_hill_profile_matches_runtime_reconstruction_exactly() {
        let base_model = coaster_models()
            .into_iter()
            .find(|model| model.key == "hyper")
            .expect("hyper model exists");
        let model = model_for_train_length(&base_model, 4);
        let train = OfflineTrain::Coaster {
            cars: model.train.cars,
            length_metres: model.train.length_metres,
        };
        let spec = OfflineHillSpec {
            entry_speed_mps: 28.0,
            pull_g: 2.2,
            crest_g: -0.8,
            flank_angle_radians: 30.0_f64.to_radians(),
            taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
        };

        let generated = generate_hill_profile_once(spec, train)
            .expect("generator profile integrates at the selected speed");
        let runtime = synthesize_hill_profile(
            spec.entry_speed_mps,
            spec.pull_g,
            spec.crest_g,
            spec.flank_angle_radians,
            train,
            1.0,
            2.0,
        )
        .expect("runtime profile integrates at the selected speed");

        assert_eq!(generated, runtime);
    }

    #[test]
    fn hill_strength_grid_covers_tenths_above_the_margin_and_each_default() {
        let models = coaster_models();
        for model in &models {
            let targets = hill_strength_targets(model);
            assert!(targets.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(targets.iter().all(|target| *target <= 0.0));
            let first_tenth = (model.comfortable_force_limits.minimum_vertical_g + 0.15) * 10.0;
            let first_grid_target = first_tenth.ceil() / 10.0;
            assert!(targets.contains(&first_grid_target));
            assert!(
                targets
                    .iter()
                    .filter(|target| **target >= first_grid_target)
                    .all(|target| (target * 10.0).fract().abs() < 1e-9)
            );
            for feel in [
                HillFeel::Floater,
                HillFeel::StrongFloater,
                HillFeel::Ejector,
            ] {
                assert!(targets.contains(&super::hill_strength(model, feel)));
            }
        }
    }

    #[test]
    fn hybrid_low_speed_candidate_is_measured_against_minimum_radius() {
        let hybrid = coaster_models()
            .into_iter()
            .find(|model| model.key == "hybrid")
            .expect("hybrid model exists");
        let model = model_for_train_length(&hybrid, 4);
        let (_, piece) = super::generate_hill_candidate(&model, -0.9, 14.0)
            .expect("the diagnostic candidate can be constructed");
        assert!(matches!(
            super::minimum_radius_failure(&model, &piece),
            Some(super::CellFailure::MinimumRadiusMiss { .. })
        ));
        let cell = super::generate_cell(
            &model,
            GridElement::Hill {
                feel: HillFeel::StrongFloater,
                strength_g: -0.9,
            },
            14.0,
        );
        match cell.outcome {
            GridCellOutcome::Solved { spec, .. } => {
                let GeneratedSpec::Hill {
                    minimum_radius_m, ..
                } = spec
                else {
                    panic!("hill intent resolved to a turn")
                };
                assert!(minimum_radius_m >= model.minimum_radius_metres);
            }
            GridCellOutcome::Infeasible {
                best_measurements, ..
            } => assert_eq!(best_measurements.len(), 6),
            GridCellOutcome::Refused(_) | GridCellOutcome::Unresolved { .. } => {
                panic!("bounded profile candidate did not retain its actual train measurements")
            }
        }
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
                GeneratedGridCell {
                    model_key: "hyper",
                    cars: 8,
                    element: GridElement::Hill {
                        feel: HillFeel::Floater,
                        strength_g: -1.2,
                    },
                    entry_speed_mps: 15.0,
                    outcome: GridCellOutcome::Infeasible {
                        failure: super::CellFailure::ForceHeadroomMiss {
                            required_g: 0.25,
                            observed_g: 0.1,
                        },
                        best_spec: GeneratedSpec::Hill {
                            requested_target_g: -1.2,
                            target_g: -1.0,
                            delivered_g: -1.0,
                            pull_g: 2.0,
                            crest_g: -1.0,
                            flank_angle_radians: 30.0_f64.to_radians(),
                            height_m: 20.0,
                            span_m: 80.0,
                            minimum_radius_m: 20.0,
                            connector_insert_m: 0.0,
                            fixed_point_iterations: 1,
                            measurement_attempts: 1,
                        },
                        attempts: 1,
                        best_measurements: vec![super::ScenarioMeasurement {
                            entry_speed_mps: 15.0,
                            physics: super::PhysicsVariant::RuntimePreview,
                            status: Some(crate::physics::PreviewStatus::ReachedEnd),
                            car_count: 8,
                            worst_sustained_minimum_vertical_g: Some(-1.0),
                            minimum_force_headroom_g: Some(0.1),
                            maximum_lateral_g: Some(0.0),
                        }],
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
        assert!(first.contains("best_measurements: &["));
        assert!(!first.contains("best_measurements: vec!["));
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

    #[test]
    fn cap_selects_the_closest_shallower_measured_passing_target() {
        let measured_candidate = |target_g: f64, delivered_g: f64, headroom_g: f64| {
            let spec = GeneratedSpec::Hill {
                requested_target_g: target_g,
                target_g,
                delivered_g,
                pull_g: 2.0,
                crest_g: target_g,
                flank_angle_radians: 30.0_f64.to_radians(),
                height_m: 20.0,
                span_m: 80.0,
                minimum_radius_m: 20.0,
                connector_insert_m: 0.0,
                fixed_point_iterations: 1,
                measurement_attempts: 1,
            };
            let measurements = [
                super::ScenarioMeasurement {
                    entry_speed_mps: 28.0,
                    physics: super::PhysicsVariant::RuntimePreview,
                    status: Some(crate::physics::PreviewStatus::ReachedEnd),
                    car_count: 8,
                    worst_sustained_minimum_vertical_g: Some(delivered_g),
                    minimum_force_headroom_g: Some(headroom_g),
                    maximum_lateral_g: Some(0.0),
                },
                super::ScenarioMeasurement {
                    entry_speed_mps: 28.0,
                    physics: super::PhysicsVariant::ConstructionTest,
                    status: Some(crate::physics::PreviewStatus::ReachedEnd),
                    car_count: 8,
                    worst_sustained_minimum_vertical_g: Some(delivered_g),
                    minimum_force_headroom_g: Some(headroom_g),
                    maximum_lateral_g: Some(0.0),
                },
            ];
            (spec, measurements.to_vec())
        };
        let (nearby_spec, nearby_measurements) = measured_candidate(-1.0, -1.0, 0.30);
        let (shallow_spec, shallow_measurements) = measured_candidate(-0.9, -0.9, 0.40);
        let (unrelated_spec, unrelated_measurements) = measured_candidate(-1.3, -1.0, 0.30);
        let mut rows = vec![
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.2,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Refused(CellRefusal::InfeasibleTarget {
                    target_g: -1.2,
                    delivered_g: None,
                    available_headroom_g: Some(0.1),
                    failure: super::CellFailure::IntentHeadroomConflict {
                        required_minimum_vertical_g: -1.25,
                        maximum_intent_vertical_g: -1.1,
                    },
                }),
            },
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.0,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Solved {
                    spec: nearby_spec,
                    measurements: nearby_measurements,
                },
            },
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -0.9,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Solved {
                    spec: shallow_spec,
                    measurements: shallow_measurements,
                },
            },
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.3,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Infeasible {
                    failure: super::CellFailure::AirtimeIntentMiss {
                        target_g: -1.3,
                        observed_g: -1.0,
                    },
                    best_spec: unrelated_spec,
                    attempts: 1,
                    best_measurements: unrelated_measurements,
                },
            },
        ];
        super::cap_measured_hill_rows(&mut rows, 28.0);
        let GridCellOutcome::Solved { spec, .. } = rows[0].outcome else {
            panic!("measured -1.0 g candidate should cap the -1.2 g request")
        };
        let GeneratedSpec::Hill {
            requested_target_g,
            target_g,
            delivered_g,
            ..
        } = spec
        else {
            panic!("hill intent resolved to a turn")
        };
        assert_eq!(requested_target_g, -1.2);
        assert_eq!(target_g, -1.0);
        assert_eq!(delivered_g, -1.0);
        assert!(matches!(
            rows[3].outcome,
            GridCellOutcome::Infeasible {
                failure: super::CellFailure::AirtimeIntentMiss { .. },
                ..
            }
        ));
    }

    #[test]
    fn cap_promotes_only_force_conflict_intents_from_neighboring_strengths() {
        let measured_candidate = |target_g: f64| GeneratedSpec::Hill {
            requested_target_g: target_g,
            target_g,
            delivered_g: target_g,
            pull_g: 2.0,
            crest_g: target_g,
            flank_angle_radians: 30.0_f64.to_radians(),
            height_m: 20.0,
            span_m: 80.0,
            minimum_radius_m: 20.0,
            connector_insert_m: 0.0,
            fixed_point_iterations: 1,
            measurement_attempts: 1,
        };
        let measurements = vec![super::ScenarioMeasurement {
            entry_speed_mps: 28.0,
            physics: super::PhysicsVariant::RuntimePreview,
            status: Some(crate::physics::PreviewStatus::ReachedEnd),
            car_count: 8,
            worst_sustained_minimum_vertical_g: Some(-1.0),
            minimum_force_headroom_g: Some(0.30),
            maximum_lateral_g: Some(0.0),
        }];
        let conflict = CellRefusal::InfeasibleTarget {
            target_g: -1.2,
            delivered_g: None,
            available_headroom_g: Some(0.1),
            failure: super::CellFailure::IntentHeadroomConflict {
                required_minimum_vertical_g: -1.25,
                maximum_intent_vertical_g: -1.1,
            },
        };
        let mut rows = vec![
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.2,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Refused(conflict),
            },
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.1,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Unresolved {
                    failure: super::CellFailure::PreviewUnavailable,
                    measurements: Vec::new(),
                },
            },
            GeneratedGridCell {
                model_key: "hyper",
                cars: 8,
                element: GridElement::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -1.0,
                },
                entry_speed_mps: 28.0,
                outcome: GridCellOutcome::Solved {
                    spec: measured_candidate(-1.0),
                    measurements,
                },
            },
        ];

        super::cap_measured_hill_rows(&mut rows, 28.0);

        assert!(matches!(
            rows[0].outcome,
            GridCellOutcome::Solved {
                spec: GeneratedSpec::Hill {
                    requested_target_g: -1.2,
                    target_g: -1.0,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            rows[1].outcome,
            GridCellOutcome::Unresolved {
                failure: super::CellFailure::PreviewUnavailable,
                ..
            }
        ));
        assert!(matches!(rows[2].outcome, GridCellOutcome::Solved { .. }));
    }

    #[test]
    fn below_floor_lookup_uses_the_lowest_measured_strength_and_preserves_the_request() {
        let row = super::GeneratedTableCell {
            model_key: "hyper",
            cars: 8,
            element: GridElement::Hill {
                feel: HillFeel::Floater,
                strength_g: -1.0,
            },
            entry_speed_mps: 28.0,
            outcome: super::GeneratedTableOutcome::Solved(GeneratedSpec::Hill {
                requested_target_g: -1.0,
                target_g: -1.0,
                delivered_g: -1.0,
                pull_g: 2.0,
                crest_g: -1.0,
                flank_angle_radians: 30.0_f64.to_radians(),
                height_m: 20.0,
                span_m: 80.0,
                minimum_radius_m: 20.0,
                connector_insert_m: 0.0,
                fixed_point_iterations: 1,
                measurement_attempts: 1,
            }),
        };

        let spec = super::lookup_generated_spec_in_table(
            &[row],
            "hyper",
            8,
            GridElement::Hill {
                feel: HillFeel::Floater,
                strength_g: -1.2,
            },
            28.0,
        )
        .expect("below-floor lookup uses the lowest measured row");
        assert!(matches!(
            spec,
            GeneratedSpec::Hill {
                requested_target_g: -1.2,
                target_g: -1.0,
                delivered_g: -1.0,
                ..
            }
        ));
    }
}
