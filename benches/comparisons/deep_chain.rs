//! Deep chain comparison: dagx vs dagrs
//!
//! Long sequential chain: 100 tasks in sequence

use criterion::Criterion;
use dagx::TaskHandle;
use futures::FutureExt;

pub fn bench_deep_chain(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("deep_chain_100_tasks");

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::{task_fn, DagRunner};

                let dag = DagRunner::new();

                let first: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| { 0 })).into();
                let mut prev = dag
                    .add_task(task_fn::<i32, _, _>(|&x: &i32| { x + 1 }))
                    .depends_on(first);

                for i in 2..100 {
                    prev = dag
                        .add_task(task_fn::<i32, _, _>(move |&x: &i32| { x + i }))
                        .depends_on(prev);
                }

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
                dag.get(prev).unwrap()
            })
        });
    });

    // dagrs implementation
    group.bench_function("dagrs", |b| {
        b.iter(|| {
            use async_trait::async_trait;
            use dagrs::*;
            use std::sync::Arc;

            struct Initial;
            #[async_trait]
            impl Action for Initial {
                async fn run(
                    &self,
                    _: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    out.broadcast(Content::new(0i32)).await;
                    Output::Out(Some(Content::new(0i32)))
                }
            }

            struct Increment(i32);
            #[async_trait]
            impl Action for Increment {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] + self.0;
                    out.broadcast(Content::new(result)).await;
                    Output::Out(Some(Content::new(result)))
                }
            }

            let mut table = NodeTable::new();
            let mut nodes = vec![];
            let mut node_ids = vec![];

            let initial = DefaultNode::with_action("task_0".to_string(), Initial, &mut table);
            node_ids.push(initial.id());
            nodes.push(initial);

            for i in 1..100 {
                let node =
                    DefaultNode::with_action(format!("task_{}", i), Increment(i), &mut table);
                node_ids.push(node.id());
                nodes.push(node);
            }

            let mut graph = Graph::new();
            for node in nodes {
                graph.add_node(node);
            }

            for i in 1..100 {
                graph.add_edge(node_ids[i - 1], vec![node_ids[i]]);
            }

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
