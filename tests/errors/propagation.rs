//! Tests for error propagation through the DAG

use dagx::{task, DagError, DagRunner};
use dagx_test::task_fn;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_linear_error_propagation() {
    let mut dag = DagRunner::new();

    let t1 = dag.add_task(task_fn::<(), _, _>(|_: ()| 1));
    let t2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| -> i32 {
            panic!("Error at t2 with input {}", x);
        }))
        .depends_on(t1);
    let t3 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
        .depends_on(t2);
    dag.add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(t3);

    assert!(matches!(
        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await,
        Err(DagError::TaskPanicked { .. })
    ));
}

#[tokio::test]
async fn test_diamond_error_propagation() {
    let mut dag = DagRunner::new();

    //     source
    //     /    \
    //   left   right (fails)
    //     \    /
    //      sink

    let source = dag.add_task(task_fn::<(), _, _>(|_: ()| 100));

    let left = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x / 2))
        .depends_on(source);

    let right = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| {
            panic!("Right path fails with {}", x);
        }))
        .depends_on(source);

    dag.add_task(task_fn::<(i32, i32), _, _>(|(l, r): (&i32, &i32)| l + r))
        .depends_on((&left, &right));

    assert!(matches!(
        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await,
        Err(DagError::TaskPanicked { .. })
    ));
}

#[tokio::test]
async fn test_error_in_wide_fanout() {
    let mut dag = DagRunner::new();

    let source = dag.add_task(task_fn::<(), _, _>(|_: ()| -> i32 {
        panic!("Source fails");
    }));

    // Create wide fanout from failing source
    for i in 0..20 {
        dag.add_task(task_fn::<i32, _, _>(move |&x: &i32| x + i))
            .depends_on(source);
    }

    assert!(matches!(
        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await,
        Err(DagError::TaskPanicked { .. })
    ));
}

#[tokio::test]
async fn test_selective_error_propagation() {
    let mut dag = DagRunner::new();

    // Source that produces a Result
    let source = dag.add_task(task_fn::<(), _, _>(|_: ()| {
        Result::<i32, String>::Err("Source error".to_string())
    }));

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

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Source returns an Err variant (not a panic)
    assert_eq!(output.get(source).unwrap(), Err("Source error".to_string()));

    // Handler processes it
    assert_eq!(output.get(handler).unwrap(), -1);

    // Final task handles the error case
    assert_eq!(output.get(final_task).unwrap(), "Handled error case");
}

#[tokio::test]
async fn test_error_propagation_timing() {
    let mut dag = DagRunner::new();
    let propagation_order = Arc::new(Mutex::new(Vec::new()));

    struct T1Task {
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    #[task]
    impl T1Task {
        async fn run(&self) -> i32 {
            sleep(Duration::from_millis(10)).await;
            self.order.lock().unwrap().push("t1_complete");
            1
        }
    }

    let t1 = dag.add_task(T1Task {
        order: propagation_order.clone(),
    });

    struct T2Task {
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    #[task]
    impl T2Task {
        async fn run(&self, x: &i32) -> i32 {
            self.order.lock().unwrap().push("t2_start");
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
                order.lock().unwrap().push("t3_should_not_run");
                x + 1
            }
        }))
        .depends_on(t2);

    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    let order = propagation_order.lock().unwrap().clone();

    // t1 completes, t2 starts and fails, t3 never runs
    assert!(order.contains(&"t1_complete"));
    assert!(order.contains(&"t2_start"));
    assert!(!order.contains(&"t3_should_not_run"));
}
