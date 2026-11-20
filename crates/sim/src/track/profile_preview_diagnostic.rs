use std::collections::BTreeMap;
use std::sync::Arc;

use crate::physics::{PhysicsConfig, PreviewCache, PreviewStatus};
use crate::track::{
    CoasterModel, Material, Piece, PieceId, PieceKind, Shape, TrackState, catalog, coaster_models,
};

use super::{
    OfflineHillSpec, OfflineSolveError, OfflineTrain, integrate_hill_offline_once_profile,
    solve_hill_offline_profile,
};

const ENTRY_SPEEDS_MPS: [f64; 2] = [14.0, 36.0];
const INTEGRATION_STEPS: usize = 512;

#[derive(Clone, Debug)]
struct PreviewMeasurement {
    cars: u8,
    entry_speed_mps: f64,
    feel: &'static str,
    target_g: f64,
    sustained_minimum_g: f64,
    sustained_target_error_g: f64,
    minimum_vertical_g: f64,
    maximum_vertical_g: f64,
    maximum_longitudinal_g: f64,
    minimum_force_headroom_g: f64,
    every_car_within_limits: bool,
}

#[derive(Default)]
struct PreviewRows {
    rows: Vec<PreviewMeasurement>,
    preview_not_completed: usize,
    offline_errors: BTreeMap<String, usize>,
}

/// Re-run with `tools/run-in-budget.sh cargo test -p theme-park-sim --lib \
/// profile::profile_preview_diagnostic::preview_actual_trains_on_offline_profiles -- --ignored --nocapture`.
#[test]
#[ignore = "bounded TASK-156 actual-train profile diagnostic"]
fn preview_actual_trains_on_offline_profiles() {
    let mut rows = BTreeMap::<(String, &'static str), PreviewRows>::new();
    let mut paired = BTreeMap::<
        (String, u8, u64, &'static str),
        (Option<PreviewMeasurement>, Option<PreviewMeasurement>),
    >::new();

    for base_model in coaster_models() {
        let minimum_cars = *base_model.cars_per_train.start();
        let maximum_cars = *base_model.cars_per_train.end();
        for cars in [minimum_cars, maximum_cars] {
            let mut model = base_model.clone();
            model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
            model.train.cars = cars;
            for entry_speed_mps in ENTRY_SPEEDS_MPS {
                for (feel, crest_g) in hill_feels(&model) {
                    let spec = OfflineHillSpec {
                        entry_speed_mps,
                        pull_g: model.shaped_element_rules().hill_pull_g,
                        crest_g,
                        flank_angle_radians: 35.0_f64.to_radians(),
                        taper_angle_radians: None,
                    };
                    let train = OfflineTrain::Coaster {
                        cars,
                        length_metres: model.train.length_metres,
                    };
                    let fixed = solve_hill_offline_profile(spec, train, INTEGRATION_STEPS);
                    let one_pass =
                        integrate_hill_offline_once_profile(spec, train, INTEGRATION_STEPS);
                    let (fixed, one_pass) = match (fixed, one_pass) {
                        (Ok((fixed, _, _)), Ok((one_pass, _, _))) => (fixed, one_pass),
                        (fixed_result, one_pass_result) => {
                            record_offline_error(
                                &mut rows,
                                &model,
                                "fixed_point",
                                fixed_result.err(),
                            );
                            record_offline_error(
                                &mut rows,
                                &model,
                                "one_pass",
                                one_pass_result.err(),
                            );
                            continue;
                        }
                    };

                    for (phase, profile) in [("fixed_point", &fixed), ("one_pass", &one_pass)] {
                        let summary = rows.entry((model.key.to_owned(), phase)).or_default();
                        match actual_preview(&model, profile, entry_speed_mps, feel, crest_g) {
                            Ok(measurement) => {
                                let pair = paired
                                    .entry((
                                        model.key.to_owned(),
                                        cars,
                                        entry_speed_mps.to_bits(),
                                        feel,
                                    ))
                                    .or_default();
                                if phase == "fixed_point" {
                                    pair.0 = Some(measurement.clone());
                                } else {
                                    pair.1 = Some(measurement.clone());
                                }
                                summary.rows.push(measurement);
                            }
                            Err(()) => summary.preview_not_completed += 1,
                        }
                    }
                }
            }
        }
    }

    for ((model_key, phase), summary) in rows {
        let outside_limits = summary
            .rows
            .iter()
            .filter(|row| !row.every_car_within_limits)
            .count();
        println!(
            "model={model_key} phase={phase} cases={} outside_limits={} preview_not_completed={} offline_errors={:?}",
            summary.rows.len(),
            outside_limits,
            summary.preview_not_completed,
            summary.offline_errors
        );
        print_extreme("worst_sustained_error_g", &summary.rows, true, |row| {
            row.sustained_target_error_g
        });
        print_extreme("least_force_headroom_g", &summary.rows, false, |row| {
            row.minimum_force_headroom_g
        });
        print_extreme("maximum_longitudinal_g", &summary.rows, true, |row| {
            row.maximum_longitudinal_g
        });
    }

    let mut worst_one_pass_by_model =
        BTreeMap::<String, (PreviewMeasurement, PreviewMeasurement)>::new();
    for ((model_key, _, _, _), (fixed, one_pass)) in paired {
        let (Some(fixed), Some(one_pass)) = (fixed, one_pass) else {
            continue;
        };
        let replace = worst_one_pass_by_model
            .get(&model_key)
            .is_none_or(|(_, previous)| {
                one_pass.sustained_target_error_g > previous.sustained_target_error_g
            });
        if replace {
            worst_one_pass_by_model.insert(model_key, (fixed, one_pass));
        }
    }
    for (model_key, (fixed, one_pass)) in worst_one_pass_by_model {
        println!(
            "paired_worst_one_pass model={model_key} cars={} speed={:.1} feel={} target_g={:.3} fixed_sustained_g={:.6} fixed_error_g={:.6} one_pass_sustained_g={:.6} one_pass_error_g={:.6} fixed_headroom_g={:.6} one_pass_headroom_g={:.6} fixed_longitudinal_g={:.6} one_pass_longitudinal_g={:.6}",
            one_pass.cars,
            one_pass.entry_speed_mps,
            one_pass.feel,
            one_pass.target_g,
            fixed.sustained_minimum_g,
            fixed.sustained_target_error_g,
            one_pass.sustained_minimum_g,
            one_pass.sustained_target_error_g,
            fixed.minimum_force_headroom_g,
            one_pass.minimum_force_headroom_g,
            fixed.maximum_longitudinal_g,
            one_pass.maximum_longitudinal_g,
        );
    }
}

fn actual_preview(
    model: &CoasterModel,
    profile: &super::SolvedProfile,
    entry_speed_mps: f64,
    feel: &'static str,
    target_g: f64,
) -> Result<PreviewMeasurement, ()> {
    let physics = PhysicsConfig {
        lift_speed: entry_speed_mps,
        ..PhysicsConfig::default()
    };
    let prefix = lift_prefix(model, entry_speed_mps, physics);
    let mut pieces = prefix.clone();
    let candidate_piece_index = pieces.len();
    let candidate = profile_piece(profile, prefix.last().unwrap().exit);
    let candidate_exit = candidate.exit;
    pieces.push(candidate);
    pieces.extend(runout(candidate_exit, &model.train));
    let mut cache = PreviewCache::default();
    let preview = cache
        .preview_candidate(&pieces, model, physics, candidate_piece_index)
        .expect("offline candidate can be previewed");
    if preview.status != PreviewStatus::ReachedEnd
        || preview.cars.len() != usize::from(model.train.cars)
    {
        return Err(());
    }

    let limits = model.comfortable_force_limits;
    let sustained_minimum_g = preview
        .cars
        .iter()
        .map(|car| car.sustained_minimum_vertical_g.unwrap_or(0.0))
        .fold(f64::INFINITY, f64::min);
    let minimum_vertical_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.minimum_vertical_g)
        .fold(f64::INFINITY, f64::min);
    let maximum_vertical_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.maximum_vertical_g)
        .fold(f64::NEG_INFINITY, f64::max);
    let maximum_longitudinal_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.maximum_longitudinal_g)
        .fold(0.0, f64::max);
    let minimum_force_headroom_g = preview
        .cars
        .iter()
        .map(|car| {
            (car.peaks.minimum_vertical_g - limits.minimum_vertical_g)
                .min(limits.maximum_vertical_g - car.peaks.maximum_vertical_g)
                .min(limits.maximum_lateral_g - car.peaks.maximum_lateral_g)
                .min(limits.maximum_longitudinal_g - car.peaks.maximum_longitudinal_g)
        })
        .fold(f64::INFINITY, f64::min);

    Ok(PreviewMeasurement {
        cars: model.train.cars,
        entry_speed_mps,
        feel,
        target_g,
        sustained_minimum_g,
        sustained_target_error_g: (sustained_minimum_g - target_g).abs(),
        minimum_vertical_g,
        maximum_vertical_g,
        maximum_longitudinal_g,
        minimum_force_headroom_g,
        every_car_within_limits: preview.cars.iter().all(|car| car.within_limits),
    })
}

fn hill_feels(model: &CoasterModel) -> [(&'static str, f64); 3] {
    [
        ("floater", -0.5),
        ("strong_floater", -0.9),
        (
            "ejector",
            if model.track_material == Material::Wood {
                -0.8
            } else {
                -1.2
            },
        ),
    ]
}

fn profile_piece(profile: &super::SolvedProfile, entry: TrackState) -> Piece {
    let mut piece = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap()
        .at(entry)
        .expect("profile starts on the lift connector");
    let span_metres = profile.endpoint()[0];
    assert!((span_metres / 2.0 - (span_metres / 2.0).round()).abs() < 1e-9);
    piece.id = PieceId::AirtimeHill;
    piece.name = "Offline diagnostic hill";
    piece.exit.x = entry.x + (span_metres / 2.0).round() as i32;
    piece.kind = PieceKind::Track;
    piece.design_speeds = None;
    piece.family = None;
    piece.shape = Shape::Profile(Arc::new(profile.clone()));
    piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
    piece.clear_arc_table();
    piece
}

fn lift_prefix(model: &CoasterModel, speed_mps: f64, physics: PhysicsConfig) -> Vec<Piece> {
    let templates = catalog();
    let station = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .unwrap();
    let straight = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let mut prefix = vec![
        station
            .at(TrackState::origin())
            .expect("station starts at origin"),
    ];
    let acceleration_distance = (speed_mps.powi(2) - physics.station_speed.powi(2)).max(0.0)
        / (2.0 * physics.drive_acceleration);
    let required_length = acceleration_distance + model.train.length_metres + 40.0;
    let straight_count = (required_length / straight.length()).ceil() as usize;
    for _ in 0..straight_count {
        let mut lift = straight
            .at(prefix.last().unwrap().exit)
            .expect("lift pieces connect");
        lift.kind = PieceKind::Lift;
        prefix.push(lift);
    }
    let mut cache = PreviewCache::default();
    let preview = cache
        .preview(&prefix, model, physics)
        .expect("lift prefix contains a station");
    assert_eq!(preview.status, PreviewStatus::ReachedEnd);
    let observed_speed = preview.end_speed_mps.unwrap();
    assert!(
        (observed_speed - speed_mps).abs() <= 0.01,
        "lift delivered {observed_speed:.4} m/s, requested {speed_mps:.4} m/s"
    );
    prefix
}

fn runout(exit: TrackState, train: &crate::track::TrainConfiguration) -> Vec<Piece> {
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .unwrap();
    let straight_count = (train.length_metres / straight.length()).ceil() as usize + 2;
    let mut pieces = Vec::with_capacity(straight_count);
    let mut state = exit;
    for _ in 0..straight_count {
        let piece = straight.at(state).expect("runout pieces connect");
        state = piece.exit;
        pieces.push(piece);
    }
    pieces
}

fn record_offline_error(
    rows: &mut BTreeMap<(String, &'static str), PreviewRows>,
    model: &CoasterModel,
    phase: &'static str,
    error: Option<OfflineSolveError>,
) {
    if let Some(error) = error {
        *rows
            .entry((model.key.to_owned(), phase))
            .or_default()
            .offline_errors
            .entry(format!("{error:?}"))
            .or_default() += 1;
    }
}

fn print_extreme(
    label: &str,
    rows: &[PreviewMeasurement],
    highest: bool,
    metric: impl Fn(&PreviewMeasurement) -> f64,
) {
    let row = if highest {
        rows.iter()
            .max_by(|left, right| metric(left).total_cmp(&metric(right)))
    } else {
        rows.iter()
            .min_by(|left, right| metric(left).total_cmp(&metric(right)))
    };
    if let Some(row) = row {
        println!(
            "  {label}={:.6} cars={} speed={:.1} feel={} sustained_min_g={:.6} min_vertical_g={:.6} max_vertical_g={:.6} longitudinal_g={:.6} headroom_g={:.6}",
            metric(row),
            row.cars,
            row.entry_speed_mps,
            row.feel,
            row.sustained_minimum_g,
            row.minimum_vertical_g,
            row.maximum_vertical_g,
            row.maximum_longitudinal_g,
            row.minimum_force_headroom_g,
        );
    }
}
