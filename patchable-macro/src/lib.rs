//! # Patchable Macro
//!
//! Procedural macros backing the `patchable` crate.
//!
//! Provided macros:
//!
//! - `#[patchable_model]`: injects `Patchable`/`Patch` derives; with the `serde`
//!   Cargo feature enabled for this macro crate it also adds `serde::Serialize`
//!   and applies `#[serde(skip)]` to fields marked `#[patchable(skip)]`.
//!
//! - `#[derive(Patchable)]`: generates the companion `<Struct>Patch` type and the
//!   `Patchable` impl; with the `impl_from` Cargo feature it also generates
//!   `From<Struct>` for the patch type.
//!
//! - `#[derive(Patch)]`: generates the `Patch` implementation and recursively
//!   patches fields annotated with `#[patchable]`.
//!
//! Feature flags are evaluated in the `patchable-macro` crate itself. See `context`
//! for details about the generated patch struct and trait implementations.

use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, ItemStruct, parse_macro_input, parse_quote};

mod context;

use syn::DeriveInput;

use crate::context::{IS_SERDE_ENABLED, has_patchable_skip_attr, use_site_crate_path};

const IS_IMPL_FROM_ENABLED: bool = cfg!(feature = "impl_from");

#[proc_macro_attribute]
/// Attribute macro that augments a struct with Patchable/Patch derives.
///
/// - Always adds `#[derive(Patchable, Patch)]`.
/// - When the `serde` feature is enabled for the macro crate, it also adds
///   `#[derive(serde::Serialize)]`.
/// - For fields annotated with `#[patchable(skip)]`, it injects `#[serde(skip)]`
///   to keep serde output aligned with patching behavior.
///
/// This macro preserves the original struct shape and only mutates attributes.
pub fn patchable_model(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);
    let crate_root = use_site_crate_path();

    let derives = if IS_SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_root::Patchable, #crate_root::Patch, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_root::Patchable, #crate_root::Patch)]
        }
    };
    input.attrs.push(derives);

    if IS_SERDE_ENABLED {
        add_serde_skip_attrs(&mut input.fields);
    }

    (quote! { #input }).into()
}

#[proc_macro_derive(Patchable, attributes(patchable))]
/// Derive macro that generates the companion `Patch` type and `Patchable` impl.
///
/// The generated patch type:
/// - mirrors the original struct shape (named/tuple/unit),
/// - includes fields unless marked with `#[patchable(skip)]`,
/// - also derives `serde::Deserialize` when the `serde` feature is enabled for the
///   macro crate.
///
/// The `Patchable` impl sets `type Patch = <StructName>Patch<...>` and adds
/// any required generic bounds.
///
/// When the `impl_from` feature is enabled for the macro crate, a
/// `From<Struct>` implementation is also generated for the patch type.
pub fn derive_patchable(input: TokenStream) -> TokenStream {
    expand(input, |ctx| {
        let patch_struct_def = ctx.build_patch_struct();
        let patchable_trait_impl = ctx.build_patchable_trait_impl();
        let from_struct_impl = IS_IMPL_FROM_ENABLED.then(|| {
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
/// Derive macro that generates the `Patch` trait implementation.
///
/// The generated `patch` method:
/// - assigns fields directly by default,
/// - recursively calls `patch` on fields marked with `#[patchable]`,
/// - respects `#[patchable(skip)]` by omitting those fields from patching.
pub fn derive_patch(input: TokenStream) -> TokenStream {
    expand(input, |ctx| {
        let patch_trait_impl = ctx.build_patch_trait_impl();

        quote! {
            const _: () = {
                #[automatically_derived]
                #patch_trait_impl
            };
        }
    })
}

fn expand<F>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(&context::MacroContext) -> TokenStream2,
{
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => f(&ctx).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn add_serde_skip_attrs(fields: &mut Fields) {
    for field in fields.iter_mut() {
        if has_patchable_skip_attr(field) {
            field.attrs.push(parse_quote! { #[serde(skip)] });
        }
    }
}
