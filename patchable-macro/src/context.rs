//! # Macro Context
//!
//! [`MacroContext::new`] parses the derive input and normalizes it into a
//! [`MacroContext`] that drives code generation.
//!
//! The context records field actions, preserved generics, and crate paths so the
//! macro can emit the companion patch struct plus the `Patchable` and `Patch`
//! trait implementations.

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

    // ============================================================
    // #[derive(::serde::Deserialize)]
    // struct InputTypePatch<T, ...> ...
    // ============================================================

    pub(crate) fn build_patch_struct(&self) -> TokenStream2 {
        let generic_params = self.build_patch_type_generics();
        let where_clause = self.build_where_clause_with_bound(&self.patchable_trait);
        let patch_fields = self.generate_patch_fields();
        let body = match &self.fields {
            Fields::Named(_) => quote! { #generic_params #where_clause { #(#patch_fields),* } },
            Fields::Unnamed(_) => quote! { #generic_params ( #(#patch_fields),* ) #where_clause; },
            Fields::Unit => quote! {;},
        };
        let patch_name = &self.patch_struct_name;
        let derive_attr = if IS_SERDE_ENABLED {
            quote! { #[derive(::core::fmt::Debug, ::serde::Deserialize)] }
        } else {
            quote! { #[derive(::core::fmt::Debug)] }
        };

        quote! {
            #derive_attr
            pub struct #patch_name #body
        }
    }

    // ============================================================
    // impl<T, ...> Patchable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patchable_trait_impl(&self) -> TokenStream2 {
        let patchable_trait = &self.patchable_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_where_clause_with_bound(patchable_trait);
        let assoc_type_decl = self.build_associated_type_declaration();

        let input_struct_name = self.struct_name;

        quote! {
            impl #impl_generics #patchable_trait
                for #input_struct_name #type_generics
            #where_clause {
                #assoc_type_decl
            }
        }
    }

    // ============================================================
    // impl<T, ...> Patch for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patch_trait_impl(&self) -> TokenStream2 {
        let patch_trait = &self.patch_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_where_clause_with_bound(patch_trait);

        let input_struct_name = self.struct_name;

        let patch_param_name = if self.field_actions.is_empty() {
            quote! { _patch }
        } else {
            quote! { patch }
        };

        let patch_method_body = self.generate_patch_method_body();
        quote! {
            impl #impl_generics #patch_trait
                for #input_struct_name #type_generics
            #where_clause {
                #[inline(always)]
                fn patch(&mut self, #patch_param_name: Self::Patch) {
                    #patch_method_body
                }
            }
        }
    }

    // ======================================================================
    // impl<T, ...> From<OriginalStruct<T, ...>> for OriginalStructPatch<...>
    // ======================================================================

    pub(crate) fn build_from_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let patch_type_generics = self.build_patch_type_generics();
        let where_clause = self.build_where_clause_for_from_impl();

        let input_struct_name = self.struct_name;
        let patch_struct_name = &self.patch_struct_name;
        let from_body = self.generate_from_body();

        quote! {
            impl #impl_generics ::core::convert::From<#input_struct_name #type_generics>
                for #patch_struct_name #patch_type_generics
            #where_clause {
                #[inline(always)]
                fn from(value: #input_struct_name #type_generics) -> Self {
                    #from_body
                }
            }
        }
    }

    fn generate_patch_fields(&self) -> Vec<TokenStream2> {
        let patchable_trait = &self.patchable_trait;
        self.field_actions
            .iter()
            .map(|action| match action {
                FieldAction::Keep { member, ty } => match member {
                    FieldMember::Named(name) => quote! { #name : #ty },
                    FieldMember::Unnamed(_) => quote! { #ty },
                },
                FieldAction::Patch { member, ty } => {
                    let field = match member {
                        FieldMember::Named(name) => quote! { #name : <#ty as #patchable_trait>::Patch },
                        FieldMember::Unnamed(_) => quote! { <#ty as #patchable_trait>::Patch },
                    };
                    if IS_SERDE_ENABLED {
                        let bound = quote! { <#ty as #patchable_trait>::Patch: ::serde::de::DeserializeOwned };
                        let bound_string = bound.to_string();
                        let bound_lit = syn::LitStr::new(&bound_string, Span::call_site());
                        quote! {
                            #[serde(bound(deserialize = #bound_lit))]
                            #field
                        }
                    } else {
                        quote! { #field }
                    }
                }
            })
            .collect()
    }

    fn generate_patch_method_body(&self) -> TokenStream2 {
        if self.field_actions.is_empty() {
            return quote! {};
        }

        let statements = self
            .field_actions
            .iter()
            .enumerate()
            .map(|(patch_index, action)| match action {
                FieldAction::Keep { member, .. } => {
                    let patch_member = patch_member(member, patch_index);
                    quote! {
                        self.#member = patch.#patch_member;
                    }
                }
                FieldAction::Patch { member, .. } => {
                    let patch_member = patch_member(member, patch_index);
                    quote! {
                        self.#member.patch(patch.#patch_member);
                    }
                }
            });

        quote! {
            #(#statements)*
        }
    }

    fn generate_from_body(&self) -> TokenStream2 {
        let field_expressions = self.field_actions.iter().map(|action| {
            let (member, expr) = match action {
                FieldAction::Keep { member, .. } => (member, quote! { value.#member }),
                FieldAction::Patch { member, .. } => (
                    member,
                    quote! { ::core::convert::From::from(value.#member) },
                ),
            };

            match &self.fields {
                Fields::Named(_) => quote! { #member: #expr },
                Fields::Unnamed(_) => quote! { #expr },
                Fields::Unit => quote! {},
            }
        });

        let body = quote! { #(#field_expressions),* };

        match &self.fields {
            Fields::Named(_) => quote! { Self { #body } },
            Fields::Unnamed(_) => quote! { Self(#body) },
            Fields::Unit => quote! { Self },
        }
    }

    fn iter_patchable_type_params(&self) -> impl Iterator<Item = &Ident> + '_ {
        self.generics.type_params().filter_map(|param| {
            matches!(
                self.preserved_types.get(&param.ident),
                Some(TypeUsage::Patchable)
            )
            .then_some(&param.ident)
        })
    }

    fn iter_preserved_type_params(&self) -> impl Iterator<Item = &Ident> + '_ {
        self.generics.type_params().filter_map(|param| {
            self.preserved_types
                .contains_key(&param.ident)
                .then_some(&param.ident)
        })
    }

    // ============================================================
    // type Patch = MyPatch<T, U, ...>
    // ============================================================

    fn build_associated_type_declaration(&self) -> TokenStream2 {
        let patch_type_generics = self.build_patch_type_generics();
        let state_name = &self.patch_struct_name;
        quote! {
            type Patch = #state_name #patch_type_generics;
        }
    }

    fn build_patch_type_generics(&self) -> TokenStream2 {
        let patch_generic_params = self.iter_preserved_type_params();
        // Empty `<>` is legal in Rust, and adding or dropping the `<>` doesn't affect the
        // definition. For example, `struct A<>(i32)` and `struct A(i32)` have the
        // same HIR.
        quote! { <#(#patch_generic_params),*> }
    }

    // ===========================================
    // Helper functions for building where clauses
    // ===========================================

    fn build_where_clause_with_bound(&self, bound: &TokenStream2) -> TokenStream2 {
        self.build_where_clause_for_patchable_types(|ty, patchable_trait| {
            quote! {
                #ty: #bound,
                <#ty as #patchable_trait>::Patch: ::core::fmt::Debug,
            }
        })
    }

    fn build_where_clause_for_from_impl(&self) -> TokenStream2 {
        self.build_where_clause_for_patchable_types(|ty, patchable_trait| {
            quote! {
                #ty: #patchable_trait,
                <#ty as #patchable_trait>::Patch: ::core::convert::From<#ty> + ::core::fmt::Debug,
            }
        })
    }

    fn build_where_clause_for_patchable_types<F>(&self, mut build_bounds: F) -> TokenStream2
    where
        F: FnMut(&Ident, &TokenStream2) -> TokenStream2,
    {
        let patchable_trait = &self.patchable_trait;
        let bounded_types: Vec<_> = self
            .iter_patchable_type_params()
            .map(|ty| build_bounds(ty, patchable_trait))
            .collect();
        self.extend_where_clause(bounded_types)
    }

    fn extend_where_clause(&self, bounds: Vec<TokenStream2>) -> TokenStream2 {
        match (&self.generics.where_clause, bounds.is_empty()) {
            (None, true) => quote! {},
            (None, false) => quote! { where #(#bounds),* },
            (Some(where_clause), true) => quote! { #where_clause },
            (Some(where_clause), false) => {
                let sep = (!where_clause.predicates.trailing_punct()).then_some(quote! {,});
                quote! { #where_clause #sep #(#bounds),* }
            }
        }
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

fn patch_member(member: &FieldMember<'_>, patch_index: usize) -> TokenStream2 {
    match member {
        FieldMember::Named(name) => quote! { #name },
        FieldMember::Unnamed(_) => {
            let index = Index::from(patch_index);
            quote! { #index }
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
