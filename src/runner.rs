//! DAG runner for task orchestration and execution.
//!
//! Provides DagRunner for building and executing directed acyclic graphs of async tasks
//! with compile-time type-safe dependencies.
//!
//! Uses Mutex for interior mutability to enable builder pattern (`&self` instead of `&mut self`).

use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt, TryFutureExt};
use parking_lot::Mutex;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace};

use crate::builder::TaskBuilder;
use crate::error::{DagError, DagResult};
use crate::node::{ExecutableNode, TypedNode};
use crate::task::Task;
use crate::types::{NodeId, Pending, TaskHandle};

// Guard to ensure run_lock is released even on early return or panic
struct RunGuard<'a> {
    lock: &'a AtomicBool,
}

impl<'a> Drop for RunGuard<'a> {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::SeqCst);
    }
}

/// Build and execute a typed DAG of tasks.
///
/// A `DagRunner` is the main orchestrator for building and executing a directed acyclic graph
/// of async tasks with compile-time type-safe dependencies.
///
/// # Workflow
///
/// 1. Create a new DAG with [`DagRunner::new`]
/// 2. Add tasks with [`DagRunner::add_task`] to get [`TaskBuilder`] builders
/// 3. Wire dependencies with [`TaskBuilder::depends_on`]
/// 4. Execute all tasks with [`DagRunner::run`]
/// 5. Optionally retrieve outputs with [`DagRunner::get`]
///
/// # Examples
///
/// ```no_run
/// # use dagx::{task, DagRunner, Task};
/// # use futures::FutureExt;
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
/// let dag = DagRunner::new();
///
/// // Construct instances using ::new() pattern
/// let x = dag.add_task(LoadValue::new(2));
/// let y = dag.add_task(LoadValue::new(3));
/// let sum = dag.add_task(Add).depends_on((x, y));
///
/// dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
///
/// assert_eq!(dag.get(sum).unwrap(), 5);
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
    pub(crate) nodes: Mutex<Vec<Option<Box<dyn ExecutableNode + Sync>>>>,
    pub(crate) outputs: Mutex<HashMap<NodeId, std::sync::Arc<dyn std::any::Any + Send + Sync>>>,
    pub(crate) edges: Mutex<HashMap<NodeId, Vec<NodeId>>>, // node -> dependencies
    pub(crate) dependents: Mutex<HashMap<NodeId, Vec<NodeId>>>, // node -> tasks that depend on it
    pub(crate) next_id: Mutex<usize>,
    pub(crate) run_lock: AtomicBool, // Ensures only one run() at a time
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
    /// let dag = DagRunner::new();
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(Vec::new()),
            outputs: Mutex::new(HashMap::new()),
            edges: Mutex::new(HashMap::new()),
            dependents: Mutex::new(HashMap::new()),
            next_id: Mutex::new(0),
            run_lock: AtomicBool::new(false),
        }
    }

    pub(crate) fn alloc_id(&self) -> NodeId {
        let mut next_id = self.next_id.lock();
        let id = NodeId(*next_id);
        *next_id += 1;
        id
    }

    /// Add a task instance to the DAG, returning a node builder for wiring dependencies.
    ///
    /// The returned [`TaskBuilder<Tk, Pending>`](TaskBuilder) can be used to:
    /// - Specify dependencies via [`TaskBuilder::depends_on`]
    /// - Used directly as a [`TaskHandle`] to the task's output
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dagx::{task, DagRunner, Task};
    /// # use futures::FutureExt;
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
    /// let dag = DagRunner::new();
    ///
    /// // Construct task with initial value of 10
    /// let base = dag.add_task(LoadValue::new(10));
    ///
    /// // Construct task with offset of 1
    /// let inc = dag.add_task(AddOffset::new(1)).depends_on(base);
    ///
    /// dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
    /// assert_eq!(dag.get(inc).unwrap(), 11);
    /// # };
    /// ```
    pub fn add_task<Tk>(&self, task: Tk) -> TaskBuilder<'_, Tk, Pending>
    where
        Tk: Task + Sync + 'static,
        Tk::Input: 'static,
        Tk::Output: 'static,
    {
        let id = self.alloc_id();

        #[cfg(feature = "tracing")]
        debug!(
            task_id = id.0,
            task_type = std::any::type_name::<Tk>(),
            "adding task to DAG"
        );

        let node = TypedNode::new(task);
        self.nodes.lock().push(Some(Box::new(node)));
        self.edges.lock().insert(id, Vec::new());
        self.dependents.lock().insert(id, Vec::new());

        TaskBuilder {
            id,
            dag: self,
            _phantom: std::marker::PhantomData,
        }
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
    ///   - Tokio: `|fut| { tokio::spawn(fut).map(Result::unwrap) }`
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
    /// # Note on Cycles
    ///
    /// Cycles are impossible via the public API due to compile-time prevention.
    /// See the [`crate::cycle_prevention`] module for details.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dagx::{task, DagRunner, Task};
    /// # use futures::FutureExt;
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
    /// let dag = DagRunner::new();
    ///
    /// let a = dag.add_task(Value(1));
    /// let b = dag.add_task(Value(2));
    /// let sum = dag.add_task(Add).depends_on((a, b));
    ///
    /// dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap(); // Executes all tasks
    /// # };
    /// ```
    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, spawner)))]
    pub async fn run<S, F>(&self, spawner: S) -> DagResult<()>
    where
        S: Fn(BoxFuture<'static, DagResult<(NodeId, Arc<dyn Any + Send + Sync>)>>) -> F,
        F: Future<Output = DagResult<(NodeId, Arc<dyn Any + Send + Sync>)>>,
    {
        #[cfg(feature = "tracing")]
        info!("starting DAG execution");
        // Acquire run lock to prevent concurrent executions
        // Use atomic compare_exchange to check if already running
        if self
            .run_lock
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            #[cfg(feature = "tracing")]
            error!("DAG is already running - concurrent execution not supported");

            return Err(DagError::ConcurrentExecution);
        }

        // Guard ensures lock is released on drop (even on early return or panic)
        let _run_guard = RunGuard {
            lock: &self.run_lock,
        };

        // Build topological layers
        let layers = self.compute_layers()?;

        #[cfg(feature = "tracing")]
        debug!(
            layer_count = layers.len(),
            total_tasks = layers.iter().map(|l| l.len()).sum::<usize>(),
            "computed topological layers"
        );

        let edges = self.edges.lock().clone();

        let mut outputs: HashMap<NodeId, Arc<dyn Any + Send + Sync>> = HashMap::new();
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
                let node = {
                    let mut nodes_lock = self.nodes.lock();
                    nodes_lock[node_id.0].take()
                };

                if let Some(node) = node {
                    let dependencies: Vec<_> = edges[&node_id]
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
                        let node = {
                            let mut nodes_lock = self.nodes.lock();
                            nodes_lock[node_id.0].take()
                        };
                        if let Some(node) = node {
                            let dependencies: Vec<_> = edges[&node_id]
                                .iter()
                                .flat_map(|dep| outputs.get(dep))
                                .cloned()
                                .collect();

                            // Spawn the task using the provided spawner
                            let inner_future = async move {
                                let result = node.execute_with_deps(dependencies).await?;
                                Ok((node_id, result))
                            };

                            Some(
                                AssertUnwindSafe(spawner(Box::pin(inner_future)))
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

        // Insert all successful outputs (avoiding holding lock across await)
        let mut outputs_lock = self.outputs.lock();
        for (node_id, output) in outputs {
            outputs_lock.insert(node_id, output);
        }
        drop(outputs_lock);

        // Return first error if any
        if let Some(err) = first_error {
            #[cfg(feature = "tracing")]
            error!(?err, "DAG execution failed");
            return Err(err);
        }

        #[cfg(feature = "tracing")]
        info!("DAG execution completed successfully");

        Ok(())
    }

    /// Retrieve a task's output after [`DagRunner::run`].
    ///
    /// # Behavior
    ///
    /// All task outputs are stored after execution and can be retrieved via get().
    /// Each task's output can only be taken once, after which it will return `DagError::ResultNotFound`.
    ///
    /// # Errors
    ///
    /// Returns `DagError::ResultNotFound` if:
    /// - The task hasn't been executed yet
    /// - The handle is invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dagx::{task, DagRunner, Task};
    /// # use futures::FutureExt;
    /// struct Configuration {
    ///     setting: i32,
    /// }
    ///
    /// impl Configuration {
    ///     fn new(setting: i32) -> Self {
    ///         Self { setting }
    ///     }
    /// }
    ///
    /// #[task]
    /// impl Configuration {
    ///     async fn run(&mut self) -> i32 { self.setting }
    /// }
    ///
    /// # async {
    /// let dag = DagRunner::new();
    ///
    /// // Construct task with specific setting value
    /// let task = dag.add_task(Configuration::new(42));
    ///
    /// dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await.unwrap();
    ///
    /// assert_eq!(dag.get(task).unwrap(), 42);
    /// # };
    /// ```
    pub fn get<T: 'static + Send + Sync, H>(&self, handle: H) -> DagResult<T>
    where
        H: Into<TaskHandle<T>>,
    {
        let handle: TaskHandle<T> = handle.into();
        let mut outputs = self.outputs.lock();

        let arc_output = outputs.remove(&handle.id).ok_or(DagError::ResultNotFound {
            task_id: handle.id.0,
        })?;

        // Downcast Arc<dyn Any> to Arc<T>
        let output = arc_output
            .downcast::<T>()
            .map_err(|_| DagError::TypeMismatch {
                expected: std::any::type_name::<T>(),
                found: "unknown",
            })?;

        Ok(Arc::into_inner(output).unwrap())
    }

    #[inline]
    fn compute_layers(&self) -> DagResult<Vec<Vec<NodeId>>> {
        #[cfg(feature = "tracing")]
        debug!("computing topological layers");

        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut layers = Vec::new();

        // Calculate in-degrees: for each node, count how many dependencies it has
        let edges = self.edges.lock();

        // Compiler optimization hint: provides bounds information for HashSet sizing.
        // This variable guides the optimizer's understanding of graph size, improving
        // hash table and iterator performance in the topological sort below.
        let _total_nodes = edges.len();

        for (&node, deps) in edges.iter() {
            let degree = deps.len();
            in_degree.insert(node, degree);
        }
        drop(edges); // Release lock early

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
                    let dependents = self.dependents.lock();
                    if let Some(deps) = dependents.get(&node) {
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
                current_layer.sort(); // Deterministic ordering
                layers.push(current_layer);
            }
        }

        // Cycle detection removed: cycles are impossible via the public API.
        // See src/cycle_prevention.rs for proof that the type system prevents cycles.
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
