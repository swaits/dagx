//! Butterfly pattern DAG tests

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_butterfly_network() -> DagResult<()> {
    let dag = DagRunner::new();

    // Simplified butterfly network - avoid tuples entirely
    // Layer 0: inputs
    let inputs: Vec<_> = (0..8)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i as f64 })))
        .collect();

    // Layer 1: butterfly operations (compute pairs separately)
    let mut layer1 = Vec::new();
    for i in 0..4 {
        let idx1 = i * 2;
        let idx2 = i * 2 + 1;
        // Sum operation
        layer1.push(
            dag.add_task(task_fn(|(a, b): (f64, f64)| async move { (a + b) / 2.0 }))
                .depends_on((&inputs[idx1], &inputs[idx2])),
        );
        // Difference operation
        layer1.push(
            dag.add_task(task_fn(|(a, b): (f64, f64)| async move { (a - b) / 2.0 }))
                .depends_on((&inputs[idx1], &inputs[idx2])),
        );
    }

    // Layer 2: crossover connections
    let layer2: Vec<_> = vec![
        dag.add_task(task_fn(|(a, b): (f64, f64)| async move { a + b }))
            .depends_on((&layer1[0], &layer1[4])),
        dag.add_task(task_fn(|(a, b): (f64, f64)| async move { a - b }))
            .depends_on((&layer1[1], &layer1[5])),
        dag.add_task(task_fn(|(a, b): (f64, f64)| async move { a + b }))
            .depends_on((&layer1[2], &layer1[6])),
        dag.add_task(task_fn(|(a, b): (f64, f64)| async move { a - b }))
            .depends_on((&layer1[3], &layer1[7])),
    ];

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Verify some outputs exist
    for task in &layer2 {
        let _ = dag.get(task)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_recursive_butterfly() -> DagResult<()> {
    let dag = DagRunner::new();

    fn build_butterfly(
        dag: &DagRunner,
        inputs: Vec<dagx::TaskHandle<i32>>,
    ) -> Vec<dagx::TaskHandle<i32>> {
        if inputs.len() <= 2 {
            return inputs;
        }

        let mid = inputs.len() / 2;
        let mut outputs = Vec::new();

        // Butterfly connections
        for i in 0..mid {
            let sum = dag
                .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
                .depends_on((&inputs[i], &inputs[i + mid]));

            let diff = dag
                .add_task(task_fn(|(a, b): (i32, i32)| async move { (a - b).abs() }))
                .depends_on((&inputs[i], &inputs[i + mid]));

            outputs.push(sum);
            outputs.push(diff);
        }

        outputs
    }

    // Create 4 inputs
    let inputs: Vec<dagx::TaskHandle<i32>> = vec![10, 20, 30, 40]
        .into_iter()
        .map(|val| {
            let task = dag.add_task(task_fn(move |_: ()| async move { val }));
            (&task).into()
        })
        .collect();

    let outputs = build_butterfly(&dag, inputs);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(outputs.len(), 4);
    for output in outputs {
        let _ = dag.get(output)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_benes_network() -> DagResult<()> {
    // Simplified Benes network - avoid tuples
    let dag = DagRunner::new();

    // 4 inputs
    let inputs: Vec<_> = (0..4)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { 1 << i }))) // Powers of 2
        .collect();

    // First stage: sort pairs
    let stage1: Vec<_> = vec![
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a.min(b) }))
            .depends_on((&inputs[0], &inputs[1])),
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a.max(b) }))
            .depends_on((&inputs[0], &inputs[1])),
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a.min(b) }))
            .depends_on((&inputs[2], &inputs[3])),
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a.max(b) }))
            .depends_on((&inputs[2], &inputs[3])),
    ];

    // Second stage with crossover
    let outputs: Vec<_> = vec![
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a | b }))
            .depends_on((&stage1[0], &stage1[2])),
        dag.add_task(task_fn(|(a, b): (i32, i32)| async move { a | b }))
            .depends_on((&stage1[1], &stage1[3])),
    ];

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    for output in outputs {
        let _ = dag.get(output)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_shuffle_exchange_network() -> DagResult<()> {
    let dag = DagRunner::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Create 8 inputs
    let inputs: Vec<_> = (0..8)
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

    // Shuffle stage: bit rotation
    let shuffled: Vec<_> = (0..8)
        .map(|i| {
            // Shuffle pattern: rotate bits
            let source = ((i << 1) | (i >> 2)) & 0x7;
            dag.add_task(task_fn(move |x: i32| async move { x }))
                .depends_on(&inputs[source])
        })
        .collect();

    // Exchange stage: adjacent pairs
    let exchanged: Vec<_> = (0..4)
        .flat_map(|i| {
            let idx = i * 2;
            vec![
                dag.add_task(task_fn(|(a, b): (i32, i32)| async move {
                    if a > b {
                        b
                    } else {
                        a
                    }
                }))
                .depends_on((&shuffled[idx], &shuffled[idx + 1])),
                dag.add_task(task_fn(|(a, b): (i32, i32)| async move {
                    if a > b {
                        a
                    } else {
                        b
                    }
                }))
                .depends_on((&shuffled[idx], &shuffled[idx + 1])),
            ]
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(counter.load(Ordering::Relaxed), 8);

    for task in exchanged {
        let _ = dag.get(task)?;
    }

    Ok(())
}
