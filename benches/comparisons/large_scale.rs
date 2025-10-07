//! Large scale comparison: dagx vs dagrs
//!
//! 10,000 independent tasks to test scaling

use criterion::Criterion;

pub fn bench_large_scale(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("large_scale_10k_tasks");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(20));

    // dagx implementation
    group.bench_function("dagx", |b| {
        b.iter(|| {
            rt.block_on(async {
                use dagx::{task_fn, DagRunner};

                let dag = DagRunner::new();
                for i in 0..10_000 {
                    dag.add_task(task_fn(move |_: ()| async move { i * 2 }));
                }

                dag.run(|fut| {
                    tokio::spawn(fut);
                })
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

            struct Task(i32);
            #[async_trait]
            impl Action for Task {
                async fn run(
                    &self,
                    _: &mut InChannels,
                    _: &mut OutChannels,
                    _: Arc<EnvVar>,
                ) -> Output {
                    let result = self.0 * 2;
                    Output::Out(Some(Content::new(result)))
                }
            }

            let mut table = NodeTable::new();
            let mut graph = Graph::new();

            for i in 0..10_000 {
                let node = DefaultNode::with_action(format!("task_{}", i), Task(i), &mut table);
                graph.add_node(node);
            }

            graph.set_env(EnvVar::new(table));
            graph.start().unwrap();
        });
    });

    group.finish();
}
