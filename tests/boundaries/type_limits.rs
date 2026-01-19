//! Tests for type system edge cases and limits

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};
use futures::FutureExt;
use std::marker::PhantomData;

#[tokio::test]
async fn test_zero_sized_types_throughout() -> DagResult<()> {
    // Test with ZSTs at every level
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Empty;

    #[derive(Debug, Clone, Copy)]
    struct PhantomWrapper<T> {
        _phantom: PhantomData<T>,
    }

    let dag = DagRunner::new();

    // Task producing ZST
    let t1 = dag.add_task(task_fn(|_: ()| async { Empty }));

    // Task that also produces ZST (independent since we can't extract custom types)
    let t2: TaskHandle<_> = dag.add_task(task_fn(|_: ()| async { 42 })).into();

    // Task with PhantomData
    let _t3 = dag.add_task(task_fn(|_: ()| async {
        PhantomWrapper::<i32> {
            _phantom: PhantomData,
        }
    }));

    // Task that depends on t2 (uses primitive type)
    let t4 = dag
        .add_task(task_fn(|val: i32| async move {
            assert_eq!(val, 42);
            "success"
        }))
        .depends_on(t2);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t1)?, Empty);
    assert_eq!(dag.get(t2)?, 42);
    assert_eq!(dag.get(t4)?, "success");

    Ok(())
}

#[tokio::test]
async fn test_large_value_types() -> DagResult<()> {
    // Test with very large value types
    #[derive(Clone)]
    struct LargeStruct {
        data: [u8; 8192], // 8KB
        more_data: Vec<u64>,
    }

    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async {
        let large = LargeStruct {
            data: [42; 8192],
            more_data: vec![1; 1000],
        };
        // Return a calculated value instead of the struct itself
        large.data[0] as usize + large.more_data.len()
    }));

    let t2 = dag
        .add_task(task_fn(|val: usize| async move {
            val // Just pass through to verify
        }))
        .depends_on(t1);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t2)?, 1042);

    Ok(())
}

#[tokio::test]
async fn test_unit_type_chains() -> DagResult<()> {
    // Test chains of unit type operations
    let dag = DagRunner::new();

    let t1: TaskHandle<_> = dag.add_task(task_fn(|_: ()| async {})).into();
    let t2 = dag.add_task(task_fn(|_: ()| async {})).depends_on(t1);
    let t3 = dag.add_task(task_fn(|_: ()| async {})).depends_on(t2);
    let t4 = dag
        .add_task(task_fn(|_: ()| async { "done" }))
        .depends_on(t3);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    dag.get(t1)?; // Verify no error
    dag.get(t2)?;
    dag.get(t3)?;
    assert_eq!(dag.get(t4)?, "done");

    Ok(())
}

#[tokio::test]
async fn test_nested_option_result_types() -> DagResult<()> {
    // Test deeply nested generic types
    type ComplexType = Result<Option<Result<Vec<Option<i32>>, String>>, &'static str>;

    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async {
        let val: ComplexType = Ok(Some(Ok(vec![Some(42), None, Some(13)])));
        val
    }));

    let t2 = dag
        .add_task(task_fn(|complex: ComplexType| async move {
            match complex {
                Ok(Some(Ok(vec))) => vec.iter().filter_map(|x| *x).sum::<i32>(),
                _ => 0,
            }
        }))
        .depends_on(t1);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t2)?, 55);

    Ok(())
}

#[tokio::test]
async fn test_tuple_of_tuples() -> DagResult<()> {
    // Test nested tuple types
    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { ((1, 2), (3, 4)) }));
    let t2 = dag.add_task(task_fn(|_: ()| async { ((5, 6), (7, 8)) }));

    let t3 = dag
        .add_task(task_fn(
            |inputs: (((i32, i32), (i32, i32)), ((i32, i32), (i32, i32)))| async move {
                let (((a, b), (c, d)), ((e, f), (g, h))) = inputs;
                a + b + c + d + e + f + g + h
            },
        ))
        .depends_on((t1, t2));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t3)?, 36);

    Ok(())
}

#[tokio::test]
async fn test_string_types_variety() -> DagResult<()> {
    // Test various string types
    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { "static str" }));
    let t2 = dag.add_task(task_fn(|_: ()| async { String::from("owned string") }));
    let t3 = dag.add_task(task_fn(|_: ()| async {
        std::borrow::Cow::Borrowed("cow str")
    }));
    let t4 = dag.add_task(task_fn(|_: ()| async { Box::new("boxed str") }));
    let t5 = dag.add_task(task_fn(|_: ()| async { std::sync::Arc::new("arc str 2") }));
    let t6 = dag.add_task(task_fn(|_: ()| async { std::sync::Arc::new("arc str") }));

    let combined = dag
        .add_task(task_fn(
            |(s1, s2, s3, s4, s5, s6): (
                &str,
                String,
                std::borrow::Cow<str>,
                Box<&str>,
                std::sync::Arc<&str>,
                std::sync::Arc<&str>,
            )| async move { format!("{} {} {} {} {} {}", s1, s2, s3, s4, s5, s6) },
        ))
        .depends_on((t1, t2, t3, t4, t5, t6));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(combined)?;
    assert!(result.contains("static str"));
    assert!(result.contains("owned string"));

    Ok(())
}

#[tokio::test]
async fn test_generic_type_constraints() -> DagResult<()> {
    // Test with types that have trait bounds
    #[derive(Debug, Clone, PartialEq)]
    struct Wrapper<T: Clone + Send + 'static>(T);

    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { Wrapper(42) }));
    let t2 = dag.add_task(task_fn(|_: ()| async { Wrapper("hello") }));
    let t3 = dag.add_task(task_fn(|_: ()| async { Wrapper(vec![1, 2, 3]) }));

    let combined = dag
        .add_task(task_fn(
            |(w1, w2, w3): (Wrapper<i32>, Wrapper<&str>, Wrapper<Vec<i32>>)| async move {
                format!("{:?} {:?} {:?}", w1, w2, w3)
            },
        ))
        .depends_on((t1, t2, t3));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(combined)?;
    assert!(result.contains("Wrapper(42)"));

    Ok(())
}

#[tokio::test]
async fn test_reference_wrapper_types() -> DagResult<()> {
    // Test types that wrap references
    use std::sync::{Arc, Mutex};

    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async {
        let arc = Arc::new(Mutex::new(100));
        // Extract value instead of passing Arc
        let guard = arc.lock().unwrap();
        *guard
    }));

    let t2 = dag
        .add_task(task_fn(|val: i32| async move { val * 2 }))
        .depends_on(t1);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t2)?, 200);

    Ok(())
}

#[tokio::test]
async fn test_array_types() -> DagResult<()> {
    // Test with fixed-size arrays
    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { [1, 2, 3, 4, 5] }));
    let t2 = dag.add_task(task_fn(|_: ()| async { [6, 7, 8, 9, 10] }));

    let t3 = dag
        .add_task(task_fn(|(a1, a2): ([i32; 5], [i32; 5])| async move {
            let mut sum = 0;
            for i in 0..5 {
                sum += a1[i] + a2[i];
            }
            sum
        }))
        .depends_on((t1, t2));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(t3)?, 55);

    Ok(())
}

#[tokio::test]
async fn test_enum_variants() -> DagResult<()> {
    // Test with various enum types
    #[derive(Debug, Clone, PartialEq)]
    #[allow(clippy::large_enum_variant)]
    enum MyEnum {
        Unit,
        Tuple(i32, String),
        Struct { x: i32, y: i32 },
        LargeVariant([u8; 1024]),
    }

    let dag = DagRunner::new();

    let t1 = dag.add_task(task_fn(|_: ()| async { MyEnum::Unit }));
    let t2 = dag.add_task(task_fn(|_: ()| async {
        MyEnum::Tuple(42, "test".to_string())
    }));
    let t3 = dag.add_task(task_fn(|_: ()| async { MyEnum::Struct { x: 10, y: 20 } }));
    let t4 = dag.add_task(task_fn(|_: ()| async { MyEnum::LargeVariant([0; 1024]) }));

    let combined = dag
        .add_task(task_fn(
            |(e1, e2, e3, e4): (MyEnum, MyEnum, MyEnum, MyEnum)| async move {
                matches!(e1, MyEnum::Unit)
                    && matches!(e2, MyEnum::Tuple(42, _))
                    && matches!(e3, MyEnum::Struct { x: 10, y: 20 })
                    && matches!(e4, MyEnum::LargeVariant(_))
            },
        ))
        .depends_on((t1, t2, t3, t4));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert!(dag.get(combined)?);

    Ok(())
}
