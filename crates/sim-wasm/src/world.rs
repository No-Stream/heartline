use super::command_errors::record_command_result;
use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, agent_count: usize) -> Self {
        let world = demo_world();
        let mut facilities = ParkFacilities::new();
        facilities
            .place(
                &world,
                FacilityKind::Entrance,
                Position {
                    x: 218,
                    y: 246,
                    height: 8,
                },
                Direction::South,
            )
            .expect("demo entrance placement");
        for (x, kind) in [
            (220, FacilityKind::Food),
            (223, FacilityKind::Drink),
            (226, FacilityKind::Restroom),
            (229, FacilityKind::Souvenir),
            (232, FacilityKind::Carousel),
            (235, FacilityKind::FerrisWheel),
        ] {
            facilities
                .place(
                    &world,
                    kind,
                    Position {
                        x,
                        y: 254,
                        height: 8,
                    },
                    Direction::South,
                )
                .expect("demo facility placement");
        }
        let mut simulation = ParkGuests::new(
            seed,
            agent_count,
            &world,
            Position {
                x: 218,
                y: 250,
                height: 8,
            },
            agent_count.max(1),
        );
        simulation.open_with_guests(&facilities.guest_facilities(&world));
        let circuit = demo_circuit();
        let train_configuration = hybrid_model().train;
        for building in facilities.buildings() {
            assert!(
                !facility_intersects_track(
                    building.kind,
                    building.position,
                    building.direction,
                    circuit.pieces(),
                    &train_configuration
                ),
                "demo facility intersects track"
            );
        }
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
            park: Park::new(
                world,
                facilities,
                simulation,
                circuit,
                train,
                train_configuration,
            ),
            facility_message: String::new(),
            path_message: String::new(),
            terrain_message: String::new(),
            scenery_message: String::new(),
            terrain_invalidated_paths: Vec::new(),
            construction_model: ConstructionModel::Hybrid,
            selected: None,
            construction_preview_supports_cache: Vec::new(),
            construction_placed_supports_cache: Vec::new(),
            recovered_station: None,
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

    /// Path kind, style, and edge mask triples aligned with `world_paths()`.
    pub fn world_path_metadata(&self) -> Vec<i32> {
        self.world
            .paths()
            .flat_map(|path| {
                let metadata = self
                    .world
                    .path_metadata(path.position)
                    .expect("world path metadata");
                [
                    metadata.kind as i32,
                    i32::from(metadata.style),
                    i32::from(
                        self.world
                            .path_edges(path.position)
                            .expect("world path edges"),
                    ),
                ]
            })
            .collect()
    }

    /// Attraction access openings: x, y, height, direction mask.
    pub fn world_facility_openings(&self) -> Vec<i32> {
        self.park
            .facility_openings()
            .into_iter()
            .flat_map(|opening| {
                [
                    i32::from(opening.position.x),
                    i32::from(opening.position.y),
                    i32::from(opening.position.height),
                    i32::from(opening.mask),
                ]
            })
            .collect()
    }

    pub fn facility_access_rotation(
        &self,
        kind: &str,
        x: u16,
        y: u16,
        height: i16,
        preferred: u8,
    ) -> i32 {
        let kind = match kind {
            "entrance" => FacilityKind::Entrance,
            "food_stall" => FacilityKind::Food,
            "drink_stall" => FacilityKind::Drink,
            "souvenir" => FacilityKind::Souvenir,
            "restroom" => FacilityKind::Restroom,
            "carousel" => FacilityKind::Carousel,
            "ferris_wheel" => FacilityKind::FerrisWheel,
            _ => return -1,
        };
        let directions = [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ];
        let Some(preferred) = directions.get(usize::from(preferred)).copied() else {
            return -1;
        };
        facility_access_direction(&self.world, kind, Position { x, y, height }, preferred)
            .map(|direction| match direction {
                Direction::North => 0,
                Direction::East => 1,
                Direction::South => 2,
                Direction::West => 3,
            })
            .unwrap_or(-1)
    }

    /// Preview entries: x, y, height, slope, kind, style, refusal, path edges, facility openings.
    /// An error uses a single record with x and y set to -1 and its refusal code set.
    pub fn path_preview(&self, coordinates: Vec<i32>, kind: &str, style: u8) -> Vec<i32> {
        let Some(kind) = parse_path_kind(kind) else {
            return path_preview_error_record(CommandError::UnknownPathKind);
        };
        let (start, end) = match path_positions(&coordinates) {
            Ok(positions) => positions,
            Err(error) => return path_preview_error_record(error),
        };
        match self.park.preview_path_route(start, end, kind, style) {
            Ok(route) => {
                let openings = self.park.facility_openings();
                route
                    .into_iter()
                    .flat_map(|tile| {
                        let metadata = self
                            .world
                            .path_metadata(tile.path.position)
                            .unwrap_or(theme_park_sim::world::PathMetadata { kind, style });
                        [
                            i32::from(tile.path.position.x),
                            i32::from(tile.path.position.y),
                            i32::from(tile.path.position.height),
                            path_slope_code(tile.path.slope),
                            metadata.kind as i32,
                            i32::from(metadata.style),
                            tile.refusal.map_or(0, path_error_code),
                            i32::from(tile.edges),
                            if tile.refusal.is_none() && metadata.kind == PathKind::Queue {
                                i32::from(
                                    openings
                                        .iter()
                                        .filter(|opening| opening.position == tile.path.position)
                                        .fold(0, |mask, opening| mask | opening.mask),
                                )
                            } else {
                                0
                            },
                        ]
                    })
                    .collect()
            }
            Err(error) => path_preview_error_record(error.into()),
        }
    }

    pub fn path_build(&mut self, coordinates: Vec<i32>, kind: &str, style: u8) -> bool {
        let Some(kind) = parse_path_kind(kind) else {
            record_command_result::<()>(
                Err(CommandError::UnknownPathKind),
                &mut self.path_message,
                false,
            );
            return false;
        };
        let (start, end) = match path_positions(&coordinates) {
            Ok(positions) => positions,
            Err(error) => {
                record_command_result::<()>(Err(error), &mut self.path_message, false);
                return false;
            }
        };
        let result = self
            .park
            .build_path_route(start, end, kind, style)
            .map_err(CommandError::from);
        let built = record_command_result(result, &mut self.path_message, true).is_some();
        if built {
            self.refresh_construction_support_rendering();
        }
        built
    }

    pub fn path_remove(&mut self, x: u16, y: u16, height: i16) -> bool {
        let position = Position { x, y, height };
        let result = self.park.remove_path(position).map_err(CommandError::from);
        let removed = record_command_result(result, &mut self.path_message, true).is_some();
        if removed {
            self.refresh_construction_support_rendering();
        }
        removed
    }

    pub fn path_message(&self) -> String {
        self.path_message.clone()
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

    pub fn terrain_apply(&mut self, name: &str, parameters: Vec<i32>) -> bool {
        self.terrain_invalidated_paths.clear();
        let TerrainParameters {
            x,
            y,
            size,
            corner,
            steps,
            height,
            bottom,
            top,
        } = match parse_terrain_parameters(parameters) {
            Ok(parameters) => parameters,
            Err(error) => {
                record_command_result::<()>(Err(error), &mut self.terrain_message, false);
                return false;
            }
        };
        let area = Brush { x, y, size };
        let terrain_command = match name {
            "raise_corner" | "lower_corner" => {
                let Some(corner) = parse_corner(corner) else {
                    record_command_result::<()>(
                        Err(CommandError::UnknownTerrainCorner),
                        &mut self.terrain_message,
                        false,
                    );
                    return false;
                };
                if name == "raise_corner" {
                    TerrainCommand::RaiseCorner {
                        x,
                        y,
                        corner,
                        steps,
                    }
                } else {
                    TerrainCommand::LowerCorner {
                        x,
                        y,
                        corner,
                        steps,
                    }
                }
            }
            "raise_tiles" => TerrainCommand::RaiseTiles { area, steps },
            "lower_tiles" => TerrainCommand::LowerTiles { area, steps },
            "flatten" => TerrainCommand::Flatten { area, height },
            "smooth" => TerrainCommand::Smooth { area },
            "set_water" => TerrainCommand::SetWater {
                area,
                level: Some(height),
            },
            "clear_water" => TerrainCommand::SetWater { area, level: None },
            "raise_water" => TerrainCommand::RaiseWater { area, steps },
            "lower_water" => TerrainCommand::LowerWater { area, steps },
            "carve" => TerrainCommand::Carve { area, bottom, top },
            "fill" => TerrainCommand::Fill { area, bottom, top },
            _ => {
                record_command_result::<()>(
                    Err(CommandError::UnknownTerrainCommand),
                    &mut self.terrain_message,
                    false,
                );
                return false;
            }
        };
        let result = self
            .park
            .apply_terrain(terrain_command, self.selected)
            .map_err(CommandError::from);
        let Some(result) = record_command_result(result, &mut self.terrain_message, false) else {
            return false;
        };
        self.terrain_invalidated_paths = result.invalidated_paths;
        self.refresh_construction_support_rendering();
        self.terrain_message = "Terrain updated".into();
        true
    }

    pub fn terrain_message(&self) -> String {
        self.terrain_message.clone()
    }

    /// Invalidated paths from the most recent terrain edit: x, y, height triples.
    pub fn terrain_invalidated_paths(&self) -> Vec<i32> {
        self.terrain_invalidated_paths
            .iter()
            .flat_map(|position| {
                [
                    i32::from(position.x),
                    i32::from(position.y),
                    i32::from(position.height),
                ]
            })
            .collect()
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
        self.refresh_construction_support_rendering();
        true
    }

    pub fn construction_message(&self) -> String {
        if self.construction_message.is_empty() {
            self.construction_outcome.clone().unwrap_or_default()
        } else {
            self.construction_message.clone()
        }
    }
}
