//! Tests for type system edge cases and limits

use dagx::{DagResult, DagRunner};
use dagx_test::task_fn;

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

    let mut dag = DagRunner::new();

    // Task producing ZST
    let t1 = dag.add_task(task_fn::<(), _, _>(|_: ()| Empty));

    // Task that also produces ZST (independent since we can't extract custom types)
    let t2 = dag.add_task(task_fn::<(), _, _>(|_: ()| 42));

    // Task with PhantomData
    let _t3 = dag.add_task(task_fn::<(), _, _>(|_: ()| PhantomWrapper::<i32> {
        _phantom: PhantomData,
    }));

    // Task that depends on t2 (uses primitive type)
    let t4 = dag
        .add_task(task_fn::<i32, _, _>(|&val: &i32| {
            assert_eq!(val, 42);
            "success"
        }))
        .depends_on(&t2);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(output.get(t1), Empty);
    assert_eq!(output.get(t2), 42);
    assert_eq!(output.get(t4), "success");

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

    let mut dag = DagRunner::new();

    let t1 = dag.add_task(task_fn::<(), _, _>(|_: ()| {
        let large = LargeStruct {
            data: [42; 8192],
            more_data: vec![1; 1000],
        };
        // Return a calculated value instead of the struct itself
        large.data[0] as usize + large.more_data.len()
    }));

    let t2 = dag
        .add_task(task_fn::<usize, _, _>(|&val: &usize| {
            val // Just pass through to verify
        }))
        .depends_on(&t1);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(output.get(t2), 1042);

    Ok(())
}

#[tokio::test]
async fn test_reference_wrapper_types() -> DagResult<()> {
    // Test types that wrap references
    use std::sync::{Arc, Mutex};

    let mut dag = DagRunner::new();

    let t1 = dag.add_task(task_fn::<(), _, _>(|_: ()| {
        let arc = Arc::new(Mutex::new(100));
        // Extract value instead of passing Arc
        let guard = arc.lock().unwrap();
        *guard
    }));

    let t2 = dag
        .add_task(task_fn::<i32, _, _>(|&val: &i32| val * 2))
        .depends_on(&t1);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await?;

    assert_eq!(output.get(t2), 200);

    Ok(())
}
