use std::f64::consts::{FRAC_PI_4, TAU};
use std::ops::{Add, Div, Mul, Sub};

use super::Material;
use crate::state_hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    Right,
    Level,
    Left,
}
impl Bank {
    pub fn radians(self) -> f64 {
        match self {
            Self::Right => -FRAC_PI_4,
            Self::Level => 0.0,
            Self::Left => FRAC_PI_4,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalCurvature {
    Straight,
    LeftSmall,
    LeftLarge,
    RightSmall,
    RightLarge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalCurvature {
    Straight,
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Track,
    Lift,
    Launch,
    Brakes,
    Station,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inversion {
    VerticalLoop,
    Corkscrew,
    HeartlineRoll,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape {
    Plain,
    AirtimeHill,
    Overbank,
    Loop,
    Corkscrew,
    Heartline,
}
#[derive(Clone, Debug)]
pub struct Piece {
    pub id: PieceId,
    pub name: &'static str,
    pub kind: PieceKind,
    pub inversion: Option<Inversion>,
    pub structure_material: Option<Material>,
    pub entry: TrackState,
    pub exit: TrackState,
    pub(crate) shape: Shape,
    pub(crate) tangent_scale: f64,
    pub(crate) mirrored: bool,
    pub(crate) arc_lengths: Vec<f64>,
}
#[derive(Clone, Copy, Debug)]
pub struct TrackSample {
    pub position: Vec3,
    pub tangent: Vec3,
    pub up: Vec3,
    pub curvature: f64,
    /// Unwrapped radians relative to the curve's unbanked frame; compare modulo 2π at joins.
    pub roll: f64,
}
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
impl Piece {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_u8(self.id as u8);
        hash.write_bytes(self.name.as_bytes());
        hash.write_u8(self.kind as u8);
        hash.write_option(self.inversion, |hash, inversion| {
            hash.write_u8(inversion as u8);
        });
        hash.write_option(self.structure_material, |hash, material| {
            hash.write_u8(material as u8);
        });
        self.entry.hash_into(hash);
        self.exit.hash_into(hash);
        hash.write_u8(self.shape as u8);
        hash.write_f64(self.tangent_scale);
        hash.write_bool(self.mirrored);
        hash.write_len(self.arc_lengths.len());
        for &length in &self.arc_lengths {
            hash.write_f64(length);
        }
    }

    fn raw(&self, t: f64) -> (Vec3, Vec3, Vec3) {
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
        if matches!(self.shape, Shape::AirtimeHill) {
            let height = 3.0;
            let bump = 64.0 * (t3 - 3.0 * t4 + 3.0 * t5 - t5 * t);
            let derivative = 64.0 * (3.0 * t2 - 12.0 * t3 + 15.0 * t4 - 6.0 * t5);
            let second = 64.0 * (6.0 * t - 36.0 * t2 + 60.0 * t3 - 30.0 * t4);
            p.z += height * bump;
            d.z += height * derivative;
            dd.z += height * second;
        }
        if matches!(
            self.shape,
            Shape::Loop | Shape::Corkscrew | Shape::Heartline
        ) {
            let angle = TAU * (35.0 * t4 - 84.0 * t5 + 70.0 * t6 - 20.0 * t7);
            let da = TAU * (140.0 * t3 - 420.0 * t4 + 420.0 * t5 - 140.0 * t6);
            let dda = TAU * (420.0 * t2 - 1680.0 * t3 + 2100.0 * t4 - 840.0 * t5);
            let forward = Vec3::new(self.entry.tangent().x, self.entry.tangent().y, 0.0).unit();
            let left = Vec3::new(-forward.y, forward.x, 0.0);
            let vertical = Vec3::new(0.0, 0.0, 1.0);
            let (a, b, radius) = if matches!(self.shape, Shape::Loop) {
                (forward, vertical, 16.0)
            } else if matches!(self.shape, Shape::Corkscrew) {
                (left, vertical, 1.5)
            } else {
                (left * -1.0, vertical, 1.0)
            };
            let a = if self.mirrored && !matches!(self.shape, Shape::Loop) {
                a * -1.0
            } else {
                a
            };
            let radius = if self.entry.heading.index() % 2 == 0 {
                radius
            } else {
                radius * std::f64::consts::SQRT_2
            };
            p = p + (a * angle.sin() + b * (1.0 - angle.cos())) * radius;
            d = d + (a * (angle.cos() * da) + b * (angle.sin() * da)) * radius;
            dd = dd
                + (a * (angle.cos() * dda - angle.sin() * da * da)
                    + b * (angle.sin() * dda + angle.cos() * da * da))
                    * radius;
        }
        (p, d, dd)
    }
    pub(crate) fn rebuild_arc(&mut self) {
        self.arc_lengths = Vec::with_capacity(ARC_STEPS + 1);
        self.arc_lengths.push(0.0);
        let mut previous = self.raw(0.0).1.length();
        let mut total = 0.0;
        for index in 1..=ARC_STEPS {
            let speed = self.raw(index as f64 / ARC_STEPS as f64).1.length();
            total += (previous + speed) / (2.0 * ARC_STEPS as f64);
            self.arc_lengths.push(total);
            previous = speed;
        }
    }
    pub fn length(&self) -> f64 {
        *self.arc_lengths.last().expect("arc table")
    }
    /// The maximum curvature sampled at the arc table's parameter points.
    pub fn maximum_curvature(&self) -> f64 {
        (0..=ARC_STEPS)
            .map(|index| {
                let (_, velocity, acceleration) = self.raw(index as f64 / ARC_STEPS as f64);
                velocity.cross(acceleration).length() / velocity.length().powi(3)
            })
            .fold(0.0, f64::max)
    }
    fn parameter_at_distance(&self, distance: f64) -> f64 {
        assert!(
            distance.is_finite() && distance >= 0.0 && distance <= self.length(),
            "distance outside piece"
        );
        let index = self
            .arc_lengths
            .partition_point(|length| *length < distance)
            .min(ARC_STEPS);
        if index == 0 {
            0.0
        } else {
            let low = self.arc_lengths[index - 1];
            (index as f64 - 1.0 + (distance - low) / (self.arc_lengths[index] - low))
                / ARC_STEPS as f64
        }
    }
    pub(crate) fn tangent_at(&self, distance: f64) -> Vec3 {
        self.raw(self.parameter_at_distance(distance)).1.unit()
    }
    pub fn sample(&self, distance: f64) -> TrackSample {
        let t = self.parameter_at_distance(distance);
        let (position, velocity, acceleration) = self.raw(t);
        let tangent = velocity.unit();
        let horizontal = (tangent.x * tangent.x + tangent.y * tangent.y).sqrt();
        let base_up = if matches!(self.shape, Shape::Loop) {
            let forward = Vec3::new(self.entry.tangent().x, self.entry.tangent().y, 0.0).unit();
            forward * (-tangent.z) + Vec3::new(0.0, 0.0, 1.0) * tangent.dot(forward)
        } else {
            Vec3::new(
                -tangent.x * tangent.z / horizontal,
                -tangent.y * tangent.z / horizontal,
                horizontal,
            )
        };
        let (s, _, _) = smooth(t);
        let turns = if matches!(self.shape, Shape::Corkscrew | Shape::Heartline) {
            TAU * s * if self.mirrored { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        let overbank = if matches!(self.shape, Shape::Overbank) {
            2.0 * std::f64::consts::PI / 3.0
                * (std::f64::consts::PI * t).sin().powi(2)
                * if self.mirrored { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        let angle = overbank
            + self.entry.bank.radians()
            + (self.exit.bank.radians() - self.entry.bank.radians()) * s
            + turns;
        let left = base_up.cross(tangent).unit();
        let up = base_up * angle.cos() + left * angle.sin();
        TrackSample {
            position,
            tangent,
            up,
            curvature: velocity.cross(acceleration).length() / velocity.length().powi(3),
            roll: angle,
        }
    }
    pub fn at(&self, state: TrackState) -> Option<Self> {
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
        let mut piece = self.clone();
        piece.exit = TrackState {
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
        };
        piece.entry = state;
        if rotation % 2 != 0 {
            piece.tangent_scale *= (piece.exit.position() - piece.entry.position()).length()
                / (self.exit.position() - self.entry.position()).length();
        }
        piece.rebuild_arc();
        Some(piece)
    }
}
