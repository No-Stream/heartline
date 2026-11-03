mod guest_benchmark;

use std::collections::HashMap;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::facilities::{
    FacilityKind, ParkFacilities, coaster_facility, station_positions,
};
use theme_park_sim::guest_behavior::{Facility, RidePhase};
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{
    Circuit, CoasterRun, PhysicsConfig, TestRun, TestRunRecorder, Train, demo_circuit,
};
use theme_park_sim::track::clearance::{ClearanceCollision, check_clearance};
use theme_park_sim::track::construction::{Construction, track_support_obstacles};
use theme_park_sim::track::supports::{
    SUPPORT_INFLUENCE_RADIUS_METRES, SupportError, SupportObstacle, SupportSegment,
    generate_steel_columns, generate_wooden_lattice,
};
use theme_park_sim::track::{
    Heading, Piece, PieceId, PieceKind, SupportStyle, TrackSample, TrackState, TrainConfiguration,
    Vec3, catalog, hybrid_model, multi_launch_model,
};
use theme_park_sim::world::{
    Brush, Corner, Direction, PathBuildError, PathKind, PathPreviewTile, Position, Surface,
    TerrainCommand, TerrainEditError, World,
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
    coaster_run: Option<(Circuit, CoasterRun)>,
    coaster_riders: Vec<GuestId>,
    coaster_facility: Option<Facility>,
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
            coaster_run: None,
            coaster_riders: Vec::new(),
            coaster_facility: None,
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

    /// Path kind and style pairs aligned with `world_paths()`.
    pub fn world_path_metadata(&self) -> Vec<i32> {
        self.world
            .paths()
            .flat_map(|path| {
                let metadata = self
                    .world
                    .path_metadata(path.position)
                    .expect("world path metadata");
                [metadata.kind as i32, i32::from(metadata.style)]
            })
            .collect()
    }

    /// Preview route entries: x, y, height, slope, kind, style, refusal code.
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
        if self.coaster_open && self.coaster_riders.is_empty() {
            self.coaster_riders = self.simulation.board_coaster(
                self.coaster_facility
                    .as_ref()
                    .expect("open coaster has access"),
            );
        }
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            self.train.step(&self.circuit);
            let outcome = if self.coaster_riders.is_empty() {
                None
            } else {
                self.coaster_run
                    .as_mut()
                    .and_then(|(circuit, run)| run.step(circuit))
            };
            if let Some(outcome) = outcome {
                let construction = self.construction.as_ref().expect("open coaster has track");
                let seats_per_car = usize::from(construction.model().train.rows_per_car)
                    * usize::from(construction.model().train.seats_per_row);
                let ratings: Vec<_> = (0..self.coaster_riders.len())
                    .map(|seat| {
                        let report = &outcome.reports[seat / seats_per_car];
                        (report.excitement, report.intensity, report.nausea)
                    })
                    .collect();
                self.simulation.finish_coaster(
                    &self.world,
                    self.coaster_facility
                        .as_ref()
                        .expect("open coaster has access"),
                    &ratings,
                    outcome.recovered,
                );
                self.coaster_riders.clear();
                let (circuit, run) = self.coaster_run.as_mut().expect("open coaster has train");
                *run = CoasterRun::new(
                    circuit,
                    &construction.model().train,
                    PhysicsConfig {
                        lift_speed: 5.0,
                        ..PhysicsConfig::default()
                    },
                );
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
                    }
                    if self.construction_outcome.is_some() {
                        let recorder = self.construction_recorder.take().expect("test recorder");
                        self.construction_report = Some(recorder.finish(
                            self.construction_message == "Train completed a lap.",
                            !train.notes().is_empty(),
                        ));
                    }
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
        let run = CoasterRun::new(
            &circuit,
            &construction.model().train,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
        );
        self.coaster_run = Some((circuit, run));
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
        self.coaster_riders
            .iter()
            .enumerate()
            .flat_map(|(seat, guest)| {
                let seat = seat as u32;
                [seat / seats_per_car, seat % seats_per_car, guest.0]
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
        format!(
            "{{\"samples\":[{samples}],\"notes\":[{notes}],\"rating\":{{\"excitement\":{},\"intensity\":{},\"nausea\":{}}}}}",
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
        let live = self
            .coaster_run
            .as_ref()
            .map(|(circuit, run)| (circuit, run.train()));
        let Some((circuit, train)) = live.or(self
            .construction_train
            .as_ref()
            .map(|(circuit, train)| (circuit, train)))
        else {
            return Vec::new();
        };
        let config = &self
            .construction
            .as_ref()
            .expect("train requires construction")
            .model()
            .train;
        let mut result = Vec::new();
        for index in 0..usize::from(config.cars) {
            let offset = index as f64 * config.length_metres / f64::from(config.cars);
            let sample = match (&self.recovered_station, live.is_some()) {
                (Some(station), false) => station.sample((station.length() - offset).max(0.0)),
                _ => circuit.sample(train.distance() - offset),
            };
            result.push(index as f32);
            append_vector(&mut result, sample.position / 4.0);
            append_vector(&mut result, sample.tangent);
            append_vector(&mut result, sample.up);
            let seats_per_car =
                usize::from(config.rows_per_car) * usize::from(config.seats_per_row);
            let occupied = self
                .coaster_riders
                .len()
                .saturating_sub(index * seats_per_car)
                .min(seats_per_car);
            result.push(((1_u32 << occupied) - 1) as f32);
        }
        result
    }
    pub fn state_hash(&self) -> u64 {
        self.simulation.walking().state_hash()
    }
}

impl WasmSimulation {
    fn close_live_coaster(&mut self) {
        if !self.coaster_riders.is_empty() {
            let (_, run) = self.coaster_run.as_mut().expect("riders have a train");
            let outcome = run.recover();
            self.simulation.finish_coaster(
                &self.world,
                self.coaster_facility
                    .as_ref()
                    .expect("riders have a station"),
                &[],
                outcome.recovered,
            );
            self.coaster_riders.clear();
        }
        if let Some(facility) = &self.coaster_facility {
            self.simulation.close_coaster(&self.world, facility);
        }
        self.coaster_open = false;
        self.coaster_run = None;
        self.coaster_facility = None;
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
        Ok(preview)
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

#[cfg(test)]
mod world_tests {
    use super::{Position, Surface, WasmSimulation, track_support_obstacles};
    use theme_park_sim::track::supports::generate_supports;
    use theme_park_sim::track::{PieceId, TrackState, catalog, hybrid_model};
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
        simulation.construction_test();
        for _ in 0..9000 {
            simulation.tick();
            if simulation.construction_message() == "Train completed a lap." {
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
    let supports = generate_supports(piece, world, obstacles, style)?;
    let mut result = Vec::new();
    for support in supports {
        if support.start != support.end {
            result.push(0.0);
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
        let count = (piece.length() / 8.0).ceil() as usize;
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
            let segments = generate_supports(piece, world, &obstacles, style);
            let cached = cache.get_mut(id).expect("support cache entry");
            cached.world_revision = world.revision();
            cached.dependencies = dependencies;
            cached.segments = segments;
        }
        for segment in cache[id].segments.as_ref().map_err(|error| *error)? {
            if segment.start != segment.end {
                result.push(**id as f32);
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
        let supports = match model.support_style {
            SupportStyle::WoodenLattice => generate_wooden_lattice(piece, world, &obstacles),
            SupportStyle::SteelColumns => generate_steel_columns(piece, world, &obstacles),
        }?;
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
        assert_eq!(preview.len(), 7 * 7);
        assert!(preview.chunks_exact(7).all(|tile| tile[6] == 0));
        assert!(
            preview
                .chunks_exact(7)
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

        let before_preview = simulation.world_state_hash();
        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 1, 2, 0, // x, y, height, slope, kind, style, refusal
                1, 0, 0, 0, 1, 2, 0, 2, 0, 0, 0, 1, 2, 0,
            ]
        );
        assert_eq!(simulation.world_state_hash(), before_preview);
        assert!(simulation.path_message().is_empty());

        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
        assert_eq!(
            simulation.world_paths(),
            vec![0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]
        );
        assert_eq!(simulation.world_path_metadata(), vec![1, 2, 1, 2, 1, 2]);

        assert!(simulation.path_remove(1, 0, 0));
        assert_eq!(simulation.world_paths(), vec![0, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(simulation.world_path_metadata(), vec![1, 2, 1, 2]);
        let revision = simulation.world.revision();
        assert!(!simulation.path_remove(1, 0, 0));
        assert_eq!(simulation.world.revision(), revision);
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
        assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "normal", 3);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 3, 0, 2, 0, 0, 0, 0, 3, 0
            ]
        );
        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "normal", 3));
        assert_eq!(simulation.world_path_metadata(), vec![0, 1, 0, 3, 0, 3]);
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
        assert!(simulation.path_build(vec![0, 0, 0, 0, 0, 0], "normal", 1));

        let preview = simulation.path_preview(vec![0, 0, 0, 2, 0, 0], "queue", 2);
        assert_eq!(
            preview,
            vec![
                0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 2, 0, 2, 0, 0, 0, 1, 2, 0
            ]
        );
        assert!(simulation.path_build(vec![0, 0, 0, 2, 0, 0], "queue", 2));
        assert_eq!(simulation.world_path_metadata(), vec![0, 1, 1, 2, 1, 2]);
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

        let before_preview = simulation.world_state_hash();
        let preview = simulation.path_preview(vec![0, 0, 0, 3, 0, 1], "normal", 0);
        assert_eq!(preview.len(), 4 * 7);
        assert!(preview.chunks_exact(7).any(|tile| tile[6] != 0));
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
        assert_eq!(preview.len(), 7);
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
