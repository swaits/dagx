//! # Pattern: Fan-In (N→1 Dependencies)
//!
//! This example demonstrates the fan-in pattern where multiple independent source
//! tasks produce values that are aggregated by a single downstream task.
//!
//! ## What You'll Learn
//! - How to create many-to-one task dependencies
//! - How to aggregate results from multiple upstream tasks
//! - How parameter order in `depends_on()` maps to function parameters
//! - How independent tasks run in parallel before aggregation
//!
//! ## Pattern Diagram
//! ```text
//!   ┌──────────┐  ┌─────────┐  ┌──────────┐  ┌────────────┐
//!   │FetchName │  │FetchAge │  │FetchCity │  │CheckActive │
//!   └────┬─────┘  └────┬────┘  └────┬─────┘  └──────┬─────┘
//!        │             │             │               │
//!      "Alice"        30      "San Francisco"      true
//!        │             │             │               │
//!        └─────────────┴─────────────┴───────────────┘
//!                              │
//!                      ┌───────▼────────┐
//!                      │ BuildProfile   │
//!                      └────────────────┘
//!                              │
//!                       "User Profile..."
//! ```
//!
//! ## Key Concepts
//! - **Fan-in**: Multiple tasks' outputs are consumed by a single downstream task
//! - **Aggregation**: Combining multiple values into a single result
//! - **Parameter order**: The order in `.depends_on((&a, &b, &c))` must match
//!   the function signature `fn run(a: &T1, b: &T2, c: &T3)`
//!
//! ## Use Cases
//! - Aggregating data from multiple API endpoints
//! - Combining results from parallel computations
//! - Building composite objects from multiple sources
//! - Waiting for multiple prerequisites before proceeding
//!
//! ## Running This Example
//! ```bash
//! cargo run --example fan_in
//! ```
//!
//! ## Expected Output
//! ```text
//! Running fan-in DAG...
//!
//! Fetching name...
//! Fetching age...
//! Fetching city...
//! Checking active status...
//!
//! Building user profile...
//!
//! User Profile:
//!   Name: Alice
//!   Age: 30
//!   City: San Francisco
//!   Active: true
//! ```

use dagx::{task, DagRunner};

struct FetchName;

#[task]
impl FetchName {
    async fn run() -> String {
        println!("Fetching name...");
        "Alice".to_string()
    }
}

struct FetchAge;

#[task]
impl FetchAge {
    async fn run() -> i32 {
        println!("Fetching age...");
        30
    }
}

struct FetchCity;

#[task]
impl FetchCity {
    async fn run() -> String {
        println!("Fetching city...");
        "San Francisco".to_string()
    }
}

struct CheckActive;

#[task]
impl CheckActive {
    async fn run() -> bool {
        println!("Checking active status...");
        true
    }
}

struct BuildProfile;

#[task]
impl BuildProfile {
    async fn run(name: &String, age: &i32, city: &String, active: &bool) -> String {
        println!("\nBuilding user profile...");
        format!(
            "User Profile:\n  Name: {}\n  Age: {}\n  City: {}\n  Active: {}",
            name, age, city, active
        )
    }
}

#[tokio::main]
async fn main() {
    let dag = DagRunner::new();

    // Multiple independent source tasks
    let name = dag.add_task(FetchName);
    let age = dag.add_task(FetchAge);
    let city = dag.add_task(FetchCity);
    let active = dag.add_task(CheckActive);

    // Single downstream task consuming all values
    let profile = dag
        .add_task(BuildProfile)
        .depends_on((name, age, city, active));

    // Run the DAG
    println!("Running fan-in DAG...\n");
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Print result
    println!("\n{}", dag.get(profile).unwrap());
}
