//! Incremental block layout analysis and installation.

#[cfg(test)]
#[path = "block_error_tests.rs"]
mod error_tests;
#[cfg(test)]
#[path = "block_analysis_tests.rs"]
mod tests;

use super::tick::COASTER_PLACE_ID;
use super::tick::{
    block_layout_fingerprint, confirmed_stalled_baseline, editable_block_boundaries,
};
use super::{CoasterId, Park};
use crate::facilities::coaster_facility;
use crate::physics::{BlockOperation, Circuit, PhysicsConfig};
use crate::track::TrainConfiguration;
use crate::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, BlockSectionPlacement,
    ThroughputEstimator, brake_runs,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCommandError {
    InvalidTrainCount,
    InvalidCarsPerTrain,
    CoasterOpen,
    RecordedTrainCountExceedsCapacity,
    NoConstruction,
    CompleteCircuitRequired,
    MissingStationAccess,
    ClosedCircuitRequiredForPlacement,
    ClosedCircuitRequiredForEditing,
    RecordedLayoutWhileOpen,
    NoAutomaticSearch,
    NoBlockLayout,
    IndexOutOfRange,
    Validation(&'static str),
}

pub fn block_physics_config() -> PhysicsConfig {
    PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    }
}

impl Park {
    pub fn construction_set_cars_per_train(
        &mut self,
        id: CoasterId,
        cars: u8,
    ) -> Result<(), BlockCommandError> {
        let coaster = self.coaster(id).ok_or(BlockCommandError::NoConstruction)?;
        if coaster.coaster_open {
            return Err(BlockCommandError::CoasterOpen);
        }
        let construction = &coaster.construction;
        if !construction.model().cars_per_train.contains(&cars) {
            return Err(BlockCommandError::InvalidCarsPerTrain);
        }
        if construction.model().train.cars == cars {
            return Ok(());
        }
        let mut train = construction.model().train.clone();
        train.length_metres = train.length_metres / f64::from(train.cars) * f64::from(cars);
        train.cars = cars;
        let search = match construction.circuit() {
            Ok(circuit) if circuit.length() > train.length_metres => Some(
                BlockPlacementSearch::new(circuit, train, block_physics_config())
                    .map_err(BlockCommandError::Validation)?,
            ),
            _ => None,
        };
        let coaster = self.coaster_mut(id).expect("coaster checked above");
        coaster.construction.set_cars_per_train(cars);
        coaster.construction_train = None;
        coaster.construction_recorder = None;
        coaster.construction_report = None;
        coaster.construction_outcome = None;
        coaster.support_render_cache.clear();
        self.clear_block_layout(id);
        if let Some(search) = search {
            self.install_block_layout(id, search.best().clone(), true);
            self.coaster_mut(id)
                .expect("coaster checked above")
                .block_placement_search = Some(search);
        }
        Ok(())
    }

    pub fn construction_can_test(&self, id: CoasterId) -> bool {
        self.coaster(id).is_some_and(|coaster| {
            coaster.construction.is_closed()
                && coaster.construction.circuit().is_ok_and(|circuit| {
                    circuit.length() > coaster.construction.model().train.length_metres
                })
        })
    }

    pub fn construction_set_train_count(
        &mut self,
        id: CoasterId,
        count: usize,
    ) -> Result<(), BlockCommandError> {
        let coaster = self
            .coaster(id)
            .ok_or(BlockCommandError::InvalidTrainCount)?;
        if count == 0
            || coaster
                .block_layout
                .as_ref()
                .is_none_or(|layout| count > layout.capacity())
        {
            return Err(BlockCommandError::InvalidTrainCount);
        }
        self.coaster_mut(id)
            .expect("coaster checked above")
            .desired_train_count = Some(count);
        Ok(())
    }

    pub fn construction_set_open(
        &mut self,
        id: CoasterId,
        open: bool,
    ) -> Result<(), BlockCommandError> {
        if !open {
            self.close_live_coaster(id);
            self.construction_message.clear();
            return Ok(());
        }
        if self.coaster(id).is_some_and(|coaster| coaster.coaster_open) {
            return Ok(());
        }
        if !self.construction_can_test(id) {
            return Err(BlockCommandError::CompleteCircuitRequired);
        }
        let (facility, circuit, train, block_layout_missing) = {
            let coaster = self.coaster(id).expect("complete coaster exists");
            let construction = &coaster.construction;
            let Some(facility) = coaster_facility(
                &self.world,
                construction,
                COASTER_PLACE_ID - id.0,
                coaster.coaster_price,
                coaster.coaster_ratings,
            ) else {
                return Err(BlockCommandError::MissingStationAccess);
            };
            let circuit = construction
                .circuit()
                .expect("complete circuit checked above");
            (
                facility,
                circuit,
                construction.model().train.clone(),
                coaster.block_layout.is_none(),
            )
        };
        if block_layout_missing {
            let config = block_physics_config();
            let layout = match BlockLayout::unverified_baseline(&circuit, &train, config) {
                Ok(layout) => layout,
                Err(reason) => {
                    return Err(BlockCommandError::Validation(reason));
                }
            };
            self.install_block_layout(id, layout, false);
            self.coaster_mut(id)
                .expect("coaster checked above")
                .block_manual_search = Some(
                BlockManualSearch::new(circuit.clone(), train, config, Vec::new())
                    .expect("baseline inputs were validated"),
            );
            self.coaster_mut(id)
                .expect("coaster checked above")
                .block_baseline_validation = true;
        }
        let coaster = self.coaster(id).expect("open coaster exists");
        let model = coaster.construction.model();
        let operation = BlockOperation::new(
            coaster
                .block_layout
                .as_ref()
                .expect("open coaster has a block layout")
                .clone(),
            model,
        );
        let coaster = self.coaster_mut(id).expect("open coaster exists");
        coaster.coaster_operation = Some(operation);
        coaster.coaster_facility = Some(facility);
        coaster.construction_train = None;
        coaster.construction_recorder = None;
        coaster.coaster_open = true;
        self.construction_message.clear();
        Ok(())
    }

    fn coaster_has_active_trains(&self, id: CoasterId) -> bool {
        self.coaster(id)
            .and_then(|coaster| coaster.coaster_operation.as_ref())
            .is_some_and(|operation| operation.trains().next().is_some())
    }

    pub fn construction_circuit_and_train(
        &self,
        id: CoasterId,
    ) -> Option<(Circuit, TrainConfiguration)> {
        let construction = &self.coaster(id)?.construction;
        Some((
            construction.circuit().ok()?,
            construction.model().train.clone(),
        ))
    }

    pub fn clear_block_layout(&mut self, id: CoasterId) {
        let Some(coaster) = self.coaster_mut(id) else {
            return;
        };
        coaster.block_layout = None;
        coaster.desired_train_count = None;
        coaster.block_placement_search = None;
        coaster.block_placement_cache = None;
        coaster.block_apply_best_requested = false;
        coaster.block_manual_search = None;
        coaster.block_baseline_validation = false;
        coaster.block_layout_is_automatic = false;
        coaster.block_message.clear();
        coaster.block_hourly_throughput = 0.0;
        coaster.block_throughput_reason.clear();
        coaster.block_throughput_estimator = None;
        coaster.block_analysis_extra_to_throughput = false;
        coaster.block_throughput_cache.clear();
    }

    pub fn install_block_layout(&mut self, id: CoasterId, layout: BlockLayout, automatic: bool) {
        let Some(coaster) = self.coaster(id) else {
            return;
        };
        let is_open = coaster.coaster_open;
        let model = coaster.construction.model().clone();
        let coaster = self.coaster_mut(id).expect("coaster checked above");
        coaster.block_placement_search = None;
        coaster.block_apply_best_requested = false;
        coaster.block_manual_search = None;
        coaster.block_baseline_validation = false;
        let fingerprint = block_layout_fingerprint(&layout);
        let (throughput, throughput_reason, estimator) = if layout.capacity() == 0 {
            (0.0, "A train could not complete the circuit.".into(), None)
        } else {
            match coaster.block_throughput_cache.get(&fingerprint) {
                Some((throughput, reason)) => (*throughput, reason.clone(), None),
                None => (0.0, String::new(), Some(ThroughputEstimator::new(&layout))),
            }
        };
        if is_open {
            // Release the mutable coaster borrow before recovery changes its operation.
            let _ = coaster;
            self.recover_live_trains(id);
            let coaster = self.coaster_mut(id).expect("coaster checked above");
            coaster.coaster_operation = Some(BlockOperation::new(layout.clone(), &model));
            coaster.block_layout = Some(layout);
            coaster.block_layout_is_automatic = automatic;
            coaster.block_hourly_throughput = throughput;
            coaster.block_throughput_reason = throughput_reason;
            coaster.block_throughput_estimator = estimator;
            coaster.block_message.clear();
            self.construction_message.clear();
            return;
        }
        coaster.block_layout = Some(layout);
        coaster.block_layout_is_automatic = automatic;
        coaster.block_hourly_throughput = throughput;
        coaster.block_throughput_reason = throughput_reason;
        coaster.block_throughput_estimator = estimator;
        coaster.block_message.clear();
        self.construction_message.clear();
    }

    /// Advance pending block analysis by at most the requested number of physics steps.
    /// Returns true once analysis has finished. Large requests are capped per call.
    pub fn construction_advance_block_analysis(
        &mut self,
        id: CoasterId,
        work_steps: u32,
    ) -> (bool, Option<BlockCommandError>) {
        self.advance_block_analysis(id, work_steps, false)
    }

    /// Advance only automatic placement by 1..=1,000 steps for frozen-mode tests.
    pub fn construction_advance_block_placement_for_test(
        &mut self,
        id: CoasterId,
        work_steps: u32,
    ) {
        assert!(
            (1..=1_000).contains(&work_steps),
            "invalid test placement budget"
        );
        assert!(
            self.coaster(id)
                .is_some_and(|coaster| coaster.block_placement_search.is_some()),
            "automatic placement must be searching"
        );
        assert!(
            !self
                .coaster(id)
                .is_some_and(|coaster| coaster.block_apply_best_requested),
            "automatic placement must not be stopping"
        );
        assert!(
            self.coaster(id)
                .is_none_or(|coaster| coaster.block_manual_search.is_none()),
            "manual validation must not be active"
        );
        self.advance_block_analysis(id, work_steps, true);
    }

    fn advance_block_analysis(
        &mut self,
        id: CoasterId,
        work_steps: u32,
        placement_only: bool,
    ) -> (bool, Option<BlockCommandError>) {
        let Some(coaster) = self.coaster_mut(id) else {
            return (true, None);
        };
        let has_active_trains = coaster
            .coaster_operation
            .as_ref()
            .is_some_and(|operation| operation.trains().next().is_some());
        let mut validation_error = None;
        let budget = work_steps.min(1_000) as usize;
        let has_layout_search =
            coaster.block_manual_search.is_some() || coaster.block_placement_search.is_some();
        let search_budget = if placement_only {
            budget
        } else if has_layout_search && coaster.block_throughput_estimator.is_some() {
            let extra_to_throughput = coaster.block_analysis_extra_to_throughput && budget % 2 == 1;
            if budget % 2 == 1 {
                coaster.block_analysis_extra_to_throughput =
                    !coaster.block_analysis_extra_to_throughput;
            }
            (budget + usize::from(budget % 2 == 1 && !extra_to_throughput)) / 2
        } else if has_layout_search {
            budget
        } else {
            0
        };
        let mut search_steps = 0;
        if let Some(mut search) = coaster.block_manual_search.take() {
            let before = search.work_completed();
            search.advance(search_budget);
            search_steps = search.work_completed() - before;
            match search.result() {
                Some(Ok(layout)) => {
                    let installed = coaster
                        .block_layout
                        .as_ref()
                        .expect("validation has an installed layout");
                    if block_layout_fingerprint(layout) == block_layout_fingerprint(installed) {
                        coaster.block_layout = Some(layout.clone());
                        coaster.block_layout_is_automatic = false;
                        coaster.block_baseline_validation = false;
                    } else if has_active_trains {
                        coaster.block_manual_search = Some(search);
                    } else {
                        let layout = layout.clone();
                        let _ = coaster;
                        self.install_block_layout(id, layout, false);
                    }
                }
                Some(Err(reason)) => {
                    coaster.block_baseline_validation = false;
                    validation_error = Some(BlockCommandError::Validation(reason));
                }
                None => {
                    coaster.block_manual_search = Some(search);
                }
            }
        } else if let Some(mut search) = coaster.block_placement_search.take() {
            if !coaster.block_apply_best_requested {
                let before = search.work_completed();
                search.advance(search_budget);
                search_steps = search.work_completed() - before;
            }
            let complete = search.is_complete() || coaster.block_apply_best_requested;
            let best = search.best();
            let installed = coaster
                .block_layout
                .as_ref()
                .expect("search has an installed layout");
            let improves_capacity = best.capacity() > installed.capacity();
            let same_layout = block_layout_fingerprint(best) == block_layout_fingerprint(installed);
            let verifies_installed = same_layout
                && !installed.capacity_search_complete()
                && best.capacity_search_complete();
            let should_install = coaster.block_apply_best_requested
                || improves_capacity
                || verifies_installed
                || confirmed_stalled_baseline(best, installed)
                || (complete && best.capacity() >= installed.capacity());
            if should_install && !same_layout && has_active_trains {
                coaster.block_placement_search = Some(search);
            } else {
                if should_install {
                    let layout = best.clone();
                    if same_layout {
                        coaster.block_layout = Some(layout);
                        coaster.block_layout_is_automatic = true;
                    } else {
                        let _ = coaster;
                        self.install_block_layout(id, layout, true);
                    }
                }
                if complete {
                    let coaster = self
                        .coaster_mut(id)
                        .expect("coaster exists during analysis");
                    coaster.block_apply_best_requested = false;
                    if search.is_complete() {
                        coaster.block_placement_cache = Some(search);
                    }
                } else {
                    self.coaster_mut(id)
                        .expect("coaster exists during analysis")
                        .block_placement_search = Some(search);
                }
            }
        }
        let coaster = self
            .coaster_mut(id)
            .expect("coaster exists during analysis");
        if !placement_only && let Some(mut estimator) = coaster.block_throughput_estimator.take() {
            if let Some(result) = estimator.advance(budget - search_steps) {
                let estimate = match result {
                    Ok(throughput) => (throughput, String::new()),
                    Err(reason) => (0.0, reason.to_owned()),
                };
                coaster.block_hourly_throughput = estimate.0;
                coaster.block_throughput_reason.clone_from(&estimate.1);
                let layout = coaster
                    .block_layout
                    .as_ref()
                    .expect("estimator has a layout");
                coaster
                    .block_throughput_cache
                    .insert(block_layout_fingerprint(layout), estimate);
            } else {
                coaster.block_throughput_estimator = Some(estimator);
            }
        }
        (
            coaster.block_manual_search.is_none()
                && coaster.block_placement_search.is_none()
                && coaster.block_throughput_estimator.is_none(),
            validation_error,
        )
    }

    pub fn construction_auto_blocks(&mut self, id: CoasterId) -> Result<(), BlockCommandError> {
        if let Some(coaster) = self.coaster_mut(id)
            && coaster.block_placement_search.is_some()
        {
            coaster.block_message.clear();
            self.construction_message.clear();
            return Ok(());
        }
        let Some((circuit, train)) = self.construction_circuit_and_train(id) else {
            return Err(BlockCommandError::ClosedCircuitRequiredForPlacement);
        };
        let config = block_physics_config();
        let search = match self
            .coaster_mut(id)
            .expect("construction exists for block analysis")
            .block_placement_cache
            .take()
        {
            Some(search) => Ok(search),
            None => BlockPlacementSearch::new(circuit, train, config),
        };
        match search {
            Ok(search) => {
                {
                    let coaster = self
                        .coaster_mut(id)
                        .expect("construction exists for block analysis");
                    coaster.block_manual_search = None;
                    coaster.block_baseline_validation = false;
                    coaster.block_apply_best_requested = false;
                }
                if self
                    .coaster(id)
                    .is_some_and(|coaster| coaster.block_layout.is_none())
                {
                    self.install_block_layout(id, search.best().clone(), true);
                }
                self.coaster_mut(id)
                    .expect("construction exists for block analysis")
                    .block_placement_search = Some(search);
                let (_, error) = self.construction_advance_block_analysis(id, 0);
                if let Some(error) = error {
                    return Err(error);
                }
                self.coaster_mut(id)
                    .expect("construction exists for block analysis")
                    .block_message
                    .clear();
                self.construction_message.clear();
                Ok(())
            }
            Err(reason) => Err(BlockCommandError::Validation(reason)),
        }
    }

    /// Install the best verified placement found so far and stop searching.
    pub fn construction_apply_best_block_sections(
        &mut self,
        id: CoasterId,
    ) -> Result<(), BlockCommandError> {
        let Some(search) = self
            .coaster_mut(id)
            .and_then(|coaster| coaster.block_placement_search.take())
        else {
            return Err(BlockCommandError::NoAutomaticSearch);
        };
        if self.coaster_has_active_trains(id) {
            let coaster = self.coaster_mut(id).expect("coaster exists");
            coaster.block_apply_best_requested = true;
            coaster.block_placement_search = Some(search);
            return Ok(());
        }
        self.install_block_layout(id, search.best().clone(), true);
        if search.is_complete() {
            self.coaster_mut(id)
                .expect("coaster exists")
                .block_placement_cache = Some(search);
        }
        Ok(())
    }

    pub fn construction_add_block_section(
        &mut self,
        id: CoasterId,
        boundary_metres: f64,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = self
            .coaster(id)
            .and_then(|coaster| coaster.block_layout.as_ref())
        else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        boundaries.push(boundary_metres);
        self.construction_set_block_sections(id, boundaries)
    }

    pub fn construction_move_block_section(
        &mut self,
        id: CoasterId,
        index: usize,
        boundary_metres: f64,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = self
            .coaster(id)
            .and_then(|coaster| coaster.block_layout.as_ref())
        else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        let Some(boundary) = boundaries.get_mut(index) else {
            return Err(BlockCommandError::IndexOutOfRange);
        };
        *boundary = boundary_metres;
        self.construction_set_block_sections(id, boundaries)
    }

    pub fn construction_remove_block_section(
        &mut self,
        id: CoasterId,
        index: usize,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = self
            .coaster(id)
            .and_then(|coaster| coaster.block_layout.as_ref())
        else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        if index >= boundaries.len() {
            return Err(BlockCommandError::IndexOutOfRange);
        }
        boundaries.remove(index);
        self.construction_set_block_sections(id, boundaries)
    }

    pub fn construction_block_candidates(&self, id: CoasterId) -> Vec<f64> {
        let Some((circuit, train)) = self.construction_circuit_and_train(id) else {
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

    pub fn construction_set_block_sections(
        &mut self,
        id: CoasterId,
        boundaries: Vec<f64>,
    ) -> Result<(), BlockCommandError> {
        let Some((circuit, train)) = self.construction_circuit_and_train(id) else {
            return Err(BlockCommandError::ClosedCircuitRequiredForEditing);
        };
        match BlockManualSearch::new(circuit, train, block_physics_config(), boundaries) {
            Ok(search) => {
                let coaster = self
                    .coaster_mut(id)
                    .expect("construction exists for block analysis");
                coaster.block_placement_search = None;
                coaster.block_apply_best_requested = false;
                coaster.block_baseline_validation = false;
                coaster.block_manual_search = Some(search);
                coaster.block_message.clear();
                self.construction_message.clear();
                Ok(())
            }
            Err(reason) => Err(BlockCommandError::Validation(reason)),
        }
    }

    pub fn construction_install_block_layout(
        &mut self,
        id: CoasterId,
        sections: Vec<BlockSectionPlacement>,
        capacity: usize,
        capacity_search_complete: bool,
        placement_search_complete: bool,
    ) -> Result<(), BlockCommandError> {
        if self.coaster(id).is_some_and(|coaster| coaster.coaster_open) {
            return Err(BlockCommandError::RecordedLayoutWhileOpen);
        }
        let Some((circuit, train)) = self.construction_circuit_and_train(id) else {
            return Err(BlockCommandError::ClosedCircuitRequiredForEditing);
        };
        let layout = BlockLayout::from_explicit_result(
            &circuit,
            &train,
            block_physics_config(),
            &sections,
            capacity,
            capacity_search_complete,
            placement_search_complete,
        )
        .map_err(BlockCommandError::Validation)?;
        let desired_train_count = self
            .coaster(id)
            .expect("construction exists for block analysis")
            .desired_train_count;
        if desired_train_count.is_some_and(|count| count > layout.capacity()) {
            return Err(BlockCommandError::RecordedTrainCountExceedsCapacity);
        }
        self.clear_block_layout(id);
        self.coaster_mut(id)
            .expect("construction exists for block analysis")
            .desired_train_count = desired_train_count;
        self.install_block_layout(id, layout, true);
        Ok(())
    }
}
