use std::f64::consts::FRAC_PI_4;
use std::ops::{Add, Div, Mul, Sub};
use std::sync::{Arc, OnceLock};

use super::Material;
use super::roll::{RollGeometry, RollSample, Twist};
use crate::state_hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, b: Self) -> f64 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
    pub fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub(crate) fn unit(self) -> Self {
        self / self.length()
    }
}
impl Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, b: f64) -> Self {
        Self::new(self.x * b, self.y * b, self.z * b)
    }
}
impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, b: f64) -> Self {
        self * (1.0 / b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Heading {
    East,
    NorthEast,
    North,
    NorthWest,
    West,
    SouthWest,
    South,
    SouthEast,
}
impl Heading {
    pub(crate) fn index(self) -> i32 {
        self as i32
    }
    pub(crate) fn from_index(index: i32) -> Self {
        match index.rem_euclid(8) {
            0 => Self::East,
            1 => Self::NorthEast,
            2 => Self::North,
            3 => Self::NorthWest,
            4 => Self::West,
            5 => Self::SouthWest,
            6 => Self::South,
            _ => Self::SouthEast,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Slope {
    DownSteep,
    DownSixty,
    DownModerate,
    DownGentle,
    Level,
    UpGentle,
    UpModerate,
    UpSteep,
    UpSixty,
}
impl Slope {
    pub(crate) fn gradient(self) -> f64 {
        match self {
            Self::DownSteep => -1.0,
            Self::DownSixty => -1.732_050_807_568_877_2,
            Self::DownModerate => -0.75,
            Self::DownGentle => -0.5,
            Self::Level => 0.0,
            Self::UpGentle => 0.5,
            Self::UpModerate => 0.75,
            Self::UpSteep => 1.0,
            Self::UpSixty => 1.732_050_807_568_877_2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Bank {
    Right,
    Level,
    Left,
    Inverted,
}
impl Bank {
    pub fn radians(self) -> f64 {
        match self {
            Self::Right => -FRAC_PI_4,
            Self::Level => 0.0,
            Self::Left => FRAC_PI_4,
            Self::Inverted => std::f64::consts::PI,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HorizontalCurvature {
    Straight,
    LeftSmall,
    LeftLarge,
    RightSmall,
    RightLarge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VerticalCurvature {
    Straight,
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrackState {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: Heading,
    pub slope: Slope,
    pub bank: Bank,
    pub horizontal_curvature: HorizontalCurvature,
    pub vertical_curvature: VerticalCurvature,
}
impl TrackState {
    pub const fn origin() -> Self {
        Self {
            x: 0,
            y: 0,
            z: 0,
            heading: Heading::East,
            slope: Slope::Level,
            bank: Bank::Level,
            horizontal_curvature: HorizontalCurvature::Straight,
            vertical_curvature: VerticalCurvature::Straight,
        }
    }
    pub fn position(self) -> Vec3 {
        Vec3::new(
            f64::from(self.x) * 2.0,
            f64::from(self.y) * 2.0,
            f64::from(self.z) * 2.0,
        )
    }
    pub fn tangent(self) -> Vec3 {
        let a = f64::from(self.heading.index()) * FRAC_PI_4;
        Vec3::new(a.cos(), a.sin(), self.slope.gradient()).unit()
    }

    pub(crate) fn hash_into(self, hash: &mut StateHasher) {
        hash.write_i32(self.x);
        hash.write_i32(self.y);
        hash.write_i32(self.z);
        hash.write_u8(self.heading as u8);
        hash.write_u8(self.slope as u8);
        hash.write_u8(self.bank as u8);
        hash.write_u8(self.horizontal_curvature as u8);
        hash.write_u8(self.vertical_curvature as u8);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PieceId {
    Straight,
    GentleUp,
    GentleDown,
    SteepUp,
    SteepDown,
    SteepToSixtyUp,
    SixtyUp,
    SixtyToSteepUp,
    SteepToSixtyDown,
    SixtyDown,
    SixtyToSteepDown,
    LevelToGentleUp,
    GentleUpToLevel,
    GentleToSteepUp,
    SteepToGentleUp,
    LevelToGentleDown,
    GentleDownToLevel,
    GentleToSteepDown,
    SteepToGentleDown,
    TurnSmallEntry,
    TurnSmall,
    TurnSmallExit,
    TurnLargeEntry,
    TurnLarge,
    TurnLargeExit,
    BankedTurnEntry,
    BankedTurnExit,
    TurnSmallStandalone,
    TurnLargeStandalone,
    BankedTurnStandalone,
    LargeLeftBankedTurnStandalone,
    SmallRightBankedTurnStandalone,
    LargeRightBankedTurnStandalone,
    LargeLeftBankedTurnEntry,
    LargeLeftBankedTurn,
    LargeLeftBankedTurnExit,
    SmallRightBankedTurnEntry,
    SmallRightBankedTurn,
    SmallRightBankedTurnExit,
    LargeRightBankedTurnEntry,
    LargeRightBankedTurn,
    LargeRightBankedTurnExit,
    VerticalUpEntry,
    VerticalUpContinue,
    VerticalUpExit,
    VerticalDownEntry,
    VerticalDownContinue,
    VerticalDownExit,
    BankLeft,
    UnbankLeft,
    BankRight,
    UnbankRight,
    BankedTurn,
    OffsetLeft,
    OffsetRight,
    ShortOffsetLeft,
    ShortOffsetRight,
    Loop,
    Corkscrew,
    HeartlineRoll,
    Brakes,
    Station,
    TurnSmallRightEntry,
    TurnSmallRight,
    TurnSmallRightExit,
    TurnSmallRightStandalone,
    TurnLargeRightEntry,
    TurnLargeRight,
    TurnLargeRightExit,
    TurnLargeRightStandalone,
    CorkscrewRight,
    HeartlineRollRight,
    LaunchShort,
    Launch,
    LaunchLong,
    TerrainRise,
    TerrainFall,
    SteelLoop,
    TwisterFanLeft,
    TwisterFanRight,
    TwisterHelixLeft,
    TwisterHelixRight,
    AirtimeHill,
    TightTwisterLeft,
    TightTwisterRight,
    HybridOverbankLeft,
    HybridOverbankRight,
    HalfTileStraight,
    FullOffsetLeft,
    FullOffsetRight,
    OneTileStraight,
    OneTileGentleUp,
    OneTileGentleDown,
    ShortRise,
    ShortFall,
    LoopRight,
    SteelLoopRight,
    HalfLoopUp,
    HalfLoopDown,
    HalfCorkscrewLeftUp,
    HalfCorkscrewRightUp,
    HalfCorkscrewLeftDown,
    HalfCorkscrewRightDown,
    TopHat,
    LaunchedTopHat,
    LevelToSixtyUp,
    SixtyUpToLevel,
    LevelToSixtyDown,
    SixtyDownToLevel,
    BankedDropLeft,
    BankedDropRight,
    TwistedDropLeft,
    TwistedDropRight,
    BarrelDropLeft,
    BarrelDropRight,
    RavenRollLeftUp,
    RavenRollRightUp,
    RavenRollLeftDown,
    RavenRollRightDown,
    LargeLoop,
    LargeLoopRight,
    BankedStraightLeft,
    BankedStraightRight,
    RolloutLeft,
    RolloutRight,
    EjectorHill8x48,
    EjectorHill12x64,
    EjectorHill8x64,
    EjectorHill12x80,
    EjectorHill16x96,
    FloaterHill4x48,
    FloaterHill8x64,
    FloaterHill12x80,
    FloaterHill6x64,
    FloaterHill12x96,
    FloaterHill16x112,
    SpeedHill4x64,
    SpeedHill6x80,
    WoodenEjectorHill6x48,
    WoodenEjectorHill8x64,
    WoodenEjectorHill16x80,
    EjectorHill8x48OuterLeft,
    EjectorHill8x48OuterRight,
    EjectorHill12x64OuterLeft,
    EjectorHill12x64OuterRight,
    EjectorHill8x64OuterLeft,
    EjectorHill8x64OuterRight,
    EjectorHill12x80OuterLeft,
    EjectorHill12x80OuterRight,
    EjectorHill16x96OuterLeft,
    EjectorHill16x96OuterRight,
    FloaterHill4x48OuterLeft,
    FloaterHill4x48OuterRight,
    FloaterHill8x64OuterLeft,
    FloaterHill8x64OuterRight,
    FloaterHill12x80OuterLeft,
    FloaterHill12x80OuterRight,
    FloaterHill6x64OuterLeft,
    FloaterHill6x64OuterRight,
    FloaterHill12x96OuterLeft,
    FloaterHill12x96OuterRight,
    FloaterHill16x112OuterLeft,
    FloaterHill16x112OuterRight,
    ZeroGRollFastLeft,
    ZeroGRollFastRight,
    ZeroGRollCompactLeft,
    ZeroGRollCompactRight,
    StallLeft,
    StallRight,
    WaveTurnLeft,
    WaveTurnRight,
    SpeedTurnLarge16Left,
    SpeedTurnLarge16Right,
    SpeedTurnLarge20Left,
    SpeedTurnLarge20Right,
    SpeedTurnLarge24Left,
    SpeedTurnLarge24Right,
    SpeedTurnLarge28Left,
    SpeedTurnLarge28Right,
    SpeedTurnLarge32Left,
    SpeedTurnLarge32Right,
    SpeedTurnSmall12Left,
    SpeedTurnSmall12Right,
    SpeedTurnSmall16Left,
    SpeedTurnSmall16Right,
    SpeedTurnSmall20Left,
    SpeedTurnSmall20Right,
    SpeedTurnSmall24Left,
    SpeedTurnSmall24Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PieceKind {
    Track,
    Lift,
    Launch,
    Brakes,
    Station,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Inversion {
    VerticalLoop,
    Corkscrew,
    HeartlineRoll,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Heartlining {
    Heartlined,
    NotHeartlined,
}
use crate::track::profile::SolvedProfile;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Shape {
    Plain,
    AirtimeHill,
    Overbank,
    WaveTurn { bump: f64, peak: f64 },
    Profile(Arc<SolvedProfile>),
    Roll(Arc<RollGeometry>),
    RolledProfile(Arc<SolvedProfile>, Twist),
    SpeedBank { design_speed: f64 },
}
impl Shape {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        let tag = match self {
            Self::Plain => 0,
            Self::AirtimeHill => 1,
            Self::Overbank => 2,
            Self::WaveTurn { .. } => 14,
            Self::SpeedBank { .. } => 15,
            Self::Profile(_) => 11,
            Self::Roll(_) => 12,
            Self::RolledProfile(_, _) => 13,
        };
        hash.write_u8(tag);
        match self {
            Self::WaveTurn { bump, peak } => {
                hash.write_f64(*bump);
                hash.write_f64(*peak);
            }
            Self::SpeedBank { design_speed } => hash.write_f64(*design_speed),
            Self::Profile(profile) => profile.hash_into(hash),
            Self::Roll(roll) => roll.hash_into(hash),
            Self::RolledProfile(profile, twist) => {
                profile.hash_into(hash);
                twist.hash_into(hash);
            }
            _ => {}
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RatedSpeeds {
    pub minimum_entry: f64,
    pub maximum_entry: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HillFeel {
    Ejector,
    Floater,
    Speed,
    ZeroGRoll,
    Stall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HillTwist {
    OuterBankLeft,
    OuterBankRight,
    LeftRoll,
    RightRoll,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HillFamily {
    pub feel: HillFeel,
    pub height_metres: u8,
    pub span_metres: u16,
    pub twist: Option<HillTwist>,
    pub design_speed_metres_per_second: f64,
}

impl HillFamily {
    pub fn same_recommendation_family(self, other: Self) -> bool {
        self.feel == other.feel && self.twist == other.twist
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PieceFamily {
    Hill(HillFamily),
    SpeedTurn {
        radius_metres: u8,
        hand: Hand,
        design_speed_metres_per_second: f64,
    },
}

// Keep the original HillFamily postcard layout so version 2 parks remain readable.
#[derive(serde::Serialize, serde::Deserialize)]
enum FamilyWireFeel {
    Ejector,
    Floater,
    Speed,
    ZeroGRoll,
    Stall,
    SpeedTurn,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FamilyWire {
    feel: FamilyWireFeel,
    height_metres: u8,
    span_metres: u16,
    twist: Option<HillTwist>,
    design_speed_metres_per_second: f64,
}

impl serde::Serialize for PieceFamily {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = match *self {
            Self::Hill(hill) => FamilyWire {
                feel: match hill.feel {
                    HillFeel::Ejector => FamilyWireFeel::Ejector,
                    HillFeel::Floater => FamilyWireFeel::Floater,
                    HillFeel::Speed => FamilyWireFeel::Speed,
                    HillFeel::ZeroGRoll => FamilyWireFeel::ZeroGRoll,
                    HillFeel::Stall => FamilyWireFeel::Stall,
                },
                height_metres: hill.height_metres,
                span_metres: hill.span_metres,
                twist: hill.twist,
                design_speed_metres_per_second: hill.design_speed_metres_per_second,
            },
            Self::SpeedTurn {
                radius_metres,
                hand,
                design_speed_metres_per_second,
            } => FamilyWire {
                feel: FamilyWireFeel::SpeedTurn,
                height_metres: radius_metres,
                span_metres: 0,
                twist: Some(match hand {
                    Hand::Left => HillTwist::LeftRoll,
                    Hand::Right => HillTwist::RightRoll,
                }),
                design_speed_metres_per_second,
            },
        };
        serde::Serialize::serialize(&wire, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PieceFamily {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = <FamilyWire as serde::Deserialize>::deserialize(deserializer)?;
        let feel = match wire.feel {
            FamilyWireFeel::Ejector => HillFeel::Ejector,
            FamilyWireFeel::Floater => HillFeel::Floater,
            FamilyWireFeel::Speed => HillFeel::Speed,
            FamilyWireFeel::ZeroGRoll => HillFeel::ZeroGRoll,
            FamilyWireFeel::Stall => HillFeel::Stall,
            FamilyWireFeel::SpeedTurn => {
                let hand = match wire.twist {
                    Some(HillTwist::LeftRoll) => Hand::Left,
                    Some(HillTwist::RightRoll) => Hand::Right,
                    _ => return Err(serde::de::Error::custom("invalid speed turn hand")),
                };
                if wire.span_metres != 0 {
                    return Err(serde::de::Error::custom("invalid speed turn span"));
                }
                return Ok(Self::SpeedTurn {
                    radius_metres: wire.height_metres,
                    hand,
                    design_speed_metres_per_second: wire.design_speed_metres_per_second,
                });
            }
        };
        Ok(Self::Hill(HillFamily {
            feel,
            height_metres: wire.height_metres,
            span_metres: wire.span_metres,
            twist: wire.twist,
            design_speed_metres_per_second: wire.design_speed_metres_per_second,
        }))
    }
}

impl PieceFamily {
    pub fn same_recommendation_family(self, other: Self) -> bool {
        match (self, other) {
            (Self::Hill(first), Self::Hill(second)) => first.same_recommendation_family(second),
            (
                Self::SpeedTurn {
                    radius_metres: first_radius,
                    hand: first_hand,
                    ..
                },
                Self::SpeedTurn {
                    radius_metres: second_radius,
                    hand: second_hand,
                    ..
                },
            ) => first_radius == second_radius && first_hand == second_hand,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ArcLengthTable {
    lengths: Vec<f64>,
    speeds: Vec<f64>,
    maximum_curvature: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Piece {
    pub id: PieceId,
    #[serde(with = "crate::save::catalog_text")]
    pub name: crate::save::CatalogText,
    pub kind: PieceKind,
    /// Per-piece launch target speed in metres per second, or `None` for model defaults.
    pub launch_target_speed: Option<f64>,
    pub inversion: Option<Inversion>,
    pub heartlining: Option<Heartlining>,
    pub structure_material: Option<Material>,
    pub rated_speeds: Option<RatedSpeeds>,
    pub family: Option<PieceFamily>,
    pub entry: TrackState,
    pub exit: TrackState,
    pub(crate) shape: Shape,
    pub(crate) tangent_scale: f64,
    pub(crate) mirrored: bool,
    #[serde(with = "arc_table_serde")]
    pub(super) arc_table: Arc<OnceLock<ArcLengthTable>>,
}

mod arc_table_serde {
    use super::{Arc, ArcLengthTable, OnceLock};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Context {
        encoded: HashMap<*const OnceLock<ArcLengthTable>, u64>,
        decoded: HashMap<u64, Arc<OnceLock<ArcLengthTable>>>,
    }

    thread_local! {
        static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
    }

    pub(super) fn scoped<R>(operation: impl FnOnce() -> R) -> R {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                CONTEXT.with(|context| *context.borrow_mut() = None);
            }
        }
        CONTEXT.with(|context| {
            assert!(context.borrow().is_none(), "nested snapshot operation");
            *context.borrow_mut() = Some(Context::default());
        });
        let _reset = Reset;
        operation()
    }

    pub fn serialize<S: Serializer>(
        table: &Arc<OnceLock<ArcLengthTable>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let id = CONTEXT.with(|context| {
            let mut context = context.borrow_mut();
            context.as_mut().map(|context| {
                let next = context.encoded.len() as u64;
                *context.encoded.entry(Arc::as_ptr(table)).or_insert(next)
            })
        });
        (id, table.get()).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<OnceLock<ArcLengthTable>>, D::Error> {
        let (id, value) = <(Option<u64>, Option<ArcLengthTable>)>::deserialize(deserializer)?;
        if let Some(id) = id {
            let existing = CONTEXT.with(|context| {
                context
                    .borrow()
                    .as_ref()
                    .and_then(|context| context.decoded.get(&id).cloned())
            });
            if let Some(existing) = existing {
                if existing.get() != value.as_ref() {
                    return Err(D::Error::custom("inconsistent shared arc table"));
                }
                return Ok(existing);
            }
        }
        let table = OnceLock::new();
        if let Some(value) = value {
            table.set(value).expect("new arc table is empty");
        }
        let table = Arc::new(table);
        if let Some(id) = id {
            CONTEXT.with(|context| {
                if let Some(context) = context.borrow_mut().as_mut() {
                    context.decoded.insert(id, table.clone());
                }
            });
        }
        Ok(table)
    }
}

/// Preserve shared track tables while encoding or decoding one complete snapshot.
pub fn with_snapshot_arc_tables<R>(operation: impl FnOnce() -> R) -> R {
    arc_table_serde::scoped(operation)
}
#[derive(Clone, Copy, Debug)]
pub struct TrackSample {
    pub position: Vec3,
    pub heart: Vec3,
    pub tangent: Vec3,
    pub up: Vec3,
    pub curvature: f64,
    /// Unwrapped radians relative to the curve's unbanked frame; compare modulo 2π at joins.
    pub roll: f64,
}
pub const HEART_HEIGHT_METRES: f64 = 1.2;
const ARC_STEPS: usize = 1024;
impl TrackState {
    pub(crate) fn curvature_vector(self) -> Vec3 {
        let tangent = self.tangent();
        let angle = f64::from(self.heading.index()) * FRAC_PI_4;
        let left = Vec3::new(-angle.sin(), angle.cos(), 0.0);
        let up = tangent.cross(left);
        let scale = if self.heading.index() % 2 == 0 {
            1.0
        } else {
            std::f64::consts::SQRT_2
        };
        let horizontal = match self.horizontal_curvature {
            HorizontalCurvature::Straight => 0.0,
            HorizontalCurvature::LeftSmall => 1.0 / 16.0,
            HorizontalCurvature::LeftLarge => 1.0 / 32.0,
            HorizontalCurvature::RightSmall => -1.0 / 16.0,
            HorizontalCurvature::RightLarge => -1.0 / 32.0,
        };
        let vertical = match self.vertical_curvature {
            VerticalCurvature::Straight => 0.0,
            VerticalCurvature::Up => 1.0 / 80.0,
            VerticalCurvature::Down => -1.0 / 80.0,
        };
        (left * horizontal + up * vertical) / scale
    }
}
fn smooth(t: f64) -> (f64, f64, f64) {
    (
        t * t * t * (10.0 + t * (-15.0 + 6.0 * t)),
        30.0 * t * t * (1.0 - t) * (1.0 - t),
        60.0 * t * (1.0 - t) * (1.0 - 2.0 * t),
    )
}
fn curvature(velocity: Vec3, acceleration: Vec3) -> f64 {
    velocity.cross(acceleration).length() / velocity.length().powi(3)
}
const SPEED_TURN_STEPS: usize = 4096;
const SPEED_TURN_RAMP: f64 = 0.39;
const SPEED_TURN_EASING: f64 = 0.08;

struct SpeedTurnGeometry {
    radius: f64,
    design_speed: f64,
    peak_bank: f64,
    length: f64,
    knots: Vec<(f64, f64, f64)>,
}

fn speed_turn_bank_fraction(t: f64) -> f64 {
    let u = (t.min(1.0 - t) / SPEED_TURN_RAMP).clamp(0.0, 1.0);
    let easing = SPEED_TURN_EASING;
    if u < easing {
        let v = u / easing;
        easing * (v.powi(3) - 0.5 * v.powi(4)) / (1.0 - easing)
    } else if u > 1.0 - easing {
        let v = (1.0 - u) / easing;
        1.0 - easing * (v.powi(3) - 0.5 * v.powi(4)) / (1.0 - easing)
    } else {
        (u - easing / 2.0) / (1.0 - easing)
    }
}

fn build_speed_turn(radius: f64, design_speed: f64) -> SpeedTurnGeometry {
    let integrate = |peak_bank: f64| {
        let step = 1.0 / SPEED_TURN_STEPS as f64;
        let curvatures: Vec<_> = (0..SPEED_TURN_STEPS)
            .map(|index| {
                let t = (index as f64 + 0.5) * step;
                (peak_bank * speed_turn_bank_fraction(t)).tan() * crate::physics::GRAVITY
                    / design_speed.powi(2)
            })
            .collect();
        let length = std::f64::consts::FRAC_PI_2 / (curvatures.iter().sum::<f64>() * step);
        let distance = length * step;
        let (mut x, mut y, mut heading) = (0.0, 0.0, 0.0);
        let mut knots = Vec::with_capacity(SPEED_TURN_STEPS + 1);
        knots.push((x, y, heading));
        for curvature in curvatures {
            let angle = curvature * distance;
            let midpoint = heading + angle / 2.0;
            x += distance * midpoint.cos();
            y += distance * midpoint.sin();
            heading += angle;
            knots.push((x, y, heading));
        }
        ((x + y) / 2.0, length, knots)
    };
    let (mut low, mut high) = (0.0, std::f64::consts::FRAC_PI_2 - 0.0001);
    for _ in 0..40 {
        let peak_bank = (low + high) / 2.0;
        if integrate(peak_bank).0 > radius {
            low = peak_bank;
        } else {
            high = peak_bank;
        }
    }
    let peak_bank = (low + high) / 2.0;
    let (_, length, knots) = integrate(peak_bank);
    SpeedTurnGeometry {
        radius,
        design_speed,
        peak_bank,
        length,
        knots,
    }
}

fn speed_turn_geometry(radius: f64, design_speed: f64) -> &'static SpeedTurnGeometry {
    static GEOMETRIES: std::sync::LazyLock<Vec<SpeedTurnGeometry>> =
        std::sync::LazyLock::new(|| {
            [(32.0, 16.0), (32.0, 20.0), (32.0, 24.0)]
                .into_iter()
                .flat_map(|(radius, speed)| {
                    [radius, radius * std::f64::consts::SQRT_2]
                        .into_iter()
                        .map(move |actual_radius| build_speed_turn(actual_radius, speed))
                })
                .collect()
        });
    GEOMETRIES
        .iter()
        .find(|geometry| {
            (geometry.radius - radius).abs() < 1e-8 && geometry.design_speed == design_speed
        })
        .unwrap_or_else(|| {
            panic!("speed turn must have a fitted geometry: radius {radius}, speed {design_speed}")
        })
}

impl Piece {
    fn speed_turn_frame(&self, design_speed: f64, t: f64) -> (Vec3, Vec3, Vec3, f64) {
        let forward = self.entry.tangent().unit();
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        let vertical = Vec3::new(0.0, 0.0, 1.0);
        let displacement = self.exit.position() - self.entry.position();
        let hand = displacement.dot(left).signum();
        let radius = displacement.dot(forward);
        let geometry = speed_turn_geometry(radius, design_speed);
        let scaled = t.clamp(0.0, 1.0) * SPEED_TURN_STEPS as f64;
        let index = (scaled.floor() as usize).min(SPEED_TURN_STEPS - 1);
        let fraction = scaled - index as f64;
        let (x0, y0, heading0) = geometry.knots[index];
        let (x1, y1, heading1) = geometry.knots[index + 1];
        let h = geometry.length / SPEED_TURN_STEPS as f64;
        let h00 = 2.0 * fraction.powi(3) - 3.0 * fraction.powi(2) + 1.0;
        let h10 = fraction.powi(3) - 2.0 * fraction.powi(2) + fraction;
        let h01 = -2.0 * fraction.powi(3) + 3.0 * fraction.powi(2);
        let h11 = fraction.powi(3) - fraction.powi(2);
        let x = h00 * x0 + h10 * h * heading0.cos() + h01 * x1 + h11 * h * heading1.cos();
        let y = h00 * y0 + h10 * h * heading0.sin() + h01 * y1 + h11 * h * heading1.sin();
        let heading = heading0 + (heading1 - heading0) * fraction;
        let tangent = forward * heading.cos() + left * (hand * heading.sin());
        let roll = hand * geometry.peak_bank * speed_turn_bank_fraction(t);
        let up = vertical * roll.cos() + vertical.cross(tangent) * roll.sin();
        let heart = self.entry.position()
            + forward * x
            + left * (hand * y)
            + vertical * HEART_HEIGHT_METRES;
        (heart - up * HEART_HEIGHT_METRES, heart, up, roll)
    }

    fn speed_turn_raw(&self, design_speed: f64, t: f64) -> (Vec3, Vec3, Vec3) {
        let position = |u| self.speed_turn_frame(design_speed, u).0;
        let point = position(t);
        let step = 0.0001;
        if t == 0.0 || t == 1.0 {
            let tangent = if t == 0.0 {
                self.entry.tangent()
            } else {
                self.exit.tangent()
            };
            let radius =
                (self.exit.position() - self.entry.position()).dot(self.entry.tangent().unit());
            let length = speed_turn_geometry(radius, design_speed).length;
            return (point, tangent * length, Vec3::new(0.0, 0.0, 0.0));
        }
        let (previous, next) = (position(t - step), position(t + step));
        (
            point,
            (next - previous) / (2.0 * step),
            (previous - point * 2.0 + next) / step.powi(2),
        )
    }

    fn rolled_profile_sample(&self, profile: &SolvedProfile, twist: Twist, t: f64) -> RollSample {
        let length = profile.length();
        let (position, first, second, third) = profile.sample_with_third(t * length);
        let forward = Vec3::new(self.entry.tangent().x, self.entry.tangent().y, 0.0).unit();
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        let vertical = Vec3::new(0.0, 0.0, 1.0);
        let diagonal = if self.entry.heading.index() % 2 == 0 {
            1.0
        } else {
            std::f64::consts::SQRT_2
        };
        let correction =
            self.exit.position().z - self.entry.position().z - profile.endpoint()[1] * diagonal;
        let (smooth, smooth_first, smooth_second) = smooth(t);
        let smooth_third = 60.0 * (1.0 - 6.0 * t + 6.0 * t * t);
        let velocity = (forward * first[0] + vertical * first[1]) * (length * diagonal)
            + vertical * (correction * smooth_first);
        let acceleration = (forward * second[0] + vertical * second[1])
            * (length.powi(2) * diagonal)
            + vertical * (correction * smooth_second);
        let jerk = (forward * third[0] + vertical * third[1]) * (length.powi(3) * diagonal)
            + vertical * (correction * smooth_third);
        let tangent = velocity.unit();
        let base_up = (forward * -tangent.z + vertical * tangent.dot(forward)).unit();
        let speed_squared = velocity.dot(velocity);
        let omega = -velocity.cross(acceleration).dot(left) / speed_squared;
        let omega_first = -velocity.cross(jerk).dot(left) / speed_squared
            - 2.0 * omega * velocity.dot(acceleration) / speed_squared;
        let base_first = tangent * -omega;
        let base_second = tangent * -omega_first - base_up * omega.powi(2);
        let (angle, angle_first, angle_second) = twist.sample(t * length, self.mirrored);
        let angle = angle + self.entry.bank.radians();
        let angle_first = angle_first * length;
        let angle_second = angle_second * length.powi(2);
        let (sin, cos) = angle.sin_cos();
        let up = base_up * cos + left * sin;
        let up_first = base_first * cos + (left * cos - base_up * sin) * angle_first;
        let up_second = base_second * cos - base_first * (2.0 * sin * angle_first)
            + (left * cos - base_up * sin) * angle_second
            - up * angle_first.powi(2);
        let heart = self.entry.position()
            + forward * (position[0] * diagonal)
            + vertical * (position[1] * diagonal + correction * smooth)
            + base_up * HEART_HEIGHT_METRES;
        RollSample {
            position: heart - up * HEART_HEIGHT_METRES,
            first: velocity + (base_first - up_first) * HEART_HEIGHT_METRES,
            second: acceleration + (base_second - up_second) * HEART_HEIGHT_METRES,
            heart,
            up,
            roll: angle,
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_u8(self.id as u8);
        hash.write_bytes(self.name.as_bytes());
        hash.write_u8(self.kind as u8);
        if let Some(speed) = self.launch_target_speed {
            hash.write_u8(1);
            hash.write_f64(speed);
        }
        hash.write_option(self.inversion, |hash, inversion| {
            hash.write_u8(inversion as u8);
        });
        hash.write_option(self.heartlining, |hash, heartlining| {
            hash.write_u8(heartlining as u8);
        });
        hash.write_option(self.structure_material, |hash, material| {
            hash.write_u8(material as u8);
        });
        hash.write_option(self.rated_speeds, |hash, speeds| {
            hash.write_f64(speeds.minimum_entry);
            hash.write_f64(speeds.maximum_entry);
        });
        self.entry.hash_into(hash);
        self.exit.hash_into(hash);
        self.shape.hash_into(hash);
        hash.write_f64(self.tangent_scale);
        hash.write_bool(self.mirrored);
        match self.family {
            Some(PieceFamily::Hill(family)) => {
                hash.write_u8(1);
                hash.write_u8(family.feel as u8);
                hash.write_u8(family.height_metres);
                hash.write_u16(family.span_metres);
                hash.write_u8(family.twist.map_or(0, |twist| twist as u8 + 1));
                hash.write_f64(family.design_speed_metres_per_second);
            }
            Some(PieceFamily::SpeedTurn {
                radius_metres,
                hand,
                design_speed_metres_per_second,
            }) => {
                hash.write_u8(2);
                hash.write_u8(radius_metres);
                hash.write_u8(hand as u8);
                hash.write_f64(design_speed_metres_per_second);
            }
            None => {}
        }
        let arc_table = self.arc_table();
        hash.write_f64(arc_table.maximum_curvature);
        hash.write_len(arc_table.lengths.len());
        for &length in &arc_table.lengths {
            hash.write_f64(length);
        }
        hash.write_len(arc_table.speeds.len());
        for &speed in &arc_table.speeds {
            hash.write_f64(speed);
        }
    }

    fn raw(&self, t: f64) -> (Vec3, Vec3, Vec3) {
        if let Shape::SpeedBank { design_speed } = self.shape {
            return self.speed_turn_raw(design_speed, t);
        }
        if let Shape::RolledProfile(profile, twist) = &self.shape {
            let sample = self.rolled_profile_sample(profile, *twist, t);
            return (sample.position, sample.first, sample.second);
        }
        if let Shape::Roll(geometry) = &self.shape {
            let sample =
                geometry.sample(self.entry, self.exit, self.mirrored, self.tangent_scale, t);
            return (sample.position, sample.first, sample.second);
        }
        if let Shape::Profile(profile) = &self.shape {
            let ascending = self.exit.z >= self.entry.z;
            let distance = if ascending {
                t * profile.length()
            } else {
                (1.0 - t) * profile.length()
            };
            let (position, first, second) = profile.sample(distance);
            let endpoint = profile.endpoint();
            let (x, z, dx, dz, ddx, ddz) = if ascending {
                (
                    position[0],
                    position[1],
                    first[0],
                    first[1],
                    second[0],
                    second[1],
                )
            } else if self.id == PieceId::HalfLoopDown {
                (
                    position[0] - endpoint[0],
                    position[1] - endpoint[1],
                    -first[0],
                    -first[1],
                    second[0],
                    second[1],
                )
            } else {
                (
                    endpoint[0] - position[0],
                    position[1] - endpoint[1],
                    first[0],
                    -first[1],
                    -second[0],
                    second[1],
                )
            };
            let forward = Vec3::new(self.entry.tangent().x, self.entry.tangent().y, 0.0).unit();
            let left = Vec3::new(-forward.y, forward.x, 0.0);
            let up = Vec3::new(0.0, 0.0, 1.0);
            let side_displacement = (self.exit.position() - self.entry.position()).dot(left);
            let (smooth, smooth_first, smooth_second) = smooth(t);
            let scale = profile.length();
            let diagonal_scale = if self.entry.heading.index() % 2 == 0 {
                1.0
            } else {
                std::f64::consts::SQRT_2
            };
            let vertical_correction = if diagonal_scale == 1.0 {
                0.0
            } else {
                self.exit.position().z
                    - self.entry.position().z
                    - if ascending { endpoint[1] } else { -endpoint[1] } * diagonal_scale
            };
            return (
                self.entry.position()
                    + forward * (x * diagonal_scale)
                    + left * (side_displacement * smooth)
                    + up * (z * diagonal_scale + vertical_correction * smooth),
                forward * (dx * scale * diagonal_scale)
                    + left * (side_displacement * smooth_first)
                    + up * (dz * scale * diagonal_scale + vertical_correction * smooth_first),
                forward * (ddx * scale.powi(2) * diagonal_scale)
                    + left * (side_displacement * smooth_second)
                    + up * (ddz * scale.powi(2) * diagonal_scale
                        + vertical_correction * smooth_second),
            );
        }
        let (s, ds, dds) = smooth(t);
        let t2 = t * t;
        let t3 = t2 * t;
        let t4 = t3 * t;
        let t5 = t4 * t;
        let h0 = t - 6.0 * t3 + 8.0 * t4 - 3.0 * t5;
        let h1 = -4.0 * t3 + 7.0 * t4 - 3.0 * t5;
        let dh0 = 1.0 - 18.0 * t2 + 32.0 * t3 - 15.0 * t4;
        let dh1 = -12.0 * t2 + 28.0 * t3 - 15.0 * t4;
        let ddh0 = -36.0 * t + 96.0 * t2 - 60.0 * t3;
        let ddh1 = -24.0 * t + 84.0 * t2 - 60.0 * t3;
        let p0 = self.entry.position();
        let delta = self.exit.position() - p0;
        let v0 = self.entry.tangent() * self.tangent_scale;
        let v1 = self.exit.tangent() * self.tangent_scale;
        let a0 = self.entry.curvature_vector() * self.tangent_scale.powi(2);
        let a1 = self.exit.curvature_vector() * self.tangent_scale.powi(2);
        let q0 = (t2 - 3.0 * t3 + 3.0 * t4 - t5) / 2.0;
        let q1 = (t3 - 2.0 * t4 + t5) / 2.0;
        let dq0 = (2.0 * t - 9.0 * t2 + 12.0 * t3 - 5.0 * t4) / 2.0;
        let dq1 = (3.0 * t2 - 8.0 * t3 + 5.0 * t4) / 2.0;
        let ddq0 = (2.0 - 18.0 * t + 36.0 * t2 - 20.0 * t3) / 2.0;
        let ddq1 = (6.0 * t - 24.0 * t2 + 20.0 * t3) / 2.0;
        let j0 = delta * 60.0 - v0 * 36.0 - v1 * 24.0 - a0 * 9.0 + a1 * 3.0;
        let j1 = delta * 60.0 - v0 * 24.0 - v1 * 36.0 - a0 * 3.0 + a1 * 9.0;
        let desired_j0 = v0
            * (-self.tangent_scale.powi(2)
                * self
                    .entry
                    .curvature_vector()
                    .dot(self.entry.curvature_vector()));
        let desired_j1 = v1
            * (-self.tangent_scale.powi(2)
                * self
                    .exit
                    .curvature_vector()
                    .dot(self.exit.curvature_vector()));
        let correction0 = (desired_j0 - j0) / 6.0;
        let correction1 = (j1 - desired_j1) / 6.0 - correction0;
        let t6 = t5 * t;
        let t7 = t6 * t;
        let f = t3 - 3.0 * t4 + 3.0 * t5 - t6;
        let g = t4 - 3.0 * t5 + 3.0 * t6 - t7;
        let df = 3.0 * t2 - 12.0 * t3 + 15.0 * t4 - 6.0 * t5;
        let dg = 4.0 * t3 - 15.0 * t4 + 18.0 * t5 - 7.0 * t6;
        let ddf = 6.0 * t - 36.0 * t2 + 60.0 * t3 - 30.0 * t4;
        let ddg = 12.0 * t2 - 60.0 * t3 + 90.0 * t4 - 42.0 * t5;
        let mut p = p0
            + delta * s
            + v0 * h0
            + v1 * h1
            + a0 * q0
            + a1 * q1
            + correction0 * f
            + correction1 * g;
        let mut d = delta * ds
            + v0 * dh0
            + v1 * dh1
            + a0 * dq0
            + a1 * dq1
            + correction0 * df
            + correction1 * dg;
        let mut dd = delta * dds
            + v0 * ddh0
            + v1 * ddh1
            + a0 * ddq0
            + a1 * ddq1
            + correction0 * ddf
            + correction1 * ddg;
        if let Some(height) = match &self.shape {
            Shape::AirtimeHill => Some(3.0),
            Shape::WaveTurn { bump, .. } => Some(*bump),
            _ => None,
        } {
            let bump = 64.0 * (t3 - 3.0 * t4 + 3.0 * t5 - t5 * t);
            let derivative = 64.0 * (3.0 * t2 - 12.0 * t3 + 15.0 * t4 - 6.0 * t5);
            let second = 64.0 * (6.0 * t - 36.0 * t2 + 60.0 * t3 - 30.0 * t4);
            p.z += height * bump;
            d.z += height * derivative;
            dd.z += height * second;
        }
        (p, d, dd)
    }
    #[cfg(test)]
    pub(crate) fn rebuild_arc(&mut self) {
        self.rebuild_arc_with_steps(ARC_STEPS);
    }
    #[cfg(test)]
    fn rebuild_arc_with_steps(&mut self, steps: usize) {
        self.arc_table = Arc::new(OnceLock::from(self.calculate_arc_table(steps)));
    }
    pub(crate) fn clear_arc_table(&mut self) {
        self.arc_table = Arc::new(OnceLock::new());
    }
    fn arc_table(&self) -> &ArcLengthTable {
        self.arc_table
            .get_or_init(|| self.calculate_arc_table(ARC_STEPS))
    }
    fn calculate_arc_table(&self, steps: usize) -> ArcLengthTable {
        let mut lengths = Vec::with_capacity(steps + 1);
        let mut speeds = Vec::with_capacity(steps + 1);
        lengths.push(0.0);
        let (_, initial_velocity, initial_acceleration) = self.raw(0.0);
        let mut previous = initial_velocity.length();
        let mut maximum_curvature = curvature(initial_velocity, initial_acceleration);
        speeds.push(previous);
        let mut total = 0.0;
        for index in 1..=steps {
            let (_, velocity, acceleration) = self.raw(index as f64 / steps as f64);
            let speed = velocity.length();
            speeds.push(speed);
            total += (previous + speed) / (2.0 * steps as f64);
            lengths.push(total);
            previous = speed;
            maximum_curvature = maximum_curvature.max(curvature(velocity, acceleration));
        }
        let maximum_curvature = if steps == ARC_STEPS {
            maximum_curvature
        } else {
            self.calculate_maximum_curvature()
        };
        ArcLengthTable {
            lengths,
            speeds,
            maximum_curvature,
        }
    }
    pub fn length(&self) -> f64 {
        *self.arc_table().lengths.last().expect("arc table")
    }
    /// The maximum curvature sampled at the arc table's parameter points.
    pub fn maximum_curvature(&self) -> f64 {
        self.arc_table().maximum_curvature
    }
    fn calculate_maximum_curvature(&self) -> f64 {
        (0..=ARC_STEPS)
            .map(|index| {
                let (_, velocity, acceleration) = self.raw(index as f64 / ARC_STEPS as f64);
                curvature(velocity, acceleration)
            })
            .fold(0.0, f64::max)
    }
    pub(super) fn parameter_at_distance(&self, distance: f64) -> f64 {
        assert!(
            distance.is_finite() && distance >= 0.0 && distance <= self.length(),
            "distance outside piece"
        );
        if distance == self.length() {
            return 1.0;
        }
        let arc_table = self.arc_table();
        let steps = arc_table.lengths.len() - 1;
        let index = arc_table
            .lengths
            .partition_point(|length| *length < distance)
            .min(steps);
        if index == 0 {
            0.0
        } else {
            let low = arc_table.lengths[index - 1];
            let start_speed = arc_table.speeds[index - 1];
            let end_speed = arc_table.speeds[index];
            let scaled_distance = (distance - low) * steps as f64;
            let discriminant =
                start_speed.powi(2) + 2.0 * (end_speed - start_speed) * scaled_distance;
            let fraction = 2.0 * scaled_distance / (start_speed + discriminant.sqrt());
            (index as f64 - 1.0 + fraction) / steps as f64
        }
    }
    pub(crate) fn tangent_at(&self, distance: f64) -> Vec3 {
        self.raw(self.parameter_at_distance(distance)).1.unit()
    }
    pub fn sample(&self, distance: f64) -> TrackSample {
        let t = self.parameter_at_distance(distance);
        if let Shape::SpeedBank { design_speed } = self.shape {
            let (position, heart, desired_up, roll) = self.speed_turn_frame(design_speed, t);
            let (_, first, second) = self.speed_turn_raw(design_speed, t);
            let tangent = first.unit();
            return TrackSample {
                position,
                heart,
                tangent,
                up: (desired_up - tangent * desired_up.dot(tangent)).unit(),
                curvature: curvature(first, second),
                roll,
            };
        }
        if let Shape::RolledProfile(profile, twist) = &self.shape {
            let sample = self.rolled_profile_sample(profile, *twist, t);
            return TrackSample {
                position: sample.position,
                heart: sample.heart,
                tangent: sample.first.unit(),
                up: sample.up,
                curvature: sample.first.cross(sample.second).length()
                    / sample.first.length().powi(3),
                roll: sample.roll,
            };
        }
        if let Shape::Roll(geometry) = &self.shape {
            let sample =
                geometry.sample(self.entry, self.exit, self.mirrored, self.tangent_scale, t);
            return TrackSample {
                position: sample.position,
                heart: sample.heart,
                tangent: sample.first.unit(),
                up: sample.up,
                curvature: sample.first.cross(sample.second).length()
                    / sample.first.length().powi(3),
                roll: sample.roll,
            };
        }
        let (position, velocity, acceleration) = self.raw(t);
        let tangent = velocity.unit();
        let horizontal = (tangent.x * tangent.x + tangent.y * tangent.y).sqrt();
        let base_up = if matches!(&self.shape, Shape::Profile(_)) {
            let forward = Vec3::new(self.entry.tangent().x, self.entry.tangent().y, 0.0).unit();
            (forward * (-tangent.z) + Vec3::new(0.0, 0.0, 1.0) * tangent.dot(forward)).unit()
        } else {
            Vec3::new(
                -tangent.x * tangent.z / horizontal,
                -tangent.y * tangent.z / horizontal,
                horizontal,
            )
        };
        let (s, _, _) = smooth(t);
        let overbank = if matches!(&self.shape, Shape::Overbank) {
            2.0 * std::f64::consts::PI / 3.0
                * (std::f64::consts::PI * t).sin().powi(2)
                * if self.mirrored { -1.0 } else { 1.0 }
        } else if let Shape::WaveTurn { peak, .. } = &self.shape {
            peak * 64.0 * t.powi(3) * (1.0 - t).powi(3) * if self.mirrored { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        let bank = if matches!(&self.shape, Shape::Profile(_)) {
            self.entry.bank.radians()
        } else {
            self.entry.bank.radians() + (self.exit.bank.radians() - self.entry.bank.radians()) * s
        };
        let angle = overbank + bank;
        let left = base_up.cross(tangent).unit();
        let up = base_up * angle.cos() + left * angle.sin();
        TrackSample {
            position,
            heart: position + up * HEART_HEIGHT_METRES,
            tangent,
            up,
            curvature: velocity.cross(acceleration).length() / velocity.length().powi(3),
            roll: angle
                + if matches!(&self.shape, Shape::Profile(_))
                    && (self.entry.bank == Bank::Inverted || self.exit.bank == Bank::Inverted)
                {
                    std::f64::consts::PI * s
                } else {
                    0.0
                },
        }
    }
    pub(crate) fn exit_at(&self, state: TrackState) -> Option<TrackState> {
        if state.slope != self.entry.slope
            || state.bank != self.entry.bank
            || state.horizontal_curvature != self.entry.horizontal_curvature
            || state.vertical_curvature != self.entry.vertical_curvature
        {
            return None;
        }
        let rotation = (state.heading.index() - self.entry.heading.index()).rem_euclid(8);
        let dx = self.exit.x - self.entry.x;
        let dy = self.exit.y - self.entry.y;
        let (x, y) = match rotation {
            0 => (dx, dy),
            1 => (dx - dy, dx + dy),
            2 => (-dy, dx),
            3 => (-dx - dy, dx - dy),
            4 => (-dx, -dy),
            5 => (dy - dx, -dx - dy),
            6 => (dy, -dx),
            7 => (dx + dy, dy - dx),
            _ => unreachable!(),
        };
        Some(TrackState {
            x: state.x + x,
            y: state.y + y,
            z: state.z
                + if rotation % 2 == 0 {
                    self.exit.z - self.entry.z
                } else {
                    (f64::from(self.exit.z - self.entry.z) * std::f64::consts::SQRT_2).round()
                        as i32
                },
            heading: Heading::from_index(self.exit.heading.index() + rotation),
            slope: self.exit.slope,
            bank: self.exit.bank,
            horizontal_curvature: self.exit.horizontal_curvature,
            vertical_curvature: self.exit.vertical_curvature,
        })
    }
    pub fn at(&self, state: TrackState) -> Option<Self> {
        let exit = self.exit_at(state)?;
        let rotation = (state.heading.index() - self.entry.heading.index()).rem_euclid(8);
        if rotation % 2 == 0 {
            self.arc_table();
        }
        let mut piece = self.clone();
        piece.exit = exit;
        piece.entry = state;
        if rotation % 2 != 0 {
            piece.tangent_scale *= (piece.exit.position() - piece.entry.position()).length()
                / (self.exit.position() - self.entry.position()).length();
        }
        if rotation % 2 != 0 {
            piece.clear_arc_table();
        }
        Some(piece)
    }
}

#[cfg(test)]
mod shape_tests {
    use std::sync::{Arc, OnceLock};

    use super::{
        Bank, HEART_HEIGHT_METRES, Heading, Heartlining, HorizontalCurvature, Piece, PieceId,
        PieceKind, Shape, Vec3,
    };
    use crate::state_hash::StateHasher;
    use crate::track::roll::RollGeometry;

    #[test]
    fn piece_family_preserves_legacy_hill_postcard_encoding() {
        use super::{Hand, HillFamily, HillFeel, HillTwist, PieceFamily};

        // The exact bytes are the version 2 encoding of Some(HillFamily).
        let legacy_bytes = [1, 0, 8, 48, 0, 0, 0, 0, 0, 0, 0, 52, 64];
        let hill = HillFamily {
            feel: HillFeel::Ejector,
            height_metres: 8,
            span_metres: 48,
            twist: None,
            design_speed_metres_per_second: 20.0,
        };
        assert_eq!(postcard::to_allocvec(&Some(hill)).unwrap(), legacy_bytes);
        let restored: Option<PieceFamily> = postcard::from_bytes(&legacy_bytes).unwrap();
        assert_eq!(restored, Some(PieceFamily::Hill(hill)));
        assert_eq!(postcard::to_allocvec(&restored).unwrap(), legacy_bytes);

        for feel in [
            HillFeel::Ejector,
            HillFeel::Floater,
            HillFeel::Speed,
            HillFeel::ZeroGRoll,
            HillFeel::Stall,
        ] {
            for twist in [
                None,
                Some(HillTwist::OuterBankLeft),
                Some(HillTwist::OuterBankRight),
                Some(HillTwist::LeftRoll),
                Some(HillTwist::RightRoll),
            ] {
                let hill = HillFamily {
                    feel,
                    twist,
                    ..hill
                };
                let old_bytes = postcard::to_allocvec(&Some(hill)).unwrap();
                let new_bytes = postcard::to_allocvec(&Some(PieceFamily::Hill(hill))).unwrap();
                assert_eq!(old_bytes, new_bytes);
                assert_eq!(
                    postcard::from_bytes::<Option<PieceFamily>>(&old_bytes).unwrap(),
                    Some(PieceFamily::Hill(hill))
                );
            }
        }
        for hand in [Hand::Left, Hand::Right] {
            let family = PieceFamily::SpeedTurn {
                radius_metres: 32,
                hand,
                design_speed_metres_per_second: 24.0,
            };
            let bytes = postcard::to_allocvec(&family).unwrap();
            assert_eq!(postcard::from_bytes::<PieceFamily>(&bytes).unwrap(), family);
        }
    }

    #[test]
    fn save_preserves_shared_uninitialized_arc_tables() {
        let mut piece = crate::track::catalog()[0].clone();
        piece.clear_arc_table();
        let mut translated = piece.clone();
        translated.entry.x += 100;
        translated.exit.x += 100;
        assert!(Arc::ptr_eq(&piece.arc_table, &translated.arc_table));
        assert!(piece.arc_table.get().is_none());
        let bytes = super::with_snapshot_arc_tables(|| postcard::to_allocvec(&(piece, translated)))
            .unwrap();
        let (first, second): (Piece, Piece) =
            super::with_snapshot_arc_tables(|| postcard::from_bytes(&bytes)).unwrap();
        assert!(Arc::ptr_eq(&first.arc_table, &second.arc_table));
        assert!(first.arc_table.get().is_none());
        first.length();
        assert!(second.arc_table.get().is_some());
    }
    use crate::track::{Slope, TrackState, VerticalCurvature};

    #[test]
    fn arc_length_table_is_built_on_first_geometry_query() {
        let mut piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HeartlineRoll)
            .unwrap()
            .clone();
        piece.clear_arc_table();
        assert!(piece.arc_table.get().is_none());

        assert!(piece.length() > 0.0);

        assert!(piece.arc_table.get().is_some());
    }

    #[test]
    fn arc_length_inversion_has_continuous_speed_at_table_boundaries() {
        let piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::BarrelDropLeft)
            .unwrap();
        let delta = 1e-5;
        for index in (1..super::ARC_STEPS).step_by(11) {
            let distance = piece.arc_table().lengths[index];
            let parameter = index as f64 / super::ARC_STEPS as f64;
            let expected = 1.0 / piece.raw(parameter).1.length();
            let before = (parameter - piece.parameter_at_distance(distance - delta)) / delta;
            let after = (piece.parameter_at_distance(distance + delta) - parameter) / delta;
            assert!(
                (before - expected).abs() < 1e-7,
                "boundary {index}: {before} != {expected}"
            );
            assert!(
                (after - expected).abs() < 1e-7,
                "boundary {index}: {after} != {expected}"
            );
        }
    }

    #[test]
    fn arc_speed_cache_matches_geometry_after_cloning_and_placement() {
        let piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::BarrelDropLeft)
            .unwrap();
        let cloned = piece.clone();
        let mut state = piece.entry;
        state.heading = Heading::NorthEast;
        let placed = piece.at(state).unwrap();
        for candidate in [piece, &cloned, &placed] {
            let arc_table = candidate.arc_table();
            assert_eq!(arc_table.speeds.len(), arc_table.lengths.len());
            let steps = arc_table.speeds.len() - 1;
            for (index, speed) in arc_table.speeds.iter().enumerate() {
                let raw_speed = candidate.raw(index as f64 / steps as f64).1.length();
                // Debug and release builds round the cached and raw paths differently (about 4e-10 here).
                assert!(
                    (*speed - raw_speed).abs() < 1e-8,
                    "boundary {index}: cached speed {speed} != raw speed {raw_speed}"
                );
            }
        }
    }

    #[test]
    fn rigid_placement_reuses_arc_length_and_curvature_cache() {
        let mut piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HeartlineRoll)
            .unwrap()
            .clone();
        piece.clear_arc_table();
        let placed = piece
            .at(TrackState {
                x: 32,
                y: 48,
                heading: Heading::North,
                ..piece.entry
            })
            .unwrap();

        assert_eq!(placed.arc_table(), piece.arc_table());
        let expected_maximum_curvature = (0..=super::ARC_STEPS)
            .map(|index| {
                let (_, velocity, acceleration) = piece.raw(index as f64 / super::ARC_STEPS as f64);
                super::curvature(velocity, acceleration)
            })
            .fold(0.0, f64::max);
        assert_eq!(piece.maximum_curvature(), expected_maximum_curvature);
    }

    #[test]
    fn diagonal_placement_rebuilds_geometry_and_curvature_cache() {
        let piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HeartlineRoll)
            .unwrap();
        let placed = piece
            .at(TrackState {
                x: 32,
                y: 48,
                heading: Heading::NorthEast,
                ..piece.entry
            })
            .unwrap();
        let mut expected = piece.clone();
        expected.entry = placed.entry;
        expected.exit = placed.exit;
        expected.tangent_scale *= (placed.exit.position() - placed.entry.position()).length()
            / (piece.exit.position() - piece.entry.position()).length();
        expected.rebuild_arc();
        let expected_maximum_curvature = (0..=super::ARC_STEPS)
            .map(|index| {
                let (_, velocity, acceleration) =
                    expected.raw(index as f64 / super::ARC_STEPS as f64);
                super::curvature(velocity, acceleration)
            })
            .fold(0.0, f64::max);

        assert_ne!(placed.arc_table(), piece.arc_table());
        assert_eq!(placed.arc_table(), expected.arc_table());
        assert_eq!(placed.maximum_curvature(), expected_maximum_curvature);
    }

    #[test]
    fn heart_acceleration_converges_as_arc_table_resolution_increases() {
        let piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::BarrelDropLeft)
            .unwrap();
        let mut pieces = Vec::new();
        for steps in [512, 1024, 2048, 4096] {
            let mut candidate = piece.clone();
            candidate.rebuild_arc_with_steps(steps);
            pieces.push(candidate);
        }
        let curvature = |piece: &Piece, fraction: f64| {
            let distance = piece.length() * fraction;
            let delta = 0.02;
            (piece.sample(distance - delta).heart - piece.sample(distance).heart * 2.0
                + piece.sample(distance + delta).heart)
                / delta.powi(2)
        };
        for index in 1..20 {
            let fraction = f64::from(index) / 20.0;
            let reference = curvature(&pieces[3], fraction);
            for candidate in &pieces[..3] {
                let error = (curvature(candidate, fraction) - reference).length();
                assert!(
                    error < 0.001,
                    "fraction {fraction}: heart curvature error {error}"
                );
            }
        }
    }

    #[test]
    fn connector_only_exit_matches_placed_piece_for_every_heading() {
        let entry = TrackState::origin();
        let piece = Piece {
            id: PieceId::Straight,
            name: "test connector",
            kind: PieceKind::Track,
            inversion: None,
            heartlining: None::<Heartlining>,
            structure_material: None,
            rated_speeds: None,
            family: None,
            entry,
            exit: TrackState {
                x: 8,
                y: 2,
                z: 4,
                heading: Heading::North,
                slope: Slope::Level,
                bank: Bank::Level,
                horizontal_curvature: HorizontalCurvature::Straight,
                vertical_curvature: VerticalCurvature::Straight,
            },
            shape: Shape::Plain,
            tangent_scale: 16.0,
            mirrored: false,
            launch_target_speed: None,
            arc_table: Arc::new(OnceLock::new()),
        };

        for heading in [
            Heading::East,
            Heading::NorthEast,
            Heading::North,
            Heading::NorthWest,
            Heading::West,
            Heading::SouthWest,
            Heading::South,
            Heading::SouthEast,
        ] {
            let start = TrackState { heading, ..entry };
            let placed = piece.at(start).unwrap();
            assert_eq!(piece.exit_at(start), Some(placed.exit), "{heading:?}");
        }
    }

    #[test]
    fn rolled_profile_rail_derivatives_match_position() {
        let mut piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| matches!(&piece.shape, Shape::Profile(_)))
            .unwrap()
            .clone();
        let Shape::Profile(profile) = &piece.shape else {
            unreachable!()
        };
        let length = profile.length();
        piece.shape = Shape::RolledProfile(
            Arc::clone(profile),
            super::Twist::OuterBank {
                peak: 110_f64.to_radians(),
                start: length * 0.2,
                end: length * 0.8,
            },
        );
        let step = 1e-6;
        for t in [0.1373, 0.3137, 0.4873, 0.6137, 0.8373] {
            let before = piece.raw(t - step);
            let center = piece.raw(t);
            let after = piece.raw(t + step);
            assert!(((after.0 - before.0) / (2.0 * step) - center.1).length() < 1e-4);
            assert!(((after.1 - before.1) / (2.0 * step) - center.2).length() < 1e-2);
        }
    }

    #[test]
    fn rolled_profiles_follow_the_planar_heart_and_join_connectors() {
        let pieces = crate::track::catalog::catalog();
        let mut count = 0;
        for piece in pieces
            .iter()
            .filter(|piece| matches!(piece.shape, Shape::RolledProfile(..)))
        {
            count += 1;
            let Shape::RolledProfile(profile, _) = &piece.shape else {
                unreachable!()
            };
            for heading in [Heading::East, Heading::NorthEast] {
                let mut entry = piece.entry;
                entry.heading = heading;
                let placed = piece.at(entry).unwrap();
                let mut planar = placed.clone();
                planar.shape = Shape::Profile(Arc::clone(profile));
                for index in 0..=100 {
                    let t = f64::from(index) / 100.0;
                    let sample = if let Shape::RolledProfile(profile, twist) = &placed.shape {
                        placed.rolled_profile_sample(profile, *twist, t)
                    } else {
                        unreachable!()
                    };
                    let (planar_rail, planar_velocity, _) = planar.raw(t);
                    let forward =
                        Vec3::new(placed.entry.tangent().x, placed.entry.tangent().y, 0.0).unit();
                    let planar_tangent = planar_velocity.unit();
                    let planar_up = Vec3::new(0.0, 0.0, 1.0) * planar_tangent.dot(forward)
                        - forward * planar_tangent.z;
                    let expected_heart = planar_rail + planar_up.unit() * HEART_HEIGHT_METRES;
                    assert!(
                        (sample.heart - expected_heart).length() < 1e-9,
                        "{:?} {heading:?} {t}",
                        piece.id
                    );
                    if index == 0 || index == 100 {
                        let connector = if index == 0 {
                            placed.entry
                        } else {
                            placed.exit
                        };
                        assert!(
                            (sample.position - connector.position()).length() < 1e-7,
                            "{:?} {heading:?} {t}",
                            piece.id
                        );
                        assert!(sample.first.unit().dot(connector.tangent().unit()) > 1.0 - 1e-10);
                        assert!(
                            sample.first.cross(sample.second).length()
                                / sample.first.length().powi(3)
                                < 1e-8
                        );
                    }
                    let mut mirrored = placed.clone();
                    mirrored.mirrored = !placed.mirrored;
                    let mirrored_sample =
                        if let Shape::RolledProfile(profile, twist) = &mirrored.shape {
                            mirrored.rolled_profile_sample(profile, *twist, t)
                        } else {
                            unreachable!()
                        };
                    assert!((sample.heart - mirrored_sample.heart).length() < 1e-9);
                    assert!((sample.roll + mirrored_sample.roll).abs() < 1e-9);
                    let forward =
                        Vec3::new(placed.entry.tangent().x, placed.entry.tangent().y, 0.0).unit();
                    let left = Vec3::new(-forward.y, forward.x, 0.0);
                    let delta = sample.position - mirrored_sample.position;
                    assert!((delta - left * delta.dot(left)).length() < 1e-9);
                    assert!(
                        ((sample.position + mirrored_sample.position) / 2.0 - sample.heart)
                            .dot(left)
                            .abs()
                            < 1e-9
                    );
                    assert!((sample.up.dot(left) + mirrored_sample.up.dot(left)).abs() < 1e-9);
                    assert!(
                        ((sample.up - mirrored_sample.up)
                            - left * (sample.up - mirrored_sample.up).dot(left))
                        .length()
                            < 1e-9
                    );
                }
            }
        }
        assert!(count > 0);
    }

    #[test]
    fn rolled_hill_inversions_limit_roll_rate_for_full_trains() {
        let mut failures = Vec::new();
        let mut count = 0;
        for piece in crate::track::catalog::catalog().iter().filter(|piece| {
            piece.family.is_some_and(|family| {
                matches!(
                    family,
                    super::PieceFamily::Hill(super::HillFamily {
                        feel: super::HillFeel::ZeroGRoll | super::HillFeel::Stall,
                        ..
                    })
                )
            })
        }) {
            count += 1;
            let Shape::RolledProfile(profile, twist) = &piece.shape else {
                unreachable!()
            };
            let entry_speed = piece.rated_speeds.unwrap().maximum_entry;
            let mut peak = 0.0_f64;
            for index in 0..=2000 {
                let distance = piece.length() * f64::from(index) / 2000.0;
                let t = piece.parameter_at_distance(distance);
                let (_, angle_first, _) = twist.sample(t * profile.length(), piece.mirrored);
                let roll_per_metre = angle_first * profile.length() / piece.raw(t).1.length();
                for cars in [6, 12] {
                    let spacing = 18.0 / cars as f64;
                    for car in 0..cars {
                        let mean_height = (0..cars)
                            .map(|other| {
                                let other_distance =
                                    distance + (car as f64 - other as f64) * spacing;
                                piece
                                    .sample(other_distance.clamp(0.0, piece.length()))
                                    .position
                                    .z
                                    - piece.entry.position().z
                            })
                            .sum::<f64>()
                            / cars as f64;
                        let speed = (entry_speed.powi(2) - 2.0 * 9.81 * mean_height).sqrt();
                        peak = peak.max((roll_per_metre * speed).abs().to_degrees());
                    }
                }
            }
            eprintln!(
                "{:?} at {entry_speed:.2} m/s: {peak:.3} degrees/s",
                piece.id
            );
            if peak > 200.0 + 1e-6 {
                failures.push((piece.id, peak));
            }
        }
        assert_eq!(count, 6);
        assert!(
            failures.is_empty(),
            "roll rates above 200 degrees/s: {failures:?}"
        );
    }

    #[test]
    fn shape_hash_keeps_the_existing_variant_tags() {
        let shapes = [Shape::Plain, Shape::AirtimeHill, Shape::Overbank];

        for (expected_tag, shape) in [0_u8, 1, 2].into_iter().zip(shapes) {
            let mut actual = StateHasher::new();
            shape.hash_into(&mut actual);

            let mut expected = StateHasher::new();
            expected.write_u8(expected_tag);

            assert_eq!(actual.finish(), expected.finish());
        }
    }

    #[test]
    fn shape_equality_distinguishes_variants() {
        assert_eq!(Shape::Plain, Shape::Plain);
        assert_ne!(Shape::Plain, Shape::AirtimeHill);
    }

    #[test]
    fn payload_shapes_use_stable_unique_tags_and_structural_equality() {
        let profile = Shape::Profile(Arc::new(crate::track::profile::test_profile()));
        let equivalent_profile = Shape::Profile(Arc::new(crate::track::profile::test_profile()));
        let roll = Shape::Roll(Arc::new(RollGeometry::spline()));

        assert_eq!(profile, equivalent_profile);
        assert_ne!(profile, roll);

        for (shape, tag) in [(profile, 11), (roll, 12)] {
            let mut actual = StateHasher::new();
            shape.hash_into(&mut actual);

            let mut expected = StateHasher::new();
            expected.write_u8(tag);
            match &shape {
                Shape::Profile(profile) => profile.hash_into(&mut expected),
                Shape::Roll(roll) => roll.hash_into(&mut expected),
                _ => unreachable!(),
            }

            assert_eq!(actual.finish(), expected.finish());
        }
    }
}
