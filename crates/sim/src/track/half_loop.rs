use std::f64::consts::PI;
use std::sync::OnceLock;

const GRAVITY: f64 = 9.81;
const ENTRY_SPEED: f64 = 32.0;
const HEIGHT: f64 = 32.0;
const SPAN: f64 = 32.0;
// These geometry parameters solve the 32 m rise and span at the design speed.
const DESIGN_FORCE: f64 = 4.39552384;
const ENTRY_TAPER: f64 = 0.31645005;
const EXIT_TAPER: f64 = 0.3;
const STEPS: usize = 2048;

#[derive(Clone, Copy)]
struct Point {
    distance: f64,
    x: f64,
    z: f64,
    angle: f64,
    curvature: f64,
}

// Resolve curvature from the desired rider load at the design speed. Tapering
// it at each end gives straight-curvature connectors; energy conservation
// determines the speed as the track rises.
fn integrate(force: f64, entry_taper: f64, steps: usize) -> Vec<Point> {
    let mut points = Vec::with_capacity(steps + 1);
    points.push(Point {
        distance: 0.0,
        x: 0.0,
        z: 0.0,
        angle: 0.0,
        curvature: 0.0,
    });
    for index in 0..steps {
        let previous = points[index];
        let parameter = (index as f64 + 0.5) / steps as f64;
        let angle = PI * parameter.powi(3) * (10.0 + parameter * (-15.0 + 6.0 * parameter));
        let angle_step = PI * 30.0 * parameter.powi(2) * (1.0 - parameter).powi(2) / steps as f64;
        let taper = (angle / entry_taper)
            .powf(2.0 / 3.0)
            .min(((PI - angle) / EXIT_TAPER).powf(2.0 / 3.0))
            .min(1.0);
        let speed_squared = ENTRY_SPEED.powi(2) - 2.0 * GRAVITY * previous.z;
        let curvature = GRAVITY * (force - angle.cos()) * taper / speed_squared;
        let distance = angle_step / curvature;
        let end_parameter = (index + 1) as f64 / steps as f64;
        let end_angle =
            PI * end_parameter.powi(3) * (10.0 + end_parameter * (-15.0 + 6.0 * end_parameter));
        let end_z = previous.z + distance * angle.sin();
        let end_taper = (end_angle / entry_taper)
            .powf(2.0 / 3.0)
            .min(((PI - end_angle).max(0.0) / EXIT_TAPER).powf(2.0 / 3.0))
            .min(1.0);
        points.push(Point {
            distance: previous.distance + distance,
            x: previous.x + distance * angle.cos(),
            z: end_z,
            angle: end_angle,
            curvature: GRAVITY * (force - end_angle.cos()) * end_taper
                / (ENTRY_SPEED.powi(2) - 2.0 * GRAVITY * end_z),
        });
    }
    points
}

fn curve() -> &'static [Point] {
    static CURVE: OnceLock<Vec<Point>> = OnceLock::new();
    CURVE.get_or_init(|| {
        let points = integrate(DESIGN_FORCE, ENTRY_TAPER, STEPS);
        let end = points.last().unwrap();
        assert!((end.x - SPAN).abs() < 1e-4 && (end.z - HEIGHT).abs() < 1e-4);
        points
    })
}

/// Position and its first two derivatives with respect to the unit parameter.
pub(super) fn sample(t: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let points = curve();
    let length = points.last().unwrap().distance;
    let distance = t * length;
    let index = points
        .partition_point(|point| point.distance < distance)
        .clamp(1, points.len() - 1);
    let start = points[index - 1];
    let end = points[index];
    let segment_length = end.distance - start.distance;
    let u = (distance - start.distance) / segment_length;
    let mut position = [0.0; 2];
    let mut first = [0.0; 2];
    let mut second = [0.0; 2];
    for axis in 0..2 {
        let (p0, p1, v0, v1, k0, k1) = if axis == 0 {
            (
                start.x,
                end.x,
                start.angle.cos(),
                end.angle.cos(),
                -start.angle.sin() * start.curvature,
                -end.angle.sin() * end.curvature,
            )
        } else {
            (
                start.z,
                end.z,
                start.angle.sin(),
                end.angle.sin(),
                start.angle.cos() * start.curvature,
                end.angle.cos() * end.curvature,
            )
        };
        let delta = p1 - p0;
        let m0 = segment_length * v0;
        let m1 = segment_length * v1;
        let n0 = segment_length.powi(2) * k0;
        let n1 = segment_length.powi(2) * k1;
        let a = 6.0 * delta - 3.0 * (m0 + m1) - (n0 - n1) / 2.0;
        let b = -15.0 * delta + 8.0 * m0 + 7.0 * m1 + 1.5 * n0 - n1;
        let c = 10.0 * delta - 6.0 * m0 - 4.0 * m1 - 1.5 * n0 + n1 / 2.0;
        position[axis] = ((((a * u + b) * u + c) * u + n0 / 2.0) * u + m0) * u + p0;
        first[axis] =
            ((((5.0 * a * u + 4.0 * b) * u + 3.0 * c) * u + n0) * u + m0) * length / segment_length;
        second[axis] = (((20.0 * a * u + 12.0 * b) * u + 6.0 * c) * u + n0)
            * (length / segment_length).powi(2);
    }
    (position, first, second)
}

pub(super) fn endpoint() -> [f64; 2] {
    let last = curve().last().unwrap();
    [last.x, last.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolated_curve_has_continuous_tangent_and_curvature() {
        let points = curve();
        let length = points.last().unwrap().distance;
        for point in points.iter().skip(1).take(points.len() - 2).step_by(17) {
            let parameter = point.distance / length;
            let (_, left_tangent, left_curvature) = sample(parameter - 1e-9);
            let (_, right_tangent, right_curvature) = sample(parameter + 1e-9);
            for axis in 0..2 {
                assert!((left_tangent[axis] - right_tangent[axis]).abs() < 1e-4);
                assert!((left_curvature[axis] - right_curvature[axis]).abs() < 0.02);
            }
        }
    }

    #[test]
    fn integration_is_repeatable() {
        let first = integrate(3.5, 0.2, STEPS);
        let second = integrate(3.5, 0.2, STEPS);
        for (left, right) in first.iter().zip(second) {
            assert_eq!(left.x.to_bits(), right.x.to_bits());
            assert_eq!(left.z.to_bits(), right.z.to_bits());
            assert_eq!(left.distance.to_bits(), right.distance.to_bits());
        }
    }

    #[test]
    fn integration_converges_with_finer_angle_steps() {
        let normal = integrate(3.5, 0.2, STEPS);
        let finer = integrate(3.5, 0.2, STEPS * 4);
        let normal_end = normal.last().unwrap();
        let finer_end = finer.last().unwrap();
        assert!((normal_end.x - finer_end.x).abs() < 0.3);
        assert!((normal_end.z - finer_end.z).abs() < 0.05);
        assert!((normal_end.distance - finer_end.distance).abs() < 0.3);
    }
}
