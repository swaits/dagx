//! Runtime compatibility tests for async-std

use crate::common::task_fn;
use dagx::{DagRunner, Task};

struct Value(i32);
#[dagx::task]
impl Value {
    async fn run(&mut self) -> i32 {
        self.0
    }
}

struct Add;
#[dagx::task]
impl Add {
    async fn run(&mut self, a: &i32, b: &i32) -> i32 {
        a + b
    }
}

#[async_std::test]
async fn test_basic_dag_async_std() {
    let dag = DagRunner::new();

    let x = dag.add_task(Value(2));
    let y = dag.add_task(Value(3));
    let sum = dag.add_task(Add).depends_on((&x, &y));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();

    assert_eq!(dag.get(sum).unwrap(), 5);
}

#[async_std::test]
async fn test_parallel_execution_async_std() {
    let dag = DagRunner::new();

    let tasks: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i * 2 })))
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();

    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task).unwrap(), i * 2);
    }
}

#[async_std::test]
async fn test_complex_dependencies_async_std() {
    let dag = DagRunner::new();

    let a = dag.add_task(Value(10));
    let b = dag.add_task(Value(20));
    let sum = dag.add_task(Add).depends_on((&a, &b));
    let double = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(sum);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();

    assert_eq!(dag.get(double).unwrap(), 60);
}
