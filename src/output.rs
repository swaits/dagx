use std::{any::Any, sync::Arc};

use crate::{builder::NodeId, runner::PassThroughHashMap, TaskHandle};

pub struct DagOutput {
    outputs: PassThroughHashMap<NodeId, Arc<dyn Any + Send + Sync + 'static>>,
}

impl DagOutput {
    /// Retrieve a task's output after [`crate::DagRunner::run`].
    ///
    /// # Behavior
    ///
    /// All task outputs are stored after execution and can be retrieved via get().
    ///
    /// # Panics
    ///
    /// This function panics if called with a [`TaskHandle`] from a [`crate::DagRunner`] instance other than the
    /// one which produced this DagOutput.
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
    /// let mut output = dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() }).await.unwrap();
    ///
    /// assert_eq!(output.get(task), 42);
    /// # };
    /// ```
    pub fn get<T: 'static + Send + Sync>(&mut self, handle: TaskHandle<T>) -> T {
        // Because this consumes the task handle, it can only be retrieved once, so is certain to be there
        let arc_output = self.outputs.remove(&handle.id).unwrap();

        // Downcast Arc<dyn Any> to Arc<T>
        let output = arc_output.downcast::<T>().unwrap();

        // Because we never give the user access to anything but a reference to the Arc,
        // we can guarantee that the strong reference count is exactly one here
        Arc::into_inner(output).unwrap()
    }

    pub(super) fn new(
        outputs: PassThroughHashMap<NodeId, Arc<dyn Any + Send + Sync + 'static>>,
    ) -> Self {
        Self { outputs }
    }
}
