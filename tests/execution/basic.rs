// Basic execution tests for DagRunner
use crate::common::task_fn;

use dagx::*;

#[tokio::test]
async fn test_empty_dag() {
    let dag = DagRunner::new();
    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap(); // Should complete without error
}

#[tokio::test]
async fn test_single_task() {
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn(|_: ()| async { 42 }));
    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(task).unwrap(), 42);
}

#[tokio::test]
async fn test_deep_chain() {
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 1 }));
    let b = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(&a);
    let c = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(b);
    let d = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(c);
    let e = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(d);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Note: a, b, c, d are not sinks (they have dependents in the chain)
    // Only e is a sink and can be retrieved
    assert_eq!(dag.get(e).unwrap(), 5);
}

#[tokio::test]
async fn test_wide_parallel() {
    let dag = DagRunner::new();

    let tasks: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i * 2 })))
        .collect();

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(dag.get(task).unwrap(), i * 2);
    }
}

#[tokio::test]
async fn test_diamond_dependency() {
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 10 }));
    let b = dag
        .add_task(task_fn(|x: i32| async move { x + 5 }))
        .depends_on(&a);
    let c = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(&a);
    let d = dag
        .add_task(task_fn(|(x, y): (i32, i32)| async move { x + y }))
        .depends_on((&b, &c));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Note: a, b, and c are not sinks (they have dependents)
    // Only d is a sink and can be retrieved
    assert_eq!(dag.get(d).unwrap(), 35); // 15 + 20
}

#[derive(Clone)]
struct User {
    name: String,
    age: u32,
}

#[tokio::test]
async fn test_different_output_types() {
    let dag = DagRunner::new();

    let string_task = dag.add_task(task_fn(|_: ()| async { "hello".to_string() }));
    let vec_task = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3] }));
    let struct_task = dag.add_task(task_fn(|_: ()| async {
        User {
            name: "Alice".to_string(),
            age: 30,
        }
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    assert_eq!(&dag.get(string_task).unwrap(), "hello");
    assert_eq!(dag.get(vec_task).unwrap(), vec![1, 2, 3]);
    let user = dag.get(struct_task).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[tokio::test]
async fn test_multiple_sinks() {
    let dag = DagRunner::new();

    let source = dag.add_task(task_fn(|_: ()| async { 10 }));

    // Branch 1
    let branch1 = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(&source);
    let sink1 = dag
        .add_task(task_fn(|x: i32| async move { x + 1 }))
        .depends_on(branch1);

    // Branch 2
    let branch2 = dag
        .add_task(task_fn(|x: i32| async move { x + 5 }))
        .depends_on(&source);
    let sink2 = dag
        .add_task(task_fn(|x: i32| async move { x * 3 }))
        .depends_on(branch2);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap(); // Should wait for both sinks

    assert_eq!(dag.get(sink1).unwrap(), 21); // (10 * 2) + 1
    assert_eq!(dag.get(sink2).unwrap(), 45); // (10 + 5) * 3
}
