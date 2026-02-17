//! Unit tests for task module

use crate::task::{Task, TaskInput};
use std::any::Any;
use std::sync::Arc;

// Test custom Task implementation
struct CustomTask {
    state: i32,
}

impl Task<(i32,)> for CustomTask {
    type Output = i32;

    async fn run(mut self, input: TaskInput<'_, (i32,)>) -> i32 {
        self.state += input.next().0;
        self.state
    }
}

#[tokio::test]
async fn test_custom_task_implementation() {
    let task = CustomTask { state: 10 };

    let result1 = task
        .run(TaskInput::new(
            [Arc::new(5) as Arc<dyn Any + Send + Sync + 'static>].iter(),
        ))
        .await;
    assert_eq!(result1, 15);

    // Can't call run again since task was consumed
    // This demonstrates the ownership model
}

#[test]
fn test_empty_task_input() {
    let empty = TaskInput::empty();
    assert_eq!(empty.inputs.len(), 0);
}
