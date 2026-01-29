// Combined task pattern tests
use crate::common::task_fn;
// Comprehensive test showing all three patterns working together

use dagx::*;
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Pattern 1: Stateless - Pure computation, no state
struct StatelessAdd;
#[task]
impl StatelessAdd {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b
    }
}

// Pattern 2: Read-only state - Configuration data
struct ReadOnlyMultiplier(i32);
#[task]
impl ReadOnlyMultiplier {
    async fn run(&self, input: &i32) -> i32 {
        input * self.0 // Read-only access
    }
}

// Pattern 3: Mutable state - Accumulates state
struct MutableAccumulator(i32);
#[task]
impl MutableAccumulator {
    async fn run(&mut self, input: &i32) -> i32 {
        self.0 += input; // Modifies state
        self.0
    }
}

#[tokio::test]
async fn test_all_three_task_patterns() {
    // Comprehensive test showing all three patterns working together
    let dag = DagRunner::new();

    // Create initial values
    let x = dag.add_task(task_fn(|_: ()| async { 5 }));
    let y = dag.add_task(task_fn(|_: ()| async { 3 }));

    // Pattern 1: Stateless addition
    let sum = dag.add_task(StatelessAdd).depends_on((x, y)); // 5 + 3 = 8

    // Pattern 2: Read-only multiplication
    let doubled = dag.add_task(ReadOnlyMultiplier(2)).depends_on(sum); // 8 * 2 = 16

    // Pattern 3: Mutable accumulation
    let accumulated = dag.add_task(MutableAccumulator(10)).depends_on(doubled); // 10 + 16 = 26

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(sum).unwrap(), 8);
    assert_eq!(dag.get(doubled).unwrap(), 16);
    assert_eq!(dag.get(accumulated).unwrap(), 26);
}

#[tokio::test]
async fn test_parallel_execution() {
    let dag = DagRunner::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Create 3 independent tasks that should run in parallel
    let c1 = counter.clone();
    let c2 = counter.clone();
    let c3 = counter.clone();

    let t1 = dag.add_task(task_fn(move |_: ()| {
        let c = c1.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            1
        }
    }));

    let t2 = dag.add_task(task_fn(move |_: ()| {
        let c = c2.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            2
        }
    }));

    let t3 = dag.add_task(task_fn(move |_: ()| {
        let c = c3.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            3
        }
    }));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // All three tasks should have run
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    assert_eq!(dag.get(t1).unwrap(), 1);
    assert_eq!(dag.get(t2).unwrap(), 2);
    assert_eq!(dag.get(t3).unwrap(), 3);
}

#[tokio::test]
async fn test_task_fn_with_captured_state() {
    let dag = DagRunner::new();

    let multiplier = 7;
    let base = dag.add_task(task_fn(|_: ()| async { 5 }));
    let scaled = dag
        .add_task(task_fn(move |x: i32| async move { x * multiplier }))
        .depends_on(base);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(scaled).unwrap(), 35);
}
