use theme_park_sim::demo_world::demo_world;
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::physics::{Circuit, PhysicsConfig, Train, demo_circuit};
use theme_park_sim::track::supports::{SupportObstacle, generate_wooden_lattice};
use theme_park_sim::track::{
    Piece, SupportStyle, TrackSample, TrainConfiguration, Vec3, hybrid_model,
};
use theme_park_sim::walking::WalkingSimulation;
use theme_park_sim::world::{Direction, Position, Surface, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmSimulation {
    simulation: WalkingSimulation,
    world: World,
    circuit: Circuit,
    train: Train,
    train_configuration: TrainConfiguration,
    physics_remainder: u32,
}

#[wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, agent_count: usize) -> Self {
        let world = demo_world();
        let simulation = WalkingSimulation::with_entrance(
            seed,
            agent_count,
            &world,
            Position {
                x: 218,
                y: 256,
                height: 8,
            },
        );
        let circuit = demo_circuit();
        let train_configuration = hybrid_model().train;
        let train = Train::new(
            &circuit,
            &train_configuration,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            0.0,
            2.0,
        );
        Self {
            simulation,
            world,
            circuit,
            train,
            train_configuration,
            physics_remainder: 0,
        }
    }

    /// Width and height in tiles.
    pub fn world_dimensions(&self) -> Vec<u16> {
        let (width, height) = self.world.dimensions();
        vec![width, height]
    }

    /// Row-major tiles: NW, NE, SE, SW, water present, water height.
    pub fn world_surfaces(&self) -> Vec<i32> {
        let (width, height) = self.world.dimensions();
        let mut result = Vec::with_capacity(usize::from(width) * usize::from(height) * 6);
        for y in 0..height {
            for x in 0..width {
                let surface = self.world.surface(x, y);
                result.extend(surface.corners.map(i32::from));
                result.push(i32::from(surface.water.is_some()));
                result.push(i32::from(surface.water.unwrap_or(0)));
            }
        }
        result
    }

    /// Paths: x, y, height, slope (0 flat, 1 north, 2 east, 3 south, 4 west).
    pub fn world_paths(&self) -> Vec<i32> {
        self.world
            .paths()
            .flat_map(|path| {
                [
                    i32::from(path.position.x),
                    i32::from(path.position.y),
                    i32::from(path.position.height),
                    match path.slope {
                        None => 0,
                        Some(Direction::North) => 1,
                        Some(Direction::East) => 2,
                        Some(Direction::South) => 3,
                        Some(Direction::West) => 4,
                    },
                ]
            })
            .collect()
    }

    /// Carved half-open intervals: x, y, bottom, top.
    pub fn world_carved(&self) -> Vec<i32> {
        self.world
            .carved_spans()
            .flat_map(|(x, y, bottom, top)| {
                [
                    i32::from(x),
                    i32::from(y),
                    i32::from(bottom),
                    i32::from(top),
                ]
            })
            .collect()
    }

    pub fn world_state_hash(&self) -> u64 {
        self.world.state_hash()
    }

    /// Replace a surface and report whether the world changed.
    pub fn set_world_surface(
        &mut self,
        x: u16,
        y: u16,
        corners: Vec<i16>,
        water: Option<i16>,
    ) -> bool {
        let surface = Surface {
            corners: corners.try_into().expect("surface must have four corners"),
            water,
        };
        if self.world.surface(x, y) == surface {
            return false;
        }
        self.world.set_surface(x, y, surface);
        true
    }

    pub fn tick(&mut self) {
        self.simulation.tick(&self.world);
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            self.train.step(&self.circuit);
            self.physics_remainder -= 30;
        }
        self.train.take_readings();
    }

    /// Samples: piece index, position, lateral, up, cumulative distance. Positions and
    /// distance are in tiles (four metres); Z is up and vectors are unit length.
    pub fn coaster_geometry(&self) -> Vec<f32> {
        let mut result = Vec::new();
        let mut start = 0.0;
        for (index, piece) in self.circuit.pieces().iter().enumerate() {
            let intervals = (piece.length() / 2.0).ceil() as usize;
            for sample_index in 0..=intervals {
                let distance = piece.length() * sample_index as f64 / intervals as f64;
                append_pose(&mut result, index, piece.sample(distance));
                result.push(((start + distance) / 4.0) as f32);
            }
            start += piece.length();
        }
        result
    }

    /// Rail segments: piece index and start/end frames (position, lateral, up).
    /// Positions are in tiles; lateral and up are unit vectors.
    pub fn coaster_rails(&self) -> Vec<f32> {
        let mut result = Vec::new();
        for (index, piece) in self.circuit.pieces().iter().enumerate() {
            let intervals = (piece.length() / 2.0).ceil() as usize;
            for sample_index in 0..intervals {
                let start = piece.sample(piece.length() * sample_index as f64 / intervals as f64);
                let end =
                    piece.sample(piece.length() * (sample_index + 1) as f64 / intervals as f64);
                for offset in [-0.72, 0.72] {
                    result.push(index as f32);
                    for sample in [start, end] {
                        let lateral = sample.up.cross(sample.tangent);
                        append_vector(&mut result, (sample.position + lateral * offset) / 4.0);
                        append_vector(&mut result, lateral);
                        append_vector(&mut result, sample.up);
                    }
                }
            }
        }
        result
    }

    /// Cross ties: piece index, endpoints, up, tangent. Positions are in tiles.
    /// Ties are spaced two metres apart over the complete circuit, including joins.
    pub fn coaster_ties(&self) -> Vec<f32> {
        let mut result = Vec::new();
        let mut piece_start = 0.0;
        let mut next_tie = 0_usize;
        for (index, piece) in self.circuit.pieces().iter().enumerate() {
            let piece_end = piece_start + piece.length();
            while (next_tie as f64) * 2.0 < piece_end {
                let sample = piece.sample((next_tie as f64 * 2.0 - piece_start).max(0.0));
                let lateral = sample.up.cross(sample.tangent);
                result.push(index as f32);
                append_vector(&mut result, (sample.position - lateral * 0.88) / 4.0);
                append_vector(&mut result, (sample.position + lateral * 0.88) / 4.0);
                append_vector(&mut result, sample.up);
                append_vector(&mut result, sample.tangent);
                next_tie += 1;
            }
            piece_start = piece_end;
        }
        result
    }

    /// Segments: piece index, start position, end position, in tiles.
    pub fn coaster_supports(&self) -> Vec<f32> {
        let mut result = Vec::new();
        for (index, piece) in self.circuit.pieces().iter().enumerate() {
            let obstacles: Vec<_> = self
                .circuit
                .pieces()
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != index)
                .flat_map(|(_, other)| track_obstacles(other))
                .collect();
            let supports = match hybrid_model().support_style {
                SupportStyle::WoodenLattice => {
                    generate_wooden_lattice(piece, &self.world, &obstacles)
                }
            }
            .expect("demo support cannot be routed");
            for support in supports {
                if support.start != support.end {
                    result.push(index as f32);
                    append_vector(&mut result, support.start / 4.0);
                    append_vector(&mut result, support.end / 4.0);
                }
            }
        }
        result
    }

    /// Cars: car index, position, tangent, up, occupied-seat bitmask.
    pub fn coaster_train_poses(&self) -> Vec<f32> {
        let train = &self.train_configuration;
        let mut result = Vec::new();
        for index in 0..usize::from(train.cars) {
            let distance =
                self.train.distance() - index as f64 * train.length_metres / f64::from(train.cars);
            let sample = self.circuit.sample(distance);
            result.push(index as f32);
            append_vector(&mut result, sample.position / 4.0);
            append_vector(&mut result, sample.tangent);
            append_vector(&mut result, sample.up);
            result.push(
                ((1_u32 << (u32::from(train.rows_per_car) * u32::from(train.seats_per_row))) - 1)
                    as f32,
            );
        }
        result
    }

    pub fn tick_count(&self) -> u64 {
        self.simulation.tick_count()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.simulation
            .guests()
            .iter()
            .flat_map(|guest| {
                [
                    guest.x,
                    guest.y,
                    guest.height,
                    guest.heading_x,
                    guest.heading_y,
                    guest.distance,
                    if guest.moving { 1.0 } else { 0.0 },
                ]
            })
            .collect()
    }

    pub fn guest_ids(&self) -> Vec<u32> {
        self.simulation.traits().ids().map(|id| id.0).collect()
    }

    pub fn guest_appearances(&self) -> Vec<u8> {
        let traits = self.simulation.traits();
        traits
            .ids()
            .flat_map(|id: GuestId| {
                let appearance = traits.appearance(id);
                let age = match traits.age(id) {
                    AgeGroup::Child => 0,
                    AgeGroup::Adult => 1,
                    AgeGroup::Senior => 2,
                };
                [
                    appearance.skin_tone,
                    appearance.hair_style,
                    appearance.clothing_style,
                    appearance.body_shape,
                    age,
                ]
            })
            .collect()
    }

    pub fn state_hash(&self) -> u64 {
        self.simulation.state_hash()
    }
}

fn append_vector(result: &mut Vec<f32>, vector: Vec3) {
    result.extend([vector.x as f32, vector.y as f32, vector.z as f32]);
}

fn track_obstacles(piece: &Piece) -> Vec<SupportObstacle> {
    let intervals = (piece.length() / 2.0).ceil() as usize;
    let samples: Vec<_> = (0..=intervals)
        .map(|index| {
            piece
                .sample(piece.length() * index as f64 / intervals as f64)
                .position
        })
        .collect();
    samples
        .windows(2)
        .map(|pair| {
            let (start, end) = (pair[0], pair[1]);
            SupportObstacle {
                min: Vec3::new(
                    start.x.min(end.x) - 0.5,
                    start.y.min(end.y) - 0.5,
                    start.z.min(end.z) - 0.5,
                ),
                max: Vec3::new(
                    start.x.max(end.x) + 0.5,
                    start.y.max(end.y) + 0.5,
                    start.z.max(end.z) + 0.5,
                ),
            }
        })
        .collect()
}

fn append_pose(result: &mut Vec<f32>, index: usize, sample: TrackSample) {
    result.push(index as f32);
    append_vector(result, sample.position / 4.0);
    append_vector(result, sample.up.cross(sample.tangent));
    append_vector(result, sample.up);
}

#[cfg(test)]
mod world_tests {
    use super::{Position, Surface, WasmSimulation, track_obstacles};
    use theme_park_sim::track::supports::generate_supports;
    use theme_park_sim::track::{PieceId, TrackState, catalog};
    use theme_park_sim::world::World;

    #[test]
    fn demo_exports_wooden_lattice_diagonals() {
        let simulation = WasmSimulation::new(42, 0);
        assert!(
            simulation
                .coaster_supports()
                .chunks_exact(7)
                .any(|segment| {
                    (segment[1] != segment[4] || segment[2] != segment[5])
                        && segment[3] != segment[6]
                })
        );
    }

    #[test]
    fn coaster_exports_rails_and_evenly_spaced_ties_from_circuit_curves() {
        let simulation = WasmSimulation::new(42, 0);
        let rails = simulation.coaster_rails();
        let ties = simulation.coaster_ties();
        assert!(!rails.is_empty() && rails.len() % 19 == 0);
        assert_eq!(
            ties.len() / 13,
            (simulation.circuit.length() / 2.0).ceil() as usize
        );
        assert_eq!(ties.len() % 13, 0);
        for (index, tie) in ties.chunks_exact(13).enumerate() {
            let sample = simulation.circuit.sample(index as f64 * 2.0);
            for axis in 0..3 {
                let expected =
                    [sample.position.x, sample.position.y, sample.position.z][axis] / 4.0;
                let midpoint = (f64::from(tie[1 + axis]) + f64::from(tie[4 + axis])) / 2.0;
                assert!((midpoint - expected).abs() < 0.0001);
            }
            let dx = tie[1] - tie[4];
            let dy = tie[2] - tie[5];
            let dz = tie[3] - tie[6];
            assert!(((dx * dx + dy * dy + dz * dz).sqrt() - 0.44).abs() < 0.0001);
        }
        let first = simulation.circuit.pieces()[0].sample(0.0);
        let lateral = first.up.cross(first.tangent);
        for (rail, sign) in rails.chunks_exact(19).take(2).zip([-1.0, 1.0]) {
            let expected = (first.position + lateral * (sign * 0.72)) / 4.0;
            for (actual, expected) in rail[1..4].iter().zip([expected.x, expected.y, expected.z]) {
                assert!((f64::from(*actual) - expected).abs() < 0.0001);
            }
        }
        let segments: Vec<_> = rails.chunks_exact(38).collect();
        for index in 0..segments.len() {
            let current = segments[index];
            let next = segments[(index + 1) % segments.len()];
            for rail_offset in [0, 19] {
                for axis in 0..3 {
                    assert!(
                        (current[rail_offset + 10 + axis] - next[rail_offset + 1 + axis]).abs()
                            < 0.0001
                    );
                }
            }
        }
        assert!(rails.iter().chain(&ties).all(|value| value.is_finite()));
    }

    #[test]
    fn supports_reroute_around_another_sampled_track() {
        let straight = catalog()
            .into_iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap();
        let state = TrackState {
            x: 4,
            y: 4,
            z: 5,
            ..TrackState::origin()
        };
        let upper = straight.at(state).unwrap();
        let lower = straight.at(TrackState { z: 2, ..state }).unwrap();
        let world = World::new(
            16,
            16,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let direct = generate_supports(&upper, &world, &[]).unwrap();
        let rerouted = generate_supports(&upper, &world, &track_obstacles(&lower)).unwrap();
        assert!(
            direct
                .iter()
                .all(|support| support.footing.y == support.anchor.y)
        );
        assert!(
            rerouted
                .iter()
                .all(|support| support.footing.y != support.anchor.y)
        );
    }

    #[test]
    fn coaster_snapshots_contain_stable_geometry_and_moving_train_poses() {
        let mut simulation = WasmSimulation::new(42, 0);
        let geometry = simulation.coaster_geometry();
        let supports = simulation.coaster_supports();
        let initial_poses = simulation.coaster_train_poses();
        assert!(!geometry.is_empty() && geometry.len() % 11 == 0);
        assert!(!supports.is_empty() && supports.len() % 7 == 0);
        assert!(!initial_poses.is_empty() && initial_poses.len() % 11 == 0);
        let tangent = simulation
            .circuit
            .sample(simulation.train.distance())
            .tangent;
        assert_eq!(
            &initial_poses[4..7],
            &[tangent.x as f32, tangent.y as f32, tangent.z as f32]
        );
        assert!(
            geometry
                .iter()
                .chain(&supports)
                .chain(&initial_poses)
                .all(|value| value.is_finite())
        );
        for _ in 0..30 {
            simulation.tick();
        }
        assert_ne!(simulation.coaster_train_poses(), initial_poses);
        assert_eq!(simulation.coaster_geometry(), geometry);
        assert_eq!(simulation.coaster_supports(), supports);
    }

    #[test]
    fn demo_train_completes_a_circuit_on_simulation_ticks() {
        let mut simulation = WasmSimulation::new(42, 0);
        let circuit_length = simulation.circuit.length();
        let mut maximum_distance: f64 = 0.0;
        for _ in 0..9000 {
            simulation.tick();
            maximum_distance = maximum_distance.max(simulation.train.distance());
            if simulation.train.distance() >= circuit_length {
                return;
            }
        }
        panic!(
            "demo train failed to complete circuit: distance={}, maximum={}, speed={}, length={}, notes={:?}",
            simulation.train.distance(),
            maximum_distance,
            simulation.train.speed(),
            circuit_length,
            simulation.train.notes()
        );
    }

    #[test]
    fn guest_snapshot_contains_stable_ids_appearance_and_walking_state() {
        let mut simulation = WasmSimulation::new(42, 12);
        let ids = simulation.guest_ids();
        let appearances = simulation.guest_appearances();
        let initial = simulation.snapshot();
        assert_eq!(ids, (0..12).collect::<Vec<_>>());
        assert_eq!(appearances.len(), 12 * 5);
        assert_eq!(initial.len(), 12 * 7);
        for _ in 0..30 {
            simulation.tick();
        }
        assert_eq!(simulation.guest_ids(), ids);
        assert_eq!(simulation.guest_appearances(), appearances);
        assert_ne!(simulation.snapshot(), initial);
        assert_eq!(simulation.tick_count(), 30);
    }

    #[test]
    fn surface_edits_report_changes_and_update_the_snapshot() {
        let mut simulation = WasmSimulation::new(42, 0);
        let surface = simulation.world.surface(10, 20);
        assert!(!simulation.set_world_surface(10, 20, surface.corners.to_vec(), surface.water));
        assert!(simulation.set_world_surface(10, 20, vec![1, 2, 3, 4], Some(-1)));
        assert_eq!(
            simulation.world.surface(10, 20),
            Surface {
                corners: [1, 2, 3, 4],
                water: Some(-1)
            }
        );
        let offset = (20 * 512 + 10) * 6;
        assert_eq!(
            &simulation.world_surfaces()[offset..offset + 6],
            &[1, 2, 3, 4, 1, -1]
        );
        let path = Position {
            x: 218,
            y: 256,
            height: 8,
        };
        assert!(simulation.world.path(path).is_some());
        assert!(simulation.set_world_surface(218, 256, vec![9; 4], None));
        assert!(simulation.world.path(path).is_none());
        assert!(
            !simulation
                .world_paths()
                .chunks_exact(4)
                .any(|entry| entry == [218, 256, 8, 0])
        );
    }

    #[test]
    fn demo_has_two_localized_hills_with_matching_neighbor_edges() {
        let simulation = WasmSimulation::new(42, 0);
        for (center_x, center_y, base) in [(90, 90, 8), (400, 400, 23)] {
            assert!(simulation.world.surface(center_x, center_y).corners[0] > base);
            assert_eq!(
                simulation.world.surface(center_x - 13, center_y).corners,
                [base; 4]
            );
            for y in center_y - 12..center_y + 12 {
                for x in center_x - 12..center_x + 12 {
                    let tile = simulation.world.surface(x, y).corners;
                    let east = simulation.world.surface(x + 1, y).corners;
                    let south = simulation.world.surface(x, y + 1).corners;
                    assert_eq!([tile[1], tile[2]], [east[0], east[3]]);
                    assert_eq!([tile[3], tile[2]], [south[0], south[1]]);
                }
            }
        }
    }

    #[test]
    fn demo_snapshot_contains_real_terrain_water_and_paths() {
        let simulation = WasmSimulation::new(42, 0);
        assert_eq!(simulation.world_dimensions(), vec![512, 512]);
        let surfaces = simulation.world_surfaces();
        assert_eq!(surfaces.len(), 512 * 512 * 6);
        assert!(surfaces.chunks_exact(6).any(|tile| tile[0..4] != [8; 4]));
        assert!(surfaces.chunks_exact(6).any(|tile| tile[4] == 1));
        let carved = simulation.world_carved();
        assert!(carved.chunks_exact(4).any(|span| span == [224, 250, 3, 5]));
        let hash = simulation.world_state_hash();
        assert_eq!(hash, WasmSimulation::new(42, 0).world_state_hash());
        let paths = simulation.world_paths();
        assert!(!paths.is_empty());
        assert_eq!(paths.len() % 4, 0);
        assert!(paths.chunks_exact(4).any(|path| path[3] != 0));
        for path in paths.chunks_exact(4) {
            let tile = (path[1] as usize * 512 + path[0] as usize) * 6;
            let corners = &surfaces[tile..tile + 4];
            let height = path[2];
            let expected = match path[3] {
                0 => [height; 4],
                1 => [height + 1, height + 1, height, height],
                2 => [height, height + 1, height + 1, height],
                3 => [height, height, height + 1, height + 1],
                4 => [height + 1, height, height, height + 1],
                _ => panic!("invalid slope"),
            };
            let surface_path = corners == expected;
            let underground_path = carved.chunks_exact(4).any(|span| {
                span[0] == path[0]
                    && span[1] == path[1]
                    && span[2] <= height
                    && span[3] >= height + if path[3] == 0 { 1 } else { 2 }
            });
            assert!(surface_path || underground_path);
        }
    }

    #[test]
    fn demo_paths_form_a_surface_network_connected_to_an_underground_loop() {
        let simulation = WasmSimulation::new(42, 0);
        let world = &simulation.world;
        let position = |x, y, height| Position { x, y, height };
        let junction = position(218, 256, 8);
        assert!(world.neighbours(junction).len() >= 2);
        let entrance = [
            position(218, 250, 8),
            position(219, 250, 7),
            position(220, 250, 6),
            position(221, 250, 5),
            position(222, 250, 4),
            position(223, 250, 3),
            position(224, 250, 3),
        ];
        for pair in entrance.windows(2) {
            assert!(world.neighbours(pair[0]).contains(&pair[1]));
        }
        let loop_positions = [
            position(224, 250, 3),
            position(225, 250, 3),
            position(226, 250, 3),
            position(227, 250, 3),
            position(228, 250, 3),
            position(228, 251, 3),
            position(228, 252, 3),
            position(228, 253, 3),
            position(228, 254, 3),
            position(227, 254, 3),
            position(226, 254, 3),
            position(225, 254, 3),
            position(224, 254, 3),
            position(224, 253, 3),
            position(224, 252, 3),
            position(224, 251, 3),
            position(224, 250, 3),
        ];
        for pair in loop_positions.windows(2) {
            assert!(world.neighbours(pair[0]).contains(&pair[1]));
        }
        assert_eq!(world.neighbours(position(224, 250, 3)).len(), 3);
        for pair in (250..256).map(|y| (position(218, y, 8), position(218, y + 1, 8))) {
            assert!(world.neighbours(pair.0).contains(&pair.1));
        }
    }
}
