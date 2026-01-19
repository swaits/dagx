//! Runtime integration tests - verify dagx works correctly across different runtimes

use dagx::{DagResult, DagRunner};
use futures::FutureExt;

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

#[tokio::test]
async fn runtime_integration_tokio() -> DagResult<()> {
    let dag = DagRunner::new();

    let a = dag.add_task(Value(10));
    let b = dag.add_task(Value(20));
    let sum = dag.add_task(Add).depends_on((&a, &b));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 30);
    Ok(())
}
