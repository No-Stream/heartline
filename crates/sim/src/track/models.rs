use super::colors::{CoasterColor, CoasterColors};
use super::{Inversion, Piece, PieceId, PieceKind, Shape, Slope, TrackState, catalog};
use crate::state_hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiftRules {
    pub max_slope: Slope,
    pub curved: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LaunchType {
    LinearMotor,
}
/// Sustained launch acceleration limit for linear motor launches (1.5 g).
pub const LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED: f64 = 1.5 * 9.81;
/// Maximum launch speed, based on the 240 km/h record launch speed.
pub const LAUNCH_MAX_SPEED_METRES_PER_SECOND: f64 = 240.0 / 3.6;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SupportStyle {
    WoodenLattice,
    DenseWoodenLattice,
    SteelColumns,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Material {
    Steel,
    Wood,
}
impl SupportStyle {
    pub fn for_piece(self, piece: &Piece) -> Self {
        match piece.structure_material {
            Some(Material::Steel) => Self::SteelColumns,
            Some(Material::Wood) => Self::WoodenLattice,
            None => self,
        }
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TrainConfiguration {
    pub cars: u8,
    pub rows_per_car: u8,
    pub seats_per_row: u8,
    pub length_metres: f64,
    pub width_metres: f64,
    pub height_metres: f64,
    pub launch: Option<LaunchParameters>,
}

impl TrainConfiguration {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_u8(self.cars);
        hash.write_u8(self.rows_per_car);
        hash.write_u8(self.seats_per_row);
        hash.write_f64(self.length_metres);
        hash.write_f64(self.width_metres);
        hash.write_f64(self.height_metres);
        hash.write_option(self.launch, |hash, launch| {
            hash.write_f64(launch.target_speed);
            hash.write_f64(launch.acceleration);
        });
    }
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaunchParameters {
    pub target_speed: f64,
    pub acceleration: f64,
}
/// Provisional game design targets for instantaneous rider forces, in multiples of gravity.
/// Real ride safety and comfort also depend on duration, direction and restraint design.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComfortableForceLimits {
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub maximum_lateral_g: f64,
    pub maximum_longitudinal_g: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirtimeRating {
    Weak,
    Good,
    Great,
    TooMuch,
    Dangerous,
}

impl ComfortableForceLimits {
    /// Rates negative vertical force using the airtime bands shown by the force overlay.
    pub fn classify_airtime(self, vertical_g: f64) -> AirtimeRating {
        let airtime_g = -vertical_g;
        let comfortable_airtime_limit_g = -self.minimum_vertical_g;
        if airtime_g <= 0.2 {
            AirtimeRating::Weak
        } else if airtime_g <= 0.9 {
            AirtimeRating::Good
        } else if airtime_g <= comfortable_airtime_limit_g {
            AirtimeRating::Great
        } else if airtime_g <= 1.3 * comfortable_airtime_limit_g {
            AirtimeRating::TooMuch
        } else {
            AirtimeRating::Dangerous
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchSpeedBounds {
    pub minimum: f64,
    pub maximum: f64,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CoasterModel {
    #[serde(with = "crate::save::catalog_text")]
    pub manufacturer: crate::save::CatalogText,
    #[serde(with = "crate::save::catalog_text")]
    pub name: crate::save::CatalogText,
    #[serde(with = "crate::save::catalog_text")]
    pub description: crate::save::CatalogText,
    pub allowed_pieces: Vec<PieceId>,
    pub sloped_turn_slopes: Vec<Slope>,
    pub minimum_radius_metres: f64,
    pub comfortable_force_limits: ComfortableForceLimits,
    pub minimum_height_cm: u16,
    pub allowed_inversions: Vec<Inversion>,
    pub lift_rules: Option<LiftRules>,
    pub launch_types: Vec<LaunchType>,
    pub support_style: SupportStyle,
    pub train: TrainConfiguration,
    pub track_material: Material,
    pub structure_material: Material,
    /// The colours a new coaster of this model starts with.
    pub default_colors: CoasterColors,
}
impl CoasterModel {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_bytes(self.manufacturer.as_bytes());
        hash.write_bytes(self.name.as_bytes());
        hash.write_bytes(self.description.as_bytes());
        hash.write_len(self.allowed_pieces.len());
        for piece in &self.allowed_pieces {
            hash.write_u8(*piece as u8);
        }
        hash.write_len(self.sloped_turn_slopes.len());
        for slope in &self.sloped_turn_slopes {
            hash.write_u8(*slope as u8);
        }
        hash.write_f64(self.minimum_radius_metres);
        hash.write_f64(self.comfortable_force_limits.minimum_vertical_g);
        hash.write_f64(self.comfortable_force_limits.maximum_vertical_g);
        hash.write_f64(self.comfortable_force_limits.maximum_lateral_g);
        hash.write_f64(self.comfortable_force_limits.maximum_longitudinal_g);
        hash.write_u16(self.minimum_height_cm);
        hash.write_len(self.allowed_inversions.len());
        for inversion in &self.allowed_inversions {
            hash.write_u8(*inversion as u8);
        }
        hash.write_option(self.lift_rules, |hash, rules| {
            hash.write_u8(rules.max_slope as u8);
            hash.write_bool(rules.curved);
        });
        hash.write_len(self.launch_types.len());
        for launch in &self.launch_types {
            hash.write_u8(*launch as u8);
        }
        hash.write_u8(self.support_style as u8);
        self.train.hash_into(hash);
        hash.write_u8(self.track_material as u8);
        hash.write_u8(self.structure_material as u8);
        self.default_colors.hash_into(hash);
    }

    pub fn allows_piece(&self, piece: &Piece) -> bool {
        self.allowed_pieces.contains(&piece.id)
            && (piece.entry.heading == piece.exit.heading
                || piece.entry.slope == Slope::Level
                || self.sloped_turn_slopes.contains(&piece.entry.slope))
            && piece.inversion.is_none_or(|inversion| {
                self.allowed_inversions.contains(&inversion)
                    && (self.track_material != Material::Wood
                        || piece.structure_material.unwrap_or(self.structure_material)
                            == Material::Steel)
            })
    }
    pub fn allows_chain(&self, piece: &Piece) -> bool {
        let Some(rules) = self.lift_rules else {
            return false;
        };
        piece.kind == PieceKind::Track
            && piece.inversion.is_none()
            && matches!(piece.shape, Shape::Plain | Shape::Profile(_))
            && [piece.entry.slope, piece.exit.slope].iter().all(|slope| {
                slope.gradient() >= 0.0 && slope.gradient() <= rules.max_slope.gradient()
            })
            && (rules.curved || piece.entry.heading == piece.exit.heading)
    }
    pub fn support_style_for_piece(&self, piece: &Piece) -> SupportStyle {
        self.support_style.for_piece(piece)
    }
    pub fn available_pieces(&self, end: TrackState) -> Vec<Piece> {
        catalog()
            .iter()
            .filter(|piece| self.allows_piece(piece))
            .filter_map(|piece| piece.at(end))
            .filter(|piece| piece.maximum_curvature() <= 1.0 / self.minimum_radius_metres)
            .collect()
    }
}
pub fn wooden_twister_model() -> CoasterModel {
    CoasterModel {
        manufacturer: "Great Coaster Workshop",
        name: "Briar Blitz",
        description: "An articulated wooden twister with a chain lift and dense timber structure. The steel-structure loop is a plausible extension.",
        // A steel-structure loop is a plausible extension to a wooden twister.
        allowed_pieces: {
            use PieceId::*;
            vec![
                Straight,
                BankedStraightLeft,
                BankedStraightRight,
                RolloutLeft,
                RolloutRight,
                HalfTileStraight,
                OneTileStraight,
                OneTileGentleUp,
                OneTileGentleDown,
                GentleUp,
                GentleDown,
                SteepUp,
                SteepDown,
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
                TurnSmallStandalone,
                TurnSmallRightEntry,
                TurnSmallRight,
                TurnSmallRightExit,
                TurnSmallRightStandalone,
                TurnLargeEntry,
                TurnLarge,
                TurnLargeExit,
                TurnLargeStandalone,
                TurnLargeRightEntry,
                TurnLargeRight,
                TurnLargeRightExit,
                TurnLargeRightStandalone,
                VerticalUpEntry,
                VerticalUpContinue,
                VerticalUpExit,
                VerticalDownEntry,
                VerticalDownContinue,
                VerticalDownExit,
                BankedDropLeft,
                BankedDropRight,
                BankLeft,
                UnbankLeft,
                BankRight,
                UnbankRight,
                BankedTurnEntry,
                BankedTurn,
                BankedTurnExit,
                BankedTurnStandalone,
                SmallRightBankedTurnEntry,
                SmallRightBankedTurn,
                SmallRightBankedTurnExit,
                SmallRightBankedTurnStandalone,
                LargeLeftBankedTurnEntry,
                LargeLeftBankedTurn,
                LargeLeftBankedTurnExit,
                LargeLeftBankedTurnStandalone,
                LargeRightBankedTurnEntry,
                LargeRightBankedTurn,
                LargeRightBankedTurnExit,
                LargeRightBankedTurnStandalone,
                OffsetLeft,
                OffsetRight,
                ShortOffsetLeft,
                ShortOffsetRight,
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                SteelLoop,
                TopHat,
                SteelLoopRight,
                TwisterFanLeft,
                TwisterFanRight,
                TwisterHelixLeft,
                TwisterHelixRight,
                AirtimeHill,
                FloaterHill4x48,
                FloaterHill8x64,
                FloaterHill12x80,
                FloaterHill6x64,
                FloaterHill12x96,
                FloaterHill16x112,
                SpeedHill4x64,
                SpeedHill6x80,
                WoodenEjectorHill6x48,
                TightTwisterLeft,
                TightTwisterRight,
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 8.0,
        comfortable_force_limits: ComfortableForceLimits {
            minimum_vertical_g: -1.0,
            maximum_vertical_g: 4.0,
            maximum_lateral_g: 1.5,
            maximum_longitudinal_g: 1.5,
        },
        minimum_height_cm: 122,
        allowed_inversions: vec![Inversion::VerticalLoop],
        lift_rules: Some(LiftRules {
            max_slope: Slope::UpSteep,
            curved: true,
        }),
        launch_types: vec![],
        support_style: SupportStyle::DenseWoodenLattice,
        train: TrainConfiguration {
            cars: 12,
            rows_per_car: 1,
            seats_per_row: 2,
            length_metres: 18.0,
            width_metres: 2.0,
            height_metres: 2.5,
            launch: None,
        },
        track_material: Material::Wood,
        structure_material: Material::Wood,
        default_colors: CoasterColors {
            track: CoasterColor::Timber,
            supports: CoasterColor::Timber,
            trains: CoasterColor::Slate,
        },
    }
}
pub fn hybrid_model() -> CoasterModel {
    CoasterModel {
        manufacturer: "Rocky Road Construction",
        name: "Timber Tumbler",
        description: "Steel box track on a wooden structure. Some elements are plausible extensions beyond the rides that inspired this model.",
        // Vertical loops are a plausible extension to the hybrid inversion repertoire.
        allowed_pieces: {
            use PieceId::*;
            vec![
                Straight,
                BankedStraightLeft,
                BankedStraightRight,
                RolloutLeft,
                RolloutRight,
                HalfTileStraight,
                OneTileStraight,
                OneTileGentleUp,
                OneTileGentleDown,
                GentleUp,
                GentleDown,
                SteepUp,
                SteepDown,
                LevelToSixtyUp,
                SixtyUpToLevel,
                LevelToSixtyDown,
                SixtyDownToLevel,
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
                TurnSmallStandalone,
                TurnSmallRightEntry,
                TurnSmallRight,
                TurnSmallRightExit,
                TurnSmallRightStandalone,
                TurnLargeEntry,
                TurnLarge,
                TurnLargeExit,
                TurnLargeStandalone,
                TurnLargeRightEntry,
                TurnLargeRight,
                TurnLargeRightExit,
                TurnLargeRightStandalone,
                VerticalUpEntry,
                VerticalUpContinue,
                VerticalUpExit,
                VerticalDownEntry,
                VerticalDownContinue,
                VerticalDownExit,
                BankedDropLeft,
                BankedDropRight,
                BankLeft,
                UnbankLeft,
                BankRight,
                UnbankRight,
                BankedTurnEntry,
                BankedTurn,
                BankedTurnExit,
                BankedTurnStandalone,
                SmallRightBankedTurnEntry,
                SmallRightBankedTurn,
                SmallRightBankedTurnExit,
                SmallRightBankedTurnStandalone,
                LargeLeftBankedTurnEntry,
                LargeLeftBankedTurn,
                LargeLeftBankedTurnExit,
                LargeLeftBankedTurnStandalone,
                LargeRightBankedTurnEntry,
                LargeRightBankedTurn,
                LargeRightBankedTurnExit,
                LargeRightBankedTurnStandalone,
                OffsetLeft,
                OffsetRight,
                ShortOffsetLeft,
                ShortOffsetRight,
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Loop,
                LoopRight,
                LargeLoop,
                LargeLoopRight,
                Corkscrew,
                CorkscrewRight,
                HeartlineRoll,
                HeartlineRollRight,
                HalfLoopUp,
                HalfLoopDown,
                HalfCorkscrewLeftUp,
                HalfCorkscrewRightUp,
                HalfCorkscrewLeftDown,
                HalfCorkscrewRightDown,
                RavenRollLeftUp,
                RavenRollRightUp,
                RavenRollLeftDown,
                RavenRollRightDown,
                LaunchShort,
                Launch,
                LaunchLong,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                AirtimeHill,
                WaveTurnLeft,
                WaveTurnRight,
                HybridOverbankLeft,
                HybridOverbankRight,
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
                TwistedDropLeft,
                TwistedDropRight,
                BarrelDropLeft,
                BarrelDropRight,
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 5.0,
        comfortable_force_limits: ComfortableForceLimits {
            minimum_vertical_g: -1.5,
            maximum_vertical_g: 4.5,
            maximum_lateral_g: 1.5,
            maximum_longitudinal_g: 1.5,
        },
        minimum_height_cm: 122,
        allowed_inversions: vec![
            Inversion::VerticalLoop,
            Inversion::Corkscrew,
            Inversion::HeartlineRoll,
        ],
        lift_rules: Some(LiftRules {
            max_slope: Slope::UpSixty,
            curved: true,
        }),
        launch_types: vec![LaunchType::LinearMotor],
        support_style: SupportStyle::WoodenLattice,
        train: TrainConfiguration {
            cars: 6,
            rows_per_car: 2,
            seats_per_row: 2,
            length_metres: 18.0,
            width_metres: 2.0,
            height_metres: 2.5,
            launch: Some(LaunchParameters {
                target_speed: 31.3,
                acceleration: 5.6,
            }),
        },
        track_material: Material::Steel,
        structure_material: Material::Wood,
        default_colors: CoasterColors {
            track: CoasterColor::Coral,
            supports: CoasterColor::Timber,
            trains: CoasterColor::Sage,
        },
    }
}

pub fn multi_launch_model() -> CoasterModel {
    CoasterModel {
        manufacturer: "Impulse Engineering",
        name: "Velocity Viper",
        description: "A steel coaster with linear motor launches. Some elements are plausible extensions beyond the rides that inspired this model.",
        // Vertical loops and corkscrews are plausible extensions to LSM layouts.
        allowed_pieces: {
            use PieceId::*;
            vec![
                LaunchedTopHat,
                Straight,
                BankedStraightLeft,
                BankedStraightRight,
                RolloutLeft,
                RolloutRight,
                HalfTileStraight,
                OneTileStraight,
                OneTileGentleUp,
                OneTileGentleDown,
                GentleUp,
                GentleDown,
                SteepUp,
                SteepDown,
                LevelToSixtyUp,
                SixtyUpToLevel,
                LevelToSixtyDown,
                SixtyDownToLevel,
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
                TurnSmallStandalone,
                TurnSmallRightEntry,
                TurnSmallRight,
                TurnSmallRightExit,
                TurnSmallRightStandalone,
                TurnLargeEntry,
                TurnLarge,
                TurnLargeExit,
                TurnLargeStandalone,
                TurnLargeRightEntry,
                TurnLargeRight,
                TurnLargeRightExit,
                TurnLargeRightStandalone,
                VerticalUpEntry,
                VerticalUpContinue,
                VerticalUpExit,
                VerticalDownEntry,
                VerticalDownContinue,
                VerticalDownExit,
                BankedDropLeft,
                BankedDropRight,
                BankLeft,
                UnbankLeft,
                BankRight,
                UnbankRight,
                BankedTurnEntry,
                BankedTurn,
                BankedTurnExit,
                BankedTurnStandalone,
                SmallRightBankedTurnEntry,
                SmallRightBankedTurn,
                SmallRightBankedTurnExit,
                SmallRightBankedTurnStandalone,
                LargeLeftBankedTurnEntry,
                LargeLeftBankedTurn,
                LargeLeftBankedTurnExit,
                LargeLeftBankedTurnStandalone,
                LargeRightBankedTurnEntry,
                LargeRightBankedTurn,
                LargeRightBankedTurnExit,
                LargeRightBankedTurnStandalone,
                OffsetLeft,
                OffsetRight,
                ShortOffsetLeft,
                ShortOffsetRight,
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Loop,
                LoopRight,
                LargeLoop,
                LargeLoopRight,
                Corkscrew,
                CorkscrewRight,
                HeartlineRoll,
                HeartlineRollRight,
                HalfLoopUp,
                HalfLoopDown,
                HalfCorkscrewLeftUp,
                HalfCorkscrewRightUp,
                HalfCorkscrewLeftDown,
                HalfCorkscrewRightDown,
                RavenRollLeftUp,
                RavenRollRightUp,
                RavenRollLeftDown,
                RavenRollRightDown,
                LaunchShort,
                Launch,
                LaunchLong,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                AirtimeHill,
                WaveTurnLeft,
                WaveTurnRight,
                HybridOverbankLeft,
                HybridOverbankRight,
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
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 5.0,
        comfortable_force_limits: ComfortableForceLimits {
            minimum_vertical_g: -1.5,
            maximum_vertical_g: 5.0,
            maximum_lateral_g: 1.5,
            maximum_longitudinal_g: 1.5,
        },
        minimum_height_cm: 132,
        allowed_inversions: vec![
            Inversion::VerticalLoop,
            Inversion::Corkscrew,
            Inversion::HeartlineRoll,
        ],
        lift_rules: Some(LiftRules {
            max_slope: Slope::UpSixty,
            curved: true,
        }),
        launch_types: vec![LaunchType::LinearMotor],
        support_style: SupportStyle::SteelColumns,
        train: TrainConfiguration {
            cars: 6,
            rows_per_car: 2,
            seats_per_row: 2,
            length_metres: 18.0,
            width_metres: 2.0,
            height_metres: 2.5,
            launch: Some(LaunchParameters {
                target_speed: 31.3,
                acceleration: 5.6,
            }),
        },
        track_material: Material::Steel,
        structure_material: Material::Steel,
        default_colors: CoasterColors {
            track: CoasterColor::Teal,
            supports: CoasterColor::Cream,
            trains: CoasterColor::Coral,
        },
    }
}
