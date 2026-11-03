//! Deterministic executors for per-guest simulation work.

#[cfg(feature = "threads")]
use rayon::prelude::*;

pub trait Executor<T> {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync;
}

pub struct SequentialExecutor {
    partition_size: usize,
}

impl SequentialExecutor {
    pub fn new(partition_size: usize) -> Self {
        assert!(partition_size > 0, "partition size must be positive");
        Self { partition_size }
    }
}

impl<T> Executor<T> for SequentialExecutor {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync,
    {
        let mut output = Vec::with_capacity(agent_count);
        for start in (0..agent_count).step_by(self.partition_size) {
            let end = (start + self.partition_size).min(agent_count);
            output.extend((start..end).map(&process));
        }
        output
    }
}

#[cfg(feature = "threads")]
pub struct ThreadedExecutor {
    pool: rayon::ThreadPool,
    partition_size: usize,
}

#[cfg(feature = "threads")]
impl ThreadedExecutor {
    pub fn new(thread_count: usize, partition_size: usize) -> Self {
        assert!(thread_count > 0, "thread count must be positive");
        assert!(partition_size > 0, "partition size must be positive");
        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .expect("failed to create simulation worker pool"),
            partition_size,
        }
    }
}

#[cfg(feature = "threads")]
impl<T: Send> Executor<T> for ThreadedExecutor {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Sync,
    {
        self.pool.install(|| {
            (0..agent_count)
                .into_par_iter()
                .with_min_len(self.partition_size)
                .map(&process)
                .collect()
        })
    }
}

#[cfg(all(test, feature = "threads"))]
mod tests {
    use super::{Executor, ThreadedExecutor};
    use std::sync::Mutex;

    #[test]
    fn executor_reuses_worker_between_calls() {
        let executor = ThreadedExecutor::new(1, 1);
        let worker_ids = Mutex::new(Vec::new());
        for _ in 0..2 {
            executor.execute(1, |_| {
                worker_ids.lock().unwrap().push(std::thread::current().id());
                0
            });
        }
        let worker_ids = worker_ids.into_inner().unwrap();
        assert_eq!(worker_ids[0], worker_ids[1]);
    }
}
