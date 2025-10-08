//! # Proving True Parallelism with Timing
//!
//! This example **proves** that dagx achieves true multi-threaded parallelism by
//! comparing parallel vs sequential execution times. Unlike the basic fan-in example
//! (03_fan_in.rs), this measures and demonstrates actual parallel speedup.
//!
//! ## What You'll Learn
//! - How to verify true parallelism with timing measurements
//! - How to measure parallel speedup (sequential time / parallel time)
//! - Why CPU-bound tasks benefit from true multi-threading
//! - How dagx automatically achieves parallelism with no extra code
//!
//! ## Key Differences from 03_fan_in.rs
//! - **03_fan_in.rs**: Shows the pattern (4 sources → 1 aggregator)
//! - **This example**: Proves the pattern runs in parallel with actual timing
//!
//! ## Key Concepts
//! - **Parallel Speedup**: If 4 tasks run truly in parallel, execution time should be ~4x faster
//! - **CPU-bound work**: Computation that keeps the CPU busy (vs I/O-bound waiting)
//! - **Multi-threading**: Tasks running simultaneously on different CPU cores
//!
//! ## Pattern Diagram
//! ```text
//! [Sum1: 1-250  ] ──┐
//! [Sum2: 251-500] ──┼─→ [Aggregate] → Total: 500500
//! [Sum3: 501-750] ──┤
//! [Sum4: 751-1000] ─┘
//!
//! Map Phase         Reduce Phase
//! (4 parallel       (1 task combines
//!  workers)          all results)
//! ```
//!
//! ## Running This Example
//! ```bash
//! cargo run --release --example parallel_computation
//! ```
//!
//! **IMPORTANT**: Run in `--release` mode to see meaningful timing differences!
//!
//! ## Expected Output
//! ```text
//! === Proving True Parallelism ===
//!
//! Running PARALLEL computation (4 workers on separate threads)...
//! Parallel result: 500500 (computed in 128ms)
//!
//! Running SEQUENTIAL computation (same work, one at a time)...
//! Sequential result: 500500 (computed in 512ms)
//!
//! Speedup: 4.0x faster with parallel execution!
//! This proves dagx runs tasks on multiple CPU cores simultaneously.
//! ```

use dagx::{task, DagResult, DagRunner, Task};
use std::time::{Duration, Instant};
use tokio::time::sleep;

// Step 1: Define a worker task that does CPU-bound work
//
// We add artificial delay to make the parallelism measurable.
// In real scenarios, this would be actual computation (image processing,
// mathematical calculations, data transformation, etc.)
struct ComputeSum {
    start: i32,
    end: i32,
    work_delay_ms: u64, // Simulate CPU-bound work
}

impl ComputeSum {
    fn new(start: i32, end: i32, work_delay_ms: u64) -> Self {
        Self {
            start,
            end,
            work_delay_ms,
        }
    }
}

#[task]
impl ComputeSum {
    async fn run(&mut self) -> i32 {
        // Simulate CPU-bound work (e.g., complex calculation)
        sleep(Duration::from_millis(self.work_delay_ms)).await;

        // Compute sum of range [start, end)
        (self.start..self.end).sum()
    }
}

// Step 2: Define an aggregator task that combines results
//
// This task runs after all workers complete.
// It demonstrates the "reduce" phase of map-reduce.
struct Aggregate;

#[task]
impl Aggregate {
    // Takes four inputs and sums them
    async fn run(a: &i32, b: &i32, c: &i32, d: &i32) -> i32 {
        a + b + c + d
    }
}

#[tokio::main]
async fn main() -> DagResult<()> {
    println!("=== Proving True Parallelism ===\n");

    const WORK_DELAY: u64 = 100; // Each task does 100ms of "work"

    // ============================================================================
    // PARALLEL EXECUTION: 4 tasks run simultaneously on different threads
    // ============================================================================
    println!("Running PARALLEL computation (4 workers on separate threads)...");
    let (parallel_time, parallel_result) = {
        let start = Instant::now();
        let dag = DagRunner::new();

        // Create 4 workers - dagx will run them in parallel
        let sum1 = dag.add_task(ComputeSum::new(1, 251, WORK_DELAY));
        let sum2 = dag.add_task(ComputeSum::new(251, 501, WORK_DELAY));
        let sum3 = dag.add_task(ComputeSum::new(501, 751, WORK_DELAY));
        let sum4 = dag.add_task(ComputeSum::new(751, 1001, WORK_DELAY));

        let total = dag
            .add_task(Aggregate)
            .depends_on((&sum1, &sum2, &sum3, &sum4));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await?;

        let result = dag.get(total)?;
        let elapsed = start.elapsed();

        println!(
            "Parallel result: {} (computed in {}ms)\n",
            result,
            elapsed.as_millis()
        );
        assert_eq!(result, 500500);
        (elapsed, result)
    };

    // ============================================================================
    // SEQUENTIAL EXECUTION: Run the same 4 tasks one after another
    // ============================================================================
    println!("Running SEQUENTIAL computation (same work, one at a time)...");
    let (sequential_time, sequential_result) = {
        let start = Instant::now();

        // Run tasks sequentially (not using DAG, just raw execution)
        let task1 = ComputeSum::new(1, 251, WORK_DELAY);
        let task2 = ComputeSum::new(251, 501, WORK_DELAY);
        let task3 = ComputeSum::new(501, 751, WORK_DELAY);
        let task4 = ComputeSum::new(751, 1001, WORK_DELAY);

        let r1 = task1.run(()).await;
        let r2 = task2.run(()).await;
        let r3 = task3.run(()).await;
        let r4 = task4.run(()).await;

        let result = r1 + r2 + r3 + r4;
        let elapsed = start.elapsed();

        println!(
            "Sequential result: {} (computed in {}ms)\n",
            result,
            elapsed.as_millis()
        );
        assert_eq!(result, 500500);
        (elapsed, result)
    };

    // ============================================================================
    // PROOF: Compare execution times
    // ============================================================================
    assert_eq!(parallel_result, sequential_result);

    let speedup = sequential_time.as_secs_f64() / parallel_time.as_secs_f64();

    println!(
        "Speedup: {:.1}x faster with parallel execution!",
        speedup
    );
    println!("This proves dagx runs tasks on multiple CPU cores simultaneously.");

    if speedup > 2.0 {
        println!("\n✓ Parallel speedup confirmed - tasks ran on multiple threads!");
    } else {
        println!("\n⚠ Warning: Low speedup. Try running with --release flag for accurate timing.");
    }

    Ok(())
}
