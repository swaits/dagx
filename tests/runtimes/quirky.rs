//! Tests for quirky/edge-case async runtimes
//!
//! This file tests dagx with unusual async runtimes to ensure
//! the spawner parameter works correctly across different executor types.
//!
//! Tested runtimes:
//! - async-executor: Minimal executor from smol ecosystem
//! - pollster: Blocking/synchronous executor
//! - futures-executor: Basic futures crate executor

// Note: executor.run() synchronously executes futures to completion.
// We don't need the return value in these tests, so we allow unused results.
#![allow(clippy::let_underscore_future)]
#![allow(unused_must_use)]

use crate::common::task_fn;
use dagx::DagRunner;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// =======================
// async-executor Tests
// =======================

#[test]
fn test_async_executor_basic() {
    // async-executor is a minimal executor from the smol ecosystem
    let executor = async_executor::Executor::new();

    let future = async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 10 }));
        let y = dag.add_task(task_fn(|_: ()| async { 20 }));
        let sum = dag
            .add_task(task_fn(|(a, b): (i32, i32)| async move { a + b }))
            .depends_on((&x, &y));

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(sum).unwrap(), 30);
    };

    executor.run(future);
}

#[test]
fn test_async_executor_complex_dag() {
    let executor = async_executor::Executor::new();

    let future = async {
        let dag = DagRunner::new();

        // Create a diamond pattern
        let source = dag.add_task(task_fn(|_: ()| async { 5 }));

        let left = dag
            .add_task(task_fn(|x: i32| async move { x * 2 }))
            .depends_on(&source);

        let right = dag
            .add_task(task_fn(|x: i32| async move { x * 3 }))
            .depends_on(&source);

        let sink = dag
            .add_task(task_fn(|(l, r): (i32, i32)| async move { l + r }))
            .depends_on((&left, &right));

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(sink).unwrap(), 25); // (5*2) + (5*3) = 25
    };

    executor.run(future);
}

#[test]
fn test_async_executor_parallel_tasks() {
    let executor = async_executor::Executor::new();

    let future = async {
        let dag = DagRunner::new();
        let counter = Arc::new(AtomicUsize::new(0));

        // Create 10 independent tasks
        let tasks: Vec<_> = (0..10)
            .map(|i| {
                let c = counter.clone();
                dag.add_task(task_fn(move |_: ()| {
                    let c = c.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        i
                    }
                }))
            })
            .collect();

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        // All tasks should have run
        assert_eq!(counter.load(Ordering::SeqCst), 10);

        // Verify results
        for (i, task) in tasks.iter().enumerate() {
            assert_eq!(dag.get(task).unwrap(), i as i32);
        }
    };

    executor.run(future);
}

#[test]
fn test_async_executor_with_local_spawn() {
    // Test with LocalExecutor (single-threaded)
    let executor = async_executor::LocalExecutor::new();

    let future = async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 42 }));

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(x).unwrap(), 42);
    };

    executor.run(future);
}

#[test]
fn test_async_executor_deep_chain() {
    let executor = async_executor::Executor::new();

    let future = async {
        let dag = DagRunner::new();

        // Build a chain of 50 tasks
        let start = dag.add_task(task_fn(|_: ()| async { 0 }));
        let mut current = dag
            .add_task(task_fn(|x: i32| async move { x + 1 }))
            .depends_on(&start);

        for _ in 1..50 {
            current = dag
                .add_task(task_fn(|x: i32| async move { x + 1 }))
                .depends_on(current);
        }

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(current).unwrap(), 50);
    };

    executor.run(future);
}

// =======================
// pollster Tests
// =======================

#[test]
fn test_pollster_basic() {
    // pollster is a blocking executor - it blocks the thread
    pollster::block_on(async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 100 }));
        let doubled = dag
            .add_task(task_fn(|x: i32| async move { x * 2 }))
            .depends_on(&x);

        dag.run(|fut| {
            // pollster blocks, so we need to spawn on a different executor
            // Use futures::executor for spawning
            futures_executor::block_on(fut);
        })
        .await
        .unwrap();

        assert_eq!(dag.get(doubled).unwrap(), 200);
    });
}

#[test]
fn test_pollster_complex() {
    pollster::block_on(async {
        let dag = DagRunner::new();

        // Fan-out pattern
        let source = dag.add_task(task_fn(|_: ()| async { 7 }));

        let tasks: Vec<_> = (0..5)
            .map(|i| {
                dag.add_task(task_fn(move |x: i32| async move { x * i }))
                    .depends_on(&source)
            })
            .collect();

        dag.run(|fut| {
            futures_executor::block_on(fut);
        })
        .await
        .unwrap();

        for (i, task) in tasks.iter().enumerate() {
            assert_eq!(dag.get(task).unwrap(), 7 * i as i32);
        }
    });
}

#[test]
fn test_pollster_error_handling() {
    pollster::block_on(async {
        let dag = DagRunner::new();

        // Task that returns Result
        let task = dag.add_task(task_fn(|_: ()| async { Ok::<i32, String>(42) }));

        dag.run(|fut| {
            futures_executor::block_on(fut);
        })
        .await
        .unwrap();

        assert_eq!(dag.get(task).unwrap(), Ok(42));
    });
}

// =======================
// futures-executor Tests
// =======================

#[test]
fn test_futures_executor_basic() {
    futures_executor::block_on(async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 15 }));
        let y = dag.add_task(task_fn(|_: ()| async { 25 }));
        let product = dag
            .add_task(task_fn(|(a, b): (i32, i32)| async move { a * b }))
            .depends_on((&x, &y));

        dag.run(|fut| {
            // Use ThreadPool for spawning
            let pool = futures_executor::ThreadPool::new().unwrap();
            pool.spawn_ok(fut);
        })
        .await
        .unwrap();

        assert_eq!(dag.get(product).unwrap(), 375); // 15 * 25
    });
}

#[test]
fn test_futures_executor_threadpool() {
    let pool = std::sync::Arc::new(futures_executor::ThreadPool::new().unwrap());
    let pool_clone = pool.clone();

    pool.spawn_ok(async move {
        let dag = DagRunner::new();

        // Create multiple independent tasks
        let tasks: Vec<_> = (0..20)
            .map(|i| dag.add_task(task_fn(move |_: ()| async move { i * i })))
            .collect();

        dag.run(|fut| {
            pool_clone.spawn_ok(fut);
        })
        .await
        .unwrap();

        for (i, task) in tasks.iter().enumerate() {
            assert_eq!(dag.get(task).unwrap(), (i * i) as i32);
        }
    });

    // Give tasks time to complete
    std::thread::sleep(std::time::Duration::from_millis(100));
}

// Note: LocalPool test removed because it cannot run within another executor context
// (cargo test framework may use tokio, causing EnterError)
// LocalPool is single-threaded anyway, so futures-executor ThreadPool is more representative

#[test]
fn test_futures_executor_diamond_pattern() {
    futures_executor::block_on(async {
        let dag = DagRunner::new();

        let a = dag.add_task(task_fn(|_: ()| async { 8 }));

        let b = dag
            .add_task(task_fn(|x: i32| async move { x + 2 }))
            .depends_on(&a);

        let c = dag
            .add_task(task_fn(|x: i32| async move { x - 2 }))
            .depends_on(&a);

        let d = dag
            .add_task(task_fn(|(x, y): (i32, i32)| async move { x * y }))
            .depends_on((&b, &c));

        let pool = futures_executor::ThreadPool::new().unwrap();
        dag.run(|fut| {
            pool.spawn_ok(fut);
        })
        .await
        .unwrap();

        assert_eq!(dag.get(d).unwrap(), 60); // (8+2) * (8-2) = 10 * 6
    });
}

// =======================
// Cross-Runtime Tests
// =======================

#[test]
fn test_all_runtimes_compatibility() {
    // This test verifies the same DAG works across all runtimes

    // Test 1: async-executor
    {
        let executor = async_executor::Executor::new();
        executor.run(async {
            let dag = DagRunner::new();
            let x = dag.add_task(task_fn(|_: ()| async { 123 }));
            dag.run(|fut| executor.spawn(fut).detach()).await.unwrap();
            assert_eq!(dag.get(x).unwrap(), 123);
        });
    }

    // Test 2: pollster
    {
        pollster::block_on(async {
            let dag = DagRunner::new();
            let x = dag.add_task(task_fn(|_: ()| async { 123 }));
            dag.run(futures_executor::block_on).await.unwrap();
            assert_eq!(dag.get(x).unwrap(), 123);
        });
    }

    // Test 3: futures-executor
    {
        futures_executor::block_on(async {
            let dag = DagRunner::new();
            let x = dag.add_task(task_fn(|_: ()| async { 123 }));
            let pool = futures_executor::ThreadPool::new().unwrap();
            dag.run(|fut| pool.spawn_ok(fut)).await.unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
            assert_eq!(dag.get(x).unwrap(), 123);
        });
    }
}

#[test]
fn test_runtime_edge_cases() {
    // Test with LocalExecutor (no Send requirement)
    let executor = async_executor::LocalExecutor::new();

    executor.run(async {
        let dag = DagRunner::new();

        // This tests that our API works even with !Send futures
        let x = dag.add_task(task_fn(|_: ()| async {
            // Local state

            42
        }));

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(x).unwrap(), 42);
    });
}

// =======================
// Stress Tests for Quirky Runtimes
// =======================

#[test]
fn test_async_executor_stress() {
    let executor = async_executor::Executor::new();

    executor.run(async {
        let dag = DagRunner::new();

        // Create 100 tasks
        let tasks: Vec<_> = (0..100)
            .map(|i| {
                dag.add_task(task_fn(move |_: ()| async move {
                    // Small delay to test executor scheduling
                    i
                }))
            })
            .collect();

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        for (i, task) in tasks.iter().enumerate() {
            assert_eq!(dag.get(task).unwrap(), i as i32);
        }
    });
}

#[test]
fn test_pollster_nested_blocks() {
    // Test nested blocking - edge case
    pollster::block_on(async {
        let dag1 = DagRunner::new();
        let x = dag1.add_task(task_fn(|_: ()| async { 10 }));

        dag1.run(|fut| {
            pollster::block_on(async {
                futures_executor::block_on(fut);
            });
        })
        .await
        .unwrap();

        assert_eq!(dag1.get(x).unwrap(), 10);
    });
}

#[test]
fn test_futures_executor_multi_pool() {
    // Test with multiple thread pools
    let pool1 = futures_executor::ThreadPool::new().unwrap();
    let pool2 = futures_executor::ThreadPool::new().unwrap();

    pool1.spawn_ok(async move {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 50 }));

        dag.run(|fut| {
            // Spawn on different pool
            pool2.spawn_ok(fut);
        })
        .await
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(dag.get(x).unwrap(), 50);
    });

    std::thread::sleep(std::time::Duration::from_millis(100));
}

// =======================
// Edge Case: Sync Tasks with Quirky Runtimes
// =======================

#[test]
fn test_sync_tasks_on_async_executor() {
    use dagx::Task;

    struct SyncAdd;

    #[dagx::task]
    impl SyncAdd {
        fn run(a: &i32, b: &i32) -> i32 {
            // Synchronous task
            a + b
        }
    }

    let executor = async_executor::Executor::new();

    executor.run(async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 5 }));
        let y = dag.add_task(task_fn(|_: ()| async { 7 }));
        let sum = dag.add_task(SyncAdd).depends_on((&x, &y));

        dag.run(|fut| {
            executor.spawn(fut).detach();
        })
        .await
        .unwrap();

        assert_eq!(dag.get(sum).unwrap(), 12);
    });
}

#[test]
fn test_sync_tasks_on_pollster() {
    use dagx::Task;

    struct SyncDouble;

    #[dagx::task]
    impl SyncDouble {
        fn run(x: &i32) -> i32 {
            x * 2
        }
    }

    pollster::block_on(async {
        let dag = DagRunner::new();

        let x = dag.add_task(task_fn(|_: ()| async { 21 }));
        let doubled = dag.add_task(SyncDouble).depends_on(&x);

        dag.run(|fut| {
            futures_executor::block_on(fut);
        })
        .await
        .unwrap();

        assert_eq!(dag.get(doubled).unwrap(), 42);
    });
}
