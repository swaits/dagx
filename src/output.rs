use std::{any::Any, sync::Arc};

use crate::{builder::NodeId, runner::PassThroughHashMap, DagError, DagResult, TaskHandle};

pub struct DagOutput {
    outputs: PassThroughHashMap<NodeId, Arc<dyn Any + Send + Sync + 'static>>,
}

impl DagOutput {
    /// Retrieve a task's output after [`crate::DagRunner::run`].
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
    /// #
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
    /// let mut dag = DagRunner::new();
    ///
    /// // Construct task with specific setting value
    /// let task = dag.add_task(Configuration::new(42));
    ///
    ///let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
    ///
    /// assert_eq!(output.get(task).unwrap(), 42);
    /// # };
    /// ```
    pub fn get<T: 'static + Send + Sync, H>(&mut self, handle: H) -> DagResult<T>
    where
        H: Into<TaskHandle<T>>,
    {
        let handle: TaskHandle<T> = handle.into();

        let arc_output = self
            .outputs
            .remove(&handle.id)
            .ok_or(DagError::ResultNotFound {
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

    pub(super) fn new(
        outputs: PassThroughHashMap<NodeId, Arc<dyn Any + Send + Sync + 'static>>,
    ) -> Self {
        Self { outputs }
    }
}
