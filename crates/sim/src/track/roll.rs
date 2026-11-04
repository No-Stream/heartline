use std::f64::consts::{PI, TAU};

use crate::state_hash::StateHasher;
use crate::track::curves::{HEART_HEIGHT_METRES, TrackState, Vec3};

const DIFFERENCE_STEP: f64 = 1.0e-5;

#[derive(Clone, Debug, PartialEq)]
enum RollPath {
    Spline {
        extra_turns: Option<f64>,
        descent_bias: f64,
        descent_asymmetry: f64,
    },
    Helix {
        axial_length: f64,
        total_roll: f64,
        radius: f64,
    },
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
            },
        }
    }

    pub(crate) fn helix(axial_length: f64, total_roll: f64, radius: f64) -> Self {
        assert!(axial_length > 0.0, "helix length must be positive");
        assert!(total_roll.is_finite(), "helix roll must be finite");
        assert!(radius > 0.0, "helix radius must be positive");
        Self {
            path: RollPath::Helix {
                axial_length,
                total_roll,
                radius,
            },
        }
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        match self.path {
            RollPath::Spline {
                extra_turns,
                descent_bias,
                descent_asymmetry,
            } => {
                hash.write_u8(0);
                hash.write_option(extra_turns, |hash, turns| hash.write_f64(turns));
                hash.write_f64(descent_bias);
                hash.write_f64(descent_asymmetry);
            }
            RollPath::Helix {
                axial_length,
                total_roll,
                radius,
            } => {
                hash.write_u8(1);
                hash.write_f64(axial_length);
                hash.write_f64(total_roll);
                hash.write_f64(radius);
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
        let heart = self.heart_position(entry, exit, mirrored, tangent_scale, t);
        let roll = self.roll_angle(entry, exit, mirrored, t);
        let heart_tangent = self
            .heart_kinematics(entry, exit, mirrored, tangent_scale, t)
            .1;
        let desired_up = self.desired_up(entry, heart_tangent, roll);
        let position = heart - desired_up * HEART_HEIGHT_METRES;
        let (first, second) = self.rail_derivatives(entry, exit, mirrored, tangent_scale, t);
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
            let at_start = rail(t);
            let one_step = rail(t + step);
            let two_steps = rail(t + 2.0 * step);
            let first = (at_start * -3.0 + one_step * 4.0 - two_steps) / (2.0 * step);
            let second = (at_start - one_step * 2.0 + two_steps) / step.powi(2);
            (first, second)
        } else if 1.0 - t < step {
            let at_end = rail(t);
            let one_step = rail(t - step);
            let two_steps = rail(t - 2.0 * step);
            let first = (at_end * 3.0 - one_step * 4.0 + two_steps) / (2.0 * step);
            let second = (at_end - one_step * 2.0 + two_steps) / step.powi(2);
            (first, second)
        } else {
            let previous = rail(t - step);
            let current = rail(t);
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
        let heart = self.heart_position(entry, exit, mirrored, tangent_scale, t);
        let tangent = self
            .heart_kinematics(entry, exit, mirrored, tangent_scale, t)
            .1;
        let roll = self.roll_angle(entry, exit, mirrored, t);
        let up = self.desired_up(entry, tangent, roll);
        heart - up * HEART_HEIGHT_METRES
    }

    fn desired_up(&self, entry: TrackState, heart_tangent: Vec3, roll: f64) -> Vec3 {
        match self.path {
            RollPath::Spline { .. } => {
                let tangent = heart_tangent.unit();
                let base_up = unbanked_up(tangent);
                let left = base_up.cross(tangent).unit();
                base_up * roll.cos() + left * roll.sin()
            }
            RollPath::Helix { .. } => {
                let phase = roll - entry.bank.radians();
                let up = bank_up(entry);
                let side = up.cross(horizontal_forward(entry)).unit();
                up * phase.cos() - side * phase.sin()
            }
        }
    }

    fn heart_position(
        &self,
        entry: TrackState,
        exit: TrackState,
        mirrored: bool,
        tangent_scale: f64,
        t: f64,
    ) -> Vec3 {
        self.heart_kinematics(entry, exit, mirrored, tangent_scale, t)
            .0
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
        match self.path {
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
                let offset = -256.0 * descent_bias;
                let vertical = Vec3::new(0.0, 0.0, offset);
                let base = t.powi(4) * u.powi(4);
                let base_first = 4.0 * t.powi(3) * u.powi(3) * (1.0 - 2.0 * t);
                let base_second = 12.0 * t.powi(2) * u.powi(2) * (1.0 - 2.0 * t).powi(2)
                    - 8.0 * t.powi(3) * u.powi(3);
                let asymmetry = 1.0 + descent_asymmetry * (t - 0.5);
                (
                    position + vertical * (base * asymmetry),
                    first + vertical * (base_first * asymmetry + base * descent_asymmetry),
                    second
                        + vertical
                            * (base_second * asymmetry + 2.0 * base_first * descent_asymmetry),
                )
            }
            RollPath::Helix {
                axial_length: _,
                total_roll,
                radius,
            } => {
                let forward = horizontal_forward(entry);
                let axial_length = (end - start).dot(forward);
                let banked_up = bank_up(entry);
                let side = banked_up.cross(forward).unit();
                let (smooth, smooth_first, smooth_second) = smooth7(t);
                let direction = if mirrored { -1.0 } else { 1.0 };
                let angle = total_roll * direction * smooth;
                let angle_first = total_roll * direction * smooth_first;
                let angle_second = total_roll * direction * smooth_second;
                let endpoint_radius = if (total_roll.abs() - PI).abs() < 1.0e-8 {
                    (end - start).dot(banked_up) / 2.0
                } else {
                    radius
                };
                let position = start
                    + forward * (axial_length * t)
                    + side * (endpoint_radius * angle.sin())
                    + banked_up * (endpoint_radius * (1.0 - angle.cos()));
                let first = forward * axial_length
                    + side * (endpoint_radius * angle.cos() * angle_first)
                    + banked_up * (endpoint_radius * angle.sin() * angle_first);
                let second = side
                    * (endpoint_radius
                        * (angle.cos() * angle_second - angle.sin() * angle_first.powi(2)))
                    + banked_up
                        * (endpoint_radius
                            * (angle.sin() * angle_second + angle.cos() * angle_first.powi(2)));
                (position, first, second)
            }
        }
    }

    fn roll_angle(&self, entry: TrackState, exit: TrackState, mirrored: bool, t: f64) -> f64 {
        let start = entry.bank.radians();
        let direction = if mirrored { -1.0 } else { 1.0 };
        let end = match self.path {
            RollPath::Spline {
                extra_turns: None, ..
            } => {
                let mut difference = exit.bank.radians() - start;
                if difference.abs() > PI || (difference.abs() == PI && mirrored) {
                    difference = -PI.copysign(difference);
                }
                start + difference
            }
            RollPath::Spline {
                extra_turns: Some(turns),
                ..
            } => exit.bank.radians() + direction * turns * TAU,
            RollPath::Helix { total_roll, .. } => start + direction * total_roll,
        };
        let (smooth, _, _) = smooth7(t);
        start + (end - start) * smooth
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

fn smooth7(t: f64) -> (f64, f64, f64) {
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    let t6 = t5 * t;
    let t7 = t6 * t;
    (
        35.0 * t4 - 84.0 * t5 + 70.0 * t6 - 20.0 * t7,
        140.0 * t3 - 420.0 * t4 + 420.0 * t5 - 140.0 * t6,
        420.0 * t2 - 1680.0 * t3 + 2100.0 * t4 - 840.0 * t5,
    )
}

fn horizontal_forward(state: TrackState) -> Vec3 {
    let tangent = state.tangent();
    Vec3::new(tangent.x, tangent.y, 0.0).unit()
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
    use std::collections::{HashSet, VecDeque};
    use std::f64::consts::TAU;
    use std::sync::Arc;

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

    use crate::physics::{Circuit, PhysicsConfig, Train};
    use crate::track::curves::{
        Bank, Heading, Heartlining, Inversion, Piece, PieceId, PieceKind, RatedSpeeds, Shape,
        Slope, TrackState, Vec3,
    };
    use crate::track::{
        CoasterModel, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
    };

    use super::{RollGeometry, RollPath};

    const GRAVITY: f64 = 9.81;

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
    fn heartline_roll_keeps_the_rider_on_a_straight_heart_path() {
        let first_passing_span = (248..=320)
            .step_by(2)
            .map(f64::from)
            .find(|span| heartline_force_peaks(*span).3 <= 0.05);
        assert_eq!(first_passing_span, Some(274.0));
    }

    #[test]
    fn catalog_heartline_roll_reads_gravity_at_twenty_five_metres_per_second() {
        let (minimum_vertical, maximum_vertical, maximum_lateral, maximum_longitudinal) =
            heartline_force_peaks(284.0);
        assert!((maximum_vertical - 1.0).abs() < 0.05, "{maximum_vertical}");
        assert!((minimum_vertical + 1.0).abs() < 0.05, "{minimum_vertical}");
        assert!(maximum_lateral <= 1.05, "{maximum_lateral}");
        assert!(maximum_longitudinal <= 0.05, "{maximum_longitudinal}");
    }

    fn heartline_force_peaks(axial_span: f64) -> (f64, f64, f64, f64) {
        let piece = roll_piece(
            PieceId::HeartlineRoll,
            (axial_span / 2.0) as i32,
            0,
            Bank::Level,
            Bank::Level,
            RollGeometry::spline_turns(1.0),
        );
        let sample_step = piece.length() / 500.0;
        let mut minimum_vertical = f64::INFINITY;
        let mut maximum_vertical = f64::NEG_INFINITY;
        let mut maximum_lateral: f64 = 0.0;
        let mut maximum_longitudinal: f64 = 0.0;
        for index in 2..499 {
            let distance = sample_step * f64::from(index);
            let sample = piece.sample(distance);
            let previous = piece.sample(distance - 0.02).heart;
            let next = piece.sample(distance + 0.02).heart;
            let acceleration =
                (previous - sample.heart * 2.0 + next) / 0.02_f64.powi(2) * 25.0_f64.powi(2);
            let force = acceleration + Vec3::new(0.0, 0.0, GRAVITY);
            minimum_vertical = minimum_vertical.min(force.dot(sample.up) / GRAVITY);
            maximum_vertical = maximum_vertical.max(force.dot(sample.up) / GRAVITY);
            maximum_lateral =
                maximum_lateral.max((force.dot(sample.up.cross(sample.tangent)) / GRAVITY).abs());
            maximum_longitudinal =
                maximum_longitudinal.max((force.dot(sample.tangent) / GRAVITY).abs());
        }

        (
            minimum_vertical,
            maximum_vertical,
            maximum_lateral,
            maximum_longitudinal,
        )
    }

    #[test]
    fn helix_endpoints_match_the_track_connectors() {
        let piece = roll_piece(
            PieceId::Corkscrew,
            16,
            0,
            Bank::Level,
            Bank::Level,
            RollGeometry::helix(32.0, std::f64::consts::TAU, 5.0),
        );

        let start = piece.sample(0.0);
        let end = piece.sample(piece.length());
        assert!((start.position - piece.entry.position()).length() < 1.0e-7);
        assert!((end.position - piece.exit.position()).length() < 1.0e-7);
        assert!((start.heart - (piece.entry.position() + start.up * 1.2)).length() < 1.0e-6);
        assert!((end.heart - (piece.exit.position() + end.up * 1.2)).length() < 1.0e-6);
    }

    #[test]
    fn half_helix_ends_on_the_inverted_connector() {
        let piece = roll_piece(
            PieceId::HalfCorkscrewLeftUp,
            16,
            4,
            Bank::Level,
            Bank::Inverted,
            RollGeometry::helix(32.0, std::f64::consts::PI, 5.0),
        );

        let start = piece.sample(0.0);
        let end = piece.sample(piece.length());
        assert!((start.position - piece.entry.position()).length() < 1.0e-7);
        assert!((end.position - piece.exit.position()).length() < 1.0e-7);
        assert!((start.up.z - 1.0).abs() < 1.0e-7);
        assert!((end.up.z + 1.0).abs() < 1.0e-7);
    }

    #[test]
    fn diagonal_helices_end_on_the_rotated_connector() {
        for (exit_z, exit_bank, total_roll) in [
            (0, Bank::Level, std::f64::consts::TAU),
            (6, Bank::Inverted, std::f64::consts::PI),
        ] {
            let template = roll_piece(
                PieceId::Corkscrew,
                20,
                exit_z,
                Bank::Level,
                exit_bank,
                RollGeometry::helix(40.0, total_roll, 5.0),
            );
            let mut entry = TrackState::origin();
            entry.heading = Heading::NorthEast;
            let placed = template.at(entry).unwrap();
            let end = placed.sample(placed.length());
            assert!(
                (end.position - placed.exit.position()).length() < 1.0e-6,
                "{:?} {:?}",
                end.position,
                placed.exit.position()
            );
        }
    }

    #[test]
    fn mirrored_helix_reverses_the_side_displacement() {
        let left = roll_piece(
            PieceId::Corkscrew,
            16,
            0,
            Bank::Level,
            Bank::Level,
            RollGeometry::helix(32.0, std::f64::consts::TAU, 5.0),
        );
        let mut right = left.clone();
        right.mirrored = true;

        let left_sample = left.sample(left.length() * 0.25);
        let right_sample = right.sample(right.length() * 0.25);
        assert!((left_sample.heart.y + right_sample.heart.y).abs() < 1.0e-6);
        assert!((left_sample.heart.z - right_sample.heart.z).abs() < 1.0e-6);
    }

    #[test]
    fn full_corkscrew_uses_the_shortest_passing_two_metre_grid_length() {
        let catalog_length = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .unwrap()
            .exit
            .x as f64
            * 2.0;
        let first_passing_length = (20..=catalog_length as i32)
            .step_by(2)
            .map(f64::from)
            .find(|length| corkscrew_passes_at_rated_endpoints(*length));
        assert_eq!(first_passing_length, Some(catalog_length));
    }

    fn corkscrew_passes_at_rated_endpoints(axial_length: f64) -> bool {
        let templates = catalog();
        let mut element_template = templates
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .expect("corkscrew catalog template")
            .clone();
        element_template.exit.x = (axial_length / 2.0) as i32;
        element_template.shape = Shape::Roll(Arc::new(RollGeometry::helix(axial_length, TAU, 5.0)));

        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in [PieceId::Station, PieceId::Straight, PieceId::Straight] {
            let template = templates
                .iter()
                .find(|piece| piece.id == id)
                .expect("prefix catalog template");
            let piece = template.at(state).expect("prefix connector");
            state = piece.exit;
            pieces.push(piece);
        }

        let element_start = pieces.iter().map(Piece::length).sum();
        let element = element_template.at(state).expect("corkscrew connector");
        state = element.exit;
        pieces.push(element);
        let element_end = pieces.iter().map(Piece::length).sum();
        for id in level_return_path(state, &templates) {
            let template = templates
                .iter()
                .find(|piece| piece.id == id)
                .expect("return-path catalog template");
            let piece = template.at(state).expect("return-path connector");
            state = piece.exit;
            pieces.push(piece);
        }
        assert_eq!(state, TrackState::origin());
        let circuit = Circuit::new(pieces).expect("closed corkscrew circuit");
        let element = circuit
            .pieces()
            .iter()
            .find(|piece| piece.id == PieceId::Corkscrew)
            .expect("placed corkscrew");
        let rated = element.rated_speeds.expect("rated corkscrew");

        [wooden_twister_model(), hybrid_model(), multi_launch_model()]
            .into_iter()
            .filter(|model| model.allows_piece(element))
            .all(|model| {
                [6, 12].into_iter().all(|cars| {
                    [rated.minimum_entry, rated.maximum_entry]
                        .into_iter()
                        .all(|speed| {
                            corkscrew_train_passes(
                                &model,
                                &circuit,
                                element_start,
                                element_end,
                                cars,
                                speed,
                            )
                        })
                })
            })
    }

    fn corkscrew_train_passes(
        model: &CoasterModel,
        circuit: &Circuit,
        element_start: f64,
        element_end: f64,
        cars: u8,
        entry_speed: f64,
    ) -> bool {
        let mut train_configuration = model.train.clone();
        train_configuration.cars = cars;
        let mut train = Train::new(
            circuit,
            &train_configuration,
            PhysicsConfig {
                rolling_resistance: 0.0,
                drag_coefficient: 0.0,
                powered_sections: false,
                ..PhysicsConfig::default()
            },
            element_start,
            entry_speed,
        );
        let mut cars_seen = vec![false; usize::from(cars)];
        let mut minimum_vertical = f64::INFINITY;
        let mut maximum_vertical = f64::NEG_INFINITY;
        let mut maximum_lateral: f64 = 0.0;
        let mut maximum_longitudinal: f64 = 0.0;

        for _ in 0..100_000 {
            train.step(circuit);
            for frame in train.take_readings() {
                for (index, reading) in frame.into_iter().enumerate() {
                    if (element_start..element_end).contains(&reading.distance) {
                        cars_seen[index] = true;
                        minimum_vertical = minimum_vertical.min(reading.g.vertical);
                        maximum_vertical = maximum_vertical.max(reading.g.vertical);
                        maximum_lateral = maximum_lateral.max(reading.g.lateral.abs());
                        maximum_longitudinal =
                            maximum_longitudinal.max(reading.g.longitudinal.abs());
                    }
                }
            }
            if train.distance() >= element_end + train_configuration.length_metres {
                break;
            }
            if train.speed() <= 0.0 || !train.notes().is_empty() {
                return false;
            }
        }

        let limits = model.comfortable_force_limits;
        cars_seen.into_iter().all(|seen| seen)
            && train.distance() >= element_end + train_configuration.length_metres
            && minimum_vertical >= limits.minimum_vertical_g
            && maximum_vertical <= limits.maximum_vertical_g
            && maximum_lateral <= limits.maximum_lateral_g
            && maximum_longitudinal <= limits.maximum_longitudinal_g
    }

    fn level_return_path(start: TrackState, templates: &[Piece]) -> Vec<PieceId> {
        let mut queue = VecDeque::from([(start, Vec::new())]);
        let mut visited = HashSet::from([connector_key(start)]);
        while let Some((state, path)) = queue.pop_front() {
            if state == TrackState::origin() {
                return path;
            }
            if path.len() == 48 {
                continue;
            }
            for id in [
                PieceId::Straight,
                PieceId::HalfTileStraight,
                PieceId::OneTileStraight,
                PieceId::TurnSmallStandalone,
                PieceId::TurnSmallRightStandalone,
                PieceId::TurnLargeStandalone,
                PieceId::TurnLargeRightStandalone,
                PieceId::OffsetLeft,
                PieceId::OffsetRight,
            ] {
                let template = templates
                    .iter()
                    .find(|piece| piece.id == id)
                    .expect("return-path catalog template");
                if !same_connector_shape(state, template.entry) {
                    continue;
                }
                let next = next_connector(template, state);
                if next.x.abs() > 300 || next.y.abs() > 300 || next.z.abs() > 300 {
                    continue;
                }
                if !visited.insert(connector_key(next)) {
                    continue;
                }
                let mut next_path = path.clone();
                next_path.push(id);
                queue.push_back((next, next_path));
            }
        }
        panic!("no bounded level return path from {start:?}");
    }

    fn same_connector_shape(first: TrackState, second: TrackState) -> bool {
        first.slope == second.slope
            && first.bank == second.bank
            && first.horizontal_curvature == second.horizontal_curvature
            && first.vertical_curvature == second.vertical_curvature
    }

    fn connector_key(state: TrackState) -> (i32, i32, i32, u8, u8, u8, u8, u8) {
        (
            state.x,
            state.y,
            state.z,
            state.heading as u8,
            state.slope as u8,
            state.bank as u8,
            state.horizontal_curvature as u8,
            state.vertical_curvature as u8,
        )
    }

    fn next_connector(piece: &Piece, state: TrackState) -> TrackState {
        let rotation = (state.heading as i32 - piece.entry.heading as i32).rem_euclid(8);
        let delta_x = piece.exit.x - piece.entry.x;
        let delta_y = piece.exit.y - piece.entry.y;
        let (rotated_x, rotated_y) = match rotation {
            0 => (delta_x, delta_y),
            1 => (delta_x - delta_y, delta_x + delta_y),
            2 => (-delta_y, delta_x),
            3 => (-delta_x - delta_y, delta_x - delta_y),
            4 => (-delta_x, -delta_y),
            5 => (delta_y - delta_x, -delta_x - delta_y),
            6 => (delta_y, -delta_x),
            7 => (delta_x + delta_y, delta_y - delta_x),
            _ => unreachable!(),
        };
        TrackState {
            x: state.x + rotated_x,
            y: state.y + rotated_y,
            z: state.z
                + if rotation % 2 == 0 {
                    piece.exit.z - piece.entry.z
                } else {
                    (f64::from(piece.exit.z - piece.entry.z) * std::f64::consts::SQRT_2).round()
                        as i32
                },
            heading: Heading::from_index(piece.exit.heading.index() + rotation),
            slope: piece.exit.slope,
            bank: piece.exit.bank,
            horizontal_curvature: piece.exit.horizontal_curvature,
            vertical_curvature: piece.exit.vertical_curvature,
        }
    }

    fn roll_piece(
        id: PieceId,
        exit_x: i32,
        exit_z: i32,
        entry_bank: Bank,
        exit_bank: Bank,
        geometry: RollGeometry,
    ) -> Piece {
        let mut entry = TrackState::origin();
        entry.heading = Heading::East;
        entry.slope = Slope::Level;
        entry.bank = entry_bank;
        let mut exit = entry;
        exit.x = exit_x;
        exit.z = exit_z;
        exit.bank = exit_bank;
        let mut piece = Piece {
            id,
            name: "roll test",
            kind: PieceKind::Track,
            inversion: Some(Inversion::Corkscrew),
            heartlining: Some(Heartlining::NotHeartlined),
            structure_material: None,
            rated_speeds: Some(RatedSpeeds {
                minimum_entry: 2.0,
                maximum_entry: 20.0,
            }),
            entry,
            exit,
            shape: Shape::Roll(std::sync::Arc::new(geometry)),
            tangent_scale: (exit.position() - entry.position()).length(),
            mirrored: false,
            arc_lengths: Vec::new(),
            arc_speeds: Vec::new(),
            maximum_curvature: 0.0,
        };
        piece.rebuild_arc();
        piece
    }
}
