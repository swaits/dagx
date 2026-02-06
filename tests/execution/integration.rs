//! Integration tests for end-to-end functionality

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, Task};
use futures::FutureExt;
use std::time::Instant;
use tokio::time::{sleep, Duration};

// === Shared Test Tasks ===

struct LoadValue {
    value: i32,
}
impl LoadValue {
    fn new(value: i32) -> Self {
        Self { value }
    }
}
#[dagx::task]
impl LoadValue {
    async fn run(&self) -> i32 {
        self.value // Read-only state: use &self
    }
}

struct Multiply {
    factor: i32,
}
impl Multiply {
    fn new(factor: i32) -> Self {
        Self { factor }
    }
}
#[dagx::task]
impl Multiply {
    async fn run(&self, input: &i32) -> i32 {
        input * self.factor // Read-only state: use &self
    }
}

struct Add;
#[dagx::task]
impl Add {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b // Stateless: no self needed
    }
}

// === Integration Tests ===

#[tokio::test]
async fn integration_test_etl_pipeline() -> DagResult<()> {
    // Simulate an Extract-Transform-Load pipeline
    let dag = DagRunner::new();

    // Extract: Load data from sources
    let source1 = dag.add_task(LoadValue::new(10));
    let source2 = dag.add_task(LoadValue::new(20));
    let source3 = dag.add_task(LoadValue::new(30));

    // Transform: Process each source
    let transform1 = dag.add_task(Multiply::new(2)).depends_on(source1);
    let transform2 = dag.add_task(Multiply::new(3)).depends_on(source2);
    let transform3 = dag.add_task(Multiply::new(4)).depends_on(source3);

    // Load: Aggregate results
    let combine12 = dag.add_task(Add).depends_on((&transform1, &transform2));
    let final_result = dag.add_task(Add).depends_on((&combine12, &transform3));

    // Execute pipeline
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify
    let result = dag.get(final_result)?;
    assert_eq!(result, (10 * 2) + (20 * 3) + (30 * 4)); // 20 + 60 + 120 = 200

    Ok(())
}

#[tokio::test]
async fn integration_test_multi_layer_dag() -> DagResult<()> {
    // Complex multi-layer DAG
    let dag = DagRunner::new();

    // Layer 1: Sources
    let a = dag.add_task(LoadValue::new(1));
    let b = dag.add_task(LoadValue::new(2));
    let c = dag.add_task(LoadValue::new(3));

    // Layer 2: First transformations
    let ab = dag.add_task(Add).depends_on((&a, &b));
    let bc = dag.add_task(Add).depends_on((&b, &c));

    // Layer 3: Second transformations
    let double_ab = dag.add_task(Multiply::new(2)).depends_on(ab);
    let double_bc = dag.add_task(Multiply::new(2)).depends_on(bc);

    // Layer 4: Final result
    let final_result = dag.add_task(Add).depends_on((&double_ab, &double_bc));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(final_result)?;
    // (1+2)*2 + (2+3)*2 = 6 + 10 = 16
    assert_eq!(result, 16);

    Ok(())
}

#[tokio::test]
async fn integration_test_multiple_sinks() -> DagResult<()> {
    // DAG with multiple output sinks
    let dag = DagRunner::new();

    let source = dag.add_task(LoadValue::new(10));

    // Multiple independent branches
    let branch1 = dag.add_task(Multiply::new(2)).depends_on(source);
    let branch2 = dag.add_task(Multiply::new(3)).depends_on(source);
    let branch3 = dag.add_task(Multiply::new(4)).depends_on(source);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // All branches should complete
    assert_eq!(dag.get(branch1)?, 20);
    assert_eq!(dag.get(branch2)?, 30);
    assert_eq!(dag.get(branch3)?, 40);

    Ok(())
}

#[tokio::test]
async fn integration_test_diamond_cascade() -> DagResult<()> {
    // Multiple diamond patterns in sequence
    let dag = DagRunner::new();

    // Diamond 1
    let source1 = dag.add_task(LoadValue::new(10));
    let left1 = dag.add_task(Multiply::new(2)).depends_on(source1);
    let right1 = dag.add_task(Multiply::new(3)).depends_on(source1);
    let sink1 = dag.add_task(Add).depends_on((&left1, &right1));

    // Diamond 2 (depends on Diamond 1)
    let left2 = dag.add_task(Multiply::new(2)).depends_on(sink1);
    let right2 = dag.add_task(Multiply::new(3)).depends_on(sink1);
    let sink2 = dag.add_task(Add).depends_on((&left2, &right2));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(sink2)?;
    // Diamond 1: (10*2) + (10*3) = 50
    // Diamond 2: (50*2) + (50*3) = 250
    assert_eq!(result, 250);

    Ok(())
}

#[tokio::test]
async fn integration_test_wide_and_deep() -> DagResult<()> {
    // Combination of wide (parallel) and deep (sequential) patterns
    let dag = DagRunner::new();

    // Wide: 5 parallel sources
    let sources: Vec<_> = (0..5)
        .map(|i| dag.add_task(LoadValue::new(i * 10)))
        .collect();

    // Deep: Chain each source through 3 transformations
    let mut final_nodes = Vec::new();
    for source in &sources {
        let step1 = dag.add_task(Multiply::new(2)).depends_on(source);
        let step2 = dag
            .add_task(task_fn(|x: i32| async move { x + 5 }))
            .depends_on(step1);
        let step3 = dag.add_task(Multiply::new(3)).depends_on(step2);
        final_nodes.push(step3);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify each chain
    for (i, node) in final_nodes.iter().enumerate() {
        let expected = ((i as i32 * 10) * 2 + 5) * 3;
        assert_eq!(dag.get(node)?, expected);
    }

    Ok(())
}

#[tokio::test]
async fn integration_test_error_recovery() -> DagResult<()> {
    // Test that DAG handles normal execution gracefully
    let dag = DagRunner::new();

    let source = dag.add_task(LoadValue::new(42));
    let transform = dag.add_task(Multiply::new(2)).depends_on(source);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Should be able to access results
    assert_eq!(dag.get(transform)?, 84);

    // Getting valid task should return ok
    let result = dag.get(transform);
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn integration_test_large_scale() -> DagResult<()> {
    // Large-scale integration test
    let dag = DagRunner::new();

    // Create 100 sources
    let sources: Vec<_> = (0..100).map(|i| dag.add_task(LoadValue::new(i))).collect();

    // Create 50 aggregation tasks (each combines 2 sources)
    let aggregations: Vec<_> = (0..50)
        .map(|i| {
            dag.add_task(Add)
                .depends_on((&sources[i * 2], &sources[i * 2 + 1]))
        })
        .collect();

    // Create 25 second-level aggregations
    let second_level: Vec<_> = (0..25)
        .map(|i| {
            dag.add_task(Add)
                .depends_on((&aggregations[i * 2], &aggregations[i * 2 + 1]))
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify some results
    assert_eq!(dag.get(aggregations[0])?, 1); // First two
    assert_eq!(dag.get(second_level[0])?, 1 + (2 + 3)); // First four

    Ok(())
}

#[tokio::test]
async fn integration_test_reused_values() -> DagResult<()> {
    // Test that a single source can be used by multiple consumers
    let dag = DagRunner::new();

    let source = dag.add_task(LoadValue::new(100));

    // Use source in multiple different ways
    let double = dag.add_task(Multiply::new(2)).depends_on(source);
    let triple = dag.add_task(Multiply::new(3)).depends_on(source);
    let quadruple = dag.add_task(Multiply::new(4)).depends_on(source);

    // Combine some of the results
    let combined = dag.add_task(Add).depends_on((&double, &triple));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(double)?, 200);
    assert_eq!(dag.get(triple)?, 300);
    assert_eq!(dag.get(quadruple)?, 400);
    assert_eq!(dag.get(combined)?, 500);

    Ok(())
}

#[tokio::test]
async fn integration_test_parallel_execution_timing() -> DagResult<()> {
    // Test that independent tasks actually run in parallel by measuring execution time
    let dag = DagRunner::new();

    let sleep_duration = Duration::from_millis(100);
    let num_parallel_tasks = 5;

    // Create 5 independent tasks that each sleep for 100ms
    let tasks: Vec<_> = (0..num_parallel_tasks)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                sleep(sleep_duration).await;
                i
            }))
        })
        .collect();

    let start = Instant::now();
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
    let elapsed = start.elapsed();

    // Verify all tasks completed
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
    }

    // If tasks ran sequentially, it would take 5 * 100ms = 500ms
    // With parallelism, it should take ~100ms (plus overhead)
    // We'll check it's less than 300ms (generous margin for CI environments)
    assert!(
        elapsed < Duration::from_millis(300),
        "Parallel execution took {:?}, expected < 300ms (sequential would be 500ms)",
        elapsed
    );

    Ok(())
}

#[tokio::test]
async fn integration_test_parallel_vs_sequential_timing() -> DagResult<()> {
    // Test mixed parallel and sequential execution patterns
    let dag = DagRunner::new();

    let sleep_duration = Duration::from_millis(50);

    // Layer 1: 4 parallel tasks (should take ~50ms)
    let parallel_tasks: Vec<_> = (0..4)
        .map(|i| {
            dag.add_task(task_fn(move |_: ()| async move {
                sleep(sleep_duration).await;
                i * 10
            }))
        })
        .collect();

    // Layer 2: Sequential chain of 3 tasks depending on the first parallel task
    // (should add ~150ms)
    let seq1 = dag
        .add_task(task_fn(move |x: i32| async move {
            sleep(sleep_duration).await;
            x + 1
        }))
        .depends_on(parallel_tasks[0]);

    let seq2 = dag
        .add_task(task_fn(move |x: i32| async move {
            sleep(sleep_duration).await;
            x + 1
        }))
        .depends_on(seq1);

    let seq3 = dag
        .add_task(task_fn(move |x: i32| async move {
            sleep(sleep_duration).await;
            x + 1
        }))
        .depends_on(seq2);

    let start = Instant::now();
    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
    let elapsed = start.elapsed();

    // Verify results
    assert_eq!(dag.get(seq3)?, 3); // 0 + 1 + 1 + 1

    // Expected time: ~200ms (4 layers of 50ms each: parallel layer + 3 sequential)
    // If everything ran sequentially: 7 * 50ms = 350ms
    // We'll check it's less than 300ms
    assert!(
        elapsed < Duration::from_millis(300),
        "Execution took {:?}, expected < 300ms (fully sequential would be 350ms)",
        elapsed
    );

    // Also verify it took more than just the parallel layer
    assert!(
        elapsed >= Duration::from_millis(150),
        "Execution took {:?}, expected >= 150ms (should include sequential chain)",
        elapsed
    );

    Ok(())
}
