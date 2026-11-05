//! Incremental block layout analysis and installation.

#[cfg(test)]
#[path = "block_error_tests.rs"]
mod error_tests;
#[cfg(test)]
#[path = "block_analysis_tests.rs"]
mod tests;

use super::Park;
use super::tick::COASTER_PLACE_ID;
use super::tick::{
    block_layout_fingerprint, confirmed_stalled_baseline, editable_block_boundaries,
};
use crate::facilities::coaster_facility;
use crate::physics::{BlockOperation, Circuit, PhysicsConfig};
use crate::track::TrainConfiguration;
use crate::track::blocks::{
    BlockLayout, BlockManualSearch, BlockPlacementSearch, ThroughputEstimator, brake_runs,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCommandError {
    InvalidTrainCount,
    CompleteCircuitRequired,
    MissingStationAccess,
    ClosedCircuitRequiredForPlacement,
    ClosedCircuitRequiredForEditing,
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
    pub fn construction_can_test(&self) -> bool {
        self.construction.as_ref().is_some_and(|construction| {
            construction.is_closed()
                && construction.circuit().is_ok_and(|circuit| {
                    circuit.length() > construction.model().train.length_metres
                })
        })
    }

    pub fn construction_set_train_count(&mut self, count: usize) -> Result<(), BlockCommandError> {
        if count == 0
            || self
                .block_layout
                .as_ref()
                .is_none_or(|layout| count > layout.capacity())
        {
            return Err(BlockCommandError::InvalidTrainCount);
        }
        self.desired_train_count = Some(count);
        Ok(())
    }

    pub fn construction_set_open(&mut self, open: bool) -> Result<(), BlockCommandError> {
        if !open {
            self.close_live_coaster();
            self.construction_message.clear();
            return Ok(());
        }
        if self.coaster_open {
            return Ok(());
        }
        if !self.construction_can_test() {
            return Err(BlockCommandError::CompleteCircuitRequired);
        }
        let construction = self.construction.as_ref().expect("complete coaster exists");
        let Some(facility) = coaster_facility(
            &self.world,
            construction,
            COASTER_PLACE_ID,
            self.coaster_price,
            self.coaster_ratings,
        ) else {
            return Err(BlockCommandError::MissingStationAccess);
        };
        let circuit = construction
            .circuit()
            .expect("complete circuit checked above");
        if self.block_layout.is_none() {
            let train = construction.model().train.clone();
            let config = block_physics_config();
            let layout = match BlockLayout::unverified_baseline(&circuit, &train, config) {
                Ok(layout) => layout,
                Err(reason) => {
                    return Err(BlockCommandError::Validation(reason));
                }
            };
            self.install_block_layout(layout, false);
            self.block_manual_search = Some(
                BlockManualSearch::new(circuit.clone(), train, config, Vec::new())
                    .expect("baseline inputs were validated"),
            );
            self.block_baseline_validation = true;
        }
        let operation = BlockOperation::new_with_limits(
            self.block_layout
                .as_ref()
                .expect("open coaster has a block layout")
                .clone(),
            self.construction
                .as_ref()
                .expect("open coaster has construction")
                .model()
                .comfortable_force_limits,
        );
        self.coaster_operation = Some(operation);
        self.coaster_facility = Some(facility);
        self.construction_train = None;
        self.construction_recorder = None;
        self.construction_report = None;
        self.coaster_open = true;
        self.construction_message.clear();
        Ok(())
    }

    fn coaster_has_active_trains(&self) -> bool {
        self.coaster_operation
            .as_ref()
            .is_some_and(|operation| operation.trains().next().is_some())
    }

    pub fn construction_circuit_and_train(&self) -> Option<(Circuit, TrainConfiguration)> {
        let construction = self.construction.as_ref()?;
        Some((
            construction.circuit().ok()?,
            construction.model().train.clone(),
        ))
    }

    pub fn clear_block_layout(&mut self) {
        self.block_layout = None;
        self.desired_train_count = None;
        self.block_placement_search = None;
        self.block_placement_cache = None;
        self.block_apply_best_requested = false;
        self.block_manual_search = None;
        self.block_baseline_validation = false;
        self.block_layout_is_automatic = false;
        self.block_message.clear();
        self.block_hourly_throughput = 0.0;
        self.block_throughput_reason.clear();
        self.block_throughput_estimator = None;
        self.block_analysis_extra_to_throughput = false;
        self.block_throughput_cache.clear();
    }

    pub fn install_block_layout(&mut self, layout: BlockLayout, automatic: bool) {
        self.block_placement_search = None;
        self.block_apply_best_requested = false;
        self.block_manual_search = None;
        self.block_baseline_validation = false;
        let fingerprint = block_layout_fingerprint(&layout);
        let (throughput, throughput_reason, estimator) = if layout.capacity() == 0 {
            (0.0, "A train could not complete the circuit.".into(), None)
        } else {
            match self.block_throughput_cache.get(&fingerprint) {
                Some((throughput, reason)) => (*throughput, reason.clone(), None),
                None => (0.0, String::new(), Some(ThroughputEstimator::new(&layout))),
            }
        };
        if self.coaster_open {
            self.recover_live_trains();
            let limits = self
                .construction
                .as_ref()
                .expect("open coaster has construction")
                .model()
                .comfortable_force_limits;
            self.coaster_operation = Some(BlockOperation::new_with_limits(layout.clone(), limits));
        }
        self.block_layout = Some(layout);
        self.block_layout_is_automatic = automatic;
        self.block_hourly_throughput = throughput;
        self.block_throughput_reason = throughput_reason;
        self.block_throughput_estimator = estimator;
        self.block_message.clear();
        self.construction_message.clear();
    }

    /// Advance pending block analysis by at most the requested number of physics steps.
    /// Returns true once analysis has finished. Large requests are capped per call.
    pub fn construction_advance_block_analysis(
        &mut self,
        work_steps: u32,
    ) -> (bool, Option<BlockCommandError>) {
        self.advance_block_analysis(work_steps, false)
    }

    /// Advance only automatic placement by 1..=1,000 steps for frozen-mode tests.
    pub fn construction_advance_block_placement_for_test(&mut self, work_steps: u32) {
        assert!(
            (1..=1_000).contains(&work_steps),
            "invalid test placement budget"
        );
        assert!(
            self.block_placement_search.is_some(),
            "automatic placement must be searching"
        );
        assert!(
            !self.block_apply_best_requested,
            "automatic placement must not be stopping"
        );
        assert!(
            self.block_manual_search.is_none(),
            "manual validation must not be active"
        );
        self.advance_block_analysis(work_steps, true);
    }

    fn advance_block_analysis(
        &mut self,
        work_steps: u32,
        placement_only: bool,
    ) -> (bool, Option<BlockCommandError>) {
        let mut validation_error = None;
        let budget = work_steps.min(1_000) as usize;
        let has_layout_search =
            self.block_manual_search.is_some() || self.block_placement_search.is_some();
        let search_budget = if placement_only {
            budget
        } else if has_layout_search && self.block_throughput_estimator.is_some() {
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
                    validation_error = Some(BlockCommandError::Validation(reason));
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
        if !placement_only && let Some(mut estimator) = self.block_throughput_estimator.take() {
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
        (
            self.block_manual_search.is_none()
                && self.block_placement_search.is_none()
                && self.block_throughput_estimator.is_none(),
            validation_error,
        )
    }

    pub fn construction_auto_blocks(&mut self) -> Result<(), BlockCommandError> {
        if self.block_placement_search.is_some() {
            self.block_message.clear();
            self.construction_message.clear();
            return Ok(());
        }
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            return Err(BlockCommandError::ClosedCircuitRequiredForPlacement);
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
                let (_, error) = self.construction_advance_block_analysis(0);
                if let Some(error) = error {
                    return Err(error);
                }
                self.block_message.clear();
                self.construction_message.clear();
                Ok(())
            }
            Err(reason) => Err(BlockCommandError::Validation(reason)),
        }
    }

    /// Install the best verified placement found so far and stop searching.
    pub fn construction_apply_best_block_sections(&mut self) -> Result<(), BlockCommandError> {
        let Some(search) = self.block_placement_search.take() else {
            return Err(BlockCommandError::NoAutomaticSearch);
        };
        if self.coaster_has_active_trains() {
            self.block_apply_best_requested = true;
            self.block_placement_search = Some(search);
            return Ok(());
        }
        self.install_block_layout(search.best().clone(), true);
        if search.is_complete() {
            self.block_placement_cache = Some(search);
        }
        Ok(())
    }

    pub fn construction_add_block_section(
        &mut self,
        boundary_metres: f64,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = &self.block_layout else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        boundaries.push(boundary_metres);
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_move_block_section(
        &mut self,
        index: usize,
        boundary_metres: f64,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = &self.block_layout else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        let Some(boundary) = boundaries.get_mut(index) else {
            return Err(BlockCommandError::IndexOutOfRange);
        };
        *boundary = boundary_metres;
        self.construction_set_block_sections(boundaries)
    }

    pub fn construction_remove_block_section(
        &mut self,
        index: usize,
    ) -> Result<(), BlockCommandError> {
        let Some(layout) = &self.block_layout else {
            return Err(BlockCommandError::NoBlockLayout);
        };
        let mut boundaries = editable_block_boundaries(layout);
        if index >= boundaries.len() {
            return Err(BlockCommandError::IndexOutOfRange);
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

    pub fn construction_set_block_sections(
        &mut self,
        boundaries: Vec<f64>,
    ) -> Result<(), BlockCommandError> {
        let Some((circuit, train)) = self.construction_circuit_and_train() else {
            return Err(BlockCommandError::ClosedCircuitRequiredForEditing);
        };
        match BlockManualSearch::new(circuit, train, block_physics_config(), boundaries) {
            Ok(search) => {
                self.block_placement_search = None;
                self.block_apply_best_requested = false;
                self.block_baseline_validation = false;
                self.block_manual_search = Some(search);
                self.block_message.clear();
                self.construction_message.clear();
                Ok(())
            }
            Err(reason) => Err(BlockCommandError::Validation(reason)),
        }
    }
}
