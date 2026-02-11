//! Stress tests and benchmarks

use crate::common::task_fn;
use dagx::DagRunner;

use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_deep_chain_100_levels() {
    let dag = Arc::new(DagRunner::new());

    // Create a chain 100 levels deep
    let start = Instant::now();
    let first = dag.add_task(task_fn::<(), _, _>(|_: ()| 0));
    let mut handles = vec![first.into()];

    for _ in 1..=100 {
        let next = dag
            .add_task(task_fn::<i32, _, _>(move |&x: &i32| x + 1))
            .depends_on(handles.last().unwrap());
        handles.push(next);
    }
    let build_time = start.elapsed();

    println!("Built 100-level chain in {:?}", build_time);

    // Execute
    let start = Instant::now();
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();
    let exec_time = start.elapsed();

    println!("Executed 100-level chain in {:?}", exec_time);
    assert!(
        exec_time < Duration::from_secs(5),
        "Execution took too long"
    );

    // Verify result
    assert_eq!(dag.get(handles.last().unwrap()).unwrap(), 100);
}

#[tokio::test]
async fn test_stress_mixed_patterns() {
    // Stress test with mixed DAG patterns
    let dag = Arc::new(DagRunner::new());
    let mut all_tasks = vec![];

    // Add various patterns
    for i in 0..10 {
        // Linear chains
        let first = dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 1000));
        let mut chain_handles = vec![first.into()];
        for j in 1..10 {
            let next = dag
                .add_task(task_fn::<i32, _, _>(move |&x: &i32| x + j))
                .depends_on(chain_handles.last().unwrap());
            chain_handles.push(next);
        }
        all_tasks.push(*chain_handles.last().unwrap());

        // Diamonds
        let source = dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 100));
        let source_handle: dagx::TaskHandle<i32> = source.into();
        let left = dag
            .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
            .depends_on(source_handle);
        let right = dag
            .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 3))
            .depends_on(source_handle);
        let merge = dag
            .add_task(task_fn::<(i32, i32), _, _>(|(l, r): (&i32, &i32)| l + r))
            .depends_on((&left, &right));
        all_tasks.push(merge);

        // Fan-outs
        let hub = dag.add_task(task_fn::<(), _, _>(move |_: ()| i));
        let hub_handle: dagx::TaskHandle<i32> = hub.into();
        for j in 0..5 {
            let fan = dag
                .add_task(task_fn::<i32, _, _>(move |&x: &i32| x + j * 10))
                .depends_on(hub_handle);
            all_tasks.push(fan);
        }
    }

    // Execute
    let start = Instant::now();
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();
    let exec_time = start.elapsed();

    println!("Executed mixed pattern stress test in {:?}", exec_time);

    // Verify some results
    for task in all_tasks.iter().take(10) {
        assert!(dag.get(task).is_ok());
    }
}
