//! Input extraction for task dependencies via a list of dependencies.
//!
//! # Legacy Trait for Internal Use Only
//!
//! **NOTE**: This module is now primarily for internal use. The `ExtractInput` trait is only
//! used by the internal `task_fn` test helper function.
//!
//! **For regular tasks**: The `#[task]` macro generates inline `extract_and_run()` methods with
//! type-specific extraction logic. This means **ANY type** implementing `Clone + Send + Sync`
//! works automatically without needing `ExtractInput` implementations!
//!
//! # How it works
//!
//! When a task executes, it needs to extract its inputs from type-erased oneshot dependencies.
//! Outputs are wrapped in Arc for efficient fanout (cheap Arc clones instead of data clones).
//!
//! **Modern approach (via `#[task]` macro)**:
//! - The macro generates custom extraction logic inline in `extract_and_run()`
//! - Works with ANY type (just needs `Clone + Send + Sync`)
//! - No trait implementations needed!
//!
//! **Legacy approach (via `ExtractInput` trait)**:
//! - Used only by the internal `task_fn` test helper
//! - Provides implementations for primitives, standard types, and tuples
//! - Requires explicit trait implementations for custom types (not recommended)
//!
//! # Why macros?
//!
//! Rust lacks variadic generics, so we use macros to generate implementations for:
//! - Common primitive/standard types (one impl per type)
//! - Tuples of different sizes (one impl per size, up to 8 elements)
//!
//! This is standard practice in Rust for working with tuples of different arities.

use std::any::Any;
use std::sync::Arc;

/// Helper trait for async input extraction from list of dependencies.
///
/// Type erasure occurs only at the ExecutableNode trait boundary - by the time
/// we're in ExtractInput, we know the concrete type and can safely downcast.
///
/// NOTE: This trait is now primarily used by the internal `task_fn` test helper.
/// The `#[task]` macro generates inline extraction logic via `extract_and_run()`,
/// which allows ANY type to work without requiring ExtractInput implementations.
#[allow(dead_code)]
pub(crate) trait ExtractInput: Sized + Clone {
    fn extract_from_deps(
        deps: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> impl std::future::Future<Output = Result<Self, String>> + Send;
}

// Unit type - no dependencies
impl ExtractInput for () {
    async fn extract_from_deps(_deps: Vec<Arc<dyn Any + Send + Sync>>) -> Result<Self, String> {
        Ok(())
    }
}

// Implementations for common primitive and standard library types.
// These allow tasks to receive these types as single inputs without wrapping in tuples.
// Receives Arc<T> from runner, clones the inner value for task consumption.
// For custom types, implement ExtractInput following the same pattern.
macro_rules! impl_extract_single {
    ($($t:ty),+) => {
        $(
            impl ExtractInput for $t {
                async fn extract_from_deps(mut dependencies: Vec<Arc<dyn Any + Send + Sync>>) -> Result<Self, String> {
                    if dependencies.len() != 1 {
                        return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
                    }
                    let arc_value = dependencies.pop()
                        .unwrap()
                        .downcast::<$t>()
                        .map_err(|_| format!("Type mismatch: expected Arc<{}>", std::any::type_name::<$t>()))?;
                    Ok((*arc_value).clone())
                }
            }
        )+
    };
}

impl_extract_single!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool, char, String
);

// Implement ExtractInput for Result types to support error handling patterns
impl<T, E> ExtractInput for Result<T, E>
where
    T: 'static + Clone + Send + Sync,
    E: 'static + Clone + Send + Sync,
{
    async fn extract_from_deps(
        mut dependencies: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Self, String> {
        if dependencies.len() != 1 {
            return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
        }
        let arc_value = dependencies
            .pop()
            .unwrap()
            .downcast::<Result<T, E>>()
            .map_err(|_| {
                format!(
                    "Type mismatch: expected Arc<Result<{}, {}>>",
                    std::any::type_name::<T>(),
                    std::any::type_name::<E>()
                )
            })?;
        Ok((*arc_value).clone())
    }
}

// Implement ExtractInput for Option types
impl<T> ExtractInput for Option<T>
where
    T: 'static + Clone + Send + Sync,
{
    async fn extract_from_deps(
        mut dependencies: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Self, String> {
        if dependencies.len() != 1 {
            return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
        }
        let arc_value = dependencies
            .pop()
            .unwrap()
            .downcast::<Option<T>>()
            .map_err(|_| {
                format!(
                    "Type mismatch: expected Arc<Option<{}>>",
                    std::any::type_name::<T>()
                )
            })?;
        Ok((*arc_value).clone())
    }
}

// Implement ExtractInput for Vec types
impl<T> ExtractInput for Vec<T>
where
    T: 'static + Clone + Send + Sync,
{
    async fn extract_from_deps(
        mut dependencies: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Self, String> {
        if dependencies.len() != 1 {
            return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
        }
        let arc_value = dependencies
            .pop()
            .unwrap()
            .downcast::<Vec<T>>()
            .map_err(|_| {
                format!(
                    "Type mismatch: expected Arc<Vec<{}>>",
                    std::any::type_name::<T>()
                )
            })?;
        Ok((*arc_value).clone())
    }
}

// Implement ExtractInput for HashMap types
impl<K, V> ExtractInput for std::collections::HashMap<K, V>
where
    K: 'static + Clone + Send + Sync + Eq + std::hash::Hash,
    V: 'static + Clone + Send + Sync,
{
    async fn extract_from_deps(
        mut dependencies: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Self, String> {
        if dependencies.len() != 1 {
            return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
        }
        let arc_value = dependencies
            .pop()
            .unwrap()
            .downcast::<std::collections::HashMap<K, V>>()
            .map_err(|_| {
                format!(
                    "Type mismatch: expected Arc<HashMap<{}, {}>>",
                    std::any::type_name::<K>(),
                    std::any::type_name::<V>()
                )
            })?;
        Ok((*arc_value).clone())
    }
}

// Implement ExtractInput for Arc types - SPECIAL CASE: just clone the Arc, don't unwrap!
// This allows zero-copy data sharing for users who wrap their data in Arc.
impl<T> ExtractInput for Arc<T>
where
    T: 'static + Clone + Send + Sync,
{
    async fn extract_from_deps(
        mut dependencies: Vec<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Self, String> {
        if dependencies.len() != 1 {
            return Err(format!("Expected 1 dependency, got {}", dependencies.len()));
        }
        let arc_arc_value = dependencies
            .pop()
            .unwrap()
            .downcast::<Arc<T>>()
            .map_err(|_| {
                format!(
                    "Type mismatch: expected Arc<Arc<{}>>",
                    std::any::type_name::<T>()
                )
            })?;
        // Return the inner Arc (clone is cheap - just refcount increment)
        Ok(Arc::clone(&*arc_arc_value))
    }
}

/// Macro to implement ExtractInput for tuple types.
///
/// We need separate implementations for each tuple size (2-8) because Rust doesn't support
/// variadic generics. This is standard practice when working with tuples in Rust.
///
/// # Compile-time Safety
///
/// The number of dependencies is guaranteed to match the tuple size at compile time through:
/// 1. The `DepsTuple<Tk::Input>` trait bound on `depends_on()` ensures type/arity matching
/// 2. The `IsUnitType` constraint on `From<TaskBuilder>` prevents using tasks without calling `depends_on()`
///
/// Therefore, no runtime validation is needed - if it compiles, the dependency count is correct.
macro_rules! impl_extract_tuple {
    ($($T:ident : $idx:tt),+) => {
        impl<$($T: 'static + Clone + Send + Sync),+> ExtractInput for ($($T,)+) {
            async fn extract_from_deps(dependencies: Vec<Arc<dyn Any + Send + Sync>>) -> Result<Self, String> {
                // Verify we have the correct number of dependencies
                let expected_count = [$($idx),+].len();
                if dependencies.len() != expected_count {
                    return Err(format!("Expected {} dependencies, got {}", expected_count, dependencies.len()));
                }

                // Convert Vec to iterator so we can consume it
                let mut iter = dependencies.into_iter();

                // dependencies now contain Arc-wrapped values for efficient fanout
                // Dereference Box after downcasting to get the parameter itself
                // Use type names as variable names to avoid numeric suffix issues
                let arc_results = (
                    $(
                            iter.next()
                            .ok_or_else(|| format!("Missing parameter at index {}", $idx))?
                            .downcast::<$T>()
                            .map_err(|_| format!("Type mismatch at index {}: expected Arc<{}>",
                                $idx, std::any::type_name::<$T>()))?,
                    )+
                );

                // Clone inner values from Arc (cheap for small types, necessary for ownership)
                Ok(($(
                    (*arc_results.$idx).clone()
                ,)+))
            }
        }
    };
}

// Generate ExtractInput implementations for tuples of size 2-8.
// Each invocation creates an impl for a specific tuple size.
// Supporting up to 8 elements covers the vast majority of use cases.
impl_extract_tuple!(A:0, B:1);
impl_extract_tuple!(A:0, B:1, C:2);
impl_extract_tuple!(A:0, B:1, C:2, D:3);
impl_extract_tuple!(A:0, B:1, C:2, D:3, E:4);
impl_extract_tuple!(A:0, B:1, C:2, D:3, E:4, F:5);
impl_extract_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
impl_extract_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests;
