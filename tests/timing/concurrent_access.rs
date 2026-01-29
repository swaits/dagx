//! Tests for concurrent access patterns and race conditions

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};
use futures::FutureExt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_concurrent_result_retrieval() -> DagResult<()> {
    // Test concurrent get() calls after execution
    let dag = Arc::new(DagRunner::new());

    // Add tasks with minimal delays
    let tasks: Vec<dagx::TaskHandle<usize>> = (0..10)
        .map(|i| {
            let task = dag.add_task(task_fn(move |_: ()| async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                i as usize
            }));
            task.into()
        })
        .collect();

    // Execute the DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Now try to get results concurrently from multiple threads
    let mut get_handles = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        let dag_clone = dag.clone();
        let task = *task;
        get_handles.push(tokio::spawn(async move {
            // Results should be available immediately
            dag_clone.get(task).map(|v| (i, v))
        }));
    }

    // Verify all gets succeeded
    for handle in get_handles {
        let (i, result) = handle.await.unwrap()?;
        assert_eq!(result, i);
    }

    Ok(())
}

#[tokio::test]
async fn test_simultaneous_dag_building_and_execution() -> DagResult<()> {
    // Test building DAG while another part is executing
    let dag = Arc::new(DagRunner::new());
    let building_complete = Arc::new(AtomicBool::new(false));

    // Start with a few initial tasks
    let initial: Vec<_> = (0..5)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    // Start building more tasks in background
    let dag_build = dag.clone();
    let building = building_complete.clone();
    let build_handle = tokio::spawn(async move {
        let mut tasks: Vec<dagx::TaskHandle<i32>> = Vec::new();
        for i in 5..50 {
            let task = dag_build.add_task(task_fn(move |_: ()| async move { i }));
            tasks.push(task.into());
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
        building.store(true, Ordering::SeqCst);
        tasks
    });

    // Wait a bit for some tasks to be added
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Execute the DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Get the additional tasks
    let additional_tasks = build_handle.await.unwrap();

    // Initial tasks should have results
    for (i, task) in initial.into_iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
    }

    // Additional tasks added during execution won't have results yet
    // (they need another run cycle)
    if building_complete.load(Ordering::SeqCst) {
        // Run again for the new tasks
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

        // Now they should have results
        for (i, task) in additional_tasks.iter().enumerate() {
            assert_eq!(dag.get(task)?, (i + 5) as i32);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_concurrent_dependency_resolution() -> DagResult<()> {
    // Test multiple tasks resolving dependencies concurrently
    let dag = DagRunner::new();
    let resolution_counter = Arc::new(AtomicUsize::new(0));

    // Create a source task
    let source: TaskHandle<_> = dag
        .add_task(task_fn({
            let counter = resolution_counter.clone();
            move |_: ()| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    42
                }
            }
        }))
        .into();

    // Create many tasks depending on the same source
    let dependents: Vec<_> = (0..100)
        .map(|i| {
            let counter = resolution_counter.clone();
            dag.add_task(task_fn(move |val: i32| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    val + i
                }
            }))
            .depends_on(source)
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Check all completed
    assert_eq!(resolution_counter.load(Ordering::SeqCst), 101);

    // Verify results
    for (i, task) in dependents.iter().enumerate() {
        assert_eq!(dag.get(task)?, 42 + i as i32);
    }

    Ok(())
}

#[tokio::test]
async fn test_read_write_race_patterns() -> DagResult<()> {
    // Test read-write patterns with shared state
    let dag = DagRunner::new();
    let shared_state = Arc::new(RwLock::new(Vec::new()));

    // Writers
    let writers: Vec<_> = (0..10)
        .map(|i| {
            let state = shared_state.clone();
            dag.add_task(task_fn(move |_: ()| {
                let state = state.clone();
                async move {
                    let mut guard = state.write().await;
                    guard.push(i);
                    tokio::task::yield_now().await; // Force potential race
                    guard.push(i * 10);
                    i
                }
            }))
            .into()
        })
        .collect();

    // Readers (depend on writers to ensure ordering)
    let readers: Vec<_> = writers
        .iter()
        .map(|writer| {
            let state = shared_state.clone();
            dag.add_task(task_fn(move |writer_id: i32| {
                let state = state.clone();
                async move {
                    let guard = state.read().await;
                    let contains_writer = guard.contains(&writer_id);
                    let len = guard.len();
                    (writer_id, contains_writer, len)
                }
            }))
            .depends_on(writer)
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify readers saw their writer's data
    for reader in &readers {
        let (writer_id, saw_writer, _len) = dag.get(reader)?;
        assert!(saw_writer, "Reader didn't see writer {}", writer_id);
    }

    // Final state should have all writes
    let final_state = shared_state.read().await;
    assert_eq!(final_state.len(), 20); // 10 writers * 2 writes each

    Ok(())
}

#[tokio::test]
async fn test_atomic_counter_races() -> DagResult<()> {
    // Test atomic operations under high concurrency
    let dag = DagRunner::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    // Create tasks that increment and track max
    let _tasks: Vec<_> = (0..200)
        .map(|_| {
            let counter = counter.clone();
            let max_seen = max_seen.clone();
            dag.add_task(task_fn(move |_: ()| {
                let counter = counter.clone();
                let max_seen = max_seen.clone();
                async move {
                    let mut local_max = 0;
                    for _ in 0..100 {
                        let val = counter.fetch_add(1, Ordering::SeqCst);
                        local_max = local_max.max(val);

                        // Update global max
                        let mut current_max = max_seen.load(Ordering::SeqCst);
                        while val > current_max {
                            match max_seen.compare_exchange_weak(
                                current_max,
                                val,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            ) {
                                Ok(_) => break,
                                Err(x) => current_max = x,
                            }
                        }
                    }
                    local_max
                }
            }))
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Final counter should be 200 * 100 = 20000
    assert_eq!(counter.load(Ordering::SeqCst), 20000);

    // Max seen should be at least 19999 (last increment - 1)
    assert!(max_seen.load(Ordering::SeqCst) >= 19999);

    Ok(())
}

#[tokio::test]
async fn test_multiple_dag_runners_interaction() -> DagResult<()> {
    // Test multiple DAG runners potentially interfering
    let shared_counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    for dag_id in 0..5 {
        let counter = shared_counter.clone();
        let handle = tokio::spawn(async move {
            let dag = DagRunner::new();

            let tasks: Vec<_> = (0..20)
                .map(|task_id| {
                    let counter = counter.clone();
                    dag.add_task(task_fn(move |_: ()| {
                        let counter = counter.clone();
                        async move {
                            counter.fetch_add(1, Ordering::SeqCst);
                            (dag_id, task_id)
                        }
                    }))
                })
                .collect();

            dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

            // Collect results
            let mut results = Vec::new();
            for task in tasks {
                results.push(dag.get(task)?);
            }

            Ok::<_, dagx::DagError>(results)
        });
        handles.push(handle);
    }

    // Wait for all DAGs to complete
    for handle in handles {
        handle.await.unwrap()?;
    }

    // Should have executed 5 * 20 = 100 tasks total
    assert_eq!(shared_counter.load(Ordering::SeqCst), 100);

    Ok(())
}
