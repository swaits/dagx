//! Tests for timing and speedup verification

use dagx::{task, DagResult, DagRunner};

use std::time::{Duration, Instant};
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
    let dag = DagRunner::new();

    let work_duration = Duration::from_millis(50);
    let num_tasks = 10;

    // Create independent tasks that each take 50ms
    let tasks: Vec<_> = (0..num_tasks)
        .map(|i| dag.add_task(SleepTask(i, work_duration)))
        .collect();

    let start = Instant::now();
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;
    let elapsed = start.elapsed();

    // Verify all tasks completed
    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
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
