use super::{Inversion, Piece, PieceId, PieceKind, Shape, Slope, TrackState, catalog};
use crate::state_hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftRules {
    pub max_slope: Slope,
    pub curved: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchType {
    LinearMotor,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportStyle {
    WoodenLattice,
    DenseWoodenLattice,
    SteelColumns,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Copy, Debug)]
pub struct LaunchParameters {
    pub target_speed: f64,
    pub acceleration: f64,
}
#[derive(Clone, Debug)]
pub struct CoasterModel {
    pub manufacturer: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub allowed_pieces: Vec<PieceId>,
    pub sloped_turn_slopes: Vec<Slope>,
    pub minimum_radius_metres: f64,
    pub minimum_height_cm: u16,
    pub allowed_inversions: Vec<Inversion>,
    pub lift_rules: Option<LiftRules>,
    pub launch_types: Vec<LaunchType>,
    pub support_style: SupportStyle,
    pub train: TrainConfiguration,
    pub track_material: Material,
    pub structure_material: Material,
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
            && matches!(piece.shape, Shape::Plain)
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
            .into_iter()
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
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                SteelLoop,
                SteelLoopRight,
                TwisterFanLeft,
                TwisterFanRight,
                TwisterHelixLeft,
                TwisterHelixRight,
                AirtimeHill,
                TightTwisterLeft,
                TightTwisterRight,
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 8.0,
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
                HalfTileStraight,
                OneTileStraight,
                OneTileGentleUp,
                OneTileGentleDown,
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
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Loop,
                LoopRight,
                Corkscrew,
                CorkscrewRight,
                HeartlineRoll,
                HeartlineRollRight,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                AirtimeHill,
                HybridOverbankLeft,
                HybridOverbankRight,
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 5.0,
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
        launch_types: vec![],
        support_style: SupportStyle::WoodenLattice,
        train: TrainConfiguration {
            cars: 6,
            rows_per_car: 2,
            seats_per_row: 2,
            length_metres: 18.0,
            width_metres: 2.0,
            height_metres: 2.5,
            launch: None,
        },
        track_material: Material::Steel,
        structure_material: Material::Wood,
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
                Straight,
                HalfTileStraight,
                OneTileStraight,
                OneTileGentleUp,
                OneTileGentleDown,
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
                FullOffsetLeft,
                FullOffsetRight,
                ShortRise,
                ShortFall,
                Loop,
                LoopRight,
                Corkscrew,
                CorkscrewRight,
                HeartlineRoll,
                HeartlineRollRight,
                LaunchShort,
                Launch,
                LaunchLong,
                Brakes,
                Station,
                TerrainRise,
                TerrainFall,
                AirtimeHill,
            ]
        },
        sloped_turn_slopes: vec![Slope::UpGentle, Slope::DownGentle],
        minimum_radius_metres: 5.0,
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
    }
}
