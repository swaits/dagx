# Compile-Time Cycle Prevention

**dagx guarantees acyclic graphs at compile time with zero runtime overhead.** Unlike other DAG libraries that detect cycles at runtime, dagx uses Rust's type system to make cycles impossible to express in the first place.

## Why This Matters

Most DAG libraries check for cycles when you execute the graph:

```rust
// Other libraries: cycle detected at RUNTIME
let mut dag = DagRunner::new();
let a = dag.add_task(TaskA);
let b = dag.add_task(TaskB);
a.depends_on(b);
b.depends_on(a);  // Compiles fine...
dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
```

**dagx prevents this at compile time:**

```rust
// dagx: cycle prevented at COMPILE TIME
let mut dag = DagRunner::new();
let a_builder = dag.add_task(TaskA);
let b_builder = dag.add_task(TaskB);

// To make A depend on B, we need B as a TaskHandle.
// But calling depends_on() consumes the builder!
let b_handle = b_builder.depends_on(some_source);

// Now we can't make B depend on A because b_builder was moved!
// This won't compile:
// a_builder.depends_on(b_handle);  // ✓ OK: A→B
// b_handle.depends_on(a_handle);   // ❌ ERROR: TaskHandle has no depends_on() method!
```

## How It Works: The Type-State Pattern

dagx uses two types to enforce acyclic structure:

1. **`TaskBuilder<T, Deps>`**: Mutable builder that can have dependencies added via `depends_on()`
   - Consumed (moved) when you call `depends_on()` or convert to `TaskHandle`
   - Can only be used once for wiring

2. **`TaskHandle<T>`**: Immutable reference to a finalized task
   - Copy type (just wraps a task ID)
   - Can be used as a dependency for other tasks
   - **Has no `depends_on()` method** - can't be modified after creation

This creates an impossible ordering constraint for cycles:

```rust
// Attempting a cycle A→B→A:
let a_builder = dag.add_task(TaskA);  // TaskBuilder
let b_builder = dag.add_task(TaskB);  // TaskBuilder

// To wire A→B, we need B as a TaskHandle
let b_handle = b_builder.into();  // Convert B to TaskHandle (consumes builder)

// To wire B→A, we need A as a TaskHandle
let a_handle = a_builder.depends_on(b_handle);  // A now depends on B

// Now if we try to complete the cycle:
// a_handle.depends_on(...);  // ❌ ERROR: no method named `depends_on` found
```

**The type system enforces strict topological ordering!** You can't create a `TaskHandle` until all its dependencies exist as handles, which prevents cycles.

## Proof & Examples

See [`tests/cycle_prevention.rs`](../tests/cycle_prevention.rs) for runtime tests demonstrating the type-state pattern in action.

## Benefits

- ✅ **Zero runtime cost**: No cycle detection code to execute
- ✅ **Catches errors early**: Cycles are caught at compile time, not in production
- ✅ **Compiler-verified**: The type system proves your graph is acyclic
- ✅ **No mental overhead**: You can't accidentally create cycles even if you try

This is a true **zero-cost abstraction**—the type system provides safety guarantees without any runtime checks or performance penalty.
