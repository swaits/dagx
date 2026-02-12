//! Unit tests for types module

use crate::types::{NodeId, TaskHandle};
use std::marker::PhantomData;

#[test]
#[allow(clippy::clone_on_copy)]
fn test_task_handle_clone() {
    let handle: TaskHandle<(i32,)> = TaskHandle {
        id: NodeId(42),
        _phantom: PhantomData,
    };

    // Test Clone implementation
    let cloned = handle.clone();
    assert_eq!(handle.id.0, cloned.id.0);

    let handle2 = handle.clone();
    let handle3 = handle2.clone();
    assert_eq!(handle3.id.0, 42);
}

#[test]
fn test_task_handle_copy() {
    let handle: TaskHandle<String> = TaskHandle {
        id: NodeId(99),
        _phantom: PhantomData,
    };

    // Test that TaskHandle implements Copy
    let copied = handle;
    let copied2 = handle; // This wouldn't compile if not Copy
    assert_eq!(copied.id.0, 99);
    assert_eq!(copied2.id.0, 99);
}

#[test]
fn test_task_handle_ref_from_conversion() {
    // Test From<&TaskHandle<T>> for TaskHandle<T> (line 90-93)
    let handle: TaskHandle<String> = TaskHandle {
        id: NodeId(123),
        _phantom: PhantomData,
    };

    let handle_ref = &handle;
    let converted: TaskHandle<String> = handle_ref.into();

    assert_eq!(converted.id.0, 123);
    assert_eq!(handle.id, converted.id);
}
