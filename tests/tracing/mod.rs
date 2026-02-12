//! Tests for tracing functionality
//!
//! These tests verify that the tracing feature works correctly when enabled
//! and that the library compiles and works without it when disabled.

#[cfg(feature = "tracing")]
mod with_tracing;
