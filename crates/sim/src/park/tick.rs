use std::collections::BTreeMap;

use super::Park;
use crate::executor::{Executor, SequentialExecutor};
use crate::facilities::{FacilityKind, coaster_facility};
use crate::guests::knowledge::guest_rating;
use crate::guests::walking::WalkingGuest;
use crate::physics::{CoasterOutcome, TestRun};
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
    1.0 - (-weighted_scenery_per_100_metres / THEMING_SCALE_PER_100_METRES).exp()
}

pub struct SupportRenderCache {
    obstacles: Vec<SupportObstacle>,
    dependencies: Vec<usize>,
    world_revision: u64,
    segments: Result<Vec<SupportSegment>, SupportError>,
}

impl Park {
    pub(super) fn remember_front_car_ratings(&mut self, report: &TestRun) {
        if report.completed {
            self.coaster_ratings =
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
        if self.coaster_open {
            let access = self.construction.as_ref().and_then(|construction| {
                coaster_facility(
                    &self.world,
                    construction,
                    COASTER_PLACE_ID,
                    self.coaster_price,
                    self.coaster_ratings,
                )
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
            .tick_with(&self.world, &facilities, 100, Some(population), executor);
        self.physics_remainder += 200;
        while self.physics_remainder >= 30 {
            let mut report_finished = false;
            let demo_trains: Vec<_> = if let Some(coaster) = &mut self.demo_coaster {
                coaster.train.step(&coaster.circuit);
                let positions = coaster
                    .train
                    .readings()
                    .last()
                    .expect("step produces readings")
                    .iter()
                    .map(|reading| {
                        let sample = coaster.circuit.sample(reading.distance);
                        TrainPosition {
                            position: sample.position,
                            tangent: sample.tangent,
                            piece_index: coaster.circuit.piece_index(reading.distance),
                            distance: reading.distance.rem_euclid(coaster.circuit.length()),
                            speed: reading.speed,
                        }
                    })
                    .collect();
                coaster.train.take_readings();
                positions
            } else {
                Vec::new()
            };
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
                .map(|operation| {
                    let model_key = self
                        .construction
                        .as_ref()
                        .expect("coaster has construction")
                        .model()
                        .name;
                    operation.step(&demo_trains, model_key)
                })
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
                        let model_key = self
                            .construction
                            .as_ref()
                            .expect("test run has construction")
                            .model()
                            .name;
                        recorder.record(&readings, model_key);
                        for frame in &readings {
                            if let Some(reading) = frame.first() {
                                recorder.record_dueling(circuit, *reading, &demo_trains);
                            }
                        }
                    }
                    if self.construction_outcome.is_some() {
                        let recorder = self.construction_recorder.take().expect("test recorder");
                        let (report, front_car_report) = recorder.finish_with_front_car(
                            self.construction_message == "Train completed a lap.",
                            !train.notes().is_empty(),
                        );
                        self.remember_front_car_ratings(&front_car_report);
                        self.construction_report = Some(report);
                        report_finished = true;
                    }
                }
            }
            if report_finished {
                if self.refresh_construction_supports().is_err() {
                    self.construction_message =
                        "Could not route supports for the placed track.".into();
                }
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
                        self.demo_coaster
                            .as_ref()
                            .map_or(&[], |coaster| coaster.circuit.pieces()),
                        &obstacles,
                        &supports,
                    );
                    let events = geometry.events_for_run(&report.samples);
                    report.add_proximity_events(&events);
                    report.set_theming(track_theming(&self.world, construction.pieces()));
                }
            }
            self.physics_remainder -= 30;
        }
    }

    pub fn block_layout_change_pending(&self) -> bool {
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

    pub fn proximity_obstacles(&self) -> Vec<ClearanceBox> {
        self.facility_obstacles()
            .into_iter()
            .map(|obstacle| ClearanceBox {
                min: obstacle.min,
                max: obstacle.max,
            })
            .collect()
    }

    pub fn proximity_supports(&self) -> Vec<OwnedSupportSegment> {
        let piece_ids = self
            .construction
            .as_ref()
            .map_or(&[][..], |construction| construction.piece_ids());
        self.support_render_cache
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

    pub fn close_live_coaster(&mut self) {
        self.recover_live_trains();
        if let Some(facility) = &self.coaster_facility {
            self.simulation.close_coaster(&self.world, facility);
        }
        self.coaster_open = false;
        self.coaster_operation = None;
        self.coaster_facility = None;
    }

    pub fn recover_live_trains(&mut self) {
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

        if self.refresh_construction_supports().is_err() {
            self.construction_message = "Could not route supports for the placed track.".into();
        }
        let obstacles = self.proximity_obstacles();
        let supports = self.proximity_supports();
        let construction = self.construction.as_ref().expect("open coaster has track");
        let geometry = scan_proximity_with_supports(
            &self.world,
            construction.pieces(),
            &construction.model().train,
            self.demo_coaster
                .as_ref()
                .map_or(&[], |coaster| coaster.circuit.pieces()),
            &obstacles,
            &supports,
        );
        let theming = track_theming(&self.world, construction.pieces());
        for report in &mut outcome.reports {
            let events = geometry.events_for_run(&report.samples);
            report.add_proximity_events(&events);
            report.set_theming(theming);
        }
        let seats_per_car = usize::from(construction.model().train.rows_per_car)
            * usize::from(construction.model().train.seats_per_row);
        self.remember_front_car_ratings(&outcome.reports[0]);
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
    pub fn refresh_construction_supports(&mut self) -> Result<Vec<f32>, SupportError> {
        let facility_obstacles = self.facility_obstacles();
        let Some(construction) = &self.construction else {
            return Ok(Vec::new());
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
        render_cached_supports(
            &entries,
            &self.world,
            &construction.model().train,
            construction.model().support_style,
            &facility_obstacles,
            &mut self.support_render_cache,
        )
    }

    pub fn construction_train_count(&self) -> usize {
        self.block_layout.as_ref().map_or(0, |layout| {
            self.desired_train_count.unwrap_or(1).min(layout.capacity())
        })
    }
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
    use super::{OwnedSupportSegment, Park, SupportRenderCache, SupportSegment, Vec3};
    use crate::facilities::{FacilityKind, ParkFacilities};
    use crate::guests::park::ParkGuests;
    use crate::physics::{PhysicsConfig, Train, demo_circuit};
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
        let circuit = demo_circuit();
        let configuration = hybrid_model().train;
        let train = Train::new(&circuit, &configuration, PhysicsConfig::default(), 0.0, 0.0);
        let mut park = Park::new(world, facilities, simulation, circuit, train, configuration);

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
        let circuit = demo_circuit();
        let configuration = hybrid_model().train;
        let train = Train::new(&circuit, &configuration, PhysicsConfig::default(), 0.0, 0.0);
        let mut park = Park::new(
            world,
            ParkFacilities::new(),
            guests,
            circuit,
            train,
            configuration,
        );
        let segment = SupportSegment {
            start: Vec3::new(0.0, 0.0, 0.0),
            end: Vec3::new(10.0, 10.0, 0.0),
        };
        park.support_render_cache.insert(
            1,
            SupportRenderCache {
                obstacles: Vec::new(),
                dependencies: Vec::new(),
                world_revision: 0,
                segments: Ok(vec![segment]),
            },
        );
        assert_eq!(
            park.proximity_supports(),
            vec![OwnedSupportSegment {
                piece_index: None,
                segment,
            }]
        );
    }
}
