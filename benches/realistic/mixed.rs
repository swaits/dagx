//! Mixed realistic patterns - fan-out, fan-in, and processing

use criterion::Criterion;
use dagx::DagRunner;
use dagx_test::task_fn;
pub fn bench_mixed_patterns(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("mixed_patterns_realistic", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut dag = DagRunner::new();

                // Stage 1: Load data (framework wraps in Arc automatically)
                let data = dag.add_task(task_fn::<(), _, _>(|_: ()| {
                    (0..1000)
                        .map(|i| format!("Record-{:04}", i))
                        .collect::<Vec<_>>()
                }));

                // Stage 2: Fan-out for parallel analysis
                let analysis1 = dag
                    .add_task(task_fn::<Vec<_>, _, _>(|d: &Vec<String>| {
                        d.iter().filter(|s| s.contains("00")).count()
                    }))
                    .depends_on(&data);

                let analysis2 = dag
                    .add_task(task_fn::<Vec<_>, _, _>(|d: &Vec<String>| {
                        d.iter().map(|s| s.len()).sum::<usize>()
                    }))
                    .depends_on(&data);

                let analysis3 = dag
                    .add_task(task_fn::<Vec<_>, _, _>(|d: &Vec<String>| d.len()))
                    .depends_on(&data);

                // Stage 3: Fan-in to aggregate
                let summary = dag
                    .add_task(task_fn::<(usize, usize, usize), _, _>(
                        |(a1, a2, a3): (&usize, &usize, &usize)| {
                            format!("Matches: {}, Total bytes: {}, Count: {}", a1, a2, a3)
                        },
                    ))
                    .depends_on((&analysis1, &analysis2, &analysis3));

                // Stage 4: Fan-out final report to multiple destinations
                for i in 0..3 {
                    dag.add_task(task_fn::<String, _, _>(move |report: &String| {
                        format!("Destination {}: {}", i, report)
                    }))
                    .depends_on(&summary);
                }

                let _output = dag
                    .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });
}
