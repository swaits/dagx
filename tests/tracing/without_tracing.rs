//! Tests that the library works correctly without tracing feature

use dagx::{task, DagRunner};

struct Value(i32);

#[task]
impl Value {
    async fn run(&self) -> i32 {
        self.0
    }
}

struct Add;

#[task]
impl Add {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_basic_dag_without_tracing() {
    let dag = DagRunner::new();

    let a = dag.add_task(Value(2));
    let b = dag.add_task(Value(3));
    let sum = dag.add_task(Add).depends_on((a, b));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(dag.get(sum).unwrap(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complex_dag_without_tracing() {
    let dag = DagRunner::new();

    // Create multiple layers
    let v1 = dag.add_task(Value(1));
    let v2 = dag.add_task(Value(2));
    let v3 = dag.add_task(Value(3));
    let v4 = dag.add_task(Value(4));

    let sum12 = dag.add_task(Add).depends_on((v1, v2));
    let sum34 = dag.add_task(Add).depends_on((v3, v4));
    let final_sum = dag.add_task(Add).depends_on((&sum12, &sum34));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(dag.get(final_sum).unwrap(), 10);
}
