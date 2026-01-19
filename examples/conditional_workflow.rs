//! # Pattern: Conditional Execution with Data Validation
//!
//! This example demonstrates how to implement conditional logic in DAGs by using
//! simple data transformations rather than traditional Result types.
//!
//! ## What You'll Learn
//! - How to implement validation logic in parallel tasks
//! - How to use return values for conditional behavior (e.g., 0 for invalid)
//! - How to categorize and branch based on computed values
//! - How to aggregate results from multiple conditional branches
//!
//! ## Workflow Diagram
//! ```text
//!   VALIDATE                CATEGORIZE              SUMMARIZE
//! ┌─────────────┐         ┌──────────────┐
//! │ Validate(10)│────20──►│Categorize(25)│────"low"────┐
//! └─────────────┘         └──────────────┘             │
//!                                                       │
//! ┌─────────────┐         ┌──────────────┐             │
//! │ Validate(-5)│────0───►│Categorize(25)│──"invalid"──┼──►┌────────────┐
//! └─────────────┘         └──────────────┘             │   │ Summarize  │
//!                                                       │   └────────────┘
//! ┌─────────────┐         ┌──────────────┐             │
//! │ Validate(20)│────40──►│Categorize(25)│────"high"───┘
//! └─────────────┘         └──────────────┘
//! ```
//!
//! ## Key Concepts
//! - **Validation as transformation**: Invalid inputs become sentinel values (0)
//! - **Parallel validation**: All inputs validated simultaneously
//! - **Data-driven branching**: Categorization based on computed values
//! - **Type-safe aggregation**: All branches produce the same type (String)
//!
//! ## Use Cases
//! - Input validation pipelines
//! - Multi-stage data cleaning
//! - Classification and categorization workflows
//! - Quality control systems
//! - Data preprocessing for ML pipelines
//!
//! ## Running This Example
//! ```bash
//! cargo run --example conditional_workflow
//! ```
//!
//! ## Expected Output
//! ```text
//! Processing and categorizing inputs in parallel...
//!
//! Valid input 10: doubling to 20
//! Invalid input -5: returning 0
//! Valid input 20: doubling to 40
//!
//! Final summary: Categories: [low, invalid, high]
//! Expected: Categories: [low, invalid, high]
//! ```

use dagx::{task, DagResult, DagRunner};
use futures::FutureExt;

// Task that validates and processes input
struct ValidateAndDouble {
    value: i32,
}

impl ValidateAndDouble {
    fn new(value: i32) -> Self {
        Self { value }
    }
}

#[task]
impl ValidateAndDouble {
    async fn run(&mut self) -> i32 {
        if self.value > 0 {
            println!("Valid input {}: doubling to {}", self.value, self.value * 2);
            self.value * 2
        } else {
            println!("Invalid input {}: returning 0", self.value);
            0 // Return 0 for invalid inputs
        }
    }
}

// Task that categorizes a number
struct Categorize {
    threshold: i32,
}

impl Categorize {
    fn new(threshold: i32) -> Self {
        Self { threshold }
    }
}

#[task]
impl Categorize {
    async fn run(&mut self, value: &i32) -> String {
        if *value == 0 {
            "invalid".to_string()
        } else if *value >= self.threshold {
            "high".to_string()
        } else {
            "low".to_string()
        }
    }
}

// Task that combines categorization results
struct SummarizeResults;

#[task]
impl SummarizeResults {
    async fn run(cat1: &String, cat2: &String, cat3: &String) -> String {
        format!("Categories: [{}, {}, {}]", cat1, cat2, cat3)
    }
}

#[tokio::main]
async fn main() -> DagResult<()> {
    let dag = DagRunner::new();

    println!("Processing and categorizing inputs in parallel...\n");

    // Validate and process multiple inputs in parallel
    let processed1 = dag.add_task(ValidateAndDouble::new(10)); // Valid: 20
    let processed2 = dag.add_task(ValidateAndDouble::new(-5)); // Invalid: 0
    let processed3 = dag.add_task(ValidateAndDouble::new(20)); // Valid: 40

    // Categorize each result (also parallel, threshold = 25)
    let cat1 = dag.add_task(Categorize::new(25)).depends_on(&processed1); // 20 < 25 = "low"
    let cat2 = dag.add_task(Categorize::new(25)).depends_on(&processed2); // 0 = "invalid"
    let cat3 = dag.add_task(Categorize::new(25)).depends_on(&processed3); // 40 >= 25 = "high"

    // Combine results
    let summary = dag
        .add_task(SummarizeResults)
        .depends_on((&cat1, &cat2, &cat3));

    // Execute
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Display results
    println!("\nFinal summary: {}", dag.get(summary)?);
    println!("Expected: Categories: [low, invalid, high]");

    Ok(())
}
