//! Memory efficiency benchmark with large data structures

use criterion::{BenchmarkId, Criterion};
use dagx::DagRunner;
use dagx_test::task_fn;
use std::hint::black_box;

pub fn bench_memory_efficiency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_efficiency");

    // Test with different data sizes (in MB)
    for mb_size in [1, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("large_data_20_consumers", format!("{}MB", mb_size)),
            mb_size,
            |b, &mb| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut dag = DagRunner::new();

                        let source = dag.add_task(task_fn::<(), _, _>(move |_: ()| {
                            // Create MB of data - framework wraps in Arc automatically
                            vec![0u8; mb * 1024 * 1024]
                        }));

                        // 20 consumers share the data via automatic Arc wrapping
                        for i in 0..20 {
                            dag.add_task(task_fn::<Vec<_>, _, _>(move |data: &Vec<u8>| {
                                // Process a small portion (data extracted from Arc)
                                let sample_sum: u32 = data
                                    .iter()
                                    .skip(i * 1000)
                                    .take(1000)
                                    .map(|&b| b as u32)
                                    .sum();
                                black_box(sample_sum)
                            }))
                            .depends_on(source);
                        }

                        let _output = dag
                            .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                            .await
                            .unwrap();
                    })
                });
            },
        );
    }

    group.finish();
}
