//! # Patchable Macro
//!
//! Procedural macros for the `patchable` crate: `Patchable` and `Patch` derives.
//!
//! See `context` for details on the generated patch struct and trait implementations.

use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{self, DeriveInput};

mod context;

#[proc_macro_derive(Patchable, attributes(patchable))]
pub fn derive_patchable(input: TokenStream) -> TokenStream {
    derive_with(input, |ctx| {
        let patch_struct_def = ctx.build_patch_struct();
        let patchable_trait_impl = ctx.build_patchable_trait_impl();
        let from_struct_impl = cfg!(feature = "impl_from").then(|| {
            let from_struct_impl = ctx.build_from_trait_impl();
            quote! {
                #[automatically_derived]
                #from_struct_impl
            }
        });

        quote! {
            const _: () = {
                #[automatically_derived]
                #patch_struct_def
                #[automatically_derived]
                #patchable_trait_impl
                #from_struct_impl
            };
        }
    })
}

#[proc_macro_derive(Patch, attributes(patchable))]
pub fn derive_patch(input: TokenStream) -> TokenStream {
    derive_with(input, |ctx| {
        let patch_trait_impl = ctx.build_patch_trait_impl();

        quote! {
            const _: () = {
                #[automatically_derived]
                #patch_trait_impl
            };
        }
    })
}

fn derive_with<F>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(&context::MacroContext) -> TokenStream2,
{
    let input: DeriveInput = syn::parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => f(&ctx).into(),
        Err(e) => e.to_compile_error().into(),
    }
}
