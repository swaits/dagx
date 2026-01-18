//! Tests for dependency resolution order and transitive dependencies

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_dependency_resolution_order() -> DagResult<()> {
    // Test that dependencies are resolved in the correct order
    let dag = DagRunner::new();

    let execution_log = Arc::new(Mutex::new(Vec::new()));

    // Create a chain: A -> B -> C -> D -> E
    let a = {
        let log = execution_log.clone();
        dag.add_task(task_fn(move |_: ()| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push(("A_start", 0));
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                log.lock().unwrap().push(("A_end", 1));
                1
            }
        }))
    };

    let b = {
        let log = execution_log.clone();
        dag.add_task(task_fn(move |x: i32| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push(("B_start", x));
                assert_eq!(x, 1, "B should receive 1 from A");
                log.lock().unwrap().push(("B_end", x + 1));
                x + 1
            }
        }))
        .depends_on(&a)
    };

    let c = {
        let log = execution_log.clone();
        dag.add_task(task_fn(move |x: i32| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push(("C_start", x));
                assert_eq!(x, 2, "C should receive 2 from B");
                log.lock().unwrap().push(("C_end", x + 1));
                x + 1
            }
        }))
        .depends_on(b)
    };

    let d = {
        let log = execution_log.clone();
        dag.add_task(task_fn(move |x: i32| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push(("D_start", x));
                assert_eq!(x, 3, "D should receive 3 from C");
                log.lock().unwrap().push(("D_end", x + 1));
                x + 1
            }
        }))
        .depends_on(c)
    };

    let e = {
        let log = execution_log.clone();
        dag.add_task(task_fn(move |x: i32| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push(("E_start", x));
                assert_eq!(x, 4, "E should receive 4 from D");
                log.lock().unwrap().push(("E_end", x + 1));
                x + 1
            }
        }))
        .depends_on(d)
    };

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(e)?, 5);

    // Verify execution order
    let log = execution_log.lock().unwrap();
    let order: Vec<String> = log.iter().map(|(name, _)| name.to_string()).collect();

    // Each task should start after the previous one ends
    let a_end_pos = order.iter().position(|x| x == "A_end").unwrap();
    let b_start_pos = order.iter().position(|x| x == "B_start").unwrap();
    assert!(b_start_pos > a_end_pos, "B should start after A ends");

    let b_end_pos = order.iter().position(|x| x == "B_end").unwrap();
    let c_start_pos = order.iter().position(|x| x == "C_start").unwrap();
    assert!(c_start_pos > b_end_pos, "C should start after B ends");

    Ok(())
}

#[tokio::test]
async fn test_transitive_dependencies() -> DagResult<()> {
    // Test that transitive dependencies work correctly
    // A -> B -> C (creates transitive A -> C relationship)
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 10 }));
    let b = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(&a);
    let c = dag
        .add_task(task_fn(|x: i32| async move { x + 5 }))
        .depends_on(b);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(&a)?, 10);
    assert_eq!(dag.get(b)?, 20);
    assert_eq!(dag.get(c)?, 25); // 20 + 5

    Ok(())
}

#[tokio::test]
async fn test_deep_chain_50_levels() -> DagResult<()> {
    let dag = DagRunner::new();

    let first = dag.add_task(task_fn(|_: ()| async { 0 }));
    let mut handles = vec![(&first).into()];

    for _i in 1..=50 {
        let next = dag
            .add_task(task_fn(move |x: i32| async move { x + 1 }))
            .depends_on(handles.last().unwrap());
        handles.push(next);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(handles.last().unwrap())?, 50);
    Ok(())
}

#[tokio::test]
async fn test_deep_chain_200_levels() -> DagResult<()> {
    let dag = DagRunner::new();

    let first = dag.add_task(task_fn(|_: ()| async { 1 }));
    let mut current = (&first).into();

    for _ in 0..200 {
        current = dag
            .add_task(task_fn(|x: i32| async move { x + 1 }))
            .depends_on(current);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(current)?, 201);
    Ok(())
}
