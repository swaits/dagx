//! Task trait and implementations.
//!
//! This module defines the core Task trait that all DAG nodes must implement,
//! along with convenience functions for creating tasks from closures.

use std::future::Future;

/// A unit of async work with typed inputs and outputs.
///
/// Use the [`task`](crate::task) macro to implement this trait. The macro automatically
/// derives `Input` and `Output` types from your `run()` method signature.
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
/// You define `Output = T`, but the framework wraps it in `Arc<T>` before sending to channels.
/// This enables O(1) sharing for 1:N dependencies - Arc is cloned (cheap), not your data.
///
/// Downstream tasks receive `&T` after ExtractInput extracts from the Arc. This is transparent
/// to your task code - you work with `T`, the framework handles Arc wrapping/unwrapping.
pub trait Task: Send {
    type Input: Send;
    type Output: Send + Sync; // Sync required for Arc-wrapping and cross-thread sharing

    fn run(self, input: Self::Input) -> impl Future<Output = Self::Output> + Send;
}

/// Convenience function to create a task from a closure.
///
/// **INTERNAL USE ONLY**: This function is for testing purposes only and should
/// not be used in production code. Use the `#[task]` macro instead.
#[doc(hidden)]
pub fn task_fn<I, O, Fut, F>(f: F) -> impl Task<Input = I, Output = O>
where
    F: FnMut(I) -> Fut + Send + 'static,
    Fut: Future<Output = O> + Send + 'static,
    I: Send + 'static,
    O: Send + Sync + 'static,
{
    struct TaskFn<I, O, Fut, F>
    where
        F: FnMut(I) -> Fut + Send,
        Fut: Future<Output = O> + Send,
        I: Send,
        O: Send,
    {
        f: F,
        _phantom: std::marker::PhantomData<fn(I) -> O>,
    }

    impl<I, O, Fut, F> Task for TaskFn<I, O, Fut, F>
    where
        F: FnMut(I) -> Fut + Send,
        Fut: Future<Output = O> + Send,
        I: Send,
        O: Send + Sync,
    {
        type Input = I;
        type Output = O;

        async fn run(mut self, input: I) -> O {
            (self.f)(input).await
        }
    }

    TaskFn {
        f,
        _phantom: std::marker::PhantomData,
    }
}

#[cfg(test)]
mod tests;
