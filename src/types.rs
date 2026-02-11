//! Core type definitions for DAG nodes and handles.
//!
//! This module defines the fundamental types used throughout the DAG system,
//! including node identifiers, task handles, and type-state markers.

use crate::builder::TaskBuilder;
use crate::task::Task;

/// Opaque node identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

/// Opaque, typed token for a node's output.
///
/// A `TaskHandle<T>` provides compile-time type-safe access to a task's output.
/// You can:
/// 1. Pass it to [`crate::TaskBuilder::depends_on`] to wire up dependencies
/// 2. Use it with [`crate::DagRunner::get`] to retrieve the output after [`crate::DagRunner::run`]
///
/// Handles are cheap to clone and copy.
///
/// # Examples
///
/// ```no_run
/// # use dagx::{task, DagRunner, Task};
/// #
/// # struct LoadValue { value: i32 }
/// # impl LoadValue { pub fn new(v: i32) -> Self { Self { value: v } } }
/// # #[task]
/// # impl LoadValue {
/// #     async fn run(&mut self) -> i32 { self.value }
/// # }
/// # async {
/// let dag = DagRunner::new();
/// let node = dag.add_task(LoadValue::new(42));
///
/// dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
///
/// assert_eq!(dag.get(node).unwrap(), 42);
/// # };
/// ```
pub struct TaskHandle<T> {
    pub(crate) id: NodeId,
    pub(crate) _phantom: std::marker::PhantomData<fn() -> T>,
}

impl<T> Clone for TaskHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TaskHandle<T> {}

/// Type-level marker for empty dependency list.
///
/// This type appears in the type signature of [`crate::TaskBuilder<Tk, Pending>`](crate::TaskBuilder) before
/// dependencies are specified with [`crate::TaskBuilder::depends_on`].
///
/// You don't interact with this type directly; it's part of the type-state
/// pattern that ensures dependencies are wired correctly at compile time.
pub struct Pending;

// TaskBuilder can be converted to a TaskHandle ONLY if Input is () (unit type)
// This enforces at compile time that tasks with non-unit inputs MUST call .depends_on()
impl<'a, Tk: Task<Input = ()>, Deps> From<TaskBuilder<'a, Tk, Deps>> for TaskHandle<Tk::Output> {
    fn from(node: TaskBuilder<'a, Tk, Deps>) -> Self {
        TaskHandle {
            id: node.id,
            _phantom: std::marker::PhantomData,
        }
    }
}

// TaskHandle can be converted from &TaskHandle (for .get() calls)
impl<T> From<&TaskHandle<T>> for TaskHandle<T> {
    fn from(handle: &TaskHandle<T>) -> Self {
        *handle
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests;
