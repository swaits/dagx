//! Tests for execution correctness and parallelism

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_layers_execute_in_parallel() -> DagResult<()> {
    // Test that tasks in the same topological layer execute in parallel
    let dag = DagRunner::new();

    let layer_timing = Arc::new(Mutex::new(Vec::new()));

    // Layer 0: 3 independent sources
    let sources: Vec<_> = (0..3)
        .map(|i| {
            let timing = layer_timing.clone();
            dag.add_task(task_fn(move |_: ()| {
                let timing = timing.clone();
                async move {
                    let start = Instant::now();
                    timing.lock().unwrap().push((format!("L0_{}", i), start));
                    sleep(Duration::from_millis(20)).await;
                    i * 10
                }
            }))
        })
        .collect();

    // Layer 1: Tasks depending on layer 0
    let layer1: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let timing = layer_timing.clone();
            dag.add_task(task_fn(move |x: i32| {
                let timing = timing.clone();
                async move {
                    let start = Instant::now();
                    timing.lock().unwrap().push((format!("L1_{}", i), start));
                    sleep(Duration::from_millis(20)).await;
                    x + 1
                }
            }))
            .depends_on(source)
        })
        .collect();

    // Layer 2: Final aggregation
    let final_task = {
        let timing = layer_timing.clone();
        dag.add_task(task_fn(move |(a, b, c): (i32, i32, i32)| {
            let timing = timing.clone();
            async move {
                let start = Instant::now();
                timing.lock().unwrap().push(("L2_final".to_string(), start));
                a + b + c
            }
        }))
        .depends_on((&layer1[0], &layer1[1], &layer1[2]))
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(final_task)?, 33); // (0+1) + (10+1) + (20+1)

    // Analyze timing
    let timing = layer_timing.lock().unwrap();

    // Get layer 0 start times
    let l0_times: Vec<Instant> = (0..3)
        .map(|i| {
            timing
                .iter()
                .find(|(name, _)| name == &format!("L0_{}", i))
                .map(|(_, t)| *t)
                .unwrap()
        })
        .collect();

    // Get layer 1 start times
    let l1_times: Vec<Instant> = (0..3)
        .map(|i| {
            timing
                .iter()
                .find(|(name, _)| name == &format!("L1_{}", i))
                .map(|(_, t)| *t)
                .unwrap()
        })
        .collect();

    // Layer 0 tasks should all start nearly simultaneously
    let l0_spread = l0_times
        .iter()
        .max()
        .unwrap()
        .duration_since(*l0_times.iter().min().unwrap());
    assert!(
        l0_spread < Duration::from_millis(5),
        "Layer 0 tasks should start together, spread was {:?}",
        l0_spread
    );

    // Layer 1 tasks should also start nearly simultaneously (after layer 0 completes)
    let l1_spread = l1_times
        .iter()
        .max()
        .unwrap()
        .duration_since(*l1_times.iter().min().unwrap());
    assert!(
        l1_spread < Duration::from_millis(5),
        "Layer 1 tasks should start together, spread was {:?}",
        l1_spread
    );

    // Layer 1 should start after layer 0
    let earliest_l1 = *l1_times.iter().min().unwrap();
    let earliest_l0 = *l0_times.iter().min().unwrap();
    assert!(
        earliest_l1 >= earliest_l0 + Duration::from_millis(15),
        "Layer 1 should start after layer 0 completes"
    );

    Ok(())
}

#[tokio::test]
async fn test_completion_order_matches_dependencies() -> DagResult<()> {
    // Verify that tasks complete in dependency order
    let dag = DagRunner::new();

    let completion_order = Arc::new(Mutex::new(Vec::new()));

    // Create a complex DAG with clear ordering requirements
    let a = {
        let order = completion_order.clone();
        dag.add_task(task_fn(move |_: ()| {
            let order = order.clone();
            async move {
                sleep(Duration::from_millis(10)).await;
                order.lock().unwrap().push("A");
                1
            }
        }))
    };

    let b = {
        let order = completion_order.clone();
        dag.add_task(task_fn(move |_: ()| {
            let order = order.clone();
            async move {
                sleep(Duration::from_millis(5)).await;
                order.lock().unwrap().push("B");
                2
            }
        }))
    };

    let c = {
        let order = completion_order.clone();
        dag.add_task(task_fn(move |(x, y): (i32, i32)| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("C");
                x + y
            }
        }))
        .depends_on((&a, &b))
    };

    let d = {
        let order = completion_order.clone();
        dag.add_task(task_fn(move |x: i32| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("D");
                x * 2
            }
        }))
        .depends_on(c)
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(d)?, 6); // (1 + 2) * 2

    // Check completion order
    let order = completion_order.lock().unwrap();

    // A and B can complete in any order (parallel)
    assert!(order.contains(&"A"));
    assert!(order.contains(&"B"));

    // C must come after both A and B
    let c_pos = order.iter().position(|name| *name == "C").unwrap();
    let a_pos = order.iter().position(|name| *name == "A").unwrap();
    let b_pos = order.iter().position(|name| *name == "B").unwrap();
    assert!(c_pos > a_pos, "C should complete after A");
    assert!(c_pos > b_pos, "C should complete after B");

    // D must come after C
    let d_pos = order.iter().position(|name| *name == "D").unwrap();
    assert!(d_pos > c_pos, "D should complete after C");

    Ok(())
}

#[tokio::test]
async fn test_dependent_tasks_respect_ordering() -> DagResult<()> {
    // Prove that dependent tasks wait for their dependencies
    let dag = DagRunner::new();

    let execution_order = Arc::new(Mutex::new(Vec::new()));

    // Create a chain: A -> B -> C -> D
    let a = {
        let order = execution_order.clone();
        dag.add_task(task_fn(move |_: ()| {
            let order = order.clone();
            async move {
                sleep(Duration::from_millis(20)).await;
                order.lock().unwrap().push("A");
                1
            }
        }))
    };

    let b = {
        let order = execution_order.clone();
        dag.add_task(task_fn(move |x: i32| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("B");
                sleep(Duration::from_millis(10)).await;
                x + 1
            }
        }))
        .depends_on(&a)
    };

    let c = {
        let order = execution_order.clone();
        dag.add_task(task_fn(move |x: i32| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("C");
                sleep(Duration::from_millis(10)).await;
                x + 1
            }
        }))
        .depends_on(b)
    };

    let d = {
        let order = execution_order.clone();
        dag.add_task(task_fn(move |x: i32| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("D");
                x + 1
            }
        }))
        .depends_on(c)
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify results
    assert_eq!(dag.get(a)?, 1);
    assert_eq!(dag.get(b)?, 2);
    assert_eq!(dag.get(c)?, 3);
    assert_eq!(dag.get(d)?, 4);

    // Verify execution order
    let order = execution_order.lock().unwrap();
    let expected = vec!["A", "B", "C", "D"];
    assert_eq!(order.as_slice(), expected.as_slice());

    Ok(())
}

#[tokio::test]
async fn test_independent_tasks_dont_wait() -> DagResult<()> {
    // Prove that independent tasks don't wait for each other
    let dag = DagRunner::new();

    let task_started = Arc::new(Mutex::new(Vec::new()));
    let task_finished = Arc::new(Mutex::new(Vec::new()));

    // Create tasks with different durations
    let fast1 = {
        let started = task_started.clone();
        let finished = task_finished.clone();
        dag.add_task(task_fn(move |_: ()| {
            let started = started.clone();
            let finished = finished.clone();
            async move {
                started.lock().unwrap().push(("fast1", Instant::now()));
                sleep(Duration::from_millis(10)).await;
                finished.lock().unwrap().push(("fast1", Instant::now()));
                "fast1"
            }
        }))
    };

    let slow = {
        let started = task_started.clone();
        let finished = task_finished.clone();
        dag.add_task(task_fn(move |_: ()| {
            let started = started.clone();
            let finished = finished.clone();
            async move {
                started.lock().unwrap().push(("slow", Instant::now()));
                sleep(Duration::from_millis(100)).await;
                finished.lock().unwrap().push(("slow", Instant::now()));
                "slow"
            }
        }))
    };

    let fast2 = {
        let started = task_started.clone();
        let finished = task_finished.clone();
        dag.add_task(task_fn(move |_: ()| {
            let started = started.clone();
            let finished = finished.clone();
            async move {
                started.lock().unwrap().push(("fast2", Instant::now()));
                sleep(Duration::from_millis(10)).await;
                finished.lock().unwrap().push(("fast2", Instant::now()));
                "fast2"
            }
        }))
    };

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Check results
    assert_eq!(dag.get(fast1)?, "fast1");
    assert_eq!(dag.get(slow)?, "slow");
    assert_eq!(dag.get(fast2)?, "fast2");

    // Verify that fast tasks finished before slow task
    let finished = task_finished.lock().unwrap();
    let fast1_finish = finished.iter().find(|x| x.0 == "fast1").unwrap().1;
    let fast2_finish = finished.iter().find(|x| x.0 == "fast2").unwrap().1;
    let slow_finish = finished.iter().find(|x| x.0 == "slow").unwrap().1;

    assert!(
        fast1_finish < slow_finish,
        "fast1 should finish before slow"
    );
    assert!(
        fast2_finish < slow_finish,
        "fast2 should finish before slow"
    );

    Ok(())
}
