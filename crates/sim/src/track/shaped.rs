use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::physics::{
    CandidateCalibrationPreview, CandidateCarForces, GRAVITY, PhysicsConfig, PreviewCache,
    PreviewStatus,
};
use crate::state_hash::StateHasher;
use crate::track::construction::ConstructionError;
use crate::track::profile::{
    OfflineHillSpec, OfflineTrain, SHAPED_HILL_TAPER_ANGLE, SolvedProfile,
    solve_shaped_hill_offline_profile,
};
pub(crate) use crate::track::shaped::precision::canonicalize_element_spec;
use crate::track::shaped::precision::{ELEMENT_SPEC_HALF_DECIMAL_STEP, round_element_value};
use crate::track::supports::SupportObstacle;
use crate::track::{
    Bank, CoasterModel, Hand, Heading, HillFeel, HorizontalCurvature, Piece, PieceId, PieceKind,
    Shape, Slope, TrackState, TrainConfiguration, VerticalCurvature, catalog,
};
use crate::world::World;

mod precision;

pub use crate::track::curves::{SpeedTurnGeometry as ResolvedTurnGeometry, SpeedTurnKnot};
pub use crate::track::profile::SolvedProfile as ResolvedHillProfile;

const PROFILE_STEPS: usize = 512;
const REQUIRED_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_HILL_INTENT_ERROR_G: f64 = 0.05;
const MAXIMUM_FULLY_BANKED_LATERAL_G: f64 = 0.15;
const MINIMUM_SHAPED_SPEED_MPS: f64 = 22.0;
const MAXIMUM_SHAPED_SPEED_MPS: f64 = 36.0;
const MINIMUM_SHAPED_TRAIN_CARS: u8 = 4;
const MAXIMUM_SHAPED_TRAIN_CARS: u8 = 8;
const MAXIMUM_SHAPED_PREVIEWS: u8 = 4;
const HILL_HEIGHT_ENERGY_FRACTION: f64 = 0.4;
const SHAPED_HILL_PULL_G: f64 = 3.3;
const SHAPED_FLANK_SEARCH_STEPS: usize = 8;
const SHAPED_HILL_NAME: &str = "Shaped airtime hill";
const SHAPED_LEFT_TURN_NAME: &str = "Shaped banked turn left";
const SHAPED_RIGHT_TURN_NAME: &str = "Shaped banked turn right";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum HillSizeHint {
    Auto,
    Height(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TurnSizeHint {
    Auto,
    Radius(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ElementIntent {
    Hill {
        feel: HillFeel,
        strength_g: Option<f64>,
        size: HillSizeHint,
    },
    Turn {
        hand: Hand,
        size: TurnSizeHint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElementSpec {
    Hill {
        design_speed_mps: f64,
        cars: u8,
        target_g: f64,
        delivered_g: f64,
        height_m: f64,
        span_m: f64,
        profile: ResolvedHillProfile,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        geometry: ResolvedTurnGeometry,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HumanReadableElementSpec {
    Hill {
        design_speed_mps: f64,
        cars: u8,
        target_g: f64,
        delivered_g: f64,
        height_m: f64,
        span_m: f64,
        profile: ResolvedHillProfile,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        geometry: ResolvedTurnGeometry,
    },
}

#[derive(Serialize, Deserialize)]
enum BinaryElementSpec {
    Hill {
        design_speed_mps: f64,
        cars: u8,
        target_g: f64,
        delivered_g: f64,
        height_m: f64,
        span_m: f64,
        profile: ResolvedHillProfile,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        geometry: ResolvedTurnGeometry,
    },
}

impl Serialize for ElementSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut canonicalized = self.clone();
        if serializer.is_human_readable() {
            canonicalize_element_spec(&mut canonicalized);
        }
        let serializable_spec = &canonicalized;
        if serializer.is_human_readable() {
            let spec = match serializable_spec {
                Self::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                } => HumanReadableElementSpec::Hill {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    target_g: *target_g,
                    delivered_g: *delivered_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    profile: profile.clone(),
                },
                Self::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                } => HumanReadableElementSpec::Turn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    hand: *hand,
                    vertical_g: *vertical_g,
                    lateral_g: *lateral_g,
                    radius_m: *radius_m,
                    bank_degrees: *bank_degrees,
                    geometry: geometry.clone(),
                },
            };
            spec.serialize(serializer)
        } else {
            let spec = match serializable_spec {
                Self::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                } => BinaryElementSpec::Hill {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    target_g: *target_g,
                    delivered_g: *delivered_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    profile: profile.clone(),
                },
                Self::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                } => BinaryElementSpec::Turn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    hand: *hand,
                    vertical_g: *vertical_g,
                    lateral_g: *lateral_g,
                    radius_m: *radius_m,
                    bank_degrees: *bank_degrees,
                    geometry: geometry.clone(),
                },
            };
            spec.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for ElementSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let mut spec = match HumanReadableElementSpec::deserialize(deserializer)? {
                HumanReadableElementSpec::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                } => Self::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                },
                HumanReadableElementSpec::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                } => Self::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                },
            };
            canonicalize_element_spec(&mut spec);
            Ok(spec)
        } else {
            Ok(match BinaryElementSpec::deserialize(deserializer)? {
                BinaryElementSpec::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                } => Self::Hill {
                    design_speed_mps,
                    cars,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    profile,
                },
                BinaryElementSpec::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                } => Self::Turn {
                    design_speed_mps,
                    cars,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry,
                },
            })
        }
    }
}

impl ElementSpec {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        match self {
            Self::Hill {
                design_speed_mps,
                cars,
                target_g,
                delivered_g,
                height_m,
                span_m,
                profile,
            } => {
                hash.write_u8(0);
                hash.write_f64(*design_speed_mps);
                hash.write_u8(*cars);
                hash.write_f64(*target_g);
                hash.write_f64(*delivered_g);
                hash.write_f64(*height_m);
                hash.write_f64(*span_m);
                profile.hash_into(hash);
            }
            Self::Turn {
                design_speed_mps,
                cars,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry,
            } => {
                hash.write_u8(1);
                hash.write_f64(*design_speed_mps);
                hash.write_u8(*cars);
                hash.write_u8(*hand as u8);
                hash.write_f64(*vertical_g);
                hash.write_f64(*lateral_g);
                hash.write_f64(*radius_m);
                hash.write_f64(*bank_degrees);
                geometry.hash_into(hash);
            }
        }
    }

    pub fn design_speed_mps(&self) -> f64 {
        match self {
            Self::Hill {
                design_speed_mps, ..
            }
            | Self::Turn {
                design_speed_mps, ..
            } => *design_speed_mps,
        }
    }

    pub fn cars(&self) -> u8 {
        match self {
            Self::Hill { cars, .. } | Self::Turn { cars, .. } => *cars,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SolvedElement {
    pub piece: Piece,
    pub spec: ElementSpec,
    pub calibration_runs: u8,
    pub cap_note: Option<FeelCapNote>,
    pub(crate) calibration_seed: Option<CalibrationSeed>,
}

#[derive(Clone, Debug)]
pub(crate) struct CalibrationSeed {
    preview: CandidateCalibrationPreview,
    hill_crest_g: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeelCapNote {
    pub feel: HillFeel,
    pub requested_target_g: f64,
    pub capped_target_g: f64,
    pub delivered_g: f64,
}

pub struct SolveContext<'a> {
    pub entry: TrackState,
    pub entry_speed_mps: f64,
    pub model: &'a CoasterModel,
    pub train: &'a TrainConfiguration,
    pub prefix: &'a [Piece],
    pub preview_cache: &'a mut PreviewCache,
    pub physics: PhysicsConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "camelCase")]
pub enum SolveRefusal {
    Stall {
        entry_speed_mps: f64,
        height_m: f64,
    },
    InfeasibleTarget {
        target_g: f64,
        available_headroom_g: f64,
    },
    BelowMinimumSpeed {
        entry_speed_mps: f64,
        minimum_speed_mps: f64,
    },
    UnsupportedScope,
    PreviewTimedOut,
    InvalidContext,
    UnsupportedConnector,
    UnsupportedSizeOverride,
    InvalidGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedElementRules {
    pub allow_airtime_hills: bool,
    pub allow_fully_banked_turns: bool,
    pub default_floater_g: f64,
    pub default_strong_floater_g: f64,
    pub steel_ejector_g: f64,
    pub wooden_ejector_g: f64,
    pub hill_pull_g: f64,
    pub turn_vertical_g: f64,
    pub maximum_bank_degrees: f64,
    pub minimum_turn_radius_m: f64,
    pub maximum_turn_radius_m: f64,
}

impl CoasterModel {
    pub fn shaped_element_rules(&self) -> ShapedElementRules {
        ShapedElementRules {
            allow_airtime_hills: true,
            allow_fully_banked_turns: true,
            default_floater_g: -0.5,
            default_strong_floater_g: -0.85,
            steel_ejector_g: -1.2,
            wooden_ejector_g: -0.8,
            hill_pull_g: (self.comfortable_force_limits.maximum_vertical_g - 0.5).min(2.0),
            turn_vertical_g: (self.comfortable_force_limits.maximum_vertical_g - 1.1)
                .min(1.0 / 75.0_f64.to_radians().cos()),
            maximum_bank_degrees: 75.0,
            minimum_turn_radius_m: self.minimum_radius_metres,
            maximum_turn_radius_m: 256.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapedConstructionError {
    Solve(SolveRefusal),
    Construction(ConstructionError),
    InvalidResolvedSpec,
}

pub fn solve(
    intent: &ElementIntent,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    solve_with_preview_limit(intent, context, MAXIMUM_SHAPED_PREVIEWS, false, None)
}

pub fn solve_preview(
    intent: &ElementIntent,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    solve_with_preview_limit(intent, context, 1, true, None)
}

pub fn solve_confirm(
    intent: &ElementIntent,
    context: &mut SolveContext<'_>,
    preview: &SolvedElement,
) -> Result<SolvedElement, SolveRefusal> {
    validate_context(context)?;
    validate_scope(context)?;
    if !preview_candidate_is_confirmed(intent, context, preview) {
        let maximum_previews = match intent {
            ElementIntent::Hill { .. } => MAXIMUM_SHAPED_PREVIEWS,
            ElementIntent::Turn { .. } => 3,
        };
        return solve_with_preview_limit(intent, context, maximum_previews, false, Some(preview));
    }
    let mut confirmed = preview.clone();
    confirmed.calibration_seed = None;
    Ok(confirmed)
}

fn solve_with_preview_limit(
    intent: &ElementIntent,
    context: &mut SolveContext<'_>,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
) -> Result<SolvedElement, SolveRefusal> {
    validate_context(context)?;
    validate_scope(context)?;
    match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => solve_hill(
            *feel,
            *strength_g,
            *size,
            maximum_previews,
            provisional,
            preview_seed,
            context,
        ),
        ElementIntent::Turn { hand, size } => solve_turn(
            *hand,
            *size,
            maximum_previews.min(3),
            provisional,
            preview_seed,
            context,
        ),
    }
}

fn validate_context(context: &SolveContext<'_>) -> Result<(), SolveRefusal> {
    if !context.entry_speed_mps.is_finite()
        || context.entry_speed_mps <= 0.0
        || context.train.cars == 0
        || context.train.cars != context.model.train.cars
        || (context.train.length_metres - context.model.train.length_metres).abs() > 1e-9
        || context
            .prefix
            .last()
            .is_none_or(|piece| piece.exit != context.entry)
        || !context
            .prefix
            .iter()
            .any(|piece| piece.kind == PieceKind::Station)
    {
        return Err(SolveRefusal::InvalidContext);
    }
    if context.entry.slope != Slope::Level
        || context.entry.bank != Bank::Level
        || context.entry.horizontal_curvature != HorizontalCurvature::Straight
        || context.entry.vertical_curvature != VerticalCurvature::Straight
    {
        return Err(SolveRefusal::UnsupportedConnector);
    }
    Ok(())
}

fn validate_scope(context: &SolveContext<'_>) -> Result<(), SolveRefusal> {
    if context.model.key != "hyper"
        || !(MINIMUM_SHAPED_TRAIN_CARS..=MAXIMUM_SHAPED_TRAIN_CARS).contains(&context.train.cars)
        || context.entry_speed_mps > MAXIMUM_SHAPED_SPEED_MPS
    {
        return Err(SolveRefusal::UnsupportedScope);
    }
    if context.entry_speed_mps < MINIMUM_SHAPED_SPEED_MPS {
        return Err(SolveRefusal::BelowMinimumSpeed {
            entry_speed_mps: context.entry_speed_mps,
            minimum_speed_mps: MINIMUM_SHAPED_SPEED_MPS,
        });
    }
    Ok(())
}

fn preview_candidate_is_confirmed(
    intent: &ElementIntent,
    context: &SolveContext<'_>,
    candidate: &SolvedElement,
) -> bool {
    let Some(seed) = candidate.calibration_seed.as_ref() else {
        return false;
    };
    if seed.preview.cars.len() != usize::from(context.train.cars)
        || !intent_matches_spec(intent, &candidate.spec, context.model, context.entry)
        || seed.preview.cars.iter().any(|car| !car.within_limits)
        || force_headroom_for_candidate(context.model, &seed.preview.cars)
            < REQUIRED_FORCE_HEADROOM_G
    {
        return false;
    }
    match (&candidate.spec, intent) {
        (ElementSpec::Hill { target_g, .. }, ElementIntent::Hill { .. }) => seed
            .preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min)
            .is_some_and(|delivered_g| {
                (delivered_g - target_g).abs() <= MAXIMUM_HILL_INTENT_ERROR_G
            }),
        (ElementSpec::Turn { .. }, ElementIntent::Turn { .. }) => {
            seed.preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_lateral_g)
                .fold(0.0, f64::max)
                <= MAXIMUM_FULLY_BANKED_LATERAL_G
        }
        _ => false,
    }
}

fn solve_hill(
    feel: HillFeel,
    strength_g: Option<f64>,
    size: HillSizeHint,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    match size {
        HillSizeHint::Auto => solve_hill_analytically(
            feel,
            strength_g,
            None,
            maximum_previews,
            provisional,
            preview_seed,
            context,
        ),
        HillSizeHint::Height(height_m) => {
            if !height_m.is_finite() || height_m <= 0.0 {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g: strength_g.unwrap_or(0.0),
                    available_headroom_g: 0.0,
                });
            }
            let energy_height_m = context.entry_speed_mps.powi(2) / (2.0 * GRAVITY);
            if height_m >= energy_height_m * 0.9 {
                return Err(SolveRefusal::Stall {
                    entry_speed_mps: context.entry_speed_mps,
                    height_m,
                });
            }
            solve_hill_analytically(
                feel,
                strength_g,
                Some(height_m),
                maximum_previews,
                provisional,
                preview_seed,
                context,
            )
        }
    }
}

fn solve_hill_analytically(
    feel: HillFeel,
    strength_g: Option<f64>,
    fixed_height_m: Option<f64>,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    if !matches!(
        feel,
        HillFeel::Floater | HillFeel::StrongFloater | HillFeel::Ejector
    ) {
        return Err(SolveRefusal::InvalidGeometry);
    }
    let requested_target_g = strength_g.unwrap_or(default_hill_strength(feel, context.model));
    if !requested_target_g.is_finite() {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: requested_target_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let target_floor =
        context.model.comfortable_force_limits.minimum_vertical_g + REQUIRED_FORCE_HEADROOM_G;
    let mut target_g = requested_target_g.min(-0.05);
    if target_g < target_floor {
        let cap_steps = ((target_floor - target_g) / 0.05).ceil();
        target_g += cap_steps * 0.05;
    }
    let seed = preview_seed.and_then(|candidate| {
        let ElementSpec::Hill { target_g, .. } = candidate.spec else {
            return None;
        };
        let seed = candidate.calibration_seed.as_ref()?;
        let crest_g = seed.hill_crest_g?;
        let delivered_g = seed
            .preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min)?;
        Some((target_g, delivered_g, crest_g, seed))
    });
    if let Some((seed_target_g, ..)) = seed {
        target_g = seed_target_g;
    }
    let mut preview_count = u8::from(seed.is_some());
    let mut previous_attempt = seed.map(|(_, delivered_g, crest_g, _)| (crest_g, delivered_g));
    let mut crest_g = target_g;
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_delivered_g = None;
    if let Some((_, delivered_g, seed_crest_g, seed)) = seed {
        last_headroom_g = force_headroom_for_candidate(context.model, &seed.preview.cars);
        last_delivered_g = Some(delivered_g);
        if last_headroom_g < REQUIRED_FORCE_HEADROOM_G
            || seed.preview.cars.iter().any(|car| !car.within_limits)
        {
            let next_target_g = (target_g + 0.05).min(-0.05);
            if feel == HillFeel::StrongFloater
                && context.entry_speed_mps >= 24.0
                && next_target_g > -0.75
            {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g: last_headroom_g,
                });
            }
            target_g = next_target_g;
            crest_g = target_g;
            previous_attempt = None;
        } else {
            crest_g = seed_crest_g + target_g - delivered_g;
        }
    }

    while preview_count < maximum_previews {
        let (profile, height_m, span_m) = hill_profile_for_height(
            context,
            design_speed_mps,
            crest_g,
            fixed_height_m.unwrap_or_else(|| {
                design_speed_mps.powi(2) / (2.0 * GRAVITY) * HILL_HEIGHT_ENERGY_FRACTION
            }),
        )?;
        let mut candidate_spec = ElementSpec::Hill {
            design_speed_mps,
            cars: context.train.cars,
            target_g,
            delivered_g: target_g,
            height_m,
            span_m,
            profile,
        };
        canonicalize_element_spec(&mut candidate_spec);
        let candidate = piece_from_spec(context.entry, &candidate_spec)?;
        let preview = calibrate_piece(context, &candidate)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let headroom_g = force_headroom_for_candidate(context.model, &preview.cars);
        last_headroom_g = headroom_g;
        let delivered_g = preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min)
            .ok_or(SolveRefusal::InfeasibleTarget {
                target_g,
                available_headroom_g: headroom_g,
            })?;
        last_delivered_g = Some(delivered_g);

        if provisional {
            let mut preview_spec = candidate_spec;
            let ElementSpec::Hill {
                height_m,
                span_m,
                profile,
                ..
            } = preview_spec
            else {
                unreachable!();
            };
            preview_spec = ElementSpec::Hill {
                design_speed_mps,
                cars: context.train.cars,
                target_g,
                delivered_g: round_element_value(delivered_g),
                height_m,
                span_m,
                profile,
            };
            canonicalize_element_spec(&mut preview_spec);
            let piece = piece_from_spec(context.entry, &preview_spec)?;
            let cap_note = ((requested_target_g - target_g).abs() > 1e-9).then_some(FeelCapNote {
                feel,
                requested_target_g,
                capped_target_g: target_g,
                delivered_g: round_element_value(delivered_g),
            });
            return Ok(SolvedElement {
                piece,
                spec: preview_spec,
                calibration_runs: preview_count,
                cap_note,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: Some(crest_g),
                }),
            });
        }

        if headroom_g < REQUIRED_FORCE_HEADROOM_G
            || preview.cars.iter().any(|car| !car.within_limits)
        {
            let next_target_g = (target_g + 0.05).min(-0.05);
            if feel == HillFeel::StrongFloater
                && context.entry_speed_mps >= 24.0
                && next_target_g > -0.75
            {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g: headroom_g,
                });
            }
            if (next_target_g - target_g).abs() < 1e-9 {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g: headroom_g,
                });
            }
            target_g = next_target_g;
            crest_g = target_g;
            previous_attempt = None;
            continue;
        }

        if (delivered_g - target_g).abs() <= MAXIMUM_HILL_INTENT_ERROR_G {
            let mut final_spec = candidate_spec;
            let ElementSpec::Hill {
                height_m,
                span_m,
                profile,
                ..
            } = final_spec
            else {
                unreachable!();
            };
            final_spec = ElementSpec::Hill {
                design_speed_mps,
                cars: context.train.cars,
                target_g,
                delivered_g: round_element_value(delivered_g),
                height_m,
                span_m,
                profile,
            };
            canonicalize_element_spec(&mut final_spec);
            let piece = piece_from_spec(context.entry, &final_spec)?;
            let cap_note = ((requested_target_g - target_g).abs() > 1e-9).then_some(FeelCapNote {
                feel,
                requested_target_g,
                capped_target_g: target_g,
                delivered_g: round_element_value(delivered_g),
            });
            return Ok(SolvedElement {
                piece,
                spec: final_spec,
                calibration_runs: preview_count,
                cap_note,
                calibration_seed: None,
            });
        }

        let next_crest_g = match previous_attempt {
            Some((previous_crest, previous_delivered)) => {
                let slope = (delivered_g - previous_delivered) / (crest_g - previous_crest);
                if !slope.is_finite() || slope.abs() < 1e-6 {
                    return Err(SolveRefusal::InfeasibleTarget {
                        target_g,
                        available_headroom_g: headroom_g,
                    });
                }
                crest_g + (target_g - delivered_g) / slope
            }
            None => crest_g + target_g - delivered_g,
        };
        previous_attempt = Some((crest_g, delivered_g));
        crest_g = next_crest_g;
    }

    Err(SolveRefusal::InfeasibleTarget {
        target_g: last_delivered_g.unwrap_or(target_g),
        available_headroom_g: last_headroom_g,
    })
}

fn default_hill_strength(feel: HillFeel, model: &CoasterModel) -> f64 {
    let rules = model.shaped_element_rules();
    match feel {
        HillFeel::Floater => rules.default_floater_g,
        HillFeel::StrongFloater => rules.default_strong_floater_g,
        HillFeel::Ejector if model.track_material == crate::track::Material::Wood => {
            rules.wooden_ejector_g
        }
        HillFeel::Ejector => rules.steel_ejector_g,
        _ => unreachable!("only supported hill feelings have defaults"),
    }
}

fn hill_profile_for_height(
    context: &SolveContext<'_>,
    design_speed_mps: f64,
    crest_g: f64,
    target_height_m: f64,
) -> Result<(SolvedProfile, f64, f64), SolveRefusal> {
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };
    let mut low_flank_degrees: f64 = 2.0;
    let mut high_flank_degrees: f64 = 75.0;
    let mut best: Option<(f64, SolvedProfile, f64)> = None;
    for _ in 0..SHAPED_FLANK_SEARCH_STEPS {
        let flank_degrees = (low_flank_degrees + high_flank_degrees) / 2.0;
        let profile_spec = OfflineHillSpec {
            entry_speed_mps: design_speed_mps,
            pull_g: SHAPED_HILL_PULL_G,
            crest_g,
            flank_angle_radians: flank_degrees.to_radians(),
            taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
        };
        let Ok((profile, _, _)) = solve_shaped_hill_offline_profile(profile_spec, train, 256)
        else {
            high_flank_degrees = flank_degrees;
            continue;
        };
        let height_m = profile
            .resolved_points()
            .iter()
            .map(|point| point[2])
            .fold(f64::NEG_INFINITY, f64::max);
        let error_m = (height_m - target_height_m).abs();
        if best
            .as_ref()
            .is_none_or(|(best_error, ..)| error_m < *best_error)
        {
            best = Some((error_m, profile.clone(), flank_degrees));
        }
        if height_m < target_height_m {
            low_flank_degrees = flank_degrees;
        } else {
            high_flank_degrees = flank_degrees;
        }
    }
    let (_, _, flank_degrees) = best.ok_or(SolveRefusal::InvalidGeometry)?;
    let profile_spec = OfflineHillSpec {
        entry_speed_mps: design_speed_mps,
        pull_g: SHAPED_HILL_PULL_G,
        crest_g,
        flank_angle_radians: flank_degrees.to_radians(),
        taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
    };
    let (mut profile, _, _) = solve_shaped_hill_offline_profile(profile_spec, train, PROFILE_STEPS)
        .map_err(|failure| match failure {
            crate::track::profile::OfflineSolveError::NonPositiveSpeedSquared { height_metres } => {
                SolveRefusal::Stall {
                    entry_speed_mps: design_speed_mps,
                    height_m: height_metres,
                }
            }
            crate::track::profile::OfflineSolveError::InvalidCurvature { .. }
            | crate::track::profile::OfflineSolveError::DidNotConverge => {
                SolveRefusal::InvalidGeometry
            }
        })?;
    let diagonal_scale = if context.entry.heading.index() % 2 == 0 {
        1.0
    } else {
        std::f64::consts::SQRT_2
    };
    let span_m = (profile.endpoint()[0] * diagonal_scale / (2.0 * diagonal_scale)).ceil()
        * 2.0
        * diagonal_scale;
    profile.append_level_straight(span_m / diagonal_scale - profile.endpoint()[0]);
    profile.round_to_nine_decimal_places();
    let height_m = profile
        .resolved_points()
        .iter()
        .map(|point| point[2])
        .fold(f64::NEG_INFINITY, f64::max);
    Ok((profile, height_m, span_m))
}
/// Hill height and span are stored at six decimals while the profile keeps nine, so a stored value matches
/// when it lies within half a six-decimal step of the profile value, plus slack for JSON parsing.
fn stored_value_matches(profile_value: f64, stored_value: f64) -> bool {
    (profile_value - stored_value).abs() <= ELEMENT_SPEC_HALF_DECIMAL_STEP + 1e-9
}

#[cfg(test)]
pub(crate) fn synthesize_hill_profile(
    design_speed_mps: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle: f64,
    train: OfflineTrain,
    connector_grid_scale: f64,
    connector_grid_span_metres: f64,
) -> Result<(SolvedProfile, f64, f64), SolveRefusal> {
    let spec = OfflineHillSpec {
        entry_speed_mps: design_speed_mps,
        pull_g,
        crest_g,
        flank_angle_radians: flank_angle,
        taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
    };
    let (mut profile, _, _) = solve_shaped_hill_offline_profile(spec, train, PROFILE_STEPS)
        .map_err(|failure| match failure {
            crate::track::profile::OfflineSolveError::NonPositiveSpeedSquared { height_metres } => {
                SolveRefusal::Stall {
                    entry_speed_mps: design_speed_mps,
                    height_m: height_metres,
                }
            }
            crate::track::profile::OfflineSolveError::InvalidCurvature { .. } => {
                SolveRefusal::InvalidGeometry
            }
            crate::track::profile::OfflineSolveError::DidNotConverge => {
                unreachable!("single profile integration does not iterate")
            }
        })?;
    let endpoint_x = profile.endpoint()[0];
    let span_metres = (endpoint_x * connector_grid_scale / connector_grid_span_metres).ceil()
        * connector_grid_span_metres;
    let connector_insert_metres = span_metres / connector_grid_scale - endpoint_x;
    profile.append_level_straight(connector_insert_metres);
    Ok((profile, span_metres, connector_insert_metres))
}

fn solve_turn(
    hand: Hand,
    size: TurnSizeHint,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let rules = context.model.shaped_element_rules();
    if !rules.allow_fully_banked_turns {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: 0.0,
            available_headroom_g: 0.0,
        });
    }
    if let TurnSizeHint::Radius(radius_m) = size {
        if !radius_m.is_finite() || radius_m <= 0.0 {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: rules.turn_vertical_g,
                available_headroom_g: 0.0,
            });
        }
    }
    let model_limits = context.model.comfortable_force_limits;
    let target_vertical_g = rules.turn_vertical_g;
    if target_vertical_g < model_limits.minimum_vertical_g + 0.5
        || target_vertical_g > model_limits.maximum_vertical_g - 0.5
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: (target_vertical_g - model_limits.minimum_vertical_g)
                .min(model_limits.maximum_vertical_g - target_vertical_g),
        });
    }
    let radius_step = if context.entry.heading.index() % 2 == 0 {
        2.0
    } else {
        2.0 * std::f64::consts::SQRT_2
    };
    let requested_radius_m = match size {
        TurnSizeHint::Auto => None,
        TurnSizeHint::Radius(radius_m) => Some(radius_m),
    };
    if requested_radius_m.is_some_and(|radius| {
        radius < rules.minimum_turn_radius_m || radius > rules.maximum_turn_radius_m
    }) {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: 0.0,
        });
    }

    let mut peak_bank = target_vertical_g
        .recip()
        .acos()
        .min(rules.maximum_bank_degrees.to_radians());
    let mut preview_count = 0_u8;
    if let Some(seed) = preview_seed
        && let ElementSpec::Turn { bank_degrees, .. } = seed.spec
    {
        preview_count = 1;
        let preview_lateral_g = seed
            .calibration_seed
            .as_ref()
            .map(|seed| {
                seed.preview
                    .cars
                    .iter()
                    .map(|car| car.peaks.maximum_lateral_g)
                    .fold(0.0, f64::max)
            })
            .unwrap_or(0.0);
        let adjustment = if requested_radius_m.is_some()
            && preview_lateral_g > MAXIMUM_FULLY_BANKED_LATERAL_G
        {
            1.1
        } else {
            0.9
        };
        peak_bank = (bank_degrees.to_radians().tan() * adjustment)
            .atan()
            .min(rules.maximum_bank_degrees.to_radians());
    }
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_lateral_g = f64::INFINITY;
    while preview_count < maximum_previews {
        let speed_loss_fraction =
            estimated_turn_speed_loss_fraction(context, design_speed_mps, peak_bank);
        let (geometry, radius_m, resolved_bank) = super::curves::shaped_speed_turn_geometry(
            design_speed_mps,
            peak_bank,
            speed_loss_fraction,
            radius_step,
            rules.minimum_turn_radius_m,
            requested_radius_m,
        );
        peak_bank = resolved_bank;
        if radius_m > rules.maximum_turn_radius_m {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: target_vertical_g,
                available_headroom_g: 0.0,
            });
        }
        let bank_degrees = peak_bank.to_degrees();
        if bank_degrees > rules.maximum_bank_degrees {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: target_vertical_g,
                available_headroom_g: rules.maximum_bank_degrees - bank_degrees,
            });
        }
        let mut candidate_spec = ElementSpec::Turn {
            design_speed_mps,
            cars: context.train.cars,
            hand,
            vertical_g: peak_bank.cos().recip(),
            lateral_g: 0.0,
            radius_m,
            bank_degrees,
            geometry,
        };
        canonicalize_element_spec(&mut candidate_spec);
        let piece = piece_from_spec(context.entry, &candidate_spec)?;
        let preview = calibrate_piece(context, &piece)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let lateral_g = preview
            .cars
            .iter()
            .map(|car| car.peaks.maximum_lateral_g)
            .fold(0.0, f64::max);
        let headroom_g = force_headroom_for_candidate(context.model, &preview.cars);
        last_headroom_g = headroom_g;
        last_lateral_g = lateral_g;
        let every_car_within_limits = preview.cars.iter().all(|car| car.within_limits);
        if provisional {
            let vertical_g = preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            let mut spec = ElementSpec::Turn {
                design_speed_mps,
                cars: context.train.cars,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry: match candidate_spec {
                    ElementSpec::Turn { geometry, .. } => geometry,
                    ElementSpec::Hill { .. } => unreachable!(),
                },
            };
            canonicalize_element_spec(&mut spec);
            return Ok(SolvedElement {
                piece: piece_from_spec(context.entry, &spec)?,
                spec,
                calibration_runs: preview_count,
                cap_note: None,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: None,
                }),
            });
        }
        if lateral_g <= MAXIMUM_FULLY_BANKED_LATERAL_G
            && headroom_g >= REQUIRED_FORCE_HEADROOM_G
            && every_car_within_limits
        {
            let vertical_g = preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            let mut spec = ElementSpec::Turn {
                design_speed_mps,
                cars: context.train.cars,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry: match candidate_spec {
                    ElementSpec::Turn { geometry, .. } => geometry,
                    ElementSpec::Hill { .. } => unreachable!(),
                },
            };
            canonicalize_element_spec(&mut spec);
            return Ok(SolvedElement {
                piece: piece_from_spec(context.entry, &spec)?,
                spec,
                calibration_runs: preview_count,
                cap_note: None,
                calibration_seed: None,
            });
        }
        if preview_count >= maximum_previews {
            break;
        }
        let adjustment = if requested_radius_m.is_some()
            && lateral_g > MAXIMUM_FULLY_BANKED_LATERAL_G
            && headroom_g >= REQUIRED_FORCE_HEADROOM_G
            && every_car_within_limits
        {
            1.1
        } else {
            0.9
        };
        peak_bank = (peak_bank.tan() * adjustment)
            .atan()
            .min(rules.maximum_bank_degrees.to_radians());
    }
    Err(SolveRefusal::InfeasibleTarget {
        target_g: target_vertical_g,
        available_headroom_g: last_headroom_g.min(MAXIMUM_FULLY_BANKED_LATERAL_G - last_lateral_g),
    })
}

fn estimated_turn_speed_loss_fraction(
    context: &SolveContext<'_>,
    design_speed_mps: f64,
    peak_bank: f64,
) -> f64 {
    let nominal_radius_m = design_speed_mps.powi(2) / (GRAVITY * peak_bank.tan());
    let turn_length_estimate_m = std::f64::consts::FRAC_PI_2 * nominal_radius_m;
    let deceleration_mps2 = context.physics.rolling_resistance * GRAVITY
        + context.physics.drag_coefficient * design_speed_mps.powi(2)
            / (context.physics.car_mass_kg * f64::from(context.train.cars));
    deceleration_mps2 * turn_length_estimate_m / design_speed_mps.powi(2)
}

pub(super) fn piece_from_spec(
    entry: TrackState,
    spec: &ElementSpec,
) -> Result<Piece, SolveRefusal> {
    let id = match spec {
        ElementSpec::Hill { .. } => PieceId::ShapedHill,
        ElementSpec::Turn { hand, .. } => match hand {
            Hand::Left => PieceId::ShapedTurnLeft,
            Hand::Right => PieceId::ShapedTurnRight,
        },
    };
    let template = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .ok_or(SolveRefusal::InvalidContext)?;
    let mut piece = template
        .at(entry)
        .ok_or(SolveRefusal::UnsupportedConnector)?;
    piece.id = id;
    piece.name = match id {
        PieceId::ShapedHill => SHAPED_HILL_NAME,
        PieceId::ShapedTurnLeft => SHAPED_LEFT_TURN_NAME,
        PieceId::ShapedTurnRight => SHAPED_RIGHT_TURN_NAME,
        _ => unreachable!(),
    };
    let spec = Arc::new(spec.clone());
    match spec.as_ref() {
        ElementSpec::Hill { profile, .. } => {
            let points = profile.resolved_points();
            if points.len() < PROFILE_STEPS * 2 + 1
                || profile.length() <= 0.0
                || profile.endpoint()[1].abs() > 0.01
            {
                return Err(SolveRefusal::InvalidContext);
            }
            piece.shape = Shape::ShapedProfile {
                spec: Arc::clone(&spec),
            };
            let ElementSpec::Hill { span_m, .. } = spec.as_ref() else {
                unreachable!()
            };
            piece.exit = hill_exit(entry, *span_m)?;
        }
        ElementSpec::Turn {
            hand,
            radius_m,
            geometry,
            ..
        } => {
            if geometry.knots.len() != super::curves::SPEED_TURN_STEPS + 1 {
                return Err(SolveRefusal::InvalidContext);
            }
            piece.shape = Shape::ResolvedBank {
                spec: Arc::clone(&spec),
            };
            piece.exit = turn_exit(entry, *hand, *radius_m)?;
        }
    }
    piece.kind = PieceKind::Track;
    piece.design_speeds = None;
    piece.family = None;
    piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
    piece.clear_arc_table();
    Ok(piece)
}

fn hill_exit(entry: TrackState, span_m: f64) -> Result<TrackState, SolveRefusal> {
    let diagonal_scale = if entry.heading.index() % 2 == 0 {
        1.0
    } else {
        std::f64::consts::SQRT_2
    };
    let grid_step_m = 2.0 * diagonal_scale;
    let half_tiles = (span_m / grid_step_m).round();
    if !half_tiles.is_finite()
        || half_tiles < 1.0
        || (span_m / grid_step_m - half_tiles).abs() > 1e-8
    {
        return Err(SolveRefusal::InvalidContext);
    }
    let tiles = half_tiles as i32;
    let (dx, dy) = grid_heading_delta(entry.heading, tiles);
    let mut exit = entry;
    exit.x += dx;
    exit.y += dy;
    exit.slope = Slope::Level;
    exit.bank = Bank::Level;
    exit.horizontal_curvature = HorizontalCurvature::Straight;
    exit.vertical_curvature = VerticalCurvature::Straight;
    Ok(exit)
}

fn turn_exit(entry: TrackState, hand: Hand, radius_m: f64) -> Result<TrackState, SolveRefusal> {
    let grid_step_m = if entry.heading.index() % 2 == 0 {
        2.0
    } else {
        2.0 * std::f64::consts::SQRT_2
    };
    let half_radius = radius_m / grid_step_m;
    let steps = half_radius.round();
    if !half_radius.is_finite()
        || steps < 1.0
        || !stored_radius_is_grid_aligned(radius_m, grid_step_m)
    {
        return Err(SolveRefusal::InvalidContext);
    }
    let turn_tiles = match hand {
        Hand::Left => steps as i32,
        Hand::Right => -(steps as i32),
    };
    let quarter_turn = match hand {
        Hand::Left => 2,
        Hand::Right => -2,
    };
    let (forward_x, forward_y) = grid_heading_delta(entry.heading, steps as i32);
    let (side_x, side_y) =
        grid_heading_delta(Heading::from_index(entry.heading.index() + 2), turn_tiles);
    let mut exit = entry;
    exit.x += forward_x + side_x;
    exit.y += forward_y + side_y;
    exit.heading = Heading::from_index(entry.heading.index() + quarter_turn);
    exit.slope = Slope::Level;
    exit.bank = Bank::Level;
    exit.horizontal_curvature = HorizontalCurvature::Straight;
    exit.vertical_curvature = VerticalCurvature::Straight;
    Ok(exit)
}

fn stored_radius_is_grid_aligned(radius_m: f64, grid_step_m: f64) -> bool {
    let nearest_grid_radius_m = (radius_m / grid_step_m).round() * grid_step_m;
    (radius_m - nearest_grid_radius_m).abs() <= ELEMENT_SPEC_HALF_DECIMAL_STEP + 1e-9
}

fn grid_heading_delta(heading: Heading, tiles: i32) -> (i32, i32) {
    match heading {
        Heading::East => (tiles, 0),
        Heading::NorthEast => (tiles, tiles),
        Heading::North => (0, tiles),
        Heading::NorthWest => (-tiles, tiles),
        Heading::West => (-tiles, 0),
        Heading::SouthWest => (-tiles, -tiles),
        Heading::South => (0, -tiles),
        Heading::SouthEast => (tiles, -tiles),
    }
}

fn calibrate_piece(
    context: &mut SolveContext<'_>,
    candidate: &Piece,
) -> Result<CandidateCalibrationPreview, SolveRefusal> {
    let entry_preview = context
        .preview_cache
        .preview(context.prefix, context.model, context.physics)
        .ok_or(SolveRefusal::InvalidContext)?;
    if entry_preview.status != PreviewStatus::ReachedEnd {
        return Err(SolveRefusal::PreviewTimedOut);
    }
    let observed_speed = entry_preview
        .end_speed_mps
        .ok_or(SolveRefusal::InvalidContext)?;
    if (observed_speed - context.entry_speed_mps).abs() > 0.01 {
        return Err(SolveRefusal::InvalidContext);
    }
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let runout_count = (context.train.length_metres / straight.length()).ceil() as usize + 2;
    let mut pieces = context.prefix.to_vec();
    pieces.push(candidate.clone());
    let mut state = candidate.exit;
    for _ in 0..runout_count {
        let piece = straight
            .at(state)
            .ok_or(SolveRefusal::UnsupportedConnector)?;
        state = piece.exit;
        pieces.push(piece);
    }
    let preview = context
        .preview_cache
        .preview_candidate(
            &pieces,
            context.model,
            context.physics,
            context.prefix.len(),
        )
        .ok_or(SolveRefusal::InvalidContext)?;
    match preview.status {
        PreviewStatus::ReachedEnd => Ok(preview),
        PreviewStatus::Stalled => {
            let height_m = match candidate.element_spec() {
                Some(ElementSpec::Hill { height_m, .. }) => *height_m,
                _ => 0.0,
            };
            Err(SolveRefusal::Stall {
                entry_speed_mps: context.entry_speed_mps,
                height_m,
            })
        }
        PreviewStatus::TimedOut => Err(SolveRefusal::PreviewTimedOut),
    }
}

fn force_headroom_for_candidate(model: &CoasterModel, cars: &[CandidateCarForces]) -> f64 {
    let limits = model.comfortable_force_limits;
    cars.iter()
        .map(|car| {
            [
                car.peaks.minimum_vertical_g - limits.minimum_vertical_g,
                limits.maximum_vertical_g - car.peaks.maximum_vertical_g,
                limits.maximum_lateral_g - car.peaks.maximum_lateral_g,
                limits.maximum_longitudinal_g - car.peaks.maximum_longitudinal_g,
            ]
            .into_iter()
            .fold(f64::INFINITY, f64::min)
        })
        .fold(f64::INFINITY, f64::min)
}

impl crate::track::construction::Construction {
    pub fn shape_candidate(
        &self,
        intent: &ElementIntent,
        entry_speed_mps: f64,
        physics: PhysicsConfig,
    ) -> Result<SolvedElement, SolveRefusal> {
        self.shape_candidate_with_mode(intent, entry_speed_mps, physics, false)
    }

    pub fn shape_candidate_preview(
        &self,
        intent: &ElementIntent,
        entry_speed_mps: f64,
        physics: PhysicsConfig,
    ) -> Result<SolvedElement, SolveRefusal> {
        self.shape_candidate_with_mode(intent, entry_speed_mps, physics, true)
    }

    fn shape_candidate_with_mode(
        &self,
        intent: &ElementIntent,
        entry_speed_mps: f64,
        physics: PhysicsConfig,
        provisional: bool,
    ) -> Result<SolvedElement, SolveRefusal> {
        let mut cache_key = StateHasher::new();
        cache_key.write_tag(b"shaped-candidate-v2");
        cache_key.write_u64(self.state_hash());
        cache_key.write_f64(entry_speed_mps);
        physics.hash_into(&mut cache_key);
        cache_key.write_bytes(&postcard::to_allocvec(intent).expect("shaped intent serializes"));
        let cache_key = cache_key.finish();
        let cached_entry = self
            .shaped_solve_cache
            .borrow()
            .as_ref()
            .filter(|cached_entry| cached_entry.cache_key == cache_key)
            .map(|cached_entry| (cached_entry.provisional, cached_entry.result.clone()));
        if let Some((cached_provisional, result)) = &cached_entry
            && provisional == *cached_provisional
        {
            return result.clone();
        }
        let mut cache = self.force_preview_cache_mut();
        let prefix = self.pieces();
        let entry = self.end();
        let model = self.model();
        let train = &model.train;
        let mut context = SolveContext {
            entry,
            entry_speed_mps,
            model,
            train,
            prefix,
            preview_cache: &mut cache,
            physics,
        };
        let result = match (provisional, cached_entry.as_ref()) {
            (true, _) => solve_preview(intent, &mut context),
            (false, Some((true, Ok(preview)))) if preview.calibration_seed.is_some() => {
                solve_confirm(intent, &mut context, preview)
            }
            (false, _) => solve(intent, &mut context),
        };
        *self.shaped_solve_cache.borrow_mut() =
            Some(crate::track::construction::ShapedSolveCacheEntry {
                cache_key,
                provisional,
                result: result.clone(),
            });
        result
    }

    pub fn append_shaped(
        &mut self,
        solved: &SolvedElement,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ShapedConstructionError> {
        self.append_resolved_piece(solved.piece.clone(), &solved.spec, world, obstacles)
    }

    pub fn piece_from_resolved_spec(&self, spec: &ElementSpec) -> Result<Piece, SolveRefusal> {
        if spec.cars() != self.model().train.cars {
            return Err(SolveRefusal::InvalidContext);
        }
        piece_from_spec(self.end(), spec)
    }

    pub fn append_resolved(
        &mut self,
        intent: &ElementIntent,
        spec: &ElementSpec,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ShapedConstructionError> {
        if !intent_matches_spec(intent, spec, self.model(), self.end()) {
            return Err(ShapedConstructionError::InvalidResolvedSpec);
        }
        let piece = piece_from_spec(self.end(), spec).map_err(ShapedConstructionError::Solve)?;
        self.append_resolved_piece(piece, spec, world, obstacles)
    }

    fn append_resolved_piece(
        &mut self,
        piece: Piece,
        spec: &ElementSpec,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ShapedConstructionError> {
        if self.is_closed() {
            return Err(ShapedConstructionError::Construction(
                ConstructionError::Closed,
            ));
        }
        if piece.entry != self.end() {
            return Err(ShapedConstructionError::Construction(
                ConstructionError::Unavailable,
            ));
        }
        if !self.model().allows_piece(&piece)
            || piece.maximum_curvature() > 1.0 / self.model().minimum_radius_metres
        {
            return Err(ShapedConstructionError::Construction(
                ConstructionError::ModelForbidden,
            ));
        }
        let validation = self
            .validate_piece(&piece, world, obstacles)
            .map_err(ShapedConstructionError::Construction)?;
        let would_tunnel = validation.tunnel_plan.would_tunnel();
        if piece.element_spec().is_none_or(|stored| stored != spec) {
            return Err(ShapedConstructionError::InvalidResolvedSpec);
        }
        self.append_validated_piece(piece, validation);
        Ok(would_tunnel)
    }
}

fn intent_matches_spec(
    intent: &ElementIntent,
    spec: &ElementSpec,
    model: &CoasterModel,
    entry: TrackState,
) -> bool {
    if spec.cars() != model.train.cars || !spec.design_speed_mps().is_finite() {
        return false;
    }
    match (intent, spec) {
        (
            ElementIntent::Hill {
                feel,
                strength_g,
                size,
            },
            ElementSpec::Hill {
                design_speed_mps,
                target_g,
                delivered_g,
                height_m,
                span_m,
                profile,
                ..
            },
        ) => {
            let rules = model.shaped_element_rules();
            let default_target = match feel {
                HillFeel::Floater => Some(rules.default_floater_g),
                HillFeel::StrongFloater => Some(rules.default_strong_floater_g),
                HillFeel::Ejector if model.track_material == crate::track::Material::Wood => {
                    Some(rules.wooden_ejector_g)
                }
                HillFeel::Ejector => Some(rules.steel_ejector_g),
                _ => None,
            };
            let Some(default_target) = default_target else {
                return false;
            };
            let requested_target_g = strength_g.unwrap_or(default_target);
            let cap_steps = (target_g - requested_target_g) / 0.05;
            let target_matches = (target_g - requested_target_g).abs() <= 1e-9
                || (cap_steps > 0.0
                    && (cap_steps - cap_steps.round()).abs() <= 1e-8
                    && *target_g <= -0.05
                    && (*feel != HillFeel::StrongFloater
                        || *design_speed_mps < 24.0
                        || *target_g <= -0.75));
            let size_matches = match size {
                HillSizeHint::Auto => true,
                HillSizeHint::Height(height) => {
                    height.is_finite() && (*height - *height_m).abs() <= 0.5
                }
            };
            let diagonal_scale = if entry.heading.index() % 2 == 0 {
                1.0
            } else {
                std::f64::consts::SQRT_2
            };
            let expected_span = profile.endpoint()[0] * diagonal_scale;
            let resolved_points = profile.resolved_points();
            let resolved_height = resolved_points
                .iter()
                .map(|point| point[2])
                .fold(f64::NEG_INFINITY, f64::max);
            let profile_is_valid = resolved_points.len() > PROFILE_STEPS * 2
                && resolved_points
                    .iter()
                    .flatten()
                    .all(|coordinate| coordinate.is_finite())
                && resolved_points[0][0] == 0.0
                && resolved_points[0][1] == 0.0
                && resolved_points[0][2] == 0.0
                && resolved_points
                    .windows(2)
                    .all(|pair| pair[0][0] < pair[1][0]);
            target_matches
                && size_matches
                && profile_is_valid
                && stored_value_matches(expected_span, *span_m)
                && profile.endpoint()[1].abs() <= 0.01
                && stored_value_matches(resolved_height, *height_m)
                && target_g.is_finite()
                && *target_g <= 0.0
                && delivered_g.is_finite()
                && (*delivered_g - *target_g).abs() <= MAXIMUM_HILL_INTENT_ERROR_G
                && (*delivered_g - model.comfortable_force_limits.minimum_vertical_g)
                    >= REQUIRED_FORCE_HEADROOM_G
                && height_m.is_finite()
                && *height_m > 0.0
                && span_m.is_finite()
                && *span_m > 0.0
        }
        (
            ElementIntent::Turn { hand, size },
            ElementSpec::Turn {
                hand: resolved_hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry,
                ..
            },
        ) => {
            if hand != resolved_hand || !radius_m.is_finite() || *radius_m <= 0.0 {
                return false;
            }
            let grid_step = if entry.heading.index() % 2 == 0 {
                2.0
            } else {
                2.0 * std::f64::consts::SQRT_2
            };
            turn_radius_matches_intent(*size, *radius_m, grid_step, model.shaped_element_rules())
                && geometry.knots.len() == super::curves::SPEED_TURN_STEPS + 1
                && geometry.length.is_finite()
                && geometry.length > 0.0
                && bank_degrees.is_finite()
                && *bank_degrees <= model.shaped_element_rules().maximum_bank_degrees
                && vertical_g.is_finite()
                && *vertical_g
                    <= model.comfortable_force_limits.maximum_vertical_g - REQUIRED_FORCE_HEADROOM_G
                && lateral_g.is_finite()
                && lateral_g.abs() <= MAXIMUM_FULLY_BANKED_LATERAL_G
                && geometry.knots.iter().all(|knot| {
                    knot.x.is_finite() && knot.y.is_finite() && knot.heading.is_finite()
                })
        }
        _ => false,
    }
}

fn turn_radius_matches_intent(
    size: TurnSizeHint,
    radius_m: f64,
    grid_step_m: f64,
    rules: ShapedElementRules,
) -> bool {
    if !radius_m.is_finite()
        || radius_m < rules.minimum_turn_radius_m
        || radius_m > rules.maximum_turn_radius_m
        || !stored_radius_is_grid_aligned(radius_m, grid_step_m)
    {
        return false;
    }
    match size {
        TurnSizeHint::Auto => true,
        TurnSizeHint::Radius(requested_radius_m) => {
            if !requested_radius_m.is_finite()
                || requested_radius_m < rules.minimum_turn_radius_m
                || requested_radius_m > rules.maximum_turn_radius_m
            {
                return false;
            }
            let fixed_grid_radius_m =
                super::curves::grid_radius_at_least(requested_radius_m, grid_step_m);
            (radius_m - fixed_grid_radius_m).abs() <= ELEMENT_SPEC_HALF_DECIMAL_STEP + 1e-9
        }
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;

    #[test]
    fn invalid_curvature_is_reported_as_invalid_geometry() {
        let flank_angle_radians = 30.0_f64.to_radians();
        let result = synthesize_hill_profile(
            20.0,
            (flank_angle_radians * 0.5).cos(),
            -0.5,
            flank_angle_radians,
            OfflineTrain::Legacy18MetreEnvelope,
            1.0,
            2.0,
        );
        assert_eq!(result, Err(SolveRefusal::InvalidGeometry));
    }
}

#[cfg(test)]
mod turn_radius_guard_tests {
    use super::*;

    fn test_rules() -> ShapedElementRules {
        ShapedElementRules {
            allow_airtime_hills: true,
            allow_fully_banked_turns: true,
            default_floater_g: -0.5,
            default_strong_floater_g: -0.85,
            steel_ejector_g: -1.2,
            wooden_ejector_g: -0.8,
            hill_pull_g: 2.0,
            turn_vertical_g: 3.0,
            maximum_bank_degrees: 75.0,
            minimum_turn_radius_m: 5.0,
            maximum_turn_radius_m: 256.0,
        }
    }

    #[test]
    fn explicit_radius_matches_its_grid_value_and_stays_within_model_limits() {
        let rules = test_rules();
        let size = TurnSizeHint::Radius(64.0);

        assert!(turn_radius_matches_intent(size, 64.0, 2.0, rules));
        assert!(!turn_radius_matches_intent(size, 80.0, 2.0, rules));
        assert!(!turn_radius_matches_intent(size, 62.0, 2.0, rules));
        assert!(!turn_radius_matches_intent(size, 258.0, 2.0, rules));
        assert!(!turn_radius_matches_intent(
            TurnSizeHint::Radius(258.0),
            256.0,
            2.0,
            rules
        ));
    }

    #[test]
    fn rounded_diagonal_radius_still_matches_its_grid() {
        let diagonal_grid_step_m = 2.0 * std::f64::consts::SQRT_2;
        let stored_radius_m = round_element_value(diagonal_grid_step_m * 4.0);
        let diagonal_entry = TrackState {
            heading: Heading::from_index(1),
            ..TrackState::origin()
        };

        assert!(stored_radius_is_grid_aligned(
            stored_radius_m,
            diagonal_grid_step_m
        ));
        assert!(turn_exit(diagonal_entry, Hand::Left, stored_radius_m).is_ok());
    }
}
