//! # Real-World: Data Processing Pipeline
//!
//! This example demonstrates a realistic data processing pipeline with multiple
//! stages of transformation and statistical analysis.
//!
//! ## What You'll Learn
//! - How to structure a multi-stage data pipeline
//! - How to use stateful tasks with configuration (DataTransformer)
//! - How to parallelize statistical computations on the same dataset
//! - How to combine results from parallel operations into a final report
//!
//! ## Pipeline Architecture
//! ```text
//! STAGE 1: Load    STAGE 2: Transform    STAGE 3: Analyze      STAGE 4: Report
//! ┌─────────┐      ┌──────────────┐      ┌────────────┐
//! │LoadVal1 │─────►│DataTransform │─────►│            │
//! │  (100)  │      │    (×1.5)    │──┐   │ ComputeSum │──┐
//! └─────────┘      └──────────────┘  │   │            │  │
//!                                     ├──►│   (525)    │  │
//! ┌─────────┐      ┌──────────────┐  │   └────────────┘  │
//! │LoadVal2 │─────►│DataTransform │──┤                   ├──►┌──────────┐
//! │  (200)  │      │    (×2.0)    │  │   ┌────────────┐  │   │ Generate │
//! └─────────┘      └──────────────┘  │   │ComputeProd │──┤   │  Report  │
//!                                     ├──►│            │  │   └──────────┘
//! ┌─────────┐      ┌──────────────┐  │   │ (3750000)  │  │
//! │LoadVal3 │─────►│DataTransform │──┘   └────────────┘  │
//! │  (50)   │      │    (×0.5)    │  │                   │
//! └─────────┘      └──────────────┘  │   ┌────────────┐  │
//!                                     │   │  FindMax   │  │
//!                                     └──►│   (400)    │──┘
//!                                         └────────────┘
//! ```
//!
//! ## Key Concepts
//! - **ETL Pattern**: Extract (Load) → Transform → Load (Report)
//! - **Parallel transformation**: Each data source is transformed independently
//! - **Parallel analysis**: Sum, Product, and Max computed simultaneously
//! - **Stateful tasks**: DataTransformer uses different multipliers for each instance
//!
//! ## Use Cases
//! - Log processing and analysis
//! - Data transformation pipelines
//! - Batch job orchestration
//! - Statistical analysis on datasets
//! - Multi-metric reporting systems
//!
//! ## Running This Example
//! ```bash
//! cargo run --example data_pipeline
//! ```
//!
//! ## Expected Output
//! ```text
//! Building data pipeline DAG...
//!
//! Executing pipeline...
//!
//! Stage 1: Loading raw value 1...
//! Stage 1: Loading raw value 2...
//! Stage 1: Loading raw value 3...
//!   Transforming data with multiplier 1.5...
//!   Transforming data with multiplier 2...
//!   Transforming data with multiplier 0.5...
//!
//! Stage 2: Computing sum...
//! Stage 2: Computing product...
//! Stage 2: Finding maximum...
//!
//! Stage 3: Generating report...
//!
//! === Data Pipeline Results ===
//! Sum of transformed values: 525.00
//! Product of transformed values: 3750000.00
//! Maximum value: 400.00
//! ============================
//! ```

use dagx::{task, DagRunner};

// Data source tasks
struct LoadValue1;

#[task]
impl LoadValue1 {
    async fn run() -> i32 {
        println!("Stage 1: Loading raw value 1...");
        100
    }
}

struct LoadValue2;

#[task]
impl LoadValue2 {
    async fn run() -> i32 {
        println!("Stage 1: Loading raw value 2...");
        200
    }
}

struct LoadValue3;

#[task]
impl LoadValue3 {
    async fn run() -> i32 {
        println!("Stage 1: Loading raw value 3...");
        50
    }
}

// Custom task with state for data transformation
struct DataTransformer {
    multiplier: f64,
}

impl DataTransformer {
    fn new(multiplier: f64) -> Self {
        Self { multiplier }
    }
}

#[task]
impl DataTransformer {
    async fn run(&mut self, input: &i32) -> f64 {
        println!("  Transforming data with multiplier {}...", self.multiplier);
        *input as f64 * self.multiplier
    }
}

// Statistical analysis tasks
struct ComputeSum;

#[task]
impl ComputeSum {
    async fn run(a: &f64, b: &f64, c: &f64) -> f64 {
        println!("\nStage 2: Computing sum...");
        a + b + c
    }
}

struct ComputeProduct;

#[task]
impl ComputeProduct {
    async fn run(a: &f64, b: &f64, c: &f64) -> f64 {
        println!("Stage 2: Computing product...");
        a * b * c
    }
}

struct FindMax;

#[task]
impl FindMax {
    async fn run(a: &f64, b: &f64, c: &f64) -> f64 {
        println!("Stage 2: Finding maximum...");
        a.max(*b).max(*c)
    }
}

// Report generation task
struct GenerateReport;

#[task]
impl GenerateReport {
    async fn run(sum: &f64, product: &f64, max: &f64) -> String {
        println!("\nStage 3: Generating report...");
        format!(
            "\n=== Data Pipeline Results ===\n\
             Sum of transformed values: {:.2}\n\
             Product of transformed values: {:.2}\n\
             Maximum value: {:.2}\n\
             ============================",
            sum, product, max
        )
    }
}

#[tokio::main]
async fn main() {
    let mut dag = DagRunner::new();

    println!("Building data pipeline DAG...\n");

    // Stage 1: Data sources
    let raw_value_1 = dag.add_task(LoadValue1);
    let raw_value_2 = dag.add_task(LoadValue2);
    let raw_value_3 = dag.add_task(LoadValue3);

    // Stage 2: Data transformation (with stateful tasks)
    let transformed_1 = dag
        .add_task(DataTransformer::new(1.5))
        .depends_on(&raw_value_1);

    let transformed_2 = dag
        .add_task(DataTransformer::new(2.0))
        .depends_on(&raw_value_2);

    let transformed_3 = dag
        .add_task(DataTransformer::new(0.5))
        .depends_on(&raw_value_3);

    // Stage 3: Statistical analysis (parallel operations on transformed data)
    let sum = dag
        .add_task(ComputeSum)
        .depends_on((&transformed_1, &transformed_2, &transformed_3));

    let product =
        dag.add_task(ComputeProduct)
            .depends_on((&transformed_1, &transformed_2, &transformed_3));

    let max = dag
        .add_task(FindMax)
        .depends_on((&transformed_1, &transformed_2, &transformed_3));

    // Stage 4: Final report
    let report = dag
        .add_task(GenerateReport)
        .depends_on((&sum, &product, &max));

    // Execute the pipeline
    println!("Executing pipeline...\n");
    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Print final report
    println!("{}", output.get(report));
}
