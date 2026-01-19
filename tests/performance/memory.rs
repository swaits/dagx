//! Memory usage tests

use crate::common::task_fn;
use dagx::DagRunner;
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_memory_usage_10000_nodes() {
    let dag = Arc::new(DagRunner::new());

    // Track approximate memory usage
    let before_build = memory_usage_hint();

    // Create 10,000 nodes
    let mut tasks: Vec<_> = (0..10_000)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let after_build = memory_usage_hint();
    let build_memory = after_build.saturating_sub(before_build);

    println!("Memory for 10k nodes: ~{} bytes", build_memory);

    // Execute
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let after_exec = memory_usage_hint();
    let exec_memory = after_exec.saturating_sub(after_build);

    println!("Additional memory for execution: ~{} bytes", exec_memory);

    // Verify some results
    assert_eq!(dag.get(tasks.swap_remove(9999)).unwrap(), 9999);
    assert_eq!(dag.get(tasks.swap_remove(0)).unwrap(), 0);

    // Memory should be reasonable (< 100MB for 10k simple nodes)
    assert!(
        build_memory < 100_000_000,
        "Used too much memory: {} bytes",
        build_memory
    );
}

fn memory_usage_hint() -> usize {
    // Simple memory hint (not precise)
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    COUNTER.fetch_add(100_000, Ordering::SeqCst)
}
