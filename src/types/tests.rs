//! Unit tests for types module

use crate::task::Task;
use crate::types::{NodeId, TaskHandle};
use std::marker::PhantomData;

#[test]
fn test_node_id_operations() {
    let id1 = NodeId(1);
    let id2 = NodeId(2);
    let id3 = NodeId(1);

    // Test equality
    assert_eq!(id1, id3);
    assert_ne!(id1, id2);

    // Test ordering
    assert!(id1 < id2);
    assert!(id2 > id1);

    // Test Debug
    let debug_str = format!("{:?}", id1);
    assert!(debug_str.contains("NodeId"));
    assert!(debug_str.contains("1"));

    // Test Clone
    let cloned = id1;
    assert_eq!(id1, cloned);

    // Test Copy
    let copied = id1;
    assert_eq!(id1, copied);

    // Test Hash (compile-time check that it implements Hash)
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(id1);
    set.insert(id2);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_task_handle_clone() {
    let handle: TaskHandle<i32> = TaskHandle {
        id: NodeId(42),
        _phantom: PhantomData,
    };

    // Test Clone implementation (lines 44-46 in types.rs)
    let cloned = handle;
    assert_eq!(handle.id.0, cloned.id.0);

    // Test that clone is actually a copy (same memory semantics)
    let handle2 = handle;
    let handle3 = handle2;
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
fn test_task_handle_with_different_types() {
    // Test that TaskHandle works with various types
    let int_handle: TaskHandle<i32> = TaskHandle {
        id: NodeId(1),
        _phantom: PhantomData,
    };

    let string_handle: TaskHandle<String> = TaskHandle {
        id: NodeId(2),
        _phantom: PhantomData,
    };

    let vec_handle: TaskHandle<Vec<u8>> = TaskHandle {
        id: NodeId(3),
        _phantom: PhantomData,
    };

    // Clone and copy should work for all
    let _ = int_handle;
    let _ = string_handle;
    let _ = vec_handle;

    let _ = int_handle;
    let _ = string_handle;
    let _ = vec_handle;
}

#[test]
fn test_task_handle_explicit_clone() {
    // Test explicit .clone() call to cover line 47-48
    let handle: TaskHandle<i32> = TaskHandle {
        id: NodeId(42),
        _phantom: PhantomData,
    };

    #[allow(clippy::clone_on_copy)]
    let cloned = handle.clone();
    assert_eq!(handle.id.0, cloned.id.0);
    assert_eq!(cloned.id.0, 42);
}

#[test]
fn test_task_builder_from_conversion() {
    use crate::runner::DagRunner;

    // Test From<TaskBuilder> for TaskHandle (line 69-74)
    struct SimpleTask;
    #[crate::task]
    impl SimpleTask {
        async fn run(&self) -> i32 {
            42
        }
    }

    let dag = DagRunner::new();
    let builder = dag.add_task(SimpleTask);
    let builder_id = builder.id;

    // This conversion uses the From implementation
    let handle: TaskHandle<i32> = builder.into();
    assert_eq!(handle.id, builder_id);
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
