//! DAG creation benchmarks

use criterion::Criterion;
use dagx::DagRunner;
use std::hint::black_box;

// Simple task for benchmarking
struct Value(i32);

#[dagx::task]
impl Value {
    async fn run(&self) -> i32 {
        self.0
    }
}

pub fn bench_dag_creation(c: &mut Criterion) {
    c.bench_function("create_empty_dag", |b| {
        b.iter(|| black_box(DagRunner::new()));
    });

    c.bench_function("add_100_tasks", |b| {
        b.iter(|| {
            let dag = DagRunner::new();
            for i in 0..100 {
                black_box(dag.add_task(Value(i)));
            }
        });
    });
}
