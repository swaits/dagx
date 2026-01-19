//! # Debug Tracing and Task Naming
//!
//! This example demonstrates how to add debug metadata and tracing information to tasks
//! for better observability in complex DAG workflows.
//!
//! ## What You'll Learn
//! - How to add task names and IDs for debugging
//! - How to implement correlation IDs for distributed tracing
//! - How to track task execution timing and lifecycle events
//! - How to build traceable task wrappers
//!
//! ## Debugging Patterns
//!
//! **1. Named Tasks** - Add human-readable names to tasks
//! ```rust
//! struct TaskMetadata {
//!     id: String,
//!     name: String,
//!     start_time: Option<Instant>,
//! }
//! ```
//!
//! **2. Correlation IDs** - Track requests across task boundaries
//! ```rust
//! struct TraceContext {
//!     correlation_id: String,
//!     parent_task: Option<String>,
//! }
//! ```
//!
//! **3. Execution Metrics** - Measure task performance
//! ```rust
//! struct TaskTrace {
//!     name: String,
//!     duration_ms: u64,
//!     status: &'static str,
//! }
//! ```
//!
//! ## Workflow Diagram
//! ```text
//! TRACE: order-12345
//! ┌──────────────────┐
//! │ ValidateOrder    │ [order-12345] Start: 0ms
//! │  ID: task-001    │──────► Status: Success
//! └──────────────────┘        Duration: 10ms
//!         │
//!         ├──────────────────┬──────────────────┐
//!         ▼                  ▼                  ▼
//! ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
//! │ CheckInventory   │ │ ValidatePayment  │ │ CalculateShipping│
//! │  ID: task-002    │ │  ID: task-003    │ │  ID: task-004    │
//! └──────────────────┘ └──────────────────┘ └──────────────────┘
//!  [order-12345]        [order-12345]        [order-12345]
//!  Duration: 45ms       Duration: 120ms      Duration: 15ms
//!         │                  │                  │
//!         └──────────────────┴──────────────────┘
//!                            ▼
//!                  ┌──────────────────┐
//!                  │  FulfillOrder    │ [order-12345]
//!                  │  ID: task-005    │ Duration: 30ms
//!                  └──────────────────┘
//! ```
//!
//! ## Key Concepts
//! - **Observability**: Make task execution visible for debugging
//! - **Correlation IDs**: Track a request through multiple tasks
//! - **Structured logging**: Consistent task metadata across logs
//! - **Performance metrics**: Measure task execution time
//! - **Task lifecycle**: Track start, completion, and errors
//!
//! ## Use Cases
//! - Debugging complex workflows in production
//! - Performance analysis and bottleneck identification
//! - Distributed tracing in microservices
//! - SLA monitoring and alerting
//! - Audit logging for compliance
//!
//! ## Running This Example
//! ```bash
//! cargo run --example debug_tracing
//! ```
//!
//! ## Expected Output
//! ```text
//! === Debug Tracing Example ===
//!
//! Starting order processing for order-12345
//!
//! [order-12345] [task-001] ValidateOrder: Starting
//! [order-12345] [task-001] ValidateOrder: Completed in 10ms - Success
//!
//! [order-12345] [task-002] CheckInventory: Starting
//! [order-12345] [task-003] ValidatePayment: Starting
//! [order-12345] [task-004] CalculateShipping: Starting
//! [order-12345] [task-002] CheckInventory: Completed in 45ms - Success
//! [order-12345] [task-004] CalculateShipping: Completed in 15ms - Success
//! [order-12345] [task-003] ValidatePayment: Completed in 120ms - Success
//!
//! [order-12345] [task-005] FulfillOrder: Starting
//! [order-12345] [task-005] FulfillOrder: Completed in 30ms - Success
//!
//! === Execution Trace Summary ===
//! Correlation ID: order-12345
//! Total Tasks: 5
//! Total Duration: 220ms
//! Success: 5, Failed: 0
//!
//! Task Breakdown:
//!   ValidateOrder: 10ms
//!   CheckInventory: 45ms
//!   ValidatePayment: 120ms (slowest)
//!   CalculateShipping: 15ms
//!   FulfillOrder: 30ms
//! ```

use dagx::{task, DagRunner};
use futures::FutureExt;
use std::time::Instant;
use tokio::time::{sleep, Duration};

// Stateful task that tracks execution metadata for debugging
struct TracedTask {
    task_id: String,
    task_name: String,
    correlation_id: String,
    work_duration_ms: u64,
    start_time: Option<Instant>,
}

impl TracedTask {
    fn new(
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        correlation_id: impl Into<String>,
        work_duration_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            task_name: task_name.into(),
            correlation_id: correlation_id.into(),
            work_duration_ms,
            start_time: None,
        }
    }
}

#[task]
impl TracedTask {
    async fn run(&mut self) -> Result<TaskTrace, String> {
        // Record start
        let start = Instant::now();
        self.start_time = Some(start);

        println!(
            "[{}] [{}] {}: Starting",
            self.correlation_id, self.task_id, self.task_name
        );

        // Simulate work
        sleep(Duration::from_millis(self.work_duration_ms)).await;

        // Record completion
        let duration_ms = start.elapsed().as_millis() as u64;

        println!(
            "[{}] [{}] {}: Completed in {}ms - Success",
            self.correlation_id, self.task_id, self.task_name, duration_ms
        );

        Ok(TaskTrace {
            task_id: self.task_id.clone(),
            task_name: self.task_name.clone(),
            correlation_id: self.correlation_id.clone(),
            duration_ms,
            status: "Success",
        })
    }
}

// Task that can fail, demonstrating error tracing
struct TracedTaskWithError {
    task_id: String,
    task_name: String,
    correlation_id: String,
    should_fail: bool,
}

impl TracedTaskWithError {
    fn new(
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        correlation_id: impl Into<String>,
        should_fail: bool,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            task_name: task_name.into(),
            correlation_id: correlation_id.into(),
            should_fail,
        }
    }
}

#[task]
impl TracedTaskWithError {
    async fn run(&mut self) -> Result<TaskTrace, TaskTrace> {
        let start = Instant::now();

        println!(
            "[{}] [{}] {}: Starting",
            self.correlation_id, self.task_id, self.task_name
        );

        sleep(Duration::from_millis(50)).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        if self.should_fail {
            println!(
                "[{}] [{}] {}: Failed in {}ms - Validation Error",
                self.correlation_id, self.task_id, self.task_name, duration_ms
            );

            Err(TaskTrace {
                task_id: self.task_id.clone(),
                task_name: self.task_name.clone(),
                correlation_id: self.correlation_id.clone(),
                duration_ms,
                status: "Failed",
            })
        } else {
            println!(
                "[{}] [{}] {}: Completed in {}ms - Success",
                self.correlation_id, self.task_id, self.task_name, duration_ms
            );

            Ok(TaskTrace {
                task_id: self.task_id.clone(),
                task_name: self.task_name.clone(),
                correlation_id: self.correlation_id.clone(),
                duration_ms,
                status: "Success",
            })
        }
    }
}

// Task trace data structure
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct TaskTrace {
    task_id: String,
    task_name: String,
    correlation_id: String,
    duration_ms: u64,
    status: &'static str,
}

// Task that aggregates traces from multiple upstream tasks
struct TraceAggregator {
    correlation_id: String,
}

impl TraceAggregator {
    fn new(correlation_id: impl Into<String>) -> Self {
        Self {
            correlation_id: correlation_id.into(),
        }
    }
}

#[task]
impl TraceAggregator {
    async fn run(
        &mut self,
        t1: &Result<TaskTrace, String>,
        t2: &Result<TaskTrace, String>,
        t3: &Result<TaskTrace, String>,
    ) -> TraceSummary {
        let traces = vec![t1, t2, t3];

        let mut summary = TraceSummary {
            correlation_id: self.correlation_id.clone(),
            task_count: traces.len(),
            total_duration_ms: 0,
            success_count: 0,
            failed_count: 0,
            task_details: Vec::new(),
        };

        for trace_result in traces {
            match trace_result {
                Ok(trace) => {
                    summary.success_count += 1;
                    summary.total_duration_ms += trace.duration_ms;
                    summary.task_details.push(trace.clone());
                }
                Err(e) => {
                    summary.failed_count += 1;
                    eprintln!("[{}] Task failed: {}", self.correlation_id, e);
                }
            }
        }

        summary
    }
}

// Summary of traced execution
#[derive(Clone, Debug)]
struct TraceSummary {
    correlation_id: String,
    task_count: usize,
    total_duration_ms: u64,
    success_count: usize,
    failed_count: usize,
    task_details: Vec<TaskTrace>,
}

impl TraceSummary {
    fn print(&self) {
        println!("\n=== Execution Trace Summary ===");
        println!("Correlation ID: {}", self.correlation_id);
        println!("Total Tasks: {}", self.task_count);
        println!("Total Duration: {}ms", self.total_duration_ms);
        println!(
            "Success: {}, Failed: {}",
            self.success_count, self.failed_count
        );

        if !self.task_details.is_empty() {
            println!("\nTask Breakdown:");
            let max_duration = self
                .task_details
                .iter()
                .map(|t| t.duration_ms)
                .max()
                .unwrap_or(0);

            for trace in &self.task_details {
                let slowest_tag = if trace.duration_ms == max_duration {
                    " (slowest)"
                } else {
                    ""
                };
                println!(
                    "  {}: {}ms{}",
                    trace.task_name, trace.duration_ms, slowest_tag
                );
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("=== Debug Tracing Example ===\n");

    // Example 1: Successful order processing with full tracing
    println!("Example 1: Successful order processing\n");
    {
        let correlation_id = "order-12345";
        println!("Starting order processing for {}\n", correlation_id);

        let dag = DagRunner::new();

        // Root task: Validate order
        let _validate = dag.add_task(TracedTask::new(
            "task-001",
            "ValidateOrder",
            correlation_id,
            10,
        ));

        // Parallel validation tasks (independent, no dependencies)
        let inventory = dag.add_task(TracedTask::new(
            "task-002",
            "CheckInventory",
            correlation_id,
            45,
        ));

        let payment = dag.add_task(TracedTask::new(
            "task-003",
            "ValidatePayment",
            correlation_id,
            120,
        ));

        let shipping = dag.add_task(TracedTask::new(
            "task-004",
            "CalculateShipping",
            correlation_id,
            15,
        ));

        // Aggregate traces
        let summary = dag
            .add_task(TraceAggregator::new(correlation_id))
            .depends_on((inventory, payment, shipping));

        // Run the DAG
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();

        // Print summary
        dag.get(summary).unwrap().print();
    }

    println!("\n");

    // Example 2: Order with failed payment
    println!("Example 2: Order with failed payment\n");
    {
        let correlation_id = "order-67890";
        println!("Starting order processing for {}\n", correlation_id);

        let dag = DagRunner::new();

        let _validate = dag.add_task(TracedTaskWithError::new(
            "task-101",
            "ValidateOrder",
            correlation_id,
            false,
        ));

        let _inventory = dag.add_task(TracedTaskWithError::new(
            "task-102",
            "CheckInventory",
            correlation_id,
            false,
        ));

        // This task will fail
        let payment = dag.add_task(TracedTaskWithError::new(
            "task-103",
            "ValidatePayment",
            correlation_id,
            true, // will fail
        ));

        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
            .await
            .unwrap();

        // Check results
        match dag.get(payment).unwrap() {
            Ok(_) => println!("\nPayment validation succeeded"),
            Err(trace) => {
                println!("\n[{}] Payment validation failed:", correlation_id);
                println!("  Task ID: {}", trace.task_id);
                println!("  Duration: {}ms", trace.duration_ms);
                println!("  Status: {}", trace.status);
            }
        }
    }

    println!("\n=== Debug Tracing Example Complete ===");
}
