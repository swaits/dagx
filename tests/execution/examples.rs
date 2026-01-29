// Example pattern tests for DagRunner
use crate::common::task_fn;

use crate::common::tasks::*;
use dagx::*;
use futures::FutureExt;

#[tokio::test]
async fn test_example_a_fan_out() {
    let dag = DagRunner::new();

    let base: TaskHandle<_> = dag.add_task(Seed::new(10)).into(); // Input=() → no depends_on, returns TaskHandle
    let plus1 = dag.add_task(Inc::new()).depends_on(base); // 11
    let plus2 = dag.add_task(Inc::new()).depends_on(base); // 11

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap(); // SPEC API with spawner

    assert_eq!(dag.get(plus1).unwrap(), 11);
    assert_eq!(dag.get(plus2).unwrap(), 11);
}

#[tokio::test]
async fn test_example_b_fan_in() {
    let dag = DagRunner::new();

    let s = dag.add_task(task_fn(|_: ()| async { "hi".to_owned() }));
    let n = dag.add_task(task_fn(|_: ()| async { 42usize }));
    let f = dag.add_task(task_fn(|_: ()| async { true }));

    let output = dag.add_task(OutputResult::new()).depends_on((s, n, f));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(&dag.get(output).unwrap(), "hi:42:true");
}

#[tokio::test]
async fn test_example_c_many_to_many() {
    let dag = DagRunner::new();

    let x = dag.add_task(Seed::new(2));
    let y = dag.add_task(Seed::new(3)).into();
    let z = dag.add_task(Seed::new(5));

    let sum_xy = dag.add_task(Add::new()).depends_on((x, &y)); // 5
    let prod_yz = dag.add_task(Mul::new()).depends_on((&y, z)); // 15
    let total = dag.add_task(Add::new()).depends_on((&sum_xy, &prod_yz)); // 20

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(dag.get(total).unwrap(), 20);
}

#[tokio::test]
async fn test_single_input_forms() {
    let dag = DagRunner::new();

    let s = dag
        .add_task(task_fn(|_: ()| async { "hello".to_owned() }))
        .into();

    // Preferred form: &node
    let upper1 = dag
        .add_task(task_fn(|s: String| async move { s.to_uppercase() }))
        .depends_on(s);

    // Also allowed: (&node,)
    let upper2 = dag
        .add_task(task_fn(|s: String| async move { s.to_uppercase() }))
        .depends_on((&s,));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(&dag.get(upper1).unwrap(), "HELLO");
    assert_eq!(&dag.get(upper2).unwrap(), "HELLO");
}
