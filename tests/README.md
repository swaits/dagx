# Test Suite

## Purpose

Integration and performance tests for dagx. Unit tests live in `src/{module}/tests.rs`.

## Directory Structure

```
tests/
├── common/          # Shared test utilities
├── dependencies/    # Dependency resolution tests
├── execution/       # Task execution and DAG running
├── parallelism/     # Parallel execution verification
├── panic/           # Panic handling and isolation
├── performance/     # Stress tests and benchmarks
├── runtimes/        # Runtime compatibility tests
├── task_patterns/   # Common task pattern tests
├── type_safety/     # Type system validation
└── lib_tests.rs     # Main test entry point
```

## Running Tests

```bash
# All tests (unit + integration)
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test lib_tests

# Specific category
cargo test --test lib_tests -- parallelism::
cargo test --test lib_tests -- execution::

# With output
cargo test -- --nocapture

# Single threaded (for debugging)
cargo test -- --test-threads=1

# Performance tests (excluded by default)
cargo test --release -- --ignored
```

## Test Categories

### Unit Tests (`src/{module}/tests.rs`)

- Module-specific logic
- Edge cases and error conditions
- Type implementations

### Integration Tests (`tests/`)

- **execution**: End-to-end DAG execution
- **dependencies**: Complex dependency graphs
- **parallelism**: Concurrent task execution
- **panic**: Error propagation and isolation
- **performance**: Large DAGs and stress tests
- **runtimes**: Tokio, async-std, smol compatibility
- **task_patterns**: Common usage patterns
- **type_safety**: Type system guarantees

## Writing Tests

Integration tests use common utilities from `tests/common/mod.rs`.

Unit tests should:

- Test single units of functionality
- Mock external dependencies
- Cover error cases

Integration tests should:

- Test component interactions
- Use real implementations
- Verify end-to-end behavior
