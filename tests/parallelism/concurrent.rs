//! Concurrent access tests

use crate::common::task_fn;
use dagx::DagRunner;
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

#[tokio::test]
async fn test_concurrent_run_calls() {
    // Test that multiple run() calls are handled correctly
    let dag = Arc::new(DagRunner::new());

    // Add some tasks
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                i
            }))
        })
        .collect();

    // Try to run the DAG multiple times concurrently
    let mut handles = JoinSet::new();

    for _ in 0..5 {
        let dag = Arc::clone(&dag);
        handles.spawn(async move { dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await });
    }

    // Only one run should succeed, others should fail with concurrent execution error
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = handles.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    // Exactly one should succeed, the rest should fail
    assert_eq!(success_count, 1);
    assert_eq!(error_count, 4);

    // Verify results are consistent from the successful run
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task).unwrap(), i);
    }
}

#[tokio::test]
async fn test_concurrent_execution_and_building() {
    // Test that we can build and execute concurrently
    let dag = Arc::new(DagRunner::new());

    // Start with some initial tasks
    let initial: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    // Execute the initial DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Now try to add more tasks while also reading results
    let mut handles = JoinSet::new();

    // Reader tasks
    for task in &initial {
        let dag = Arc::clone(&dag);
        let task_handle: dagx::TaskHandle<i32> = task.into();
        handles.spawn(async move { dag.get(task_handle).unwrap() });
    }

    // Builder tasks (adding new nodes)
    for i in 10..20 {
        let dag = Arc::clone(&dag);
        handles.spawn(async move {
            dag.add_task(task_fn(move |_: ()| async move { i }));
            i
        });
    }

    // Wait for all operations
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }

    // Execute again with the new tasks
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_dag_builder_thread_safety() {
    // Test that the DAG builder is thread-safe
    let dag = Arc::new(DagRunner::new());
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = JoinSet::new();

    // Spawn tasks that build complex sub-graphs concurrently
    for thread_id in 0..10 {
        let dag = Arc::clone(&dag);
        let counter = Arc::clone(&counter);

        handles.spawn(async move {
            // Each thread creates a small pipeline
            let source = dag.add_task(task_fn(move |_: ()| async move { thread_id }));

            let transform1 = dag
                .add_task(task_fn(move |x: i32| async move { x * 10 }))
                .depends_on(&source);

            let transform2 = dag
                .add_task(task_fn(move |x: i32| async move { x + thread_id }))
                .depends_on(transform1);

            counter.fetch_add(1, Ordering::SeqCst);
            transform2
        });
    }

    let mut final_tasks = vec![];
    while let Some(result) = handles.join_next().await {
        final_tasks.push(result.unwrap());
    }

    // Verify all threads completed
    assert_eq!(counter.load(Ordering::SeqCst), 10);

    // Execute the combined DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Verify each pipeline computed correctly
    for task in final_tasks.iter() {
        let result = dag.get(task).unwrap();
        // Each pipeline: thread_id * 10 + thread_id
        assert!((0..100).contains(&result));
    }
}

#[tokio::test]
async fn test_concurrent_get_operations() {
    // Test concurrent get() operations
    let dag = Arc::new(DagRunner::new());

    // Create and execute tasks
    let tasks: Vec<_> = (0..100)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Now access results concurrently
    let mut handles = JoinSet::new();

    for task in tasks.iter() {
        let dag = Arc::clone(&dag);
        let task_handle: dagx::TaskHandle<i32> = task.into();
        handles.spawn(async move { dag.get(task_handle).unwrap() });
    }

    // Collect and verify results
    let mut results = vec![];
    while let Some(result) = handles.join_next().await {
        results.push(result.unwrap());
    }

    // Results should contain all values 0..100
    results.sort();
    for (i, val) in results.iter().enumerate() {
        assert_eq!(*val, i as i32);
    }
}

#[tokio::test]
async fn test_no_race_in_dependency_tracking() {
    // Test that concurrent dependency modifications don't cause races
    let dag = Arc::new(DagRunner::new());
    let source = dag.add_task(task_fn(|_: ()| async { 42 }));

    let mut handles = JoinSet::new();

    // Multiple threads create tasks depending on the same source
    let source_handle: dagx::TaskHandle<i32> = (&source).into();
    for i in 0..50 {
        let dag = Arc::clone(&dag);
        let source_clone = source_handle;

        handles.spawn(async move {
            dag.add_task(task_fn(move |x: i32| async move { x + i }))
                .depends_on(source_clone)
        });
    }

    let mut dependents = vec![];
    while let Some(result) = handles.join_next().await {
        dependents.push(result.unwrap());
    }

    // Execute
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // All dependents should have correct values
    for dep in dependents.iter() {
        let result = dag.get(dep).unwrap();
        assert!((42..92).contains(&result)); // 42 + (0..50)
    }
}

#[tokio::test]
async fn test_high_concurrency_stress() {
    // Stress test with high concurrency
    let dag = Arc::new(DagRunner::new());
    let completed = Arc::new(AtomicUsize::new(0));

    let mut handles = JoinSet::new();

    // 1000 concurrent operations
    for i in 0..1000 {
        let dag = Arc::clone(&dag);
        let completed = Arc::clone(&completed);

        handles.spawn(async move {
            // Mix of operations
            match i % 3 {
                0 => {
                    // Add a source task
                    dag.add_task(task_fn(move |_: ()| async move { i }));
                }
                1 => {
                    // Add a task with a simple dependency
                    let source = dag.add_task(task_fn(move |_: ()| async move { i }));
                    dag.add_task(task_fn(move |x: i32| async move { x * 2 }))
                        .depends_on(&source);
                }
                2 => {
                    // Add a more complex subgraph
                    let a = dag.add_task(task_fn(move |_: ()| async move { i }));
                    let b = dag.add_task(task_fn(move |_: ()| async move { i + 1 }));
                    dag.add_task(task_fn(move |(x, y): (i32, i32)| async move { x + y }))
                        .depends_on((&a, &b));
                }
                _ => unreachable!(),
            }
            completed.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }

    assert_eq!(completed.load(Ordering::SeqCst), 1000);

    // The DAG should still be executable
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_lock_contention_measurement() {
    // Test to measure lock contention under concurrent load
    let dag = Arc::new(DagRunner::new());
    let start = tokio::time::Instant::now();
    let operations = Arc::new(AtomicUsize::new(0));

    let mut handles = JoinSet::new();

    // Create many concurrent operations that will contend for locks
    for i in 0..100 {
        let dag = Arc::clone(&dag);
        let ops = Arc::clone(&operations);

        handles.spawn(async move {
            for j in 0..10 {
                dag.add_task(task_fn(move |_: ()| async move { i * 10 + j }));
                ops.fetch_add(1, Ordering::SeqCst);

                // Small yield to increase contention likelihood
                tokio::task::yield_now().await;
            }
        });
    }

    // Wait for completion
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }

    let duration = start.elapsed();
    let total_ops = operations.load(Ordering::SeqCst);

    println!(
        "Completed {} operations in {:?} ({:.0} ops/sec)",
        total_ops,
        duration,
        total_ops as f64 / duration.as_secs_f64()
    );

    // Should complete reasonably quickly even with contention
    assert!(
        duration < Duration::from_secs(5),
        "Too much lock contention"
    );
}

#[tokio::test]
async fn test_atomic_dag_operations() {
    // Test that DAG operations are atomic (all-or-nothing)
    let dag = Arc::new(DagRunner::new());

    // Create a base task
    let base = dag.add_task(task_fn(|_: ()| async { 100 }));

    // Try to create dependent tasks concurrently
    let mut handles = JoinSet::new();
    let success_count = Arc::new(AtomicUsize::new(0));

    let base_handle: dagx::TaskHandle<i32> = (&base).into();
    for i in 0..50 {
        let dag = Arc::clone(&dag);
        let base_clone = base_handle;
        let success = Arc::clone(&success_count);

        handles.spawn(async move {
            let dep = dag
                .add_task(task_fn(move |x: i32| async move { x + i }))
                .depends_on(base_clone);

            success.fetch_add(1, Ordering::SeqCst);
            dep
        });
    }

    let mut deps = vec![];
    while let Some(result) = handles.join_next().await {
        deps.push(result.unwrap());
    }

    // All operations should have succeeded
    assert_eq!(success_count.load(Ordering::SeqCst), 50);
    assert_eq!(deps.len(), 50);

    // Execute and verify
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    for dep in deps.iter() {
        let result = dag.get(dep).unwrap();
        assert!((100..150).contains(&result));
    }
}

#[tokio::test]
async fn test_no_deadlock_on_circular_waiting() {
    // Test that we don't deadlock even with complex concurrent operations
    let dag = Arc::new(DagRunner::new());

    // Create multiple threads that might cause circular waiting patterns
    let mut handles = JoinSet::new();

    for i in 0..10 {
        let dag = Arc::clone(&dag);

        handles.spawn(async move {
            // Each thread creates a chain of tasks
            let first = dag.add_task(task_fn(move |_: ()| async move { i }));
            let mut chain = vec![(&first).into()];

            for j in 0..5 {
                let next = dag
                    .add_task(task_fn(move |x: i32| async move { x + j }))
                    .depends_on(chain.last().unwrap());
                chain.push(next);

                // Yield to potentially cause interleaving
                tokio::task::yield_now().await;
            }

            *chain.last().unwrap()
        });
    }

    // Set a timeout to detect deadlocks
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut finals = vec![];
        while let Some(result) = handles.join_next().await {
            finals.push(result.unwrap());
        }
        finals
    })
    .await;

    assert!(result.is_ok(), "Deadlock detected - operation timed out");

    // Should be able to execute the DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
}
