# Changelog

All notable changes to dagx will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-10-08

[View changes](https://github.com/swaits/dagx/compare/v0.1.0...v0.2.0)

### Changed

- **BREAKING**: Updated MSRV (Minimum Supported Rust Version) from 1.78.0 to 1.81.0
  - Required by updated dependencies: `criterion@0.7.0`, `half@2.6.0`, `rayon@1.11.0`
  - Updated in both `dagx` and `dagx-macros` crates
  - Updated all documentation (README.md, CONTRIBUTING.md)

### Added

- Comprehensive test coverage improvements (82.61% → 92.94%)
  - Added 9 new unit tests for error paths in `ExtractInput` implementations
  - Added 3 new type conversion tests in `src/types/tests.rs`
  - Added 1 new trait implementation test in `src/task/tests.rs`
  - Added 1 new dependency tuple test in `src/deps/tests.rs`
  - Added 6 new execution path tests in `tests/execution/basic.rs`
  - Added error handling tests for HashMap, Result, Option, Vec, and Arc types
  - Added tests for tuple dependency count validation
  - Improved concurrent run protection test with better synchronization
- Added `tarpaulin_include` to lint configuration for coverage tooling compatibility

### Fixed

- Fixed GitHub Actions `cargo-audit` workflow
  - Replaced deprecated `actions-rs/audit-check@v1` with direct `cargo audit` command
  - Resolved "Resource not accessible by integration" error
- Fixed clippy warnings in test code
  - Removed unnecessary borrows in `.depends_on()` calls
  - Added `#[allow(clippy::clone_on_copy)]` for explicit clone test
- Marked timing-sensitive and resource-intensive tests with `#[cfg_attr(tarpaulin, ignore)]`
  - `test_arc_parallel_execution` (timing unreliable under instrumentation)
  - `test_100000_nodes_stress` (resource intensive)
  - `test_10000_level_chain_stress` (resource intensive)

### Internal

- Improved test organization following existing patterns in `tests/` directory
- Enhanced error path coverage for all `ExtractInput` trait implementations
- Better separation between unit tests and integration tests
- Refactored `scripts/release_check.sh` to use configurable VERSION variable
  - Replaced hardcoded version references throughout script
  - Updated instructions to use `jj` commands instead of `git`
  - Simplified version bumps for future releases

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

[0.2.0]: https://github.com/swaits/dagx/tree/v0.2.0
[0.1.0]: https://github.com/swaits/dagx/tree/v0.1.0
