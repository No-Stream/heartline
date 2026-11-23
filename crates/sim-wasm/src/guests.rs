use super::*;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoasterRenderEntry {
    id: u32,
    track_revision: String,
    colors: serde_json::Value,
    train_layout: serde_json::Value,
}

fn track_revision(track_hash: u64, world_revision: u64, facility_hash: u64) -> String {
    format!("{track_hash:016x}:{world_revision}:{facility_hash:016x}")
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn test_begin_queue_entry_capture(&mut self) {
        self.park.simulation.test_begin_queue_entry_capture();
    }

    pub fn test_take_queue_entry_places_json(&mut self) -> String {
        serde_json::to_string(&self.park.simulation.test_take_queue_entry_places())
            .expect("test queue entry places serialize")
    }

    pub fn tick(&mut self) {
        self.park.tick();
    }

    pub fn park_outcomes_json(&self) -> String {
        serde_json::to_string(&self.park_outcomes()).expect("park outcomes serialize")
    }

    pub fn coaster_render_list_json(&self) -> String {
        let mut entries = Vec::with_capacity(self.park.coasters.len());
        for (id, coaster) in self.park.coasters() {
            let model = coaster.construction.model();
            entries.push(CoasterRenderEntry {
                id: id.0,
                track_revision: track_revision(
                    coaster.construction.state_hash(),
                    self.world.revision(),
                    self.park.facilities.state_hash(),
                ),
                colors: crate::construction::colors_value(Some(coaster.construction.colors())),
                train_layout: crate::construction::train_layout_value(model, &model.train),
            });
        }
        serde_json::to_string(&entries).expect("coaster render list serializes")
    }

    pub fn coaster_ids(&self) -> Vec<u32> {
        self.park.coasters().map(|(id, _)| id.0).collect()
    }

    pub fn editing_coaster_id_value(&self) -> Option<u32> {
        self.editing_coaster_id().map(|id| id.0)
    }

    /// Samples: piece index, position, lateral, up, cumulative distance. Positions and
    /// distance are in tiles (four metres); Z is up and vectors are unit length.
    pub fn coaster_geometry(&self, id: u32) -> Vec<f32> {
        let Some(pieces) = self.coaster_track_pieces(id) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut start = 0.0;
        for (index, piece) in pieces.iter().enumerate() {
            let intervals = (piece.length() / 2.0).ceil() as usize;
            for sample_index in 0..=intervals {
                let distance = piece.length() * (sample_index as f64 / intervals as f64);
                append_pose(&mut result, index, piece.sample(distance));
                result.push(((start + distance) / 4.0) as f32);
            }
            start += piece.length();
        }
        result
    }

    /// Rail segments: piece index and start/end frames (position, lateral, up).
    /// Positions are in tiles; lateral and up are unit vectors.
    pub fn coaster_rails(&self, id: u32) -> Vec<f32> {
        self.coaster_track_pieces(id)
            .map_or_else(Vec::new, |pieces| render_rails(&pieces, &self.world))
    }

    /// Cross ties: piece index, endpoints, up, tangent. Positions are in tiles.
    /// Ties are spaced two metres apart over the complete track, including joins.
    pub fn coaster_ties(&self, id: u32) -> Vec<f32> {
        self.coaster_track_pieces(id)
            .map_or_else(Vec::new, |pieces| render_ties(&pieces, &self.world))
    }

    /// Support segments: piece index, start position and end position, in tiles.
    pub fn coaster_supports(&self, id: u32) -> Vec<f32> {
        let Some(coaster) = self.park.coaster(CoasterId(id)) else {
            return Vec::new();
        };
        let pieces = construction_pieces(&coaster.construction);
        render_construction_supports(
            &pieces,
            &self.world,
            coaster.construction.model(),
            &self.facility_obstacles_excluding(Some(CoasterId(id))),
        )
        .expect("placed coaster supports must be routable")
    }

    /// Per-train speed, track kind, rider forces, and occupancy.
    pub fn coaster_train_states(&self, id: u32) -> Vec<f32> {
        let Some(coaster) = self.park.coaster(CoasterId(id)) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut has_train = false;
        if let Some(operation) = &coaster.coaster_operation {
            let circuit = coaster
                .block_layout
                .as_ref()
                .expect("operation has a block layout")
                .circuit();
            let mut trains: Vec<_> = operation.trains().collect();
            trains.sort_by_key(|(train_id, _)| *train_id);
            for (train_id, train) in trains {
                has_train = true;
                let occupied = coaster
                    .coaster_riders
                    .get(&train_id)
                    .is_some_and(|riders| !riders.is_empty());
                append_train_state(&mut result, circuit, train, train_id, occupied);
            }
        } else if let Some((circuit, train)) = &coaster.construction_train {
            has_train = true;
            append_train_state(&mut result, circuit, train, 0, false);
            if self.editing_coaster_id() == Some(CoasterId(id))
                && let Some(station) = &self.recovered_station
            {
                let sample = station.sample(station.length());
                result[2] = 4.0;
                result[3] = sample.up.z as f32;
                result[4] = sample.up.cross(sample.tangent).z as f32;
            }
        }
        if !has_train && let Some((circuit, train)) = parked_train(coaster) {
            append_train_state(&mut result, &circuit, &train, 0, false);
        }
        result
    }

    /// Cars: car index, position, tangent, up, occupied-seat bitmask.
    pub fn coaster_train_poses(&self, id: u32) -> Vec<f32> {
        let Some(coaster) = self.park.coaster(CoasterId(id)) else {
            return Vec::new();
        };
        let configuration = &coaster.construction.model().train;
        let mut result = Vec::new();
        if let Some(operation) = &coaster.coaster_operation {
            let circuit = coaster
                .block_layout
                .as_ref()
                .expect("operation has a block layout")
                .circuit();
            let mut trains: Vec<_> = operation.trains().collect();
            trains.sort_by_key(|(train_id, _)| *train_id);
            for (train_id, train) in &trains {
                let riders = coaster
                    .coaster_riders
                    .get(train_id)
                    .map_or(&[][..], Vec::as_slice);
                append_train_poses(
                    &mut result,
                    circuit,
                    train,
                    configuration,
                    *train_id,
                    riders,
                    None,
                );
            }
            if !trains.is_empty() {
                return result;
            }
        }
        if let Some((circuit, train)) = &coaster.construction_train {
            let recovered_station = (self.editing_coaster_id() == Some(CoasterId(id)))
                .then_some(self.recovered_station.as_ref())
                .flatten();
            append_train_poses(
                &mut result,
                circuit,
                train,
                configuration,
                0,
                &[],
                recovered_station,
            );
        } else if let Some((circuit, train)) = parked_train(coaster) {
            append_train_poses(&mut result, &circuit, &train, configuration, 0, &[], None);
        }
        result
    }

    /// Car index, seat index, and guest ID for each occupied seat on this coaster.
    pub fn coaster_riders(&self, id: u32) -> Vec<u32> {
        let Some(coaster) = self.park.coaster(CoasterId(id)) else {
            return Vec::new();
        };
        let seats_per_car = u32::from(coaster.construction.model().train.rows_per_car)
            * u32::from(coaster.construction.model().train.seats_per_row);
        coaster
            .coaster_riders
            .iter()
            .flat_map(|(train_id, guests)| {
                guests.iter().enumerate().flat_map(move |(seat, guest)| {
                    let car_index = rider_car_index(
                        *train_id,
                        coaster.construction.model().train.cars,
                        seat / seats_per_car as usize,
                    );
                    [car_index, (seat as u32) % seats_per_car, guest.0]
                })
            })
            .collect()
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
        let detail = self
            .simulation
            .guest_detail(GuestId(id))
            .map(|detail| crate::guest_report::GuestDetailReport::from(&detail));
        serde_json::to_string(&detail).expect("guest detail serializes")
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
}

fn construction_pieces(construction: &Construction) -> Vec<Piece> {
    construction
        .pieces()
        .iter()
        .chain(
            construction
                .disconnected_segments()
                .iter()
                .flat_map(|segment| &segment.pieces),
        )
        .cloned()
        .collect()
}

impl WasmSimulation {
    fn coaster_track_pieces(&self, id: u32) -> Option<Vec<Piece>> {
        self.park
            .coaster(CoasterId(id))
            .map(|coaster| construction_pieces(&coaster.construction))
    }
}

fn parked_train(coaster: &Coaster) -> Option<(Circuit, Train)> {
    if coaster.coaster_open
        || !coaster.construction.is_closed()
        || coaster.construction_train.is_some()
    {
        return None;
    }
    let circuit = coaster.construction.circuit().ok()?;
    let train = Train::new(
        &circuit,
        &coaster.construction.model().train,
        PhysicsConfig::default(),
        0.0,
        0.0,
    );
    Some((circuit, train))
}

fn render_construction_supports(
    pieces: &[Piece],
    world: &World,
    model: &CoasterModel,
    external_obstacles: &[SupportObstacle],
) -> Result<Vec<f32>, SupportError> {
    let mut result = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let mut obstacles = external_obstacles.to_vec();
        obstacles.extend(
            pieces
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != index)
                .flat_map(|(_, other)| {
                    track_support_obstacles(std::slice::from_ref(other), &model.train)
                }),
        );
        for support in generate_supports(
            piece,
            world,
            &obstacles,
            model.support_style_for_piece(piece),
        )? {
            if support.start != support.end {
                result.push(index as f32);
                append_vector(&mut result, support.start / 4.0);
                append_vector(&mut result, support.end / 4.0);
            }
        }
    }
    Ok(result)
}

impl WasmSimulation {
    pub fn park_outcomes(&self) -> theme_park_sim::guests::ParkOutcomes {
        let mut facilities = self.facilities.guest_facilities(&self.world);
        if let Some(coaster) = self
            .editing_coaster()
            .filter(|coaster| coaster.coaster_open)
        {
            if let Some(coaster) = coaster.coaster_facility {
                facilities.push(coaster);
            }
        }
        self.simulation.outcomes(&self.world, &facilities)
    }
}

#[cfg(test)]
#[path = "tests/coaster_rendering.rs"]
mod rendering_tests;
