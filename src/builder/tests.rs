//! Unit tests for builder module

use crate::builder::IsUnitType;
use crate::runner::DagRunner;
use crate::task::Task;
use crate::types::TaskHandle;

struct TestTask {
    value: i32,
}

#[crate::task]
impl TestTask {
    async fn run(&self) -> i32 {
        self.value
    }
}

struct TestTaskWithInput;

#[crate::task]
impl TestTaskWithInput {
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[test]
fn test_task_builder_from_conversion() {
    // Test line 165 in builder.rs - the From implementation
    let dag = DagRunner::new();
    let builder = dag.add_task(TestTask { value: 42 });

    // Convert builder to TaskHandle
    let handle: TaskHandle<i32> = builder.into();

    // Check that the ID is preserved
    assert_eq!(handle.id.0, 0); // First task should have ID 0
}

#[test]
fn test_is_unit_type_trait() {
    // Test that IsUnitType is correctly implemented for ()
    fn assert_is_unit_type<T: IsUnitType>() {}

    // This should compile - () implements IsUnitType
    assert_is_unit_type::<()>();

    // We can't test the negative case (non-unit types don't implement it)
    // because that would be a compile error
}

#[test]
fn test_task_builder_depends_on_returns_handle() {
    let dag = DagRunner::new();

    let source = dag.add_task(TestTask { value: 10 });
    let dependent = dag.add_task(TestTaskWithInput).depends_on(&source);

    // depends_on should return a TaskHandle
    let _handle: TaskHandle<i32> = dependent;
}

#[test]
fn test_task_builder_chain() {
    let dag = DagRunner::new();

    // Test chaining multiple tasks
    let t1 = dag.add_task(TestTask { value: 1 });
    let t2 = dag.add_task(TestTaskWithInput).depends_on(&t1);
    let t3 = dag.add_task(TestTaskWithInput).depends_on(t2);

    // All should return TaskHandles
    let _: TaskHandle<i32> = t1.into();
    let _: TaskHandle<i32> = t2;
    let _: TaskHandle<i32> = t3;
}

#[test]
fn test_multiple_dependencies() {
    struct AddTask;

    #[crate::task]
    impl AddTask {
        async fn run(a: &i32, b: &i32) -> i32 {
            a + b
        }
    }

    let dag = DagRunner::new();

    let a = dag.add_task(TestTask { value: 10 });
    let b = dag.add_task(TestTask { value: 20 });

    // Test multiple dependencies
    let sum = dag.add_task(AddTask).depends_on((&a, &b));

    // Should return a TaskHandle
    let _: TaskHandle<i32> = sum;
}

#[test]
fn test_task_builder_stores_correct_id() {
    let dag = DagRunner::new();

    let builder1 = dag.add_task(TestTask { value: 1 });
    let builder2 = dag.add_task(TestTask { value: 2 });
    let builder3 = dag.add_task(TestTask { value: 3 });

    // Check that builders have sequential IDs
    assert_eq!(builder1.id.0, 0);
    assert_eq!(builder2.id.0, 1);
    assert_eq!(builder3.id.0, 2);
}
