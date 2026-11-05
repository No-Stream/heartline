use super::*;
use serde::Serialize;
use theme_park_sim::physics::{Circuit, TestRun};
use theme_park_sim::track::Bank;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionPieceWire {
    id: &'static str,
    label: &'static str,
    group: &'static str,
    heading: &'static str,
    slope: &'static str,
    bank: &'static str,
    refusal: String,
    would_tunnel: bool,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionElementWire {
    id: String,
    name: &'static str,
    piece_id: &'static str,
    slope: &'static str,
    kind: &'static str,
    would_tunnel: bool,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionTrackWire {
    id: usize,
    label: &'static str,
    name: &'static str,
    group: &'static str,
    slope: &'static str,
    kind: &'static str,
    launch_target_speed: Option<f64>,
    launch_minimum_speed: Option<f64>,
    launch_maximum_speed: Option<f64>,
}

fn report_launches(circuit: &Circuit, report: &TestRun) -> Vec<crate::reports::LaunchReport> {
    let mut start_distance = 0.0;
    circuit
        .pieces()
        .iter()
        .enumerate()
        .filter_map(|(piece_index, piece)| {
            let end_distance = start_distance + piece.length();
            let launch = (piece.kind == PieceKind::Launch).then(|| {
                let exit_distance = end_distance.rem_euclid(circuit.length());
                let sample = report
                    .samples
                    .iter()
                    .min_by(|first, second| {
                        let distance_to_exit = |distance: f64| {
                            let difference = (distance - exit_distance).abs();
                            difference.min(circuit.length() - difference)
                        };
                        distance_to_exit(first.distance)
                            .total_cmp(&distance_to_exit(second.distance))
                    })
                    .expect("a completed test report has samples");
                crate::reports::LaunchReport {
                    piece_index,
                    exit_speed: sample.speed,
                }
            });
            start_distance = end_distance;
            launch
        })
        .collect()
}

#[derive(Serialize, TS)]
pub struct ConstructionColorsWire {
    track: &'static str,
    supports: &'static str,
    trains: &'static str,
}

pub(crate) fn colors_json(colors: Option<CoasterColors>) -> String {
    let colors = colors.map(|colors| ConstructionColorsWire {
        track: colors.track.name(),
        supports: colors.supports.name(),
        trains: colors.trains.name(),
    });
    serde_json::to_string(&colors).expect("coaster colors serialize")
}

fn element_representative(element: &ElementChoice) -> &Piece {
    element
        .geometry
        .iter()
        .find(|piece| piece.entry.heading != piece.exit.heading)
        .or_else(|| {
            element
                .geometry
                .iter()
                .max_by_key(|piece| slope_rank(piece.entry.slope).max(slope_rank(piece.exit.slope)))
        })
        .or_else(|| element.geometry.first())
        .expect("an element choice contains at least one piece")
}

fn slope_rank(slope: Slope) -> u8 {
    match slope {
        Slope::Level => 0,
        Slope::UpGentle | Slope::DownGentle => 1,
        Slope::UpModerate | Slope::DownModerate => 2,
        Slope::UpSteep | Slope::DownSteep => 3,
        Slope::UpSixty | Slope::DownSixty => 4,
    }
}

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
        let world = self.park.world.clone();
        let obstacles = self.facility_obstacles();
        let station_id = construction.next_piece_id();
        let tunnel_generation = self.park.next_construction_tunnel_generation();
        let station_plan = match construction.tunnel_plan(PieceId::Station, &world, &obstacles) {
            Ok(plan) => plan,
            Err(error) => {
                self.construction_message = construction_error(error);
                return;
            }
        };
        if let Err(error) = self
            .park
            .carve_for_construction_at(tunnel_generation, &[(station_id, station_plan)])
        {
            self.construction_message = error.to_string();
            return;
        }
        let carved_world = self.park.world.clone();
        if let Err(error) = construction.append(PieceId::Station, &carved_world, &obstacles) {
            self.park
                .restore_tunnel_piece_at(tunnel_generation, station_id);
            self.construction_message = construction_error(error);
            return;
        }
        self.park
            .activate_construction_tunnel_generation(tunnel_generation);
        self.close_live_coaster();
        self.construction = Some(construction);
        self.construction_undo_groups.clear();
        self.clear_block_layout();
        self.selected = None;
        self.selected_element = None;
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

    pub fn construction_set_launch_intensity(&mut self, id: usize, target_speed: f64) -> bool {
        self.construction_finish_invalidate();
        let changed = self.construction.as_mut().is_some_and(|construction| {
            construction.set_launch_intensity(id, target_speed).is_ok()
        });
        if changed {
            self.construction_recorder = None;
            self.construction_report = None;
            self.construction_train = None;
            self.construction_outcome = None;
            self.construction_message.clear();
        }
        changed
    }

    /// The coaster's track, supports and train colours, or null without a coaster.
    pub fn construction_colors(&self) -> String {
        colors_json(
            self.construction
                .as_ref()
                .map(|construction| construction.colors()),
        )
    }

    /// The coaster's train layout, or null without a coaster.
    pub fn construction_train_layout(&self) -> String {
        let layout = self.construction.as_ref().map(|construction| {
            train_layout(
                train_model_key(construction.model()),
                &construction.model().train,
            )
        });
        serde_json::to_string(&layout).expect("serialize train layout")
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
        self.selected_element = None;
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
        if let Some(element_id) = id.strip_prefix("element:") {
            let obstacles = self.facility_obstacles();
            self.selected = None;
            self.selected_element = self.park.construction.as_ref().and_then(|construction| {
                construction
                    .element_choices(&self.world, &obstacles)
                    .into_iter()
                    .find(|element| element.id == element_id)
                    .map(|element| element.id)
            });
            self.construction_message = if self.selected_element.is_some() {
                String::new()
            } else {
                "Unknown element".into()
            };
            self.refresh_construction_support_rendering();
            return;
        }
        self.selected = pieceid_from_wire(id);
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
        if let Some(element_id) = self.selected_element {
            return format!("element:{element_id}");
        }
        self.selected
            .map(|id| wire_pieceid(id).to_owned())
            .unwrap_or_default()
    }

    pub fn construction_choices(&self) -> String {
        let obstacles = self.facility_obstacles();
        let choices: Vec<_> = self
            .construction
            .as_ref()
            .map(|construction| {
                construction
                    .available_choices(&self.world, &obstacles)
                    .into_iter()
                    .map(|(piece, choice)| ConstructionPieceWire {
                        id: wire_pieceid(piece.id),
                        label: piece.name,
                        group: piece_group(&piece),
                        heading: wire_heading(piece.exit.heading),
                        slope: wire_slope(piece.exit.slope),
                        bank: wire_bank(piece.exit.bank),
                        refusal: choice
                            .result
                            .err()
                            .map(construction_error)
                            .unwrap_or_default(),
                        would_tunnel: choice.would_tunnel,
                    })
                    .collect()
            })
            .unwrap_or_default();
        serde_json::to_string(&choices).expect("construction choices serialize")
    }

    pub fn construction_elements(&self) -> String {
        let obstacles = self.facility_obstacles();
        let elements: Vec<_> = self
            .construction
            .as_ref()
            .map(|construction| {
                construction
                    .element_choices(&self.world, &obstacles)
                    .into_iter()
                    .map(|element| {
                        let representative = element_representative(&element);
                        ConstructionElementWire {
                            id: format!("element:{}", element.id),
                            name: element.name,
                            piece_id: wire_pieceid(representative.id),
                            slope: wire_slope(representative.exit.slope),
                            kind: wire_piecekind(representative.kind),
                            would_tunnel: element.would_tunnel,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        serde_json::to_string(&elements).expect("construction elements serialize")
    }

    pub fn construction_track(&self) -> String {
        let track: Vec<_> = self
            .construction
            .as_ref()
            .map(|construction| {
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
                    .map(|(id, piece)| ConstructionTrackWire {
                        id: *id,
                        label: wire_pieceid(piece.id),
                        name: piece.name,
                        group: piece_group(piece),
                        slope: wire_slope(piece.exit.slope),
                        kind: wire_piecekind(piece.kind),
                        launch_target_speed: (piece.kind == PieceKind::Launch)
                            .then(|| {
                                piece.launch_target_speed.or_else(|| {
                                    construction
                                        .launch_speed_bounds(*id)
                                        .map(|bounds| bounds.minimum)
                                })
                            })
                            .flatten(),
                        launch_minimum_speed: construction
                            .launch_speed_bounds(*id)
                            .map(|bounds| bounds.minimum),
                        launch_maximum_speed: construction
                            .launch_speed_bounds(*id)
                            .map(|bounds| bounds.maximum),
                    })
                    .collect()
            })
            .unwrap_or_default();
        serde_json::to_string(&track).expect("construction track serializes")
    }

    pub fn construction_can_append(&self) -> bool {
        let Some(construction) = self.construction.as_ref() else {
            return false;
        };
        let obstacles = self.facility_obstacles();
        if let Some(element_id) = self.selected_element {
            return construction
                .element_choices(&self.world, &obstacles)
                .iter()
                .any(|element| element.id == element_id);
        }
        self.selected.is_some_and(|selected| {
            construction
                .choices(&self.world, &obstacles)
                .iter()
                .any(|choice| choice.id == selected && choice.result.is_ok())
        })
    }

    pub fn construction_preview_valid(&self) -> bool {
        self.construction_completion.is_some() || self.construction_can_append()
    }

    pub fn construction_preview_reason(&self) -> String {
        if let Some(element_id) = self.selected_element {
            let available = self.construction.as_ref().is_some_and(|construction| {
                construction
                    .element_choices(&self.world, &self.facility_obstacles())
                    .iter()
                    .any(|element| element.id == element_id)
            });
            return if available {
                String::new()
            } else {
                "Element is no longer available here.".into()
            };
        }
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
        let selected_element = self.selected_element;
        let world = self.park.world.clone();
        let undo_group_size = selected_element
            .and_then(|element_id| {
                self.park.construction.as_ref().and_then(|construction| {
                    construction
                        .element_choices(&world, &obstacles)
                        .into_iter()
                        .find(|element| element.id == element_id)
                        .map(|element| element.pieces.len())
                })
            })
            .unwrap_or(1);
        let result: Result<(), String> = (|| {
            let construction = self
                .park
                .construction
                .as_ref()
                .ok_or_else(|| "Construction is inactive".to_string())?;
            if let Some(element_id) = selected_element {
                let element = construction
                    .element_choices(&world, &obstacles)
                    .into_iter()
                    .find(|element| element.id == element_id)
                    .ok_or_else(|| "Selected element is no longer available here.".to_string())?;
                let first_id = construction.next_piece_id();
                let tunnel_plans = construction
                    .element_tunnel_plans(&element, &world, &obstacles)
                    .map_err(construction_error)?;
                let piece_ids: Vec<_> = (0..tunnel_plans.len())
                    .map(|offset| first_id + offset)
                    .collect();
                let ownership: Vec<_> = piece_ids.iter().copied().zip(tunnel_plans).collect();
                if let Err(error) = self.park.carve_for_construction(&ownership) {
                    return Err(error.to_string());
                }
                let carved_world = self.park.world.clone();
                if let Err(error) = self
                    .park
                    .construction
                    .as_mut()
                    .expect("construction remains active")
                    .apply_element(&element, &carved_world, &obstacles)
                {
                    for piece_id in piece_ids {
                        self.park.restore_tunnel_piece(piece_id);
                    }
                    return Err(construction_error(error));
                }
                return Ok(());
            }

            let selected = selected.ok_or_else(|| "Select a piece first".to_string())?;
            let piece_id = construction.next_piece_id();
            let tunnel_plan = construction
                .tunnel_plan(selected, &world, &obstacles)
                .map_err(construction_error)?;
            if let Err(error) = self.park.carve_for_construction(&[(piece_id, tunnel_plan)]) {
                return Err(error.to_string());
            }
            let carved_world = self.park.world.clone();
            if let Err(error) = self
                .park
                .construction
                .as_mut()
                .expect("construction remains active")
                .append(selected, &carved_world, &obstacles)
            {
                self.park.restore_tunnel_piece(piece_id);
                return Err(construction_error(error));
            }
            Ok(())
        })();
        let changed = result.is_ok();
        if changed {
            self.construction_undo_groups.push(undo_group_size);
            self.selected_element = None;
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
        if self
            .construction
            .as_ref()
            .is_none_or(|construction| construction.piece_ids().is_empty())
        {
            self.construction_message = "There is no active track to undo.".into();
            return;
        }
        let mut undo_groups = std::mem::take(&mut self.construction_undo_groups);
        let undo_group_size = undo_groups.pop().unwrap_or(1);
        for _ in 0..undo_group_size {
            let Some(id) = self
                .construction
                .as_ref()
                .and_then(|construction| construction.piece_ids().last())
                .copied()
            else {
                break;
            };
            self.construction_delete_piece(id);
        }
        self.construction_undo_groups = undo_groups;
    }

    pub fn construction_delete(&mut self, index: usize) {
        self.construction_undo_groups.clear();
        self.construction_delete_piece(index);
    }

    fn construction_delete_piece(&mut self, index: usize) {
        self.construction_finish_invalidate();
        let result = self
            .construction
            .as_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.delete_from(index).map_err(construction_error));
        let changed = result.is_ok();
        if changed {
            self.park.restore_tunnel_piece(index);
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
        self.construction_completion_unavailable_reason = None;
        if had_preview {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_finish_search_start(&mut self) {
        self.construction_completion_available = None;
        let (search, reason) = self.construction.as_ref().map_or(
            (None, Some("Place a station before finishing the coaster.")),
            |construction| {
                let search = construction.completion_search();
                let reason = search
                    .is_none()
                    .then(|| construction_finish_unavailable_reason(construction));
                (search, reason)
            },
        );
        self.construction_completion_search = search;
        self.construction_completion_unavailable_reason = reason;
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
            self.construction_completion_unavailable_reason
                .unwrap_or("No nearby connection to the station was found.")
                .into()
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
        let world = self.park.world.clone();
        let Some(construction) = self.park.construction.as_ref() else {
            self.construction_message = "Construction is inactive".into();
            return;
        };
        let first_id = construction.next_piece_id();
        let tunnel_plans =
            match construction.tunnel_plans_for_geometry(&completion.geometry, &world) {
                Ok(plans) => plans,
                Err(error) => {
                    self.construction_message = construction_error(error);
                    self.refresh_construction_support_rendering();
                    return;
                }
            };
        let piece_ids: Vec<_> = (0..tunnel_plans.len())
            .map(|offset| first_id + offset)
            .collect();
        let ownership: Vec<_> = piece_ids.iter().copied().zip(tunnel_plans).collect();
        let result = self.park.carve_for_construction(&ownership);
        let message = match result {
            Err(error) => error.to_string(),
            Ok(()) => {
                let apply_result = self
                    .park
                    .construction
                    .as_mut()
                    .expect("completion requires construction")
                    .apply_completion(&completion, &self.park.world, &obstacles);
                match apply_result {
                    Ok(()) => String::new(),
                    Err(error) => {
                        for piece_id in &piece_ids {
                            self.park.restore_tunnel_piece(*piece_id);
                        }
                        construction_error(error)
                    }
                }
            }
        };
        if message.is_empty() {
            self.close_live_coaster();
            self.clear_block_layout();
            self.park.support_render_cache.clear();
            self.construction_recorder = None;
            self.construction_report = None;
            self.construction_train = None;
            self.construction_outcome = None;
            self.select_compatible_piece();
        }
        self.construction_message = message;
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
        self.construction_recorder = Some(TestRunRecorder::for_circuit(&circuit));
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
        let samples = report
            .samples
            .iter()
            .map(|sample| {
                let position = circuit.sample(sample.distance).position / 4.0;
                crate::reports::TestingSample {
                    distance: sample.distance,
                    x: position.x,
                    y: position.y,
                    z: position.z,
                    speed: sample.speed,
                    vertical_g: sample.vertical_g,
                    lateral_g: sample.lateral_g,
                    longitudinal_g: sample.longitudinal_g,
                    airtime: sample.airtime,
                }
            })
            .collect();
        let proximity_events = report
            .proximity_events
            .iter()
            .map(|event| {
                let kind = match event.kind {
                    ProximityKind::NearMiss => "nearMiss",
                    ProximityKind::Headchopper => "headchopper",
                    ProximityKind::Dueling => "dueling",
                    ProximityKind::Tunnel => "tunnel",
                    ProximityKind::TerrainHugging => "terrainHugging",
                };
                crate::reports::ProximityEventReport {
                    kind: kind.to_owned(),
                    piece_index: event.piece_index,
                    distance: event.distance,
                    x: event.position.x / 4.0,
                    y: event.position.y / 4.0,
                    z: event.position.z / 4.0,
                    strength: event.strength,
                }
            })
            .collect();
        serde_json::to_string(&crate::reports::TestingReport {
            samples,
            launches: report_launches(circuit, report),
            notes: report.notes.clone(),
            proximity_events,
            rating: crate::reports::TestingRating {
                excitement: report.excitement,
                intensity: report.intensity,
                nausea: report.nausea,
            },
        })
        .expect("serialize testing report")
    }

    pub fn construction_rails(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_rails(&self.construction_preview(), &self.world),
            layouts::RAIL.len(),
        )
    }

    pub fn construction_ties(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_ties(&self.construction_preview(), &self.world),
            layouts::TIE.len(),
        )
    }

    pub fn construction_supports(&self) -> Vec<f32> {
        self.construction_preview_supports_cache.clone()
    }

    pub fn construction_placed_rails(&self) -> Vec<f32> {
        self.construction_render(render_rails, layouts::RAIL.len())
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
        self.construction_render(render_ties, layouts::TIE.len())
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

fn construction_finish_unavailable_reason(construction: &Construction) -> &'static str {
    if construction.is_closed() {
        "The coaster circuit is already closed."
    } else if construction
        .pieces()
        .first()
        .is_none_or(|piece| piece.kind != PieceKind::Station)
    {
        "Place a station before finishing the coaster."
    } else if !construction.disconnected_segments().is_empty() {
        "Reconnect the disconnected track before finishing the coaster."
    } else {
        "No nearby connection to the station was found."
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
        let tunnel_plans = construction
            .tunnel_plans_for_geometry(&preview, &self.world)
            .map_err(|_| SupportError::NoRoute)?;
        let carve_cells: Vec<_> = tunnel_plans
            .into_iter()
            .flat_map(|plan| {
                plan.carve_cells
                    .into_iter()
                    .map(|cell| (cell.x, cell.y, cell.height))
            })
            .collect();
        let mut preview_world = self.world.clone();
        preview_world
            .carve_cells(&carve_cells)
            .map_err(|_| SupportError::NoRoute)?;
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
                &preview_world,
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

macro_rules! define_piece_id_wire_names {
    ($($variant:ident),+ $(,)?) => {
        pub(crate) fn wire_pieceid(value: PieceId) -> &'static str {
            match value {
                $(PieceId::$variant => stringify!($variant),)+
            }
        }

        pub(crate) fn pieceid_from_wire(value: &str) -> Option<PieceId> {
            match value {
                $(stringify!($variant) => Some(PieceId::$variant),)+
                _ => None,
            }
        }
    };
}

define_piece_id_wire_names!(
    Straight,
    GentleUp,
    GentleDown,
    SteepUp,
    SteepDown,
    SteepToSixtyUp,
    SixtyUp,
    SixtyToSteepUp,
    SteepToSixtyDown,
    SixtyDown,
    SixtyToSteepDown,
    LevelToGentleUp,
    GentleUpToLevel,
    GentleToSteepUp,
    SteepToGentleUp,
    LevelToGentleDown,
    GentleDownToLevel,
    GentleToSteepDown,
    SteepToGentleDown,
    TurnSmallEntry,
    TurnSmall,
    TurnSmallExit,
    TurnLargeEntry,
    TurnLarge,
    TurnLargeExit,
    BankedTurnEntry,
    BankedTurnExit,
    TurnSmallStandalone,
    TurnLargeStandalone,
    BankedTurnStandalone,
    LargeLeftBankedTurnStandalone,
    SmallRightBankedTurnStandalone,
    LargeRightBankedTurnStandalone,
    LargeLeftBankedTurnEntry,
    LargeLeftBankedTurn,
    LargeLeftBankedTurnExit,
    SmallRightBankedTurnEntry,
    SmallRightBankedTurn,
    SmallRightBankedTurnExit,
    LargeRightBankedTurnEntry,
    LargeRightBankedTurn,
    LargeRightBankedTurnExit,
    VerticalUpEntry,
    VerticalUpContinue,
    VerticalUpExit,
    VerticalDownEntry,
    VerticalDownContinue,
    VerticalDownExit,
    BankLeft,
    UnbankLeft,
    BankRight,
    UnbankRight,
    BankedTurn,
    OffsetLeft,
    OffsetRight,
    ShortOffsetLeft,
    ShortOffsetRight,
    Loop,
    Corkscrew,
    HeartlineRoll,
    Brakes,
    Station,
    TurnSmallRightEntry,
    TurnSmallRight,
    TurnSmallRightExit,
    TurnSmallRightStandalone,
    TurnLargeRightEntry,
    TurnLargeRight,
    TurnLargeRightExit,
    TurnLargeRightStandalone,
    CorkscrewRight,
    HeartlineRollRight,
    LaunchShort,
    Launch,
    LaunchLong,
    TerrainRise,
    TerrainFall,
    SteelLoop,
    TwisterFanLeft,
    TwisterFanRight,
    TwisterHelixLeft,
    TwisterHelixRight,
    AirtimeHill,
    TightTwisterLeft,
    TightTwisterRight,
    HybridOverbankLeft,
    HybridOverbankRight,
    HalfTileStraight,
    FullOffsetLeft,
    FullOffsetRight,
    OneTileStraight,
    OneTileGentleUp,
    OneTileGentleDown,
    ShortRise,
    ShortFall,
    LoopRight,
    SteelLoopRight,
    HalfLoopUp,
    HalfLoopDown,
    HalfCorkscrewLeftUp,
    HalfCorkscrewRightUp,
    HalfCorkscrewLeftDown,
    HalfCorkscrewRightDown,
    TopHat,
    LevelToSixtyUp,
    SixtyUpToLevel,
    LevelToSixtyDown,
    SixtyDownToLevel,
    BankedDropLeft,
    BankedDropRight,
    TwistedDropLeft,
    TwistedDropRight,
    BarrelDropLeft,
    BarrelDropRight,
    RavenRollLeftUp,
    RavenRollRightUp,
    RavenRollLeftDown,
    RavenRollRightDown,
    LargeLoop,
    LargeLoopRight,
);

fn wire_heading(value: Heading) -> &'static str {
    match value {
        Heading::East => "East",
        Heading::NorthEast => "NorthEast",
        Heading::North => "North",
        Heading::NorthWest => "NorthWest",
        Heading::West => "West",
        Heading::SouthWest => "SouthWest",
        Heading::South => "South",
        Heading::SouthEast => "SouthEast",
    }
}

pub(crate) fn wire_slope(value: Slope) -> &'static str {
    match value {
        Slope::DownSteep => "DownSteep",
        Slope::DownSixty => "DownSixty",
        Slope::DownModerate => "DownModerate",
        Slope::DownGentle => "DownGentle",
        Slope::Level => "Level",
        Slope::UpGentle => "UpGentle",
        Slope::UpModerate => "UpModerate",
        Slope::UpSteep => "UpSteep",
        Slope::UpSixty => "UpSixty",
    }
}

pub(crate) fn wire_bank(value: Bank) -> &'static str {
    match value {
        Bank::Right => "Right",
        Bank::Level => "Level",
        Bank::Left => "Left",
        Bank::Inverted => "Inverted",
    }
}

pub(crate) fn wire_piecekind(value: PieceKind) -> &'static str {
    match value {
        PieceKind::Track => "Track",
        PieceKind::Lift => "Lift",
        PieceKind::Launch => "Launch",
        PieceKind::Brakes => "Brakes",
        PieceKind::Station => "Station",
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
