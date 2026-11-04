use std::sync::OnceLock;

const DESIGN_SPEED: f64 = 31.3;
const DESIGN_NORMAL_G: f64 = 3.5;
const ENTRY_TAPER: f64 = 0.15;
const STEPS: usize = 16_384;
const GRID_SPAN_METRES: f64 = 72.0;

#[derive(Clone, Copy)]
struct Point {
    distance: f64,
    x: f64,
    z: f64,
    angle: f64,
    curvature: f64,
}

#[derive(Clone, Copy)]
struct State {
    x: f64,
    z: f64,
    angle: f64,
}

fn smooth(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value.powi(3) * (10.0 + value * (-15.0 + 6.0 * value))
}

fn normal_load(parameter: f64) -> f64 {
    let taper = (parameter / ENTRY_TAPER).min((1.0 - parameter) / ENTRY_TAPER);
    1.0 + (DESIGN_NORMAL_G - 1.0) * smooth(taper)
}

// Shape curvature from the vertical rider load for a 31.3 m/s design entry.
fn derivative(state: State, parameter: f64, length: f64) -> State {
    let normal_acceleration = normal_load(parameter);
    let speed_squared = DESIGN_SPEED.powi(2) - 2.0 * 9.81 * state.z;
    assert!(speed_squared > 0.0, "vertical loop exceeds design energy");
    let curvature = 9.81 * (normal_acceleration - state.angle.cos()) / speed_squared;
    State {
        x: length * state.angle.cos(),
        z: length * state.angle.sin(),
        angle: length * curvature,
    }
}

fn add_scaled(state: State, derivative: State, scale: f64) -> State {
    State {
        x: state.x + derivative.x * scale,
        z: state.z + derivative.z * scale,
        angle: state.angle + derivative.angle * scale,
    }
}

fn integrate(length: f64, steps: usize) -> Vec<Point> {
    let mut points = Vec::with_capacity(steps + 1);
    let mut state = State {
        x: 0.0,
        z: 0.0,
        angle: 0.0,
    };
    points.push(Point {
        distance: 0.0,
        x: state.x,
        z: state.z,
        angle: state.angle,
        curvature: 0.0,
    });
    let step = 1.0 / steps as f64;
    for index in 0..steps {
        let parameter = index as f64 * step;
        let middle = parameter + step / 2.0;
        let first = derivative(state, parameter, length);
        let second = derivative(add_scaled(state, first, step / 2.0), middle, length);
        let third = derivative(add_scaled(state, second, step / 2.0), middle, length);
        let fourth = derivative(add_scaled(state, third, step), parameter + step, length);
        state.x += step * (first.x + 2.0 * second.x + 2.0 * third.x + fourth.x) / 6.0;
        state.z += step * (first.z + 2.0 * second.z + 2.0 * third.z + fourth.z) / 6.0;
        state.angle +=
            step * (first.angle + 2.0 * second.angle + 2.0 * third.angle + fourth.angle) / 6.0;
        let parameter = (index + 1) as f64 * step;
        let speed_squared = DESIGN_SPEED.powi(2) - 2.0 * 9.81 * state.z;
        let curvature = 9.81 * (normal_load(parameter) - state.angle.cos()) / speed_squared;
        points.push(Point {
            distance: parameter * length,
            x: state.x,
            z: state.z,
            angle: state.angle,
            curvature,
        });
    }
    points
}

fn find_length() -> f64 {
    let mut low = 120.0;
    let mut high = 160.0;
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if integrate(middle, STEPS).last().unwrap().angle < std::f64::consts::TAU {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

fn loop_points() -> &'static [Point] {
    static POINTS: OnceLock<Vec<Point>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let points = integrate(find_length(), STEPS);
        let end = points.last().unwrap();
        assert!((end.angle - std::f64::consts::TAU).abs() < 1e-8);
        assert!(end.z.abs() < 1e-5);
        assert!(end.x < GRID_SPAN_METRES);
        points
    })
}

pub(super) fn dimensions() -> (f64, f64) {
    let last = loop_points().last().unwrap();
    (last.x, GRID_SPAN_METRES - last.x)
}

pub(super) fn sample(parameter: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let points = loop_points();
    let last = points.last().unwrap();
    let (_, trailing_straight) = dimensions();
    let total_length = last.distance + trailing_straight;
    let distance = parameter * total_length;
    if distance >= last.distance {
        let straight_distance = distance - last.distance;
        return (
            [last.x + straight_distance, 0.0],
            [total_length, 0.0],
            [0.0, 0.0],
        );
    }
    let index = (distance / last.distance * STEPS as f64)
        .floor()
        .clamp(0.0, (STEPS - 1) as f64) as usize;
    let start = points[index];
    let end = points[index + 1];
    let segment_length = end.distance - start.distance;
    let u = (distance - start.distance) / segment_length;
    let mut position = [0.0; 2];
    let mut first = [0.0; 2];
    let mut second = [0.0; 2];
    for axis in 0..2 {
        let (p0, p1, v0, v1, a0, a1) = if axis == 0 {
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
        let n0 = segment_length.powi(2) * a0;
        let n1 = segment_length.powi(2) * a1;
        let a = 6.0 * delta - 3.0 * (m0 + m1) - (n0 - n1) / 2.0;
        let b = -15.0 * delta + 8.0 * m0 + 7.0 * m1 + 1.5 * n0 - n1;
        let c = 10.0 * delta - 6.0 * m0 - 4.0 * m1 - 1.5 * n0 + n1 / 2.0;
        position[axis] = ((((a * u + b) * u + c) * u + n0 / 2.0) * u + m0) * u + p0;
        first[axis] = ((((5.0 * a * u + 4.0 * b) * u + 3.0 * c) * u + n0) * u + m0) * total_length
            / segment_length;
        second[axis] = (((20.0 * a * u + 12.0 * b) * u + 6.0 * c) * u + n0)
            * (total_length / segment_length).powi(2);
    }
    (position, first, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{PieceId, Vec3, catalog};

    #[test]
    fn force_shaped_loop_stays_within_wooden_model_limits_across_entry_speeds() {
        let points = loop_points();
        for entry_speed in [29.0_f64, 30.0, DESIGN_SPEED] {
            let mut minimum_vertical_g: f64 = f64::INFINITY;
            let mut maximum_vertical_g: f64 = f64::NEG_INFINITY;
            for point in points {
                let speed_squared = entry_speed.powi(2) - 2.0 * 9.81 * point.z;
                assert!(speed_squared > 0.0, "loop stalls at {entry_speed} m/s");
                let vertical_g = speed_squared * point.curvature / 9.81 + point.angle.cos();
                minimum_vertical_g = minimum_vertical_g.min(vertical_g);
                maximum_vertical_g = maximum_vertical_g.max(vertical_g);
            }
            assert!(
                minimum_vertical_g >= -1.0,
                "{entry_speed}: {minimum_vertical_g}"
            );
            assert!(
                maximum_vertical_g <= 4.0,
                "{entry_speed}: {maximum_vertical_g}"
            );
        }
    }

    #[test]
    fn loop_connects_to_level_track_and_fits_its_grid_endpoint() {
        let (loop_span, trailing_straight) = dimensions();
        assert!((loop_span + trailing_straight - GRID_SPAN_METRES).abs() < 1e-8);
        for parameter in [0.0, 1.0] {
            let (position, tangent, second) = sample(parameter);
            assert!(position.iter().all(|value| value.is_finite()));
            assert!(tangent.iter().all(|value| value.is_finite()));
            assert!(second.iter().all(|value| value.is_finite()));
            assert!(tangent[1].abs() < 1e-8);
            assert!(second[0].abs() < 1e-8 && second[1].abs() < 1e-8);
        }
    }

    #[test]
    fn catalog_loop_sampled_rider_forces_stay_within_model_limits() {
        let piece = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap();
        let limits = crate::track::wooden_twister_model().comfortable_force_limits;
        let sample_count = 2_000;
        for entry_speed in [26.2_f64, 29.0, 31.3] {
            let mut minimum_vertical_g: f64 = f64::INFINITY;
            let mut maximum_vertical_g: f64 = f64::NEG_INFINITY;
            let mut maximum_lateral_g: f64 = 0.0;
            let mut maximum_longitudinal_g: f64 = 0.0;
            for index in 0..=sample_count {
                let distance = piece.length() * index as f64 / sample_count as f64;
                let sample = piece.sample(distance);
                let speed_squared = entry_speed.powi(2) - 2.0 * 9.81 * sample.position.z;
                assert!(speed_squared > 0.0, "loop stalls at {entry_speed} m/s");
                let delta = 0.01;
                let before = piece.sample((distance - delta).max(0.0));
                let after = piece.sample((distance + delta).min(piece.length()));
                let curvature_vector = (after.tangent - before.tangent)
                    / ((distance + delta).min(piece.length()) - (distance - delta).max(0.0));
                let force = curvature_vector * speed_squared + Vec3::new(0.0, 0.0, 9.81);
                let vertical_g = force.dot(sample.up) / 9.81;
                let lateral_g = force.dot(sample.up.cross(sample.tangent)) / 9.81;
                let longitudinal_g = force.dot(sample.tangent) / 9.81;
                minimum_vertical_g = minimum_vertical_g.min(vertical_g);
                maximum_vertical_g = maximum_vertical_g.max(vertical_g);
                maximum_lateral_g = maximum_lateral_g.max(lateral_g.abs());
                maximum_longitudinal_g = maximum_longitudinal_g.max(longitudinal_g.abs());
            }
            assert!(
                minimum_vertical_g >= limits.minimum_vertical_g,
                "{entry_speed}: minimum vertical {minimum_vertical_g}"
            );
            assert!(
                maximum_vertical_g <= limits.maximum_vertical_g,
                "{entry_speed}: maximum vertical {maximum_vertical_g}"
            );
            assert!(
                maximum_lateral_g <= limits.maximum_lateral_g,
                "{entry_speed}: lateral {maximum_lateral_g}"
            );
            assert!(
                maximum_longitudinal_g <= limits.maximum_longitudinal_g,
                "{entry_speed}: longitudinal {maximum_longitudinal_g}"
            );
        }
    }
}
