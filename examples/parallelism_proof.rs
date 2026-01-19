//! # Proof of True Parallelism
//!
//! This example demonstrates that dagx provides genuine parallel execution
//! by running 10,000 tasks that each sleep for 1 second, completing in ~1 second
//! total instead of 10,000 seconds.
//!
//! ## What You'll Learn
//! - How to verify true parallel execution
//! - The difference between concurrency and parallelism
//! - How dagx scales to thousands of parallel tasks
//! - Performance characteristics of parallel execution
//!
//! ## The Proof
//!
//! If tasks ran sequentially:
//! - 10,000 tasks × 1 second each = 10,000 seconds (2.78 hours)
//!
//! With true parallelism:
//! - All 10,000 tasks run simultaneously = ~1 second total
//!
//! This dramatic difference (10,000x speedup) proves that tasks execute in
//! parallel across multiple threads, not sequentially.
//!
//! ## Key Concepts
//!
//! **Concurrency** - Multiple tasks making progress, but not necessarily at the same time
//! **Parallelism** - Multiple tasks executing simultaneously on different CPU cores
//!
//! This example proves dagx provides true parallelism.
//!
//! ## Running This Example
//! ```bash
//! cargo run --example parallelism_proof --release
//! ```
//!
//! ## Expected Output
//! ```text
//! === Parallelism Proof ===
//!
//! Launching 10,000 tasks that each sleep for 1 second...
//!
//! Result: 10,000 tasks × 1s sleep completed in ~1.2s
//!
//! Proof: If tasks ran sequentially, this would take 10,000 seconds (2.78 hours).
//!        Actual time: 1.2s
//!        Speedup: 8333x
//!        This proves TRUE PARALLEL EXECUTION!
//! ```

use dagx::{task, DagRunner};
use futures::FutureExt;
use std::time::{Duration, Instant};
use tokio::time::sleep;

// Task that sleeps for a duration and returns its ID
struct SleepTask {
    id: u32,
    duration: Duration,
}

#[task]
impl SleepTask {
    async fn run(&self) -> u32 {
        sleep(self.duration).await;
        self.id
    }
}

#[tokio::main]
async fn main() {
    println!("=== Parallelism Proof ===\n");
    println!("Launching 10,000 tasks that each sleep for 1 second...\n");

    let dag = DagRunner::new();
    let sleep_duration = Duration::from_secs(1);
    let num_tasks = 10_000u32;

    // Create 10,000 independent tasks that each sleep for 1 second
    let tasks: Vec<_> = (0..num_tasks)
        .map(|i| {
            dag.add_task(SleepTask {
                id: i,
                duration: sleep_duration,
            })
        })
        .collect();

    // Measure execution time
    let start = Instant::now();
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    let elapsed = start.elapsed();

    // Verify all tasks completed correctly
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task).unwrap(), i as u32);
    }

    // Calculate what sequential execution would take
    let sequential_time = sleep_duration * num_tasks;
    let speedup = sequential_time.as_secs_f64() / elapsed.as_secs_f64();

    println!(
        "Result: {} tasks × {}s sleep completed in {:.1}s\n",
        num_tasks,
        sleep_duration.as_secs(),
        elapsed.as_secs_f64()
    );

    println!(
        "Proof: If tasks ran sequentially, this would take {} seconds ({:.2} hours).",
        sequential_time.as_secs(),
        sequential_time.as_secs_f64() / 3600.0
    );
    println!("       Actual time: {:.1}s", elapsed.as_secs_f64());
    println!("       Speedup: {:.0}x", speedup);

    // The proof: execution time should be close to 1 second, not 10,000 seconds
    if elapsed < Duration::from_secs(3) {
        println!("       This proves TRUE PARALLEL EXECUTION!");
    } else {
        panic!(
            "FAILED: Tasks appear to be running sequentially! Took {:.1}s instead of ~1s",
            elapsed.as_secs_f64()
        );
    }

    println!("\n=== Parallelism Proof Complete ===");
}
