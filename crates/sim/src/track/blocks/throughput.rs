use crate::physics::{BlockOperation, TIME_STEP};
use crate::state_hash::StateHasher;
use crate::track::blocks::BlockLayout;

/// Incrementally estimates hourly riders from sustained train dispatches.
pub struct ThroughputEstimator {
    operation: BlockOperation,
    max_idle_steps: usize,
    step: usize,
    last_dispatch_step: usize,
    first_measured_step: Option<usize>,
    measured_intervals: usize,
    seats_per_train: usize,
}

impl ThroughputEstimator {
    pub fn new(layout: &BlockLayout) -> Self {
        let max_idle_seconds = (layout.circuit().length() * 4.0 + 60.0).clamp(300.0, 3600.0);
        let mut operation = BlockOperation::new_without_reports(layout.clone());
        while operation.can_dispatch() {
            operation.dispatch();
        }
        Self {
            operation,
            max_idle_steps: (max_idle_seconds / TIME_STEP) as usize,
            step: 0,
            last_dispatch_step: 0,
            first_measured_step: None,
            measured_intervals: 0,
            seats_per_train: usize::from(layout.train().cars)
                * usize::from(layout.train().rows_per_car)
                * usize::from(layout.train().seats_per_row),
        }
    }

    pub fn advance(&mut self, work_steps: usize) -> Option<Result<f64, &'static str>> {
        const MEASURED_INTERVALS: usize = 3;
        for _ in 0..work_steps {
            if self.step - self.last_dispatch_step >= self.max_idle_steps {
                return Some(Err(
                    "The throughput simulation did not observe enough sustained dispatches.",
                ));
            }
            self.step += 1;
            if self
                .operation
                .step(&[])
                .iter()
                .any(|(_, outcome)| outcome.recovered)
            {
                return Some(Err("A train recovered during the throughput simulation."));
            }
            while self.operation.can_dispatch() {
                self.operation.dispatch();
                self.last_dispatch_step = self.step;
                let Some(first_step) = self.first_measured_step else {
                    self.first_measured_step = Some(self.step);
                    continue;
                };
                self.measured_intervals += 1;
                if self.measured_intervals >= MEASURED_INTERVALS && self.step > first_step {
                    let elapsed_seconds = (self.step - first_step) as f64 * TIME_STEP;
                    return Some(Ok(self.seats_per_train as f64
                        * self.measured_intervals as f64
                        * 3600.0
                        / elapsed_seconds));
                }
            }
        }
        None
    }

    pub fn work_steps_completed(&self) -> usize {
        self.step
    }

    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"ThroughputEstimator-v1");
        self.operation.hash_into(hash);
        hash.write_len(self.max_idle_steps);
        hash.write_len(self.step);
        hash.write_len(self.last_dispatch_step);
        hash.write_option(self.first_measured_step, |hash, step| hash.write_len(step));
        hash.write_len(self.measured_intervals);
        hash.write_len(self.seats_per_train);
    }
}

#[cfg(test)]
mod tests {
    use super::ThroughputEstimator;
    use crate::physics::{PhysicsConfig, demo_circuit};
    use crate::state_hash::StateHasher;
    use crate::track::blocks::BlockLayout;
    use crate::track::hybrid_model;

    fn estimator_hash(estimator: &ThroughputEstimator) -> u64 {
        let mut hash = StateHasher::new();
        estimator.hash_into(&mut hash);
        hash.finish()
    }

    #[test]
    fn throughput_estimator_hash_tracks_progress_and_is_chunk_independent() {
        let layout = BlockLayout::unverified_baseline(
            &demo_circuit(),
            &hybrid_model().train,
            PhysicsConfig::default(),
        )
        .unwrap();
        let mut first = ThroughputEstimator::new(&layout);
        let mut second = ThroughputEstimator::new(&layout);

        first.advance(2);
        second.advance(1);
        second.advance(1);
        assert_eq!(estimator_hash(&first), estimator_hash(&second));

        second.advance(1);
        assert_ne!(estimator_hash(&first), estimator_hash(&second));
    }
}
