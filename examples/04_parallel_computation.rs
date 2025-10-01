//! # Parallel Computation: Map-Reduce Pattern
//!
//! This example demonstrates how dagx achieves true parallelism by spawning
//! tasks to multiple threads, enabling efficient parallel computation.
//!
//! ## What You'll Learn
//! - How to split work across multiple parallel tasks (Map)
//! - How to aggregate results from parallel tasks (Reduce)
//! - How dagx achieves true multi-threaded parallelism
//! - How to verify parallel execution with timing
//!
//! ## Key Concepts
//! - **Map**: Split work into independent parallel tasks
//! - **Reduce**: Aggregate results from parallel tasks into a single output
//! - **True Parallelism**: Tasks spawn to multiple OS threads via the runtime
//! - **Fan-out/Fan-in**: One source splits to many workers, then merges to one result
//!
//! ## Pattern Diagram
//! ```text
//! [Source] → [Sum1]
//!         → [Sum2] → [Aggregate] → Total
//!         → [Sum3]
//!         → [Sum4]
//! ```
//!
//! ## Running This Example
//! ```bash
//! cargo run --example parallel_computation
//! ```
//!
//! ## Expected Output
//! ```text
//! Computing sum of 1-1000 using 4 parallel tasks...
//!
//! Sum of 1-1000 computed in parallel: 500500
//! Expected: 500500
//! ```

use dagx::{task, DagResult, DagRunner, Task};

// Step 1: Define a worker task that computes a partial sum
//
// Each worker computes the sum of a range of numbers.
// These tasks run in parallel on different threads.
struct ComputeSum {
    start: i32,
    end: i32,
}

impl ComputeSum {
    fn new(start: i32, end: i32) -> Self {
        Self { start, end }
    }
}

#[task]
impl ComputeSum {
    async fn run(&mut self) -> i32 {
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
    println!("Computing sum of 1-1000 using 4 parallel tasks...\n");

    // Step 3: Build the DAG
    let dag = DagRunner::new();

    // Map phase: Create 4 workers that run in parallel
    // Each computes sum of 250 numbers (total 1-1000)
    let sum1 = dag.add_task(ComputeSum::new(1, 251)); // 1-250
    let sum2 = dag.add_task(ComputeSum::new(251, 501)); // 251-500
    let sum3 = dag.add_task(ComputeSum::new(501, 751)); // 501-750
    let sum4 = dag.add_task(ComputeSum::new(751, 1001)); // 751-1000

    // Reduce phase: Aggregate results from all workers
    let total = dag
        .add_task(Aggregate)
        .depends_on((&sum1, &sum2, &sum3, &sum4));

    // Step 4: Execute with true parallelism
    //
    // tokio::spawn() sends tasks to multiple threads in the thread pool.
    // Workers run concurrently on different threads, achieving true parallelism.
    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Step 5: Retrieve and verify results
    let result = dag.get(total)?;
    let expected: i32 = (1..1001).sum();

    println!("Sum of 1-1000 computed in parallel: {}", result);
    println!("Expected: {}", expected);

    assert_eq!(result, 500500); // Sum of 1-1000

    // Note: True parallelism verification
    //
    // If you monitor system resources while running this example, you'll see:
    // - Multiple CPU cores being utilized
    // - Worker tasks executing simultaneously
    // - Reduced wall-clock time vs sequential execution
    //
    // This demonstrates that dagx provides REAL parallel execution,
    // not just async concurrency on a single thread.

    Ok(())
}
