//! # Patchable Macro
//!
//! Procedural macros for the `patchable` crate: `Patchable` and `Patch` derives.
//!
//! See `context` for details on the generated patch struct and trait implementations.

use proc_macro::TokenStream;

use quote::quote;
use syn::{self, DeriveInput};

mod context;

#[proc_macro_derive(Patchable, attributes(patchable))]
pub fn derive_patchable(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => {
            let patch_struct_def = ctx.build_patch_struct();
            let patchable_trait_impl = ctx.build_patchable_trait_impl();

            quote! {
                const _: () = {
                    #[automatically_derived]
                    #patch_struct_def
                    #[automatically_derived]
                    #patchable_trait_impl
                };
            }
            .into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Patch, attributes(patchable))]
pub fn derive_patch(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => {
            let patch_trait_impl = ctx.build_patch_trait_impl();

            quote! {
                const _: () = {
                    #[automatically_derived]
                    #patch_trait_impl
                };
            }
            .into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}
