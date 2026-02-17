//! Task builder with type-state pattern for dependency tracking.
//!
//! TaskBuilder uses compile-time type checking to ensure dependencies are wired correctly.
//! The type parameter tracks whether dependencies have been specified yet.

use std::marker::PhantomData;

use crate::deps::DepsTuple;
use crate::runner::DagRunner;
use crate::task::Task;

#[cfg(feature = "tracing")]
use tracing::debug;

/// Node builder that tracks dependency completion via type state.
///
/// A `TaskBuilder<Tk, Deps>` is returned from [`DagRunner::add_task`] and uses the type-state
/// pattern to ensure dependencies are wired correctly at compile time:
/// - `TaskBuilder<Tk, Pending>`: No dependencies specified yet
/// - `TaskBuilder<Tk, Tk::Input>`: Dependencies complete; acts as a [`TaskHandle<Tk::Output>`]
///
/// # Examples
///
/// ```no_run
/// # use dagx::{task, DagRunner, Task};
/// #
/// // Using tuple struct for simple constants
/// struct Constant(i32);
///
/// #[task]
/// impl Constant {
///     async fn run(&mut self) -> i32 { self.0 }
/// }
///
/// // Task with state constructed via ::new()
/// struct Multiplier { factor: i32 }
///
/// impl Multiplier {
///     fn new(factor: i32) -> Self { Self { factor } }
/// }
///
/// #[task]
/// impl Multiplier {
///     async fn run(&mut self, x: &i32) -> i32 { x * self.factor }
/// }
///
/// # async {
/// let mut dag = DagRunner::new();
///
/// // Construct with initial value (tuple struct)
/// let a = dag.add_task(Constant(10));
/// // a is TaskBuilder<_, Pending> since no dependencies needed (Input = ())
///
/// // Construct with ::new() pattern
/// let b = dag.add_task(Multiplier::new(2));
/// // b is TaskBuilder<_, Pending> until we call depends_on()
///
/// let b = b.depends_on(a);
/// // Now b is a TaskHandle<i32>
///
///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
/// assert_eq!(output.get(b).unwrap(), 20);
/// # };
/// ```
#[must_use]
pub struct TaskBuilder<'a, Input, Tk>
where
    Tk: Task<Input>,
    Input: Send + Sync + 'static,
{
    pub(crate) id: NodeId,
    pub(crate) dag: &'a mut DagRunner,
    pub(crate) _phantom: PhantomData<(Tk, Input)>,
}

impl<'a, Input, Tk> TaskBuilder<'a, Input, Tk>
where
    Tk: Task<Input>,
    Input: Send + Sync + 'static,
{
    /// Provide all dependencies exactly once as a tuple.
    ///
    /// The dependencies must match the task's `Input` type exactly:
    /// - `Input = ()`: Do **not** call `depends_on`
    /// - `Input = T`: Pass `&TaskHandle` or `(&TaskHandle,)`
    /// - `Input = (A, B, ...)`: Pass `(&TaskHandle<A>, &TaskHandle<B>, ...)`
    ///
    /// The order of dependencies in the tuple must match the order in `Input`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dagx::{task, DagRunner, Task};
    /// #
    /// // Tuple struct
    /// struct Value(i32);
    ///
    /// #[task]
    /// impl Value {
    ///     async fn run(&mut self) -> i32 { self.0 }
    /// }
    ///
    /// // Tuple struct with multiplier
    /// struct Scale(i32);
    ///
    /// #[task]
    /// impl Scale {
    ///     async fn run(&mut self, x: &i32) -> i32 { x * self.0 }
    /// }
    ///
    /// // Unit struct
    /// struct Add;
    ///
    /// #[task]
    /// impl Add {
    ///     async fn run(&mut self, a: &i32, b: &i32) -> i32 { a + b }
    /// }
    ///
    /// # async {
    /// let mut dag = DagRunner::new();
    ///
    /// let x = dag.add_task(Value(2)).into();
    /// let y = dag.add_task(Value(3));
    ///
    /// // Single dependency
    /// let double = dag.add_task(Scale(2)).depends_on(&x);
    ///
    /// // Multiple dependencies: tuple form
    /// let sum = dag.add_task(Add).depends_on((&x, y));
    ///
    ///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
    /// # };
    /// ```
    #[allow(private_bounds)]
    pub fn depends_on<D>(self, deps: D) -> TaskHandle<Tk::Output>
    where
        D: DepsTuple<Input>,
    {
        // Register dependencies in the DAG
        let dep_ids = deps.to_node_ids();

        #[cfg(feature = "tracing")]
        debug!(
            task_id = self.id.0,
            dependency_ids = ?dep_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
            dependency_count = dep_ids.len(),
            "wiring task dependencies"
        );

        for &dep_id in &dep_ids {
            // Add edge from this node to dependency
            if let Some(node_edges) = self.dag.edges.get_mut(&self.id) {
                node_edges.push(dep_id);
            }

            // Add this node as dependent of the dependency
            if let Some(node_dependents) = self.dag.dependents.get_mut(&dep_id) {
                node_dependents.push(self.id);
            }
        }

        TaskHandle {
            id: self.id,
            _phantom: PhantomData,
        }
    }
}

/// Opaque node identifier
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct NodeId(pub u32);

/// Opaque, typed token for a node's output.
///
/// A `TaskHandle<T>` provides compile-time type-safe access to a task's output.
/// You can:
/// 1. Pass it to [`crate::TaskBuilder::depends_on`] to wire up dependencies
/// 2. Use it with [`crate::DagOutput::get`] to retrieve the output after [`crate::DagRunner::run`]
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
/// let mut dag = DagRunner::new();
/// let node = dag.add_task(LoadValue::new(42));
///
///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
///
/// assert_eq!(output.get(node).unwrap(), 42);
/// # };
/// ```
pub struct TaskHandle<T> {
    pub(crate) id: NodeId,
    pub(crate) _phantom: PhantomData<fn() -> T>,
}

impl<T> Clone for TaskHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TaskHandle<T> {}

/// Takes a task and converts it to either a TaskBuilder or a TaskHandle,
/// depending on whether it has inputs or not.
///
/// This is useful to enforce at compile time that a task with unit input never has
/// depends_on called, and that it can be used directly as a dependency
/// without converting it manually to a TaskHandle.
pub trait TaskWire<Input>: Task<Input> + Sync + 'static
where
    Input: Send + Sync + 'static,
{
    type Retval<'dag>;

    fn new_from_dag<'dag>(id: NodeId, dag: &'dag mut DagRunner) -> Self::Retval<'dag>;
}

impl<Tk> TaskWire<()> for Tk
where
    Tk: Task<()> + Sync + 'static,
{
    type Retval<'dag> = TaskHandle<Tk::Output>;

    fn new_from_dag(id: NodeId, _dag: &mut DagRunner) -> Self::Retval<'static> {
        Self::Retval {
            id,
            _phantom: PhantomData,
        }
    }
}

/// Macro to implement TaskWire for different tuple sizes.
macro_rules! impl_wire_tuple {
    ($($T:ident),+) => {
        // For tuples of any combination of &TaskHandle, TaskHandle, and TaskBuilder
        impl<Tk, $($T: Send + Sync + 'static),+> TaskWire<($($T,)+)> for Tk
        where
            Tk: Task<($($T,)+)> + Sync + 'static
        {
            type Retval<'dag> = TaskBuilder<'dag, ($($T,)+), Tk>;

            fn new_from_dag<'dag>(id: NodeId, dag: &'dag mut DagRunner) -> Self::Retval<'dag> {
                Self::Retval {
                    id,
                    dag,
                    _phantom: PhantomData,
                }
            }
        }
    };
}

impl_wire_tuple!(T1);
impl_wire_tuple!(T1, T2);
impl_wire_tuple!(T1, T2, T3);
impl_wire_tuple!(T1, T2, T3, T4);
impl_wire_tuple!(T1, T2, T3, T4, T5);
impl_wire_tuple!(T1, T2, T3, T4, T5, T6);
impl_wire_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_wire_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests;
