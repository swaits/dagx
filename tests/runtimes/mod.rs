//! Runtime tests module

use std::{any::Any, sync::Arc};

use dagx::{DagResult, DagRunner};
use dagx_test::task_fn;
use futures_util::{future::BoxFuture, task::SpawnExt, Future};
use test_case::test_case;

type BoxFunc = Box<
    dyn Fn(
        BoxFuture<'static, DagResult<Arc<dyn Any + Send + Sync>>>,
    ) -> BoxFuture<'static, DagResult<Arc<dyn Any + Send + Sync>>>,
>;

trait RuntimeTest {
    fn run_dagx_test<F>(self, f: impl Fn(BoxFunc) -> F)
    where
        F: Future<Output = ()>;
}

struct SmolRuntimeTest;

impl RuntimeTest for SmolRuntimeTest {
    fn run_dagx_test<F>(self, f: impl Fn(BoxFunc) -> F)
    where
        F: Future<Output = ()>,
    {
        smol::block_on(f(Box::new(|fut| Box::pin(smol::spawn(fut)))));
    }
}

struct FuturesExecutorRuntimeTest;

impl RuntimeTest for FuturesExecutorRuntimeTest {
    fn run_dagx_test<F>(self, f: impl Fn(BoxFunc) -> F)
    where
        F: Future<Output = ()>,
    {
        let spawner = futures_executor::ThreadPool::new().unwrap();

        futures_executor::block_on(f(Box::new(move |fut| {
            Box::pin(spawner.spawn_with_handle(fut).unwrap())
        })));
    }
}

struct PollsterRuntimeTest;

impl RuntimeTest for PollsterRuntimeTest {
    fn run_dagx_test<F>(self, f: impl Fn(BoxFunc) -> F)
    where
        F: Future<Output = ()>,
    {
        pollster::block_on(f(Box::new(|fut| {
            Box::pin(async { pollster::block_on(fut) })
        })));
    }
}

static ASYNC_EXECUTOR: async_executor::Executor = async_executor::Executor::new();

struct AsyncExecutorRuntimeTest;

impl RuntimeTest for AsyncExecutorRuntimeTest {
    fn run_dagx_test<F>(self, f: impl Fn(BoxFunc) -> F)
    where
        F: Future<Output = ()>,
    {
        smol::future::block_on(
            ASYNC_EXECUTOR.run(f(Box::new(|fut| Box::pin(ASYNC_EXECUTOR.spawn(fut))))),
        );
    }
}

struct Value(i32);
#[dagx::task]
impl Value {
    async fn run(&mut self) -> i32 {
        self.0
    }
}

struct Add;
#[dagx::task]
impl Add {
    async fn run(&mut self, a: &i32, b: &i32) -> i32 {
        a + b
    }
}

#[test_case(SmolRuntimeTest)]
#[test_case(FuturesExecutorRuntimeTest)]
#[test_case(PollsterRuntimeTest)]
#[test_case(AsyncExecutorRuntimeTest)]
fn test_basic_dag(runner: impl RuntimeTest) {
    runner.run_dagx_test(|spawner| async {
        let mut dag = DagRunner::new();

        let x = dag.add_task(Value(2));
        let y = dag.add_task(Value(3));
        let sum = dag.add_task(Add).depends_on((&x, &y));

        let mut output = dag.run(spawner).await.unwrap();

        assert_eq!(output.get(sum), 5);
    })
}

#[test_case(SmolRuntimeTest)]
#[test_case(FuturesExecutorRuntimeTest)]
#[test_case(PollsterRuntimeTest)]
#[test_case(AsyncExecutorRuntimeTest)]
fn test_parallel_execution(runner: impl RuntimeTest) {
    runner.run_dagx_test(|spawner| async {
        let mut dag = DagRunner::new();

        let tasks: Vec<_> = (0..10)
            .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| i * 2)))
            .collect();

        let mut output = dag.run(spawner).await.unwrap();

        for (i, task) in tasks.into_iter().enumerate() {
            assert_eq!(output.get(task), i * 2);
        }
    });
}

#[test_case(SmolRuntimeTest)]
#[test_case(FuturesExecutorRuntimeTest)]
#[test_case(PollsterRuntimeTest)]
#[test_case(AsyncExecutorRuntimeTest)]
fn test_complex_dependencies(runner: impl RuntimeTest) {
    runner.run_dagx_test(|spawner| async {
        let mut dag = DagRunner::new();

        let a = dag.add_task(Value(10));
        let b = dag.add_task(Value(20));
        let sum = dag.add_task(Add).depends_on((&a, &b));
        let double = dag
            .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
            .depends_on(&sum);

        let mut output = dag.run(spawner).await.unwrap();

        assert_eq!(output.get(double).clone(), 60);
    });
}
