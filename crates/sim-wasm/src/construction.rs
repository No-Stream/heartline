use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_start(&mut self, x: i32, y: i32, z: i32, heading: u8) {
        self.construction_finish_invalidate();
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
        self.park.support_render_cache.clear();
        self.construction_outcome = None;
        self.construction_recorder = None;
        self.construction_report = None;
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
    }

    pub fn construction_active(&self) -> bool {
        self.construction.is_some()
    }

    pub fn construction_chain(&self) -> bool {
        self.construction.as_ref().is_some_and(Construction::chain)
    }

    pub fn construction_set_chain(&mut self, enabled: bool) {
        self.construction_finish_invalidate();
        if let Some(construction) = self.construction.as_mut() {
            construction.set_chain(enabled);
            self.construction_message.clear();
            self.refresh_construction_support_rendering();
        }
    }

    /// The coaster's track, supports and train colours as palette names separated by tabs; empty without a coaster.
    pub fn construction_colors(&self) -> String {
        self.construction
            .as_ref()
            .map_or_else(String::new, |construction| construction.colors().names())
    }

    /// Repaints the coaster. Names outside the palette leave it unchanged and explain why in the construction message.
    pub fn construction_set_colors(&mut self, track: &str, supports: &str, trains: &str) {
        let Some(colors) = CoasterColors::from_names(track, supports, trains) else {
            self.construction_message = "Unknown coaster colour".into();
            return;
        };
        if let Some(construction) = self.construction.as_mut() {
            construction.set_colors(colors);
            self.construction_message.clear();
        }
    }

    pub fn construction_select(&mut self, id: &str) {
        self.construction_finish_invalidate();
        match id {
            "model:hybrid" => {
                self.construction_model = ConstructionModel::Hybrid;
                self.construction_message.clear();
                self.refresh_construction_support_rendering();
                return;
            }
            "model:velocity_viper" => {
                self.construction_model = ConstructionModel::MultiLaunch;
                self.construction_message.clear();
                self.refresh_construction_support_rendering();
                return;
            }
            "model:wooden_twister" => {
                self.construction_model = ConstructionModel::WoodenTwister;
                self.construction_message.clear();
                self.refresh_construction_support_rendering();
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
        if self.selected.is_some() {
            self.refresh_construction_support_rendering();
        } else {
            self.construction_preview_supports_cache.clear();
        }
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
                    piece_group(&piece),
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
            .map(|(id, piece)| {
                format!(
                    "{id}\t{:?}\t{}\t{}\t{:?}",
                    piece.id,
                    piece.name,
                    piece_group(piece),
                    piece.exit.slope
                )
            })
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
        self.construction_completion.is_some() || self.construction_can_append()
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
        self.park.construction_can_test()
    }

    pub fn construction_is_open(&self) -> bool {
        self.coaster_open
    }

    pub fn construction_coaster_price(&self) -> u16 {
        self.coaster_price
    }

    pub fn construction_coaster_income(&self) -> u64 {
        self.coaster_income()
    }

    pub fn construction_set_coaster_price(&mut self, price: u16) {
        self.set_coaster_price(price);
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
        self.park.construction_train_count()
    }

    pub fn construction_set_train_count(&mut self, count: usize) -> bool {
        self.park.construction_set_train_count(count).is_ok()
    }

    pub fn construction_set_open(&mut self, open: bool) -> bool {
        match self.park.construction_set_open(open) {
            Ok(()) => true,
            Err(error) => {
                if matches!(
                    error,
                    theme_park_sim::park::block_analysis::BlockCommandError::Validation(_)
                ) {
                    self.record_block_error(error);
                } else {
                    self.park.construction_message =
                        crate::command_errors::block_error_message(error).to_owned();
                }
                false
            }
        }
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

    pub fn construction_append(&mut self) {
        self.construction_finish_invalidate();
        let obstacles = self.facility_obstacles();
        let selected = self.selected;
        let world = &self.park.world;
        let result = self
            .park
            .construction
            .as_mut()
            .zip(selected)
            .ok_or("Select a piece first".to_string())
            .and_then(|(construction, selected)| {
                construction
                    .append(selected, world, &obstacles)
                    .map_err(construction_error)
            });
        let changed = result.is_ok();
        if changed {
            self.close_live_coaster();
            self.clear_block_layout();
            self.park.support_render_cache.clear();
            self.construction_recorder = None;
            self.construction_report = None;
            self.construction_train = None;
            self.construction_outcome = None;
            self.select_compatible_piece();
        }
        self.construction_message = result.err().unwrap_or_default();
        if changed {
            self.refresh_construction_support_rendering();
        }
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
        self.construction_finish_invalidate();
        let result = self
            .construction
            .as_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.delete_from(index).map_err(construction_error));
        let changed = result.is_ok();
        if changed {
            self.close_live_coaster();
            self.clear_block_layout();
            self.park.support_render_cache.clear();
            self.construction_recorder = None;
            self.construction_report = None;
            let (station, train_configuration) = {
                let construction = self
                    .park
                    .construction
                    .as_ref()
                    .expect("successful deletion");
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
                (station, construction.model().train.clone())
            };
            if let (Some((circuit, train)), Some(station)) =
                (&mut self.park.construction_train, station)
            {
                *train = Train::new(
                    circuit,
                    &train_configuration,
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
        if changed {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_close(&mut self) {
        self.construction_finish_invalidate();
        let result = self
            .construction
            .as_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.close().map_err(construction_error));
        let changed = result.is_ok();
        if changed {
            self.clear_block_layout();
        }
        self.construction_message = result.err().unwrap_or_default();
        if changed {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_finish_preview_active(&self) -> bool {
        self.construction_completion.is_some()
    }

    pub fn construction_finish_invalidate(&mut self) {
        let had_preview = self.construction_completion.is_some();
        self.construction_completion = None;
        self.construction_completion_available = None;
        self.construction_completion_search = None;
        self.construction_completion_checked = false;
        if had_preview {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_finish_search_start(&mut self) {
        self.construction_completion_available = None;
        self.construction_completion_search = self
            .construction
            .as_ref()
            .and_then(Construction::completion_search);
        self.construction_completion_checked = self.construction_completion_search.is_none();
    }

    pub fn construction_finish_search_advance(&mut self) -> bool {
        let obstacles = self.facility_obstacles();
        let Some(search) = &mut self.construction_completion_search else {
            return true;
        };
        match search.advance(&self.park.world, &obstacles) {
            CompletionSearchProgress::Pending => false,
            CompletionSearchProgress::Finished(completion) => {
                self.construction_completion_available = completion;
                self.construction_completion_search = None;
                self.construction_completion_checked = true;
                true
            }
        }
    }

    pub fn construction_can_finish(&self) -> bool {
        self.construction_completion_available.is_some() || self.construction_completion.is_some()
    }

    pub fn construction_finish_reason(&self) -> String {
        if self.construction_can_finish() {
            String::new()
        } else if self.construction_completion_checked {
            "No nearby connection to the station was found.".into()
        } else {
            "Checking for a connection to the station.".into()
        }
    }

    pub fn construction_finish_preview(&mut self) {
        self.construction_completion = self.construction_completion_available.take();
        self.construction_message = if self.construction_completion.is_some() {
            "Review the connection, then confirm to finish the coaster.".into()
        } else {
            "No nearby connection to the station was found.".into()
        };
        self.refresh_construction_support_rendering();
    }

    pub fn construction_finish_cancel(&mut self) {
        self.construction_completion_available = self.construction_completion.take();
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
    }

    pub fn construction_finish_confirm(&mut self) {
        let Some(completion) = self.construction_completion.take() else {
            self.construction_message = "Preview a connection first.".into();
            return;
        };
        let obstacles = self.facility_obstacles();
        let result = self
            .park
            .construction
            .as_mut()
            .expect("completion requires construction")
            .apply_completion(&completion, &self.park.world, &obstacles);
        if result.is_ok() {
            self.close_live_coaster();
            self.clear_block_layout();
            self.park.support_render_cache.clear();
            self.construction_recorder = None;
            self.construction_report = None;
            self.construction_train = None;
            self.construction_outcome = None;
            self.select_compatible_piece();
        }
        self.construction_message = result.err().map(construction_error).unwrap_or_default();
        self.refresh_construction_support_rendering();
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

    pub fn construction_supports(&self) -> Vec<f32> {
        self.construction_preview_supports_cache.clone()
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

    pub fn construction_placed_supports(&self) -> Vec<f32> {
        self.construction_placed_supports_cache.clone()
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
}

impl WasmSimulation {
    fn construction_support_entries(&self) -> Option<Vec<(&usize, &Piece)>> {
        let construction = self.construction.as_ref()?;
        Some(
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
                .collect(),
        )
    }

    fn construction_preview_supports_geometry(&self) -> Result<Vec<f32>, SupportError> {
        if !self.construction_preview_valid() {
            return Ok(Vec::new());
        }
        let construction = self
            .construction
            .as_ref()
            .expect("valid preview requires construction");
        let preview = self.construction_preview();
        if preview.is_empty() {
            return Ok(Vec::new());
        }
        let mut obstacles: Vec<_> = self
            .construction_support_entries()
            .expect("valid preview requires construction")
            .into_iter()
            .flat_map(|(_, placed_piece)| {
                track_support_obstacles(
                    std::slice::from_ref(placed_piece),
                    &construction.model().train,
                )
            })
            .collect();
        obstacles.extend(self.facility_obstacles());
        let mut supports = Vec::new();
        for piece in &preview {
            supports.extend(render_preview_supports(
                piece,
                &obstacles,
                &self.world,
                construction.model().support_style,
            )?);
            obstacles.extend(track_support_obstacles(
                std::slice::from_ref(piece),
                &construction.model().train,
            ));
        }
        Ok(self.encode_construction_geometry(supports, 7))
    }

    pub(crate) fn refresh_construction_support_rendering(&mut self) {
        let placed_supports = self.park.refresh_construction_supports();
        let preview_supports = self.construction_preview_supports_geometry();
        let placed_supports_failed = placed_supports.is_err();
        let preview_supports_failed = preview_supports.is_err();

        self.construction_placed_supports_cache = placed_supports
            .map(|supports| self.encode_construction_geometry(supports, 7))
            .unwrap_or_default();
        self.construction_preview_supports_cache = preview_supports.unwrap_or_default();

        if placed_supports_failed {
            self.park.construction_message =
                "Could not route supports for the placed track.".into();
        } else if preview_supports_failed {
            self.park.construction_message = "Could not route preview supports.".into();
        } else if matches!(
            self.park.construction_message.as_str(),
            "Could not route supports for the placed track." | "Could not route preview supports."
        ) {
            self.park.construction_message.clear();
        }
    }
}

#[cfg(test)]
mod getter_tests {
    use super::WasmSimulation;

    #[test]
    fn support_render_getters_do_not_mutate_park_state() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("Straight");
        let message_before = simulation.park.construction_message.clone();
        let preview_supports_before = simulation.construction_preview_supports_cache.clone();
        let placed_supports_before = simulation.construction_placed_supports_cache.clone();
        let cache_entry_count = simulation.park.support_render_cache.len();

        assert!(!simulation.construction_supports().is_empty());
        assert!(!simulation.construction_placed_supports().is_empty());

        assert_eq!(simulation.park.construction_message, message_before);
        assert_eq!(
            simulation.construction_preview_supports_cache,
            preview_supports_before
        );
        assert_eq!(
            simulation.construction_placed_supports_cache,
            placed_supports_before
        );
        assert_eq!(
            simulation.park.support_render_cache.len(),
            cache_entry_count
        );
    }

    #[test]
    fn support_render_errors_are_derived_without_mutating_park_state() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        for y in 95..105 {
            for x in 95..110 {
                simulation.set_world_surface(x, y, vec![100; 4], None);
            }
        }
        let message_before = simulation.park.construction_message.clone();
        let placed_supports_before = simulation.construction_placed_supports_cache.clone();
        let cache_entry_count = simulation.park.support_render_cache.len();

        assert!(simulation.construction_placed_supports().is_empty());
        assert_eq!(
            simulation.construction_message(),
            "Could not route supports for the placed track."
        );

        assert_eq!(simulation.park.construction_message, message_before);
        assert_eq!(
            simulation.construction_placed_supports_cache,
            placed_supports_before
        );
        assert_eq!(
            simulation.park.support_render_cache.len(),
            cache_entry_count
        );
    }

    #[test]
    fn newer_construction_command_error_overrides_cached_support_error() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        for y in 95..105 {
            for x in 95..110 {
                simulation.set_world_surface(x, y, vec![100; 4], None);
            }
        }
        assert_eq!(
            simulation.construction_message(),
            "Could not route supports for the placed track."
        );

        simulation.construction_select("unknown-piece");

        assert_eq!(simulation.construction_message(), "Unknown piece");
    }
}
