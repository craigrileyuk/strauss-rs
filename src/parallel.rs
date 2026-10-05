//! The worker pool that analyses and lists files in parallel.
//!
//! `parallel` in `strauss.json` (and `--parallel`) says how many workers to
//! use. Work is spread with rayon, which hands out files as workers become free
//! rather than splitting them up front, so a few large files don't leave the
//! other workers idle.

use std::thread;

use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::config::Parallel;

/// The analyser recurses into nested code, so give each worker room for deeply
/// nested files. The main thread's stack is smaller than this, so even a
/// single worker runs in the pool.
const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

/// How many workers to use for `work_items` items of work: no more than there
/// are items, and at least one.
pub fn worker_count(parallel: Parallel, work_items: usize) -> usize {
    let workers = match parallel {
        Parallel::Disabled => 1,
        Parallel::Workers(workers) => workers,
        // Every CPU. PHP Strauss leaves one spare, but its workers are whole
        // processes; here the workers are threads of one process.
        Parallel::Auto => thread::available_parallelism()
            .map(|cpus| cpus.get())
            .unwrap_or(1),
    };

    workers.clamp(1, work_items.max(1))
}

/// A pool of exactly `workers` threads.
pub fn pool(workers: usize) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(workers.max(1))
        .stack_size(WORKER_STACK_SIZE)
        .thread_name(|index| format!("strauss-worker-{index}"))
        .build()
        .expect("could not start the worker threads")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_worker_when_disabled() {
        assert_eq!(worker_count(Parallel::Disabled, 1000), 1);
    }

    #[test]
    fn a_number_of_workers_is_capped_by_the_amount_of_work() {
        assert_eq!(worker_count(Parallel::Workers(8), 1000), 8);
        assert_eq!(worker_count(Parallel::Workers(8), 3), 3);
        assert_eq!(worker_count(Parallel::Workers(8), 0), 1);
    }

    #[test]
    fn automatic_uses_every_available_cpu() {
        let cpus = thread::available_parallelism()
            .map(|cpus| cpus.get())
            .unwrap_or(1);

        assert_eq!(worker_count(Parallel::Auto, 100_000), cpus);
        // Not more workers than there is work.
        assert_eq!(worker_count(Parallel::Auto, 1), 1);
        assert_eq!(worker_count(Parallel::Auto, 0), 1);
    }

    #[test]
    fn the_pool_has_the_requested_number_of_threads() {
        for workers in [1, 3, 8] {
            assert_eq!(pool(workers).current_num_threads(), workers);
        }
    }

    #[test]
    fn workers_have_room_for_deep_recursion() {
        // Recurse well past what the default 2 MiB thread stack allows.
        fn depth(n: usize) -> usize {
            let padding = [n as u8; 256];

            if n == 0 {
                0
            } else {
                1 + depth(n - 1) + padding[0] as usize * 0
            }
        }

        assert_eq!(pool(2).install(|| depth(20_000)), 20_000);
    }

    #[test]
    fn a_pool_runs_work_in_order_of_the_input() {
        let numbers: Vec<usize> = (0..1000).collect();

        let doubled: Vec<usize> = pool(4).install(|| {
            use rayon::prelude::*;

            numbers.par_iter().map(|n| n * 2).collect()
        });

        assert_eq!(doubled, numbers.iter().map(|n| n * 2).collect::<Vec<_>>());
    }
}
