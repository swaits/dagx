//! Task trait and implementations.
//!
//! This module defines the core Task trait that all DAG nodes must implement,
//! along with convenience functions for creating tasks from closures.

use std::{any::Any, future::Future, marker::PhantomData, slice::Iter, sync::Arc};

/// A unit of async work with typed inputs and outputs.
///
/// Use the [`crate::task`] macro to implement this trait. The macro automatically
/// derives `Input` and `Output` types from your `run()` method signature and generates
/// extraction logic in `run()`.
///
/// **Custom types work automatically!** As long as your output type implements
/// `Send + Sync + 'static`, the macro handles everything.
///
/// **Note:** When implementing this trait manually for a task with a single dependency, the Input type must be (T,) instead of T.
///
/// # Input Patterns
///
/// - `()`: No dependencies (source task)
/// - `&T`: Single dependency
/// - `&A, &B, &C, ...`: Multiple dependencies (up to 8, order matters)
///
/// # Output Wrapping
///
/// **Task outputs are automatically wrapped in `Arc<T>` internally** for efficient fan-out.
/// You define `Output = T`, but the framework wraps it in `Arc<T>` before sending to dependents.
/// This enables O(1) sharing for 1:N dependencies - Arc is cloned (cheap), not your data.
///
/// Downstream tasks receive `&T` after ExtractInput extracts from the Arc. This is transparent
/// to your task code - you work with `T`, the framework handles Arc wrapping/unwrapping.
pub trait Task<Input>: Send
where
    Input: Send + Sync + 'static,
{
    type Output: Send + Sync + 'static; // Sync required for Arc-wrapping and cross-thread sharing

    fn run(self, input: TaskInput<Input>) -> impl Future<Output = Self::Output> + Send;
}

/// Linear type providing a [`Task`] access to its dependencies.
///
/// The framework ensures that all dependencies will be present when this task runs, and
/// can be retrieved with [`TaskInput::next`]. The nth call to `next` consumes this TaskInput instances and returns a
/// (&T, TaskInput) tuple with the nth dependency specified in the call to [`crate::TaskBuilder::depends_on`] and a
/// TaskInput with the remaining dependencies.
///
/// No dependencies can be retrieved from an empty TaskInput (Input = ()).
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
            /// Get a reference to the next dependency from this TaskInput, and a [`TaskInput`] instance with the remaining dependencies.
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

#[cfg(test)]
mod tests;
