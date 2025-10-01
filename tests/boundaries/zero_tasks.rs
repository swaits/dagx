//! Tests for empty DAGs and minimal task configurations

use crate::common::task_fn;
use dagx::{task, DagResult, DagRunner, Task};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_empty_dag_execution() -> DagResult<()> {
    // Test that an empty DAG can be created and run without issues
    let dag = DagRunner::new();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Should complete successfully with no tasks
    Ok(())
}

#[tokio::test]
async fn test_single_task_no_deps() -> DagResult<()> {
    // Test the absolute minimum: one task with no dependencies
    let dag = DagRunner::new();

    let task = dag.add_task(task_fn(|_: ()| async { 42 }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(task)?, 42);
    Ok(())
}

#[tokio::test]
async fn test_single_task_unit_input_output() -> DagResult<()> {
    // Test a task that takes unit and returns unit
    let dag = DagRunner::new();

    let executed = Arc::new(AtomicUsize::new(0));
    let exec_clone = executed.clone();

    let task = dag.add_task(task_fn(move |_: ()| {
        let exec = exec_clone.clone();
        async move {
            exec.fetch_add(1, Ordering::SeqCst);
        }
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    dag.get(task)?; // Just verify it doesn't error
    assert_eq!(executed.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn test_zero_sized_type_task() -> DagResult<()> {
    // Test with zero-sized types
    #[derive(Debug, Clone, PartialEq)]
    struct ZeroSized;

    let dag = DagRunner::new();

    let task = dag.add_task(task_fn(|_: ()| async { ZeroSized }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(&task)?, ZeroSized);
    Ok(())
}

#[tokio::test]
async fn test_empty_dag_multiple_runs() -> DagResult<()> {
    // Test that an empty DAG can be run multiple times
    let dag = DagRunner::new();

    for _ in 0..10 {
        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await?;
    }

    Ok(())
}

#[tokio::test]
async fn test_dag_with_only_source_tasks() -> DagResult<()> {
    // Test a DAG with multiple source tasks but no dependencies
    let dag = DagRunner::new();

    let tasks: Vec<_> = (0..100)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    for (i, task) in tasks.iter().enumerate() {
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

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(task)?, 42);
    Ok(())
}

#[tokio::test]
async fn test_alternating_empty_and_filled_dag_runs() -> DagResult<()> {
    // Test alternating between empty DAG and DAG with tasks
    for i in 0..5 {
        let dag = DagRunner::new();

        if i % 2 == 0 {
            // Empty DAG
            dag.run(|fut| {
                tokio::spawn(fut);
            })
            .await?;
        } else {
            // DAG with a task
            let task = dag.add_task(task_fn(move |_: ()| async move { i }));
            dag.run(|fut| {
                tokio::spawn(fut);
            })
            .await?;
            assert_eq!(dag.get(task)?, i);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_dag_with_never_type_simulation() -> DagResult<()> {
    // Test handling of tasks that conceptually "never" complete
    // (simulated by immediate completion with special marker)
    #[derive(Debug, Clone, PartialEq)]
    enum Never {}

    let dag = DagRunner::new();

    // This task completes immediately but returns a Result containing "never" type
    let task = dag.add_task(task_fn(|_: ()| async { Result::<i32, Never>::Ok(42) }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(&task)?, Ok(42));
    Ok(())
}

#[tokio::test]
async fn test_minimal_task_with_minimal_async_work() -> DagResult<()> {
    // Test a task that does the absolute minimum async work
    let dag = DagRunner::new();

    let task = dag.add_task(task_fn(|_: ()| async {
        // Just yield once to the runtime
        tokio::task::yield_now().await;
        1337
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    assert_eq!(dag.get(task)?, 1337);
    Ok(())
}
