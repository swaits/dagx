//! Realistic ETL pipeline comparison: dagx vs dagrs
//!
//! Extract → Transform → Validate → Load pattern

use criterion::Criterion;
use futures::FutureExt;

pub fn bench_etl_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("etl_pipeline");

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::{task_fn, DagRunner};

                let dag = DagRunner::new();

                // Extract from 3 sources
                let source1 = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3, 4, 5] }));
                let source2 = dag.add_task(task_fn(|_: ()| async { vec![6, 7, 8, 9, 10] }));
                let source3 = dag.add_task(task_fn(|_: ()| async { vec![11, 12, 13, 14, 15] }));

                // Transform each source
                let transform1 = dag
                    .add_task(task_fn(|data: Vec<i32>| async move {
                        data.into_iter().map(|x| x * 2).collect::<Vec<_>>()
                    }))
                    .depends_on(&source1);

                let transform2 = dag
                    .add_task(task_fn(|data: Vec<i32>| async move {
                        data.into_iter().map(|x| x * 2).collect::<Vec<_>>()
                    }))
                    .depends_on(&source2);

                let transform3 = dag
                    .add_task(task_fn(|data: Vec<i32>| async move {
                        data.into_iter().map(|x| x * 2).collect::<Vec<_>>()
                    }))
                    .depends_on(&source3);

                // Merge all transformed data
                let merge = dag
                    .add_task(task_fn(
                        |(d1, d2, d3): (Vec<i32>, Vec<i32>, Vec<i32>)| async move {
                            let mut result = d1;
                            result.extend(d2);
                            result.extend(d3);
                            result
                        },
                    ))
                    .depends_on((&transform1, &transform2, &transform3));

                // Validate
                let validate = dag
                    .add_task(task_fn(|data: Vec<i32>| async move {
                        data.into_iter().filter(|&x| x > 0).collect::<Vec<_>>()
                    }))
                    .depends_on(merge);

                // Load (compute sum)
                let load = dag
                    .add_task(task_fn(|data: Vec<i32>| async move {
                        data.into_iter().sum::<i32>()
                    }))
                    .depends_on(validate);

                dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
                    .await
                    .unwrap();
                dag.get(load).unwrap()
            })
        });
    });

    // dagrs implementation
    group.bench_function("dagrs", |b| {
        b.iter(|| {
            use async_trait::async_trait;
            use dagrs::*;
            use std::sync::Arc;

            struct Extract(Vec<i32>);
            #[async_trait]
            impl Action for Extract {
                async fn run(
                    &self,
                    _: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    out.broadcast(Content::new(self.0.clone())).await;
                    Output::Out(Some(Content::new(self.0.clone())))
                }
            }

            struct Transform;
            #[async_trait]
            impl Action for Transform {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let inputs: Vec<Vec<i32>> = input
                        .map(|c| (**c.unwrap().into_inner::<Vec<i32>>().unwrap()).to_vec())
                        .await;
                    let data = &inputs[0];
                    let transformed: Vec<i32> = data.iter().map(|&x| x * 2).collect();
                    out.broadcast(Content::new(transformed.clone())).await;
                    Output::Out(Some(Content::new(transformed)))
                }
            }

            struct Merge;
            #[async_trait]
            impl Action for Merge {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let inputs: Vec<Vec<i32>> = input
                        .map(|c| (**c.unwrap().into_inner::<Vec<i32>>().unwrap()).to_vec())
                        .await;
                    let mut result = Vec::new();
                    for input in inputs {
                        result.extend(input);
                    }
                    out.broadcast(Content::new(result.clone())).await;
                    Output::Out(Some(Content::new(result)))
                }
            }

            struct Validate;
            #[async_trait]
            impl Action for Validate {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    out: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let inputs: Vec<Vec<i32>> = input
                        .map(|c| (**c.unwrap().into_inner::<Vec<i32>>().unwrap()).to_vec())
                        .await;
                    let data = &inputs[0];
                    let validated: Vec<i32> = data.iter().filter(|&&x| x > 0).copied().collect();
                    out.broadcast(Content::new(validated.clone())).await;
                    Output::Out(Some(Content::new(validated)))
                }
            }

            struct Load;
            #[async_trait]
            impl Action for Load {
                async fn run(
                    &self,
                    input: &mut InChannels,
                    _: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let inputs: Vec<Vec<i32>> = input
                        .map(|c| (**c.unwrap().into_inner::<Vec<i32>>().unwrap()).to_vec())
                        .await;
                    let data = &inputs[0];
                    let sum: i32 = data.iter().sum();
                    Output::Out(Some(Content::new(sum)))
                }
            }

            let mut table = NodeTable::new();

            let source1 = DefaultNode::with_action(
                "source1".to_string(),
                Extract(vec![1, 2, 3, 4, 5]),
                &mut table,
            );
            let source1_id = source1.id();
            let source2 = DefaultNode::with_action(
                "source2".to_string(),
                Extract(vec![6, 7, 8, 9, 10]),
                &mut table,
            );
            let source2_id = source2.id();
            let source3 = DefaultNode::with_action(
                "source3".to_string(),
                Extract(vec![11, 12, 13, 14, 15]),
                &mut table,
            );
            let source3_id = source3.id();

            let transform1 =
                DefaultNode::with_action("transform1".to_string(), Transform, &mut table);
            let transform1_id = transform1.id();
            let transform2 =
                DefaultNode::with_action("transform2".to_string(), Transform, &mut table);
            let transform2_id = transform2.id();
            let transform3 =
                DefaultNode::with_action("transform3".to_string(), Transform, &mut table);
            let transform3_id = transform3.id();

            let merge = DefaultNode::with_action("merge".to_string(), Merge, &mut table);
            let merge_id = merge.id();
            let validate = DefaultNode::with_action("validate".to_string(), Validate, &mut table);
            let validate_id = validate.id();
            let load = DefaultNode::with_action("load".to_string(), Load, &mut table);
            let load_id = load.id();

            let mut graph = Graph::new();
            graph.add_node(source1);
            graph.add_node(source2);
            graph.add_node(source3);
            graph.add_node(transform1);
            graph.add_node(transform2);
            graph.add_node(transform3);
            graph.add_node(merge);
            graph.add_node(validate);
            graph.add_node(load);

            graph.add_edge(source1_id, vec![transform1_id]);
            graph.add_edge(source2_id, vec![transform2_id]);
            graph.add_edge(source3_id, vec![transform3_id]);
            graph.add_edge(transform1_id, vec![merge_id]);
            graph.add_edge(transform2_id, vec![merge_id]);
            graph.add_edge(transform3_id, vec![merge_id]);
            graph.add_edge(merge_id, vec![validate_id]);
            graph.add_edge(validate_id, vec![load_id]);

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
