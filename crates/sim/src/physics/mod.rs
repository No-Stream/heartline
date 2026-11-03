//! Fixed-step train motion along placed track, in metres, seconds and kilograms.
use crate::track::{Piece, PieceKind, TrackSample, TrainConfiguration, Vec3};

#[cfg(test)]
mod tests;

pub const GRAVITY: f64 = 9.81;
/// 200 Hz keeps integration error small while remaining inexpensive for game simulation.
pub const TIME_STEP: f64 = 0.005;

#[derive(Clone, Debug)]
pub struct Circuit {
    pieces: Vec<Piece>,
    starts: Vec<f64>,
    length: f64,
}

impl Circuit {
    pub fn new(pieces: Vec<Piece>) -> Result<Self, &'static str> {
        if pieces.is_empty() {
            return Err("circuit requires track pieces");
        }
        if !pieces.iter().any(|piece| piece.kind == PieceKind::Station) {
            return Err("circuit requires a station for recovery");
        }
        for index in 0..pieces.len() {
            if pieces[index].exit != pieces[(index + 1) % pieces.len()].entry {
                return Err("track pieces must join to form a closed circuit");
            }
        }
        let mut starts = Vec::with_capacity(pieces.len());
        let mut length = 0.0;
        for piece in &pieces {
            starts.push(length);
            length += piece.length();
        }
        Ok(Self {
            pieces,
            starts,
            length,
        })
    }
    pub fn length(&self) -> f64 {
        self.length
    }
    fn locate(&self, distance: f64) -> (usize, f64) {
        let distance = distance.rem_euclid(self.length);
        let index = self.starts.partition_point(|start| *start <= distance) - 1;
        (index, distance - self.starts[index])
    }
    pub fn sample(&self, distance: f64) -> TrackSample {
        let (index, local) = self.locate(distance);
        self.pieces[index].sample(local)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicsConfig {
    /// The track model specifies dimensions, not mass; callers choose a mass per loaded car.
    pub car_mass_kg: f64,
    pub rolling_resistance: f64,
    /// Drag force coefficient in kg/m, including air density, drag coefficient and frontal area.
    pub drag_coefficient: f64,
    pub lift_speed: f64,
    pub brake_speed: f64,
    pub station_speed: f64,
    pub drive_acceleration: f64,
    pub braking_acceleration: f64,
    pub powered_sections: bool,
}
impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            car_mass_kg: 1200.0,
            rolling_resistance: 0.005,
            drag_coefficient: 1.0,
            lift_speed: 3.0,
            brake_speed: 4.0,
            station_speed: 2.0,
            drive_acceleration: 2.0,
            braking_acceleration: 3.0,
            powered_sections: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SectionControl {
    Launch {
        target_speed: f64,
        acceleration: f64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GForces {
    pub vertical: f64,
    pub lateral: f64,
    pub longitudinal: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarReading {
    pub distance: f64,
    pub speed: f64,
    pub g: GForces,
    pub airtime: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    RecoveredAfterRollback,
}

#[derive(Clone, Debug)]
pub struct Train {
    distance: f64,
    speed: f64,
    cars: usize,
    spacing: f64,
    config: PhysicsConfig,
    controls: Vec<Option<SectionControl>>,
    readings: Vec<Vec<CarReading>>,
    notes: Vec<Note>,
    rolled_back: bool,
    recovered: bool,
}
impl Train {
    pub fn new(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        distance: f64,
        speed: f64,
    ) -> Self {
        assert!(train.cars > 0 && train.length_metres.is_finite() && train.length_metres > 0.0);
        assert!(distance.is_finite() && speed.is_finite());
        assert!(config.car_mass_kg.is_finite() && config.car_mass_kg > 0.0);
        assert!(
            [
                config.rolling_resistance,
                config.drag_coefficient,
                config.lift_speed,
                config.brake_speed,
                config.station_speed,
                config.drive_acceleration,
                config.braking_acceleration
            ]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        );
        let cars = usize::from(train.cars);
        assert!(train.length_metres < circuit.length());
        Self {
            distance,
            speed,
            cars,
            spacing: train.length_metres / cars as f64,
            config,
            controls: vec![None; circuit.pieces.len()],
            readings: Vec::new(),
            notes: Vec::new(),
            rolled_back: false,
            recovered: false,
        }
    }
    pub fn set_control(&mut self, piece_index: usize, control: Option<SectionControl>) {
        if let Some(SectionControl::Launch {
            target_speed,
            acceleration,
        }) = control
        {
            assert!(
                target_speed.is_finite()
                    && target_speed >= 0.0
                    && acceleration.is_finite()
                    && acceleration > 0.0
            );
        }
        self.controls[piece_index] = control;
    }
    pub fn distance(&self) -> f64 {
        self.distance
    }
    pub fn speed(&self) -> f64 {
        self.speed
    }
    pub fn readings(&self) -> &[Vec<CarReading>] {
        &self.readings
    }
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }
    pub fn mechanical_energy(&self, circuit: &Circuit) -> f64 {
        (0..self.cars)
            .map(|index| {
                let height = circuit
                    .sample(self.distance - index as f64 * self.spacing)
                    .position
                    .z;
                self.config.car_mass_kg * (0.5 * self.speed * self.speed + GRAVITY * height)
            })
            .sum()
    }
    fn acceleration(&self, circuit: &Circuit, distance: f64, speed: f64) -> f64 {
        let mean_slope = (0..self.cars)
            .map(|index| {
                circuit
                    .sample(distance - index as f64 * self.spacing)
                    .tangent
                    .z
            })
            .sum::<f64>()
            / self.cars as f64;
        let gravity = -GRAVITY * mean_slope;
        let rolling = if speed == 0.0 {
            0.0
        } else {
            self.config.rolling_resistance * GRAVITY * speed.signum()
        };
        let drag = self.config.drag_coefficient * speed * speed.abs()
            / (self.config.car_mass_kg * self.cars as f64);
        gravity - rolling - drag
    }
    pub fn step(&mut self, circuit: &Circuit) {
        if self.recovered {
            return;
        }
        let previous_speed = self.speed;
        let resistance = self.config.rolling_resistance * GRAVITY;
        let initial_gravity = self.acceleration(circuit, self.distance, 0.0);
        let held_at_rest = previous_speed == 0.0 && initial_gravity.abs() <= resistance;
        let acceleration = if held_at_rest {
            0.0
        } else {
            self.acceleration(circuit, self.distance, self.speed)
        };
        let next_distance =
            self.distance + self.speed * TIME_STEP + 0.5 * acceleration * TIME_STEP * TIME_STEP;
        let estimate = self.speed + acceleration * TIME_STEP;
        let next_acceleration = self.acceleration(circuit, next_distance, estimate);
        let mut next_speed = self.speed + 0.5 * (acceleration + next_acceleration) * TIME_STEP;
        let downhill = self.acceleration(circuit, next_distance, 0.0);
        if downhill.abs() <= resistance && previous_speed * next_speed <= 0.0 {
            next_speed = 0.0;
        }
        let (index, _) = circuit.locate(next_distance);
        if self.config.powered_sections && !self.rolled_back {
            if let Some(SectionControl::Launch {
                target_speed,
                acceleration,
            }) = self.controls[index]
            {
                next_speed =
                    next_speed.max((previous_speed + acceleration * TIME_STEP).min(target_speed));
            } else {
                match circuit.pieces[index].kind {
                    PieceKind::Lift => next_speed = self.config.lift_speed,
                    PieceKind::Brakes | PieceKind::Station => {
                        let target = if circuit.pieces[index].kind == PieceKind::Brakes {
                            self.config.brake_speed
                        } else {
                            self.config.station_speed
                        };
                        if circuit.pieces[index].kind == PieceKind::Station && next_speed < target {
                            next_speed = (next_speed + self.config.drive_acceleration * TIME_STEP)
                                .min(target);
                        }
                        if next_speed > target {
                            next_speed = (next_speed
                                - self.config.braking_acceleration * TIME_STEP)
                                .max(target);
                        }
                    }
                    PieceKind::Track => {}
                }
            }
        }
        self.distance = next_distance;
        self.speed = next_speed;
        if previous_speed >= 0.0 && self.speed < 0.0 {
            self.rolled_back = true;
        }
        let longitudinal_acceleration = (self.speed - previous_speed) / TIME_STEP;
        self.readings.push(
            (0..self.cars)
                .map(|car| {
                    let distance = self.distance - car as f64 * self.spacing;
                    let (index, local) = circuit.locate(distance);
                    let g = rider_g(
                        &circuit.pieces[index],
                        local,
                        self.speed,
                        longitudinal_acceleration,
                    );
                    CarReading {
                        distance: distance.rem_euclid(circuit.length),
                        speed: self.speed.abs(),
                        g,
                        airtime: g.vertical < 0.0,
                    }
                })
                .collect(),
        );
        let stopped_on_slope = previous_speed > 0.0 && self.speed == 0.0 && downhill.abs() > 1e-9;
        if stopped_on_slope
            || (self.rolled_back
                && (circuit.pieces[index].kind == PieceKind::Station
                    || (previous_speed < 0.0 && self.speed >= 0.0)))
        {
            let station = circuit
                .pieces
                .iter()
                .position(|piece| piece.kind == PieceKind::Station)
                .expect("rollback recovery requires a station");
            self.distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
            self.speed = 0.0;
            self.recovered = true;
            self.notes.push(Note::RecoveredAfterRollback);
        }
    }
    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in self
            .distance
            .to_bits()
            .to_le_bytes()
            .into_iter()
            .chain(self.speed.to_bits().to_le_bytes())
            .chain([u8::from(self.rolled_back), u8::from(self.recovered)])
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        hash
    }
}

fn rider_g(piece: &Piece, distance: f64, speed: f64, acceleration: f64) -> GForces {
    let sample = piece.sample(distance);
    let delta = 0.001_f64.min(piece.length() / 2.0);
    let before = (distance - delta).max(0.0);
    let after = (distance + delta).min(piece.length());
    let curvature_vector =
        (piece.sample(after).tangent - piece.sample(before).tangent) / (after - before);
    let force = curvature_vector * (speed * speed)
        + sample.tangent * acceleration
        + Vec3::new(0.0, 0.0, GRAVITY);
    GForces {
        vertical: force.dot(sample.up) / GRAVITY,
        lateral: force.dot(sample.up.cross(sample.tangent)) / GRAVITY,
        longitudinal: force.dot(sample.tangent) / GRAVITY,
    }
}
