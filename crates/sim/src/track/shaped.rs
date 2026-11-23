use std::cell::Cell;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::physics::{
    CandidateCalibrationPreview, CandidateCarForces, GRAVITY, PhysicsConfig, PreviewCache,
    PreviewStatus,
};
use crate::state_hash::StateHasher;
use crate::track::construction::ConstructionError;
use crate::track::profile::{
    OfflineHillSpec, OfflineTrain, SHAPED_HILL_TAPER_ANGLE, SolvedProfile,
    solve_shaped_hill_apex_height_offline, solve_shaped_hill_offline_profile,
};
use crate::track::roll::Twist;
pub(crate) use crate::track::shaped::precision::canonicalize_element_spec;
use crate::track::shaped::precision::{ELEMENT_SPEC_HALF_DECIMAL_STEP, round_element_value};
use crate::track::supports::SupportObstacle;
use crate::track::{
    Bank, CoasterModel, ENVELOPE_DISPLAY_LIMITS, Hand, Heading, Heartlining, HillFeel,
    HorizontalCurvature, Inversion, Material, Piece, PieceId, PieceKind, Shape, Slope, TrackState,
    TrainConfiguration, VerticalCurvature, catalog,
};

std::thread_local! {
    static LEGACY_V4_SAVE_DECODE_ACTIVE: Cell<Option<bool>> = const { Cell::new(None) };
    static CURRENT_SAVE_DECODE_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CURRENT_SAVE_DIGEST_MISMATCH: RefCell<Option<GeometryDigestMismatch>> = const { RefCell::new(None) };
}

struct LegacyV4SaveDecodeGuard;

impl Drop for LegacyV4SaveDecodeGuard {
    fn drop(&mut self) {
        LEGACY_V4_SAVE_DECODE_ACTIVE.with(|active| active.set(None));
    }
}

pub fn with_legacy_v4_save_decode_context<T>(decode: impl FnOnce() -> T) -> (T, bool) {
    let entered = LEGACY_V4_SAVE_DECODE_ACTIVE.with(|active| {
        if active.get().is_some() {
            false
        } else {
            active.set(Some(false));
            true
        }
    });
    assert!(entered, "legacy v4 save decode contexts cannot be nested");
    let guard = LegacyV4SaveDecodeGuard;
    let result = decode();
    let shaped_element_seen = LEGACY_V4_SAVE_DECODE_ACTIVE.with(|active| {
        active
            .get()
            .expect("legacy v4 save decode context remains active")
    });
    drop(guard);
    (result, shaped_element_seen)
}

fn reject_shaped_element_in_legacy_v4_save() -> bool {
    LEGACY_V4_SAVE_DECODE_ACTIVE.with(|active| match active.get() {
        Some(_) => {
            active.set(Some(true));
            true
        }
        None => false,
    })
}

struct CurrentSaveDecodeGuard;

impl Drop for CurrentSaveDecodeGuard {
    fn drop(&mut self) {
        CURRENT_SAVE_DECODE_ACTIVE.with(|active| active.set(false));
        CURRENT_SAVE_DIGEST_MISMATCH.with(|mismatch| mismatch.borrow_mut().take());
    }
}

pub fn with_current_save_decode_context<T>(
    decode: impl FnOnce() -> T,
) -> (T, Option<GeometryDigestMismatch>) {
    let entered = CURRENT_SAVE_DECODE_ACTIVE.with(|active| {
        if active.get() {
            false
        } else {
            active.set(true);
            true
        }
    });
    assert!(entered, "current save decode contexts cannot be nested");
    CURRENT_SAVE_DIGEST_MISMATCH.with(|mismatch| mismatch.borrow_mut().take());
    let guard = CurrentSaveDecodeGuard;
    let result = decode();
    let mismatch = CURRENT_SAVE_DIGEST_MISMATCH.with(|mismatch| *mismatch.borrow());
    drop(guard);
    (result, mismatch)
}

fn capture_current_save_digest_mismatch(mismatch: GeometryDigestMismatch) -> bool {
    CURRENT_SAVE_DECODE_ACTIVE.with(|active| {
        if !active.get() {
            return false;
        }
        CURRENT_SAVE_DIGEST_MISMATCH.with(|recorded| {
            let mut recorded = recorded.borrow_mut();
            if recorded.is_none() {
                *recorded = Some(mismatch);
            }
        });
        true
    })
}
use crate::world::World;

mod inversions;
mod precision;

pub use crate::track::curves::{SpeedTurnGeometry as ResolvedTurnGeometry, SpeedTurnKnot};
pub use crate::track::profile::SolvedProfile as ResolvedHillProfile;

const PROFILE_STEPS: usize = 512;
const REQUIRED_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_HILL_INTENT_ERROR_G: f64 = 0.05;
const MAXIMUM_FULLY_BANKED_LATERAL_G: f64 = 0.15;
const MAXIMUM_SHAPED_PREVIEWS: u8 = 4;
const SHAPED_FLANK_SEARCH_STEPS: usize = 8;
const SHAPED_HILL_NAME: &str = "Shaped airtime hill";
const SHAPED_LEFT_TURN_NAME: &str = "Shaped banked turn left";
const SHAPED_RIGHT_TURN_NAME: &str = "Shaped banked turn right";
const SHAPED_LEFT_LOOP_NAME: &str = "Shaped vertical loop left";
const SHAPED_RIGHT_LOOP_NAME: &str = "Shaped vertical loop right";
const SHAPED_LEFT_STEEL_LOOP_NAME: &str = "Shaped steel-structure loop left";
const SHAPED_RIGHT_STEEL_LOOP_NAME: &str = "Shaped steel-structure loop right";
const SHAPED_LEFT_ZERO_G_ROLL_NAME: &str = "Shaped zero-g roll left";
const SHAPED_RIGHT_ZERO_G_ROLL_NAME: &str = "Shaped zero-g roll right";
const SHAPED_LEFT_RAVEN_TURN_NAME: &str = "Shaped Raven turn left";
const SHAPED_RIGHT_RAVEN_TURN_NAME: &str = "Shaped Raven turn right";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoopTopFeel {
    Floaty,
    Neutral,
    PressedIn,
}

impl LoopTopFeel {
    pub fn target_g(self) -> f64 {
        match self {
            Self::Floaty => 0.3,
            Self::Neutral => 1.0,
            Self::PressedIn => 1.5,
        }
    }
}

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
#[serde(rename_all = "camelCase")]
pub enum ElementIntent {
    Hill {
        feel: HillFeel,
        strength_g: f64,
        size: HillSizeHint,
    },
    Turn {
        hand: Hand,
        size: TurnSizeHint,
    },
    Loop {
        hand: Hand,
        peak_g: f64,
        top_feel: LoopTopFeel,
    },
    ZeroGRoll {
        hand: Hand,
        peak_g: f64,
    },
    RavenTurn {
        hand: Hand,
        peak_g: f64,
        top_feel: LoopTopFeel,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElementSpec {
    Hill {
        design_speed_mps: f64,
        cars: u8,
        train_length_metres: f64,
        target_g: f64,
        delivered_g: f64,
        height_m: f64,
        span_m: f64,
        target_height_m: f64,
        pull_g: f64,
        crest_g: f64,
        flank_angle_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        geometry_digest: u64,
        profile: ResolvedHillProfile,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        train_length_metres: f64,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        geometry_peak_bank_radians: f64,
        geometry_speed_loss_fraction: f64,
        geometry_radius_m: f64,
        geometry_digest: u64,
        geometry: ResolvedTurnGeometry,
    },
    Loop {
        design_speed_mps: f64,
        cars: u8,
        train_length_metres: f64,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        height_m: f64,
        span_m: f64,
        bottom_g: f64,
        top_g: f64,
        hold_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        steel_structure: bool,
        geometry_digest: u64,
        profile: ResolvedHillProfile,
    },
    ZeroGRoll {
        design_speed_mps: f64,
        cars: u8,
        train_length_metres: f64,
        hand: Hand,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_crest_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        pull_g: f64,
        flank_angle_degrees: f64,
        twist_ramp: f64,
        twist_start_m: f64,
        twist_end_m: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        geometry_digest: u64,
        profile: ResolvedHillProfile,
    },
    RavenTurn {
        design_speed_mps: f64,
        cars: u8,
        train_length_metres: f64,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        bottom_g: f64,
        top_g: f64,
        hold_degrees: f64,
        vertical_insert_m: f64,
        exit_dx_half_tiles: i32,
        exit_dy_half_tiles: i32,
        exit_dz_half_tiles: i32,
        roll_dx_m: f64,
        roll_lateral_m: f64,
        diagonal: bool,
        geometry_digest: u64,
        half_loop: ResolvedHillProfile,
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
        #[serde(with = "recipe_f64")]
        train_length_metres: f64,
        #[serde(with = "recipe_f64")]
        target_height_m: f64,
        #[serde(with = "recipe_f64")]
        pull_g: f64,
        #[serde(with = "recipe_f64")]
        crest_g: f64,
        #[serde(with = "recipe_f64")]
        flank_angle_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        #[serde(with = "digest_u64")]
        geometry_digest: u64,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        #[serde(with = "recipe_f64")]
        train_length_metres: f64,
        #[serde(with = "recipe_f64")]
        geometry_peak_bank_radians: f64,
        #[serde(with = "recipe_f64")]
        geometry_speed_loss_fraction: f64,
        #[serde(with = "recipe_f64")]
        geometry_radius_m: f64,
        #[serde(with = "digest_u64")]
        geometry_digest: u64,
    },
    Loop {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        height_m: f64,
        span_m: f64,
        #[serde(with = "recipe_f64")]
        train_length_metres: f64,
        #[serde(with = "recipe_f64")]
        bottom_g: f64,
        #[serde(with = "recipe_f64")]
        top_g: f64,
        #[serde(with = "recipe_f64")]
        hold_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        steel_structure: bool,
        #[serde(with = "digest_u64")]
        geometry_digest: u64,
    },
    ZeroGRoll {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_crest_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        #[serde(with = "recipe_f64")]
        train_length_metres: f64,
        #[serde(with = "recipe_f64")]
        pull_g: f64,
        #[serde(with = "recipe_f64")]
        flank_angle_degrees: f64,
        #[serde(with = "recipe_f64")]
        twist_ramp: f64,
        #[serde(with = "recipe_f64")]
        twist_start_m: f64,
        #[serde(with = "recipe_f64")]
        twist_end_m: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        #[serde(with = "digest_u64")]
        geometry_digest: u64,
    },
    RavenTurn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        #[serde(with = "recipe_f64")]
        train_length_metres: f64,
        #[serde(with = "recipe_f64")]
        bottom_g: f64,
        #[serde(with = "recipe_f64")]
        top_g: f64,
        #[serde(with = "recipe_f64")]
        hold_degrees: f64,
        #[serde(with = "recipe_f64")]
        vertical_insert_m: f64,
        exit_dx_half_tiles: i32,
        exit_dy_half_tiles: i32,
        exit_dz_half_tiles: i32,
        #[serde(with = "recipe_f64")]
        roll_dx_m: f64,
        #[serde(with = "recipe_f64")]
        roll_lateral_m: f64,
        diagonal: bool,
        #[serde(with = "digest_u64")]
        geometry_digest: u64,
    },
}

mod recipe_f64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let value = encoded
            .parse::<f64>()
            .map_err(<D::Error as serde::de::Error>::custom)?;
        if !value.is_finite() {
            return Err(<D::Error as serde::de::Error>::custom(
                "shaped recipe parameter must be finite",
            ));
        }
        Ok(value)
    }
}

mod digest_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let value = encoded
            .parse::<u64>()
            .map_err(<D::Error as serde::de::Error>::custom)?;
        if value.to_string() != encoded {
            return Err(<D::Error as serde::de::Error>::custom(
                "geometry digest must be a canonical decimal u64 string",
            ));
        }
        Ok(value)
    }
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
        train_length_metres: f64,
        target_height_m: f64,
        pull_g: f64,
        crest_g: f64,
        flank_angle_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        geometry_digest: u64,
    },
    Turn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        vertical_g: f64,
        lateral_g: f64,
        radius_m: f64,
        bank_degrees: f64,
        train_length_metres: f64,
        geometry_peak_bank_radians: f64,
        geometry_speed_loss_fraction: f64,
        geometry_radius_m: f64,
        geometry_digest: u64,
    },
    Loop {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        height_m: f64,
        span_m: f64,
        train_length_metres: f64,
        bottom_g: f64,
        top_g: f64,
        hold_degrees: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        steel_structure: bool,
        geometry_digest: u64,
    },
    ZeroGRoll {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_crest_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        train_length_metres: f64,
        pull_g: f64,
        flank_angle_degrees: f64,
        twist_ramp: f64,
        twist_start_m: f64,
        twist_end_m: f64,
        profile_grid_half_tiles: u16,
        diagonal: bool,
        geometry_digest: u64,
    },
    RavenTurn {
        design_speed_mps: f64,
        cars: u8,
        hand: Hand,
        top_feel: LoopTopFeel,
        target_peak_g: f64,
        delivered_peak_g: f64,
        delivered_top_g: f64,
        peak_roll_rate_deg_s: f64,
        height_m: f64,
        span_m: f64,
        train_length_metres: f64,
        bottom_g: f64,
        top_g: f64,
        hold_degrees: f64,
        vertical_insert_m: f64,
        exit_dx_half_tiles: i32,
        exit_dy_half_tiles: i32,
        exit_dz_half_tiles: i32,
        roll_dx_m: f64,
        roll_lateral_m: f64,
        diagonal: bool,
        geometry_digest: u64,
    },
}

fn resolve_human_readable_spec(
    wire: HumanReadableElementSpec,
) -> Result<ElementSpec, ElementSpecLoadError> {
    let mut spec = match wire {
        HumanReadableElementSpec::Hill {
            design_speed_mps,
            cars,
            train_length_metres,
            target_g,
            delivered_g,
            height_m,
            span_m,
            target_height_m,
            pull_g,
            crest_g,
            flank_angle_degrees,
            profile_grid_half_tiles,
            diagonal,
            geometry_digest,
        } => ElementSpec::Hill {
            design_speed_mps,
            cars,
            train_length_metres,
            target_g,
            delivered_g,
            height_m,
            span_m,
            target_height_m,
            pull_g,
            crest_g,
            flank_angle_degrees,
            profile_grid_half_tiles,
            diagonal,
            geometry_digest,
            profile: regenerate_hill_profile(
                cars,
                design_speed_mps,
                train_length_metres,
                pull_g,
                crest_g,
                flank_angle_degrees,
                profile_grid_half_tiles,
            )
            .map_err(ElementSpecLoadError::InvalidRecipe)?,
        },
        HumanReadableElementSpec::Turn {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
            geometry_peak_bank_radians,
            geometry_speed_loss_fraction,
            geometry_radius_m,
            geometry_digest,
        } => ElementSpec::Turn {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
            geometry_peak_bank_radians,
            geometry_speed_loss_fraction,
            geometry_radius_m,
            geometry_digest,
            geometry: super::curves::regenerate_shaped_speed_turn_geometry(
                round_element_value(design_speed_mps),
                geometry_peak_bank_radians,
                geometry_speed_loss_fraction,
                geometry_radius_m,
            )
            .map_err(ElementSpecLoadError::InvalidRecipe)?,
        },
        HumanReadableElementSpec::Loop {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            height_m,
            span_m,
            bottom_g,
            top_g,
            hold_degrees,
            profile_grid_half_tiles,
            diagonal,
            steel_structure,
            geometry_digest,
        } => ElementSpec::Loop {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            height_m,
            span_m,
            bottom_g,
            top_g,
            hold_degrees,
            profile_grid_half_tiles,
            diagonal,
            steel_structure,
            geometry_digest,
            profile: inversions::regenerate_loop_profile(
                cars,
                design_speed_mps,
                train_length_metres,
                bottom_g,
                top_g,
                hold_degrees,
                profile_grid_half_tiles,
                diagonal,
            )
            .map_err(ElementSpecLoadError::InvalidRecipe)?,
        },
        HumanReadableElementSpec::ZeroGRoll {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            target_peak_g,
            delivered_peak_g,
            delivered_crest_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            pull_g,
            flank_angle_degrees,
            twist_ramp,
            twist_start_m,
            twist_end_m,
            profile_grid_half_tiles,
            diagonal,
            geometry_digest,
        } => {
            let profile = inversions::regenerate_zero_g_roll_profile(
                cars,
                design_speed_mps,
                train_length_metres,
                pull_g,
                flank_angle_degrees,
                twist_ramp,
                twist_start_m,
                twist_end_m,
                profile_grid_half_tiles,
                diagonal,
            )
            .map_err(ElementSpecLoadError::InvalidRecipe)?;
            ElementSpec::ZeroGRoll {
                design_speed_mps,
                cars,
                train_length_metres,
                hand,
                target_peak_g,
                delivered_peak_g,
                delivered_crest_g,
                peak_roll_rate_deg_s,
                height_m,
                span_m,
                pull_g,
                flank_angle_degrees,
                twist_ramp,
                twist_start_m,
                twist_end_m,
                profile_grid_half_tiles,
                diagonal,
                geometry_digest,
                profile,
            }
        }
        HumanReadableElementSpec::RavenTurn {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            bottom_g,
            top_g,
            hold_degrees,
            vertical_insert_m,
            exit_dx_half_tiles,
            exit_dy_half_tiles,
            exit_dz_half_tiles,
            roll_dx_m,
            roll_lateral_m,
            diagonal,
            geometry_digest,
        } => ElementSpec::RavenTurn {
            design_speed_mps,
            cars,
            train_length_metres,
            hand,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            bottom_g,
            top_g,
            hold_degrees,
            vertical_insert_m,
            exit_dx_half_tiles,
            exit_dy_half_tiles,
            exit_dz_half_tiles,
            roll_dx_m,
            roll_lateral_m,
            diagonal,
            geometry_digest,
            half_loop: inversions::regenerate_raven_turn_half_loop(
                cars,
                design_speed_mps,
                train_length_metres,
                bottom_g,
                top_g,
                hold_degrees,
                vertical_insert_m,
                exit_dz_half_tiles,
            )
            .map_err(ElementSpecLoadError::InvalidRecipe)?,
        },
    };
    canonicalize_element_spec(&mut spec);
    spec.verify_geometry_digest()
        .map_err(ElementSpecLoadError::GeometryDigestMismatch)?;
    Ok(spec)
}

fn regenerate_hill_profile(
    cars: u8,
    design_speed_mps: f64,
    train_length_metres: f64,
    pull_g: f64,
    crest_g: f64,
    flank_angle_degrees: f64,
    profile_grid_half_tiles: u16,
) -> Result<ResolvedHillProfile, String> {
    let entry_speed_mps = round_element_value(design_speed_mps);
    if cars == 0
        || !entry_speed_mps.is_finite()
        || entry_speed_mps <= 0.0
        || !train_length_metres.is_finite()
        || train_length_metres <= 0.0
        || !pull_g.is_finite()
        || !crest_g.is_finite()
        || !flank_angle_degrees.is_finite()
        || !(0.0..90.0).contains(&flank_angle_degrees)
        || profile_grid_half_tiles == 0
    {
        return Err("shaped hill recipe contains invalid parameters".into());
    }
    let profile_spec = OfflineHillSpec {
        entry_speed_mps,
        pull_g,
        crest_g,
        flank_angle_radians: flank_angle_degrees.to_radians(),
        taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
    };
    let train = OfflineTrain::Coaster {
        cars,
        length_metres: train_length_metres,
    };
    let (mut profile, _, _) = solve_shaped_hill_offline_profile(profile_spec, train, PROFILE_STEPS)
        .map_err(|error| format!("could not regenerate shaped hill geometry: {error:?}"))?;
    let snapped_span_metres = f64::from(profile_grid_half_tiles) * 2.0;
    let connector_insert_metres = snapped_span_metres - profile.endpoint()[0];
    if !connector_insert_metres.is_finite() || connector_insert_metres < 0.0 {
        return Err("shaped hill recipe has an invalid grid span".into());
    }
    profile.append_level_straight(connector_insert_metres);
    profile.round_to_nine_decimal_places();
    Ok(profile)
}

impl Serialize for ElementSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.verify_geometry_digest()
            .map_err(serde::ser::Error::custom)?;
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
                    train_length_metres,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    target_height_m,
                    pull_g,
                    crest_g,
                    flank_angle_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                    ..
                } => HumanReadableElementSpec::Hill {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    target_g: *target_g,
                    delivered_g: *delivered_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    target_height_m: *target_height_m,
                    pull_g: *pull_g,
                    crest_g: *crest_g,
                    flank_angle_degrees: *flank_angle_degrees,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
                },
                Self::Turn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry_peak_bank_radians,
                    geometry_speed_loss_fraction,
                    geometry_radius_m,
                    geometry_digest,
                    ..
                } => HumanReadableElementSpec::Turn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    vertical_g: *vertical_g,
                    lateral_g: *lateral_g,
                    radius_m: *radius_m,
                    bank_degrees: *bank_degrees,
                    geometry_peak_bank_radians: *geometry_peak_bank_radians,
                    geometry_speed_loss_fraction: *geometry_speed_loss_fraction,
                    geometry_radius_m: *geometry_radius_m,
                    geometry_digest: *geometry_digest,
                },
                Self::Loop {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    steel_structure,
                    geometry_digest,
                    ..
                } => HumanReadableElementSpec::Loop {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    top_feel: *top_feel,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_top_g: *delivered_top_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    bottom_g: *bottom_g,
                    top_g: *top_g,
                    hold_degrees: *hold_degrees,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    steel_structure: *steel_structure,
                    geometry_digest: *geometry_digest,
                },
                Self::ZeroGRoll {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_crest_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    pull_g,
                    flank_angle_degrees,
                    twist_ramp,
                    twist_start_m,
                    twist_end_m,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                    ..
                } => HumanReadableElementSpec::ZeroGRoll {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_crest_g: *delivered_crest_g,
                    peak_roll_rate_deg_s: *peak_roll_rate_deg_s,
                    height_m: *height_m,
                    span_m: *span_m,
                    pull_g: *pull_g,
                    flank_angle_degrees: *flank_angle_degrees,
                    twist_ramp: *twist_ramp,
                    twist_start_m: *twist_start_m,
                    twist_end_m: *twist_end_m,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
                },
                Self::RavenTurn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    vertical_insert_m,
                    exit_dx_half_tiles,
                    exit_dy_half_tiles,
                    exit_dz_half_tiles,
                    roll_dx_m,
                    roll_lateral_m,
                    diagonal,
                    geometry_digest,
                    ..
                } => HumanReadableElementSpec::RavenTurn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    top_feel: *top_feel,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_top_g: *delivered_top_g,
                    peak_roll_rate_deg_s: *peak_roll_rate_deg_s,
                    height_m: *height_m,
                    span_m: *span_m,
                    bottom_g: *bottom_g,
                    top_g: *top_g,
                    hold_degrees: *hold_degrees,
                    vertical_insert_m: *vertical_insert_m,
                    exit_dx_half_tiles: *exit_dx_half_tiles,
                    exit_dy_half_tiles: *exit_dy_half_tiles,
                    exit_dz_half_tiles: *exit_dz_half_tiles,
                    roll_dx_m: *roll_dx_m,
                    roll_lateral_m: *roll_lateral_m,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
                },
            };
            spec.serialize(serializer)
        } else {
            let spec = match serializable_spec {
                Self::Hill {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    target_height_m,
                    pull_g,
                    crest_g,
                    flank_angle_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                    ..
                } => BinaryElementSpec::Hill {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    target_g: *target_g,
                    delivered_g: *delivered_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    target_height_m: *target_height_m,
                    pull_g: *pull_g,
                    crest_g: *crest_g,
                    flank_angle_degrees: *flank_angle_degrees,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
                },
                Self::Turn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry_peak_bank_radians,
                    geometry_speed_loss_fraction,
                    geometry_radius_m,
                    geometry_digest,
                    ..
                } => BinaryElementSpec::Turn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    vertical_g: *vertical_g,
                    lateral_g: *lateral_g,
                    radius_m: *radius_m,
                    bank_degrees: *bank_degrees,
                    geometry_peak_bank_radians: *geometry_peak_bank_radians,
                    geometry_speed_loss_fraction: *geometry_speed_loss_fraction,
                    geometry_radius_m: *geometry_radius_m,
                    geometry_digest: *geometry_digest,
                },
                Self::Loop {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    steel_structure,
                    geometry_digest,
                    ..
                } => BinaryElementSpec::Loop {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    top_feel: *top_feel,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_top_g: *delivered_top_g,
                    height_m: *height_m,
                    span_m: *span_m,
                    bottom_g: *bottom_g,
                    top_g: *top_g,
                    hold_degrees: *hold_degrees,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    steel_structure: *steel_structure,
                    geometry_digest: *geometry_digest,
                },
                Self::ZeroGRoll {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_crest_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    pull_g,
                    flank_angle_degrees,
                    twist_ramp,
                    twist_start_m,
                    twist_end_m,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                    ..
                } => BinaryElementSpec::ZeroGRoll {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_crest_g: *delivered_crest_g,
                    peak_roll_rate_deg_s: *peak_roll_rate_deg_s,
                    height_m: *height_m,
                    span_m: *span_m,
                    pull_g: *pull_g,
                    flank_angle_degrees: *flank_angle_degrees,
                    twist_ramp: *twist_ramp,
                    twist_start_m: *twist_start_m,
                    twist_end_m: *twist_end_m,
                    profile_grid_half_tiles: *profile_grid_half_tiles,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
                },
                Self::RavenTurn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    vertical_insert_m,
                    exit_dx_half_tiles,
                    exit_dy_half_tiles,
                    exit_dz_half_tiles,
                    roll_dx_m,
                    roll_lateral_m,
                    diagonal,
                    geometry_digest,
                    ..
                } => BinaryElementSpec::RavenTurn {
                    design_speed_mps: *design_speed_mps,
                    cars: *cars,
                    train_length_metres: *train_length_metres,
                    hand: *hand,
                    top_feel: *top_feel,
                    target_peak_g: *target_peak_g,
                    delivered_peak_g: *delivered_peak_g,
                    delivered_top_g: *delivered_top_g,
                    peak_roll_rate_deg_s: *peak_roll_rate_deg_s,
                    height_m: *height_m,
                    span_m: *span_m,
                    bottom_g: *bottom_g,
                    top_g: *top_g,
                    hold_degrees: *hold_degrees,
                    vertical_insert_m: *vertical_insert_m,
                    exit_dx_half_tiles: *exit_dx_half_tiles,
                    exit_dy_half_tiles: *exit_dy_half_tiles,
                    exit_dz_half_tiles: *exit_dz_half_tiles,
                    roll_dx_m: *roll_dx_m,
                    roll_lateral_m: *roll_lateral_m,
                    diagonal: *diagonal,
                    geometry_digest: *geometry_digest,
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
            let wire = HumanReadableElementSpec::deserialize(deserializer)?;
            resolve_human_readable_spec(wire).map_err(D::Error::custom)
        } else {
            if reject_shaped_element_in_legacy_v4_save() {
                return Err(D::Error::custom(
                    "version 4 saves with shaped coaster elements are unsupported",
                ));
            }
            let mut spec = match BinaryElementSpec::deserialize(deserializer)? {
                BinaryElementSpec::Hill {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    target_height_m,
                    pull_g,
                    crest_g,
                    flank_angle_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                } => Self::Hill {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    target_g,
                    delivered_g,
                    height_m,
                    span_m,
                    target_height_m,
                    pull_g,
                    crest_g,
                    flank_angle_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                    profile: regenerate_hill_profile(
                        cars,
                        design_speed_mps,
                        train_length_metres,
                        pull_g,
                        crest_g,
                        flank_angle_degrees,
                        profile_grid_half_tiles,
                    )
                    .map_err(D::Error::custom)?,
                },
                BinaryElementSpec::Turn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry_peak_bank_radians,
                    geometry_speed_loss_fraction,
                    geometry_radius_m,
                    geometry_digest,
                } => Self::Turn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    vertical_g,
                    lateral_g,
                    radius_m,
                    bank_degrees,
                    geometry_peak_bank_radians,
                    geometry_speed_loss_fraction,
                    geometry_radius_m,
                    geometry_digest,
                    geometry: super::curves::regenerate_shaped_speed_turn_geometry(
                        round_element_value(design_speed_mps),
                        geometry_peak_bank_radians,
                        geometry_speed_loss_fraction,
                        geometry_radius_m,
                    )
                    .map_err(D::Error::custom)?,
                },
                BinaryElementSpec::Loop {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    steel_structure,
                    geometry_digest,
                } => Self::Loop {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    profile_grid_half_tiles,
                    diagonal,
                    steel_structure,
                    geometry_digest,
                    profile: inversions::regenerate_loop_profile(
                        cars,
                        design_speed_mps,
                        train_length_metres,
                        bottom_g,
                        top_g,
                        hold_degrees,
                        profile_grid_half_tiles,
                        diagonal,
                    )
                    .map_err(D::Error::custom)?,
                },
                BinaryElementSpec::ZeroGRoll {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_crest_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    pull_g,
                    flank_angle_degrees,
                    twist_ramp,
                    twist_start_m,
                    twist_end_m,
                    profile_grid_half_tiles,
                    diagonal,
                    geometry_digest,
                } => {
                    let profile = inversions::regenerate_zero_g_roll_profile(
                        cars,
                        design_speed_mps,
                        train_length_metres,
                        pull_g,
                        flank_angle_degrees,
                        twist_ramp,
                        twist_start_m,
                        twist_end_m,
                        profile_grid_half_tiles,
                        diagonal,
                    )
                    .map_err(D::Error::custom)?;
                    Self::ZeroGRoll {
                        design_speed_mps,
                        cars,
                        train_length_metres,
                        hand,
                        target_peak_g,
                        delivered_peak_g,
                        delivered_crest_g,
                        peak_roll_rate_deg_s,
                        height_m,
                        span_m,
                        pull_g,
                        flank_angle_degrees,
                        twist_ramp,
                        twist_start_m,
                        twist_end_m,
                        profile_grid_half_tiles,
                        diagonal,
                        geometry_digest,
                        profile,
                    }
                }
                BinaryElementSpec::RavenTurn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    vertical_insert_m,
                    exit_dx_half_tiles,
                    exit_dy_half_tiles,
                    exit_dz_half_tiles,
                    roll_dx_m,
                    roll_lateral_m,
                    diagonal,
                    geometry_digest,
                } => Self::RavenTurn {
                    design_speed_mps,
                    cars,
                    train_length_metres,
                    hand,
                    top_feel,
                    target_peak_g,
                    delivered_peak_g,
                    delivered_top_g,
                    peak_roll_rate_deg_s,
                    height_m,
                    span_m,
                    bottom_g,
                    top_g,
                    hold_degrees,
                    vertical_insert_m,
                    exit_dx_half_tiles,
                    exit_dy_half_tiles,
                    exit_dz_half_tiles,
                    roll_dx_m,
                    roll_lateral_m,
                    diagonal,
                    geometry_digest,
                    half_loop: inversions::regenerate_raven_turn_half_loop(
                        cars,
                        design_speed_mps,
                        train_length_metres,
                        bottom_g,
                        top_g,
                        hold_degrees,
                        vertical_insert_m,
                        exit_dz_half_tiles,
                    )
                    .map_err(D::Error::custom)?,
                },
            };
            canonicalize_element_spec(&mut spec);
            if let Err(mismatch) = spec.verify_geometry_digest() {
                if !capture_current_save_digest_mismatch(mismatch) {
                    return Err(D::Error::custom(mismatch));
                }
            }
            Ok(spec)
        }
    }
}

impl ElementSpec {
    pub(crate) fn from_human_readable_json(
        value: serde_json::Value,
    ) -> Result<Self, ElementSpecLoadError> {
        let wire = serde_json::from_value(value)
            .map_err(|error| ElementSpecLoadError::InvalidRecipe(error.to_string()))?;
        resolve_human_readable_spec(wire)
    }

    pub fn resolved_geometry_digest(&self) -> u64 {
        let mut hash = StateHasher::new();
        hash.write_tag(b"shaped-geometry-v1");
        match self {
            Self::Hill { profile, .. } | Self::Loop { profile, .. } => profile.hash_into(&mut hash),
            Self::Turn { geometry, .. } => geometry.hash_into(&mut hash),
            Self::ZeroGRoll {
                profile,
                twist_ramp,
                twist_start_m,
                twist_end_m,
                ..
            } => {
                profile.hash_into(&mut hash);
                Twist::ZeroGRoll {
                    ramp: *twist_ramp,
                    start: *twist_start_m,
                    end: *twist_end_m,
                }
                .hash_into(&mut hash);
            }
            Self::RavenTurn {
                half_loop,
                roll_dx_m,
                roll_lateral_m,
                ..
            } => {
                half_loop.hash_into(&mut hash);
                hash.write_f64(*roll_dx_m);
                hash.write_f64(*roll_lateral_m);
            }
        }
        hash.finish()
    }

    pub fn verify_geometry_digest(&self) -> Result<(), GeometryDigestMismatch> {
        let (expected, actual) = match self {
            Self::Hill {
                geometry_digest, ..
            }
            | Self::Turn {
                geometry_digest, ..
            }
            | Self::Loop {
                geometry_digest, ..
            }
            | Self::ZeroGRoll {
                geometry_digest, ..
            }
            | Self::RavenTurn {
                geometry_digest, ..
            } => (*geometry_digest, self.resolved_geometry_digest()),
        };
        if expected == actual {
            Ok(())
        } else {
            Err(GeometryDigestMismatch { expected, actual })
        }
    }

    pub(crate) fn set_resolved_geometry_digest(&mut self) {
        let digest = self.resolved_geometry_digest();
        match self {
            Self::Hill {
                geometry_digest, ..
            }
            | Self::Turn {
                geometry_digest, ..
            }
            | Self::Loop {
                geometry_digest, ..
            }
            | Self::ZeroGRoll {
                geometry_digest, ..
            }
            | Self::RavenTurn {
                geometry_digest, ..
            } => *geometry_digest = digest,
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        match self {
            Self::Hill {
                design_speed_mps,
                cars,
                train_length_metres: _,
                target_g,
                delivered_g,
                height_m,
                span_m,
                target_height_m: _,
                pull_g: _,
                crest_g: _,
                flank_angle_degrees: _,
                profile_grid_half_tiles: _,
                diagonal: _,
                geometry_digest: _,
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
                train_length_metres: _,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry_peak_bank_radians: _,
                geometry_speed_loss_fraction: _,
                geometry_radius_m: _,
                geometry_digest: _,
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
            Self::Loop {
                design_speed_mps,
                cars,
                hand,
                top_feel,
                target_peak_g,
                delivered_peak_g,
                delivered_top_g,
                height_m,
                span_m,
                steel_structure,
                profile,
                ..
            } => {
                hash.write_u8(2);
                hash.write_f64(*design_speed_mps);
                hash.write_u8(*cars);
                hash.write_u8(*hand as u8);
                hash.write_u8(*top_feel as u8);
                hash.write_f64(*target_peak_g);
                hash.write_f64(*delivered_peak_g);
                hash.write_f64(*delivered_top_g);
                hash.write_f64(*height_m);
                hash.write_f64(*span_m);
                hash.write_bool(*steel_structure);
                profile.hash_into(hash);
            }
            Self::ZeroGRoll {
                design_speed_mps,
                cars,
                hand,
                target_peak_g,
                delivered_peak_g,
                delivered_crest_g,
                peak_roll_rate_deg_s,
                height_m,
                span_m,
                profile,
                twist_ramp,
                twist_start_m,
                twist_end_m,
                ..
            } => {
                hash.write_u8(3);
                hash.write_f64(*design_speed_mps);
                hash.write_u8(*cars);
                hash.write_u8(*hand as u8);
                hash.write_f64(*target_peak_g);
                hash.write_f64(*delivered_peak_g);
                hash.write_f64(*delivered_crest_g);
                hash.write_f64(*peak_roll_rate_deg_s);
                hash.write_f64(*height_m);
                hash.write_f64(*span_m);
                profile.hash_into(hash);
                Twist::ZeroGRoll {
                    ramp: *twist_ramp,
                    start: *twist_start_m,
                    end: *twist_end_m,
                }
                .hash_into(hash);
            }
            Self::RavenTurn {
                design_speed_mps,
                cars,
                hand,
                top_feel,
                target_peak_g,
                delivered_peak_g,
                delivered_top_g,
                peak_roll_rate_deg_s,
                height_m,
                span_m,
                roll_dx_m,
                roll_lateral_m,
                half_loop,
                ..
            } => {
                hash.write_u8(4);
                hash.write_f64(*design_speed_mps);
                hash.write_u8(*cars);
                hash.write_u8(*hand as u8);
                hash.write_u8(*top_feel as u8);
                hash.write_f64(*target_peak_g);
                hash.write_f64(*delivered_peak_g);
                hash.write_f64(*delivered_top_g);
                hash.write_f64(*peak_roll_rate_deg_s);
                hash.write_f64(*height_m);
                hash.write_f64(*span_m);
                hash.write_f64(*roll_dx_m);
                hash.write_f64(*roll_lateral_m);
                half_loop.hash_into(hash);
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
            }
            | Self::Loop {
                design_speed_mps, ..
            }
            | Self::ZeroGRoll {
                design_speed_mps, ..
            }
            | Self::RavenTurn {
                design_speed_mps, ..
            } => *design_speed_mps,
        }
    }

    pub fn cars(&self) -> u8 {
        match self {
            Self::Hill { cars, .. }
            | Self::Turn { cars, .. }
            | Self::Loop { cars, .. }
            | Self::ZeroGRoll { cars, .. }
            | Self::RavenTurn { cars, .. } => *cars,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryDigestMismatch {
    pub expected: u64,
    pub actual: u64,
}

impl Display for GeometryDigestMismatch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "resolved geometry digest mismatch: stored {:016x}, regenerated {:016x}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for GeometryDigestMismatch {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementSpecLoadError {
    InvalidRecipe(String),
    GeometryDigestMismatch(GeometryDigestMismatch),
}

impl Display for ElementSpecLoadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecipe(message) => formatter.write_str(message),
            Self::GeometryDigestMismatch(mismatch) => Display::fmt(mismatch, formatter),
        }
    }
}

impl std::error::Error for ElementSpecLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRecipe(_) => None,
            Self::GeometryDigestMismatch(mismatch) => Some(mismatch),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SolvedElement {
    pub piece: Piece,
    pub spec: ElementSpec,
    pub calibration_runs: u8,
    pub cap_note: Option<FeelCapNote>,
    /// True when the solved element sits inside the global ASTM comfort envelope but
    /// pushes past this model's typical feel or design targets.
    pub exceeds_model_feel: bool,
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
    GeometryDigestMismatch {
        expected: u64,
        actual: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedProfile {
    pub minimum_speed_mps: f64,
    pub maximum_speed_mps: f64,
    pub minimum_cars: u8,
    pub maximum_cars: u8,
    pub default_floater_g: f64,
    pub default_strong_floater_g: f64,
    pub default_ejector_g: f64,
    /// Hill feels typical of this model's vibe. Players may still choose other feels or step
    /// strength up to the global ASTM comfort envelope, receiving a mild feel note.
    pub typical_hill_feels: &'static [HillFeel],
    pub strong_floater_minimum_speed_mps: f64,
    pub strong_floater_minimum_target_g: f64,
    pub hill_pull_g: f64,
    pub hill_height_energy_fraction: f64,
    pub turn_vertical_g: f64,
    pub maximum_bank_degrees: f64,
    pub minimum_turn_radius_m: f64,
    pub maximum_turn_radius_m: f64,
    pub finish_turn_max_vertical_g: f64,
    pub finish_turn_max_lateral_g: f64,
    pub minimum_inversion_speed_mps: f64,
    pub maximum_inversion_speed_mps: f64,
    pub default_loop_peak_g: f64,
    pub default_zero_g_roll_peak_g: f64,
    pub default_raven_turn_peak_g: f64,
}

impl ShapedProfile {
    pub fn allows_cars(self, cars: u8) -> bool {
        (self.minimum_cars..=self.maximum_cars).contains(&cars)
    }

    pub fn default_hill_strength(self, feel: HillFeel) -> f64 {
        match feel {
            HillFeel::Floater => self.default_floater_g,
            HillFeel::StrongFloater => self.default_strong_floater_g,
            HillFeel::Ejector => self.default_ejector_g,
            _ => unreachable!("only supported hill feelings have defaults"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapedConstructionError {
    Solve(SolveRefusal),
    Construction(ConstructionError),
    InvalidResolvedSpec,
    GeometryDigestMismatch(GeometryDigestMismatch),
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

pub fn default_loop_peak_g(model: &CoasterModel) -> f64 {
    model
        .shaped_profile
        .filter(|profile| profile.default_loop_peak_g > 0.0)
        .map_or(4.0, |profile| profile.default_loop_peak_g)
}

pub fn default_zero_g_roll_peak_g(model: &CoasterModel) -> f64 {
    model
        .shaped_profile
        .filter(|profile| profile.default_zero_g_roll_peak_g > 0.0)
        .map_or(3.8, |profile| profile.default_zero_g_roll_peak_g)
}

pub fn default_raven_turn_peak_g(model: &CoasterModel) -> f64 {
    model
        .shaped_profile
        .filter(|profile| profile.default_raven_turn_peak_g > 0.0)
        .map_or(4.0, |profile| profile.default_raven_turn_peak_g)
}

pub fn solve_confirm(
    intent: &ElementIntent,
    context: &mut SolveContext<'_>,
    preview: &SolvedElement,
) -> Result<SolvedElement, SolveRefusal> {
    validate_context(context)?;
    let profile = match intent {
        ElementIntent::Hill { .. } | ElementIntent::Turn { .. } => validate_scope(context)?,
        ElementIntent::Loop { .. }
        | ElementIntent::ZeroGRoll { .. }
        | ElementIntent::RavenTurn { .. } => inversions::validate_inversion_scope(intent, context)?,
    };
    if !preview_candidate_is_confirmed(intent, context, profile, preview) {
        let maximum_previews = match intent {
            ElementIntent::Hill { .. }
            | ElementIntent::Loop { .. }
            | ElementIntent::ZeroGRoll { .. }
            | ElementIntent::RavenTurn { .. } => MAXIMUM_SHAPED_PREVIEWS,
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
    match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => {
            let profile = validate_scope(context)?;
            verify_prefix_entry_speed(context)?;
            solve_hill(
                profile,
                *feel,
                *strength_g,
                *size,
                maximum_previews,
                provisional,
                preview_seed,
                context,
            )
        }
        ElementIntent::Turn { hand, size } => {
            let profile = validate_scope(context)?;
            verify_prefix_entry_speed(context)?;
            solve_turn(
                profile,
                *hand,
                *size,
                maximum_previews.min(3),
                provisional,
                preview_seed,
                context,
            )
        }
        ElementIntent::Loop {
            hand,
            peak_g,
            top_feel,
        } => {
            inversions::validate_inversion_scope(intent, context)?;
            verify_prefix_entry_speed(context)?;
            inversions::solve_loop(
                *hand,
                *peak_g,
                *top_feel,
                maximum_previews,
                provisional,
                preview_seed,
                context,
            )
        }
        ElementIntent::ZeroGRoll { hand, peak_g } => {
            inversions::validate_inversion_scope(intent, context)?;
            verify_prefix_entry_speed(context)?;
            inversions::solve_zero_g_roll(
                *hand,
                *peak_g,
                maximum_previews,
                provisional,
                preview_seed,
                context,
            )
        }
        ElementIntent::RavenTurn {
            hand,
            peak_g,
            top_feel,
        } => {
            inversions::validate_inversion_scope(intent, context)?;
            verify_prefix_entry_speed(context)?;
            inversions::solve_raven_turn(
                *hand,
                *peak_g,
                *top_feel,
                maximum_previews,
                provisional,
                preview_seed,
                context,
            )
        }
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

/// Whether the solver can shape elements for this model and train at some entry speed.
pub fn model_supports_shaped_elements(model: &CoasterModel, train: &TrainConfiguration) -> bool {
    model
        .shaped_profile
        .is_some_and(|profile| profile.allows_cars(train.cars))
}

fn validate_scope(context: &SolveContext<'_>) -> Result<ShapedProfile, SolveRefusal> {
    let Some(profile) = context.model.shaped_profile else {
        return Err(SolveRefusal::UnsupportedScope);
    };
    if !profile.allows_cars(context.train.cars)
        || context.entry_speed_mps > profile.maximum_speed_mps
    {
        return Err(SolveRefusal::UnsupportedScope);
    }
    if context.entry_speed_mps < profile.minimum_speed_mps {
        return Err(SolveRefusal::BelowMinimumSpeed {
            entry_speed_mps: context.entry_speed_mps,
            minimum_speed_mps: profile.minimum_speed_mps,
        });
    }
    Ok(profile)
}

fn hill_within_model_vibe(
    model: &CoasterModel,
    profile: ShapedProfile,
    feel: HillFeel,
    requested_target_g: f64,
) -> bool {
    let design_floor = model.design_force_targets.minimum_vertical_g + REQUIRED_FORCE_HEADROOM_G;
    profile.typical_hill_feels.contains(&feel) && requested_target_g >= design_floor - 1e-9
}

fn turn_within_model_vibe(
    model: &CoasterModel,
    design_speed_mps: f64,
    requested_radius_m: Option<f64>,
) -> bool {
    let Some(radius_m) = requested_radius_m else {
        return true;
    };
    let ideal_vertical_g = crate::math::hypot(
        1.0,
        crate::math::powi(design_speed_mps, 2) / (GRAVITY * radius_m),
    );
    ideal_vertical_g <= model.design_force_targets.maximum_vertical_g - REQUIRED_FORCE_HEADROOM_G
}

fn candidate_passes_force_budget(
    model: &CoasterModel,
    cars: &[CandidateCarForces],
    within_vibe: bool,
) -> (bool, f64) {
    let every_car_within_limits = cars.iter().all(|car| car.within_limits);
    if within_vibe {
        let headroom_g = force_headroom_for_candidate(model, cars);
        (
            headroom_g >= REQUIRED_FORCE_HEADROOM_G && every_car_within_limits,
            headroom_g,
        )
    } else {
        let headroom_g = envelope_headroom_for_candidate(cars);
        (every_car_within_limits, headroom_g)
    }
}

fn preview_candidate_is_confirmed(
    intent: &ElementIntent,
    context: &SolveContext<'_>,
    profile: ShapedProfile,
    candidate: &SolvedElement,
) -> bool {
    let Some(seed) = candidate.calibration_seed.as_ref() else {
        return false;
    };
    if seed.preview.cars.len() != usize::from(context.train.cars)
        || !intent_matches_spec(intent, &candidate.spec, context.model, context.entry)
    {
        return false;
    }
    let within_vibe = match intent {
        ElementIntent::Hill {
            feel, strength_g, ..
        } => hill_within_model_vibe(context.model, profile, *feel, *strength_g),
        ElementIntent::Turn { size, .. } => {
            let requested_radius = match size {
                TurnSizeHint::Auto => None,
                TurnSizeHint::Radius(radius) => Some(*radius),
            };
            turn_within_model_vibe(
                context.model,
                round_element_value(context.entry_speed_mps),
                requested_radius,
            )
        }
        ElementIntent::Loop { peak_g, .. }
        | ElementIntent::ZeroGRoll { peak_g, .. }
        | ElementIntent::RavenTurn { peak_g, .. } => {
            inversions::inversion_within_model_vibe(context.model, *peak_g)
        }
    };
    let (passes_budget, _) =
        candidate_passes_force_budget(context.model, &seed.preview.cars, within_vibe);
    if !passes_budget {
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
        (ElementSpec::Loop { target_peak_g, .. }, ElementIntent::Loop { .. }) => {
            let delivered = seed
                .preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            (delivered - target_peak_g).abs() <= inversions::MAXIMUM_INVERSION_PEAK_ERROR_G
                && candidate.piece.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres
        }
        (ElementSpec::ZeroGRoll { target_peak_g, .. }, ElementIntent::ZeroGRoll { .. })
        | (ElementSpec::RavenTurn { target_peak_g, .. }, ElementIntent::RavenTurn { .. }) => {
            let delivered = seed
                .preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            let peak_roll_rate = seed
                .preview
                .cars
                .iter()
                .map(|car| car.peak_roll_rate_deg_s)
                .fold(0.0_f64, f64::max);
            (delivered - target_peak_g).abs() <= inversions::MAXIMUM_INVERSION_PEAK_ERROR_G
                && peak_roll_rate <= inversions::MAXIMUM_SHAPED_ROLL_RATE_DEG_S
                && candidate.piece.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn solve_hill(
    profile: ShapedProfile,
    feel: HillFeel,
    strength_g: f64,
    size: HillSizeHint,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    match size {
        HillSizeHint::Auto => solve_hill_analytically(
            profile,
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
                    target_g: strength_g,
                    available_headroom_g: 0.0,
                });
            }
            let energy_height_m = crate::math::powi(context.entry_speed_mps, 2) / (2.0 * GRAVITY);
            if height_m >= energy_height_m * 0.9 {
                return Err(SolveRefusal::Stall {
                    entry_speed_mps: context.entry_speed_mps,
                    height_m,
                });
            }
            solve_hill_analytically(
                profile,
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

#[allow(clippy::too_many_arguments)]
fn solve_hill_analytically(
    shaped_profile: ShapedProfile,
    feel: HillFeel,
    strength_g: f64,
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
    let requested_target_g = strength_g;
    if !requested_target_g.is_finite() {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: requested_target_g,
            available_headroom_g: 0.0,
        });
    }
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    let within_vibe =
        hill_within_model_vibe(context.model, shaped_profile, feel, requested_target_g);
    let target_floor = if within_vibe {
        context.model.design_force_targets.minimum_vertical_g + REQUIRED_FORCE_HEADROOM_G
    } else {
        ENVELOPE_DISPLAY_LIMITS.minimum_vertical_g
    };
    let enforce_strong_floater_floor = within_vibe
        && feel == HillFeel::StrongFloater
        && context.entry_speed_mps >= shaped_profile.strong_floater_minimum_speed_mps;
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
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &seed.preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        last_delivered_g = Some(delivered_g);
        if !passes_budget {
            let next_target_g = (target_g + 0.05).min(-0.05);
            if enforce_strong_floater_floor
                && next_target_g > shaped_profile.strong_floater_minimum_target_g
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
        // Each crest correction comes from a measured train preview, whose last bits differ between
        // native and WASM builds; the hill geometry is sensitive to those bits. Rounding the crest
        // to the element-spec grid, as the design speed is, keeps both builds solving the same hill.
        crest_g = round_element_value(crest_g);
        let target_height_m = fixed_height_m.unwrap_or_else(|| {
            crate::math::powi(design_speed_mps, 2) / (2.0 * GRAVITY)
                * shaped_profile.hill_height_energy_fraction
        });
        let (profile, height_m, span_m, flank_angle_degrees) = hill_profile_for_height(
            context,
            design_speed_mps,
            shaped_profile.hill_pull_g,
            crest_g,
            target_height_m,
        )?;
        let diagonal = context.entry.heading.index() % 2 != 0;
        let diagonal_scale = if diagonal {
            std::f64::consts::SQRT_2
        } else {
            1.0
        };
        let profile_grid_half_tiles = (span_m / diagonal_scale / 2.0).round() as u16;
        let mut candidate_spec = ElementSpec::Hill {
            design_speed_mps,
            cars: context.train.cars,
            train_length_metres: context.train.length_metres,
            target_g,
            delivered_g: target_g,
            height_m,
            span_m,
            target_height_m,
            pull_g: shaped_profile.hill_pull_g,
            crest_g,
            flank_angle_degrees,
            profile_grid_half_tiles,
            diagonal,
            geometry_digest: 0,
            profile,
        };
        canonicalize_element_spec(&mut candidate_spec);
        candidate_spec.set_resolved_geometry_digest();
        let candidate = piece_from_spec(context.entry, &candidate_spec)?;
        let preview = calibrate_piece(context, &candidate)?;
        preview_count += 1;
        if preview.cars.len() != usize::from(context.train.cars) {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        let sustained_delivered_g = preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min);
        let Some(delivered_g) = sustained_delivered_g else {
            if provisional || preview_count >= maximum_previews {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g: headroom_g,
                });
            }
            let peak_min_g = preview
                .cars
                .iter()
                .map(|car| car.peaks.minimum_vertical_g)
                .fold(f64::INFINITY, f64::min);
            if !peak_min_g.is_finite() {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g,
                    available_headroom_g: headroom_g,
                });
            }
            let next_crest_g = match previous_attempt {
                Some((previous_crest, previous_delivered)) => {
                    let slope = (peak_min_g - previous_delivered) / (crest_g - previous_crest);
                    if !slope.is_finite() || slope.abs() < 1e-6 {
                        crest_g + (target_g - peak_min_g).min(-0.15)
                    } else {
                        crest_g + (target_g - peak_min_g) / slope
                    }
                }
                None => crest_g + (target_g - peak_min_g).min(-0.15),
            };
            previous_attempt = Some((crest_g, peak_min_g));
            crest_g = next_crest_g;
            continue;
        };
        last_delivered_g = Some(delivered_g);
        let exceeds_model_feel = !shaped_profile.typical_hill_feels.contains(&feel)
            || candidate_exceeds_design_targets(context.model, &preview.cars);

        if provisional {
            let mut preview_spec = candidate_spec;
            if let ElementSpec::Hill {
                delivered_g: stored_delivery_g,
                ..
            } = &mut preview_spec
            {
                *stored_delivery_g = round_element_value(delivered_g);
            };
            canonicalize_element_spec(&mut preview_spec);
            preview_spec.set_resolved_geometry_digest();
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
                exceeds_model_feel,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: Some(crest_g),
                }),
            });
        }

        if !passes_budget {
            let next_target_g = (target_g + 0.05).min(-0.05);
            if enforce_strong_floater_floor
                && next_target_g > shaped_profile.strong_floater_minimum_target_g
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
            if let ElementSpec::Hill {
                delivered_g: stored_delivery_g,
                ..
            } = &mut final_spec
            {
                *stored_delivery_g = round_element_value(delivered_g);
            }
            canonicalize_element_spec(&mut final_spec);
            final_spec.set_resolved_geometry_digest();
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
                exceeds_model_feel,
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

pub fn default_hill_strength(feel: HillFeel, model: &CoasterModel) -> f64 {
    if let Some(profile) = model.shaped_profile {
        return profile.default_hill_strength(feel);
    }
    match feel {
        HillFeel::Floater => -0.5,
        HillFeel::StrongFloater => -0.85,
        HillFeel::Ejector if model.track_material == crate::track::Material::Wood => -0.8,
        HillFeel::Ejector => -1.2,
        _ => unreachable!("only supported hill feelings have defaults"),
    }
}

fn hill_profile_for_height(
    context: &SolveContext<'_>,
    design_speed_mps: f64,
    pull_g: f64,
    crest_g: f64,
    target_height_m: f64,
) -> Result<(SolvedProfile, f64, f64, f64), SolveRefusal> {
    let train = OfflineTrain::Coaster {
        cars: context.train.cars,
        length_metres: context.train.length_metres,
    };
    let mut low_flank_degrees: f64 = 2.0;
    let mut high_flank_degrees: f64 = 75.0;
    let mut best: Option<(f64, f64)> = None;
    for _ in 0..SHAPED_FLANK_SEARCH_STEPS {
        let flank_degrees = (low_flank_degrees + high_flank_degrees) / 2.0;
        let profile_spec = OfflineHillSpec {
            entry_speed_mps: design_speed_mps,
            pull_g,
            crest_g,
            flank_angle_radians: flank_degrees.to_radians(),
            taper_angle_radians: Some(SHAPED_HILL_TAPER_ANGLE),
        };
        let Ok(height_m) = solve_shaped_hill_apex_height_offline(profile_spec, train, 256) else {
            high_flank_degrees = flank_degrees;
            continue;
        };
        let error_m = (height_m - target_height_m).abs();
        if best
            .as_ref()
            .is_none_or(|(best_error, _)| error_m < *best_error)
        {
            best = Some((error_m, flank_degrees));
        }
        if height_m < target_height_m {
            low_flank_degrees = flank_degrees;
        } else {
            high_flank_degrees = flank_degrees;
        }
    }
    let (_, flank_degrees) = best.ok_or(SolveRefusal::InvalidGeometry)?;
    let profile_spec = OfflineHillSpec {
        entry_speed_mps: design_speed_mps,
        pull_g,
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
    Ok((profile, height_m, span_m, flank_degrees))
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
    profile: ShapedProfile,
    hand: Hand,
    size: TurnSizeHint,
    maximum_previews: u8,
    provisional: bool,
    preview_seed: Option<&SolvedElement>,
    context: &mut SolveContext<'_>,
) -> Result<SolvedElement, SolveRefusal> {
    let design_speed_mps = round_element_value(context.entry_speed_mps);
    if let TurnSizeHint::Radius(radius_m) = size {
        if !radius_m.is_finite() || radius_m <= 0.0 {
            return Err(SolveRefusal::InfeasibleTarget {
                target_g: profile.turn_vertical_g,
                available_headroom_g: 0.0,
            });
        }
    }
    let model_limits = context.model.design_force_targets;
    let target_vertical_g = profile.turn_vertical_g;
    if target_vertical_g < model_limits.minimum_vertical_g + 0.5
        || target_vertical_g > model_limits.maximum_vertical_g - 0.5
    {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: (target_vertical_g - model_limits.minimum_vertical_g)
                .min(model_limits.maximum_vertical_g - target_vertical_g),
        });
    }
    let radius_step = turn_radius_grid_step(context.entry.heading);
    let requested_radius_m = match size {
        TurnSizeHint::Auto => None,
        TurnSizeHint::Radius(radius_m) => Some(radius_m),
    };
    if requested_radius_m.is_some_and(|radius| {
        radius < profile.minimum_turn_radius_m || radius > profile.maximum_turn_radius_m
    }) {
        return Err(SolveRefusal::InfeasibleTarget {
            target_g: target_vertical_g,
            available_headroom_g: 0.0,
        });
    }
    let within_vibe = turn_within_model_vibe(context.model, design_speed_mps, requested_radius_m);

    let initial_bank = if !within_vibe && let Some(radius_m) = requested_radius_m {
        crate::math::atan(crate::math::powi(design_speed_mps, 2) / (GRAVITY * radius_m))
            .max(crate::math::acos(target_vertical_g.recip()))
    } else {
        crate::math::acos(target_vertical_g.recip())
    };
    let mut peak_bank = initial_bank.min(profile.maximum_bank_degrees.to_radians());
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
        let adjustment =
            if requested_radius_m.is_some() && preview_lateral_g > MAXIMUM_FULLY_BANKED_LATERAL_G {
                1.1
            } else {
                0.9
            };
        peak_bank = crate::math::atan(crate::math::tan(bank_degrees.to_radians()) * adjustment)
            .min(profile.maximum_bank_degrees.to_radians());
    }
    let mut last_headroom_g = f64::NEG_INFINITY;
    let mut last_lateral_g = f64::INFINITY;
    while preview_count < maximum_previews {
        let mut speed_loss_fraction;
        let mut radius_m;
        let mut resolved_bank;
        let mut candidate_spec;
        let mut piece;
        let mut radius_adjustments = 0_u8;
        loop {
            speed_loss_fraction = estimated_turn_speed_loss_fraction(
                context.physics,
                context.train,
                design_speed_mps,
                peak_bank,
            );
            let (_, candidate_radius_m, candidate_bank) = super::curves::shaped_speed_turn_geometry(
                design_speed_mps,
                peak_bank,
                speed_loss_fraction,
                radius_step,
                profile.minimum_turn_radius_m,
                requested_radius_m,
            );
            radius_m = candidate_radius_m;
            resolved_bank = candidate_bank;
            if radius_m > profile.maximum_turn_radius_m {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g: target_vertical_g,
                    available_headroom_g: 0.0,
                });
            }
            let bank_degrees = resolved_bank.to_degrees();
            if bank_degrees > profile.maximum_bank_degrees {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g: target_vertical_g,
                    available_headroom_g: profile.maximum_bank_degrees - bank_degrees,
                });
            }
            let geometry = super::curves::regenerate_shaped_speed_turn_geometry(
                design_speed_mps,
                resolved_bank,
                speed_loss_fraction,
                radius_m,
            )
            .map_err(|_| SolveRefusal::InvalidGeometry)?;
            candidate_spec = ElementSpec::Turn {
                design_speed_mps,
                cars: context.train.cars,
                train_length_metres: context.train.length_metres,
                hand,
                vertical_g: crate::math::cos(resolved_bank).recip(),
                lateral_g: 0.0,
                radius_m,
                bank_degrees,
                geometry_peak_bank_radians: resolved_bank,
                geometry_speed_loss_fraction: speed_loss_fraction,
                geometry_radius_m: radius_m,
                geometry_digest: 0,
                geometry,
            };
            canonicalize_element_spec(&mut candidate_spec);
            candidate_spec.set_resolved_geometry_digest();
            piece = piece_from_spec(context.entry, &candidate_spec)?;
            if requested_radius_m.is_some()
                || piece.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres
            {
                break;
            }
            radius_adjustments += 1;
            if radius_adjustments > 32 {
                return Err(SolveRefusal::InfeasibleTarget {
                    target_g: target_vertical_g,
                    available_headroom_g: 0.0,
                });
            }
            peak_bank = crate::math::atan(crate::math::tan(resolved_bank) * 0.95);
        }
        peak_bank = resolved_bank;
        let bank_degrees = peak_bank.to_degrees();
        let rail_radius_ok = piece.maximum_curvature() <= 1.0 / context.model.minimum_radius_metres;
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
        let (passes_budget, headroom_g) =
            candidate_passes_force_budget(context.model, &preview.cars, within_vibe);
        last_headroom_g = headroom_g;
        last_lateral_g = lateral_g;
        let exceeds_model_feel = candidate_exceeds_design_targets(context.model, &preview.cars);
        if provisional {
            let vertical_g = preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            let ElementSpec::Turn {
                train_length_metres,
                geometry_peak_bank_radians,
                geometry_speed_loss_fraction,
                geometry_radius_m,
                geometry,
                ..
            } = candidate_spec
            else {
                unreachable!("turn solve produced a hill candidate")
            };
            let mut spec = ElementSpec::Turn {
                design_speed_mps,
                cars: context.train.cars,
                train_length_metres,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry_peak_bank_radians,
                geometry_speed_loss_fraction,
                geometry_radius_m,
                geometry_digest: 0,
                geometry,
            };
            canonicalize_element_spec(&mut spec);
            spec.set_resolved_geometry_digest();
            return Ok(SolvedElement {
                piece: piece_from_spec(context.entry, &spec)?,
                spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: Some(CalibrationSeed {
                    preview,
                    hill_crest_g: None,
                }),
            });
        }
        if lateral_g <= MAXIMUM_FULLY_BANKED_LATERAL_G && passes_budget && rail_radius_ok {
            let vertical_g = preview
                .cars
                .iter()
                .map(|car| car.peaks.maximum_vertical_g)
                .fold(f64::NEG_INFINITY, f64::max);
            let ElementSpec::Turn {
                train_length_metres,
                geometry_peak_bank_radians,
                geometry_speed_loss_fraction,
                geometry_radius_m,
                geometry,
                ..
            } = candidate_spec
            else {
                unreachable!("turn solve produced a hill candidate")
            };
            let mut spec = ElementSpec::Turn {
                design_speed_mps,
                cars: context.train.cars,
                train_length_metres,
                hand,
                vertical_g,
                lateral_g,
                radius_m,
                bank_degrees,
                geometry_peak_bank_radians,
                geometry_speed_loss_fraction,
                geometry_radius_m,
                geometry_digest: 0,
                geometry,
            };
            canonicalize_element_spec(&mut spec);
            spec.set_resolved_geometry_digest();
            return Ok(SolvedElement {
                piece: piece_from_spec(context.entry, &spec)?,
                spec,
                calibration_runs: preview_count,
                cap_note: None,
                exceeds_model_feel,
                calibration_seed: None,
            });
        }
        if preview_count >= maximum_previews {
            break;
        }
        let adjustment = if requested_radius_m.is_some()
            && lateral_g > MAXIMUM_FULLY_BANKED_LATERAL_G
            && passes_budget
        {
            1.1
        } else {
            0.9
        };
        peak_bank = crate::math::atan(crate::math::tan(peak_bank) * adjustment)
            .min(profile.maximum_bank_degrees.to_radians());
    }
    Err(SolveRefusal::InfeasibleTarget {
        target_g: target_vertical_g,
        available_headroom_g: last_headroom_g.min(MAXIMUM_FULLY_BANKED_LATERAL_G - last_lateral_g),
    })
}

fn estimated_turn_speed_loss_fraction(
    physics: PhysicsConfig,
    train: &TrainConfiguration,
    design_speed_mps: f64,
    peak_bank: f64,
) -> f64 {
    let nominal_radius_m =
        crate::math::powi(design_speed_mps, 2) / (GRAVITY * crate::math::tan(peak_bank));
    let turn_length_estimate_m = std::f64::consts::FRAC_PI_2 * nominal_radius_m;
    let deceleration_mps2 = physics.rolling_resistance * GRAVITY
        + physics.drag_coefficient * crate::math::powi(design_speed_mps, 2)
            / (physics.car_mass_kg * f64::from(train.cars));
    deceleration_mps2 * turn_length_estimate_m / crate::math::powi(design_speed_mps, 2)
}

fn turn_radius_grid_step(heading: Heading) -> f64 {
    if heading.index() % 2 == 0 {
        2.0
    } else {
        2.0 * std::f64::consts::SQRT_2
    }
}

/// The radius the solver starts an automatically sized fully banked turn with at this entry
/// speed. Fixed radii close to it need the least bank correction to keep lateral force low.
pub(crate) fn automatic_turn_radius_m(
    model: &CoasterModel,
    train: &TrainConfiguration,
    heading: Heading,
    entry_speed_mps: f64,
    physics: PhysicsConfig,
) -> f64 {
    let profile = model
        .shaped_profile
        .expect("automatic turn radius requires a shaped profile");
    let design_speed_mps = round_element_value(entry_speed_mps);
    let peak_bank = crate::math::acos(profile.turn_vertical_g.recip())
        .min(profile.maximum_bank_degrees.to_radians());
    let speed_loss_fraction =
        estimated_turn_speed_loss_fraction(physics, train, design_speed_mps, peak_bank);
    let (_, radius_m, _) = super::curves::shaped_speed_turn_geometry(
        design_speed_mps,
        peak_bank,
        speed_loss_fraction,
        turn_radius_grid_step(heading),
        profile.minimum_turn_radius_m,
        None,
    );
    radius_m
}

pub(super) fn piece_from_spec(
    entry: TrackState,
    spec: &ElementSpec,
) -> Result<Piece, SolveRefusal> {
    spec.verify_geometry_digest()
        .map_err(|mismatch| SolveRefusal::GeometryDigestMismatch {
            expected: mismatch.expected,
            actual: mismatch.actual,
        })?;
    let id = match spec {
        ElementSpec::Hill { .. } => PieceId::ShapedHill,
        ElementSpec::Turn { hand, .. } => match hand {
            Hand::Left => PieceId::ShapedTurnLeft,
            Hand::Right => PieceId::ShapedTurnRight,
        },
        ElementSpec::Loop { hand, .. } => match hand {
            Hand::Left => PieceId::ShapedLoopLeft,
            Hand::Right => PieceId::ShapedLoopRight,
        },
        ElementSpec::ZeroGRoll { hand, .. } => match hand {
            Hand::Left => PieceId::ShapedZeroGRollLeft,
            Hand::Right => PieceId::ShapedZeroGRollRight,
        },
        ElementSpec::RavenTurn { hand, .. } => match hand {
            Hand::Left => PieceId::ShapedRavenTurnLeft,
            Hand::Right => PieceId::ShapedRavenTurnRight,
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
    piece.name = match spec {
        ElementSpec::Hill { .. } => SHAPED_HILL_NAME,
        ElementSpec::Turn {
            hand: Hand::Left, ..
        } => SHAPED_LEFT_TURN_NAME,
        ElementSpec::Turn {
            hand: Hand::Right, ..
        } => SHAPED_RIGHT_TURN_NAME,
        ElementSpec::Loop {
            hand: Hand::Left,
            steel_structure: true,
            ..
        } => SHAPED_LEFT_STEEL_LOOP_NAME,
        ElementSpec::Loop {
            hand: Hand::Right,
            steel_structure: true,
            ..
        } => SHAPED_RIGHT_STEEL_LOOP_NAME,
        ElementSpec::Loop {
            hand: Hand::Left,
            steel_structure: false,
            ..
        } => SHAPED_LEFT_LOOP_NAME,
        ElementSpec::Loop {
            hand: Hand::Right,
            steel_structure: false,
            ..
        } => SHAPED_RIGHT_LOOP_NAME,
        ElementSpec::ZeroGRoll {
            hand: Hand::Left, ..
        } => SHAPED_LEFT_ZERO_G_ROLL_NAME,
        ElementSpec::ZeroGRoll {
            hand: Hand::Right, ..
        } => SHAPED_RIGHT_ZERO_G_ROLL_NAME,
        ElementSpec::RavenTurn {
            hand: Hand::Left, ..
        } => SHAPED_LEFT_RAVEN_TURN_NAME,
        ElementSpec::RavenTurn {
            hand: Hand::Right, ..
        } => SHAPED_RIGHT_RAVEN_TURN_NAME,
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
        ElementSpec::Loop {
            hand,
            span_m,
            steel_structure,
            profile,
            ..
        } => {
            if profile.length() <= 0.0 || profile.endpoint()[1].abs() > 0.01 {
                return Err(SolveRefusal::InvalidContext);
            }
            piece.shape = Shape::ShapedProfile {
                spec: Arc::clone(&spec),
            };
            piece.inversion = Some(Inversion::VerticalLoop);
            piece.heartlining = Some(Heartlining::NotHeartlined);
            piece.structure_material = steel_structure.then_some(Material::Steel);
            piece.mirrored = matches!(hand, Hand::Right);
            piece.exit = inversions::loop_exit(entry, *hand, *span_m)?;
        }
        ElementSpec::ZeroGRoll {
            hand,
            span_m,
            profile,
            ..
        } => {
            if profile.length() <= 0.0 || profile.endpoint()[1].abs() > 0.01 {
                return Err(SolveRefusal::InvalidContext);
            }
            piece.shape = Shape::ShapedProfile {
                spec: Arc::clone(&spec),
            };
            piece.inversion = Some(Inversion::HeartlineRoll);
            piece.heartlining = Some(Heartlining::Heartlined);
            piece.mirrored = matches!(hand, Hand::Right);
            piece.exit = inversions::zero_g_roll_exit(entry, *span_m)?;
        }
        ElementSpec::RavenTurn {
            hand,
            exit_dx_half_tiles,
            exit_dy_half_tiles,
            exit_dz_half_tiles,
            half_loop,
            ..
        } => {
            if half_loop.length() <= 0.0 {
                return Err(SolveRefusal::InvalidContext);
            }
            piece.shape = Shape::ShapedProfile {
                spec: Arc::clone(&spec),
            };
            piece.inversion = Some(Inversion::VerticalLoop);
            piece.heartlining = Some(Heartlining::NotHeartlined);
            piece.mirrored = matches!(hand, Hand::Right);
            piece.exit = inversions::raven_turn_exit(
                entry,
                *exit_dx_half_tiles,
                *exit_dy_half_tiles,
                *exit_dz_half_tiles,
            )?;
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

pub(crate) fn turn_exit(
    entry: TrackState,
    hand: Hand,
    radius_m: f64,
) -> Result<TrackState, SolveRefusal> {
    let grid_step_m = turn_radius_grid_step(entry.heading);
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

fn verify_prefix_entry_speed(context: &mut SolveContext<'_>) -> Result<(), SolveRefusal> {
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
    Ok(())
}

fn calibrate_piece(
    context: &mut SolveContext<'_>,
    candidate: &Piece,
) -> Result<CandidateCalibrationPreview, SolveRefusal> {
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
                Some(
                    ElementSpec::Hill { height_m, .. }
                    | ElementSpec::Loop { height_m, .. }
                    | ElementSpec::ZeroGRoll { height_m, .. }
                    | ElementSpec::RavenTurn { height_m, .. },
                ) => *height_m,
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
    let limits = model.design_force_targets;
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

fn envelope_headroom_for_candidate(cars: &[CandidateCarForces]) -> f64 {
    let limits = ENVELOPE_DISPLAY_LIMITS;
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

fn candidate_exceeds_design_targets(model: &CoasterModel, cars: &[CandidateCarForces]) -> bool {
    let targets = model.design_force_targets;
    cars.iter().any(|car| {
        car.peaks.minimum_vertical_g < targets.minimum_vertical_g
            || car.peaks.maximum_vertical_g > targets.maximum_vertical_g
            || car.peaks.maximum_lateral_g > targets.maximum_lateral_g
            || car.peaks.maximum_longitudinal_g > targets.maximum_longitudinal_g
    })
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
        spec.verify_geometry_digest()
            .map_err(|mismatch| SolveRefusal::GeometryDigestMismatch {
                expected: mismatch.expected,
                actual: mismatch.actual,
            })?;
        piece_from_spec(self.end(), spec)
    }

    pub fn append_resolved(
        &mut self,
        intent: &ElementIntent,
        spec: &ElementSpec,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ShapedConstructionError> {
        spec.verify_geometry_digest()
            .map_err(ShapedConstructionError::GeometryDigestMismatch)?;
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
    let Some(shaped_profile) = model.shaped_profile else {
        return false;
    };
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
            if !matches!(
                feel,
                HillFeel::Floater | HillFeel::StrongFloater | HillFeel::Ejector
            ) {
                return false;
            }
            let requested_target_g = *strength_g;
            let within_vibe =
                hill_within_model_vibe(model, shaped_profile, *feel, requested_target_g);
            let cap_steps = (target_g - requested_target_g) / 0.05;
            let enforce_strong_floater_floor = within_vibe
                && *feel == HillFeel::StrongFloater
                && *design_speed_mps >= shaped_profile.strong_floater_minimum_speed_mps;
            let target_matches = (target_g - requested_target_g).abs() <= 1e-9
                || (cap_steps > 0.0
                    && (cap_steps - cap_steps.round()).abs() <= 1e-8
                    && *target_g <= -0.05
                    && (!enforce_strong_floater_floor
                        || *target_g <= shaped_profile.strong_floater_minimum_target_g));
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
            let delivery_within_floor = if within_vibe {
                (*delivered_g - model.design_force_targets.minimum_vertical_g)
                    >= REQUIRED_FORCE_HEADROOM_G
            } else {
                *delivered_g
                    >= ENVELOPE_DISPLAY_LIMITS.minimum_vertical_g - MAXIMUM_HILL_INTENT_ERROR_G
            };
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
                && delivery_within_floor
                && height_m.is_finite()
                && *height_m > 0.0
                && span_m.is_finite()
                && *span_m > 0.0
        }
        (
            ElementIntent::Turn { hand, size },
            ElementSpec::Turn {
                design_speed_mps,
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
            let requested_radius = match size {
                TurnSizeHint::Auto => None,
                TurnSizeHint::Radius(radius) => Some(*radius),
            };
            let within_vibe = turn_within_model_vibe(model, *design_speed_mps, requested_radius);
            let max_allowed_vertical_g = if within_vibe {
                model.design_force_targets.maximum_vertical_g - REQUIRED_FORCE_HEADROOM_G
            } else {
                // ASTM F2291 allows brief +6.0 g peaks; solver calibration already checks EnvelopeEvaluator.
                6.0
            };
            turn_radius_matches_intent(*size, *radius_m, grid_step, shaped_profile)
                && geometry.knots.len() == super::curves::SPEED_TURN_STEPS + 1
                && geometry.length.is_finite()
                && geometry.length > 0.0
                && bank_degrees.is_finite()
                && *bank_degrees <= shaped_profile.maximum_bank_degrees
                && vertical_g.is_finite()
                && *vertical_g <= max_allowed_vertical_g
                && lateral_g.is_finite()
                && lateral_g.abs() <= MAXIMUM_FULLY_BANKED_LATERAL_G
                && geometry.knots.iter().all(|knot| {
                    knot.x.is_finite() && knot.y.is_finite() && knot.heading.is_finite()
                })
        }
        (
            ElementIntent::Loop { .. }
            | ElementIntent::ZeroGRoll { .. }
            | ElementIntent::RavenTurn { .. },
            ElementSpec::Loop { .. }
            | ElementSpec::ZeroGRoll { .. }
            | ElementSpec::RavenTurn { .. },
        ) => inversions::inversion_intent_matches_spec(intent, spec, model, entry),
        _ => false,
    }
}

fn turn_radius_matches_intent(
    size: TurnSizeHint,
    radius_m: f64,
    grid_step_m: f64,
    profile: ShapedProfile,
) -> bool {
    if !radius_m.is_finite()
        || radius_m < profile.minimum_turn_radius_m
        || radius_m > profile.maximum_turn_radius_m
        || !stored_radius_is_grid_aligned(radius_m, grid_step_m)
    {
        return false;
    }
    match size {
        TurnSizeHint::Auto => true,
        TurnSizeHint::Radius(requested_radius_m) => {
            if !requested_radius_m.is_finite()
                || requested_radius_m < profile.minimum_turn_radius_m
                || requested_radius_m > profile.maximum_turn_radius_m
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
            crate::math::cos(flank_angle_radians * 0.5),
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
mod provisional_hill_reporting_tests {
    use super::{
        ElementIntent, ElementSpec, HillSizeHint, SolveContext, calibrate_piece,
        round_element_value, solve_confirm, solve_preview,
    };
    use crate::physics::{PhysicsConfig, PreviewCache};
    use crate::track::{HillFeel, PieceId, PieceKind, TrackState, catalog, hyper_model};

    #[test]
    fn provisional_hill_spec_records_the_actual_first_preview() {
        let speed_mps = 34.0;
        let mut model = hyper_model();
        model.train.cars = 4;
        model.train.length_metres *= 0.5;
        let physics = PhysicsConfig {
            lift_speed: speed_mps,
            ..PhysicsConfig::default()
        };
        let templates = catalog();
        let station_template = templates
            .iter()
            .find(|piece| piece.id == PieceId::Station)
            .expect("catalog includes a station");
        let straight_template = templates
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .expect("catalog includes straight track");
        let mut prefix = vec![
            station_template
                .at(TrackState::origin())
                .expect("station starts at the origin"),
        ];
        let acceleration_distance_m = (crate::math::powi(speed_mps, 2)
            - crate::math::powi(physics.station_speed, 2))
            / (2.0 * physics.drive_acceleration);
        let lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
        let lift_piece_count = (lift_length_m / straight_template.length()).ceil() as usize;
        for _ in 0..lift_piece_count {
            let mut lift = straight_template
                .at(prefix.last().expect("prefix has a station").exit)
                .expect("straight pieces connect");
            lift.kind = PieceKind::Lift;
            prefix.push(lift);
        }
        let entry = prefix.last().expect("prefix has a station").exit;
        let intent = ElementIntent::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: -0.85,
            size: HillSizeHint::Auto,
        };
        let mut preview_cache = PreviewCache::default();
        let mut context = SolveContext {
            entry,
            entry_speed_mps: speed_mps,
            model: &model,
            train: &model.train,
            prefix: &prefix,
            preview_cache: &mut preview_cache,
            physics,
        };

        let provisional =
            solve_preview(&intent, &mut context).expect("one-preview hill candidate is available");
        let measured_delivery_g = provisional
            .calibration_seed
            .as_ref()
            .expect("provisional solve retains its candidate preview")
            .preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min)
            .expect("preview records sustained force for every car");
        let ElementSpec::Hill {
            target_g,
            delivered_g: provisional_delivery_g,
            ..
        } = &provisional.spec
        else {
            panic!("hill intent resolves to a hill");
        };

        assert_eq!(
            *provisional_delivery_g,
            round_element_value(measured_delivery_g),
            "provisional spec must store the measured worst-car delivery"
        );
        assert!(
            (*provisional_delivery_g - target_g).abs() > 0.1,
            "the selected provisional case must expose its measured target error"
        );

        let confirmed = solve_confirm(&intent, &mut context, &provisional)
            .expect("confirmation calibrates the provisional hill");
        let confirmed_preview = calibrate_piece(&mut context, &confirmed.piece)
            .expect("confirmed hill can be previewed again");
        let measured_confirmed_delivery_g = confirmed_preview
            .cars
            .iter()
            .filter_map(|car| car.sustained_minimum_vertical_g)
            .reduce(f64::min)
            .expect("confirmed preview records sustained force for every car");
        let ElementSpec::Hill {
            target_g: confirmed_target_g,
            delivered_g: confirmed_delivery_g,
            crest_g: confirmed_crest_g,
            ..
        } = &confirmed.spec
        else {
            panic!("hill intent resolves to a hill");
        };
        assert_eq!(
            *confirmed_delivery_g,
            round_element_value(measured_confirmed_delivery_g),
            "confirmed spec must store the measured worst-car delivery"
        );
        assert_eq!(
            *confirmed_crest_g,
            round_element_value(*confirmed_crest_g),
            "a crest corrected from a measured preview must sit on the element-spec grid"
        );
        assert!((*confirmed_delivery_g - confirmed_target_g).abs() <= 0.05);
    }
}

#[cfg(test)]
mod turn_radius_guard_tests {
    use super::*;

    fn test_profile() -> ShapedProfile {
        ShapedProfile {
            minimum_speed_mps: 20.0,
            maximum_speed_mps: 36.0,
            minimum_cars: 4,
            maximum_cars: 8,
            default_floater_g: -0.5,
            default_strong_floater_g: -0.85,
            default_ejector_g: -1.2,
            typical_hill_feels: &[HillFeel::Floater, HillFeel::StrongFloater],
            strong_floater_minimum_speed_mps: 24.0,
            strong_floater_minimum_target_g: -0.75,
            hill_pull_g: 2.0,
            hill_height_energy_fraction: 0.4,
            turn_vertical_g: 3.0,
            maximum_bank_degrees: 75.0,
            minimum_turn_radius_m: 5.0,
            maximum_turn_radius_m: 256.0,
            finish_turn_max_vertical_g: 3.5,
            finish_turn_max_lateral_g: 0.6,
            minimum_inversion_speed_mps: 22.0,
            maximum_inversion_speed_mps: 36.0,
            default_loop_peak_g: 4.0,
            default_zero_g_roll_peak_g: 3.8,
            default_raven_turn_peak_g: 4.0,
        }
    }

    #[test]
    fn explicit_radius_matches_its_grid_value_and_stays_within_model_limits() {
        let profile = test_profile();
        let size = TurnSizeHint::Radius(64.0);

        assert!(turn_radius_matches_intent(size, 64.0, 2.0, profile));
        assert!(!turn_radius_matches_intent(size, 80.0, 2.0, profile));
        assert!(!turn_radius_matches_intent(size, 62.0, 2.0, profile));
        assert!(!turn_radius_matches_intent(size, 258.0, 2.0, profile));
        assert!(!turn_radius_matches_intent(
            TurnSizeHint::Radius(258.0),
            256.0,
            2.0,
            profile
        ));
    }

    #[test]
    fn solver_source_names_no_coaster_model_key() {
        for (name, source) in [
            ("shaped.rs", include_str!("shaped.rs")),
            ("shaped/inversions.rs", include_str!("shaped/inversions.rs")),
        ] {
            let production_source = source
                .split("\n#[cfg(test)]\n")
                .next()
                .expect("solver source has production code before tests");
            for model in crate::track::coaster_models() {
                let quoted_key = format!("\"{}\"", model.key);
                assert!(
                    !production_source.contains(&quoted_key),
                    "{name} production code must read ShapedProfile instead of naming model key {quoted_key}"
                );
            }
        }
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

#[cfg(test)]
mod legacy_save_decode_tests {
    use super::{
        BinaryElementSpec, ElementSpec, with_current_save_decode_context,
        with_legacy_v4_save_decode_context,
    };
    use crate::track::{Hand, curves::regenerate_shaped_speed_turn_geometry};

    #[test]
    fn legacy_v4_decode_context_marks_only_shaped_element_payloads() {
        let (classic_result, classic_shaped_seen) = with_legacy_v4_save_decode_context(|| {
            postcard::from_bytes::<Option<ElementSpec>>(&[0])
        });
        assert_eq!(classic_result, Ok(None));
        assert!(!classic_shaped_seen);

        let geometry = regenerate_shaped_speed_turn_geometry(30.0, 1.0, 0.1, 64.0).unwrap();
        let mut spec = ElementSpec::Turn {
            design_speed_mps: 30.0,
            cars: 1,
            train_length_metres: 8.0,
            hand: Hand::Left,
            vertical_g: 1.0,
            lateral_g: 0.0,
            radius_m: 64.0,
            bank_degrees: 1.0_f64.to_degrees(),
            geometry_peak_bank_radians: 1.0,
            geometry_speed_loss_fraction: 0.1,
            geometry_radius_m: 64.0,
            geometry_digest: 0,
            geometry,
        };
        spec.set_resolved_geometry_digest();
        let bytes = postcard::to_allocvec(&spec).unwrap();
        let (shaped_result, shaped_seen) =
            with_legacy_v4_save_decode_context(|| postcard::from_bytes::<ElementSpec>(&bytes));
        assert_eq!(shaped_result, Err(postcard::Error::SerdeDeCustom));
        assert!(shaped_seen);
    }

    #[test]
    fn current_save_decode_context_captures_only_digest_mismatches() {
        let turn_recipe = BinaryElementSpec::Turn {
            design_speed_mps: 30.0,
            cars: 1,
            train_length_metres: 8.0,
            hand: Hand::Left,
            vertical_g: 1.0,
            lateral_g: 0.0,
            radius_m: 64.0,
            bank_degrees: 1.0_f64.to_degrees(),
            geometry_peak_bank_radians: 1.0,
            geometry_speed_loss_fraction: 0.1,
            geometry_radius_m: 64.0,
            geometry_digest: 1,
        };
        let bytes = postcard::to_allocvec(&turn_recipe).unwrap();
        let (decoded, mismatch) =
            with_current_save_decode_context(|| postcard::from_bytes::<ElementSpec>(&bytes));
        let decoded = decoded.unwrap();
        let mismatch = mismatch.expect("wrong digest is captured after geometry regeneration");
        assert_eq!(mismatch.expected, 1);
        assert_eq!(mismatch.actual, decoded.resolved_geometry_digest());

        let invalid_recipe = BinaryElementSpec::Turn {
            design_speed_mps: 30.0,
            cars: 1,
            train_length_metres: 8.0,
            hand: Hand::Left,
            vertical_g: 1.0,
            lateral_g: 0.0,
            radius_m: 64.0,
            bank_degrees: 1.0_f64.to_degrees(),
            geometry_peak_bank_radians: 1.0,
            geometry_speed_loss_fraction: 0.1,
            geometry_radius_m: 0.0,
            geometry_digest: 1,
        };
        let bytes = postcard::to_allocvec(&invalid_recipe).unwrap();
        let (decoded, mismatch) =
            with_current_save_decode_context(|| postcard::from_bytes::<ElementSpec>(&bytes));
        assert_eq!(decoded, Err(postcard::Error::SerdeDeCustom));
        assert_eq!(mismatch, None);
    }
}
