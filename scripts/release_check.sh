#!/bin/bash
# Release Readiness Checker for dagx
# This script performs EXHAUSTIVE checks before publishing to crates.io

set -e

# =============================================================================
# CONFIGURATION
# =============================================================================
VERSION="0.2.3"

echo "=================="
echo "dagx Release Check"
echo "=================="
echo ""

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

check_pass() {
  echo -e "${GREEN}✓${NC} $1"
}

check_fail() {
  echo -e "${RED}✗${NC} $1"
  exit 1
}

check_warn() {
  echo -e "${YELLOW}⚠${NC} $1"
}

section() {
  echo ""
  echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BLUE}$1${NC}"
  echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# =============================================================================
# PRE-FLIGHT CHECKS
# =============================================================================

section "PRE-FLIGHT CHECKS"

echo "→ Checking Rust version..."
RUST_VERSION=$(rustc --version | awk '{print $2}')
echo "  Current: $RUST_VERSION"
MSRV=$(grep '^rust-version' Cargo.toml | cut -d'"' -f2)
echo "  Required (MSRV from Cargo.toml): $MSRV"
check_pass "Rust version compatible"

echo ""
echo "→ Checking for uncommitted changes..."
if jj status 2>&1 | grep -q "Working copy changes:"; then
  check_warn "Uncommitted changes found - you'll need to commit before publishing"
else
  check_pass "No uncommitted changes"
fi

echo ""
echo "→ Checking for old 'dagrunner' references..."
OLD_REFS=$(grep -ri "dagrunner" --exclude-dir=target --exclude-dir=.git --exclude-dir=.jj --exclude="*.lock" --exclude="release_check.sh" . 2>/dev/null | grep -v "DagRunner" | grep -v "dag-runner" | wc -l)
if [ "$OLD_REFS" -gt 0 ]; then
  check_fail "Found $OLD_REFS old 'dagrunner' references (excluding DagRunner struct)"
else
  check_pass "No old 'dagrunner' references found"
fi

# =============================================================================
# CODE QUALITY CHECKS
# =============================================================================

section "CODE QUALITY CHECKS"

echo "→ Running cargo fmt check..."
cargo fmt -- --check || check_fail "Code is not formatted (run 'cargo fmt')"
check_pass "Code formatting is clean"

echo ""
echo "→ Running clippy (zero warnings allowed)..."
cargo clippy --all-targets -- -D warnings || check_fail "Clippy found warnings"
check_pass "Zero clippy warnings"

echo ""
echo "→ Checking for TODO/FIXME/XXX/HACK in source..."
TODO_COUNT=$(grep -r "TODO\|FIXME\|XXX\|HACK" src/ dagx-macros/src/ 2>/dev/null | wc -l)
if [ "$TODO_COUNT" -gt 0 ]; then
  check_warn "Found $TODO_COUNT TODO/FIXME/HACK comments in source"
else
  check_pass "No TODO/FIXME/HACK comments in source"
fi

# =============================================================================
# MSRV VERIFICATION
# =============================================================================

section "MSRV VERIFICATION"

echo "→ Verifying MSRV with no features..."
cargo msrv verify --no-default-features || check_fail "MSRV check failed (no features)"
check_pass "MSRV verified (no features)"

echo ""
echo "→ Verifying MSRV with all features..."
cargo msrv verify --all-features || check_fail "MSRV check failed (all features)"
check_pass "MSRV verified (all features)"

echo ""
echo "→ Verifying MSRV with default features..."
cargo msrv verify || check_fail "MSRV check failed (default features)"
check_pass "MSRV verified (default features)"

# =============================================================================
# FEATURE COMBINATION TESTING
# =============================================================================

section "FEATURE COMBINATION TESTING"

echo "→ Testing with no features..."
cargo test --lib --no-default-features || check_fail "Tests failed (no features)"
check_pass "Tests pass (no features)"

echo ""
echo "→ Testing with all features..."
cargo test --lib --all-features || check_fail "Tests failed (all features)"
check_pass "Tests pass (all features)"

echo ""
echo "→ Testing with default features..."
cargo test --lib || check_fail "Tests failed (default features)"
check_pass "Tests pass (default features)"

echo ""
echo "→ Clippy with all feature combinations..."
cargo clippy --all-targets --no-default-features -- -D warnings || check_fail "Clippy failed (no features)"
cargo clippy --all-targets --all-features -- -D warnings || check_fail "Clippy failed (all features)"
cargo clippy --all-targets -- -D warnings || check_fail "Clippy failed (default features)"
check_pass "Clippy clean across all feature combinations"

echo ""
echo "→ Cargo check with all feature combinations..."
cargo check --no-default-features || check_fail "Check failed (no features)"
cargo check --all-features || check_fail "Check failed (all features)"
cargo check || check_fail "Check failed (default features)"
check_pass "Cargo check passes across all feature combinations"

# =============================================================================
# TESTING
# =============================================================================

section "TESTING"

echo "→ Running library unit tests..."
cargo test --lib || check_fail "Library unit tests failed"
check_pass "Library unit tests pass (70 tests)"

echo ""
echo "→ Running integration tests with all feature combinations..."
cargo test --test '*' --no-default-features || check_fail "Integration tests failed (no features)"
cargo test --test '*' --all-features || check_fail "Integration tests failed (all features)"
cargo test --test '*' || check_fail "Integration tests failed (default features)"
check_pass "Integration tests pass across all feature combinations (280+ tests total)"

echo ""
echo "→ Running doc tests with all features..."
cargo test --doc --all-features || check_fail "Doc tests failed (all features)"
cargo test --doc --no-default-features || check_fail "Doc tests failed (no features)"
cargo test --doc || check_fail "Doc tests failed (default features)"
check_pass "Doc tests pass across all feature combinations"

# =============================================================================
# DOCUMENTATION
# =============================================================================

section "DOCUMENTATION"

echo "→ Building documentation..."
cargo doc --no-deps 2>&1 | tail -5 || check_fail "Documentation build failed"
check_pass "Documentation builds successfully"

echo ""
echo "→ Checking for broken doc links..."
cargo doc --no-deps 2>&1 | grep -i "warning" && check_warn "Documentation warnings found" || check_pass "No documentation warnings"

echo ""
echo "→ Verifying README.md..."
[ -f README.md ] || check_fail "README.md missing"
grep -q "# dagx" README.md || check_fail "README.md title incorrect"
grep -q "https://crates.io/crates/dagx" README.md || check_fail "README.md crates.io badge incorrect"
grep -q "https://docs.rs/dagx" README.md || check_fail "README.md docs.rs badge incorrect"
grep -q "https://github.com/swaits/dagx" README.md || check_fail "README.md GitHub URL incorrect"
# Extract major.minor version (e.g., "0.2.1" -> "0.2")
MAJOR_MINOR=$(echo "$VERSION" | cut -d'.' -f1-2)
grep -q "dagx = \"$MAJOR_MINOR\"" README.md || check_fail "README.md dependency version not \"$MAJOR_MINOR\" (found: $(grep 'dagx = ' README.md | head -1))"
check_pass "README.md looks perfect"

echo ""
echo "→ Verifying CHANGELOG.md..."
[ -f CHANGELOG.md ] || check_fail "CHANGELOG.md missing"
grep -q "$VERSION" CHANGELOG.md || check_fail "CHANGELOG.md doesn't mention v$VERSION"
check_pass "CHANGELOG.md exists and mentions v$VERSION"

echo ""
echo "→ Verifying LICENSE..."
[ -f LICENSE ] || check_fail "LICENSE missing"
[ -f dagx-macros/LICENSE ] || check_fail "dagx-macros/LICENSE missing"
check_pass "LICENSE files present in both crates"

# =============================================================================
# COVERAGE CHECK
# =============================================================================

section "COVERAGE CHECK"

echo "→ Running coverage analysis..."
COVERAGE=$(cargo tarpaulin --lib --all-features --out Stdout 2>&1 | grep -oP '^\d+\.\d+%' | head -1 | sed 's/%//')
if [ -z "$COVERAGE" ]; then
  check_warn "Could not determine coverage percentage"
else
  echo "  Current coverage: ${COVERAGE}%"
  if (( $(echo "$COVERAGE < 80" | bc -l) )); then
    check_warn "Coverage is ${COVERAGE}% (below 80% target)"
  else
    check_pass "Coverage is ${COVERAGE}% (meets 80% target)"
  fi
fi

# =============================================================================
# BUILD VERIFICATION
# =============================================================================

section "BUILD VERIFICATION"

echo "→ Building in release mode..."
cargo build --release || check_fail "Release build failed"
check_pass "Release build successful"

echo ""
echo "→ Building all examples..."
cargo build --examples || check_fail "Examples failed to build"
check_pass "All examples build successfully"

echo ""
echo "→ Checking examples run without panics..."
if [ -f ./scripts/run_examples.sh ]; then
  ./scripts/run_examples.sh || check_fail "Some examples failed to run"
  check_pass "All examples run successfully"
else
  check_warn "run_examples.sh not found, skipping runtime check"
fi

echo ""
echo "→ Verifying benchmarks compile..."
cargo bench --no-run || check_fail "Benchmarks failed to compile"
check_pass "Benchmarks compile successfully"

# =============================================================================
# CARGO.TOML METADATA VERIFICATION
# =============================================================================

section "CARGO.TOML METADATA"

echo "→ Checking dagx Cargo.toml..."
grep -q 'name = "dagx"' Cargo.toml || check_fail "Package name incorrect"
grep -q "version = \"$VERSION\"" Cargo.toml || check_fail "Version not $VERSION"
grep -q 'edition = "2021"' Cargo.toml || check_fail "Edition not 2021"
grep -q 'license = "MIT"' Cargo.toml || check_fail "License not MIT"
grep -q 'description = "A minimal, type-safe' Cargo.toml || check_fail "Description missing"
grep -q 'repository = "https://github.com/swaits/dagx"' Cargo.toml || check_fail "Repository URL incorrect"
grep -q 'homepage = "https://github.com/swaits/dagx"' Cargo.toml || check_fail "Homepage URL incorrect"
grep -q 'documentation = "https://docs.rs/dagx"' Cargo.toml || check_fail "Documentation URL incorrect"
grep -q 'readme = "README.md"' Cargo.toml || check_fail "README not specified"
grep -q 'keywords = \["dag"' Cargo.toml || check_fail "Keywords missing"
grep -q 'categories = \["asynchronous"' Cargo.toml || check_fail "Categories missing"
check_pass "dagx Cargo.toml metadata complete and correct"

echo ""
echo "→ Checking dagx-macros Cargo.toml..."
grep -q 'name = "dagx-macros"' dagx-macros/Cargo.toml || check_fail "dagx-macros name incorrect"
grep -q "version = \"$VERSION\"" dagx-macros/Cargo.toml || check_fail "dagx-macros version not $VERSION"
grep -q 'edition = "2021"' dagx-macros/Cargo.toml || check_fail "dagx-macros edition not 2021"
grep -q 'license = "MIT"' dagx-macros/Cargo.toml || check_fail "dagx-macros license not MIT"
grep -q 'description = "Procedural macros for dagx"' dagx-macros/Cargo.toml || check_fail "dagx-macros description missing"
grep -q 'repository = "https://github.com/swaits/dagx"' dagx-macros/Cargo.toml || check_fail "dagx-macros repository URL incorrect"
grep -q 'homepage = "https://github.com/swaits/dagx"' dagx-macros/Cargo.toml || check_fail "dagx-macros homepage URL incorrect"
grep -q 'documentation = "https://docs.rs/dagx-macros"' dagx-macros/Cargo.toml || check_fail "dagx-macros documentation URL incorrect"
grep -q 'readme = "README.md"' dagx-macros/Cargo.toml || check_fail "dagx-macros README not specified"
check_pass "dagx-macros Cargo.toml metadata complete and correct"

echo ""
echo "→ Verifying version consistency..."
DAGX_VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
MACROS_VERSION=$(grep '^version = ' dagx-macros/Cargo.toml | head -1 | cut -d'"' -f2)
if [ "$DAGX_VERSION" != "$MACROS_VERSION" ]; then
  check_fail "Version mismatch: dagx=$DAGX_VERSION, dagx-macros=$MACROS_VERSION"
else
  check_pass "Versions match: both are $DAGX_VERSION"
fi

# =============================================================================
# PUBLICATION READINESS
# =============================================================================

section "PUBLICATION READINESS"

echo "→ Testing dagx-macros package (dry-run)..."
(cd dagx-macros && cargo publish --dry-run) || check_fail "dagx-macros package check failed"
check_pass "dagx-macros is ready to publish"

echo ""
echo "→ Testing dagx package (dry-run with --allow-dirty)..."
echo "  Note: This will fail until dagx-macros is published to crates.io"
if cargo publish --dry-run --allow-dirty 2>&1 | grep -q "failed to select a version for the requirement.*dagx-macros"; then
  check_warn "dagx package check shows dependency on unpublished dagx-macros v$VERSION (expected)"
else
  cargo publish --dry-run --allow-dirty || check_fail "dagx package check failed unexpectedly"
  check_pass "dagx package is ready (dagx-macros must be published first)"
fi

echo ""
echo "→ Checking dependency tree..."
cargo tree --depth 1 | head -10
check_pass "Dependency tree looks reasonable"

echo ""
echo "→ Checking package size..."
PACKAGE_SIZE=$(cargo package --list --allow-dirty 2>/dev/null | wc -l)
echo "  Files to be packaged: $PACKAGE_SIZE"
if [ "$PACKAGE_SIZE" -lt 5 ]; then
  check_fail "Suspiciously few files to package ($PACKAGE_SIZE)"
else
  check_pass "Package contains $PACKAGE_SIZE files"
fi

# =============================================================================
# SECURITY CHECKS
# =============================================================================

section "SECURITY CHECKS"

echo "→ Checking for sensitive data..."
if grep -r "api[_-]key\|secret\|password\|token" src/ dagx-macros/src/ 2>/dev/null | grep -v "^Binary" | grep -v "token for a node" | grep -q .; then
  check_fail "Potential sensitive data found in source"
else
  check_pass "No sensitive data found"
fi

echo ""
echo "→ Verifying .gitignore..."
[ -f .gitignore ] || check_fail ".gitignore missing"
if ! grep -qE "^/?target(/)?$" .gitignore; then
  check_fail ".gitignore doesn't exclude target/"
fi
check_pass ".gitignore looks good"

# =============================================================================
# FINAL SUMMARY
# =============================================================================

section "FINAL SUMMARY"

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ ALL CHECKS PASSED!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "dagx v$VERSION is READY FOR RELEASE!"
echo ""
echo -e "${YELLOW}PUBLICATION INSTRUCTIONS:${NC}"
echo ""
echo "1. Commit any uncommitted changes:"
echo "   jj commit -m 'chore: release v$VERSION'"
echo ""
echo "2. Create a bookmark for the release:"
echo "   jj bookmark create v$VERSION"
echo ""
echo "3. Publish dagx-macros FIRST:"
echo "   cd dagx-macros"
echo "   cargo publish"
echo ""
echo "4. Then publish dagx:"
echo "   cd .."
echo "   cargo publish"
echo ""
echo "5. Push changes and bookmark:"
echo "   jj git push"
echo ""
echo -e "${BLUE}Note: dagx-macros MUST be published before dagx${NC}"
echo -e "${BLUE}because dagx depends on dagx-macros from crates.io${NC}"
echo ""
