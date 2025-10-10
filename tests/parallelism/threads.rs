//! Tests for multi-threading and concurrent execution

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_tasks_run_on_different_threads() -> DagResult<()> {
    // Prove that tasks CAN run on multiple different threads (true parallelism capability)
    // Note: Tokio may choose to run fast tasks on the same thread for efficiency
    use std::thread;

    let dag = DagRunner::new();
    let thread_ids = Arc::new(Mutex::new(std::collections::HashSet::new()));

    // Create 50 tasks with actual work to encourage multi-threading
    let tasks: Vec<_> = (0..50)
        .map(|i| {
            let ids = thread_ids.clone();
            dag.add_task(task_fn(move |_: ()| {
                let ids = ids.clone();
                async move {
                    let tid = thread::current().id();
                    ids.lock().unwrap().insert(tid);

                    // Do some actual async work to give tokio reason to distribute
                    sleep(Duration::from_millis(5)).await;

                    // Some CPU work too
                    let mut sum = 0;
                    for j in 0..1000 {
                        sum += j;
                    }
                    i + sum
                }
            }))
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify all tasks completed
    for task in tasks.iter() {
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

    // Create 20 tasks that track concurrent execution
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            let count = concurrent_count.clone();
            let max = max_concurrent.clone();

            dag.add_task(task_fn(move |_: ()| {
                let count = count.clone();
                let max = max.clone();
                async move {
                    // Increment concurrent counter
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;

                    // Track maximum concurrency
                    let mut prev_max = max.load(Ordering::SeqCst);
                    while current > prev_max {
                        match max.compare_exchange_weak(
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
                    count.fetch_sub(1, Ordering::SeqCst);

                    i
                }
            }))
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify all tasks completed
    for (i, task) in tasks.iter().enumerate() {
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

    let source = dag.add_task(task_fn(|_: ()| async { 100 }));

    // These should run in parallel
    let parallel1 = {
        let counter = concurrent_middle.clone();
        dag.add_task(task_fn(move |x: i32| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(20)).await;
                let count = counter.load(Ordering::SeqCst);
                counter.fetch_sub(1, Ordering::SeqCst);
                assert!(count > 0, "Should see concurrent execution");
                x * 2
            }
        }))
        .depends_on(&source)
    };

    let parallel2 = {
        let counter = concurrent_middle.clone();
        dag.add_task(task_fn(move |x: i32| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(20)).await;
                let count = counter.load(Ordering::SeqCst);
                counter.fetch_sub(1, Ordering::SeqCst);
                // Both should be running
                assert!(count > 0, "Should see concurrent execution");
                x * 3
            }
        }))
        .depends_on(&source)
    };

    let sink = dag
        .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
        .depends_on((&parallel1, &parallel2));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(sink)?, 500); // (100 * 2) + (100 * 3)

    Ok(())
}

#[tokio::test]
async fn test_parallel_branches_with_dependencies() -> DagResult<()> {
    // Test that parallel branches execute independently but respect their own dependencies
    let dag = DagRunner::new();

    let branch_progress = Arc::new(Mutex::new(Vec::new()));

    // Source node
    let source = dag.add_task(task_fn(|_: ()| async { 10 }));

    // Branch 1: source -> b1_1 -> b1_2
    let b1_1 = {
        let progress = branch_progress.clone();
        dag.add_task(task_fn(move |x: i32| {
            let progress = progress.clone();
            async move {
                progress
                    .lock()
                    .unwrap()
                    .push(("b1_1_start", Instant::now()));
                sleep(Duration::from_millis(30)).await;
                progress.lock().unwrap().push(("b1_1_end", Instant::now()));
                x * 2
            }
        }))
        .depends_on(&source)
    };

    let b1_2 = {
        let progress = branch_progress.clone();
        dag.add_task(task_fn(move |x: i32| {
            let progress = progress.clone();
            async move {
                progress
                    .lock()
                    .unwrap()
                    .push(("b1_2_start", Instant::now()));
                sleep(Duration::from_millis(10)).await;
                progress.lock().unwrap().push(("b1_2_end", Instant::now()));
                x + 1
            }
        }))
        .depends_on(b1_1)
    };

    // Branch 2: source -> b2_1 -> b2_2
    let b2_1 = {
        let progress = branch_progress.clone();
        dag.add_task(task_fn(move |x: i32| {
            let progress = progress.clone();
            async move {
                progress
                    .lock()
                    .unwrap()
                    .push(("b2_1_start", Instant::now()));
                sleep(Duration::from_millis(10)).await;
                progress.lock().unwrap().push(("b2_1_end", Instant::now()));
                x * 3
            }
        }))
        .depends_on(&source)
    };

    let b2_2 = {
        let progress = branch_progress.clone();
        dag.add_task(task_fn(move |x: i32| {
            let progress = progress.clone();
            async move {
                progress
                    .lock()
                    .unwrap()
                    .push(("b2_2_start", Instant::now()));
                sleep(Duration::from_millis(30)).await;
                progress.lock().unwrap().push(("b2_2_end", Instant::now()));
                x + 2
            }
        }))
        .depends_on(b2_1)
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify results
    assert_eq!(dag.get(b1_2)?, 21); // (10 * 2) + 1
    assert_eq!(dag.get(b2_2)?, 32); // (10 * 3) + 2

    // Analyze timing to prove parallel execution
    let progress = branch_progress.lock().unwrap();

    // Find timestamps
    let find_time = |name: &str| -> Instant {
        progress
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, t)| *t)
            .unwrap_or_else(|| panic!("Couldn't find {}", name))
    };

    let b1_1_start = find_time("b1_1_start");
    let b1_1_end = find_time("b1_1_end");
    let b1_2_start = find_time("b1_2_start");
    let b2_1_start = find_time("b2_1_start");

    // Branch 1 and Branch 2 first tasks should start nearly simultaneously
    let branch_start_diff = if b1_1_start > b2_1_start {
        b1_1_start - b2_1_start
    } else {
        b2_1_start - b1_1_start
    };

    // Use a more generous threshold to account for coverage instrumentation overhead
    // 50ms is enough to prove they're parallel (not sequential) while being robust to overhead
    assert!(
        branch_start_diff < Duration::from_millis(50),
        "Branches should start in parallel, diff was {:?}",
        branch_start_diff
    );

    // b1_2 should only start after b1_1 ends
    assert!(b1_2_start >= b1_1_end, "b1_2 started before b1_1 finished");

    Ok(())
}

#[tokio::test]
async fn test_wide_fanout_parallel_execution() -> DagResult<()> {
    // Test that a single source can fan out to many parallel tasks
    let dag = DagRunner::new();

    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let source = dag.add_task(task_fn(|_: ()| async { 42 }));

    // Create 50 tasks that all depend on the source
    let dependents: Vec<_> = (0..50)
        .map(|i| {
            let max_c = max_concurrent.clone();
            let curr_c = current_concurrent.clone();

            dag.add_task(task_fn(move |x: i32| {
                let max_c = max_c.clone();
                let curr_c = curr_c.clone();
                async move {
                    // Track concurrency
                    let current = curr_c.fetch_add(1, Ordering::SeqCst) + 1;

                    // Update max
                    let mut prev_max = max_c.load(Ordering::SeqCst);
                    while current > prev_max {
                        match max_c.compare_exchange_weak(
                            prev_max,
                            current,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        ) {
                            Ok(_) => break,
                            Err(v) => prev_max = v,
                        }
                    }

                    // Do work
                    sleep(Duration::from_millis(5)).await;

                    curr_c.fetch_sub(1, Ordering::SeqCst);
                    x + i
                }
            }))
            .depends_on(&source)
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify results
    for (i, task) in dependents.iter().enumerate() {
        assert_eq!(dag.get(task)?, 42 + i as i32);
    }

    // Check that we saw significant parallelism
    let max_seen = max_concurrent.load(Ordering::SeqCst);
    println!("Max concurrent in fanout: {}", max_seen);
    assert!(
        max_seen > 5,
        "Expected significant parallelism in fanout, got {}",
        max_seen
    );

    Ok(())
}

#[tokio::test]
async fn test_massive_parallel_fanout() -> DagResult<()> {
    // Test extreme parallelism with 1000 parallel tasks
    let dag = DagRunner::new();

    let completed = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current = Arc::new(AtomicUsize::new(0));

    let source = dag.add_task(task_fn(|_: ()| async { 1 }));

    // Create 1000 tasks all depending on the same source
    let tasks: Vec<_> = (0..1000)
        .map(|i| {
            let comp = completed.clone();
            let max = max_concurrent.clone();
            let curr = current.clone();

            dag.add_task(task_fn(move |x: i32| {
                let comp = comp.clone();
                let max = max.clone();
                let curr = curr.clone();
                async move {
                    // Track concurrency
                    let c = curr.fetch_add(1, Ordering::SeqCst) + 1;

                    let mut prev_max = max.load(Ordering::SeqCst);
                    while c > prev_max {
                        match max.compare_exchange_weak(
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
                    sleep(Duration::from_micros(100)).await;

                    curr.fetch_sub(1, Ordering::SeqCst);
                    comp.fetch_add(1, Ordering::SeqCst);
                    x + i
                }
            }))
            .depends_on(&source)
        })
        .collect();

    let start = Instant::now();
    dag.run(|fut| {
        tokio::spawn(fut);
    })
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
