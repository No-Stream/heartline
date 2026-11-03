use super::*;
use theme_park_sim::track::blocks::brake_runs;

#[wasm_bindgen]
impl WasmSimulation {
    /// Advance pending block analysis by at most the requested number of physics steps.
    /// Returns true once analysis has finished. Large requests are capped per call.
    pub fn construction_advance_block_analysis(&mut self, work_steps: u32) -> bool {
        let budget = work_steps.min(1_000) as usize;
        let has_layout_search =
            self.block_manual_search.is_some() || self.block_placement_search.is_some();
        let search_budget = if has_layout_search && self.block_throughput_estimator.is_some() {
            let extra_to_throughput = self.block_analysis_extra_to_throughput && budget % 2 == 1;
            if budget % 2 == 1 {
                self.block_analysis_extra_to_throughput = !self.block_analysis_extra_to_throughput;
            }
            (budget + usize::from(budget % 2 == 1 && !extra_to_throughput)) / 2
        } else if has_layout_search {
            budget
        } else {
            0
        };
        let mut search_steps = 0;
        if let Some(mut search) = self.block_manual_search.take() {
            let before = search.work_completed();
            search.advance(search_budget);
            search_steps = search.work_completed() - before;
            match search.result() {
                Some(Ok(layout)) => {
                    let installed = self
                        .block_layout
                        .as_ref()
                        .expect("validation has an installed layout");
                    if block_layout_fingerprint(layout) == block_layout_fingerprint(installed) {
                        self.block_layout = Some(layout.clone());
                        self.block_layout_is_automatic = false;
                        self.block_baseline_validation = false;
                    } else if self.coaster_has_active_trains() {
                        self.block_manual_search = Some(search);
                    } else {
                        self.install_block_layout(layout.clone(), false);
                    }
                }
                Some(Err(reason)) => {
                    self.block_baseline_validation = false;
                    self.set_block_error(reason);
                }
                None => {
                    self.block_manual_search = Some(search);
                }
            }
        } else if let Some(mut search) = self.block_placement_search.take() {
            if !self.block_apply_best_requested {
                let before = search.work_completed();
                search.advance(search_budget);
                search_steps = search.work_completed() - before;
            }
            let complete = search.is_complete() || self.block_apply_best_requested;
            let best = search.best();
            let installed = self
                .block_layout
                .as_ref()
                .expect("search has an installed layout");
            let improves_capacity = best.capacity() > installed.capacity();
            let same_layout = block_layout_fingerprint(best) == block_layout_fingerprint(installed);
            let verifies_installed = same_layout
                && !installed.capacity_search_complete()
                && best.capacity_search_complete();
            let should_install = self.block_apply_best_requested
                || improves_capacity
                || verifies_installed
                || confirmed_stalled_baseline(best, installed)
                || (complete && best.capacity() >= installed.capacity());
            if should_install && !same_layout && self.coaster_has_active_trains() {
                self.block_placement_search = Some(search);
            } else {
                if should_install {
                    let layout = best.clone();
                    if same_layout {
                        self.block_layout = Some(layout);
                        self.block_layout_is_automatic = true;
                    } else {
                        self.install_block_layout(layout, true);
                    }
                }
                if complete {
                    self.block_apply_best_requested = false;
                    if search.is_complete() {
                        self.block_placement_cache = Some(search);
                    }
                } else {
                    self.block_placement_search = Some(search);
                }
            }
        }
        if let Some(mut estimator) = self.block_throughput_estimator.take() {
            if let Some(result) = estimator.advance(budget - search_steps) {
                let estimate = match result {
                    Ok(throughput) => (throughput, String::new()),
                    Err(reason) => (0.0, reason.to_owned()),
                };
                self.block_hourly_throughput = estimate.0;
                self.block_throughput_reason.clone_from(&estimate.1);
                let layout = self.block_layout.as_ref().expect("estimator has a layout");
                self.block_throughput_cache
                    .insert(block_layout_fingerprint(layout), estimate);
            } else {
                self.block_throughput_estimator = Some(estimator);
            }
        }
        self.block_manual_search.is_none()
            && self.block_placement_search.is_none()
            && self.block_throughput_estimator.is_none()
    }

    pub fn construction_auto_blocks(&mut self) -> bool {
        if self.block_placement_search.is_some() {
            self.block_message.clear();
            self.construction_message.clear();
            return true;
        }
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            self.set_block_error("A closed circuit is required to place block sections.");
            return false;
        };
        let config = block_physics_config();
        let search = match self.block_placement_cache.take() {
            Some(search) => Ok(search),
            None => BlockPlacementSearch::new(circuit, train, config),
        };
        match search {
            Ok(search) => {
                self.block_manual_search = None;
                self.block_baseline_validation = false;
                self.block_apply_best_requested = false;
                if self.block_layout.is_none() {
                    self.install_block_layout(search.best().clone(), true);
                }
                self.block_placement_search = Some(search);
                self.construction_advance_block_analysis(0);
                self.block_message.clear();
                self.construction_message.clear();
                true
            }
            Err(reason) => {
                self.set_block_error(reason);
                false
            }
        }
    }

    /// Install the best verified placement found so far and stop searching.
    pub fn construction_apply_best_block_sections(&mut self) -> bool {
        let Some(search) = self.block_placement_search.take() else {
            self.set_block_error("There is no automatic block placement search in progress.");
            return false;
        };
        if self.coaster_has_active_trains() {
            self.block_apply_best_requested = true;
            self.block_placement_search = Some(search);
            return true;
        }
        self.install_block_layout(search.best().clone(), true);
        if search.is_complete() {
            self.block_placement_cache = Some(search);
        }
        true
    }

    pub fn construction_add_block_section(&mut self, boundary_metres: f64) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        boundaries.push(boundary_metres);
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_move_block_section(&mut self, index: usize, boundary_metres: f64) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        let Some(boundary) = boundaries.get_mut(index) else {
            self.set_block_error("Block section index is out of range.");
            return false;
        };
        *boundary = boundary_metres;
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_remove_block_section(&mut self, index: usize) -> bool {
        let Some(layout) = &self.block_layout else {
            self.set_block_error("Place automatic block sections before editing them.");
            return false;
        };
        let mut boundaries = editable_block_boundaries(layout);
        if index >= boundaries.len() {
            self.set_block_error("Block section index is out of range.");
            return false;
        }
        boundaries.remove(index);
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_block_candidates(&self) -> Vec<f64> {
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            return Vec::new();
        };
        let mut candidates: Vec<_> = brake_runs(&circuit, train.length_metres)
            .into_iter()
            .map(|run| run.boundary_metres)
            .collect();
        candidates.sort_by(f64::total_cmp);
        candidates.dedup();
        candidates
    }

    pub fn construction_set_block_sections(&mut self, boundaries: Vec<f64>) -> bool {
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            self.set_block_error("A closed circuit is required to edit block sections.");
            return false;
        };
        match BlockManualSearch::new(circuit, train, block_physics_config(), boundaries) {
            Ok(search) => {
                self.block_placement_search = None;
                self.block_apply_best_requested = false;
                self.block_baseline_validation = false;
                self.block_manual_search = Some(search);
                self.block_message.clear();
                self.construction_message.clear();
                true
            }
            Err(reason) => {
                self.set_block_error(reason);
                false
            }
        }
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
