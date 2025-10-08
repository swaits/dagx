// Main test file that includes all reorganized tests from lib.rs

#![allow(clippy::type_complexity)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::redundant_clone)]

#[path = "common/mod.rs"]
mod common;

mod boundaries;
mod dependencies;
mod errors;
mod execution;
mod parallelism;
mod patterns;
mod performance;
mod runtimes;
mod task_patterns;
mod timing;
mod tracing;
mod type_safety;
