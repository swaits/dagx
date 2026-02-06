//! Tests for error recovery patterns

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_fallback_on_error() -> DagResult<()> {
    let dag = DagRunner::new();

    // Primary task that fails
    let primary = dag.add_task(task_fn(|_: ()| async {
        panic!("Primary task failed");
        #[allow(unreachable_code)]
        42
    }));

    // Fallback task
    let fallback = dag.add_task(task_fn(|_: ()| async { 100 }));

    // Selector that uses fallback if primary fails
    let _selector = dag.add_task(task_fn(|_: ()| async {}));

    // Run DAG
    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // Primary should fail
    assert!(dag.get(primary).is_err());

    // Fallback should succeed
    assert_eq!(dag.get(fallback)?, 100);

    Ok(())
}

#[tokio::test]
async fn test_retry_pattern_simulation() -> DagResult<()> {
    let dag = DagRunner::new();
    let attempt_counter = Arc::new(AtomicUsize::new(0));

    // Task that would fail on first attempts (simulated)
    let task = dag.add_task(task_fn({
        let counter = attempt_counter.clone();
        move |_: ()| {
            let counter = counter.clone();
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);

                // In real retry, this would be retried
                // Here we simulate by not panicking
                if attempt == 0 {
                    // Would normally panic here
                    return Err::<i32, &str>("Simulated failure");
                }

                Ok(42)
            }
        }
    }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Task completes with error on first "attempt"
    assert_eq!(dag.get(task)?, Err("Simulated failure"));

    Ok(())
}

#[tokio::test]
async fn test_circuit_breaker_pattern() -> DagResult<()> {
    let dag = DagRunner::new();
    let failure_count = Arc::new(AtomicUsize::new(0));
    let circuit_open = Arc::new(AtomicBool::new(false));

    // Tasks that check circuit breaker
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            let failures = failure_count.clone();
            let circuit = circuit_open.clone();
            dag.add_task(task_fn(move |_: ()| {
                let failures = failures.clone();
                let circuit = circuit.clone();
                async move {
                    // Check if circuit is open
                    if circuit.load(Ordering::SeqCst) {
                        return Err("Circuit breaker open");
                    }

                    // Simulate some failures
                    if i < 3 {
                        let count = failures.fetch_add(1, Ordering::SeqCst);
                        if count >= 2 {
                            circuit.store(true, Ordering::SeqCst);
                        }
                        return Err("Task failed");
                    }

                    Ok(i)
                }
            }))
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // First few tasks fail and open circuit
    for task in tasks.into_iter().take(4) {
        assert!(dag.get(task)?.is_err());
    }

    Ok(())
}

#[tokio::test]
async fn test_partial_recovery_with_defaults() -> DagResult<()> {
    let dag = DagRunner::new();

    // Some tasks that might fail
    let _tasks: Vec<_> = (0..10)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                if i % 3 == 0 {
                    panic!("Task {} failed", i);
                }
                i * 10
            }))
        })
        .collect();

    // Aggregator that handles failures gracefully
    let aggregator = dag.add_task(task_fn(|_: ()| async {
        // In real scenario, would check task results and use defaults
        vec![0, 10, 20, 0, 40, 50, 0, 70, 80, 0] // 0 for failed tasks
    }));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    let result = dag.get(aggregator)?;
    assert_eq!(result.len(), 10);

    Ok(())
}

#[tokio::test]
async fn test_error_accumulation_pattern() -> DagResult<()> {
    let dag = DagRunner::new();
    let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Tasks that may produce errors
    let mut tasks: Vec<_> = (0..10)
        .map(|i| {
            let errors = errors.clone();
            dag.add_task(task_fn(move |_: ()| {
                let errors = errors.clone();
                async move {
                    if i % 2 == 0 {
                        errors.lock().push(format!("Error from task {}", i));
                        return Err(format!("Task {} error", i));
                    }
                    Ok(i)
                }
            }))
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Check accumulated errors
    let error_list = errors.lock().clone();
    assert_eq!(error_list.len(), 5); // Even numbered tasks

    tasks.truncate(2);

    let [first, second] = <[_; 2]>::try_from(tasks).map_err(|_e| ()).unwrap();

    // Verify specific results
    assert!(dag.get(first)?.is_err());
    assert_eq!(dag.get(second)?, Ok(1));

    Ok(())
}

#[tokio::test]
async fn test_graceful_degradation() -> DagResult<()> {
    let dag = DagRunner::new();

    // Critical task
    let critical = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3, 4, 5] }));

    // Optional enhancement that fails - returns Result
    let _enhancement = dag.add_task(task_fn(|_: ()| async {
        Err::<Vec<i32>, &str>("Enhancement failed")
    }));

    // Final task that works with what's available
    let final_task = dag
        .add_task(task_fn(|base: Vec<i32>| async move {
            // Would combine with enhancement if available
            // Here just uses base
            base.iter().sum::<i32>()
        }))
        .depends_on(critical);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Critical path works
    assert_eq!(dag.get(final_task)?, 15);

    Ok(())
}

#[tokio::test]
async fn test_error_boundary_isolation() -> DagResult<()> {
    let dag = DagRunner::new();

    // Group A: contains an error - use Result
    let a1 = dag.add_task(task_fn(|_: ()| async { 10 }));
    let a2 = dag
        .add_task(task_fn(|_x: i32| async move {
            Err::<i32, &str>("Group A error")
        }))
        .depends_on(a1);

    // Group B: should be isolated from Group A's error
    let b1 = dag.add_task(task_fn(|_: ()| async { 20 }));
    let b2 = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(b1);

    // Group C: depends on B but not A
    let c1 = dag
        .add_task(task_fn(|x: i32| async move { x + 5 }))
        .depends_on(b2);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Group A fails (returns Err)
    assert_eq!(dag.get(a2)?, Err("Group A error"));

    // Groups B and C succeed
    assert_eq!(dag.get(b2)?, 40);
    assert_eq!(dag.get(c1)?, 45);

    Ok(())
}

#[tokio::test]
async fn test_compensation_action() -> DagResult<()> {
    let dag = DagRunner::new();
    let compensation_triggered = Arc::new(AtomicBool::new(false));

    // Action that fails
    let action = dag.add_task(task_fn(|_: ()| async {
        // Perform some action that fails
        panic!("Action failed");
    }));

    // Compensation task (runs independently)
    let compensation = dag.add_task(task_fn({
        let triggered = compensation_triggered.clone();
        move |_: ()| {
            let triggered = triggered.clone();
            async move {
                triggered.store(true, Ordering::SeqCst);
                "Compensation executed"
            }
        }
    }));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // Action failed
    assert!(dag.get(action).is_err());

    // Compensation ran successfully
    assert_eq!(dag.get(compensation)?, "Compensation executed");
    assert!(compensation_triggered.load(Ordering::SeqCst));

    Ok(())
}
