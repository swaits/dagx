//! Diamond pattern benchmarks (fan-out then fan-in)

use criterion::Criterion;
use dagx::{task_fn, DagRunner};
use futures::FutureExt;

pub fn bench_diamond(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("diamond_pattern_50", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();

                for i in 0..50 {
                    let source = dag.add_task(task_fn(move |_: ()| async move { i }));

                    let left = dag
                        .add_task(task_fn(|x: i32| async move { x * 2 }))
                        .depends_on(&source);
                    let right = dag
                        .add_task(task_fn(|x: i32| async move { x * 3 }))
                        .depends_on(&source);

                    dag.add_task(task_fn(|(l, r): (i32, i32)| async move { l + r }))
                        .depends_on((&left, &right));
                }

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
            })
        });
    });
}
