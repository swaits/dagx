//! Unit tests for runner module

use std::collections::HashMap;
use std::hash::Hasher;

use crate::builder::TaskHandle;
use crate::runner::{DagRunner, PassThroughHasher};

// Initialize tracing subscriber for tests (idempotent)
#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::TRACE)
            .try_init()
            .ok();
    });
}

#[cfg(not(feature = "tracing"))]
fn init_tracing() {
    // No-op when tracing is disabled
}

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
    let mut dag = DagRunner::new();

    // Initial state should be empty
    assert_eq!(dag.nodes.len(), 0);
    assert_eq!(dag.edges.len(), 0);
    assert_eq!(dag.dependents.len(), 0);

    // Adding a task works
    dag.add_task(TestTask { value: 42 });
    assert_eq!(dag.nodes.len(), 1);
}

#[test]
fn test_dag_runner_default() {
    // Test lines 105-106 in runner.rs - Default implementation
    let dag = DagRunner::default();

    // Should behave the same as new()
    assert_eq!(dag.nodes.len(), 0);
    assert_eq!(dag.edges.len(), 0);
    assert_eq!(dag.dependents.len(), 0);
}

#[tokio::test]
#[should_panic]
async fn test_get_wrong_type() {
    init_tracing();
    // Test getting with wrong type - this should panic
    let mut dag = DagRunner::new();
    let handle = dag.add_task(TestTask { value: 42 });

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Try to get with wrong type - downcast will fail
    let fake_handle: TaskHandle<String> = TaskHandle {
        id: handle.id,
        _phantom: std::marker::PhantomData,
    };

    let _result = output.get(fake_handle);
}

#[tokio::test]
#[should_panic]
async fn test_invalid_node_id_in_get() {
    init_tracing();
    // Test getting with an invalid node ID
    let dag = DagRunner::new();

    // Create a handle with an ID that doesn't exist
    let invalid_handle: TaskHandle<i32> = TaskHandle {
        id: crate::builder::NodeId(999),
        _phantom: std::marker::PhantomData,
    };

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    let _result = output.get(invalid_handle);
}

#[tokio::test]
async fn test_task_panic_in_multi_task_layer() {
    init_tracing();
    // Test that a panic in one task of a multi-task layer is caught and reported

    struct Source;
    #[crate::task]
    impl Source {
        async fn run(&self) -> i32 {
            42
        }
    }

    struct PanicTask;
    #[crate::task]
    impl PanicTask {
        async fn run(&self, _input: &i32) -> i32 {
            panic!("This task panics!");
        }
    }

    struct GoodTask;
    #[crate::task]
    impl GoodTask {
        async fn run(&self, input: &i32) -> i32 {
            input * 2
        }
    }

    let mut dag = DagRunner::new();
    let source = dag.add_task(Source);

    // Create two tasks in the same layer - one panics, one doesn't
    let _panic_task = dag.add_task(PanicTask).depends_on(&source);
    let _good_task = dag.add_task(GoodTask).depends_on(&source);

    // Run the DAG - the panic should be caught
    let result = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await;

    // The run should fail due to the panic
    assert!(result.is_err());
}

#[test]
fn test_passthrough_hasher() {
    let mut passthrough_hashmap = HashMap::with_hasher(PassThroughHasher::default());
    passthrough_hashmap.insert(64u32, "test string");
}

#[test]
#[should_panic]
fn test_passthrough_hasher_only_u32() {
    let mut passthrough_hasher = PassThroughHasher::default();
    passthrough_hasher.write(&[0; 4]);
}
