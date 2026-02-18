# dagx

[![Crates.io](https://img.shields.io/crates/v/dagx.svg)](https://crates.io/crates/dagx)
[![Documentation](https://docs.rs/dagx/badge.svg)](https://docs.rs/dagx)
[![Build Status](https://github.com/swaits/dagx/workflows/CI/badge.svg)](https://github.com/swaits/dagx/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.81+-blue.svg)](https://www.rust-lang.org)

A minimal, type-safe, runtime-agnostic async DAG (Directed Acyclic Graph) executor with compile-time cycle prevention and true parallel execution.

## Why dagx?

### Blazing Fast: 1-100x faster than dagrs

| Workload          | Tasks  | dagx      | dagrs     | Speedup            |
| ----------------- | ------ | --------- | --------- | ------------------ |
| Sequential chain  | 5      | 1.02 µs   | 770.42 µs | **755x faster** 🚀 |
| Diamond pattern   | 4      | 5.16 µs   | 770.87 µs | **149x faster**    |
| Sequential chain  | 100    | 25.09 µs  | 1.19 ms   | **47.4x faster**   |
| Fan-out (1→100)   | 101    | 100.75 µs | 1.02 ms   | **10.1x faster**   |
| Independent tasks | 10,000 | 8.61 ms   | 15.37 ms  | **1.79x faster**   |

### Simple API

```rust
let sum = dag.add_task(Add).depends_on((x, y));
dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;
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
    let mut dag = DagRunner::new();

    // Add source tasks with no dependencies
    let x = dag.add_task(Value(2));
    let y = dag.add_task(Value(3));

    // Add task that depends on both x and y.
    let sum = dag.add_task(Add).depends_on((x, y));

    // Execute with true parallelism
    let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();

    // Retrieve results
    assert_eq!(output.get(sum).unwrap(), 5);
}
```

## Features

### Compile-Time Safety

- **Cycles are impossible** — the type system prevents them at compile time, zero runtime overhead
- **No runtime type errors** — dependencies validated at compile time
- **Compiler-verified correctness** — no surprise failures in production

See [how it works](docs/CYCLE_PREVENTION.md).

### Runtime Agnostic

dagx works with any async runtime. Provide a spawner function to `run()`:

```rust
// With Tokio
// The join handle result can be unwrapped because dagx catches panics internally
dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();

// With smol
dag.run(|fut| smol::spawn(fut)).await.unwrap();

// Single-threaded concurrency on the invoking runtime
// Can be faster in situations where waiting time dominates
dag.run(|fut| fut).await.unwrap()
```

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

### Tracing

dagx provides optional observability using the `tracing` crate, controlled by the `tracing` feature flag.

**Enabling Tracing**

```toml
[dependencies]
dagx = { version = "0.3", features = ["tracing"] }
tracing-subscriber = "0.3"
```

**Log Levels**

- **INFO**: DAG execution start/completion
- **DEBUG**: Task additions, dependency wiring, layer computation
- **TRACE**: Individual task execution (inline vs spawned), detailed execution flow
- **ERROR**: Task panics, concurrent execution attempts

### Other

- **True parallelism**: Chosen runtime executes tasks concurrently and/or in parallel
- **No boilerplate**: The `derive` feature and the `#[task]` macro are enabled by default to simplify task implementation.

## Performance

dagx provides true parallel execution with sub-microsecond overhead per task.

**How is dagx so fast?**

- **Inline fast-path**: Sequential chains execute inline without spawning
- **Adaptive execution**: Inline for sequential work, executor-agnostic parallelism for concurrent work
- **Zero-cost abstractions**: Compile-time graph validation eliminates overhead

See [design philosophy](docs/DESIGN_PHILOSOPHY.md) for details.

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
- [`data_pipeline.rs`](examples/data_pipeline.rs) - ETL data processing pipeline
- [`error_handling.rs`](examples/error_handling.rs) - Error propagation and recovery

Run any example: `cargo run --example circuit_breaker`

## Documentation

Full API documentation is available at [docs.rs/dagx](https://docs.rs/dagx).

Detailed documentation on dagx's internals and advanced features:

- [**Compile-Time Cycle Prevention**](docs/CYCLE_PREVENTION.md) - How the type system prevents cycles
- [**Design Philosophy**](docs/DESIGN_PHILOSOPHY.md) - Primitives as scheduler, inline fast-path optimization
- [**Library Comparisons**](docs/COMPARISONS.md) - Detailed comparison with dagrs, async_dag, and others

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

_Benchmarks run on Intel i9-13950HX @ 5.5GHz._

## Code of Conduct

This project follows the [Builder's Code of Conduct](https://builderscode.org).

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

For security issues, see [SECURITY.md](SECURITY.md).

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

Copyright (c) 2025 Stephen Waits <steve@waits.net>
