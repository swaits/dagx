//! Coverage tests for types module - specifically targeting the TaskBuilder to TaskHandle conversion
//!
//! These tests ensure we hit line 71 in the From<TaskBuilder> implementation

use crate::{task, DagRunner, TaskHandle};
use futures::FutureExt;

#[tokio::test]
async fn test_unit_type_taskbuilder_to_handle_conversion() {
    // Test that explicitly uses the From<TaskBuilder> conversion for unit-type tasks
    // This covers line 71 in types.rs

    // Unit struct (no fields) - its Input type is ()
    struct UnitTask;

    #[task]
    impl UnitTask {
        async fn run(&self) -> i32 {
            42
        }
    }

    let dag = DagRunner::new();

    // This creates a TaskBuilder<UnitTask, Pending>
    // Since UnitTask has Input = (), it can be converted to TaskHandle
    let builder = dag.add_task(UnitTask);

    // Explicitly convert to TaskHandle using From implementation
    // This triggers the From<TaskBuilder> implementation at line 69-74,
    // specifically covering line 71
    let handle: TaskHandle<_> = builder.into();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Verify the task ran and we can get its output
    assert_eq!(dag.get(handle).unwrap(), 42);
}

#[tokio::test]
async fn test_unit_type_conversion_with_multiple_tasks() {
    // Test conversion with multiple unit-type tasks

    struct FirstTask;
    #[task]
    impl FirstTask {
        async fn run(&self) -> String {
            "first".to_string()
        }
    }

    struct SecondTask;
    #[task]
    impl SecondTask {
        async fn run(&self) -> i32 {
            100
        }
    }

    struct ThirdTask;
    #[task]
    impl ThirdTask {
        async fn run(&self) -> bool {
            true
        }
    }

    let dag = DagRunner::new();

    // Create builders for all tasks
    let first_builder = dag.add_task(FirstTask);
    let second_builder = dag.add_task(SecondTask);
    let third_builder = dag.add_task(ThirdTask);

    // Convert each builder to handle explicitly
    let first_handle: TaskHandle<String> = first_builder.into();
    let second_handle: TaskHandle<_> = second_builder.into();
    let third_handle: TaskHandle<bool> = third_builder.into();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Verify all outputs
    assert_eq!(dag.get(first_handle).unwrap(), "first");
    assert_eq!(dag.get(second_handle).unwrap(), 100);
    assert!(dag.get(third_handle).unwrap());
}

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

    let dag = DagRunner::new();

    // Create task with initial state
    let builder = dag.add_task(StatefulTask::new(21));

    // Explicit conversion - triggers line 71
    let handle: TaskHandle<_> = builder.into();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(handle).unwrap(), 42);
}

#[tokio::test]
async fn test_taskbuilder_conversion_assignment() {
    // Test the conversion through assignment (different from .into())

    struct SimpleTask;
    #[task]
    impl SimpleTask {
        async fn run(&self) -> Vec<i32> {
            vec![1, 2, 3]
        }
    }

    let dag = DagRunner::new();
    let builder = dag.add_task(SimpleTask);

    // Conversion through type annotation and assignment
    // This also uses From<TaskBuilder> implementation
    let handle: TaskHandle<Vec<i32>> = builder.into();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let result = dag.get(handle).unwrap();
    assert_eq!(result, vec![1, 2, 3]);
}

#[tokio::test]
async fn test_mixed_unit_and_dependent_tasks() {
    // Test that unit-type tasks work alongside tasks with dependencies

    struct Source;
    #[task]
    impl Source {
        async fn run(&self) -> i32 {
            10
        }
    }

    struct Doubler;
    #[task]
    impl Doubler {
        async fn run(&self, input: &i32) -> i32 {
            input * 2
        }
    }

    let dag = DagRunner::new();

    // Source is unit-type, can be converted
    let source_builder = dag.add_task(Source);
    let source_handle: TaskHandle<_> = source_builder.into(); // Covers line 71

    // Doubler needs dependencies, so we use it differently
    let doubler = dag.add_task(Doubler).depends_on(source_handle);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(source_handle).unwrap(), 10);
    assert_eq!(dag.get(doubler).unwrap(), 20);
}

#[test]
fn test_taskbuilder_conversion_without_running() {
    // Test that conversion works even without running the DAG
    // This ensures the conversion is purely a type-level operation

    struct TestTask;
    #[task]
    impl TestTask {
        async fn run(&self) -> String {
            "test".to_string()
        }
    }

    let dag = DagRunner::new();
    let builder = dag.add_task(TestTask);
    let builder_id = builder.id;

    // Convert without running the DAG
    let handle: TaskHandle<String> = builder.into();

    // Verify the ID was transferred correctly
    assert_eq!(handle.id, builder_id);
}

#[test]
fn test_multiple_conversions_preserve_ids() {
    // Ensure that multiple conversions maintain correct IDs

    struct Task1;
    #[task]
    impl Task1 {
        async fn run(&self) -> i32 {
            1
        }
    }

    struct Task2;
    #[task]
    impl Task2 {
        async fn run(&self) -> i32 {
            2
        }
    }

    struct Task3;
    #[task]
    impl Task3 {
        async fn run(&self) -> i32 {
            3
        }
    }

    let dag = DagRunner::new();

    let builder1 = dag.add_task(Task1);
    let builder2 = dag.add_task(Task2);
    let builder3 = dag.add_task(Task3);

    let id1 = builder1.id;
    let id2 = builder2.id;
    let id3 = builder3.id;

    // Convert all builders
    let handle1: TaskHandle<_> = builder1.into();
    let handle2: TaskHandle<_> = builder2.into();
    let handle3: TaskHandle<_> = builder3.into();

    // Verify IDs are preserved and sequential
    assert_eq!(handle1.id, id1);
    assert_eq!(handle2.id, id2);
    assert_eq!(handle3.id, id3);
    assert_eq!(handle1.id.0 + 1, handle2.id.0);
    assert_eq!(handle2.id.0 + 1, handle3.id.0);
}
