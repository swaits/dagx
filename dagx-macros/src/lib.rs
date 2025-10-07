//! Procedural macros for dagx
//!
//! This crate provides the `#[task]` attribute macro that automatically implements
//! the `Task` trait by deriving Input and Output types from the `run()` method signature.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ImplItem, ItemImpl, Pat, PatType, ReturnType, Type};

/// Attribute macro to automatically implement the `Task` trait.
///
/// Apply this to an `impl` block containing a `run()` method (sync or async). The macro:
/// - Derives `Input` and `Output` types from the `run()` signature
/// - Automatically implements the `Task` trait
/// - Supports both sync and async run methods
/// - Supports stateless (no self) and stateful (&self, &mut self) tasks
/// - Handles various input patterns (no inputs, single input, multiple inputs)
///
/// # Task Patterns
///
/// The `#[task]` macro supports three patterns based on state requirements:
///
/// ## 1. Stateless Tasks (No State)
///
/// Unit structs for pure computations. Use **no `self` parameter**:
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct Add;
///
/// #[task]
/// impl Add {
///     async fn run(a: &i32, b: &i32) -> i32 {
///         a + b  // Pure function, no state
///     }
/// }
/// ```
///
/// ## 2. Read-Only State Tasks
///
/// Tasks that read configuration or constant data. Use **`&self`**:
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct Multiplier {
///     factor: i32,
/// }
///
/// #[task]
/// impl Multiplier {
///     async fn run(&self, input: &i32) -> i32 {
///         input * self.factor  // Read-only access
///     }
/// }
/// ```
///
/// ## 3. Mutable State Tasks
///
/// Tasks that accumulate or modify state. Use **`&mut self`**:
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct Counter {
///     count: i32,
/// }
///
/// #[task]
/// impl Counter {
///     async fn run(&mut self, increment: &i32) -> i32 {
///         self.count += increment;  // Modifies state
///         self.count
///     }
/// }
/// ```
///
/// # Input Patterns
///
/// ## No Inputs (Source Tasks)
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct LoadData {
///     value: i32,
/// }
///
/// #[task]
/// impl LoadData {
///     async fn run(&mut self) -> i32 {
///         self.value
///     }
/// }
/// ```ignore
///
/// ## Single Input
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct Double;
///
/// #[task]
/// impl Double {
///     async fn run(&mut self, input: &i32) -> i32 {
///         input * 2
///     }
/// }
/// ```ignore
///
/// ## Multiple Inputs (up to 8)
///
/// ```ignore
/// use dagx::{task, Task};
///
/// struct Combine;
///
/// #[task]
/// impl Combine {
///     async fn run(&mut self, a: &i32, b: &String, c: &bool) -> String {
///         format!("{}: {} ({})", b, a, c)
///     }
/// }
/// ```ignore
///
/// # Requirements
///
/// - The impl block must contain exactly one `async fn run()` method
/// - The `run()` method can be stateless (no self parameter) or stateful (`&mut self`)
/// - All input parameters must be references (e.g., `&i32`, not `i32`)
/// - The macro requires `Task` to be in scope: `use dagx::Task;`
/// - For stateless tasks, the struct must implement `Default` (e.g., unit structs)
///
/// # Generated Code
///
/// The macro transforms your implementation into a full `Task` trait implementation.
///
/// For stateless tasks (no self parameter):
///
/// ```ignore
/// // Your code:
/// #[task]
/// impl Add {
///     async fn run(a: &i32, b: &i32) -> i32 {
///         a + b
///     }
/// }
///
/// // Generated:
/// impl Task for Add {
///     type Input = (i32, i32);
///     type Output = i32;
///
///     async fn run(&mut self, input: Self::Input) -> Self::Output {
///         let (a, b) = input;
///         Self::run_impl(&a, &b).await
///     }
/// }
///
/// impl Add {
///     #[inline]
///     async fn run_impl(a: &i32, b: &i32) -> i32 {
///         a + b
///     }
/// }
/// ```ignore
///
/// For stateful tasks (with &mut self):
///
/// ```ignore
/// // Your code:
/// #[task]
/// impl Counter {
///     async fn run(&mut self, inc: &i32) -> i32 {
///         self.count += inc;
///         self.count
///     }
/// }
///
/// // Generated:
/// impl Task for Counter {
///     type Input = i32;
///     type Output = i32;
///
///     async fn run(&mut self, input: Self::Input) -> Self::Output {
///         let inc = input;
///         self.run_impl(&inc).await
///     }
/// }
///
/// impl Counter {
///     async fn run_impl(&mut self, inc: &i32) -> i32 {
///         self.count += inc;
///         self.count
///     }
/// }
/// ```ignore
#[proc_macro_attribute]
pub fn task(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);

    // Extract the struct name
    let struct_name = &impl_block.self_ty;

    // Find the run() method
    let run_method = match impl_block.items.iter().find_map(|item| {
        if let ImplItem::Fn(method) = item {
            if method.sig.ident == "run" {
                return Some(method);
            }
        }
        None
    }) {
        Some(method) => method,
        None => {
            return syn::Error::new_spanned(
                &impl_block,
                "impl block must contain a run() method\n\n\
                 Expected signature: fn run(&mut self, ...) -> OutputType\n\
                              or: async fn run(&mut self, ...) -> OutputType\n\
                 The #[task] macro requires a run() method to implement the Task trait.",
            )
            .to_compile_error()
            .into();
        }
    };

    // Check if the method is async or sync
    let is_async = run_method.sig.asyncness.is_some();

    // Extract parameters (excluding self)
    let params_result: Result<Vec<_>, _> = run_method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
                // Extract the parameter name
                let param_name = if let Pat::Ident(pat_ident) = &**pat {
                    &pat_ident.ident
                } else {
                    return Some(Err(syn::Error::new_spanned(
                        pat,
                        "Unsupported parameter pattern\n\n\
                         Parameters must be simple identifiers like 'input: &T' or 'a: &i32'.",
                    )));
                };

                // Extract the inner type from &Type
                let inner_type = if let Type::Reference(type_ref) = &**ty {
                    &type_ref.elem
                } else {
                    return Some(Err(syn::Error::new_spanned(
                        ty,
                        "All parameters must be references (&T)\n\n\
                         Task inputs must be references to allow sharing data between tasks.\n\
                         Change this parameter from 'T' to '&T'.",
                    )));
                };

                Some(Ok((param_name.clone(), inner_type.clone())))
            } else {
                None // Skip self parameter
            }
        })
        .collect();

    let params = match params_result {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into(),
    };

    // Extract return type
    let output_type = match &run_method.sig.output {
        ReturnType::Default => {
            return syn::Error::new_spanned(
                &run_method.sig,
                "run() method must have an explicit return type\n\n\
                 Specify the output type: async fn run(...) -> OutputType\n\
                 For tasks that don't return a value, use '-> ()'.",
            )
            .to_compile_error()
            .into();
        }
        ReturnType::Type(_, ty) => ty.clone(),
    };

    // Build Input type based on parameter count
    let input_type = match params.len() {
        0 => quote! { () },
        1 => {
            let (_name, ty) = &params[0];
            quote! { #ty }
        }
        _ => {
            let types: Vec<_> = params.iter().map(|(_, ty)| ty).collect();
            quote! { ( #(#types),* ) }
        }
    };

    // Generate parameter destructuring for the wrapper run() method
    let (param_destructure, param_refs) = if params.is_empty() {
        (quote! { _ }, quote! {})
    } else if params.len() == 1 {
        let (name, _) = &params[0];
        (quote! { #name }, quote! { &#name })
    } else {
        let names: Vec<_> = params.iter().map(|(name, _)| name).collect();
        let refs: Vec<_> = params.iter().map(|(name, _)| quote! { &#name }).collect();
        (quote! { ( #(#names),* ) }, quote! { #(#refs),* })
    };

    // Clone the run method and rename it to run_impl
    let mut run_impl_method = run_method.clone();
    run_impl_method.sig.ident = syn::Ident::new("run_impl", run_method.sig.ident.span());

    // Check if the method has a self receiver
    let has_self_receiver = run_method
        .sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, FnArg::Receiver(_)));

    // Generate the Task trait implementation based on whether we have self and async/sync
    let expanded = if has_self_receiver {
        // Stateful task - consumes self but delegates to a method that borrows
        if is_async {
            // Async with self
            quote! {
                impl Task for #struct_name {
                    type Input = #input_type;
                    type Output = #output_type;

                    async fn run(mut self, input: Self::Input) -> Self::Output {
                        let #param_destructure = input;
                        self.run_impl(#param_refs).await
                    }
                }

                impl #struct_name {
                    #run_impl_method
                }
            }
        } else {
            // Sync with self - wrap in async block
            quote! {
                impl Task for #struct_name {
                    type Input = #input_type;
                    type Output = #output_type;

                    async fn run(mut self, input: Self::Input) -> Self::Output {
                        let #param_destructure = input;
                        self.run_impl(#param_refs)
                    }
                }

                impl #struct_name {
                    #run_impl_method
                }
            }
        }
    } else {
        // Stateless task
        if is_async {
            // Async stateless
            quote! {
                impl Task for #struct_name {
                    type Input = #input_type;
                    type Output = #output_type;

                    async fn run(self, input: Self::Input) -> Self::Output {
                        let #param_destructure = input;
                        Self::run_impl(#param_refs).await
                    }
                }

                impl #struct_name {
                    #[inline]
                    #run_impl_method
                }
            }
        } else {
            // Sync stateless
            quote! {
                impl Task for #struct_name {
                    type Input = #input_type;
                    type Output = #output_type;

                    async fn run(self, input: Self::Input) -> Self::Output {
                        let #param_destructure = input;
                        Self::run_impl(#param_refs)
                    }
                }

                impl #struct_name {
                    #[inline]
                    #run_impl_method
                }
            }
        }
    };

    TokenStream::from(expanded)
}
