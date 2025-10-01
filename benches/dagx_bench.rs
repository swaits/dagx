//! dagx benchmark suite
//!
//! Organized into logical modules:
//! - basic/: DAG creation, execution, and scaling benchmarks
//! - patterns/: Common DAG patterns (fanout, diamond, 10k breakdown)
//! - realistic/: Real-world workload simulations (ETL, config broadcast, etc.)
//! - comparisons/: Head-to-head benchmarks against dagrs

use criterion::{criterion_group, criterion_main, Criterion};

mod basic;
mod comparisons;
mod patterns;
mod realistic;

// Configure criterion with better defaults
fn configure_criterion() -> Criterion {
    Criterion::default()
        .configure_from_args()
        .noise_threshold(0.05) // 5% noise threshold for detecting changes
        .significance_level(0.05) // 95% confidence interval
        .warm_up_time(std::time::Duration::from_secs(3))
}

criterion_group! {
    name = creation_benches;
    config = configure_criterion();
    targets = basic::creation::bench_dag_creation
}

criterion_group! {
    name = execution_benches;
    config = configure_criterion();
    targets = basic::execution::bench_dag_execution
}

criterion_group! {
    name = scaling_benches;
    config = configure_criterion();
    targets = basic::scaling::bench_dag_scaling
}

criterion_group! {
    name = pattern_benches;
    config = configure_criterion();
    targets =
        patterns::fanout::bench_fanout,
        patterns::diamond::bench_diamond,
        patterns::breakdown::bench_10k_breakdown
}

criterion_group! {
    name = realistic_benches;
    config = configure_criterion();
    targets =
        realistic::etl::bench_etl_pipeline,
        realistic::config_broadcast::bench_config_broadcast,
        realistic::memory_efficiency::bench_memory_efficiency,
        realistic::mixed::bench_mixed_patterns
}

criterion_group! {
    name = comparison_benches;
    config = configure_criterion();
    targets =
        comparisons::linear_pipeline::bench_linear_pipeline,
        comparisons::wide_fanout::bench_wide_fanout,
        comparisons::diamond_pattern::bench_diamond_pattern,
        comparisons::deep_chain::bench_deep_chain,
        comparisons::etl_pipeline::bench_etl_pipeline,
        comparisons::large_scale::bench_large_scale
}

criterion_main!(
    creation_benches,
    execution_benches,
    scaling_benches,
    pattern_benches,
    realistic_benches,
    comparison_benches
);
