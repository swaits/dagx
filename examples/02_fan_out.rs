//! # Pattern: Fan-Out (1→N Dependencies)
//!
//! This example demonstrates the fan-out pattern where a single source task
//! produces a value that is consumed by multiple independent downstream tasks.
//!
//! ## What You'll Learn
//! - How to create one-to-many task dependencies
//! - How multiple tasks can consume the same upstream result
//! - How tasks with the same dependency run in parallel
//! - How to configure tasks with different parameters
//!
//! ## Pattern Diagram
//! ```text
//!          ┌──────────┐
//!          │  Source  │ (produces: 10)
//!          └────┬─────┘
//!        ┌──────┼──────┬──────┐
//!        │      │      │      │
//!    ┌───▼──┐ ┌─▼───┐ ┌▼────┐│
//!    │ +1   │ │ ×2  │ │ ^2  ││
//!    └───┬──┘ └──┬──┘ └─┬───┘│
//!        │      │      │     │
//!     (11)   (20)   (100)    │
//! ```
//!
//! ## Key Concepts
//! - **Fan-out**: A single task's output is used by multiple downstream tasks
//! - **Parallel execution**: All three downstream tasks run simultaneously
//! - **Reusable results**: The source value is cloned to each dependent task
//!
//! ## Use Cases
//! - Broadcasting configuration to multiple workers
//! - Applying different transformations to the same data
//! - Parallel A/B testing with the same input
//! - Computing multiple metrics from a single dataset
//!
//! ## Running This Example
//! ```bash
//! cargo run --example fan_out
//! ```
//!
//! ## Expected Output
//! ```text
//! Running fan-out DAG...
//!
//! Source: Computing initial value 10...
//! AddOffset: 10 + 1
//! Multiply: 10 * 2
//! Power: 10 ^ 2
//!
//! Results:
//!   Source: 10
//!   Plus One: 11
//!   Times Two: 20
//!   Squared: 100
//! ```

use dagx::{task, DagRunner};
use futures::FutureExt;

// Source task with a value field
struct Source {
    initial_value: i32,
}

impl Source {
    fn new(initial_value: i32) -> Self {
        Self { initial_value }
    }
}

#[task]
impl Source {
    async fn run(&mut self) -> i32 {
        println!("Source: Computing initial value {}...", self.initial_value);
        self.initial_value
    }
}

// Transformation tasks with configuration fields
struct AddOffset {
    offset: i32,
}

impl AddOffset {
    fn new(offset: i32) -> Self {
        Self { offset }
    }
}

#[task]
impl AddOffset {
    async fn run(&mut self, x: &i32) -> i32 {
        println!("AddOffset: {} + {}", x, self.offset);
        x + self.offset
    }
}

struct Multiply {
    factor: i32,
}

impl Multiply {
    fn new(factor: i32) -> Self {
        Self { factor }
    }
}

#[task]
impl Multiply {
    async fn run(&mut self, x: &i32) -> i32 {
        println!("Multiply: {} * {}", x, self.factor);
        x * self.factor
    }
}

struct Power {
    exponent: u32,
}

#[task]
impl Power {
    async fn run(&mut self, x: &i32) -> i32 {
        println!("Power: {} ^ {}", x, self.exponent);
        x.pow(self.exponent)
    }
}

#[tokio::main]
async fn main() {
    let dag = DagRunner::new();

    // Single source task - constructed with ::new()
    let source = dag.add_task(Source::new(10));

    // Multiple downstream tasks consuming the same value
    // Each constructed differently to show flexibility
    let plus_one = dag.add_task(AddOffset::new(1)).depends_on(&source);
    let times_two = dag.add_task(Multiply::new(2)).depends_on(&source);
    let squared = dag.add_task(Power { exponent: 2 }).depends_on(&source);

    // Run the DAG
    println!("Running fan-out DAG...\n");
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Print all results
    println!("\nResults:");
    println!("  Source: {}", dag.get(source).unwrap());
    println!("  Plus One: {}", dag.get(plus_one).unwrap());
    println!("  Times Two: {}", dag.get(times_two).unwrap());
    println!("  Squared: {}", dag.get(squared).unwrap());
}
