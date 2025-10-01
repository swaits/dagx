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

pub(crate) trait DepsTuple<Input> {
    fn to_node_ids(&self) -> Vec<NodeId>;
}

// Implementation for unit (no dependencies)
impl DepsTuple<()> for () {
    fn to_node_ids(&self) -> Vec<NodeId> {
        Vec::new()
    }
}

// Implementation for single dependency (all forms)
impl<T> DepsTuple<T> for &TaskHandle<T> {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.id]
    }
}

impl<T> DepsTuple<T> for TaskHandle<T> {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.id]
    }
}

impl<T> DepsTuple<T> for (&TaskHandle<T>,) {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.0.id]
    }
}

impl<T> DepsTuple<T> for (TaskHandle<T>,) {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.0.id]
    }
}

// Support for &TaskBuilder
impl<'a, Tk: Task, Deps> DepsTuple<Tk::Output> for &TaskBuilder<'a, Tk, Deps> {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.id]
    }
}

impl<'a, Tk: Task, Deps> DepsTuple<Tk::Output> for (&TaskBuilder<'a, Tk, Deps>,) {
    fn to_node_ids(&self) -> Vec<NodeId> {
        vec![self.0.id]
    }
}

/// Macro to implement DepsTuple for different tuple sizes.
///
/// This macro exists because Rust lacks variadic generics - we need separate implementations
/// for each tuple size.
///
/// For each tuple size, generates two implementations:
/// - `(&TaskHandle<A>, &TaskHandle<B>, ...)` → `DepsTuple<(A, B, ...)>`
/// - `(&TaskBuilder<TkA, _>, &TaskBuilder<TkB, _>, ...)` → `DepsTuple<(TkA::Output, TkB::Output, ...)>`
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
    ($($T:ident : $Tk:ident : $D:ident),+) => {
        // For &TaskHandle tuples
        impl<$($T),+> DepsTuple<($($T,)+)> for ($(&TaskHandle<$T>,)+) {
            #[allow(non_snake_case)]
            fn to_node_ids(&self) -> Vec<NodeId> {
                let ($($T,)+) = self;
                vec![$($T.id,)+]
            }
        }

        // For &TaskBuilder tuples
        impl<'a, $($Tk: Task, $D),+> DepsTuple<($($Tk::Output,)+)> for ($(&TaskBuilder<'a, $Tk, $D>,)+) {
            #[allow(non_snake_case)]
            fn to_node_ids(&self) -> Vec<NodeId> {
                let ($($T,)+) = self;
                vec![$($T.id,)+]
            }
        }
    };
}

// Generate DepsTuple implementations for tuples of size 2-8.
// Each invocation generates 2 implementations (TaskHandle refs and TaskBuilder refs).
// Supporting up to 8 elements covers the vast majority of use cases.
impl_deps_tuple!(A:TkA:DA, B:TkB:DB);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC, D:TkD:DD);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC, D:TkD:DD, E:TkE:DE);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC, D:TkD:DD, E:TkE:DE, F:TkF:DF);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC, D:TkD:DD, E:TkE:DE, F:TkF:DF, G:TkG:DG);
impl_deps_tuple!(A:TkA:DA, B:TkB:DB, C:TkC:DC, D:TkD:DD, E:TkE:DE, F:TkF:DF, G:TkG:DG, H:TkH:DH);

#[cfg(test)]
mod tests;
