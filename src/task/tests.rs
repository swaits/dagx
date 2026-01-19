//! Unit tests for task module

use crate::task::{task_fn, Task};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Test the task_fn function and TaskFn struct
#[tokio::test]
async fn test_task_fn_creation_and_execution() {
    // Test line 111 in task.rs - Task implementation for TaskFn

    // Create a simple task function
    let task = task_fn(|input: i32| async move { input * 2 });

    // Test the Task trait implementation
    let result = task.run(42).await;
    assert_eq!(result, 84);
}

#[tokio::test]
async fn test_task_fn_with_unit_input() {
    // Test task_fn with unit input
    let task = task_fn(|_: ()| async { 100 });

    let result = task.run(()).await;
    assert_eq!(result, 100);
}

#[tokio::test]
async fn test_task_fn_with_tuple_input() {
    // Test task_fn with tuple input
    let task = task_fn(|(a, b): (i32, i32)| async move { a + b });

    let result = task.run((10, 20)).await;
    assert_eq!(result, 30);
}

#[tokio::test]
async fn test_task_fn_with_string_input() {
    // Test task_fn with String input
    let task = task_fn(|input: String| {
        let len = input.len();
        async move { len }
    });

    let test_string = String::from("hello");
    let result = task.run(test_string).await;
    assert_eq!(result, 5);
}

#[tokio::test]
async fn test_task_fn_with_complex_output() {
    // Test task_fn with complex output type
    let task = task_fn(|input: i32| async move { vec![input, input * 2, input * 3] });

    let result = task.run(5).await;
    assert_eq!(result, vec![5, 10, 15]);
}

#[tokio::test]
async fn test_task_fn_stateful_closure() {
    // Test that task_fn can capture state
    let multiplier = 3;
    let task = task_fn(move |input: i32| async move { input * multiplier });

    let result = task.run(7).await;
    assert_eq!(result, 21);
}

// Test custom Task implementation
struct CustomTask {
    state: i32,
}

impl Task for CustomTask {
    type Input = i32;
    type Output = i32;

    #[allow(refining_impl_trait)]
    fn run(mut self, input: Self::Input) -> Pin<Box<dyn Future<Output = Self::Output> + Send>> {
        Box::pin(async move {
            self.state += input;
            self.state
        })
    }

    #[allow(refining_impl_trait)]
    fn extract_and_run(
        self,
        _dependencies: Vec<Arc<dyn std::any::Any + Send + Sync>>,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, String>> + Send>> {
        Box::pin(async move {
            // This is a test helper - just return an error
            Err("extract_and_run not implemented for CustomTask (test helper)".to_string())
        })
    }
}

#[tokio::test]
async fn test_custom_task_implementation() {
    let task = CustomTask { state: 10 };

    let result1 = task.run(5).await;
    assert_eq!(result1, 15);

    // Can't call run again since task was consumed
    // This demonstrates the ownership model
}

// Test that Task trait bounds are correct
fn assert_task_bounds<T: Task>() {
    // This function just checks that T implements Task with the right bounds
    fn assert_send<T: Send>() {}

    // These should compile if bounds are correct
    assert_send::<T>();
    assert_send::<T::Input>();
    assert_send::<T::Output>();
}

#[test]
fn test_task_trait_bounds() {
    // Verify that our test tasks meet the Task trait bounds
    assert_task_bounds::<CustomTask>();

    // We can't directly test TaskFn because it's a private type and needs a concrete closure type,
    // but the fact that task_fn works in the tests above proves it's correct
}

#[tokio::test]
async fn test_task_fn_single_call() {
    // Test that task_fn works (can only call once due to ownership)
    let task = task_fn(|x: i32| async move { x + 1 });

    assert_eq!(task.run(1).await, 2);
}

#[tokio::test]
async fn test_task_fn_with_result_type() {
    // Test task_fn returning Result
    let task = task_fn(|x: i32| async move {
        if x > 0 {
            Ok(x * 2)
        } else {
            Err("negative input")
        }
    });

    assert_eq!(task.run(5).await, Ok(10));
}

#[tokio::test]
async fn test_task_fn_with_option_type() {
    // Test task_fn returning Option
    let task = task_fn(|x: i32| async move {
        if x > 0 {
            Some(x * 2)
        } else {
            None
        }
    });

    assert_eq!(task.run(5).await, Some(10));
}

#[tokio::test]
async fn test_task_fn_explicit_trait_call() {
    // Test explicit Task trait method call to cover line 121

    let task = task_fn(|x: i32| async move { x * 3 });

    // Explicitly call the Task trait's run method
    let result = Task::run(task, 10).await;
    assert_eq!(result, 30);
}
