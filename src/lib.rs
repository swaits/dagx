//! Async DAG Task Runner
//!
//! A minimal, type-safe, runtime-agnostic DAG (Directed Acyclic Graph) executor with
//! compile-time dependency validation and optimal parallel execution.
//!
//! # Features
//!
//! - **Compile-time cycle prevention**: The type system makes cycles **impossible**—no runtime
//!   cycle detection needed! See [the repository documentation](https://github.com/swaits/dagx/blob/main/docs/CYCLE_PREVENTION.md) for a detailed explanation.
//! - **Compile-time type safety**: Dependencies are validated at compile time through the type
//!   system. The public API is fully type-safe with no runtime type errors. Internal execution
//!   uses type erasure for heterogeneous task storage, but this is never exposed to users.
//! - **Runtime-agnostic**: Works with any async runtime (Tokio, smol, Embassy, etc.)
//! - **Optimal execution**: Topological scheduling with maximum safe parallelism
//!
//! # Quick Start
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task, TaskHandle};
//!
//! // Source task with read-only state (tuple struct)
//! struct Value(i32);
//!
//! #[task]
//! impl Value {
//!     async fn run(&self) -> i32 { self.0 }  // Read-only: use &self
//! }
//!
//! // Stateless task (unit struct)
//! struct Add;
//!
//! #[task]
//! impl Add {
//!     async fn run(a: &i32, b: &i32) -> i32 { a + b }  // No self needed!
//! }
//!
//! # async {
//! let mut dag = DagRunner::new();
//!
//! // Add source tasks
//! let x = dag.add_task(Value(2));
//! let y = dag.add_task(Value(3));
//!
//! // Add task with dependencies
//! let sum = dag.add_task(Add).depends_on((x, y));
//!
//! // Execute and retrieve results
//!let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
//! assert_eq!(output.get(sum).unwrap(), 5);
//! # };
//! ```
//!
//! # Implementation Notes
//!
//! ## Inline Execution Fast-Path
//!
//! dagx automatically optimizes execution for sequential workloads using an inline fast-path.
//! When a layer contains only a single task (common in deep chains and linear pipelines),
//! that task executes inline rather than being spawned. This eliminates spawning overhead,
//! and context switching.
//!
//! **Panic handling**: To maintain consistent behavior between runtimes,
//! panics in tasks are caught using `FutureExt::catch_unwind()` and converted to
//! `DagError::TaskPanicked`. This matches the behavior of async runtimes like Tokio.
//!
//! **What this means for you**:
//! - Tasks behave identically whether executed inline or spawned
//! - Panics become errors regardless of execution path
//! - Sequential workloads see 10-100x performance improvements
//! - The optimization is completely transparent - no code changes needed
//!
//! **When inline execution happens**:
//! - Any layer where `layer.len() == 1` after topological ordering
//!
//! ## Dependency Limits
//!
//! dagx supports up to 8 dependencies per task. If you need more than 8 dependencies:
//! 1. Group related inputs into a struct
//! 2. Use intermediate aggregation tasks
//! 3. Consider if 8+ dependencies indicates a design issue
//!
//! # Core Concepts
//!
//! ## Task
//!
//! A [`Task`] is a unit of async work with typed inputs and outputs. Use the `#[task]` macro to generate an implementation.
//!
//! ### Task Patterns
//!
//! dagx supports three task patterns based on state requirements:
//!
//! **1. Stateless** - No state (unit struct, no `self` parameter):
//!
//! ```
//! use dagx::{task, Task};
//!
//! struct Add;
//!
//! #[task]
//! impl Add {
//!     async fn run(a: &i32, b: &i32) -> i32 { a + b }
//! }
//! ```
//!
//! **2. Read-only state** - Immutable access (use `&self`):
//!
//! ```
//! use dagx::{task, Task};
//!
//! struct Scale(i32);
//!
//! #[task]
//! impl Scale {
//!     async fn run(&self, input: &i32) -> i32 {
//!         input * self.0  // Read-only access
//!     }
//! }
//! ```
//!
//! **3. Mutable state** - State modification (use `&mut self`):
//!
//! ```
//! use dagx::{task, Task};
//!
//! struct Counter(i32);
//!
//! #[task]
//! impl Counter {
//!     async fn run(&mut self, input: &i32) -> i32 {
//!         self.0 += input;  // Modifies state
//!         self.0
//!     }
//! }
//! ```
//!
//! ## DagRunner
//!
//! The [`DagRunner`] orchestrates task execution. Add tasks with [`DagRunner::add_task`],
//! wire dependencies with [`TaskBuilder::depends_on`], then run everything with [`DagRunner::run`].
//!
//! ## TaskHandle
//!
//! A [`TaskHandle<T>`] is a typed, opaque reference to a task's output. Use it to:
//! - Wire dependencies between tasks
//! - Retrieve results after execution with [`DagOutput::get`]
//!
//! ## Dependency Patterns
//!
//! dagx supports three dependency patterns:
//!
//! ### No Dependencies (Source Tasks)
//!
//! Tasks with no dependencies return a [`TaskHandle`] directly and don't call `depends_on()`:
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! # async {
//! # let mut dag = DagRunner::new();
//! let source = dag.add_task(Value(42));
//! # };
//! ```
//!
//! ### Single Dependency
//!
//! Tasks with a single dependency receive a reference to that value:
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! struct Double;
//! #[task]
//! impl Double {
//!     async fn run(input: &i32) -> i32 { input * 2 }
//! }
//! # async {
//! # let mut dag = DagRunner::new();
//! # let source = dag.add_task(Value(42));
//! let doubled = dag.add_task(Double).depends_on(source);
//! # };
//! ```
//!
//! To manually implement [`Task`] for a single-dependency type, you must implement [`Task<(T,)>`], not [`Task<T>`].
//!
//! ### Multiple Dependencies
//!
//! Tasks with multiple dependencies receive separate reference parameters (order matters!):
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! struct Add;
//! #[task]
//! impl Add {
//!     async fn run(a: &i32, b: &i32) -> i32 { a + b }
//! }
//! # async {
//! # let mut dag = DagRunner::new();
//! # let x = dag.add_task(Value(2));
//! # let y = dag.add_task(Value(3));
//! let sum = dag.add_task(Add).depends_on((x, y));
//! # };
//! ```
//!
//! # Examples
//!
//! ## Fan-out Pattern (1 → n)
//!
//! One task produces a value consumed by multiple downstream tasks:
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task};
//!
//!
//! // Source task (tuple struct)
//! struct Value(i32);
//!
//! #[task]
//! impl Value {
//!     async fn run(&self) -> i32 { self.0 }
//! }
//!
//! // Stateful task (tuple struct)
//! struct Add(i32);
//!
//! #[task]
//! impl Add {
//!     async fn run(&self, input: &i32) -> i32 { input + self.0 }
//! }
//!
//! // Stateful task (tuple struct)
//! struct Scale(i32);
//!
//! #[task]
//! impl Scale {
//!     async fn run(&self, input: &i32) -> i32 { input * self.0 }
//! }
//!
//! # async {
//! let mut dag = DagRunner::new();
//!
//! let base = dag.add_task(Value(10));
//! let plus1 = dag.add_task(Add(1)).depends_on(base);
//! let times2 = dag.add_task(Scale(2)).depends_on(base);
//!
//!let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
//!
//! assert_eq!(output.get(plus1).unwrap(), 11);
//! assert_eq!(output.get(times2).unwrap(), 20);
//! # };
//! ```
//!
//! ## Fan-in Pattern (m → 1)
//!
//! Multiple tasks produce values consumed by a single downstream task:
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task};
//!
//!
//! // Source task for String (tuple struct)
//! struct Name(String);
//!
//! #[task]
//! impl Name {
//!     async fn run(&self) -> String { self.0.clone() }
//! }
//!
//! // Source task for i32 (tuple struct)
//! struct Age(i32);
//!
//! #[task]
//! impl Age {
//!     async fn run(&self) -> i32 { self.0 }
//! }
//!
//! // Source task for bool (tuple struct)
//! struct Active(bool);
//!
//! #[task]
//! impl Active {
//!     async fn run(&self) -> bool { self.0 }
//! }
//!
//! // Stateless formatter (unit struct)
//! struct FormatUser;
//!
//! #[task]
//! impl FormatUser {
//!     async fn run(n: &String, a: &i32, f: &bool) -> String {
//!         format!("User: {n}, Age: {a}, Active: {f}")
//!     }
//! }
//!
//! # async {
//! let mut dag = DagRunner::new();
//!
//! let name = dag.add_task(Name("Alice".to_string()));
//! let age = dag.add_task(Age(30));
//! let active = dag.add_task(Active(true));
//! let result = dag.add_task(FormatUser).depends_on((name, age, active));
//!
//!let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
//!
//! assert_eq!(output.get(result).unwrap(), "User: Alice, Age: 30, Active: true");
//! # };
//! ```
//!
//! # Runtime Agnostic
//!
//! dagx works with any async runtime. The library has been tested with:
//! - **Tokio** - Most popular async runtime
//! - **smol** - Lightweight async runtime
//!
//! Examples with different runtimes:
//!
//! ```ignore
//! // With Tokio
//! #[tokio::main]
//! async fn main() {
//!     let mut dag = DagRunner::new();
//!     // ... build DAG
//!     let result = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await;
//! }
//!
//! // With smol
//! fn main() {
//!     smol::block_on(async {
//!         let mut dag = DagRunner::new();
//!         // ... build and run DAG
//!         let result = dag.run(|fut| smol::spawn(fut)).await;
//!     });
//! }
//! ```
//!
//! # Error Handling
//!
//! dagx uses [`DagResult<T>`] (an alias for `Result<T, DagError>`) for operations that
//! can fail:
//!
//! - [`DagRunner::run`] returns `DagResult<DagOutput>` and can fail if tasks panic or if multiple
//!   concurrent runs are attempted
//! - [`DagOutput::get`] returns `DagResult<T>` and can fail if the task hasn't executed or
//!   the handle is invalid
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! #
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! # async {
//! # let mut dag = DagRunner::new();
//! # let node = dag.add_task(Value(42));
//! // Simple approach with .unwrap()
//! let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
//! let result = output.get(node).unwrap();
//! # let mut dag = DagRunner::new();
//!
//! // Or handle errors explicitly
//! match dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await {
//!     Ok(_) => println!("DAG executed successfully"),
//!     Err(e) => eprintln!("DAG execution failed: {}", e),
//! }
//! # };
//! ```
//! # API Design Principles
//!
//! dagx follows these API design principles:
//!
//! 1. **Type Safety**: Dependencies validated at compile time via type-state pattern
//! 2. **Builder Pattern**: Fluent interface with `add_task().depends_on()`
//! 3. **Error Handling**: All fallible operations return `DagResult<T>`
//! 4. **Minimal Surface**: Small, focused API
//!
//! # Performance Characteristics
//!
//! dagx is designed for minimal overhead and optimal parallel execution.
//!
//! See the [repository](https://github.com/swaits/dagx) for up-to-date benchmarks.
//!
//! ## Performance Tips
//!
//! ### Automatic Arc Wrapping (No Manual Arc Needed!)
//!
//! **Task outputs are automatically wrapped in `Arc<T>` internally** for efficient fan-out patterns.
//! You output `T`, the framework handles the Arc wrapping:
//!
//! ```
//! # use dagx::{task, DagRunner, Task};
//! #
//!
//! // ✅ CORRECT: Just output Vec<String>, framework wraps in Arc internally
//! struct FetchData;
//! #[task]
//! impl FetchData {
//!     async fn run() -> Vec<String> {
//!         vec!["data".to_string(); 10_000]
//!     }
//! }
//!
//! // Downstream tasks receive &Vec<String> as normal
//! // Behind the scenes: Arc<Vec<String>> is cloned cheaply, then a reference to the inner Vec is extracted
//! struct ProcessData;
//! #[task]
//! impl ProcessData {
//!     async fn run(data: &Vec<String>) -> usize {
//!         data.len()
//!     }
//! }
//!
//! # async {
//! let mut dag = DagRunner::new();
//! let data = dag.add_task(FetchData);
//!
//! // All three tasks get efficient Arc-wrapped sharing automatically
//! let task1 = dag.add_task(ProcessData).depends_on(data);
//! let task2 = dag.add_task(ProcessData).depends_on(data);
//! let task3 = dag.add_task(ProcessData).depends_on(data);
//!
//!let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
//! # };
//! ```
//!
//! **How it works:**
//! - Your task outputs `T` (e.g., `Vec<String>`)
//! - Framework wraps it in `Arc<T>` internally
//! - For fan-out (1→N), Arc is cloned N times
//! - Each downstream task receives `&T` after extracting from Arc
//!
//! ### Other Tips
//!
//! 1. **Minimize task count**: Combine small operations into larger tasks
//! 2. **Use appropriate granularity**: Don't create tasks for trivial work (< 1µs)
//!
//! Run `cargo bench` to see comprehensive benchmarks including basic operations, scaling
//! characteristics, common patterns (fan-out, diamond), and realistic workloads.
//!
//! # Optional Tracing Support
//!
//! dagx provides optional observability through the `tracing` crate. It can be enabled with the `tracing` feature flag.
//!
//! ## Log Levels
//!
//! - **INFO**: DAG execution start/completion
//! - **DEBUG**: Task additions, dependency wiring, layer computation
//! - **TRACE**: Individual task execution (inline vs spawned), detailed execution flow
//! - **ERROR**: Task panics, concurrent execution attempts
//!
//! Control log level with the `RUST_LOG` environment variable:
//!
//! ```bash
//! RUST_LOG=dagx=info  cargo run    # High-level execution info
//! RUST_LOG=dagx=debug cargo run    # Task and layer details
//! RUST_LOG=dagx=trace cargo run    # All execution details
//! ```

extern crate self as dagx;

mod builder;
mod deps;
mod error;
mod node;
mod output;
mod runner;
mod task;

// Public re-exports
pub use builder::{TaskBuilder, TaskHandle};
pub use error::{DagError, DagResult};
pub use output::DagOutput;
pub use runner::DagRunner;
pub use task::{Task, TaskInput};

// Re-export the procedural macro
#[cfg(feature = "derive")]
pub use dagx_macros::task;
