use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use theme_park_sim::track::shaped_generator::{
    CandidateSearchAttempt, CellFailure, GENERATED_SHAPED_ELEMENT_GRID, GeneratedGridCell,
    GeneratedSpec, GeneratedTableOutcome, GridCellOutcome, GridElement, PhysicsVariant,
    generate_cell_for_model, generate_grid, generate_grid_for_model_train, merge_generated_shards,
    render_generated_shard, render_generated_table, report_summary,
    search_hill_candidates_for_model,
};
use theme_park_sim::track::{CoasterModel, HillFeel, coaster_models};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == "--caps-summary" {
        print_caps_summary();
        return;
    }
    if arguments.len() == 1 && arguments[0] == "--infeasible-summary" {
        print_infeasible_summary();
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--shard")
    {
        run_shard(&arguments[1..]);
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--merge")
    {
        run_merge(&arguments[1..]);
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--sample")
    {
        run_sample(&arguments[1..]);
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--search-sample")
    {
        run_search_sample(&arguments[1..]);
        return;
    }

    let output_path = output_path(&arguments);
    let started = Instant::now();
    let report = generate_grid();
    eprintln!("{}", report_summary(&report));
    for diagnostic in summarize_unresolved(&report.cells) {
        eprintln!("{diagnostic}");
    }
    for diagnostic in summarize_infeasible(&report.cells) {
        eprintln!("{diagnostic}");
    }
    let generated_source = match render_generated_table(&report) {
        Ok(source) => source,
        Err(unresolved) => {
            eprintln!(
                "table was not written: {} grid points remain unverified",
                unresolved.len()
            );
            std::process::exit(1);
        }
    };
    std::fs::write(&output_path, generated_source)
        .unwrap_or_else(|error| panic!("could not write {}: {error}", output_path.display()));
    eprintln!(
        "wrote {} in {:.3}s",
        output_path.display(),
        started.elapsed().as_secs_f64()
    );
}

fn run_shard(arguments: &[String]) {
    let [model_key, cars, output_path] = arguments else {
        usage();
    };
    let cars = cars.parse::<u8>().unwrap_or_else(|error| {
        panic!("invalid car count {cars}: {error}");
    });
    let base_model = coaster_models()
        .into_iter()
        .find(|model| model.key == model_key)
        .unwrap_or_else(|| panic!("unknown model key {model_key}"));
    let started = Instant::now();
    let report = generate_grid_for_model_train(&base_model, cars);
    eprintln!("{}", report_summary(&report));
    for diagnostic in summarize_unresolved(&report.cells) {
        eprintln!("{diagnostic}");
    }
    let source = render_generated_shard(&base_model, cars, &report)
        .unwrap_or_else(|error| panic!("could not render shard: {error}"));
    std::fs::write(output_path, source)
        .unwrap_or_else(|error| panic!("could not write {output_path}: {error}"));
    eprintln!(
        "wrote {output_path} in {:.3}s",
        started.elapsed().as_secs_f64()
    );
}

fn run_merge(arguments: &[String]) {
    let [output_path, shard_paths @ ..] = arguments else {
        usage();
    };
    if shard_paths.is_empty() {
        usage();
    }
    let sources = shard_paths
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("could not read shard {path}: {error}"))
        })
        .collect::<Vec<_>>();
    let source = merge_generated_shards(&sources, &coaster_models())
        .unwrap_or_else(|error| panic!("could not merge shaped-element shards: {error}"));
    std::fs::write(output_path, source)
        .unwrap_or_else(|error| panic!("could not write {output_path}: {error}"));
    eprintln!("merged {} shards into {output_path}", shard_paths.len());
}

fn print_caps_summary() {
    for model in coaster_models() {
        for cars in model.cars_per_train.clone() {
            for feel_name in ["floater", "strong-floater", "ejector"] {
                let GridElement::Hill {
                    feel,
                    strength_g: requested_target_g,
                } = element_for_sample(&model, feel_name)
                else {
                    unreachable!("feel summary must select a hill")
                };
                let mut capped_targets = Vec::new();
                let mut delivered_targets = Vec::new();
                let mut solved_speeds = 0_usize;
                let mut infeasible_speeds = 0_usize;
                let mut refused_speeds = 0_usize;
                for row in GENERATED_SHAPED_ELEMENT_GRID {
                    let GridElement::Hill {
                        feel: row_feel,
                        strength_g: row_target_g,
                    } = row.element
                    else {
                        continue;
                    };
                    if row.model_key != model.key
                        || row.cars != cars
                        || row_feel != feel
                        || (row_target_g - requested_target_g).abs() >= 1e-9
                    {
                        continue;
                    }
                    match &row.outcome {
                        GeneratedTableOutcome::Solved(GeneratedSpec::Hill {
                            target_g,
                            delivered_g,
                            ..
                        }) => {
                            capped_targets.push(*target_g);
                            delivered_targets.push(*delivered_g);
                            solved_speeds += 1;
                        }
                        GeneratedTableOutcome::Infeasible {
                            best_spec:
                                GeneratedSpec::Hill {
                                    target_g,
                                    delivered_g,
                                    ..
                                },
                            ..
                        } => {
                            capped_targets.push(*target_g);
                            delivered_targets.push(*delivered_g);
                            infeasible_speeds += 1;
                        }
                        GeneratedTableOutcome::Refused(_) => refused_speeds += 1,
                        GeneratedTableOutcome::Solved(GeneratedSpec::Turn { .. })
                        | GeneratedTableOutcome::Infeasible {
                            best_spec: GeneratedSpec::Turn { .. },
                            ..
                        } => unreachable!("hill feel key resolved to a turn"),
                    }
                }
                sort_unique_g_values(&mut capped_targets);
                sort_unique_g_values(&mut delivered_targets);
                println!(
                    "{} train {} feel {} requested {:.3} g: capped targets {:?}; delivered {:?}; solved speeds {}; infeasible speeds {}; refused speeds {}",
                    model.key,
                    cars,
                    feel_name,
                    requested_target_g,
                    capped_targets,
                    delivered_targets,
                    solved_speeds,
                    infeasible_speeds,
                    refused_speeds,
                );
            }
        }
    }
}

fn print_infeasible_summary() {
    let infeasible_cells = GENERATED_SHAPED_ELEMENT_GRID
        .iter()
        .filter_map(|cell| match &cell.outcome {
            GeneratedTableOutcome::Infeasible {
                failure,
                best_spec,
                attempts,
                best_measurements,
            } => Some(GeneratedGridCell {
                model_key: cell.model_key,
                cars: cell.cars,
                element: cell.element,
                entry_speed_mps: cell.entry_speed_mps,
                outcome: GridCellOutcome::Infeasible {
                    failure: *failure,
                    best_spec: *best_spec,
                    attempts: *attempts,
                    best_measurements: best_measurements.to_vec(),
                },
            }),
            GeneratedTableOutcome::Solved(_) | GeneratedTableOutcome::Refused(_) => None,
        })
        .collect::<Vec<_>>();
    for summary in summarize_infeasible(&infeasible_cells) {
        println!("{summary}");
    }
    for summary in summarize_invalid_geometry_refusals() {
        println!("{summary}");
    }
}

fn summarize_invalid_geometry_refusals() -> Vec<String> {
    let mut by_region = BTreeMap::<(String, u8, String, &'static str), Vec<(f64, u8)>>::new();
    for cell in GENERATED_SHAPED_ELEMENT_GRID {
        let GeneratedTableOutcome::Refused(
            theme_park_sim::track::shaped_generator::CellRefusal::InvalidGeometry {
                failure,
                attempts,
            },
        ) = &cell.outcome
        else {
            continue;
        };
        let element = match cell.element {
            GridElement::Hill { feel, strength_g } => {
                format!("{feel:?} requested {strength_g:.1} g")
            }
            GridElement::FullyBankedTurn => "fully banked turn".to_owned(),
        };
        by_region
            .entry((
                cell.model_key.to_owned(),
                cell.cars,
                element,
                failure_name(failure),
            ))
            .or_default()
            .push((cell.entry_speed_mps, *attempts));
    }

    by_region
        .into_iter()
        .flat_map(|((model_key, cars, element, cause), mut rows)| {
            rows.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut summaries = Vec::new();
            let mut range_start = 0;
            while range_start < rows.len() {
                let mut range_end = range_start + 1;
                while range_end < rows.len()
                    && (rows[range_end].0 - rows[range_end - 1].0 - 0.5).abs() < 1e-9
                {
                    range_end += 1;
                }
                let range = &rows[range_start..range_end];
                let maximum_attempts = range
                    .iter()
                    .map(|(_, attempts)| *attempts)
                    .max()
                    .expect("a refusal range contains at least one row");
                summaries.push(format!(
                    "{model_key} train {cars} {element}: {:.1} to {:.1} m/s ({cause}, no measurable geometry after at most {maximum_attempts} attempts)",
                    range[0].0,
                    range[range.len() - 1].0,
                ));
                range_start = range_end;
            }
            summaries
        })
        .collect()
}

fn sort_unique_g_values(values: &mut Vec<f64>) {
    values.sort_by(f64::total_cmp);
    values.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
}

fn output_path(arguments: &[String]) -> PathBuf {
    match arguments {
        [] => PathBuf::from(theme_park_sim::track::shaped_generator::default_output_path()),
        [flag, path] if flag == "--output" => PathBuf::from(path),
        _ => usage(),
    }
}

fn run_sample(arguments: &[String]) {
    let [model_key, cars, element_name, speed_mps] = arguments else {
        usage();
    };
    let cars = cars.parse::<u8>().unwrap_or_else(|error| {
        panic!("invalid car count {cars}: {error}");
    });
    let speed_mps = speed_mps.parse::<f64>().unwrap_or_else(|error| {
        panic!("invalid speed {speed_mps}: {error}");
    });
    let model = coaster_models()
        .into_iter()
        .find(|model| model.key == model_key)
        .unwrap_or_else(|| panic!("unknown model key {model_key}"));
    let element = element_for_sample(&model, element_name);
    let started = Instant::now();
    let cell = generate_cell_for_model(&model, cars, element, speed_mps);
    eprintln!("sample generation: {:.3}s", started.elapsed().as_secs_f64());
    println!("{cell:#?}");
}

fn run_search_sample(arguments: &[String]) {
    let [model_key, cars, feel_name, speed_mps] = arguments else {
        usage();
    };
    let cars = cars.parse::<u8>().unwrap_or_else(|error| {
        panic!("invalid car count {cars}: {error}");
    });
    let speed_mps = speed_mps.parse::<f64>().unwrap_or_else(|error| {
        panic!("invalid speed {speed_mps}: {error}");
    });
    let feel = feel_for_sample(feel_name);
    let model = coaster_models()
        .into_iter()
        .find(|model| model.key == model_key)
        .unwrap_or_else(|| panic!("unknown model key {model_key}"));
    let started = Instant::now();
    let attempts = search_hill_candidates_for_model(&model, cars, feel, speed_mps);
    eprintln!(
        "searched {} parameter schedules in {:.3}s",
        attempts.len(),
        started.elapsed().as_secs_f64()
    );
    if let Some((index, attempt)) = attempts
        .iter()
        .enumerate()
        .find(|(_, attempt)| attempt.failure.is_none())
    {
        print_search_attempt(index, attempt, speed_mps);
    } else if let Some((index, attempt)) =
        attempts.iter().enumerate().min_by(|(_, left), (_, right)| {
            candidate_score(left, speed_mps).total_cmp(&candidate_score(right, speed_mps))
        })
    {
        eprintln!("no candidate passed; best measured attempt follows:");
        print_search_attempt(index, attempt, speed_mps);
    }
}

fn feel_for_sample(name: &str) -> HillFeel {
    match name {
        "floater" => HillFeel::Floater,
        "strong-floater" => HillFeel::StrongFloater,
        "ejector" => HillFeel::Ejector,
        _ => usage(),
    }
}

fn print_search_attempt(index: usize, attempt: &CandidateSearchAttempt, design_speed_mps: f64) {
    let design_measurements = attempt
        .measurements
        .iter()
        .filter(|measurement| {
            (measurement.entry_speed_mps - design_speed_mps).abs() < 0.01
                && measurement.physics == PhysicsVariant::RuntimePreview
        })
        .collect::<Vec<_>>();
    let delivered_g = design_measurements
        .first()
        .and_then(|measurement| measurement.worst_sustained_minimum_vertical_g);
    let minimum_headroom_g = attempt
        .measurements
        .iter()
        .filter_map(|measurement| measurement.minimum_force_headroom_g)
        .reduce(f64::min);
    println!(
        "candidate {index}: spec={:?}; failure={:?}; design_delivery={delivered_g:?}; min_headroom={minimum_headroom_g:?}",
        attempt.spec, attempt.failure,
    );
}

fn candidate_score(attempt: &CandidateSearchAttempt, design_speed_mps: f64) -> f64 {
    let target_g = match attempt.spec {
        Some(theme_park_sim::track::shaped_generator::GeneratedSpec::Hill { target_g, .. }) => {
            target_g
        }
        _ => return f64::INFINITY,
    };
    let delivered_g = attempt
        .measurements
        .iter()
        .find(|measurement| {
            (measurement.entry_speed_mps - design_speed_mps).abs() < 0.01
                && measurement.physics == PhysicsVariant::RuntimePreview
        })
        .and_then(|measurement| measurement.worst_sustained_minimum_vertical_g);
    let minimum_headroom_g = attempt
        .measurements
        .iter()
        .filter_map(|measurement| measurement.minimum_force_headroom_g)
        .reduce(f64::min);
    match (delivered_g, minimum_headroom_g) {
        (Some(delivered), Some(headroom)) => {
            (delivered - target_g).abs() + (0.25 - headroom).max(0.0)
        }
        _ => f64::INFINITY,
    }
}

fn element_for_sample(model: &CoasterModel, name: &str) -> GridElement {
    let rules = model.shaped_element_rules();
    match name {
        "floater" => GridElement::Hill {
            feel: HillFeel::Floater,
            strength_g: rules.default_floater_g,
        },
        "strong-floater" => GridElement::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: rules.default_strong_floater_g,
        },
        "ejector" => GridElement::Hill {
            feel: HillFeel::Ejector,
            strength_g: if model.track_material == theme_park_sim::track::Material::Wood {
                rules.wooden_ejector_g
            } else {
                rules.steel_ejector_g
            },
        },
        "turn" => GridElement::FullyBankedTurn,
        _ => usage(),
    }
}

fn summarize_unresolved(cells: &[GeneratedGridCell]) -> Vec<String> {
    let mut by_model = std::collections::BTreeMap::<String, Vec<&GeneratedGridCell>>::new();
    for cell in cells {
        if matches!(cell.outcome, GridCellOutcome::Unresolved { .. }) {
            by_model
                .entry(cell.model_key.to_owned())
                .or_default()
                .push(cell);
        }
    }
    by_model
        .into_iter()
        .map(|(model_key, failures)| {
            let first = failures[0];
            let last = failures[failures.len() - 1];
            let counts = failures.iter().fold(
                std::collections::BTreeMap::<String, usize>::new(),
                |mut counts, cell| {
                    let failure = match &cell.outcome {
                        GridCellOutcome::Unresolved { failure, .. } => failure,
                        _ => unreachable!(),
                    };
                    *counts.entry(failure_name(failure).to_owned()).or_default() += 1;
                    counts
                },
            );
            format!(
                "{model_key}: {} unresolved rows from {} to {} m/s; causes {counts:?}; first {:?} at {} cars / {:.1} m/s",
                failures.len(),
                first.entry_speed_mps,
                last.entry_speed_mps,
                first.element,
                first.cars,
                first.entry_speed_mps,
            )
        })
        .collect()
}

fn summarize_infeasible(cells: &[GeneratedGridCell]) -> Vec<String> {
    let mut by_region =
        BTreeMap::<(String, u8, String, &'static str), Vec<&GeneratedGridCell>>::new();
    for cell in cells {
        let GridCellOutcome::Infeasible { failure, .. } = &cell.outcome else {
            continue;
        };
        let element = match cell.element {
            GridElement::Hill { feel, strength_g } => {
                format!("{feel:?} requested {strength_g:.1} g")
            }
            GridElement::FullyBankedTurn => "fully banked turn".to_owned(),
        };
        by_region
            .entry((
                cell.model_key.to_owned(),
                cell.cars,
                element,
                failure_name(failure),
            ))
            .or_default()
            .push(cell);
    }

    by_region
        .into_iter()
        .flat_map(|((model_key, cars, element, cause), rows)| {
            let mut summaries = Vec::new();
            let mut range_start = 0;
            while range_start < rows.len() {
                let mut range_end = range_start + 1;
                while range_end < rows.len()
                    && (rows[range_end].entry_speed_mps
                        - rows[range_end - 1].entry_speed_mps
                        - 0.5)
                        .abs()
                        < 1e-9
                {
                    range_end += 1;
                }
                let range = &rows[range_start..range_end];
                let best = range
                    .iter()
                    .min_by(|left, right| {
                        infeasible_score(left).total_cmp(&infeasible_score(right))
                    })
                    .expect("an infeasible speed range has at least one row");
                let best_measurement = infeasible_best_measurement(best);
                summaries.push(format!(
                    "{model_key} train {cars} {element}: {:.1} to {:.1} m/s ({cause}); {best_measurement}",
                    range[0].entry_speed_mps,
                    range[range.len() - 1].entry_speed_mps,
                ));
                range_start = range_end;
            }
            summaries
        })
        .collect()
}

fn infeasible_score(cell: &GeneratedGridCell) -> f64 {
    let GridCellOutcome::Infeasible {
        best_spec,
        best_measurements,
        ..
    } = &cell.outcome
    else {
        return f64::INFINITY;
    };
    let delivered_g = best_measurements
        .iter()
        .filter(|measurement| (measurement.entry_speed_mps - cell.entry_speed_mps).abs() < 0.01)
        .filter_map(|measurement| measurement.worst_sustained_minimum_vertical_g)
        .reduce(f64::min);
    let headroom_g = best_measurements
        .iter()
        .filter_map(|measurement| measurement.minimum_force_headroom_g)
        .reduce(f64::min);
    match (best_spec, delivered_g, headroom_g) {
        (
            theme_park_sim::track::shaped_generator::GeneratedSpec::Hill { target_g, .. },
            Some(delivered),
            Some(headroom),
        ) => (delivered - target_g).abs().max(0.0) + (0.25 - headroom).max(0.0),
        (_, _, Some(headroom)) => (0.25 - headroom).max(0.0),
        _ => f64::INFINITY,
    }
}

fn infeasible_best_measurement(cell: &GeneratedGridCell) -> String {
    let GridCellOutcome::Infeasible {
        best_spec,
        best_measurements,
        ..
    } = &cell.outcome
    else {
        unreachable!("infeasible summary receives only infeasible rows")
    };
    let headroom_g = best_measurements
        .iter()
        .filter_map(|measurement| measurement.minimum_force_headroom_g)
        .reduce(f64::min)
        .unwrap_or(f64::NAN);
    let design_delivery_g = best_measurements
        .iter()
        .filter(|measurement| (measurement.entry_speed_mps - cell.entry_speed_mps).abs() < 0.01)
        .filter_map(|measurement| measurement.worst_sustained_minimum_vertical_g)
        .reduce(f64::min);
    match best_spec {
        GeneratedSpec::Hill { target_g, .. } => match design_delivery_g {
            Some(delivered_g) => format!(
                "best measured hill target {target_g:.3} g, delivery {delivered_g:.3} g, headroom {headroom_g:.3} g"
            ),
            None => format!(
                "best measured hill target {target_g:.3} g, no design delivery, headroom {headroom_g:.3} g"
            ),
        },
        GeneratedSpec::Turn {
            vertical_g,
            bank_degrees,
            radius_m,
            ..
        } => {
            let lateral_g = best_measurements
                .iter()
                .filter_map(|measurement| measurement.maximum_lateral_g)
                .reduce(f64::max);
            match lateral_g {
                Some(lateral_g) => format!(
                    "best measured turn bank {bank_degrees:.2} degrees, radius {radius_m:.1} m, design vertical {vertical_g:.3} g, lateral {lateral_g:.3} g, headroom {headroom_g:.3} g"
                ),
                None => format!(
                    "best measured turn bank {bank_degrees:.2} degrees, radius {radius_m:.1} m, design vertical {vertical_g:.3} g, no lateral measurement, headroom {headroom_g:.3} g"
                ),
            }
        }
    }
}

fn failure_name(failure: &CellFailure) -> &'static str {
    match failure {
        CellFailure::OfflineSolver(_) => "offline_solver",
        CellFailure::PieceConstruction(_) => "piece_construction",
        CellFailure::TurnRadiusOutOfRange { .. } => "turn_radius",
        CellFailure::MinimumRadiusMiss { .. } => "minimum_radius",
        CellFailure::IntentHeadroomConflict { .. } => "intent_headroom_conflict",
        CellFailure::PreviewUnavailable => "preview_unavailable",
        CellFailure::PreviewDidNotReachEnd(_) => "preview_did_not_reach_end",
        CellFailure::WrongCarCount { .. } => "wrong_car_count",
        CellFailure::MissingSustainedAirtime => "missing_sustained_airtime",
        CellFailure::AirtimeIntentMiss { .. } => "airtime_intent_miss",
        CellFailure::ForceHeadroomMiss { .. } => "force_headroom_miss",
        CellFailure::TurnLateralMiss { .. } => "turn_lateral_miss",
        CellFailure::TurnBankMiss { .. } => "turn_bank_miss",
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: generate_shaped_elements [--output PATH] | --shard MODEL_KEY CARS OUTPUT_PATH | --merge OUTPUT_PATH SHARD_PATH... | --caps-summary | --infeasible-summary | --sample MODEL_KEY CARS floater|strong-floater|ejector|turn SPEED_MPS | --search-sample MODEL_KEY CARS floater|strong-floater|ejector SPEED_MPS"
    );
    std::process::exit(2)
}
