//! Dependency tuple trait and implementations.
//!
//! This module defines the DepsTuple trait that allows specifying dependencies in
//! various forms (handles, builders, tuples). It uses macro-generated implementations
//! for different tuple sizes.
//!
//! This trait is internal and not meant for external implementation. Users should only
//! use the provided implementations.

use crate::builder::{NodeId, TaskHandle};

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
impl<O> DepsTuple<(O,)> for TaskHandle<O> {
    fn to_node_ids(self) -> Vec<NodeId> {
        vec![self.id]
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
/// task.depends_on((handle_a, handle_b))
///
/// // Using builders directly
/// task.depends_on((dag.add_task(TaskA), dag.add_task(TaskB)))
/// ```
macro_rules! impl_deps_tuple {
    ($($T:ident : $O:ident),+) => {
        impl<$($O),+> DepsTuple<($($O,)+)> for ($(TaskHandle<$O>,)+)
        {
            #[allow(non_snake_case)]
            fn to_node_ids(self) -> Vec<NodeId> {
                let ($($O,)+) = self;
                vec![$($O.id,)+]
            }
        }
    };
}

// Generate DepsTuple implementations for tuples of size 1-8.
// Supporting up to 8 elements covers the vast majority of use cases.
impl_deps_tuple!(T1:O1);
impl_deps_tuple!(T1:O1, T2:O2);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3, T4:O4);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3, T4:O4, T5:O5);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3, T4:O4, T5:O5, T6:O6);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3, T4:O4, T5:O5, T6:O6, T7:O7);
impl_deps_tuple!(T1:O1, T2:O2, T3:O3, T4:O4, T5:O5, T6:O6, T7:O7, T8:O8);

#[cfg(test)]
mod tests;
