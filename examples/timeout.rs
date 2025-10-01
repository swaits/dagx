//! # Managing Timeouts in Tasks
//!
//! This example demonstrates how to implement timeout patterns in DAG tasks using
//! tokio's timeout utilities for time-bounded execution.
//!
//! ## What You'll Learn
//! - How to implement timeouts using `tokio::time::timeout`
//! - How to build tasks with built-in timeout logic
//! - How to handle timeout errors as Result types
//! - How to apply timeouts at both task-level and DAG-level
//!
//! ## Timeout Patterns
//!
//! **1. Task-Level Timeout** - Individual task with timeout
//! ```rust
//! async fn run(&mut self) -> Result<String, String> {
//!     let work = async { /* ... */ };
//!     match timeout(Duration::from_millis(self.timeout_ms), work).await {
//!         Ok(result) => Ok(result),
//!         Err(_) => Err("Task timed out".to_string()),
//!     }
//! }
//! ```
//!
//! **2. DAG-Level Timeout** - Entire workflow with timeout
//! ```rust
//! match timeout(dag_timeout, dag.run(spawner)).await {
//!     Ok(result) => { /* DAG completed */ },
//!     Err(_) => { /* DAG timed out */ },
//! }
//! ```
//!
//! ## Workflow Diagram
//! ```text
//! ┌─────────────┐
//! │  FastTask   │ (completes in 100ms)
//! │  (timeout   │ ───► Ok("Fast task completed")
//! │   500ms)    │
//! └─────────────┘
//!
//! ┌─────────────┐
//! │  SlowTask   │ (takes 1000ms)
//! │  (timeout   │ ───► Err("Task timed out after 200ms")
//! │   200ms)    │
//! └─────────────┘
//!        │
//!        └──────►┌──────────────┐
//!                │ProcessResult │ ───► "✗ Task timed out..."
//!                └──────────────┘
//! ```
//!
//! ## Key Concepts
//! - **Timeout as error**: Timeouts produce `Err` results that propagate through the DAG
//! - **Error as data**: Use Result<T, E> to represent timeout states
//! - **Graceful degradation**: Downstream tasks can handle timeout errors
//! - **Time-bounded execution**: Enforce SLAs and prevent runaway tasks
//!
//! ## Use Cases
//! - API calls with timeout limits
//! - SLA enforcement in workflows
//! - Preventing resource exhaustion from slow tasks
//! - Circuit breaker patterns
//! - External service integration with timeouts
//!
//! ## Running This Example
//! ```bash
//! cargo run --example timeout
//! ```
//!
//! ## Expected Output
//! ```text
//! === Timeout Example ===
//!
//! Example 1: All tasks complete successfully
//! Fast task: Ok("Fast task completed")
//! Timed task: Ok("Work completed in 100ms")
//!
//! Example 2: Task exceeds timeout
//! Result: ✗ Task timed out after 200ms (work would take 1000ms)
//!
//! Example 3: Parallel tasks with different speeds
//! Fast task 1: Ok("Fast task completed")
//! Fast task 2: Ok("Fast task completed")
//! Slow task: Ok("Slow task completed after 500ms")
//! Timed task 1: Ok("Work completed in 100ms")
//! Timed task 2: Err("Task timed out after 200ms...")
//!
//! Example 4: Downstream tasks handling timeout errors
//! Processed 1: ✗ Task timed out after 100ms (work would take 500ms)
//! Processed 2: ✗ Task timed out after 100ms (work would take 500ms)
//!
//! Example 5: Whole-DAG timeout
//! DAG completed within timeout
//! Task 1: Ok("Slow task completed after 200ms")
//! Task 2: Ok("Slow task completed after 200ms")
//! Task 3: Ok("Slow task completed after 200ms")
//!
//! === Timeout Example Complete ===
//! ```

use dagx::{task, DagRunner, Task};
use tokio::time::{sleep, timeout, Duration};

// Fast task that completes quickly
struct FastTask;

#[task]
impl FastTask {
    async fn run() -> Result<String, String> {
        sleep(Duration::from_millis(100)).await;
        Ok("Fast task completed".to_string())
    }
}

// Slow task that takes a long time
struct SlowTask {
    duration_ms: u64,
}

#[task]
impl SlowTask {
    async fn run(&mut self) -> Result<String, String> {
        sleep(Duration::from_millis(self.duration_ms)).await;
        Ok(format!("Slow task completed after {}ms", self.duration_ms))
    }
}

// Task with built-in timeout
struct TaskWithTimeout {
    work_duration_ms: u64,
    timeout_ms: u64,
}

#[task]
impl TaskWithTimeout {
    async fn run(&mut self) -> Result<String, String> {
        let work = async {
            sleep(Duration::from_millis(self.work_duration_ms)).await;
            format!("Work completed in {}ms", self.work_duration_ms)
        };

        match timeout(Duration::from_millis(self.timeout_ms), work).await {
            Ok(result) => Ok(result),
            Err(_) => Err(format!(
                "Task timed out after {}ms (work would take {}ms)",
                self.timeout_ms, self.work_duration_ms
            )),
        }
    }
}

// Task that processes results (including timeouts)
struct ProcessResult;

#[task]
impl ProcessResult {
    async fn run(input: &Result<String, String>) -> String {
        match input {
            Ok(msg) => format!("✓ {}", msg),
            Err(err) => format!("✗ {}", err),
        }
    }
}

#[tokio::main]
async fn main() {
    println!("=== Timeout Example ===\n");

    // Example 1: All tasks complete within timeout
    println!("Example 1: All tasks complete successfully");
    {
        let dag = DagRunner::new();

        let fast = dag.add_task(FastTask);
        let timed = dag.add_task(TaskWithTimeout {
            work_duration_ms: 100,
            timeout_ms: 500,
        });

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Fast task: {:?}", dag.get(fast).unwrap());
        println!("Timed task: {:?}\n", dag.get(timed).unwrap());
    }

    // Example 2: Task exceeds timeout
    println!("Example 2: Task exceeds timeout");
    {
        let dag = DagRunner::new();

        let timed = dag.add_task(TaskWithTimeout {
            work_duration_ms: 1000,
            timeout_ms: 200,
        });

        let processed = dag.add_task(ProcessResult).depends_on(&timed);

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {}\n", dag.get(processed).unwrap());
    }

    // Example 3: Mixed fast and slow tasks
    println!("Example 3: Parallel tasks with different speeds");
    {
        let dag = DagRunner::new();

        let fast1 = dag.add_task(FastTask);
        let fast2 = dag.add_task(FastTask);

        let slow = dag.add_task(SlowTask { duration_ms: 500 });

        let timed1 = dag.add_task(TaskWithTimeout {
            work_duration_ms: 100,
            timeout_ms: 200,
        });

        let timed2 = dag.add_task(TaskWithTimeout {
            work_duration_ms: 300,
            timeout_ms: 200,
        });

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Fast task 1: {:?}", dag.get(fast1).unwrap());
        println!("Fast task 2: {:?}", dag.get(fast2).unwrap());
        println!("Slow task: {:?}", dag.get(slow).unwrap());
        println!("Timed task 1: {:?}", dag.get(timed1).unwrap());
        println!("Timed task 2: {:?}\n", dag.get(timed2).unwrap());
    }

    // Example 4: Cascading timeout errors
    println!("Example 4: Downstream tasks handling timeout errors");
    {
        let dag = DagRunner::new();

        let source = dag.add_task(TaskWithTimeout {
            work_duration_ms: 500,
            timeout_ms: 100,
        });

        let processed1 = dag.add_task(ProcessResult).depends_on(&source);
        let processed2 = dag.add_task(ProcessResult).depends_on(&source);

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Processed 1: {}", dag.get(processed1).unwrap());
        println!("Processed 2: {}\n", dag.get(processed2).unwrap());
    }

    // Example 5: Implementing whole-DAG timeout
    println!("Example 5: Whole-DAG timeout");
    {
        let dag = DagRunner::new();

        let t1 = dag.add_task(SlowTask { duration_ms: 200 });
        let t2 = dag.add_task(SlowTask { duration_ms: 200 });
        let t3 = dag.add_task(SlowTask { duration_ms: 200 });

        // Run the entire DAG with a timeout
        let dag_timeout = Duration::from_millis(500);
        match timeout(
            dag_timeout,
            dag.run(|fut| {
                tokio::spawn(fut);
            }),
        )
        .await
        {
            Ok(result) => {
                result.unwrap();
                println!("DAG completed within timeout");
                println!("Task 1: {:?}", dag.get(t1).unwrap());
                println!("Task 2: {:?}", dag.get(t2).unwrap());
                println!("Task 3: {:?}", dag.get(t3).unwrap());
            }
            Err(_) => {
                eprintln!(
                    "DAG execution timed out after {}ms",
                    dag_timeout.as_millis()
                );
            }
        }
    }

    println!("\n=== Timeout Example Complete ===");
}
