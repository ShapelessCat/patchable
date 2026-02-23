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
mod patchable_impl;
mod utils;

use std::collections::HashMap;

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::visit::Visit;
use syn::{
    Attribute, Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Generics, Ident, Index,
    Meta, PathArguments, Type,
};

pub const IS_SERDE_ENABLED: bool = cfg!(feature = "serde");

const PATCHABLE: &str = "patchable";

#[derive(Debug)]
enum TypeUsage {
    NotPatchable,
    Patchable,
}

#[derive(Debug)]
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
    /// The generated companion patch struct type (e.g., `MyStructPatch<T, ...>`).
    patch_struct_type: TokenStream2,
    /// Fully qualified path to the `Patchable` trait.
    patchable_trait: TokenStream2,
    /// Fully qualified path to the `Patch` trait.
    patch_trait: TokenStream2,
}

impl<'a> MacroContext<'a> {
    pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        Self::validate_generics(input)?;
        let fields = Self::extract_struct_fields(input)?;
        let (preserved_types, field_actions) = Self::collect_field_actions(fields)?;
        let patch_struct_type =
            Self::build_patch_struct_type(&input.ident, &input.generics, &preserved_types);
        let (patchable_trait, patch_trait) = Self::build_trait_paths();

        Ok(Self {
            struct_name: &input.ident,
            generics: &input.generics,
            fields,
            preserved_types,
            field_actions,
            patch_struct_type,
            patchable_trait,
            patch_trait,
        })
    }

    fn validate_generics(input: &DeriveInput) -> syn::Result<()> {
        if input
            .generics
            .params
            .iter()
            .any(|g| matches!(g, GenericParam::Lifetime(_)))
        {
            Err(syn::Error::new_spanned(
                &input.generics,
                "Patch derives do not support borrowed fields",
            ))
        } else {
            Ok(())
        }
    }

    fn extract_struct_fields(input: &'a DeriveInput) -> syn::Result<&'a Fields> {
        if let Data::Struct(DataStruct { fields, .. }) = &input.data {
            Ok(fields)
        } else {
            Err(syn::Error::new_spanned(
                input,
                "This derive macro can only be applied to structs",
            ))
        }
    }

    fn collect_field_actions(
        fields: &'a Fields,
    ) -> syn::Result<(HashMap<&'a Ident, TypeUsage>, Vec<FieldAction<'a>>)> {
        let mut preserved_types = HashMap::new();
        let mut field_actions = Vec::new();

        for (index, field) in fields.iter().enumerate() {
            Self::collect_field_action(index, field, &mut preserved_types, &mut field_actions)?;
        }

        Ok((preserved_types, field_actions))
    }

    fn collect_field_action(
        index: usize,
        field: &'a Field,
        preserved_types: &mut HashMap<&'a Ident, TypeUsage>,
        field_actions: &mut Vec<FieldAction<'a>>,
    ) -> syn::Result<()> {
        Self::validate_patchable_params(field)?;
        if has_patchable_skip_attr(field) {
            return Ok(());
        }

        let member = Self::field_member(field, index);
        let field_type = &field.ty;

        if has_patchable_attr(field) {
            let type_name = Self::extract_patchable_type_name(field_type)?;
            // `Patchable` usage overrides `NotPatchable` usage.
            preserved_types.insert(type_name, TypeUsage::Patchable);
            field_actions.push(FieldAction::Patch {
                member,
                ty: field_type,
            });
        } else {
            Self::record_non_patchable_type_usage(field_type, preserved_types);
            field_actions.push(FieldAction::Keep {
                member,
                ty: field_type,
            });
        }
        Ok(())
    }

    fn validate_patchable_params(field: &Field) -> syn::Result<()> {
        for attr in field.attrs.iter().filter(|attr| is_patchable_attr(attr)) {
            match &attr.meta {
                Meta::Path(_) => {}
                Meta::List(_) => {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("skip") {
                            Ok(())
                        } else {
                            Err(meta.error("unrecognized `patchable` parameter"))
                        }
                    })?;
                }
                Meta::NameValue(_) => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "unrecognized `patchable` parameter",
                    ));
                }
            }
        }
        Ok(())
    }

    fn field_member(field: &'a Field, index: usize) -> FieldMember<'a> {
        if let Some(field_name) = field.ident.as_ref() {
            FieldMember::Named(field_name)
        } else {
            FieldMember::Unnamed(Index::from(index))
        }
    }

    fn extract_patchable_type_name(field_type: &'a Type) -> syn::Result<&'a Ident> {
        get_abstract_simple_type_name(field_type).ok_or_else(|| {
            syn::Error::new_spanned(
                field_type,
                "Only a simple generic type is supported here", // TODO: remove this limit
            )
        })
    }

    fn record_non_patchable_type_usage(
        field_type: &'a Type,
        preserved_types: &mut HashMap<&'a Ident, TypeUsage>,
    ) {
        for type_name in collect_used_simple_types(field_type) {
            // Only mark as `NotPatchable` if not already marked as `Patchable`.
            preserved_types
                .entry(type_name)
                .or_insert(TypeUsage::NotPatchable);
        }
    }

    fn build_patch_struct_type(
        struct_name: &Ident,
        generics: &Generics,
        preserved_types: &HashMap<&'a Ident, TypeUsage>,
    ) -> TokenStream2 {
        let patch_struct_name = quote::format_ident!("{}Patch", struct_name);
        let patch_generic_params = generics.type_params().filter_map(|param| {
            preserved_types
                .contains_key(&param.ident)
                .then_some(&param.ident)
        });
        quote! { #patch_struct_name <#(#patch_generic_params),*> }
    }

    fn build_trait_paths() -> (TokenStream2, TokenStream2) {
        let crate_path = use_site_crate_path();
        (
            quote! { #crate_path :: Patchable },
            quote! { #crate_path :: Patch },
        )
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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
