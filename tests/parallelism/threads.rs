//! Tests for multi-threading and concurrent execution

use dagx::{task, DagResult, DagRunner, TaskHandle};
use dagx_test::task_fn;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_concurrent_execution_with_atomic_counter() -> DagResult<()> {
    // Prove concurrent execution using atomic counters to track simultaneous tasks
    let dag = DagRunner::new();
    let concurrent_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));

    struct LayerTask {
        count: Arc<AtomicUsize>,
        max: Arc<AtomicUsize>,
        idx: usize,
    }

    #[task]
    impl LayerTask {
        async fn run(&self) -> usize {
            // Increment concurrent counter
            let current = self.count.fetch_add(1, Ordering::SeqCst) + 1;

            // Track maximum concurrency
            let mut prev_max = self.max.load(Ordering::SeqCst);
            while current > prev_max {
                match self.max.compare_exchange_weak(
                    prev_max,
                    current,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(x) => prev_max = x,
                }
            }

            // Simulate work
            sleep(Duration::from_millis(10)).await;

            // Decrement counter
            self.count.fetch_sub(1, Ordering::SeqCst);

            self.idx
        }
    }

    // Create 20 tasks that track concurrent execution
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            dag.add_task(LayerTask {
                count: concurrent_count.clone(),
                max: max_concurrent.clone(),
                idx: i,
            })
        })
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Verify all tasks completed
    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
    }

    // Check that we achieved real parallelism
    let max_seen = max_concurrent.load(Ordering::SeqCst);
    println!("Maximum concurrent tasks: {}", max_seen);

    // Should see multiple tasks running concurrently
    assert!(
        max_seen > 1,
        "Expected concurrent execution, max was {}",
        max_seen
    );

    // Final count should be 0
    assert_eq!(concurrent_count.load(Ordering::SeqCst), 0);

    Ok(())
}

#[tokio::test]
async fn test_massive_parallel_fanout() -> DagResult<()> {
    // Test extreme parallelism with 1000 parallel tasks
    let dag = DagRunner::new();

    let completed = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current = Arc::new(AtomicUsize::new(0));

    let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 1)).into();

    struct FanoutTask {
        comp: Arc<AtomicUsize>,
        max: Arc<AtomicUsize>,
        curr: Arc<AtomicUsize>,
        idx: i32,
    }

    #[task]
    impl FanoutTask {
        async fn run(&self, x: &i32) -> i32 {
            // Track concurrency
            let c = self.curr.fetch_add(1, Ordering::SeqCst) + 1;

            let mut prev_max = self.max.load(Ordering::SeqCst);
            while c > prev_max {
                match self.max.compare_exchange_weak(
                    prev_max,
                    c,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(v) => prev_max = v,
                }
            }

            // Quick work
            sleep(Duration::from_millis(100)).await;

            self.curr.fetch_sub(1, Ordering::SeqCst);
            self.comp.fetch_add(1, Ordering::SeqCst);
            x + self.idx
        }
    }

    // Create 1000 tasks all depending on the same source
    let tasks: Vec<_> = (0..1000)
        .map(|i| {
            dag.add_task(FanoutTask {
                comp: completed.clone(),
                max: max_concurrent.clone(),
                curr: current.clone(),
                idx: i,
            })
            .depends_on(source)
        })
        .collect();

    let start = Instant::now();
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;
    let elapsed = start.elapsed();

    // All should complete
    assert_eq!(completed.load(Ordering::SeqCst), 1000);

    // Check some results
    assert_eq!(dag.get(tasks[0])?, 1);
    assert_eq!(dag.get(tasks[500])?, 501);
    assert_eq!(dag.get(tasks[999])?, 1000);

    let max_seen = max_concurrent.load(Ordering::SeqCst);
    println!(
        "Massive fanout: {} concurrent tasks, completed in {:?}",
        max_seen, elapsed
    );

    // Should see significant parallelism
    assert!(max_seen > 10, "Expected high parallelism, got {}", max_seen);

    // Should complete quickly (not sequentially)
    assert!(
        elapsed < Duration::from_secs(2),
        "1000 tasks took too long: {:?}",
        elapsed
    );

    Ok(())
}
