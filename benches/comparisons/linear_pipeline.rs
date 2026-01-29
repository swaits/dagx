//! Linear pipeline comparison: dagx vs dagrs
//!
//! Simple chain of tasks: A → B → C → D → E

use criterion::Criterion;
use futures::FutureExt;

pub fn bench_linear_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("linear_pipeline_5_tasks");

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::{task_fn, DagRunner};

                let dag = DagRunner::new();

                let a = dag.add_task(task_fn(|_: ()| async { 1 }));
                let b = dag
                    .add_task(task_fn(|x: i32| async move { x + 1 }))
                    .depends_on(a);
                let c = dag
                    .add_task(task_fn(|x: i32| async move { x * 2 }))
                    .depends_on(b);
                let d = dag
                    .add_task(task_fn(|x: i32| async move { x - 1 }))
                    .depends_on(c);
                let e = dag
                    .add_task(task_fn(|x: i32| async move { x * 3 }))
                    .depends_on(d);

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
                dag.get(e).unwrap()
            })
        });
    });

    // dagrs implementation
    group.bench_function("dagrs", |b| {
        b.iter(|| {
            use async_trait::async_trait;
            use dagrs::*;
            use std::sync::Arc;

            struct TaskA;
            #[async_trait]
            impl Action for TaskA {
                async fn run(
                    &self,
                    _: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    out.broadcast(Content::new(1i32)).await;
                    Output::Out(Some(Content::new(1i32)))
                }
            }

            struct TaskB;
            #[async_trait]
            impl Action for TaskB {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] + 1;
                    out.broadcast(Content::new(result)).await;
                    Output::Out(Some(Content::new(result)))
                }
            }

            struct TaskC;
            #[async_trait]
            impl Action for TaskC {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] * 2;
                    out.broadcast(Content::new(result)).await;
                    Output::Out(Some(Content::new(result)))
                }
            }

            struct TaskD;
            #[async_trait]
            impl Action for TaskD {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] - 1;
                    out.broadcast(Content::new(result)).await;
                    Output::Out(Some(Content::new(result)))
                }
            }

            struct TaskE;
            #[async_trait]
            impl Action for TaskE {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    _: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] * 3;
                    Output::Out(Some(Content::new(result)))
                }
            }

            let mut table = NodeTable::new();
            let a = DefaultNode::with_action("a".to_string(), TaskA, &mut table);
            let a_id = a.id();
            let b = DefaultNode::with_action("b".to_string(), TaskB, &mut table);
            let b_id = b.id();
            let c = DefaultNode::with_action("c".to_string(), TaskC, &mut table);
            let c_id = c.id();
            let d = DefaultNode::with_action("d".to_string(), TaskD, &mut table);
            let d_id = d.id();
            let e = DefaultNode::with_action("e".to_string(), TaskE, &mut table);
            let e_id = e.id();

            let mut graph = Graph::new();
            graph.add_node(a);
            graph.add_node(b);
            graph.add_node(c);
            graph.add_node(d);
            graph.add_node(e);

            graph.add_edge(a_id, vec![b_id]);
            graph.add_edge(b_id, vec![c_id]);
            graph.add_edge(c_id, vec![d_id]);
            graph.add_edge(d_id, vec![e_id]);

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
