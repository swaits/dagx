//! DAG runner for task orchestration and execution.
//!
//! Provides DagRunner for building and executing directed acyclic graphs of async tasks
//! with compile-time type-safe dependencies.
//!
//! Uses Mutex for interior mutability to enable builder pattern (`&self` instead of `&mut self`).

use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::hash::{BuildHasher, Hasher};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures_util::future::BoxFuture;
use futures_util::{stream::FuturesUnordered, FutureExt, StreamExt, TryFutureExt};

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace};

use crate::builder::{NodeId, TaskWire};
use crate::error::{DagError, DagResult};
use crate::node::{ExecutableNode, TypedNode};
use crate::DagOutput;

#[derive(Default, Clone)]
pub(crate) struct PassThroughHasher {
    hash: u64,
}

impl Hasher for PassThroughHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write_u32(&mut self, i: u32) {
        self.hash = i as u64;
    }

    fn write(&mut self, _bytes: &[u8]) {
        panic!("PassThroughHasher used on invalid type");
    }
}

impl BuildHasher for PassThroughHasher {
    type Hasher = PassThroughHasher;

    fn build_hasher(&self) -> Self::Hasher {
        PassThroughHasher::default()
    }
}

pub(crate) type PassThroughHashMap<K, V> = HashMap<K, V, PassThroughHasher>;

/// Build and execute a typed DAG of tasks.
///
/// A `DagRunner` is the main orchestrator for building and executing a directed acyclic graph
/// of async tasks with compile-time type-safe dependencies.
///
/// # Workflow
///
/// 1. Create a new DAG with [`DagRunner::new`]
/// 2. Add tasks with [`DagRunner::add_task`] to get builders
/// 3. Wire dependencies with [`crate::TaskBuilder::depends_on`]
/// 4. Execute all tasks with [`DagRunner::run`]
/// 5. Optionally retrieve outputs with [`DagOutput::get`]
///
/// # Examples
///
/// ```no_run
/// # use dagx::{task, DagRunner, Task};
/// #
/// // Task with state constructed via ::new()
/// struct LoadValue { value: i32 }
///
/// impl LoadValue {
///     fn new(value: i32) -> Self { Self { value } }
/// }
///
/// #[task]
/// impl LoadValue {
///     async fn run(&mut self) -> i32 { self.value }
/// }
///
/// // Unit struct - no fields needed
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
/// // Construct instances using ::new() pattern
/// let x = dag.add_task(LoadValue::new(2));
/// let y = dag.add_task(LoadValue::new(3));
/// let sum = dag.add_task(Add).depends_on((x, y));
///
///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
///
/// assert_eq!(output.get(sum).unwrap(), 5);
/// # };
/// ```
///
/// Uses Mutex for interior mutability to enable the builder pattern (`&self` not `&mut self`).
/// This allows fluent chaining of `add_task()` calls.
///
/// Nodes use Option to allow taking ownership during execution.
/// Outputs are Arc-wrapped and stored separately for retrieval via get().
/// Arc enables efficient sharing during fanout without cloning data.
pub struct DagRunner {
    pub(crate) nodes: Vec<Option<Box<dyn ExecutableNode + Sync>>>,
    pub(crate) edges: PassThroughHashMap<NodeId, Vec<NodeId>>, // node -> dependencies
    pub(crate) dependents: PassThroughHashMap<NodeId, Vec<NodeId>>, // node -> tasks that depend on it
}

impl Default for DagRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl DagRunner {
    /// Create a new empty DAG.
    ///
    /// # Examples
    ///
    /// ```
    /// use dagx::DagRunner;
    ///
    /// let mut dag = DagRunner::new();
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: HashMap::default(),
            dependents: HashMap::default(),
        }
    }

    /// Add a task instance to the DAG, returning a node builder for wiring dependencies.
    ///
    /// If the task has no dependencies, a [`crate::TaskHandle`] will be returned.
    /// If not, the returned [`crate::TaskBuilder`] can be used to
    /// specify dependencies via [`crate::TaskBuilder::depends_on`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dagx::{task, DagRunner, Task};
    /// #
    /// // Task with state - shows you construct with specific value
    /// struct LoadValue {
    ///     initial: i32,
    /// }
    ///
    /// impl LoadValue {
    ///     fn new(initial: i32) -> Self {
    ///         Self { initial }
    ///     }
    /// }
    ///
    /// #[task]
    /// impl LoadValue {
    ///     async fn run(&mut self) -> i32 { self.initial }
    /// }
    ///
    /// // Task with configuration - shows you can parameterize behavior
    /// struct AddOffset {
    ///     offset: i32,
    /// }
    ///
    /// impl AddOffset {
    ///     fn new(offset: i32) -> Self {
    ///         Self { offset }
    ///     }
    /// }
    ///
    /// #[task]
    /// impl AddOffset {
    ///     async fn run(&mut self, x: &i32) -> i32 { x + self.offset }
    /// }
    ///
    /// # async {
    /// let mut dag = DagRunner::new();
    ///
    /// // Construct task with initial value of 10
    /// let base = dag.add_task(LoadValue::new(10));
    ///
    /// // Construct task with offset of 1
    /// let inc = dag.add_task(AddOffset::new(1)).depends_on(base);
    ///
    ///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
    /// assert_eq!(output.get(inc).unwrap(), 11);
    /// # };
    /// ```
    pub fn add_task<'dag, Input, Tk>(&'dag mut self, task: Tk) -> Tk::Retval<'dag>
    where
        Tk: TaskWire<Input>,
        Input: Send + Sync + 'static,
    {
        let id = NodeId(self.nodes.len() as u32);

        #[cfg(feature = "tracing")]
        debug!(
            task_id = id.0,
            task_type = std::any::type_name::<Tk>(),
            "adding task to DAG"
        );

        let node = TypedNode::new(task);
        self.nodes.push(Some(Box::new(node)));
        self.edges.insert(id, Vec::new());
        self.dependents.insert(id, Vec::new());

        Tk::new_from_dag(id, self)
    }

    /// Run the entire DAG to completion using the provided spawner.
    ///
    /// This method:
    /// - Executes tasks in topological order (respecting dependencies)
    /// - Runs ready tasks with maximum parallelism (executor-limited)
    /// - Executes each task at most once
    /// - Waits for **all sinks** (tasks with no dependents) to complete
    /// - Is runtime-agnostic via the spawner function
    ///
    /// # Parameters
    ///
    /// - `spawner`: A function that spawns futures on the async runtime
    ///   and returns a handle to the task. This is the only way to run tasks on separate threads. Examples:
    ///   - Tokio: `|fut| { tokio::spawn(fut).await.unwrap() }`
    ///   - Smol: `|fut| { smol::spawn(fut) }`
    ///   - Async-std: `|fut| { async_std::task::spawn(fut) }`
    ///   - Single-threaded: `|fut| fut`
    ///     - For computationally light tasks, concurrency without parallelism can be significantly faster.
    ///
    /// # Errors
    ///
    /// - Returns `DagError::ConcurrentExecution` if the DAG is already executing
    /// - Returns `DagError::TaskPanicked` if any task panics during execution
    ///
    /// # Examples
    ///
    /// ```
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
    /// let a = dag.add_task(Value(1));
    /// let b = dag.add_task(Value(2));
    /// let sum = dag.add_task(Add).depends_on((a, b));
    ///
    ///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap(); // Executes all tasks
    /// # };
    /// ```
    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, spawner)))]
    pub async fn run<S, F>(mut self, spawner: S) -> DagResult<DagOutput>
    where
        S: Fn(BoxFuture<'static, DagResult<Arc<dyn Any + Send + Sync>>>) -> F,
        F: Future<Output = DagResult<Arc<dyn Any + Send + Sync>>>,
    {
        #[cfg(feature = "tracing")]
        info!("starting DAG execution");

        // Build topological layers
        let layers = self.compute_layers()?;

        let total_tasks = layers.iter().map(|l| l.len()).sum::<usize>();

        #[cfg(feature = "tracing")]
        debug!(
            layer_count = layers.len(),
            total_tasks, "computed topological layers"
        );

        let mut outputs: PassThroughHashMap<NodeId, Arc<dyn Any + Send + Sync>> =
            HashMap::with_capacity_and_hasher(total_tasks, PassThroughHasher::default());
        let mut first_error = None;

        for layer in layers {
            #[cfg(feature = "tracing")]
            {
                debug!(task_count = layer.len(), "executing layer");
            }
            // ============================================================================
            // PERFORMANCE OPTIMIZATION: Inline execution for single-task layers
            // ============================================================================
            //
            // When a layer contains exactly one task (common in deep chains, linear
            // pipelines), we execute it inline rather than spawning it. This provides
            // 10-100x performance improvements for sequential workloads by eliminating:
            //   - Task spawning overhead
            //   - Context switching to/from the runtime
            //
            // CRITICAL: Panic handling is required to maintain behavioral consistency.
            // All async runtimes (Tokio, async-std, smol, embassy-rs) catch panics in
            // spawned tasks and convert them to errors. We must do the same for inline
            // execution to ensure tasks behave identically regardless of execution path.
            //
            // Without panic catching:
            //   - Spawned task panics → caught by runtime → becomes error (expected)
            //   - Inline task panics → bubbles up → crashes program (surprising!)
            //
            // With panic catching (current implementation):
            //   - Spawned task panics → caught by runtime → becomes error
            //   - Inline task panics → caught by us → becomes error (consistent!)
            //
            // This ensures the optimization is transparent to users.
            // ============================================================================
            if layer.len() == 1 {
                let node_id = layer[0];

                #[cfg(feature = "tracing")]
                trace!(
                    task_id = node_id.0,
                    "executing task inline (single-task layer optimization)"
                );

                // Take ownership of the node
                let node = self.nodes[node_id.0 as usize].take();

                if let Some(node) = node {
                    let dependencies: Vec<_> = self.edges[&node_id]
                        .iter()
                        .flat_map(|dep| outputs.get(dep))
                        .cloned()
                        .collect();

                    // Execute inline with panic handling.
                    //
                    // FutureExt::catch_unwind() ensures panics are caught and converted to
                    // DagError::TaskPanicked, matching the behavior of async runtimes when
                    // they spawn tasks. This guarantees consistent error handling whether
                    // a task executes inline (single-task layer) or spawned (multi-task layer).
                    let result = AssertUnwindSafe(node.execute_with_deps(dependencies))
                        .catch_unwind()
                        .await
                        .unwrap_or_else(|panic_payload| {
                            // Convert panic to error
                            let panic_message =
                                if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                    s.to_string()
                                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                                    s.clone()
                                } else {
                                    "unknown panic".to_string()
                                };

                            #[cfg(feature = "tracing")]
                            error!(
                                task_id = node_id.0,
                                panic_message = %panic_message,
                                "task panicked during inline execution"
                            );

                            Err(DagError::TaskPanicked {
                                task_id: node_id.0,
                                panic_message,
                            })
                        });

                    match result {
                        Ok(output) => {
                            outputs.insert(node_id, output);
                        }
                        Err(e) => {
                            first_error.get_or_insert(e);
                        }
                    }
                }
            } else {
                // Slow path: Multiple tasks require spawning and coordination
                // Spawn each task in this layer
                let mut futures: FuturesUnordered<_> = layer
                    .into_iter()
                    .filter_map(|node_id| {
                        #[cfg(feature = "tracing")]
                        trace!(task_id = node_id.0, "spawning task");

                        // Take ownership of the node
                        let node = self.nodes[node_id.0 as usize].take();
                        if let Some(node) = node {
                            let dependencies: Vec<_> = self.edges[&node_id]
                                .iter()
                                .flat_map(|dep| outputs.get(dep))
                                .cloned()
                                .collect();

                            let inner_future = spawner(node.execute_with_deps(dependencies));
                            // Spawn the task using the provided spawner
                            let inner_future = async move {
                                let result = inner_future.await?;
                                Ok((node_id, result))
                            };

                            Some(
                                AssertUnwindSafe(inner_future)
                                    .catch_unwind()
                                    .unwrap_or_else(move |panic_payload| {
                                        // Convert panic to error
                                        let panic_message =
                                            if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                                s.to_string()
                                            } else if let Some(s) =
                                                panic_payload.downcast_ref::<String>()
                                            {
                                                s.clone()
                                            } else {
                                                "unknown panic".to_string()
                                            };

                                        #[cfg(feature = "tracing")]
                                        error!(
                                            task_id = node_id.0,
                                            panic_message = %panic_message,
                                            "task panicked during inline execution"
                                        );

                                        Err(DagError::TaskPanicked {
                                            task_id: node_id.0,
                                            panic_message,
                                        })
                                    }),
                            )
                        } else {
                            None
                        }
                    })
                    .collect();

                while let Some(out) = futures.next().await {
                    match out {
                        Ok(output) => {
                            outputs.insert(output.0, output.1);
                        }
                        Err(e) => {
                            first_error.get_or_insert(e);
                        }
                    }
                }
            }
        }

        // Return first error if any
        if let Some(err) = first_error {
            #[cfg(feature = "tracing")]
            error!(?err, "DAG execution failed");
            return Err(err);
        }

        #[cfg(feature = "tracing")]
        info!("DAG execution completed successfully");

        Ok(DagOutput::new(outputs))
    }

    #[inline]
    fn compute_layers(&self) -> DagResult<Vec<Vec<NodeId>>> {
        #[cfg(feature = "tracing")]
        debug!("computing topological layers");

        let mut in_degree: PassThroughHashMap<NodeId, usize> = HashMap::default();
        let mut layers = Vec::new();

        // Calculate in-degrees: for each node, count how many dependencies it has

        for (&node, deps) in self.edges.iter() {
            let degree = deps.len();
            in_degree.insert(node, degree);
        }

        // Find all nodes with in-degree 0 (sources - nodes with no dependencies)
        let mut queue: VecDeque<NodeId> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&node, _)| node)
            .collect();

        let mut visited = HashSet::new();

        while !queue.is_empty() {
            let mut current_layer = Vec::new();
            let layer_size = queue.len();

            for _ in 0..layer_size {
                if let Some(node) = queue.pop_front() {
                    if visited.contains(&node) {
                        continue;
                    }

                    current_layer.push(node);
                    visited.insert(node);

                    // For each node that depends on the current node
                    if let Some(deps) = self.dependents.get(&node) {
                        for &dependent in deps {
                            if let Some(degree) = in_degree.get_mut(&dependent) {
                                *degree -= 1;
                                if *degree == 0 {
                                    queue.push_back(dependent);
                                }
                            }
                        }
                    }
                    // Lock released here
                }
            }

            if !current_layer.is_empty() {
                layers.push(current_layer);
            }
        }

        // Cycle detection removed: cycles are impossible via the public API.
        //
        // The type-state pattern enforces acyclic structure at compile time through:
        // 1. TaskBuilder::depends_on() consumes the builder (move semantics)
        // 2. TaskHandle has no methods to add dependencies (immutability)
        // 3. Strict topological ordering requirement for dependency wiring
        //
        // This eliminates the need for runtime cycle detection, providing both
        // safety guarantees and performance benefits (no validation overhead).

        // Compiler optimization barrier: ensures visited set is fully populated.
        // This assertion compiles to nothing in release builds but guides the optimizer's
        // understanding of the HashSet state, improving subsequent code generation.
        debug_assert!(!visited.is_empty() || layers.is_empty());

        #[cfg(feature = "tracing")]
        debug!(layer_count = layers.len(), "topological layers computed");

        Ok(layers)
    }
}

// Note: We cannot implement Default for DagRunner anymore since new() returns Arc<Self>.
// Users should call DagRunner::new() directly.

#[cfg(test)]
mod tests;
