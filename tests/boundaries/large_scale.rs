//! Large scale stress tests with thousands of nodes and edges

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_10000_independent_tasks() -> DagResult<()> {
    // Test with 10,000 independent tasks
    let dag = DagRunner::new();

    let counter = Arc::new(AtomicUsize::new(0));

    let tasks: Vec<_> = (0..10_000)
        .map(|i| {
            let counter = counter.clone();
            dag.add_task(task_fn(move |_: ()| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    i
                }
            }))
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify all tasks completed
    assert_eq!(counter.load(Ordering::Relaxed), 10_000);

    // Spot check some results
    assert_eq!(dag.get(&tasks[0])?, 0);
    assert_eq!(dag.get(&tasks[9_999])?, 9_999);
    assert_eq!(dag.get(&tasks[5_000])?, 5_000);

    Ok(())
}

#[tokio::test]
async fn test_5000_node_pyramid() -> DagResult<()> {
    // Create a pyramid structure with ~5000 nodes
    // Layer sizes: 2048 -> 512 -> 128 -> 32 -> 8 -> 2 -> 1
    let dag = DagRunner::new();

    // Bottom layer: 2048 nodes - convert to TaskHandles
    let mut current_layer: Vec<dagx::TaskHandle<i32>> = (0..2048)
        .map(|i| {
            let task = dag.add_task(task_fn(move |_: ()| async move {
                if i < 1024 {
                    1
                } else {
                    0
                }
            }));
            (&task).into()
        })
        .collect();

    let layer_sizes = vec![512, 128, 32, 8, 2];

    for layer_size in layer_sizes {
        let mut next_layer = Vec::with_capacity(layer_size);
        let group_size = current_layer.len() / layer_size;

        for i in 0..layer_size {
            let start = i * group_size;
            let deps: Vec<_> = current_layer[start..start + group_size.min(4)]
                .iter()
                .take(4)
                .collect();

            let task = match deps.len() {
                1 => dag
                    .add_task(task_fn(|a: i32| async move { a }))
                    .depends_on(deps[0]),
                2 => dag
                    .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
                    .depends_on((deps[0], deps[1])),
                3 => dag
                    .add_task(task_fn(
                        |(a, b, c): (i32, i32, i32)| async move { a + b + c },
                    ))
                    .depends_on((deps[0], deps[1], deps[2])),
                4 => dag
                    .add_task(task_fn(|(a, b, c, d): (i32, i32, i32, i32)| async move {
                        a + b + c + d
                    }))
                    .depends_on((deps[0], deps[1], deps[2], deps[3])),
                _ => unreachable!(),
            };
            next_layer.push(task);
        }
        current_layer = next_layer;
    }

    // Final layer combines the last 2
    let root = dag
        .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
        .depends_on((&current_layer[0], &current_layer[1]));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // We created 1024 ones and 1024 zeros at the bottom
    // The sum should propagate up to 1024
    let result = dag.get(root)?;
    assert!(result > 0, "Expected positive result, got {}", result);
    Ok(())
}

#[tokio::test]
async fn test_wide_dag_1000_sources_1000_sinks() -> DagResult<()> {
    // Test a very wide DAG with 1000 sources and 1000 sinks
    let dag = DagRunner::new();

    // Create 1000 source tasks
    let sources: Vec<_> = (0..1000)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i % 10 })))
        .collect();

    // Create 1000 sink tasks, each depending on 3 random sources
    let sinks: Vec<_> = (0..1000)
        .map(|i| {
            let idx1 = (i * 7) % 1000;
            let idx2 = (i * 13) % 1000;
            let idx3 = (i * 17) % 1000;

            dag.add_task(task_fn(move |(a, b, c): (i32, i32, i32)| async move {
                a + b + c + i as i32
            }))
            .depends_on((&sources[idx1], &sources[idx2], &sources[idx3]))
        })
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify some sinks completed correctly
    let result0 = dag.get(sinks[0])?;
    let result500 = dag.get(sinks[500])?;
    let result999 = dag.get(sinks[999])?;

    assert!(result0 >= 0);
    assert!(result500 >= 500);
    assert!(result999 >= 999);

    Ok(())
}

#[tokio::test]
async fn test_10000_node_linear_chain_segments() -> DagResult<()> {
    // Test 10000 nodes arranged as 100 parallel chains of 100 nodes each
    let dag = DagRunner::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut chain_ends = Vec::new();

    for chain_id in 0..100 {
        let first = dag.add_task(task_fn({
            let counter = counter.clone();
            move |_: ()| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    chain_id * 100
                }
            }
        }));
        let mut current: dagx::TaskHandle<i32> = (&first).into();

        for _i in 1..100 {
            current = dag
                .add_task(task_fn({
                    let counter = counter.clone();
                    move |prev: i32| {
                        let counter = counter.clone();
                        async move {
                            counter.fetch_add(1, Ordering::Relaxed);
                            prev + 1
                        }
                    }
                }))
                .depends_on(current);
        }

        chain_ends.push(current);
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // All 10,000 tasks should have executed
    assert_eq!(counter.load(Ordering::Relaxed), 10_000);

    // Each chain should end with start + 99
    for (chain_id, &chain_end) in chain_ends.iter().enumerate() {
        let expected = chain_id as i32 * 100 + 99;
        assert_eq!(dag.get(chain_end)?, expected);
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(tarpaulin, ignore)] // This test is very resource intensive
async fn test_100000_nodes_stress() -> DagResult<()> {
    // Ultimate stress test: 100,000 nodes
    let dag = DagRunner::new();
    let completed = Arc::new(AtomicUsize::new(0));

    // Create 100,000 tasks in batches
    let batch_size = 1000;
    let num_batches = 100;

    for batch in 0..num_batches {
        let _: Vec<_> = (0..batch_size)
            .map(|i| {
                let completed = completed.clone();
                let task_id = batch * batch_size + i;
                dag.add_task(task_fn(move |_: ()| {
                    let completed = completed.clone();
                    async move {
                        completed.fetch_add(1, Ordering::Relaxed);
                        task_id
                    }
                }))
            })
            .collect();
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(completed.load(Ordering::Relaxed), 100_000);
    Ok(())
}
