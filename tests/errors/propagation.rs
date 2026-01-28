//! Tests for error propagation through the DAG

use crate::common::task_fn;
use dagx::DagRunner;
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_linear_error_propagation() {
    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { 1 }));
    let t2 = dag
        .add_task(task_fn(|x: i32| async move {
            panic!("Error at t2 with input {}", x);
        }))
        .depends_on(&t1);
    let t3 = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(t2);
    let t4 = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(t3);

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // t1 succeeds
    assert_eq!(dag.get(&t1).unwrap(), 1);

    // t2 fails
    assert!(dag.get(t2).is_err());

    // t3 and t4 fail due to dependency
    assert!(dag.get(t3).is_err());
    assert!(dag.get(t4).is_err());
}

#[tokio::test]
async fn test_diamond_error_propagation() {
    let dag = DagRunner::new();

    //     source
    //     /    \
    //   left   right (fails)
    //     \    /
    //      sink

    let source = dag.add_task(task_fn(|_: ()| async { 100 }));

    let left = dag
        .add_task(task_fn(|x: i32| async move { x / 2 }))
        .depends_on(&source);

    let right = dag
        .add_task(task_fn(|x: i32| async move {
            panic!("Right path fails with {}", x);
        }))
        .depends_on(&source);

    let sink = dag
        .add_task(task_fn(|(l, r): (i32, i32)| async move { l + r }))
        .depends_on((&left, &right));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    assert_eq!(dag.get(&source).unwrap(), 100);
    assert_eq!(dag.get(left).unwrap(), 50);
    assert!(dag.get(right).is_err());
    assert!(dag.get(sink).is_err()); // Fails due to right dependency
}

#[tokio::test]
async fn test_error_stops_at_boundary() {
    let dag = DagRunner::new();
    let execution_tracker = Arc::new(AtomicUsize::new(0));

    // Branch with error
    let error_source = dag.add_task(task_fn({
        let tracker = execution_tracker.clone();
        move |_: ()| {
            let tracker = tracker.clone();
            async move {
                tracker.fetch_add(1, Ordering::SeqCst);
                panic!("Error source");
            }
        }
    }));

    let error_dependent = dag
        .add_task(task_fn({
            let tracker = execution_tracker.clone();
            move |_: i32| {
                let tracker = tracker.clone();
                async move {
                    tracker.fetch_add(100, Ordering::SeqCst); // Should not execute
                    42
                }
            }
        }))
        .depends_on(&error_source);

    // Independent branch
    let independent = dag.add_task(task_fn({
        let tracker = execution_tracker.clone();
        move |_: ()| {
            let tracker = tracker.clone();
            async move {
                tracker.fetch_add(10, Ordering::SeqCst);
                10
            }
        }
    }));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // Error branch: source runs but dependent doesn't
    assert!(dag.get(&error_source).is_err());
    assert!(dag.get(error_dependent).is_err());

    // Independent branch runs
    assert_eq!(dag.get(&independent).unwrap(), 10);

    // Check execution: 1 (error_source) + 10 (independent) = 11
    assert_eq!(execution_tracker.load(Ordering::SeqCst), 11);
}

#[tokio::test]
async fn test_multiple_error_sources_convergence() {
    let dag = DagRunner::new();

    // Multiple error sources
    let error1 = dag.add_task(task_fn(|_: ()| async {
        panic!("Error source 1");
    }));

    let error2 = dag.add_task(task_fn(|_: ()| async {
        panic!("Error source 2");
    }));

    let success = dag.add_task(task_fn(|_: ()| async { 42 }));

    // Convergence point depends on all three
    let convergence = dag
        .add_task(task_fn(|(_, _, _): (i32, i32, i32)| async { 100 }))
        .depends_on((&error1, &error2, &success));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // All error sources fail
    assert!(dag.get(&error1).is_err());
    assert!(dag.get(&error2).is_err());

    // Success task works
    assert_eq!(dag.get(&success).unwrap(), 42);

    // Convergence fails due to any error dependency
    assert!(dag.get(convergence).is_err());
}

#[tokio::test]
async fn test_error_in_wide_fanout() {
    let dag = DagRunner::new();

    let source = dag.add_task(task_fn(|_: ()| async {
        panic!("Source fails");
    }));

    // Create wide fanout from failing source
    let dependents: Vec<_> = (0..20)
        .map(|i| {
            dag.add_task(task_fn(move |x: i32| async move { x + i }))
                .depends_on(&source)
        })
        .collect();

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // Source fails
    assert!(dag.get(&source).is_err());

    // All dependents fail
    for dependent in &dependents {
        assert!(dag.get(dependent).is_err());
    }
}

#[tokio::test]
async fn test_selective_error_propagation() {
    let dag = DagRunner::new();

    // Source that produces a Result
    let source = dag.add_task(task_fn(|_: ()| async {
        Result::<i32, String>::Err("Source error".to_string())
    }));

    // Handler that processes the Result
    let handler = dag
        .add_task(task_fn(|result: Result<i32, String>| async move {
            match result {
                Ok(val) => val * 2,
                Err(_) => -1, // Default value on error
            }
        }))
        .depends_on(&source);

    // Further processing
    let final_task = dag
        .add_task(task_fn(|val: i32| async move {
            if val < 0 {
                "Handled error case".to_string()
            } else {
                format!("Success: {}", val)
            }
        }))
        .depends_on(handler);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Source returns an Err variant (not a panic)
    assert_eq!(dag.get(&source).unwrap(), Err("Source error".to_string()));

    // Handler processes it
    assert_eq!(dag.get(handler).unwrap(), -1);

    // Final task handles the error case
    assert_eq!(dag.get(final_task).unwrap(), "Handled error case");
}

#[tokio::test]
async fn test_error_propagation_timing() {
    let dag = DagRunner::new();
    let propagation_order = Arc::new(parking_lot::Mutex::new(Vec::new()));

    let t1 = dag.add_task(task_fn({
        let order = propagation_order.clone();
        move |_: ()| {
            let order = order.clone();
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                order.lock().push("t1_complete");
                1
            }
        }
    }));

    let t2 = dag
        .add_task(task_fn({
            let order = propagation_order.clone();
            move |x: i32| {
                let order = order.clone();
                async move {
                    order.lock().push("t2_start");
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    panic!("t2 fails with {}", x);
                }
            }
        }))
        .depends_on(&t1);

    let _t3 = dag
        .add_task(task_fn({
            let order = propagation_order.clone();
            move |x: i32| {
                let order = order.clone();
                async move {
                    order.lock().push("t3_should_not_run");
                    x + 1
                }
            }
        }))
        .depends_on(&t2);

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    let order = propagation_order.lock().clone();

    // t1 completes, t2 starts and fails, t3 never runs
    assert!(order.contains(&"t1_complete"));
    assert!(order.contains(&"t2_start"));
    assert!(!order.contains(&"t3_should_not_run"));
}

#[tokio::test]
async fn test_partial_branch_failure_propagation() {
    let dag = DagRunner::new();

    //        root
    //      /   |   \
    //    a1   b1   c1
    //     |    |    |
    //    a2   b2   c2
    //     |  (err)  |
    //    a3   b3   c3
    //      \  |   /
    //        sink

    let root = dag.add_task(task_fn(|_: ()| async { 100 }));

    // Branch A
    let a1 = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(&root);
    let a2 = dag
        .add_task(task_fn(|x: i32| async move { x + 2 }))
        .depends_on(a1);
    let a3 = dag
        .add_task(task_fn(|x: i32| async move { x + 3 }))
        .depends_on(a2);

    // Branch B (with error) - use Result instead of panic
    let b1 = dag
        .add_task(task_fn(|x: i32| async move { Ok(x * 2) }))
        .depends_on(&root);
    let b2 = dag
        .add_task(task_fn(|_x: Result<i32, &str>| async move {
            Err("Branch B fails at b2")
        }))
        .depends_on(&b1);
    let b3 = dag
        .add_task(task_fn(
            |x: Result<i32, &str>| async move { x.map(|v| v * 3) },
        ))
        .depends_on(&b2);

    // Branch C
    let c1 = dag
        .add_task(task_fn(|x: i32| async move { x - 1 }))
        .depends_on(&root);
    let c2 = dag
        .add_task(task_fn(|x: i32| async move { x - 2 }))
        .depends_on(c1);
    let c3 = dag
        .add_task(task_fn(|x: i32| async move { x - 3 }))
        .depends_on(c2);

    // Sink depends on all branches - handle Result from B
    let sink = dag
        .add_task(task_fn(
            |(a, b, c): (i32, Result<i32, &str>, i32)| async move {
                match b {
                    Ok(b_val) => Ok(a + b_val + c),
                    Err(e) => Err(e),
                }
            },
        ))
        .depends_on((&a3, &b3, &c3));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .ok();

    // Branch A succeeds
    assert_eq!(dag.get(a3).unwrap(), 106);

    // Branch B fails at b2
    assert_eq!(dag.get(b1).unwrap(), Ok(200));
    assert_eq!(dag.get(b2).unwrap(), Err("Branch B fails at b2"));
    assert_eq!(dag.get(b3).unwrap(), Err("Branch B fails at b2")); // Propagated error

    // Branch C succeeds
    assert_eq!(dag.get(c3).unwrap(), 94);

    // Sink fails due to branch B
    assert_eq!(dag.get(sink).unwrap(), Err("Branch B fails at b2"));
}
