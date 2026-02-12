# Tracing Support

dagx provides optional observability through the `tracing` crate with **zero runtime overhead when disabled**. The tracing instrumentation is conditionally compiled using feature flags, meaning when the feature is off, the logging code doesn't exist in the compiled binary at all.

## Enabling Tracing

```toml
[dependencies]
dagx = { version = "0.3", features = ["tracing"] }
tracing-subscriber = "0.3"
```

## Usage Example

```rust
use tracing_subscriber::{fmt, EnvFilter};

// Initialize tracing subscriber
fmt()
    .with_env_filter(
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("dagx=info"))
    )
    .init();

// Your DAG code here - tracing will log execution details
```

## Log Levels

- **INFO**: DAG execution start/completion
- **DEBUG**: Task additions, dependency wiring, layer computation
- **TRACE**: Individual task execution (inline vs spawned), detailed execution flow
- **ERROR**: Task panics, concurrent execution attempts

Run with different levels:

```bash
RUST_LOG=dagx=info  cargo run    # High-level execution info
RUST_LOG=dagx=debug cargo run    # Task and layer details
RUST_LOG=dagx=trace cargo run    # All execution details
```

## Zero-Cost Guarantee

When the `tracing` feature is disabled (the default), there is **literally 0ns overhead**:

- Logging code is removed at compile time via `#[cfg(feature = "tracing")]`
- No branches, no function calls, nothing
- The `tracing` crate isn't even linked
- Benchmarks verify identical performance with/without the feature

This follows the same pattern used by Tokio, Hyper, and other performance-critical Rust async libraries.
