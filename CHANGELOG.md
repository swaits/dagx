# Changelog

All notable changes to dagx will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-10-05

### Added

#### Core Features

- Type-safe async DAG executor with compile-time dependency validation
- Runtime-agnostic design (works with Tokio, async-std, smol, and other runtimes)
- Fluent builder API with type-state pattern for compile-time safety
- Comprehensive error handling with `DagResult<T>` and `DagError` enum
- Support for up to 8 dependencies per task
- True parallel execution with automatic task spawning

#### Task Patterns

- **Stateless tasks**: Pure functions with no self parameter
- **Read-only state**: Tasks with `&self` for configuration
- **Mutable state**: Tasks with `&mut self` for stateful operations
- **Sync and async**: Both synchronous and asynchronous task execution
- **Procedural macro**: `#[task]` macro for ergonomic task definitions

#### Error Handling

- Cycle detection with detailed node information
- Type-safe handle validation
- Panic isolation and conversion to errors
- Actionable error messages with recovery suggestions

#### Testing

- Comprehensive unit tests covering core functionality
- Documentation tests ensuring examples compile and run
- Panic handling and isolation tests
- Type safety verification tests
- Concurrency stress tests
- Integration tests for end-to-end workflows
- Runtime compatibility tests (Tokio, async-std, smol)
- Quirky runtime tests (async-executor, pollster, futures-executor)
- Dependency tuple tests (1-8 dependency support)
- Large DAG scalability tests (10,000+ nodes)

#### Performance

- Benchmark suite using Criterion
- True parallel execution (tasks spawn to multiple threads)
- Linear scaling verified up to 10,000+ tasks
- ~1-2µs overhead per task
- Efficient memory usage (~200 bytes per task)
- Zero-cost abstractions via generics and monomorphization

#### Documentation

- Comprehensive API documentation
- Tutorial examples (numbered, beginner-friendly):
  - `01_basic.rs` - Getting started
  - `02_fan_out.rs` - 1→N dependencies
  - `03_fan_in.rs` - N→1 dependencies
  - `04_parallel_computation.rs` - Parallel map-reduce
- Reference examples (practical patterns):
  - `complex_dag.rs` - Multi-layer workflows
  - `conditional_workflow.rs` - Conditional execution
  - `data_pipeline.rs` - ETL pipeline pattern
  - `error_handling.rs` - Error propagation and recovery
  - `timeout.rs` - Task timeouts
  - `large_dag.rs` - Scalability demonstration
  - `parallelism_proof.rs` - True parallelism proof
- Architecture documentation
- Security policy
- Contributing guidelines

### Dependencies

- `futures = "0.3"` (runtime-agnostic async)
- `parking_lot = "0.12"` (efficient synchronization)
- `dagx-macros` (procedural macros)

### Dev Dependencies

- `tokio = "1"` (async runtime)
- `async-std = "1"` (async runtime)
- `smol = "2"` (async runtime)
- `criterion = "0.7"` (benchmarking)
- `async-executor = "1.13"` (quirky runtime testing)
- `pollster = "0.4"` (quirky runtime testing)
- `futures-executor = "0.3"` (quirky runtime testing)

### Tested Platforms

- Linux (primary development and CI)
- macOS (compatible)
- Windows (compatible)

[0.1.0]: https://github.com/swaits/dagx/releases/tag/v0.1.0
