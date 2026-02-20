//! Tests for error recovery patterns

use dagx::{DagResult, DagRunner};
use dagx_test::task_fn;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_retry_pattern_simulation() -> DagResult<()> {
    let mut dag = DagRunner::new();
    let attempt_counter = Arc::new(AtomicUsize::new(0));

    // Task that would fail on first attempts (simulated)
    let task = dag.add_task(task_fn::<(), _, _>({
        let counter = attempt_counter.clone();
        move |_: ()| {
            let counter = counter.clone();
            let attempt = counter.fetch_add(1, Ordering::SeqCst);

            // In real retry, this would be retried
            // Here we simulate by not panicking
            if attempt == 0 {
                // Would normally panic here
                return Err::<i32, &str>("Simulated failure");
            }

            Ok(42)
        }
    }));

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Task completes with error on first "attempt"
    assert_eq!(output.get(task), Err("Simulated failure"));

    Ok(())
}

#[tokio::test]
async fn test_circuit_breaker_pattern() -> DagResult<()> {
    let mut dag = DagRunner::new();
    let failure_count = Arc::new(AtomicUsize::new(0));
    let circuit_open = Arc::new(AtomicBool::new(false));

    // Tasks that check circuit breaker
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            let failures = failure_count.clone();
            let circuit = circuit_open.clone();
            dag.add_task(task_fn::<(), _, _>(move |_: ()| {
                let failures = failures.clone();
                let circuit = circuit.clone();
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
            }))
        })
        .collect();

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // First few tasks fail and open circuit
    for task in tasks.into_iter().take(4) {
        assert!(output.get(task).is_err());
    }

    Ok(())
}

#[tokio::test]
async fn test_error_accumulation_pattern() -> DagResult<()> {
    let mut dag = DagRunner::new();
    let errors = Arc::new(Mutex::new(Vec::new()));

    // Tasks that may produce errors
    let mut tasks: Vec<_> = (0..10)
        .map(|i| {
            let errors = errors.clone();
            dag.add_task(task_fn::<(), _, _>(move |_: ()| {
                let errors = errors.clone();
                if i % 2 == 0 {
                    errors
                        .lock()
                        .unwrap()
                        .push(format!("Error from task {}", i));
                    return Err(format!("Task {} error", i));
                }
                Ok(i)
            }))
        })
        .collect();

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Check accumulated errors
    let error_list = errors.lock().unwrap().clone();
    assert_eq!(error_list.len(), 5); // Even numbered tasks

    tasks.truncate(2);

    let [first, second] = <[_; 2]>::try_from(tasks).map_err(|_e| ()).unwrap();

    // Verify specific results
    assert!(output.get(first).is_err());
    assert_eq!(output.get(second), Ok(1));

    Ok(())
}

#[tokio::test]
async fn test_error_boundary_isolation() -> DagResult<()> {
    let mut dag = DagRunner::new();

    // Group A: contains an error - use Result
    let a1 = dag.add_task(task_fn::<(), _, _>(|_: ()| 10));
    let a2 = dag
        .add_task(task_fn::<i32, _, _>(|_x: &i32| {
            Err::<i32, &str>("Group A error")
        }))
        .depends_on(&a1);

    // Group B: should be isolated from Group A's error
    let b1 = dag.add_task(task_fn::<(), _, _>(|_: ()| 20));
    let b2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(&b1);

    // Group C: depends on B but not A
    let c1 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(&b2);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Group A fails (returns Err)
    assert_eq!(output.get(a2), Err("Group A error"));

    // Groups B and C succeed
    assert_eq!(output.get(b2), 40);
    assert_eq!(output.get(c1), 45);

    Ok(())
}
