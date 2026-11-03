//! Track geometry uses metres; grid coordinates are half tiles (two metres).
//! X and Y are horizontal, Z is up, and headings turn counterclockwise from X.
//! Positive bank rotates the up vector towards the left of travel.

//! Seventh-degree Hermite curves constrain endpoint curvature and its rate as well as position
//! and tangent, preserving exact grid endpoints and smooth forces at compatible connectors.
pub mod supports;

use std::f64::consts::{FRAC_PI_4, TAU};
use std::ops::{Add, Div, Mul, Sub};

#[cfg(test)]
mod tests;

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
    fn unit(self) -> Self {
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
    fn index(self) -> i32 {
        self as i32
    }
    fn from_index(index: i32) -> Self {
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
    DownModerate,
    DownGentle,
    Level,
    UpGentle,
    UpModerate,
    UpSteep,
}
impl Slope {
    fn gradient(self) -> f64 {
        match self {
            Self::DownSteep => -1.0,
            Self::DownModerate => -0.75,
            Self::DownGentle => -0.5,
            Self::Level => 0.0,
            Self::UpGentle => 0.5,
            Self::UpModerate => 0.75,
            Self::UpSteep => 1.0,
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceId {
    Straight,
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
    Lift,
    LiftFlat,
    LiftLevelToGentleUp,
    LiftGentleUpToLevel,
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
enum Shape {
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
    shape: Shape,
    tangent_scale: f64,
    mirrored: bool,
    arc_lengths: Vec<f64>,
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
    fn curvature_vector(self) -> Vec3 {
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
    fn rebuild_arc(&mut self) {
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
    pub fn sample(&self, distance: f64) -> TrackSample {
        assert!(
            distance.is_finite() && distance >= 0.0 && distance <= self.length(),
            "distance outside piece"
        );
        let index = self
            .arc_lengths
            .partition_point(|length| *length < distance)
            .min(ARC_STEPS);
        let t = if index == 0 {
            0.0
        } else {
            let low = self.arc_lengths[index - 1];
            (index as f64 - 1.0 + (distance - low) / (self.arc_lengths[index] - low))
                / ARC_STEPS as f64
        };
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

/// Diagonal templates stretch horizontally by sqrt(2) so all endpoints stay on the grid.
pub fn catalog() -> Vec<Piece> {
    use Bank::{Left, Level as FlatBank, Right};
    use Heading::{East, North};
    use PieceId::*;
    use Slope::{DownGentle, DownModerate, DownSteep, Level, UpGentle, UpModerate, UpSteep};
    let definitions = [
        (
            Straight,
            "Straight",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleUp,
            "Gentle climb",
            8,
            0,
            4,
            East,
            UpGentle,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleDown,
            "Gentle descent",
            8,
            0,
            -4,
            East,
            DownGentle,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepUp,
            "Steep climb",
            8,
            0,
            8,
            East,
            UpSteep,
            UpSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepDown,
            "Steep descent",
            8,
            0,
            -8,
            East,
            DownSteep,
            DownSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            LevelToGentleUp,
            "Begin gentle climb",
            8,
            0,
            2,
            East,
            Level,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleUpToLevel,
            "End gentle climb",
            8,
            0,
            2,
            East,
            UpGentle,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleToSteepUp,
            "Begin steep climb",
            8,
            0,
            6,
            East,
            UpGentle,
            UpSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepToGentleUp,
            "End steep climb",
            8,
            0,
            6,
            East,
            UpSteep,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            LevelToGentleDown,
            "Begin gentle descent",
            8,
            0,
            -2,
            East,
            Level,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleDownToLevel,
            "End gentle descent",
            8,
            0,
            -2,
            East,
            DownGentle,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleToSteepDown,
            "Begin steep descent",
            8,
            0,
            -6,
            East,
            DownGentle,
            DownSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepToGentleDown,
            "End steep descent",
            8,
            0,
            -6,
            East,
            DownSteep,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            TurnSmall,
            "Small left turn",
            8,
            8,
            0,
            North,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            TurnLarge,
            "Large left turn",
            16,
            16,
            0,
            North,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            BankLeft,
            "Bank left",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            Left,
            Shape::Plain,
        ),
        (
            UnbankLeft,
            "Level from left bank",
            8,
            0,
            0,
            East,
            Level,
            Level,
            Left,
            FlatBank,
            Shape::Plain,
        ),
        (
            BankRight,
            "Bank right",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            Right,
            Shape::Plain,
        ),
        (
            UnbankRight,
            "Level from right bank",
            8,
            0,
            0,
            East,
            Level,
            Level,
            Right,
            FlatBank,
            Shape::Plain,
        ),
        (
            LargeLeftBankedTurn,
            "Large left-banked left turn",
            16,
            16,
            0,
            North,
            Level,
            Level,
            Left,
            Left,
            Shape::Plain,
        ),
        (
            BankedTurn,
            "Small left-banked left turn",
            8,
            8,
            0,
            North,
            Level,
            Level,
            Left,
            Left,
            Shape::Plain,
        ),
        (
            OffsetLeft,
            "Left half-tile offset",
            12,
            1,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            Loop,
            "Vertical loop",
            32,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Loop,
        ),
        (
            Corkscrew,
            "Left corkscrew",
            16,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Corkscrew,
        ),
        (
            HeartlineRoll,
            "Left heartline roll",
            16,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Heartline,
        ),
        (
            Lift,
            "Chain lift",
            8,
            0,
            4,
            East,
            UpGentle,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            Brakes,
            "Brakes",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            Station,
            "Station",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
    ];
    let mut pieces: Vec<Piece> = definitions
        .into_iter()
        .map(
            |(
                id,
                name,
                x,
                y,
                z,
                heading,
                entry_slope,
                exit_slope,
                entry_bank,
                exit_bank,
                shape,
            )| {
                let entry = TrackState {
                    slope: entry_slope,
                    bank: entry_bank,
                    ..TrackState::origin()
                };
                let exit = TrackState {
                    x,
                    y,
                    z,
                    heading,
                    slope: exit_slope,
                    bank: exit_bank,
                    horizontal_curvature: HorizontalCurvature::Straight,
                    vertical_curvature: VerticalCurvature::Straight,
                };
                let distance = (exit.position() - entry.position()).length();
                let scale = if heading == North {
                    distance * 1.1
                } else {
                    distance
                };
                let (kind, inversion) = match id {
                    Lift => (PieceKind::Lift, None),
                    Brakes => (PieceKind::Brakes, None),
                    Station => (PieceKind::Station, None),
                    Loop => (PieceKind::Track, Some(Inversion::VerticalLoop)),
                    Corkscrew => (PieceKind::Track, Some(Inversion::Corkscrew)),
                    HeartlineRoll => (PieceKind::Track, Some(Inversion::HeartlineRoll)),
                    _ => (PieceKind::Track, None),
                };
                let mut piece = Piece {
                    id,
                    name,
                    kind,
                    inversion,
                    structure_material: None,
                    entry,
                    exit,
                    shape,
                    tangent_scale: scale,
                    mirrored: false,
                    arc_lengths: Vec::new(),
                };
                piece.rebuild_arc();
                piece
            },
        )
        .collect();
    for (
        base,
        entry_id,
        exit_id,
        standalone_id,
        curvature,
        entry_name,
        exit_name,
        standalone_name,
    ) in [
        (
            TurnSmall,
            TurnSmallEntry,
            TurnSmallExit,
            TurnSmallStandalone,
            HorizontalCurvature::LeftSmall,
            "Begin small left turn",
            "End small left turn",
            "Standalone small left turn",
        ),
        (
            TurnLarge,
            TurnLargeEntry,
            TurnLargeExit,
            TurnLargeStandalone,
            HorizontalCurvature::LeftLarge,
            "Begin large left turn",
            "End large left turn",
            "Standalone large left turn",
        ),
        (
            BankedTurn,
            BankedTurnEntry,
            BankedTurnExit,
            BankedTurnStandalone,
            HorizontalCurvature::LeftSmall,
            "Begin small left-banked left turn",
            "End small left-banked left turn",
            "Standalone small left-banked left turn",
        ),
        (
            LargeLeftBankedTurn,
            LargeLeftBankedTurnEntry,
            LargeLeftBankedTurnExit,
            LargeLeftBankedTurnStandalone,
            HorizontalCurvature::LeftLarge,
            "Begin large left-banked left turn",
            "End large left-banked left turn",
            "Standalone large left-banked left turn",
        ),
    ] {
        let index = pieces.iter().position(|piece| piece.id == base).unwrap();
        let mut standalone = pieces[index].clone();
        standalone.id = standalone_id;
        standalone.name = standalone_name;
        pieces.push(standalone);
        let mut entry = pieces[index].clone();
        entry.id = entry_id;
        entry.name = entry_name;
        entry.exit.horizontal_curvature = curvature;
        entry.rebuild_arc();
        let mut exit = pieces[index].clone();
        exit.id = exit_id;
        exit.name = exit_name;
        exit.entry.horizontal_curvature = curvature;
        exit.rebuild_arc();
        pieces[index].entry.horizontal_curvature = curvature;
        pieces[index].exit.horizontal_curvature = curvature;
        pieces[index].tangent_scale = match curvature {
            HorizontalCurvature::LeftSmall => 16.0 * std::f64::consts::FRAC_PI_2,
            HorizontalCurvature::LeftLarge => 32.0 * std::f64::consts::FRAC_PI_2,
            HorizontalCurvature::Straight
            | HorizontalCurvature::RightSmall
            | HorizontalCurvature::RightLarge => unreachable!(),
        };
        pieces[index].rebuild_arc();
        pieces.push(entry);
        pieces.push(exit);
    }
    for (base, id, name, entry_curvature, exit_curvature) in [
        (
            LevelToGentleUp,
            VerticalUpEntry,
            "Begin upward curve",
            VerticalCurvature::Straight,
            VerticalCurvature::Up,
        ),
        (
            GentleToSteepUp,
            VerticalUpContinue,
            "Continue upward curve",
            VerticalCurvature::Up,
            VerticalCurvature::Up,
        ),
        (
            GentleToSteepUp,
            VerticalUpExit,
            "End upward curve",
            VerticalCurvature::Up,
            VerticalCurvature::Straight,
        ),
        (
            LevelToGentleDown,
            VerticalDownEntry,
            "Begin downward curve",
            VerticalCurvature::Straight,
            VerticalCurvature::Down,
        ),
        (
            GentleToSteepDown,
            VerticalDownContinue,
            "Continue downward curve",
            VerticalCurvature::Down,
            VerticalCurvature::Down,
        ),
        (
            GentleToSteepDown,
            VerticalDownExit,
            "End downward curve",
            VerticalCurvature::Down,
            VerticalCurvature::Straight,
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.entry.vertical_curvature = entry_curvature;
        piece.exit.vertical_curvature = exit_curvature;
        match id {
            VerticalUpContinue => {
                piece.exit.slope = UpModerate;
                piece.exit.z = 5;
            }
            VerticalUpExit => {
                piece.entry.slope = UpModerate;
                piece.exit.z = 7;
            }
            VerticalDownContinue => {
                piece.exit.slope = DownModerate;
                piece.exit.z = -5;
            }
            VerticalDownExit => {
                piece.entry.slope = DownModerate;
                piece.exit.z = -7;
            }
            _ => {}
        }
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (base, id, name) in [
        (Straight, LiftFlat, "Flat chain lift"),
        (
            LevelToGentleUp,
            LiftLevelToGentleUp,
            "Begin chain lift climb",
        ),
        (GentleUpToLevel, LiftGentleUpToLevel, "End chain lift climb"),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.kind = PieceKind::Lift;
        pieces.push(piece);
    }
    for (id, name, half_tiles, height, kind) in [
        (LaunchShort, "Short launch", 4, 0, PieceKind::Launch),
        (Launch, "Launch", 8, 0, PieceKind::Launch),
        (LaunchLong, "Long launch", 16, 0, PieceKind::Launch),
        (TerrainRise, "Shallow rise", 8, 1, PieceKind::Track),
        (TerrainFall, "Shallow descent", 8, -1, PieceKind::Track),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == Straight)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.kind = kind;
        piece.exit.x = half_tiles;
        piece.exit.z = height;
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (base, id, name) in [
        (Loop, SteelLoop, "Steel-structure loop"),
        (TurnSmallStandalone, TwisterFanLeft, "Left fan turn"),
        (TurnSmallStandalone, TightTwisterLeft, "Tight left turn"),
        (
            BankedTurnStandalone,
            TwisterHelixLeft,
            "Descending left banked helix",
        ),
        (Straight, AirtimeHill, "Small airtime hill"),
        (
            TurnLargeStandalone,
            HybridOverbankLeft,
            "Left overbanked turn",
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        match id {
            SteelLoop => piece.structure_material = Some(Material::Steel),
            TwisterFanLeft => piece.exit.bank = Left,
            TightTwisterLeft => {
                piece.exit.x = 6;
                piece.exit.y = 6;
                piece.tangent_scale *= 0.75;
            }
            TwisterHelixLeft => piece.exit.z = -4,
            HybridOverbankLeft => piece.shape = Shape::Overbank,
            AirtimeHill => {
                piece.exit.x = 16;
                piece.tangent_scale = 32.0;
                piece.shape = Shape::AirtimeHill;
            }
            _ => unreachable!(),
        }
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (source, id, name) in [
        (
            HybridOverbankLeft,
            HybridOverbankRight,
            "Right overbanked turn",
        ),
        (TwisterFanLeft, TwisterFanRight, "Right fan turn"),
        (TightTwisterLeft, TightTwisterRight, "Tight right turn"),
        (
            TwisterHelixLeft,
            TwisterHelixRight,
            "Descending right banked helix",
        ),
        (
            TurnSmallEntry,
            TurnSmallRightEntry,
            "Begin small right turn",
        ),
        (TurnSmall, TurnSmallRight, "Small right turn"),
        (TurnSmallExit, TurnSmallRightExit, "End small right turn"),
        (
            TurnSmallStandalone,
            TurnSmallRightStandalone,
            "Standalone small right turn",
        ),
        (
            TurnLargeEntry,
            TurnLargeRightEntry,
            "Begin large right turn",
        ),
        (TurnLarge, TurnLargeRight, "Large right turn"),
        (TurnLargeExit, TurnLargeRightExit, "End large right turn"),
        (
            TurnLargeStandalone,
            TurnLargeRightStandalone,
            "Standalone large right turn",
        ),
        (
            BankedTurnEntry,
            SmallRightBankedTurnEntry,
            "Begin small right-banked right turn",
        ),
        (
            BankedTurn,
            SmallRightBankedTurn,
            "Small right-banked right turn",
        ),
        (
            BankedTurnExit,
            SmallRightBankedTurnExit,
            "End small right-banked right turn",
        ),
        (
            BankedTurnStandalone,
            SmallRightBankedTurnStandalone,
            "Standalone small right-banked right turn",
        ),
        (
            LargeLeftBankedTurnEntry,
            LargeRightBankedTurnEntry,
            "Begin large right-banked right turn",
        ),
        (
            LargeLeftBankedTurn,
            LargeRightBankedTurn,
            "Large right-banked right turn",
        ),
        (
            LargeLeftBankedTurnExit,
            LargeRightBankedTurnExit,
            "End large right-banked right turn",
        ),
        (
            LargeLeftBankedTurnStandalone,
            LargeRightBankedTurnStandalone,
            "Standalone large right-banked right turn",
        ),
        (OffsetLeft, OffsetRight, "Right half-tile offset"),
        (Corkscrew, CorkscrewRight, "Right corkscrew"),
        (HeartlineRoll, HeartlineRollRight, "Right heartline roll"),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == source)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        for state in [&mut piece.entry, &mut piece.exit] {
            state.y = -state.y;
            state.heading = Heading::from_index(-state.heading.index());
            state.bank = match state.bank {
                Bank::Left => Bank::Right,
                Bank::Right => Bank::Left,
                Bank::Level => Bank::Level,
            };
            state.horizontal_curvature = match state.horizontal_curvature {
                HorizontalCurvature::Straight => HorizontalCurvature::Straight,
                HorizontalCurvature::LeftSmall => HorizontalCurvature::RightSmall,
                HorizontalCurvature::LeftLarge => HorizontalCurvature::RightLarge,
                HorizontalCurvature::RightSmall => HorizontalCurvature::LeftSmall,
                HorizontalCurvature::RightLarge => HorizontalCurvature::LeftLarge,
            };
        }
        piece.mirrored = true;
        piece.rebuild_arc();
        pieces.push(piece);
    }
    pieces
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftType {
    Chain,
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
    pub minimum_radius_metres: f64,
    pub minimum_height_cm: u16,
    pub allowed_inversions: Vec<Inversion>,
    pub lift_types: Vec<LiftType>,
    pub launch_types: Vec<LaunchType>,
    pub support_style: SupportStyle,
    pub train: TrainConfiguration,
    pub track_material: Material,
    pub structure_material: Material,
}
impl CoasterModel {
    pub fn allows_piece(&self, piece: &Piece) -> bool {
        self.allowed_pieces.contains(&piece.id)
            && piece.inversion.is_none_or(|inversion| {
                self.allowed_inversions.contains(&inversion)
                    && (self.track_material != Material::Wood
                        || piece.structure_material.unwrap_or(self.structure_material)
                            == Material::Steel)
            })
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
                Lift,
                LiftFlat,
                LiftLevelToGentleUp,
                LiftGentleUpToLevel,
                Brakes,
                Station,
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
            ]
        },
        minimum_radius_metres: 8.0,
        minimum_height_cm: 122,
        allowed_inversions: vec![Inversion::VerticalLoop],
        lift_types: vec![LiftType::Chain],
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
                Loop,
                Corkscrew,
                CorkscrewRight,
                HeartlineRoll,
                HeartlineRollRight,
                Lift,
                LiftFlat,
                LiftLevelToGentleUp,
                LiftGentleUpToLevel,
                Brakes,
                Station,
                AirtimeHill,
                HybridOverbankLeft,
                HybridOverbankRight,
            ]
        },
        minimum_radius_metres: 5.0,
        minimum_height_cm: 122,
        allowed_inversions: vec![
            Inversion::VerticalLoop,
            Inversion::Corkscrew,
            Inversion::HeartlineRoll,
        ],
        lift_types: vec![LiftType::Chain],
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
                Loop,
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
        minimum_radius_metres: 5.0,
        minimum_height_cm: 132,
        allowed_inversions: vec![
            Inversion::VerticalLoop,
            Inversion::Corkscrew,
            Inversion::HeartlineRoll,
        ],
        lift_types: vec![],
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

pub mod construction;

pub mod clearance;

pub mod proximity;
