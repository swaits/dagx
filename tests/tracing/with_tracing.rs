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
async fn test_tracing_with_complex_dag() {
    // Initialize tracing subscriber
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let dag = DagRunner::new();

    // Create a diamond pattern
    let source = dag.add_task(Value(10)).into();
    let left = dag.add_task(Add).depends_on((&source, &source));
    let right = dag.add_task(Multiply).depends_on((&source, &source));
    let sink = dag.add_task(Add).depends_on((&left, &right));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // source = 10
    // left = 10 + 10 = 20
    // right = 10 * 10 = 100
    // sink = 20 + 100 = 120
    assert_eq!(dag.get(sink).unwrap(), 120);
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

#[tokio::test(flavor = "multi_thread")]
async fn test_tracing_multiple_layers() {
    // Initialize tracing subscriber
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .with_test_writer()
        .try_init();

    let dag = DagRunner::new();

    // Layer 0
    let v1 = dag.add_task(Value(1));
    let v2 = dag.add_task(Value(2)).into();
    let v3 = dag.add_task(Value(3));

    // Layer 1
    let sum12 = dag.add_task(Add).depends_on((v1, &v2));
    let mul23 = dag.add_task(Multiply).depends_on((&v2, v3));

    // Layer 2
    let final_sum = dag.add_task(Add).depends_on((&sum12, &mul23));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // v1 = 1, v2 = 2, v3 = 3
    // sum12 = 1 + 2 = 3
    // mul23 = 2 * 3 = 6
    // final_sum = 3 + 6 = 9
    assert_eq!(dag.get(final_sum).unwrap(), 9);
}
