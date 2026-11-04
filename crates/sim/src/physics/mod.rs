//! Fixed-step train motion along placed track, in metres, seconds and kilograms.
use crate::state_hash::StateHasher;
use crate::track::{LaunchParameters, Piece, PieceKind, TrackSample, TrainConfiguration, Vec3};

#[cfg(test)]
mod tests;

pub const GRAVITY: f64 = 9.81;
/// 200 Hz keeps integration error small while remaining inexpensive for game simulation.
pub const TIME_STEP: f64 = 0.005;

mod testing;
pub use testing::{TestRun, TestRunRecorder, TestSample};
mod coaster_run;
pub use coaster_run::{CoasterOutcome, CoasterRun};
mod block_operation;
pub use block_operation::BlockOperation;

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
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn piece_index(&self, distance: f64) -> usize {
        self.locate(distance).0
    }
    fn locate(&self, distance: f64) -> (usize, f64) {
        let distance = distance.rem_euclid(self.length);
        let distance = if distance == self.length {
            0.0
        } else {
            distance
        };
        let index = self.starts.partition_point(|start| *start <= distance) - 1;
        (
            index,
            (distance - self.starts[index]).min(self.pieces[index].length()),
        )
    }
    pub fn sample(&self, distance: f64) -> TrackSample {
        let (index, local) = self.locate(distance);
        self.pieces[index].sample(local)
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"Circuit-v1");
        hash.write_len(self.pieces.len());
        for piece in &self.pieces {
            piece.hash_into(hash);
        }
        hash.write_len(self.starts.len());
        for &start in &self.starts {
            hash.write_f64(start);
        }
        hash.write_f64(self.length);
    }
}

/// A small closed circuit for the playable demonstration world.
pub fn demo_circuit() -> Circuit {
    use std::collections::{HashSet, VecDeque};

    use crate::track::{PieceId, TrackState, catalog};
    use PieceId::*;
    let catalog = catalog();
    let mut state = TrackState {
        x: 450,
        y: 500,
        z: 10,
        ..TrackState::origin()
    };
    let start = state;
    let mut pieces = Vec::new();
    for id in [
        Station,
        LevelToGentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUp,
        GentleUpToLevel,
        Straight,
        OneTileStraight,
        OneTileStraight,
        OffsetLeft,
        TurnLargeEntry,
        TurnLargeExit,
        LevelToGentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDown,
        GentleDownToLevel,
        Loop,
    ] {
        let mut piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .expect("demo piece in catalog")
            .at(state)
            .expect("demo piece joins");
        if (1..=6).contains(&pieces.len()) {
            piece.kind = PieceKind::Lift;
        }
        state = piece.exit;
        pieces.push(piece);
    }

    for id in [OffsetRight, OneTileStraight, OneTileStraight] {
        let piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .expect("demo alignment piece in catalog")
            .at(state)
            .expect("demo alignment piece joins");
        state = piece.exit;
        pieces.push(piece);
    }

    let connector_key = |connector: TrackState| {
        (
            connector.x,
            connector.y,
            connector.z,
            connector.heading as u8,
            connector.slope as u8,
            connector.bank as u8,
            connector.horizontal_curvature as u8,
            connector.vertical_curvature as u8,
        )
    };
    let return_options = [
        Straight,
        TurnSmallStandalone,
        TurnSmallRightStandalone,
        TurnLargeStandalone,
        TurnLargeRightStandalone,
        OffsetLeft,
        OffsetRight,
    ];
    let mut queue = VecDeque::from([(state, Vec::<PieceId>::new())]);
    let mut visited = HashSet::from([connector_key(state)]);
    let return_path = loop {
        let (current, path) = queue.pop_front().expect("demo has a level return path");
        if current == start {
            break path;
        }
        if path.len() >= 80 {
            continue;
        }
        for id in return_options {
            let piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .and_then(|piece| piece.at(current))
                .expect("demo return piece joins");
            if piece.exit.x.abs_diff(start.x) > 300 || piece.exit.y.abs_diff(start.y) > 300 {
                continue;
            }
            if !visited.insert(connector_key(piece.exit)) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(id);
            queue.push_back((piece.exit, next_path));
        }
    };
    for id in return_path {
        let piece = catalog
            .iter()
            .find(|piece| piece.id == id)
            .expect("demo return piece in catalog")
            .at(state)
            .expect("demo return piece joins");
        state = piece.exit;
        pieces.push(piece);
    }
    Circuit::new(pieces).expect("demo circuit closes")
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

impl PhysicsConfig {
    pub(crate) fn hash_into(self, hash: &mut StateHasher) {
        hash.write_f64(self.car_mass_kg);
        hash.write_f64(self.rolling_resistance);
        hash.write_f64(self.drag_coefficient);
        hash.write_f64(self.lift_speed);
        hash.write_f64(self.brake_speed);
        hash.write_f64(self.station_speed);
        hash.write_f64(self.drive_acceleration);
        hash.write_f64(self.braking_acceleration);
        hash.write_bool(self.powered_sections);
    }
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
    pub height_m: f64,
    pub inverted: bool,
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
    launch: Option<LaunchParameters>,
    controls: Vec<Option<SectionControl>>,
    readings: Vec<Vec<CarReading>>,
    notes: Vec<Note>,
    rolled_back: bool,
    recovered: bool,
}
impl Train {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"Train-v1");
        hash.write_f64(self.distance);
        hash.write_f64(self.speed);
        hash.write_len(self.cars);
        hash.write_f64(self.spacing);
        self.config.hash_into(hash);
        hash.write_option(self.launch, |hash, launch| {
            hash.write_f64(launch.target_speed);
            hash.write_f64(launch.acceleration);
        });
        hash.write_len(self.controls.len());
        for control in &self.controls {
            hash.write_option(*control, |hash, control| match control {
                SectionControl::Launch {
                    target_speed,
                    acceleration,
                } => {
                    hash.write_u8(0);
                    hash.write_f64(target_speed);
                    hash.write_f64(acceleration);
                }
            });
        }
        hash.write_len(self.readings.len());
        for frame in &self.readings {
            hash.write_len(frame.len());
            for reading in frame {
                hash.write_f64(reading.distance);
                hash.write_f64(reading.speed);
                hash.write_f64(reading.g.vertical);
                hash.write_f64(reading.g.lateral);
                hash.write_f64(reading.g.longitudinal);
                hash.write_bool(reading.airtime);
                hash.write_f64(reading.height_m);
                hash.write_bool(reading.inverted);
            }
        }
        hash.write_len(self.notes.len());
        for note in &self.notes {
            match note {
                Note::RecoveredAfterRollback => hash.write_u8(0),
            }
        }
        hash.write_bool(self.rolled_back);
        hash.write_bool(self.recovered);
    }

    pub fn new(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        distance: f64,
        speed: f64,
    ) -> Self {
        assert!(train.cars > 0 && train.length_metres.is_finite() && train.length_metres > 0.0);
        assert!(distance.is_finite() && speed.is_finite());
        if circuit
            .pieces
            .iter()
            .any(|piece| piece.kind == PieceKind::Launch)
        {
            let launch = train
                .launch
                .expect("launch pieces require model launch parameters");
            assert!(
                launch.target_speed.is_finite()
                    && launch.target_speed > 0.0
                    && launch.acceleration.is_finite()
                    && launch.acceleration > 0.0
            );
        }
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
            launch: train.launch,
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
    pub fn take_readings(&mut self) -> Vec<Vec<CarReading>> {
        std::mem::take(&mut self.readings)
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
    fn gravity_acceleration(&self, circuit: &Circuit, distance: f64) -> f64 {
        let mean_slope = (0..self.cars)
            .map(|index| {
                let (piece, local) = circuit.locate(distance - index as f64 * self.spacing);
                circuit.pieces[piece].tangent_at(local).z
            })
            .sum::<f64>()
            / self.cars as f64;
        -GRAVITY * mean_slope
    }
    fn acceleration_with_gravity(&self, gravity: f64, speed: f64) -> f64 {
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
        self.step_block_brake(circuit, &[], false);
    }
    /// Apply installed block brakes over the given track pieces.
    pub fn step_block_brake(&mut self, circuit: &Circuit, brake_pieces: &[usize], hold: bool) {
        self.step_block_brake_impl(circuit, brake_pieces, hold, true);
    }
    /// Advance the same motion without producing rider force readings.
    pub fn step_block_brake_without_readings(
        &mut self,
        circuit: &Circuit,
        brake_pieces: &[usize],
        hold: bool,
    ) {
        self.step_block_brake_impl(circuit, brake_pieces, hold, false);
    }
    fn step_block_brake_impl(
        &mut self,
        circuit: &Circuit,
        brake_pieces: &[usize],
        hold: bool,
        record_readings: bool,
    ) {
        if self.recovered {
            return;
        }
        let previous_speed = self.speed;
        let resistance = self.config.rolling_resistance * GRAVITY;
        let initial_gravity = self.gravity_acceleration(circuit, self.distance);
        let held_at_rest = previous_speed == 0.0 && initial_gravity.abs() <= resistance;
        let acceleration = if held_at_rest {
            0.0
        } else {
            self.acceleration_with_gravity(initial_gravity, self.speed)
        };
        let mut next_distance =
            self.distance + self.speed * TIME_STEP + 0.5 * acceleration * TIME_STEP * TIME_STEP;
        let estimate = self.speed + acceleration * TIME_STEP;
        let downhill = self.gravity_acceleration(circuit, next_distance);
        let next_acceleration = self.acceleration_with_gravity(downhill, estimate);
        let mut next_speed = self.speed + 0.5 * (acceleration + next_acceleration) * TIME_STEP;
        if downhill.abs() <= resistance && previous_speed * next_speed <= 0.0 {
            next_speed = 0.0;
        }
        let (index, _) = circuit.locate(next_distance);
        if self.config.powered_sections
            && !self.rolled_back
            && !(hold && brake_pieces.contains(&index))
        {
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
                    PieceKind::Launch => {
                        let launch = self
                            .launch
                            .expect("launch parameters validated at construction");
                        next_speed = next_speed.max(
                            (previous_speed + launch.acceleration * TIME_STEP)
                                .min(launch.target_speed),
                        );
                    }
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
        let on_block_brake = brake_pieces.contains(&index) && !self.rolled_back;
        let brake_can_hold = on_block_brake
            && hold
            && self.config.braking_acceleration >= initial_gravity.abs().max(downhill.abs());
        let native_powered_hold = matches!(
            circuit.pieces[index].kind,
            PieceKind::Lift | PieceKind::Launch
        );
        if on_block_brake && (hold || !native_powered_hold) {
            let target = if hold { 0.0 } else { self.config.brake_speed };
            if next_speed > target {
                next_speed =
                    (next_speed - self.config.braking_acceleration * TIME_STEP).max(target);
            } else if next_speed < target {
                let speed_before_drive = next_speed;
                next_speed = (next_speed + self.config.drive_acceleration * TIME_STEP).min(target);
                if !hold && initial_gravity < 0.0 {
                    next_distance += 0.5 * (next_speed - speed_before_drive) * TIME_STEP;
                }
            }
            if brake_can_hold && next_speed <= 0.0 {
                next_distance = if previous_speed == 0.0 {
                    self.distance
                } else {
                    next_distance.max(self.distance)
                };
                next_speed = 0.0;
            }
            if !hold && previous_speed == 0.0 && initial_gravity < 0.0 {
                let net_acceleration =
                    self.config.drive_acceleration + initial_gravity - resistance;
                if net_acceleration > 0.0 {
                    next_distance = self.distance + 0.5 * net_acceleration * TIME_STEP * TIME_STEP;
                    next_speed = net_acceleration * TIME_STEP;
                }
            }
        }
        self.distance = next_distance;
        self.speed = next_speed;
        if previous_speed >= 0.0 && self.speed < 0.0 {
            self.rolled_back = true;
        }
        if record_readings {
            let longitudinal_acceleration = (self.speed - previous_speed) / TIME_STEP;
            self.readings.push(
                (0..self.cars)
                    .map(|car| {
                        let distance = self.distance - car as f64 * self.spacing;
                        let (index, local) = circuit.locate(distance);
                        let track_sample = circuit.pieces[index].sample(local);
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
                            height_m: track_sample.position.z,
                            inverted: track_sample.up.z < 0.0,
                        }
                    })
                    .collect(),
            );
        }
        let stopped_on_slope =
            previous_speed > 0.0 && self.speed == 0.0 && downhill.abs() > 1e-9 && !brake_can_hold;
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
    let step = 0.02_f64.min(piece.length() / 4.0);
    let (heart_tangent, heart_curvature) = if distance < step {
        let next = piece.sample(distance + step).heart;
        let next_next = piece.sample(distance + 2.0 * step).heart;
        (
            (sample.heart * -3.0 + next * 4.0 - next_next) / (2.0 * step),
            (sample.heart - next * 2.0 + next_next) / step.powi(2),
        )
    } else if piece.length() - distance < step {
        let previous = piece.sample(distance - step).heart;
        let previous_previous = piece.sample(distance - 2.0 * step).heart;
        (
            (sample.heart * 3.0 - previous * 4.0 + previous_previous) / (2.0 * step),
            (sample.heart - previous * 2.0 + previous_previous) / step.powi(2),
        )
    } else {
        let previous = piece.sample(distance - step).heart;
        let next = piece.sample(distance + step).heart;
        (
            (next - previous) / (2.0 * step),
            (previous - sample.heart * 2.0 + next) / step.powi(2),
        )
    };
    let force = heart_curvature * speed.powi(2)
        + heart_tangent * acceleration
        + Vec3::new(0.0, 0.0, GRAVITY);
    GForces {
        vertical: force.dot(sample.up) / GRAVITY,
        lateral: force.dot(sample.up.cross(sample.tangent)) / GRAVITY,
        longitudinal: force.dot(sample.tangent) / GRAVITY,
    }
}
