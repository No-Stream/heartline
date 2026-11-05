mod blocks;
mod command_errors;
mod construction;
mod construction_speed_match;
mod facilities;
mod guest_benchmark;
mod guest_report;
mod guests;
pub mod layouts;
mod reports;
mod save;
#[cfg(test)]
mod save_tests;
mod scenery;
mod world;
pub use reports::piece_shapes_json;

use command_errors::CommandError;

#[cfg(test)]
use std::collections::HashMap;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::facilities::{
    FacilityKind, ParkFacilities, facility_access_direction, station_positions,
};
use theme_park_sim::guests::{AgeGroup, GuestId};
use theme_park_sim::park::{DemoCoaster, Park};
use theme_park_sim::park_guests::ParkGuests;
#[cfg(test)]
use theme_park_sim::physics::BlockOperation;
use theme_park_sim::physics::{Circuit, PhysicsConfig, TestRunRecorder, Train, demo_circuit};
#[cfg(test)]
use theme_park_sim::track::blocks::ThroughputEstimator;
use theme_park_sim::track::blocks::{BlockLayout, BlockManualSearch, BlockPlacementSearch};
#[cfg(test)]
use theme_park_sim::track::clearance::ClearanceBox;
#[cfg(test)]
use theme_park_sim::track::construction::PieceChoice;
use theme_park_sim::track::construction::{
    Completion, CompletionSearch, CompletionSearchProgress, Construction, ElementChoice,
    track_support_obstacles,
};
use theme_park_sim::track::proximity::ProximityKind;
#[cfg(test)]
use theme_park_sim::track::supports::SUPPORT_INFLUENCE_RADIUS_METRES;
use theme_park_sim::track::supports::{
    SupportError, SupportObstacle, SupportSegment, generate_dense_wooden_lattice,
    generate_steel_columns, generate_wooden_lattice,
};
use theme_park_sim::track::{
    CoasterColors, CoasterModel, Heading, Material, Piece, PieceId, PieceKind, Slope, SupportStyle,
    TrackSample, TrackState, TrainConfiguration, Vec3, catalog, hybrid_model, multi_launch_model,
    wooden_twister_model,
};
use theme_park_sim::world::{
    Brush, Corner, Direction, PathBuildError, PathKind, Position, Scenery, SceneryKind, Surface,
    TerrainCommand, World,
};
use wasm_bindgen::prelude::*;

#[cfg(test)]
struct SupportRenderCache {
    obstacles: Vec<SupportObstacle>,
    dependencies: Vec<usize>,
    world_revision: u64,
    segments: Result<Vec<SupportSegment>, SupportError>,
}

#[wasm_bindgen]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WasmSimulation {
    park: Park,
    facility_message: String,
    path_message: String,
    terrain_message: String,
    scenery_message: String,
    terrain_invalidated_paths: Vec<Position>,
    construction_model: ConstructionModel,
    selected: Option<PieceId>,
    selected_element: Option<String>,
    construction_undo_groups: Vec<usize>,
    construction_completion: Option<Completion>,
    construction_completion_available: Option<Completion>,
    construction_completion_search: Option<CompletionSearch>,
    construction_completion_checked: bool,
    construction_completion_unavailable_reason: Option<String>,
    #[serde(skip)]
    construction_preview_supports_cache: Vec<f32>,
    #[serde(skip)]
    construction_placed_supports_cache: Vec<f32>,
    recovered_station: Option<Piece>,
}

impl std::ops::Deref for WasmSimulation {
    type Target = Park;

    fn deref(&self) -> &Park {
        &self.park
    }
}

impl std::ops::DerefMut for WasmSimulation {
    fn deref_mut(&mut self) -> &mut Park {
        &mut self.park
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
enum ConstructionModel {
    Hybrid,
    MultiLaunch,
    WoodenTwister,
}

fn same_track_orientation(first: TrackState, second: TrackState) -> bool {
    first.heading == second.heading
        && first.slope == second.slope
        && first.bank == second.bank
        && first.horizontal_curvature == second.horizontal_curvature
        && first.vertical_curvature == second.vertical_curvature
}

#[cfg(test)]
fn default_construction_piece_from_choices(
    available: &[(Piece, PieceChoice)],
    connector: TrackState,
) -> Option<PieceId> {
    let mut first_valid = None;
    for (piece, choice) in available {
        if choice.result.is_err() {
            continue;
        }
        first_valid.get_or_insert(piece.id);
        if piece.inversion.is_none()
            && matches!(piece.kind, PieceKind::Track | PieceKind::Lift)
            && same_track_orientation(piece.entry, connector)
            && same_track_orientation(piece.exit, connector)
        {
            return Some(piece.id);
        }
    }
    first_valid
}

#[cfg(test)]
use theme_park_sim::park::block_analysis::block_physics_config;
#[cfg(test)]
use theme_park_sim::park::tick::{
    COASTER_PLACE_ID, block_boundary_fingerprint, block_layout_fingerprint,
};

impl WasmSimulation {
    pub(crate) fn demo_coaster(&self) -> Option<&DemoCoaster> {
        self.park.demo_coaster.as_ref()
    }

    fn block_reasons(&self) -> Vec<String> {
        [&self.block_message, &self.block_throughput_reason]
            .into_iter()
            .filter(|reason| !reason.is_empty())
            .cloned()
            .collect()
    }

    fn facility_obstacles(&self) -> Vec<SupportObstacle> {
        let mut obstacles: Vec<_> = self
            .park
            .facilities
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
            .collect();
        if let Some(coaster) = &self.park.demo_coaster {
            obstacles.extend(track_support_obstacles(
                coaster.circuit.pieces(),
                &coaster.train_configuration,
            ));
        }
        obstacles
    }
    fn select_compatible_piece(&mut self) {
        self.selected_element = None;
        let Some(construction) = &self.park.construction else {
            return;
        };
        let obstacles = self.facility_obstacles();
        if self
            .selected
            .is_some_and(|selected| construction.candidate_piece(selected).is_some())
        {
            return;
        }
        let connector = construction.end();
        self.selected = construction.first_valid_piece(&self.park.world, &obstacles, |piece| {
            piece.inversion.is_none()
                && matches!(piece.kind, PieceKind::Track | PieceKind::Lift)
                && same_track_orientation(piece.entry, connector)
                && same_track_orientation(piece.exit, connector)
        });
    }
    fn construction_render(
        &self,
        render: fn(&[Piece], &World) -> Vec<f32>,
        stride: usize,
    ) -> Vec<f32> {
        let Some(construction) = &self.park.construction else {
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
        let mut geometry = render(&pieces, &self.park.world);
        for record in geometry.chunks_exact_mut(stride) {
            record[0] = *entries[record[0] as usize].0 as f32;
        }
        self.encode_construction_geometry(geometry, stride)
    }
    fn encode_construction_geometry(&self, mut geometry: Vec<f32>, stride: usize) -> Vec<f32> {
        if self.park.construction.as_ref().is_some_and(|construction| {
            construction.model().support_style == SupportStyle::SteelColumns
        }) {
            // Steel records use -(piece ID + 1); consumers decode before picking.
            for record in geometry.chunks_exact_mut(stride) {
                record[0] = -record[0] - 1.0;
            }
        } else if stride != layouts::SUPPORT.len()
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
        if let Some(completion) = &self.construction_completion {
            return completion.geometry.clone();
        }
        let Some(construction) = &self.park.construction else {
            return Vec::new();
        };
        if construction.is_closed() {
            return Vec::new();
        }
        let obstacles = self.facility_obstacles();
        if let Some(element_id) = self.selected_element.as_deref() {
            return construction
                .element_choices(&self.park.world, &obstacles)
                .into_iter()
                .find(|element| element.id == element_id)
                .map_or_else(Vec::new, |element| element.geometry);
        }
        construction
            .available_choices(&self.park.world, &obstacles)
            .into_iter()
            .filter_map(|(piece, _)| (Some(piece.id) == self.selected).then_some(piece))
            .collect()
    }
}

fn piece_group(piece: &Piece) -> &'static str {
    if piece.inversion.is_some() {
        "inversion"
    } else if piece.entry.heading != piece.exit.heading {
        "turn"
    } else if piece.entry.slope != Slope::Level || piece.exit.slope != Slope::Level {
        "slope"
    } else if piece.entry.bank != piece.exit.bank {
        "bank"
    } else if matches!(
        piece.kind,
        PieceKind::Station | PieceKind::Brakes | PieceKind::Launch
    ) || piece.entry.z != piece.exit.z
    {
        "special"
    } else {
        "straight"
    }
}

fn parse_path_kind(kind: &str) -> Option<PathKind> {
    match kind {
        "normal" => Some(PathKind::Normal),
        "queue" => Some(PathKind::Queue),
        _ => None,
    }
}

fn path_positions(coordinates: &[i32]) -> Result<(Position, Position), CommandError> {
    let [start_x, start_y, start_height, end_x, end_y, end_height] = coordinates else {
        return Err(CommandError::PathRouteNeedsSixCoordinates);
    };
    let position = |x: i32, y: i32, height: i32| -> Result<Position, CommandError> {
        Ok(Position {
            x: u16::try_from(x).map_err(|_| CommandError::PathCoordinatesOutsideSupportedRange)?,
            y: u16::try_from(y).map_err(|_| CommandError::PathCoordinatesOutsideSupportedRange)?,
            height: i16::try_from(height)
                .map_err(|_| CommandError::PathCoordinatesOutsideSupportedRange)?,
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

fn parse_terrain_parameters(values: Vec<i32>) -> Result<TerrainParameters, CommandError> {
    let [x, y, size, corner, steps, height, bottom, top] = values
        .try_into()
        .map_err(|_| CommandError::TerrainNeedsEightParameters)?;
    Ok(TerrainParameters {
        x: u16::try_from(x).map_err(|_| CommandError::TerrainCoordinatesOutsideSupportedRange)?,
        y: u16::try_from(y).map_err(|_| CommandError::TerrainCoordinatesOutsideSupportedRange)?,
        size: u8::try_from(size)
            .map_err(|_| CommandError::TerrainBrushSizeOutsideSupportedRange)?,
        corner: u8::try_from(corner)
            .map_err(|_| CommandError::TerrainCornerOutsideSupportedRange)?,
        steps: i16::try_from(steps).map_err(|_| CommandError::TerrainStepsOutsideSupportedRange)?,
        height: i16::try_from(height)
            .map_err(|_| CommandError::TerrainHeightOutsideSupportedRange)?,
        bottom: i16::try_from(bottom)
            .map_err(|_| CommandError::ExcavationBottomOutsideSupportedRange)?,
        top: i16::try_from(top).map_err(|_| CommandError::ExcavationTopOutsideSupportedRange)?,
    })
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

fn path_preview_error_code(error: CommandError) -> i32 {
    match error {
        CommandError::Park(theme_park_sim::park::commands::ParkCommandError::Path(error)) => {
            path_error_code(error)
        }
        CommandError::UnknownPathKind => 10,
        CommandError::PathRouteNeedsSixCoordinates => 11,
        CommandError::PathCoordinatesOutsideSupportedRange => 12,
        _ => unreachable!("only path preview errors can be encoded in a path preview"),
    }
}

fn path_preview_error_record(error: CommandError) -> Vec<i32> {
    vec![-1, -1, 0, 0, 0, 0, path_preview_error_code(error), 0, 0]
}

fn construction_error(error: theme_park_sim::track::construction::ConstructionError) -> String {
    use theme_park_sim::track::clearance::ClearanceCollision;
    use theme_park_sim::track::construction::ConstructionError;
    match error {
        ConstructionError::Closed => "Delete track before editing a closed circuit.",
        ConstructionError::Unavailable => "This piece cannot connect here.",
        ConstructionError::AmbiguousConnection => "More than one track segment connects here.",
        ConstructionError::ModelForbidden => "This coaster model cannot use this piece.",
        ConstructionError::ChainNotAllowed => "No chain lift on this piece",
        ConstructionError::NoSupportRoute => "Supports cannot be placed here.",
        ConstructionError::InvalidIndex => "That track piece does not exist.",
        ConstructionError::InvalidLaunchIntensity => {
            "Launch speed must be within the limits for this launch section."
        }
        ConstructionError::NotClosed => "Close the circuit first.",
        ConstructionError::InvalidCircuit => "The track must connect back to its station.",
        ConstructionError::Clearance(ClearanceCollision::Terrain) => {
            "The train would collide with terrain."
        }
        ConstructionError::Clearance(ClearanceCollision::TerrainDepth) => {
            "The track would go below the minimum terrain depth."
        }
        ConstructionError::Clearance(ClearanceCollision::Water) => {
            "Track cannot tunnel through water."
        }
        ConstructionError::Clearance(ClearanceCollision::Path) => {
            "The train would collide with a path."
        }
        ConstructionError::Clearance(ClearanceCollision::Scenery) => {
            "The train would collide with scenery, a facility, or another ride."
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

#[cfg(test)]
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

#[cfg(test)]
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

/// The renderer's key for a coaster model's trains; the same keys name the models in `construction_select`.
#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
enum TrainModelKey {
    Hybrid,
    VelocityViper,
    WoodenTwister,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct TrainLayoutReport {
    model: TrainModelKey,
    cars_per_train: u8,
    rows_per_car: u8,
    seats_per_row: u8,
    length_metres: f64,
}

fn train_model_key(model: &CoasterModel) -> TrainModelKey {
    [
        (TrainModelKey::Hybrid, hybrid_model()),
        (TrainModelKey::VelocityViper, multi_launch_model()),
        (TrainModelKey::WoodenTwister, wooden_twister_model()),
    ]
    .into_iter()
    .find_map(|(key, known)| (known.name == model.name).then_some(key))
    .unwrap_or_else(|| panic!("no train look for coaster model {}", model.name))
}

fn train_layout(model: TrainModelKey, train: &TrainConfiguration) -> TrainLayoutReport {
    TrainLayoutReport {
        model,
        cars_per_train: train.cars,
        rows_per_car: train.rows_per_car,
        seats_per_row: train.seats_per_row,
        length_metres: train.length_metres,
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

fn append_train_state(
    result: &mut Vec<f32>,
    circuit: &Circuit,
    train: &Train,
    train_id: u64,
    occupied: bool,
) {
    let kind = match circuit.pieces()[circuit.piece_index(train.distance())].kind {
        PieceKind::Track => 0.0,
        PieceKind::Lift => 1.0,
        PieceKind::Launch => 2.0,
        PieceKind::Brakes => 3.0,
        PieceKind::Station => 4.0,
    };
    let forces = train.lead_car_forces(circuit);
    result.extend_from_slice(&[
        train_id as f32,
        train.speed().abs() as f32,
        kind,
        forces.vertical as f32,
        forces.lateral as f32,
        f32::from(occupied),
    ]);
}

#[cfg(test)]
fn block_layout_for_open(
    circuit: &Circuit,
    train: &TrainConfiguration,
    config: PhysicsConfig,
) -> Result<BlockLayout, &'static str> {
    BlockLayout::unverified_baseline(circuit, train, config)
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
            let start = piece.sample(piece.length() * (sample_index as f64 / intervals as f64));
            let end = piece.sample(piece.length() * ((sample_index + 1) as f64 / intervals as f64));
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

#[cfg(test)]
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

#[cfg(test)]
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
