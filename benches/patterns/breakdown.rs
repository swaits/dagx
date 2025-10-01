//! 10,000 task breakdown benchmarks - construction vs execution

use criterion::Criterion;
use dagx::{task_fn, DagRunner};

pub fn bench_10k_breakdown(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("10k_tasks");
    group.measurement_time(std::time::Duration::from_secs(10));

    // a) Adding 10k tasks (construction only)
    group.bench_function("construction", |b| {
        b.iter(|| {
            let dag = DagRunner::new();
            for i in 0..10_000 {
                dag.add_task(task_fn(move |_: ()| async move { i }));
            }
        });
    });

    // b) Full lifecycle: add 10k tasks + run them
    group.bench_function("full_execution", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                for i in 0..10_000 {
                    dag.add_task(task_fn(move |_: ()| async move { i }));
                }
                dag.run(|fut| {
                    tokio::spawn(fut);
                })
                .await
                .unwrap();
            })
        });
    });

    // c) Execution only (pre-built DAG, just run)
    group.bench_function("execution_only", |b| {
        // Pre-build the DAG outside the measurement
        let dag = DagRunner::new();
        for i in 0..10_000 {
            dag.add_task(task_fn(move |_: ()| async move { i }));
        }

        b.iter(|| {
            rt.block_on(async {
                dag.run(|fut| {
                    tokio::spawn(fut);
                })
                .await
                .unwrap();
            })
        });
    });

    group.finish();
}
