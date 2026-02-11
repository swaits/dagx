//! # Handling Errors in DAGs
//!
//! This example demonstrates comprehensive error handling patterns using Result types
//! to represent success and failure states in DAG workflows.
//!
//! ## What You'll Learn
//! - How to use Result<T, E> types in task outputs
//! - How to propagate errors through dependency chains
//! - How to handle and transform errors from upstream tasks
//! - How to implement error recovery with fallback values
//!
//! ## A Note on Panics vs Result
//!
//! **This example uses Result<T, E> for expected errors** (like parsing failures). For
//! unexpected failures (bugs), tasks may panic. dagx automatically catches panics and
//! converts them to `DagError::TaskPanicked`, regardless of whether the task executes
//! inline (single-task layer) or spawned (multi-task layer). This ensures consistent
//! error handling across all execution paths.
//!
//! **Inline execution**: When a layer has only one task, dagx executes it inline for
//! performance. Panics are caught via `FutureExt::catch_unwind()` and become errors,
//! matching the behavior of spawned tasks where the async runtime catches panics.
//!
//! **Bottom line**: Use Result for expected errors (validation, parsing, I/O). If a task
//! panics (bug), it becomes an error automatically. Both behave consistently.
//!
//! ## Error Handling Patterns
//!
//! **1. Error Propagation** - Errors flow through dependencies
//! ```text
//! ┌─────────┐      ┌──────────┐      ┌─────────┐
//! │ Parse   │─Err─►│ Validate │─Err─►│ Process │
//! │  "abc"  │      │          │      │         │
//! └─────────┘      └──────────┘      └─────────┘
//! ```
//!
//! **2. Error Recovery** - Fallback values on error
//! ```text
//! ┌─────────┐      ┌──────────────┐
//! │ Parse   │─Err─►│ WithFallback │─────► 0 (default)
//! │  "abc"  │      │  (default=0) │
//! └─────────┘      └──────────────┘
//! ```
//!
//! **3. Error Logging** - Observe errors without blocking
//! ```text
//! ┌─────────┐      ┌──────────┐      ┌──────────┐
//! │  Task   │──?──►│ LogError │──?──►│   Next   │
//! └─────────┘      └──────────┘      └──────────┘
//!                   (logs but passes through)
//! ```
//!
//! ## Workflow Diagram
//! ```text
//! INPUT           PARSE           VALIDATE        PROCESS          LOG
//! ┌─────────┐   ┌─────────┐    ┌──────────┐   ┌─────────┐   ┌──────────┐
//! │ "42"    │──►│ParseInt │───►│ Validate │──►│ Process │──►│LogError  │
//! └─────────┘   │  Ok(42) │    │  Ok(42)  │   │ Ok(...) │   │  ✓       │
//!               └─────────┘    └──────────┘   └─────────┘   └──────────┘
//!
//! ┌─────────┐   ┌─────────┐    ┌──────────┐   ┌─────────┐   ┌──────────┐
//! │ "abc"   │──►│ParseInt │───►│ Validate │──►│ Process │──►│LogError  │
//! └─────────┘   │ Err(...)│    │ Err(...) │   │ Err(...)│   │  ✗       │
//!               └─────────┘    └──────────┘   └─────────┘   └──────────┘
//!
//! ┌─────────┐   ┌─────────┐    ┌──────────┐   ┌─────────┐   ┌──────────┐
//! │ "150"   │──►│ParseInt │───►│ Validate │──►│ Process │──►│LogError  │
//! └─────────┘   │ Ok(150) │    │Err(range)│   │ Err(...)│   │  ✗       │
//!               └─────────┘    └──────────┘   └─────────┘   └──────────┘
//!
//! ┌─────────┐   ┌─────────┐    ┌──────────────┐
//! │"invalid"│──►│ParseInt │───►│ WithFallback │───► 0
//! └─────────┘   │ Err(...)│    │  (default=0) │
//!               └─────────┘    └──────────────┘
//! ```
//!
//! ## Key Concepts
//! - **Errors as values**: Use Result<T, E> to represent errors in the type system
//! - **Error propagation**: Errors flow through dependencies using `?` operator
//! - **Type safety**: The compiler ensures errors are handled
//! - **Graceful degradation**: Recover from errors with fallback logic
//!
//! ## Use Cases
//! - Input validation pipelines
//! - Resilient data processing workflows
//! - API integration with error handling
//! - Multi-stage validation with early exit
//! - Data cleaning and sanitization
//!
//! ## Running This Example
//! ```bash
//! cargo run --example error_handling
//! ```
//!
//! ## Expected Output
//! ```text
//! === Error Handling Example ===
//!
//! Example 1: Valid input
//! ✓ Success: Processed value: 84
//! Final result: Processed value: 84
//!
//! Example 2: Invalid input (parse error)
//! ✗ Error: Failed to parse 'not a number': invalid digit found in string
//! Final error: Failed to parse 'not a number': invalid digit found in string
//!
//! Example 3: Out of range
//! ✗ Error: Value 150 is out of range [0, 100]
//! Final error: Value 150 is out of range [0, 100]
//!
//! Example 4: Error recovery with fallback
//! Recovered with fallback: 0
//!
//! === Error Handling Example Complete ===
//! ```

use dagx::{task, DagRunner};


// Source task that provides a string input
struct StringSource(&'static str);

#[task]
impl StringSource {
    async fn run(&self) -> String {
        self.0.to_string()
    }
}

// Task that may fail: parsing a string to integer
struct ParseInt;

#[task]
impl ParseInt {
    async fn run(input: &String) -> Result<i32, String> {
        input
            .parse::<i32>()
            .map_err(|e| format!("Failed to parse '{}': {}", input, e))
    }
}

// Task that validates input range
struct ValidateRange {
    min: i32,
    max: i32,
}

#[task]
impl ValidateRange {
    async fn run(&mut self, input: &Result<i32, String>) -> Result<i32, String> {
        let value = input.as_ref().map_err(|e| e.clone())?;

        if *value < self.min || *value > self.max {
            return Err(format!(
                "Value {} is out of range [{}, {}]",
                value, self.min, self.max
            ));
        }

        Ok(*value)
    }
}

// Task that processes valid data
struct ProcessData;

#[task]
impl ProcessData {
    async fn run(input: &Result<i32, String>) -> Result<String, String> {
        let value = input.as_ref().map_err(|e| e.clone())?;
        Ok(format!("Processed value: {}", value * 2))
    }
}

// Task that provides a fallback value on error
struct WithFallback {
    fallback: i32,
}

#[task]
impl WithFallback {
    async fn run(&mut self, input: &Result<i32, String>) -> i32 {
        *input.as_ref().unwrap_or(&self.fallback)
    }
}

// Task that logs errors
struct LogError;

#[task]
impl LogError {
    async fn run(input: &Result<String, String>) -> Result<String, String> {
        match input {
            Ok(s) => {
                println!("✓ Success: {}", s);
                Ok(s.clone())
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
                Err(e.clone())
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("=== Error Handling Example ===\n");

    // Example 1: Successful path
    println!("Example 1: Valid input");
    {
        let dag = DagRunner::new();

        let input = dag.add_task(StringSource("42"));

        let parsed = dag.add_task(ParseInt).depends_on(input);

        let validated = dag
            .add_task(ValidateRange { min: 0, max: 100 })
            .depends_on(parsed);

        let processed = dag.add_task(ProcessData).depends_on(validated);
        let logged = dag.add_task(LogError).depends_on(processed);

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        match dag.get(logged).unwrap().as_ref() {
            Ok(result) => println!("Final result: {}\n", result),
            Err(e) => eprintln!("Final error: {}\n", e),
        }
    }

    // Example 2: Parse error
    println!("Example 2: Invalid input (parse error)");
    {
        let dag = DagRunner::new();

        let input = dag.add_task(StringSource("not a number"));

        let parsed = dag.add_task(ParseInt).depends_on(input);

        let validated = dag
            .add_task(ValidateRange { min: 0, max: 100 })
            .depends_on(parsed);

        let processed = dag.add_task(ProcessData).depends_on(validated);
        let logged = dag.add_task(LogError).depends_on(processed);

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        match dag.get(logged).unwrap().as_ref() {
            Ok(result) => println!("Final result: {}\n", result),
            Err(e) => eprintln!("Final error: {}\n", e),
        }
    }

    // Example 3: Validation error
    println!("Example 3: Out of range");
    {
        let dag = DagRunner::new();

        let input = dag.add_task(StringSource("150"));

        let parsed = dag.add_task(ParseInt).depends_on(input);

        let validated = dag
            .add_task(ValidateRange { min: 0, max: 100 })
            .depends_on(parsed);

        let processed = dag.add_task(ProcessData).depends_on(validated);
        let logged = dag.add_task(LogError).depends_on(processed);

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        match dag.get(logged).unwrap().as_ref() {
            Ok(result) => println!("Final result: {}\n", result),
            Err(e) => eprintln!("Final error: {}\n", e),
        }
    }

    // Example 4: Error recovery with fallback
    println!("Example 4: Error recovery with fallback");
    {
        let dag = DagRunner::new();

        let input = dag.add_task(StringSource("invalid"));

        let parsed = dag.add_task(ParseInt).depends_on(input);

        let with_fallback = dag
            .add_task(WithFallback { fallback: 0 })
            .depends_on(parsed);

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        let result = dag.get(with_fallback).unwrap();
        println!("Recovered with fallback: {}\n", result);
    }

    println!("=== Error Handling Example Complete ===");
}
