use super::*;

#[wasm_bindgen]
impl WasmSimulation {
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
}
