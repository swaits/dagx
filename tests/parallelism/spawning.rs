//! Tests for spawner functionality

use dagx::task_fn;
use dagx::{DagResult, DagRunner};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_spawner_actually_spawns_tasks() -> DagResult<()> {
    // Verify that the spawner function is actually called for each task
    let dag = DagRunner::new();
    let spawn_count = Arc::new(AtomicUsize::new(0));

    // Create 10 independent tasks
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            dag.add_task(task_fn::<(), _, _>({
                let value = spawn_count.clone();
                move |_: ()| {
                    let value = value.clone();
                    value.fetch_add(1, Ordering::SeqCst);
                    i
                }
            }))
        })
        .collect();

    // Custom spawner that counts invocations
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Verify all tasks were spawned
    for (i, task) in tasks.into_iter().enumerate() {
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
