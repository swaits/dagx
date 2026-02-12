//! Tests for empty DAGs and minimal task configurations

use dagx::task_fn;
use dagx::{task, DagResult, DagRunner};

use tokio::task::yield_now;

#[tokio::test]
async fn test_dag_with_only_source_tasks() -> DagResult<()> {
    // Test a DAG with multiple source tasks but no dependencies
    let dag = DagRunner::new();

    let tasks: Vec<_> = (0..100)
        .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| i)))
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(dag.get(task)?, i);
    }

    Ok(())
}

#[tokio::test]
async fn test_single_self_contained_stateful_task() -> DagResult<()> {
    // Test a single stateful task with internal state
    struct Counter {
        value: i32,
    }

    #[task]
    impl Counter {
        async fn run(&self) -> i32 {
            self.value * 2
        }
    }

    let dag = DagRunner::new();
    let task = dag.add_task(Counter { value: 21 });

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(dag.get(task)?, 42);
    Ok(())
}

#[tokio::test]
async fn test_minimal_task_with_minimal_async_work() -> DagResult<()> {
    // Test a task that does the absolute minimum async work
    let dag = DagRunner::new();

    struct YieldTask;

    #[task]
    impl YieldTask {
        async fn run() -> i32 {
            yield_now().await;
            1337
        }
    }

    let task = dag.add_task(YieldTask);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(dag.get(task)?, 1337);
    Ok(())
}
