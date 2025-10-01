//! Head-to-head comparison benchmarks: dagx vs dagrs
//!
//! These benchmarks compare dagx performance against dagrs (the most popular
//! Rust DAG library) across various workload patterns.

pub mod deep_chain;
pub mod diamond_pattern;
pub mod etl_pipeline;
pub mod large_scale;
pub mod linear_pipeline;
pub mod wide_fanout;
