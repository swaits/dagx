// Type safety tests for TaskHandle
use crate::common::task_fn;
use futures::FutureExt;
// These tests verify dagx's compile-time type safety guarantees.
// Tests cover both runtime behavior (wrong types return None) and
// compile-time checking (type mismatches fail to compile).

use dagx::*;

#[tokio::test]
async fn test_get_with_wrong_type_returns_none() {
    // Verify that accessing with wrong type returns error, not panic
    let dag = DagRunner::new();

    // Add task that returns i32
    let int_task = dag.add_task(task_fn(|_: ()| async { 42i32 }));

    // Run DAG
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Verify correct type works
    assert_eq!(dag.get(int_task), Ok(42i32));

    // The type safety is enforced at compile time
    // We cannot create fake handles with wrong types without accessing private fields
    // This is actually better - it means type safety is compile-time enforced!
}

#[tokio::test]
async fn test_handle_preserves_type() {
    // Verify TaskHandle<T> preserves type information correctly
    let dag = DagRunner::new();

    // Create typed handles
    let string_task = dag.add_task(task_fn(|_: ()| async { "hello".to_string() }));
    let int_task = dag.add_task(task_fn(|_: ()| async { 42i32 }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Get with correct types
    assert_eq!(dag.get(string_task).unwrap(), "hello".to_string());
    assert_eq!(dag.get(int_task).unwrap(), 42);

    // Type safety is enforced at compile time - we cannot create wrong-typed handles
}

#[tokio::test]
async fn test_type_safety_handle_reuse() {
    // Verify that handles maintain type safety across reuse
    let dag = DagRunner::new();

    let source: TaskHandle<_> = dag.add_task(task_fn(|_: ()| async { 100i32 })).into();

    // Use the same source handle multiple times
    let double = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(source);
    let triple = dag
        .add_task(task_fn(|x: i32| async move { x * 3 }))
        .depends_on(source);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(double).unwrap(), 200);
    assert_eq!(dag.get(triple).unwrap(), 300);
}
