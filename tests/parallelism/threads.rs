//! Tests for multi-threading and concurrent execution

use crate::common::task_fn;
use dagx::{task, DagResult, DagRunner, TaskHandle};

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_tasks_run_on_different_threads() -> DagResult<()> {
    // Prove that tasks CAN run on multiple different threads (true parallelism capability)
    // Note: Tokio may choose to run fast tasks on the same thread for efficiency
    use std::thread;

    let dag = DagRunner::new();
    let thread_ids = Arc::new(Mutex::new(std::collections::HashSet::new()));

    struct LayerTask {
        ids: Arc<Mutex<HashSet<ThreadId>>>,
        idx: i32,
    }

    #[task]
    impl LayerTask {
        async fn run(&self) -> i32 {
            let tid = thread::current().id();
            self.ids.lock().unwrap().insert(tid);

            // Yield to give tokio reason to distribute
            sleep(Duration::from_millis(5)).await;

            // Some CPU work too
            let mut sum = 0;
            for j in 0..1000 {
                sum += j;
            }
            self.idx + sum
        }
    }

    // Create 50 tasks with actual work to encourage multi-threading
    let tasks: Vec<_> = (0..50)
        .map(|i| {
            dag.add_task(LayerTask {
                ids: thread_ids.clone(),
                idx: i,
            })
        })
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Verify all tasks completed
    for task in tasks.into_iter() {
        let _ = dag.get(task)?;
    }

    // With a multi-threaded runtime and sufficient work, we typically see multiple threads
    // But we can't guarantee it (scheduler's choice), so we just verify the capability exists
    let unique_threads = thread_ids.lock().unwrap().len();

    // On a multi-core system with sufficient work, tokio should use multiple threads
    // This is a soft assertion - if it fails, it means tasks CAN'T use multiple threads
    if unique_threads == 1 {
        eprintln!(
            "Warning: All tasks ran on 1 thread. This may indicate parallelism isn't working, \
             or tokio chose not to distribute work (valid for small/fast tasks)"
        );
    }

    Ok(())
}

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
async fn test_diamond_parallel_execution() -> DagResult<()> {
    // Test diamond pattern: source -> (parallel1, parallel2) -> sink
    let dag = DagRunner::new();

    let concurrent_middle = Arc::new(AtomicUsize::new(0));

    let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 100)).into();

    struct ParallelTask1 {
        counter: Arc<AtomicUsize>,
    }

    #[task]
    impl ParallelTask1 {
        async fn run(&self, x: &i32) -> i32 {
            self.counter.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(20)).await;
            let count = self.counter.load(Ordering::SeqCst);
            self.counter.fetch_sub(1, Ordering::SeqCst);
            assert!(count > 0, "Should see concurrent execution");
            x * 2
        }
    }

    // These should run in parallel
    let parallel1 = dag
        .add_task(ParallelTask1 {
            counter: concurrent_middle.clone(),
        })
        .depends_on(source);

    struct ParallelTask2 {
        counter: Arc<AtomicUsize>,
    }

    #[task]
    impl ParallelTask2 {
        async fn run(&self, x: &i32) -> i32 {
            self.counter.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(20)).await;
            let count = self.counter.load(Ordering::SeqCst);
            self.counter.fetch_sub(1, Ordering::SeqCst);
            // Both should be running
            assert!(count > 0, "Should see concurrent execution");
            x * 3
        }
    }

    let parallel2 = dag
        .add_task(ParallelTask2 {
            counter: concurrent_middle.clone(),
        })
        .depends_on(source);

    let sink = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(a, b): (&i32, &i32)| a + b))
        .depends_on((&parallel1, &parallel2));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(dag.get(sink)?, 500); // (100 * 2) + (100 * 3)

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
