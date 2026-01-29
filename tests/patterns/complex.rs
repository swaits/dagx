//! Complex DAG pattern tests

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_workflow_with_conditional_paths() -> DagResult<()> {
    let dag = DagRunner::new();

    // Input validation
    let input = dag.add_task(task_fn(|_: ()| async { 42 }));

    let validate = dag
        .add_task(task_fn(|x: i32| async move {
            if x > 0 && x < 100 {
                Ok(x)
            } else {
                Err("Invalid input")
            }
        }))
        .depends_on(input);

    // Process valid input
    let process = dag
        .add_task(task_fn(|result: Result<i32, &str>| async move {
            match result {
                Ok(x) => x * 2,
                Err(_) => -1,
            }
        }))
        .depends_on(validate);

    // Parallel enhancement steps
    let enhance1 = dag
        .add_task(task_fn(|x: i32| async move { x + 10 }))
        .depends_on(process);

    let enhance2 = dag
        .add_task(task_fn(|x: i32| async move { x * 3 }))
        .depends_on(process);

    let enhance3 = dag
        .add_task(task_fn(|x: i32| async move { x - 5 }))
        .depends_on(process);

    // Combine enhancements
    let combine = dag
        .add_task(task_fn(|(e1, e2, e3): (i32, i32, i32)| async move {
            vec![e1, e2, e3]
        }))
        .depends_on((&enhance1, &enhance2, &enhance3));

    // Final aggregation
    let final_result = dag
        .add_task(task_fn(|values: Vec<i32>| async move {
            values.iter().sum::<i32>()
        }))
        .depends_on(combine);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(process)?, 84);
    let result = dag.get(final_result)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_map_reduce_pattern() -> DagResult<()> {
    let dag = DagRunner::new();

    // Map phase: process 16 inputs in parallel
    let mappers: Vec<dagx::TaskHandle<Vec<(&str, usize)>>> = (0..16)
        .map(|i| {
            let task = dag.add_task(task_fn(move |_: ()| async move {
                // Simulate word count
                vec![("word1", i % 3), ("word2", i % 5), ("word3", i % 7)]
            }));
            task.into()
        })
        .collect();

    // Shuffle phase: group by key (simplified - just 4 reducers)
    let shuffled: Vec<_> = (0..4)
        .map(|reducer_id| {
            // Each reducer gets data from 4 mappers
            let mapper_indices: Vec<usize> = (0..4).map(|i| reducer_id * 4 + i).collect();

            let deps: Vec<_> = mapper_indices.iter().map(|&i| mappers[i]).collect();

            match deps.len() {
                4 => dag
                    .add_task(task_fn(
                        move |inputs: (
                            Vec<(&str, usize)>,
                            Vec<(&str, usize)>,
                            Vec<(&str, usize)>,
                            Vec<(&str, usize)>,
                        )| async move {
                            let mut combined = Vec::new();
                            combined.extend(inputs.0);
                            combined.extend(inputs.1);
                            combined.extend(inputs.2);
                            combined.extend(inputs.3);
                            combined
                        },
                    ))
                    .depends_on((&deps[0], &deps[1], &deps[2], &deps[3])),
                _ => unreachable!(),
            }
        })
        .collect();

    // Reduce phase
    let reducers: Vec<_> = shuffled
        .iter()
        .map(|shuffled_data| {
            dag.add_task(task_fn(|data: Vec<(&str, usize)>| async move {
                // Sum values by key
                let mut totals = std::collections::HashMap::new();
                for (key, value) in data {
                    *totals.entry(key).or_insert(0) += value;
                }
                totals.len()
            }))
            .depends_on(shuffled_data)
        })
        .collect();

    // Final aggregation
    let final_task = dag
        .add_task(task_fn(
            |(r1, r2, r3, r4): (usize, usize, usize, usize)| async move { r1 + r2 + r3 + r4 },
        ))
        .depends_on((&reducers[0], &reducers[1], &reducers[2], &reducers[3]));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(final_task)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_pipeline_with_feedback() -> DagResult<()> {
    let dag = DagRunner::new();
    let iteration_count = Arc::new(AtomicUsize::new(0));

    // Initial value
    let init = dag.add_task(task_fn(|_: ()| async { 1 }));

    // Stage 1: Transform
    let stage1 = dag
        .add_task(task_fn({
            let counter = iteration_count.clone();
            move |x: i32| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    x * 2
                }
            }
        }))
        .depends_on(init);

    // Stage 2: Filter
    let stage2 = dag
        .add_task(task_fn(|x: i32| async move {
            if x < 100 {
                Some(x)
            } else {
                None
            }
        }))
        .depends_on(stage1);

    // Stage 3: Accumulate
    let stage3 = dag
        .add_task(task_fn(|opt: Option<i32>| async move {
            opt.map(|x| x + 10).unwrap_or(0)
        }))
        .depends_on(stage2);

    // Note: Can't create actual cycles in DAG, but we simulate feedback
    // by having a separate "feedback" task that would feed back in a real system
    let _feedback = dag
        .add_task(task_fn(|x: i32| async move {
            // In a real feedback loop, this would feed back to stage1
            x / 2
        }))
        .depends_on(stage3);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(stage1)?, 2);
    assert_eq!(dag.get(stage3)?, 12);
    assert_eq!(iteration_count.load(Ordering::SeqCst), 1);

    Ok(())
}

#[tokio::test]
async fn test_scatter_gather_pattern() -> DagResult<()> {
    let dag = DagRunner::new();

    // Scatter: distribute work to multiple workers
    let source: TaskHandle<_> = dag
        .add_task(task_fn(|_: ()| async {
            vec![10, 20, 30, 40, 50, 60, 70, 80]
        }))
        .into();

    // Workers process chunks
    let workers: Vec<_> = (0..4)
        .map(|worker_id| {
            dag.add_task(task_fn(move |data: Vec<i32>| async move {
                // Each worker processes a chunk
                let chunk_size = data.len() / 4;
                let start = worker_id * chunk_size;
                let end = if worker_id == 3 {
                    data.len()
                } else {
                    start + chunk_size
                };

                data[start..end].iter().sum::<i32>()
            }))
            .depends_on(source)
        })
        .collect();

    // Gather: collect results from all workers
    let gather = dag
        .add_task(task_fn(
            |(w0, w1, w2, w3): (i32, i32, i32, i32)| async move { w0 + w1 + w2 + w3 },
        ))
        .depends_on((&workers[0], &workers[1], &workers[2], &workers[3]));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Sum of 10+20+30+40+50+60+70+80 = 360
    assert_eq!(dag.get(gather)?, 360);

    Ok(())
}

#[tokio::test]
async fn test_fork_join_with_barriers() -> DagResult<()> {
    let dag = DagRunner::new();
    let phase_counter = Arc::new(AtomicUsize::new(0));

    // Fork phase
    let parent = dag.add_task(task_fn({
        let counter = phase_counter.clone();
        move |_: ()| {
            let counter = counter.clone();
            async move {
                counter.store(1, Ordering::SeqCst);
                100
            }
        }
    }));

    // Multiple parallel paths
    let mut path1 = Vec::new();
    let parent_handle: dagx::TaskHandle<i32> = parent.into();
    let mut prev1 = parent_handle;
    for i in 0..3 {
        let counter = phase_counter.clone();
        let task = dag
            .add_task(task_fn(move |x: i32| {
                let counter = counter.clone();
                async move {
                    assert_eq!(counter.load(Ordering::SeqCst), 1);
                    x + i
                }
            }))
            .depends_on(prev1);
        path1.push(task);
        prev1 = task;
    }

    let mut path2 = Vec::new();
    let mut prev2 = parent_handle;
    for i in 0..3 {
        let counter = phase_counter.clone();
        let task = dag
            .add_task(task_fn(move |x: i32| {
                let counter = counter.clone();
                async move {
                    assert_eq!(counter.load(Ordering::SeqCst), 1);
                    x * 2 + i
                }
            }))
            .depends_on(prev2);
        path2.push(task);
        prev2 = task;
    }

    // Barrier: wait for all paths
    let barrier = dag
        .add_task(task_fn({
            let counter = phase_counter.clone();
            move |(p1, p2): (i32, i32)| {
                let counter = counter.clone();
                async move {
                    counter.store(2, Ordering::SeqCst);
                    p1 + p2
                }
            }
        }))
        .depends_on((&path1[2], &path2[2]));

    // Join phase
    let join = dag
        .add_task(task_fn({
            let counter = phase_counter.clone();
            move |x: i32| {
                let counter = counter.clone();
                async move {
                    assert_eq!(counter.load(Ordering::SeqCst), 2);
                    x
                }
            }
        }))
        .depends_on(barrier);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let _ = dag.get(join)?;
    assert_eq!(phase_counter.load(Ordering::SeqCst), 2);

    Ok(())
}
