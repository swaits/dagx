//! Diamond pattern comparison: dagx vs dagrs
//!
//! Classic diamond: A → B,C → D

use criterion::Criterion;

pub fn bench_diamond_pattern(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("diamond_pattern");

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::{task_fn, DagRunner};

                let dag = DagRunner::new();

                let a = dag.add_task(task_fn(|_: ()| async { 10 }));
                let b = dag
                    .add_task(task_fn(|x: i32| async move { x * 2 }))
                    .depends_on(&a);
                let c = dag
                    .add_task(task_fn(|x: i32| async move { x + 5 }))
                    .depends_on(&a);
                let d = dag
                    .add_task(task_fn(|(x, y): (i32, i32)| async move { x + y }))
                    .depends_on((&b, &c));

                dag.run(|fut| {
                    tokio::spawn(fut);
                })
                .await
                .unwrap();
                dag.get(d).unwrap()
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
                    out.broadcast(Content::new(10i32)).await;
                    Output::Out(Some(Content::new(10i32)))
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
                    let result = vals[0] * 2;
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
                    let result = vals[0] + 5;
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
                    _: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result: i32 = vals.iter().sum();
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

            let mut graph = Graph::new();
            graph.add_node(a);
            graph.add_node(b);
            graph.add_node(c);
            graph.add_node(d);

            graph.add_edge(a_id, vec![b_id, c_id]);
            graph.add_edge(b_id, vec![d_id]);
            graph.add_edge(c_id, vec![d_id]);

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
