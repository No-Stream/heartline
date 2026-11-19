use std::path::PathBuf;
use std::time::Instant;

use theme_park_sim::track::shaped_generator::{
    CandidateSearchAttempt, CellFailure, GeneratedGridCell, GridCellOutcome, GridElement,
    PhysicsVariant, generate_cell_for_model, generate_grid, render_generated_table, report_summary,
    search_hill_candidates_for_model,
};
use theme_park_sim::track::{CoasterModel, HillFeel, coaster_models};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
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

fn failure_name(failure: &CellFailure) -> &'static str {
    match failure {
        CellFailure::OfflineSolver(_) => "offline_solver",
        CellFailure::PieceConstruction(_) => "piece_construction",
        CellFailure::TurnRadiusOutOfRange { .. } => "turn_radius",
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
        "usage: generate_shaped_elements [--output PATH] | --sample MODEL_KEY CARS floater|strong-floater|ejector|turn SPEED_MPS | --search-sample MODEL_KEY CARS floater|strong-floater|ejector SPEED_MPS"
    );
    std::process::exit(2)
}
