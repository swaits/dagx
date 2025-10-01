//! Internal node types for DAG execution.
//!
//! Provides type erasure to store tasks with different types in a single collection.
//! The public API remains fully type-safe; type erasure is purely an internal implementation detail.
//!
//! - **TypedNode\<T\>**: Stores a task with full type information
//! - **ExecutableNode**: Trait for executing type-erased tasks

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures::channel::oneshot;

use crate::error::DagError;
use crate::extract::ExtractInput;
use crate::task::Task;
use crate::types::NodeId;

/// Type-erased channel endpoints (senders or receivers)
type ErasedChannels = Vec<Box<dyn Any + Send>>;

/// Type-erased channel pair (senders, receivers)
type ChannelPair = (ErasedChannels, ErasedChannels);

/// Type-erased async execution result (Arc-wrapped for efficient fanout)
type ExecuteFuture =
    Pin<Box<dyn Future<Output = Result<Arc<dyn Any + Send + Sync>, DagError>> + Send>>;

/// Internal trait for executing heterogeneous tasks.
///
/// This trait provides type erasure necessary for storing different task types in a single
/// collection. While the public API (Task, Handle, Node) is fully type-safe, internally we
/// need dynamic dispatch to execute tasks with different input/output types.
///
/// This is the ONLY place where type erasure occurs - channels are created with full type
/// information and only erased at this trait boundary.
///
/// This is an implementation detail and not part of the public API.
pub(crate) trait ExecutableNode: Send {
    /// Create type-erased output channels for this node's dependents.
    ///
    /// Returns (senders, receivers) where:
    /// - senders: Type-erased senders that this node will use to send outputs
    /// - receivers: Type-erased receivers that dependents will use to receive inputs
    fn create_output_channels(&self, num_dependents: usize) -> ChannelPair;

    /// Execute the task with type-erased input receivers and output senders.
    ///
    /// The TypedNode implementation knows the concrete types and can safely downcast.
    /// Consumes the node and returns the output value.
    fn execute_with_channels(
        self: Box<Self>,
        receivers: ErasedChannels,
        senders: ErasedChannels,
    ) -> ExecuteFuture;
}

/// Fully-typed node storage for a single task.
///
/// Stores a task of type T. Implements ExecutableNode for type erasure,
/// allowing heterogeneous tasks to be stored in a single Vec.
///
/// # Ownership Model
///
/// The task is owned directly (no Mutex needed) and consumed during execution.
/// Each task executes exactly once, taking ownership and producing an output.
///
/// # Oneshot Channels
///
/// Outputs are communicated via oneshot channels for inter-task communication.
/// The output is also returned from execute_with_channels for central storage.
pub(crate) struct TypedNode<T: Task> {
    pub(crate) id: NodeId,
    pub(crate) task: T,
}

impl<T: Task + 'static> TypedNode<T> {
    pub(crate) fn new(id: NodeId, task: T) -> Self {
        Self { id, task }
    }
}

impl<T: Task + 'static> ExecutableNode for TypedNode<T>
where
    T::Input: 'static + Clone + ExtractInput,
    T::Output: 'static + Clone,
{
    fn create_output_channels(&self, num_dependents: usize) -> ChannelPair {
        let mut senders = Vec::new();
        let mut receivers = Vec::new();

        for _ in 0..num_dependents {
            // Create channels that pass Arc-wrapped outputs (efficient for fanout)
            let (tx, rx) = oneshot::channel::<Arc<T::Output>>();
            senders.push(Box::new(tx) as Box<dyn Any + Send>);
            receivers.push(Box::new(rx) as Box<dyn Any + Send>);
        }

        (senders, receivers)
    }

    fn execute_with_channels(
        self: Box<Self>,
        receivers: ErasedChannels,
        senders: ErasedChannels,
    ) -> ExecuteFuture {
        Box::pin(async move {
            // Extract typed input from type-erased receivers
            // TypedNode<T> knows T::Input, so it can safely downcast
            let input = match T::Input::extract_from_channels(receivers).await {
                Ok(input) => input,
                Err(_msg) => {
                    // Failed to extract input - likely channel closed or type mismatch
                    return Err(DagError::InvalidDependency { task_id: self.id.0 });
                }
            };

            // Execute task (consumes self.task)
            let output = self.task.run(input).await;

            // Arc Wrapping Strategy:
            // Wrap output in Arc ONCE for efficient fan-out sharing.
            // For 1:N dependencies, this enables O(1) sharing (Arc clone = atomic increment)
            // instead of O(N) cloning (N deep copies of output data).
            // Downstream tasks receive &T after ExtractInput extracts from Arc.
            let arc_output = Arc::new(output);

            // Send Arc to all dependents via their channels
            // TypedNode<T> knows T::Output, so downcast is safe
            for sender_any in senders {
                if let Ok(sender) = sender_any.downcast::<oneshot::Sender<Arc<T::Output>>>() {
                    // Clone Arc (just atomic refcount increment, NOT data clone)
                    // This is the key performance optimization for fan-out patterns
                    // Ignore send errors - dependent might have been cancelled
                    let _ = sender.send(Arc::clone(&arc_output));
                }
            }

            // Return Arc-wrapped output (for storage by runner)
            Ok(arc_output as Arc<dyn Any + Send + Sync>)
        })
    }
}

#[cfg(test)]
mod tests;
