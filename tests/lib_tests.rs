// Main test file that includes all reorganized tests from lib.rs

#![allow(clippy::type_complexity)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::redundant_clone)]

mod boundaries;
mod dependencies;
mod errors;
mod execution;
mod interleaving;
mod parallelism;
mod runtimes;
mod tracing;
