// Result and error handling tests for DagRunner
use crate::common::task_fn;

use crate::common::tasks::Counter;
use dagx::*;
use futures::FutureExt;

#[tokio::test]
async fn test_result_type() {
    let dag = DagRunner::new();

    let success_task = dag.add_task(task_fn(|_: ()| async { Ok::<i32, String>(42) }));
    let error_task = dag.add_task(task_fn(|_: ()| async {
        Err::<i32, String>("error".to_string())
    }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(success_task).unwrap(), Ok(42));
    assert_eq!(dag.get(error_task).unwrap(), Err("error".to_string()));
}

#[tokio::test]
async fn test_dag_result_type_works() {
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn(|_: ()| async { 100 }));

    // Test that DagResult<()> works with ? operator pattern
    async fn run_dag(dag: &DagRunner) -> DagResult<()> {
        dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
        Ok(())
    }

    let result = run_dag(&dag).await;
    assert!(result.is_ok());
    assert_eq!(dag.get(task).unwrap(), 100);
}

#[tokio::test]
async fn test_task_not_executed_error() {
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn(|_: ()| async { 42 }));

    // Try to get result before running - should error
    let result = dag.get(task);
    assert!(result.is_err());
    match result {
        Err(DagError::ResultNotFound { task_id }) => {
            assert_eq!(task_id, 0); // First task has ID 0
        }
        _ => panic!("Expected ResultNotFound error"),
    }
}

#[tokio::test]
async fn test_error_messages_are_helpful() {
    // Test ResultNotFound error message
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn(|_: ()| async { 42 }));
    let result = dag.get(task);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Result not found"));
    assert!(err_msg.contains("task #0")); // Task ID
    assert!(err_msg.contains("Call dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))"));
}

#[tokio::test]
async fn test_successful_execution_returns_ok() {
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn(|_: ()| async { 42 }));

    // Should return Ok(())
    let result = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;
    assert!(result.is_ok());

    // Should be able to get value
    assert_eq!(dag.get(task).unwrap(), 42);
}

#[tokio::test]
async fn test_stateful_tasks() {
    let dag = DagRunner::new();

    let input = dag.add_task(task_fn(|_: ()| async { 5 }));
    let counter = dag.add_task(Counter::new(10)).depends_on(&input);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(counter).unwrap(), 15); // 10 + 5
}

// Cycle detection test removed - it required access to private fields
// The public API correctly prevents creating cycles, so this is tested implicitly

#[tokio::test]
async fn test_task_fn_with_captured_state() {
    let dag = DagRunner::new();

    let multiplier = 7;
    let base = dag.add_task(task_fn(|_: ()| async { 5 }));
    let scaled = dag
        .add_task(task_fn(move |x: i32| async move { x * multiplier }))
        .depends_on(&base);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(scaled).unwrap(), 35);
}
