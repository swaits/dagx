// Basic execution tests for DagRunner
use dagx_test::task_fn;

use dagx::*;

#[tokio::test]
async fn test_empty_dag() {
    let dag = DagRunner::new();
    let _ = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap(); // Should complete without error
}

#[tokio::test]
async fn test_single_task() {
    let mut dag = DagRunner::new();
    let task = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));
    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();
    assert_eq!(output.get(task).unwrap(), 42);
}

#[tokio::test]
async fn test_wide_parallel() {
    let mut dag = DagRunner::new();

    let tasks: Vec<_> = (0..10)
        .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 2)))
        .collect();

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    for (i, task) in tasks.into_iter().enumerate() {
        assert_eq!(output.get(task).unwrap(), i * 2);
    }
}

#[tokio::test]
async fn test_diamond_dependency() {
    let mut dag = DagRunner::new();

    let a = dag.add_task(task_fn::<(), _, _>(|_: ()| 10));
    let b = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(a);
    let c = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(a);
    let d = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(x, y): (&i32, &i32)| x + y))
        .depends_on((b, c));

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    // Note: a, b, and c are not sinks (they have dependents)
    // Only d is a sink and can be retrieved
    assert_eq!(output.get(d).unwrap(), 35); // 15 + 20
}

#[derive(Clone)]
struct User {
    name: String,
    age: u32,
}

#[tokio::test]
async fn test_different_output_types() {
    let mut dag = DagRunner::new();

    let string_task = dag.add_task(task_fn::<(), _, _>(|_: ()| "hello".to_string()));
    let vec_task = dag.add_task(task_fn::<(), _, _>(|_: ()| vec![1, 2, 3]));
    let struct_task = dag.add_task(task_fn::<(), _, _>(|_: ()| User {
        name: "Alice".to_string(),
        age: 30,
    }));

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(&output.get(string_task).unwrap(), "hello");
    assert_eq!(output.get(vec_task).unwrap(), vec![1, 2, 3]);
    let user = output.get(struct_task).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[tokio::test]
async fn test_multiple_sinks() {
    let mut dag = DagRunner::new();

    let source = dag.add_task(task_fn::<(), _, _>(|_: ()| 10));

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

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap(); // Should wait for both sinks

    assert_eq!(output.get(sink1).unwrap(), 21); // (10 * 2) + 1
    assert_eq!(output.get(sink2).unwrap(), 45); // (10 + 5) * 3
}

#[tokio::test]
async fn test_single_task_inline_path() {
    // Test single-task layer inline execution path (line 367-410 in runner.rs)
    let mut dag = DagRunner::new();

    // Single task in its own layer
    let t1 = dag.add_task(task_fn::<(), _, _>(|_: ()| 10));
    let t2 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(t1);
    let t3 = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x + 5))
        .depends_on(t2);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(output.get(t3).unwrap(), 25); // (10 * 2) + 5
}

#[tokio::test]
async fn test_multi_consumer_fanout() {
    // Test fanout with multiple consumers from one producer
    let mut dag = DagRunner::new();

    let producer = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));

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

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(output.get(c1).unwrap(), 84);
    assert_eq!(output.get(c2).unwrap(), 52);
    assert_eq!(output.get(c3).unwrap(), 37);
}
