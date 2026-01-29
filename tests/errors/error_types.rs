//! Tests for different error types and error handling

use crate::common::task_fn;
use dagx::{DagError, DagRunner, TaskHandle};
use futures::FutureExt;

#[tokio::test]
async fn test_result_not_found_error() {
    let dag = DagRunner::new();

    let t1: TaskHandle<_> = dag.add_task(task_fn(|_: ()| async { 42 })).into();

    // Try to get result before running
    let result = dag.get(t1);
    assert!(matches!(result, Err(DagError::ResultNotFound { .. })));

    // Run and verify it works after
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(t1).unwrap(), 42);
}

#[tokio::test]
async fn test_nested_result_error_handling() {
    let dag = DagRunner::new();

    // Task that returns a Result
    let task1 = dag.add_task(task_fn(|_: ()| async {
        Result::<i32, String>::Err("Internal error".to_string())
    }));

    let task2 = dag.add_task(task_fn(|_: ()| async { Result::<i32, String>::Ok(42) }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Both tasks complete successfully (they return Results)
    assert_eq!(dag.get(task1).unwrap(), Err("Internal error".to_string()));
    assert_eq!(dag.get(task2).unwrap(), Ok(42));
}

#[tokio::test]
async fn test_option_none_not_error() {
    let dag = DagRunner::new();

    let task = dag.add_task(task_fn(|_: ()| async { None::<i32> }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // None is a valid result, not an error
    assert_eq!(dag.get(task).unwrap(), None);
}

#[tokio::test]
async fn test_custom_error_types() {
    #[derive(Debug, Clone, PartialEq)]
    enum CustomError {
        ValidationError(String),
        ProcessingError { code: i32, details: String },
    }

    let dag = DagRunner::new();

    let task1 = dag.add_task(task_fn(|_: ()| async {
        Result::<i32, CustomError>::Err(CustomError::ValidationError("Invalid input".to_string()))
    }));

    let task2 = dag.add_task(task_fn(|_: ()| async {
        Result::<i32, CustomError>::Err(CustomError::ProcessingError {
            code: 500,
            details: "Processing failed".to_string(),
        })
    }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Check custom errors are preserved
    match dag.get(task1).unwrap() {
        Err(CustomError::ValidationError(msg)) => assert_eq!(msg, "Invalid input"),
        _ => panic!("Wrong error type"),
    }

    match dag.get(task2).unwrap() {
        Err(CustomError::ProcessingError { code, .. }) => assert_eq!(code, 500),
        _ => panic!("Wrong error type"),
    }
}

#[tokio::test]
async fn test_error_in_tuple_dependency() {
    let dag = DagRunner::new();

    let ok_task = dag.add_task(task_fn(|_: ()| async { 10 }));
    let err_task = dag.add_task(task_fn(|_: ()| async {
        panic!("Error in dependency");
    }));

    let dependent = dag
        .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
        .depends_on((ok_task, err_task));

    let result = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    assert!(result.is_err());
    assert!(dag.get(dependent).is_err());
}

#[tokio::test]
async fn test_string_panic_vs_structured_panic() {
    let dag = DagRunner::new();

    let string_panic = dag.add_task(task_fn(|_: ()| async {
        panic!("String panic message");
    }));

    let formatted_panic = dag.add_task(task_fn(|_: ()| async {
        panic!("Formatted panic: value={}, code={}", 42, "ERROR");
    }));

    let _ = dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await;

    // Both should result in errors
    assert!(dag.get(string_panic).is_err());
    assert!(dag.get(formatted_panic).is_err());
}

#[tokio::test]
async fn test_anyhow_like_error_chain() {
    // Test error chains with simple string-based errors
    let dag = DagRunner::new();

    let task = dag.add_task(task_fn(|_: ()| async {
        // Simulate an error chain with nested Results
        let _inner: Result<i32, String> = Err("Inner error".to_string());
        let outer: Result<Result<i32, String>, String> = Err("Outer error".to_string());

        outer
    }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Get the chained error
    let result = dag.get(task).unwrap();
    assert!(result.is_err());

    if let Err(e) = result {
        assert_eq!(e, "Outer error");
    }
}
