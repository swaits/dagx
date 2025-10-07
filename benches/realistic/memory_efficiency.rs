//! Memory efficiency benchmark with large data structures

use criterion::{BenchmarkId, Criterion};
use dagx::{task_fn, DagRunner};
use std::hint::black_box;

pub fn bench_memory_efficiency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_efficiency");

    // Test with different data sizes (in MB)
    for mb_size in [1, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("large_data_20_consumers", format!("{}MB", mb_size)),
            mb_size,
            |b, &mb| {
                b.iter(|| {
                    rt.block_on(async {
                        let dag = DagRunner::new();

                        let source = dag.add_task(task_fn(move |_: ()| async move {
                            // Create MB of data - framework wraps in Arc automatically
                            vec![0u8; mb * 1024 * 1024]
                        }));

                        // 20 consumers share the data via automatic Arc wrapping
                        for i in 0..20 {
                            dag.add_task(task_fn(move |data: Vec<u8>| async move {
                                // Process a small portion (data extracted from Arc)
                                let sample_sum: u32 = data
                                    .iter()
                                    .skip(i * 1000)
                                    .take(1000)
                                    .map(|&b| b as u32)
                                    .sum();
                                black_box(sample_sum)
                            }))
                            .depends_on(&source);
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
