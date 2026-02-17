//! Unit tests for deps module

use crate::builder::{NodeId, TaskHandle};
use crate::deps::DepsTuple;
use crate::runner::DagRunner;

// Test task for dependency testing
struct TestTask {
    value: i32,
}

#[crate::task]
impl TestTask {
    #[cfg(not(tarpaulin_include))]
    async fn run(&self) -> i32 {
        self.value
    }
}

#[test]
fn test_deps_tuple_unit() {
    // Test lines 20-22 in deps.rs - unit type implementation
    let unit = ();
    let node_ids = unit.to_node_ids();
    assert!(node_ids.is_empty());
}

#[test]
fn test_deps_tuple_single_handle_ref() {
    // Test lines 26-30 in deps.rs - &TaskHandle<T>
    let handle: TaskHandle<i32> = TaskHandle {
        id: NodeId(42),
        _phantom: std::marker::PhantomData,
    };
    let node_ids = handle.to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], NodeId(42));
}

#[test]
fn test_deps_tuple_single_handle_owned() {
    // Test lines 32-36 in deps.rs - TaskHandle<T>
    let handle: TaskHandle<String> = TaskHandle {
        id: NodeId(99),
        _phantom: std::marker::PhantomData,
    };
    let handle_copy = handle; // Copy for use
    let node_ids = handle_copy.to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], NodeId(99));
}

#[test]
fn test_deps_tuple_single_handle_tuple_ref() {
    // Test lines 38-42 in deps.rs - (&TaskHandle<T>,)
    let handle: TaskHandle<bool> = TaskHandle {
        id: NodeId(7),
        _phantom: std::marker::PhantomData,
    };
    let node_ids = (handle,).to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], NodeId(7));
}

#[test]
fn test_deps_tuple_single_handle_tuple_owned() {
    // Test lines 44-48 in deps.rs - (TaskHandle<T>,)
    let handle: TaskHandle<Vec<u8>> = TaskHandle {
        id: NodeId(13),
        _phantom: std::marker::PhantomData,
    };
    let handle_copy = handle; // Copy for use
    let node_ids = (handle_copy,).to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], NodeId(13));
}

#[test]
fn test_deps_tuple_task_builder_ref() {
    // Test lines 51-55 in deps.rs - &TaskBuilder
    let mut dag = DagRunner::new();
    let builder = dag.add_task(TestTask { value: 42 });
    let builder_id = builder.id;

    // We need to test the DepsTuple implementation directly
    // The builder has an id field we can check
    let node_ids = builder.to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], builder_id);
}

#[test]
fn test_deps_tuple_task_builder_tuple() {
    // Test lines 57-61 in deps.rs - (&TaskBuilder,)
    let mut dag = DagRunner::new();
    let builder = dag.add_task(TestTask { value: 100 });
    let builder_id = builder.id;

    let node_ids = (builder,).to_node_ids();
    assert_eq!(node_ids.len(), 1);
    assert_eq!(node_ids[0], builder_id);
}

#[test]
fn test_deps_tuple_multiple_handles() {
    // Test macro-generated implementations for multiple handles
    let handle1: TaskHandle<i32> = TaskHandle {
        id: NodeId(1),
        _phantom: std::marker::PhantomData,
    };
    let handle2: TaskHandle<String> = TaskHandle {
        id: NodeId(2),
        _phantom: std::marker::PhantomData,
    };

    // Test 2-tuple
    let node_ids = (handle1, handle2).to_node_ids();
    assert_eq!(node_ids.len(), 2);
    assert_eq!(node_ids[0], NodeId(1));
    assert_eq!(node_ids[1], NodeId(2));

    // Test 3-tuple
    let handle3: TaskHandle<bool> = TaskHandle {
        id: NodeId(3),
        _phantom: std::marker::PhantomData,
    };
    let node_ids = (handle1, handle2, handle3).to_node_ids();
    assert_eq!(node_ids.len(), 3);
    assert_eq!(node_ids[0], NodeId(1));
    assert_eq!(node_ids[1], NodeId(2));
    assert_eq!(node_ids[2], NodeId(3));
}

#[test]
fn test_deps_tuple_multiple_builders() {
    // Test macro-generated implementations for multiple builders
    let mut dag = DagRunner::new();
    let builder1 = dag.add_task(TestTask { value: 10 });
    let builder1_id = builder1.id;
    let builder2 = dag.add_task(TestTask { value: 20 });
    let builder2_id = builder2.id;
    let builder3 = dag.add_task(TestTask { value: 30 });
    let builder3_id = builder3.id;

    // Test 3-tuple of builders
    let node_ids = (builder1, builder2, builder3).to_node_ids();
    assert_eq!(node_ids.len(), 3);
    assert_eq!(node_ids[0], builder1_id);
    assert_eq!(node_ids[1], builder2_id);
    assert_eq!(node_ids[2], builder3_id);
}

#[test]
fn test_deps_tuple_large_tuples() {
    // Test that larger tuples work (up to 16 elements)
    let mut handles = Vec::new();
    for i in 0..8 {
        handles.push(TaskHandle::<i32> {
            id: NodeId(i),
            _phantom: std::marker::PhantomData,
        });
    }

    // Test 4-tuple
    let node_ids = (handles[0], handles[1], handles[2], handles[3]).to_node_ids();
    assert_eq!(node_ids.len(), 4);
    for (i, node_id) in node_ids.iter().enumerate().take(4) {
        assert_eq!(*node_id, NodeId(i as u32));
    }

    // Test 8-tuple
    let node_ids = (
        handles[0], handles[1], handles[2], handles[3], handles[4], handles[5], handles[6],
        handles[7],
    )
        .to_node_ids();
    assert_eq!(node_ids.len(), 8);
    for (i, node_id) in node_ids.iter().enumerate().take(8) {
        assert_eq!(*node_id, NodeId(i as u32));
    }
}
