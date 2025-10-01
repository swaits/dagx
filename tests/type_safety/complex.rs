// Complex type safety tests for DagRunner
use crate::common::task_fn;

use dagx::*;

#[tokio::test]
async fn test_complex_type_safety() {
    // Verify type safety with complex types
    #[derive(Debug, Clone, PartialEq)]
    struct CustomType {
        value: i32,
        name: String,
    }

    let dag = DagRunner::new();

    let custom_task = dag.add_task(task_fn(|_: ()| async {
        CustomType {
            value: 42,
            name: "test".to_string(),
        }
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Correct type works
    let custom = dag.get(&custom_task).unwrap();
    assert_eq!(custom.value, 42);

    // Type safety is enforced at compile time - we cannot get with wrong types
}

#[tokio::test]
async fn test_generic_type_safety() {
    // Verify type safety with generics
    let dag = DagRunner::new();

    // Task returning Vec<i32>
    let vec_i32 = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3] }));

    // Task returning Vec<String>
    let vec_string = dag.add_task(task_fn(|_: ()| async {
        vec!["a".to_string(), "b".to_string()]
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Correct types work
    assert_eq!(dag.get(&vec_i32).unwrap(), vec![1, 2, 3]);
    assert!(dag.get(&vec_string).is_ok());
}

#[tokio::test]
async fn test_option_result_type_safety() {
    // Verify type safety with wrapped types
    let dag = DagRunner::new();

    // Task returning Option<i32>
    let opt_task = dag.add_task(task_fn(|_: ()| async { Some(42) }));

    // Task returning Result<i32, String>
    let result_task = dag.add_task(task_fn(|_: ()| async { Ok::<i32, String>(42) }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Correct wrapped types work
    assert_eq!(dag.get(&opt_task).unwrap(), Some(42));
    assert!(dag.get(&result_task).is_ok());
}

#[tokio::test]
async fn test_zero_sized_type_safety() {
    // Verify ZSTs are handled correctly
    #[derive(Clone, Copy)]
    struct Zst1;

    #[derive(Clone, Copy)]
    struct Zst2;

    let dag = DagRunner::new();

    let zst1_task = dag.add_task(task_fn(|_: ()| async { Zst1 }));
    let _zst2_task = dag.add_task(task_fn(|_: ()| async { Zst2 }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Correct ZSTs work
    assert!(dag.get(&zst1_task).is_ok());
}

#[tokio::test]
async fn test_type_safety_with_many_types() {
    // Verify type safety with many different types
    let dag = DagRunner::new();

    let i32_task = dag.add_task(task_fn(|_: ()| async { 42i32 }));
    let i64_task = dag.add_task(task_fn(|_: ()| async { 42i64 }));
    let u32_task = dag.add_task(task_fn(|_: ()| async { 42u32 }));
    let f32_task = dag.add_task(task_fn(|_: ()| async { 42.0f32 }));
    let f64_task = dag.add_task(task_fn(|_: ()| async { 42.0f64 }));
    let string_task = dag.add_task(task_fn(|_: ()| async { "42".to_string() }));
    let bool_task = dag.add_task(task_fn(|_: ()| async { true }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    // Each type works correctly
    assert_eq!(dag.get(&i32_task).unwrap(), 42i32);
    assert_eq!(dag.get(&i64_task).unwrap(), 42i64);
    assert_eq!(dag.get(&u32_task).unwrap(), 42u32);
    assert_eq!(dag.get(&f32_task).unwrap(), 42.0f32);
    assert_eq!(dag.get(&f64_task).unwrap(), 42.0f64);
    assert_eq!(dag.get(&string_task).unwrap(), "42".to_string());
    assert!(dag.get(&bool_task).unwrap());
}

#[tokio::test]
async fn test_type_safety_single_dependency() {
    // Verify that single dependency type matching works
    let dag = DagRunner::new();

    let source = dag.add_task(task_fn(|_: ()| async { 42i32 }));

    // This compiles because types match (i32 -> i32)
    let transform = dag
        .add_task(task_fn(|x: i32| async move { x * 2 }))
        .depends_on(&source);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(transform).unwrap(), 84);
}

#[tokio::test]
async fn test_type_safety_tuple_dependencies() {
    // Verify that tuple dependency type matching works
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 10i32 }));
    let b = dag.add_task(task_fn(|_: ()| async { "hello".to_string() }));
    let c = dag.add_task(task_fn(|_: ()| async { true }));

    // This compiles because types match (i32, String, bool)
    let combine = dag
        .add_task(task_fn(|(x, s, b): (i32, String, bool)| async move {
            format!("{}: {} ({})", s, x, b)
        }))
        .depends_on((&a, &b, &c));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(combine).unwrap(), "hello: 10 (true)");
}

#[tokio::test]
async fn test_type_safety_different_output_types() {
    // Verify that different types can flow through the DAG
    let dag = DagRunner::new();

    let int_source = dag.add_task(task_fn(|_: ()| async { 42i32 }));
    let str_source = dag.add_task(task_fn(|_: ()| async { "test".to_string() }));
    let vec_source = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3] }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    assert_eq!(dag.get(&int_source).unwrap(), 42);
    assert_eq!(dag.get(&str_source).unwrap(), "test");
    assert_eq!(dag.get(&vec_source).unwrap(), vec![1, 2, 3]);
}

#[tokio::test]
async fn test_type_safety_complex_types() {
    // Verify that complex types work correctly as outputs
    // Complex inputs need ExtractInput implementation
    let dag = DagRunner::new();

    // Tasks can return complex types like tuples
    let tuple_task = dag.add_task(task_fn(|_: ()| async { (42, "test".to_string()) }));

    // Tasks can return vectors
    let vec_task = dag.add_task(task_fn(|_: ()| async { vec![1, 2, 3, 4, 5] }));

    // Tasks can return nested structures
    let nested_task = dag.add_task(task_fn(|_: ()| async {
        vec![Some(1), None, Some(2), Some(3)]
    }));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    let tuple_result = dag.get(tuple_task).unwrap();
    assert_eq!(tuple_result.0, 42);
    assert_eq!(tuple_result.1, "test");

    let vec_result = dag.get(vec_task).unwrap();
    assert_eq!(vec_result.len(), 5);

    let nested_result = dag.get(nested_task).unwrap();
    assert_eq!(nested_result.len(), 4);
}

#[tokio::test]
async fn test_type_safety_with_generics() {
    // Verify that generic types work correctly
    // Using Option as it already implements necessary traits
    let dag = DagRunner::new();

    let source = dag.add_task(task_fn(|_: ()| async { Some(42i32) }));

    let transform = dag
        .add_task(task_fn(
            |opt: Option<i32>| async move { opt.map(|x| x * 2) },
        ))
        .depends_on(&source);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    assert_eq!(dag.get(transform).unwrap(), Some(84));

    // Test with None variant
    let source2 = dag.add_task(task_fn(|_: ()| async { None::<i32> }));
    let transform2 = dag
        .add_task(task_fn(
            |opt: Option<i32>| async move { opt.map(|x| x * 2) },
        ))
        .depends_on(&source2);

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();
    assert_eq!(dag.get(transform2).unwrap(), None);
}

#[tokio::test]
async fn test_large_tuple_input() {
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 1 }));
    let b = dag.add_task(task_fn(|_: ()| async { 2 }));
    let c = dag.add_task(task_fn(|_: ()| async { 3 }));
    let d = dag.add_task(task_fn(|_: ()| async { 4 }));
    let e = dag.add_task(task_fn(|_: ()| async { 5 }));

    let sum = dag
        .add_task(task_fn(
            |(a, b, c, d, e): (i32, i32, i32, i32, i32)| async move { a + b + c + d + e },
        ))
        .depends_on((&a, &b, &c, &d, &e));

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await
    .unwrap();

    assert_eq!(dag.get(sum).unwrap(), 15);
}
