//! Error types for DAG operations.
//!
//! This module defines the error types that can occur during DAG construction and execution.

/// Errors that can occur during DAG construction and execution
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DagError {
    /// Task panicked during execution
    TaskPanicked { task_id: u32, panic_message: String },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for DagError {}

/// Result type for DAG operations
#[cfg(not(tarpaulin_include))]
pub type DagResult<T> = Result<T, DagError>;

#[cfg(test)]
mod tests;
