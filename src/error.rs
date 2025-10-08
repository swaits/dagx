//! Error types for DAG operations.
//!
//! This module defines the error types that can occur during DAG construction and execution.

/// Errors that can occur during DAG construction and execution
#[derive(Debug, Clone, PartialEq)]
pub enum DagError {
    /// A cycle was detected in the DAG
    CycleDetected {
        nodes: Vec<usize>,
        description: String,
    },
    /// Invalid dependency: task does not exist
    InvalidDependency { task_id: usize },
    /// Type mismatch in task dependencies
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },
    /// Task panicked during execution
    TaskPanicked {
        task_id: usize,
        panic_message: String,
    },
    /// Result not found for task
    ResultNotFound { task_id: usize },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagError::CycleDetected { nodes, description } => {
                write!(
                    f,
                    "Cycle detected in DAG: {}\n\
                     Nodes involved: {:?}\n\
                     \n\
                     A DAG cannot have cycles. Review your dependency graph to identify \
                     and remove circular dependencies.",
                    description, nodes
                )
            }
            DagError::InvalidDependency { task_id } => {
                write!(
                    f,
                    "Invalid dependency: task #{} does not exist.\n\
                     \n\
                     Ensure all task handles reference tasks that have been added to the DAG.",
                    task_id
                )
            }
            DagError::TypeMismatch { expected, found } => {
                write!(
                    f,
                    "Type mismatch in task dependencies.\n\
                     Expected: {}\n\
                     Found: {}\n\
                     \n\
                     Verify that dependency types match the task's Input type exactly.",
                    expected, found
                )
            }
            DagError::TaskPanicked {
                task_id,
                panic_message,
            } => {
                write!(
                    f,
                    "Task #{} panicked during execution: {}\n\
                     \n\
                     A task panicked, indicating a bug. The entire DAG execution is aborted.",
                    task_id, panic_message
                )
            }
            DagError::ResultNotFound { task_id } => {
                write!(
                    f,
                    "Result not found for task #{}.\n\
                     \n\
                     Call dag.run() before accessing results with get().",
                    task_id
                )
            }
        }
    }
}

impl std::error::Error for DagError {}

/// Result type for DAG operations
#[cfg(not(tarpaulin_include))]
pub type DagResult<T> = Result<T, DagError>;

#[cfg(test)]
mod tests;
