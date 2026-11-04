use std::f64::consts::PI;

use crate::state_hash::StateHasher;
use crate::track::curves::HEART_HEIGHT_METRES;
use crate::track::curves::RatedSpeeds;

const GRAVITY: f64 = 9.81;
const HEART_HEIGHT: f64 = HEART_HEIGHT_METRES;

#[derive(Clone, Copy, Debug)]
enum Schedule {
    Constant(f64),
    Loop { bottom: f64, top: f64, hold: f64 },
}

impl Schedule {
    fn force(self, angle: f64) -> f64 {
        match self {
            Self::Constant(force) => force,
            Self::Loop { bottom, top, hold } => {
                if angle <= hold {
                    bottom
                } else {
                    top + (bottom - top) * (1.0 + (PI * (angle - hold) / (PI - hold)).cos()) / 2.0
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Arc { angle: f64, schedule: Schedule },
    Insert(f64),
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileSpec {
    segments: Vec<Segment>,
    maximum_entry: f64,
    fixed_speed: bool,
    mirror_apex: bool,
    start_angle: f64,
    name: &'static str,
}

impl ProfileSpec {
    pub(crate) fn half_loop(maximum_entry: f64, bottom: f64, top: f64) -> Self {
        Self {
            segments: vec![Segment::Arc {
                angle: PI,
                schedule: Schedule::Loop {
                    bottom,
                    top,
                    hold: 120.0_f64.to_radians(),
                },
            }],
            maximum_entry,
            fixed_speed: false,
            mirror_apex: false,
            start_angle: 0.0,
            name: "half loop",
        }
    }
    pub(crate) fn pull(start_angle: f64, end_angle: f64, maximum_entry: f64, force: f64) -> Self {
        Self {
            segments: vec![Segment::Arc {
                angle: end_angle - start_angle,
                schedule: Schedule::Constant(force),
            }],
            maximum_entry,
            fixed_speed: false,
            mirror_apex: false,
            start_angle,
            name: "pull profile",
        }
    }
    pub(crate) fn full_loop(maximum_entry: f64, bottom: f64, top: f64) -> Self {
        Self {
            mirror_apex: true,
            name: "full loop",
            ..Self::half_loop(maximum_entry, bottom, top)
        }
    }
    #[cfg(test)]
    fn arc(angle: f64, force: f64, speed: f64) -> Self {
        Self {
            segments: vec![Segment::Arc {
                angle,
                schedule: Schedule::Constant(force),
            }],
            maximum_entry: speed,
            fixed_speed: true,
            mirror_apex: false,
            start_angle: 0.0,
            name: "test arc",
        }
    }
    pub(crate) fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SolvedProfile {
    points: Vec<Point>,
}

impl SolvedProfile {
    pub(crate) fn rated_speeds(
        &self,
        name: &str,
        maximum_entry: f64,
        minimum_window: f64,
        model_radius: f64,
    ) -> RatedSpeeds {
        let minimum_entry = self.minimum_entry();
        assert!(
            maximum_entry - minimum_entry >= minimum_window,
            "{name}: speed window is too narrow; raise the hold angle or lower the rated maximum"
        );
        assert!(
            self.minimum_radius() >= model_radius + 0.5,
            "{name}: minimum radius is too small; raise the top force or lower the rated maximum"
        );
        RatedSpeeds {
            minimum_entry,
            maximum_entry,
        }
    }
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_len(self.points.len());
        for p in &self.points {
            for value in [
                p.distance,
                p.x,
                p.z,
                p.angle,
                p.heart_curvature,
                p.curvature,
            ] {
                hash.write_f64(if value == 0.0 { 0.0 } else { value });
            }
        }
    }
    pub(crate) fn length(&self) -> f64 {
        self.points.last().unwrap().distance
    }
    pub(crate) fn endpoint(&self) -> [f64; 2] {
        let p = self.points.last().unwrap();
        [p.x, p.z]
    }
    pub(crate) fn minimum_radius(&self) -> f64 {
        self.points
            .iter()
            .map(|p| p.curvature.abs())
            .fold(0.0_f64, f64::max)
            .recip()
    }
    pub(crate) fn minimum_entry(&self) -> f64 {
        let max_mean_height = self
            .points
            .iter()
            .map(|p| maximum_mean_height(self, p.distance))
            .fold(f64::NEG_INFINITY, f64::max);
        (2.0 * GRAVITY * max_mean_height + 36.0).sqrt()
    }
    pub(crate) fn maximum_force(&self, entry: f64) -> f64 {
        self.points
            .iter()
            .map(|p| {
                p.heart_curvature * worst_speed_squared(self, p.distance, entry, false) / GRAVITY
                    + p.angle.cos()
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }
    pub(crate) fn sample(&self, distance: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
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
        angle: spec.start_angle,
        heart_curvature: 0.0,
        curvature: 0.0,
    }];
    let mut heart_x = -HEART_HEIGHT * spec.start_angle.sin();
    let mut heart_z = HEART_HEIGHT * spec.start_angle.cos();
    let mut angle: f64 = spec.start_angle;
    let mut distance = 0.0;
    for (segment_index, segment) in spec.segments.iter().enumerate() {
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
                            if spec.mirror_apex && segment_index + 1 == spec.segments.len() {
                                1.0
                            } else {
                                ((start_angle + delta - midpoint_angle).abs() / taper_angle)
                                    .powf(2.0 / 3.0)
                            },
                        )
                        .min(1.0);
                    let point_speed_squared = spec.maximum_entry.powi(2)
                        - if spec.fixed_speed {
                            0.0
                        } else {
                            2.0 * GRAVITY * (heart_z - HEART_HEIGHT * angle.cos())
                        };
                    let speed_squared = previous.map_or(point_speed_squared, |profile| {
                        worst_speed_squared(profile, distance, spec.maximum_entry, spec.mirror_apex)
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
                        .min(
                            if spec.mirror_apex && segment_index + 1 == spec.segments.len() {
                                1.0
                            } else {
                                ((start_angle + delta - angle).abs() / taper_angle).powf(2.0 / 3.0)
                            },
                        )
                        .min(1.0);
                    let end_point_speed_squared = spec.maximum_entry.powi(2)
                        - if spec.fixed_speed {
                            0.0
                        } else {
                            2.0 * GRAVITY * (heart_z - HEART_HEIGHT * angle.cos())
                        };
                    let end_speed_squared = previous.map_or(end_point_speed_squared, |profile| {
                        worst_speed_squared(profile, distance, spec.maximum_entry, spec.mirror_apex)
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

fn minimum_mean_height(profile: &SolvedProfile, distance: f64, mirror: bool) -> f64 {
    [6, 12]
        .into_iter()
        .flat_map(|cars| {
            (0..cars).map(move |car| {
                let spacing = 18.0 / cars as f64;
                (0..cars)
                    .map(|other| {
                        rail_height(
                            profile,
                            distance + (car as f64 - other as f64) * spacing,
                            mirror,
                        )
                    })
                    .sum::<f64>()
                    / cars as f64
            })
        })
        .fold(f64::INFINITY, f64::min)
}

fn maximum_mean_height(profile: &SolvedProfile, distance: f64) -> f64 {
    [6, 12]
        .into_iter()
        .flat_map(|cars| {
            (0..cars).map(move |car| {
                let spacing = 18.0 / cars as f64;
                (0..cars)
                    .map(|other| {
                        rail_height(
                            profile,
                            distance + (car as f64 - other as f64) * spacing,
                            false,
                        )
                    })
                    .sum::<f64>()
                    / cars as f64
            })
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn worst_speed_squared(
    profile: &SolvedProfile,
    distance: f64,
    entry_speed: f64,
    mirror: bool,
) -> f64 {
    entry_speed.powi(2) - 2.0 * GRAVITY * minimum_mean_height(profile, distance, mirror)
}

fn rail_height(profile: &SolvedProfile, distance: f64, mirror: bool) -> f64 {
    let first = profile.points.first().unwrap();
    let last = profile.points.last().unwrap();
    if distance < 0.0 {
        first.z + distance * first.angle.sin()
    } else if distance > last.distance {
        if mirror {
            rail_height(profile, 2.0 * last.distance - distance, false)
        } else {
            last.z + (distance - last.distance) * last.angle.sin()
        }
    } else {
        profile.sample(distance).0[1]
    }
}

pub(crate) fn solve(spec: &ProfileSpec) -> SolvedProfile {
    let mut profile = integrate(spec, None, 2048);
    for _ in 0..12 {
        let next = integrate(spec, Some(&profile), 2048);
        let before = profile.points.last().unwrap();
        let after = next.points.last().unwrap();
        let movement = (before.x - after.x).hypot(before.z - after.z);
        profile = next;
        if movement < 0.000001 {
            return profile;
        }
    }
    panic!(
        "{}: profile did not converge; adjust the force schedule or rated speed",
        spec.name
    );
}

pub(crate) fn snap_half_loop(spec: &ProfileSpec) -> SolvedProfile {
    snap_half_loop_with_hold_range(spec, 100.0_f64.to_radians(), 140.0_f64.to_radians())
}

fn snap_half_loop_with_hold_range(
    spec: &ProfileSpec,
    mut low: f64,
    mut high: f64,
) -> SolvedProfile {
    let target_height = (solve(spec).endpoint()[1] / 2.0).round() * 2.0;
    let mut adjusted = spec.clone();
    if let Segment::Arc {
        schedule: Schedule::Loop { hold, .. },
        ..
    } = &mut adjusted.segments[0]
    {
        *hold = low;
    } else {
        panic!("half loop requires a loop arc");
    }
    let lower_height = solve(&adjusted).endpoint()[1];
    if let Segment::Arc {
        schedule: Schedule::Loop { hold, .. },
        ..
    } = &mut adjusted.segments[0]
    {
        *hold = high;
    }
    let upper_height = solve(&adjusted).endpoint()[1];
    if !(upper_height <= target_height && target_height <= lower_height) {
        return snap_half_loop_with_vertical_insert(spec);
    }
    for _ in 0..40 {
        let hold = (low + high) / 2.0;
        if let Segment::Arc {
            schedule:
                Schedule::Loop {
                    hold: ref mut value,
                    ..
                },
            ..
        } = adjusted.segments[0]
        {
            *value = hold;
        } else {
            panic!("half loop requires a loop arc");
        }
        if solve(&adjusted).endpoint()[1] > target_height {
            low = hold;
        } else {
            high = hold;
        }
    }
    let core = solve(&adjusted);
    let target_x = (core.endpoint()[0] / 2.0).floor() * 2.0;
    adjusted
        .segments
        .push(Segment::Insert(core.endpoint()[0] - target_x));
    let mut result = solve(&adjusted);
    let last = result.points.last_mut().unwrap();
    last.x = target_x;
    last.z = target_height;
    result
}

fn snap_half_loop_with_vertical_insert(spec: &ProfileSpec) -> SolvedProfile {
    let Segment::Arc { angle, schedule } = spec.segments[0] else {
        panic!("{}: half loop requires an arc", spec.name);
    };
    assert!((angle - PI).abs() < 1e-12);
    let mut adjusted = spec.clone();
    adjusted.segments = vec![
        Segment::Arc {
            angle: PI / 2.0,
            schedule,
        },
        Segment::Insert(0.0),
        Segment::Arc {
            angle: PI / 2.0,
            schedule,
        },
    ];
    let core = solve(&adjusted);
    let target_height = (core.endpoint()[1] / 2.0).ceil() * 2.0;
    let mut low = 0.0;
    let mut high = 2.0;
    adjusted.segments[1] = Segment::Insert(high);
    while solve(&adjusted).endpoint()[1] < target_height {
        high *= 2.0;
        adjusted.segments[1] = Segment::Insert(high);
    }
    for _ in 0..40 {
        let length = (low + high) / 2.0;
        adjusted.segments[1] = Segment::Insert(length);
        if solve(&adjusted).endpoint()[1] < target_height {
            low = length;
        } else {
            high = length;
        }
    }
    let core = solve(&adjusted);
    let target_x = (core.endpoint()[0] / 2.0).floor() * 2.0;
    adjusted
        .segments
        .push(Segment::Insert(core.endpoint()[0] - target_x));
    let mut profile = solve(&adjusted);
    let last = profile.points.last_mut().unwrap();
    last.x = target_x;
    last.z = target_height;
    profile
}

pub(crate) fn snap_full_loop(spec: &ProfileSpec) -> SolvedProfile {
    let mut profile = solve(spec);
    let apex = *profile.points.last().unwrap();
    let reflected: Vec<_> = profile
        .points
        .iter()
        .rev()
        .skip(1)
        .map(|p| Point {
            distance: 2.0 * apex.distance - p.distance,
            x: 2.0 * apex.x - p.x,
            z: p.z,
            angle: 2.0 * PI - p.angle,
            heart_curvature: p.heart_curvature,
            curvature: p.curvature,
        })
        .collect();
    profile.points.extend(reflected);
    let target_x = (2.0 * apex.x / 2.0).ceil() * 2.0;
    let mut end = *profile.points.last().unwrap();
    if target_x > end.x {
        end.distance += target_x - end.x;
        end.x = target_x;
        profile.points.push(end);
    }
    profile
}

pub(crate) fn snap_pull(spec: &ProfileSpec) -> SolvedProfile {
    let Segment::Arc { angle, .. } = spec.segments[0] else {
        panic!("pull requires one arc");
    };
    assert_eq!(spec.segments.len(), 1);
    let end_angle = spec.start_angle + angle;
    let determinant = (end_angle - spec.start_angle).sin();
    assert!(determinant > 0.0);
    let raw = solve(spec);
    let [raw_x, raw_z] = raw.endpoint();
    let first_x = (raw_x / 2.0).ceil() as i32;
    let first_z = (raw_z / 2.0).ceil() as i32;
    for shell in 0.. {
        let mut candidates = Vec::new();
        for x in first_x..=first_x + shell {
            for z in first_z..=first_z + shell {
                if x != first_x + shell && z != first_z + shell {
                    continue;
                }
                let dx = 2.0 * x as f64 - raw_x;
                let dz = 2.0 * z as f64 - raw_z;
                let start = (dx * end_angle.sin() - dz * end_angle.cos()) / determinant;
                let end = (dz * spec.start_angle.cos() - dx * spec.start_angle.sin()) / determinant;
                if start >= 0.0 && end >= 0.0 {
                    candidates.push((dx.hypot(dz), x, z, start, end));
                }
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, x, z, mut start, mut end) in candidates {
            let mut adjusted = spec.clone();
            for _ in 0..20 {
                adjusted.segments = vec![
                    Segment::Insert(start),
                    spec.segments[0],
                    Segment::Insert(end),
                ];
                let mut profile = solve(&adjusted);
                let [actual_x, actual_z] = profile.endpoint();
                let dx = 2.0 * x as f64 - actual_x;
                let dz = 2.0 * z as f64 - actual_z;
                if dx.hypot(dz) < 1e-10 {
                    let last = profile.points.last_mut().unwrap();
                    last.x = 2.0 * x as f64;
                    last.z = 2.0 * z as f64;
                    return profile;
                }
                start += (dx * end_angle.sin() - dz * end_angle.cos()) / determinant;
                end += (dz * spec.start_angle.cos() - dx * spec.start_angle.sin()) / determinant;
                if start < 0.0 || end < 0.0 {
                    break;
                }
            }
        }
    }
    unreachable!()
}

#[cfg(test)]
pub(crate) fn test_profile() -> SolvedProfile {
    integrate(
        &ProfileSpec::arc(std::f64::consts::FRAC_PI_2, 4.2, 30.0),
        None,
        32,
    )
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
        let profile = snap_full_loop(&ProfileSpec::full_loop(31.3, 4.2, 1.5));
        assert!(profile.endpoint()[1].abs() < 1e-9);
    }

    #[test]
    fn half_loop_iteration_converges() {
        let spec = ProfileSpec {
            segments: vec![Segment::Arc {
                angle: PI,
                schedule: Schedule::Loop {
                    bottom: 4.2,
                    top: 1.0,
                    hold: 120.0_f64.to_radians(),
                },
            }],
            maximum_entry: 31.3,
            fixed_speed: false,
            mirror_apex: false,
            start_angle: 0.0,
            name: "half loop test",
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
                    * worst_speed_squared(&profile, point.distance, spec.maximum_entry, false)
                    / GRAVITY
                    + point.angle.cos()
            })
            .fold(f64::NEG_INFINITY, f64::max);
        eprintln!(
            "minimum entry={minimum_entry}, maximum entry={}, maximum estimated force={maximum_force}",
            spec.maximum_entry
        );
        assert!(minimum_entry < spec.maximum_entry - 2.0);
    }
}

#[cfg(test)]
mod solver_acceptance_tests {
    use super::*;

    #[test]
    fn half_loop_falls_back_to_a_vertical_insert() {
        let spec = ProfileSpec::half_loop(31.3, 4.2, 1.5).named("fallback half loop");
        let profile =
            snap_half_loop_with_hold_range(&spec, 120.0_f64.to_radians(), 120.0_f64.to_radians());
        let [x, z] = profile.endpoint();
        eprintln!("half-loop snapping used a vertical insert; exit ({x}, {z})");
        assert!(profile.maximum_force(31.3) <= 4.2 + 1e-6);
        assert!((x / 2.0 - (x / 2.0).round()).abs() < 1e-9);
        assert!((z / 2.0 - (z / 2.0).round()).abs() < 1e-9);
        assert!(
            profile
                .points
                .windows(2)
                .any(|points| (points[0].angle - PI / 2.0).abs() < 1e-9
                    && (points[1].angle - PI / 2.0).abs() < 1e-9)
        );
    }

    #[test]
    fn rated_speeds_validate_the_window_and_radius() {
        let spec = ProfileSpec::half_loop(31.3, 4.2, 1.5);
        let profile = snap_half_loop(&spec);
        let speeds = profile.rated_speeds("half loop", 31.3, 1.5, 8.0);
        assert_eq!(speeds.maximum_entry, 31.3);
        assert!((speeds.minimum_entry - profile.minimum_entry()).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "test piece: minimum radius")]
    fn rated_speeds_reject_invalid_radius() {
        let profile = solve(&ProfileSpec::half_loop(31.3, 4.2, 1.5));
        profile.rated_speeds("test piece", 31.3, 1.5, 100.0);
    }

    #[test]
    fn pull_profiles_snap_both_connector_coordinates() {
        for start_angle in [0.0, PI / 4.0] {
            let spec = ProfileSpec::pull(start_angle, PI / 3.0, 34.0, 4.2);
            let profile = snap_pull(&spec);
            for coordinate in profile.endpoint() {
                assert!((coordinate / 2.0 - (coordinate / 2.0).round()).abs() < 1e-9);
            }
            assert!(profile.maximum_force(34.0) <= 4.2 + 1e-6);
        }
    }

    #[test]
    fn large_half_loop_meets_milestone_and_snaps_to_grid() {
        let spec = ProfileSpec::half_loop(31.3, 4.2, 1.5);
        let raw = solve(&spec);
        let end = raw.points.last().unwrap();
        eprintln!(
            "unsnapped x={} z={} length={} radius={} minimum={} force={}",
            end.x,
            end.z,
            end.distance,
            raw.minimum_radius(),
            raw.minimum_entry(),
            raw.maximum_force(31.3)
        );
        assert!((end.x / 27.9 - 1.0).abs() < 0.03);
        assert!((end.z / 38.2 - 1.0).abs() < 0.03);
        assert!((end.distance / 87.1 - 1.0).abs() < 0.03);
        assert!((raw.minimum_radius() / 11.4 - 1.0).abs() < 0.05);
        assert!((raw.minimum_entry() - 28.0).abs() < 0.3);
        assert!(raw.maximum_force(31.3) <= 4.2 + 1e-6);
        let snapped = snap_half_loop(&spec);
        let exit = snapped.points.last().unwrap();
        eprintln!("snapped x={} z={} length={}", exit.x, exit.z, exit.distance);
        assert!((exit.x - 28.0).abs() < 1e-9);
        assert!((exit.z - 38.0).abs() < 1e-9);
        assert!(snapped.maximum_force(31.3) <= 4.2 + 1e-6);
        assert!(31.3 - snapped.minimum_entry() >= 1.5);
        assert!(snapped.minimum_radius() >= 8.5);
    }

    #[test]
    fn mirrored_loop_closes_on_the_grid() {
        let spec = ProfileSpec::full_loop(31.3, 4.2, 1.5);
        let profile = snap_full_loop(&spec);
        let end = profile.points.last().unwrap();
        assert!(end.z.abs() < 1e-9);
        assert!((end.x / 2.0 - (end.x / 2.0).round()).abs() < 1e-9);
        assert!(profile.maximum_force(31.3) <= 4.2 + 1e-6);
    }
}
