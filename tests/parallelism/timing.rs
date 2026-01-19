//! Tests for timing and speedup verification

use crate::common::task_fn;

use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_parallel_execution_speedup() -> DagResult<()> {
    // Prove parallel execution by comparing timing
    let dag = DagRunner::new();

    let work_duration = Duration::from_millis(50);
    let num_tasks = 10;

    // Create independent tasks that each take 50ms
    let tasks: Vec<_> = (0..num_tasks)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                sleep(work_duration).await;
                i
            }))
        })
        .collect();

    let start = Instant::now();
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
    let elapsed = start.elapsed();

    // Verify all tasks completed
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
    }

    // If tasks ran sequentially: 10 * 50ms = 500ms
    // If tasks ran in parallel: ~50ms (plus overhead)
    let sequential_time = work_duration * num_tasks as u32;

    println!(
        "Parallel execution took {:?}, sequential would be {:?}",
        elapsed, sequential_time
    );

    // Parallel execution should be significantly faster
    assert!(
        elapsed < sequential_time / 2,
        "Parallel execution too slow: {:?} vs sequential {:?}",
        elapsed,
        sequential_time
    );

    Ok(())
}

struct SyncTask;
#[dagx::task]
impl SyncTask {
    fn run(&self, x: &i32) -> i32 {
        // Synchronous task - no async/await
        std::thread::sleep(Duration::from_millis(1)); // Blocking sleep
        x * 2
    }
}

struct AsyncTask;
#[dagx::task]
impl AsyncTask {
    async fn run(&self, x: &i32) -> i32 {
        // Async task
        sleep(Duration::from_millis(1)).await; // Non-blocking sleep
        x * 3
    }
}

#[tokio::test]
async fn test_sync_tasks_dont_block_async_runtime() -> DagResult<()> {
    // Test that sync tasks don't block the async runtime
    let dag = DagRunner::new();

    let start_times = Arc::new(Mutex::new(Vec::new()));

    // Mix of sync and async tasks
    let source = dag.add_task(task_fn(|_: ()| async { 10 }));

    // Sync tasks
    let sync1 = dag.add_task(SyncTask).depends_on(&source);
    let sync2 = dag.add_task(SyncTask).depends_on(&source);

    // Async tasks
    let async1 = dag.add_task(AsyncTask).depends_on(&source);
    let async2 = dag.add_task(AsyncTask).depends_on(&source);

    // Track timing
    let timing_task = {
        let times = start_times.clone();
        dag.add_task(task_fn(move |_: ()| {
            let times = times.clone();
            async move {
                for i in 0..5 {
                    times
                        .lock()
                        .unwrap()
                        .push((format!("tick_{}", i), Instant::now()));
                    sleep(Duration::from_millis(5)).await;
                }
                "timing"
            }
        }))
    };

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify results
    assert_eq!(dag.get(sync1)?, 20);
    assert_eq!(dag.get(sync2)?, 20);
    assert_eq!(dag.get(async1)?, 30);
    assert_eq!(dag.get(async2)?, 30);
    assert_eq!(dag.get(timing_task)?, "timing");

    // Check that timing task made progress (wasn't blocked by sync tasks)
    let times = start_times.lock().unwrap();
    assert_eq!(
        times.len(),
        5,
        "Timing task should have completed all ticks"
    );

    Ok(())
}
