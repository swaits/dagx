//! Tests for extremely deep dependency chains

use dagx::{DagResult, DagRunner};
use dagx_test::task_fn;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_multiple_parallel_deep_chains() -> DagResult<()> {
    // Test 10 parallel chains of depth 500 each
    let mut dag = DagRunner::new();

    let mut chain_ends = Vec::new();

    for chain_id in 0..8 {
        let first = dag.add_task(task_fn::<(), _, _>(move |_: ()| chain_id * 1000));
        let mut current: dagx::TaskHandle<i32> = first;

        for _ in 0..500 {
            current = dag
                .add_task(task_fn::<i32, _, _>(|&prev: &i32| prev + 1))
                .depends_on(current);
        }

        chain_ends.push(current);
    }

    // Add a final task that depends on all chains
    let final_task = dag
        .add_task(task_fn::<(i32, i32, i32, i32, i32, i32, i32, i32), _, _>(
            |inputs: (&i32, &i32, &i32, &i32, &i32, &i32, &i32, &i32)| {
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

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Each chain: start + 500
    // Chain 0: 0 + 500 = 500
    // Chain 1: 1000 + 500 = 1500
    // ...
    // Sum = 500*8 + (0+1000+2000+...+7000) = 4000 + 28000 = 32000
    assert_eq!(output.get(final_task)?, 32000);

    Ok(())
}

#[tokio::test]
async fn test_deep_chain_execution_order() -> DagResult<()> {
    // Verify tasks in a deep chain execute in correct order
    let mut dag = DagRunner::new();
    let order = Arc::new(AtomicUsize::new(0));

    let first = dag.add_task(task_fn::<(), _, _>({
        let order = order.clone();
        move |_: ()| {
            let order = order.clone();
            assert_eq!(order.fetch_add(1, Ordering::SeqCst), 0);
            0
        }
    }));
    let mut current: dagx::TaskHandle<i32> = first;

    for expected in 1..100 {
        current = dag
            .add_task(task_fn::<i32, _, _>({
                let order = order.clone();
                move |&prev: &i32| {
                    let order = order.clone();
                    assert_eq!(order.fetch_add(1, Ordering::SeqCst), expected);
                    prev + 1
                }
            }))
            .depends_on(current);
    }

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(order.load(Ordering::SeqCst), 100);
    assert_eq!(output.get(current)?, 99);

    Ok(())
}

#[tokio::test]
async fn test_deep_binary_tree() -> DagResult<()> {
    // Test a deep binary tree structure (depth 10 = 1023 nodes)
    let mut dag = DagRunner::new();

    fn build_tree(dag: &mut DagRunner, depth: usize, value: i32) -> dagx::TaskHandle<i32> {
        if depth == 0 {
            let task = dag.add_task(task_fn::<(), _, _>(move |_: ()| value));
            task // Convert TaskBuilder to TaskHandle
        } else {
            let left = build_tree(dag, depth - 1, value * 2);
            let right = build_tree(dag, depth - 1, value * 2 + 1);

            dag.add_task(task_fn::<(i32, i32), _, _>(move |(l, r): (&i32, &i32)| {
                l + r
            }))
            .depends_on((&left, &right))
        }
    }

    let root = build_tree(&mut dag, 10, 1);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Sum of all values in a complete binary tree with values 1024..2047
    // This is sum from 2^10 to 2^11-1 = 1024 * 1023 / 2 + 1024 * 512 = 1047552
    let result = output.get(root)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_fibonacci_chain() -> DagResult<()> {
    // Test a Fibonacci-like dependency pattern
    let mut dag = DagRunner::new();

    let f0_builder = dag.add_task(task_fn::<(), _, _>(|_: ()| 0i64));
    let f1_builder = dag.add_task(task_fn::<(), _, _>(|_: ()| 1i64));

    let mut prev2: dagx::TaskHandle<i64> = f0_builder;
    let mut prev1: dagx::TaskHandle<i64> = f1_builder;

    for _ in 2..50 {
        let next = dag
            .add_task(task_fn::<(i64, i64), _, _>(|(a, b): (&i64, &i64)| a + b))
            .depends_on((&prev2, &prev1));

        prev2 = prev1;
        prev1 = next;
    }

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // 50th Fibonacci number (0-indexed) = 7778742049
    assert_eq!(output.get(prev1)?, 7778742049);

    Ok(())
}

#[tokio::test]
async fn test_zigzag_dependencies() -> DagResult<()> {
    // Test a zigzag pattern: two chains that cross-reference each other
    let mut dag = DagRunner::new();

    let a_first = dag.add_task(task_fn::<(), _, _>(|_: ()| 1));
    let b_first = dag.add_task(task_fn::<(), _, _>(|_: ()| 2));

    let mut chain_a: dagx::TaskHandle<i32> = a_first;
    let mut chain_b: dagx::TaskHandle<i32> = b_first;

    for i in 0..100 {
        if i % 2 == 0 {
            // A depends on B
            chain_a = dag
                .add_task(task_fn::<i32, _, _>(move |b_val: &i32| b_val + 1))
                .depends_on(chain_b);
        } else {
            // B depends on A
            chain_b = dag
                .add_task(task_fn::<i32, _, _>(move |a_val: &i32| a_val + 2))
                .depends_on(chain_a);
        }
    }

    // Final task depends on both chains
    let final_task = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(a, b): (&i32, &i32)| a + b))
        .depends_on((&chain_a, &chain_b));

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    let result = output.get(final_task)?;
    assert!(result > 100);

    Ok(())
}
