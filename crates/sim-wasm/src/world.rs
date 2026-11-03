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
            simulation,
            world,
            facilities,
            facility_message: String::new(),
            path_message: String::new(),
            terrain_message: String::new(),
            scenery_message: String::new(),
            terrain_invalidated_paths: Vec::new(),
            circuit,
            train,
            train_configuration,
            physics_remainder: 0,
            construction: None,
            construction_model: ConstructionModel::Hybrid,
            selected: None,
            construction_message: String::new(),
            construction_train: None,
            coaster_open: false,
            coaster_operation: None,
            coaster_riders: HashMap::new(),
            coaster_facility: None,
            block_layout: None,
            desired_train_count: None,
            block_placement_search: None,
            block_placement_cache: None,
            block_apply_best_requested: false,
            block_manual_search: None,
            block_baseline_validation: false,
            block_layout_is_automatic: false,
            block_message: String::new(),
            block_hourly_throughput: 0.0,
            block_throughput_reason: String::new(),
            block_throughput_estimator: None,
            block_analysis_extra_to_throughput: false,
            block_throughput_cache: HashMap::new(),
            recovered_station: None,
            support_render_cache: HashMap::new(),
            construction_slow_steps: 0,
            construction_outcome: None,
            construction_recorder: None,
            construction_report: None,
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
        self.facility_openings()
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
    pub fn path_preview(&mut self, coordinates: Vec<i32>, kind: &str, style: u8) -> Vec<i32> {
        let Some(kind) = parse_path_kind(kind) else {
            self.path_message = "Unknown path kind".into();
            return Vec::new();
        };
        let (start, end) = match path_positions(&coordinates) {
            Ok(positions) => positions,
            Err(message) => {
                self.path_message = message.into();
                return Vec::new();
            }
        };
        match self.checked_path_preview(start, end, kind, style) {
            Ok(route) => {
                let openings = self.facility_openings();
                self.path_message = route
                    .iter()
                    .find_map(|tile| tile.refusal)
                    .map_or_else(String::new, |error| path_build_error(error).into());
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
            Err(error) => {
                self.path_message = path_build_error(error).into();
                Vec::new()
            }
        }
    }

    pub fn path_build(&mut self, coordinates: Vec<i32>, kind: &str, style: u8) -> bool {
        let Some(kind) = parse_path_kind(kind) else {
            self.path_message = "Unknown path kind".into();
            return false;
        };
        let (start, end) = match path_positions(&coordinates) {
            Ok(positions) => positions,
            Err(message) => {
                self.path_message = message.into();
                return false;
            }
        };
        let preview = match self.checked_path_preview(start, end, kind, style) {
            Ok(preview) => preview,
            Err(error) => {
                self.path_message = path_build_error(error).into();
                return false;
            }
        };
        if let Some(error) = preview.iter().find_map(|tile| tile.refusal) {
            self.path_message = path_build_error(error).into();
            return false;
        }
        match self.world.build_path_route(start, end, kind, style) {
            Ok(_) => {
                self.path_message.clear();
                true
            }
            Err(error) => {
                self.path_message = path_build_error(error).into();
                false
            }
        }
    }

    pub fn path_remove(&mut self, x: u16, y: u16, height: i16) -> bool {
        let position = Position { x, y, height };
        if self.world.path(position).is_none() {
            self.path_message = "No path at that position".into();
            return false;
        }
        self.world.remove_path(position);
        self.path_message.clear();
        true
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
            Err(message) => {
                self.terrain_message = message.into();
                return false;
            }
        };
        let area = Brush { x, y, size };
        let terrain_command = match name {
            "raise_corner" | "lower_corner" => {
                let Some(corner) = parse_corner(corner) else {
                    self.terrain_message = "Unknown terrain corner".into();
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
                self.terrain_message = "Unknown terrain command".into();
                return false;
            }
        };
        if let Err(error) = self.world.terrain_edit_tiles(&terrain_command) {
            self.terrain_message = terrain_edit_error(error).into();
            return false;
        }

        let mut candidate = self.world.clone();
        let candidate_result = match candidate.apply_terrain(terrain_command) {
            Ok(result) => result,
            Err(error) => {
                self.terrain_message = terrain_edit_error(error).into();
                return false;
            }
        };
        if candidate_result.changed_tiles.iter().any(|tile| {
            self.facilities.buildings().iter().any(|building| {
                let (width, depth) = building.kind.footprint(building.direction);
                tile.x >= building.position.x
                    && u32::from(tile.x) < u32::from(building.position.x) + u32::from(width)
                    && tile.y >= building.position.y
                    && u32::from(tile.y) < u32::from(building.position.y) + u32::from(depth)
            })
        }) {
            self.terrain_message = "Cannot edit terrain beneath a building or ride".into();
            return false;
        }
        if self.terrain_hits_track(&candidate) {
            return false;
        }

        let result = self
            .world
            .apply_terrain(terrain_command)
            .expect("validated terrain command remains valid");
        assert_eq!(result, candidate_result, "terrain preflight result changed");
        self.terrain_invalidated_paths = result.invalidated_paths;
        self.support_render_cache.clear();
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

    pub(super) fn terrain_hits_track(&mut self, candidate: &World) -> bool {
        let clearance_worsened = |piece: &Piece, train: &TrainConfiguration| {
            !matches!(
                check_clearance(&self.world, piece, train, &[], &[]),
                Err(ClearanceCollision::Terrain)
            ) && matches!(
                check_clearance(candidate, piece, train, &[], &[]),
                Err(ClearanceCollision::Terrain)
            )
        };
        if self
            .circuit
            .pieces()
            .iter()
            .any(|piece| clearance_worsened(piece, &self.train_configuration))
        {
            self.terrain_message = "Terrain edit would collide with coaster track".into();
            return true;
        }
        if try_render_supports(self.circuit.pieces(), &self.world).is_ok()
            && try_render_supports(self.circuit.pieces(), candidate).is_err()
        {
            self.terrain_message = "Terrain edit leaves no route for coaster supports".into();
            return true;
        }
        let Some(construction) = &self.construction else {
            return false;
        };
        let pieces: Vec<_> = construction
            .pieces()
            .iter()
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| &segment.pieces),
            )
            .collect();
        if pieces
            .iter()
            .any(|piece| clearance_worsened(piece, &construction.model().train))
        {
            self.terrain_message = "Terrain edit would collide with coaster track".into();
            return true;
        }
        let entries: Vec<_> = construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .collect();
        let facility_obstacles = self.facility_obstacles();
        let mut original_cache = HashMap::new();
        let mut candidate_cache = HashMap::new();
        let original_supports = render_cached_supports(
            &entries,
            &self.world,
            &construction.model().train,
            construction.model().support_style,
            &facility_obstacles,
            &mut original_cache,
        );
        let candidate_supports = render_cached_supports(
            &entries,
            candidate,
            &construction.model().train,
            construction.model().support_style,
            &facility_obstacles,
            &mut candidate_cache,
        );
        if original_supports.is_ok() && candidate_supports.is_err() {
            self.terrain_message = "Terrain edit leaves no route for coaster supports".into();
            return true;
        }
        if self.construction_preview_valid() {
            let preview = self.construction_preview();
            if preview
                .first()
                .is_some_and(|piece| clearance_worsened(piece, &construction.model().train))
            {
                self.terrain_message = "Terrain edit would collide with coaster track".into();
                return true;
            }
            if let Some(piece) = preview.first() {
                let mut obstacles = facility_obstacles;
                obstacles.extend(entries.iter().flat_map(|(_, placed)| {
                    track_support_obstacles(
                        std::slice::from_ref(placed),
                        &construction.model().train,
                    )
                }));
                if render_preview_supports(
                    piece,
                    &obstacles,
                    &self.world,
                    construction.model().support_style,
                )
                .is_ok()
                    && render_preview_supports(
                        piece,
                        &obstacles,
                        candidate,
                        construction.model().support_style,
                    )
                    .is_err()
                {
                    self.terrain_message =
                        "Terrain edit leaves no route for coaster supports".into();
                    return true;
                }
            }
        }
        false
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

    pub fn construction_message(&self) -> String {
        if self.construction_message.is_empty() {
            self.construction_outcome.clone().unwrap_or_default()
        } else {
            self.construction_message.clone()
        }
    }
}
