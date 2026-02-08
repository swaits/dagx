//! # Custom Types: Using Your Own Structs
//!
//! This example demonstrates that **ANY custom type** works in dagx as long as it implements
//! ` Send + Sync + 'static`. You don't need to implement any special traits!
//!
//! ## What You'll Learn
//! - How to use custom structs as task outputs
//! - How custom types flow through the DAG automatically
//! - How one task's output can be consumed by multiple downstream tasks
//!
//! ## Key Insight
//!
//! **dagx works with ANY type automatically!** The `#[task]` macro generates type-specific
//! extraction logic, so you don't need to implement any special traits. Just make sure your
//! type implements:
//! - `Send + Sync` (required for async tasks)
//! - `'static` (no lifetime constraints)
//!
//! That's it! No ExtractInput, no special annotations, no trait boilerplate.
//!
//! ## Running This Example
//! ```bash
//! cargo run --example custom_types
//! ```
//!
//! ## Expected Output
//! ```text
//! Creating custom types example...
//!
//! Creating user: Alice (age: 30)
//! Processing user Alice for greeting
//! Processing user Alice for validation
//! Greeting: Hello, Alice!
//! Validation: Alice is valid (age 30 >= 18)
//! ```

use dagx::{task, DagRunner, TaskHandle};
use futures::FutureExt;

#[derive(Debug)]
struct User {
    name: String,
    age: u32,
}

// Task that produces a custom type
struct CreateUser {
    name: String,
    age: u32,
}

#[task]
impl CreateUser {
    async fn run(&self) -> User {
        println!("Creating user: {} (age: {})", self.name, self.age);
        User {
            name: self.name.clone(),
            age: self.age,
        }
    }
}

// Task that consumes the custom type and produces a String
struct GreetUser;

#[task]
impl GreetUser {
    async fn run(user: &User) -> String {
        println!("Processing user {} for greeting", user.name);
        format!("Hello, {}!", user.name)
    }
}

// Another task that also consumes the same custom type
// This demonstrates fan-out: one output consumed by multiple tasks
struct ValidateUser;

#[task]
impl ValidateUser {
    async fn run(user: &User) -> String {
        println!("Processing user {} for validation", user.name);
        if user.age >= 18 {
            format!("{} is valid (age {} >= 18)", user.name, user.age)
        } else {
            format!("{} is too young (age {} < 18)", user.name, user.age)
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Creating custom types example...\n");

    let dag = DagRunner::new();

    // Create a task that produces a custom User type
    let user_task: TaskHandle<_> = dag
        .add_task(CreateUser {
            name: "Alice".to_string(),
            age: 30,
        })
        .into();

    // Two different tasks consume the same custom User type
    // dagx automatically handles Arc wrapping, so both tasks get &User
    let greeting = dag.add_task(GreetUser).depends_on(user_task);
    let validation = dag.add_task(ValidateUser).depends_on(user_task);

    // Execute the DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Print results
    println!("Greeting: {}", dag.get(greeting).unwrap());
    println!("Validation: {}", dag.get(validation).unwrap());
}
