//! Fan-out pattern benchmarks (1 → N dependencies)

use criterion::Criterion;
use dagx::{task_fn, DagRunner, TaskHandle};

pub fn bench_fanout(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Fan-out pattern (1 → n) with simple i32
    c.bench_function("fanout_1_to_100_i32", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 42)).into();

                for i in 0..100 {
                    dag.add_task(task_fn::<i32, _, _>(move |&x: &i32| x + i))
                        .depends_on(source);
                }

                dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });

    // Fan-out pattern with Vec (automatic Arc wrapping)
    c.bench_function("fanout_1_to_100_vec", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                let source: TaskHandle<_> = dag
                    .add_task(task_fn::<(), _, _>(|_: ()| {
                        // Create 1000 strings - framework wraps in Arc automatically
                        (0..1000).map(|i| format!("Item {}", i)).collect::<Vec<_>>()
                    }))
                    .into();

                for i in 0..100 {
                    dag.add_task(task_fn::<Vec<_>, _, _>(move |data: &Vec<String>| {
                        // Each consumer gets data extracted from Arc
                        data.iter().filter(|s| s.contains(&i.to_string())).count()
                    }))
                    .depends_on(source);
                }

                dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });
}
