//! Tests for error propagation through the DAG

use dagx::task_fn;
use dagx::{task, DagRunner, TaskHandle};

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_linear_error_propagation() {
    let dag = DagRunner::new();

    let t1: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 1)).into();
    let t2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| -> i32 {
            panic!("Error at t2 with input {}", x);
        }))
        .depends_on(t1);
    let t3 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
        .depends_on(t2);
    let t4 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(t3);

    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    // t1 succeeds
    assert_eq!(dag.get(t1).unwrap(), 1);

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

    let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 100)).into();

    let left = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x / 2))
        .depends_on(source);

    let right = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| {
            panic!("Right path fails with {}", x);
        }))
        .depends_on(source);

    let sink = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(l, r): (&i32, &i32)| l + r))
        .depends_on((&left, &right));

    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    assert_eq!(dag.get(source).unwrap(), 100);
    assert_eq!(dag.get(left).unwrap(), 50);
    assert!(dag.get(right).is_err());
    assert!(dag.get(sink).is_err()); // Fails due to right dependency
}

#[tokio::test]
async fn test_error_in_wide_fanout() {
    let dag = DagRunner::new();

    let source: TaskHandle<_> = dag
        .add_task(task_fn::<(), _, _>(|_: ()| -> i32 {
            panic!("Source fails");
        }))
        .into();

    // Create wide fanout from failing source
    let dependents: Vec<_> = (0..20)
        .map(|i| {
            dag.add_task(task_fn::<i32, _, _>(move |&x: &i32| x + i))
                .depends_on(source)
        })
        .collect();

    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    // Source fails
    assert!(dag.get(source).is_err());

    // All dependents fail
    for dependent in &dependents {
        assert!(dag.get(dependent).is_err());
    }
}

#[tokio::test]
async fn test_selective_error_propagation() {
    let dag = DagRunner::new();

    // Source that produces a Result
    let source: TaskHandle<_> = dag
        .add_task(task_fn::<(), _, _>(|_: ()| {
            Result::<i32, String>::Err("Source error".to_string())
        }))
        .into();

    // Handler that processes the Result
    let handler = dag
        .add_task(task_fn::<Result<_, _>, _, _>(
            |result: &Result<i32, String>| {
                match result {
                    Ok(val) => val * 2,
                    Err(_) => -1, // Default value on error
                }
            },
        ))
        .depends_on(source);

    // Further processing
    let final_task = dag
        .add_task(task_fn::<i32, _, _>(|&val: &i32| {
            if val < 0 {
                "Handled error case".to_string()
            } else {
                format!("Success: {}", val)
            }
        }))
        .depends_on(handler);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Source returns an Err variant (not a panic)
    assert_eq!(dag.get(source).unwrap(), Err("Source error".to_string()));

    // Handler processes it
    assert_eq!(dag.get(handler).unwrap(), -1);

    // Final task handles the error case
    assert_eq!(dag.get(final_task).unwrap(), "Handled error case");
}

#[tokio::test]
async fn test_error_propagation_timing() {
    let dag = DagRunner::new();
    let propagation_order = Arc::new(parking_lot::Mutex::new(Vec::new()));

    struct T1Task {
        order: Arc<parking_lot::Mutex<Vec<&'static str>>>,
    }

    #[task]
    impl T1Task {
        async fn run(&self) -> i32 {
            sleep(Duration::from_millis(10)).await;
            self.order.lock().push("t1_complete");
            1
        }
    }

    let t1 = dag.add_task(T1Task {
        order: propagation_order.clone(),
    });

    struct T2Task {
        order: Arc<parking_lot::Mutex<Vec<&'static str>>>,
    }

    #[task]
    impl T2Task {
        async fn run(&self, x: &i32) -> i32 {
            self.order.lock().push("t2_start");
            sleep(Duration::from_millis(10)).await;
            panic!("t2 fails with {x}");
        }
    }

    let t2 = dag
        .add_task(T2Task {
            order: propagation_order.clone(),
        })
        .depends_on(t1);

    let _t3 = dag
        .add_task(task_fn::<i32, _, _>({
            let order = propagation_order.clone();
            move |x: &i32| {
                let order = order.clone();
                order.lock().push("t3_should_not_run");
                x + 1
            }
        }))
        .depends_on(t2);

    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    let order = propagation_order.lock().clone();

    // t1 completes, t2 starts and fails, t3 never runs
    assert!(order.contains(&"t1_complete"));
    assert!(order.contains(&"t2_start"));
    assert!(!order.contains(&"t3_should_not_run"));
}
