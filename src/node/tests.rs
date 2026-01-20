//! Unit tests for node module

use crate::node::{ExecutableNode, TypedNode};
use crate::task::Task;
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

#[test]
fn test_create_output_channels() {
    let node = TypedNode::new(NodeId(0), TestTask { value: 42 });

    // Create channels for 3 dependents
    let (senders, receivers) = node.create_output_channels(3);

    assert_eq!(senders.len(), 3);
    assert_eq!(receivers.len(), 3);
}

#[test]
fn test_create_output_channels_zero_dependents() {
    let node = TypedNode::new(NodeId(0), TestTask { value: 42 });

    // Create channels for 0 dependents (sink node)
    let (senders, receivers) = node.create_output_channels(0);

    assert_eq!(senders.len(), 0);
    assert_eq!(receivers.len(), 0);
}

struct TaskWithDependency;

#[crate::task]
impl TaskWithDependency {
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[tokio::test]
async fn test_execute_with_channels_missing_input() {
    // Test when task expects input but doesn't receive it
    let node = Box::new(TypedNode::new(NodeId(0), TaskWithDependency));

    // TaskWithDependency expects an i32 input, but we provide no receivers
    let receivers = vec![];
    let senders = vec![];

    let result = node.execute_with_channels(receivers, senders).await;
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
    let dep_handle = tokio::spawn(async move {
        dependency_node
            .execute_with_channels(dep_receivers, dep_senders)
            .await
    });

    // Execute dependent node with receiver
    let dependencies = vec![dep_result as Arc<dyn std::any::Any + Send + Sync>];

    let result = dependent_node.execute_with_deps(dependencies).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_channels_sink_node() {
    // Test that sink nodes (no dependents) return their output
    let node = Box::new(TypedNode::new(NodeId(0), TestTask { value: 42 }));

    // No senders means this is a sink node
    let dependencies = vec![];

    let result = node.execute_with_channels(receivers, senders).await;
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
    let exec_receivers = vec![];
    let result = node.execute_with_channels(exec_receivers, senders).await;
    assert!(result.is_ok());

    // Verify output was sent through channel (Arc-wrapped)
    let arc_value = result.unwrap().downcast::<i32>().unwrap();

    // Also verify output was returned (Arc-wrapped)
    let output = result.unwrap();
    let arc_value = output.downcast::<i32>().unwrap();
    assert_eq!(*arc_value, 42);
}

#[tokio::test]
async fn test_execute_with_multiple_dependents() {
    // Test that output is sent to multiple dependents
    let node = TypedNode::new(NodeId(0), TestTask { value: 100 });

    // Create output channels for three dependents
    let (senders, receivers) = node.create_output_channels(3);

    // Execute node
    let node = Box::new(node);
    let exec_receivers = vec![];
    let result = node.execute_with_channels(exec_receivers, senders).await;
    assert!(result.is_ok());

    // Verify all three dependents receive the output (Arc-wrapped)
    for receiver in receivers {
        let rx = receiver
            .downcast::<futures::channel::oneshot::Receiver<std::sync::Arc<i32>>>()
            .unwrap();

        let received_arc = rx.await;
        assert!(received_arc.is_ok());
        assert_eq!(*received_arc.unwrap(), 100);
    }
}
