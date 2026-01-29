//! ETL (Extract, Transform, Load) pipeline benchmark

use criterion::{BenchmarkId, Criterion};
use dagx::{task_fn, DagRunner, TaskHandle};
use futures::FutureExt;

pub fn bench_etl_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("etl_scaling");

    // ETL pipeline needs more time for larger datasets (10k records needs ~12.3s, set to 15s for headroom)
    group.measurement_time(std::time::Duration::from_secs(15));

    for record_count in [1_000, 5_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("etl", record_count),
            record_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let dag = DagRunner::new();

                        // Extract: Load JSON-like data (framework wraps in Arc automatically)
                        let extract: TaskHandle<_> = dag.add_task(task_fn(move |_: ()| async move {
                            (0..count)
                                .map(|i| {
                                    format!(
                                        "{{\"id\":{},\"name\":\"User{}\",\"score\":{}}}",
                                        i,
                                        i,
                                        i * 10
                                    )
                                })
                                .collect::<Vec<_>>()
                        })).into();

                        // Transform: Three parallel transformations

                        // T1: Parse and validate
                        let validate = dag
                            .add_task(task_fn(move |data: Vec<String>| async move {
                                data.iter()
                                    .filter(|s| {
                                        s.contains("\"id\"")
                                            && s.contains("\"name\"")
                                            && s.contains("\"score\"")
                                    })
                                    .count()
                            }))
                            .depends_on(extract);

                        // T2: Extract scores
                        let scores = dag
                            .add_task(task_fn(move |data: Vec<String>| async move {
                                data.iter()
                                    .filter_map(|s| {
                                        s.split("\"score\":").nth(1).and_then(|part| {
                                            part.trim_end_matches('}').parse::<i32>().ok()
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            }))
                            .depends_on(extract);

                        // T3: Extract names
                        let names = dag
                            .add_task(task_fn(move |data: Vec<String>| async move {
                                data.iter()
                                    .filter_map(|s| {
                                        s.split("\"name\":\"")
                                            .nth(1)
                                            .and_then(|part| part.split('"').next())
                                            .map(String::from)
                                    })
                                    .collect::<Vec<_>>()
                            }))
                            .depends_on(extract);

                        // Load: Aggregate all transformations
                        dag.add_task(task_fn(
                            move |(valid, scores, names): (usize, Vec<i32>, Vec<String>)| async move {
                                let avg_score =
                                    scores.iter().sum::<i32>() / scores.len().max(1) as i32;
                                format!(
                                    "Processed {} valid records, {} names, avg score: {}",
                                    valid,
                                    names.len(),
                                    avg_score
                                )
                            },
                        ))
                        .depends_on((&validate, &scores, &names));

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                        .await
                        .unwrap();
                    })
                });
            },
        );
    }

    group.finish();
}
