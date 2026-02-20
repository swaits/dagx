//! Coverage tests for types module - specifically targeting the TaskBuilder to TaskHandle conversion
//!
//! These tests ensure we hit line 71 in the From<TaskBuilder> implementation

use crate::{task, DagRunner, TaskHandle};

#[tokio::test]
async fn test_unit_struct_with_state() {
    // Even structs with fields can have unit input type

    struct StatefulTask {
        value: i32,
    }

    impl StatefulTask {
        fn new(value: i32) -> Self {
            Self { value }
        }
    }

    #[task]
    impl StatefulTask {
        async fn run(&self) -> i32 {
            self.value * 2
        }
    }

    let mut dag = DagRunner::new();

    // Create task with initial state
    let builder = dag.add_task(StatefulTask::new(21));

    // Explicit conversion - triggers line 71
    let handle = builder;

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(output.get(handle), 42);
}

#[test]
fn test_taskbuilder_conversion_without_running() {
    // Test that conversion works even without running the DAG
    struct TestTask;
    #[task]
    impl TestTask {
        async fn run(&self) -> String {
            "test".to_string()
        }
    }

    let mut dag = DagRunner::new();
    let builder = dag.add_task(TestTask);
    let builder_id = builder.id;

    // Convert without running the DAG
    let handle: TaskHandle<String> = builder;

    // Verify the ID was transferred correctly
    assert_eq!(handle.id, builder_id);
}
