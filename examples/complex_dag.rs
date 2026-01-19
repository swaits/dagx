//! # Building Complex Workflows: Multi-Layer DAG
//!
//! This example demonstrates how to build complex, real-world DAGs by combining
//! multiple dependency patterns into a cohesive workflow.
//!
//! ## What You'll Learn
//! - How to combine fan-out, fan-in, and diamond patterns in one DAG
//! - How to build multi-layer workflows with intermediate results
//! - How to structure an ETL (Extract-Transform-Load) pipeline
//! - How to pass results through multiple transformation stages
//!
//! ## Workflow Diagram
//! ```text
//!     EXTRACT          TRANSFORM        COMBINE        AGGREGATE
//!   ┌──────────┐     ┌──────────┐
//!   │ LoadData │────►│Transform │────┐
//!   │    A     │     │   (×2)   │    │
//!   └──────────┘     └──────────┘    │
//!                                     ├──►┌─────────┐───►┌─────────┐───►┌─────────┐
//!                                     │   │ Combine │    │ Average │    │ Report  │
//!                                     └──►└─────────┘    └─────────┘    └─────────┘
//!   ┌──────────┐     ┌──────────┐    ┌──►                    │              │
//!   │ LoadData │────►│Transform │────┘                       │              │
//!   │    B     │     │   (×3)   │                            └──────────────┘
//!   └──────────┘     └──────────┘
//! ```
//!
//! ## Key Concepts
//! - **Multi-layer execution**: Tasks organized into logical stages (Extract → Transform → Combine)
//! - **Many-to-many dependencies**: The Combine task depends on both Transform tasks
//! - **Result reuse**: The combined result is used by both Average and Report tasks
//! - **Diamond pattern**: Multiple paths from sources converge and then diverge again
//!
//! ## Use Cases
//! - ETL pipelines with multiple data sources
//! - Multi-stage data processing workflows
//! - Complex business logic with branching and merging
//! - Data aggregation and reporting systems
//!
//! ## Running This Example
//! ```bash
//! cargo run --example complex_dag
//! ```
//!
//! ## Expected Output
//! ```text
//! Building ETL pipeline...
//!
//! Executing pipeline...
//!
//! Loading data from source A
//! Loading data from source B
//! Transforming: 6 * 2 = 12
//! Transforming: 15 * 3 = 45
//! Combining datasets: 12 + 45 = 57
//! Computing average: 57 / 6 = 9.50
//!
//! ETL Pipeline Results:
//!   Items: 6
//!   Total: 57
//!   Average: 9.50
//!
//! Calculation:
//!   Source A: sum=6, transformed (x2) = 12
//!   Source B: sum=15, transformed (x3) = 45
//!   Combined: 6 items, sum=57, avg=9.50
//! ```

use dagx::{task, DagResult, DagRunner};
use futures::FutureExt;

// Extract: Load data from different sources
struct LoadData {
    source: &'static str,
}

impl LoadData {
    fn new(source: &'static str) -> Self {
        Self { source }
    }
}

#[task]
impl LoadData {
    async fn run(&mut self) -> i32 {
        // Simulate loading and summing data
        println!("Loading data from source {}", self.source);
        match self.source {
            "A" => 6,  // sum of [1, 2, 3]
            "B" => 15, // sum of [4, 5, 6]
            _ => 0,
        }
    }
}

// Transform: Process data with a multiplier
struct Transform {
    multiplier: i32,
}

impl Transform {
    fn new(multiplier: i32) -> Self {
        Self { multiplier }
    }
}

#[task]
impl Transform {
    async fn run(&mut self, sum: &i32) -> i32 {
        let result = sum * self.multiplier;
        println!("Transforming: {} * {} = {}", sum, self.multiplier, result);
        result
    }
}

// Combine: Merge two datasets
struct Combine;

#[task]
impl Combine {
    async fn run(sum_a: &i32, sum_b: &i32) -> i32 {
        println!(
            "Combining datasets: {} + {} = {}",
            sum_a,
            sum_b,
            sum_a + sum_b
        );
        sum_a + sum_b
    }
}

// Compute average (using a fixed count for simplicity)
struct ComputeAverage {
    total_count: i32,
}

impl ComputeAverage {
    fn new(total_count: i32) -> Self {
        Self { total_count }
    }
}

#[task]
impl ComputeAverage {
    async fn run(&mut self, sum: &i32) -> f64 {
        let avg = *sum as f64 / self.total_count as f64;
        println!(
            "Computing average: {} / {} = {:.2}",
            sum, self.total_count, avg
        );
        avg
    }
}

// Format report
struct FormatReport {
    count: i32,
}

impl FormatReport {
    fn new(count: i32) -> Self {
        Self { count }
    }
}

#[task]
impl FormatReport {
    async fn run(&mut self, sum: &i32, avg: &f64) -> String {
        format!(
            "ETL Pipeline Results:\n  Items: {}\n  Total: {}\n  Average: {:.2}",
            self.count, sum, avg
        )
    }
}

#[tokio::main]
async fn main() -> DagResult<()> {
    let dag = DagRunner::new();

    println!("Building ETL pipeline...\n");

    // Extract: Load from two sources (parallel)
    let source_a = dag.add_task(LoadData::new("A"));
    let source_b = dag.add_task(LoadData::new("B"));

    // Transform: Process each source (parallel)
    let transform_a = dag.add_task(Transform::new(2)).depends_on(&source_a);
    let transform_b = dag.add_task(Transform::new(3)).depends_on(&source_b);

    // Combine: Merge the transformed datasets
    let combined = dag
        .add_task(Combine)
        .depends_on((&transform_a, &transform_b));

    // Compute average on combined data (we know we have 6 items total)
    let avg = dag.add_task(ComputeAverage::new(6)).depends_on(combined);

    // Format final report (needs both sum and average)
    let report = dag
        .add_task(FormatReport::new(6))
        .depends_on((&combined, &avg));

    // Execute the entire ETL pipeline
    println!("Executing pipeline...\n");
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Display final report
    println!("\n{}", dag.get(report)?);
    println!("\nCalculation:");
    println!("  Source A: sum=6, transformed (x2) = 12");
    println!("  Source B: sum=15, transformed (x3) = 45");
    println!("  Combined: 6 items, sum=57, avg=9.50");

    Ok(())
}
