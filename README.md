# dagx

[![Crates.io](https://img.shields.io/crates/v/dagx.svg)](https://crates.io/crates/dagx)
[![Documentation](https://docs.rs/dagx/badge.svg)](https://docs.rs/dagx)
[![Build Status](https://github.com/swaits/dagx/workflows/CI/badge.svg)](https://github.com/swaits/dagx/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.81+-blue.svg)](https://www.rust-lang.org)

A minimal, type-safe, runtime-agnostic async DAG (Directed Acyclic Graph) executor with compile-time cycle prevention and true parallel execution.

## Why dagx?

### 🚀 Blazing Fast: 1.04-129x faster than dagrs

| Workload          | Tasks  | dagx    | dagrs   | Speedup            |
| ----------------- | ------ | ------- | ------- | ------------------ |
| Sequential chain  | 5      | 3.0 µs  | 385 µs  | **129x faster** 🚀 |
| Sequential chain  | 100    | 79 µs   | 703 µs  | **8.9x faster**    |
| Diamond pattern   | 4      | 11 µs   | 387 µs  | **34x faster**     |
| Fan-out (1→100)   | 101    | 155 µs  | 595 µs  | **3.85x faster**   |
| Independent tasks | 10,000 | 12.7 ms | 13.3 ms | **1.04x faster**   |

**Per-task overhead:**

- Construction: ~100 ns/task
- Inline execution (sequential): ~790 ns/task
- Parallel execution: ~1.3 µs/task

### 🛡️ Compile-Time Safety

- **Cycles are impossible** — the type system prevents them at compile time, zero runtime overhead
- **No runtime type errors** — dependencies validated at compile time
- **Compiler-verified correctness** — no surprise failures in production

See [how it works](docs/CYCLE_PREVENTION.md).

### ✨ Simple API

```rust
let sum = dag.add_task(Add).depends_on((&x, &y));
dag.run().await?;
```

That's it. No trait boilerplate, no manual channels, no node IDs.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
dagx = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Basic example:

```rust
use dagx::{task, DagRunner, Task};

// Define tasks with the #[task] macro

struct Value(i32);

#[task]
impl Value {
    async fn run(&self) -> i32 {
        self.0
    }
}

struct Add;

#[task]
impl Add {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b
    }
}

#[tokio::main]
async fn main() {
    let dag = DagRunner::new();

    // Add source tasks with no dependencies
    let x = dag.add_task(Value(2));
    let y = dag.add_task(Value(3));

    // Add task that depends on both x and y
    let sum = dag.add_task(Add).depends_on((&x, &y));

    // Execute with true parallelism
    dag.run().await.unwrap();

    // Retrieve results
    assert_eq!(dag.get(sum).unwrap(), 5);
}
```

## Performance

dagx provides true parallel execution with sub-microsecond overhead per task.

**Why is dagx so fast?**

- **Inline fast-path**: Sequential chains execute inline without spawning (8.9-129x faster)
- **Primitives as scheduler**: No custom scheduler — channels coordinate execution
- **Adaptive execution**: Inline for sequential work, true parallelism for concurrent work
- **Zero-cost abstractions**: Generics and monomorphization eliminate overhead

See [design philosophy](docs/DESIGN_PHILOSOPHY.md) for details.

## Features

- **Compile-time cycle prevention**: Type system makes cycles impossible — no runtime checks
- **Compile-time type safety**: Dependencies validated at compile time, no runtime type errors
- **Works with ANY type**: Custom types work automatically — just `Clone + Send + Sync`, no trait implementations needed
- **Runtime-agnostic**: Works with Tokio, async-std, smol, or any async runtime
- **True parallelism**: Tasks spawn to multiple threads for genuine parallel execution
- **Type-state pattern**: API prevents incorrect wiring through compile-time errors
- **Zero-cost abstractions**: Leverages generics and monomorphization for minimal overhead
- **Flexible task patterns**: Supports stateless, read-only, and mutable state tasks
- **Simple API**: Just `#[task]`, `DagRunner`, `TaskHandle`, and `TaskBuilder`
- **Comprehensive error handling**: Result-based errors with actionable messages
- **Optional tracing**: Zero-cost observability via optional `tracing` feature flag

## Core Concepts

### Task Patterns

dagx supports three task patterns:

**1. Stateless** - Pure functions with no state:

```rust
struct Add;

#[task]
impl Add {
    async fn run(a: &i32, b: &i32) -> i32 { a + b }
}
```

**2. Read-only state** - Configuration accessed via `&self`:

```rust
struct Multiplier(i32);

#[task]
impl Multiplier {
    async fn run(&self, input: &i32) -> i32 { input * self.0 }
}
```

**3. Mutable state** - State modification via `&mut self`:

```rust
struct Counter(i32);

#[task]
impl Counter {
    async fn run(&mut self, value: &i32) -> i32 {
        self.0 += value;
        self.0
    }
}
```

### DagRunner

The `DagRunner` orchestrates task execution:

```rust
let dag = DagRunner::new();
let handle = dag.add_task(MyTask::new());
```

### TaskHandle

A `TaskHandle<T>` is a typed reference to a task's output. Use it to wire dependencies and retrieve results:

```rust
// Single dependency
let task = dag.add_task(my_task).depends_on(&upstream);

// Multiple dependencies (order matters!)
let task = dag.add_task(my_task).depends_on((&upstream1, &upstream2));
```

### Custom Types

**dagx works with ANY type automatically!** As long as your type implements `Clone + Send + Sync + 'static`, it works seamlessly:

```rust
#[derive(Clone)]  // Just derive Clone!
struct User {
    name: String,
    age: u32,
}

#[task]
impl CreateUser {
    async fn run(&self) -> User {
        User { name: "Alice".to_string(), age: 30 }
    }
}

#[task]
impl ProcessUser {
    async fn run(user: &User) -> String {
        format!("{} is {} years old", user.name, user.age)
    }
}
```

**No trait implementations needed!** The `#[task]` macro generates type-specific extraction logic automatically. This includes nested structs, collections, enums, and any other type you define.

See [`custom_types.rs`](examples/custom_types.rs) for a complete example with nested custom types.

### Runtime Agnostic

dagx works with any async runtime. Provide a spawner function to `run()`:

```rust
// With Tokio
dag.run().await.unwrap();

// With async-std
dag.run().await.unwrap();

// With smol
dag.run().await.unwrap();
```

## Tutorials & Examples

### Tutorials (Start Here)

Step-by-step introduction to dagx:

- [`01_basic.rs`](examples/01_basic.rs) - Your first DAG
- [`02_fan_out.rs`](examples/02_fan_out.rs) - One task feeds many (1→N)
- [`03_fan_in.rs`](examples/03_fan_in.rs) - Many tasks feed one (N→1)
- [`04_parallel_computation.rs`](examples/04_parallel_computation.rs) - Map-reduce with true parallelism

Run tutorial examples:

```bash
cargo run --example 01_basic
cargo run --example 02_fan_out
cargo run --example 03_fan_in
cargo run --example 04_parallel_computation
```

### Advanced Examples

Real-world patterns:

- [`circuit_breaker.rs`](examples/circuit_breaker.rs) - Circuit breaker pattern for resilient systems
- [`complex_dag.rs`](examples/complex_dag.rs) - Multi-layer workflows with complex dependencies
- [`conditional_workflow.rs`](examples/conditional_workflow.rs) - Conditional execution with Result types
- [`custom_types.rs`](examples/custom_types.rs) - Using your own custom types (no trait implementations needed!)
- [`data_pipeline.rs`](examples/data_pipeline.rs) - ETL data processing pipeline
- [`debug_tracing.rs`](examples/debug_tracing.rs) - Debug metadata and task naming
- [`error_handling.rs`](examples/error_handling.rs) - Error propagation and recovery
- [`large_dag.rs`](examples/large_dag.rs) - Performance at 10,000+ nodes
- [`parallelism_proof.rs`](examples/parallelism_proof.rs) - Proof of true parallel execution
- [`retry_strategies.rs`](examples/retry_strategies.rs) - Retry patterns for transient failures
- [`timeout.rs`](examples/timeout.rs) - Managing task timeouts
- [`tracing_example.rs`](examples/tracing_example.rs) - Observability with tracing support

Run any example: `cargo run --example custom_types`

## Advanced Topics

Detailed documentation on dagx internals and advanced features:

- [**Compile-Time Cycle Prevention**](docs/CYCLE_PREVENTION.md) - How the type system prevents cycles
- [**Design Philosophy**](docs/DESIGN_PHILOSOPHY.md) - Primitives as scheduler, inline fast-path optimization
- [**Tracing Support**](docs/TRACING.md) - Zero-cost observability with the `tracing` crate
- [**Library Comparisons**](docs/COMPARISONS.md) - Detailed comparison with dagrs, async_dag, and others

## Important Limitations

### Tasks Cannot Return Bare Tuples

Tasks cannot return bare tuples as output types. If you need to return multiple values, use one of these workarounds:

**Option 1: Use a struct (recommended)**

```rust
struct UserData {
    name: String,
    age: i32,
}

struct FetchUser;

#[task]
impl FetchUser {
    async fn run(id: &i32) -> UserData {
        UserData {
            name: "Alice".to_string(),
            age: 30,
        }
    }
}
```

**Option 2: Wrap in Result**

```rust
struct FetchData;

#[task]
impl FetchData {
    async fn run(id: &i32) -> Result<(String, i32), String> {
        Ok(("Alice".to_string(), 30))
    }
}
```

Structs are preferred because they're self-documenting and easier to refactor.

## When to Use dagx

dagx is ideal for:

- **Data pipelines** with complex dependencies between stages
- **Build systems** where tasks depend on outputs of other tasks
- **Parallel computation** where work can be split and aggregated
- **Workflow engines** with typed data flow between stages
- **ETL processes** with validation and transformation steps

## Benchmarks

Run the full benchmark suite:

```bash
cargo bench
```

View detailed HTML reports:

```bash
# macOS
open target/criterion/report/index.html

# Linux
xdg-open target/criterion/report/index.html

# Windows
start target/criterion/report/index.html
```

_Benchmarks run on AMD Ryzen 7 7840U (Zen 4) @ 3.3GHz._

## Code of Conduct

This project follows the [Builder's Code of Conduct](https://builderscode.org).

## Documentation

Full API documentation is available at [docs.rs/dagx](https://docs.rs/dagx).

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

For security issues, see [SECURITY.md](SECURITY.md).

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

Copyright (c) 2025 Stephen Waits <steve@waits.net>
