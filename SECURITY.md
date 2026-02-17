# Security Policy

## Overview

dagx is a task execution library designed for safety and predictability. This document outlines security considerations, best practices, and limitations you should be aware of when using dagx in security-sensitive contexts.

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Security Model

### What dagx Provides

- **Compile-time type safety**: Dependencies are validated at compile time, preventing type errors
- **Cycle detection**: Runtime detection of circular dependencies
- **Memory safety**: Rust's ownership system prevents memory corruption

### What dagx Does NOT Provide

- **Panic isolation**: Panics in one task will propagate and may affect other tasks (see Panic Handling below)
- **Resource limits**: No built-in timeout, memory limits, or CPU throttling
- **Sandboxing**: Tasks run with full process privileges
- **Authentication/Authorization**: No access control between tasks
- **Input validation**: Tasks must validate their own inputs

## Security Considerations

### 1. Panic Handling

**Current Behavior**: dagx does NOT isolate panics between tasks. If a task panics:

- The panic will propagate through the executor
- Other tasks in the same execution layer may or may not complete (timing-dependent)
- The entire DAG execution will terminate

**Recommendation**:

- Use `std::panic::catch_unwind` in tasks that may panic
- Return `Result<T, E>` from tasks and handle errors explicitly
- Test panic scenarios thoroughly

```rust
use dagx::{task, Task};
use std::panic;

struct SafeTask;

#[task]
impl SafeTask {
    async fn run(&mut self) -> Result<i32, String> {
        // Catch panics and convert to Result
        panic::catch_unwind(|| {
            // Potentially panicking code
            42
        })
        .map_err(|_| "Task panicked".to_string())
    }
}
```

### 2. Resource Limits

dagx does NOT enforce:

- Execution timeouts
- Memory limits
- CPU usage limits
- Task queue depth limits

**Recommendation**: Implement resource limits at the task level using your async runtime's facilities:

```rust
use dagx::{task, Task};
use tokio::time::{timeout, Duration};

struct TimeLimitedTask;

#[task]
impl TimeLimitedTask {
    async fn run(&mut self) -> Result<i32, String> {
        timeout(Duration::from_secs(30), async {
            // Your task logic here
            expensive_computation().await
        })
        .await
        .map_err(|_| "Task timed out".to_string())
    }
}

async fn expensive_computation() -> i32 {
    // Actual work
    42
}
```

### 3. Untrusted Input

If your DAG processes untrusted input:

- **Validate all inputs** at task boundaries
- **Sanitize outputs** before using in security-sensitive contexts
- **Limit resource consumption** (see Resource Limits above)
- **Use strongly-typed inputs** to leverage compile-time safety

```rust
use dagx::{task, Task};

struct ValidatedInput {
    max_size: usize,
}

#[task]
impl ValidatedInput {
    async fn run(&mut self, input: &String) -> Result<String, String> {
        // Validate input size
        if input.len() > self.max_size {
            return Err(format!("Input exceeds maximum size of {}", self.max_size));
        }

        // Validate input format
        if !input.chars().all(|c| c.is_alphanumeric()) {
            return Err("Input contains invalid characters".to_string());
        }

        Ok(input.clone())
    }
}
```

### 4. Dependency Injection

Tasks receive dependencies as references, which:

- **Prevents modification** of upstream results (read-only access)
- **Ensures consistency** across multiple dependent tasks
- **Avoids data races** through Rust's borrow checker

However, if your task output contains interior mutability (e.g., `Arc<Mutex<T>>`), be aware that:

- Multiple downstream tasks can mutate shared state
- This is intentional but requires careful synchronization

### 5. State Management

Tasks with mutable state (`&mut self`):

- Execute exactly once per DAG run
- Have exclusive access to their internal state during execution
- Are not reentrant (cannot execute concurrently with themselves)

**Safe**: Each task instance maintains isolated state
**Unsafe**: Sharing mutable state between tasks via global variables or `Arc<Mutex<T>>` requires manual synchronization

## Best Practices

### 1. Error Handling

Use `Result<T, E>` for all fallible operations:

```rust
use dagx::{task, Task};

struct FallibleTask;

#[task]
impl FallibleTask {
    async fn run(&mut self, input: &String) -> Result<i32, String> {
        input.parse::<i32>()
            .map_err(|e| format!("Parse error: {}", e))
    }
}
```

### 2. Timeouts

Implement timeouts for long-running or potentially-hanging operations:

```rust
use tokio::time::{timeout, Duration};

#[task]
impl MyTask {
    async fn run(&mut self) -> Result<String, String> {
        timeout(Duration::from_secs(10), async {
            // Network request or other potentially-slow operation
            fetch_data().await
        })
        .await
        .map_err(|_| "Operation timed out".to_string())?
    }
}

async fn fetch_data() -> Result<String, String> {
    Ok("data".to_string())
}
```

### 3. Resource Cleanup

Use RAII patterns and `Drop` implementations for resource cleanup:

```rust
struct FileTask {
    path: String,
}

#[task]
impl FileTask {
    async fn run(&mut self) -> Result<String, String> {
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| format!("File read error: {}", e))?;
        Ok(content)
    }
}
// File handle automatically closed when task completes
```

### 4. Logging and Monitoring

Add logging to tasks for observability:

```rust
use dagx::{task, Task};

struct MonitoredTask;

#[task]
impl MonitoredTask {
    async fn run(&mut self, input: &i32) -> i32 {
        println!("Task started with input: {}", input);
        let result = input * 2;
        println!("Task completed with result: {}", result);
        result
    }
}
```

### 5. Minimize Privilege

- Run dagx applications with minimum necessary privileges
- Avoid running as root or with elevated permissions
- Use operating system security features (containers, namespaces, etc.)

## Security Limitations

### Known Limitations

1. **No panic recovery**: Task panics will crash the DAG execution
2. **No resource limits**: Tasks can consume unlimited CPU, memory, or time
3. **No isolation**: All tasks run in the same process with same privileges
4. **Shared executor**: Tasks compete for executor resources

### Not Security Features

dagx is **not** designed to:

- Run untrusted code safely
- Enforce resource quotas
- Provide sandboxing
- Implement authentication or authorization
- Prevent side-channel attacks

If you need these features, consider:

- **WebAssembly sandboxing** (wasmtime, wasmer)
- **Container isolation** (Docker, podman)
- **Operating system isolation** (VMs, seccomp, AppArmor)
- **Dedicated security runtimes** (gVisor, Firecracker)

## Reporting a Vulnerability

If you discover a security vulnerability in dagx:

1. **DO NOT** open a public GitHub issue
2. Email the maintainer directly: <steve@waits.net>
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

You should receive a response within 48 hours. If the vulnerability is confirmed:

- A fix will be developed privately
- A security advisory will be published
- Credit will be given (unless you prefer anonymity)

## Security Updates

Security updates will be:

- Released as patch versions (e.g., 0.1.1)
- Announced in GitHub releases
- Published to crates.io immediately
- Documented in CHANGELOG.md

## Additional Resources

- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [async-std Security](https://async.rs/security)
- [Tokio Security](https://tokio.rs/tokio/topics/security)

## License

This security policy is part of the dagx project and is licensed under the MIT License.
