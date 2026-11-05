#[cfg(test)]
use std::f64::consts::PI;

use crate::state_hash::StateHasher;
use crate::track::curves::{HEART_HEIGHT_METRES, RatedSpeeds};
#[cfg(test)]
const HEART_HEIGHT: f64 = HEART_HEIGHT_METRES;

const GRAVITY: f64 = 9.81;

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
enum Schedule {
    Constant(f64),
    Loop { bottom: f64, top: f64, hold: f64 },
}

#[cfg(test)]
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

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
enum Segment {
    Arc { angle: f64, schedule: Schedule },
    Insert(f64),
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct ProfileSpec {
    segments: Vec<Segment>,
    maximum_entry: f64,
    fixed_speed: bool,
    mirror_apex: bool,
    start_angle: f64,
    taper_angle: Option<f64>,
    name: &'static str,
}

#[cfg(test)]
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
            taper_angle: None,
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
            taper_angle: None,
            name: "pull profile",
        }
    }
    pub(crate) fn crest(
        start_angle: f64,
        end_angle: f64,
        maximum_entry: f64,
        minimum_force: f64,
    ) -> Self {
        assert!(end_angle < start_angle);
        assert!(minimum_force < 0.0);
        Self {
            segments: vec![Segment::Arc {
                angle: end_angle - start_angle,
                schedule: Schedule::Constant(minimum_force),
            }],
            maximum_entry,
            fixed_speed: false,
            mirror_apex: false,
            start_angle,
            taper_angle: None,
            name: "crest profile",
        }
    }
    pub(crate) fn full_loop(maximum_entry: f64, bottom: f64, top: f64) -> Self {
        Self {
            mirror_apex: true,
            name: "full loop",
            ..Self::half_loop(maximum_entry, bottom, top)
        }
    }
    pub(crate) fn hill(
        design_speed: f64,
        pull_force: f64,
        crest_force: f64,
        flank_angle: f64,
    ) -> Self {
        assert!(design_speed > 0.0);
        assert!(flank_angle > 0.0 && flank_angle < PI / 2.0);
        Self {
            segments: vec![
                Segment::Arc {
                    angle: flank_angle,
                    schedule: Schedule::Constant(pull_force),
                },
                Segment::Arc {
                    angle: -flank_angle,
                    schedule: Schedule::Constant(crest_force),
                },
            ],
            maximum_entry: design_speed,
            fixed_speed: false,
            mirror_apex: true,
            start_angle: 0.0,
            taper_angle: None,
            name: "airtime hill",
        }
    }
    pub(crate) fn zero_g_hill(design_speed: f64, pull_force: f64, flank_angle: f64) -> Self {
        Self::hill(design_speed, pull_force, 0.0, flank_angle).named("zero-g hill")
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
            taper_angle: None,
            name: "test arc",
        }
    }
    pub(crate) fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }
    pub(crate) fn with_taper_angle(mut self, taper_angle: f64) -> Self {
        assert!(taper_angle > 0.0 && taper_angle <= PI / 4.0);
        self.taper_angle = Some(taper_angle);
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
    cached_minimum_entry: Option<f64>,
    cached_minimum_radius: Option<f64>,
}

include!("profiles.generated.rs");
include!("hills.generated.rs");

#[derive(Clone, Copy, Debug)]
pub(super) struct GeneratedHillProfile {
    pub(super) name: &'static str,
    pub(super) minimum_entry: f64,
    pub(super) minimum_radius: f64,
    pub(super) length: f64,
    pub(super) endpoint: [f64; 2],
    pub(super) point_count: usize,
    // The first two values are per-sample position deltas, accumulated in f64 at load time.
    pub(super) values: &'static [[f32; 4]],
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct GeneratedHillDesign {
    pub(super) name: &'static str,
    pub(super) design_speed: f64,
    pub(super) flank_angle: f64,
    pub(super) insert: f64,
    pub(super) crest_start: f64,
    pub(super) crest_end: f64,
}

pub(crate) fn embedded_profile(name: &str) -> SolvedProfile {
    if let Some(profile) = GENERATED_HILL_PROFILES
        .iter()
        .find(|profile| profile.name == name)
    {
        return embedded_hill_profile_from_values(
            profile.minimum_entry,
            profile.minimum_radius,
            profile.length,
            profile.endpoint,
            profile.point_count,
            profile.values,
        );
    }
    let (_, minimum_entry, minimum_radius, length, values) = GENERATED_PROFILES
        .iter()
        .find(|(profile_name, _, _, _, _)| *profile_name == name)
        .unwrap_or_else(|| panic!("missing generated profile {name}"));
    embedded_profile_from_values(
        *minimum_entry,
        *minimum_radius,
        *length,
        values.len(),
        values,
    )
}

fn embedded_hill_profile_from_values<T>(
    minimum_entry: f64,
    minimum_radius: f64,
    length: f64,
    endpoint: [f64; 2],
    point_count: usize,
    values: &[[T; 4]],
) -> SolvedProfile
where
    T: Copy + Into<f64>,
{
    assert_eq!(point_count, values.len());
    let mut x = 0.0;
    let mut z = 0.0;
    let mut points: Vec<Point> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            x += value[0].into();
            z += value[1].into();
            let curvature: f64 = value[3].into();
            Point {
                distance: length * index as f64 / (point_count - 1) as f64,
                x,
                z,
                angle: value[2].into(),
                heart_curvature: curvature / (1.0 - HEART_HEIGHT_METRES * curvature),
                curvature,
            }
        })
        .collect();
    let last = points.last_mut().expect("hill profile has points");
    last.x = endpoint[0];
    last.z = endpoint[1];
    SolvedProfile {
        points,
        cached_minimum_entry: Some(minimum_entry),
        cached_minimum_radius: Some(minimum_radius),
    }
}

fn embedded_profile_from_values<T>(
    minimum_entry: f64,
    minimum_radius: f64,
    length: f64,
    point_count: usize,
    values: &[[T; 4]],
) -> SolvedProfile
where
    T: Copy + Into<f64>,
{
    assert_eq!(point_count, values.len());
    let points = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let curvature = value[3].into();
            Point {
                distance: length * index as f64 / (point_count - 1) as f64,
                x: value[0].into(),
                z: value[1].into(),
                angle: value[2].into(),
                heart_curvature: curvature / (1.0 - HEART_HEIGHT_METRES * curvature),
                curvature,
            }
        })
        .collect();
    SolvedProfile {
        points,
        cached_minimum_entry: Some(minimum_entry),
        cached_minimum_radius: Some(minimum_radius),
    }
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
        if let Some(minimum_radius) = self.cached_minimum_radius {
            return minimum_radius;
        }
        self.points
            .iter()
            .map(|p| p.curvature.abs())
            .fold(0.0_f64, f64::max)
            .recip()
    }
    pub(crate) fn minimum_entry(&self) -> f64 {
        if let Some(minimum_entry) = self.cached_minimum_entry {
            return minimum_entry;
        }
        let max_mean_height_during_piece = self
            .points
            .iter()
            .map(|p| maximum_mean_height(self, p.distance))
            .fold(f64::NEG_INFINITY, f64::max);
        let minimum_initial_mean = [6, 12]
            .into_iter()
            .map(|cars| train_mean_height(self, 0.0, cars))
            .fold(f64::INFINITY, f64::min);
        let maximum_clearance_mean = [6, 12]
            .into_iter()
            .map(|cars| train_mean_height(self, self.length() + 18.0, cars))
            .fold(f64::NEG_INFINITY, f64::max);
        let height_gain =
            max_mean_height_during_piece.max(maximum_clearance_mean) - minimum_initial_mean;
        (2.0 * GRAVITY * height_gain + 36.0).sqrt()
    }
    #[cfg(test)]
    pub(crate) fn maximum_force(&self, entry: f64) -> f64 {
        self.points
            .iter()
            .map(|p| {
                p.heart_curvature * worst_speed_squared(self, p.distance, entry, false) / GRAVITY
                    + p.angle.cos()
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }
    #[cfg(test)]
    pub(crate) fn minimum_force(&self, entry: f64) -> f64 {
        self.points
            .iter()
            .map(|p| {
                p.heart_curvature * worst_speed_squared(self, p.distance, entry, false) / GRAVITY
                    + p.angle.cos()
            })
            .fold(f64::INFINITY, f64::min)
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

#[cfg(test)]
fn profile_curvature_from_derivatives(tangent: [f64; 2], second: [f64; 2]) -> f64 {
    (tangent[0] * second[1] - tangent[1] * second[0]) / tangent[0].hypot(tangent[1]).powi(3)
}

#[cfg(test)]
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
                let taper_angle = spec.taper_angle.unwrap_or_else(|| {
                    0.3_f64.min(delta.abs() / 4.0 * if delta.abs() < PI / 3.0 { 1.0 } else { 4.0 })
                });
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
                    assert!(
                        speed_squared.is_finite() && speed_squared > 0.0,
                        "{}: non-positive design speed squared at angle {midpoint_angle}",
                        spec.name
                    );
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
                    assert!(
                        end_speed_squared.is_finite() && end_speed_squared > 0.0,
                        "{}: non-positive design speed squared at angle {angle}",
                        spec.name
                    );
                    let heart_curvature =
                        GRAVITY * (schedule.force(angle) - angle.cos()) * end_taper
                            / end_speed_squared;
                    let point = Point {
                        distance,
                        x: heart_x + HEART_HEIGHT * angle.sin(),
                        z: heart_z - HEART_HEIGHT * angle.cos(),
                        angle,
                        heart_curvature,
                        curvature: heart_curvature / (1.0 + HEART_HEIGHT * heart_curvature),
                    };
                    const MINIMUM_POINT_SPACING: f64 = 0.001_000_001;
                    if distance - points.last().unwrap().distance < MINIMUM_POINT_SPACING {
                        if index + 1 == steps {
                            assert!(
                                points.len() > 1,
                                "arc is shorter than minimum point spacing"
                            );
                            points.pop();
                        } else {
                            continue;
                        }
                    }
                    points.push(point);
                }
            }
        }
    }
    SolvedProfile {
        points,
        cached_minimum_entry: None,
        cached_minimum_radius: None,
    }
}

fn train_mean_height(profile: &SolvedProfile, front_distance: f64, cars: usize) -> f64 {
    let spacing = 18.0 / cars as f64;
    (0..cars)
        .map(|car| rail_height(profile, front_distance - car as f64 * spacing, false))
        .sum::<f64>()
        / cars as f64
}

#[cfg(test)]
fn minimum_mean_height(profile: &SolvedProfile, distance: f64, mirror: bool) -> f64 {
    let (six_car_means, twelve_car_means) = train_mean_heights(profile, distance, mirror);
    six_car_means
        .into_iter()
        .chain(twelve_car_means)
        .fold(f64::INFINITY, f64::min)
}

fn maximum_mean_height(profile: &SolvedProfile, distance: f64) -> f64 {
    let (six_car_means, twelve_car_means) = train_mean_heights(profile, distance, false);
    six_car_means
        .into_iter()
        .chain(twelve_car_means)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn train_mean_heights(
    profile: &SolvedProfile,
    front_distance: f64,
    mirror: bool,
) -> ([f64; 6], [f64; 12]) {
    let rail_heights: [f64; 23] = std::array::from_fn(|index| {
        rail_height(
            profile,
            front_distance + (index as f64 - 11.0) * 1.5,
            mirror,
        )
    });
    let mut six_car_means = [0.0; 6];
    for (front_car, mean_height) in six_car_means.iter_mut().enumerate() {
        for other_car in 0..6 {
            let offset = (front_car as isize - other_car as isize) * 2;
            *mean_height += rail_heights[(offset + 11) as usize];
        }
        *mean_height /= 6.0;
    }
    let mut twelve_car_means = [0.0; 12];
    for (front_car, mean_height) in twelve_car_means.iter_mut().enumerate() {
        for other_car in 0..12 {
            let offset = front_car as isize - other_car as isize;
            *mean_height += rail_heights[(offset + 11) as usize];
        }
        *mean_height /= 12.0;
    }
    (six_car_means, twelve_car_means)
}

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn solve(spec: &ProfileSpec) -> SolvedProfile {
    let profile = solve_with_steps(spec, 2048);
    assert_design_force_schedule(spec, &profile);
    profile
}

#[cfg(test)]
fn solve_with_steps(spec: &ProfileSpec, steps: usize) -> SolvedProfile {
    let mut profile = integrate(spec, None, steps);
    for _ in 0..12 {
        let next = integrate(spec, Some(&profile), steps);
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

#[cfg(test)]
fn assert_design_force_schedule(spec: &ProfileSpec, profile: &SolvedProfile) {
    for point in &profile.points {
        let Some(design_force) = schedule_force_at(spec, point.angle) else {
            continue;
        };
        let actual_force = point.heart_curvature
            * worst_speed_squared(
                profile,
                point.distance,
                spec.maximum_entry,
                spec.mirror_apex,
            )
            / GRAVITY
            + point.angle.cos();
        if design_force >= point.angle.cos() {
            assert!(
                actual_force <= design_force + 1e-6,
                "{}: design force exceeded at angle {}: {actual_force} g > {design_force} g",
                spec.name,
                point.angle
            );
        } else {
            assert!(
                actual_force >= design_force - 1e-6,
                "{}: design force fell below target at angle {}: {actual_force} g < {design_force} g",
                spec.name,
                point.angle
            );
        }
    }
}

#[cfg(test)]
fn schedule_force_at(spec: &ProfileSpec, angle: f64) -> Option<f64> {
    let mut segment_start_angle = spec.start_angle;
    for segment in &spec.segments {
        if let Segment::Arc {
            angle: delta,
            schedule,
        } = *segment
        {
            let segment_end_angle = segment_start_angle + delta;
            let lower_angle = segment_start_angle.min(segment_end_angle) - 1e-10;
            let upper_angle = segment_start_angle.max(segment_end_angle) + 1e-10;
            if (lower_angle..=upper_angle).contains(&angle) {
                return Some(schedule.force(angle));
            }
            segment_start_angle = segment_end_angle;
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn snap_half_loop(spec: &ProfileSpec) -> SolvedProfile {
    snap_half_loop_with_hold_range(spec, 100.0_f64.to_radians(), 140.0_f64.to_radians())
}

#[cfg(test)]
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
    let mut lower_height = solve(&adjusted).endpoint()[1];
    if let Segment::Arc {
        schedule: Schedule::Loop { hold, .. },
        ..
    } = &mut adjusted.segments[0]
    {
        *hold = high;
    }
    let mut upper_height = solve(&adjusted).endpoint()[1];
    if !(upper_height <= target_height && target_height <= lower_height) {
        return snap_half_loop_with_vertical_insert(spec);
    }
    let mut converged = (upper_height - target_height).abs() < 1e-10;
    if (lower_height - target_height).abs() < 1e-10 {
        if let Segment::Arc {
            schedule: Schedule::Loop { hold, .. },
            ..
        } = &mut adjusted.segments[0]
        {
            *hold = low;
        }
        converged = true;
    }
    for _ in 0..40 {
        if converged {
            break;
        }
        let secant_hold =
            low + (target_height - lower_height) * (high - low) / (upper_height - lower_height);
        let hold = if (low..high).contains(&secant_hold) {
            secant_hold
        } else {
            (low + high) / 2.0
        };
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
        let height = solve(&adjusted).endpoint()[1];
        if (height - target_height).abs() < 1e-10 {
            converged = true;
            break;
        }
        if height > target_height {
            low = hold;
            lower_height = height;
        } else {
            high = hold;
            upper_height = height;
        }
    }
    assert!(converged, "half loop height refinement did not converge");
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

#[cfg(test)]
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
    let mut lower_height = core.endpoint()[1];
    let mut high = 2.0;
    adjusted.segments[1] = Segment::Insert(high);
    let mut upper_height = solve(&adjusted).endpoint()[1];
    while upper_height < target_height {
        high *= 2.0;
        adjusted.segments[1] = Segment::Insert(high);
        upper_height = solve(&adjusted).endpoint()[1];
    }
    let mut converged = (lower_height - target_height).abs() < 1e-10;
    if converged {
        adjusted.segments[1] = Segment::Insert(0.0);
    }
    for _ in 0..40 {
        if converged {
            break;
        }
        let length =
            low + (target_height - lower_height) * (high - low) / (upper_height - lower_height);
        adjusted.segments[1] = Segment::Insert(length);
        let height = solve(&adjusted).endpoint()[1];
        if (height - target_height).abs() < 1e-10 {
            converged = true;
            break;
        }
        if height < target_height {
            low = length;
            lower_height = height;
        } else {
            high = length;
            upper_height = height;
        }
    }
    assert!(
        converged,
        "half loop vertical insert refinement did not converge"
    );
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

#[cfg(test)]
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

#[cfg(test)]
#[derive(Clone, Debug)]
struct SnappedHill {
    profile: SolvedProfile,
    design_speed: f64,
    flank_angle: f64,
    insert: f64,
    crest_start: f64,
    crest_end: f64,
}

#[cfg(test)]
fn hill_spec_at(spec: &ProfileSpec, design_speed: f64, flank_angle: f64) -> ProfileSpec {
    assert_eq!(
        spec.segments.len(),
        2,
        "{}: hill requires two arcs",
        spec.name
    );
    let [
        Segment::Arc { schedule: pull, .. },
        Segment::Arc {
            schedule: crest, ..
        },
    ] = spec.segments.as_slice()
    else {
        panic!("{}: hill requires pull and crest arcs", spec.name);
    };
    let mut adjusted = ProfileSpec::hill(
        design_speed,
        match pull {
            Schedule::Constant(force) => *force,
            Schedule::Loop { .. } => panic!("{}: hill pull must be constant", spec.name),
        },
        match crest {
            Schedule::Constant(force) => *force,
            Schedule::Loop { .. } => panic!("{}: hill crest must be constant", spec.name),
        },
        flank_angle,
    );
    adjusted.name = spec.name;
    adjusted
}

#[cfg(test)]
fn hill_half(spec: &ProfileSpec, insert: f64, steps: usize) -> SolvedProfile {
    let mut adjusted = spec.clone();
    adjusted.segments.insert(0, Segment::Insert(insert));
    solve_with_steps(&adjusted, steps)
}

#[cfg(test)]
fn hill_crest_start(profile: &SolvedProfile, flank_angle: f64, name: &str) -> f64 {
    profile
        .points
        .iter()
        .find(|point| (point.angle - flank_angle).abs() < 1e-12)
        .unwrap_or_else(|| panic!("{name}: missing pull-to-crest boundary"))
        .distance
}

#[cfg(test)]
fn reflect_hill(half: &SolvedProfile) -> SolvedProfile {
    let apex = *half.points.last().unwrap();
    let mut profile = half.clone();
    profile
        .points
        .extend(half.points.iter().rev().skip(1).map(|point| Point {
            distance: 2.0 * apex.distance - point.distance,
            x: 2.0 * apex.x - point.x,
            z: point.z,
            angle: 2.0 * PI - point.angle,
            heart_curvature: point.heart_curvature,
            curvature: point.curvature,
        }));
    profile.cached_minimum_entry = None;
    profile.cached_minimum_radius = None;
    profile
}

#[cfg(test)]
fn hill_outputs(spec: &ProfileSpec, design_speed: f64, flank_angle: f64, steps: usize) -> [f64; 2] {
    let adjusted = hill_spec_at(spec, design_speed, flank_angle);
    let half = hill_half(&adjusted, 0.0, steps);
    [half.endpoint()[1], 2.0 * half.endpoint()[0]]
}

#[cfg(test)]
fn refine_hill_design(
    spec: &ProfileSpec,
    target_height: f64,
    target_span: f64,
    mut design_speed: f64,
    mut flank_angle: f64,
    steps: usize,
) -> Option<(f64, f64)> {
    const HEIGHT_TOLERANCE: f64 = 1e-10;
    const SPAN_TOLERANCE: f64 = 1e-10;
    const MINIMUM_FLANK_ANGLE: f64 = 10.0_f64.to_radians();
    const MAXIMUM_FLANK_ANGLE: f64 = 55.0_f64.to_radians();
    for _ in 0..24 {
        let [height, span] = hill_outputs(spec, design_speed, flank_angle, steps);
        let height_error = target_height - height;
        let span_error = target_span - span;
        if height_error.abs() <= HEIGHT_TOLERANCE && span_error.abs() <= SPAN_TOLERANCE {
            return Some((design_speed, flank_angle));
        }

        const SPEED_PERTURBATION: f64 = 0.05;
        const ANGLE_PERTURBATION: f64 = 1e-4;
        let [speed_height, speed_span] =
            hill_outputs(spec, design_speed + SPEED_PERTURBATION, flank_angle, steps);
        let [angle_height, angle_span] =
            hill_outputs(spec, design_speed, flank_angle + ANGLE_PERTURBATION, steps);
        let height_speed_derivative = (speed_height - height) / SPEED_PERTURBATION;
        let span_speed_derivative = (speed_span - span) / SPEED_PERTURBATION;
        let height_angle_derivative = (angle_height - height) / ANGLE_PERTURBATION;
        let span_angle_derivative = (angle_span - span) / ANGLE_PERTURBATION;
        let determinant = height_speed_derivative * span_angle_derivative
            - height_angle_derivative * span_speed_derivative;
        assert!(
            determinant.abs() > 1e-12,
            "{}: hill target Jacobian is singular",
            spec.name
        );
        let speed_update = ((height_error * span_angle_derivative
            - height_angle_derivative * span_error)
            / determinant)
            .clamp(-4.0, 4.0);
        let angle_update = ((height_speed_derivative * span_error
            - height_error * span_speed_derivative)
            / determinant)
            .clamp(-0.15, 0.15);
        design_speed += speed_update;
        flank_angle += angle_update;
        assert!(
            (8.0..=60.0).contains(&design_speed),
            "{}: hill design speed left the supported range: {design_speed}",
            spec.name
        );
        if !(MINIMUM_FLANK_ANGLE..=MAXIMUM_FLANK_ANGLE).contains(&flank_angle) {
            return None;
        }
    }

    panic!("{}: hill target solve did not converge", spec.name)
}

#[cfg(test)]
fn hill_design_at_height_and_angle(
    spec: &ProfileSpec,
    target_height: f64,
    flank_angle: f64,
    mut design_speed: f64,
    steps: usize,
) -> (f64, f64) {
    for _ in 0..16 {
        let [height, span] = hill_outputs(spec, design_speed, flank_angle, steps);
        let height_error = target_height - height;
        if height_error.abs() <= 1e-10 {
            return (design_speed, span);
        }
        const SPEED_PERTURBATION: f64 = 0.05;
        let [perturbed_height, _] =
            hill_outputs(spec, design_speed + SPEED_PERTURBATION, flank_angle, steps);
        let derivative = (perturbed_height - height) / SPEED_PERTURBATION;
        design_speed += (height_error / derivative).clamp(-4.0, 4.0);
        assert!(
            (8.0..=60.0).contains(&design_speed),
            "{}: hill design speed left the supported range: {design_speed}",
            spec.name
        );
    }
    panic!("{}: fixed-angle height solve did not converge", spec.name)
}

#[cfg(test)]
fn solve_hill_at_design(
    spec: &ProfileSpec,
    design_speed: f64,
    flank_angle: f64,
    insert: f64,
) -> SnappedHill {
    let adjusted = hill_spec_at(spec, design_speed, flank_angle);
    let half = hill_half(&adjusted, insert, 2048);
    assert_design_force_schedule(&adjusted, &half);
    let crest_start = hill_crest_start(&half, flank_angle, spec.name);
    let crest_end = 2.0 * half.length() - crest_start;
    let profile = reflect_hill(&half);
    SnappedHill {
        profile,
        design_speed,
        flank_angle,
        insert,
        crest_start,
        crest_end,
    }
}

#[cfg(test)]
fn snap_hill(spec: &ProfileSpec, target_height: f64, target_span: f64) -> SnappedHill {
    assert!(target_height > 0.0 && target_span > 0.0);
    let initial_angle = match spec.segments.first() {
        Some(Segment::Arc { angle, .. }) => *angle,
        _ => panic!("{}: hill requires a pull arc", spec.name),
    };
    let coarse_design = refine_hill_design(
        spec,
        target_height,
        target_span,
        spec.maximum_entry,
        initial_angle,
        512,
    );
    let refined_design = coarse_design.and_then(|(design_speed, flank_angle)| {
        refine_hill_design(
            spec,
            target_height,
            target_span,
            design_speed,
            flank_angle,
            2048,
        )
    });
    let (design_speed, flank_angle, insert) =
        if let Some((design_speed, flank_angle)) = refined_design {
            (design_speed, flank_angle, 0.0)
        } else {
            const MINIMUM_FLANK_ANGLE: f64 = 10.0_f64.to_radians();
            const MAXIMUM_FLANK_ANGLE: f64 = 55.0_f64.to_radians();
            let mut candidates = Vec::new();
            for flank_angle in [MINIMUM_FLANK_ANGLE, MAXIMUM_FLANK_ANGLE] {
                let (coarse_speed, _) = hill_design_at_height_and_angle(
                    spec,
                    target_height,
                    flank_angle,
                    spec.maximum_entry,
                    512,
                );
                let (design_speed, core_span) = hill_design_at_height_and_angle(
                    spec,
                    target_height,
                    flank_angle,
                    coarse_speed,
                    2048,
                );
                let total_insert = target_span - core_span;
                if (0.0..=2.0).contains(&total_insert) {
                    candidates.push((total_insert, design_speed, flank_angle));
                }
            }
            candidates.sort_by(|left, right| left.0.total_cmp(&right.0));
            let (total_insert, design_speed, flank_angle) = candidates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("{}: no feasible snapped hill design", spec.name));
            (design_speed, flank_angle, total_insert / 2.0)
        };
    let solved = solve_hill_at_design(spec, design_speed, flank_angle, insert);
    let height = solved
        .profile
        .points
        .iter()
        .map(|point| point.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = solved.profile.endpoint()[0];
    assert!(
        (height - target_height).abs() <= 1e-9,
        "{}: hill height {height} != {target_height}",
        spec.name
    );
    assert!(
        (span - target_span).abs() <= 1e-9,
        "{}: hill span {span} != {target_span}",
        spec.name
    );
    assert!(solved.crest_start < solved.crest_end);
    assert!((0.0..=1.0).contains(&solved.insert));
    assert!(
        design_speed - solved.profile.minimum_entry() >= 2.0,
        "{}: speed window is too narrow",
        spec.name
    );
    solved
}

#[cfg(test)]
fn hill_crest_time(spec: &ProfileSpec, flank_angle: f64, half: &SolvedProfile) -> Option<f64> {
    let crest_start = hill_crest_start(half, flank_angle, spec.name);
    let profile = reflect_hill(half);
    let crest_end = 2.0 * half.length() - crest_start;
    let first_crest_index = profile
        .points
        .partition_point(|point| point.distance < crest_start)
        .min(profile.points.len() - 1);
    let last_crest_index = profile
        .points
        .partition_point(|point| point.distance < crest_end)
        .min(profile.points.len() - 1);
    let mut durations = Vec::with_capacity(2);
    for cars in [6, 12] {
        let mut duration = 0.0;
        for pair in profile.points[first_crest_index..=last_crest_index].windows(2) {
            let midpoint_distance = (pair[0].distance + pair[1].distance) / 2.0;
            let train_mean_height = train_mean_height(&profile, midpoint_distance, cars);
            let speed_squared = spec.maximum_entry.powi(2) - 2.0 * GRAVITY * train_mean_height;
            if !speed_squared.is_finite() || speed_squared <= 0.0 {
                return None;
            }
            duration += (pair[1].distance - pair[0].distance) / speed_squared.sqrt();
        }
        durations.push(duration);
    }
    // The roll-rate limit must hold for the fastest supported train layout.
    durations.into_iter().reduce(f64::min)
}

#[cfg(test)]
fn solve_zero_g_hill_at_design(
    spec: &ProfileSpec,
    design_speed: f64,
    flank_angle: f64,
    insert: f64,
) -> SnappedHill {
    solve_hill_at_design(spec, design_speed, flank_angle, insert)
}

#[cfg(test)]
fn snap_zero_g_hill(spec: &ProfileSpec, target_crest_time: f64) -> SnappedHill {
    assert!(target_crest_time > 0.0);
    const MINIMUM_FLANK_ANGLE: f64 = 10.0_f64.to_radians();
    const MAXIMUM_FLANK_ANGLE: f64 = 55.0_f64.to_radians();
    let duration_at = |angle: f64, steps: usize| {
        let adjusted = hill_spec_at(spec, spec.maximum_entry, angle);
        let half = hill_half(&adjusted, 0.0, steps);
        hill_crest_time(&adjusted, angle, &half).unwrap_or(f64::INFINITY)
    };
    let duration_at_minimum = duration_at(MINIMUM_FLANK_ANGLE, 512);
    let duration_at_maximum = duration_at(MAXIMUM_FLANK_ANGLE, 512);
    assert!(
        (duration_at_minimum..=duration_at_maximum).contains(&target_crest_time),
        "{}: requested crest time {target_crest_time} s is outside {}..{} s",
        spec.name,
        duration_at_minimum,
        duration_at_maximum
    );
    let mut low = MINIMUM_FLANK_ANGLE;
    let mut high = MAXIMUM_FLANK_ANGLE;
    for _ in 0..40 {
        let flank_angle = (low + high) / 2.0;
        if duration_at(flank_angle, 512) < target_crest_time {
            low = flank_angle;
        } else {
            high = flank_angle;
        }
    }
    low = MINIMUM_FLANK_ANGLE;
    high = MAXIMUM_FLANK_ANGLE;
    for _ in 0..40 {
        let flank_angle = (low + high) / 2.0;
        if duration_at(flank_angle, 2048) < target_crest_time {
            low = flank_angle;
        } else {
            high = flank_angle;
        }
    }
    let flank_angle = (low + high) / 2.0;
    let adjusted = hill_spec_at(spec, spec.maximum_entry, flank_angle);
    let half = hill_half(&adjusted, 0.0, 2048);
    let crest_time = hill_crest_time(&adjusted, flank_angle, &half).unwrap();
    assert!((crest_time - target_crest_time).abs() <= 1e-8);
    let core_span = 2.0 * half.endpoint()[0];
    let nearest_grid_span = (core_span / 2.0).round() * 2.0;
    let snapped_span = if (nearest_grid_span - core_span).abs() <= 1e-9 {
        nearest_grid_span
    } else {
        (core_span / 2.0).ceil() * 2.0
    };
    let total_insert = snapped_span - core_span;
    assert!((0.0..2.0).contains(&total_insert));
    let insert = total_insert / 2.0;
    let solved = solve_zero_g_hill_at_design(spec, spec.maximum_entry, flank_angle, insert);
    let half_with_insert = hill_half(&adjusted, insert, 2048);
    let snapped_crest_time = hill_crest_time(&adjusted, flank_angle, &half_with_insert).unwrap();
    assert!((snapped_crest_time - target_crest_time).abs() <= 1e-8);
    assert!((solved.profile.endpoint()[0] - snapped_span).abs() <= 1e-9);
    solved
}

#[cfg(test)]
pub(crate) fn snap_pull(spec: &ProfileSpec) -> SolvedProfile {
    let Segment::Arc { angle, .. } = spec.segments[0] else {
        panic!("pull requires one arc");
    };
    assert_eq!(spec.segments.len(), 1);
    let end_angle = spec.start_angle + angle;
    let determinant = (end_angle - spec.start_angle).sin();
    assert!(determinant.abs() > f64::EPSILON);
    let raw = solve_with_steps(spec, 512);
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
            let mut coarse_converged = false;
            for _ in 0..6 {
                adjusted.segments = vec![
                    Segment::Insert(start),
                    spec.segments[0],
                    Segment::Insert(end),
                ];
                let profile = solve_with_steps(&adjusted, 512);
                let [actual_x, actual_z] = profile.endpoint();
                let dx = 2.0 * x as f64 - actual_x;
                let dz = 2.0 * z as f64 - actual_z;
                if dx.hypot(dz) < 1e-7 {
                    coarse_converged = true;
                    break;
                }
                const START_PERTURBATION: f64 = 0.1;
                adjusted.segments[0] = Segment::Insert(start + START_PERTURBATION);
                let perturbed = solve_with_steps(&adjusted, 512);
                let [perturbed_x, perturbed_z] = perturbed.endpoint();
                let start_derivative_x = (perturbed_x - actual_x) / START_PERTURBATION;
                let start_derivative_z = (perturbed_z - actual_z) / START_PERTURBATION;
                let end_tangent_x = end_angle.cos();
                let end_tangent_z = end_angle.sin();
                let jacobian_determinant =
                    start_derivative_x * end_tangent_z - end_tangent_x * start_derivative_z;
                if jacobian_determinant.abs() < f64::EPSILON {
                    break;
                }
                start += (dx * end_tangent_z - end_tangent_x * dz) / jacobian_determinant;
                end += (start_derivative_x * dz - start_derivative_z * dx) / jacobian_determinant;
                if start < 0.0 || end < 0.0 {
                    break;
                }
            }
            if !coarse_converged {
                continue;
            }
            for _ in 0..6 {
                adjusted.segments = vec![
                    Segment::Insert(start),
                    spec.segments[0],
                    Segment::Insert(end),
                ];
                let mut profile = solve_with_steps(&adjusted, 2048);
                let [actual_x, actual_z] = profile.endpoint();
                let dx = 2.0 * x as f64 - actual_x;
                let dz = 2.0 * z as f64 - actual_z;
                if dx.hypot(dz) < 1e-10 {
                    assert_design_force_schedule(&adjusted, &profile);
                    let last = profile.points.last_mut().unwrap();
                    last.x = 2.0 * x as f64;
                    last.z = 2.0 * z as f64;
                    return profile;
                }
                const START_PERTURBATION: f64 = 0.1;
                adjusted.segments[0] = Segment::Insert(start + START_PERTURBATION);
                let perturbed = solve_with_steps(&adjusted, 2048);
                let [perturbed_x, perturbed_z] = perturbed.endpoint();
                let start_derivative_x = (perturbed_x - actual_x) / START_PERTURBATION;
                let start_derivative_z = (perturbed_z - actual_z) / START_PERTURBATION;
                let end_tangent_x = end_angle.cos();
                let end_tangent_z = end_angle.sin();
                let jacobian_determinant =
                    start_derivative_x * end_tangent_z - end_tangent_x * start_derivative_z;
                if jacobian_determinant.abs() < f64::EPSILON {
                    break;
                }
                start += (dx * end_tangent_z - end_tangent_x * dz) / jacobian_determinant;
                end += (start_derivative_x * dz - start_derivative_z * dx) / jacobian_determinant;
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
    use std::fmt::Write as _;

    const GENERATED_PROFILE_POINTS: usize = 512;
    const MAXIMUM_PROFILE_POSITION_ERROR_METRES: f64 = 0.001;
    const MAXIMUM_PROFILE_CURVATURE_ERROR: f64 = 2e-4;

    fn hill_profile_point_count(name: &str) -> usize {
        if name == "ZeroGRollCompact" {
            1024
        } else {
            512
        }
    }

    #[derive(Clone, Copy)]
    struct HillCase {
        name: &'static str,
        height: f64,
        span: f64,
        design_speed: f64,
        flank_angle: f64,
        pull_force: f64,
        crest_force: f64,
        model_radius: f64,
    }

    const HILL_CASES: [HillCase; 14] = [
        HillCase {
            name: "EjectorHill8x48",
            height: 8.0,
            span: 48.0,
            design_speed: 23.0,
            flank_angle: 40.0_f64.to_radians(),
            pull_force: 4.2,
            crest_force: -1.0,
            model_radius: 8.0,
        },
        HillCase {
            name: "EjectorHill12x64",
            height: 12.0,
            span: 64.0,
            design_speed: 26.0,
            flank_angle: 45.0_f64.to_radians(),
            pull_force: 4.2,
            crest_force: -1.05,
            model_radius: 8.0,
        },
        HillCase {
            name: "EjectorHill8x64",
            height: 8.0,
            span: 64.0,
            design_speed: 30.0,
            flank_angle: 30.0_f64.to_radians(),
            pull_force: 4.2,
            crest_force: -1.2,
            model_radius: 8.0,
        },
        HillCase {
            name: "EjectorHill12x80",
            height: 12.0,
            span: 80.0,
            design_speed: 31.0,
            flank_angle: 36.0_f64.to_radians(),
            pull_force: 4.2,
            crest_force: -1.2,
            model_radius: 8.0,
        },
        HillCase {
            name: "EjectorHill16x96",
            height: 16.0,
            span: 96.0,
            design_speed: 33.0,
            flank_angle: 40.0_f64.to_radians(),
            pull_force: 4.2,
            crest_force: -1.2,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill4x48",
            height: 4.0,
            span: 48.0,
            design_speed: 22.0,
            flank_angle: 29.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill8x64",
            height: 8.0,
            span: 64.0,
            design_speed: 26.0,
            flank_angle: 29.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill12x80",
            height: 12.0,
            span: 80.0,
            design_speed: 27.0,
            flank_angle: 35.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill6x64",
            height: 6.0,
            span: 64.0,
            design_speed: 29.0,
            flank_angle: 22.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill12x96",
            height: 12.0,
            span: 96.0,
            design_speed: 31.0,
            flank_angle: 29.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "FloaterHill16x112",
            height: 16.0,
            span: 112.0,
            design_speed: 32.0,
            flank_angle: 33.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "SpeedHill4x64",
            height: 4.0,
            span: 64.0,
            design_speed: 28.0,
            flank_angle: 15.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: 0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "SpeedHill6x80",
            height: 6.0,
            span: 80.0,
            design_speed: 29.0,
            flank_angle: 17.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: 0.3,
            model_radius: 8.0,
        },
        HillCase {
            name: "WoodenEjectorHill6x48",
            height: 6.0,
            span: 48.0,
            design_speed: 24.0,
            flank_angle: 29.0_f64.to_radians(),
            pull_force: 3.7,
            crest_force: -0.7,
            model_radius: 5.0,
        },
    ];

    #[derive(Clone, Copy)]
    struct CrestTimeCase {
        name: &'static str,
        design_speed: f64,
        flank_angle: f64,
        pull_force: f64,
        target_crest_time: f64,
    }

    const CREST_TIME_CASES: [CrestTimeCase; 3] = [
        CrestTimeCase {
            name: "ZeroGRollFast",
            design_speed: 27.0,
            flank_angle: 32.0_f64.to_radians(),
            pull_force: 4.2,
            target_crest_time: 2.4,
        },
        CrestTimeCase {
            name: "ZeroGRollCompact",
            design_speed: 20.0,
            flank_angle: 40.0_f64.to_radians(),
            pull_force: 4.2,
            target_crest_time: 2.4,
        },
        CrestTimeCase {
            name: "Stall",
            design_speed: 27.0,
            flank_angle: 45.0_f64.to_radians(),
            pull_force: 4.2,
            target_crest_time: 3.0,
        },
    ];

    #[derive(Clone, Debug)]
    struct HillTableEntry {
        name: &'static str,
        profile: SolvedProfile,
        design_speed: f64,
        flank_angle: f64,
        insert: f64,
        crest_start: f64,
        crest_end: f64,
    }

    fn catalog_profiles() -> Vec<(&'static str, SolvedProfile)> {
        vec![
            (
                "HalfLoop",
                snap_half_loop(&ProfileSpec::half_loop(31.3, 4.2, 1.5)),
            ),
            (
                "Loop",
                snap_full_loop(&ProfileSpec::full_loop(27.5, 4.2, 1.5).named("Loop")),
            ),
            (
                "LargeLoop",
                snap_full_loop(&ProfileSpec::full_loop(31.3, 4.2, 1.5).named("LargeLoop")),
            ),
            (
                "SteelLoop",
                snap_full_loop(&ProfileSpec::full_loop(27.0, 3.7, 1.0).named("SteelLoop")),
            ),
            (
                "SixtyPull",
                snap_pull(&ProfileSpec::pull(0.0, PI / 3.0, 34.0, 4.2).named("LevelToSixtyUp")),
            ),
            (
                "SixtyCrest",
                snap_pull(
                    &ProfileSpec::crest(PI / 3.0, 0.0, 30.0, -1.45)
                        .with_taper_angle(0.1)
                        .named("SixtyUpToLevel"),
                ),
            ),
            (
                "FifteenPull",
                snap_pull(
                    &ProfileSpec::pull(PI / 4.0, PI / 3.0, 34.0, 4.2).named("SteepToSixtyUp"),
                ),
            ),
        ]
    }

    fn hill_spec(case: HillCase) -> ProfileSpec {
        ProfileSpec::hill(
            case.design_speed,
            case.pull_force,
            case.crest_force,
            case.flank_angle,
        )
        .named(case.name)
    }

    fn crest_time_spec(case: CrestTimeCase) -> ProfileSpec {
        ProfileSpec::zero_g_hill(case.design_speed, case.pull_force, case.flank_angle)
            .named(case.name)
    }

    fn hill_template(name: &str) -> ProfileSpec {
        if let Some(case) = HILL_CASES.iter().find(|case| case.name == name) {
            hill_spec(*case)
        } else {
            let case = CREST_TIME_CASES
                .iter()
                .find(|case| case.name == name)
                .unwrap_or_else(|| panic!("missing hill case {name}"));
            crest_time_spec(*case)
        }
    }

    fn hill_entries_from_generated_designs() -> Vec<HillTableEntry> {
        GENERATED_HILL_DESIGNS
            .iter()
            .map(|design| {
                let table = GENERATED_HILL_PROFILES
                    .iter()
                    .find(|table| table.name == design.name)
                    .unwrap();
                let spec = hill_template(design.name);
                let solved = solve_hill_at_design(
                    &spec,
                    design.design_speed,
                    design.flank_angle,
                    design.insert,
                );
                assert_eq!(table.point_count, hill_profile_point_count(design.name));
                assert_eq!(
                    table.values.len(),
                    table.point_count,
                    "{} table length",
                    design.name
                );
                assert!(
                    (table.minimum_entry - solved.profile.minimum_entry()).abs() < 1e-12,
                    "{} minimum entry",
                    design.name
                );
                assert!(
                    (table.minimum_radius - solved.profile.minimum_radius()).abs() < 1e-12,
                    "{} minimum radius",
                    design.name
                );
                assert!(
                    (table.length - solved.profile.length()).abs() < 1e-12,
                    "{} length",
                    design.name
                );
                assert!((design.crest_start - solved.crest_start).abs() < 1e-12);
                assert!((design.crest_end - solved.crest_end).abs() < 1e-12);
                HillTableEntry {
                    name: design.name,
                    profile: solved.profile,
                    design_speed: design.design_speed,
                    flank_angle: design.flank_angle,
                    insert: design.insert,
                    crest_start: design.crest_start,
                    crest_end: design.crest_end,
                }
            })
            .collect()
    }

    fn generated_hill_source(entries: &[HillTableEntry]) -> String {
        let mut output = String::from(
            "// Generated by the hill_profiles_are_current test.\n#[allow(clippy::approx_constant)]\npub(super) const GENERATED_HILL_PROFILES: &[GeneratedHillProfile] = &[\n",
        );
        for entry in entries {
            let profile = &entry.profile;
            let point_count = hill_profile_point_count(entry.name);
            let mut previous_position = [0.0; 2];
            writeln!(
                output,
            "    GeneratedHillProfile {{ name: {:?}, minimum_entry: {:?}, minimum_radius: {:?}, length: {:?}, endpoint: {:?}, point_count: {}, values: &[",
                entry.name,
                profile.minimum_entry(),
                profile.minimum_radius(),
                profile.length(),
                profile.endpoint(),
                point_count,
            )
            .unwrap();
            for index in 0..point_count {
                let distance = profile.length() * index as f64 / (point_count - 1) as f64;
                let (position, tangent, _) = profile.sample(distance);
                let position_delta = [
                    position[0] - previous_position[0],
                    position[1] - previous_position[1],
                ];
                previous_position = position;
                let curvature = reference_curvature(profile, distance);
                let values = [
                    format!("{:.9e}", position_delta[0]).parse::<f32>().unwrap(),
                    format!("{:.9e}", position_delta[1]).parse::<f32>().unwrap(),
                    format!("{:.9e}", tangent[1].atan2(tangent[0]))
                        .parse::<f32>()
                        .unwrap(),
                    format!("{:.9e}", curvature).parse::<f32>().unwrap(),
                ];
                writeln!(
                    output,
                    "        [{:?}f32, {:?}f32, {:?}f32, {:?}f32],",
                    values[0], values[1], values[2], values[3]
                )
                .unwrap();
            }
            output.push_str("    ] },\n");
        }
        output.push_str("];\n\n#[cfg(test)]\n#[allow(clippy::approx_constant)]\npub(super) const GENERATED_HILL_DESIGNS: &[GeneratedHillDesign] = &[\n");
        for entry in entries {
            writeln!(
                output,
                "    GeneratedHillDesign {{ name: {:?}, design_speed: {:?}, flank_angle: {:?}, insert: {:?}, crest_start: {:?}, crest_end: {:?} }},",
                entry.name,
                entry.design_speed,
                entry.flank_angle,
                entry.insert,
                entry.crest_start,
                entry.crest_end,
            )
            .unwrap();
        }
        output.push_str("];\n");
        output
    }

    #[test]
    fn catalog_profiles_have_no_intervals_shorter_than_one_millimetre() {
        for (name, profile) in catalog_profiles() {
            for points in profile.points.windows(2) {
                let interval = points[1].distance - points[0].distance;
                assert!(interval >= 0.001, "{name}: interval {interval}");
            }
        }
    }

    fn reference_curvature(profile: &SolvedProfile, distance: f64) -> f64 {
        let index = profile
            .points
            .partition_point(|point| point.distance < distance)
            .clamp(1, profile.points.len() - 1);
        let start = profile.points[index - 1];
        let end = profile.points[index];
        let fraction = (distance - start.distance) / (end.distance - start.distance);
        start.curvature + fraction * (end.curvature - start.curvature)
    }

    fn generated_source(profiles: &[(&str, SolvedProfile)]) -> String {
        let mut output = String::from(
            "// Generated by the solved_profiles_are_current test.\n#[allow(clippy::excessive_precision, clippy::approx_constant, clippy::type_complexity)]\npub(super) const GENERATED_PROFILES: &[(&str, f64, f64, f64, &[[f64; 4]])] = &[\n",
        );
        for (name, profile) in profiles {
            writeln!(
                output,
                "    ({name:?}, {:.17}, {:.17}, {:.17}, &[",
                profile.minimum_entry(),
                profile.minimum_radius(),
                profile.length()
            )
            .unwrap();
            for index in 0..GENERATED_PROFILE_POINTS {
                let distance =
                    profile.length() * index as f64 / (GENERATED_PROFILE_POINTS - 1) as f64;
                let (position, tangent, _) = profile.sample(distance);
                let curvature = reference_curvature(profile, distance);
                writeln!(
                    output,
                    "        [{:.17}, {:.17}, {:.17}, {:.17}],",
                    position[0],
                    position[1],
                    tangent[1].atan2(tangent[0]),
                    curvature
                )
                .unwrap();
            }
            output.push_str("    ]),\n");
        }
        output.push_str("];\n");
        output
    }

    #[test]
    fn solved_profiles_are_current() {
        let profiles = catalog_profiles();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/track/profiles.generated.rs"
        );
        if std::env::var_os("UPDATE_SOLVED_PROFILES").is_some() {
            std::fs::write(path, generated_source(&profiles)).unwrap();
            return;
        }
        for (name, expected) in profiles {
            let (_, generated_minimum_entry, generated_minimum_radius, _, values) =
                GENERATED_PROFILES
                    .iter()
                    .find(|(profile_name, _, _, _, _)| *profile_name == name)
                    .unwrap();
            assert_eq!(values.len(), GENERATED_PROFILE_POINTS, "{name} point count");
            let actual = embedded_profile(name);
            assert!(
                (generated_minimum_entry - expected.minimum_entry()).abs() < 1e-12,
                "{name} minimum entry"
            );
            assert!(
                (generated_minimum_radius - expected.minimum_radius()).abs() < 1e-12,
                "{name} minimum radius"
            );
            let mut maximum_position_error = 0.0_f64;
            let mut maximum_curvature_error = 0.0_f64;
            let mut check = |distance: f64| {
                let (expected_position, _, _) = expected.sample(distance);
                let (actual_position, tangent, second) = actual.sample(distance);
                let position_error = (expected_position[0] - actual_position[0])
                    .hypot(expected_position[1] - actual_position[1]);
                let curvature_error = (reference_curvature(&expected, distance)
                    - profile_curvature_from_derivatives(tangent, second))
                .abs();
                maximum_position_error = maximum_position_error.max(position_error);
                maximum_curvature_error = maximum_curvature_error.max(curvature_error);
                assert!(
                    position_error <= MAXIMUM_PROFILE_POSITION_ERROR_METRES,
                    "{name} position error {position_error} at {distance}"
                );
                assert!(
                    curvature_error <= MAXIMUM_PROFILE_CURVATURE_ERROR,
                    "{name} curvature error {curvature_error} at {distance}"
                );
            };
            let length = expected.length();
            for index in 0..=16_381 {
                check(length * index as f64 / 16_381.0);
            }
            for point in expected.points.iter().chain(&actual.points) {
                for offset in [-1e-9, 0.0, 1e-9] {
                    check((point.distance + offset * length).clamp(0.0, length));
                }
            }
            eprintln!(
                "{name}: maximum position error {maximum_position_error} m, maximum curvature error {maximum_curvature_error} 1/m"
            );
        }
    }

    #[test]
    fn hills_solve_to_their_dimensions_and_rate_limits() {
        let mut entries = Vec::with_capacity(HILL_CASES.len() + CREST_TIME_CASES.len());
        for case in HILL_CASES {
            let spec = hill_spec(case);
            let solved = snap_hill(&spec, case.height, case.span);
            let height = solved
                .profile
                .points
                .iter()
                .map(|point| point.z)
                .fold(f64::NEG_INFINITY, f64::max);
            let span = solved.profile.endpoint()[0];
            let rated =
                solved
                    .profile
                    .rated_speeds(case.name, solved.design_speed, 2.0, case.model_radius);
            assert!(
                (height - case.height).abs() <= 1e-9,
                "{} height {height}",
                case.name
            );
            assert!(
                (span - case.span).abs() <= 1e-9,
                "{} span {span}",
                case.name
            );
            assert!(
                (0.0..=1.0).contains(&solved.insert),
                "{} equal connector inserts: {} m",
                case.name,
                solved.insert
            );
            assert!(
                solved.crest_start < solved.crest_end,
                "{} crest range",
                case.name
            );
            assert!(
                (span / 2.0 - (span / 2.0).round()).abs() < 1e-9,
                "{} grid span",
                case.name
            );
            eprintln!(
                "{}: height {:.6} m, span {:.6} m, flank {:.3} deg, rated {:.3}..{:.3} m/s, minimum radius {:.3} m",
                case.name,
                height,
                span,
                solved.flank_angle.to_degrees(),
                rated.minimum_entry,
                rated.maximum_entry,
                solved.profile.minimum_radius()
            );
            entries.push(HillTableEntry {
                name: case.name,
                profile: solved.profile,
                design_speed: solved.design_speed,
                flank_angle: solved.flank_angle,
                insert: solved.insert,
                crest_start: solved.crest_start,
                crest_end: solved.crest_end,
            });
        }

        const MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND: f64 = 200.0;
        const ROLL_SCHEDULE_PEAK_FACTOR: f64 = 4.0 / 3.0;
        for case in CREST_TIME_CASES {
            let spec = crest_time_spec(case);
            let solved = snap_zero_g_hill(&spec, case.target_crest_time);
            let adjusted = hill_spec_at(&spec, solved.design_speed, solved.flank_angle);
            let half = hill_half(&adjusted, solved.insert, 2048);
            let crest_time = hill_crest_time(&adjusted, solved.flank_angle, &half).unwrap();
            let peak_roll_rate = 360.0 * ROLL_SCHEDULE_PEAK_FACTOR / crest_time;
            let rated = solved
                .profile
                .rated_speeds(case.name, solved.design_speed, 2.0, 8.0);
            assert!(
                (crest_time - case.target_crest_time).abs() <= 1e-8,
                "{} crest time {crest_time}",
                case.name
            );
            assert!(
                peak_roll_rate <= MAXIMUM_ROLL_RATE_DEGREES_PER_SECOND + 1e-6,
                "{} peak roll rate {peak_roll_rate} degrees/s",
                case.name
            );
            assert!(
                (0.0..=1.0).contains(&solved.insert),
                "{} insert {} m",
                case.name,
                solved.insert
            );
            let span = solved.profile.endpoint()[0];
            assert!(
                (span / 2.0 - (span / 2.0).round()).abs() < 1e-9,
                "{} grid span {span}",
                case.name
            );
            eprintln!(
                "{}: height {:.6} m, span {:.6} m, flank {:.3} deg, crest time {:.6} s, roll rate {:.3} deg/s, rated {:.3}..{:.3} m/s, minimum radius {:.3} m",
                case.name,
                solved
                    .profile
                    .points
                    .iter()
                    .map(|point| point.z)
                    .fold(f64::NEG_INFINITY, f64::max),
                solved.profile.endpoint()[0],
                solved.flank_angle.to_degrees(),
                crest_time,
                peak_roll_rate,
                rated.minimum_entry,
                rated.maximum_entry,
                solved.profile.minimum_radius()
            );
            entries.push(HillTableEntry {
                name: case.name,
                profile: solved.profile,
                design_speed: solved.design_speed,
                flank_angle: solved.flank_angle,
                insert: solved.insert,
                crest_start: solved.crest_start,
                crest_end: solved.crest_end,
            });
        }
        if std::env::var_os("UPDATE_SOLVED_PROFILES").is_some() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/track/hills.generated.rs");
            std::fs::write(path, generated_hill_source(&entries)).unwrap();
        }
    }

    #[test]
    fn generated_hill_profiles_end_at_their_connectors() {
        for profile in GENERATED_HILL_PROFILES {
            let [x, z] = embedded_profile(profile.name).endpoint();
            assert!(
                (x - (x / 2.0).round() * 2.0).abs() < 1e-9,
                "{}: endpoint x {x}",
                profile.name
            );
            assert!(z.abs() < 1e-9, "{}: endpoint z {z}", profile.name);
        }
    }

    #[test]
    fn hill_profiles_are_current() {
        assert_eq!(
            GENERATED_HILL_DESIGNS.len(),
            HILL_CASES.len() + CREST_TIME_CASES.len()
        );
        let entries = hill_entries_from_generated_designs();
        let mut acceptance_failures = Vec::new();
        for entry in entries {
            let actual = embedded_profile(entry.name);
            let generated = GENERATED_HILL_PROFILES
                .iter()
                .find(|profile| profile.name == entry.name)
                .unwrap();
            let design = GENERATED_HILL_DESIGNS
                .iter()
                .find(|design| design.name == entry.name)
                .unwrap();
            assert_eq!(
                generated.point_count,
                hill_profile_point_count(entry.name),
                "{} point count",
                entry.name
            );
            assert!((design.design_speed - entry.design_speed).abs() < 1e-12);
            assert!((design.flank_angle - entry.flank_angle).abs() < 1e-12);
            assert!((design.insert - entry.insert).abs() < 1e-12);
            assert!((design.crest_start - entry.crest_start).abs() < 1e-12);
            assert!((design.crest_end - entry.crest_end).abs() < 1e-12);
            let mut previous_position = [0.0; 2];
            let precise_values: Vec<[f64; 4]> = (0..generated.point_count)
                .map(|index| {
                    let distance =
                        generated.length * index as f64 / (generated.point_count - 1) as f64;
                    let (position, tangent, _) = entry.profile.sample(distance);
                    let position_delta = [
                        position[0] - previous_position[0],
                        position[1] - previous_position[1],
                    ];
                    previous_position = position;
                    [
                        position_delta[0],
                        position_delta[1],
                        tangent[1].atan2(tangent[0]),
                        reference_curvature(&entry.profile, distance),
                    ]
                })
                .collect();
            let precise = embedded_hill_profile_from_values(
                generated.minimum_entry,
                generated.minimum_radius,
                generated.length,
                generated.endpoint,
                generated.point_count,
                &precise_values,
            );
            let mut maximum_position_error = 0.0_f64;
            let mut maximum_curvature_error = 0.0_f64;
            let mut maximum_f32_curvature_delta = 0.0_f64;
            let mut check = |distance: f64| {
                let (expected_position, _, _) = entry.profile.sample(distance);
                let (actual_position, tangent, second) = actual.sample(distance);
                let (_, precise_tangent, precise_second) = precise.sample(distance);
                let position_error = (expected_position[0] - actual_position[0])
                    .hypot(expected_position[1] - actual_position[1]);
                let curvature_error = (reference_curvature(&entry.profile, distance)
                    - profile_curvature_from_derivatives(tangent, second))
                .abs();
                let f32_curvature_delta = (profile_curvature_from_derivatives(tangent, second)
                    - profile_curvature_from_derivatives(precise_tangent, precise_second))
                .abs();
                maximum_position_error = maximum_position_error.max(position_error);
                maximum_curvature_error = maximum_curvature_error.max(curvature_error);
                maximum_f32_curvature_delta = maximum_f32_curvature_delta.max(f32_curvature_delta);
            };
            let length = entry.profile.length();
            for index in 0..=16_381 {
                check(length * index as f64 / 16_381.0);
            }
            for point in entry.profile.points.iter().chain(&actual.points) {
                for offset in [-1e-9, 0.0, 1e-9] {
                    check((point.distance + offset * length).clamp(0.0, length));
                }
            }
            eprintln!(
                "{}: maximum position error {:.9} m, maximum curvature error {:.9} 1/m, f32 curvature delta {:.9} 1/m",
                entry.name,
                maximum_position_error,
                maximum_curvature_error,
                maximum_f32_curvature_delta
            );
            if maximum_position_error > MAXIMUM_PROFILE_POSITION_ERROR_METRES {
                acceptance_failures.push(format!(
                    "{} position error {:.9} m",
                    entry.name, maximum_position_error
                ));
            }
            if maximum_curvature_error > MAXIMUM_PROFILE_CURVATURE_ERROR {
                acceptance_failures.push(format!(
                    "{} curvature error {:.9} 1/m",
                    entry.name, maximum_curvature_error
                ));
            }
            assert!(
                maximum_f32_curvature_delta <= 1e-5,
                "{} f32 storage adds {:.9} 1/m geometric curvature error",
                entry.name,
                maximum_f32_curvature_delta
            );
        }
        assert!(
            acceptance_failures.is_empty(),
            "{}",
            acceptance_failures.join(", ")
        );
    }

    #[test]
    fn embedded_curvature_matches_tangent_derivative() {
        for (name, _) in catalog_profiles() {
            let profile = embedded_profile(name);
            for pair in profile.points.windows(2) {
                for fraction in [0.0, 0.0001, 0.25, 0.5, 0.75, 0.9999, 1.0] {
                    let distance =
                        pair[0].distance + fraction * (pair[1].distance - pair[0].distance);
                    let step = 1e-5;
                    let (_, tangent, second) = profile.sample(distance);
                    let (_, before, _) = profile.sample(distance - step);
                    let (_, after, _) = profile.sample(distance + step);
                    let finite_difference = [
                        (after[0] - before[0]) / (2.0 * step),
                        (after[1] - before[1]) / (2.0 * step),
                    ];
                    let reported = profile_curvature_from_derivatives(tangent, second);
                    let measured = profile_curvature_from_derivatives(tangent, finite_difference);
                    assert!(
                        (reported - measured).abs() < 1e-7,
                        "{name}: curvature at {distance}: reported {reported}, tangent derivative {measured}"
                    );
                }
            }
        }
    }

    #[test]
    fn profile_hash_includes_embedded_curvature() {
        let first = embedded_profile("HalfLoop");
        let mut second = first.clone();
        second.points[1].curvature += 1.0e-6;
        let mut first_hash = StateHasher::new();
        first.hash_into(&mut first_hash);
        let mut second_hash = StateHasher::new();
        second.hash_into(&mut second_hash);

        assert_ne!(first_hash.finish(), second_hash.finish());
    }

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
            taper_angle: None,
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
    fn snapped_half_loop_exit_insert_follows_its_tangent() {
        let spec = ProfileSpec::half_loop(31.3, 4.2, 1.5);
        for profile in [
            snap_half_loop(&spec),
            snap_half_loop_with_hold_range(&spec, 120.0_f64.to_radians(), 120.0_f64.to_radians()),
        ] {
            let [before, end] = &profile.points[profile.points.len() - 2..] else {
                unreachable!();
            };
            let distance = end.distance - before.distance;
            let error = (end.x - before.x - distance * before.angle.cos())
                .hypot(end.z - before.z - distance * before.angle.sin());
            assert!(error < 1e-9, "exit insert tangent error: {error}");
        }
    }

    #[test]
    fn minimum_entry_clears_the_whole_train_on_an_uphill_exit() {
        let profile = snap_pull(&ProfileSpec::pull(0.0, PI / 3.0, 34.0, 4.2));
        for cars in [6, 12] {
            let spacing = 18.0 / cars as f64;
            let mean_height = |front: f64| {
                (0..cars)
                    .map(|car| rail_height(&profile, front - car as f64 * spacing, false))
                    .sum::<f64>()
                    / cars as f64
            };
            let speed_squared = profile.minimum_entry().powi(2)
                - 2.0 * GRAVITY * (mean_height(profile.length() + 18.0) - mean_height(0.0));
            assert!(speed_squared >= 36.0 - 1e-6, "{cars} cars: {speed_squared}");
        }
    }

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
    fn descending_crest_profile_respects_its_negative_force_target() {
        let spec = ProfileSpec::crest(PI / 3.0, 0.0, 30.0, -1.45)
            .with_taper_angle(0.1)
            .named("sixty-degree crest");
        let profile = solve(&spec);
        assert!(profile.minimum_force(30.0) >= -1.45 - 1e-6);
        assert!(profile.maximum_force(30.0) <= 1.0 + 1e-6);
        assert!(30.0 - profile.minimum_entry() >= 2.0);
        let snapped = snap_pull(&spec);
        for coordinate in snapped.endpoint() {
            assert!((coordinate / 2.0 - (coordinate / 2.0).round()).abs() < 1e-9);
        }
        assert!(snapped.minimum_force(30.0) >= -1.45 - 1e-6);
        assert!(30.0 - snapped.minimum_entry() >= 2.0);
    }

    #[test]
    #[should_panic(expected = "stalled crest: speed window is too narrow")]
    fn solver_rejects_a_crest_that_stalls_inside_its_design_speed() {
        let profile = solve(&ProfileSpec::crest(PI / 3.0, 0.0, 30.0, -0.7));
        profile.rated_speeds("stalled crest", 30.0, 2.0, 8.0);
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
