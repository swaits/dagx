//! DAG scaling benchmarks - how performance scales with task count

use criterion::{BenchmarkId, Criterion};
use dagx::DagRunner;
use dagx_test::task_fn;

pub fn bench_dag_scaling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("scaling");

    // Give extra time for 1000-task benchmark (needs ~5.8s, set to 8s for headroom)
    group.measurement_time(std::time::Duration::from_secs(8));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let dag = DagRunner::new();
                    for i in 0..size {
                        dag.add_task(task_fn::<(), _, _>(move |_: ()| i));
                    }
                    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                        .await
                        .unwrap();
                })
            });
        });
    }

    group.finish();
}
