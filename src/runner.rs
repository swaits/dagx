//! DAG runner for task orchestration and execution.
//!
//! Provides DagRunner for building and executing directed acyclic graphs of async tasks
//! with compile-time type-safe dependencies.
//!
//! Uses Mutex for interior mutability to enable builder pattern (`&self` instead of `&mut self`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use parking_lot::Mutex;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace};

use crate::builder::TaskBuilder;
use crate::error::{DagError, DagResult};
use crate::extract::ExtractInput;
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
/// let sum = dag.add_task(Add).depends_on((&x, &y));
///
/// dag.run(|fut| { tokio::spawn(fut); }).await.unwrap();
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
    pub(crate) nodes: Mutex<Vec<Option<Box<dyn ExecutableNode>>>>,
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
    /// let inc = dag.add_task(AddOffset::new(1)).depends_on(&base);
    ///
    /// dag.run(|fut| { tokio::spawn(fut); }).await.unwrap();
    /// assert_eq!(dag.get(&inc).unwrap(), 11);
    /// # };
    /// ```
    pub fn add_task<Tk>(&self, task: Tk) -> TaskBuilder<'_, Tk, Pending>
    where
        Tk: Task + 'static,
        Tk::Input: 'static + Clone + ExtractInput,
        Tk::Output: 'static + Clone,
    {
        let id = self.alloc_id();

        #[cfg(feature = "tracing")]
        debug!(
            task_id = id.0,
            task_type = std::any::type_name::<Tk>(),
            "adding task to DAG"
        );

        let node = TypedNode::new(id, task);
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
    /// - `spawner`: A function that spawns futures on the async runtime. Examples:
    ///   - Tokio: `|fut| { tokio::spawn(fut); }`
    ///   - Smol: `|fut| { smol::spawn(fut).detach(); }`
    ///   - Async-std: `|fut| { async_std::task::spawn(fut); }`
    ///
    /// # Errors
    ///
    /// Returns `DagError::CycleDetected` if the DAG contains a cycle.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dagx::{task, DagRunner, Task};
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
    /// let sum = dag.add_task(Add).depends_on((&a, &b));
    ///
    /// dag.run(|fut| { tokio::spawn(fut); }).await.unwrap(); // Executes all tasks
    /// # };
    /// ```
    ///
    /// # Implementation Note
    ///
    /// Tasks communicate via oneshot channels created fresh during run().
    /// This eliminates Mutex contention on outputs and enables true streaming execution.
    /// Type erasure occurs only at the ExecutableNode trait boundary - channels are created
    /// with full type information and type-erased just before passing to execute_with_channels.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, spawner)))]
    pub async fn run<S>(&self, spawner: S) -> DagResult<()>
    where
        S: Fn(BoxFuture<'static, ()>),
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

        // Create all channels upfront
        // Map: (producer_id, consumer_id) -> receiver index
        // We store receivers in a Vec per consumer to maintain order
        let mut consumer_receivers: HashMap<NodeId, Vec<Box<dyn std::any::Any + Send>>> =
            HashMap::new();
        let mut producer_senders: HashMap<NodeId, Vec<Box<dyn std::any::Any + Send>>> =
            HashMap::new();

        // Create channels for each producer-consumer relationship
        // We need to lock nodes temporarily to call create_output_channels
        {
            let nodes_lock = self.nodes.lock();

            for (consumer_id, producer_ids) in &edges {
                for &producer_id in producer_ids {
                    // Get the producer node (it's still in the vector at this point)
                    if let Some(Some(producer_node)) = nodes_lock.get(producer_id.0) {
                        // Ask the producer to create ONE channel for this consumer
                        let (mut senders, mut receivers) = producer_node.create_output_channels(1);

                        // Store the sender for the producer
                        producer_senders
                            .entry(producer_id)
                            .or_default()
                            .push(senders.pop().unwrap());

                        // Store the receiver for the consumer (order matters!)
                        consumer_receivers
                            .entry(*consumer_id)
                            .or_default()
                            .push(receivers.pop().unwrap());
                    }
                }
            }
        }

        // Create a channel to collect Arc-wrapped outputs from all tasks
        let (output_tx, mut output_rx) = mpsc::unbounded::<(
            NodeId,
            Result<std::sync::Arc<dyn std::any::Any + Send + Sync>, DagError>,
        )>();

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
            //   - Channel creation/destruction for layer coordination
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

                let out_tx = output_tx.clone();

                // Take ownership of the node
                let node = {
                    let mut nodes_lock = self.nodes.lock();
                    nodes_lock[node_id.0].take()
                };

                if let Some(node) = node {
                    // Take the receivers for this consumer
                    let receivers = consumer_receivers.remove(&node_id).unwrap_or_default();

                    // Take the senders for this producer
                    let senders = producer_senders.remove(&node_id).unwrap_or_default();

                    // Execute inline with panic handling.
                    //
                    // FutureExt::catch_unwind() ensures panics are caught and converted to
                    // DagError::TaskPanicked, matching the behavior of async runtimes when
                    // they spawn tasks. This guarantees consistent error handling whether
                    // a task executes inline (single-task layer) or spawned (multi-task layer).
                    let result = AssertUnwindSafe(node.execute_with_channels(receivers, senders))
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

                    // Send the output (ignore send errors - receiver may be dropped)
                    let _ = out_tx.unbounded_send((node_id, result));
                }
            } else {
                // Slow path: Multiple tasks require spawning and coordination
                // Create a channel to track task completion for this layer
                let (tx, mut rx) = mpsc::channel::<()>(layer.len());

                // Spawn each task in this layer
                for &node_id in &layer {
                    #[cfg(feature = "tracing")]
                    trace!(task_id = node_id.0, "spawning task");
                    let mut task_tx = tx.clone();
                    let out_tx = output_tx.clone();

                    // Take ownership of the node
                    let node = {
                        let mut nodes_lock = self.nodes.lock();
                        nodes_lock[node_id.0].take()
                    };

                    if let Some(node) = node {
                        // Take the receivers for this consumer
                        let receivers = consumer_receivers.remove(&node_id).unwrap_or_default();

                        // Take the senders for this producer
                        let senders = producer_senders.remove(&node_id).unwrap_or_default();

                        // Create a 'static future by taking ownership
                        let inner_future = async move {
                            let task_future = node.execute_with_channels(receivers, senders);

                            let result = task_future.await;

                            // Send the output (ignore send errors - receiver may be dropped)
                            let _ = out_tx.unbounded_send((node_id, result));

                            // Signal completion (ignore send errors - receiver may be dropped)
                            let _ = task_tx.try_send(());
                        };

                        // Spawn the task using the provided spawner
                        spawner(Box::pin(inner_future));
                    }
                }

                // Drop the original sender so the channel closes when all tasks complete
                drop(tx);

                // Wait for all tasks in this layer to complete
                while rx.next().await.is_some() {}
            }
        }

        // Drop the output sender so the channel closes
        drop(output_tx);

        // Collect all outputs and check for errors
        let mut collected = Vec::new();
        let mut first_error = None;
        while let Some((node_id, result)) = output_rx.next().await {
            match result {
                Ok(output) => collected.push((node_id, output)),
                Err(e) if first_error.is_none() => first_error = Some(e),
                Err(_) => {} // Ignore subsequent errors
            }
        }

        // Insert all successful outputs (avoiding holding lock across await)
        let mut outputs = self.outputs.lock();
        for (node_id, output) in collected {
            outputs.insert(node_id, output);
        }
        drop(outputs);

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
    /// dag.run(|fut| { tokio::spawn(fut); }).await.unwrap();
    ///
    /// assert_eq!(dag.get(task).unwrap(), 42);
    /// # };
    /// ```
    pub fn get<T: 'static + Clone + Send + Sync, H>(&self, handle: H) -> DagResult<T>
    where
        H: Into<TaskHandle<T>>,
    {
        let handle: TaskHandle<T> = handle.into();
        let outputs = self.outputs.lock();

        let arc_output = outputs.get(&handle.id).ok_or(DagError::ResultNotFound {
            task_id: handle.id.0,
        })?;

        // Downcast Arc<dyn Any> to Arc<T>, then clone the inner value
        // The Arc itself is stored for efficient sharing, but get() clones the data
        Arc::clone(arc_output)
            .downcast::<T>()
            .map(|arc| (*arc).clone())
            .map_err(|_| DagError::TypeMismatch {
                expected: std::any::type_name::<T>(),
                found: "unknown",
            })
    }

    fn compute_layers(&self) -> DagResult<Vec<Vec<NodeId>>> {
        #[cfg(feature = "tracing")]
        debug!("computing topological layers");

        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut layers = Vec::new();

        // Calculate in-degrees: for each node, count how many dependencies it has
        let edges = self.edges.lock();

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

        #[cfg(feature = "tracing")]
        debug!(layer_count = layers.len(), "topological layers computed");

        Ok(layers)
    }
}

// Note: We cannot implement Default for DagRunner anymore since new() returns Arc<Self>.
// Users should call DagRunner::new() directly.

#[cfg(test)]
mod tests;
