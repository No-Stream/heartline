mod blocks;
mod construction;
mod facilities;
mod guest_benchmark;
mod guests;
mod reports;
mod scenery;
mod world;
pub use reports::piece_shapes_json;

use std::collections::HashMap;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::facilities::{
    FacilityKind, ParkFacilities, PathOpening, coaster_access_openings, coaster_facility,
    facility_access_direction, station_positions, validate_facility_path_access,
};
use theme_park_sim::guest_behavior::Facility;
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::park_guests::ParkGuests;
use theme_park_sim::physics::{
    BlockOperation, Circuit, CoasterOutcome, PhysicsConfig, TIME_STEP, TestRun, TestRunRecorder,
    Train, demo_circuit,
};
use theme_park_sim::track::blocks::{BlockLayout, BlockManualSearch, BlockPlacementSearch};
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

fn facility_intersects_track_sets(
    kind: FacilityKind,
    position: Position,
    direction: Direction,
    demo_pieces: &[Piece],
    demo_train: &TrainConfiguration,
    construction_pieces: &[Piece],
    construction_train: &TrainConfiguration,
) -> bool {
    facility_intersects_track(kind, position, direction, demo_pieces, demo_train)
        || facility_intersects_track(
            kind,
            position,
            direction,
            construction_pieces,
            construction_train,
        )
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
#[path = "tests/construction.rs"]
mod construction_bridge_tests;
#[cfg(test)]
#[path = "tests/facilities.rs"]
mod facility_bridge_tests;
#[cfg(test)]
#[path = "tests/paths.rs"]
mod path_bridge_tests;
#[cfg(test)]
#[path = "tests/piece_shapes.rs"]
mod piece_shape_tests;
#[cfg(test)]
#[path = "tests/scenery.rs"]
mod scenery_bridge_tests;
#[cfg(test)]
#[path = "tests/terrain.rs"]
mod terrain_bridge_tests;
#[cfg(test)]
#[path = "tests/world.rs"]
mod world_tests;
