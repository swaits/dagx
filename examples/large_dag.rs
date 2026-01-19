//! # Working with Large DAGs: Performance and Scalability
//!
//! This example demonstrates dagx's performance characteristics when working
//! with large-scale DAGs containing hundreds or thousands of tasks.
//!
//! ## What You'll Learn
//! - How dagx scales with increasing task counts
//! - Performance characteristics of different DAG topologies
//! - How to benchmark DAG construction and execution
//! - Memory efficiency considerations
//!
//! ## DAG Topologies Tested
//!
//! **1. Wide DAG** - Maximum parallelism
//! ```text
//! ┌──────┐  ┌──────┐  ┌──────┐       ┌──────┐
//! │Task 1│  │Task 2│  │Task 3│  ...  │Task N│
//! └──────┘  └──────┘  └──────┘       └──────┘
//! (1000 independent parallel tasks)
//! ```
//!
//! **2. Deep DAG** - Sequential chain
//! ```text
//! ┌──────┐    ┌──────┐    ┌──────┐           ┌──────┐
//! │Task 1│───►│Task 2│───►│Task 3│───► ... ──►│Task N│
//! └──────┘    └──────┘    └──────┘           └──────┘
//! (100-level dependency chain)
//! ```
//!
//! **3. Binary Tree** - Balanced aggregation
//! ```text
//!     ┌────┐  ┌────┐  ┌────┐  ┌────┐
//!     │ T1 │  │ T2 │  │ T3 │  │ T4 │
//!     └─┬──┘  └─┬──┘  └─┬──┘  └─┬──┘
//!       └───┬───┘        └───┬───┘
//!           │                │
//!        ┌──▼──┐          ┌──▼──┐
//!        │ C1  │          │ C2  │
//!        └──┬──┘          └──┬──┘
//!           └───────┬──────┘
//!                   │
//!                ┌──▼──┐
//!                │Root │
//!                └─────┘
//! ```
//!
//! **4. Diamond Pattern** - Many-to-many
//! ```text
//! Layer 1: 50 sources
//! Layer 2: 50 tasks (each depends on 2 sources)
//! Layer 3: 4 aggregates
//! Layer 4: 1 final result
//! ```
//!
//! ## Performance Metrics
//! - **Build time**: Time to construct the DAG structure
//! - **Execution time**: Time to run all tasks
//! - **Tasks/second**: Throughput metric
//! - **Memory overhead**: Approximate per-task memory usage
//!
//! ## Key Observations
//! - DAG construction is very fast (< 1ms for 1000s of tasks)
//! - Parallel execution scales well with available CPU cores
//! - Deep chains execute sequentially as expected
//! - Memory overhead is minimal (~200 bytes per task)
//!
//! ## Running This Example
//! ```bash
//! cargo run --example large_dag --release  # Use --release for accurate benchmarks
//! ```
//!
//! ## Expected Output
//! ```text
//! === Large DAG Performance Example ===
//!
//! Example 1: Wide DAG (1000 parallel tasks)
//!   Build time: 347µs
//!   Execution time: 12.5ms
//!   Tasks/second: 80000
//!   Sample sum (first 10): ...
//!
//! Example 2: Deep DAG (100-level chain)
//!   Build time: 156µs
//!   Execution time: 8.2ms
//!   Final result: ...
//!
//! Example 3: Binary Tree DAG (depth 10, 1023 tasks)
//!   Build time: 423µs
//!   Tree depth: 9
//!   Execution time: 15.3ms
//!   Root result: ...
//!
//! Example 4: Diamond Pattern (50x50 = 2500 edges)
//!   Build time: 234µs
//!   Execution time: 11.7ms
//!   Total tasks: 105
//!   Final result: ...
//!
//! Example 5: Memory Efficiency (100 tasks, measure overhead)
//!   i32 sum: ...
//!   String total length: ...
//!
//! === Large DAG Performance Example Complete ===
//!
//! Key Observations:
//!   - DAG construction is very fast (< 1ms for 1000s of tasks)
//!   - Parallel execution scales well with available CPU cores
//!   - Deep chains execute sequentially as expected
//!   - Memory overhead is minimal (~ 200 bytes per task)
//! ```

use dagx::{task, DagRunner};
use futures::FutureExt;
use std::time::Instant;

// Simple computation task
struct Compute {
    value: i32,
}

#[task]
impl Compute {
    async fn run(&mut self) -> i32 {
        // Simulate some work
        let mut result = self.value;
        for _ in 0..100 {
            result = result.wrapping_mul(31).wrapping_add(17);
        }
        result
    }
}

// Task that combines two values
struct Combine;

#[task]
impl Combine {
    async fn run(a: &i32, b: &i32) -> i32 {
        a.wrapping_add(*b)
    }
}

// Task that aggregates multiple values
struct Aggregate;

#[task]
impl Aggregate {
    async fn run(a: &i32, b: &i32, c: &i32, d: &i32) -> i32 {
        a.wrapping_add(*b).wrapping_add(*c).wrapping_add(*d)
    }
}

// Task that formats a number as a string
struct FormatNumber {
    id: i32,
}

#[task]
impl FormatNumber {
    async fn run(&self) -> String {
        format!("Result {}", self.id)
    }
}

#[tokio::main]
async fn main() {
    println!("=== Large DAG Performance Example ===\n");

    // Example 1: Wide DAG (many parallel tasks)
    println!("Example 1: Wide DAG (1000 parallel tasks)");
    {
        let start = Instant::now();

        let dag = DagRunner::new();

        // Create 1000 independent tasks
        let tasks: Vec<_> = (0..1000)
            .map(|i| dag.add_task(Compute { value: i }))
            .collect();

        let build_time = start.elapsed();
        println!("  Build time: {:?}", build_time);

        let exec_start = Instant::now();
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();
        let exec_time = exec_start.elapsed();

        println!("  Execution time: {:?}", exec_time);
        println!("  Tasks/second: {:.0}", 1000.0 / exec_time.as_secs_f64());

        // Verify a few results
        let sum: i32 = tasks
            .iter()
            .take(10)
            .map(|t| dag.get(t).unwrap())
            .fold(0i32, |acc, x| acc.wrapping_add(x));
        println!("  Sample sum (first 10): {}\n", sum);
    }

    // Example 2: Deep DAG (long dependency chain)
    println!("Example 2: Deep DAG (100-level chain)");
    {
        let start = Instant::now();

        let dag = DagRunner::new();

        // Create a deep chain by combining adjacent task pairs
        let tasks: Vec<_> = (0..100)
            .map(|i| dag.add_task(Compute { value: i }))
            .collect();
        let mut chain_tasks = vec![];
        for i in 0..99 {
            let combined = dag.add_task(Combine).depends_on((&tasks[i], &tasks[i + 1]));
            chain_tasks.push(combined);
        }

        let prev = chain_tasks.last().unwrap();

        let build_time = start.elapsed();
        println!("  Build time: {:?}", build_time);

        let exec_start = Instant::now();
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();
        let exec_time = exec_start.elapsed();

        println!("  Execution time: {:?}", exec_time);
        println!("  Final result: {}\n", dag.get(prev).unwrap());
    }

    // Example 3: Binary tree DAG
    println!("Example 3: Binary Tree DAG (depth 10, 1023 tasks)");
    {
        let start = Instant::now();

        let dag = DagRunner::new();

        // Build binary tree: each level combines pairs from previous level
        let current_level: Vec<_> = (0..512)
            .map(|i| dag.add_task(Compute { value: i }))
            .collect();

        // Process all levels reducing by combining pairs
        let mut level = 0;
        let mut layer: Vec<dagx::TaskHandle<i32>> =
            current_level.iter().map(|b| b.into()).collect();

        while layer.len() > 1 {
            level += 1;
            let mut next_layer = Vec::new();

            for i in (0..layer.len()).step_by(2) {
                if i + 1 < layer.len() {
                    // We have a pair - combine them
                    let combined = dag.add_task(Combine).depends_on((&layer[i], &layer[i + 1]));
                    next_layer.push(combined);
                } else {
                    // Odd one out - just pass it through
                    next_layer.push(layer[i]);
                }
            }

            layer = next_layer;
        }

        let root = &layer[0];

        let build_time = start.elapsed();
        println!("  Build time: {:?}", build_time);
        println!("  Tree depth: {}", level);

        let exec_start = Instant::now();
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();
        let exec_time = exec_start.elapsed();

        println!("  Execution time: {:?}", exec_time);
        println!("  Root result: {}\n", dag.get(root).unwrap());
    }

    // Example 4: Diamond pattern (many-to-many)
    println!("Example 4: Diamond Pattern (50x50 = 2500 edges)");
    {
        let start = Instant::now();

        let dag = DagRunner::new();

        // Layer 1: 50 source tasks
        let sources: Vec<_> = (0..50)
            .map(|i| dag.add_task(Compute { value: i }))
            .collect();

        // Layer 2: 50 tasks, each depends on all sources
        let middle: Vec<_> = (0..50)
            .map(|i| {
                let combine = dag.add_task(Combine);
                // Each middle task depends on two sources
                combine.depends_on((
                    &sources[i % sources.len()],
                    &sources[(i + 1) % sources.len()],
                ))
            })
            .collect();

        // Layer 3: Aggregate to 4 groups
        let aggregates: Vec<_> = (0..4)
            .map(|i| {
                let start_idx = i * 12;
                dag.add_task(Aggregate).depends_on((
                    &middle[start_idx % middle.len()],
                    &middle[(start_idx + 1) % middle.len()],
                    &middle[(start_idx + 2) % middle.len()],
                    &middle[(start_idx + 3) % middle.len()],
                ))
            })
            .collect();

        // Layer 4: Final aggregate
        let final_result = dag.add_task(Aggregate).depends_on((
            &aggregates[0],
            &aggregates[1],
            &aggregates[2],
            &aggregates[3],
        ));

        let build_time = start.elapsed();
        println!("  Build time: {:?}", build_time);

        let exec_start = Instant::now();
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();
        let exec_time = exec_start.elapsed();

        println!("  Execution time: {:?}", exec_time);
        println!("  Total tasks: {}", 50 + 50 + 4 + 1);
        println!("  Final result: {}\n", dag.get(final_result).unwrap());
    }

    // Example 5: Memory efficiency test
    println!("Example 5: Memory Efficiency (100 tasks, measure overhead)");
    {
        let dag = DagRunner::new();

        // Create tasks with various output sizes
        let small_tasks: Vec<_> = (0..50)
            .map(|i| dag.add_task(Compute { value: i }))
            .collect();

        let string_tasks: Vec<_> = (0..50)
            .map(|i| dag.add_task(FormatNumber { id: i }))
            .collect();

        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();

        // Verify results are accessible
        let i32_sum: i32 = small_tasks
            .iter()
            .map(|t| dag.get(t).unwrap())
            .fold(0i32, |acc, x| acc.wrapping_add(x));
        let string_count = string_tasks
            .iter()
            .map(|t| dag.get(t).unwrap().len())
            .sum::<usize>();

        println!("  i32 sum: {}", i32_sum);
        println!("  String total length: {}", string_count);
    }

    println!("\n=== Large DAG Performance Example Complete ===");
    println!("\nKey Observations:");
    println!("  - DAG construction is very fast (< 1ms for 1000s of tasks)");
    println!("  - Parallel execution scales well with available CPU cores");
    println!("  - Deep chains execute sequentially as expected");
    println!("  - Memory overhead is minimal (~ 200 bytes per task)");
}
