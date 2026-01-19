//! Unit tests for node module

use std::sync::Arc;

use crate::node::{ExecutableNode, TypedNode};
use crate::types::NodeId;

// Simple test task
struct TestTask {
    value: i32,
}

#[crate::task]
impl TestTask {
    async fn run(&self) -> i32 {
        self.value
    }
}

struct TaskWithDependency;

#[crate::task]
impl TaskWithDependency {
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[tokio::test]
async fn test_execute_with_deps_missing_input() {
    // Test when task expects input but doesn't receive it
    let node = Box::new(TypedNode::new(NodeId(0), TaskWithDependency));

    // TaskWithDependency expects an i32 input, but we provide no dependencies
    let dependencies = vec![];

    let result = node.execute_with_deps(dependencies).await;
    assert!(result.is_err());

    if let Err(crate::error::DagError::InvalidDependency { task_id }) = result {
        assert_eq!(task_id, 0);
    } else {
        panic!("Expected InvalidDependency error, got: {:?}", result);
    }
}

#[tokio::test]
async fn test_execute_with_deps_success() {
    // Test successful execution with channels
    let dependency_node = Box::new(TypedNode::new(NodeId(0), TestTask { value: 21 }));
    let dependent_node = Box::new(TypedNode::new(NodeId(1), TaskWithDependency));

    // Execute dependency node with sender
    let dep_dependencies = vec![];

    // Execute in background
    let dep_result =
        tokio::spawn(async move { dependency_node.execute_with_deps(dep_dependencies).await })
            .await
            .unwrap()
            .unwrap();

    // Execute dependent node with receiver
    let dependencies = vec![dep_result as Arc<dyn std::any::Any + Send + Sync>];

    let result = dependent_node.execute_with_deps(dependencies).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_deps_sink_node() {
    // Test that sink nodes (no dependents) return their output
    let node = Box::new(TypedNode::new(NodeId(0), TestTask { value: 42 }));

    // No senders means this is a sink node
    let dependencies = vec![];

    let result = node.execute_with_deps(dependencies).await;
    assert!(result.is_ok());

    // Check output was returned (Arc-wrapped)
    let output = result.unwrap();
    let arc_value = output.downcast::<i32>().unwrap();
    assert_eq!(*arc_value, 42);
}

#[tokio::test]
async fn test_execute_with_deps_non_sink_node() {
    // Test that non-sink nodes send output via channels
    let node = TypedNode::new(NodeId(0), TestTask { value: 42 });

    // Execute with senders (non-sink node)
    let node = Box::new(node);
    let result = node.execute_with_deps(vec![]).await;
    assert!(result.is_ok());

    // Verify output was sent through channel (Arc-wrapped)
    let arc_value = result.unwrap().downcast::<i32>().unwrap();

    // Also verify output was returned (Arc-wrapped)
    assert_eq!(*arc_value, 42);
}
