//! Async DAG Task Runner
//!
//! A minimal, type-safe, runtime-agnostic DAG (Directed Acyclic Graph) executor with
//! compile-time dependency validation and optimal parallel execution.
//!
//! # Features
//!
//! - **Compile-time cycle prevention**: The type system makes cycles **impossible**—no runtime
//!   cycle detection needed! See [`cycle_prevention`] module for detailed explanation and proof.
//! - **Compile-time type safety**: Dependencies are validated at compile time through the type
//!   system. The public API is fully type-safe with no runtime type errors. Internal execution
//!   uses type erasure for heterogeneous task storage, but this is never exposed to users.
//! - **Works with ANY type**: Custom types work automatically with the `#[task]` macro—just
//!   implement `Clone + Send + Sync`, no trait implementations needed! The macro generates
//!   type-specific extraction logic for seamless integration.
//! - **Runtime-agnostic**: Works with any async runtime (Tokio, async-std, smol, Embassy, etc.)
//! - **Type-state pattern**: The API guides you with compile-time errors if you wire dependencies
//!   incorrectly. This is what prevents cycles at compile time.
//! - **Optimal execution**: Topological scheduling with maximum safe parallelism
//! - **Zero-cost abstractions**: Leverages generics and monomorphization for minimal overhead
//! - **Error handling**: Result-based error handling with [`DagResult<T>`] for validation
//!
//! # Quick Start
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task, TaskHandle};
//! use futures::FutureExt; // for FutureExt::map
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
//! let dag = DagRunner::new();
//!
//! // Add source tasks
//! let x = dag.add_task(Value(2));
//! let y: TaskHandle<_> = dag.add_task(Value(3)).into(); // Allows y to be reused as a dependency
//!
//! // Add task with dependencies
//! let sum = dag.add_task(Add).depends_on((x, &y));
//!
//! // Execute and retrieve results
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//! assert_eq!(dag.get(sum).unwrap(), 5);
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
//! **Panic handling**: To maintain consistent behavior between inline and spawned execution,
//! panics in inline tasks are caught using `FutureExt::catch_unwind()` and converted to
//! `DagError::TaskPanicked`. This matches the behavior of async runtimes like Tokio, async-std,
//! and smol, which catch panics in spawned tasks and convert them to errors.
//!
//! **What this means for you**:
//! - Tasks behave identically whether executed inline or spawned
//! - Panics become errors regardless of execution path
//! - Sequential workloads see 10-100x performance improvements
//! - The optimization is completely transparent - no code changes needed
//!
//! **When inline execution happens**:
//! - Single-task layers (e.g., long sequential chains)
//! - Linear pipelines (A→B→C→D)
//! - Any layer where `layer.len() == 1` after topological ordering
//!
//! ## Dependency Limits
//!
//! dagx supports up to 8 dependencies per task. If you need more than 8 dependencies:
//! 1. Group related inputs into a struct
//! 2. Use intermediate aggregation tasks
//! 3. Consider if 8+ dependencies indicates a design issue
//!
//! ## Task Output Limitations
//!
//! **IMPORTANT**: Tasks **cannot return bare tuples** as output types. This is a technical
//! limitation of the current implementation. If you need to return multiple values, use one
//! of these workarounds:
//!
//! ### Workaround 1: Wrap in Result (Recommended for Fallible Operations)
//!
//! ```
//! use dagx::{task, Task};
//!
//! struct ProcessData;
//! #[task]
//! impl ProcessData {
//!     async fn run(input: &String) -> Result<(String, i32, bool), String> {
//!         // Wrap the tuple in Result - this works!
//!         Ok(("Alice".to_string(), 30, true))
//!     }
//! }
//! ```
//!
//! ### Workaround 2: Use a Struct (Recommended for Most Cases)
//!
//! ```
//! use dagx::{task, Task};
//!
//! // Define a struct to hold your multiple values
//! struct UserData {
//!     name: String,
//!     age: i32,
//!     active: bool,
//! }
//!
//! struct ProcessDataStruct;
//! #[task]
//! impl ProcessDataStruct {
//!     async fn run(input: &String) -> UserData {
//!         // Return the struct - clean and self-documenting
//!         UserData {
//!             name: "Alice".to_string(),
//!             age: 30,
//!             active: true,
//!         }
//!     }
//! }
//! ```
//!
//! ### Why Use Structs Over `Result<(...)>`?
//!
//! While both workarounds solve the technical limitation, **structs are the better choice**
//! for most cases:
//!
//! - **Self-documenting**: `user.name` is clearer than `user.0`
//! - **Refactorable**: Easy to add/remove fields without breaking all dependencies
//! - **Type-safe**: Named fields prevent field order mistakes
//! - **Semantic clarity**: `Result` should indicate fallibility, not just wrap data
//!
//! Use `Result<(...), E>` only when your operation can genuinely fail and you need to
//! return multiple values on success
//!
//! # Core Concepts
//!
//! ## Task
//!
//! A [`Task`] is a unit of async work with typed inputs and outputs. Use the `#[task]` macro.
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
//! - Retrieve results after execution with [`DagRunner::get`]
//!
//! ## Dependency Patterns
//!
//! dagx supports three dependency patterns:
//!
//! ### No Dependencies (Source Tasks)
//!
//! Tasks with no dependencies don't call `depends_on()`:
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! # async {
//! # let dag = DagRunner::new();
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
//! # let dag = DagRunner::new();
//! # let source = dag.add_task(Value(42));
//! let doubled = dag.add_task(Double).depends_on(source);
//! # };
//! ```
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
//! # let dag = DagRunner::new();
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
//! use futures::FutureExt;
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
//! let dag = DagRunner::new();
//!
//! let base = dag.add_task(Value(10)).into();
//! let plus1 = dag.add_task(Add(1)).depends_on(&base);
//! let times2 = dag.add_task(Scale(2)).depends_on(&base);
//!
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//!
//! assert_eq!(dag.get(plus1).unwrap(), 11);
//! assert_eq!(dag.get(times2).unwrap(), 20);
//! # };
//! ```
//!
//! ## Fan-in Pattern (m → 1)
//!
//! Multiple tasks produce values consumed by a single downstream task:
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task};
//! use futures::FutureExt;
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
//! let dag = DagRunner::new();
//!
//! let name = dag.add_task(Name("Alice".to_string()));
//! let age = dag.add_task(Age(30));
//! let active = dag.add_task(Active(true));
//! let result = dag.add_task(FormatUser).depends_on((name, age, active));
//!
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//!
//! assert_eq!(dag.get(result).unwrap(), "User: Alice, Age: 30, Active: true");
//! # };
//! ```
//!
//! ## Many-to-Many Pattern (m ↔ n)
//!
//! Complex DAGs with multiple layers and dependencies:
//!
//! ```no_run
//! use dagx::{task, DagRunner, Task};
//! use futures::FutureExt;
//!
//! // Source task (tuple struct)
//! struct Value(i32);
//!
//! #[task]
//! impl Value {
//!     async fn run(&self) -> i32 { self.0 }
//! }
//!
//! // Stateless addition (unit struct)
//! struct Add;
//!
//! #[task]
//! impl Add {
//!     async fn run(a: &i32, b: &i32) -> i32 { a + b }
//! }
//!
//! // Stateless multiplication (unit struct)
//! struct Multiply;
//!
//! #[task]
//! impl Multiply {
//!     async fn run(a: &i32, b: &i32) -> i32 { a * b }
//! }
//!
//! # async {
//! let dag = DagRunner::new();
//!
//! // Layer 1: Sources
//! let x = dag.add_task(Value(2));
//! let y = dag.add_task(Value(3)).into();
//! let z = dag.add_task(Value(5));
//!
//! // Layer 2: Intermediate computations
//! let sum_xy = dag.add_task(Add).depends_on((x, &y));  // 2 + 3 = 5
//! let prod_yz = dag.add_task(Multiply).depends_on((&y, z)); // 3 * 5 = 15
//!
//! // Layer 3: Final result
//! let total = dag.add_task(Add).depends_on((&sum_xy, &prod_yz)); // 5 + 15 = 20
//!
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//!
//! assert_eq!(dag.get(total).unwrap(), 20);
//! # };
//! ```
//!
//! ## Custom Types
//!
//! **dagx works with ANY type automatically!** As long as your type implements `Clone + Send + Sync + 'static`,
//! the `#[task]` macro generates the necessary extraction logic:
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # use futures::FutureExt;
//!
//! // Just derive Clone - that's all you need!
//! #[derive(Clone)]
//! struct User {
//!     name: String,
//!     age: u32,
//! }
//!
//! struct CreateUser;
//! #[task]
//! impl CreateUser {
//!     async fn run() -> User {
//!         User { name: "Alice".to_string(), age: 30 }
//!     }
//! }
//!
//! struct FormatUser;
//! #[task]
//! impl FormatUser {
//!     async fn run(user: &User) -> String {
//!         format!("{} is {} years old", user.name, user.age)
//!     }
//! }
//!
//! # async {
//! let dag = DagRunner::new();
//!
//! let user = dag.add_task(CreateUser);
//! let formatted = dag.add_task(FormatUser).depends_on(user);
//!
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//!
//! assert_eq!(dag.get(formatted).unwrap(), "Alice is 30 years old");
//! # };
//! ```
//!
//! **No trait implementations needed!** Works with nested structs, collections, enums, and any
//! other custom type. See [`examples/custom_types.rs`](https://github.com/swaits/dagx/blob/main/examples/custom_types.rs)
//! for a complete example with complex nested types.
//!
//! # Runtime Agnostic
//!
//! dagx works with any async runtime. The library has been tested with:
//! - **Tokio** - Most popular async runtime
//! - **async-std** - Alternative async runtime
//! - **smol** - Lightweight async runtime
//!
//! Examples with different runtimes:
//!
//! ```ignore
//! // With Tokio
//! #[tokio::main]
//! async fn main() {
//!     let dag = DagRunner::new();
//!     // ... build and run DAG
//! }
//!
//! // With async-std
//! #[async_std::main]
//! async fn main() {
//!     let dag = DagRunner::new();
//!     // ... build and run DAG
//! }
//!
//! // With smol
//! fn main() {
//!     smol::block_on(async {
//!         let dag = DagRunner::new();
//!         // ... build and run DAG
//!     });
//! }
//! ```
//!
//! # Error Handling
//!
//! dagx uses [`DagResult<T>`] (an alias for `Result<T, DagError>`) for operations that
//! can fail:
//!
//! - [`DagRunner::run`] returns `DagResult<()>` and can fail if tasks panic or if multiple
//!   concurrent runs are attempted
//! - [`DagRunner::get`] returns `DagResult<T>` and can fail if the task hasn't executed or
//!   the handle is invalid
//!
//! You can handle errors explicitly or use `.unwrap()` for simple cases:
//!
//! ```no_run
//! # use dagx::{task, DagRunner, Task};
//! # use futures::FutureExt;
//! # struct Value(i32);
//! # #[task]
//! # impl Value {
//! #     async fn run(&self) -> i32 { self.0 }
//! # }
//! # async {
//! # let dag = DagRunner::new();
//! # let node = dag.add_task(Value(42));
//! // Simple approach with .unwrap()
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//! let result = dag.get(node).unwrap();
//!
//! // Or handle errors explicitly
//! match dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await {
//!     Ok(_) => println!("DAG executed successfully"),
//!     Err(e) => eprintln!("DAG execution failed: {}", e),
//! }
//! # };
//! ```
//!
//! # Documentation Accuracy
//!
//! This library maintains documentation accuracy through:
//!
//! 1. **Doctests**: All code examples are tested via `cargo test --doc`
//! 2. **Examples**: All files in `examples/` are tested via `cargo build --examples`
//! 3. **Type signatures**: Documentation reflects actual API return types
//! 4. **Error handling**: All examples show proper `DagResult<T>` handling
//! 5. **Safety claims**: All safety guarantees are verified by tests
//!
//! Last audited: 2025-10-06 for v0.1.0
//!
//! Changes that require documentation updates:
//! - API signature changes (`run()`, `get()`, etc.)
//! - New error types or error handling changes
//! - Safety guarantee changes
//! - Example code updates
//!
//! # API Design Principles
//!
//! dagx follows these API design principles:
//!
//! 1. **Type Safety**: Dependencies validated at compile time via type-state pattern
//! 2. **Builder Pattern**: Fluent interface with `add_task().depends_on()`
//! 3. **Error Handling**: All fallible operations return `DagResult<T>`
//! 4. **Minimal Surface**: Small, focused API with 4 core types and 1 trait
//! 5. **Zero Cost**: Leverages generics for monomorphization
//! 6. **Consistency**:
//!    - All types are PascalCase
//!    - All methods are snake_case
//!    - Mutable operations take `&mut self`
//!    - Builder methods consume `self`
//!    - Results are always `DagResult<T>`
//!
//! # Performance Characteristics
//!
//! dagx is designed for minimal overhead and optimal parallel execution.
//!
//! ## Benchmarks
//!
//! Run benchmarks with:
//! ```bash
//! cargo bench
//! ```
//!
//! View the detailed HTML reports:
//! ```bash
//! # macOS
//! open target/criterion/report/index.html
//!
//! # Linux
//! xdg-open target/criterion/report/index.html
//!
//! # Windows
//! start target/criterion/report/index.html
//!
//! # Or manually open target/criterion/report/index.html in your browser
//! ```
//!
//! Typical performance on modern hardware (AMD 7840U - Zen 4 laptop CPU):
//! - **DAG creation**: ~20ns empty DAG, ~96ns per task added
//! - **Execution overhead**: ~119ns per task (framework coordination only)
//! - **Total overhead** (construction + execution): ~223ns per task
//! - **Scaling**: Linear overhead with task count - 100 tasks in ~123µs, 1000 tasks in ~1.15ms
//!
//! For real-world workloads where tasks perform meaningful work (I/O, computation), the framework
//! overhead is typically well under 1% of total execution time.
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
//! # use futures::FutureExt;
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
//! let dag = DagRunner::new();
//! let data = dag.add_task(FetchData).into();
//!
//! // All three tasks get efficient Arc-wrapped sharing automatically
//! let task1 = dag.add_task(ProcessData).depends_on(&data);
//! let task2 = dag.add_task(ProcessData).depends_on(&data);
//! let task3 = dag.add_task(ProcessData).depends_on(&data);
//!
//! dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
//! # };
//! ```
//!
//! **How it works:**
//! - Your task outputs `T` (e.g., `Vec<String>`)
//! - Framework wraps it in `Arc<T>` internally
//! - For fan-out (1→N), Arc is cloned N times (just atomic pointer increments - O(1))
//! - Each downstream task receives `&T` after extracting from Arc
//! - When you call `dag.get()`, the Arc is cloned and inner value extracted
//!
//! **Performance characteristics:**
//! - **Heap types** (Vec, String, HashMap): Arc overhead is negligible (~few ns)
//! - **Copy types** (i32, usize): Small Arc overhead but usually still negligible
//! - See `cargo bench` for actual measurements on your hardware
//!
//! **Advanced - Zero-copy optimization:**
//! If you want true zero-copy sharing (no extraction), output `Arc<T>` explicitly:
//! ```
//! # use dagx::{task, Task};
//! # use std::sync::Arc;
//! struct ZeroCopyData;
//! #[task]
//! impl ZeroCopyData {
//!     // Output Arc<T> directly
//!     async fn run() -> Arc<Vec<String>> {
//!         Arc::new(vec!["data".to_string(); 10_000])
//!     }
//! }
//!
//! struct ZeroCopyProcess;
//! #[task]
//! impl ZeroCopyProcess {
//!     // Receive &Arc<T> - just clones the Arc pointer, no data extraction
//!     async fn run(data: &Arc<Vec<String>>) -> usize {
//!         data.len()
//!     }
//! }
//! ```
//! This becomes `Arc<Arc<T>>` internally, but ExtractInput unwraps one layer automatically.
//! Use this when you have many dependents AND want to avoid the clone() of inner data
//!
//! ### Other Tips
//!
//! 1. **Minimize task count**: Combine small operations into larger tasks
//! 2. **Use appropriate granularity**: Don't create tasks for trivial work (< 1µs)
//! 3. **Parallel execution**: dagx automatically maximizes parallelism
//!
//! Run `cargo bench` to see comprehensive benchmarks including basic operations, scaling
//! characteristics, common patterns (fan-out, diamond), and realistic workloads.
//!
//! # Optional Tracing Support
//!
//! dagx provides optional observability through the `tracing` crate with **zero runtime overhead
//! when disabled**. The tracing instrumentation is conditionally compiled using feature flags.
//!
//! ## Enabling Tracing
//!
//! Add the `tracing` feature to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! dagx = { version = "0.2", features = ["tracing"] }
//! tracing-subscriber = "0.3"
//! ```
//!
//! Then initialize a tracing subscriber in your application:
//!
//! ```no_run
//! use tracing_subscriber::{fmt, EnvFilter};
//!
//! fmt()
//!     .with_env_filter(
//!         EnvFilter::try_from_default_env()
//!             .unwrap_or_else(|_| EnvFilter::new("dagx=info"))
//!     )
//!     .init();
//! ```
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
//!
//! ## Zero-Cost Guarantee
//!
//! When the `tracing` feature is disabled (the default), there is **literally 0ns overhead**:
//! - Logging code is removed at compile time via `#[cfg(feature = "tracing")]`
//! - No branches, no function calls, nothing in the compiled binary
//! - The `tracing` crate isn't even linked
//! - Benchmarks verify identical performance with/without the feature
//!
//! This follows the same zero-cost pattern used by Tokio, Hyper, and other performance-critical
//! Rust async libraries.
//!
//! See [`examples/tracing_example.rs`](https://github.com/swaits/dagx/blob/main/examples/tracing_example.rs)
//! for a complete working example.
//!

#![allow(private_bounds, private_interfaces)]

extern crate self as dagx;

// Module declarations
mod builder;
pub mod cycle_prevention;
mod deps;
mod error;
mod extract;
mod node;
mod runner;
mod task;
mod types;

// Public re-exports
pub use builder::TaskBuilder;
pub use error::{DagError, DagResult};
pub use runner::DagRunner;
pub use task::{Task, TaskInput};
pub use types::{Pending, TaskHandle};

// Re-export the procedural macro
pub use dagx_macros::task;

// Internal testing support - not part of the public API
#[doc(hidden)]
pub use task::task_fn;
