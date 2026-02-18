# Test Suite

## Purpose

Integration and performance tests for dagx. Unit tests live in `src/{module}/tests.rs`.

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

- **boundaries**: Edge cases (very large/very small graphs)
- **dependencies**: Complex dependency graphs
- **errors**: Error handling
- **execution**: End-to-end DAG execution
- **interleaving**: Layer interleaving patterns
- **parallelism**: Concurrent task execution
- **runtimes**: Tokio, smol compatibility
- **tracing**: Execution observability

## Writing Tests

Unit tests should:

- Test single units of functionality
- Mock external dependencies
- Cover error cases

Integration tests should:

- Test component interactions
- Use real implementations
- Verify end-to-end behavior
