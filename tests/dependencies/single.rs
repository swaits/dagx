//! Tests for single dependencies and basic dependency data flow

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};


#[tokio::test]
async fn test_single_dependency() -> DagResult<()> {
    let dag = DagRunner::new();

    let source = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));
    let dependent = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
        .depends_on(source);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;

    assert_eq!(dag.get(dependent)?, 43);
    Ok(())
}

#[tokio::test]
async fn test_dependency_data_flow() -> DagResult<()> {
    // Test that data flows correctly through dependencies
    let dag = DagRunner::new();

    let source: TaskHandle<_> = dag
        .add_task(task_fn::<(), _, _>(|_: ()| vec![1, 2, 3, 4, 5]))
        .into();

    let sum = dag
        .add_task(task_fn::<Vec<_>, _, _>(|v: &Vec<i32>| {
            v.iter().sum::<i32>()
        }))
        .depends_on(source);

    let product = dag
        .add_task(task_fn::<Vec<_>, _, _>(|v: &Vec<i32>| {
            v.iter().product::<i32>()
        }))
        .depends_on(source);

    let final_result = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(s, p): (&i32, &i32)| s + p))
        .depends_on((&sum, &product));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;

    // Note: sum and product are not sinks (final_result depends on them)
    // Only final_result is a sink and can be retrieved
    assert_eq!(dag.get(final_result)?, 135); // 15+120

    Ok(())
}

#[tokio::test]
async fn test_dependencies_with_different_types() -> DagResult<()> {
    // Test that dependencies with different types work correctly
    let dag = DagRunner::new();

    let int_source = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));
    let string_source = dag.add_task(task_fn::<(), _, _>(|_: ()| "hello".to_string()));
    let bool_source = dag.add_task(task_fn::<(), _, _>(|_: ()| true));

    let combined = dag
        .add_task(task_fn::<(i32, String, bool), _, _>(
            |(i, s, b): (&i32, &String, &bool)| format!("{}: {} ({})", s, i, b),
        ))
        .depends_on((int_source, string_source, bool_source));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;

    assert_eq!(dag.get(combined)?, "hello: 42 (true)");
    Ok(())
}

#[tokio::test]
async fn test_shared_dependencies() -> DagResult<()> {
    // Test that a single node can be a dependency for multiple downstream nodes
    let dag = DagRunner::new();

    let shared: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 42)).into();

    // Create 10 tasks that all depend on the shared node
    let dependents: Vec<_> = (0..10)
        .map(|i| {
            dag.add_task(task_fn::<i32, _, _>(move |&x: &i32| x * (i + 1)))
                .depends_on(shared)
        })
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;

    // Verify all dependents got the correct value from shared
    for (i, handle) in dependents.iter().enumerate() {
        assert_eq!(dag.get(handle)?, 42 * (i as i32 + 1));
    }

    Ok(())
}

#[tokio::test]
async fn test_multiple_source_nodes() -> DagResult<()> {
    // Test DAG with multiple source nodes (no dependencies)
    let dag = DagRunner::new();

    let sources: Vec<_> = (0..20)
        .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 10)))
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await?;

    for (i, source) in sources.into_iter().enumerate() {
        assert_eq!(dag.get(source)?, i as i32 * 10);
    }

    Ok(())
}
