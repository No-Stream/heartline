use crate::math::{atan2, hypot, sin_cos};
use crate::physics::{EnvelopeEvaluator, GForces, GRAVITY, TIME_STEP};
use crate::track::curves::Vec3;
use crate::track::{Hand, Heading};
use std::f64::consts::FRAC_PI_4;

const PULL_G: f64 = 3.3;
const MAXIMUM_HYPER_VERTICAL_G: f64 = 4.0;
const MINIMUM_HYPER_VERTICAL_G: f64 = -1.0;
const MAXIMUM_HYPER_LATERAL_G: f64 = 1.5;
const FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_FULLY_BANKED_LATERAL_G: f64 = 0.15;
const MAXIMUM_INTEGRATION_STEP_M: f64 = 1.0;
const MAXIMUM_INTEGRATION_STEPS: usize = 2_048;
const MAXIMUM_ENTRY_SPEED_MPS: f64 = 44.0;
const MAXIMUM_NEWTON_STEPS: usize = 18;
const MAXIMUM_GRID_HEIGHT_STEPS: usize = 18;
const APEX_HOLD_LENGTH_M: f64 = 6.0;
const NINE_DECIMAL_SCALE: f64 = 1_000_000_000.0;
const FIXED_HEIGHT_TOLERANCE_M: f64 = 1e-6;
const GRID_POSITION_TOLERANCE_M: f64 = 1e-7;
const ENTRY_HEADINGS: [Heading; 8] = [
    Heading::East,
    Heading::NorthEast,
    Heading::North,
    Heading::NorthWest,
    Heading::West,
    Heading::SouthWest,
    Heading::South,
    Heading::SouthEast,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrestTurnSweep {
    Quarter,
    Half,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CrestTurnSizeHint {
    Auto,
    FixedHeight(f64),
}

impl CrestTurnSweep {
    pub fn radians(self) -> f64 {
        match self {
            Self::Quarter => std::f64::consts::FRAC_PI_2,
            Self::Half => std::f64::consts::PI,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrestApexFeel {
    Floaty,
    Neutral,
    PressedIn,
}

impl CrestApexFeel {
    pub fn vertical_g(self) -> f64 {
        match self {
            Self::Floaty => 0.3,
            Self::Neutral => 1.0,
            Self::PressedIn => 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrestTurnRefusal {
    InfeasibleTarget,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrestTurnKnot {
    pub distance_m: f64,
    pub position: Vec3,
    pub tangent: Vec3,
    pub seat_up: Vec3,
    pub curvature_per_m: Vec3,
    pub bank_degrees: f64,
    pub vertical_g: f64,
    pub lateral_g: f64,
    pub longitudinal_g: f64,
    pub speed_mps: f64,
}

impl CrestTurnKnot {
    fn point_mass_normal_force_g(self) -> f64 {
        let world_up = Vec3::new(0.0, 0.0, 1.0);
        let vertical_projection = world_up - self.tangent * world_up.dot(self.tangent);
        let reaction_normal = self.curvature_per_m * (self.speed_mps * self.speed_mps)
            + vertical_projection * GRAVITY;
        reaction_normal.dot(self.seat_up) / GRAVITY
    }

    #[cfg(test)]
    fn point_mass_force_g(self) -> f64 {
        self.point_mass_normal_force_g()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrestTurnGeometry {
    pub knots: Vec<CrestTurnKnot>,
    pub apex_index: usize,
    pub sweep: CrestTurnSweep,
    pub hand: Hand,
    pub entry_speed_mps: f64,
    pub apex_bank_degrees: f64,
    pub apex_vertical_g: f64,
    pub pull_g: f64,
    pub ramp_length_m: f64,
    pub pull_hold_length_m: f64,
    pub ease_length_m: f64,
    pub apex_hold_length_m: f64,
    pub target_height_m: Option<f64>,
    pub integration_target_height_m: Option<f64>,
    pub half_length_m: f64,
    pub placement_heading: Heading,
    pub placement_scale: f64,
}

impl CrestTurnGeometry {
    pub fn apex(&self) -> CrestTurnKnot {
        self.knots[self.apex_index]
    }

    pub fn pull_hold_end_m(&self) -> f64 {
        self.ramp_length_m + self.pull_hold_length_m
    }

    pub fn apex_heading_radians(&self) -> f64 {
        heading_radians(self.apex().tangent)
    }

    pub fn exit_heading_radians(&self) -> f64 {
        heading_radians(self.knots[self.knots.len() - 1].tangent)
    }

    pub fn exit_position(&self) -> Vec3 {
        self.knots[self.knots.len() - 1].position
    }

    fn exit_is_on_grid(&self) -> bool {
        let exit_position = self.exit_position();
        [exit_position.x, exit_position.y]
            .into_iter()
            .all(|coordinate| {
                ((coordinate / 2.0) - (coordinate / 2.0).round()).abs() * 2.0
                    <= GRID_POSITION_TOLERANCE_M
            })
    }

    pub fn grid_scale_for_heading(&self, heading: Heading) -> f64 {
        let current_angle = f64::from(self.placement_heading.index()) * FRAC_PI_4;
        let unplaced_exit =
            rotate_around_world_up(self.exit_position(), -current_angle) / self.placement_scale;
        grid_scale_for_exit(unplaced_exit, heading)
    }

    pub fn placed_on_grid(&self, heading: Heading) -> Result<Self, CrestTurnRefusal> {
        self.try_placed_on_grid(heading)
    }

    pub fn try_placed_on_grid(&self, heading: Heading) -> Result<Self, CrestTurnRefusal> {
        let base = self.rebuild_from_recipe()?;
        if let Some(target_height_m) = self.target_height_m {
            let mut scale_guess = grid_scale_for_exit(base.exit_position(), heading);
            for _ in 0..MAXIMUM_GRID_HEIGHT_STEPS {
                let integration_target_height_m =
                    round_nine_decimal_places(target_height_m / scale_guess);
                let mut candidate = solve_crest_turn_internal(
                    self.entry_speed_mps,
                    self.sweep,
                    self.hand,
                    self.apex_bank_degrees,
                    feel_from_vertical_g(self.apex_vertical_g),
                    CrestTurnSizeHint::FixedHeight(integration_target_height_m),
                    false,
                )?;
                candidate.target_height_m = Some(target_height_m);
                let scale = grid_scale_for_exit(candidate.exit_position(), heading);
                let placed = candidate.apply_grid_transform(heading, scale);
                if (placed.apex().position.z - target_height_m).abs() <= FIXED_HEIGHT_TOLERANCE_M
                    && placed.exit_is_on_grid()
                    && placed.passes_hyper_and_comfort_limits()
                    && (placed.apex().vertical_g - self.apex_vertical_g).abs() <= 0.2
                {
                    return Ok(placed);
                }
                scale_guess = scale;
            }
            return Err(CrestTurnRefusal::InfeasibleTarget);
        }

        let scale = grid_scale_for_exit(base.exit_position(), heading);
        let placed = base.apply_grid_transform(heading, scale);
        if !placed.exit_is_on_grid()
            || !placed.passes_hyper_and_comfort_limits()
            || (placed.apex().vertical_g - self.apex_vertical_g).abs() > 0.2
        {
            return Err(CrestTurnRefusal::InfeasibleTarget);
        }
        Ok(placed)
    }

    pub fn replay(&self) -> Result<Self, CrestTurnRefusal> {
        let mut replayed = self.rebuild_from_recipe()?;
        if self.placement_heading == Heading::East && (self.placement_scale - 1.0).abs() < 1e-15 {
            return Ok(replayed);
        }
        replayed.target_height_m = self.target_height_m;
        Ok(replayed.apply_grid_transform(self.placement_heading, self.placement_scale))
    }

    fn rebuild_from_recipe(&self) -> Result<Self, CrestTurnRefusal> {
        build_geometry_from_recipe(CrestTurnRecipe {
            sweep: self.sweep,
            schedule: CrestTurnSchedule {
                entry_speed_mps: self.entry_speed_mps,
                hand: self.hand,
                ramp_length_m: self.ramp_length_m,
                pull_hold_length_m: self.pull_hold_length_m,
                ease_length_m: self.ease_length_m,
                apex_bank_degrees: self.apex_bank_degrees,
                apex_vertical_g: self.apex_vertical_g,
            },
            apex_hold_length_m: self.apex_hold_length_m,
            target_height_m: self.target_height_m,
            integration_target_height_m: self.integration_target_height_m,
        })
    }

    fn apply_grid_transform(&self, heading: Heading, scale: f64) -> Self {
        let scale = round_nine_decimal_places(scale);
        let heading_angle = f64::from(heading.index()) * FRAC_PI_4;
        let mut placed = self.clone();
        for knot in &mut placed.knots {
            knot.position = rotate_around_world_up(knot.position, heading_angle) * scale;
            knot.tangent = normalize(rotate_around_world_up(knot.tangent, heading_angle));
            knot.seat_up = normalize(rotate_around_world_up(knot.seat_up, heading_angle));
            knot.curvature_per_m =
                rotate_around_world_up(knot.curvature_per_m, heading_angle) / scale;
            knot.distance_m *= scale;
            knot.speed_mps = speed_at_height(self.entry_speed_mps, knot.position.z)
                .expect("uniform grid scaling preserves a positive speed profile");
            knot.vertical_g = knot.point_mass_normal_force_g();
            knot.lateral_g = lateral_force_g(*knot);
        }
        placed.placement_heading = heading;
        placed.placement_scale = scale;
        placed
    }

    pub fn peak_roll_rate_degrees_per_second(&self) -> f64 {
        self.knots
            .windows(2)
            .map(|pair| {
                let distance_m = pair[1].distance_m - pair[0].distance_m;
                ((pair[1].bank_degrees - pair[0].bank_degrees) / distance_m).abs()
                    * pair[0].speed_mps
            })
            .fold(0.0, f64::max)
    }

    pub fn passes_hyper_and_comfort_limits(&self) -> bool {
        let maximum_vertical_g = MAXIMUM_HYPER_VERTICAL_G - FORCE_HEADROOM_G;
        let minimum_vertical_g = MINIMUM_HYPER_VERTICAL_G + FORCE_HEADROOM_G;
        let maximum_lateral_g =
            MAXIMUM_FULLY_BANKED_LATERAL_G.min(MAXIMUM_HYPER_LATERAL_G - FORCE_HEADROOM_G);
        let pointwise_forces_pass = self.knots.iter().all(|knot| {
            let vertical_g = knot.point_mass_normal_force_g();
            vertical_g >= minimum_vertical_g
                && vertical_g <= maximum_vertical_g
                && lateral_force_g(*knot).abs() <= maximum_lateral_g
        });
        pointwise_forces_pass
            && self.passes_astm_comfort_envelope()
            && self.peak_roll_rate_degrees_per_second()
                <= crate::track::Piece::MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND
    }

    fn passes_astm_comfort_envelope(&self) -> bool {
        let mut evaluator = EnvelopeEvaluator::new(TIME_STEP);
        let mut knot_times_seconds = Vec::with_capacity(self.knots.len());
        let mut elapsed_seconds = 0.0;
        knot_times_seconds.push(elapsed_seconds);
        for pair in self.knots.windows(2) {
            let distance_m = pair[1].distance_m - pair[0].distance_m;
            let average_speed_mps = (pair[0].speed_mps + pair[1].speed_mps) * 0.5;
            elapsed_seconds += distance_m / average_speed_mps;
            knot_times_seconds.push(elapsed_seconds);
        }

        let mut segment_index = 0;
        let sample_count = (elapsed_seconds / TIME_STEP).ceil() as usize;
        for sample_index in 0..=sample_count {
            let sample_time_seconds = (sample_index as f64 * TIME_STEP).min(elapsed_seconds);
            while segment_index + 1 < knot_times_seconds.len() - 1
                && knot_times_seconds[segment_index + 1] < sample_time_seconds
            {
                segment_index += 1;
            }
            let segment_duration =
                knot_times_seconds[segment_index + 1] - knot_times_seconds[segment_index];
            let progress = ((sample_time_seconds - knot_times_seconds[segment_index])
                / segment_duration)
                .clamp(0.0, 1.0);
            let start = self.knots[segment_index];
            let end = self.knots[segment_index + 1];
            let vertical_g = start.point_mass_normal_force_g()
                + (end.point_mass_normal_force_g() - start.point_mass_normal_force_g()) * progress;
            let lateral_g =
                lateral_force_g(start) + (lateral_force_g(end) - lateral_force_g(start)) * progress;
            if !evaluator
                .update(GForces {
                    vertical: vertical_g,
                    lateral: lateral_g,
                    // The solver uses lossless point-mass energy, so tangential seat force is zero.
                    longitudinal: start.longitudinal_g
                        + (end.longitudinal_g - start.longitudinal_g) * progress,
                })
                .within_limits
            {
                return false;
            }
        }
        true
    }
}

/// Solves the entry-to-apex half of a symmetric cresting turn using point-mass forces only.
/// The train simulator later owns car-by-car force calibration and acceptance.
pub fn solve_crest_turn(
    entry_speed_mps: f64,
    sweep: CrestTurnSweep,
    hand: Hand,
    apex_bank_degrees: f64,
    apex_feel: CrestApexFeel,
) -> Result<CrestTurnGeometry, CrestTurnRefusal> {
    solve_crest_turn_with_size(
        entry_speed_mps,
        sweep,
        hand,
        apex_bank_degrees,
        apex_feel,
        CrestTurnSizeHint::Auto,
    )
}

pub fn solve_crest_turn_with_size(
    entry_speed_mps: f64,
    sweep: CrestTurnSweep,
    hand: Hand,
    apex_bank_degrees: f64,
    apex_feel: CrestApexFeel,
    size: CrestTurnSizeHint,
) -> Result<CrestTurnGeometry, CrestTurnRefusal> {
    solve_crest_turn_internal(
        entry_speed_mps,
        sweep,
        hand,
        apex_bank_degrees,
        apex_feel,
        size,
        true,
    )
}

fn solve_crest_turn_internal(
    entry_speed_mps: f64,
    sweep: CrestTurnSweep,
    hand: Hand,
    apex_bank_degrees: f64,
    apex_feel: CrestApexFeel,
    size: CrestTurnSizeHint,
    validate_grid_placements: bool,
) -> Result<CrestTurnGeometry, CrestTurnRefusal> {
    let entry_speed_mps = round_nine_decimal_places(entry_speed_mps);
    let apex_bank_degrees = round_nine_decimal_places(apex_bank_degrees);
    if !entry_speed_mps.is_finite()
        || entry_speed_mps <= 0.0
        || entry_speed_mps > MAXIMUM_ENTRY_SPEED_MPS
        || !apex_bank_degrees.is_finite()
        || !(90.0..=120.0).contains(&apex_bank_degrees)
    {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }

    let energy_height_m = entry_speed_mps * entry_speed_mps / (2.0 * GRAVITY);
    if !energy_height_m.is_finite() {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    let ramp_length_m = round_nine_decimal_places((energy_height_m * 0.08).clamp(3.0, 8.0));
    let target_heading = hand_sign(hand) * sweep.radians() / 2.0;
    let (pull_hold_fraction, ease_fraction) = match sweep {
        CrestTurnSweep::Quarter => (0.14, 0.77),
        CrestTurnSweep::Half => (0.22, 1.1),
    };
    let target_height_m = match size {
        CrestTurnSizeHint::Auto => None,
        CrestTurnSizeHint::FixedHeight(height_m) => {
            let height_m = round_nine_decimal_places(height_m);
            if !height_m.is_finite() || height_m <= 0.0 || height_m >= energy_height_m {
                return Err(CrestTurnRefusal::InfeasibleTarget);
            }
            Some(height_m)
        }
    };
    let schedule = CrestTurnSchedule {
        entry_speed_mps,
        hand,
        ramp_length_m,
        pull_hold_length_m: energy_height_m * pull_hold_fraction,
        ease_length_m: energy_height_m * ease_fraction,
        apex_bank_degrees,
        apex_vertical_g: apex_feel.vertical_g(),
    };
    let context = CrestTurnSolveContext {
        schedule,
        energy_height_m,
        target_heading,
    };
    let (pull_hold_length_m, ease_length_m, apex_hold_length_m) = match target_height_m {
        None => {
            let (pull_hold_length_m, ease_length_m) =
                solve_auto_lengths(context, pull_hold_fraction, ease_fraction)?;
            (pull_hold_length_m, ease_length_m, APEX_HOLD_LENGTH_M)
        }
        Some(target_height_m) => solve_fixed_height_lengths(
            context,
            target_height_m,
            [
                energy_height_m * pull_hold_fraction,
                energy_height_m * ease_fraction,
                APEX_HOLD_LENGTH_M,
            ],
        )?,
    };
    let pull_hold_length_m = round_nine_decimal_places(pull_hold_length_m);
    let ease_length_m = round_nine_decimal_places(ease_length_m);
    let apex_hold_length_m = round_nine_decimal_places(apex_hold_length_m);
    let geometry = build_geometry_from_recipe(CrestTurnRecipe {
        sweep,
        schedule: CrestTurnSchedule {
            pull_hold_length_m,
            ease_length_m,
            ..schedule
        },
        apex_hold_length_m,
        target_height_m,
        integration_target_height_m: target_height_m,
    })?;
    if !geometry.passes_hyper_and_comfort_limits() {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    if validate_grid_placements
        && target_height_m.is_none()
        && ENTRY_HEADINGS.into_iter().any(|heading| {
            geometry
                .try_placed_on_grid(heading)
                .map_or(true, |placed_geometry| {
                    !placed_geometry.exit_is_on_grid()
                        || !placed_geometry.passes_hyper_and_comfort_limits()
                        || (placed_geometry.apex().vertical_g - apex_feel.vertical_g()).abs() > 0.2
                        || placed_geometry.target_height_m.is_some_and(|target| {
                            (placed_geometry.apex().position.z - target).abs() > 1e-6
                        })
                })
        })
    {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    Ok(geometry)
}

fn build_geometry_from_recipe(
    recipe: CrestTurnRecipe,
) -> Result<CrestTurnGeometry, CrestTurnRefusal> {
    let schedule = recipe.schedule;
    let half_length_m = round_nine_decimal_places(
        schedule.ramp_length_m
            + schedule.pull_hold_length_m
            + schedule.ease_length_m
            + recipe.apex_hold_length_m,
    );
    if !half_length_m.is_finite() || half_length_m > MAXIMUM_INTEGRATION_STEPS as f64 {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    let half = integrate_half(schedule, half_length_m, true)?
        .expect("captured integration must return knots");
    let endpoint = half.knots[half.knots.len() - 1];
    let target_heading = hand_sign(schedule.hand) * recipe.sweep.radians() / 2.0;
    let endpoint_pitch = atan2(
        endpoint.tangent.z,
        hypot(endpoint.tangent.x, endpoint.tangent.y),
    );
    if (heading_radians(endpoint.tangent) - target_heading).abs() > 1e-6
        || endpoint_pitch.abs() > 1e-6
        || recipe
            .integration_target_height_m
            .is_some_and(|target| (endpoint.position.z - target).abs() > 1e-6)
    {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }

    let apex_index = half.knots.len() - 1;
    let apex_position = half.knots[apex_index].position;
    let apex_tangent = half.knots[apex_index].tangent;
    let mut knots = half.knots;
    for knot_index in (0..apex_index).rev() {
        let original = knots[knot_index];
        let reflected_position =
            apex_position + reflect_across_normal(original.position - apex_position, apex_tangent);
        let reflected_tangent = reflect_across_normal(original.tangent, apex_tangent) * -1.0;
        let reflected_up = reflect_across_normal(original.seat_up, apex_tangent);
        let reflected_curvature = reflect_across_normal(original.curvature_per_m, apex_tangent);
        let speed_mps = speed_at_height(schedule.entry_speed_mps, reflected_position.z)
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        let mut knot = original;
        knot.distance_m = 2.0 * half_length_m - original.distance_m;
        knot.position = reflected_position;
        knot.tangent = normalize(reflected_tangent);
        knot.seat_up = normalize(reflected_up);
        knot.curvature_per_m = reflected_curvature;
        knot.speed_mps = speed_mps;
        knot.lateral_g = lateral_force_g(knot);
        knots.push(knot);
    }

    Ok(CrestTurnGeometry {
        knots,
        apex_index,
        sweep: recipe.sweep,
        hand: schedule.hand,
        entry_speed_mps: schedule.entry_speed_mps,
        apex_bank_degrees: schedule.apex_bank_degrees,
        apex_vertical_g: schedule.apex_vertical_g,
        pull_g: PULL_G,
        ramp_length_m: schedule.ramp_length_m,
        pull_hold_length_m: schedule.pull_hold_length_m,
        ease_length_m: schedule.ease_length_m,
        apex_hold_length_m: recipe.apex_hold_length_m,
        target_height_m: recipe.target_height_m,
        integration_target_height_m: recipe.integration_target_height_m,
        half_length_m,
        placement_heading: Heading::East,
        placement_scale: 1.0,
    })
}

fn solve_auto_lengths(
    context: CrestTurnSolveContext,
    pull_hold_fraction: f64,
    ease_fraction: f64,
) -> Result<(f64, f64), CrestTurnRefusal> {
    let mut pull_hold_length_m = context.energy_height_m * pull_hold_fraction;
    let mut ease_length_m = context.energy_height_m * ease_fraction;
    for _ in 0..MAXIMUM_NEWTON_STEPS {
        let residual = endpoint_residual(
            context,
            [pull_hold_length_m, ease_length_m],
            APEX_HOLD_LENGTH_M,
        )?;
        if residual[0].abs() <= 1e-11 && residual[1].abs() <= 1e-11 {
            continue;
        }
        let hold_step_m = (pull_hold_length_m.abs() * 1e-5).max(1e-3);
        let ease_step_m = (ease_length_m.abs() * 1e-5).max(1e-3);
        let hold_delta = endpoint_residual(
            context,
            [pull_hold_length_m + hold_step_m, ease_length_m],
            APEX_HOLD_LENGTH_M,
        )?;
        let ease_delta = endpoint_residual(
            context,
            [pull_hold_length_m, ease_length_m + ease_step_m],
            APEX_HOLD_LENGTH_M,
        )?;
        let j00 = (hold_delta[0] - residual[0]) / hold_step_m;
        let j10 = (hold_delta[1] - residual[1]) / hold_step_m;
        let j01 = (ease_delta[0] - residual[0]) / ease_step_m;
        let j11 = (ease_delta[1] - residual[1]) / ease_step_m;
        let determinant = j00 * j11 - j01 * j10;
        if determinant.abs() < 1e-12 {
            return Err(CrestTurnRefusal::InfeasibleTarget);
        }
        let hold_update = (-residual[0] * j11 + residual[1] * j01) / determinant;
        let ease_update = (-j00 * residual[1] + j10 * residual[0]) / determinant;
        pull_hold_length_m =
            (pull_hold_length_m + hold_update).clamp(0.1, context.energy_height_m * 4.0);
        ease_length_m = (ease_length_m + ease_update).clamp(1.0, context.energy_height_m * 4.0);
    }
    let final_residual = endpoint_residual(
        context,
        [pull_hold_length_m, ease_length_m],
        APEX_HOLD_LENGTH_M,
    )?;
    if pull_hold_length_m <= 0.1
        || ease_length_m <= 1.0
        || final_residual[0].abs() > 1e-6
        || final_residual[1].abs() > 1e-6
    {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    Ok((pull_hold_length_m, ease_length_m))
}

fn solve_fixed_height_lengths(
    context: CrestTurnSolveContext,
    target_height_m: f64,
    initial_lengths: [f64; 3],
) -> Result<(f64, f64, f64), CrestTurnRefusal> {
    let mut lengths = initial_lengths;
    for _ in 0..MAXIMUM_NEWTON_STEPS {
        let residual = fixed_height_residual(context, lengths, target_height_m)?;
        if residual[0].abs() <= 1e-11 && residual[1].abs() <= 1e-11 && residual[2].abs() <= 1e-8 {
            continue;
        }
        let steps = lengths.map(|length| (length.abs() * 1e-5).max(1e-3));
        let mut jacobian = [[0.0; 3]; 3];
        for column in 0..3 {
            let mut perturbed_lengths = lengths;
            perturbed_lengths[column] += steps[column];
            let perturbed_residual =
                fixed_height_residual(context, perturbed_lengths, target_height_m)?;
            for row in 0..3 {
                jacobian[row][column] = (perturbed_residual[row] - residual[row]) / steps[column];
            }
        }
        let updates = solve_three_by_three(jacobian, [-residual[0], -residual[1], -residual[2]])
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        lengths[0] = (lengths[0] + updates[0]).clamp(0.1, context.energy_height_m * 4.0);
        lengths[1] = (lengths[1] + updates[1]).clamp(1.0, context.energy_height_m * 4.0);
        lengths[2] = (lengths[2] + updates[2]).clamp(1.0, context.energy_height_m * 4.0);
    }
    let final_residual = fixed_height_residual(context, lengths, target_height_m)?;
    if final_residual[0].abs() > 1e-6
        || final_residual[1].abs() > 1e-6
        || final_residual[2].abs() > 1e-6
    {
        return Err(CrestTurnRefusal::InfeasibleTarget);
    }
    Ok((lengths[0], lengths[1], lengths[2]))
}

fn solve_three_by_three(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> Option<[f64; 3]> {
    let mut augmented = [
        [matrix[0][0], matrix[0][1], matrix[0][2], vector[0]],
        [matrix[1][0], matrix[1][1], matrix[1][2], vector[1]],
        [matrix[2][0], matrix[2][1], matrix[2][2], vector[2]],
    ];
    for pivot_column in 0..3 {
        let pivot_row = (pivot_column..3).max_by(|left, right| {
            augmented[*left][pivot_column]
                .abs()
                .total_cmp(&augmented[*right][pivot_column].abs())
        })?;
        if augmented[pivot_row][pivot_column].abs() < 1e-12 {
            return None;
        }
        augmented.swap(pivot_column, pivot_row);
        let pivot = augmented[pivot_column][pivot_column];
        for value in augmented[pivot_column].iter_mut().skip(pivot_column) {
            *value /= pivot;
        }
        let normalized_pivot = augmented[pivot_column];
        for (row_index, row) in augmented.iter_mut().enumerate() {
            if row_index == pivot_column {
                continue;
            }
            let factor = row[pivot_column];
            for (value, pivot_value) in row
                .iter_mut()
                .skip(pivot_column)
                .zip(normalized_pivot.iter().skip(pivot_column))
            {
                *value -= factor * pivot_value;
            }
        }
    }
    Some([augmented[0][3], augmented[1][3], augmented[2][3]])
}

#[derive(Clone, Debug)]
struct HalfPath {
    knots: Vec<CrestTurnKnot>,
}

#[derive(Clone, Copy)]
struct PositionTangent {
    position: Vec3,
    tangent: Vec3,
}

#[derive(Clone, Copy)]
struct CrestTurnSchedule {
    entry_speed_mps: f64,
    hand: Hand,
    ramp_length_m: f64,
    pull_hold_length_m: f64,
    ease_length_m: f64,
    apex_bank_degrees: f64,
    apex_vertical_g: f64,
}

#[derive(Clone, Copy)]
struct CrestTurnRecipe {
    sweep: CrestTurnSweep,
    schedule: CrestTurnSchedule,
    apex_hold_length_m: f64,
    target_height_m: Option<f64>,
    integration_target_height_m: Option<f64>,
}

#[derive(Clone, Copy)]
struct CrestTurnSolveContext {
    schedule: CrestTurnSchedule,
    energy_height_m: f64,
    target_heading: f64,
}

fn endpoint_residual(
    context: CrestTurnSolveContext,
    lengths: [f64; 2],
    apex_hold_length_m: f64,
) -> Result<[f64; 2], CrestTurnRefusal> {
    let schedule = CrestTurnSchedule {
        pull_hold_length_m: lengths[0],
        ease_length_m: lengths[1],
        ..context.schedule
    };
    let half_length_m = schedule.ramp_length_m + lengths[0] + lengths[1] + apex_hold_length_m;
    let path = integrate_half(schedule, half_length_m, false)?
        .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
    let endpoint = path.knots[path.knots.len() - 1].tangent;
    Ok([
        atan2(endpoint.z, hypot(endpoint.x, endpoint.y)),
        heading_radians(endpoint) - context.target_heading,
    ])
}

fn fixed_height_residual(
    context: CrestTurnSolveContext,
    lengths: [f64; 3],
    target_height_m: f64,
) -> Result<[f64; 3], CrestTurnRefusal> {
    let schedule = CrestTurnSchedule {
        pull_hold_length_m: lengths[0],
        ease_length_m: lengths[1],
        ..context.schedule
    };
    let half_length_m = schedule.ramp_length_m + lengths[0] + lengths[1] + lengths[2];
    let path = integrate_half(schedule, half_length_m, false)?
        .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
    let apex = path.knots[0];
    Ok([
        atan2(apex.tangent.z, hypot(apex.tangent.x, apex.tangent.y)),
        heading_radians(apex.tangent) - context.target_heading,
        apex.position.z - target_height_m,
    ])
}

fn integrate_half(
    schedule: CrestTurnSchedule,
    half_length_m: f64,
    capture: bool,
) -> Result<Option<HalfPath>, CrestTurnRefusal> {
    if schedule.pull_hold_length_m <= 0.0
        || schedule.ease_length_m <= 0.0
        || !half_length_m.is_finite()
    {
        return Ok(None);
    }
    if half_length_m <= 0.0
        || (half_length_m / MAXIMUM_INTEGRATION_STEP_M).ceil() > MAXIMUM_INTEGRATION_STEPS as f64
    {
        return Ok(None);
    }
    let step_count = (half_length_m / MAXIMUM_INTEGRATION_STEP_M).ceil() as usize;
    let step_m = half_length_m / step_count as f64;
    let mut state = PositionTangent {
        position: Vec3::new(0.0, 0.0, 0.0),
        tangent: Vec3::new(1.0, 0.0, 0.0),
    };
    let mut knots = Vec::with_capacity(step_count + 1);
    if capture {
        knots.push(make_knot(state, 0.0, schedule.entry_speed_mps, 1.0, 0.0)?);
    }

    for step_index in 0..step_count {
        let distance_m = step_index as f64 * step_m;
        let end_distance_m = distance_m + step_m;
        state = rk4_step(state, distance_m, step_m, schedule)?
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        if capture {
            let (vertical_g, bank_degrees) = schedule_at(end_distance_m, schedule);
            let speed_mps = speed_at_height(schedule.entry_speed_mps, state.position.z)
                .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
            let seat_up = banked_up(state.tangent, bank_degrees.to_radians())
                .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
            let curvature_per_m =
                path_curvature(state.tangent, speed_mps, vertical_g, bank_degrees)
                    .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
            let mut knot = CrestTurnKnot {
                distance_m: end_distance_m,
                position: state.position,
                tangent: state.tangent,
                seat_up,
                curvature_per_m,
                bank_degrees,
                vertical_g,
                lateral_g: 0.0,
                longitudinal_g: 0.0,
                speed_mps,
            };
            knot.lateral_g = lateral_force_g(knot);
            knots.push(knot);
        }
    }
    if capture {
        Ok(Some(HalfPath { knots }))
    } else {
        let (vertical_g, bank_degrees) = schedule_at(half_length_m, schedule);
        let speed_mps = speed_at_height(schedule.entry_speed_mps, state.position.z)
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        let seat_up = banked_up(state.tangent, bank_degrees.to_radians())
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        let curvature_per_m = path_curvature(state.tangent, speed_mps, vertical_g, bank_degrees)
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        let knot = CrestTurnKnot {
            distance_m: half_length_m,
            position: state.position,
            tangent: state.tangent,
            seat_up,
            curvature_per_m,
            bank_degrees,
            vertical_g,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            speed_mps,
        };
        Ok(Some(HalfPath { knots: vec![knot] }))
    }
}

fn rk4_step(
    state: PositionTangent,
    distance_m: f64,
    step_m: f64,
    schedule: CrestTurnSchedule,
) -> Result<Option<PositionTangent>, CrestTurnRefusal> {
    let derivative = |state: PositionTangent,
                      distance_m: f64|
     -> Result<PositionTangent, CrestTurnRefusal> {
        let (vertical_g, bank_degrees) = schedule_at(distance_m, schedule);
        let speed_mps = speed_at_height(schedule.entry_speed_mps, state.position.z)
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        let curvature_per_m = path_curvature(state.tangent, speed_mps, vertical_g, bank_degrees)
            .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
        Ok(PositionTangent {
            position: state.tangent,
            tangent: curvature_per_m,
        })
    };
    let add = |state: PositionTangent, derivative: PositionTangent, scale: f64| PositionTangent {
        position: state.position + derivative.position * scale,
        tangent: normalize(state.tangent + derivative.tangent * scale),
    };
    let first = derivative(state, distance_m)?;
    let second = derivative(add(state, first, step_m * 0.5), distance_m + step_m * 0.5)?;
    let third = derivative(add(state, second, step_m * 0.5), distance_m + step_m * 0.5)?;
    let fourth = derivative(add(state, third, step_m), distance_m + step_m)?;
    Ok(Some(PositionTangent {
        position: state.position
            + (first.position + second.position * 2.0 + third.position * 2.0 + fourth.position)
                * (step_m / 6.0),
        tangent: normalize(
            state.tangent
                + (first.tangent + second.tangent * 2.0 + third.tangent * 2.0 + fourth.tangent)
                    * (step_m / 6.0),
        ),
    }))
}

fn schedule_at(distance_m: f64, schedule: CrestTurnSchedule) -> (f64, f64) {
    let hold_end_m = schedule.ramp_length_m + schedule.pull_hold_length_m;
    let ease_end_m = hold_end_m + schedule.ease_length_m;
    if distance_m <= schedule.ramp_length_m {
        let progress = smoothstep(distance_m / schedule.ramp_length_m);
        (1.0 + (PULL_G - 1.0) * progress, 0.0)
    } else if distance_m <= hold_end_m {
        (PULL_G, 0.0)
    } else if distance_m <= ease_end_m {
        let progress = smoothstep((distance_m - hold_end_m) / schedule.ease_length_m);
        let bank_progress = bank_ease((distance_m - hold_end_m) / schedule.ease_length_m);
        (
            PULL_G + (schedule.apex_vertical_g - PULL_G) * progress,
            -hand_sign(schedule.hand) * schedule.apex_bank_degrees * bank_progress,
        )
    } else {
        (
            schedule.apex_vertical_g,
            -hand_sign(schedule.hand) * schedule.apex_bank_degrees,
        )
    }
}

fn smoothstep(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

fn bank_ease(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    smoothstep(progress + 0.9 * progress * (1.0 - progress))
}

fn path_curvature(
    tangent: Vec3,
    speed_mps: f64,
    vertical_g: f64,
    bank_degrees: f64,
) -> Option<Vec3> {
    let world_up = Vec3::new(0.0, 0.0, 1.0);
    let gravity_normal = world_up - tangent * world_up.dot(tangent);
    let seat_up = banked_up(tangent, bank_degrees.to_radians())?;
    let speed_squared = speed_mps * speed_mps;
    let curvature = (seat_up * (vertical_g * GRAVITY) - gravity_normal * GRAVITY) / speed_squared;
    Some(curvature)
}

fn banked_up(tangent: Vec3, bank_radians: f64) -> Option<Vec3> {
    let world_up = Vec3::new(0.0, 0.0, 1.0);
    let gravity_normal = world_up - tangent * world_up.dot(tangent);
    let gravity_normal_length = vector_length(gravity_normal);
    if gravity_normal_length <= 1e-8 {
        return None;
    }
    let unbanked_up = gravity_normal / gravity_normal_length;
    let (sine, cosine) = sin_cos(bank_radians);
    Some(normalize(
        unbanked_up * cosine + tangent.cross(unbanked_up) * sine,
    ))
}

fn make_knot(
    state: PositionTangent,
    distance_m: f64,
    entry_speed_mps: f64,
    vertical_g: f64,
    bank_degrees: f64,
) -> Result<CrestTurnKnot, CrestTurnRefusal> {
    let speed_mps = speed_at_height(entry_speed_mps, state.position.z)
        .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
    let seat_up = banked_up(state.tangent, bank_degrees.to_radians())
        .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
    let curvature_per_m = path_curvature(state.tangent, speed_mps, vertical_g, bank_degrees)
        .ok_or(CrestTurnRefusal::InfeasibleTarget)?;
    let mut knot = CrestTurnKnot {
        distance_m,
        position: state.position,
        tangent: state.tangent,
        seat_up,
        curvature_per_m,
        bank_degrees,
        vertical_g,
        lateral_g: 0.0,
        longitudinal_g: 0.0,
        speed_mps,
    };
    knot.lateral_g = lateral_force_g(knot);
    Ok(knot)
}

fn lateral_force_g(knot: CrestTurnKnot) -> f64 {
    let world_up = Vec3::new(0.0, 0.0, 1.0);
    let gravity_normal = world_up - knot.tangent * world_up.dot(knot.tangent);
    let reaction =
        knot.curvature_per_m * (knot.speed_mps * knot.speed_mps) + gravity_normal * GRAVITY;
    let seat_side = normalize(knot.tangent.cross(knot.seat_up));
    reaction.dot(seat_side) / GRAVITY
}

fn speed_at_height(entry_speed_mps: f64, height_m: f64) -> Option<f64> {
    let speed_squared = entry_speed_mps * entry_speed_mps - 2.0 * GRAVITY * height_m;
    (speed_squared > 0.0).then(|| libm::sqrt(speed_squared))
}

fn reflect_across_normal(vector: Vec3, plane_normal: Vec3) -> Vec3 {
    vector - plane_normal * (2.0 * vector.dot(plane_normal))
}

fn rotate_around_world_up(vector: Vec3, angle_radians: f64) -> Vec3 {
    let (sine, cosine) = sin_cos(angle_radians);
    Vec3::new(
        vector.x * cosine - vector.y * sine,
        vector.x * sine + vector.y * cosine,
        vector.z,
    )
}

fn heading_radians(tangent: Vec3) -> f64 {
    atan2(tangent.y, tangent.x)
}

fn vector_length(vector: Vec3) -> f64 {
    libm::sqrt(vector.dot(vector))
}

fn normalize(vector: Vec3) -> Vec3 {
    vector / vector_length(vector)
}

fn round_nine_decimal_places(value: f64) -> f64 {
    (value * NINE_DECIMAL_SCALE).round() / NINE_DECIMAL_SCALE
}

fn grid_scale_for_exit(exit_position: Vec3, heading: Heading) -> f64 {
    let heading_angle = f64::from(heading.index()) * FRAC_PI_4;
    let exit = rotate_around_world_up(exit_position, heading_angle);
    let component = if exit.x.abs() >= exit.y.abs() {
        exit.x
    } else {
        exit.y
    };
    let grid_component = (component / 2.0).round() * 2.0;
    grid_component / component
}

fn feel_from_vertical_g(vertical_g: f64) -> CrestApexFeel {
    match vertical_g {
        0.3 => CrestApexFeel::Floaty,
        1.0 => CrestApexFeel::Neutral,
        1.5 => CrestApexFeel::PressedIn,
        _ => unreachable!("crest recipe stores one of the supported apex feels"),
    }
}

#[cfg(test)]
fn wrap_angle(angle_radians: f64) -> f64 {
    (angle_radians + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI)
        - std::f64::consts::PI
}

fn hand_sign(hand: Hand) -> f64 {
    match hand {
        Hand::Left => 1.0,
        Hand::Right => -1.0,
    }
}

#[cfg(test)]
const TEST_ENTRY_SPEEDS_MPS: [f64; 5] = [26.0, 30.0, 34.0, 38.0, 42.0];
#[cfg(test)]
const TEST_BANKS_DEGREES: [f64; 7] = [90.0, 95.0, 100.0, 105.0, 110.0, 115.0, 120.0];
#[cfg(test)]
const TEST_APEX_FEELS: [CrestApexFeel; 3] = [
    CrestApexFeel::Floaty,
    CrestApexFeel::Neutral,
    CrestApexFeel::PressedIn,
];
#[cfg(test)]
const TEST_SWEEPS: [CrestTurnSweep; 2] = [CrestTurnSweep::Quarter, CrestTurnSweep::Half];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn solve(
        speed_mps: f64,
        sweep: CrestTurnSweep,
        bank_degrees: f64,
        feel: CrestApexFeel,
    ) -> CrestTurnGeometry {
        solve_crest_turn(speed_mps, sweep, Hand::Left, bank_degrees, feel)
            .unwrap_or_else(|refusal| panic!("expected feasible crest turn: {refusal:?}"))
    }

    #[test]
    fn apex_heading_and_tangent_match_the_requested_sweep() {
        for sweep in TEST_SWEEPS {
            let geometry = solve(34.0, sweep, 110.0, CrestApexFeel::Neutral);
            let apex = geometry.apex();
            assert!(apex.tangent.z.abs() <= 1e-6);
            assert!((hypot(apex.tangent.x, apex.tangent.y) - 1.0).abs() <= 1e-9);
            assert!((geometry.apex_heading_radians() - sweep.radians() / 2.0).abs() <= 1e-6);
            assert!(wrap_angle(geometry.exit_heading_radians() - sweep.radians()).abs() <= 1e-6);
        }
    }

    #[test]
    fn mirrored_halves_join_with_continuous_position_tangent_curvature_and_bank() {
        for sweep in TEST_SWEEPS {
            let geometry = solve(34.0, sweep, 110.0, CrestApexFeel::Neutral);
            let apex_index = geometry.apex_index;
            let before = geometry.knots[apex_index - 1];
            let apex = geometry.knots[apex_index];
            let after = geometry.knots[apex_index + 1];

            assert!((before.position - apex.position).length() > 0.0);
            assert!((apex.position - after.position).length() > 0.0);
            let expected_after_position = apex.position
                + reflect_across_normal(before.position - apex.position, apex.tangent);
            let expected_after_tangent = reflect_across_normal(before.tangent, apex.tangent) * -1.0;
            let expected_after_curvature =
                reflect_across_normal(before.curvature_per_m, apex.tangent);
            let expected_after_up = reflect_across_normal(before.seat_up, apex.tangent);
            assert!((after.position - expected_after_position).length() < 1e-10);
            assert!((after.tangent - expected_after_tangent).length() < 1e-10);
            assert!((after.curvature_per_m - expected_after_curvature).length() < 1e-10);
            assert!((after.seat_up - expected_after_up).length() < 1e-10);
            assert!(
                (apex.tangent + reflect_across_normal(apex.tangent, apex.tangent)).length() < 1e-10
            );
            assert!(
                (apex.curvature_per_m - reflect_across_normal(apex.curvature_per_m, apex.tangent))
                    .length()
                    < 1e-10
            );
            assert!(
                (apex.seat_up - reflect_across_normal(apex.seat_up, apex.tangent)).length() < 1e-10
            );
            assert!((apex.curvature_per_m - after.curvature_per_m).length() < 0.005);
            assert!((before.bank_degrees - after.bank_degrees).abs() < 1e-8);
            assert!((apex.bank_degrees - after.bank_degrees).abs() < 1e-8);
            assert!((apex.bank_degrees.abs() - 110.0).abs() < 1e-9);
        }
    }

    #[test]
    fn endpoint_lands_on_the_track_grid_for_all_eight_entry_headings() {
        for sweep in TEST_SWEEPS {
            let geometry = solve(34.0, sweep, 110.0, CrestApexFeel::Neutral);
            for heading in ENTRY_HEADINGS {
                let placed = geometry
                    .placed_on_grid(heading)
                    .expect("auto-height grid placement should solve");
                let grid_scale = geometry.grid_scale_for_heading(heading);
                assert_eq!(
                    placed.placement_scale,
                    round_nine_decimal_places(placed.placement_scale)
                );
                assert!((grid_scale - 1.0).abs() < 0.05);
                assert!(
                    ((placed.exit_position().x / 2.0) - (placed.exit_position().x / 2.0).round())
                        .abs()
                        * 2.0
                        <= GRID_POSITION_TOLERANCE_M
                );
                assert!(
                    ((placed.exit_position().y / 2.0) - (placed.exit_position().y / 2.0).round())
                        .abs()
                        * 2.0
                        <= GRID_POSITION_TOLERANCE_M
                );
                let expected_exit = rotate_around_world_up(
                    geometry.exit_position(),
                    f64::from(heading.index()) * FRAC_PI_4,
                ) * placed.placement_scale;
                assert!((placed.exit_position() - expected_exit).length() < 1e-7);
                let expected_heading = wrap_angle(
                    geometry.exit_heading_radians() + f64::from(heading.index()) * FRAC_PI_4,
                );
                assert!(
                    (wrap_angle(placed.exit_heading_radians() - expected_heading)).abs() < 1e-8
                );
                assert!(placed.passes_hyper_and_comfort_limits());
                for knot in &placed.knots {
                    assert!((knot.point_mass_force_g() - knot.vertical_g).abs() < 1e-9);
                    assert!((lateral_force_g(*knot) - knot.lateral_g).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn fixed_height_grid_placement_refuses_to_change_the_caller_target() {
        let auto = solve(34.0, CrestTurnSweep::Half, 110.0, CrestApexFeel::Neutral);
        let requested_height_m = round_nine_decimal_places(auto.apex().position.z + 2.0);
        let fixed = solve_crest_turn_with_size(
            34.0,
            CrestTurnSweep::Half,
            Hand::Left,
            110.0,
            CrestApexFeel::Neutral,
            CrestTurnSizeHint::FixedHeight(requested_height_m),
        )
        .unwrap_or_else(|refusal| panic!("expected fixed-height solve: {refusal:?}"));

        assert_eq!(
            fixed.placed_on_grid(Heading::East),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
        for heading in ENTRY_HEADINGS {
            if let Ok(placed) = fixed.placed_on_grid(heading) {
                assert_eq!(placed.target_height_m, Some(requested_height_m));
                assert!(
                    (placed.apex().position.z - requested_height_m).abs()
                        <= FIXED_HEIGHT_TOLERANCE_M
                );
            }
        }
    }

    #[test]
    fn stored_recipe_replays_base_and_grid_placed_geometry() {
        let geometry = solve(34.0, CrestTurnSweep::Half, 110.0, CrestApexFeel::Neutral);
        assert_eq!(geometry.replay(), Ok(geometry.clone()));
        let placed = geometry
            .placed_on_grid(Heading::NorthEast)
            .expect("auto-height placement should solve");
        assert_eq!(placed.replay(), Ok(placed));
    }

    #[test]
    fn point_mass_force_along_the_path_matches_the_force_schedule() {
        let geometry = solve(34.0, CrestTurnSweep::Half, 110.0, CrestApexFeel::Neutral);
        for knot in &geometry.knots {
            let normal_force = knot.point_mass_force_g();
            assert!((normal_force - knot.vertical_g).abs() < 0.03);
            assert!(knot.lateral_g.abs() < 1e-9);
        }
    }

    #[test]
    fn feasibility_map_covers_speeds_sweeps_banks_and_apex_feels() {
        let feasibility_map = render_feasibility_map();
        assert_eq!(
            feasibility_map,
            include_str!("../../../../../docs/crest-turn-feasibility.md")
        );
    }

    #[test]
    fn invalid_bank_and_speed_targets_are_typed_refusals() {
        assert_eq!(
            solve_crest_turn(
                34.0,
                CrestTurnSweep::Quarter,
                Hand::Left,
                89.0,
                CrestApexFeel::Neutral
            ),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
        assert_eq!(
            solve_crest_turn(
                0.0,
                CrestTurnSweep::Half,
                Hand::Right,
                110.0,
                CrestApexFeel::Floaty
            ),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
        assert_eq!(
            solve_crest_turn(
                45.0,
                CrestTurnSweep::Half,
                Hand::Right,
                110.0,
                CrestApexFeel::Floaty
            ),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
    }

    #[test]
    fn fixed_height_mode_solves_apex_height_and_stores_rounded_recipe() {
        let auto = solve(34.0, CrestTurnSweep::Half, 110.0, CrestApexFeel::Neutral);
        let target_height_m = round_nine_decimal_places(auto.apex().position.z + 2.0);
        let fixed = solve_crest_turn_with_size(
            34.123_456_789_4,
            CrestTurnSweep::Half,
            Hand::Left,
            110.000_000_000_4,
            CrestApexFeel::Neutral,
            CrestTurnSizeHint::FixedHeight(target_height_m),
        )
        .unwrap_or_else(|refusal| panic!("expected fixed-height solve: {refusal:?}"));

        assert_eq!(fixed.entry_speed_mps, 34.123_456_789);
        assert_eq!(fixed.apex_bank_degrees, 110.0);
        assert_eq!(fixed.target_height_m, Some(target_height_m));
        assert!((fixed.apex().position.z - target_height_m).abs() < 1e-6);
        assert!(fixed.apex_hold_length_m > 0.0);
        for recipe_value in [
            fixed.ramp_length_m,
            fixed.pull_hold_length_m,
            fixed.ease_length_m,
            fixed.apex_hold_length_m,
            fixed.half_length_m,
            fixed.pull_hold_end_m(),
        ] {
            assert_eq!(recipe_value, round_nine_decimal_places(recipe_value));
        }
        assert_eq!(
            fixed,
            solve_crest_turn_with_size(
                34.123_456_789_4,
                CrestTurnSweep::Half,
                Hand::Left,
                110.000_000_000_4,
                CrestApexFeel::Neutral,
                CrestTurnSizeHint::FixedHeight(target_height_m),
            )
            .expect("the rounded fixed-height recipe solves deterministically"),
        );
    }

    #[test]
    fn fixed_height_target_is_rounded_before_solving() {
        let auto = solve(34.0, CrestTurnSweep::Half, 110.0, CrestApexFeel::Neutral);
        let target_height_m = round_nine_decimal_places(auto.apex().position.z + 2.0);
        let exact = solve_crest_turn_with_size(
            34.0,
            CrestTurnSweep::Half,
            Hand::Left,
            110.0,
            CrestApexFeel::Neutral,
            CrestTurnSizeHint::FixedHeight(target_height_m),
        );
        let noisy = solve_crest_turn_with_size(
            34.0,
            CrestTurnSweep::Half,
            Hand::Left,
            110.0,
            CrestApexFeel::Neutral,
            CrestTurnSizeHint::FixedHeight(target_height_m + 0.000_000_000_4),
        );

        assert_eq!(noisy, exact);
        assert_eq!(
            exact.map(|geometry| geometry.target_height_m),
            Ok(Some(target_height_m)),
        );
    }

    #[test]
    fn fixed_height_grid_placement_solves_a_two_meter_step() {
        let target_height_m = 12.0;
        let geometry = solve_crest_turn_with_size(
            26.0,
            CrestTurnSweep::Quarter,
            Hand::Left,
            90.0,
            CrestApexFeel::Floaty,
            CrestTurnSizeHint::FixedHeight(target_height_m),
        )
        .expect("the 12 m fixed-height recipe should solve");
        let placed = geometry
            .placed_on_grid(Heading::East)
            .expect("joint snapping should preserve this fixed-height target");

        assert_eq!(placed.target_height_m, Some(target_height_m));
        assert!((placed.apex().position.z - target_height_m).abs() <= FIXED_HEIGHT_TOLERANCE_M);
        assert!(placed.exit_is_on_grid());
        assert!(placed.passes_hyper_and_comfort_limits());
        assert_eq!(placed.replay(), Ok(placed.clone()));
    }

    #[test]
    fn unreachable_fixed_height_returns_an_infeasible_refusal() {
        assert_eq!(
            solve_crest_turn_with_size(
                34.0,
                CrestTurnSweep::Half,
                Hand::Left,
                110.0,
                CrestApexFeel::Neutral,
                CrestTurnSizeHint::FixedHeight(2.0),
            ),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
        assert_eq!(
            solve_crest_turn_with_size(
                34.0,
                CrestTurnSweep::Half,
                Hand::Left,
                110.0,
                CrestApexFeel::Neutral,
                CrestTurnSizeHint::FixedHeight(1_000_000.0),
            ),
            Err(CrestTurnRefusal::InfeasibleTarget),
        );
    }

    fn render_feasibility_map() -> String {
        let mut output = String::from(
            "# Crest turn point-mass feasibility\n\n\
            Each cell shows whether the auto-height offline point-mass solver accepts that bank and apex feel.\n\
            It checks 0.25 g headroom against the hyper's -1.0 to 4.0 g vertical limits, the 0.15 g fully banked lateral limit,\n\
            the 200 Hz ASTM comfort evaluator, roll rate, and both horizontal coordinates on the 2 m grid for all eight entry headings.\n\
            Longitudinal force is zero in the lossless energy model. This map does not include train cars, drag, clearance, or supports.\n\n\
            Fixed-height mode adds apex height as a constraint and solves apex-hold length as a third unknown. Grid placement re-solves that schedule and accepts the rounded caller target within 1e-6 m; a discrete snap that cannot meet this tolerance returns a typed refusal. The 12 m target at 26 m/s, 90-degree sweep, 90-degree bank, floaty feel and East entry is covered as a successful 2 m-step placement. The stored ramp, pull hold, ease, apex hold, heading, and scale let accepted geometry replay its placement. Inputs and resolved schedule values are rounded to nine decimals.\n\n\
            A refusal means this fixed deterministic recipe missed its numerical or listed constraints; it does not prove that no other schedule can work.\n\n\
            | Sweep | Apex bank | Apex feel | 26 m/s | 30 m/s | 34 m/s | 38 m/s | 42 m/s |\n\
            | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |\n",
        );
        let mut solved_counts = [[0_usize; TEST_ENTRY_SPEEDS_MPS.len()]; TEST_SWEEPS.len()];
        let mut refusal_counts = [[0_usize; TEST_ENTRY_SPEEDS_MPS.len()]; TEST_SWEEPS.len()];
        for (sweep_index, sweep) in TEST_SWEEPS.into_iter().enumerate() {
            for feel in TEST_APEX_FEELS {
                for bank_degrees in TEST_BANKS_DEGREES {
                    let _ = write!(
                        output,
                        "| {} | {bank_degrees:.0}° | {}",
                        match sweep {
                            CrestTurnSweep::Quarter => "90°",
                            CrestTurnSweep::Half => "180°",
                        },
                        match feel {
                            CrestApexFeel::Floaty => "floaty (0.3 g)",
                            CrestApexFeel::Neutral => "neutral (1.0 g)",
                            CrestApexFeel::PressedIn => "pressed-in (1.5 g)",
                        },
                    );
                    for (speed_index, speed_mps) in TEST_ENTRY_SPEEDS_MPS.into_iter().enumerate() {
                        let mark = match solve_crest_turn(
                            speed_mps,
                            sweep,
                            Hand::Left,
                            bank_degrees,
                            feel,
                        ) {
                            Ok(geometry) => {
                                solved_counts[sweep_index][speed_index] += 1;
                                assert!(geometry.passes_hyper_and_comfort_limits());
                                assert!(
                                    (geometry.apex().bank_degrees.abs() - bank_degrees).abs()
                                        < 1e-8
                                );
                                assert!(
                                    (geometry.apex().vertical_g - feel.vertical_g()).abs() < 1e-8
                                );
                                "solve"
                            }
                            Err(CrestTurnRefusal::InfeasibleTarget) => {
                                refusal_counts[sweep_index][speed_index] += 1;
                                "refuse"
                            }
                        };
                        let _ = write!(output, " | {mark}");
                    }
                    output.push_str(" |\n");
                }
            }
        }
        for sweep_index in 0..TEST_SWEEPS.len() {
            for speed_index in 0..TEST_ENTRY_SPEEDS_MPS.len() {
                assert!(solved_counts[sweep_index][speed_index] > 0);
                assert!(refusal_counts[sweep_index][speed_index] > 0);
            }
        }
        output
    }
}
