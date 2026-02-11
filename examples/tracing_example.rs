//! # Tracing Example
//!
//! This example demonstrates how to use the optional tracing feature to gain
//! visibility into DAG execution for debugging and performance analysis.
//!
//! ## Running This Example
//!
//! ```bash
//! # With default log level (info)
//! cargo run --example tracing_example --features tracing
//!
//! # With debug level (shows more details)
//! RUST_LOG=dagx=debug cargo run --example tracing_example --features tracing
//!
//! # With trace level (shows all execution details)
//! RUST_LOG=dagx=trace cargo run --example tracing_example --features tracing
//! ```
//!
//! ## What You'll See
//!
//! - Task additions to the DAG
//! - Dependency wiring between tasks
//! - Topological layer computation
//! - Layer-by-layer execution
//! - Inline vs spawned task execution
//! - Completion status
//!
//! ## Log Levels
//!
//! - **INFO**: High-level DAG execution start/completion
//! - **DEBUG**: Task additions, dependency wiring, layer computation
//! - **TRACE**: Individual task execution details (inline vs spawned)
//! - **ERROR**: Errors, panics, cycle detection
//!

use dagx::{task, DagRunner};

use tracing_subscriber::{fmt, EnvFilter};

// Simple source task
struct Value(i32);

#[task]
impl Value {
    async fn run(&self) -> i32 {
        self.0
    }
}

// Computation task
struct Add;

#[task]
impl Add {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b
    }
}

// Another computation task
struct Multiply;

#[task]
impl Multiply {
    async fn run(a: &i32, b: &i32) -> i32 {
        a * b
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber with environment filter
    // Use RUST_LOG environment variable to control log level
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dagx=info")),
        )
        .init();

    println!("=== DAG Execution with Tracing ===\n");
    println!("Run with RUST_LOG=dagx=debug or RUST_LOG=dagx=trace for more details\n");

    // Example 1: Simple diamond pattern
    println!("Example 1: Diamond Pattern");
    println!("---------------------------");
    {
        let dag = DagRunner::new();

        // Layer 0: Source
        let source = dag.add_task(Value(10)).into();

        // Layer 1: Two parallel computations
        let left = dag.add_task(Add).depends_on((&source, &source));
        let right = dag.add_task(Multiply).depends_on((&source, &source));

        // Layer 2: Combine results
        let result = dag.add_task(Add).depends_on((&left, &right));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("\nResult: {}", dag.get(result).unwrap());
        println!("(Expected: 10+10 = 20, 10*10 = 100, 20+100 = 120)\n");
    }

    // Example 2: Linear chain (demonstrates inline execution)
    println!("\nExample 2: Linear Chain (Inline Execution)");
    println!("-------------------------------------------");
    {
        let dag = DagRunner::new();

        // Create a linear chain - each layer has only one task
        // This should trigger inline execution optimization
        let a = dag.add_task(Value(1)).into();
        let b = dag.add_task(Add).depends_on((&a, &a));
        let c = dag.add_task(Multiply).depends_on((&b, &b));
        let d = dag.add_task(Add).depends_on((&c, &c));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("\nResult: {}", dag.get(d).unwrap());
        println!("(Expected: 1+1 = 2, 2*2 = 4, 4+4 = 8)\n");
    }

    // Example 3: Wide pattern (demonstrates spawned execution)
    println!("\nExample 3: Wide Pattern (Spawned Execution)");
    println!("--------------------------------------------");
    {
        let dag = DagRunner::new();

        // Layer 0: Multiple sources
        let v1 = dag.add_task(Value(1)).into();
        let v2 = dag.add_task(Value(2)).into();
        let v3 = dag.add_task(Value(3)).into();
        let v4 = dag.add_task(Value(4)).into();

        // Layer 1: Multiple parallel computations
        let sum12 = dag.add_task(Add).depends_on((&v1, &v2));
        let sum23 = dag.add_task(Add).depends_on((&v2, &v3));
        let sum34 = dag.add_task(Add).depends_on((&v3, &v4));

        // Layer 2: Final combination
        let sum_all = dag.add_task(Add).depends_on((&sum12, &sum23));
        let final_result = dag.add_task(Add).depends_on((&sum_all, &sum34));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("\nResult: {}", dag.get(final_result).unwrap());
        println!("(Expected: (1+2)+(2+3) = 8, 8+(3+4) = 15)\n");
    }

    println!("\n=== Tracing Example Complete ===");
    println!("\nTry running with different log levels:");
    println!("  RUST_LOG=dagx=info  - High-level execution info");
    println!("  RUST_LOG=dagx=debug - Task additions and layer details");
    println!("  RUST_LOG=dagx=trace - All execution details including inline/spawned");
}
