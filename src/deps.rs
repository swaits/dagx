//! Dependency tuple trait and implementations.
//!
//! This module defines the DepsTuple trait that allows specifying dependencies in
//! various forms (handles, builders, tuples). It uses macro-generated implementations
//! for different tuple sizes.
//!
//! This trait is internal and not meant for external implementation. Users should only
//! use the provided implementations.

use crate::builder::TaskBuilder;
use crate::task::Task;
use crate::types::{NodeId, TaskHandle};

trait IntoNodeRef {
    type Output;
    fn into_node_ref(self) -> NodeId;
}

impl<T> IntoNodeRef for &TaskHandle<T> {
    type Output = T;
    fn into_node_ref(self) -> NodeId {
        self.id
    }
}

impl<T> IntoNodeRef for TaskHandle<T> {
    type Output = T;
    fn into_node_ref(self) -> NodeId {
        self.id
    }
}

impl<'runner, Tk: Task, Deps> IntoNodeRef for TaskBuilder<'runner, Tk, Deps> {
    type Output = Tk::Output;

    fn into_node_ref(self) -> NodeId {
        self.id
    }
}

pub(crate) trait DepsTuple<Input> {
    fn to_node_ids(self) -> Vec<NodeId>;
}

// Implementation for unit (no dependencies)
impl DepsTuple<()> for () {
    fn to_node_ids(self) -> Vec<NodeId> {
        Vec::new()
    }
}

// Implementation for single dependency (all forms)
impl<T> DepsTuple<T::Output> for T
where
    T: IntoNodeRef,
{
    fn to_node_ids(self) -> Vec<NodeId> {
        vec![self.into_node_ref()]
    }
}

impl<T> DepsTuple<T::Output> for (T,)
where
    T: IntoNodeRef,
{
    fn to_node_ids(self) -> Vec<NodeId> {
        vec![self.0.into_node_ref()]
    }
}

/// Macro to implement DepsTuple for different tuple sizes.
///
/// This macro exists because Rust lacks variadic generics - we need separate implementations
/// for each tuple size.
///
/// This allows flexible dependency specification:
/// ```ignore
/// // Using handles
/// task.depends_on((&handle_a, &handle_b))
///
/// // Using builders directly
/// task.depends_on((&dag.add_task(TaskA), &dag.add_task(TaskB)))
/// ```
macro_rules! impl_deps_tuple {
    ($($T:ident),+) => {
        // For tuples of any combination of &TaskHandle, TaskHandle, and TaskBuilder
        impl<$($T: IntoNodeRef),+> DepsTuple<($($T::Output,)+)> for ($($T,)+) {
            #[allow(non_snake_case)]
            fn to_node_ids(self) -> Vec<NodeId> {
                let ($($T,)+) = self;
                vec![$($T.into_node_ref(),)+]
            }
        }
    };
}

// Generate DepsTuple implementations for tuples of size 2-8.
// Supporting up to 8 elements covers the vast majority of use cases.
impl_deps_tuple!(T1, T2);
impl_deps_tuple!(T1, T2, T3);
impl_deps_tuple!(T1, T2, T3, T4);
impl_deps_tuple!(T1, T2, T3, T4, T5);
impl_deps_tuple!(T1, T2, T3, T4, T5, T6);
impl_deps_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_deps_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);

#[cfg(test)]
mod tests;
