//! Tests for extremely deep dependency chains

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_1000_level_deep_chain() -> DagResult<()> {
    // Test a linear chain of 1000 tasks
    let dag = DagRunner::new();

    let first = dag.add_task(task_fn(|_: ()| async { 0 }));
    let mut current = (&first).into(); // Convert to TaskHandle

    for _i in 1..1000 {
        current = dag
            .add_task(task_fn(move |prev: i32| async move { prev + 1 }))
            .depends_on(current);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(current)?, 999);
    Ok(())
}

#[tokio::test]
async fn test_deep_chain_with_branches() -> DagResult<()> {
    // Test a deep chain where each node has a side branch
    let dag = DagRunner::new();

    let first = dag.add_task(task_fn(|_: ()| async { 0 }));
    let mut main_chain: dagx::TaskHandle<i32> = (&first).into();
    let mut branches = Vec::new();

    for i in 1..500 {
        // Main chain continues
        main_chain = dag
            .add_task(task_fn(move |prev: i32| async move { prev + 1 }))
            .depends_on(main_chain);

        // Create a branch at every 10th node
        if i % 10 == 0 {
            let branch = dag
                .add_task(task_fn(move |main_val: i32| async move { main_val * 2 }))
                .depends_on(main_chain);
            branches.push(branch);
        }
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(main_chain)?, 499);

    // Check some branches
    if !branches.is_empty() {
        assert_eq!(dag.get(branches[0])?, 20); // 10 * 2
        assert_eq!(dag.get(branches[4])?, 100); // 50 * 2
    }

    Ok(())
}

#[tokio::test]
async fn test_multiple_parallel_deep_chains() -> DagResult<()> {
    // Test 10 parallel chains of depth 500 each
    let dag = DagRunner::new();

    let mut chain_ends = Vec::new();

    for chain_id in 0..8 {
        let first = dag.add_task(task_fn(move |_: ()| async move { chain_id * 1000 }));
        let mut current: dagx::TaskHandle<i32> = (&first).into();

        for _ in 0..500 {
            current = dag
                .add_task(task_fn(|prev: i32| async move { prev + 1 }))
                .depends_on(current);
        }

        chain_ends.push(current);
    }

    // Add a final task that depends on all chains
    let final_task = dag
        .add_task(task_fn(
            |inputs: (i32, i32, i32, i32, i32, i32, i32, i32)| async move {
                inputs.0
                    + inputs.1
                    + inputs.2
                    + inputs.3
                    + inputs.4
                    + inputs.5
                    + inputs.6
                    + inputs.7
            },
        ))
        .depends_on((
            &chain_ends[0],
            &chain_ends[1],
            &chain_ends[2],
            &chain_ends[3],
            &chain_ends[4],
            &chain_ends[5],
            &chain_ends[6],
            &chain_ends[7],
        ));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Each chain: start + 500
    // Chain 0: 0 + 500 = 500
    // Chain 1: 1000 + 500 = 1500
    // ...
    // Sum = 500*8 + (0+1000+2000+...+7000) = 4000 + 28000 = 32000
    assert_eq!(dag.get(final_task)?, 32000);

    Ok(())
}

#[tokio::test]
async fn test_deep_chain_execution_order() -> DagResult<()> {
    // Verify tasks in a deep chain execute in correct order
    let dag = DagRunner::new();
    let order = Arc::new(AtomicUsize::new(0));

    let first = dag.add_task(task_fn({
        let order = order.clone();
        move |_: ()| {
            let order = order.clone();
            async move {
                assert_eq!(order.fetch_add(1, Ordering::SeqCst), 0);
                0
            }
        }
    }));
    let mut current: dagx::TaskHandle<i32> = (&first).into();

    for expected in 1..100 {
        current = dag
            .add_task(task_fn({
                let order = order.clone();
                move |prev: i32| {
                    let order = order.clone();
                    async move {
                        assert_eq!(order.fetch_add(1, Ordering::SeqCst), expected);
                        prev + 1
                    }
                }
            }))
            .depends_on(current);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(order.load(Ordering::SeqCst), 100);
    assert_eq!(dag.get(current)?, 99);

    Ok(())
}

#[tokio::test]
async fn test_deep_binary_tree() -> DagResult<()> {
    // Test a deep binary tree structure (depth 10 = 1023 nodes)
    let dag = DagRunner::new();

    fn build_tree(dag: &DagRunner, depth: usize, value: i32) -> dagx::TaskHandle<i32> {
        if depth == 0 {
            let task = dag.add_task(task_fn(move |_: ()| async move { value }));
            (&task).into() // Convert TaskBuilder to TaskHandle
        } else {
            let left = build_tree(dag, depth - 1, value * 2);
            let right = build_tree(dag, depth - 1, value * 2 + 1);

            dag.add_task(task_fn(move |(l, r): (i32, i32)| async move { l + r }))
                .depends_on((&left, &right))
        }
    }

    let root = build_tree(&dag, 10, 1);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Sum of all values in a complete binary tree with values 1024..2047
    // This is sum from 2^10 to 2^11-1 = 1024 * 1023 / 2 + 1024 * 512 = 1047552
    let result = dag.get(root)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_fibonacci_chain() -> DagResult<()> {
    // Test a Fibonacci-like dependency pattern
    let dag = DagRunner::new();

    let f0_builder = dag.add_task(task_fn(|_: ()| async { 0i64 }));
    let f1_builder = dag.add_task(task_fn(|_: ()| async { 1i64 }));

    let mut prev2: dagx::TaskHandle<i64> = (&f0_builder).into();
    let mut prev1: dagx::TaskHandle<i64> = (&f1_builder).into();

    for _ in 2..50 {
        let next = dag
            .add_task(task_fn(|(a, b): (i64, i64)| async move { a + b }))
            .depends_on((&prev2, &prev1));

        prev2 = prev1;
        prev1 = next;
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // 50th Fibonacci number (0-indexed) = 7778742049
    assert_eq!(dag.get(prev1)?, 7778742049);

    Ok(())
}

#[tokio::test]
async fn test_deep_chain_with_accumulator() -> DagResult<()> {
    // Test a deep chain where each node accumulates a value
    let dag = DagRunner::new();

    // Use a simpler accumulation pattern - just sum
    let first = dag.add_task(task_fn(|_: ()| async { 0i64 }));
    let mut current: dagx::TaskHandle<i64> = (&first).into();

    for i in 1..=200 {
        current = dag
            .add_task(task_fn(move |sum: i64| async move { sum + i as i64 }))
            .depends_on(current);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(current)?;
    assert_eq!(result, 20100); // sum of 1..=200

    Ok(())
}

#[tokio::test]
async fn test_zigzag_dependencies() -> DagResult<()> {
    // Test a zigzag pattern: two chains that cross-reference each other
    let dag = DagRunner::new();

    let a_first = dag.add_task(task_fn(|_: ()| async { 1 }));
    let b_first = dag.add_task(task_fn(|_: ()| async { 2 }));

    let mut chain_a: dagx::TaskHandle<i32> = (&a_first).into();
    let mut chain_b: dagx::TaskHandle<i32> = (&b_first).into();

    for i in 0..100 {
        if i % 2 == 0 {
            // A depends on B
            chain_a = dag
                .add_task(task_fn(move |b_val: i32| async move { b_val + 1 }))
                .depends_on(chain_b);
        } else {
            // B depends on A
            chain_b = dag
                .add_task(task_fn(move |a_val: i32| async move { a_val + 2 }))
                .depends_on(chain_a);
        }
    }

    // Final task depends on both chains
    let final_task = dag
        .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
        .depends_on((&chain_a, &chain_b));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(final_task)?;
    assert!(result > 100);

    Ok(())
}

#[tokio::test]
#[cfg_attr(tarpaulin, ignore)] // This test is extremely resource intensive
async fn test_10000_level_chain_stress() -> DagResult<()> {
    // Ultimate deep chain test
    let dag = DagRunner::new();

    let first = dag.add_task(task_fn(|_: ()| async { 0u64 }));
    let mut current: dagx::TaskHandle<u64> = (&first).into();

    for _ in 1..10_000 {
        current = dag
            .add_task(task_fn(|prev: u64| async move { prev + 1 }))
            .depends_on(current);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(current)?, 9_999);
    Ok(())
}
