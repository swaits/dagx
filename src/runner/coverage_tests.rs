//! Coverage tests for runner module - specifically targeting uncovered error paths
//!
//! These tests ensure we hit the type mismatch error path in get() (line 601)

use crate::error::DagError;
use crate::runner::DagRunner;
use crate::task::Task;
use crate::types::TaskHandle;
use std::collections::HashMap;

#[tokio::test]
async fn test_get_type_mismatch_error() {
    // Test the type mismatch error path in get() - line 601 in runner.rs
    // This occurs when Arc downcast fails due to type mismatch

    use crate::task_fn;

    let dag = DagRunner::new();

    // Task outputs i32
    let source = dag.add_task(task_fn(|_: ()| async { 42i32 }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get as String - TYPE MISMATCH!
    // Create a handle with the same ID but wrong type
    let wrong_handle: TaskHandle<String> = TaskHandle {
        id: source.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(wrong_handle);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        DagError::TypeMismatch { expected, found } => {
            assert_eq!(expected, std::any::type_name::<String>());
            assert_eq!(found, "unknown"); // This covers line 601!
        }
        _ => panic!("Expected TypeMismatch error, got: {:?}", err),
    }
}

#[tokio::test]
async fn test_get_type_mismatch_i32_to_vec() {
    // Different type mismatch: i32 to Vec<u8>
    use crate::task_fn;

    let dag = DagRunner::new();

    // Task outputs i32
    let int_task = dag.add_task(task_fn(|_: ()| async { 123i32 }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get as Vec<u8>
    let wrong_handle: TaskHandle<Vec<u8>> = TaskHandle {
        id: int_task.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(wrong_handle);

    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::TypeMismatch { expected, found } => {
            assert_eq!(expected, std::any::type_name::<Vec<u8>>());
            assert_eq!(found, "unknown");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[tokio::test]
async fn test_get_type_mismatch_struct_to_hashmap() {
    // Complex type mismatch: custom struct to HashMap

    #[derive(Clone)]
    #[allow(dead_code)]
    struct CustomOutput {
        value: i32,
    }

    struct CustomTask;

    #[crate::task]
    impl CustomTask {
        async fn run(&self) -> CustomOutput {
            CustomOutput { value: 42 }
        }
    }

    let dag = DagRunner::new();
    let task_handle = dag.add_task(CustomTask);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get as HashMap
    let wrong_handle: TaskHandle<HashMap<String, i32>> = TaskHandle {
        id: task_handle.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(wrong_handle);

    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::TypeMismatch { expected, found } => {
            assert!(expected.contains("HashMap"));
            assert_eq!(found, "unknown");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[tokio::test]
async fn test_get_type_mismatch_vec_different_types() {
    // Type mismatch between Vec<i32> and Vec<String>
    use crate::task_fn;

    let dag = DagRunner::new();

    // Task outputs Vec<i32>
    let vec_task = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3] }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get as Vec<String>
    let wrong_handle: TaskHandle<Vec<String>> = TaskHandle {
        id: vec_task.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(wrong_handle);

    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::TypeMismatch { expected, found } => {
            assert_eq!(expected, std::any::type_name::<Vec<String>>());
            assert_eq!(found, "unknown");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[tokio::test]
async fn test_get_type_mismatch_option_types() {
    // Type mismatch with Option types
    use crate::task_fn;

    let dag = DagRunner::new();

    // Task outputs Option<i32>
    let opt_task = dag.add_task(task_fn(|_: ()| async { Some(42) }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get as Option<String>
    let wrong_handle: TaskHandle<Option<String>> = TaskHandle {
        id: opt_task.id,
        _phantom: std::marker::PhantomData,
    };

    let result = dag.get(wrong_handle);

    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::TypeMismatch { expected, found } => {
            assert_eq!(expected, std::any::type_name::<Option<String>>());
            assert_eq!(found, "unknown");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[tokio::test]
async fn test_multiple_type_mismatches_same_dag() {
    // Test multiple type mismatches in the same DAG
    use crate::task_fn;

    let dag = DagRunner::new();

    // Create tasks with different output types
    let int_task = dag.add_task(task_fn(|_: ()| async { 100i32 }));
    let string_task = dag.add_task(task_fn(|_: ()| async { "hello".to_string() }));
    let bool_task = dag.add_task(task_fn(|_: ()| async { true }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Try to get int_task as String
    let wrong_int_handle: TaskHandle<String> = TaskHandle {
        id: int_task.id,
        _phantom: std::marker::PhantomData,
    };
    assert!(matches!(
        dag.get(wrong_int_handle).unwrap_err(),
        DagError::TypeMismatch { found, .. } if found == "unknown"
    ));

    // Try to get string_task as bool
    let wrong_string_handle: TaskHandle<bool> = TaskHandle {
        id: string_task.id,
        _phantom: std::marker::PhantomData,
    };
    assert!(matches!(
        dag.get(wrong_string_handle).unwrap_err(),
        DagError::TypeMismatch { found, .. } if found == "unknown"
    ));

    // Try to get bool_task as i32
    let wrong_bool_handle: TaskHandle<i32> = TaskHandle {
        id: bool_task.id,
        _phantom: std::marker::PhantomData,
    };
    assert!(matches!(
        dag.get(wrong_bool_handle).unwrap_err(),
        DagError::TypeMismatch { found, .. } if found == "unknown"
    ));

    // Verify correct types still work
    assert_eq!(dag.get(int_task).unwrap(), 100);
    assert_eq!(dag.get(string_task).unwrap(), "hello");
    assert!(dag.get(bool_task).unwrap());
}
