use theme_park_sim::Simulation;
use theme_park_sim::world::{Direction, Path, Position, Surface, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmSimulation {
    simulation: Simulation,
    world: World,
}

#[wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, agent_count: usize) -> Self {
        Self {
            simulation: Simulation::new(seed, agent_count),
            world: demo_world(),
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
        self.simulation.tick();
    }

    pub fn tick_count(&self) -> u64 {
        self.simulation.tick_count()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.simulation
            .agents()
            .iter()
            .flat_map(|agent| [agent.x, agent.y])
            .collect()
    }
}

fn demo_world() -> World {
    let mut world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    // Adjacent tiles share vertex heights, including the gentle ramp through the path.
    let height = |x: u16| 8 + i16::try_from(x.saturating_sub(245).min(15)).unwrap();
    for y in 0..512 {
        for x in 245..512 {
            let west = height(x);
            let east = height(x + 1);
            world.set_surface(
                x,
                y,
                Surface {
                    corners: [west, east, east, west],
                    water: None,
                },
            );
        }
    }
    for (center_x, center_y) in [(90_u16, 90_u16), (400, 400)] {
        let hill_height = |x: u16, y: u16| {
            let distance = x.abs_diff(center_x).max(y.abs_diff(center_y));
            i16::try_from(12_u16.saturating_sub(distance) / 2).unwrap()
        };
        for y in center_y - 12..center_y + 12 {
            for x in center_x - 12..center_x + 12 {
                world.set_surface(
                    x,
                    y,
                    Surface {
                        corners: [
                            height(x) + hill_height(x, y),
                            height(x + 1) + hill_height(x + 1, y),
                            height(x + 1) + hill_height(x + 1, y + 1),
                            height(x) + hill_height(x, y + 1),
                        ],
                        water: None,
                    },
                );
            }
        }
    }
    for y in 235..245 {
        for x in 230..240 {
            world.set_surface(
                x,
                y,
                Surface {
                    corners: [8; 4],
                    water: Some(9),
                },
            );
        }
    }
    for x in 218..280 {
        world.place_path(Path {
            position: Position {
                x,
                y: 256,
                height: height(x),
            },
            slope: if height(x + 1) > height(x) {
                Some(Direction::East)
            } else {
                None
            },
        });
    }
    for y in 250..256 {
        world.place_path(Path {
            position: Position {
                x: 218,
                y,
                height: 8,
            },
            slope: None,
        });
    }
    for y in 257..264 {
        world.place_path(Path {
            position: Position {
                x: 235,
                y,
                height: 8,
            },
            slope: None,
        });
    }
    world.set_surface(
        219,
        250,
        Surface {
            corners: [8, 7, 7, 8],
            water: None,
        },
    );
    world.place_path(Path {
        position: Position {
            x: 219,
            y: 250,
            height: 7,
        },
        slope: Some(Direction::West),
    });
    for x in 220..224 {
        let path_height = 226 - x as i16;
        world.carve(x, 250, path_height, path_height + 2);
        world.place_path(Path {
            position: Position {
                x,
                y: 250,
                height: path_height,
            },
            slope: Some(Direction::West),
        });
    }
    for y in 250..=254 {
        for x in 224..=228 {
            if x == 224 || x == 228 || y == 250 || y == 254 {
                world.carve(x, y, 3, 5);
                world.place_path(Path {
                    position: Position { x, y, height: 3 },
                    slope: None,
                });
            }
        }
    }
    world
}

#[cfg(test)]
mod world_tests {
    use super::{Position, Surface, WasmSimulation};

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
