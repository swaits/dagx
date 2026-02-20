//! Diamond pattern benchmarks (fan-out then fan-in)

use criterion::Criterion;
use dagx::DagRunner;
use dagx_test::task_fn;
pub fn bench_diamond(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("diamond_pattern_50", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut dag = DagRunner::new();

                for i in 0..50 {
                    let source = dag.add_task(task_fn::<(), _, _>(move |_: ()| i));

                    let left = dag
                        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
                        .depends_on(&source);
                    let right = dag
                        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 3))
                        .depends_on(&source);

                    dag.add_task(task_fn::<(i32, i32), _, _>(|(l, r): (&i32, &i32)| l + r))
                        .depends_on((&left, &right));
                }

                let _output = dag
                    .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });
}
