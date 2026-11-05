use std::f64::consts::{PI, TAU};

use crate::state_hash::StateHasher;
use crate::track::curves::{HEART_HEIGHT_METRES, TrackState, Vec3};

const DIFFERENCE_STEP: f64 = 1.0e-5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Twist {
    OuterBank { peak: f64, start: f64, end: f64 },
    ZeroGRoll { ramp: f64, start: f64, end: f64 },
    Stall { ramp: f64, hold: f64, start: f64, end: f64 },
}

impl Twist {
    pub(crate) fn hash_into(self, hash: &mut StateHasher) {
        match self {
            Self::OuterBank { peak, start, end } => {
                hash.write_u8(0); hash.write_f64(peak); hash.write_f64(start); hash.write_f64(end);
            }
            Self::ZeroGRoll { ramp, start, end } => {
                hash.write_u8(1); hash.write_f64(ramp); hash.write_f64(start); hash.write_f64(end);
            }
            Self::Stall { ramp, hold, start, end } => {
                hash.write_u8(2); hash.write_f64(ramp); hash.write_f64(hold); hash.write_f64(start); hash.write_f64(end);
            }
        }
    }

    pub(crate) fn sample(self, distance: f64, mirrored: bool) -> (f64, f64, f64) {
        let (start, end) = match self {
            Self::OuterBank { start, end, .. } | Self::ZeroGRoll { start, end, .. } | Self::Stall { start, end, .. } => (start, end),
        };
        assert!(start < end);
        let u = ((distance - start) / (end - start)).clamp(0.0, 1.0);
        let (value, first, second) = match self {
            Self::OuterBank { peak, .. } => {
                let w = u * (1.0 - u);
                (peak * 64.0 * w.powi(3), peak * 192.0 * w.powi(2) * (1.0 - 2.0 * u), peak * 384.0 * w * (1.0 - 5.0 * u + 5.0 * u * u))
            }
            Self::ZeroGRoll { ramp, .. } => {
                assert!(ramp > 0.0 && ramp <= 0.5);
                let (v, d, dd) = roll_schedule(u, ramp); (TAU * v, TAU * d, TAU * dd)
            }
            Self::Stall { ramp, hold, .. } => {
                assert!(ramp > 0.0 && ramp <= 0.5 && (0.0..1.0).contains(&hold));
                let width = (1.0 - hold) / 2.0;
                if u < width {
                    let (v, d, dd) = roll_schedule(u / width, ramp);
                    (PI * v, PI * d / width, PI * dd / width.powi(2))
                } else if u > 1.0 - width {
                    let (v, d, dd) = roll_schedule((1.0 - u) / width, ramp);
                    (PI * v, -PI * d / width, PI * dd / width.powi(2))
                } else { (PI, 0.0, 0.0) }
            }
        };
        let sign = if mirrored { -1.0 } else { 1.0 };
        (sign * value, sign * first / (end - start), sign * second / (end - start).powi(2))
    }
}

#[derive(Clone, Debug)]
enum RollPath {
    Spline {
        extra_turns: Option<f64>,
        descent_bias: f64,
        descent_asymmetry: f64,
        ramp: f64,
        roll_window: f64,
    },
    GeneralHelix {
        curvature: f64,
        torsion: f64,
        ramp: f64,
        turns: f64,
        positions: Vec<Vec3>,
        tangents: Vec<Vec3>,
        unwrapped_roll: Vec<f64>,
    },
}

impl PartialEq for RollPath {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Spline {
                    extra_turns: a,
                    descent_bias: b,
                    descent_asymmetry: c,
                    ramp: d,
                    roll_window: e,
                },
                Self::Spline {
                    extra_turns: f,
                    descent_bias: g,
                    descent_asymmetry: h,
                    ramp: i,
                    roll_window: j,
                },
            ) => (a, b, c, d, e) == (f, g, h, i, j),
            (
                Self::GeneralHelix {
                    curvature: a,
                    torsion: b,
                    ramp: c,
                    turns: d,
                    ..
                },
                Self::GeneralHelix {
                    curvature: e,
                    torsion: f,
                    ramp: g,
                    turns: h,
                    ..
                },
            ) => (a, b, c, d) == (e, f, g, h),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RollGeometry {
    path: RollPath,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RollSample {
    pub(crate) position: Vec3,
    pub(crate) first: Vec3,
    pub(crate) second: Vec3,
    pub(crate) heart: Vec3,
    pub(crate) up: Vec3,
    pub(crate) roll: f64,
}

impl RollGeometry {
    pub(crate) fn spline() -> Self {
        Self {
            path: RollPath::Spline {
                extra_turns: None,
                descent_bias: 0.0,
                descent_asymmetry: 0.0,
                ramp: 0.25,
                roll_window: 0.0,
            },
        }
    }

    pub(crate) fn spline_turns(turns: f64) -> Self {
        assert!(turns.is_finite(), "roll turns must be finite");
        Self {
            path: RollPath::Spline {
                extra_turns: Some(turns),
                descent_bias: 0.0,
                descent_asymmetry: 0.0,
                ramp: 0.25,
                roll_window: 0.0,
            },
        }
    }

    pub(crate) fn spline_turns_with_asymmetric_descent(
        turns: f64,
        descent_bias: f64,
        descent_asymmetry: f64,
    ) -> Self {
        assert!(
            turns.is_finite()
                && descent_bias.is_finite()
                && descent_bias >= 0.0
                && descent_asymmetry.is_finite()
        );
        Self {
            path: RollPath::Spline {
                extra_turns: Some(turns),
                descent_bias,
                descent_asymmetry,
                ramp: 0.5,
                roll_window: 0.0,
            },
        }
    }

    pub(crate) fn half_roll() -> Self {
        let mut geometry = Self::spline();
        if let RollPath::Spline { roll_window, .. } = &mut geometry.path {
            *roll_window = 0.2;
        }
        geometry
    }

    pub(crate) fn general_helix_for_connector() -> Self {
        let (mut radius, mut beta) = (4.5914_f64, 42.530_f64.to_radians());
        let displacement = |radius: f64, beta: f64| {
            let omega = beta.sin() / radius;
            let curvature = omega * beta.sin();
            let torsion = -omega * beta.cos();
            let positions = helix_positions(curvature, torsion, 0.25, 1.0);
            let end = positions.last().unwrap();
            (end.x, end.y)
        };
        for _ in 0..10 {
            let (x, y) = displacement(radius, beta);
            let error = (x - 28.0, y - 16.0);
            if error.0.abs().max(error.1.abs()) < 1e-10 {
                break;
            }
            let step = 1e-5;
            let (rx, ry) = displacement(radius + step, beta);
            let (bx, by) = displacement(radius, beta + step);
            let (a, b, c, d) = (
                (rx - x) / step,
                (bx - x) / step,
                (ry - y) / step,
                (by - y) / step,
            );
            let determinant = a * d - b * c;
            radius -= (d * error.0 - b * error.1) / determinant;
            beta -= (a * error.1 - c * error.0) / determinant;
        }
        let (x, y) = displacement(radius, beta);
        assert!((x - 28.0).abs() < 1e-9 && (y - 16.0).abs() < 1e-9);
        let omega = beta.sin() / radius;
        let curvature = omega * beta.sin();
        let torsion = -omega * beta.cos();
        Self {
            path: RollPath::GeneralHelix {
                curvature,
                torsion,
                ramp: 0.25,
                turns: 1.0,
                positions: helix_positions(curvature, torsion, 0.25, 1.0),
                tangents: (0..=1024)
                    .map(|index| {
                        helix_frame(curvature, torsion, 0.25, 1.0, f64::from(index) / 1024.0).0
                    })
                    .collect(),
                unwrapped_roll: helix_roll_knots(curvature, torsion, 0.25, 1.0),
            },
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        match &self.path {
            RollPath::Spline {
                extra_turns,
                descent_bias,
                descent_asymmetry,
                ramp,
                roll_window,
            } => {
                hash.write_u8(0);
                hash.write_option(*extra_turns, |hash, turns| hash.write_f64(turns));
                for value in [descent_bias, descent_asymmetry, ramp, roll_window] {
                    hash.write_f64(*value);
                }
            }
            RollPath::GeneralHelix {
                curvature,
                torsion,
                ramp,
                turns,
                ..
            } => {
                hash.write_u8(1);
                for value in [curvature, torsion, ramp, turns] {
                    hash.write_f64(*value);
                }
            }
        }
    }

    pub(crate) fn sample(
        &self,
        entry: TrackState,
        exit: TrackState,
        mirrored: bool,
        tangent_scale: f64,
        t: f64,
    ) -> RollSample {
        let (heart, heart_tangent, _) =
            self.heart_kinematics(entry, exit, mirrored, tangent_scale, t);
        let roll = self.roll_angle(entry, exit, mirrored, t);
        let desired_up = self.desired_up(entry, heart_tangent, roll, mirrored, t);
        let position = heart - desired_up * HEART_HEIGHT_METRES;
        let (first, second) =
            self.rail_derivatives(entry, exit, mirrored, tangent_scale, t, position);
        let tangent = first.unit();
        let up = (desired_up - tangent * desired_up.dot(tangent)).unit();

        RollSample {
            position,
            first,
            second,
            heart,
            up,
            roll,
        }
    }

    fn rail_derivatives(
        &self,
        entry: TrackState,
        exit: TrackState,
        mirrored: bool,
        tangent_scale: f64,
        t: f64,
        position: Vec3,
    ) -> (Vec3, Vec3) {
        let step = DIFFERENCE_STEP;
        let rail = |parameter| self.rail_position(entry, exit, mirrored, tangent_scale, parameter);
        // The heart path has zero second and third derivatives at both ends,
        // as does the roll schedule, so the offset rail has zero acceleration.
        if t == 0.0 || t == 1.0 {
            let (_, first, _) = self.heart_kinematics(entry, exit, mirrored, tangent_scale, t);
            return (first, Vec3::new(0.0, 0.0, 0.0));
        }
        if t < step {
            let at_start = position;
            let one_step = rail(t + step);
            let two_steps = rail(t + 2.0 * step);
            let first = (at_start * -3.0 + one_step * 4.0 - two_steps) / (2.0 * step);
            let second = (at_start - one_step * 2.0 + two_steps) / step.powi(2);
            (first, second)
        } else if 1.0 - t < step {
            let at_end = position;
            let one_step = rail(t - step);
            let two_steps = rail(t - 2.0 * step);
            let first = (at_end * 3.0 - one_step * 4.0 + two_steps) / (2.0 * step);
            let second = (at_end - one_step * 2.0 + two_steps) / step.powi(2);
            (first, second)
        } else {
            let previous = rail(t - step);
            let current = position;
            let next = rail(t + step);
            (
                (next - previous) / (2.0 * step),
                (previous - current * 2.0 + next) / step.powi(2),
            )
        }
    }

    fn rail_position(
        &self,
        entry: TrackState,
        exit: TrackState,
        mirrored: bool,
        tangent_scale: f64,
        t: f64,
    ) -> Vec3 {
        let (heart, tangent, _) = self.heart_kinematics(entry, exit, mirrored, tangent_scale, t);
        let roll = match self.path {
            RollPath::Spline { .. } => self.roll_angle(entry, exit, mirrored, t),
            RollPath::GeneralHelix { .. } => 0.0,
        };
        let up = self.desired_up(entry, tangent, roll, mirrored, t);
        heart - up * HEART_HEIGHT_METRES
    }

    fn desired_up(
        &self,
        entry: TrackState,
        heart_tangent: Vec3,
        roll: f64,
        mirrored: bool,
        t: f64,
    ) -> Vec3 {
        match &self.path {
            RollPath::Spline { .. } => {
                let tangent = heart_tangent.unit();
                let base_up = unbanked_up(tangent);
                let left = base_up.cross(tangent).unit();
                base_up * roll.cos() + left * roll.sin()
            }
            RollPath::GeneralHelix {
                curvature,
                torsion,
                ramp,
                turns,
                ..
            } => {
                let (_, normal) = helix_frame(*curvature, *torsion, *ramp, *turns, t);
                transform_frame(entry, normal, mirrored)
            }
        }
    }

    fn heart_kinematics(
        &self,
        entry: TrackState,
        exit: TrackState,
        mirrored: bool,
        tangent_scale: f64,
        t: f64,
    ) -> (Vec3, Vec3, Vec3) {
        let (entry_up, exit_up) = (state_up(entry), state_up(exit));
        let start = entry.position() + entry_up * HEART_HEIGHT_METRES;
        let end = exit.position() + exit_up * HEART_HEIGHT_METRES;
        match &self.path {
            RollPath::Spline {
                descent_bias,
                descent_asymmetry,
                ..
            } => {
                let (position, first, second) = spline_kinematics(
                    start,
                    end,
                    entry.tangent() * tangent_scale,
                    exit.tangent() * tangent_scale,
                    t,
                );
                let u = 1.0 - t;
                let offset = -256.0 * *descent_bias;
                let vertical = Vec3::new(0.0, 0.0, offset);
                let base = t.powi(4) * u.powi(4);
                let base_first = 4.0 * t.powi(3) * u.powi(3) * (1.0 - 2.0 * t);
                let base_second = 12.0 * t.powi(2) * u.powi(2) * (1.0 - 2.0 * t).powi(2)
                    - 8.0 * t.powi(3) * u.powi(3);
                let asymmetry = 1.0 + *descent_asymmetry * (t - 0.5);
                (
                    position + vertical * (base * asymmetry),
                    first + vertical * (base_first * asymmetry + base * *descent_asymmetry),
                    second
                        + vertical
                            * (base_second * asymmetry + 2.0 * base_first * *descent_asymmetry),
                )
            }
            RollPath::GeneralHelix {
                curvature,
                torsion,
                ramp,
                turns,
                positions,
                tangents,
                ..
            } => {
                let omega = curvature.hypot(*torsion);
                let scale = (end - start).length() / positions.last().unwrap().length();
                let length = TAU * turns / omega * scale;
                let scaled = t.clamp(0.0, 1.0) * 1024.0;
                let index = (scaled.floor() as usize).min(1023);
                let u = scaled - index as f64;
                let tangent0 = tangents[index];
                let tangent1 = tangents[index + 1];
                let h = 1.0 / 1024.0;
                let position = positions[index] * scale * (2.0 * u.powi(3) - 3.0 * u.powi(2) + 1.0)
                    + tangent0 * (length * h * (u.powi(3) - 2.0 * u.powi(2) + u))
                    + positions[index + 1] * scale * (-2.0 * u.powi(3) + 3.0 * u.powi(2))
                    + tangent1 * (length * h * (u.powi(3) - u.powi(2)));
                let (tangent, normal) = helix_frame(*curvature, *torsion, *ramp, *turns, t);
                let (_, rate, _) = roll_schedule(t, *ramp);
                (
                    start + transform_frame(entry, position, mirrored),
                    transform_frame(entry, tangent * length, mirrored),
                    transform_frame(
                        entry,
                        normal * (length * length * curvature * rate / scale),
                        mirrored,
                    ),
                )
            }
        }
    }

    fn roll_angle(&self, entry: TrackState, exit: TrackState, mirrored: bool, t: f64) -> f64 {
        let start = entry.bank.radians();
        let direction = if mirrored { -1.0 } else { 1.0 };
        match &self.path {
            RollPath::Spline {
                extra_turns,
                ramp,
                roll_window,
                ..
            } => {
                let end = if let Some(turns) = extra_turns {
                    exit.bank.radians() + direction * turns * TAU
                } else {
                    let mut difference = exit.bank.radians() - start;
                    if difference.abs() > PI || (difference.abs() == PI && mirrored) {
                        difference = -PI.copysign(difference);
                    }
                    start + difference
                };
                let parameter = ((t - roll_window) / (1.0 - 2.0 * roll_window)).clamp(0.0, 1.0);
                start + (end - start) * roll_schedule(parameter, *ramp).0
            }
            RollPath::GeneralHelix {
                curvature,
                torsion,
                ramp,
                turns,
                unwrapped_roll,
                ..
            } => {
                let (tangent, normal) = helix_frame(*curvature, *torsion, *ramp, *turns, t);
                let tangent = transform_frame(entry, tangent, mirrored);
                let normal = transform_frame(entry, normal, mirrored);
                let base = unbanked_up(tangent);
                let angle = normal.dot(base.cross(tangent)).atan2(normal.dot(base));
                let index = ((t.clamp(0.0, 1.0) * 1024.0).floor() as usize).min(1024);
                let reference = start + direction * unwrapped_roll[index];
                angle + TAU * ((reference - angle) / TAU).round()
            }
        }
    }
}

fn spline_kinematics(
    start: Vec3,
    end: Vec3,
    start_tangent: Vec3,
    end_tangent: Vec3,
    t: f64,
) -> (Vec3, Vec3, Vec3) {
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    let t6 = t5 * t;
    let t7 = t6 * t;
    let h00 = 1.0 - 35.0 * t4 + 84.0 * t5 - 70.0 * t6 + 20.0 * t7;
    let h10 = t - 20.0 * t4 + 45.0 * t5 - 36.0 * t6 + 10.0 * t7;
    let h01 = 1.0 - h00;
    let h11 = -15.0 * t4 + 39.0 * t5 - 34.0 * t6 + 10.0 * t7;
    let dh00 = -140.0 * t3 + 420.0 * t4 - 420.0 * t5 + 140.0 * t6;
    let dh10 = 1.0 - 80.0 * t3 + 225.0 * t4 - 216.0 * t5 + 70.0 * t6;
    let dh01 = -dh00;
    let dh11 = -60.0 * t3 + 195.0 * t4 - 204.0 * t5 + 70.0 * t6;
    let ddh00 = -420.0 * t2 + 1680.0 * t3 - 2100.0 * t4 + 840.0 * t5;
    let ddh10 = -240.0 * t2 + 900.0 * t3 - 1080.0 * t4 + 420.0 * t5;
    let ddh01 = -ddh00;
    let ddh11 = -180.0 * t2 + 780.0 * t3 - 1020.0 * t4 + 420.0 * t5;

    (
        start * h00 + start_tangent * h10 + end * h01 + end_tangent * h11,
        start * dh00 + start_tangent * dh10 + end * dh01 + end_tangent * dh11,
        start * ddh00 + start_tangent * ddh10 + end * ddh01 + end_tangent * ddh11,
    )
}

fn roll_schedule(t: f64, ramp: f64) -> (f64, f64, f64) {
    let peak = 1.0 / (1.0 - ramp);
    if t > 1.0 - ramp {
        let (value, first, second) = roll_schedule(1.0 - t, ramp);
        return (1.0 - value, first, -second);
    }
    if t >= ramp {
        return (peak * (t - ramp / 2.0), peak, 0.0);
    }
    let x = t / ramp;
    let x2 = x * x;
    let x3 = x2 * x;
    let x4 = x3 * x;
    let x5 = x4 * x;
    let x6 = x5 * x;
    (
        peak * ramp * (2.5 * x4 - 3.0 * x5 + x6),
        peak * (10.0 * x3 - 15.0 * x4 + 6.0 * x5),
        peak / ramp * (30.0 * x2 - 60.0 * x3 + 30.0 * x4),
    )
}

fn helix_frame(curvature: f64, torsion: f64, ramp: f64, turns: f64, t: f64) -> (Vec3, Vec3) {
    let omega = curvature.hypot(torsion);
    let axis = Vec3::new(torsion / omega, -curvature / omega, 0.0);
    let angle = TAU * turns * roll_schedule(t, ramp).0;
    let rotate = |vector: Vec3| {
        vector * angle.cos()
            + axis.cross(vector) * angle.sin()
            + axis * (axis.dot(vector) * (1.0 - angle.cos()))
    };
    (
        rotate(Vec3::new(1.0, 0.0, 0.0)),
        rotate(Vec3::new(0.0, 0.0, 1.0)),
    )
}

fn helix_roll_knots(curvature: f64, torsion: f64, ramp: f64, turns: f64) -> Vec<f64> {
    let mut knots = Vec::with_capacity(1025);
    for index in 0..=1024 {
        let (tangent, normal) =
            helix_frame(curvature, torsion, ramp, turns, f64::from(index) / 1024.0);
        let base = unbanked_up(tangent);
        let angle = normal.dot(base.cross(tangent)).atan2(normal.dot(base));
        let reference = knots.last().copied().unwrap_or(0.0);
        knots.push(angle + TAU * ((reference - angle) / TAU).round());
    }
    knots
}

fn helix_positions(curvature: f64, torsion: f64, ramp: f64, turns: f64) -> Vec<Vec3> {
    let length = TAU * turns / curvature.hypot(torsion);
    let mut positions = Vec::with_capacity(1025);
    positions.push(Vec3::new(0.0, 0.0, 0.0));
    for index in 0..1024 {
        let t = index as f64 / 1024.0;
        let h = 1.0 / 1024.0;
        let tangent = |t| helix_frame(curvature, torsion, ramp, turns, t).0;
        positions.push(
            positions[index]
                + (tangent(t) + tangent(t + h / 2.0) * 4.0 + tangent(t + h)) * (length * h / 6.0),
        );
    }
    positions
}

fn transform_frame(entry: TrackState, vector: Vec3, mirrored: bool) -> Vec3 {
    let forward = entry.tangent().unit();
    let up = bank_up(entry);
    let side = up.cross(forward).unit();
    forward * vector.x + side * (vector.y * if mirrored { -1.0 } else { 1.0 }) + up * vector.z
}

fn unbanked_up(tangent: Vec3) -> Vec3 {
    let horizontal_length = (tangent.x * tangent.x + tangent.y * tangent.y).sqrt();
    Vec3::new(
        -tangent.x * tangent.z / horizontal_length,
        -tangent.y * tangent.z / horizontal_length,
        horizontal_length,
    )
}

fn bank_up(state: TrackState) -> Vec3 {
    let tangent = state.tangent().unit();
    let base_up = unbanked_up(tangent);
    let left = base_up.cross(tangent).unit();
    let bank = state.bank.radians();
    base_up * bank.cos() + left * bank.sin()
}

fn state_up(state: TrackState) -> Vec3 {
    bank_up(state)
}

#[cfg(test)]
mod tests {

    #[test]
    fn hill_twist_derivatives_match_finite_differences() {
        use super::Twist;
        for twist in [
            Twist::OuterBank { peak: 110_f64.to_radians(), start: 2.0, end: 18.0 },
            Twist::ZeroGRoll { ramp: 0.25, start: 2.0, end: 18.0 },
            Twist::Stall { ramp: 0.25, hold: 0.25, start: 2.0, end: 18.0 },
        ] {
            let step = 1e-4;
            for index in 1..100 {
                let distance = 2.0 + f64::from(index) * 0.16;
                let before = twist.sample(distance - step, false);
                let center = twist.sample(distance, false);
                let after = twist.sample(distance + step, false);
                assert!(((after.0 - before.0) / (2.0 * step) - center.1).abs() < 1e-7);
                assert!(((after.1 - before.1) / (2.0 * step) - center.2).abs() < 1e-5);
                let mirror = twist.sample(distance, true);
                assert_eq!(mirror, (-center.0, -center.1, -center.2));
            }
        }
    }

    #[test]
    fn barrel_descent_bias_keeps_the_rail_descending() {
        let mut entry = TrackState::origin();
        entry.bank = Bank::Left;
        let mut exit = entry;
        exit.x = 17;
        exit.z = -4;
        exit.slope = Slope::DownGentle;
        exit.bank = Bank::Level;
        let tangent_scale = (exit.position() - entry.position()).length();
        let maximum_upward_tangent = |bias| {
            let geometry = RollGeometry::spline_turns_with_asymmetric_descent(1.0, bias, 0.0);
            (0..=2000)
                .map(|index| {
                    geometry
                        .sample(entry, exit, false, tangent_scale, f64::from(index) / 2000.0)
                        .first
                        .unit()
                        .z
                })
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let mut low = 0.0;
        let mut high = 2.0;
        for _ in 0..14 {
            let middle = (low + high) / 2.0;
            if maximum_upward_tangent(middle) <= 1e-8 {
                high = middle;
            } else {
                low = middle;
            }
        }
        assert!(
            high <= 1.77,
            "minimum passing bias is approximately {high} metres"
        );
    }

    #[test]
    fn asymmetric_barrel_heart_paths_have_feasible_single_car_candidates() {
        let mut entry = TrackState::origin();
        entry.bank = Bank::Left;
        let mut exit = entry;
        exit.x = 16;
        exit.z = -4;
        exit.slope = Slope::DownGentle;
        exit.bank = Bank::Level;
        let scale = (exit.position() - entry.position()).length();
        let mut candidates = Vec::new();
        for asymmetry in [-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5] {
            let make = |bias| RollGeometry {
                path: RollPath::Spline {
                    extra_turns: Some(1.0),
                    descent_bias: bias,
                    descent_asymmetry: asymmetry,
                    ramp: 0.5,
                    roll_window: 0.0,
                },
            };
            let upward = |geometry: &RollGeometry| {
                (0..=1000)
                    .map(|i| {
                        geometry
                            .sample(entry, exit, false, scale, f64::from(i) / 1000.0)
                            .first
                            .unit()
                            .z
                    })
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let (mut low, mut high) = (0.0, 4.0);
            if upward(&make(high)) > 1e-8 {
                continue;
            }
            for _ in 0..13 {
                let middle = (low + high) / 2.0;
                if upward(&make(middle)) <= 1e-8 {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            let geometry = make(high + 0.01);
            let mut peak: f64 = 0.0;
            for i in 1..1000 {
                let t = f64::from(i) / 1000.0;
                let sample = geometry.sample(entry, exit, false, scale, t);
                let (_, heart_first, heart_second) =
                    geometry.heart_kinematics(entry, exit, false, scale, t);
                let speed = sample.first.length();
                let heart_curve = heart_second / speed.powi(2)
                    - heart_first * (sample.first.dot(sample.second) / speed.powi(4));
                let tangent = sample.first.unit();
                let velocity_squared = 225.0 - 19.62 * sample.position.z;
                let force = heart_curve * velocity_squared
                    + heart_first / speed * (-9.81 * tangent.z)
                    + Vec3::new(0.0, 0.0, 9.81);
                peak = peak.max((force.dot(tangent) / 9.81).abs());
            }
            candidates.push((asymmetry, high, peak));
        }
        eprintln!("asymmetry, minimum bias, single-car peak longitudinal: {candidates:?}");
        assert!(candidates.iter().any(|candidate| candidate.2 < 1.5));
    }

    use crate::track::catalog;
    use crate::track::curves::{Bank, Heading, PieceId, Slope, TrackState, Vec3};

    use super::{RollGeometry, RollPath};

    const GRAVITY: f64 = 9.81;

    #[test]
    fn rail_position_matches_heart_offset_for_both_roll_paths() {
        let entry = TrackState::origin();
        let exit = TrackState {
            x: 14,
            y: 8,
            ..entry
        };
        let scale = (exit.position() - entry.position()).length();
        for geometry in [
            RollGeometry::spline_turns(1.0),
            RollGeometry::general_helix_for_connector(),
        ] {
            for mirrored in [false, true] {
                for index in 0..=1000 {
                    let t = f64::from(index) / 1000.0;
                    let heart = geometry.heart_kinematics(entry, exit, mirrored, scale, t).0;
                    let tangent = geometry.heart_kinematics(entry, exit, mirrored, scale, t).1;
                    let roll = geometry.roll_angle(entry, exit, mirrored, t);
                    let up = geometry.desired_up(entry, tangent, roll, mirrored, t);
                    let expected = heart - up * super::HEART_HEIGHT_METRES;
                    assert_eq!(
                        geometry.rail_position(entry, exit, mirrored, scale, t),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn classic_corkscrew_lands_at_the_sideways_connector() {
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .unwrap();
        assert_eq!((piece.exit.x, piece.exit.y, piece.exit.z), (14, 8, 0));
        assert!((piece.sample(piece.length()).position - piece.exit.position()).length() < 1e-7);
    }

    #[test]
    fn spline_heartline_keeps_rail_endpoints_on_the_connectors() {
        let entry = crate::track::curves::TrackState::origin();
        let exit = crate::track::curves::TrackState { x: 16, ..entry };
        let geometry = RollGeometry::spline_turns(1.0);

        let start = geometry.sample(entry, exit, false, 32.0, 0.0);
        let middle = geometry.sample(entry, exit, false, 32.0, 0.5);
        let end = geometry.sample(entry, exit, false, 32.0, 1.0);

        assert!((start.position - entry.position()).length() < 1.0e-8);
        assert!((end.position - exit.position()).length() < 1.0e-8);
        assert!((start.heart - (entry.position() + Vec3::new(0.0, 0.0, 1.2))).length() < 1.0e-8);
        assert!((end.heart - (exit.position() + Vec3::new(0.0, 0.0, 1.2))).length() < 1.0e-8);
        assert!((middle.heart - Vec3::new(16.0, 0.0, 1.2)).length() < 1.0e-8);
        assert!((middle.roll - std::f64::consts::PI).abs() < 1.0e-8);
        assert!((middle.up.length() - 1.0).abs() < 1.0e-8);
    }

    #[test]
    fn catalog_heartline_roll_limits_heart_acceleration_and_roll_rate() {
        let piece = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::HeartlineRoll)
            .unwrap();
        let speed = piece.rated_speeds.unwrap().maximum_entry;
        let step = piece.length() / 4000.0;
        let mut peak_acceleration: f64 = 0.0;
        let mut peak_along_axis: f64 = 0.0;
        let mut peak_roll_rate: f64 = 0.0;
        for index in 2..3999 {
            let distance = step * f64::from(index);
            let previous = piece.sample(distance - step);
            let sample = piece.sample(distance);
            let next = piece.sample(distance + step);
            let acceleration =
                (previous.heart - sample.heart * 2.0 + next.heart) / step.powi(2) * speed.powi(2);
            peak_acceleration = peak_acceleration
                .max((acceleration - Vec3::new(acceleration.x, 0.0, 0.0)).length() / GRAVITY);
            peak_along_axis = peak_along_axis.max(acceleration.x.abs() / GRAVITY);
            peak_roll_rate =
                peak_roll_rate.max((next.roll - previous.roll).abs() / (2.0 * step) * speed);
        }
        eprintln!(
            "heartline perpendicular acceleration {peak_acceleration:.4} g, along-axis {peak_along_axis:.4} g, roll rate {:.2} degrees/s",
            peak_roll_rate.to_degrees()
        );
        assert!(peak_acceleration <= 0.01, "{peak_acceleration}");
        assert!(
            peak_roll_rate.to_degrees() <= 200.0,
            "{}",
            peak_roll_rate.to_degrees()
        );
    }

    #[test]
    fn general_helix_roll_is_continuous_in_every_heading() {
        let mut maximum_endpoint_error = 0.0_f64;
        for id in [PieceId::Corkscrew, PieceId::CorkscrewRight] {
            let template = catalog().iter().find(|piece| piece.id == id).unwrap();
            for heading_index in 0..8 {
                let mut entry = TrackState::origin();
                entry.heading = Heading::from_index(heading_index);
                let piece = template.at(entry).unwrap();
                let mut previous = piece.sample(0.0).roll;
                for index in 1..=8192 {
                    let sample = piece.sample(piece.length() * f64::from(index) / 8192.0);
                    assert!(
                        (sample.roll - previous).abs() < 0.02,
                        "{id:?} {heading_index} {index}"
                    );
                    previous = sample.roll;
                }
                assert!((previous.abs() - std::f64::consts::TAU).abs() < 1e-8);
                let error =
                    (piece.sample(piece.length()).position - piece.exit.position()).length();
                maximum_endpoint_error = maximum_endpoint_error.max(error);
            }
        }
        eprintln!("maximum connector error: {maximum_endpoint_error:.3e} m");
        assert!(maximum_endpoint_error < 1e-9);
    }

    #[test]
    fn general_helix_frame_and_diagonal_placement() {
        let pieces = catalog();
        let left = pieces
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .unwrap();
        let right = pieces
            .iter()
            .find(|piece| piece.id == PieceId::CorkscrewRight)
            .unwrap();
        for piece in [left, right] {
            for heading in [Heading::East, Heading::NorthEast] {
                let mut entry = TrackState::origin();
                entry.heading = heading;
                let placed = piece.at(entry).unwrap();
                for distance in [0.0, placed.length()] {
                    let sample = placed.sample(distance);
                    let connector = if distance == 0.0 {
                        placed.entry
                    } else {
                        placed.exit
                    };
                    assert!((sample.position - connector.position()).length() < 1e-7);
                    assert!(sample.curvature.abs() < 1e-8);
                }
                let step = 0.01;
                for index in 1..10 {
                    let distance = placed.length() * f64::from(index) / 10.0;
                    let before = placed.sample(distance - step);
                    let sample = placed.sample(distance);
                    let after = placed.sample(distance + step);
                    let heart_tangent = (after.heart - before.heart).unit();
                    let acceleration = before.heart - sample.heart * 2.0 + after.heart;
                    let normal =
                        (acceleration - heart_tangent * acceleration.dot(heart_tangent)).unit();
                    assert!(
                        normal.dot(sample.up) > 0.999,
                        "{index}: {:?}",
                        normal.dot(sample.up)
                    );
                }
            }
        }
        for index in 0..=10 {
            let fraction = f64::from(index) / 10.0;
            let a = left.sample(left.length() * fraction);
            let b = right.sample(right.length() * fraction);
            assert!(
                (a.position - Vec3::new(b.position.x, -b.position.y, b.position.z)).length() < 1e-7
            );
            assert!((a.roll + b.roll).abs() < 1e-7);
        }
        eprintln!(
            "corkscrew length {:.3} m, minimum radius {:.3} m, midpoint height {:.3} m",
            left.length(),
            1.0 / left.maximum_curvature(),
            left.sample(left.length() / 2.0).position.z
        );
    }

    #[test]
    fn half_roll_and_barrel_roll_rates_at_maximum_speed() {
        for id in [
            PieceId::HalfCorkscrewLeftUp,
            PieceId::HalfCorkscrewLeftDown,
            PieceId::BarrelDropLeft,
        ] {
            let piece = catalog().iter().find(|piece| piece.id == id).unwrap();
            let speed = piece.rated_speeds.unwrap().maximum_entry;
            let step = piece.length() / 4000.0;
            let mut peak = 0.0_f64;
            for index in 1..4000 {
                let distance = f64::from(index) * step;
                let before = piece.sample(distance - step);
                let after = piece.sample(distance + step);
                peak = peak.max((after.roll - before.roll).abs() * speed / (2.0 * step));
            }
            eprintln!("{id:?} peak roll rate {:.2} degrees/s", peak.to_degrees());
            if id != PieceId::BarrelDropLeft {
                assert!(peak.to_degrees() <= 200.0);
            }
        }
    }
}
