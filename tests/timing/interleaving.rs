//! Tests for forced execution interleaving patterns

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};
use futures::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_forced_interleaving_with_barriers() -> DagResult<()> {
    // Force specific interleaving using barriers
    let dag = DagRunner::new();
    let barrier = Arc::new(Barrier::new(3));
    let order = Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Three parallel tasks that synchronize at barrier
    let t1 = dag.add_task(task_fn({
        let barrier = barrier.clone();
        let order = order.clone();
        move |_: ()| {
            let barrier = barrier.clone();
            let order = order.clone();
            async move {
                order.lock().push("t1_start");
                barrier.wait().await;
                order.lock().push("t1_end");
                1
            }
        }
    }));

    let t2 = dag.add_task(task_fn({
        let barrier = barrier.clone();
        let order = order.clone();
        move |_: ()| {
            let barrier = barrier.clone();
            let order = order.clone();
            async move {
                order.lock().push("t2_start");
                barrier.wait().await;
                order.lock().push("t2_end");
                2
            }
        }
    }));

    let t3 = dag.add_task(task_fn({
        let barrier = barrier.clone();
        let order = order.clone();
        move |_: ()| {
            let barrier = barrier.clone();
            let order = order.clone();
            async move {
                order.lock().push("t3_start");
                barrier.wait().await;
                order.lock().push("t3_end");
                3
            }
        }
    }));

    // Collector
    let collector = dag
        .add_task(task_fn(
            |(a, b, c): (i32, i32, i32)| async move { a + b + c },
        ))
        .depends_on((t1, t2, t3));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(collector)?, 6);

    // Check interleaving pattern
    let events = order.lock().clone();
    assert_eq!(events.len(), 6);

    // All starts should come before all ends due to barrier
    let start_count = events[0..3]
        .iter()
        .filter(|e| e.ends_with("_start"))
        .count();
    assert_eq!(start_count, 3);

    Ok(())
}

#[tokio::test]
async fn test_alternating_execution_pattern() -> DagResult<()> {
    // Create an alternating execution pattern
    let dag = DagRunner::new();
    let turn = Arc::new(AtomicUsize::new(0));
    let events: Arc<parking_lot::Mutex<Vec<String>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Two chains that alternate execution
    let first_a = dag.add_task(task_fn({
        let turn = turn.clone();
        let events = events.clone();
        move |_: ()| {
            let turn = turn.clone();
            let events = events.clone();
            async move {
                while turn.load(Ordering::SeqCst) != 0 {
                    tokio::task::yield_now().await;
                }
                events.lock().push("A0".to_string());
                turn.store(1, Ordering::SeqCst);
                0
            }
        }
    }));
    let mut chain_a: dagx::TaskHandle<i32> = first_a.into();

    let first_b = dag.add_task(task_fn({
        let turn = turn.clone();
        let events = events.clone();
        move |_: ()| {
            let turn = turn.clone();
            let events = events.clone();
            async move {
                while turn.load(Ordering::SeqCst) != 1 {
                    tokio::task::yield_now().await;
                }
                events.lock().push("B0".to_string());
                turn.store(2, Ordering::SeqCst);
                0
            }
        }
    }));
    let mut chain_b: dagx::TaskHandle<i32> = first_b.into();

    for i in 1..5 {
        let turn_clone = turn.clone();
        let events_clone = events.clone();
        chain_a = dag
            .add_task(task_fn(move |x: i32| {
                let turn_clone = turn_clone.clone();
                let events_clone = events_clone.clone();
                async move {
                    while turn_clone.load(Ordering::SeqCst) != i * 2 {
                        tokio::task::yield_now().await;
                    }
                    events_clone.lock().push(format!("A{}", i));
                    turn_clone.store(i * 2 + 1, Ordering::SeqCst);
                    x + 1
                }
            }))
            .depends_on(chain_a);

        let turn_clone = turn.clone();
        let events_clone = events.clone();
        chain_b = dag
            .add_task(task_fn(move |x: i32| {
                let turn_clone = turn_clone.clone();
                let events_clone = events_clone.clone();
                async move {
                    while turn_clone.load(Ordering::SeqCst) != i * 2 + 1 {
                        tokio::task::yield_now().await;
                    }
                    events_clone.lock().push(format!("B{}", i));
                    turn_clone.store(i * 2 + 2, Ordering::SeqCst);
                    x + 1
                }
            }))
            .depends_on(chain_b);
    }

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Check alternating pattern
    let event_list = events.lock().clone();
    assert_eq!(
        event_list,
        vec!["A0", "B0", "A1", "B1", "A2", "B2", "A3", "B3", "A4", "B4"]
    );

    Ok(())
}

#[tokio::test]
async fn test_layered_interleaving() -> DagResult<()> {
    // Test interleaving across DAG layers
    let dag = DagRunner::new();
    let layer_active = Arc::new(AtomicUsize::new(0));

    // Layer 1: 4 tasks
    let layer1: Vec<_> = (0..4)
        .map(|i| {
            let active = layer_active.clone();
            dag.add_task(task_fn(move |_: ()| {
                let active = active.clone();
                async move {
                    assert_eq!(active.fetch_add(1, Ordering::SeqCst), i);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    i as i32
                }
            }))
            .into()
        })
        .collect();

    // Layer 2: 2 tasks, each depends on 2 from layer1
    let layer2: Vec<_> = (0..2)
        .map(|i| {
            let active = layer_active.clone();
            let idx = i * 2;
            dag.add_task(task_fn(move |(a, b): (i32, i32)| {
                let active = active.clone();
                async move {
                    active.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    a + b
                }
            }))
            .depends_on((&layer1[idx], &layer1[idx + 1]))
        })
        .collect();

    // Layer 3: 1 task depends on both layer2
    let layer3 = dag
        .add_task(task_fn({
            let active = layer_active.clone();
            move |(a, b): (i32, i32)| {
                let active = active.clone();
                async move {
                    active.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    a + b
                }
            }
        }))
        .depends_on((&layer2[0], &layer2[1]));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Sum of 0+1+2+3 = 6
    assert_eq!(dag.get(layer3)?, 6);

    Ok(())
}

#[tokio::test]
async fn test_cross_branch_synchronization() -> DagResult<()> {
    // Test synchronization between parallel branches
    let dag = DagRunner::new();
    let sync_point = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Branch A
    let a1: TaskHandle<_> = dag
        .add_task(task_fn({
            let sync = sync_point.clone();
            let events = events.clone();
            move |_: ()| {
                let sync = sync.clone();
                let events = events.clone();
                async move {
                    events.lock().push("A1");
                    sync.fetch_add(1, Ordering::SeqCst);
                    1
                }
            }
        }))
        .into();

    let a2 = dag
        .add_task(task_fn({
            let sync = sync_point.clone();
            let events = events.clone();
            move |x: i32| {
                let sync = sync.clone();
                let events = events.clone();
                async move {
                    // Wait for B1
                    while sync.load(Ordering::SeqCst) < 2 {
                        tokio::task::yield_now().await;
                    }
                    events.lock().push("A2");
                    sync.fetch_add(1, Ordering::SeqCst);
                    x + 1
                }
            }
        }))
        .depends_on(a1);

    // Branch B
    let b1 = dag.add_task(task_fn({
        let sync = sync_point.clone();
        let events = events.clone();
        move |_: ()| {
            let sync = sync.clone();
            let events = events.clone();
            async move {
                // Wait for A1
                while sync.load(Ordering::SeqCst) < 1 {
                    tokio::task::yield_now().await;
                }
                events.lock().push("B1");
                sync.fetch_add(1, Ordering::SeqCst);
                10
            }
        }
    }));

    let b2 = dag
        .add_task(task_fn({
            let sync = sync_point.clone();
            let events = events.clone();
            move |x: i32| {
                let sync = sync.clone();
                let events = events.clone();
                async move {
                    // Wait for A2
                    while sync.load(Ordering::SeqCst) < 3 {
                        tokio::task::yield_now().await;
                    }
                    events.lock().push("B2");
                    x + 1
                }
            }
        }))
        .depends_on(b1);

    // Merge
    let merge = dag
        .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
        .depends_on((&a2, &b2));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(merge)?, 13); // 2 + 11

    // Check forced interleaving
    let event_list = events.lock().clone();
    assert_eq!(event_list, vec!["A1", "B1", "A2", "B2"]);

    Ok(())
}
