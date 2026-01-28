// Stateless task pattern tests
use crate::common::task_fn;
use futures::FutureExt;

use dagx::*;

// Stateless task structs
struct StatelessAdd;

#[task]
impl StatelessAdd {
    async fn run(a: &i32, b: &i32) -> i32 {
        a + b
    }
}

struct StatelessDouble;

#[task]
impl StatelessDouble {
    async fn run(x: &i32) -> i32 {
        x * 2
    }
}

struct StatelessSource;

#[task]
impl StatelessSource {
    async fn run() -> i32 {
        42
    }
}

struct StatelessFormat;

#[task]
impl StatelessFormat {
    async fn run(n: &i32, s: &String, b: &bool) -> String {
        format!("{}: {} ({})", s, n, b)
    }
}

struct StatelessMultiply;

#[task]
impl StatelessMultiply {
    async fn run(a: &i32, b: &i32) -> i32 {
        a * b
    }
}

struct StatelessInc;

#[task]
impl StatelessInc {
    async fn run(x: &i32) -> i32 {
        x + 1
    }
}

struct StatelessSquare;

#[task]
impl StatelessSquare {
    async fn run(x: &i32) -> i32 {
        x * x
    }
}

struct StatelessConcat;

#[task]
impl StatelessConcat {
    async fn run(a: &String, b: &String) -> String {
        format!("{}{}", a, b)
    }
}

#[tokio::test]
async fn test_stateless_basic() {
    // Test basic stateless task with two inputs
    let dag = DagRunner::new();

    let x = dag.add_task(task_fn(|_: ()| async { 10 }));
    let y = dag.add_task(task_fn(|_: ()| async { 20 }));
    let sum = dag.add_task(StatelessAdd).depends_on((&x, &y));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(dag.get(sum).unwrap(), 30);
}

#[tokio::test]
async fn test_stateless_single_input() {
    // Test stateless task with single input
    let dag = DagRunner::new();

    let input = dag.add_task(task_fn(|_: ()| async { 21 }));
    let doubled = dag.add_task(StatelessDouble).depends_on(&input);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(dag.get(doubled).unwrap(), 42);
}

#[tokio::test]
async fn test_stateless_no_input() {
    // Test stateless source task with no inputs
    let dag = DagRunner::new();

    let source = dag.add_task(StatelessSource);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(dag.get(source).unwrap(), 42);
}

#[tokio::test]
async fn test_stateless_multiple_inputs() {
    // Test stateless task with multiple inputs of different types
    let dag = DagRunner::new();

    let num = dag.add_task(task_fn(|_: ()| async { 42 }));
    let text = dag.add_task(task_fn(|_: ()| async { "test".to_string() }));
    let flag = dag.add_task(task_fn(|_: ()| async { true }));

    let result = dag
        .add_task(StatelessFormat)
        .depends_on((&num, &text, &flag));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    assert_eq!(dag.get(result).unwrap(), "test: 42 (true)");
}

#[tokio::test]
async fn test_stateless_mixed_with_stateful() {
    // Test mixing stateless and stateful tasks
    // Define Counter inline for this test
    struct Counter {
        count: i32,
    }

    impl Counter {
        fn new(initial: i32) -> Self {
            Self { count: initial }
        }
    }

    #[task]
    impl Counter {
        async fn run(&mut self, input: &i32) -> i32 {
            self.count += input;
            self.count
        }
    }

    let dag = DagRunner::new();

    let x = dag.add_task(task_fn(|_: ()| async { 5 }));
    let y = dag.add_task(task_fn(|_: ()| async { 3 }));

    // Stateless multiplication
    let product = dag.add_task(StatelessMultiply).depends_on((&x, &y));

    // Stateful counter
    let counted = dag.add_task(Counter::new(10)).depends_on(product);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(product).unwrap(), 15);
    assert_eq!(dag.get(counted).unwrap(), 25); // 10 + 15
}

#[tokio::test]
async fn test_stateless_chain() {
    // Test chaining stateless tasks
    let dag = DagRunner::new();

    let start = dag.add_task(task_fn(|_: ()| async { 0 }));
    let step1 = dag.add_task(StatelessInc).depends_on(&start);
    let step2 = dag.add_task(StatelessInc).depends_on(step1);
    let step3 = dag.add_task(StatelessInc).depends_on(step2);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(step1).unwrap(), 1);
    assert_eq!(dag.get(step2).unwrap(), 2);
    assert_eq!(dag.get(step3).unwrap(), 3);
}

#[tokio::test]
async fn test_stateless_parallel() {
    // Test parallel execution of stateless tasks
    let dag = DagRunner::new();

    let tasks: Vec<_> = (1..=5)
        .map(|i| {
            let source = dag.add_task(task_fn(move |_: ()| async move { i }));
            dag.add_task(StatelessSquare).depends_on(&source)
        })
        .collect();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(tasks[0]).unwrap(), 1);
    assert_eq!(dag.get(tasks[1]).unwrap(), 4);
    assert_eq!(dag.get(tasks[2]).unwrap(), 9);
    assert_eq!(dag.get(tasks[3]).unwrap(), 16);
    assert_eq!(dag.get(tasks[4]).unwrap(), 25);
}

#[tokio::test]
async fn test_stateless_with_string_types() {
    // Test stateless tasks with complex types
    let dag = DagRunner::new();

    let hello = dag.add_task(task_fn(|_: ()| async { "Hello, ".to_string() }));
    let world = dag.add_task(task_fn(|_: ()| async { "World!".to_string() }));

    let greeting = dag.add_task(StatelessConcat).depends_on((&hello, &world));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(greeting).unwrap(), "Hello, World!");
}
