mod guest_benchmark;

use std::collections::HashMap;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::facilities::{
    FacilityKind, ParkFacilities, PathOpening, coaster_access_openings, coaster_facility,
    facility_access_direction, station_positions, validate_facility_path_access,
};
use theme_park_sim::guest_behavior::{Facility, RidePhase};
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{
    BlockOperation, Circuit, CoasterOutcome, PhysicsConfig, TIME_STEP, TestRun, TestRunRecorder,
    Train, demo_circuit,
};
use theme_park_sim::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, brake_runs,
};
use theme_park_sim::track::clearance::{ClearanceBox, ClearanceCollision, check_clearance};
use theme_park_sim::track::construction::{Construction, track_support_obstacles};
use theme_park_sim::track::proximity::{
    ProximityKind, TrainPosition, scan_proximity_with_supports,
};
use theme_park_sim::track::supports::{
    SUPPORT_INFLUENCE_RADIUS_METRES, SupportError, SupportObstacle, SupportSegment,
    generate_dense_wooden_lattice, generate_steel_columns, generate_wooden_lattice,
};
use theme_park_sim::track::{
    Heading, Material, Piece, PieceId, PieceKind, SupportStyle, TrackSample, TrackState,
    TrainConfiguration, Vec3, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{
    Brush, Corner, Direction, PathBuildError, PathKind, PathPreviewTile, Position, Scenery,
    SceneryKind, Surface, TerrainCommand, TerrainEditError, World,
};
use wasm_bindgen::prelude::*;

struct SupportRenderCache {
    obstacles: Vec<SupportObstacle>,
    dependencies: Vec<usize>,
    world_revision: u64,
    segments: Result<Vec<SupportSegment>, SupportError>,
}

#[wasm_bindgen]
pub struct WasmSimulation {
    simulation: ParkGuests,
    world: World,
    facilities: ParkFacilities,
    facility_message: String,
    path_message: String,
    terrain_message: String,
    scenery_message: String,
    terrain_invalidated_paths: Vec<Position>,
    circuit: Circuit,
    train: Train,
    train_configuration: TrainConfiguration,
    physics_remainder: u32,
    construction: Option<Construction>,
    construction_model: ConstructionModel,
    selected: Option<PieceId>,
    construction_message: String,
    construction_train: Option<(Circuit, Train)>,
    coaster_open: bool,
    coaster_operation: Option<BlockOperation>,
    coaster_riders: HashMap<u64, Vec<GuestId>>,
    coaster_facility: Option<Facility>,
    block_layout: Option<BlockLayout>,
    desired_train_count: Option<usize>,
    block_placement_search: Option<BlockPlacementSearch>,
    block_placement_cache: Option<BlockPlacementSearch>,
    block_apply_best_requested: bool,
    block_manual_search: Option<BlockManualSearch>,
    block_baseline_validation: bool,
    block_layout_is_automatic: bool,
    block_message: String,
    block_hourly_throughput: f64,
    block_throughput_reason: String,
    block_throughput_estimator: Option<ThroughputEstimator>,
    block_analysis_extra_to_throughput: bool,
    block_throughput_cache: HashMap<Vec<u64>, (f64, String)>,
    recovered_station: Option<Piece>,
    support_render_cache: HashMap<usize, SupportRenderCache>,
    construction_slow_steps: u32,
    construction_outcome: Option<String>,
    construction_recorder: Option<TestRunRecorder>,
    construction_report: Option<TestRun>,
}

#[derive(Clone, Copy)]
enum ConstructionModel {
    Hybrid,
    MultiLaunch,
    WoodenTwister,
}

const COASTER_PLACE_ID: u32 = u32::MAX;

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

    fn terrain_hits_track(&mut self, candidate: &World) -> bool {
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

    pub fn place_scenery(&mut self, kind: &str, x: u16, y: u16, rotation: u8, quarter: u8) -> bool {
        let Some(kind) = scenery_kind(kind) else {
            self.scenery_message = "Unknown scenery".into();
            return false;
        };
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            self.scenery_message = "Outside world".into();
            return false;
        }
        if rotation > 3 || quarter > 3 {
            self.scenery_message = "Invalid scenery orientation".into();
            return false;
        }
        let scenery = Scenery {
            kind,
            position: Position {
                x,
                y,
                height: self.world.surface(x, y).corners[0],
            },
            rotation,
            quarter,
        };
        let bounds = scenery.clearance_box();
        if self.facility_obstacles().iter().any(|obstacle| {
            boxes_overlap(
                bounds,
                ClearanceBox {
                    min: obstacle.min,
                    max: obstacle.max,
                },
            )
        }) {
            self.scenery_message = "Scenery intersects a facility".into();
            return false;
        }
        let hits_track = |piece: &Piece, train: &TrainConfiguration| {
            track_support_obstacles(std::slice::from_ref(piece), train)
                .iter()
                .any(|obstacle| {
                    boxes_overlap(
                        bounds,
                        ClearanceBox {
                            min: obstacle.min,
                            max: obstacle.max,
                        },
                    )
                })
        };
        if self
            .circuit
            .pieces()
            .iter()
            .any(|piece| hits_track(piece, &self.train_configuration))
            || self.construction.as_ref().is_some_and(|construction| {
                construction
                    .pieces()
                    .iter()
                    .chain(
                        construction
                            .disconnected_segments()
                            .iter()
                            .flat_map(|segment| &segment.pieces),
                    )
                    .any(|piece| hits_track(piece, &construction.model().train))
            })
        {
            self.scenery_message = "Scenery intersects coaster track or train clearance".into();
            return false;
        }
        let mut candidate = self.world.clone();
        if let Err(error) = candidate.place_scenery(scenery) {
            self.scenery_message = format!("{error:?}");
            return false;
        }
        let previous_terrain_message = self.terrain_message.clone();
        let hits_track = self.terrain_hits_track(&candidate);
        self.terrain_message = previous_terrain_message;
        if hits_track {
            self.scenery_message = "Scenery leaves no route for coaster supports".into();
            return false;
        }
        self.world
            .place_scenery(scenery)
            .expect("validated scenery placement remains valid");
        self.support_render_cache.clear();
        self.scenery_message.clear();
        true
    }

    pub fn remove_scenery(&mut self, id: &str) -> bool {
        let item = self
            .world
            .scenery()
            .find(|item| scenery_id(item) == id)
            .copied();
        let Some(item) = item else {
            self.scenery_message = "No scenery with that ID".into();
            return false;
        };
        let removed = self.world.remove_scenery(item.position, item.quarter);
        self.support_render_cache.clear();
        self.scenery_message.clear();
        removed
    }

    pub fn scenery_message(&self) -> String {
        self.scenery_message.clone()
    }

    pub fn scenery_json(&self) -> String {
        let items = self.world.scenery().map(|item| format!(
            "{{\"id\":\"{}\",\"kind\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"rotation\":{},\"quarter\":{}}}",
            scenery_id(item), scenery_kind_name(item.kind), item.position.x, item.position.y,
            item.position.height, item.rotation, item.quarter
        )).collect::<Vec<_>>().join(",");
        format!("[{items}]")
    }

    pub fn place_facility(&mut self, kind: &str, x: u16, y: u16, rotation: u8) -> bool {
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            self.facility_message = "Outside world".into();
            return false;
        }
        let surface_height = self.world.surface(x, y).corners[0];
        self.place_facility_at(kind, x, y, rotation, surface_height)
    }

    pub fn place_facility_at(
        &mut self,
        kind: &str,
        x: u16,
        y: u16,
        rotation: u8,
        height: i16,
    ) -> bool {
        let kind = match kind {
            "entrance" => FacilityKind::Entrance,
            "food_stall" => FacilityKind::Food,
            "drink_stall" => FacilityKind::Drink,
            "souvenir" => FacilityKind::Souvenir,
            "restroom" => FacilityKind::Restroom,
            "carousel" => FacilityKind::Carousel,
            "ferris_wheel" => FacilityKind::FerrisWheel,
            _ => {
                self.facility_message = "Unknown facility".into();
                return false;
            }
        };
        let direction = match rotation {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            3 => Direction::West,
            _ => {
                self.facility_message = "Invalid rotation".into();
                return false;
            }
        };
        let (width, depth) = self.world.dimensions();
        if x >= width || y >= depth {
            self.facility_message = "Outside world".into();
            return false;
        }
        let position = Position { x, y, height };
        let mut pieces = self.circuit.pieces().to_vec();
        if let Some(construction) = &self.construction {
            pieces.extend_from_slice(construction.pieces());
            for segment in construction.disconnected_segments() {
                pieces.extend_from_slice(&segment.pieces);
            }
        }
        if facility_intersects_track(
            kind,
            position,
            direction,
            &pieces,
            &self.train_configuration,
        ) {
            self.facility_message = "Facility intersects track or train clearance".into();
            return false;
        }
        match self
            .facilities
            .place(&self.world, kind, position, direction)
        {
            Ok(_) => {
                self.support_render_cache.clear();
                self.facility_message.clear();
                true
            }
            Err(error) => {
                self.facility_message = format!("{error:?}");
                false
            }
        }
    }

    pub fn remove_facility(&mut self, id: u32) -> bool {
        let removed = self.facilities.remove(id);
        if removed {
            self.support_render_cache.clear();
        }
        removed
    }

    pub fn set_facility_price(&mut self, id: u32, price: u16) -> bool {
        match self.facilities.set_price(id, price) {
            Ok(()) => {
                self.facility_message.clear();
                true
            }
            Err(error) => {
                self.facility_message = format!("{error:?}");
                false
            }
        }
    }

    pub fn facility_message(&self) -> String {
        self.facility_message.clone()
    }

    pub fn facility_revenue(&self) -> u64 {
        self.simulation.revenue()
    }

    pub fn facilities_json(&self) -> String {
        let guest_facilities = self.facilities.guest_facilities(&self.world);
        let buildings = self.facilities.buildings().iter().map(|building| {
            let kind = match building.kind {
                FacilityKind::Entrance => "entrance", FacilityKind::Food => "food_stall",
                FacilityKind::Drink => "drink_stall", FacilityKind::Souvenir => "souvenir", FacilityKind::Restroom => "restroom",
                FacilityKind::Carousel => "carousel", FacilityKind::FerrisWheel => "ferris_wheel",
            };
            let rotation = match building.direction {
                Direction::North => 0, Direction::East => 1, Direction::South => 2, Direction::West => 3,
            };
            let state = guest_facilities.iter().find(|facility| facility.place.id == building.id)
                .filter(|_| matches!(building.kind, FacilityKind::Carousel | FacilityKind::FerrisWheel))
                .map(|facility| self.simulation.behaviors().ride_state(facility));
            let (ride_state, ride_progress) = match state {
                Some(state) => (match state.phase { RidePhase::Idle => "idle", RidePhase::Running => "running", RidePhase::Unloading => "unloading" }, state.progress),
                None => ("idle", 0.0),
            };
            format!("{{\"id\":{},\"kind\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"rotation\":{},\"price\":{},\"ride_state\":\"{}\",\"ride_progress\":{}}}",
                building.id, kind, building.position.x, building.position.y, building.position.height, rotation, building.price, ride_state, ride_progress)
        }).collect::<Vec<_>>().join(",");
        format!("[{buildings}]")
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
        let population = self.simulation.walking().guests().len();
        self.simulation
            .set_entrance(&self.world, self.facilities.entrance(&self.world));
        let mut facilities = self.facilities.guest_facilities(&self.world);
        if self.coaster_open {
            let access = self.construction.as_ref().and_then(|construction| {
                coaster_facility(&self.world, construction, COASTER_PLACE_ID, 0)
            });
            if let Some(facility) = access {
                self.coaster_facility = Some(facility);
                facilities.push(facility);
            } else {
                self.close_live_coaster();
            }
        }
        self.simulation.set_external_rides(if self.coaster_open {
            &[COASTER_PLACE_ID]
        } else {
            &[]
        });
        self.simulation
            .tick(&self.world, &facilities, 100, Some(population));
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            let mut report_finished = false;
            self.train.step(&self.circuit);
            let demo_trains: Vec<_> = self
                .train
                .readings()
                .last()
                .expect("step produces readings")
                .iter()
                .map(|reading| {
                    let sample = self.circuit.sample(reading.distance);
                    TrainPosition {
                        position: sample.position,
                        tangent: sample.tangent,
                        piece_index: self.circuit.piece_index(reading.distance),
                        distance: reading.distance.rem_euclid(self.circuit.length()),
                        speed: reading.speed,
                    }
                })
                .collect();
            self.train.take_readings();
            if self.coaster_open && !self.block_layout_change_pending() {
                while let Some(train_id) = {
                    let operation = self
                        .coaster_operation
                        .as_ref()
                        .expect("open coaster has an operation");
                    if operation.trains().count() < self.construction_train_count() {
                        operation.next_dispatch_id()
                    } else {
                        None
                    }
                } {
                    let riders = self.simulation.board_coaster_train(
                        self.coaster_facility
                            .as_ref()
                            .expect("open coaster has access"),
                        train_id,
                    );
                    if riders.is_empty() {
                        break;
                    }
                    assert_eq!(
                        self.coaster_operation.as_mut().unwrap().dispatch(),
                        train_id
                    );
                    self.coaster_riders.insert(train_id, riders);
                }
            }
            let outcomes = self
                .coaster_operation
                .as_mut()
                .map(|operation| operation.step(&demo_trains))
                .unwrap_or_default();
            for (train_id, outcome) in outcomes {
                self.finish_live_train(train_id, outcome);
            }
            if self.construction_outcome.is_none() {
                if let Some((circuit, train)) = &mut self.construction_train {
                    let was_recovered = !train.notes().is_empty();
                    let previous_distance = train.distance();
                    train.step(circuit);
                    if !was_recovered && !train.notes().is_empty() {
                        self.construction_message =
                            "Train recovered to the station after rollback.".into();
                    } else if previous_distance < circuit.length()
                        && train.distance() >= circuit.length()
                    {
                        self.construction_message = "Train completed a lap.".into();
                    }
                    if train.speed().abs() < 0.05 {
                        self.construction_slow_steps += 1;
                        if self.construction_slow_steps >= 200 && train.notes().is_empty() {
                            self.construction_message =
                                "Train stopped before completing a lap.".into();
                        }
                    } else {
                        self.construction_slow_steps = 0;
                    }
                    if self.construction_message == "Train recovered to the station after rollback."
                        || self.construction_message == "Train completed a lap."
                        || self.construction_message == "Train stopped before completing a lap."
                    {
                        self.construction_outcome = Some(self.construction_message.clone());
                    }
                    let readings = train.take_readings();
                    if let Some(recorder) = &mut self.construction_recorder {
                        recorder.record(&readings);
                        for frame in &readings {
                            if let Some(reading) = frame.first() {
                                recorder.record_dueling(circuit, *reading, &demo_trains);
                            }
                        }
                    }
                    if self.construction_outcome.is_some() {
                        let recorder = self.construction_recorder.take().expect("test recorder");
                        self.construction_report = Some(recorder.finish(
                            self.construction_message == "Train completed a lap.",
                            !train.notes().is_empty(),
                        ));
                        report_finished = true;
                    }
                }
            }
            if report_finished {
                self.construction_placed_supports();
                let obstacles = self.proximity_obstacles();
                let supports = self.proximity_supports();
                if let Some(report) = &mut self.construction_report {
                    let construction = self
                        .construction
                        .as_ref()
                        .expect("report requires construction");
                    let geometry = scan_proximity_with_supports(
                        &self.world,
                        construction.pieces(),
                        &construction.model().train,
                        self.circuit.pieces(),
                        &obstacles,
                        &supports,
                    );
                    let events = geometry.events_for_run(&report.samples);
                    report.add_proximity_events(&events);
                }
            }
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
        render_rails(self.circuit.pieces(), &self.world)
    }

    /// Cross ties: piece index, endpoints, up, tangent. Positions are in tiles.
    /// Ties are spaced two metres apart over the complete circuit, including joins.
    pub fn coaster_ties(&self) -> Vec<f32> {
        render_ties(self.circuit.pieces(), &self.world)
    }

    /// Segments: piece index, start position, end position, in tiles.
    pub fn coaster_supports(&self) -> Vec<f32> {
        render_supports(self.circuit.pieces(), &self.world)
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
        self.simulation.walking().tick_count()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        let walking = self.simulation.walking();
        let traits = walking.traits();
        walking
            .guests()
            .iter()
            .enumerate()
            .filter(|(index, _)| traits.is_active(GuestId(*index as u32)))
            .map(|(_, guest)| guest)
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
        let traits = self.simulation.walking().traits();
        traits
            .ids()
            .filter(|&id| traits.is_active(id))
            .map(|id| id.0)
            .collect()
    }

    pub fn guest_detail_json(&self, id: u32) -> String {
        self.simulation
            .guest_detail(GuestId(id))
            .map_or_else(|| "null".to_owned(), |detail| detail.to_json())
    }

    pub fn guest_appearances(&self) -> Vec<u8> {
        let traits = self.simulation.walking().traits();
        traits
            .ids()
            .filter(|&id| traits.is_active(id))
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

    pub fn construction_start(&mut self, x: i32, y: i32, z: i32, heading: u8) {
        let heading = match heading {
            0 => Heading::East,
            1 => Heading::NorthEast,
            2 => Heading::North,
            3 => Heading::NorthWest,
            4 => Heading::West,
            5 => Heading::SouthWest,
            6 => Heading::South,
            7 => Heading::SouthEast,
            _ => {
                self.construction_message = "Invalid heading".into();
                return;
            }
        };
        let mut construction = Construction::new(
            TrackState {
                x,
                y,
                z,
                heading,
                ..TrackState::origin()
            },
            match self.construction_model {
                ConstructionModel::Hybrid => hybrid_model(),
                ConstructionModel::MultiLaunch => multi_launch_model(),
                ConstructionModel::WoodenTwister => wooden_twister_model(),
            },
        );
        if let Err(error) =
            construction.append(PieceId::Station, &self.world, &self.facility_obstacles())
        {
            self.construction_message = construction_error(error);
            return;
        }
        self.close_live_coaster();
        self.construction = Some(construction);
        self.clear_block_layout();
        self.selected = None;
        self.select_compatible_piece();
        self.construction_train = None;
        self.recovered_station = None;
        self.support_render_cache.clear();
        self.construction_outcome = None;
        self.construction_recorder = None;
        self.construction_report = None;
        self.construction_message.clear();
    }
    pub fn construction_active(&self) -> bool {
        self.construction.is_some()
    }
    pub fn construction_select(&mut self, id: &str) {
        match id {
            "model:hybrid" => {
                self.construction_model = ConstructionModel::Hybrid;
                self.construction_message.clear();
                return;
            }
            "model:velocity_viper" => {
                self.construction_model = ConstructionModel::MultiLaunch;
                self.construction_message.clear();
                return;
            }
            "model:wooden_twister" => {
                self.construction_model = ConstructionModel::WoodenTwister;
                self.construction_message.clear();
                return;
            }
            _ => {}
        }
        self.selected = catalog()
            .into_iter()
            .find(|piece| format!("{:?}", piece.id) == id)
            .map(|piece| piece.id);
        self.construction_message = if self.selected.is_some() {
            String::new()
        } else {
            "Unknown piece".into()
        };
    }
    pub fn construction_selected(&self) -> String {
        self.selected
            .map(|id| format!("{id:?}"))
            .unwrap_or_default()
    }
    pub fn construction_choices(&self) -> String {
        let Some(construction) = &self.construction else {
            return String::new();
        };
        construction
            .available_choices(&self.world, &self.facility_obstacles())
            .into_iter()
            .map(|(piece, choice)| {
                let reason = choice
                    .result
                    .err()
                    .map(construction_error)
                    .unwrap_or_default();
                format!(
                    "{:?}\t{}\t{}\t{:?}\t{:?}\t{:?}\t{}",
                    piece.id,
                    piece.name,
                    piece_group(piece.id),
                    piece.exit.heading,
                    piece.exit.slope,
                    piece.exit.bank,
                    reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn construction_track(&self) -> String {
        let Some(construction) = &self.construction else {
            return String::new();
        };
        construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .map(|(id, piece)| format!("{id}\t{:?}", piece.id))
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn construction_can_append(&self) -> bool {
        self.construction
            .as_ref()
            .zip(self.selected)
            .is_some_and(|(construction, selected)| {
                construction
                    .choices(&self.world, &self.facility_obstacles())
                    .iter()
                    .any(|choice| choice.id == selected && choice.result.is_ok())
            })
    }
    pub fn construction_preview_valid(&self) -> bool {
        self.construction_can_append()
    }
    pub fn construction_preview_reason(&self) -> String {
        let Some((construction, selected)) = self.construction.as_ref().zip(self.selected) else {
            return String::new();
        };
        construction
            .choices(&self.world, &self.facility_obstacles())
            .into_iter()
            .find(|choice| choice.id == selected)
            .expect("selected piece is in catalog")
            .result
            .err()
            .map(construction_error)
            .unwrap_or_default()
    }
    pub fn construction_can_close(&self) -> bool {
        self.construction.as_ref().is_some_and(|construction| {
            !construction.is_closed() && Circuit::new(construction.pieces().to_vec()).is_ok()
        })
    }
    pub fn construction_can_test(&self) -> bool {
        self.construction.as_ref().is_some_and(|construction| {
            construction.is_closed()
                && construction.circuit().is_ok_and(|circuit| {
                    circuit.length() > construction.model().train.length_metres
                })
        })
    }
    pub fn construction_is_open(&self) -> bool {
        self.coaster_open
    }

    /// Advance pending block analysis by at most the requested number of physics steps.
    /// Returns true once analysis has finished. Large requests are capped per call.
    pub fn construction_advance_block_analysis(&mut self, work_steps: u32) -> bool {
        let budget = work_steps.min(1_000) as usize;
        let has_layout_search =
            self.block_manual_search.is_some() || self.block_placement_search.is_some();
        let search_budget = if has_layout_search && self.block_throughput_estimator.is_some() {
            let extra_to_throughput = self.block_analysis_extra_to_throughput && budget % 2 == 1;
            if budget % 2 == 1 {
                self.block_analysis_extra_to_throughput = !self.block_analysis_extra_to_throughput;
            }
            (budget + usize::from(budget % 2 == 1 && !extra_to_throughput)) / 2
        } else if has_layout_search {
            budget
        } else {
            0
        };
        let mut search_steps = 0;
        if let Some(mut search) = self.block_manual_search.take() {
            let before = search.work_completed();
            search.advance(search_budget);
            search_steps = search.work_completed() - before;
            match search.result() {
                Some(Ok(layout)) => {
                    let installed = self
                        .block_layout
                        .as_ref()
                        .expect("validation has an installed layout");
                    if block_layout_fingerprint(layout) == block_layout_fingerprint(installed) {
                        self.block_layout = Some(layout.clone());
                        self.block_layout_is_automatic = false;
                        self.block_baseline_validation = false;
                    } else if self.coaster_has_active_trains() {
                        self.block_manual_search = Some(search);
                    } else {
                        self.install_block_layout(layout.clone(), false);
                    }
                }
                Some(Err(reason)) => {
                    self.block_baseline_validation = false;
                    self.set_block_error(reason);
                }
                None => {
                    self.block_manual_search = Some(search);
                }
            }
        } else if let Some(mut search) = self.block_placement_search.take() {
            if !self.block_apply_best_requested {
                let before = search.work_completed();
                search.advance(search_budget);
                search_steps = search.work_completed() - before;
            }
            let complete = search.is_complete() || self.block_apply_best_requested;
            let best = search.best();
            let installed = self
                .block_layout
                .as_ref()
                .expect("search has an installed layout");
            let improves_capacity = best.capacity() > installed.capacity();
            let same_layout = block_layout_fingerprint(best) == block_layout_fingerprint(installed);
            let verifies_installed = same_layout
                && !installed.capacity_search_complete()
                && best.capacity_search_complete();
            let should_install = self.block_apply_best_requested
                || improves_capacity
                || verifies_installed
                || confirmed_stalled_baseline(best, installed)
                || (complete && best.capacity() >= installed.capacity());
            if should_install && !same_layout && self.coaster_has_active_trains() {
                self.block_placement_search = Some(search);
            } else {
                if should_install {
                    let layout = best.clone();
                    if same_layout {
                        self.block_layout = Some(layout);
                        self.block_layout_is_automatic = true;
                    } else {
                        self.install_block_layout(layout, true);
                    }
                }
                if complete {
                    self.block_apply_best_requested = false;
                    if search.is_complete() {
                        self.block_placement_cache = Some(search);
                    }
                } else {
                    self.block_placement_search = Some(search);
                }
            }
        }
        if let Some(mut estimator) = self.block_throughput_estimator.take() {
            if let Some(result) = estimator.advance(budget - search_steps) {
                let estimate = match result {
                    Ok(throughput) => (throughput, String::new()),
                    Err(reason) => (0.0, reason.to_owned()),
                };
                self.block_hourly_throughput = estimate.0;
                self.block_throughput_reason.clone_from(&estimate.1);
                let layout = self.block_layout.as_ref().expect("estimator has a layout");
                self.block_throughput_cache
                    .insert(block_layout_fingerprint(layout), estimate);
            } else {
                self.block_throughput_estimator = Some(estimator);
            }
        }
        self.block_manual_search.is_none()
            && self.block_placement_search.is_none()
            && self.block_throughput_estimator.is_none()
    }

    pub fn construction_auto_blocks(&mut self) -> bool {
        if self.block_placement_search.is_some() {
            self.block_message.clear();
            self.construction_message.clear();
            return true;
        }
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            self.set_block_error("A closed circuit is required to place block sections.");
            return false;
        };
        let config = block_physics_config();
        let search = match self.block_placement_cache.take() {
            Some(search) => Ok(search),
            None => BlockPlacementSearch::new(circuit, train, config),
        };
        match search {
            Ok(search) => {
                self.block_manual_search = None;
                self.block_baseline_validation = false;
                self.block_apply_best_requested = false;
                if self.block_layout.is_none() {
                    self.install_block_layout(search.best().clone(), true);
                }
                self.block_placement_search = Some(search);
                self.construction_advance_block_analysis(0);
                self.block_message.clear();
                self.construction_message.clear();
                true
            }
            Err(reason) => {
                self.set_block_error(reason);
                false
            }
        }
    }

    /// Install the best verified placement found so far and stop searching.
    pub fn construction_apply_best_block_sections(&mut self) -> bool {
        let Some(search) = self.block_placement_search.take() else {
            self.set_block_error("There is no automatic block placement search in progress.");
            return false;
        };
        if self.coaster_has_active_trains() {
            self.block_apply_best_requested = true;
            self.block_placement_search = Some(search);
            return true;
        }
        self.install_block_layout(search.best().clone(), true);
        if search.is_complete() {
            self.block_placement_cache = Some(search);
        }
        true
    }

    pub fn construction_add_block_section(&mut self, boundary_metres: f64) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        boundaries.push(boundary_metres);
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_move_block_section(&mut self, index: usize, boundary_metres: f64) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        let Some(boundary) = boundaries.get_mut(index) else {
            self.set_block_error("Block section index is out of range.");
            return false;
        };
        *boundary = boundary_metres;
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_remove_block_section(&mut self, index: usize) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        if index >= boundaries.len() {
            self.set_block_error("Block section index is out of range.");
            return false;
        }
        boundaries.remove(index);
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_block_candidates(&self) -> Vec<f64> {
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            return Vec::new();
        };
        let mut candidates: Vec<_> = brake_runs(&circuit, train.length_metres)
            .into_iter()
            .map(|run| run.boundary_metres)
            .collect();
        candidates.sort_by(f64::total_cmp);
        candidates.dedup();
        candidates
    }

    pub fn construction_set_block_sections(&mut self, boundaries: Vec<f64>) -> bool {
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            self.set_block_error("A closed circuit is required to edit block sections.");
            return false;
        };
        match BlockManualSearch::new(circuit, train, block_physics_config(), boundaries) {
            Ok(search) => {
                self.block_placement_search = None;
                self.block_apply_best_requested = false;
                self.block_baseline_validation = false;
                self.block_manual_search = Some(search);
                self.block_message.clear();
                self.construction_message.clear();
                true
            }
            Err(reason) => {
                self.set_block_error(reason);
                false
            }
        }
    }

    pub fn construction_block_report_json(&self) -> String {
        let Some(layout) = &self.block_layout else {
            let reasons = self.block_reasons_json();
            return format!(
                "{{\"sections\":[],\"trainCapacity\":0,\"capacitySearchComplete\":null,\"theoreticalHourlyRiders\":null,\"throughputStatus\":\"unavailable\",\"placementStatus\":\"idle\",\"placementSearchComplete\":null,\"reasons\":{reasons},\"notes\":[],\"suggestions\":[]}}"
            );
        };
        let sections = layout
            .boundaries()
            .iter()
            .enumerate()
            .map(|(index, boundary)| {
                let brake_pieces = layout.brake_pieces(index);
                let brake_piece_indices = brake_pieces
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let powered_hold_pieces = layout.powered_hold_pieces(index);
                let powered_hold_piece_indices = powered_hold_pieces
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let editable = !brake_pieces.is_empty();
                let powered_hold = !powered_hold_pieces.is_empty();
                let editable_index = layout
                    .boundaries()
                    .iter()
                    .enumerate()
                    .take(index)
                    .filter(|(previous_index, _)| {
                        !layout.brake_pieces(*previous_index).is_empty()
                    })
                    .count();
                let (reason, track_kind) = if editable {
                    (
                        "Placed on a level brake run long enough to hold a train.",
                        "blockBrake",
                    )
                } else if powered_hold {
                    (
                        "Fixed powered hold at the start of lift or launch track.",
                        kind_for_boundary(layout.circuit(), *boundary),
                    )
                } else {
                    (
                        "Fixed pass-through boundary with no stopping mechanism.",
                        kind_for_boundary(layout.circuit(), *boundary),
                    )
                };
                format!(
                    "{{\"boundaryMetres\":{boundary},\"brakeStartMetres\":{},\"brakePieceIndices\":[{brake_piece_indices}],\"poweredHoldStartMetres\":{},\"poweredHoldPieceIndices\":[{powered_hold_piece_indices}],\"editable\":{editable},\"editableIndex\":{},\"reason\":{},\"trackKind\":{}}}",
                    if editable {
                        layout.brake_start(index).to_string()
                    } else {
                        "null".into()
                    },
                    if powered_hold {
                        layout.brake_start(index).to_string()
                    } else {
                        "null".into()
                    },
                    if editable { editable_index.to_string() } else { "null".into() },
                    json_string(reason),
                    json_string(track_kind)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let reasons = self.block_reasons_json();
        let source = if self.block_layout_is_automatic {
            "automatic"
        } else {
            "manual"
        };
        let mut notes = vec![
            "Trains wait at a block brake while the next section is occupied.",
            "The hourly estimate averages three dispatch intervals after the first refill and excludes station loading time.",
            "Lift and launch controls can hold a train at the start of powered track.",
            "Fixed station and powered-hardware boundaries cannot be moved or removed.",
        ];
        if self.block_placement_search.is_some() {
            notes.push(
                "Automatic placement is still searching; the installed layout remains in use.",
            );
        } else if self.block_manual_search.is_some() {
            if self.block_baseline_validation {
                notes.push("The one-train circuit is being checked while the coaster operates.");
            } else {
                notes.push("The requested block edit is being validated; the installed layout remains in use.");
            }
        }
        if self.block_layout_change_pending() {
            notes.push("New dispatches are paused until the updated block layout is installed.");
        }
        if self.block_placement_search.is_none()
            && self.block_manual_search.is_none()
            && self.block_layout_is_automatic
            && !layout.placement_search_complete()
        {
            notes.push("Automatic placement is incomplete. The current layout may not support the maximum number of trains.");
        }
        if layout.capacity() == 0 {
            notes.push("A train stalled and could not complete this circuit. Adjust the layout before running the coaster.");
        } else if !layout.capacity_search_complete() {
            if editable_block_boundaries(layout).is_empty() {
                notes.push("The one-train circuit has not yet been verified; a train may stall.");
            } else {
                notes.push("The reported train capacity is a verified lower bound; additional capacity may be possible.");
            }
        }
        let notes = notes
            .into_iter()
            .map(json_string)
            .collect::<Vec<_>>()
            .join(",");
        let suggestions = block_layout_suggestions(layout)
            .iter()
            .map(|suggestion| json_string(suggestion))
            .collect::<Vec<_>>()
            .join(",");
        let throughput_status = if self.block_throughput_estimator.is_some() {
            "pending"
        } else if self.block_throughput_reason.is_empty() {
            "ready"
        } else {
            "unavailable"
        };
        let throughput = if throughput_status == "ready" {
            self.block_hourly_throughput.to_string()
        } else {
            "null".into()
        };
        let placement_status = if self.block_placement_search.is_some() {
            "searching"
        } else if self.block_manual_search.is_some() {
            "validating"
        } else if !self.block_layout_is_automatic {
            "manual"
        } else if layout.placement_search_complete() {
            "complete"
        } else {
            "incomplete"
        };
        let search_complete = if matches!(placement_status, "manual" | "validating") {
            "null"
        } else if placement_status == "complete" {
            "true"
        } else {
            "false"
        };
        let placement_work_completed = self
            .block_placement_search
            .as_ref()
            .map(BlockPlacementSearch::work_completed)
            .or_else(|| {
                self.block_manual_search
                    .as_ref()
                    .map(BlockManualSearch::work_completed)
            })
            .unwrap_or(0);
        let best_capacity = self
            .block_placement_search
            .as_ref()
            .map_or(layout.capacity(), |search| search.best().capacity());
        format!(
            "{{\"sections\":[{sections}],\"source\":{},\"trainCapacity\":{},\"capacitySearchComplete\":{},\"theoreticalHourlyRiders\":{throughput},\"throughputStatus\":{},\"placementStatus\":{},\"placementSearchComplete\":{search_complete},\"placementWorkCompleted\":{placement_work_completed},\"placementBestTrainCapacity\":{best_capacity},\"reasons\":{reasons},\"notes\":[{notes}],\"suggestions\":[{suggestions}]}}",
            json_string(source),
            layout.capacity(),
            layout.capacity_search_complete(),
            json_string(throughput_status),
            json_string(placement_status)
        )
    }

    /// Queue entrance and exit tiles as x, y, height triples.
    pub fn construction_station_positions(&self) -> Vec<i32> {
        self.construction
            .as_ref()
            .and_then(|construction| {
                construction
                    .pieces()
                    .iter()
                    .find(|piece| piece.kind == PieceKind::Station)
            })
            .and_then(station_positions)
            .map_or_else(Vec::new, |(entrance, exit)| {
                [entrance, exit]
                    .into_iter()
                    .flat_map(|position| {
                        [
                            i32::from(position.x),
                            i32::from(position.y),
                            i32::from(position.height),
                        ]
                    })
                    .collect()
            })
    }

    pub fn construction_train_count(&self) -> usize {
        self.block_layout.as_ref().map_or(0, |layout| {
            self.desired_train_count.unwrap_or(1).min(layout.capacity())
        })
    }

    pub fn construction_set_train_count(&mut self, count: usize) -> bool {
        if count == 0
            || self
                .block_layout
                .as_ref()
                .is_none_or(|layout| count > layout.capacity())
        {
            return false;
        }
        self.desired_train_count = Some(count);
        true
    }

    pub fn construction_set_open(&mut self, open: bool) -> bool {
        if !open {
            self.close_live_coaster();
            self.construction_message.clear();
            return true;
        }
        if self.coaster_open {
            return true;
        }
        if open && !self.construction_can_test() {
            self.construction_message =
                "A complete circuit is required to open the coaster.".into();
            return false;
        }
        let construction = self.construction.as_ref().expect("complete coaster exists");
        let Some(facility) = coaster_facility(&self.world, construction, COASTER_PLACE_ID, 0)
        else {
            self.construction_message = "Connect a queue and an exit path to the station.".into();
            return false;
        };
        let circuit = construction
            .circuit()
            .expect("complete circuit checked above");
        if self.block_layout.is_none() {
            let train = construction.model().train.clone();
            let config = block_physics_config();
            let layout = match block_layout_for_open(&circuit, &train, config) {
                Ok(layout) => layout,
                Err(reason) => {
                    self.set_block_error(reason);
                    return false;
                }
            };
            self.install_block_layout(layout, false);
            self.block_manual_search = Some(
                BlockManualSearch::new(circuit.clone(), train, config, Vec::new())
                    .expect("baseline inputs were validated"),
            );
            self.block_baseline_validation = true;
        }
        let operation = BlockOperation::new(
            self.block_layout
                .as_ref()
                .expect("open coaster has a block layout")
                .clone(),
        );
        self.coaster_operation = Some(operation);
        self.coaster_facility = Some(facility);
        self.construction_train = None;
        self.construction_recorder = None;
        self.construction_report = None;
        self.coaster_open = true;
        self.construction_message.clear();
        true
    }

    /// Car index, seat index, and guest ID for each occupied seat.
    pub fn construction_riders(&self) -> Vec<u32> {
        let Some(construction) = &self.construction else {
            return Vec::new();
        };
        let seats_per_car = u32::from(construction.model().train.rows_per_car)
            * u32::from(construction.model().train.seats_per_row);
        let mut trains: Vec<_> = self.coaster_riders.iter().collect();
        trains.sort_by_key(|(train_id, _)| **train_id);
        trains
            .into_iter()
            .flat_map(|(train_id, guests)| {
                guests.iter().enumerate().flat_map(move |(seat, guest)| {
                    let car_index = rider_car_index(
                        *train_id,
                        construction.model().train.cars,
                        seat / seats_per_car as usize,
                    );
                    [car_index, (seat as u32) % seats_per_car, guest.0]
                })
            })
            .collect()
    }
    pub fn construction_message(&self) -> String {
        if self.construction_message.is_empty() {
            self.construction_outcome.clone().unwrap_or_default()
        } else {
            self.construction_message.clone()
        }
    }
    pub fn construction_append(&mut self) {
        let obstacles = self.facility_obstacles();
        let result = self
            .construction
            .as_mut()
            .zip(self.selected)
            .ok_or("Select a piece first".to_string())
            .and_then(|(construction, selected)| {
                construction
                    .append(selected, &self.world, &obstacles)
                    .map_err(construction_error)
            });
        if result.is_ok() {
            self.close_live_coaster();
            self.clear_block_layout();
            self.construction_recorder = None;
            self.construction_report = None;
            self.construction_train = None;
            self.construction_outcome = None;
            self.select_compatible_piece();
        }
        self.construction_message = result.err().unwrap_or_default();
    }
    pub fn construction_undo(&mut self) {
        let Some(id) = self
            .construction
            .as_ref()
            .and_then(|construction| construction.piece_ids().last())
            .copied()
        else {
            self.construction_message = "There is no active track to undo.".into();
            return;
        };
        self.construction_delete(id);
    }
    pub fn construction_delete(&mut self, index: usize) {
        let result = self
            .construction
            .as_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.delete_from(index).map_err(construction_error));
        if result.is_ok() {
            self.close_live_coaster();
            self.clear_block_layout();
            self.construction_recorder = None;
            self.construction_report = None;
            let construction = self.construction.as_ref().expect("successful deletion");
            let station = construction
                .pieces()
                .iter()
                .chain(
                    construction
                        .disconnected_segments()
                        .iter()
                        .flat_map(|segment| &segment.pieces),
                )
                .find(|piece| piece.kind == PieceKind::Station)
                .cloned();
            if let (Some((circuit, train)), Some(station)) = (&mut self.construction_train, station)
            {
                *train = Train::new(
                    circuit,
                    &construction.model().train,
                    PhysicsConfig::default(),
                    station.length(),
                    0.0,
                );
                self.recovered_station = Some(station);
                self.construction_outcome =
                    Some("Train recovered to the station after track deletion.".into());
            } else {
                self.construction_train = None;
                self.recovered_station = None;
                self.construction_outcome = None;
            }
            self.construction_slow_steps = 0;
            self.select_compatible_piece();
        }
        self.construction_message = result.err().unwrap_or_default();
    }
    pub fn construction_close(&mut self) {
        let result = self
            .construction
            .as_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.close().map_err(construction_error));
        if result.is_ok() {
            self.clear_block_layout();
        }
        self.construction_message = result.err().unwrap_or_default();
    }
    pub fn construction_test(&mut self) {
        if self.coaster_open {
            self.construction_message = "Close the coaster before testing it.".into();
            return;
        }
        if !self.construction_can_test() {
            self.construction_message = "A closed circuit longer than the train is required".into();
            return;
        }
        let construction = self.construction.as_ref().expect("checked above");
        let circuit = construction.circuit().expect("checked above");
        let train = Train::new(
            &circuit,
            &construction.model().train,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            0.0,
            2.0,
        );
        self.construction_recorder = Some(TestRunRecorder::new(circuit.length()));
        self.construction_report = None;
        self.construction_train = Some((circuit, train));
        self.recovered_station = None;
        self.construction_slow_steps = 0;
        self.construction_outcome = None;
        self.construction_message = "Test train running.".into();
    }
    pub fn construction_test_report_json(&self) -> String {
        let Some(report) = &self.construction_report else {
            return "null".into();
        };
        let circuit = &self
            .construction_train
            .as_ref()
            .expect("report requires train")
            .0;
        let samples = report.samples.iter().map(|sample| {
            let position = circuit.sample(sample.distance).position / 4.0;
            format!("{{\"distance\":{},\"x\":{},\"y\":{},\"z\":{},\"speed\":{},\"verticalG\":{},\"lateralG\":{},\"longitudinalG\":{},\"airtime\":{}}}", sample.distance, position.x, position.y, position.z, sample.speed, sample.vertical_g, sample.lateral_g, sample.longitudinal_g, sample.airtime)
        }).collect::<Vec<_>>().join(",");
        let notes = report
            .notes
            .iter()
            .map(|note| json_string(note))
            .collect::<Vec<_>>()
            .join(",");
        let events = report.proximity_events.iter().map(|event| {
            let kind = match event.kind {
                ProximityKind::NearMiss => "nearMiss",
                ProximityKind::Headchopper => "headchopper",
                ProximityKind::Dueling => "dueling",
                ProximityKind::Tunnel => "tunnel",
                ProximityKind::TerrainHugging => "terrainHugging",
            };
            format!("{{\"kind\":\"{kind}\",\"pieceIndex\":{},\"distance\":{},\"x\":{},\"y\":{},\"z\":{},\"strength\":{}}}", event.piece_index, event.distance, event.position.x / 4.0, event.position.y / 4.0, event.position.z / 4.0, event.strength)
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"samples\":[{samples}],\"notes\":[{notes}],\"proximityEvents\":[{events}],\"rating\":{{\"excitement\":{},\"intensity\":{},\"nausea\":{}}}}}",
            report.excitement, report.intensity, report.nausea
        )
    }
    pub fn construction_rails(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_rails(&self.construction_preview(), &self.world),
            19,
        )
    }
    pub fn construction_ties(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_ties(&self.construction_preview(), &self.world),
            13,
        )
    }
    pub fn construction_supports(&mut self) -> Vec<f32> {
        if !self.construction_preview_valid() {
            return Vec::new();
        }
        let construction = self
            .construction
            .as_ref()
            .expect("valid preview requires construction");
        let placed: Vec<_> = construction
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
        let preview = self.construction_preview();
        let piece = preview
            .first()
            .expect("valid preview requires a selected piece");
        sync_support_obstacles(
            &placed,
            &construction.model().train,
            &mut self.support_render_cache,
        );
        let mut obstacles: Vec<_> = placed
            .iter()
            .flat_map(|(id, _)| self.support_render_cache[id].obstacles.iter().copied())
            .collect();
        obstacles.extend(self.facility_obstacles());
        match render_preview_supports(
            piece,
            &obstacles,
            &self.world,
            construction.model().support_style,
        ) {
            Ok(supports) => self.encode_construction_geometry(supports, 7),
            Err(_) => {
                self.construction_message = "Could not route preview supports.".into();
                Vec::new()
            }
        }
    }
    pub fn construction_placed_rails(&self) -> Vec<f32> {
        self.construction_render(render_rails, 19)
    }
    pub fn construction_rail_distances(&self) -> Vec<f32> {
        let Some(construction) = &self.construction else {
            return Vec::new();
        };
        let pieces = construction.pieces().iter().chain(
            construction
                .disconnected_segments()
                .iter()
                .flat_map(|segment| &segment.pieces),
        );
        let mut distances = Vec::new();
        let mut piece_start = 0.0;
        for piece in pieces {
            let length = piece.length();
            let intervals = (length / 2.0).ceil() as usize;
            for index in 0..intervals {
                distances.push((piece_start + length * index as f64 / intervals as f64) as f32);
                distances
                    .push((piece_start + length * (index + 1) as f64 / intervals as f64) as f32);
            }
            piece_start += length;
        }
        distances
    }
    pub fn construction_placed_ties(&self) -> Vec<f32> {
        self.construction_render(render_ties, 13)
    }
    pub fn construction_placed_supports(&mut self) -> Vec<f32> {
        let facility_obstacles = self.facility_obstacles();
        let Some(construction) = &self.construction else {
            return Vec::new();
        };
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
        match render_cached_supports(
            &entries,
            &self.world,
            &construction.model().train,
            construction.model().support_style,
            &facility_obstacles,
            &mut self.support_render_cache,
        ) {
            Ok(supports) => self.encode_construction_geometry(supports, 7),
            Err(_) => {
                self.construction_message = "Could not route supports for the placed track.".into();
                Vec::new()
            }
        }
    }
    pub fn construction_train_poses(&self) -> Vec<f32> {
        let config = &self
            .construction
            .as_ref()
            .expect("train requires construction")
            .model()
            .train;
        let mut result = Vec::new();
        if let Some(operation) = &self.coaster_operation {
            let circuit = self
                .block_layout
                .as_ref()
                .expect("operation has a block layout")
                .circuit();
            let mut trains: Vec<_> = operation.trains().collect();
            trains.sort_by_key(|(train_id, _)| *train_id);
            for (train_id, train) in trains {
                let riders = self
                    .coaster_riders
                    .get(&train_id)
                    .map_or(&[][..], Vec::as_slice);
                append_train_poses(&mut result, circuit, train, config, train_id, riders, None);
            }
            return result;
        }

        let Some((circuit, train)) = &self.construction_train else {
            return result;
        };
        append_train_poses(
            &mut result,
            circuit,
            train,
            config,
            0,
            &[],
            self.recovered_station.as_ref(),
        );
        result
    }
    pub fn state_hash(&self) -> u64 {
        self.simulation.walking().state_hash()
    }
}

impl WasmSimulation {
    fn coaster_has_active_trains(&self) -> bool {
        self.coaster_operation
            .as_ref()
            .is_some_and(|operation| operation.trains().next().is_some())
    }

    fn block_layout_change_pending(&self) -> bool {
        if let Some(Ok(layout)) = self
            .block_manual_search
            .as_ref()
            .and_then(BlockManualSearch::result)
        {
            let installed = self
                .block_layout
                .as_ref()
                .expect("validation has an installed layout");
            return block_layout_fingerprint(layout) != block_layout_fingerprint(installed);
        }
        let Some(search) = &self.block_placement_search else {
            return false;
        };
        let installed = self
            .block_layout
            .as_ref()
            .expect("search has an installed layout");
        let best = search.best();
        (self.block_apply_best_requested
            || best.capacity() > installed.capacity()
            || confirmed_stalled_baseline(best, installed)
            || (search.is_complete() && best.capacity() >= installed.capacity()))
            && block_layout_fingerprint(best) != block_layout_fingerprint(installed)
    }

    fn proximity_obstacles(&self) -> Vec<ClearanceBox> {
        self.facility_obstacles()
            .into_iter()
            .map(|obstacle| ClearanceBox {
                min: obstacle.min,
                max: obstacle.max,
            })
            .collect()
    }

    fn proximity_supports(&self) -> Vec<SupportSegment> {
        self.support_render_cache
            .values()
            .flat_map(|cached| {
                cached
                    .segments
                    .as_ref()
                    .expect("placed support must be routable")
                    .iter()
                    .copied()
            })
            .collect()
    }

    fn close_live_coaster(&mut self) {
        self.recover_live_trains();
        if let Some(facility) = &self.coaster_facility {
            self.simulation.close_coaster(&self.world, facility);
        }
        self.coaster_open = false;
        self.coaster_operation = None;
        self.coaster_facility = None;
    }

    fn recover_live_trains(&mut self) {
        let outcomes = self
            .coaster_operation
            .take()
            .map_or_else(Vec::new, |mut operation| operation.recover_all());
        for (train_id, outcome) in outcomes {
            self.finish_live_train(train_id, outcome);
        }
    }

    fn finish_live_train(&mut self, train_id: u64, mut outcome: CoasterOutcome) {
        let Some(riders) = self.coaster_riders.remove(&train_id) else {
            return;
        };
        let facility = *self
            .coaster_facility
            .as_ref()
            .expect("riders have an open coaster");
        if outcome.recovered {
            self.simulation
                .finish_coaster_train(&self.world, &facility, train_id, &[], &[], true);
            return;
        }

        self.construction_placed_supports();
        let obstacles = self.proximity_obstacles();
        let supports = self.proximity_supports();
        let construction = self.construction.as_ref().expect("open coaster has track");
        let geometry = scan_proximity_with_supports(
            &self.world,
            construction.pieces(),
            &construction.model().train,
            self.circuit.pieces(),
            &obstacles,
            &supports,
        );
        for report in &mut outcome.reports {
            let events = geometry.events_for_run(&report.samples);
            report.add_proximity_events(&events);
        }
        let seats_per_car = usize::from(construction.model().train.rows_per_car)
            * usize::from(construction.model().train.seats_per_row);
        let ratings: Vec<_> = (0..riders.len())
            .map(|seat| {
                let report = &outcome.reports[seat / seats_per_car];
                (report.excitement, report.intensity, report.nausea)
            })
            .collect();
        let proximity_events: Vec<_> = (0..riders.len())
            .map(|seat| {
                outcome.reports[seat / seats_per_car]
                    .proximity_events
                    .clone()
            })
            .collect();
        self.simulation.finish_coaster_train(
            &self.world,
            &facility,
            train_id,
            &ratings,
            &proximity_events,
            false,
        );
    }

    fn construction_circuit_and_train(&self) -> Option<(Circuit, TrainConfiguration)> {
        let construction = self.construction.as_ref()?;
        Some((
            construction.circuit().ok()?,
            construction.model().train.clone(),
        ))
    }

    fn set_block_error(&mut self, reason: &str) {
        self.block_message = reason.to_owned();
        self.construction_message = self.block_message.clone();
    }

    fn block_reasons_json(&self) -> String {
        let reasons = [&self.block_message, &self.block_throughput_reason]
            .into_iter()
            .filter(|reason| !reason.is_empty())
            .map(|reason| json_string(reason))
            .collect::<Vec<_>>()
            .join(",");
        format!("[{reasons}]")
    }

    fn clear_block_layout(&mut self) {
        self.block_layout = None;
        self.desired_train_count = None;
        self.block_placement_search = None;
        self.block_placement_cache = None;
        self.block_apply_best_requested = false;
        self.block_manual_search = None;
        self.block_baseline_validation = false;
        self.block_layout_is_automatic = false;
        self.block_message.clear();
        self.block_hourly_throughput = 0.0;
        self.block_throughput_reason.clear();
        self.block_throughput_estimator = None;
        self.block_analysis_extra_to_throughput = false;
        self.block_throughput_cache.clear();
    }

    fn install_block_layout(&mut self, layout: BlockLayout, automatic: bool) {
        self.block_placement_search = None;
        self.block_apply_best_requested = false;
        self.block_manual_search = None;
        self.block_baseline_validation = false;
        let fingerprint = block_layout_fingerprint(&layout);
        let (throughput, throughput_reason, estimator) = if layout.capacity() == 0 {
            (0.0, "A train could not complete the circuit.".into(), None)
        } else {
            match self.block_throughput_cache.get(&fingerprint) {
                Some((throughput, reason)) => (*throughput, reason.clone(), None),
                None => (0.0, String::new(), Some(ThroughputEstimator::new(&layout))),
            }
        };
        if self.coaster_open {
            self.recover_live_trains();
            self.coaster_operation = Some(BlockOperation::new(layout.clone()));
        }
        self.block_layout = Some(layout);
        self.block_layout_is_automatic = automatic;
        self.block_hourly_throughput = throughput;
        self.block_throughput_reason = throughput_reason;
        self.block_throughput_estimator = estimator;
        self.block_message.clear();
        self.construction_message.clear();
    }

    fn checked_path_preview(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
        style: u8,
    ) -> Result<Vec<PathPreviewTile>, PathBuildError> {
        let mut preview = self.world.preview_path_tiles(start, end, kind, style)?;
        let mut blocked_by_facility = false;
        for tile in &mut preview {
            if self.facilities.occupies_path_position(tile.path.position) {
                tile.refusal = Some(PathBuildError::FacilityOccupied);
                blocked_by_facility = true;
            } else if blocked_by_facility && tile.refusal.is_none() {
                tile.refusal = Some(PathBuildError::Disconnected);
            }
        }
        if preview.iter().all(|tile| tile.refusal.is_none()) {
            let proposed = self.world.preview_path_world(start, end, kind)?;
            if validate_facility_path_access(
                &proposed,
                &self.facilities,
                self.construction.as_ref(),
            )
            .is_err()
            {
                for tile in &mut preview {
                    tile.refusal = Some(PathBuildError::FacilityAccess);
                }
            }
        }
        Ok(preview)
    }

    fn facility_openings(&self) -> Vec<PathOpening> {
        let mut openings = self.facilities.access_openings(&self.world);
        if let Some(coaster_openings) = self.construction.as_ref().and_then(coaster_access_openings)
        {
            openings.extend(coaster_openings);
        }
        openings
    }

    fn facility_obstacles(&self) -> Vec<SupportObstacle> {
        self.facilities
            .buildings()
            .iter()
            .map(|building| {
                let (width, depth) = building.kind.footprint(building.direction);
                let x = f64::from(building.position.x) * 4.0;
                let y = f64::from(building.position.y) * 4.0;
                let z = f64::from(building.position.height);
                let height = facility_height(building.kind);
                SupportObstacle {
                    min: Vec3::new(x, y, z),
                    max: Vec3::new(
                        x + f64::from(width) * 4.0,
                        y + f64::from(depth) * 4.0,
                        z + height,
                    ),
                }
            })
            .collect()
    }
    fn select_compatible_piece(&mut self) {
        let Some(construction) = &self.construction else {
            return;
        };
        let available = construction.available_choices(&self.world, &self.facility_obstacles());
        if self
            .selected
            .is_some_and(|selected| available.iter().any(|(piece, _)| piece.id == selected))
        {
            return;
        }
        self.selected = available
            .into_iter()
            .find(|(_, choice)| choice.result.is_ok())
            .map(|(piece, _)| piece.id);
    }
    fn construction_render(
        &self,
        render: fn(&[Piece], &World) -> Vec<f32>,
        stride: usize,
    ) -> Vec<f32> {
        let Some(construction) = &self.construction else {
            return Vec::new();
        };
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
        let pieces: Vec<_> = entries.iter().map(|(_, piece)| (*piece).clone()).collect();
        let mut geometry = render(&pieces, &self.world);
        for record in geometry.chunks_exact_mut(stride) {
            record[0] = *entries[record[0] as usize].0 as f32;
        }
        self.encode_construction_geometry(geometry, stride)
    }
    fn encode_construction_geometry(&self, mut geometry: Vec<f32>, stride: usize) -> Vec<f32> {
        if self.construction.as_ref().is_some_and(|construction| {
            construction.model().support_style == SupportStyle::SteelColumns
        }) {
            // Steel records use -(piece ID + 1); consumers decode before picking.
            for record in geometry.chunks_exact_mut(stride) {
                record[0] = -record[0] - 1.0;
            }
        } else if stride != 7
            && self
                .construction
                .as_ref()
                .is_some_and(|construction| construction.model().track_material == Material::Wood)
        {
            for record in geometry.chunks_exact_mut(stride) {
                record[0] += 1_000_000.0;
            }
        }
        geometry
    }
    fn construction_preview(&self) -> Vec<Piece> {
        let Some(construction) = &self.construction else {
            return Vec::new();
        };
        if construction.is_closed() {
            return Vec::new();
        }
        construction
            .available_choices(&self.world, &self.facility_obstacles())
            .into_iter()
            .filter_map(|(piece, _)| (Some(piece.id) == self.selected).then_some(piece))
            .collect()
    }
}

/// Each catalog piece's centreline and up vector, in metres, in a frame where the piece enters at
/// the origin heading along +x, for drawing piece icons.
#[wasm_bindgen]
pub fn piece_shapes_json() -> String {
    const SAMPLES: usize = 32;
    let shapes: Vec<String> = catalog()
        .iter()
        .map(|piece| {
            let origin = piece.entry.position();
            let angle = -(piece.entry.heading as i32 as f64) * std::f64::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let local = |vector: Vec3| {
                [vector.x * cos - vector.y * sin, vector.x * sin + vector.y * cos, vector.z]
            };
            let points: Vec<String> = (0..=SAMPLES)
                .map(|index| {
                    let sample = piece.sample(piece.length() * index as f64 / SAMPLES as f64);
                    let position = local(sample.position - origin);
                    let up = local(sample.up);
                    format!(
                        "[{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}]",
                        position[0], position[1], position[2], up[0], up[1], up[2]
                    )
                })
                .collect();
            let turn = (piece.exit.heading as i32 - piece.entry.heading as i32 + 4).rem_euclid(8) - 4;
            format!(
                "{{\"id\":\"{:?}\",\"name\":{},\"kind\":\"{:?}\",\"turn\":{turn},\"entrySlope\":\"{:?}\",\"exitSlope\":\"{:?}\",\"entryBank\":\"{:?}\",\"exitBank\":\"{:?}\",\"points\":[{}]}}",
                piece.id,
                json_string(piece.name),
                piece.kind,
                piece.entry.slope,
                piece.exit.slope,
                piece.entry.bank,
                piece.exit.bank,
                points.join(",")
            )
        })
        .collect();
    format!("[{}]", shapes.join(","))
}

fn piece_group(id: PieceId) -> &'static str {
    let name = format!("{id:?}");
    if name.contains("Bank") {
        "bank"
    } else if name.contains("Turn") {
        "turn"
    } else if name.contains("Loop")
        || name.contains("Roll")
        || name.contains("Corkscrew")
        || name.contains("Inversion")
    {
        "inversion"
    } else if name.contains("Up") || name.contains("Down") || name.contains("Vertical") {
        "slope"
    } else if name == "Straight" {
        "straight"
    } else {
        "special"
    }
}

fn json_string(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write;
                write!(result, "\\u{:04x}", character as u32).expect("write to string");
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

fn parse_path_kind(kind: &str) -> Option<PathKind> {
    match kind {
        "normal" => Some(PathKind::Normal),
        "queue" => Some(PathKind::Queue),
        _ => None,
    }
}

fn path_positions(coordinates: &[i32]) -> Result<(Position, Position), &'static str> {
    let [start_x, start_y, start_height, end_x, end_y, end_height] = coordinates else {
        return Err("Path route needs six coordinates");
    };
    let position = |x: i32, y: i32, height: i32| -> Result<Position, &'static str> {
        Ok(Position {
            x: u16::try_from(x).map_err(|_| "Path coordinates are outside the supported range")?,
            y: u16::try_from(y).map_err(|_| "Path coordinates are outside the supported range")?,
            height: i16::try_from(height)
                .map_err(|_| "Path coordinates are outside the supported range")?,
        })
    };
    Ok((
        position(*start_x, *start_y, *start_height)?,
        position(*end_x, *end_y, *end_height)?,
    ))
}

fn parse_corner(corner: u8) -> Option<Corner> {
    match corner {
        0 => Some(Corner::NorthWest),
        1 => Some(Corner::NorthEast),
        2 => Some(Corner::SouthEast),
        3 => Some(Corner::SouthWest),
        _ => None,
    }
}

struct TerrainParameters {
    x: u16,
    y: u16,
    size: u8,
    corner: u8,
    steps: i16,
    height: i16,
    bottom: i16,
    top: i16,
}

fn parse_terrain_parameters(values: Vec<i32>) -> Result<TerrainParameters, &'static str> {
    let [x, y, size, corner, steps, height, bottom, top] = values
        .try_into()
        .map_err(|_| "Terrain edit needs eight integer parameters")?;
    Ok(TerrainParameters {
        x: u16::try_from(x).map_err(|_| "Terrain coordinates are outside the supported range")?,
        y: u16::try_from(y).map_err(|_| "Terrain coordinates are outside the supported range")?,
        size: u8::try_from(size)
            .map_err(|_| "Terrain brush size is outside the supported range")?,
        corner: u8::try_from(corner)
            .map_err(|_| "Terrain corner is outside the supported range")?,
        steps: i16::try_from(steps)
            .map_err(|_| "Terrain step count is outside the supported range")?,
        height: i16::try_from(height)
            .map_err(|_| "Terrain height is outside the supported range")?,
        bottom: i16::try_from(bottom)
            .map_err(|_| "Excavation bottom is outside the supported range")?,
        top: i16::try_from(top).map_err(|_| "Excavation top is outside the supported range")?,
    })
}

fn terrain_edit_error(error: TerrainEditError) -> &'static str {
    match error {
        TerrainEditError::OutsideWorld => "Terrain edit is outside the park",
        TerrainEditError::InvalidBrushSize => "Brush size must be between 1 and 9",
        TerrainEditError::InvalidStepCount => "Terrain change must be at least one step",
        TerrainEditError::InvalidInterval => "Excavation interval is invalid",
        TerrainEditError::CarveAboveTerrain => "Excavation exceeds the terrain height",
        TerrainEditError::NoWater => "There is no water to lower in this area",
        TerrainEditError::HeightOverflow => "Terrain height exceeds the supported range",
        TerrainEditError::SceneryObstructed => "Terrain edit would disturb scenery",
    }
}

fn path_slope_code(slope: Option<Direction>) -> i32 {
    match slope {
        None => 0,
        Some(Direction::North) => 1,
        Some(Direction::East) => 2,
        Some(Direction::South) => 3,
        Some(Direction::West) => 4,
    }
}

fn path_build_error(error: PathBuildError) -> &'static str {
    match error {
        PathBuildError::OutsideWorld => "Path route is outside the park",
        PathBuildError::UnsupportedTerrain => "Path route crosses unsupported terrain",
        PathBuildError::EndpointHeight => "Path endpoint heights do not match the route",
        PathBuildError::Disconnected => "Path route does not connect across the terrain",
        PathBuildError::Occupied => "Path route overlaps another path",
        PathBuildError::Unsupported => "Path route lacks terrain support or clearance",
        PathBuildError::QueueConnections => {
            "A queue path must have at most one normal path entrance"
        }
        PathBuildError::FacilityOccupied => "Path overlaps a facility",
        PathBuildError::FacilityAccess => "Path conflicts with attraction access",
    }
}

fn path_error_code(error: PathBuildError) -> i32 {
    match error {
        PathBuildError::OutsideWorld => 1,
        PathBuildError::UnsupportedTerrain => 2,
        PathBuildError::EndpointHeight => 3,
        PathBuildError::Disconnected => 4,
        PathBuildError::Occupied => 5,
        PathBuildError::Unsupported => 6,
        PathBuildError::QueueConnections => 7,
        PathBuildError::FacilityOccupied => 8,
        PathBuildError::FacilityAccess => 9,
    }
}

fn construction_error(error: theme_park_sim::track::construction::ConstructionError) -> String {
    use theme_park_sim::track::clearance::ClearanceCollision;
    use theme_park_sim::track::construction::ConstructionError;
    match error {
        ConstructionError::Closed => "Delete track before editing a closed circuit.",
        ConstructionError::Unavailable => "This piece cannot connect here.",
        ConstructionError::AmbiguousConnection => "More than one track segment connects here.",
        ConstructionError::ModelForbidden => "This coaster model cannot use this piece.",
        ConstructionError::NoSupportRoute => "Supports cannot be placed here.",
        ConstructionError::SupportSearchExhausted => {
            "Support routing search exhausted; placement is unresolved."
        }
        ConstructionError::InvalidIndex => "That track piece does not exist.",
        ConstructionError::NotClosed => "Close the circuit first.",
        ConstructionError::InvalidCircuit => "The track must connect back to its station.",
        ConstructionError::Clearance(ClearanceCollision::Terrain) => {
            "The train would collide with terrain."
        }
        ConstructionError::Clearance(ClearanceCollision::Path) => {
            "The train would collide with a path."
        }
        ConstructionError::Clearance(ClearanceCollision::Scenery) => {
            "The train would collide with scenery."
        }
        ConstructionError::Clearance(ClearanceCollision::Track) => {
            "The train would collide with track."
        }
        ConstructionError::Clearance(ClearanceCollision::OutsideWorld) => {
            "The track would leave the park."
        }
    }
    .into()
}

fn append_vector(result: &mut Vec<f32>, vector: Vec3) {
    result.extend([vector.x as f32, vector.y as f32, vector.z as f32]);
}

fn scenery_id(item: &Scenery) -> String {
    format!(
        "{}:{}:{}:{}",
        item.position.x, item.position.y, item.position.height, item.quarter
    )
}

fn scenery_kind(name: &str) -> Option<SceneryKind> {
    match name {
        "tree_oval" => Some(SceneryKind::TreeOval),
        "bush_flowering" => Some(SceneryKind::BushFlowering),
        "flower_bed" => Some(SceneryKind::FlowerBed),
        "bench" => Some(SceneryKind::Bench),
        "lamp" => Some(SceneryKind::Lamp),
        "fountain" => Some(SceneryKind::Fountain),
        "planter" => Some(SceneryKind::Planter),
        "tree_cone" => Some(SceneryKind::TreeCone),
        "tree_cone_ochre" => Some(SceneryKind::TreeConeOchre),
        "tree_cypress" => Some(SceneryKind::TreeCypress),
        "tree_cypress_short" => Some(SceneryKind::TreeCypressShort),
        "tree_oval_coral" => Some(SceneryKind::TreeOvalCoral),
        "tree_tiers" => Some(SceneryKind::TreeTiers),
        "fence" => Some(SceneryKind::Fence),
        "signpost" => Some(SceneryKind::Signpost),
        "bin" => Some(SceneryKind::Bin),
        "balloon_cart" => Some(SceneryKind::BalloonCart),
        _ => None,
    }
}

fn scenery_kind_name(kind: SceneryKind) -> &'static str {
    match kind {
        SceneryKind::TreeOval => "tree_oval",
        SceneryKind::BushFlowering => "bush_flowering",
        SceneryKind::FlowerBed => "flower_bed",
        SceneryKind::Bench => "bench",
        SceneryKind::Lamp => "lamp",
        SceneryKind::Fountain => "fountain",
        SceneryKind::Planter => "planter",
        SceneryKind::TreeCone => "tree_cone",
        SceneryKind::TreeConeOchre => "tree_cone_ochre",
        SceneryKind::TreeCypress => "tree_cypress",
        SceneryKind::TreeCypressShort => "tree_cypress_short",
        SceneryKind::TreeOvalCoral => "tree_oval_coral",
        SceneryKind::TreeTiers => "tree_tiers",
        SceneryKind::Fence => "fence",
        SceneryKind::Signpost => "signpost",
        SceneryKind::Bin => "bin",
        SceneryKind::BalloonCart => "balloon_cart",
    }
}

fn boxes_overlap(a: ClearanceBox, b: ClearanceBox) -> bool {
    a.min.x < b.max.x
        && b.min.x < a.max.x
        && a.min.y < b.max.y
        && b.min.y < a.max.y
        && a.min.z < b.max.z
        && b.min.z < a.max.z
}

fn facility_height(kind: FacilityKind) -> f64 {
    match kind {
        FacilityKind::Carousel | FacilityKind::FerrisWheel => 12.0,
        _ => 4.0,
    }
}

fn facility_intersects_track(
    kind: FacilityKind,
    position: Position,
    direction: Direction,
    pieces: &[Piece],
    train: &TrainConfiguration,
) -> bool {
    let (width, depth) = kind.footprint(direction);
    let min_x = f64::from(position.x) * 4.0;
    let min_y = f64::from(position.y) * 4.0;
    let min_z = f64::from(position.height);
    let height = facility_height(kind);
    track_support_obstacles(pieces, train)
        .iter()
        .any(|obstacle| {
            obstacle.max.x > min_x
                && obstacle.min.x < min_x + f64::from(width) * 4.0
                && obstacle.max.y > min_y
                && obstacle.min.y < min_y + f64::from(depth) * 4.0
                && obstacle.max.z > min_z
                && obstacle.min.z < min_z + height
        })
}

fn append_pose(result: &mut Vec<f32>, index: usize, sample: TrackSample) {
    result.push(index as f32);
    append_vector(result, sample.position / 4.0);
    append_vector(result, sample.up.cross(sample.tangent));
    append_vector(result, sample.up);
}

fn block_physics_config() -> PhysicsConfig {
    PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    }
}

fn editable_block_boundaries(layout: &BlockLayout) -> Vec<f64> {
    layout
        .boundaries()
        .iter()
        .enumerate()
        .filter(|(index, _)| !layout.brake_pieces(*index).is_empty())
        .map(|(_, boundary)| *boundary)
        .collect()
}

fn block_layout_fingerprint(layout: &BlockLayout) -> Vec<u64> {
    let mut fingerprint = vec![layout.capacity() as u64];
    for (index, boundary) in layout.boundaries().iter().enumerate() {
        fingerprint.extend(block_boundary_fingerprint(
            *boundary,
            layout.brake_pieces(index),
            layout.powered_hold_pieces(index),
        ));
    }
    fingerprint
}

fn block_boundary_fingerprint(
    boundary: f64,
    brake_pieces: &[usize],
    powered_hold_pieces: &[usize],
) -> Vec<u64> {
    let mut fingerprint = vec![boundary.to_bits(), brake_pieces.len() as u64];
    fingerprint.extend(brake_pieces.iter().map(|piece| *piece as u64));
    fingerprint.push(powered_hold_pieces.len() as u64);
    fingerprint.extend(powered_hold_pieces.iter().map(|piece| *piece as u64));
    fingerprint
}

fn confirmed_stalled_baseline(best: &BlockLayout, installed: &BlockLayout) -> bool {
    best.capacity() == 0
        && best.capacity_search_complete()
        && !installed.capacity_search_complete()
        && editable_block_boundaries(installed).is_empty()
}

fn kind_for_boundary(circuit: &Circuit, distance: f64) -> &'static str {
    match circuit.pieces()[circuit.piece_index(distance - 1e-6)].kind {
        PieceKind::Station => "station",
        PieceKind::Lift => "lift",
        PieceKind::Launch => "launch",
        PieceKind::Brakes => "brake",
        PieceKind::Track => "track",
    }
}

fn rider_car_index(train_id: u64, cars_per_train: u8, car_index: usize) -> u32 {
    let global_index = train_id
        .checked_mul(u64::from(cars_per_train))
        .and_then(|offset| offset.checked_add(car_index as u64))
        .expect("train car index overflow");
    u32::try_from(global_index).expect("train car index exceeds snapshot range")
}

fn append_train_poses(
    result: &mut Vec<f32>,
    circuit: &Circuit,
    train: &Train,
    configuration: &TrainConfiguration,
    train_id: u64,
    riders: &[GuestId],
    recovered_station: Option<&Piece>,
) {
    let cars = usize::from(configuration.cars);
    let seats_per_car =
        usize::from(configuration.rows_per_car) * usize::from(configuration.seats_per_row);
    for car_index in 0..cars {
        let offset = car_index as f64 * configuration.length_metres / f64::from(configuration.cars);
        let sample = recovered_station.map_or_else(
            || circuit.sample(train.distance() - offset),
            |station| station.sample((station.length() - offset).max(0.0)),
        );
        result.push(rider_car_index(train_id, configuration.cars, car_index) as f32);
        append_vector(result, sample.position / 4.0);
        append_vector(result, sample.tangent);
        append_vector(result, sample.up);
        let occupied = riders
            .len()
            .saturating_sub(car_index * seats_per_car)
            .min(seats_per_car);
        result.push(((1_u32 << occupied) - 1) as f32);
    }
}

fn block_layout_for_open(
    circuit: &Circuit,
    train: &TrainConfiguration,
    config: PhysicsConfig,
) -> Result<BlockLayout, &'static str> {
    BlockLayout::unverified_baseline(circuit, train, config)
}

struct ThroughputEstimator {
    operation: BlockOperation,
    max_idle_steps: usize,
    step: usize,
    last_dispatch_step: usize,
    first_measured_step: Option<usize>,
    measured_intervals: usize,
    seats_per_train: usize,
}

impl ThroughputEstimator {
    fn new(layout: &BlockLayout) -> Self {
        let max_idle_seconds = (layout.circuit().length() * 4.0 + 60.0).clamp(300.0, 3600.0);
        let mut operation = BlockOperation::new_without_reports(layout.clone());
        while operation.can_dispatch() {
            operation.dispatch();
        }
        Self {
            operation,
            max_idle_steps: (max_idle_seconds / TIME_STEP) as usize,
            step: 0,
            last_dispatch_step: 0,
            first_measured_step: None,
            measured_intervals: 0,
            seats_per_train: usize::from(layout.train().cars)
                * usize::from(layout.train().rows_per_car)
                * usize::from(layout.train().seats_per_row),
        }
    }

    fn advance(&mut self, work_steps: usize) -> Option<Result<f64, &'static str>> {
        const MEASURED_INTERVALS: usize = 3;
        for _ in 0..work_steps {
            if self.step - self.last_dispatch_step >= self.max_idle_steps {
                return Some(Err(
                    "The throughput simulation did not observe enough sustained dispatches.",
                ));
            }
            self.step += 1;
            if self
                .operation
                .step(&[])
                .iter()
                .any(|(_, outcome)| outcome.recovered)
            {
                return Some(Err("A train recovered during the throughput simulation."));
            }
            while self.operation.can_dispatch() {
                self.operation.dispatch();
                self.last_dispatch_step = self.step;
                let Some(first_step) = self.first_measured_step else {
                    self.first_measured_step = Some(self.step);
                    continue;
                };
                self.measured_intervals += 1;
                if self.measured_intervals >= MEASURED_INTERVALS && self.step > first_step {
                    let elapsed_seconds = (self.step - first_step) as f64 * TIME_STEP;
                    return Some(Ok(self.seats_per_train as f64
                        * self.measured_intervals as f64
                        * 3600.0
                        / elapsed_seconds));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod scenery_bridge_tests {
    use super::WasmSimulation;

    #[test]
    fn scenery_snapshot_place_and_remove_use_stable_ids() {
        let mut simulation = WasmSimulation::new(1, 0);
        assert_ne!(simulation.scenery_json(), "[]");
        assert!(simulation.place_scenery("bench", 210, 245, 1, 2));
        simulation.tick();
        let snapshot = simulation.scenery_json();
        assert!(snapshot.contains(
            "\"kind\":\"bench\",\"x\":210,\"y\":245,\"z\":8,\"rotation\":1,\"quarter\":2"
        ));
        let id = "210:245:8:2";
        assert!(simulation.remove_scenery(id));
        assert!(!simulation.remove_scenery(id));
        assert!(!simulation.scenery_json().contains("\"x\":210,\"y\":245"));
    }

    #[test]
    fn scenery_and_facilities_reject_overlap_in_both_directions() {
        let mut simulation = WasmSimulation::new(1, 0);
        assert!(!simulation.place_scenery("tree_oval", 220, 254, 0, 0));
        assert!(!simulation.place_scenery("tree_oval", 218, 256, 0, 0));
        assert!(!simulation.place_scenery("tree_oval", 210, 245, 0, 4));
        assert!(simulation.place_scenery("tree_oval", 210, 245, 0, 0));
        assert!(!simulation.place_facility("food_stall", 210, 245, 0));
        assert!(!simulation.place_scenery("unknown", 210, 244, 0, 0));
        assert!(!simulation.scenery_message().is_empty());
    }

    #[test]
    fn scenery_identity_includes_height() {
        use theme_park_sim::world::{Position, Scenery, SceneryKind};

        let lower = Scenery {
            kind: SceneryKind::Lamp,
            position: Position {
                x: 1,
                y: 2,
                height: 3,
            },
            quarter: 0,
            rotation: 0,
        };
        let upper = Scenery {
            position: Position {
                height: 10,
                ..lower.position
            },
            ..lower
        };
        assert_ne!(super::scenery_id(&lower), super::scenery_id(&upper));
    }

    #[test]
    fn demo_scenery_does_not_intersect_track_or_facilities() {
        let simulation = WasmSimulation::new(1, 0);
        let track = super::track_support_obstacles(
            simulation.circuit.pieces(),
            &simulation.train_configuration,
        );
        for scenery in simulation.world.scenery() {
            let bounds = scenery.clearance_box();
            for obstacle in track.iter().chain(simulation.facility_obstacles().iter()) {
                assert!(
                    !super::boxes_overlap(
                        bounds,
                        super::ClearanceBox {
                            min: obstacle.min,
                            max: obstacle.max
                        }
                    ),
                    "demo scenery intersects a ride or facility: {scenery:?}"
                );
            }
        }
    }
}

#[cfg(test)]
fn theoretical_hourly_riders(layout: &BlockLayout) -> Result<f64, &'static str> {
    let mut estimator = ThroughputEstimator::new(layout);
    loop {
        if let Some(result) = estimator.advance(1_000) {
            return result;
        }
    }
}

fn block_layout_suggestions(layout: &BlockLayout) -> Vec<String> {
    vec![format!(
        "This layout supports up to {} train(s). Run automatic placement to check whether another set of brake positions can safely increase capacity.",
        layout.capacity()
    )]
}

#[cfg(test)]
mod world_tests {
    use super::{
        BlockLayout, Position, Surface, WasmSimulation, block_physics_config,
        editable_block_boundaries, theoretical_hourly_riders, track_support_obstacles,
    };
    use theme_park_sim::facilities::{FacilityKind, ParkFacilities};
    use theme_park_sim::guests::GuestId;
    use theme_park_sim::park_guests::ParkGuests;
    use theme_park_sim::physics::{Circuit, demo_circuit};
    use theme_park_sim::track::supports::generate_supports;
    use theme_park_sim::track::{PieceId, TrackState, catalog, hybrid_model};
    use theme_park_sim::world::{Direction, PathKind, World};

    fn block_test_circuit() -> Circuit {
        let pieces_catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in [
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ] {
            let piece = pieces_catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            state = piece.exit;
            pieces.push(piece);
        }
        Circuit::new(pieces).unwrap()
    }

    fn unpowered_block_test_circuit() -> Circuit {
        let mut pieces = block_test_circuit().pieces().to_vec();
        for piece in &mut pieces {
            if piece.kind == super::PieceKind::Lift {
                piece.kind = super::PieceKind::Track;
            }
        }
        Circuit::new(pieces).unwrap()
    }

    fn unpowered_block_physics_config() -> super::PhysicsConfig {
        super::PhysicsConfig {
            rolling_resistance: 0.001,
            ..block_physics_config()
        }
    }

    fn block_test_boundaries(circuit: &Circuit) -> Vec<f64> {
        [2, 6, 10, 13]
            .into_iter()
            .map(|end| {
                circuit.pieces()[..end]
                    .iter()
                    .map(|piece| piece.length())
                    .sum()
            })
            .collect()
    }

    fn block_powered_test_circuit() -> Circuit {
        let pieces_catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in [
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ] {
            let piece = pieces_catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            state = piece.exit;
            pieces.push(piece);
        }
        Circuit::new(pieces).unwrap()
    }

    fn report_number(report: &str, key: &str) -> f64 {
        let marker = format!("\"{key}\":");
        report
            .split_once(&marker)
            .unwrap()
            .1
            .split([',', '}'])
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn build_block_test_construction(simulation: &mut WasmSimulation) {
        simulation.construction_start(100, 100, 40, 0);
        for piece in [
            "Straight",
            "Straight",
            "LiftFlat",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
            "Straight",
            "Straight",
            "LiftFlat",
            "Straight",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(simulation.construction_message().is_empty());
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        assert!(simulation.construction_can_test());
    }

    fn apply_automatic_blocks_after_finding(
        simulation: &mut WasmSimulation,
        minimum_brakes: usize,
    ) {
        assert!(simulation.construction_auto_blocks());
        for _ in 0..10_000 {
            if let Some(search) = &simulation.block_placement_search {
                if editable_block_boundaries(search.best()).len() >= minimum_brakes {
                    assert!(simulation.construction_apply_best_block_sections());
                    return;
                }
                simulation.construction_advance_block_analysis(1_000);
            } else if editable_block_boundaries(simulation.block_layout.as_ref().unwrap()).len()
                >= minimum_brakes
            {
                return;
            } else {
                break;
            }
        }
        panic!("automatic search did not find {minimum_brakes} usable brake runs");
    }

    fn finish_manual_block_validation(simulation: &mut WasmSimulation) {
        for _ in 0..10_000 {
            if simulation.block_manual_search.is_none() {
                assert!(
                    simulation.block_message.is_empty(),
                    "{}",
                    simulation.block_message
                );
                return;
            }
            simulation.construction_advance_block_analysis(1_000);
        }
        panic!("manual block validation did not finish");
    }

    fn connect_station_to_park_paths(simulation: &mut WasmSimulation) {
        let construction = simulation.construction.as_ref().expect("coaster exists");
        let station = construction
            .pieces()
            .iter()
            .find(|piece| piece.kind == theme_park_sim::track::PieceKind::Station)
            .expect("station exists");
        let (queue, exit) = super::station_positions(station).expect("station access tiles");
        let network_y = 26;
        let queue_tail_x = 29;
        let exit_tail_x = 4;
        let queue_tail_y = network_y + 1;
        let at = |x, y| Position {
            x,
            y,
            height: queue.height,
        };
        assert_eq!(queue.height, exit.height);
        for x in 4..=queue_tail_x {
            simulation.world.set_surface(
                x,
                network_y,
                Surface {
                    corners: [queue.height; 4],
                    water: None,
                },
            );
        }
        for y in 4..=network_y {
            simulation.world.set_surface(
                4,
                y,
                Surface {
                    corners: [queue.height; 4],
                    water: None,
                },
            );
        }
        for x in queue.x.min(queue_tail_x)..=queue.x.max(queue_tail_x) {
            simulation.world.set_surface(
                x,
                queue.y,
                Surface {
                    corners: [queue.height; 4],
                    water: None,
                },
            );
        }
        for y in queue.y.min(queue_tail_y)..=queue.y.max(queue_tail_y) {
            simulation.world.set_surface(
                queue_tail_x,
                y,
                Surface {
                    corners: [queue.height; 4],
                    water: None,
                },
            );
        }
        for x in exit.x.min(exit_tail_x)..=exit.x.max(exit_tail_x) {
            simulation.world.set_surface(
                x,
                exit.y,
                Surface {
                    corners: [exit.height; 4],
                    water: None,
                },
            );
        }
        for (x, corners) in [
            (30, [15, 16, 16, 15]),
            (29, [14, 15, 15, 14]),
            (28, [13, 14, 14, 13]),
            (21, [14, 13, 13, 14]),
            (20, [15, 14, 14, 15]),
            (19, [16, 15, 15, 16]),
        ] {
            simulation.world.set_surface(
                x,
                exit.y,
                Surface {
                    corners,
                    water: None,
                },
            );
        }
        for x in 22..=27 {
            simulation.world.set_surface(
                x,
                exit.y,
                Surface {
                    corners: [13; 4],
                    water: None,
                },
            );
        }
        for y in exit.y.min(queue_tail_y)..=exit.y.max(queue_tail_y) {
            simulation.world.set_surface(
                exit_tail_x,
                y,
                Surface {
                    corners: [exit.height; 4],
                    water: None,
                },
            );
        }
        for y in 0..=3 {
            simulation.world.set_surface(
                4,
                y,
                Surface {
                    corners: [queue.height; 4],
                    water: None,
                },
            );
        }
        let entrance_position = Position {
            x: 4,
            y: 0,
            height: queue.height,
        };
        simulation
            .world
            .build_path_route(at(4, 4), at(4, network_y), PathKind::Normal, 0)
            .expect("build entrance path");
        simulation
            .world
            .build_path_route(
                at(4, network_y),
                at(queue_tail_x, network_y),
                PathKind::Normal,
                0,
            )
            .expect("build park path");
        let mut facilities = ParkFacilities::new();
        facilities
            .place(
                &simulation.world,
                FacilityKind::Entrance,
                entrance_position,
                Direction::South,
            )
            .expect("place park entrance");
        let entrance = facilities
            .entrance(&simulation.world)
            .expect("park entrance access");
        simulation
            .world
            .build_path_route(queue, at(queue_tail_x, queue.y), PathKind::Queue, 0)
            .expect("build coaster queue to park path");
        simulation
            .world
            .build_path_route(
                at(queue_tail_x, queue.y),
                at(queue_tail_x, queue_tail_y),
                PathKind::Queue,
                0,
            )
            .expect("extend coaster queue to park path");
        simulation
            .world
            .build_path_route(
                at(queue_tail_x, network_y),
                at(queue_tail_x, queue_tail_y),
                PathKind::Queue,
                0,
            )
            .expect("connect coaster queue to park path");
        simulation
            .world
            .build_path_route(exit, at(exit_tail_x, exit.y), PathKind::Normal, 0)
            .expect("build coaster exit path");
        simulation
            .world
            .build_path_route(
                at(exit_tail_x, exit.y),
                at(exit_tail_x, queue_tail_y),
                PathKind::Normal,
                0,
            )
            .expect("extend coaster exit to park path");
        simulation
            .world
            .build_path_route(
                at(exit_tail_x, queue_tail_y),
                at(exit_tail_x, network_y),
                PathKind::Normal,
                0,
            )
            .expect("connect coaster exit to park path");

        simulation.facilities = facilities;
        simulation.simulation = ParkGuests::new(42, 64, &simulation.world, entrance, 64);
        simulation
            .simulation
            .open_with_guests(&simulation.facilities.guest_facilities(&simulation.world));
        assert_eq!(
            simulation.simulation.walking().path_component(entrance),
            simulation.simulation.walking().path_component(queue),
            "coaster queue must connect to the park entrance"
        );
        assert!(
            simulation.construction_set_open(true),
            "{}",
            simulation.construction_message()
        );
    }

    #[test]
    fn automatic_block_search_reuses_completed_results_until_track_changes() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let config = super::PhysicsConfig {
            powered_sections: false,
            ..block_physics_config()
        };
        let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
        search.advance(2_000);
        assert!(search.is_complete());
        let work = search.work_completed();
        simulation.install_block_layout(search.best().clone(), false);
        simulation.block_placement_search = Some(search);
        simulation.construction_advance_block_analysis(1);
        assert_eq!(
            simulation
                .block_placement_cache
                .as_ref()
                .unwrap()
                .work_completed(),
            work
        );
        assert!(simulation.construction_auto_blocks());
        assert!(simulation.block_placement_search.is_none());
        assert_eq!(
            simulation
                .block_placement_cache
                .as_ref()
                .unwrap()
                .work_completed(),
            work
        );
        simulation.construction_undo();
        assert!(simulation.block_placement_cache.is_none());
    }

    #[test]
    fn repeated_automatic_block_requests_preserve_in_progress_search() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        assert!(simulation.construction_auto_blocks());
        simulation.construction_advance_block_analysis(10);
        let work = simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed();
        assert!(work > 0);
        assert!(simulation.construction_auto_blocks());
        assert_eq!(
            simulation
                .block_placement_search
                .as_ref()
                .unwrap()
                .work_completed(),
            work
        );
        assert!(simulation.block_placement_cache.is_none());
        assert!(simulation.construction_apply_best_block_sections());
        assert!(simulation.block_placement_search.is_none());
        assert!(simulation.block_placement_cache.is_none());
        assert!(simulation.construction_auto_blocks());
        assert_eq!(
            simulation
                .block_placement_search
                .as_ref()
                .unwrap()
                .work_completed(),
            0
        );
    }

    #[test]
    fn completed_automatic_search_installs_its_result() {
        let mut simulation = WasmSimulation::new(42, 0);
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let config = super::PhysicsConfig {
            powered_sections: false,
            ..block_physics_config()
        };
        let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
        search.advance(2_000);
        assert!(search.is_complete());
        simulation.install_block_layout(search.best().clone(), false);
        simulation.block_placement_search = Some(search);
        simulation.construction_advance_block_analysis(1);
        assert!(simulation.block_placement_search.is_none());
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 0);
        assert!(simulation.block_layout_is_automatic);
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("\"placementStatus\":\"complete\""),
            "{report}"
        );
    }

    #[test]
    fn block_candidates_are_sorted_exact_distances_accepted_by_manual_search() {
        let mut simulation = WasmSimulation::new(42, 0);
        assert!(simulation.construction_block_candidates().is_empty());
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let mut expected: Vec<_> =
            theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres)
                .iter()
                .map(|run| run.boundary_metres)
                .collect();
        expected.sort_by(f64::total_cmp);
        expected.dedup();
        assert!(!expected.is_empty());
        let candidates = simulation.construction_block_candidates();
        assert_eq!(candidates, expected);
        for candidate in candidates {
            assert!(simulation.construction_set_block_sections(vec![candidate]));
        }
    }

    #[test]
    fn manual_block_validation_preserves_layout_and_reports_rejection() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let original =
            BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
        simulation.install_block_layout(original, false);
        let old_boundaries = simulation
            .block_layout
            .as_ref()
            .unwrap()
            .boundaries()
            .to_vec();
        let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
        let pair = runs
            .iter()
            .enumerate()
            .find_map(|(index, first)| {
                runs[index + 1..].iter().find_map(|second| {
                    (first.boundary_metres != second.boundary_metres
                        && first
                            .piece_indices
                            .iter()
                            .any(|piece| second.piece_indices.contains(piece)))
                    .then_some((first.boundary_metres, second.boundary_metres))
                })
            })
            .expect("fixture has overlapping candidate brakes");
        assert!(simulation.construction_set_block_sections(vec![pair.0, pair.1]));
        assert!(
            simulation
                .construction_block_report_json()
                .contains("\"placementStatus\":\"validating\"")
        );
        assert_eq!(
            simulation.block_layout.as_ref().unwrap().boundaries(),
            old_boundaries
        );
        for _ in 0..1_000 {
            simulation.construction_advance_block_analysis(1_000);
            if simulation.block_manual_search.is_none() {
                break;
            }
        }
        assert!(simulation.block_manual_search.is_none());
        assert_eq!(
            simulation.block_layout.as_ref().unwrap().boundaries(),
            old_boundaries
        );
        assert!(
            simulation.construction_message().contains("overlap"),
            "{}",
            simulation.construction_message()
        );
    }

    #[test]
    fn manual_validation_and_throughput_both_advance() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let initial =
            BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
        simulation.install_block_layout(initial, false);
        let boundary = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres)[0]
            .boundary_metres;
        assert!(simulation.construction_set_block_sections(vec![boundary]));
        assert!(!simulation.construction_advance_block_analysis(200));
        let validation_steps = simulation
            .block_manual_search
            .as_ref()
            .unwrap()
            .work_completed();
        let throughput_steps = simulation.block_throughput_estimator.as_ref().unwrap().step;
        assert!(validation_steps > 0 && throughput_steps > 0);
        assert!(validation_steps + throughput_steps <= 200);
    }

    #[test]
    fn placement_and_throughput_share_bounded_work_without_starvation() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let initial =
            BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
        simulation.install_block_layout(initial, false);
        assert!(simulation.construction_auto_blocks());
        let progress = |simulation: &WasmSimulation| {
            (
                simulation
                    .block_placement_search
                    .as_ref()
                    .unwrap()
                    .work_completed(),
                simulation.block_throughput_estimator.as_ref().unwrap().step,
            )
        };
        assert_eq!(progress(&simulation), (0, 0));
        assert!(!simulation.construction_advance_block_analysis(0));
        assert_eq!(progress(&simulation), (0, 0));
        assert!(!simulation.construction_advance_block_analysis(200));
        let (placement, throughput) = progress(&simulation);
        assert!(placement > 0 && throughput > 0);
        assert!(placement + throughput <= 200);
        assert!(!simulation.construction_advance_block_analysis(1));
        assert!(!simulation.construction_advance_block_analysis(1));
        let (next_placement, next_throughput) = progress(&simulation);
        assert!(next_placement > placement && next_throughput > throughput);
        assert!(next_placement - placement + next_throughput - throughput <= 2);
        assert!(!simulation.construction_advance_block_analysis(5_000));
        let (capped_placement, capped_throughput) = progress(&simulation);
        assert!(capped_placement - next_placement + capped_throughput - next_throughput <= 1_000);
    }

    #[test]
    fn baseline_report_distinguishes_unverified_from_observed_stall() {
        let circuit = super::demo_circuit();
        let train = hybrid_model().train;
        let config = super::PhysicsConfig {
            powered_sections: false,
            ..Default::default()
        };
        let mut search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
        assert_eq!(search.best().capacity(), 1);
        assert!(!search.best().capacity_search_complete());
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(search.best().clone(), true);
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("one-train circuit has not yet been verified"),
            "{report}"
        );
        assert!(!report.contains("A train stalled"), "{report}");
        search.advance(2_000);
        assert!(search.is_complete());
        assert_eq!(search.best().capacity(), 0);
        assert!(search.best().capacity_search_complete());
        simulation.block_placement_search = Some(search);
        simulation.construction_advance_block_analysis(0);
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("A train stalled and could not complete this circuit"),
            "{report}"
        );
        assert!(
            report.contains("\"throughputStatus\":\"unavailable\""),
            "{report}"
        );
        assert!(simulation.block_throughput_estimator.is_none());
    }

    #[test]
    fn applying_an_unfinished_placement_does_not_claim_a_search_limit_was_reached() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        assert!(simulation.construction_auto_blocks());
        assert!(simulation.construction_apply_best_block_sections());
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("\"placementStatus\":\"incomplete\""),
            "{report}"
        );
        assert!(
            report.contains("Automatic placement is incomplete"),
            "{report}"
        );
        assert!(!report.contains("reached its search limit"), "{report}");
    }

    #[test]
    fn automatic_placement_is_incremental_and_preserves_installed_layout_while_searching() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        let (circuit, train) = simulation.construction_circuit_and_train().unwrap();
        let original =
            BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
        simulation.install_block_layout(original, false);
        let old_boundaries = simulation
            .block_layout
            .as_ref()
            .unwrap()
            .boundaries()
            .to_vec();
        assert!(simulation.construction_auto_blocks());
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("\"placementStatus\":\"searching\""),
            "{report}"
        );
        assert!(!simulation.construction_advance_block_analysis(1));
        assert_eq!(
            simulation.block_layout.as_ref().unwrap().boundaries(),
            old_boundaries
        );
        let before_tick = simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed();
        simulation.tick();
        let after_tick = simulation
            .block_placement_search
            .as_ref()
            .unwrap()
            .work_completed();
        assert_eq!(after_tick, before_tick);
        simulation.construction_advance_block_analysis(200);
        assert!(
            simulation
                .block_placement_search
                .as_ref()
                .unwrap()
                .work_completed()
                > after_tick
        );
        simulation.construction_delete(3);
        assert!(simulation.block_placement_search.is_none());
    }

    #[test]
    fn block_report_marks_throughput_pending_until_incremental_estimate_completes() {
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let layout =
            BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new()).unwrap();
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(layout, false);
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("\"theoreticalHourlyRiders\":null"),
            "{report}"
        );
        assert!(
            report.contains("\"throughputStatus\":\"pending\""),
            "{report}"
        );
        assert!(!simulation.construction_advance_block_analysis(1));
        for _ in 0..1_000 {
            if simulation.construction_advance_block_analysis(1_000) {
                break;
            }
        }
        let report = simulation.construction_block_report_json();
        assert!(
            report.contains("\"throughputStatus\":\"ready\""),
            "{report}"
        );
        assert!(
            report_number(&report, "theoreticalHourlyRiders") > 0.0,
            "{report}"
        );
    }

    #[test]
    fn block_suggestion_does_not_point_across_a_loop_without_a_candidate() {
        let circuit = demo_circuit();
        let mut train = hybrid_model().train;
        train.length_metres = 5.0;
        let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
        let loop_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == PieceId::Loop)
            .expect("demo circuit contains a loop");
        let loop_start: f64 = circuit.pieces()[..loop_index]
            .iter()
            .map(|piece| piece.length())
            .sum();
        let loop_end = loop_start + circuit.pieces()[loop_index].length();
        let candidate_boundaries: Vec<_> = runs.iter().map(|run| run.boundary_metres).fold(
            Vec::new(),
            |mut boundaries, boundary| {
                if boundaries.last() != Some(&boundary) {
                    boundaries.push(boundary);
                }
                boundaries
            },
        );
        let before_loop = candidate_boundaries
            .iter()
            .copied()
            .filter(|boundary| *boundary <= loop_start)
            .max_by(f64::total_cmp)
            .expect("brake candidate before loop");
        let after_loop = candidate_boundaries
            .iter()
            .copied()
            .filter(|boundary| *boundary >= loop_end)
            .min_by(f64::total_cmp)
            .expect("brake candidate after loop");
        assert!(runs.iter().all(|run| {
            run.boundary_metres <= before_loop || run.boundary_metres >= after_loop
        }));
        assert!(loop_start >= before_loop && loop_end <= after_loop);

        let validated_circuit = block_test_circuit();
        let validated_train = hybrid_model().train;
        let boundaries = block_test_boundaries(&validated_circuit);
        let layout = BlockLayout::new(
            &validated_circuit,
            &validated_train,
            block_physics_config(),
            boundaries[..3].to_vec(),
        )
        .expect("known three-brake layout validates");
        assert_eq!(editable_block_boundaries(&layout).len(), 3);

        let suggestions = super::block_layout_suggestions(&layout).join(" ");
        assert!(suggestions.contains("automatic placement"), "{suggestions}");
        assert!(!suggestions.contains("largest gap"), "{suggestions}");
        assert!(!suggestions.contains("brake there"), "{suggestions}");
    }

    #[test]
    fn throughput_cache_distinguishes_different_brake_runs_at_the_same_boundary() {
        let mut pieces = super::demo_circuit().pieces().to_vec();
        pieces
            .iter_mut()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .kind = super::PieceKind::Track;
        let circuit = Circuit::new(pieces).unwrap();
        let train = hybrid_model().train;
        let runs = theme_park_sim::track::blocks::brake_runs(&circuit, train.length_metres);
        let normal = block_physics_config();
        let gentle = super::PhysicsConfig {
            braking_acceleration: 0.5,
            ..normal
        };
        for run in &runs {
            if !runs.iter().any(|other| {
                other.boundary_metres == run.boundary_metres
                    && other.piece_indices.len() > run.piece_indices.len()
            }) {
                continue;
            }
            let Ok(first) = BlockLayout::new(&circuit, &train, normal, vec![run.boundary_metres])
            else {
                continue;
            };
            let Ok(second) = BlockLayout::new(&circuit, &train, gentle, vec![run.boundary_metres])
            else {
                continue;
            };
            if first.boundaries() == second.boundaries()
                && first
                    .boundaries()
                    .iter()
                    .enumerate()
                    .any(|(index, _)| first.brake_pieces(index) != second.brake_pieces(index))
            {
                assert_ne!(
                    super::block_layout_fingerprint(&first),
                    super::block_layout_fingerprint(&second)
                );
                return;
            }
        }
        panic!("expected two validated brake lengths at the same boundary");
    }

    #[test]
    fn block_report_distinguishes_native_powered_holds_from_pass_through_boundaries() {
        let circuit = block_powered_test_circuit();
        let train = hybrid_model().train;
        let layout = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new())
            .expect("powered lift layout validates");
        let powered_index = layout
            .boundaries()
            .iter()
            .enumerate()
            .find_map(|(index, _)| (!layout.powered_hold_pieces(index).is_empty()).then_some(index))
            .expect("layout has a native powered hold");
        assert!(layout.brake_pieces(powered_index).is_empty());

        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(layout.clone(), false);
        let report = simulation.construction_block_report_json();
        let held_piece = layout.powered_hold_pieces(powered_index)[0];
        assert!(report.contains("\"brakePieceIndices\":[]"), "{report}");
        assert!(
            report.contains(&format!("\"poweredHoldPieceIndices\":[{held_piece}]")),
            "{report}"
        );
        assert!(
            report.contains("Fixed powered hold at the start of lift or launch track."),
            "{report}"
        );
        assert!(report.contains("Fixed pass-through boundary"), "{report}");
    }

    #[test]
    fn installing_a_native_hold_layout_does_not_adopt_a_legacy_throughput_cache_entry() {
        let circuit = block_powered_test_circuit();
        let train = hybrid_model().train;
        let layout = BlockLayout::new(&circuit, &train, block_physics_config(), Vec::new())
            .expect("powered lift layout validates");
        assert!(
            layout
                .boundaries()
                .iter()
                .enumerate()
                .any(|(index, _)| { !layout.powered_hold_pieces(index).is_empty() })
        );

        let mut legacy_fingerprint = vec![layout.capacity() as u64];
        for (index, boundary) in layout.boundaries().iter().enumerate() {
            let brake_pieces = layout.brake_pieces(index);
            legacy_fingerprint.push(boundary.to_bits());
            legacy_fingerprint.push(brake_pieces.len() as u64);
            legacy_fingerprint.extend(brake_pieces.iter().map(|piece| *piece as u64));
        }
        let mut simulation = WasmSimulation::new(42, 0);
        simulation
            .block_throughput_cache
            .insert(legacy_fingerprint, (1_234.0, String::new()));

        simulation.install_block_layout(layout, false);

        assert!(
            simulation.block_throughput_estimator.is_some(),
            "native hold identity must prevent adopting the old cache entry"
        );
    }

    #[test]
    fn block_boundary_fingerprint_includes_native_hold_piece_identity() {
        assert_ne!(
            super::block_boundary_fingerprint(80.0, &[], &[3]),
            super::block_boundary_fingerprint(80.0, &[], &[4])
        );
    }

    #[test]
    fn throughput_simulation_matches_recording_train_motion() {
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let layout = BlockLayout::new(
            &circuit,
            &train,
            block_physics_config(),
            block_test_boundaries(&circuit),
        )
        .unwrap();
        assert!(layout.capacity() >= 3);
        let mut recorded = super::BlockOperation::new(layout.clone());
        let mut unrecorded = super::BlockOperation::new_without_reports(layout);
        let mut completed = 0;
        for _ in 0..30_000 {
            assert_eq!(recorded.can_dispatch(), unrecorded.can_dispatch());
            if recorded.can_dispatch() {
                assert_eq!(recorded.dispatch(), unrecorded.dispatch());
            }
            let recorded_outcomes = recorded.step(&[]);
            let unrecorded_outcomes = unrecorded.step(&[]);
            assert_eq!(recorded_outcomes.len(), unrecorded_outcomes.len());
            for ((id, outcome), (other_id, other_outcome)) in
                recorded_outcomes.iter().zip(&unrecorded_outcomes)
            {
                assert_eq!(id, other_id);
                assert_eq!(outcome.recovered, other_outcome.recovered);
                completed += 1;
            }
            let recorded_positions: Vec<_> = recorded
                .trains()
                .map(|(id, train)| (id, train.distance(), train.speed()))
                .collect();
            let unrecorded_positions: Vec<_> = unrecorded
                .trains()
                .map(|(id, train)| (id, train.distance(), train.speed()))
                .collect();
            assert_eq!(recorded_positions, unrecorded_positions);
        }
        assert!(completed > 0);
    }

    #[test]
    fn automatic_search_publishes_baseline_verification_without_recovering_train() {
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let config = block_physics_config();
        let search = super::BlockPlacementSearch::new(circuit, train, config).unwrap();
        let baseline = search.best().clone();
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(baseline.clone(), true);
        simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
        simulation.coaster_open = true;
        let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
        simulation.block_placement_search = Some(search);
        for _ in 0..1_000 {
            simulation.construction_advance_block_analysis(1_000);
            if simulation
                .block_layout
                .as_ref()
                .unwrap()
                .capacity_search_complete()
            {
                break;
            }
        }
        assert!(
            simulation
                .block_layout
                .as_ref()
                .unwrap()
                .capacity_search_complete()
        );
        assert!(simulation.block_placement_search.is_some());
        let ids: Vec<_> = simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, vec![train_id]);
    }

    #[test]
    fn stalled_baseline_waits_for_active_train_before_reducing_capacity() {
        let circuit = super::demo_circuit();
        let train = hybrid_model().train;
        let config = super::PhysicsConfig {
            powered_sections: false,
            ..Default::default()
        };
        let baseline = super::block_layout_for_open(&circuit, &train, config).unwrap();
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(baseline.clone(), false);
        simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
        simulation.coaster_open = true;
        let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
        simulation.block_manual_search =
            Some(super::BlockManualSearch::new(circuit, train, config, Vec::new()).unwrap());
        simulation.block_baseline_validation = true;
        for _ in 0..10 {
            simulation.construction_advance_block_analysis(1_000);
            if simulation.block_layout_change_pending() {
                break;
            }
        }
        assert!(simulation.block_layout_change_pending());
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 1);
        assert!(
            simulation
                .construction_block_report_json()
                .contains("New dispatches are paused")
        );
        assert_eq!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .next()
                .unwrap()
                .0,
            train_id
        );
        for _ in 0..1_000 {
            if !simulation
                .coaster_operation
                .as_mut()
                .unwrap()
                .step(&[])
                .is_empty()
            {
                break;
            }
        }
        assert_eq!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .count(),
            0
        );
        simulation.construction_advance_block_analysis(1);
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 0);
        assert!(
            simulation
                .construction_block_report_json()
                .contains("A train stalled")
        );
    }

    #[test]
    fn baseline_validation_waits_to_adopt_powered_holds_until_the_train_completes() {
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let config = block_physics_config();
        let baseline = super::block_layout_for_open(&circuit, &train, config).unwrap();
        assert!(!baseline.capacity_search_complete());
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.install_block_layout(baseline.clone(), false);
        simulation.coaster_operation = Some(super::BlockOperation::new(baseline));
        simulation.coaster_open = true;
        let train_id = simulation.coaster_operation.as_mut().unwrap().dispatch();
        simulation.block_manual_search =
            Some(super::BlockManualSearch::new(circuit, train, config, Vec::new()).unwrap());
        for _ in 0..1_000 {
            simulation.construction_advance_block_analysis(1_000);
            if simulation
                .block_manual_search
                .as_ref()
                .and_then(super::BlockManualSearch::result)
                .is_some()
            {
                break;
            }
        }
        assert!(simulation.block_layout_change_pending());
        assert!(
            !simulation
                .block_layout
                .as_ref()
                .unwrap()
                .capacity_search_complete()
        );
        let validated_layout = simulation
            .block_manual_search
            .as_ref()
            .and_then(super::BlockManualSearch::result)
            .and_then(|result| result.as_ref().ok())
            .expect("baseline validation found native powered holds");
        assert!(validated_layout.capacity_search_complete());
        assert_eq!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![train_id]
        );
        let outcome = (0..120_000)
            .find_map(|_| {
                simulation
                    .coaster_operation
                    .as_mut()
                    .unwrap()
                    .step(&[])
                    .into_iter()
                    .next()
            })
            .expect("running baseline train completes before adoption")
            .1;
        assert!(!outcome.recovered);
        simulation.construction_advance_block_analysis(0);
        assert!(simulation.block_manual_search.is_none());
        assert!(
            simulation
                .block_layout
                .as_ref()
                .unwrap()
                .capacity_search_complete()
        );
        let trains: Vec<_> = simulation
            .coaster_operation
            .as_ref()
            .unwrap()
            .trains()
            .map(|(id, _)| id)
            .collect();
        assert!(trains.is_empty());
    }

    #[test]
    fn opening_starts_with_one_train_without_block_placement() {
        let circuit = block_test_circuit();
        let train = hybrid_model().train;
        let layout =
            super::block_layout_for_open(&circuit, &train, block_physics_config()).unwrap();
        assert_eq!(layout.capacity(), 1);
        assert!(editable_block_boundaries(&layout).is_empty());
        assert!(
            !layout.capacity_search_complete(),
            "opening should not synchronously simulate a lap"
        );
    }

    #[test]
    fn throughput_uses_sustained_dispatches_after_the_initial_fill() {
        let circuit = unpowered_block_test_circuit();
        let train = hybrid_model().train;
        let physics = unpowered_block_physics_config();
        let layout =
            BlockLayout::new(&circuit, &train, physics, block_test_boundaries(&circuit)).unwrap();
        assert_eq!(layout.capacity(), 3);

        let mut operation = super::BlockOperation::new(layout.clone());
        while operation.can_dispatch() {
            operation.dispatch();
        }
        let mut dispatch_steps = Vec::new();
        for step in 1..=200_000 {
            assert!(
                operation
                    .step(&[])
                    .iter()
                    .all(|(_, outcome)| !outcome.recovered)
            );
            while operation.can_dispatch() {
                operation.dispatch();
                dispatch_steps.push(step);
            }
            if dispatch_steps.len() >= 4 {
                break;
            }
        }
        assert!(
            dispatch_steps.len() >= 4,
            "not enough dispatches for a sustained estimate"
        );
        let seconds = (dispatch_steps[3] - dispatch_steps[0]) as f64 * super::TIME_STEP;
        let seats = usize::from(train.cars)
            * usize::from(train.rows_per_car)
            * usize::from(train.seats_per_row);
        let expected = seats as f64 * 3.0 * 3600.0 / seconds;
        let actual = theoretical_hourly_riders(&layout).unwrap();
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn block_report_uses_simulated_dispatch_spacing_for_one_two_and_three_trains() {
        let circuit = unpowered_block_test_circuit();
        let train = hybrid_model().train;
        let physics = unpowered_block_physics_config();
        let brake_boundaries = block_test_boundaries(&circuit);

        let mut hourly_throughputs = Vec::new();
        for train_capacity in 1..=3 {
            let boundaries = brake_boundaries[..train_capacity + 1].to_vec();
            let layout = BlockLayout::new(&circuit, &train, physics, boundaries)
                .expect("the selected block layout is physically valid");
            assert_eq!(layout.capacity(), train_capacity);

            let mut simulation = WasmSimulation::new(42, 0);
            simulation.block_layout = Some(layout.clone());
            match theoretical_hourly_riders(&layout) {
                Ok(throughput) => simulation.block_hourly_throughput = throughput,
                Err(reason) => simulation.block_throughput_reason = reason.to_owned(),
            }
            let report = simulation.construction_block_report_json();
            assert_eq!(
                report_number(&report, "trainCapacity"),
                train_capacity as f64
            );
            assert!(
                report_number(&report, "theoreticalHourlyRiders") > 0.0,
                "capacity {train_capacity}: {report}"
            );
            assert!(report.contains("\"editable\":true"));
            assert!(report.contains("\"reasons\":[]"));
            assert!(report.contains("\"placementSearchComplete\":null"));
            assert!(report.contains("\"notes\":["));
            assert!(report.contains("\"suggestions\":[\""));
            hourly_throughputs.push(report_number(&report, "theoreticalHourlyRiders"));
        }
        assert!(
            hourly_throughputs
                .iter()
                .all(|throughput| throughput.is_finite())
        );
        assert!(
            hourly_throughputs[1] > hourly_throughputs[0],
            "two-train throughput {}/h did not exceed one-train throughput {}/h",
            hourly_throughputs[1],
            hourly_throughputs[0]
        );
        assert!(
            hourly_throughputs[2] > hourly_throughputs[0],
            "three-train throughput {}/h did not exceed one-train throughput {}/h",
            hourly_throughputs[2],
            hourly_throughputs[0]
        );
    }

    #[test]
    fn train_count_rejects_values_outside_capacity_and_limits_dispatch() {
        let mut simulation = simulation_with_two_occupied_coaster_trains();
        assert_eq!(simulation.construction_train_count(), 2);
        assert!(!simulation.construction_set_train_count(0));
        assert!(!simulation.construction_set_train_count(3));
        assert_eq!(simulation.construction_train_count(), 2);
        assert!(simulation.construction_set_train_count(1));
        for _ in 0..5000 {
            simulation.tick();
            if simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .count()
                <= 1
            {
                break;
            }
        }
        assert!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .count()
                <= 1
        );
        for _ in 0..1000 {
            simulation.tick();
            assert!(
                simulation
                    .coaster_operation
                    .as_ref()
                    .unwrap()
                    .trains()
                    .count()
                    <= 1
            );
        }
    }

    fn simulation_with_two_occupied_coaster_trains() -> WasmSimulation {
        let mut simulation = WasmSimulation::new(42, 64);
        simulation.world = World::new(
            64,
            64,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = ParkFacilities::new();
        simulation.construction_start(60, 60, 8, 0);
        for piece in [
            "Straight",
            "Straight",
            "LiftFlat",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
            "Straight",
            "Straight",
            "LiftFlat",
            "Straight",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(
                simulation.construction_message().is_empty(),
                "{piece}: {}",
                simulation.construction_message()
            );
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        apply_automatic_blocks_after_finding(&mut simulation, 3);
        assert!(editable_block_boundaries(simulation.block_layout.as_ref().unwrap()).len() >= 3);
        assert!(simulation.construction_set_block_sections(Vec::new()));
        finish_manual_block_validation(&mut simulation);
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
        assert_eq!(simulation.construction_train_count(), 1);
        assert!(simulation.construction_set_train_count(2));
        connect_station_to_park_paths(&mut simulation);
        simulation.construction_placed_supports();
        assert!(
            simulation.construction_message.is_empty(),
            "support routes: {:?}",
            simulation
                .support_render_cache
                .iter()
                .filter_map(
                    |(id, cache)| (cache.world_revision == simulation.world.revision())
                        .then(|| cache.segments.as_ref().err().map(|err| (id, err)))
                        .flatten()
                )
                .collect::<Vec<_>>()
        );
        let construction = simulation.construction.as_ref().unwrap();
        for piece in construction.pieces() {
            assert_eq!(
                super::check_clearance(
                    &simulation.world,
                    piece,
                    &construction.model().train,
                    &[],
                    &[]
                ),
                Ok(()),
                "{:?}",
                piece.id
            );
        }

        simulation.tick();
        assert_eq!(
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .count(),
            0,
            "empty trains should not leave the station"
        );

        let mut occupied_train_ids = Vec::new();
        for _ in 0..10_000 {
            simulation.tick();
            occupied_train_ids = simulation
                .coaster_riders
                .iter()
                .filter_map(|(&train_id, guests)| (!guests.is_empty()).then_some(train_id))
                .collect();
            if occupied_train_ids.len() >= 2 {
                break;
            }
        }
        assert!(
            occupied_train_ids.len() >= 2,
            "expected passengers on two simultaneous trains, got {occupied_train_ids:?}; active={}, aware={}, revenue={}, trains={:?}",
            simulation
                .simulation
                .walking()
                .traits()
                .ids()
                .filter(|&id| simulation.simulation.walking().traits().is_active(id))
                .count(),
            simulation
                .simulation
                .behaviors()
                .guests
                .iter()
                .filter(|guest| guest
                    .known_places()
                    .iter()
                    .any(|place| place.id == super::COASTER_PLACE_ID))
                .count(),
            simulation.simulation.revenue(),
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .map(|(id, train)| (id, train.distance(), train.speed()))
                .collect::<Vec<_>>()
        );
        simulation
    }

    #[test]
    fn deleting_coaster_track_recovers_passengers_from_two_live_trains() {
        let mut simulation = simulation_with_two_occupied_coaster_trains();
        let occupied_train_ids: Vec<_> = simulation.coaster_riders.keys().copied().collect();
        let riders_before_deletion: Vec<GuestId> = occupied_train_ids
            .iter()
            .flat_map(|train_id| simulation.coaster_riders[train_id].iter().copied())
            .collect();
        assert!(!riders_before_deletion.is_empty());

        simulation.construction_delete(1);

        assert!(!simulation.coaster_open);
        assert!(simulation.coaster_riders.is_empty());
        assert!(simulation.construction_riders().is_empty());
        for rider in riders_before_deletion {
            assert!(simulation.simulation.walking().traits().is_active(rider));
            assert!(!simulation.simulation.behaviors().is_coaster_riding(rider));
        }
    }

    #[test]
    fn automatic_improvement_waits_for_passengers_before_installing() {
        let mut simulation = simulation_with_two_occupied_coaster_trains();
        let riders: Vec<_> = simulation
            .coaster_riders
            .values()
            .flatten()
            .copied()
            .collect();
        let rider_train_ids: Vec<_> = simulation.coaster_riders.keys().copied().collect();
        let completed_before: Vec<_> = riders
            .iter()
            .map(|rider| simulation.simulation.behaviors().guests[rider.0 as usize].rides_completed)
            .collect();
        let circuit = simulation.block_layout.as_ref().unwrap().circuit();
        let station_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.kind == super::PieceKind::Station)
            .unwrap();
        let station_distance: f64 = circuit.pieces()[..station_index]
            .iter()
            .map(|piece| piece.length())
            .sum::<f64>()
            + circuit.pieces()[station_index].length() / 2.0;
        let circuit_length = circuit.length();
        let mut train_history = vec![(
            0,
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .map(|(id, train)| {
                    (
                        id,
                        train.distance(),
                        train.speed(),
                        simulation.coaster_riders.get(&id).map_or(0, Vec::len),
                    )
                })
                .collect::<Vec<_>>(),
        )];
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
        assert!(simulation.construction_auto_blocks());
        for _ in 0..10_000 {
            simulation.construction_advance_block_analysis(1_000);
            if simulation
                .block_placement_search
                .as_ref()
                .is_some_and(|search| search.best().capacity() > 2)
            {
                break;
            }
        }
        assert!(
            simulation
                .block_placement_search
                .as_ref()
                .is_some_and(|search| search.best().capacity() > 2)
        );
        assert_eq!(simulation.block_layout.as_ref().unwrap().capacity(), 2);
        assert!(
            riders
                .iter()
                .all(|&rider| simulation.simulation.behaviors().is_coaster_riding(rider))
        );
        let mut saw_empty_dispatch_pause = false;
        for tick in 1..=10_000 {
            simulation.construction_advance_block_analysis(0);
            simulation.tick();
            let active_train_ids: Vec<_> = simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .map(|(id, _)| id)
                .collect();
            assert!(
                simulation.block_layout.as_ref().unwrap().capacity() > 2
                    || active_train_ids
                        .iter()
                        .all(|train_id| rider_train_ids.contains(train_id)),
                "a replacement train dispatched before the pending block layout was installed: active={active_train_ids:?}, original={rider_train_ids:?}"
            );
            if active_train_ids.is_empty()
                && simulation.block_layout.as_ref().unwrap().capacity() == 2
                && simulation
                    .block_placement_search
                    .as_ref()
                    .is_some_and(|search| search.best().capacity() > 2)
            {
                assert!(simulation.block_layout_change_pending());
                saw_empty_dispatch_pause = true;
            }
            if tick % 1_000 == 0 {
                train_history.push((
                    tick,
                    simulation
                        .coaster_operation
                        .as_ref()
                        .unwrap()
                        .trains()
                        .map(|(id, train)| {
                            (
                                id,
                                train.distance(),
                                train.speed(),
                                simulation.coaster_riders.get(&id).map_or(0, Vec::len),
                            )
                        })
                        .collect::<Vec<_>>(),
                ));
            }
            if simulation.block_layout.as_ref().unwrap().capacity() > 2 {
                break;
            }
        }
        assert!(
            simulation.block_layout.as_ref().unwrap().capacity() > 2,
            "old capacity={}, open={}, waiting={}, circuit length={}, station distance={}, history={:?}, active={:?}, riders={:?}, search={:?}",
            simulation.block_layout.as_ref().unwrap().capacity(),
            simulation.coaster_open,
            simulation.block_layout_change_pending(),
            circuit_length,
            station_distance,
            train_history,
            simulation
                .coaster_operation
                .as_ref()
                .unwrap()
                .trains()
                .map(|(id, train)| (id, train.distance(), train.speed()))
                .collect::<Vec<_>>(),
            simulation
                .coaster_riders
                .iter()
                .map(|(id, guests)| (*id, guests.len()))
                .collect::<Vec<_>>(),
            simulation.block_placement_search.as_ref().map(|search| (
                search.best().capacity(),
                search.work_completed(),
                search.is_complete()
            ))
        );
        assert!(
            saw_empty_dispatch_pause,
            "fixture did not exercise the empty substep window before layout installation"
        );
        assert!(
            simulation.block_placement_search.is_some(),
            "search should continue toward the maximum"
        );
        for (rider, before) in riders.into_iter().zip(completed_before) {
            assert!(
                simulation.simulation.behaviors().guests[rider.0 as usize].rides_completed > before,
                "rider must complete, not be recovered"
            );
        }
    }

    #[test]
    fn block_commands_keep_valid_layout_after_rejected_edit() {
        let mut simulation = WasmSimulation::new(42, 0);
        build_block_test_construction(&mut simulation);
        apply_automatic_blocks_after_finding(&mut simulation, 1);
        let layout = simulation.block_layout.as_ref().unwrap();
        let original_boundaries = editable_block_boundaries(layout);
        assert!(!original_boundaries.is_empty());

        assert!(!simulation.construction_move_block_section(0, f64::NAN));
        assert!(
            simulation
                .construction_message()
                .contains("outside the circuit")
        );
        assert_eq!(
            editable_block_boundaries(simulation.block_layout.as_ref().unwrap()),
            original_boundaries
        );

        let first_boundary = original_boundaries[0];
        assert!(simulation.construction_move_block_section(0, first_boundary));
        finish_manual_block_validation(&mut simulation);
        assert!(simulation.construction_remove_block_section(0));
        finish_manual_block_validation(&mut simulation);
        assert!(simulation.construction_add_block_section(first_boundary));
        finish_manual_block_validation(&mut simulation);
        assert_eq!(
            editable_block_boundaries(simulation.block_layout.as_ref().unwrap()),
            original_boundaries
        );
        let report = simulation.construction_block_report_json();
        assert!(report.contains("\"reasons\":[]"));
        assert!(report.contains("\"editable\":false"));
        assert!(report.contains("\"trackKind\":\"station\""));

        simulation.construction_delete(3);
        assert!(simulation.block_layout.is_none());
        assert!(
            simulation
                .construction_block_report_json()
                .contains("\"trainCapacity\":0")
        );
    }

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
        let rerouted = generate_supports(
            &upper,
            &world,
            &track_support_obstacles(&[lower], &hybrid_model().train),
        )
        .unwrap();
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

#[cfg(test)]
mod facility_bridge_tests {
    use super::WasmSimulation;

    #[test]
    fn ride_snapshots_expose_idle_cycle_state() {
        let simulation = WasmSimulation::new(42, 0);
        let snapshot = simulation.facilities_json();
        assert!(snapshot.contains("\"ride_state\":\"idle\",\"ride_progress\":0"));
    }

    #[test]
    fn track_construction_rejects_existing_building_footprints() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(440, 508, 5, 0);
        assert!(!simulation.construction_active());
        assert!(!simulation.construction_message().is_empty());
    }

    #[test]
    fn placement_reports_errors_and_exports_and_removes_buildings() {
        let mut simulation = WasmSimulation::new(42, 0);
        assert!(
            simulation
                .facilities_json()
                .contains("\"kind\":\"entrance\"")
        );
        assert!(!simulation.place_facility("unknown", 240, 254, 2));
        assert!(!simulation.facility_message().is_empty());
        assert!(simulation.place_facility("carousel", 240, 254, 2));
        assert!(simulation.facility_message().is_empty());
        let snapshot = simulation.facilities_json();
        assert!(snapshot.contains("\"kind\":\"carousel\""));
        assert!(snapshot.contains("\"x\":240,\"y\":254,\"z\":8,\"rotation\":2"));
        assert!(!simulation.place_facility("restroom", 240, 254, 2));
        assert!(!simulation.facility_message().is_empty());
        let id = simulation
            .facilities
            .buildings()
            .iter()
            .find(|building| building.position.x == 240)
            .unwrap()
            .id;
        assert!(simulation.remove_facility(id));
        assert!(
            !simulation
                .facilities
                .buildings()
                .iter()
                .any(|building| building.id == id)
        );
    }
}

#[cfg(test)]
mod construction_bridge_tests {
    #[test]
    fn diagonal_support_bounds_do_not_fill_its_bounding_rectangle() {
        let simulation = super::WasmSimulation::new(42, 0);
        let piece = simulation.circuit.pieces()[0].clone();
        let point = piece.sample(piece.length() / 2.0).position;
        let segment = super::SupportSegment {
            start: point + super::Vec3::new(-40.0, 0.0, 0.0),
            end: point + super::Vec3::new(0.0, 40.0, 0.0),
        };
        let baseline = super::scan_proximity_with_supports(
            &simulation.world,
            std::slice::from_ref(&piece),
            &simulation.train_configuration,
            &[],
            &[],
            &[],
        );
        let diagonal = super::scan_proximity_with_supports(
            &simulation.world,
            &[piece],
            &simulation.train_configuration,
            &[],
            &[],
            &[segment],
        );
        assert_eq!(baseline.events_for_run(&[]), diagonal.events_for_run(&[]));
    }
    #[test]
    fn proximity_inputs_keep_routed_supports_as_segments() {
        let mut simulation = super::WasmSimulation::new(42, 0);
        let segment = super::SupportSegment {
            start: super::Vec3::new(0.0, 0.0, 0.0),
            end: super::Vec3::new(10.0, 10.0, 0.0),
        };
        simulation.support_render_cache.insert(
            1,
            super::SupportRenderCache {
                obstacles: Vec::new(),
                dependencies: Vec::new(),
                world_revision: 0,
                segments: Ok(vec![segment]),
            },
        );
        assert_eq!(simulation.proximity_supports(), vec![segment]);
    }
    use super::WasmSimulation;

    #[test]
    fn coaster_cannot_open_without_a_complete_circuit() {
        let mut simulation = WasmSimulation::new(42, 0);
        assert!(!simulation.construction_set_open(true));
        simulation.construction_start(200, 200, 20, 0);
        assert!(!simulation.construction_set_open(true));
        assert!(!simulation.construction_is_open());
        assert!(simulation.construction_riders().is_empty());
    }

    #[test]
    fn coaster_requires_station_paths_before_opening() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        for piece in [
            "TurnSmallStandalone",
            "TurnSmallStandalone",
            "Straight",
            "TurnSmallStandalone",
            "TurnSmallStandalone",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        assert!(!simulation.construction_set_open(true));
        assert!(!simulation.construction_is_open());
    }

    #[test]
    fn model_selection_controls_new_construction_and_launch_choices() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_select("model:velocity_viper");
        simulation.construction_start(200, 200, 20, 0);
        assert_eq!(
            simulation.construction.as_ref().unwrap().model().name,
            "Velocity Viper"
        );
        assert!(simulation.construction_choices().contains("LaunchShort\t"));
        simulation.construction_select("LaunchShort");
        assert!(simulation.construction_can_append());
        let construction = simulation.construction.as_ref().unwrap();
        let obstacles =
            super::track_support_obstacles(construction.pieces(), &construction.model().train);
        let preview = simulation.construction_preview();
        let expected = super::render_preview_supports(
            &preview[0],
            &obstacles,
            &simulation.world,
            super::SupportStyle::SteelColumns,
        )
        .unwrap();
        assert_eq!(
            simulation.construction_supports(),
            simulation.encode_construction_geometry(expected, 7)
        );
        simulation.construction_append();
        assert!(simulation.construction_track().contains("Launch"));
        simulation.construction_select("model:hybrid");
        simulation.construction_start(200, 200, 20, 0);
        assert_eq!(
            simulation.construction.as_ref().unwrap().model().name,
            super::hybrid_model().name
        );
    }

    #[test]
    fn rail_distances_follow_placed_rail_intervals_across_pieces() {
        let mut simulation = WasmSimulation::new(42, 0);
        assert!(simulation.construction_rail_distances().is_empty());
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("Straight");
        simulation.construction_append();
        let construction = simulation.construction.as_ref().unwrap();
        let rails = simulation.construction_placed_rails();
        let distances = simulation.construction_rail_distances();
        assert_eq!(distances.len(), rails.len() / 19);
        let mut expected = Vec::new();
        let mut start = 0.0;
        for piece in construction.pieces() {
            let intervals = (piece.length() / 2.0).ceil() as usize;
            for index in 0..intervals {
                expected.push((start + piece.length() * index as f64 / intervals as f64) as f32);
                expected
                    .push((start + piece.length() * (index + 1) as f64 / intervals as f64) as f32);
            }
            start += piece.length();
        }
        assert_eq!(distances, expected);
    }

    #[test]
    fn wooden_twister_geometry_distinguishes_track_and_steel_loop_supports() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_select("model:wooden_twister");
        simulation.construction_start(200, 200, 20, 0);
        let rails = simulation.construction_placed_rails();
        assert!(!rails.is_empty());
        assert!(
            rails
                .chunks_exact(19)
                .all(|record| record[0] == 1_000_000.0)
        );
        let ties = simulation.construction_placed_ties();
        assert!(!ties.is_empty());
        assert!(ties.chunks_exact(13).all(|record| record[0] == 1_000_000.0));
        let supports = simulation.construction_placed_supports();
        assert!(!supports.is_empty());
        assert!(supports.chunks_exact(7).all(|record| record[0] == 0.0));

        let loop_piece = super::catalog()
            .into_iter()
            .find(|piece| piece.id == super::PieceId::SteelLoop)
            .unwrap()
            .at(super::TrackState {
                x: 200,
                y: 200,
                z: 20,
                ..super::TrackState::origin()
            })
            .unwrap();
        let supports = super::render_preview_supports(
            &loop_piece,
            &[],
            &simulation.world,
            super::SupportStyle::DenseWoodenLattice,
        )
        .unwrap();
        assert!(!supports.is_empty());
        assert!(supports.chunks_exact(7).all(|record| record[0] == -1.0));
        let id = 42;
        let model = super::wooden_twister_model();
        let supports = super::render_cached_supports(
            &[(&id, &loop_piece)],
            &simulation.world,
            &model.train,
            model.support_style,
            &[],
            &mut std::collections::HashMap::new(),
        )
        .unwrap();
        assert!(!supports.is_empty());
        assert!(supports.chunks_exact(7).all(|record| record[0] == -43.0));
    }

    #[test]
    fn geometry_encodes_material_and_preserves_piece_identity() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        let wooden = simulation.construction_placed_rails();
        assert!(wooden.chunks_exact(19).all(|record| record[0] == 0.0));
        simulation.construction_select("model:velocity_viper");
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("LaunchShort");
        simulation.construction_append();
        for (geometry, stride) in [
            (simulation.construction_placed_rails(), 19),
            (simulation.construction_placed_ties(), 13),
            (simulation.construction_placed_supports(), 7),
            (simulation.construction_rails(), 19),
            (simulation.construction_ties(), 13),
            (simulation.construction_supports(), 7),
        ] {
            assert!(!geometry.is_empty());
            assert!(
                geometry
                    .chunks_exact(stride)
                    .all(|record| record[0] < 0.0 && -record[0] - 1.0 >= 0.0)
            );
        }
        let ids: std::collections::HashSet<_> = simulation
            .construction_placed_rails()
            .chunks_exact(19)
            .map(|record| (-record[0] - 1.0) as usize)
            .collect();
        assert_eq!(ids, [0, 1].into_iter().collect());
    }

    #[test]
    fn construction_commands_validate_and_preview_without_mutating_on_failure() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        assert!(simulation.construction_active());
        simulation.construction_select("Straight");
        assert!(simulation.construction_can_append());
        assert!(!simulation.construction_rails().is_empty());
        let construction = simulation.construction.as_ref().unwrap();
        let obstacles =
            super::track_support_obstacles(construction.pieces(), &construction.model().train);
        let preview = simulation.construction_preview();
        let expected_supports = super::render_preview_supports(
            &preview[0],
            &obstacles,
            &simulation.world,
            super::SupportStyle::WoodenLattice,
        )
        .unwrap();
        assert_eq!(simulation.construction_supports(), expected_supports);
        simulation.construction_append();
        assert_eq!(simulation.construction_track(), "0\tStation\n1\tStraight");
        assert!(!simulation.construction_placed_rails().is_empty());
        simulation.construction_select("GentleUp");
        simulation.construction_append();
        assert_eq!(simulation.construction_track(), "0\tStation\n1\tStraight");
        assert!(!simulation.construction_message().is_empty());
        simulation.construction_undo();
        assert_eq!(simulation.construction_track(), "0\tStation");
    }

    #[test]
    fn deleting_middle_piece_preserves_and_identifies_retained_track() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        for _ in 0..2 {
            simulation.construction_select("Straight");
            simulation.construction_append();
            assert!(simulation.construction_message().is_empty());
        }
        simulation.construction_delete(1);
        assert_eq!(simulation.construction_track(), "0\tStation\n2\tStraight");
        let rails = simulation.construction_placed_rails();
        assert!(rails.chunks_exact(19).any(|rail| rail[0] == 2.0));
        simulation.construction_delete(2);
        assert_eq!(simulation.construction_track(), "0\tStation");
    }

    #[test]
    fn terrain_change_does_not_panic_when_rendering_supports() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        assert!(simulation.construction_active());
        assert!(!simulation.construction_placed_supports().is_empty());
        for y in 95..105 {
            for x in 95..110 {
                simulation.set_world_surface(x, y, vec![100; 4], None);
            }
        }
        let supports = simulation.construction_placed_supports();
        assert!(supports.is_empty());
        assert_eq!(
            simulation.construction_message(),
            "Could not route supports for the placed track."
        );
    }

    #[test]
    fn placed_support_cache_matches_full_render_after_track_changes() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        for _ in 0..4 {
            simulation.construction_select("Straight");
            simulation.construction_append();
            assert!(simulation.construction_message().is_empty());
        }
        for deleted in [None, Some(2), Some(4)] {
            if let Some(id) = deleted {
                simulation.construction_delete(id);
            }
            let construction = simulation.construction.as_ref().unwrap();
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
            let pieces: Vec<_> = entries.iter().map(|(_, piece)| (*piece).clone()).collect();
            let mut expected = super::try_render_supports(&pieces, &simulation.world).unwrap();
            for record in expected.chunks_exact_mut(7) {
                record[0] = *entries[record[0] as usize].0 as f32;
            }
            let entry_count = entries.len();
            assert_eq!(simulation.construction_placed_supports(), expected);
            assert_eq!(simulation.support_render_cache.len(), entry_count);
            assert_eq!(simulation.construction_placed_supports(), expected);
        }
    }

    #[test]
    fn deleting_distant_track_reuses_cached_supports() {
        let simulation = WasmSimulation::new(42, 0);
        let template = super::catalog()
            .into_iter()
            .find(|piece| piece.id == super::PieceId::Straight)
            .unwrap();
        let first = template
            .at(super::TrackState {
                x: 100,
                y: 100,
                z: 20,
                ..super::TrackState::origin()
            })
            .unwrap();
        let second = template
            .at(super::TrackState {
                x: 200,
                y: 200,
                z: 20,
                ..super::TrackState::origin()
            })
            .unwrap();
        let first_id = 1;
        let second_id = 2;
        let mut cache = std::collections::HashMap::new();
        let train = super::hybrid_model().train;
        let initial = super::render_cached_supports(
            &[(&first_id, &first), (&second_id, &second)],
            &simulation.world,
            &train,
            super::SupportStyle::WoodenLattice,
            &[],
            &mut cache,
        )
        .unwrap();
        let mut expected_initial =
            super::try_render_supports(&[first.clone(), second], &simulation.world).unwrap();
        for record in expected_initial.chunks_exact_mut(7) {
            record[0] += first_id as f32;
        }
        assert_eq!(initial, expected_initial);
        let segments = cache[&first_id].segments.as_ref().unwrap();
        assert!(!segments.is_empty());
        let original_allocation = segments.as_ptr();
        let cached = super::render_cached_supports(
            &[(&first_id, &first)],
            &simulation.world,
            &train,
            super::SupportStyle::WoodenLattice,
            &[],
            &mut cache,
        )
        .unwrap();
        assert_eq!(
            cache[&first_id].segments.as_ref().unwrap().as_ptr(),
            original_allocation
        );
        let mut expected = super::try_render_supports(&[first], &simulation.world).unwrap();
        for record in expected.chunks_exact_mut(7) {
            record[0] = first_id as f32;
        }
        assert_eq!(cached, expected);
    }

    #[test]
    fn rendered_supports_clear_other_tracks_train_envelope() {
        let world = super::World::new(
            128,
            128,
            super::Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let template = super::catalog()
            .into_iter()
            .find(|piece| piece.id == super::PieceId::Straight)
            .unwrap();
        let lower = template
            .at(super::TrackState {
                x: 50,
                y: 48,
                z: 3,
                ..super::TrackState::origin()
            })
            .unwrap();
        let upper = template
            .at(super::TrackState {
                x: 54,
                y: 46,
                z: 5,
                heading: super::Heading::North,
                ..super::TrackState::origin()
            })
            .unwrap();
        let obstacles = theme_park_sim::track::construction::track_support_obstacles(
            std::slice::from_ref(&lower),
            &super::hybrid_model().train,
        );
        let expected = super::generate_wooden_lattice(&upper, &world, &obstacles).unwrap();
        let mut expected_records = Vec::new();
        for segment in expected {
            if segment.start != segment.end {
                expected_records.push(1.0);
                super::append_vector(&mut expected_records, segment.start / 4.0);
                super::append_vector(&mut expected_records, segment.end / 4.0);
            }
        }
        let mut cache = std::collections::HashMap::new();
        let actual = super::render_cached_supports(
            &[(&0, &lower), (&1, &upper)],
            &world,
            &super::hybrid_model().train,
            super::SupportStyle::WoodenLattice,
            &[],
            &mut cache,
        )
        .unwrap();
        let upper_records: Vec<_> = actual
            .chunks_exact(7)
            .filter(|record| record[0] == 1.0)
            .flatten()
            .copied()
            .collect();
        assert_eq!(upper_records, expected_records);
        let preview = super::render_preview_supports(
            &upper,
            &obstacles,
            &world,
            super::SupportStyle::WoodenLattice,
        )
        .unwrap();
        let mut expected_preview = expected_records;
        for record in expected_preview.chunks_exact_mut(7) {
            record[0] = 0.0;
        }
        assert_eq!(preview, expected_preview);
    }

    #[test]
    fn lift_drop_circuit_completes_lap() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(100, 100, 40, 0);
        for piece in [
            "LiftLevelToGentleUp",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "LiftGentleUpToLevel",
            "LiftFlat",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
            "LevelToGentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDownToLevel",
            "Loop",
            "TurnLargeEntry",
            "TurnLargeExit",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(
                simulation.construction_message().is_empty(),
                "{piece}: {}",
                simulation.construction_message()
            );
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        assert!(simulation.support_render_cache.is_empty());
        simulation.construction_test();
        for _ in 0..9000 {
            simulation.tick();
            if simulation.construction_message() == "Train completed a lap." {
                assert!(!simulation.support_render_cache.is_empty());
                assert!(
                    simulation.support_render_cache.values().any(|entry| !entry
                        .segments
                        .as_ref()
                        .unwrap()
                        .is_empty())
                );
                return;
            }
            if simulation.construction_outcome.is_some() {
                break;
            }
        }
        let (circuit, train) = simulation.construction_train.as_ref().unwrap();
        let mut start = 0.0;
        let mut piece = "unknown";
        for candidate in circuit.pieces() {
            if train.distance() < start + candidate.length() {
                piece = candidate.name;
                break;
            }
            start += candidate.length();
        }
        panic!(
            "{}: distance={}, speed={}, length={}, piece={piece}",
            simulation.construction_message(),
            train.distance(),
            train.speed(),
            circuit.length()
        );
    }

    #[test]
    fn opening_after_a_completed_test_clears_the_report() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(100, 100, 40, 0);
        for piece in [
            "LiftLevelToGentleUp",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "LiftGentleUpToLevel",
            "LiftFlat",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
            "LevelToGentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDownToLevel",
            "Loop",
            "TurnLargeEntry",
            "TurnLargeExit",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(
                simulation.construction_message().is_empty(),
                "{piece}: {}",
                simulation.construction_message()
            );
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        simulation.construction_test();
        for _ in 0..9000 {
            simulation.tick();
            if simulation.construction_message() == "Train completed a lap." {
                let station = &simulation.construction.as_ref().unwrap().pieces()[0];
                let (queue, exit) = super::station_positions(station).unwrap();
                for (position, kind) in [
                    (queue, super::PathKind::Queue),
                    (exit, super::PathKind::Normal),
                ] {
                    simulation.world.set_surface(
                        position.x,
                        position.y,
                        super::Surface {
                            corners: [position.height; 4],
                            water: None,
                        },
                    );
                    simulation
                        .world
                        .build_path_route(position, position, kind, 0)
                        .unwrap();
                }
                assert!(
                    simulation
                        .construction_test_report_json()
                        .contains("\"rating\"")
                );
                assert!(simulation.construction_set_open(true));
                assert_eq!(simulation.construction_test_report_json(), "null");
                return;
            }
            if simulation.construction_outcome.is_some() {
                break;
            }
        }
        let (circuit, train) = simulation.construction_train.as_ref().unwrap();
        let mut start = 0.0;
        let mut piece = "unknown";
        for candidate in circuit.pieces() {
            if train.distance() < start + candidate.length() {
                piece = candidate.name;
                break;
            }
            start += candidate.length();
        }
        panic!(
            "{}: distance={}, speed={}, length={}, piece={piece}",
            simulation.construction_message(),
            train.distance(),
            train.speed(),
            circuit.length()
        );
    }

    #[test]
    fn construction_closes_and_runs_physics_deterministically() {
        let mut simulation = WasmSimulation::new(42, 0);
        assert_eq!(simulation.construction_test_report_json(), "null");
        simulation.construction_start(200, 200, 20, 0);
        for piece in [
            "TurnSmallStandalone",
            "TurnSmallStandalone",
            "Straight",
            "TurnSmallStandalone",
            "TurnSmallStandalone",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(
                simulation.construction_message().is_empty(),
                "{}",
                simulation.construction_message()
            );
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        assert!(simulation.construction_can_test());
        simulation.construction_test();
        assert_eq!(simulation.construction_test_report_json(), "null");
        let initial_poses = simulation.construction_train_poses();
        assert!(initial_poses.chunks_exact(11).all(|car| car[10] == 0.0));
        for _ in 0..9000 {
            simulation.tick();
            if simulation.construction_message() == "Train stopped before completing a lap." {
                break;
            }
        }
        assert_ne!(simulation.construction_train_poses(), initial_poses);
        assert_eq!(
            simulation.construction_message(),
            "Train stopped before completing a lap."
        );
        let report = simulation.construction_test_report_json();
        assert!(report.contains("\"samples\":[{"), "{report}");
        assert!(report.contains("\"longitudinalG\":"), "{report}");
        assert!(report.contains("\"rating\":{"), "{report}");
        assert!(report.contains("\"proximityEvents\":["), "{report}");
        assert!(report.contains("\"notes\":[\""), "{report}");
        let stopped_poses = simulation.construction_train_poses();
        for _ in 0..60 {
            simulation.tick();
        }
        simulation.construction_select("Straight");
        assert_eq!(simulation.construction_train_poses(), stopped_poses);
        assert_eq!(
            simulation.construction_message(),
            "Train stopped before completing a lap."
        );
        simulation.construction_delete(5);
        assert_eq!(simulation.construction_test_report_json(), "null");
        let (_, recovered_train) = simulation.construction_train.as_ref().unwrap();
        assert_eq!(recovered_train.speed(), 0.0);
        let recovered_poses = simulation.construction_train_poses();
        let station = &simulation.construction.as_ref().unwrap().pieces()[0];
        assert_eq!(recovered_train.distance(), station.length());
        let config = &simulation.construction.as_ref().unwrap().model().train;
        assert_eq!(recovered_poses.len(), usize::from(config.cars) * 11);
        for (index, car) in recovered_poses.chunks_exact(11).enumerate() {
            let distance = (station.length()
                - index as f64 * config.length_metres / f64::from(config.cars))
            .max(0.0);
            let expected = station.sample(distance).position / 4.0;
            assert_eq!(
                &car[1..4],
                &[expected.x as f32, expected.y as f32, expected.z as f32]
            );
        }
        for _ in 0..60 {
            simulation.tick();
        }
        assert_eq!(simulation.construction_train_poses(), recovered_poses);
        assert!(!simulation.construction_can_test());
        simulation.construction_delete(0);
        assert!(simulation.construction_train_poses().is_empty());
    }
}

fn render_rails(pieces: &[Piece], _world: &World) -> Vec<f32> {
    let mut result = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let intervals = (piece.length() / 2.0).ceil() as usize;
        for sample_index in 0..intervals {
            let start = piece.sample(piece.length() * sample_index as f64 / intervals as f64);
            let end = piece.sample(piece.length() * (sample_index + 1) as f64 / intervals as f64);
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

fn render_ties(pieces: &[Piece], _world: &World) -> Vec<f32> {
    let mut result = Vec::new();
    let mut piece_start = 0.0;
    let mut next_tie = 0_usize;
    for (index, piece) in pieces.iter().enumerate() {
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

fn render_supports(pieces: &[Piece], world: &World) -> Vec<f32> {
    try_render_supports(pieces, world).expect("demo support cannot be routed")
}

fn render_preview_supports(
    piece: &Piece,
    obstacles: &[SupportObstacle],
    world: &World,
    style: SupportStyle,
) -> Result<Vec<f32>, SupportError> {
    let piece_style = style.for_piece(piece);
    let supports = generate_supports(piece, world, obstacles, piece_style)?;
    let mut result = Vec::new();
    for support in supports {
        if support.start != support.end {
            result.push(
                if piece_style == SupportStyle::SteelColumns && style != piece_style {
                    -1.0
                } else {
                    0.0
                },
            );
            append_vector(&mut result, support.start / 4.0);
            append_vector(&mut result, support.end / 4.0);
        }
    }
    Ok(result)
}

fn sync_support_obstacles(
    entries: &[(&usize, &Piece)],
    train: &TrainConfiguration,
    cache: &mut HashMap<usize, SupportRenderCache>,
) {
    cache.retain(|id, _| entries.iter().any(|(entry_id, _)| *entry_id == id));
    for (id, piece) in entries {
        cache.entry(**id).or_insert_with(|| SupportRenderCache {
            obstacles: track_support_obstacles(std::slice::from_ref(piece), train),
            dependencies: Vec::new(),
            world_revision: u64::MAX,
            segments: Err(SupportError::NoRoute),
        });
    }
}

fn render_cached_supports(
    entries: &[(&usize, &Piece)],
    world: &World,
    train: &TrainConfiguration,
    style: SupportStyle,
    facility_obstacles: &[SupportObstacle],
    cache: &mut HashMap<usize, SupportRenderCache>,
) -> Result<Vec<f32>, SupportError> {
    sync_support_obstacles(entries, train, cache);
    let mut result = Vec::new();
    for (id, piece) in entries {
        let piece_style = style.for_piece(piece);
        let spacing = if piece_style == SupportStyle::DenseWoodenLattice {
            4.0
        } else {
            8.0
        };
        let count = (piece.length() / spacing).ceil() as usize;
        let anchors: Vec<_> = (0..count)
            .map(|index| {
                piece
                    .sample(piece.length() * (index as f64 + 0.5) / count as f64)
                    .position
            })
            .collect();
        let mut dependencies: Vec<_> = entries
            .iter()
            .filter_map(|(other_id, _)| {
                if other_id == id {
                    return None;
                }
                cache[other_id]
                    .obstacles
                    .iter()
                    .any(|obstacle| {
                        anchors.iter().any(|anchor| {
                            obstacle.min.x <= anchor.x + SUPPORT_INFLUENCE_RADIUS_METRES
                                && obstacle.max.x >= anchor.x - SUPPORT_INFLUENCE_RADIUS_METRES
                                && obstacle.min.y <= anchor.y + SUPPORT_INFLUENCE_RADIUS_METRES
                                && obstacle.max.y >= anchor.y - SUPPORT_INFLUENCE_RADIUS_METRES
                        })
                    })
                    .then_some(**other_id)
            })
            .collect();
        dependencies.sort_unstable();
        let cached = &cache[id];
        if cached.world_revision != world.revision() || cached.dependencies != dependencies {
            let mut obstacles: Vec<_> = entries
                .iter()
                .filter(|(other_id, _)| dependencies.binary_search(other_id).is_ok())
                .flat_map(|(other_id, _)| cache[other_id].obstacles.iter().copied())
                .collect();
            obstacles.extend_from_slice(facility_obstacles);
            let segments = generate_supports(piece, world, &obstacles, piece_style);
            let cached = cache.get_mut(id).expect("support cache entry");
            cached.world_revision = world.revision();
            cached.dependencies = dependencies;
            cached.segments = segments;
        }
        for segment in cache[id].segments.as_ref().map_err(|error| *error)? {
            if segment.start != segment.end {
                let id = **id as f32;
                result.push(
                    if piece_style == SupportStyle::SteelColumns && style != piece_style {
                        -id - 1.0
                    } else {
                        id
                    },
                );
                append_vector(&mut result, segment.start / 4.0);
                append_vector(&mut result, segment.end / 4.0);
            }
        }
    }
    Ok(result)
}

fn generate_supports(
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
    style: SupportStyle,
) -> Result<Vec<SupportSegment>, SupportError> {
    match style {
        SupportStyle::WoodenLattice => generate_wooden_lattice(piece, world, obstacles),
        SupportStyle::DenseWoodenLattice => generate_dense_wooden_lattice(piece, world, obstacles),
        SupportStyle::SteelColumns => generate_steel_columns(piece, world, obstacles),
    }
}

fn try_render_supports(
    pieces: &[Piece],
    world: &World,
) -> Result<Vec<f32>, theme_park_sim::track::supports::SupportError> {
    let mut result = Vec::new();
    let model = hybrid_model();
    for (index, piece) in pieces.iter().enumerate() {
        let obstacles: Vec<_> = pieces
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .flat_map(|(_, other)| {
                track_support_obstacles(std::slice::from_ref(other), &model.train)
            })
            .collect();
        let supports = generate_supports(
            piece,
            world,
            &obstacles,
            model.support_style_for_piece(piece),
        )?;
        for support in supports {
            if support.start != support.end {
                result.push(index as f32);
                append_vector(&mut result, support.start / 4.0);
                append_vector(&mut result, support.end / 4.0);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod piece_shape_tests {
    #[test]
    fn catalog_names_uniquely_identify_shapes_in_construction_track() {
        let names: std::collections::HashSet<_> = super::catalog()
            .into_iter()
            .map(|piece| piece.name)
            .collect();
        assert_eq!(names.len(), super::catalog().len());
    }

    #[test]
    fn launch_shapes_expose_their_kind() {
        let json = super::piece_shapes_json();
        for id in ["LaunchShort", "Launch", "LaunchLong"] {
            let shape = json
                .split("{\"id\":")
                .find(|shape| shape.starts_with(&format!("\"{id}\"")))
                .unwrap();
            assert!(shape.contains("\"kind\":\"Launch\""));
        }
    }

    #[test]
    fn piece_shapes_describe_every_catalog_piece_from_a_shared_entry_frame() {
        let json = super::piece_shapes_json();
        assert_eq!(json.matches("\"id\":").count(), super::catalog().len());
        let straight = json
            .split("{\"id\":")
            .find(|shape| shape.starts_with("\"Straight\""))
            .expect("straight shape");
        assert!(straight.contains("\"turn\":0"));
        assert!(straight.starts_with("\"Straight\",\"name\":\"Straight\",\"kind\":\"Track\""));
        assert!(straight.contains("[0.000,0.000,0.000,0.000,0.000,1.000]"));
        assert!(straight.contains("[16.000,0.000,0.000,0.000,0.000,1.000]]"));
        let turn = json
            .split("{\"id\":")
            .find(|shape| shape.starts_with("\"TurnSmallStandalone\""))
            .expect("small turn shape");
        assert!(turn.contains("\"turn\":2"));
    }
}

#[cfg(test)]
mod path_bridge_tests {
    use super::{Surface, WasmSimulation};
    use theme_park_sim::world::{Position, World};

    #[test]
    fn tunnel_path_commands_refresh_the_guest_graph() {
        let mut simulation = WasmSimulation::new(42, 12);
        let entrance = Position {
            x: 218,
            y: 250,
            height: 8,
        };
        let tunnel = Position {
            x: 224,
            y: 250,
            height: 3,
        };
        assert_eq!(
            simulation.simulation.walking().path_component(entrance),
            simulation.simulation.walking().path_component(tunnel)
        );

        assert!(simulation.path_remove(221, 250, 5));
        simulation.tick();
        assert_ne!(
            simulation.simulation.walking().path_component(entrance),
            simulation.simulation.walking().path_component(tunnel)
        );
        let before_preview = simulation.world_state_hash();
        let coordinates = vec![218, 250, 8, 224, 250, 3];
        let preview = simulation.path_preview(coordinates.clone(), "normal", 0);
        assert_eq!(preview.len(), 7 * 9);
        assert!(preview.chunks_exact(9).all(|tile| tile[6] == 0));
        assert!(
            preview
                .chunks_exact(9)
                .any(|tile| tile[..4] == [221, 250, 5, 4])
        );
        assert_eq!(simulation.world_state_hash(), before_preview);

        assert!(simulation.path_build(coordinates, "normal", 0));
        assert!(
            simulation
                .world_paths()
                .chunks_exact(4)
                .any(|path| path == [221, 250, 5, 4])
        );
        simulation.tick();
        assert_eq!(
            simulation.simulation.walking().path_component(entrance),
            simulation.simulation.walking().path_component(tunnel)
        );
    }

    #[test]
    fn path_preview_build_snapshot_and_remove_share_the_path_wire_contract() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            4,
            2,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();

        let before_preview = simulation.world_state_hash();
        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 1, 2, 0, 2,
                0, // x, y, height, slope, kind, style, refusal, edges, openings
                1, 0, 0, 0, 1, 2, 0, 10, 0, 2, 0, 0, 0, 1, 2, 0, 8, 0,
            ]
        );
        assert_eq!(simulation.world_state_hash(), before_preview);
        assert!(simulation.path_message().is_empty());

        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
        assert_eq!(
            simulation.world_paths(),
            vec![0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]
        );
        assert_eq!(
            simulation.world_path_metadata(),
            vec![1, 2, 2, 1, 2, 10, 1, 2, 8]
        );

        assert!(simulation.path_remove(1, 0, 0));
        assert_eq!(simulation.world_paths(), vec![0, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(simulation.world_path_metadata(), vec![1, 2, 0, 1, 2, 0]);
        let revision = simulation.world.revision();
        assert!(!simulation.path_remove(1, 0, 0));
        assert_eq!(simulation.world.revision(), revision);
    }

    #[test]
    fn path_preview_reports_explicit_edges_for_adjacent_queue_paths_and_connections() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            4,
            2,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 0));
        assert!(simulation.path_build(vec![2, 1, 0, 2, 1, 0], "queue", 0));
        assert_eq!(
            simulation.world_path_metadata(),
            vec![1, 0, 2, 1, 0, 10, 1, 0, 8, 1, 0, 0]
        );

        let preview = simulation.path_preview(vec![2, 1, 0, 2, 0, 0], "queue", 0);
        assert_eq!(
            preview,
            vec![2, 1, 0, 0, 1, 0, 0, 1, 0, 2, 0, 0, 0, 1, 0, 0, 12, 0]
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![2, 1, 0, 2, 0, 0], "queue", 0));
        assert_eq!(
            simulation.world_path_metadata(),
            vec![1, 0, 2, 1, 0, 10, 1, 0, 12, 1, 0, 1]
        );
    }

    #[test]
    fn facility_openings_follow_ride_placement_and_access_paths() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            8,
            8,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![4, 3, 0, 4, 3, 0], "queue", 0));
        assert!(simulation.path_build(vec![5, 3, 0, 5, 3, 0], "normal", 0));
        let id = simulation
            .facilities
            .place(
                &simulation.world,
                super::FacilityKind::Carousel,
                Position {
                    x: 4,
                    y: 4,
                    height: 0,
                },
                super::Direction::North,
            )
            .unwrap();
        assert_eq!(
            simulation.world_facility_openings(),
            vec![4, 3, 0, 4, 5, 3, 0, 4]
        );
        assert!(simulation.path_remove(4, 3, 0));
        assert_eq!(
            simulation.world_facility_openings(),
            vec![4, 3, 0, 4, 5, 3, 0, 4]
        );
        let preview = simulation.path_preview(vec![4, 3, 0, 4, 3, 0], "queue", 0);
        assert_eq!(preview[8], 4);
        assert!(simulation.path_build(vec![3, 3, 0, 4, 3, 0], "queue", 0));
        let before = simulation.world_state_hash();
        let invalid = simulation.path_preview(vec![4, 2, 0, 4, 3, 0], "queue", 0);
        assert!(
            invalid
                .chunks_exact(9)
                .all(|tile| tile[6] == 9 && tile[8] == 0)
        );
        assert!(!simulation.path_build(vec![4, 2, 0, 4, 3, 0], "queue", 0));
        assert_eq!(simulation.world_state_hash(), before);
        assert!(simulation.remove_facility(id));
        assert!(simulation.world_facility_openings().is_empty());
    }

    #[test]
    fn path_preview_reports_existing_tile_metadata_that_build_preserves() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            4,
            2,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "normal", 3);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 0, 1, 0, 2, 0, 1, 0, 0, 0, 0, 3, 0, 10, 0, 2, 0, 0, 0, 0, 3, 0, 8, 0
            ]
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "normal", 3));
        assert_eq!(
            simulation.world_path_metadata(),
            vec![0, 1, 2, 0, 3, 10, 0, 3, 8]
        );
    }

    #[test]
    fn queue_preview_keeps_a_reused_normal_path_endpoint_normal() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            4,
            2,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 0, 1, 0, 2, 0, 1, 0, 0, 0, 1, 2, 0, 10, 0, 2, 0, 0, 0, 1, 2, 0, 8, 0
            ]
        );
        simulation.facilities = super::ParkFacilities::new();
        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
        assert_eq!(
            simulation.world_path_metadata(),
            vec![0, 1, 2, 1, 2, 10, 1, 2, 8]
        );
    }

    #[test]
    fn invalid_path_preview_returns_a_message_without_mutating_the_world() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.world = World::new(
            4,
            2,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        simulation.facilities = super::ParkFacilities::new();

        let before_preview = simulation.world_state_hash();
        let preview = simulation.path_preview(vec![0, 0, 0, 3, 0, 1], "normal", 0);
        assert_eq!(preview.len(), 4 * 9);
        assert!(preview.chunks_exact(9).any(|tile| tile[6] != 0));
        assert!(!simulation.path_message().is_empty());
        assert_eq!(simulation.world_state_hash(), before_preview);
        assert!(!simulation.path_build(vec![0, 0, 0, 3, 0, 1], "unknown", 0));
        assert_eq!(simulation.world_state_hash(), before_preview);
        assert!(
            simulation
                .path_preview(vec![0, 0, 0], "normal", 0)
                .is_empty()
        );
        assert!(simulation.path_message().contains("six coordinates"));
        assert_eq!(simulation.world_state_hash(), before_preview);
    }

    #[test]
    fn path_preview_and_build_reject_a_facility_footprint() {
        let mut simulation = WasmSimulation::new(42, 0);
        let building_position = simulation.facilities.buildings()[0].position;
        let coordinates = vec![
            i32::from(building_position.x),
            i32::from(building_position.y),
            i32::from(building_position.height),
            i32::from(building_position.x),
            i32::from(building_position.y),
            i32::from(building_position.height),
        ];
        let revision = simulation.world.revision();
        let preview = simulation.path_preview(coordinates.clone(), "normal", 0);
        assert_eq!(preview.len(), 9);
        assert_eq!(preview[6], 8);
        assert!(simulation.path_message().contains("facility"));
        assert!(!simulation.path_build(coordinates, "normal", 0));
        assert_eq!(simulation.world.revision(), revision);
    }
}

#[cfg(test)]
mod terrain_bridge_tests {
    use super::WasmSimulation;
    use theme_park_sim::facilities::ParkFacilities;
    use theme_park_sim::world::{Ground, Path, Position, Surface, World};

    #[test]
    fn terrain_edits_refuse_buildings_and_track_without_mutating_the_world() {
        let mut simulation = WasmSimulation::new(42, 0);
        let before_building_edit = simulation.world_state_hash();
        assert!(!simulation.terrain_apply("raise_tiles", vec![218, 246, 1, 0, 1, 0, 0, 0]));
        assert_eq!(simulation.world_state_hash(), before_building_edit);
        assert!(simulation.terrain_message().contains("building or ride"));

        let station = simulation.circuit.pieces()[0].sample(0.0).position;
        let track_x = (station.x / 4.0).floor() as u16;
        let track_y = (station.y / 4.0).floor() as u16;
        let before_track_edit = simulation.world_state_hash();
        assert!(!simulation.terrain_apply(
            "flatten",
            vec![i32::from(track_x), i32::from(track_y), 1, 0, 0, 100, 0, 0,]
        ));
        assert_eq!(simulation.world_state_hash(), before_track_edit);
        assert!(simulation.terrain_message().contains("coaster track"));
    }

    #[test]
    fn terrain_wire_rejects_missing_parameters_without_mutating_the_world() {
        let mut simulation = WasmSimulation::new(42, 0);
        let before = simulation.world_state_hash();
        assert!(!simulation.terrain_apply("flatten", vec![1, 2, 3]));
        assert_eq!(simulation.world_state_hash(), before);
        assert!(
            simulation
                .terrain_message()
                .contains("eight integer parameters")
        );
    }

    #[test]
    fn tunnel_fill_reports_invalidated_paths_and_guests_recover() {
        let mut simulation = WasmSimulation::new(42, 12);
        let tunnel_path = Position {
            x: 221,
            y: 250,
            height: 5,
        };
        assert!(simulation.world.path(tunnel_path).is_some());
        assert!(simulation.terrain_apply("fill", vec![221, 250, 1, 0, 0, 0, 5, 7]));
        assert_eq!(simulation.terrain_invalidated_paths(), vec![221, 250, 5]);
        assert!(simulation.terrain_message().contains("updated"));
        assert!(simulation.world.path(tunnel_path).is_none());

        simulation.tick();
        assert!(
            simulation
                .simulation
                .walking()
                .guests()
                .iter()
                .all(|guest| guest.path.is_some())
        );
    }

    #[test]
    fn facility_bridge_accepts_a_shop_inside_a_carved_cavern() {
        let mut simulation = WasmSimulation::new(42, 0);
        let mut world = World::new(
            6,
            6,
            Surface {
                corners: [8; 4],
                water: None,
            },
        );
        for (x, y) in [(2, 1), (2, 2), (3, 2), (2, 3), (3, 3)] {
            world.carve(x, y, 1, 4);
        }
        world.place_path(Path {
            position: Position {
                x: 2,
                y: 1,
                height: 1,
            },
            slope: None,
        });
        simulation.world = world;
        simulation.facilities = ParkFacilities::new();

        assert!(simulation.place_facility_at("food_stall", 2, 2, 0, 1));
        assert!(
            simulation
                .facilities_json()
                .contains("\"x\":2,\"y\":2,\"z\":1")
        );
    }

    #[test]
    fn coaster_station_can_be_constructed_inside_a_carved_cavern() {
        let mut simulation = WasmSimulation::new(42, 0);
        let mut world = World::new(
            16,
            16,
            Surface {
                corners: [12; 4],
                water: None,
            },
        );
        for y in 0..16 {
            for x in 0..16 {
                world.carve(x, y, 3, 12);
            }
        }
        simulation.world = world;

        simulation.construction_start(16, 16, 4, 0);
        assert!(
            simulation.construction_active(),
            "{}",
            simulation.construction_message()
        );
        assert_eq!(simulation.world.ground(4, 4, 8), Ground::Carved);
        assert_eq!(simulation.construction_station_positions()[2], 8);
    }

    #[test]
    fn terrain_edit_reroutes_constructed_track_supports() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("Straight");
        simulation.construction_append();
        let supports_before = simulation.construction_placed_supports();
        assert!(
            !supports_before.is_empty(),
            "{}",
            simulation.construction_message()
        );
        let first_support = supports_before.chunks_exact(7).next().unwrap();
        let x = first_support[4].floor() as u16;
        let y = first_support[5].floor() as u16;

        assert!(simulation.terrain_apply(
            "raise_tiles",
            vec![i32::from(x), i32::from(y), 1, 0, 1, 0, 0, 0]
        ));
        let supports_after = simulation.construction_placed_supports();
        assert!(
            !supports_after.is_empty(),
            "{}",
            simulation.construction_message()
        );
        assert_ne!(supports_after, supports_before);
    }
}
