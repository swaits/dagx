# Contributing to dagx

Thanks for your interest in contributing! Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before getting started.

## Quick Start

```bash
# Clone and build
git clone https://github.com/swaits/dagx
cd dagx
cargo build

# Run tests
cargo test

# Check your code
cargo fmt
cargo clippy -- -D warnings
```

## Development Setup

**Prerequisites:**

- Rust 1.81+ (edition 2021)
- Git

**Optional tools:**

```bash
rustup component add rustfmt clippy
cargo install cargo-llvm-cov  # For coverage
cargo install cargo-audit     # For security checks
```

## Making Changes

1. **Fork and branch**: Create a descriptive branch (`fix-xyz`, `feature-abc`)
2. **Code**: Make your changes following our style guide
3. **Test**: Add tests for new functionality
4. **Document**: Update docs if needed
5. **Verify**: Run the pre-flight checks below
6. **PR**: Open a pull request with a clear description

### Pre-flight Checklist

Before submitting your PR, verify:

```bash
cargo test              # All tests pass
cargo fmt -- --check    # Code is formatted
cargo clippy -- -D warnings  # No clippy warnings
cargo doc               # Docs build
cargo build --examples  # Examples compile
```

## Code Style

- **Format**: Use `rustfmt` (enforced by CI)
- **Lint**: Fix all `clippy` warnings
- **Document**: Add doc comments for public APIs
- **Test**: Cover new code with tests

### Documentation Guidelines

- Document **why**, not **what**
- Include examples for public APIs
- Keep examples simple and focused
- All doc examples must compile

Example:

````rust
/// Adds a task to the DAG.
///
/// # Examples
///
/// ```
/// use dagx::DagRunner;
///
/// let mut dag = DagRunner::new();
/// let task = dag.add_task(MyTask::new());
/// ```
pub fn add_task<T: Task>(&self, task: T) -> TaskBuilder<T> {
    // ...
}
````

## Testing

**Test organization:**

- Unit tests: `#[cfg(test)]` modules in source files
- Integration tests: `tests/` directory
- Examples: `examples/` directory (must compile)
- Doctests: In `///` comments (must pass)

**Runtime testing:**

```bash
cargo test                           # Default (tokio)
cargo test --test async_std_runtime  # async-std
cargo test --test smol_runtime       # smol
```

**Benchmarks:**

```bash
cargo bench
# View results: open target/criterion/report/index.html
```

## Continuous Integration

Our CI runs on every PR and checks:

- **Build**: Linux, macOS, Windows
- **Test**: All tests across multiple runtimes
- **Format**: `rustfmt` compliance
- **Lint**: `clippy` with no warnings
- **Coverage**: Minimum 80% code coverage
- **Security**: `cargo audit` for vulnerabilities

## Commit Guidelines

Use clear, concise commit messages:

```
type: brief description

Optional longer explanation if needed.

Closes #123
```

**Types**: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`

**Examples:**

- `feat: add timeout support for tasks`
- `fix: correct cycle detection in diamond DAG`
- `docs: clarify TaskHandle usage in README`

## Pull Requests

**PR description should include:**

- What changed and why
- How it was tested
- Any breaking changes
- Link to related issue (if applicable)

**Review process:**

- Maintainer reviews within 1 week
- Address feedback
- Once approved, maintainer merges
- Contributors are credited in releases

## Getting Help

- **Questions**: Open a GitHub Discussion
- **Bugs**: Open a GitHub Issue
- **Security**: Email steve@waits.net (see [SECURITY.md](SECURITY.md))

## Release Process

(For maintainers only)

1. Update version in `Cargo.toml` and `CHANGELOG.md`
2. Run full test suite: `cargo test --all-features`
3. Tag release: `git tag -a v0.x.y -m "Release v0.x.y"`
4. Push tag: `git push origin v0.x.y`
5. Publish: `cargo publish`
6. Create GitHub release with changelog

---

Thank you for contributing! 🎉
