use std::collections::BTreeMap;

use super::{CoasterId, Park};
use crate::executor::{Executor, SequentialExecutor};
use crate::facilities::{FacilityKind, coaster_facility};
use crate::guests::knowledge::guest_rating;
use crate::guests::walking::WalkingGuest;
use crate::physics::{Circuit, CoasterOutcome, TestRun, Train};
use crate::track::blocks::{BlockLayout, BlockManualSearch};
use crate::track::clearance::ClearanceBox;
use crate::track::construction::track_support_obstacles;
use crate::track::proximity::{OwnedSupportSegment, TrainPosition, scan_proximity_with_supports};
use crate::track::supports::{
    SUPPORT_INFLUENCE_RADIUS_METRES, SupportError, SupportObstacle, SupportSegment,
    generate_dense_wooden_lattice, generate_steel_columns, generate_wooden_lattice,
};
use crate::track::{Piece, SupportStyle, TrainConfiguration, Vec3};
use crate::world::World;

pub const COASTER_PLACE_ID: u32 = u32::MAX;
const METRES_PER_TILE: f64 = 4.0;
const THEMING_RADIUS_TILES: i32 = 3;
const THEMING_WEIGHT_PER_SCENERY: f64 = 2.0;
const THEMING_SCALE_PER_100_METRES: f64 = 12.0;

pub(super) fn track_theming(world: &World, pieces: &[Piece]) -> f64 {
    let track_length: f64 = pieces.iter().map(Piece::length).sum();
    assert!(track_length.is_finite() && track_length > 0.0);

    let scenery: Vec<_> = world.scenery().collect();
    let mut weighted_scenery_samples = 0.0;
    for piece in pieces {
        let mut distance = 0.0;
        while distance < piece.length() {
            let position = piece.sample(distance).position;
            let tile_x = (position.x / METRES_PER_TILE).floor() as i32;
            let tile_y = (position.y / METRES_PER_TILE).floor() as i32;
            let nearby_count = scenery
                .iter()
                .filter(|scenery| {
                    (i32::from(scenery.position.x) - tile_x).abs() <= THEMING_RADIUS_TILES
                        && (i32::from(scenery.position.y) - tile_y).abs() <= THEMING_RADIUS_TILES
                })
                .count();
            weighted_scenery_samples += nearby_count as f64 * THEMING_WEIGHT_PER_SCENERY;
            distance += 1.0;
        }
    }

    let weighted_scenery_per_100_metres = weighted_scenery_samples * 100.0 / track_length;
    1.0 - crate::math::exp(-weighted_scenery_per_100_metres / THEMING_SCALE_PER_100_METRES)
}

/// Adds proximity events and theming to freshly finished reports of one track.
pub(super) fn add_proximity_and_theming(
    world: &World,
    pieces: &[Piece],
    train: &TrainConfiguration,
    other_tracks: &[Piece],
    obstacles: &[ClearanceBox],
    supports: &[OwnedSupportSegment],
    reports: &mut [TestRun],
) {
    let geometry =
        scan_proximity_with_supports(world, pieces, train, other_tracks, obstacles, supports);
    let theming = track_theming(world, pieces);
    for report in reports {
        let events = geometry.events_for_run(&report.samples);
        report.add_proximity_events(&events);
        report.set_theming(theming);
    }
}

pub(super) fn train_positions_for_train(
    circuit: &Circuit,
    train: &Train,
    configuration: &TrainConfiguration,
) -> Vec<TrainPosition> {
    let cars = usize::from(configuration.cars);
    (0..cars)
        .map(|car| {
            let distance = (train.distance()
                - car as f64 * configuration.length_metres / f64::from(configuration.cars))
            .rem_euclid(circuit.length());
            let sample = circuit.sample(distance);
            TrainPosition {
                position: sample.position,
                tangent: sample.tangent,
                piece_index: circuit.piece_index(distance),
                distance,
                speed: train.speed().abs(),
            }
        })
        .collect()
}

pub(super) fn other_coaster_train_positions(
    park: &Park,
    excluded_id: Option<CoasterId>,
) -> Vec<TrainPosition> {
    park.coasters
        .iter()
        .filter(|(id, _)| Some(**id) != excluded_id)
        .flat_map(|(_, coaster)| {
            let mut positions = coaster
                .coaster_operation
                .as_ref()
                .map_or_else(Vec::new, |operation| operation.train_positions());
            if let Some((circuit, train)) = &coaster.construction_train {
                positions.extend(train_positions_for_train(
                    circuit,
                    train,
                    &coaster.construction.model().train,
                ));
            }
            positions
        })
        .collect()
}

pub struct SupportRenderCache {
    obstacles: Vec<SupportObstacle>,
    dependencies: Vec<usize>,
    world_revision: u64,
    segments: Result<Vec<SupportSegment>, SupportError>,
}

impl Park {
    pub(super) fn remember_front_car_ratings(&mut self, id: CoasterId, report: &TestRun) {
        if report.completed
            && let Some(coaster) = self.coaster_mut(id)
        {
            coaster.coaster_ratings =
                Some((guest_rating(report.intensity), guest_rating(report.nausea)));
        }
    }

    pub fn tick(&mut self) {
        self.tick_with(&SequentialExecutor::new(256));
    }

    pub fn tick_with<E: Executor<WalkingGuest>>(&mut self, executor: &E) {
        let population = self.simulation.walking().guests().len();
        self.simulation
            .set_entrance(&self.world, self.facilities.entrance(&self.world));
        self.simulation
            .set_entrance_price(self.facilities.entrance_price());
        let mut facilities = self.facilities.guest_facilities(&self.world);
        let coaster_ids: Vec<_> = self.coasters.keys().copied().collect();
        for id in &coaster_ids {
            let is_open = self
                .coaster(*id)
                .is_some_and(|coaster| coaster.coaster_open);
            if is_open {
                let access = self.coaster(*id).and_then(|coaster| {
                    coaster_facility(
                        &self.world,
                        &coaster.construction,
                        coaster_place_id(*id),
                        coaster.coaster_price,
                        coaster.coaster_ratings,
                    )
                });
                if let Some(facility) = access {
                    self.coaster_mut(*id)
                        .expect("coaster remains present")
                        .coaster_facility = Some(facility);
                    facilities.push(facility);
                } else {
                    self.close_live_coaster(*id);
                }
            }
        }
        let external_rides: Vec<_> = coaster_ids
            .iter()
            .filter_map(|id| {
                self.coaster(*id)
                    .is_some_and(|coaster| coaster.coaster_open)
                    .then_some(coaster_place_id(*id))
            })
            .collect();
        self.simulation.set_external_rides(&external_rides);
        self.simulation
            .tick_with(&self.world, &facilities, 100, Some(population), executor);
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            let coaster_trains: BTreeMap<CoasterId, Vec<TrainPosition>> = self
                .coasters
                .iter()
                .map(|(&id, coaster)| {
                    let mut positions = coaster
                        .coaster_operation
                        .as_ref()
                        .map_or_else(Vec::new, |operation| operation.train_positions());
                    if let Some((circuit, train)) = &coaster.construction_train {
                        positions.extend(train_positions_for_train(
                            circuit,
                            train,
                            &coaster.construction.model().train,
                        ));
                    }
                    (id, positions)
                })
                .collect();
            for id in &coaster_ids {
                let mut other_trains = Vec::new();
                for (&other_id, positions) in &coaster_trains {
                    if other_id != *id {
                        other_trains.extend_from_slice(positions);
                    }
                }
                self.tick_coaster(*id, &other_trains);
            }
            self.physics_remainder -= 30;
        }
    }

    fn tick_coaster(&mut self, id: CoasterId, other_trains: &[TrainPosition]) {
        if self.coaster(id).is_some_and(|coaster| coaster.coaster_open)
            && !self.block_layout_change_pending(id)
        {
            loop {
                let train_id = {
                    let coaster = self.coaster(id).expect("coaster remains present");
                    let operation = coaster
                        .coaster_operation
                        .as_ref()
                        .expect("open coaster has an operation");
                    (operation.trains().count() < self.construction_train_count(id))
                        .then(|| operation.next_dispatch_id())
                        .flatten()
                };
                let Some(train_id) = train_id else {
                    break;
                };
                let riders = {
                    let (coasters, simulation) = (&mut self.coasters, &mut self.simulation);
                    let coaster = coasters.get_mut(&id).expect("coaster remains present");
                    simulation.board_coaster_train(
                        coaster
                            .coaster_facility
                            .as_ref()
                            .expect("open coaster has access"),
                        train_id,
                    )
                };
                if riders.is_empty() {
                    break;
                }
                let coaster = self.coaster_mut(id).expect("coaster remains present");
                assert_eq!(
                    coaster
                        .coaster_operation
                        .as_mut()
                        .expect("open coaster has an operation")
                        .dispatch(),
                    train_id
                );
                coaster.coaster_riders.insert(train_id, riders);
            }
        }

        let outcomes = {
            let coaster = self.coaster_mut(id).expect("coaster remains present");
            let model_key = coaster.construction.model().name;
            coaster
                .coaster_operation
                .as_mut()
                .map(|operation| operation.step(other_trains, model_key))
                .unwrap_or_default()
        };
        for (train_id, outcome) in outcomes {
            self.finish_live_train(id, train_id, outcome);
        }

        let report_finished = {
            let coaster = self.coaster_mut(id).expect("coaster remains present");
            if coaster.construction_outcome.is_none()
                && let Some((circuit, train)) = &mut coaster.construction_train
            {
                let was_recovered = !train.notes().is_empty();
                let previous_distance = train.distance();
                train.step(circuit);
                if !was_recovered && !train.notes().is_empty() {
                    coaster.construction_message =
                        "Train recovered to the station after rollback.".into();
                } else if previous_distance < circuit.length()
                    && train.distance() >= circuit.length()
                {
                    coaster.construction_message = "Train completed a lap.".into();
                }
                if train.speed().abs() < 0.05 {
                    coaster.construction_slow_steps += 1;
                    if coaster.construction_slow_steps >= 200 && train.notes().is_empty() {
                        coaster.construction_message =
                            "Train stopped before completing a lap.".into();
                    }
                } else {
                    coaster.construction_slow_steps = 0;
                }
                if coaster.construction_message == "Train recovered to the station after rollback."
                    || coaster.construction_message == "Train completed a lap."
                    || coaster.construction_message == "Train stopped before completing a lap."
                {
                    coaster.construction_outcome = Some(coaster.construction_message.clone());
                }
                let readings = train.take_readings();
                if let Some(recorder) = &mut coaster.construction_recorder {
                    let model_key = coaster.construction.model().name;
                    recorder.record(&readings, model_key);
                    for frame in &readings {
                        if let Some(reading) = frame.first() {
                            recorder.record_dueling(circuit, *reading, other_trains);
                        }
                    }
                }
                if coaster.construction_outcome.is_some() {
                    let recorder = coaster.construction_recorder.take().expect("test recorder");
                    let (report, front_car_report) = recorder.finish_with_front_car(
                        coaster.construction_message == "Train completed a lap.",
                        !train.notes().is_empty(),
                    );
                    if front_car_report.completed {
                        coaster.coaster_ratings = Some((
                            guest_rating(front_car_report.intensity),
                            guest_rating(front_car_report.nausea),
                        ));
                    }
                    coaster.construction_report = Some(report);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if report_finished {
            if self.refresh_construction_supports(id).is_err() {
                self.construction_message = "Could not route supports for the placed track.".into();
            }
            let obstacles = self.proximity_obstacles();
            let other_tracks = self.other_tracks(id);
            let supports = self.proximity_supports(id);
            let (coasters, world) = (&mut self.coasters, &self.world);
            let coaster = coasters.get_mut(&id).expect("report requires coaster");
            let construction = &coaster.construction;
            if let Some(report) = &mut coaster.construction_report {
                add_proximity_and_theming(
                    world,
                    construction.pieces(),
                    &construction.model().train,
                    &other_tracks,
                    &obstacles,
                    &supports,
                    std::slice::from_mut(report),
                );
            }
        }
    }

    pub fn block_layout_change_pending(&self, id: CoasterId) -> bool {
        let Some(coaster) = self.coaster(id) else {
            return false;
        };
        if let Some(Ok(layout)) = coaster
            .block_manual_search
            .as_ref()
            .and_then(BlockManualSearch::result)
        {
            let installed = coaster
                .block_layout
                .as_ref()
                .expect("validation has an installed layout");
            return block_layout_fingerprint(layout) != block_layout_fingerprint(installed);
        }
        let Some(search) = &coaster.block_placement_search else {
            return false;
        };
        let installed = coaster
            .block_layout
            .as_ref()
            .expect("search has an installed layout");
        let best = search.best();
        (coaster.block_apply_best_requested
            || best.capacity() > installed.capacity()
            || confirmed_stalled_baseline(best, installed)
            || (search.is_complete() && best.capacity() >= installed.capacity()))
            && block_layout_fingerprint(best) != block_layout_fingerprint(installed)
    }

    pub fn proximity_obstacles(&self) -> Vec<ClearanceBox> {
        self.facility_obstacles()
            .into_iter()
            .map(|obstacle| ClearanceBox {
                min: obstacle.min,
                max: obstacle.max,
            })
            .collect()
    }

    pub(super) fn other_tracks(&self, id: CoasterId) -> Vec<Piece> {
        let mut pieces = Vec::new();
        for (&other_id, coaster) in &self.coasters {
            if other_id == id {
                continue;
            }
            pieces.extend_from_slice(coaster.construction.pieces());
            pieces.extend(
                coaster
                    .construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.pieces.iter().cloned()),
            );
        }
        pieces
    }

    pub fn proximity_supports(&self, id: CoasterId) -> Vec<OwnedSupportSegment> {
        let Some(coaster) = self.coaster(id) else {
            return Vec::new();
        };
        let piece_ids = coaster.construction.piece_ids();
        coaster
            .support_render_cache
            .iter()
            .flat_map(|(id, cached)| {
                let piece_index = piece_ids.iter().position(|piece_id| piece_id == id);
                cached
                    .segments
                    .as_ref()
                    .expect("placed support must be routable")
                    .iter()
                    .copied()
                    .map(move |segment| OwnedSupportSegment {
                        piece_index,
                        segment,
                    })
            })
            .collect()
    }

    pub fn close_live_coaster(&mut self, id: CoasterId) {
        self.recover_live_trains(id);
        if let Some(facility) = self
            .coaster(id)
            .and_then(|coaster| coaster.coaster_facility)
        {
            self.simulation.close_coaster(&self.world, &facility);
        }
        if let Some(coaster) = self.coaster_mut(id) {
            coaster.coaster_open = false;
            coaster.coaster_operation = None;
            coaster.coaster_facility = None;
        }
    }

    pub fn recover_live_trains(&mut self, id: CoasterId) {
        let outcomes = self
            .coaster_mut(id)
            .and_then(|coaster| coaster.coaster_operation.take())
            .map_or_else(Vec::new, |mut operation| operation.recover_all());
        for (train_id, outcome) in outcomes {
            self.finish_live_train(id, train_id, outcome);
        }
    }

    fn finish_live_train(&mut self, id: CoasterId, train_id: u64, mut outcome: CoasterOutcome) {
        let Some(riders) = self
            .coaster_mut(id)
            .and_then(|coaster| coaster.coaster_riders.remove(&train_id))
        else {
            return;
        };
        let facility = self
            .coaster(id)
            .and_then(|coaster| coaster.coaster_facility)
            .expect("riders have an open coaster");
        if outcome.recovered {
            self.simulation
                .finish_coaster_train(&self.world, &facility, train_id, &[], &[], true);
            return;
        }

        if self.refresh_construction_supports(id).is_err() {
            self.construction_message = "Could not route supports for the placed track.".into();
        }
        let obstacles = self.proximity_obstacles();
        let other_tracks = self.other_tracks(id);
        let supports = self.proximity_supports(id);
        let construction = &self
            .coaster(id)
            .expect("open coaster has track")
            .construction;
        add_proximity_and_theming(
            &self.world,
            construction.pieces(),
            &construction.model().train,
            &other_tracks,
            &obstacles,
            &supports,
            &mut outcome.reports,
        );
        let seats_per_car = usize::from(construction.model().train.rows_per_car)
            * usize::from(construction.model().train.seats_per_row);
        self.remember_front_car_ratings(id, &outcome.reports[0]);
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

    pub(super) fn facility_obstacles(&self) -> Vec<SupportObstacle> {
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

    pub(super) fn construction_support_obstacles(
        &self,
        excluded_id: CoasterId,
    ) -> Vec<SupportObstacle> {
        let mut obstacles = self.facility_obstacles();
        for (&id, coaster) in &self.coasters {
            if id == excluded_id {
                continue;
            }
            let pieces: Vec<_> = coaster
                .construction
                .pieces()
                .iter()
                .chain(
                    coaster
                        .construction
                        .disconnected_segments()
                        .iter()
                        .flat_map(|segment| &segment.pieces),
                )
                .cloned()
                .collect();
            obstacles.extend(track_support_obstacles(
                &pieces,
                &coaster.construction.model().train,
            ));
        }
        obstacles
    }

    pub fn refresh_construction_supports(
        &mut self,
        id: CoasterId,
    ) -> Result<Vec<f32>, SupportError> {
        let support_obstacles = self.construction_support_obstacles(id);
        let (coasters, world) = (&mut self.coasters, &self.world);
        let Some(coaster) = coasters.get_mut(&id) else {
            return Ok(Vec::new());
        };
        let construction = &coaster.construction;
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
        render_cached_supports(
            &entries,
            world,
            &construction.model().train,
            construction.model().support_style,
            &support_obstacles,
            &mut coaster.support_render_cache,
        )
    }

    pub fn construction_train_count(&self, id: CoasterId) -> usize {
        self.coaster(id).map_or(0, |coaster| {
            coaster.block_layout.as_ref().map_or(0, |layout| {
                coaster
                    .desired_train_count
                    .unwrap_or(1)
                    .min(layout.capacity())
            })
        })
    }

    pub fn clear_support_render_cache(&mut self, id: CoasterId) {
        if let Some(coaster) = self.coaster_mut(id) {
            coaster.support_render_cache.clear();
        }
    }

    pub fn support_render_cache_len(&self, id: CoasterId) -> usize {
        self.coaster(id)
            .map_or(0, |coaster| coaster.support_render_cache.len())
    }
}

pub const fn coaster_place_id(id: CoasterId) -> u32 {
    COASTER_PLACE_ID - id.0
}

pub(super) fn editable_block_boundaries(layout: &BlockLayout) -> Vec<f64> {
    layout
        .boundaries()
        .iter()
        .enumerate()
        .filter(|(index, _)| !layout.brake_pieces(*index).is_empty())
        .map(|(_, boundary)| *boundary)
        .collect()
}

pub fn block_layout_fingerprint(layout: &BlockLayout) -> Vec<u64> {
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

pub fn block_boundary_fingerprint(
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

pub(super) fn confirmed_stalled_baseline(best: &BlockLayout, installed: &BlockLayout) -> bool {
    best.capacity() == 0
        && best.capacity_search_complete()
        && !installed.capacity_search_complete()
        && editable_block_boundaries(installed).is_empty()
}

fn sync_support_obstacles(
    entries: &[(&usize, &Piece)],
    train: &TrainConfiguration,
    cache: &mut BTreeMap<usize, SupportRenderCache>,
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

pub(super) fn render_cached_supports(
    entries: &[(&usize, &Piece)],
    world: &World,
    train: &TrainConfiguration,
    style: SupportStyle,
    facility_obstacles: &[SupportObstacle],
    cache: &mut BTreeMap<usize, SupportRenderCache>,
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

pub(super) fn generate_supports(
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

fn facility_height(kind: FacilityKind) -> f64 {
    match kind {
        FacilityKind::Carousel | FacilityKind::FerrisWheel => 12.0,
        _ => 4.0,
    }
}

fn append_vector(result: &mut Vec<f32>, vector: Vec3) {
    result.extend([vector.x as f32, vector.y as f32, vector.z as f32]);
}

#[cfg(test)]
mod tests {
    use super::{CoasterId, OwnedSupportSegment, Park, SupportRenderCache, SupportSegment, Vec3};
    use crate::facilities::{FacilityKind, ParkFacilities};
    use crate::guests::park::ParkGuests;
    use crate::track::hybrid_model;
    use crate::world::{Direction, Path, PathKind, Position, Surface, World};

    #[test]
    fn park_tick_charges_the_entrance_fee_and_increases_revenue() {
        let mut world = World::new(
            6,
            6,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let entrance_path = Position {
            x: 1,
            y: 0,
            height: 0,
        };
        world.place_path(Path {
            position: entrance_path,
            slope: None,
        });
        let mut facilities = ParkFacilities::new();
        let entrance_id = facilities
            .place(
                &world,
                FacilityKind::Entrance,
                Position {
                    x: 1,
                    y: 1,
                    height: 0,
                },
                Direction::North,
            )
            .unwrap();
        facilities.set_price(entrance_id, 7).unwrap();
        let simulation = ParkGuests::new(42, 1, &world, entrance_path, 1);
        let mut park = Park::new_empty(world, facilities, simulation);

        park.tick();

        assert_eq!(park.simulation.revenue(), 7);
    }

    #[test]
    fn proximity_inputs_keep_routed_supports_as_segments() {
        let mut world = World::new(
            6,
            6,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let entrance = Position {
            x: 0,
            y: 0,
            height: 0,
        };
        world
            .build_path_route(entrance, entrance, PathKind::Normal, 0)
            .unwrap();
        let guests = ParkGuests::new(42, 0, &world, entrance, 1);
        let mut park = Park::new_empty(world, ParkFacilities::new(), guests);
        park.replace_coaster(
            CoasterId(0),
            crate::track::construction::Construction::new(
                crate::track::TrackState {
                    x: 8,
                    y: 8,
                    z: 0,
                    ..crate::track::TrackState::origin()
                },
                hybrid_model(),
            ),
        );
        let segment = SupportSegment {
            start: Vec3::new(0.0, 0.0, 0.0),
            end: Vec3::new(10.0, 10.0, 0.0),
        };
        park.coaster_mut(CoasterId(0))
            .unwrap()
            .support_render_cache
            .insert(
                usize::MAX,
                SupportRenderCache {
                    obstacles: Vec::new(),
                    dependencies: Vec::new(),
                    world_revision: 0,
                    segments: Ok(vec![segment]),
                },
            );
        assert_eq!(
            park.proximity_supports(CoasterId(0)),
            vec![OwnedSupportSegment {
                piece_index: None,
                segment,
            }]
        );
    }
}
