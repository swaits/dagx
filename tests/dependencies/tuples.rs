//! Tests for tuple dependencies (2-8 dependencies)

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};
use futures::FutureExt;

#[tokio::test]
async fn test_two_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 10 }));
    let b = dag.add_task(task_fn(|_: ()| async { 20 }));
    let sum = dag
        .add_task(task_fn(|(x, y): (i32, i32)| async move { x + y }))
        .depends_on((&a, &b));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 30);
    Ok(())
}

#[tokio::test]
async fn test_three_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let a = dag.add_task(task_fn(|_: ()| async { 1 }));
    let b = dag.add_task(task_fn(|_: ()| async { 2 }));
    let c = dag.add_task(task_fn(|_: ()| async { 3 }));
    let sum = dag
        .add_task(task_fn(
            |(x, y, z): (i32, i32, i32)| async move { x + y + z },
        ))
        .depends_on((&a, &b, &c));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 6);
    Ok(())
}

#[tokio::test]
async fn test_four_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let deps: Vec<_> = (1..=4)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let sum = dag
        .add_task(task_fn(|(a, b, c, d): (i32, i32, i32, i32)| async move {
            a + b + c + d
        }))
        .depends_on((&deps[0], &deps[1], &deps[2], &deps[3]));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 10); // 1+2+3+4
    Ok(())
}

#[tokio::test]
async fn test_five_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let deps: Vec<_> = (1..=5)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let sum = dag
        .add_task(task_fn(
            |(a, b, c, d, e): (i32, i32, i32, i32, i32)| async move { a + b + c + d + e },
        ))
        .depends_on((&deps[0], &deps[1], &deps[2], &deps[3], &deps[4]));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 15); // 1+2+3+4+5
    Ok(())
}

#[tokio::test]
async fn test_six_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let deps: Vec<_> = (1..=6)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let sum = dag.add_task(task_fn(
        |(a, b, c, d, e, f): (i32, i32, i32, i32, i32, i32)| async move {
            a + b + c + d + e + f
        }
    )).depends_on((
        &deps[0], &deps[1], &deps[2], &deps[3], &deps[4], &deps[5]
    ));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 21); // 1+2+3+4+5+6
    Ok(())
}

#[tokio::test]
async fn test_seven_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let deps: Vec<_> = (1..=7)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let sum = dag
        .add_task(task_fn(
            |(a, b, c, d, e, f, g): (i32, i32, i32, i32, i32, i32, i32)| async move {
                a + b + c + d + e + f + g
            },
        ))
        .depends_on((
            &deps[0], &deps[1], &deps[2], &deps[3], &deps[4], &deps[5], &deps[6],
        ));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 28); // 1+2+3+4+5+6+7
    Ok(())
}

#[tokio::test]
async fn test_eight_dependencies() -> DagResult<()> {
    let dag = DagRunner::new();

    let deps: Vec<_> = (1..=8)
        .map(|i| dag.add_task(task_fn(move |_: ()| async move { i })))
        .collect();

    let sum = dag
        .add_task(task_fn(
            |(a, b, c, d, e, f, g, h): (i32, i32, i32, i32, i32, i32, i32, i32)| async move {
                a + b + c + d + e + f + g + h
            },
        ))
        .depends_on((
            &deps[0], &deps[1], &deps[2], &deps[3], &deps[4], &deps[5], &deps[6], &deps[7],
        ));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(sum)?, 36); // Sum of 1..=8
    Ok(())
}
