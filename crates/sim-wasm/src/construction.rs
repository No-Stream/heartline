use super::*;

#[wasm_bindgen]
impl WasmSimulation {
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
}
