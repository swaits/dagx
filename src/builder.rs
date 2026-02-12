//! Task builder with type-state pattern for dependency tracking.
//!
//! TaskBuilder uses compile-time type checking to ensure dependencies are wired correctly.
//! The type parameter tracks whether dependencies have been specified yet.

use crate::deps::DepsTuple;
use crate::runner::DagRunner;
use crate::task::Task;
use crate::types::{NodeId, TaskHandle};

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
/// dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
/// assert_eq!(dag.get(b).unwrap(), 20);
/// # };
/// ```
pub struct TaskBuilder<'a, Tk: Task, Deps> {
    pub(crate) id: NodeId,
    pub(crate) dag: &'a DagRunner,
    pub(crate) _phantom: std::marker::PhantomData<(Tk, Deps)>,
}

impl<'a, Tk: Task, Deps> TaskBuilder<'a, Tk, Deps> {
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
    /// let dag = DagRunner::new();
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
    /// dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
    /// # };
    /// ```
    #[allow(private_bounds)]
    pub fn depends_on<D>(self, deps: D) -> TaskHandle<Tk::Output>
    where
        D: DepsTuple<Tk::Input>,
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

        let mut edges = self.dag.edges.lock();
        let mut dependents = self.dag.dependents.lock();

        for &dep_id in &dep_ids {
            // Add edge from this node to dependency
            if let Some(node_edges) = edges.get_mut(&self.id) {
                node_edges.push(dep_id);
            }

            // Add this node as dependent of the dependency
            if let Some(node_dependents) = dependents.get_mut(&dep_id) {
                node_dependents.push(self.id);
            }
        }

        TaskHandle {
            id: self.id,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests;
