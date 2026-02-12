//! Input extraction for tasks from a list of dependencies.

use std::{collections::HashMap, hash::Hash, sync::Arc};

use crate::TaskInput;

/// Helper trait for input extraction from list of dependencies.
pub trait ExtractInput: Sized + Clone {
    type Input: Send;
    type Retv<'input>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, Self::Input>,
    ) -> Result<Self::Retv<'input>, String>;
}

// Unit type - no dependencies
impl ExtractInput for () {
    type Input = ();
    type Retv<'input> = ();

    fn extract_from_task_input(_input: TaskInput<'_, Self>) -> Result<Self, String> {
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
                type Input = ($t,);
                type Retv<'input> = &'input $t;
                fn extract_from_task_input<'input>(input: TaskInput<'input, Self::Input>) -> Result<Self::Retv<'input>, String> {
                    Ok(input.next().0)
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
    type Input = (Self,);
    type Retv<'input> = &'input Result<T, E>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, (Self,)>,
    ) -> Result<Self::Retv<'input>, String> {
        Ok(input.next().0)
    }
}

// Implement ExtractInput for Option types
impl<T> ExtractInput for Option<T>
where
    T: 'static + Clone + Send + Sync,
{
    type Input = (Self,);
    type Retv<'input> = &'input Option<T>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, (Self,)>,
    ) -> Result<Self::Retv<'input>, String> {
        Ok(input.next().0)
    }
}

// Implement ExtractInput for Vec types
impl<T> ExtractInput for Vec<T>
where
    T: 'static + Clone + Send + Sync,
{
    type Input = (Self,);
    type Retv<'input> = &'input Vec<T>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, (Self,)>,
    ) -> Result<Self::Retv<'input>, String> {
        Ok(input.next().0)
    }
}

// Implement ExtractInput for HashMap types
impl<K, V> ExtractInput for HashMap<K, V>
where
    K: 'static + Clone + Send + Sync + Eq + Hash,
    V: 'static + Clone + Send + Sync,
{
    type Input = (Self,);
    type Retv<'input> = &'input HashMap<K, V>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, (Self,)>,
    ) -> Result<Self::Retv<'input>, String> {
        Ok(input.next().0)
    }
}

impl<T> ExtractInput for Arc<T>
where
    T: 'static + Clone + Send + Sync,
{
    type Input = (Self,);
    type Retv<'input> = &'input Arc<T>;

    fn extract_from_task_input<'input>(
        input: TaskInput<'input, (Self,)>,
    ) -> Result<Self::Retv<'input>, String> {
        Ok(input.next().0)
    }
}

macro_rules! impl_extract_tuple {
    ($($T:ident),+) => {
        impl<$($T: 'static + Clone + Send + Sync),+> ExtractInput for ($($T,)+) {
            type Input = Self;
           type Retv<'input> = ($(&'input $T,)+);

            fn extract_from_task_input<'input>(input: TaskInput<'input, Self::Input>) -> Result<Self::Retv<'input>, String> {
                $(
                    let ($T, input): (&'input $T, _) = input.next();
                )+

                let _input: TaskInput<'input, ()> = input;

                Ok(($($T,)+))
            }
        }
    };
}

// Generate ExtractInput implementations for tuples of size 2-8.
// Each invocation creates an impl for a specific tuple size.
// Supporting up to 8 elements covers the vast majority of use cases.
#[allow(non_snake_case)]
mod impls {
    use super::*;

    impl_extract_tuple!(A, B);
    impl_extract_tuple!(A, B, C);
    impl_extract_tuple!(A, B, C, D);
    impl_extract_tuple!(A, B, C, D, E);
    impl_extract_tuple!(A, B, C, D, E, F);
    impl_extract_tuple!(A, B, C, D, E, F, G);
    impl_extract_tuple!(A, B, C, D, E, F, G, H);
}
