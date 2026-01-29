//! Parallelism proof tests

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_parallelism_proof_1000_tasks() -> DagResult<()> {
    // Proof that 1000 tasks sleeping 100ms each complete in ~100ms (not 100 seconds)
    let dag = DagRunner::new();

    let sleep_duration = Duration::from_millis(100);
    let num_tasks = 1000u32;

    // Create 1000 independent tasks that each sleep for 100ms
    let tasks: Vec<_> = (0..num_tasks)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                sleep(sleep_duration).await;
                i
            }))
        })
        .collect();

    let start = Instant::now();
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
    let elapsed = start.elapsed();

    // Verify all tasks completed
    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(dag.get(task)?, i as u32);
    }

    // Sequential execution would take: 1000 * 100ms = 100 seconds
    let sequential_time = sleep_duration * num_tasks;

    println!(
        "Parallelism proof: {} tasks × {}ms completed in {:?} (sequential would be {:?})",
        num_tasks,
        sleep_duration.as_millis(),
        elapsed,
        sequential_time
    );

    // Allow generous overhead: should complete in less than 500ms (not 100 seconds!)
    assert!(
        elapsed < Duration::from_millis(500),
        "Tasks appear to be running sequentially! Took {:?} instead of ~100ms",
        elapsed
    );

    // Verify we achieved massive speedup
    let speedup = sequential_time.as_secs_f64() / elapsed.as_secs_f64();
    assert!(
        speedup > 200.0,
        "Speedup too low: {:.1}x (expected > 200x)",
        speedup
    );

    Ok(())
}
