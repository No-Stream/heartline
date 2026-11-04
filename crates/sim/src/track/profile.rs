use std::f64::consts::PI;

const GRAVITY: f64 = 9.81;
const HEART_HEIGHT: f64 = 1.2;

#[derive(Clone, Copy, Debug)]
enum Schedule {
    Constant(f64),
    Loop { bottom: f64, top: f64 },
}

impl Schedule {
    fn force(self, angle: f64) -> f64 {
        match self {
            Self::Constant(force) => force,
            Self::Loop { bottom, top } => top + (bottom - top) * (1.0 + angle.cos()) / 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Arc { angle: f64, schedule: Schedule },
    Insert(f64),
}

#[derive(Clone, Debug)]
struct ProfileSpec {
    segments: Vec<Segment>,
    maximum_entry: f64,
    fixed_speed: bool,
}

impl ProfileSpec {
    #[cfg(test)]
    fn arc(angle: f64, force: f64, speed: f64) -> Self {
        Self {
            segments: vec![Segment::Arc {
                angle,
                schedule: Schedule::Constant(force),
            }],
            maximum_entry: speed,
            fixed_speed: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    distance: f64,
    x: f64,
    z: f64,
    angle: f64,
    heart_curvature: f64,
    curvature: f64,
}

#[derive(Clone, Debug)]
struct SolvedProfile {
    points: Vec<Point>,
}

impl SolvedProfile {
    fn sample(&self, distance: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
        let index = self
            .points
            .partition_point(|point| point.distance < distance)
            .clamp(1, self.points.len() - 1);
        let start = self.points[index - 1];
        let end = self.points[index];
        let length = end.distance - start.distance;
        let u = (distance - start.distance) / length;
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
            let m0 = length * v0;
            let m1 = length * v1;
            let n0 = length.powi(2) * k0;
            let n1 = length.powi(2) * k1;
            let a = 6.0 * delta - 3.0 * (m0 + m1) - (n0 - n1) / 2.0;
            let b = -15.0 * delta + 8.0 * m0 + 7.0 * m1 + 1.5 * n0 - n1;
            let c = 10.0 * delta - 6.0 * m0 - 4.0 * m1 - 1.5 * n0 + n1 / 2.0;
            position[axis] = ((((a * u + b) * u + c) * u + n0 / 2.0) * u + m0) * u + p0;
            first[axis] = ((((5.0 * a * u + 4.0 * b) * u + 3.0 * c) * u + n0) * u + m0) / length;
            second[axis] = (((20.0 * a * u + 12.0 * b) * u + 6.0 * c) * u + n0) / length.powi(2);
        }
        (position, first, second)
    }
}

fn integrate(spec: &ProfileSpec, previous: Option<&SolvedProfile>, steps: usize) -> SolvedProfile {
    let mut points = vec![Point {
        distance: 0.0,
        x: 0.0,
        z: 0.0,
        angle: 0.0,
        heart_curvature: 0.0,
        curvature: 0.0,
    }];
    let mut heart_x = 0.0;
    let mut heart_z = HEART_HEIGHT;
    let mut angle: f64 = 0.0;
    let mut distance = 0.0;
    for segment in &spec.segments {
        match *segment {
            Segment::Insert(length) if length > 0.0 => {
                assert_eq!(points.last().unwrap().curvature, 0.0);
                heart_x += length * angle.cos();
                heart_z += length * angle.sin();
                distance += length;
                points.push(Point {
                    distance,
                    x: heart_x + HEART_HEIGHT * angle.sin(),
                    z: heart_z - HEART_HEIGHT * angle.cos(),
                    angle,
                    heart_curvature: 0.0,
                    curvature: 0.0,
                });
            }
            Segment::Insert(_) => {}
            Segment::Arc {
                angle: delta,
                schedule,
            } => {
                let start_angle = angle;
                let taper_angle =
                    0.3_f64.min(delta.abs() / 4.0 * if delta.abs() < PI / 3.0 { 1.0 } else { 4.0 });
                for index in 0..steps {
                    let midpoint_parameter = (index as f64 + 0.5) / steps as f64;
                    let smooth = |t: f64| t.powi(3) * (10.0 + t * (-15.0 + 6.0 * t));
                    let midpoint_angle = start_angle + delta * smooth(midpoint_parameter);
                    let angle_step = delta
                        * 30.0
                        * midpoint_parameter.powi(2)
                        * (1.0 - midpoint_parameter).powi(2)
                        / steps as f64;
                    let taper = ((midpoint_angle - start_angle).abs() / taper_angle)
                        .powf(2.0 / 3.0)
                        .min(
                            ((start_angle + delta - midpoint_angle).abs() / taper_angle)
                                .powf(2.0 / 3.0),
                        )
                        .min(1.0);
                    let point_speed_squared = spec.maximum_entry.powi(2)
                        - if spec.fixed_speed {
                            0.0
                        } else {
                            2.0 * GRAVITY * (heart_z - HEART_HEIGHT * angle.cos())
                        };
                    let speed_squared = previous.map_or(point_speed_squared, |profile| {
                        worst_speed_squared(profile, distance, spec.maximum_entry)
                    });
                    let curvature =
                        GRAVITY * (schedule.force(midpoint_angle) - midpoint_angle.cos()) * taper
                            / speed_squared;
                    let heart_distance = angle_step / curvature;
                    assert!(heart_distance.is_finite() && heart_distance > 0.0);
                    heart_x += heart_distance * midpoint_angle.cos();
                    heart_z += heart_distance * midpoint_angle.sin();
                    distance += heart_distance * (1.0 + HEART_HEIGHT * curvature);
                    angle = start_angle + delta * smooth((index + 1) as f64 / steps as f64);
                    let end_taper = ((angle - start_angle).abs() / taper_angle)
                        .powf(2.0 / 3.0)
                        .min(((start_angle + delta - angle).abs() / taper_angle).powf(2.0 / 3.0))
                        .min(1.0);
                    let end_point_speed_squared = spec.maximum_entry.powi(2)
                        - if spec.fixed_speed {
                            0.0
                        } else {
                            2.0 * GRAVITY * (heart_z - HEART_HEIGHT * angle.cos())
                        };
                    let end_speed_squared = previous.map_or(end_point_speed_squared, |profile| {
                        worst_speed_squared(profile, distance, spec.maximum_entry)
                    });
                    let heart_curvature =
                        GRAVITY * (schedule.force(angle) - angle.cos()) * end_taper
                            / end_speed_squared;
                    points.push(Point {
                        distance,
                        x: heart_x + HEART_HEIGHT * angle.sin(),
                        z: heart_z - HEART_HEIGHT * angle.cos(),
                        angle,
                        heart_curvature,
                        curvature: heart_curvature / (1.0 + HEART_HEIGHT * heart_curvature),
                    });
                }
            }
        }
    }
    SolvedProfile { points }
}

fn worst_speed_squared(profile: &SolvedProfile, distance: f64, entry_speed: f64) -> f64 {
    let minimum_mean_height = [6, 12]
        .into_iter()
        .flat_map(|cars| {
            (0..cars).map(move |car| {
                let spacing = 18.0 / cars as f64;
                (0..cars)
                    .map(|other| {
                        rail_height(profile, distance + (car as f64 - other as f64) * spacing)
                    })
                    .sum::<f64>()
                    / cars as f64
            })
        })
        .fold(f64::INFINITY, f64::min);
    entry_speed.powi(2) - 2.0 * GRAVITY * minimum_mean_height
}

fn rail_height(profile: &SolvedProfile, distance: f64) -> f64 {
    let first = profile.points.first().unwrap();
    let last = profile.points.last().unwrap();
    if distance < 0.0 {
        first.z + distance * first.angle.sin()
    } else if distance > last.distance {
        last.z + (distance - last.distance) * last.angle.sin()
    } else {
        profile.sample(distance).0[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn constant_speed_arc_matches_curvature_equation() {
        let spec = ProfileSpec::arc(FRAC_PI_2, 4.2, 30.0);
        let profile = integrate(&spec, None, 2048);
        let point = &profile.points[1024];
        let expected = GRAVITY * (4.2 - point.angle.cos()) / 30.0_f64.powi(2);
        assert!((point.heart_curvature / expected - 1.0).abs() < 0.01);
    }

    #[test]
    fn integration_is_repeatable_and_converges() {
        let spec = ProfileSpec::arc(FRAC_PI_2, 4.2, 30.0);
        let first = integrate(&spec, None, 2048);
        let second = integrate(&spec, None, 2048);
        let finer = integrate(&spec, None, 8192);
        assert_eq!(first.points, second.points);
        let end = first.points.last().unwrap();
        let fine_end = finer.points.last().unwrap();
        assert!((end.x - fine_end.x).abs() < 0.1);
        assert!((end.z - fine_end.z).abs() < 0.1);
    }

    #[test]
    fn interpolation_matches_tangent_and_curvature_at_nodes() {
        let spec = ProfileSpec::arc(FRAC_PI_2, 4.2, 30.0);
        let profile = integrate(&spec, None, 2048);
        for point in profile.points.iter().skip(1).take(2046).step_by(31) {
            let (_, left_tangent, left_curvature) = profile.sample(point.distance - 1e-8);
            let (_, right_tangent, right_curvature) = profile.sample(point.distance + 1e-8);
            for axis in 0..2 {
                assert!((left_tangent[axis] - right_tangent[axis]).abs() < 1e-5);
                assert!((left_curvature[axis] - right_curvature[axis]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn loop_profile_closes_vertically() {
        let spec = ProfileSpec {
            segments: vec![Segment::Arc {
                angle: 2.0 * PI,
                schedule: Schedule::Loop {
                    bottom: 4.2,
                    top: 1.0,
                },
            }],
            maximum_entry: 31.3,
            fixed_speed: false,
        };
        let mut profile = integrate(&spec, None, 2048);
        for _ in 0..4 {
            profile = integrate(&spec, Some(&profile), 2048);
        }
        let end = profile.points.last().unwrap();
        eprintln!(
            "loop end: x={}, z={}, length={}",
            end.x, end.z, end.distance
        );
        assert!(end.z.abs() < 1e-9);
    }

    #[test]
    fn half_loop_iteration_converges() {
        let spec = ProfileSpec {
            segments: vec![Segment::Arc {
                angle: PI,
                schedule: Schedule::Loop {
                    bottom: 4.2,
                    top: 1.0,
                },
            }],
            maximum_entry: 31.3,
            fixed_speed: false,
        };
        let mut profile = integrate(&spec, None, 2048);
        for iteration in 0..4 {
            let next = integrate(&spec, Some(&profile), 2048);
            let previous_end = profile.points.last().unwrap();
            let end = next.points.last().unwrap();
            eprintln!(
                "iteration {iteration}: x={}, z={}, length={}, movement={}",
                end.x,
                end.z,
                end.distance,
                (end.x - previous_end.x).hypot(end.z - previous_end.z)
            );
            profile = next;
        }
        let end = profile.points.last().unwrap();
        let next = integrate(&spec, Some(&profile), 2048);
        let next_end = next.points.last().unwrap();
        assert!((end.x - next_end.x).hypot(end.z - next_end.z) < 0.001);
        let minimum_entry = (2.0 * GRAVITY * end.z + 4.0_f64.powi(2)).sqrt();
        let maximum_force = profile
            .points
            .iter()
            .map(|point| {
                point.heart_curvature
                    * worst_speed_squared(&profile, point.distance, spec.maximum_entry)
                    / GRAVITY
                    + point.angle.cos()
            })
            .fold(f64::NEG_INFINITY, f64::max);
        eprintln!("minimum entry={minimum_entry}, maximum entry={}, maximum estimated force={maximum_force}", spec.maximum_entry);
        assert!(minimum_entry < spec.maximum_entry - 2.0);
    }
}
