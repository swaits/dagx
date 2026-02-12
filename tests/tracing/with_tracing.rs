//! Tests with tracing feature enabled

use dagx::{task, DagRunner};

use tracing_subscriber::{fmt, EnvFilter};

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

struct Multiply;

#[task]
impl Multiply {
    async fn run(a: &i32, b: &i32) -> i32 {
        a * b
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tracing_with_subscriber() {
    // Initialize tracing subscriber for this test
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

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
async fn test_tracing_inline_execution() {
    // Initialize tracing subscriber
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let dag = DagRunner::new();

    // Create a linear chain (should trigger inline execution)
    let a = dag.add_task(Value(1)).into();
    let b = dag.add_task(Add).depends_on((&a, &a));
    let c = dag.add_task(Multiply).depends_on((&b, &b));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // a = 1
    // b = 1 + 1 = 2
    // c = 2 * 2 = 4
    assert_eq!(dag.get(c).unwrap(), 4);
}
