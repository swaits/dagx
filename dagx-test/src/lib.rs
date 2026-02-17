//! DAG builder helpers for internal use in dagx tests and benchmarks.
//!
//! This crate is not meant for public use and offers no stability guarantees.
//! The ergonomics are also such that you don't want to use it anyways. **Use the `#[task]` macro instead.**

#![cfg(not(tarpaulin_include))]

use std::marker::PhantomData;

use crate::extract::ExtractInput;

use dagx::{Task, TaskInput};

mod extract;

pub struct TaskFn<I, O, F>
where
    for<'input> F: FnMut(I::Retv<'input>) -> O + Send,
    I: ExtractInput + Send + Sync + 'static,
    O: Send,
{
    f: F,
    _phantom: PhantomData<fn(I) -> O>,
}

impl<I, O, F> Task<I::Input> for TaskFn<I, O, F>
where
    for<'input> F: FnMut(I::Retv<'input>) -> O + Send,
    I: ExtractInput + Send + Sync + 'static,
    I::Input: Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    type Output = O;

    async fn run(mut self, input: TaskInput<'_, I::Input>) -> O {
        (self.f)(I::extract_from_task_input(input).unwrap())
    }
}

/// Convenience function to create a task from a closure.
pub fn task_fn<I, O, F>(f: F) -> TaskFn<I, O, F>
where
    for<'input> F: FnMut(I::Retv<'input>) -> O + Send,
    I: ExtractInput + Send + Sync + 'static,
    I::Input: Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    TaskFn {
        f,
        _phantom: PhantomData,
    }
}
