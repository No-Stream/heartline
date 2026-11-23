use crate::physics::{CandidateCarForces, GRAVITY};
use crate::track::curves::{raven_crest_dive, raven_crest_dive_rail_drop_m};
use crate::track::profile::{
    OfflineSolveError, OfflineTrain, SolvedProfile, solve_shaped_full_loop_profile,
    solve_shaped_raven_half_loop_profile, solve_shaped_zero_g_hill_profile,
    zero_g_crest_duration_seconds,
};
use crate::track::shaped::precision::{canonicalize_element_spec, round_element_value};
use crate::track::shaped::{
    CalibrationSeed, ElementIntent, ElementSpec, LoopTopFeel, PROFILE_STEPS,
    REQUIRED_FORCE_HEADROOM_G, ShapedProfile, SolveContext, SolveRefusal, SolvedElement,
    calibrate_piece, candidate_exceeds_design_targets, candidate_passes_force_budget,
    grid_heading_delta, piece_from_spec, stored_value_matches,
};
use crate::track::{
    Bank, CoasterModel, Hand, Heading, HorizontalCurvature, Inversion, Material, Slope, TrackState,
    VerticalCurvature,
};

pub(super) const MAXIMUM_INVERSION_PEAK_ERROR_G: f64 = 0.20;
pub(super) const MAXIMUM_SHAPED_ROLL_RATE_DEG_S: f64 = 200.0;
/// Every car's seat force at a Raven turn's inverted apex stays this close to the top feel.
pub(super) const MAXIMUM_RAVEN_TOP_FEEL_ERROR_G: f64 = 0.35;
const LOOP_DEFAULT_HOLD_DEGREES: f64 = 105.0;
/// Seconds over which a Raven turn's crest curvature tapers out after the apex.
const RAVEN_CREST_TAPER_SECONDS: f64 = 0.3;
/// Riders feel a Raven crest about this much lighter than its profile schedule, mostly from the
/// curvature taper inside the crest band and the 5 Hz rider filter, so the first try aims high.
const RAVEN_CREST_SCHEDULE_OFFSET_G: f64 = 0.3;
/// Shortest crest taper a low Raven turn may use to keep its exit off the ground.
const MINIMUM_RAVEN_CREST_TAPER_SECONDS: f64 = 0.15;
const MINIMUM_INVERSION_PEAK_TARGET_G: f64 = 2.5;
const MAXIMUM_INVERSION_PEAK_TARGET_G: f64 = 5.0;
const ZERO_G_TARGET_CREST_SECONDS: f64 = 2.60;
const ZERO_G_TWIST_RAMP: f64 = 0.25;
const ZERO_G_TWIST_EXTENSION_FRACTION: f64 = 0.08;
const ZERO_G_MIN_FLANK_DEGREES: f64 = 10.0;
const ZERO_G_MAX_FLANK_DEGREES: f64 = 55.0;
const ZERO_G_FLANK_SEARCH_STEPS: usize = 10;
const ZERO_G_MAX_CREST_ABS_G: f64 = 0.65;

pub(super) fn inversion_within_model_vibe(model: &CoasterModel, target_peak_g: f64) -> bool {
    target_peak_g
        <= model.design_force_targets.maximum_vertical_g - REQUIRED_FORCE_HEADROOM_G + 1e-9
}

pub(super) fn validate_inversion_scope(
    intent: &ElementIntent,
    context: &SolveContext<'_>,
) -> Result<ShapedProfile, SolveRefusal> {
    let Some(profile) = context.model.shaped_profile else {
        return Err(SolveRefusal::UnsupportedScope);
    };
    if !profile.allows_cars(context.train.cars) {
        return Err(SolveRefusal::UnsupportedScope);
    }
    let model_allows = match intent {
        ElementIntent::Loop { .. } => context
            .model
            .allowed_inversions
            .contains(&Inversion::VerticalLoop),
        ElementIntent::ZeroGRoll { .. } => context
            .model
            .allowed_inversions
            .contains(&Inversion::HeartlineRoll),
        ElementIntent::RavenTurn { .. } => {
            context
                .model
                .allowed_inversions
                .contains(&Inversion::VerticalLoop)
                && context
                    .model
                    .allowed_inversions
                    .contains(&Inversion::Corkscrew)
        }
        ElementIntent::Hill { .. } | ElementIntent::Turn { .. } => unreachable!(),
    };
    if !model_allows
        || profile.minimum_inversion_speed_mps <= 0.0
        || context.entry_speed_mps > profile.maximum_inversion_speed_mps
    {
        return Err(SolveRefusal::UnsupportedScope);
    }
    if context.entry_speed_mps < profile.minimum_inversion_speed_mps {
        return Err(SolveRefusal::BelowMinimumSpeed {
            entry_speed_mps: context.entry_speed_mps,
            minimum_speed_mps: profile.minimum_inversion_speed_mps,
        });
    }
    Ok(profile)
}

fn horizontal_grid_step(heading: Heading) -> f64 {
    if heading.index() % 2 == 0 {
        2.0
    } else {
        2.0 * std::f64::consts::SQRT_2
    }
}

fn map_offline_error(entry_speed_mps: f64, error: OfflineSolveError) -> SolveRefusal {
    match error {
        OfflineSolveError::NonPositiveSpeedSquared { height_metres } => SolveRefusal::Stall {
            entry_speed_mps,
            height_m: height_metres,
        },
        OfflineSolveError::InvalidCurvature { .. } | OfflineSolveError::DidNotConverge => {
            SolveRefusal::InvalidGeometry
        }
    }
}

fn worst_peak_vertical_g(cars: &[CandidateCarForces]) -> f64 {
    cars.iter()
        .map(|car| car.peaks.maximum_vertical_g)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn worst_inverted_vertical_g(cars: &[CandidateCarForces]) -> f64 {
    cars.iter()
        .filter_map(|car| car.inverted_minimum_vertical_g)
        .reduce(f64::min)
        .unwrap_or_else(|| {
            cars.iter()
                .map(|car| car.peaks.minimum_vertical_g)
                .fold(f64::INFINITY, f64::min)
        })
}

/// Every car's crest force lies within the top-feel tolerance of the target.
pub(super) fn raven_top_feel_reached(cars: &[CandidateCarForces], target_top_g: f64) -> bool {
    crest_top_band_g(cars).is_some_and(|(low_g, high_g)| {
        (low_g - target_top_g).abs() <= MAXIMUM_RAVEN_TOP_FEEL_ERROR_G
            && (high_g - target_top_g).abs() <= MAXIMUM_RAVEN_TOP_FEEL_ERROR_G
    })
}

/// Lowest and highest crest seat force among the cars.
fn crest_top_band_g(cars: &[CandidateCarForces]) -> Option<(f64, f64)> {
    cars.iter()
        .try_fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), car| {
            let apex_g = car.inverted_crest_vertical_g?;
            Some((low.min(apex_g), high.max(apex_g)))
        })
}

fn worst_roll_rate_deg_s(cars: &[CandidateCarForces]) -> f64 {
    cars.iter()
        .map(|car| car.peak_roll_rate_deg_s)
        .fold(0.0_f64, f64::max)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn regenerate_loop_profile(
    cars: u8,
    design_speed_mps: f64,
    train_length_metres: f64,
    bottom_g: f64,
    top_g: f64,
    hold_degrees: f64,
    profile_grid_half_tiles: u16,
    diagonal: bool,
) -> Result<SolvedProfile, String> {
    let entry_speed_mps = round_element_value(design_speed_mps);
    if cars == 0
        || !entry_speed_mps.is_finite()
        || entry_speed_mps <= 0.0
        || !train_length_metres.is_finite()
        || train_length_metres <= 0.0
        || !bottom_g.is_finite()
        || !top_g.is_finite()
        || !hold_degrees.is_finite()
        || !(60.0..170.0).contains(&hold_degrees)
        || profile_grid_half_tiles == 0
    {
        return Err("shaped loop recipe contains invalid parameters".into());
    }
    let train = OfflineTrain::Coaster {
        cars,
        length_metres: train_length_metres,
    };
    let mut profile = solve_shaped_full_loop_profile(
        entry_speed_mps,
        bottom_g,
        top_g,
        hold_degrees,
        train,
        PROFILE_STEPS,
    )
    .map_err(|error| format!("could not regenerate shaped loop geometry: {error:?}"))?;
    let grid_step_m = if diagonal {
        2.0 * std::f64::consts::SQRT_2
    } else {
        2.0
    };
    let snapped_span_metres = f64::from(profile_grid_half_tiles) * grid_step_m;
    let connector_insert_metres = snapped_span_metres - profile.endpoint()[0];
    if !connector_insert_metres.is_finite() || connector_insert_metres < -1e-6 {
        return Err("shaped loop recipe has an invalid grid span".into());
    }
    profile.append_level_straight(connector_insert_metres.max(0.0));
    profile.round_to_nine_decimal_places();
    Ok(profile)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn regenerate_zero_g_roll_profile(
    cars: u8,
    design_speed_mps: f64,
    train_length_metres: f64,
    pull_g: f64,
    flank_angle_degrees: f64,
    twist_ramp: f64,
    twist_start_m: f64,
    twist_end_m: f64,
    profile_grid_half_tiles: u16,
    diagonal: bool,
) -> Result<SolvedProfile, String> {
    let entry_speed_mps = round_element_value(design_speed_mps);
    if cars == 0
        || !entry_speed_mps.is_finite()
        || entry_speed_mps <= 0.0
        || !train_length_metres.is_finite()
        || train_length_metres <= 0.0
        || !pull_g.is_finite()
        || !flank_angle_degrees.is_finite()
        || !(5.0..80.0).contains(&flank_angle_degrees)
        || !twist_ramp.is_finite()
        || !(0.05..=0.5).contains(&twist_ramp)
        || !twist_start_m.is_finite()
        || !twist_end_m.is_finite()
        || twist_start_m < 0.0
        || twist_end_m <= twist_start_m
        || profile_grid_half_tiles == 0
    {
        return Err("shaped zero-g roll recipe contains invalid parameters".into());
    }
    let train = OfflineTrain::Coaster {
        cars,
        length_metres: train_length_metres,
    };
    let (mut profile, _, _) = solve_shaped_zero_g_hill_profile(
        entry_speed_mps,
        pull_g,
        flank_angle_degrees.to_radians(),
        train,
        PROFILE_STEPS,
    )
    .map_err(|error| format!("could not regenerate shaped zero-g roll geometry: {error:?}"))?;
    let grid_step_m = if diagonal {
        2.0 * std::f64::consts::SQRT_2
    } else {
        2.0
    };
    let snapped_span_metres = f64::from(profile_grid_half_tiles) * grid_step_m;
    let connector_insert_metres = snapped_span_metres - profile.endpoint()[0];
    if !connector_insert_metres.is_finite() || connector_insert_metres < -1e-6 {
        return Err("shaped zero-g roll recipe has an invalid grid span".into());
    }
    profile.append_level_straight(connector_insert_metres.max(0.0));
    profile.round_to_nine_decimal_places();
    if twist_end_m > profile.length() + 1e-6 {
        return Err("shaped zero-g roll twist extends past profile length".into());
    }
    Ok(profile)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn regenerate_raven_turn_half_loop(
    cars: u8,
    design_speed_mps: f64,
    train_length_metres: f64,
    bottom_g: f64,
    top_g: f64,
    hold_degrees: f64,
    exit_dz_half_tiles: i32,
    roll_dx_m: f64,
    roll_lateral_m: f64,
) -> Result<SolvedProfile, String> {
    let entry_speed_mps = round_element_value(design_speed_mps);
    if cars == 0
        || !entry_speed_mps.is_finite()
        || entry_speed_mps <= 0.0
        || !train_length_metres.is_finite()
        || train_length_metres <= 0.0
        || !bottom_g.is_finite()
        || !top_g.is_finite()
        || !hold_degrees.is_finite()
        || !(60.0..170.0).contains(&hold_degrees)
        || exit_dz_half_tiles < 3
        || !roll_dx_m.is_finite()
        || roll_dx_m <= 0.0
        || !roll_lateral_m.is_finite()
    {
        return Err("shaped Raven turn recipe contains invalid parameters".into());
    }
    let train = OfflineTrain::Coaster {
        cars,
        length_metres: train_length_metres,
    };
    let half_loop = solve_shaped_raven_half_loop_profile(
        entry_speed_mps,
        bottom_g,
        top_g,
        hold_degrees,
        train,
        PROFILE_STEPS,
    )
    .map_err(|error| format!("could not regenerate shaped Raven turn geometry: {error:?}"))?;
    let rail_drop_m = half_loop.endpoint()[1] - f64::from(exit_dz_half_tiles) * 2.0;
    if raven_crest_dive(
        &half_loop,
        crate::math::hypot(roll_dx_m, roll_lateral_m),
        rail_drop_m,
    )
    .is_none()
    {
        return Err("shaped Raven turn recipe cannot dive from its crest to its exit".into());
    }
    Ok(half_loop)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_loop(
    hand: Hand,
    peak_g: f64,
    top_feel: LoopTopFeel,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    if !peak_g.is_finite()
        || !(MINIMUM_INVERSION_PEAK_TARGET_G..=MAXIMUM_INVERSION_PEAK_TARGET_G).contains(&peak_g)
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: peak_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let target_peak_g = round_element_value(peak_g);
    let top_g = round_element_value(top_feel.target_g());
    let within_vibe = inversion_within_model_vibe(context.model, target_peak_g);
    let steel_structure = context.model.track_material == Material::Wood;
    let diagonal = context.entry.heading.index() % 2 != 0;
    let grid_step_m = horizontal_grid_step(context.entry.heading);
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };

    let seed = preview_seed.and_then(|candidate| {
        let ElementSpec::Loop {
            delivered_peak_g,
            bottom_g,
            ..
        } = candidate.spec
        else {
            return None;
        };
        let seed = candidate.calibration_seed.as_ref()?;
        Some((delivered_peak_g, bottom_g, seed))
    });

    let mut preview_count = u8::from(seed.is_some());
    let mut previous_attempt = seed.map(|(delivered, bottom_g, _)| (bottom_g, delivered));
    let mut bottom_g = match seed {
        Some((delivered, seed_bottom, _)) => {
            (seed_bottom + target_peak_g - delivered).clamp(2.0, 5.5)
        }
        None => target_peak_g,
    };
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_delivered_g = None;

    while preview_count < maximum_previews {
        bottom_g = round_element_value(bottom_g.clamp(2.0, 5.5));
        let mut hold_degrees = LOOP_DEFAULT_HOLD_DEGREES;
        let mut profile = solve_shaped_full_loop_profile(
            design_speed_mps,
            bottom_g,
            top_g,
            hold_degrees,
            train,
            PROFILE_STEPS,
        )
        .map_err(|error| map_offline_error(design_speed_mps, error))?;
        for candidate_hold in [105.0, 90.0, 75.0, 135.0, 150.0] {
            if profile.minimum_radius() >= context.model.minimum_radius_metres + 0.15 {
                break;
            }
            if let Ok(wider) = solve_shaped_full_loop_profile(
                design_speed_mps,
                bottom_g,
                top_g,
                candidate_hold,
                train,
                PROFILE_STEPS,
            ) && wider.minimum_radius() > profile.minimum_radius()
            {
                hold_degrees = candidate_hold;
                profile = wider;
            }
        }
        let endpoint_x = profile.endpoint()[0];
        let profile_grid_half_tiles = (endpoint_x / grid_step_m).ceil().max(2.0) as u16;
        let span_m = f64::from(profile_grid_half_tiles) * grid_step_m;
        profile.append_level_straight((span_m - endpoint_x).max(0.0));
        profile.round_to_nine_decimal_places();
        let height_m = profile
            .resolved_points()
            .iter()
            .map(|point| point[2])
            .fold(f64::NEG_INFINITY, f64::max);

        let mut candidate_spec = ElementSpec::Loop {
            design_speed_mps,
            cars: context.train.cars,
            train_length_metres: context.train.length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g: target_peak_g,
            delivered_top_g: top_g,
            height_m,
            span_m,
            bottom_g,
            top_g,
            hold_degrees,
            profile_grid_half_tiles,
            diagonal,
            steel_structure,
            geometry_digest: 0,
            profile,
        };
        canonicalize_element_spec(&mut candidate_spec);
        candidate_spec.set_resolved_geometry_digest();
        let candidate = piece_from_spec(context.entry, &candidate_spec)?;
        let rail_radius_ok =
            candidate.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres;
        let preview = calibrate_piece(context, &candidate)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let delivered_peak_g = worst_peak_vertical_g(&preview.cars);
        let delivered_top_g = worst_inverted_vertical_g(&preview.cars);
        last_delivered_g = Some(delivered_peak_g);
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        let exceeds_model_feel = candidate_exceeds_design_targets(context.model, &preview.cars);

        if provisional {
            if let ElementSpec::Loop {
                delivered_peak_g: stored_peak,
                delivered_top_g: stored_top,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_top = round_element_value(delivered_top_g);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: None,
                }),
            });
        }

        if passes_budget
            && rail_radius_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
        {
            if let ElementSpec::Loop {
                delivered_peak_g: stored_peak,
                delivered_top_g: stored_top,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_top = round_element_value(delivered_top_g);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: None,
            });
        }

        if !rail_radius_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
        {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: target_peak_g,
                available_headroom_g: headroom_g.min(0.0),
            });
        }

        let next_bottom_g = match previous_attempt {
            Some((prev_bottom, prev_delivered)) => {
                let slope = (delivered_peak_g - prev_delivered) / (bottom_g - prev_bottom);
                if !slope.is_finite() || slope.abs() < 1e-4 {
                    bottom_g + (target_peak_g - delivered_peak_g) * 0.8
                } else {
                    bottom_g + (target_peak_g - delivered_peak_g) / slope
                }
            }
            None => bottom_g + (target_peak_g - delivered_peak_g),
        };
        previous_attempt = Some((bottom_g, delivered_peak_g));
        bottom_g = next_bottom_g;
    }

    Err(SolveRefusal::InfeasibleTarget {
        target_g: last_delivered_g.unwrap_or(target_peak_g),
        available_headroom_g: last_headroom_g,
    })
}

fn find_zero_g_flank_angle(
    design_speed_mps: f64,
    pull_g: f64,
    target_crest_seconds: f64,
    train: OfflineTrain,
) -> Result<f64, SolveRefusal> {
    let mut low = ZERO_G_MIN_FLANK_DEGREES;
    let mut high = ZERO_G_MAX_FLANK_DEGREES;
    let mut best: Option<(f64, f64)> = None;
    for _ in 0..ZERO_G_FLANK_SEARCH_STEPS {
        let flank_degrees = 0.5 * (low + high);
        let Ok((profile, crest_start, crest_end)) = solve_shaped_zero_g_hill_profile(
            design_speed_mps,
            pull_g,
            flank_degrees.to_radians(),
            train,
            128,
        ) else {
            high = flank_degrees;
            continue;
        };
        let Some(duration_s) = zero_g_crest_duration_seconds(
            &profile,
            crest_start,
            crest_end,
            design_speed_mps,
            train,
        ) else {
            high = flank_degrees;
            continue;
        };
        let error = (duration_s - target_crest_seconds).abs();
        if best
            .as_ref()
            .is_none_or(|(best_error, _)| error < *best_error)
        {
            best = Some((error, flank_degrees));
        }
        if duration_s < target_crest_seconds {
            low = flank_degrees;
        } else {
            high = flank_degrees;
        }
    }
    best.map(|(_, flank)| flank)
        .ok_or(SolveRefusal::InvalidGeometry)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_zero_g_roll(
    hand: Hand,
    peak_g: f64,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    if !peak_g.is_finite()
        || !(MINIMUM_INVERSION_PEAK_TARGET_G..=MAXIMUM_INVERSION_PEAK_TARGET_G).contains(&peak_g)
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: peak_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let target_peak_g = round_element_value(peak_g);
    let within_vibe = inversion_within_model_vibe(context.model, target_peak_g);
    let diagonal = context.entry.heading.index() % 2 != 0;
    let grid_step_m = horizontal_grid_step(context.entry.heading);
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };

    let seed = preview_seed.and_then(|candidate| {
        let ElementSpec::ZeroGRoll {
            delivered_peak_g,
            pull_g,
            ..
        } = candidate.spec
        else {
            return None;
        };
        let seed = candidate.calibration_seed.as_ref()?;
        Some((delivered_peak_g, pull_g, seed))
    });

    let mut preview_count = u8::from(seed.is_some());
    let mut previous_attempt = seed.map(|(delivered, pull_g, _)| (pull_g, delivered));
    let mut pull_g = match seed {
        Some((delivered, seed_pull, _)) => (seed_pull + target_peak_g - delivered).clamp(2.0, 5.5),
        None => target_peak_g,
    };
    let mut target_crest_seconds = ZERO_G_TARGET_CREST_SECONDS;
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_delivered_g = None;

    while preview_count < maximum_previews {
        pull_g = round_element_value(pull_g.clamp(2.0, 5.5));
        let flank_angle_degrees = round_element_value(find_zero_g_flank_angle(
            design_speed_mps,
            pull_g,
            target_crest_seconds,
            train,
        )?);
        let (mut profile, crest_start_m, crest_end_m) = solve_shaped_zero_g_hill_profile(
            design_speed_mps,
            pull_g,
            flank_angle_degrees.to_radians(),
            train,
            PROFILE_STEPS,
        )
        .map_err(|error| map_offline_error(design_speed_mps, error))?;
        let core_length = profile.length();
        let extension = ZERO_G_TWIST_EXTENSION_FRACTION * core_length;
        let twist_start_m = round_element_value((crest_start_m - extension).max(0.0));
        let twist_end_m = round_element_value((crest_end_m + extension).min(core_length));
        let endpoint_x = profile.endpoint()[0];
        let profile_grid_half_tiles = (endpoint_x / grid_step_m).ceil().max(2.0) as u16;
        let span_m = f64::from(profile_grid_half_tiles) * grid_step_m;
        profile.append_level_straight((span_m - endpoint_x).max(0.0));
        profile.round_to_nine_decimal_places();
        let height_m = profile
            .resolved_points()
            .iter()
            .map(|point| point[2])
            .fold(f64::NEG_INFINITY, f64::max);

        let mut candidate_spec = ElementSpec::ZeroGRoll {
            design_speed_mps,
            cars: context.train.cars,
            train_length_metres: context.train.length_metres,
            hand,
            target_peak_g,
            delivered_peak_g: target_peak_g,
            delivered_crest_g: 0.0,
            peak_roll_rate_deg_s: 0.0,
            height_m,
            span_m,
            pull_g,
            flank_angle_degrees,
            twist_ramp: ZERO_G_TWIST_RAMP,
            twist_start_m,
            twist_end_m: twist_end_m.min(profile.length()),
            profile_grid_half_tiles,
            diagonal,
            geometry_digest: 0,
            profile,
        };
        canonicalize_element_spec(&mut candidate_spec);
        candidate_spec.set_resolved_geometry_digest();
        let candidate = piece_from_spec(context.entry, &candidate_spec)?;
        let rail_radius_ok =
            candidate.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres;
        let preview = calibrate_piece(context, &candidate)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let delivered_peak_g = worst_peak_vertical_g(&preview.cars);
        let delivered_crest_g = worst_inverted_vertical_g(&preview.cars);
        let peak_roll_rate_deg_s = worst_roll_rate_deg_s(&preview.cars);
        last_delivered_g = Some(delivered_peak_g);
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        let exceeds_model_feel = candidate_exceeds_design_targets(context.model, &preview.cars);
        let roll_rate_ok = peak_roll_rate_deg_s <= MAXIMUM_SHAPED_ROLL_RATE_DEG_S;
        let crest_ok = delivered_crest_g.abs() <= ZERO_G_MAX_CREST_ABS_G;

        if provisional {
            if let ElementSpec::ZeroGRoll {
                delivered_peak_g: stored_peak,
                delivered_crest_g: stored_crest,
                peak_roll_rate_deg_s: stored_roll_rate,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_crest = round_element_value(delivered_crest_g);
                *stored_roll_rate = round_element_value(peak_roll_rate_deg_s);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: None,
                }),
            });
        }

        if passes_budget
            && rail_radius_ok
            && roll_rate_ok
            && crest_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
        {
            if let ElementSpec::ZeroGRoll {
                delivered_peak_g: stored_peak,
                delivered_crest_g: stored_crest,
                peak_roll_rate_deg_s: stored_roll_rate,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_crest = round_element_value(delivered_crest_g);
                *stored_roll_rate = round_element_value(peak_roll_rate_deg_s);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: None,
            });
        }

        if !roll_rate_ok {
            target_crest_seconds *= (peak_roll_rate_deg_s / 180.0).clamp(1.08, 1.35);
            previous_attempt = None;
            continue;
        }

        let next_pull_g = match previous_attempt {
            Some((prev_pull, prev_delivered)) => {
                let slope = (delivered_peak_g - prev_delivered) / (pull_g - prev_pull);
                if !slope.is_finite() || slope.abs() < 1e-4 {
                    pull_g + (target_peak_g - delivered_peak_g) * 0.8
                } else {
                    pull_g + (target_peak_g - delivered_peak_g) / slope
                }
            }
            None => pull_g + (target_peak_g - delivered_peak_g),
        };
        previous_attempt = Some((pull_g, delivered_peak_g));
        pull_g = next_pull_g;
    }

    Err(SolveRefusal::InfeasibleTarget {
        target_g: last_delivered_g.unwrap_or(target_peak_g),
        available_headroom_g: last_headroom_g,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_raven_turn(
    hand: Hand,
    peak_g: f64,
    top_feel: LoopTopFeel,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    if !peak_g.is_finite()
        || !(MINIMUM_INVERSION_PEAK_TARGET_G..=MAXIMUM_INVERSION_PEAK_TARGET_G).contains(&peak_g)
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: peak_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let target_peak_g = round_element_value(peak_g);
    let target_top_g = round_element_value(top_feel.target_g());
    let within_vibe = inversion_within_model_vibe(context.model, target_peak_g);
    let diagonal = context.entry.heading.index() % 2 != 0;
    let grid_step_m = horizontal_grid_step(context.entry.heading);
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };

    let seed = preview_seed.and_then(|candidate| {
        let ElementSpec::RavenTurn {
            delivered_peak_g,
            bottom_g,
            top_g,
            ..
        } = candidate.spec
        else {
            return None;
        };
        let seed = candidate.calibration_seed.as_ref()?;
        let (crest_low_g, crest_high_g) = crest_top_band_g(&seed.preview.cars)?;
        Some((
            delivered_peak_g,
            bottom_g,
            top_g + target_top_g - 0.5 * (crest_low_g + crest_high_g),
        ))
    });

    let mut preview_count = u8::from(seed.is_some());
    let mut previous_attempt = seed.map(|(delivered, bottom_g, _)| (bottom_g, delivered));
    let mut bottom_g = match seed {
        Some((delivered, seed_bottom, _)) => {
            (seed_bottom + target_peak_g - delivered).clamp(2.0, 5.5)
        }
        None => target_peak_g,
    };
    let mut crest_top_g = seed.map_or(
        target_top_g + RAVEN_CREST_SCHEDULE_OFFSET_G,
        |(_, _, seed_top_g)| seed_top_g,
    );
    let mut roll_length_scale = 3.2_f64;
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_delivered_g = None;
    let mut last_crest_band_g = None;

    while preview_count < maximum_previews {
        bottom_g = round_element_value(bottom_g.clamp(2.0, 5.5));
        crest_top_g = round_element_value(crest_top_g);
        let hold_degrees = LOOP_DEFAULT_HOLD_DEGREES;
        let half_loop = solve_shaped_raven_half_loop_profile(
            design_speed_mps,
            bottom_g,
            crest_top_g,
            hold_degrees,
            train,
            PROFILE_STEPS,
        )
        .map_err(|error| map_offline_error(design_speed_mps, error))?;
        let apex = half_loop.endpoint();
        let height_m = round_element_value(apex[1]);
        let crest_speed_mps = (crate::math::powi(design_speed_mps, 2) - 2.0 * GRAVITY * height_m)
            .max(16.0)
            .sqrt();
        let exit_dy_half_tiles = match hand {
            Hand::Left => -4,
            Hand::Right => 4,
        };
        let roll_lateral_m = f64::from(exit_dy_half_tiles) * grid_step_m;
        let roll_out_for_speed = |roll_speed_mps: f64| {
            let min_roll_dx_m = (roll_length_scale * roll_speed_mps).max(38.0);
            let exit_dx_half_tiles = ((apex[0] - min_roll_dx_m) / grid_step_m).floor() as i32;
            let roll_dx_m = apex[0] - f64::from(exit_dx_half_tiles) * grid_step_m;
            (
                exit_dx_half_tiles,
                roll_dx_m,
                crate::math::hypot(roll_dx_m, roll_lateral_m),
            )
        };
        let crest_dive_drop_m = |roll_scale: f64| {
            let crest_fraction = RAVEN_CREST_TAPER_SECONDS * crest_speed_mps / roll_scale;
            raven_crest_dive_rail_drop_m(&half_loop, roll_scale, crest_fraction)
                .ok_or(SolveRefusal::InvalidGeometry)
        };
        // The train speeds up down the dive, so size the roll for its mid-roll speed.
        let crest_roll_scale = roll_out_for_speed(crest_speed_mps).2;
        let mid_roll_speed_mps = (crate::math::powi(crest_speed_mps, 2)
            + GRAVITY * crest_dive_drop_m(crest_roll_scale)?)
        .sqrt();
        let (exit_dx_half_tiles, roll_dx_m, roll_scale) = roll_out_for_speed(mid_roll_speed_mps);
        // A low crest shortens the taper to keep the exit at least 6 m up, down to a firm limit.
        let exit_dz_half_tiles =
            (((apex[1] - crest_dive_drop_m(roll_scale)?) / 2.0).floor() as i32).max(3);
        let crest_dive = raven_crest_dive(
            &half_loop,
            roll_scale,
            apex[1] - f64::from(exit_dz_half_tiles) * 2.0,
        )
        .ok_or(SolveRefusal::InvalidGeometry)?;
        if crest_dive.crest_fraction() * roll_scale / crest_speed_mps
            < MINIMUM_RAVEN_CREST_TAPER_SECONDS
        {
            return Err(SolveRefusal::InvalidGeometry);
        }
        let span_m = roll_dx_m.max(apex[0]);

        let mut candidate_spec = ElementSpec::RavenTurn {
            design_speed_mps,
            cars: context.train.cars,
            train_length_metres: context.train.length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g: target_peak_g,
            delivered_top_g: target_top_g,
            peak_roll_rate_deg_s: 0.0,
            height_m,
            span_m,
            bottom_g,
            top_g: crest_top_g,
            hold_degrees,
            exit_dx_half_tiles,
            exit_dy_half_tiles,
            exit_dz_half_tiles,
            roll_dx_m,
            roll_lateral_m,
            diagonal,
            geometry_digest: 0,
            half_loop,
        };
        canonicalize_element_spec(&mut candidate_spec);
        candidate_spec.set_resolved_geometry_digest();
        let candidate = piece_from_spec(context.entry, &candidate_spec)?;
        let rail_radius_ok =
            candidate.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres;
        let preview = calibrate_piece(context, &candidate)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let delivered_peak_g = worst_peak_vertical_g(&preview.cars);
        let (crest_low_g, crest_high_g) =
            crest_top_band_g(&preview.cars).ok_or(SolveRefusal::InvalidGeometry)?;
        let delivered_top_g =
            if (crest_low_g - target_top_g).abs() >= (crest_high_g - target_top_g).abs() {
                crest_low_g
            } else {
                crest_high_g
            };
        let crest_half_spread_g = 0.5 * (crest_high_g - crest_low_g);
        let top_feel_ok = raven_top_feel_reached(&preview.cars, target_top_g);
        let peak_roll_rate_deg_s = worst_roll_rate_deg_s(&preview.cars);
        last_delivered_g = Some(delivered_peak_g);
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        let exceeds_model_feel = candidate_exceeds_design_targets(context.model, &preview.cars);
        let roll_rate_ok = peak_roll_rate_deg_s <= MAXIMUM_SHAPED_ROLL_RATE_DEG_S;

        if provisional {
            if let ElementSpec::RavenTurn {
                delivered_peak_g: stored_peak,
                delivered_top_g: stored_top,
                peak_roll_rate_deg_s: stored_roll_rate,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_top = round_element_value(delivered_top_g);
                *stored_roll_rate = round_element_value(peak_roll_rate_deg_s);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: None,
                }),
            });
        }

        if passes_budget
            && rail_radius_ok
            && roll_rate_ok
            && top_feel_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
        {
            if let ElementSpec::RavenTurn {
                delivered_peak_g: stored_peak,
                delivered_top_g: stored_top,
                peak_roll_rate_deg_s: stored_roll_rate,
                ..
            } = &mut candidate_spec
            {
                *stored_peak = round_element_value(delivered_peak_g);
                *stored_top = round_element_value(delivered_top_g);
                *stored_roll_rate = round_element_value(peak_roll_rate_deg_s);
            }
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            let piece = piece_from_spec(context.entry, &candidate_spec)?;
            return Ok(SolvedElement {
                piece,
                spec: candidate_spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: None,
            });
        }

        // Cars cross the crest at different train speeds, and the band widens as the crest
        // tightens, so a band too wide while centred at or below the target cannot be fixed.
        let crest_centre_g = 0.5 * (crest_low_g + crest_high_g);
        if crest_half_spread_g > MAXIMUM_RAVEN_TOP_FEEL_ERROR_G && crest_centre_g <= target_top_g {
            return Err(SolveRefusal::TopFeelOutOfReach {
                target_top_g,
                lowest_car_top_g: crest_low_g,
                highest_car_top_g: crest_high_g,
            });
        }
        last_crest_band_g = (passes_budget
            && rail_radius_ok
            && roll_rate_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G)
            .then_some((crest_low_g, crest_high_g));
        crest_top_g += target_top_g - crest_centre_g;

        if !roll_rate_ok {
            roll_length_scale *= (peak_roll_rate_deg_s / 175.0).clamp(1.1, 1.4);
            previous_attempt = None;
            continue;
        }

        if !rail_radius_ok
            && (delivered_peak_g - target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
        {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: target_peak_g,
                available_headroom_g: headroom_g.min(0.0),
            });
        }

        let next_bottom_g = match previous_attempt {
            Some((prev_bottom, prev_delivered)) => {
                let slope = (delivered_peak_g - prev_delivered) / (bottom_g - prev_bottom);
                if !slope.is_finite() || slope.abs() < 1e-4 {
                    bottom_g + (target_peak_g - delivered_peak_g) * 0.8
                } else {
                    bottom_g + (target_peak_g - delivered_peak_g) / slope
                }
            }
            None => bottom_g + (target_peak_g - delivered_peak_g),
        };
        previous_attempt = Some((bottom_g, delivered_peak_g));
        bottom_g = next_bottom_g;
    }

    // The last try met every limit but the top feel, so the top feel is what is out of reach.
    if let Some((lowest_car_top_g, highest_car_top_g)) = last_crest_band_g {
        return Err(SolveRefusal::TopFeelOutOfReach {
            target_top_g,
            lowest_car_top_g,
            highest_car_top_g,
        });
    }
    Err(SolveRefusal::InfeasibleTarget {
        target_g: last_delivered_g.unwrap_or(target_peak_g),
        available_headroom_g: last_headroom_g,
    })
}

pub(super) fn zero_g_roll_exit(entry: TrackState, span_m: f64) -> Result<TrackState, SolveRefusal> {
    let grid_step_m = horizontal_grid_step(entry.heading);
    let half_tiles = (span_m / grid_step_m).round();
    if !half_tiles.is_finite()
        || half_tiles < 1.0
        || (span_m / grid_step_m - half_tiles).abs() > 1e-5
    {
        return Err(SolveRefusal::InvalidContext);
    }
    let (forward_x, forward_y) = grid_heading_delta(entry.heading, half_tiles as i32);
    let mut exit = entry;
    exit.x += forward_x;
    exit.y += forward_y;
    exit.slope = Slope::Level;
    exit.bank = Bank::Level;
    exit.horizontal_curvature = HorizontalCurvature::Straight;
    exit.vertical_curvature = VerticalCurvature::Straight;
    Ok(exit)
}

pub(super) fn loop_exit(
    entry: TrackState,
    hand: Hand,
    span_m: f64,
) -> Result<TrackState, SolveRefusal> {
    let grid_step_m = horizontal_grid_step(entry.heading);
    let half_tiles = (span_m / grid_step_m).round();
    if !half_tiles.is_finite()
        || half_tiles < 1.0
        || (span_m / grid_step_m - half_tiles).abs() > 1e-5
    {
        return Err(SolveRefusal::InvalidContext);
    }
    let forward_tiles = half_tiles as i32;
    let side_tiles = match hand {
        Hand::Left => 2,
        Hand::Right => -2,
    };
    let (forward_x, forward_y) = grid_heading_delta(entry.heading, forward_tiles);
    let (side_x, side_y) =
        grid_heading_delta(Heading::from_index(entry.heading.index() + 2), side_tiles);
    let mut exit = entry;
    exit.x += forward_x + side_x;
    exit.y += forward_y + side_y;
    exit.slope = Slope::Level;
    exit.bank = Bank::Level;
    exit.horizontal_curvature = HorizontalCurvature::Straight;
    exit.vertical_curvature = VerticalCurvature::Straight;
    Ok(exit)
}

pub(super) fn raven_turn_exit(
    entry: TrackState,
    exit_dx_half_tiles: i32,
    exit_dy_half_tiles: i32,
    exit_dz_half_tiles: i32,
) -> Result<TrackState, SolveRefusal> {
    if exit_dz_half_tiles < 3 {
        return Err(SolveRefusal::InvalidContext);
    }
    let (forward_x, forward_y) = grid_heading_delta(entry.heading, exit_dx_half_tiles);
    let (side_x, side_y) = grid_heading_delta(
        Heading::from_index(entry.heading.index() + 2),
        exit_dy_half_tiles,
    );
    let mut exit = entry;
    exit.x += forward_x + side_x;
    exit.y += forward_y + side_y;
    exit.z += exit_dz_half_tiles;
    exit.heading = Heading::from_index(entry.heading.index() + 4);
    exit.slope = Slope::Level;
    exit.bank = Bank::Level;
    exit.horizontal_curvature = HorizontalCurvature::Straight;
    exit.vertical_curvature = VerticalCurvature::Straight;
    Ok(exit)
}

pub(super) fn inversion_intent_matches_spec(
    intent: &ElementIntent,
    spec: &ElementSpec,
    model: &CoasterModel,
    entry: TrackState,
) -> bool {
    let diagonal = entry.heading.index() % 2 != 0;
    match (intent, spec) {
        (
            ElementIntent::Loop {
                hand,
                peak_g,
                top_feel,
            },
            ElementSpec::Loop {
                hand: resolved_hand,
                top_feel: resolved_top_feel,
                target_peak_g,
                delivered_peak_g,
                height_m,
                span_m,
                steel_structure,
                diagonal: spec_diagonal,
                profile,
                ..
            },
        ) => {
            let expected_steel = model.track_material == Material::Wood;
            let expected_span = profile.endpoint()[0];
            let resolved_height = profile
                .resolved_points()
                .iter()
                .map(|point| point[2])
                .fold(f64::NEG_INFINITY, f64::max);
            hand == resolved_hand
                && top_feel == resolved_top_feel
                && *steel_structure == expected_steel
                && *spec_diagonal == diagonal
                && model.allowed_inversions.contains(&Inversion::VerticalLoop)
                && (*target_peak_g - round_element_value(*peak_g)).abs() <= 1e-6
                && delivered_peak_g.is_finite()
                && (*delivered_peak_g - *target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
                && stored_value_matches(expected_span, *span_m)
                && stored_value_matches(resolved_height, *height_m)
                && *height_m > 0.0
                && *span_m > 0.0
        }
        (
            ElementIntent::ZeroGRoll { hand, peak_g },
            ElementSpec::ZeroGRoll {
                hand: resolved_hand,
                target_peak_g,
                delivered_peak_g,
                delivered_crest_g,
                peak_roll_rate_deg_s,
                height_m,
                span_m,
                diagonal: spec_diagonal,
                profile,
                ..
            },
        ) => {
            let expected_span = profile.endpoint()[0];
            let resolved_height = profile
                .resolved_points()
                .iter()
                .map(|point| point[2])
                .fold(f64::NEG_INFINITY, f64::max);
            hand == resolved_hand
                && *spec_diagonal == diagonal
                && model.allowed_inversions.contains(&Inversion::HeartlineRoll)
                && (*target_peak_g - round_element_value(*peak_g)).abs() <= 1e-6
                && delivered_peak_g.is_finite()
                && (*delivered_peak_g - *target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
                && delivered_crest_g.is_finite()
                && delivered_crest_g.abs() <= ZERO_G_MAX_CREST_ABS_G
                && peak_roll_rate_deg_s.is_finite()
                && *peak_roll_rate_deg_s <= MAXIMUM_SHAPED_ROLL_RATE_DEG_S
                && stored_value_matches(expected_span, *span_m)
                && stored_value_matches(resolved_height, *height_m)
                && *height_m > 0.0
                && *span_m > 0.0
        }
        (
            ElementIntent::RavenTurn {
                hand,
                peak_g,
                top_feel,
            },
            ElementSpec::RavenTurn {
                hand: resolved_hand,
                top_feel: resolved_top_feel,
                target_peak_g,
                delivered_peak_g,
                peak_roll_rate_deg_s,
                height_m,
                exit_dz_half_tiles,
                diagonal: spec_diagonal,
                half_loop,
                ..
            },
        ) => {
            let expected_height = f64::from(*exit_dz_half_tiles) * 2.0;
            hand == resolved_hand
                && top_feel == resolved_top_feel
                && *spec_diagonal == diagonal
                && model.allowed_inversions.contains(&Inversion::VerticalLoop)
                && model.allowed_inversions.contains(&Inversion::Corkscrew)
                && (*target_peak_g - round_element_value(*peak_g)).abs() <= 1e-6
                && delivered_peak_g.is_finite()
                && (*delivered_peak_g - *target_peak_g).abs() <= MAXIMUM_INVERSION_PEAK_ERROR_G
                && peak_roll_rate_deg_s.is_finite()
                && *peak_roll_rate_deg_s <= MAXIMUM_SHAPED_ROLL_RATE_DEG_S
                && stored_value_matches(expected_height, *height_m)
                && (half_loop.endpoint()[1] - expected_height).abs() <= 0.25
                && *height_m > 0.0
        }
        _ => false,
    }
}
