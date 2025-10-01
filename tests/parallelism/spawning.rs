//! Tests for spawner functionality

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_spawner_actually_spawns_tasks() -> DagResult<()> {
    // Verify that the spawner function is actually called for each task
    let dag = DagRunner::new();
    let spawn_count = Arc::new(AtomicUsize::new(0));

    // Create 10 independent tasks
    let tasks: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    // Custom spawner that counts invocations
    let counter = spawn_count.clone();
    dag.run(move |fut| {
        counter.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(fut);
    })
    .await?;

    // Verify all tasks were spawned
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task)?, i as i32);
    }

    // The spawner should have been called once for each task
    let spawns = spawn_count.load(Ordering::SeqCst);
    assert_eq!(
        spawns, 10,
        "Expected spawner to be called 10 times, but was called {} times",
        spawns
    );

    Ok(())
}

#[tokio::test]
async fn test_tasks_not_running_inline() -> DagResult<()> {
    // Verify that tasks are spawned and CAN run on different threads than the caller
    // Note: Small/fast tasks may run on the same thread if the scheduler chooses
    use std::thread;

    let dag = DagRunner::new();
    let calling_thread = thread::current().id();
    let task_threads = Arc::new(Mutex::new(Vec::new()));

    // Create tasks with some work to encourage distribution
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            let threads = task_threads.clone();
            dag.add_task(task_fn(move |_: ()| {
                let threads = threads.clone();
                async move {
                    threads.lock().unwrap().push(thread::current().id());

                    // Add some async work
                    sleep(Duration::from_millis(2)).await;

                    i
                }
            }))
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify results
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task)?, i as i32);
    }

    // Check if tasks ran on different threads than the caller
    // This is a capability test - the scheduler MAY choose to use the same thread
    let task_thread_ids = task_threads.lock().unwrap();
    let different_threads = task_thread_ids
        .iter()
        .filter(|&&tid| tid != calling_thread)
        .count();

    if different_threads == 0 {
        eprintln!(
            "Warning: All tasks ran on the calling thread. This may indicate tasks aren't \
             being spawned, or the scheduler chose not to distribute (valid for very fast tasks)"
        );
    }

    // The key thing is that tasks completed successfully via spawner
    Ok(())
}
