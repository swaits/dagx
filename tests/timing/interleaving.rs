//! Tests for forced execution interleaving patterns

use dagx::{task, DagResult, DagRunner};
use dagx_test::task_fn;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_forced_interleaving_with_barriers() -> DagResult<()> {
    // Force specific interleaving using barriers
    let dag = DagRunner::new();
    let barrier = Arc::new(Barrier::new(3));
    let order = Arc::new(parking_lot::Mutex::new(Vec::new()));

    struct InterleaveTask {
        barrier: Arc<Barrier>,
        order: Arc<parking_lot::Mutex<Vec<String>>>,
        prefix: &'static str,
        value: i32,
    }

    #[task]
    impl InterleaveTask {
        async fn run(&self) -> i32 {
            self.order.lock().push(format!("{}_start", self.prefix));
            self.barrier.wait().await;
            self.order.lock().push(format!("{}_end", self.prefix));
            self.value
        }
    }

    // Three parallel tasks that synchronize at barrier
    let t1 = dag.add_task(InterleaveTask {
        barrier: barrier.clone(),
        order: order.clone(),
        prefix: "t1",
        value: 1,
    });
    let t2 = dag.add_task(InterleaveTask {
        barrier: barrier.clone(),
        order: order.clone(),
        prefix: "t2",
        value: 2,
    });
    let t3 = dag.add_task(InterleaveTask {
        barrier: barrier.clone(),
        order: order.clone(),
        prefix: "t2",
        value: 3,
    });

    // Collector
    let collector = dag
        .add_task(task_fn::<(i32, i32, i32), _, _>(
            |(a, b, c): (&i32, &i32, &i32)| a + b + c,
        ))
        .depends_on((t1, t2, t3));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

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

    struct FirstA {
        turn: Arc<AtomicUsize>,
        events: Arc<parking_lot::Mutex<Vec<String>>>,
    }

    #[task]
    impl FirstA {
        async fn run(&self) -> i32 {
            while self.turn.load(Ordering::SeqCst) != 0 {
                tokio::task::yield_now().await;
            }
            self.events.lock().push("A0".to_string());
            self.turn.store(1, Ordering::SeqCst);
            0
        }
    }

    let first_a = dag.add_task(FirstA {
        turn: turn.clone(),
        events: events.clone(),
    });

    let mut chain_a: dagx::TaskHandle<i32> = first_a.into();

    struct FirstB {
        turn: Arc<AtomicUsize>,
        events: Arc<parking_lot::Mutex<Vec<String>>>,
    }

    #[task]
    impl FirstB {
        async fn run(&self) -> i32 {
            while self.turn.load(Ordering::SeqCst) != 1 {
                tokio::task::yield_now().await;
            }
            self.events.lock().push("B0".to_string());
            self.turn.store(2, Ordering::SeqCst);
            0
        }
    }

    let first_b = dag.add_task(FirstB {
        turn: turn.clone(),
        events: events.clone(),
    });

    let mut chain_b: dagx::TaskHandle<i32> = first_b.into();

    struct ChainA {
        turn: Arc<AtomicUsize>,
        events: Arc<parking_lot::Mutex<Vec<String>>>,
        idx: usize,
    }

    #[task]
    impl ChainA {
        async fn run(&self, x: &i32) -> i32 {
            while self.turn.load(Ordering::SeqCst) != self.idx * 2 {
                tokio::task::yield_now().await;
            }
            self.events.lock().push(format!("A{}", self.idx));
            self.turn.store(self.idx * 2 + 1, Ordering::SeqCst);
            x + 1
        }
    }

    struct ChainB {
        turn: Arc<AtomicUsize>,
        events: Arc<parking_lot::Mutex<Vec<String>>>,
        idx: usize,
    }

    #[task]
    impl ChainB {
        async fn run(&self, x: &i32) -> i32 {
            while self.turn.load(Ordering::SeqCst) != self.idx * 2 + 1 {
                tokio::task::yield_now().await;
            }
            self.events.lock().push(format!("B{}", self.idx));
            self.turn.store(self.idx * 2 + 2, Ordering::SeqCst);
            x + 1
        }
    }

    for i in 1..5 {
        chain_a = dag
            .add_task(ChainA {
                events: events.clone(),
                turn: turn.clone(),
                idx: i,
            })
            .depends_on(chain_a);

        chain_b = dag
            .add_task(ChainB {
                turn: turn.clone(),
                events: events.clone(),
                idx: i,
            })
            .depends_on(chain_b);
    }

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

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

    struct Layer1Task {
        active: Arc<AtomicUsize>,
        idx: usize,
    }

    #[task]
    impl Layer1Task {
        async fn run(&self) -> i32 {
            assert_eq!(self.active.fetch_add(1, Ordering::SeqCst), self.idx);
            tokio::task::yield_now().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            self.idx as i32
        }
    }

    // Layer 1: 4 tasks
    let layer1: Vec<_> = (0..4)
        .map(|i| {
            dag.add_task(Layer1Task {
                active: layer_active.clone(),
                idx: i,
            })
            .into()
        })
        .collect();

    struct Layer23Task {
        active: Arc<AtomicUsize>,
    }

    #[task]
    impl Layer23Task {
        async fn run(&self, a: &i32, b: &i32) -> i32 {
            self.active.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            a + b
        }
    }

    // Layer 2: 2 tasks, each depends on 2 from layer1
    let layer2: Vec<_> = (0..2)
        .map(|i| {
            let idx = i * 2;
            dag.add_task(Layer23Task {
                active: layer_active.clone(),
            })
            .depends_on((&layer1[idx], &layer1[idx + 1]))
        })
        .collect();

    // Layer 3: 1 task depends on both layer2
    let layer3 = dag
        .add_task(Layer23Task {
            active: layer_active.clone(),
        })
        .depends_on((&layer2[0], &layer2[1]));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    // Sum of 0+1+2+3 = 6
    assert_eq!(dag.get(layer3)?, 6);

    Ok(())
}
