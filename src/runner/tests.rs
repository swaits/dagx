//! Unit tests for runner module

use crate::error::DagError;
use crate::runner::DagRunner;
use crate::task::Task;
use crate::types::TaskHandle;

struct TestTask {
    value: i32,
}

#[crate::task]
impl TestTask {
    async fn run(&self) -> i32 {
        self.value
    }
}

#[test]
fn test_dag_runner_new() {
    // Test that DagRunner::new() creates a new instance
    let dag = DagRunner::new();

    // Initial state should be empty
    assert_eq!(dag.nodes.lock().len(), 0);
    assert_eq!(dag.edges.lock().len(), 0);
    assert_eq!(dag.dependents.lock().len(), 0);

    // Adding a task works
    dag.add_task(TestTask { value: 42 });
    assert_eq!(dag.nodes.lock().len(), 1);
}

#[test]
fn test_dag_runner_default() {
    // Test lines 105-106 in runner.rs - Default implementation
    let dag = DagRunner::default();

    // Should behave the same as new()
    assert_eq!(dag.nodes.lock().len(), 0);
    assert_eq!(dag.edges.lock().len(), 0);
    assert_eq!(dag.dependents.lock().len(), 0);
}

#[tokio::test]
async fn test_get_wrong_type() {
    // Test getting with wrong type - this should return TypeMismatch
    let dag = DagRunner::new();
    let handle = dag.add_task(TestTask { value: 42 });

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get with wrong type - downcast will fail
    let fake_handle: TaskHandle<String> = TaskHandle {
        id: handle.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(fake_handle);
    assert!(result.is_err());

    // When downcast fails, we get TypeMismatch
    match result.unwrap_err() {
        DagError::TypeMismatch { expected, .. } => {
            assert_eq!(expected, std::any::type_name::<String>());
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[tokio::test]
async fn test_get_result_not_found() {
    // Test getting a result before running the DAG
    let dag = DagRunner::new();
    let handle = dag.add_task(TestTask { value: 42 });
    let handle_id = handle.id.0; // Save ID before moving handle

    let result = dag.get(handle);
    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::ResultNotFound { task_id } => {
            assert_eq!(task_id, handle_id);
        }
        _ => panic!("Expected ResultNotFound error"),
    }
}

#[tokio::test]
async fn test_concurrent_run_protection() {
    // Test that run_lock prevents concurrent runs
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;

    let dag = DagRunner::new();

    // Add a task that takes some time
    struct SlowTask;
    #[crate::task]
    impl SlowTask {
        async fn run(&self) -> i32 {
            sleep(Duration::from_millis(100)).await;
            42
        }
    }

    dag.add_task(SlowTask);

    // Wrap in Arc for sharing between tasks
    let dag = Arc::new(dag);
    let dag1 = Arc::clone(&dag);
    let dag2 = Arc::clone(&dag);

    // Start two runs concurrently
    let handle1 = tokio::spawn(async move {
        dag1.run(|fut| {
            tokio::spawn(fut);
        })
        .await
    });

    let handle2 = tokio::spawn(async move {
        // Give the first run a head start
        sleep(Duration::from_millis(10)).await;
        dag2.run(|fut| {
            tokio::spawn(fut);
        })
        .await
    });

    // First should succeed, second should fail (concurrent execution not supported)
    let result1 = handle1.await.unwrap();
    let result2 = handle2.await.unwrap();

    assert!(result1.is_ok());
    assert!(result2.is_err()); // Concurrent run() returns error
}

#[test]
fn test_add_task_increments_id() {
    let dag = DagRunner::new();

    let handle1 = dag.add_task(TestTask { value: 1 });
    let handle2 = dag.add_task(TestTask { value: 2 });
    let handle3 = dag.add_task(TestTask { value: 3 });

    // Node IDs should be sequential
    assert_eq!(handle1.id.0, 0);
    assert_eq!(handle2.id.0, 1);
    assert_eq!(handle3.id.0, 2);

    // Check that nodes were actually added
    assert_eq!(dag.nodes.lock().len(), 3);
}

#[tokio::test]
async fn test_multiple_get_calls() {
    // Test that get() can be called multiple times for the same handle
    let dag = DagRunner::new();
    let handle = dag.add_task(TestTask { value: 100 });

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Multiple get calls should all return the same value
    let result1 = dag.get(&handle).unwrap();
    let result2 = dag.get(&handle).unwrap();
    let result3 = dag.get(&handle).unwrap();

    assert_eq!(result1, 100);
    assert_eq!(result2, 100);
    assert_eq!(result3, 100);
}

struct TaskWithDep;
#[crate::task]
impl TaskWithDep {
    #[allow(dead_code)]
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[tokio::test]
async fn test_invalid_node_id_in_get() {
    // Test getting with an invalid node ID
    let dag = DagRunner::new();

    // Create a handle with an ID that doesn't exist
    let invalid_handle: TaskHandle<i32> = TaskHandle {
        id: crate::types::NodeId(999),
        _phantom: std::marker::PhantomData,
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    let result = dag.get(invalid_handle);
    assert!(result.is_err());

    // With centralized output storage, we get ResultNotFound for invalid node IDs
    match result.unwrap_err() {
        DagError::ResultNotFound { task_id } => {
            assert_eq!(task_id, 999);
        }
        _ => panic!("Expected ResultNotFound error"),
    }
}
