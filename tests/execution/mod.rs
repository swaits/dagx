// Basic execution tests for DagRunner
use dagx::task_fn;

use dagx::*;

#[tokio::test]
async fn test_empty_dag() {
    let dag = DagRunner::new();
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap(); // Should complete without error
}

#[tokio::test]
async fn test_single_task() {
    let dag = DagRunner::new();
    let task = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));
    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();
    assert_eq!(dag.get(task).unwrap(), 42);
}

#[tokio::test]
async fn test_wide_parallel() {
    let dag = DagRunner::new();

    let tasks: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 2)))
        .collect();

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(dag.get(task).unwrap(), i * 2);
    }
}

#[tokio::test]
async fn test_diamond_dependency() {
    let dag = DagRunner::new();

    let a: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 10)).into();
    let b = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(a);
    let c = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(a);
    let d = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(x, y): (&i32, &i32)| x + y))
        .depends_on((&b, &c));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
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

    let string_task = dag.add_task(task_fn::<(), _, _>(|_: ()| "hello".to_string()));
    let vec_task = dag.add_task(task_fn::<(), _, _>(|_: ()| vec![1, 2, 3]));
    let struct_task = dag.add_task(task_fn::<(), _, _>(|_: ()| User {
        name: "Alice".to_string(),
        age: 30,
    }));

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
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

    let source: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 10)).into();

    // Branch 1
    let branch1 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(source);
    let sink1 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 1))
        .depends_on(branch1);

    // Branch 2
    let branch2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(source);
    let sink2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 3))
        .depends_on(branch2);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap(); // Should wait for both sinks

    assert_eq!(dag.get(sink1).unwrap(), 21); // (10 * 2) + 1
    assert_eq!(dag.get(sink2).unwrap(), 45); // (10 + 5) * 3
}

#[tokio::test]
async fn test_single_task_inline_path() {
    // Test single-task layer inline execution path (line 367-410 in runner.rs)
    let dag = DagRunner::new();

    // Single task in its own layer
    let t1: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 10)).into();
    let t2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(t1);
    let t3 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(t2);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(dag.get(t3).unwrap(), 25); // (10 * 2) + 5
}

#[tokio::test]
async fn test_multi_consumer_fanout() {
    // Test fanout with multiple consumers from one producer
    let dag = DagRunner::new();

    let producer: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 42)).into();

    // Multiple consumers
    let c1 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(producer);
    let c2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 10))
        .depends_on(producer);
    let c3 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x - 5))
        .depends_on(producer);

    dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(dag.get(c1).unwrap(), 84);
    assert_eq!(dag.get(c2).unwrap(), 52);
    assert_eq!(dag.get(c3).unwrap(), 37);
}
