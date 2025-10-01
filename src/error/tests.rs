//! Unit tests for error module

use crate::error::{DagError, DagResult};

#[test]
fn test_dag_error_display_invalid_dependency() {
    // Test lines 47-54 in error.rs
    let err = DagError::InvalidDependency { task_id: 42 };
    let display = format!("{}", err);

    assert!(display.contains("Invalid dependency"));
    assert!(display.contains("task #42"));
    assert!(display.contains("does not exist"));
    assert!(display.contains("Ensure all task handles"));
}

#[test]
fn test_dag_error_display_type_mismatch() {
    // Test lines 56-65 in error.rs
    let err = DagError::TypeMismatch {
        expected: "i32",
        found: "String",
    };
    let display = format!("{}", err);

    assert!(display.contains("Type mismatch"));
    assert!(display.contains("Expected: i32"));
    assert!(display.contains("Found: String"));
    assert!(display.contains("Verify that dependency types"));
}

#[test]
fn test_dag_error_display_task_panicked() {
    // Test lines 67-77 in error.rs
    let err = DagError::TaskPanicked {
        task_id: 99,
        panic_message: "assertion failed".to_string(),
    };
    let display = format!("{}", err);

    assert!(display.contains("Task #99 panicked"));
    assert!(display.contains("assertion failed"));
    assert!(display.contains("indicating a bug"));
    assert!(display.contains("entire DAG execution is aborted"));
}

#[test]
fn test_dag_error_display_cycle_detected() {
    let err = DagError::CycleDetected {
        nodes: vec![1, 2, 3, 1],
        description: "A->B->C->A".to_string(),
    };
    let display = format!("{}", err);

    assert!(display.contains("Cycle detected"));
    assert!(display.contains("A->B->C->A"));
    assert!(display.contains("[1, 2, 3, 1]"));
    assert!(display.contains("cannot have cycles"));
}

#[test]
fn test_dag_error_display_result_not_found() {
    let err = DagError::ResultNotFound { task_id: 7 };
    let display = format!("{}", err);

    assert!(display.contains("Result not found"));
    assert!(display.contains("task #7"));
    assert!(display.contains("Call dag.run()"));
}

#[test]
fn test_dag_error_std_error_impl() {
    // Test that DagError implements std::error::Error
    let err = DagError::InvalidDependency { task_id: 1 };
    let err_ref: &dyn std::error::Error = &err;

    // Should be able to call Error trait methods
    let _ = err_ref.to_string();
    assert!(err_ref.source().is_none()); // DagError doesn't chain errors
}

#[test]
fn test_dag_error_equality() {
    let err1 = DagError::InvalidDependency { task_id: 42 };
    let err2 = DagError::InvalidDependency { task_id: 42 };
    let err3 = DagError::InvalidDependency { task_id: 99 };

    assert_eq!(err1, err2);
    assert_ne!(err1, err3);
}

#[test]
fn test_dag_error_clone() {
    let err = DagError::TaskPanicked {
        task_id: 10,
        panic_message: "test panic".to_string(),
    };

    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_dag_error_debug() {
    let err = DagError::TypeMismatch {
        expected: "i32",
        found: "bool",
    };

    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("TypeMismatch"));
    assert!(debug_str.contains("expected"));
    assert!(debug_str.contains("found"));
}

#[test]
fn test_dag_result_type_alias() {
    // Test that DagResult works as expected
    fn returns_dag_result() -> DagResult<i32> {
        Ok(42)
    }

    fn returns_dag_error() -> DagResult<String> {
        Err(DagError::ResultNotFound { task_id: 1 })
    }

    assert!(returns_dag_result().is_ok());
    assert!(returns_dag_error().is_err());
}
