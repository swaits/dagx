//! # Compile-Time Cycle Prevention
//!
//! This module documents how the type system prevents cycles in the DAG.
//! **Cycles are impossible via the public API** due to the type-state pattern.
//!
//! ## Why Cycles Can't Happen
//!
//! 1. [`crate::builder::TaskBuilder::depends_on`] **consumes** the builder (takes `self` by value)
//! 2. It returns a [`crate::types::TaskHandle`] - an immutable, opaque reference
//! 3. [`crate::types::TaskHandle`] has **no methods** to add or modify dependencies
//! 4. You can only wire dependencies **once** per task
//!
//! ## The Catch-22
//!
//! To create a cycle A→B→A, you would need:
//! - Task A to depend on Task B's handle (requires B finalized)
//! - Task B to depend on Task A's handle (requires A finalized)
//!
//! But you can't finalize both before wiring them together!
//!
//! ## Proof by Compilation Failure
//!
//! The following examples demonstrate that cycles fail to compile:
//!
//! ### Proof 1: Self-Loop Prevention via Move Semantics
//!
//! ```compile_fail
//! use dagx::*;
//!
//! struct TaskA;
//! #[dagx::task]
//! impl TaskA {
//!     async fn run(&self, input: &i32) -> i32 {
//!         input + 1
//!     }
//! }
//!
//! async fn self_loop() {
//!     let dag = DagRunner::new();
//!     let a_builder = dag.add_task(TaskA);
//!
//!     // ERROR: a_builder moved in the call to depends_on
//!     // Can't use 'a_builder' as both the builder and the dependency
//!     let a = a_builder.depends_on(&a_builder);
//! }
//! ```
//!
//! ### Proof 2: TaskHandle is Immutable
//!
//! ```compile_fail,E0599
//! use dagx::*;
//!
//! struct NoInput;
//! #[dagx::task]
//! impl NoInput {
//!     async fn run(&self) -> i32 { 42 }
//! }
//!
//! async fn unit_cycle() {
//!     let dag = DagRunner::new();
//!     let a_builder = dag.add_task(NoInput);
//!     let b_builder = dag.add_task(NoInput);
//!
//!     // We can convert to handles (unit input = no dependencies required)
//!     let a_handle: TaskHandle<i32> = a_builder.into();
//!     let b_handle: TaskHandle<i32> = b_builder.into();
//!
//!     // But TaskHandle has no methods to add dependencies!
//!     // ERROR: no method named `depends_on` found for type `TaskHandle<i32>`
//!     let _ = a_handle.depends_on(&b_handle);
//! }
//! ```
//!
//! ## Implications
//!
//! Because cycles are impossible at compile-time:
//! - No runtime cycle detection is needed
//! - Topological sort will always succeed
//! - The DAG structure is guaranteed by the type system
//!
//! This is an example of **zero-cost abstraction** - the type system
//! provides safety guarantees without runtime overhead.
