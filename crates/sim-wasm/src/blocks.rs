use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    /// Advance pending block analysis by at most the requested number of physics steps.
    /// Returns true once analysis has finished. Large requests are capped per call.
    pub fn construction_advance_block_analysis(&mut self, work_steps: u32) -> bool {
        let (complete, error) = self.park.construction_advance_block_analysis(work_steps);
        if let Some(error) = error {
            self.record_block_error(error);
        }
        complete
    }

    pub fn construction_auto_blocks(&mut self) -> bool {
        let result = self.park.construction_auto_blocks();
        self.record_block_result(result)
    }

    pub fn construction_apply_best_block_sections(&mut self) -> bool {
        let result = self.park.construction_apply_best_block_sections();
        self.record_block_result(result)
    }

    pub fn construction_add_block_section(&mut self, boundary_metres: f64) -> bool {
        let result = self.park.construction_add_block_section(boundary_metres);
        self.record_block_result(result)
    }

    pub fn construction_move_block_section(&mut self, index: usize, boundary_metres: f64) -> bool {
        let result = self
            .park
            .construction_move_block_section(index, boundary_metres);
        self.record_block_result(result)
    }

    pub fn construction_remove_block_section(&mut self, index: usize) -> bool {
        let result = self.park.construction_remove_block_section(index);
        self.record_block_result(result)
    }

    pub fn construction_block_candidates(&self) -> Vec<f64> {
        self.park.construction_block_candidates()
    }

    pub fn construction_set_block_sections(&mut self, boundaries: Vec<f64>) -> bool {
        let result = self.park.construction_set_block_sections(boundaries);
        self.record_block_result(result)
    }

    pub fn construction_block_report_json(&self) -> String {
        let Some(layout) = &self.block_layout else {
            let reasons = self.block_reasons_json();
            return format!(
                "{{\"sections\":[],\"trainCapacity\":0,\"capacitySearchComplete\":null,\"theoreticalHourlyRiders\":null,\"throughputStatus\":\"unavailable\",\"placementStatus\":\"idle\",\"placementSearchComplete\":null,\"reasons\":{reasons},\"notes\":[],\"suggestions\":[]}}"
            );
        };
        let sections = layout
            .boundaries()
            .iter()
            .enumerate()
            .map(|(index, boundary)| {
                let brake_pieces = layout.brake_pieces(index);
                let brake_piece_indices = brake_pieces
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let powered_hold_pieces = layout.powered_hold_pieces(index);
                let powered_hold_piece_indices = powered_hold_pieces
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let editable = !brake_pieces.is_empty();
                let powered_hold = !powered_hold_pieces.is_empty();
                let editable_index = layout
                    .boundaries()
                    .iter()
                    .enumerate()
                    .take(index)
                    .filter(|(previous_index, _)| {
                        !layout.brake_pieces(*previous_index).is_empty()
                    })
                    .count();
                let (reason, track_kind) = if editable {
                    (
                        "Placed on a level brake run long enough to hold a train.",
                        "blockBrake",
                    )
                } else if powered_hold {
                    (
                        "Fixed powered hold at the start of lift or launch track.",
                        kind_for_boundary(layout.circuit(), *boundary),
                    )
                } else {
                    (
                        "Fixed pass-through boundary with no stopping mechanism.",
                        kind_for_boundary(layout.circuit(), *boundary),
                    )
                };
                format!(
                    "{{\"boundaryMetres\":{boundary},\"brakeStartMetres\":{},\"brakePieceIndices\":[{brake_piece_indices}],\"poweredHoldStartMetres\":{},\"poweredHoldPieceIndices\":[{powered_hold_piece_indices}],\"editable\":{editable},\"editableIndex\":{},\"reason\":{},\"trackKind\":{}}}",
                    if editable {
                        layout.brake_start(index).to_string()
                    } else {
                        "null".into()
                    },
                    if powered_hold {
                        layout.brake_start(index).to_string()
                    } else {
                        "null".into()
                    },
                    if editable { editable_index.to_string() } else { "null".into() },
                    json_string(reason),
                    json_string(track_kind)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let reasons = self.block_reasons_json();
        let source = if self.block_layout_is_automatic {
            "automatic"
        } else {
            "manual"
        };
        let mut notes = vec![
            "Trains wait at a block brake while the next section is occupied.",
            "The hourly estimate averages three dispatch intervals after the first refill and excludes station loading time.",
            "Lift and launch controls can hold a train at the start of powered track.",
            "Fixed station and powered-hardware boundaries cannot be moved or removed.",
        ];
        if self.block_placement_search.is_some() {
            notes.push(
                "Automatic placement is still searching; the installed layout remains in use.",
            );
        } else if self.block_manual_search.is_some() {
            if self.block_baseline_validation {
                notes.push("The one-train circuit is being checked while the coaster operates.");
            } else {
                notes.push("The requested block edit is being validated; the installed layout remains in use.");
            }
        }
        if self.block_layout_change_pending() {
            notes.push("New dispatches are paused until the updated block layout is installed.");
        }
        if self.block_placement_search.is_none()
            && self.block_manual_search.is_none()
            && self.block_layout_is_automatic
            && !layout.placement_search_complete()
        {
            notes.push("Automatic placement is incomplete. The current layout may not support the maximum number of trains.");
        }
        if layout.capacity() == 0 {
            notes.push("A train stalled and could not complete this circuit. Adjust the layout before running the coaster.");
        } else if !layout.capacity_search_complete() {
            if editable_block_boundaries(layout).is_empty() {
                notes.push("The one-train circuit has not yet been verified; a train may stall.");
            } else {
                notes.push("The reported train capacity is a verified lower bound; additional capacity may be possible.");
            }
        }
        let notes = notes
            .into_iter()
            .map(json_string)
            .collect::<Vec<_>>()
            .join(",");
        let suggestions = block_layout_suggestions(layout)
            .iter()
            .map(|suggestion| json_string(suggestion))
            .collect::<Vec<_>>()
            .join(",");
        let throughput_status = if self.block_throughput_estimator.is_some() {
            "pending"
        } else if self.block_throughput_reason.is_empty() {
            "ready"
        } else {
            "unavailable"
        };
        let throughput = if throughput_status == "ready" {
            self.block_hourly_throughput.to_string()
        } else {
            "null".into()
        };
        let placement_status = if self.block_placement_search.is_some() {
            "searching"
        } else if self.block_manual_search.is_some() {
            "validating"
        } else if !self.block_layout_is_automatic {
            "manual"
        } else if layout.placement_search_complete() {
            "complete"
        } else {
            "incomplete"
        };
        let search_complete = if matches!(placement_status, "manual" | "validating") {
            "null"
        } else if placement_status == "complete" {
            "true"
        } else {
            "false"
        };
        let placement_work_completed = self
            .block_placement_search
            .as_ref()
            .map(BlockPlacementSearch::work_completed)
            .or_else(|| {
                self.block_manual_search
                    .as_ref()
                    .map(BlockManualSearch::work_completed)
            })
            .unwrap_or(0);
        let best_capacity = self
            .block_placement_search
            .as_ref()
            .map_or(layout.capacity(), |search| search.best().capacity());
        format!(
            "{{\"sections\":[{sections}],\"source\":{},\"trainCapacity\":{},\"capacitySearchComplete\":{},\"theoreticalHourlyRiders\":{throughput},\"throughputStatus\":{},\"placementStatus\":{},\"placementSearchComplete\":{search_complete},\"placementWorkCompleted\":{placement_work_completed},\"placementBestTrainCapacity\":{best_capacity},\"reasons\":{reasons},\"notes\":[{notes}],\"suggestions\":[{suggestions}]}}",
            json_string(source),
            layout.capacity(),
            layout.capacity_search_complete(),
            json_string(throughput_status),
            json_string(placement_status)
        )
    }
}

impl WasmSimulation {
    pub(crate) fn record_block_error(
        &mut self,
        error: theme_park_sim::park::block_analysis::BlockCommandError,
    ) {
        let message = crate::command_errors::command_error_message(CommandError::Block(error));
        self.park.block_message = message.to_owned();
        self.park.construction_message = message.to_owned();
    }

    fn record_block_result(
        &mut self,
        result: Result<(), theme_park_sim::park::block_analysis::BlockCommandError>,
    ) -> bool {
        match result {
            Ok(()) => true,
            Err(error) => {
                self.record_block_error(error);
                false
            }
        }
    }
}
