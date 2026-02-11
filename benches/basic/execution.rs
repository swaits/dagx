//! Basic DAG execution benchmarks

use criterion::Criterion;
use dagx::{task_fn, DagRunner};

pub fn bench_dag_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("execute_10_independent_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                for i in 0..10 {
                    dag.add_task(task_fn::<(), _, _>(move |_: ()| i));
                }
                dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });

    c.bench_function("execute_100_independent_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();
                for i in 0..100 {
                    dag.add_task(task_fn::<(), _, _>(move |_: ()| i));
                }
                dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });

    c.bench_function("execute_deep_chain_20", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dag = DagRunner::new();

                // Build a manual chain (API doesn't support loop-based chaining due to type system)
                struct Value(i32);
                #[dagx::task]
                impl Value {
                    async fn run(&self) -> i32 {
                        self.0
                    }
                }

                let t0 = dag.add_task(Value(0));
                let t1 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t0);
                let t2 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t1);
                let t3 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t2);
                let t4 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t3);
                let t5 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t4);
                let t6 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t5);
                let t7 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t6);
                let t8 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t7);
                let t9 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t8);
                let t10 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t9);
                let t11 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t10);
                let t12 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t11);
                let t13 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t12);
                let t14 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t13);
                let t15 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t14);
                let t16 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t15);
                let t17 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t16);
                let t18 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t17);
                let _t19 = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
                    .depends_on(t18);

                dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
                    .await
                    .unwrap();
            })
        });
    });
}
