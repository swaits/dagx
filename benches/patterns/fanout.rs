//! Fan-out pattern benchmarks (1 → N dependencies)

use criterion::Criterion;
use dagx::{task_fn, DagRunner};
use futures::FutureExt;

pub fn bench_fanout(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Fan-out pattern (1 → n) with simple i32
    c.bench_function("fanout_1_to_100_i32", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                let source = dag.add_task(task_fn(|_: ()| async { 42 }));

                for i in 0..100 {
                    dag.add_task(task_fn(move |x: i32| async move { x + i }))
                        .depends_on(&source);
                }

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
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
                let source = dag.add_task(task_fn(|_: ()| async {
                    // Create 1000 strings - framework wraps in Arc automatically
                    (0..1000).map(|i| format!("Item {}", i)).collect::<Vec<_>>()
                }));

                for i in 0..100 {
                    dag.add_task(task_fn(move |data: Vec<String>| async move {
                        // Each consumer gets data extracted from Arc
                        data.iter().filter(|s| s.contains(&i.to_string())).count()
                    }))
                    .depends_on(&source);
                }

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
            })
        });
    });

    // Arc overhead benchmark - shows cost on Copy types
    // For Copy types like usize, Arc adds overhead (atomic refcounting + indirection)
    // For heap types like Vec, Arc overhead is negligible compared to clone cost
    c.bench_function("arc_overhead_copy_type", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();

                // Source produces a simple Copy type (usize)
                // Framework wraps in Arc<usize> internally
                let source = dag.add_task(task_fn(|_: ()| async { 42usize }));

                // 100 dependents - Arc overhead visible on Copy types
                for i in 0..100 {
                    dag.add_task(task_fn(move |x: usize| async move { x + i }))
                        .depends_on(&source);
                }

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
            })
        });
    });
}
