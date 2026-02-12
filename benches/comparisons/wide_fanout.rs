//! Wide fanout comparison: dagx vs dagrs
//!
//! One source task feeding 100 dependent tasks

use criterion::Criterion;
use dagx::TaskHandle;

pub fn bench_wide_fanout(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("wide_fanout_100_tasks");

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::DagRunner;
                use dagx_test::task_fn;

                let dag = DagRunner::new();
                let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 42)).into();

                // Create 100 tasks that all depend on source
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

    // dagrs implementation
    group.bench_function("dagrs", |b| {
        b.iter(|| {
            use async_trait::async_trait;
            use dagrs::*;
            use std::sync::Arc;

            struct Source;
            #[async_trait]
            impl Action for Source {
                async fn run(
                    &self,
                    _: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    out.broadcast(Content::new(42i32)).await;
                    Output::Out(Some(Content::new(42i32)))
                }
            }

            struct Consumer(i32);
            #[async_trait]
            impl Action for Consumer {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    _: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let vals: Vec<i32> = input
                        .map(|c| *c.unwrap().into_inner::<i32>().unwrap())
                        .await;
                    let result = vals[0] + self.0;
                    Output::Out(Some(Content::new(result)))
                }
            }

            let mut table = NodeTable::new();
            let source = DefaultNode::with_action("source".to_string(), Source, &mut table);
            let source_id = source.id();

            let mut graph = Graph::new();
            graph.add_node(source);

            let mut consumer_ids = Vec::new();
            for i in 0..100 {
                let consumer =
                    DefaultNode::with_action(format!("consumer_{}", i), Consumer(i), &mut table);
                consumer_ids.push(consumer.id());
                graph.add_node(consumer);
            }

            graph.add_edge(source_id, consumer_ids);

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
