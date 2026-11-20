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
    integrate_shaped_hill_once_profile,
};
#[cfg(test)]
use crate::track::profile::{ProfileSpec, integrate_checked, reflect_hill};
pub(crate) use crate::track::shaped::precision::canonicalize_element_spec;
use crate::track::shaped::precision::round_element_value;
use crate::track::shaped_generator::{
    CellFailure, CellRefusal, GeneratedSpec, GridElement, lookup_generated_spec,
    lookup_turn_radius_override_base_spec,
};
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
#[cfg(test)]
pub(crate) const HILL_TAPER_ANGLE: f64 = SHAPED_HILL_TAPER_ANGLE;
const REQUIRED_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_HILL_INTENT_ERROR_G: f64 = 0.1;
const MAXIMUM_FULLY_BANKED_LATERAL_G: f64 = 0.1;
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
    TableUnavailable,
    Stall {
        entry_speed_mps: f64,
        height_m: f64,
    },
    InfeasibleTarget {
        target_g: f64,
        available_headroom_g: f64,
    },
    UnmeasuredTarget {
        target_g: f64,
    },
    BelowVerifiedSpeed {
        entry_speed_mps: f64,
        minimum_speed_mps: f64,
    },
    AboveVerifiedSpeed {
        entry_speed_mps: f64,
        maximum_speed_mps: f64,
    },
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
            default_strong_floater_g: -0.9,
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
    validate_context(context)?;
    match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => solve_hill(*feel, *strength_g, *size, context),
        ElementIntent::Turn { hand, size } => solve_turn(*hand, *size, context),
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

fn solve_hill(
    feel: HillFeel,
    strength_g: Option<f64>,
    size: HillSizeHint,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    match size {
        HillSizeHint::Auto => solve_hill_from_table(feel, strength_g, context),
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
            Err(SolveRefusal::UnsupportedSizeOverride)
        }
    }
}

fn solve_hill_from_table(
    feel: HillFeel,
    strength_g: Option<f64>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    if !matches!(
        feel,
        HillFeel::Floater | HillFeel::StrongFloater | HillFeel::Ejector
    ) {
        return Err(SolveRefusal::TableUnavailable);
    }
    let requested_target_g = strength_g.unwrap_or(default_hill_strength(feel, context.model));
    if !requested_target_g.is_finite() {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: requested_target_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let generated_spec = lookup_generated_spec(
        context.model.key,
        context.train.cars,
        GridElement::Hill {
            feel,
            strength_g: requested_target_g,
        },
        context.entry_speed_mps,
    )
    .map_err(|refusal| table_refusal_to_solve(refusal, context.entry_speed_mps))?;
    let GeneratedSpec::Hill {
        requested_target_g: table_requested_target_g,
        target_g,
        delivered_g: table_delivered_g,
        pull_g,
        crest_g,
        flank_angle_radians,
        ..
    } = generated_spec
    else {
        unreachable!("hill table key resolved to a turn")
    };
    assert!(
        (table_requested_target_g - requested_target_g).abs() < 1e-9,
        "generated hill row strength does not match its lookup key"
    );
    let (profile, height_m, span_m) = resolve_hill_profile(
        context,
        design_speed_mps,
        pull_g,
        crest_g,
        flank_angle_radians,
    )?;
    let mut spec = ElementSpec::Hill {
        design_speed_mps,
        cars: context.train.cars,
        target_g,
        delivered_g: table_delivered_g,
        height_m,
        span_m,
        profile,
    };
    canonicalize_element_spec(&mut spec);
    let candidate = piece_from_spec(context.entry, &spec)?;
    let preview = calibrate_piece(context, &candidate)?;
    if preview.cars.len() != usize::from(context.train.cars) {
        return Err(SolveRefusal::PreviewTimedOut);
    }
    let headroom_g = force_headroom_for_candidate(context.model, &preview.cars);
    let delivered_g = preview
        .cars
        .iter()
        .filter_map(|car| car.sustained_minimum_vertical_g)
        .reduce(f64::min);
    if !preview
        .cars
        .iter()
        .all(|car| car.sustained_minimum_vertical_g.is_some() && car.within_limits)
        || headroom_g < REQUIRED_FORCE_HEADROOM_G
        || delivered_g
            .is_none_or(|delivered| (delivered - target_g).abs() > MAXIMUM_HILL_INTENT_ERROR_G)
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g,
            available_headroom_g: headroom_g,
        });
    }
    let delivered_g = round_element_value(delivered_g.expect("validated delivery exists"));
    let mut spec = ElementSpec::Hill {
        design_speed_mps,
        cars: context.train.cars,
        target_g,
        delivered_g,
        height_m,
        span_m,
        profile: match spec {
            ElementSpec::Hill { profile, .. } => profile,
            ElementSpec::Turn { .. } => unreachable!(),
        },
    };
    canonicalize_element_spec(&mut spec);
    let piece = piece_from_spec(context.entry, &spec)?;
    let cap_note = ((requested_target_g - target_g).abs() > 1e-9).then_some(FeelCapNote {
        feel,
        requested_target_g,
        capped_target_g: target_g,
        delivered_g,
    });
    Ok(SolvedElement {
        piece,
        spec,
        calibration_runs: 1,
        cap_note,
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

fn table_refusal_to_solve(refusal: CellRefusal, entry_speed_mps: f64) -> SolveRefusal {
    match refusal {
        CellRefusal::TableUnavailable => SolveRefusal::TableUnavailable,
        CellRefusal::BelowFirstVerifiedSpeed { minimum_speed_mps } => {
            SolveRefusal::BelowVerifiedSpeed {
                entry_speed_mps,
                minimum_speed_mps,
            }
        }
        CellRefusal::AboveLastVerifiedSpeed { maximum_speed_mps } => {
            SolveRefusal::AboveVerifiedSpeed {
                entry_speed_mps,
                maximum_speed_mps,
            }
        }
        CellRefusal::InvalidGeometry { .. } => SolveRefusal::InvalidGeometry,
        CellRefusal::InfeasibleTarget {
            target_g,
            available_headroom_g,
            failure,
            ..
        } => match failure {
            CellFailure::OfflineSolver(
                crate::track::profile::OfflineSolveError::NonPositiveSpeedSquared { height_metres },
            ) => SolveRefusal::Stall {
                entry_speed_mps,
                height_m: height_metres,
            },
            CellFailure::OfflineSolver(
                crate::track::profile::OfflineSolveError::InvalidCurvature { .. },
            ) => SolveRefusal::InvalidGeometry,
            _ => match available_headroom_g {
                Some(available_headroom_g) => SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g,
                },
                None => SolveRefusal::UnmeasuredTarget { target_g },
            },
        },
    }
}
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
    integrate_shaped_hill_once_profile(
        spec,
        train,
        PROFILE_STEPS,
        connector_grid_scale,
        connector_grid_span_metres,
    )
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
    })
}

fn resolve_hill_profile(
    context: &SolveContext<'_>,
    design_speed_mps: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle: f64,
) -> Result<(SolvedProfile, f64, f64), SolveRefusal> {
    let diagonal_scale = if context.entry.heading.index() % 2 == 0 {
        1.0
    } else {
        std::f64::consts::SQRT_2
    };
    let grid_span_m = if diagonal_scale == 1.0 {
        2.0
    } else {
        2.0 * diagonal_scale
    };
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };
    let (profile, span_m, _) = synthesize_hill_profile(
        design_speed_mps,
        pull_g,
        crest_g,
        flank_angle,
        train,
        diagonal_scale,
        grid_span_m,
    )?;
    let height_m = profile
        .resolved_points()
        .iter()
        .map(|point| point[2])
        .fold(f64::NEG_INFINITY, f64::max);
    Ok((profile, height_m, span_m))
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HillCandidateMeasurement {
    pub(crate) delivered_g: Option<f64>,
    pub(crate) headroom_g: f64,
    pub(crate) every_car_within_limits: bool,
    pub(crate) height_m: f64,
    pub(crate) span_m: f64,
    pub(crate) profile_minimum_radius_m: f64,
    pub(crate) piece_minimum_radius_m: f64,
    pub(crate) model_minimum_radius_m: f64,
    pub(crate) profile_radius_within_limit: bool,
    pub(crate) piece_radius_within_limit: bool,
    pub(crate) cars: Vec<CandidateCarForces>,
}

#[cfg(test)]
pub(crate) fn measure_hill_candidate_with_taper(
    context: &mut SolveContext<'_>,
    target_g: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle: f64,
    taper_angle: f64,
) -> Result<HillCandidateMeasurement, SolveRefusal> {
    let (profile, height_m, span_m) = resolve_hill_profile_for_calibration(
        context,
        context.entry_speed_mps,
        pull_g,
        crest_g,
        flank_angle,
        taper_angle,
    )?;
    let profile_minimum_radius_m = profile.minimum_radius();
    let spec = ElementSpec::Hill {
        design_speed_mps: context.entry_speed_mps,
        cars: context.train.cars,
        target_g,
        delivered_g: target_g,
        height_m,
        span_m,
        profile,
    };
    let candidate = piece_from_spec(context.entry, &spec)?;
    let piece_minimum_radius_m = candidate.maximum_curvature().recip();
    let model_minimum_radius_m = context.model.minimum_radius_metres;
    let preview = calibrate_piece(context, &candidate)?;
    let delivered_g = preview
        .cars
        .iter()
        .filter_map(|car| car.sustained_minimum_vertical_g)
        .reduce(f64::min);
    Ok(HillCandidateMeasurement {
        delivered_g,
        headroom_g: force_headroom_for_candidate(context.model, &preview.cars),
        every_car_within_limits: preview.cars.len() == usize::from(context.train.cars)
            && preview.cars.iter().all(|car| car.within_limits),
        height_m,
        span_m,
        profile_minimum_radius_m,
        piece_minimum_radius_m,
        model_minimum_radius_m,
        profile_radius_within_limit: profile_minimum_radius_m >= model_minimum_radius_m,
        piece_radius_within_limit: piece_minimum_radius_m >= model_minimum_radius_m,
        cars: preview.cars,
    })
}

#[cfg(test)]
fn resolve_hill_profile_for_calibration(
    context: &SolveContext<'_>,
    design_speed_mps: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle: f64,
    taper_angle: f64,
) -> Result<(SolvedProfile, f64, f64), SolveRefusal> {
    let spec =
        ProfileSpec::hill_with_taper(design_speed_mps, pull_g, crest_g, flank_angle, taper_angle);
    let half_profile =
        integrate_checked(&spec, None, PROFILE_STEPS).map_err(|height_m| SolveRefusal::Stall {
            entry_speed_mps: design_speed_mps,
            height_m,
        })?;
    let mut profile = reflect_hill(&half_profile);
    let endpoint_x = profile.endpoint()[0];
    let diagonal_scale = if context.entry.heading.index() % 2 == 0 {
        1.0
    } else {
        std::f64::consts::SQRT_2
    };
    let grid_span_m = if diagonal_scale == 1.0 {
        2.0
    } else {
        2.0 * diagonal_scale
    };
    let span_m = (endpoint_x * diagonal_scale / grid_span_m).ceil() * grid_span_m;
    profile.append_level_straight(span_m / diagonal_scale - endpoint_x);
    let height_m = profile
        .resolved_points()
        .iter()
        .map(|point| point[2])
        .fold(f64::NEG_INFINITY, f64::max);
    Ok((profile, height_m, span_m))
}

fn solve_turn(
    hand: Hand,
    size: TurnSizeHint,
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
    let generated_spec = match size {
        TurnSizeHint::Auto => lookup_generated_spec(
            context.model.key,
            context.train.cars,
            GridElement::FullyBankedTurn,
            context.entry_speed_mps,
        ),
        TurnSizeHint::Radius(_) => lookup_turn_radius_override_base_spec(
            context.model.key,
            context.train.cars,
            context.entry_speed_mps,
        ),
    }
    .map_err(|refusal| table_refusal_to_solve(refusal, context.entry_speed_mps))?;
    let GeneratedSpec::Turn {
        radius_m: automatic_radius,
        ..
    } = generated_spec
    else {
        unreachable!("turn table key resolved to a hill")
    };
    let requested_radius = match size {
        TurnSizeHint::Auto => automatic_radius.max(rules.minimum_turn_radius_m),
        TurnSizeHint::Radius(radius_m) => radius_m,
    };
    let radius_m = (requested_radius / radius_step).ceil() * radius_step;
    if radius_m < rules.minimum_turn_radius_m || radius_m > rules.maximum_turn_radius_m {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: 0.0,
        });
    }
    let geometry = super::curves::speed_turn_geometry(radius_m, design_speed_mps);
    let geometry = (*geometry).clone();
    let bank_degrees = geometry.peak_bank.to_degrees();
    if bank_degrees > rules.maximum_bank_degrees {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: rules.maximum_bank_degrees - bank_degrees,
        });
    }
    let mut initial_spec = ElementSpec::Turn {
        design_speed_mps,
        cars: context.train.cars,
        hand,
        vertical_g: 1.0 / geometry.peak_bank.cos(),
        lateral_g: 0.0,
        radius_m,
        bank_degrees,
        geometry,
    };
    canonicalize_element_spec(&mut initial_spec);
    let piece = piece_from_spec(context.entry, &initial_spec)?;
    let preview = calibrate_piece(context, &piece)?;
    if preview.cars.len() != usize::from(context.train.cars) {
        return Err(SolveRefusal::PreviewTimedOut);
    }
    let lateral_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.maximum_lateral_g)
        .fold(0.0, f64::max);
    let available_headroom = force_headroom_for_candidate(context.model, &preview.cars);
    if lateral_g > MAXIMUM_FULLY_BANKED_LATERAL_G
        || available_headroom < REQUIRED_FORCE_HEADROOM_G
        || preview.cars.iter().any(|car| !car.within_limits)
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: available_headroom
                .min(MAXIMUM_FULLY_BANKED_LATERAL_G - lateral_g),
        });
    }
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
        geometry: match initial_spec {
            ElementSpec::Turn { geometry, .. } => geometry,
            ElementSpec::Hill { .. } => unreachable!(),
        },
    };
    canonicalize_element_spec(&mut spec);
    Ok(SolvedElement {
        piece: piece_from_spec(context.entry, &spec)?,
        spec,
        calibration_runs: 1,
        cap_note: None,
    })
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
    if !half_radius.is_finite() || steps < 1.0 || (half_radius - steps).abs() > 1e-8 {
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
        let mut cache_key = StateHasher::new();
        cache_key.write_tag(b"shaped-candidate-v1");
        cache_key.write_u64(self.state_hash());
        cache_key.write_f64(entry_speed_mps);
        physics.hash_into(&mut cache_key);
        cache_key.write_bytes(&postcard::to_allocvec(intent).expect("shaped intent serializes"));
        let cache_key = cache_key.finish();
        if let Some((cached_key, result)) = self.shaped_solve_cache.borrow().as_ref()
            && *cached_key == cache_key
        {
            return result.clone();
        }
        let mut cache = self.force_preview_cache_mut();
        let prefix = self.pieces();
        let entry = self.end();
        let model = self.model();
        let train = &model.train;
        let result = solve(
            intent,
            &mut SolveContext {
                entry,
                entry_speed_mps,
                model,
                train,
                prefix,
                preview_cache: &mut cache,
                physics,
            },
        );
        *self.shaped_solve_cache.borrow_mut() = Some((cache_key, result.clone()));
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
            let target_matches = (target_g - strength_g.unwrap_or(default_target)).abs() <= 1e-9;
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
                && (expected_span - *span_m).abs() <= 1e-8
                && profile.endpoint()[1].abs() <= 0.01
                && (resolved_height - *height_m).abs() <= 1e-8
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
            let radius_steps = radius_m / grid_step;
            let radius_is_grid_aligned = (radius_steps - radius_steps.round()).abs() <= 1e-8;
            let size_matches = match size {
                TurnSizeHint::Auto => true,
                TurnSizeHint::Radius(requested) => {
                    requested.is_finite()
                        && *radius_m >= *requested
                        && *radius_m - *requested < grid_step + 1e-8
                }
            };
            radius_is_grid_aligned
                && size_matches
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

#[cfg(test)]
mod calibration_coefficients;

#[cfg(test)]
mod refusal_tests {
    use super::*;

    #[test]
    fn unavailable_generated_table_row_keeps_its_distinct_runtime_refusal() {
        assert_eq!(
            table_refusal_to_solve(CellRefusal::TableUnavailable, 20.0),
            SolveRefusal::TableUnavailable
        );
    }

    #[test]
    fn missing_runtime_model_key_is_not_reported_as_infeasible() {
        let table_refusal = lookup_generated_spec(
            "missing-model-regression-key",
            4,
            GridElement::FullyBankedTurn,
            20.0,
        )
        .expect_err("the generated table has no matching model key");

        assert_eq!(
            table_refusal_to_solve(table_refusal, 20.0),
            SolveRefusal::TableUnavailable
        );
    }

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

    #[test]
    fn generated_invalid_curvature_is_not_reported_as_force_infeasibility() {
        let refusal = table_refusal_to_solve(
            CellRefusal::InfeasibleTarget {
                target_g: -0.5,
                delivered_g: None,
                available_headroom_g: None,
                failure: CellFailure::OfflineSolver(
                    crate::track::profile::OfflineSolveError::InvalidCurvature {
                        angle_radians: 0.0,
                        curvature_per_metre: 0.0,
                        height_metres: 0.0,
                    },
                ),
            },
            20.0,
        );
        assert_eq!(refusal, SolveRefusal::InvalidGeometry);
    }

    #[test]
    fn missing_measurements_are_not_reported_as_zero_force_headroom() {
        let refusal = table_refusal_to_solve(
            CellRefusal::InfeasibleTarget {
                target_g: -0.5,
                delivered_g: None,
                available_headroom_g: None,
                failure: CellFailure::PreviewUnavailable,
            },
            20.0,
        );
        assert_eq!(refusal, SolveRefusal::UnmeasuredTarget { target_g: -0.5 });
    }

    #[test]
    fn unmeasurable_table_geometry_preserves_the_invalid_geometry_refusal() {
        let refusal = table_refusal_to_solve(
            CellRefusal::InvalidGeometry {
                attempts: 1,
                failure: CellFailure::OfflineSolver(
                    crate::track::profile::OfflineSolveError::InvalidCurvature {
                        angle_radians: 0.0,
                        curvature_per_metre: 0.0,
                        height_metres: 0.0,
                    },
                ),
            },
            20.0,
        );
        assert_eq!(refusal, SolveRefusal::InvalidGeometry);
    }
}
