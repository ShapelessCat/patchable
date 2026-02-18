//! # Macro Context
//!
//! [`MacroContext::new`] parses the derive input and normalizes it into a
//! [`MacroContext`] that drives code generation.
//!
//! The context records field actions, preserved generics, and crate paths so the
//! macro can emit the companion patch struct plus the `Patchable` and `Patch`
//! trait implementations.

mod from_impl;
mod patch_impl;
mod patch_struct;
mod patchable_impl;

use std::collections::HashMap;

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::visit::Visit;
use syn::{
    Attribute, Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Generics, Ident, Index,
    PathArguments, Type,
};

pub const IS_SERDE_ENABLED: bool = cfg!(feature = "serde");

const PATCHABLE: &str = "patchable";

#[derive(Debug)]
enum TypeUsage {
    NotPatchable,
    Patchable,
}

pub(crate) struct MacroContext<'a> {
    /// The name of the struct on which the derive macro is applied.
    struct_name: &'a Ident,
    /// The generics definition of the target struct.
    generics: &'a Generics,
    /// The fields of the target struct.
    fields: &'a Fields,
    /// Mapping from preserved type to its usage flag.
    preserved_types: HashMap<&'a Ident, TypeUsage>,
    /// The list of actions to perform for each field when generating the `patch` method and the
    /// patch struct.
    ///
    /// This determines whether a field is copied directly (`Keep`) or recursively patched
    /// (`Patch`).
    field_actions: Vec<FieldAction<'a>>,
    /// The name of the generated companion patch struct (e.g., `MyStructPatch`).
    patch_struct_name: Ident,
    /// Fully qualified path to the `Patchable` trait.
    patchable_trait: TokenStream2,
    /// Fully qualified path to the `Patch` trait.
    patch_trait: TokenStream2,
}

impl<'a> MacroContext<'a> {
    pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        let Data::Struct(DataStruct { fields, .. }) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "This derive macro can only be applied to structs",
            ));
        };

        if input
            .generics
            .params
            .iter()
            .any(|g| matches!(g, GenericParam::Lifetime(_)))
        {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "Patch derives do not support borrowed fields",
            ));
        }

        let mut preserved_types: HashMap<&Ident, TypeUsage> = HashMap::new();
        let mut field_actions = Vec::new();

        for (index, field) in fields.iter().enumerate() {
            if has_patchable_skip_attr(field) {
                continue;
            }

            let member = if let Some(field_name) = field.ident.as_ref() {
                FieldMember::Named(field_name)
            } else {
                FieldMember::Unnamed(Index::from(index))
            };

            let field_type = &field.ty;

            if has_patchable_attr(field) {
                let Some(type_name) = get_abstract_simple_type_name(field_type) else {
                    return Err(syn::Error::new_spanned(
                        field_type,
                        "Only a simple generic type is supported here", // TODO: remove this limit
                    ));
                };
                // `Patchable` usage overrides `NotPatchable` usage.
                preserved_types.insert(type_name, TypeUsage::Patchable);

                field_actions.push(FieldAction::Patch {
                    member,
                    ty: field_type,
                });
            } else {
                for type_name in collect_used_simple_types(field_type) {
                    // Only mark as `NotPatchable` if not already marked as `Patchable`.
                    preserved_types
                        .entry(type_name)
                        .or_insert(TypeUsage::NotPatchable);
                }
                field_actions.push(FieldAction::Keep {
                    member,
                    ty: field_type,
                });
            };
        }

        let crate_path = use_site_crate_path();

        Ok(MacroContext {
            struct_name: &input.ident,
            generics: &input.generics,
            fields,
            preserved_types,
            field_actions,
            patch_struct_name: quote::format_ident!("{}Patch", &input.ident),
            patchable_trait: quote! { #crate_path :: Patchable },
            patch_trait: quote! { #crate_path :: Patch },
        })
    }
}

enum FieldMember<'a> {
    Named(&'a Ident),
    Unnamed(Index),
}

impl<'a> ToTokens for FieldMember<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            FieldMember::Named(ident) => ident.to_tokens(tokens),
            FieldMember::Unnamed(index) => index.to_tokens(tokens),
        }
    }
}

enum FieldAction<'a> {
    Keep {
        member: FieldMember<'a>,
        ty: &'a Type,
    },
    Patch {
        member: FieldMember<'a>,
        ty: &'a Type,
    },
}

impl<'a> FieldAction<'a> {
    fn member(&self) -> &FieldMember<'a> {
        match self {
            FieldAction::Keep { member, .. } | FieldAction::Patch { member, .. } => member,
        }
    }

    fn build_initializer_expr(&self) -> TokenStream2 {
        let member = self.member();
        match self {
            FieldAction::Keep { .. } => quote! { value.#member },
            FieldAction::Patch { .. } => quote! { ::core::convert::From::from(value.#member) },
        }
    }
}

pub fn use_site_crate_path() -> TokenStream2 {
    let found_crate =
        crate_name(PATCHABLE).expect("patchable library should be present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => quote! { crate },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( ::#ident )
        }
    }
}

#[inline]
fn is_patchable_attr(attr: &Attribute) -> bool {
    attr.path().is_ident(PATCHABLE)
}

fn patchable_attr_has_param(attr: &Attribute, param: &str) -> bool {
    is_patchable_attr(attr)
        && attr
            .parse_nested_meta(|meta| {
                if meta.path.is_ident(param) {
                    Ok(())
                } else {
                    Err(meta.error("unrecognized `patchable` parameter"))
                }
            })
            .is_ok()
}

fn has_patchable_attr(field: &Field) -> bool {
    field.attrs.iter().any(is_patchable_attr)
}

pub fn has_patchable_skip_attr(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| patchable_attr_has_param(attr, "skip"))
}

struct SimpleTypeCollector<'a> {
    used_simple_types: Vec<&'a Ident>,
}

impl<'ast> Visit<'ast> for SimpleTypeCollector<'ast> {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none()
            && let Some(segment) = node.path.segments.first()
        {
            self.used_simple_types.push(&segment.ident);
        }
        syn::visit::visit_type_path(self, node);
    }
}

fn collect_used_simple_types(ty: &Type) -> Vec<&Ident> {
    let mut collector = SimpleTypeCollector {
        used_simple_types: Vec::new(),
    };
    collector.visit_type(ty);
    collector.used_simple_types
}

fn get_abstract_simple_type_name(t: &Type) -> Option<&Ident> {
    match t {
        Type::Path(tp) if !tp.path.segments.is_empty() => {
            let last_segment = tp.path.segments.last()?;
            // Ensure the path segment has no arguments (e.g., it's not `Vec<T>` or `Option<T>`).
            if matches!(last_segment.arguments, PathArguments::None) {
                Some(&last_segment.ident)
            } else {
                None
            }
        }
        _ => None,
    }
}
