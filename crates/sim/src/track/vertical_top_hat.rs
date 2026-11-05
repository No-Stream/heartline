use std::f64::consts::FRAC_PI_2;
use std::sync::OnceLock;

const GRAVITY: f64 = 9.81;
const DESIGN_SPEED: f64 = 31.3;
const MAXIMUM_VERTICAL_G: f64 = 4.0;
const MINIMUM_VERTICAL_G: f64 = -1.0;
const DESCENT_LIMIT_SEARCH_RANGE_G: f64 = 0.01;
const CURVATURE_TAPER_ANGLE: f64 = 0.12;
const ANGLE_STEPS: usize = 8192;

#[derive(Clone, Copy)]
struct Point {
    distance: f64,
    x: f64,
    z: f64,
    angle: f64,
    curvature: f64,
}

fn taper(angle: f64) -> f64 {
    (angle / CURVATURE_TAPER_ANGLE)
        .sqrt()
        .min(((FRAC_PI_2 - angle) / CURVATURE_TAPER_ANGLE).sqrt())
        .min(1.0)
}

fn ascent_profile() -> Vec<Point> {
    let mut points = vec![Point {
        distance: 0.0,
        x: 0.0,
        z: 0.0,
        angle: 0.0,
        curvature: 0.0,
    }];
    let step = FRAC_PI_2 / ANGLE_STEPS as f64;
    let mut speed_squared = DESIGN_SPEED.powi(2);
    let mut distance = 0.0;
    let mut x = 0.0;
    let mut z = 0.0;
    for phase in 0..2 {
        for index in 0..ANGLE_STEPS {
            let progress = (index as f64 + 0.5) * step;
            let angle = if phase == 0 {
                progress
            } else {
                FRAC_PI_2 - progress
            };
            let force_limit = if phase == 0 {
                MAXIMUM_VERTICAL_G
            } else {
                MINIMUM_VERTICAL_G
            };
            let force_component = if phase == 0 {
                force_limit - angle.cos()
            } else {
                angle.cos() - force_limit
            };
            let angle_taper = taper(progress);
            let curvature_magnitude = GRAVITY * force_component * angle_taper / speed_squared;
            let arc_step = step / curvature_magnitude;
            let signed_curvature = curvature_magnitude * if phase == 0 { 1.0 } else { -1.0 };
            distance += arc_step;
            x += angle.cos() * arc_step;
            z += angle.sin() * arc_step;
            speed_squared -= 2.0 * GRAVITY * angle.sin() * arc_step;
            assert!(speed_squared > 0.0, "top hat design profile stalls");
            let end_angle = if phase == 0 {
                (index + 1) as f64 * step
            } else {
                FRAC_PI_2 - (index + 1) as f64 * step
            };
            points.push(Point {
                distance,
                x,
                z,
                angle: end_angle,
                curvature: signed_curvature,
            });
        }
    }
    points.last_mut().unwrap().curvature = 0.0;
    points
}

fn descent_profile(mut speed_squared: f64, descent_minimum_g: f64) -> Vec<Point> {
    let mut points = vec![Point {
        distance: 0.0,
        x: 0.0,
        z: 0.0,
        angle: 0.0,
        curvature: 0.0,
    }];
    let step = FRAC_PI_2 / ANGLE_STEPS as f64;
    let mut distance = 0.0;
    let mut x = 0.0;
    let mut z = 0.0;
    for phase in 0..2 {
        for index in 0..ANGLE_STEPS {
            let progress = (index as f64 + 0.5) * step;
            let angle = if phase == 0 {
                -progress
            } else {
                -FRAC_PI_2 + progress
            };
            let force_limit = if phase == 0 {
                descent_minimum_g
            } else {
                MAXIMUM_VERTICAL_G
            };
            let force_component = force_limit - angle.cos();
            let curvature_magnitude =
                GRAVITY * force_component.abs() * taper(progress) / speed_squared;
            let arc_step = step / curvature_magnitude;
            let signed_curvature = if phase == 0 {
                -curvature_magnitude
            } else {
                curvature_magnitude
            };
            distance += arc_step;
            x += angle.cos() * arc_step;
            z += angle.sin() * arc_step;
            speed_squared -= 2.0 * GRAVITY * angle.sin() * arc_step;
            let end_angle = if phase == 0 {
                -((index + 1) as f64) * step
            } else {
                -FRAC_PI_2 + (index + 1) as f64 * step
            };
            points.push(Point {
                distance,
                x,
                z,
                angle: end_angle,
                curvature: signed_curvature,
            });
        }
    }
    points.last_mut().unwrap().curvature = 0.0;
    points
}

fn profile() -> &'static [Point] {
    static PROFILE: OnceLock<Vec<Point>> = OnceLock::new();
    PROFILE.get_or_init(|| {
        let ascent = ascent_profile();
        let ascent_end = *ascent.last().unwrap();
        let mut low_limit = MINIMUM_VERTICAL_G;
        let mut high_limit = MINIMUM_VERTICAL_G + DESCENT_LIMIT_SEARCH_RANGE_G;
        for _ in 0..24 {
            let test_limit = (low_limit + high_limit) / 2.0;
            let test_descent = descent_profile(
                DESIGN_SPEED.powi(2) - 2.0 * GRAVITY * ascent_end.z,
                test_limit,
            );
            if ascent_end.z + test_descent.last().unwrap().z > 0.0 {
                low_limit = test_limit;
            } else {
                high_limit = test_limit;
            }
        }
        let descent = descent_profile(
            DESIGN_SPEED.powi(2) - 2.0 * GRAVITY * ascent_end.z,
            (low_limit + high_limit) / 2.0,
        );
        let descent_end = *descent.last().unwrap();
        let span = 88.0;
        let plateau = span - ascent_end.x - descent_end.x;
        assert!(plateau > 0.0);
        let mut points = ascent;
        points.push(Point {
            distance: ascent_end.distance + plateau,
            x: ascent_end.x + plateau,
            z: ascent_end.z,
            angle: 0.0,
            curvature: 0.0,
        });
        for point in descent.iter().skip(1) {
            points.push(Point {
                distance: ascent_end.distance + plateau + point.distance,
                x: ascent_end.x + plateau + point.x,
                z: ascent_end.z + point.z,
                angle: point.angle,
                curvature: point.curvature,
            });
        }
        let exit = points.last().unwrap();
        assert!(
            (exit.x - span).abs() < 1e-6 && exit.z.abs() < 1e-6,
            "top hat endpoint is ({}, {}) with ascent height {} and descent height {}",
            exit.x,
            exit.z,
            ascent_end.z,
            -descent_end.z
        );
        points
    })
}

fn interpolate(
    start: Point,
    end: Point,
    distance: f64,
    total_length: f64,
) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let segment_length = end.distance - start.distance;
    let parameter = (distance - start.distance) / segment_length;
    let tangent_start = [start.angle.cos(), start.angle.sin()];
    let tangent_end = [end.angle.cos(), end.angle.sin()];
    let curvature_start = [
        -start.angle.sin() * start.curvature,
        start.angle.cos() * start.curvature,
    ];
    let curvature_end = [
        -end.angle.sin() * end.curvature,
        end.angle.cos() * end.curvature,
    ];
    let mut position = [0.0; 2];
    let mut first = [0.0; 2];
    let mut second = [0.0; 2];
    for axis in 0..2 {
        let p0 = if axis == 0 { start.x } else { start.z };
        let p1 = if axis == 0 { end.x } else { end.z };
        let v0 = tangent_start[axis];
        let v1 = tangent_end[axis];
        let a0 = curvature_start[axis];
        let a1 = curvature_end[axis];
        let delta = p1 - p0;
        let m0 = segment_length * v0;
        let m1 = segment_length * v1;
        let n0 = segment_length.powi(2) * a0;
        let n1 = segment_length.powi(2) * a1;
        let coefficient_a = 6.0 * delta - 3.0 * (m0 + m1) - (n0 - n1) / 2.0;
        let coefficient_b = -15.0 * delta + 8.0 * m0 + 7.0 * m1 + 1.5 * n0 - n1;
        let coefficient_c = 10.0 * delta - 6.0 * m0 - 4.0 * m1 - 1.5 * n0 + n1 / 2.0;
        position[axis] = ((((coefficient_a * parameter + coefficient_b) * parameter
            + coefficient_c)
            * parameter
            + n0 / 2.0)
            * parameter
            + m0)
            * parameter
            + p0;
        first[axis] = ((((5.0 * coefficient_a * parameter + 4.0 * coefficient_b) * parameter
            + 3.0 * coefficient_c)
            * parameter
            + n0)
            * parameter
            + m0)
            * total_length
            / segment_length;
        second[axis] = (((20.0 * coefficient_a * parameter + 12.0 * coefficient_b) * parameter
            + 6.0 * coefficient_c)
            * parameter
            + n0)
            * (total_length / segment_length).powi(2);
    }
    (position, first, second)
}

pub(super) fn sample(parameter: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let points = profile();
    let total_length = points.last().unwrap().distance;
    let distance = parameter * total_length;
    let index = points
        .partition_point(|point| point.distance < distance)
        .clamp(1, points.len() - 1);
    interpolate(points[index - 1], points[index], distance, total_length)
}

pub(super) fn endpoint() -> [f64; 2] {
    let end = profile().last().unwrap();
    [end.x, end.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_designed_profile_fits_its_grid_endpoint_and_has_smooth_curvature() {
        let points = profile();
        let end = points.last().unwrap();
        assert!((end.x - 88.0).abs() < 1e-6);
        assert!(points.iter().all(|point| point.z >= -1e-6));
        assert!(points.iter().all(|point| {
            let speed_squared = DESIGN_SPEED.powi(2) - 2.0 * GRAVITY * point.z;
            speed_squared > 0.0
                && (MINIMUM_VERTICAL_G - 0.01..=MAXIMUM_VERTICAL_G + 0.01)
                    .contains(&(point.angle.cos() + speed_squared * point.curvature / GRAVITY))
        }));
        for index in 0..=4000 {
            let (position, first, second) = sample(index as f64 / 4000.0);
            let tangent_speed = first[0].hypot(first[1]);
            let curvature = (first[0] * second[1] - first[1] * second[0]) / tangent_speed.powi(3);
            let speed_squared = DESIGN_SPEED.powi(2) - 2.0 * GRAVITY * position[1];
            let vertical_g = position[1];
            let local_tangent_z = first[1] / tangent_speed;
            let local_up_dot_gravity = local_tangent_z.atan2(first[0] / tangent_speed).cos();
            let measured_vertical_g = speed_squared * curvature / GRAVITY + local_up_dot_gravity;
            assert!(
                (MINIMUM_VERTICAL_G - 0.02..=MAXIMUM_VERTICAL_G + 0.02)
                    .contains(&measured_vertical_g),
                "t={}: z={vertical_g}, curvature={curvature}, g={measured_vertical_g}",
                index as f64 / 4000.0
            );
        }
        for pair in points.windows(2).step_by(37) {
            let curvature_change = (pair[1].curvature - pair[0].curvature).abs();
            assert!(curvature_change < 0.02, "curvature jump {curvature_change}");
        }
    }

    #[test]
    fn catalog_piece_preserves_the_profile_after_arc_length_sampling() {
        let piece = crate::track::catalog::catalog()
            .iter()
            .find(|piece| piece.id == crate::track::PieceId::TopHat)
            .unwrap();
        let horizontal_scale = (piece.exit.position().x - piece.entry.position().x) / endpoint()[0];
        for index in 0..=4000 {
            let distance = piece.length() * index as f64 / 4000.0;
            let actual = piece.sample(distance);
            let (position, first, _) = sample(piece.parameter_at_distance(distance));
            let expected_tangent =
                crate::track::curves::Vec3::new(first[0] * horizontal_scale, 0.0, first[1]).unit();
            assert!((actual.position.x - position[0] * horizontal_scale).abs() < 1e-8);
            assert!((actual.position.z - position[1]).abs() < 1e-8);
            assert!((actual.tangent - expected_tangent).length() < 1e-8);
        }
        assert!((piece.sample(0.0).position - piece.entry.position()).length() < 1e-8);
        assert!((piece.sample(piece.length()).position - piece.exit.position()).length() < 1e-8);
    }
}
