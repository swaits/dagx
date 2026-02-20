//! Unit tests for error module

use crate::error::DagError;

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
fn test_dag_error_std_error_impl() {
    // Test that DagError implements std::error::Error
    let err = DagError::TaskPanicked {
        task_id: 1,
        panic_message: "test panic".to_string(),
    };
    let err_ref: &dyn std::error::Error = &err;

    // Should be able to call Error trait methods
    let _ = err_ref.to_string();
    assert!(err_ref.source().is_none()); // DagError doesn't chain errors
}
