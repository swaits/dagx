//! Configuration broadcast pattern benchmark - one config shared across many workers

use criterion::{BenchmarkId, Criterion};
use dagx::{task_fn, DagRunner};
use std::collections::HashMap;
use std::hint::black_box;

pub fn bench_config_broadcast(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("config_broadcast");

    for worker_count in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("config", worker_count),
            worker_count,
            |b, &workers| {
                b.iter(|| {
                    rt.block_on(async {
                        let dag = DagRunner::new();

                        // Configuration source - framework wraps HashMap in Arc automatically
                        let config = dag.add_task(task_fn(|_: ()| async {
                            HashMap::from([
                                ("max_connections".to_string(), "100".to_string()),
                                ("timeout_ms".to_string(), "5000".to_string()),
                                ("batch_size".to_string(), "50".to_string()),
                                ("retry_count".to_string(), "3".to_string()),
                                ("worker_threads".to_string(), "4".to_string()),
                                ("buffer_size".to_string(), "8192".to_string()),
                                ("compression".to_string(), "true".to_string()),
                                ("log_level".to_string(), "info".to_string()),
                            ])
                        }));

                        // Workers receive and process based on shared config
                        for worker_id in 0..workers {
                            dag.add_task(task_fn(move |cfg: HashMap<String, String>| async move {
                                // Simulate work based on configuration
                                let batch_size: usize = cfg
                                    .get("batch_size")
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(10);
                                let timeout: usize = cfg
                                    .get("timeout_ms")
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(1000);

                                // Simulate some computation
                                black_box(worker_id * batch_size + timeout / 100)
                            }))
                            .depends_on(&config);
                        }

                        dag.run(|fut| {
                            tokio::spawn(fut);
                        })
                        .await
                        .unwrap();
                    })
                });
            },
        );
    }

    group.finish();
}
