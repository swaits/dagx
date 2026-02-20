//! Tests for timing and speedup verification

use dagx::{task, DagResult, DagRunner};
use dagx_test::task_fn;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::time::sleep;

struct SleepTask(usize, Duration);

#[task]
impl SleepTask {
    async fn run(&self) -> usize {
        sleep(self.1).await;
        self.0
    }
}

#[tokio::test]
async fn test_parallel_execution_speedup() -> DagResult<()> {
    // Prove parallel execution by comparing timing
    let mut dag = DagRunner::new();

    let work_duration = Duration::from_millis(50);
    let num_tasks = 10;

    // Create independent tasks that each take 50ms
    let tasks: Vec<_> = (0..num_tasks)
        .map(|i| dag.add_task(SleepTask(i, work_duration)))
        .collect();

    let start = Instant::now();
    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;
    let elapsed = start.elapsed();

    // Verify all tasks completed
    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(output.get(task), i);
    }

    // If tasks ran sequentially: 10 * 50ms = 500ms
    // If tasks ran in parallel: ~50ms (plus overhead)
    let sequential_time = work_duration * num_tasks as u32;

    println!(
        "Parallel execution took {:?}, sequential would be {:?}",
        elapsed, sequential_time
    );

    // Parallel execution should be significantly faster
    assert!(
        elapsed < sequential_time / 2,
        "Parallel execution too slow: {:?} vs sequential {:?}",
        elapsed,
        sequential_time
    );

    Ok(())
}

#[tokio::test]
async fn test_layers_execute_in_parallel() -> DagResult<()> {
    // Test that tasks in the same topological layer execute in parallel
    let mut dag = DagRunner::new();

    let layer_timing = Arc::new(Mutex::new(Vec::new()));

    struct Layer0Task {
        timing: Arc<Mutex<Vec<(String, Instant)>>>,
        idx: i32,
    }

    #[task]
    impl Layer0Task {
        async fn run(&self) -> i32 {
            let start = Instant::now();
            self.timing
                .lock()
                .unwrap()
                .push((format!("L0_{}", self.idx), start));
            sleep(Duration::from_millis(20)).await;
            self.idx * 10
        }
    }

    // Layer 0: 3 independent sources
    let sources: Vec<_> = (0..3)
        .map(|i| {
            dag.add_task(Layer0Task {
                timing: layer_timing.clone(),
                idx: i,
            })
        })
        .collect();

    struct Layer1Task {
        timing: Arc<Mutex<Vec<(String, Instant)>>>,
        idx: usize,
    }

    #[task]
    impl Layer1Task {
        async fn run(&self, x: &i32) -> i32 {
            let start = Instant::now();
            self.timing
                .lock()
                .unwrap()
                .push((format!("L1_{}", self.idx), start));
            sleep(Duration::from_millis(20)).await;
            x + 1
        }
    }

    // Layer 1: Tasks depending on layer 0
    let layer1: Vec<_> = sources
        .into_iter()
        .enumerate()
        .map(|(i, source)| {
            dag.add_task(Layer1Task {
                idx: i,
                timing: layer_timing.clone(),
            })
            .depends_on(&source)
        })
        .collect();

    // Layer 2: Final aggregation
    let final_task = {
        let timing = layer_timing.clone();
        dag.add_task(task_fn::<(i32, i32, i32), _, _>(
            move |(a, b, c): (&i32, &i32, &i32)| {
                let timing = timing.clone();
                let start = Instant::now();
                timing.lock().unwrap().push(("L2_final".to_string(), start));
                a + b + c
            },
        ))
        .depends_on((&layer1[0], &layer1[1], &layer1[2]))
    };

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(output.get(final_task), 33); // (0+1) + (10+1) + (20+1)

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
