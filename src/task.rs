//! Task trait and implementations.
//!
//! This module defines the core Task trait that all DAG nodes must implement,
//! along with convenience functions for creating tasks from closures.

use std::{any::Any, future::Future, marker::PhantomData, slice::Iter, sync::Arc};

/// A unit of async work with typed inputs and outputs.
///
/// Use the [`task`](crate::task) macro to implement this trait. The macro automatically
/// derives `Input` and `Output` types from your `run()` method signature and generates
/// type-specific extraction logic in `run()`.
///
/// **Custom types work automatically!** As long as your output type implements
/// `Send + Sync + 'static`, the macro handles everything. No trait implementations needed!
///
/// # Task Patterns
///
/// dagx supports three main task patterns:
///
/// ## 1. Stateless Tasks (Pure Functions)
///
/// Tasks with no internal state - just transform inputs to outputs:
///
/// ```
/// use dagx::{task, Task};
///
/// struct Add;
///
/// #[task]
/// impl Add {
///     async fn run(a: &i32, b: &i32) -> i32 {
///         a + b
///     }
/// }
/// ```
///
/// ## 2. Stateful Tasks with Shared State (`&self`)
///
/// Tasks that read from internal state but don't modify it:
///
/// ```
/// use dagx::{task, Task};
///
/// struct Multiplier {
///     factor: i32,
/// }
///
/// #[task]
/// impl Multiplier {
///     async fn run(&self, input: &i32) -> i32 {
///         input * self.factor
///     }
/// }
/// ```
///
/// ## 3. Stateful Tasks with Mutable State (`&mut self`)
///
/// Tasks that modify internal state during execution:
///
/// ```
/// use dagx::{task, Task};
///
/// struct Counter {
///     count: i32,
/// }
///
/// #[task]
/// impl Counter {
///     async fn run(&mut self, increment: &i32) -> i32 {
///         self.count += increment;
///         self.count
///     }
/// }
/// ```
///
/// # Input Patterns
///
/// - `()`: No dependencies (source task)
/// - `&T`: Single dependency
/// - `&A, &B, &C, ...`: Multiple dependencies (up to 8, order matters)
///
/// # The Underlying Trait
///
/// The `#[task]` macro generates an implementation of this trait:
///
/// # Output Wrapping
///
/// **Task outputs are automatically wrapped in `Arc<T>` internally** for efficient fan-out.
/// You define `Output = T`, but the framework wraps it in `Arc<T>` before sending to dependents.
/// This enables O(1) sharing for 1:N dependencies - Arc is cloned (cheap), not your data.
///
/// Downstream tasks receive `&T` after ExtractInput extracts from the Arc. This is transparent
/// to your task code - you work with `T`, the framework handles Arc wrapping/unwrapping.
pub trait Task: Send {
    type Input: Send;
    type Output: Send + Sync; // Sync required for Arc-wrapping and cross-thread sharing

    fn run(self, input: TaskInput<Self::Input>) -> impl Future<Output = Self::Output> + Send;
}

pub struct TaskInput<'inputs, Input: Send> {
    inputs: Iter<'inputs, Arc<dyn Any + Send + Sync + 'static>>,
    phantom: PhantomData<Input>,
}

impl<'inputs, Input: Send> TaskInput<'inputs, Input> {
    pub(crate) fn new(inputs: Iter<'inputs, Arc<dyn Any + Send + Sync + 'static>>) -> Self {
        Self {
            inputs,
            phantom: PhantomData,
        }
    }
}

macro_rules! impl_task_input {
    ($First:ident $(, $T:ident)*) => {
        impl<'inputs, $First: 'static + Send, $($T: 'static + Send),*> TaskInput<'inputs, ($First, $($T,)*)> {
            #[must_use]
            pub fn next(mut self) -> (&'inputs $First, TaskInput<'inputs, ($($T,)*)>) {
                let value = self.inputs.next().and_then(|value| value.downcast_ref()).unwrap();
                let next_inputs = TaskInput {
                    inputs: self.inputs,
                    phantom: PhantomData,
                };
                (value, next_inputs)
            }
        }
    };
}

impl_task_input!(A);
impl_task_input!(A, B);
impl_task_input!(A, B, C);
impl_task_input!(A, B, C, D);
impl_task_input!(A, B, C, D, E);
impl_task_input!(A, B, C, D, E, F);
impl_task_input!(A, B, C, D, E, F, G);
impl_task_input!(A, B, C, D, E, F, G, H);

impl TaskInput<'static, ()> {
    pub fn empty() -> Self {
        Self {
            inputs: [].iter(),
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests;
