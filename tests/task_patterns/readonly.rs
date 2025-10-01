// Read-only state task pattern tests
use crate::common::task_fn;
// These tests verify the read-only state pattern using &self

use dagx::*;

// Read-only state tasks
struct ReadOnlyValue(i32);

#[task]
impl ReadOnlyValue {
    async fn run(&self) -> i32 {
        self.0 // Read-only access
    }
}

struct ReadOnlyMultiplier(i32);

#[task]
impl ReadOnlyMultiplier {
    async fn run(&self, input: &i32) -> i32 {
        input * self.0 // Read-only access to state
    }
}

struct ReadOnlyConfig {
    multiplier: i32,
    offset: i32,
}

#[task]
impl ReadOnlyConfig {
    async fn run(&self, input: &i32) -> i32 {
        (input * self.multiplier) + self.offset
    }
}

#[tokio::test]
async fn test_readonly_state_basic() {
    // Test basic read-only state task using &self
    let dag = DagRunner::new();

    let value = dag.add_task(ReadOnlyValue(42));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(value).unwrap(), 42);
}

#[tokio::test]
async fn test_readonly_state_with_input() {
    // Test read-only state task with input using &self
    let dag = DagRunner::new();

    let input = dag.add_task(task_fn(|_: ()| async { 5 }));
    let result = dag.add_task(ReadOnlyMultiplier(7)).depends_on(&input);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(result).unwrap(), 35);
}

#[tokio::test]
async fn test_readonly_state_multiple_uses() {
    // Test that read-only state can be used multiple times
    let dag = DagRunner::new();

    let input1 = dag.add_task(task_fn(|_: ()| async { 10 }));
    let input2 = dag.add_task(task_fn(|_: ()| async { 20 }));

    let config = ReadOnlyConfig {
        multiplier: 3,
        offset: 5,
    };

    let result1 = dag.add_task(config).depends_on(&input1);
    // Note: Can't reuse config because it was moved, so create another
    let config2 = ReadOnlyConfig {
        multiplier: 3,
        offset: 5,
    };
    let result2 = dag.add_task(config2).depends_on(&input2);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(result1).unwrap(), 35); // (10 * 3) + 5
    assert_eq!(dag.get(result2).unwrap(), 65); // (20 * 3) + 5
}
