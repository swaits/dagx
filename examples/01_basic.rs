//! # Getting Started: Your First DAG
//!
//! This example introduces the fundamentals of building and executing a DAG with dagx.
//!
//! ## What You'll Learn
//! - How to define tasks using the `#[task]` macro
//! - How to create a DAG and add tasks
//! - How to wire dependencies between tasks
//! - How to execute the DAG and retrieve results
//!
//! ## Key Concepts
//! - **Task**: A unit of work with typed inputs and outputs
//! - **DAG (Directed Acyclic Graph)**: A workflow where tasks depend on other tasks
//! - **TaskHandle**: A typed reference to a task's output
//! - **Dependencies**: Relationships between tasks that determine execution order
//!
//! ## Running This Example
//! ```bash
//! cargo run --example basic
//! ```
//!
//! ## Expected Output
//! ```text
//! Running DAG...
//!
//! Computing value: 2
//! Computing value: 3
//! Sum: Computing 2 + 3
//!
//! Result: 5
//! ```

use dagx::{task, DagRunner, Task};
use futures::FutureExt;

// Step 1: Define a source task (no dependencies)
//
// Source tasks produce initial data for the DAG.
// They don't depend on any other tasks.
struct Value {
    value: i32,
}

impl Value {
    fn new(value: i32) -> Self {
        Self { value }
    }
}

// The #[task] macro converts your implementation into a full Task trait impl
#[task]
impl Value {
    // Use `&mut self` to access the task's state
    async fn run(&mut self) -> i32 {
        println!("Computing value: {}", self.value);
        self.value
    }
}

// Step 2: Define a task with dependencies
//
// This task takes TWO inputs (a and b), both of type i32.
// The order of parameters matches the order in .depends_on()
struct Add {
    label: String,
}

#[task]
impl Add {
    // Parameters after `&mut self` are dependencies
    // They're provided as references (&i32, not i32)
    async fn run(&mut self, a: &i32, b: &i32) -> i32 {
        println!("{}: Computing {} + {}", self.label, a, b);
        a + b
    }
}

#[tokio::main]
async fn main() {
    // Step 3: Create a new DAG
    let dag = DagRunner::new();

    // Step 4: Add source tasks (tasks with no dependencies)
    //
    // add_task() returns a TaskHandle<T> where T is the task's output type
    let x = dag.add_task(Value::new(2)); // TaskHandle<i32>
    let y = dag.add_task(Value::new(3)); // TaskHandle<i32>

    // Step 5: Add a task with dependencies
    //
    // The .depends_on() method wires up dependencies.
    // Order matters! The first handle maps to the first parameter, etc.
    let sum = dag
        .add_task(Add {
            label: "Sum".to_string(),
        })
        .depends_on((&x, &y)); // Pass handles as a tuple
                               // sum is now TaskHandle<i32>

    // Step 6: Execute the DAG
    //
    // The run() method needs a "spawner" - a function that spawns futures.
    // This enables runtime-agnostic execution.
    //
    // Execution order:
    // 1. Tasks with no dependencies run first (x and y in parallel)
    // 2. Once x and y complete, sum runs (it depends on both)
    println!("Running DAG...\n");
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Step 7: Retrieve results
    //
    // Use .get() with a TaskHandle to retrieve the output.
    // Results are cloned, so you can retrieve them multiple times.
    println!("\nResult: {}", dag.get(sum).unwrap());

    // Note: You can also retrieve intermediate results
    assert_eq!(dag.get(x).unwrap(), 2);
    assert_eq!(dag.get(y).unwrap(), 3);
    assert_eq!(dag.get(sum).unwrap(), 5);
}
