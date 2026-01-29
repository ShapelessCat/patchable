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
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{
    Attribute, Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Generics, Ident, Index,
    Meta, PathArguments, Token, Type,
};

#[derive(Clone, Copy, PartialEq, Eq)]
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
        let mut field_actions = vec![];

        let stateful_fields = fields.iter().filter(|f| !has_serde_skip_attr(f));

        for (index, field) in stateful_fields.enumerate() {
            let member = if let Some(field_name) = field.ident.as_ref() {
                FieldMember::Named(field_name.clone())
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
    // #[derive(::core::clone::Clone, ::serde::Deserialize)]
    // struct InputTypePatch<T, ...> ...
    // ============================================================

    pub(crate) fn build_patch_struct(&self) -> TokenStream2 {
        let generic_params = self.build_patch_type_generics();

        let mut bounded_types = Vec::new();
        let patchable_trait = &self.patchable_trait;
        for param in self.generics.type_params() {
            if let Some(TypeUsage::Patchable) = self.preserved_types.get(&param.ident) {
                bounded_types.push(quote! { #param: #patchable_trait });
            }
        }
        let where_clause = if bounded_types.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounded_types),* }
        };

        let patch_fields = self.generate_patch_fields();
        let body = self.select_fields(
            quote! { #generic_params #where_clause { #(#patch_fields),* } },
            quote! { #generic_params #where_clause ( #(#patch_fields),* ); },
            quote! {;},
        );
        let patch_name = &self.patch_struct_name;
        quote! {
            #[derive(::core::clone::Clone, ::core::cmp::PartialEq, ::serde::Deserialize)]
            pub struct #patch_name #body
        }
    }

    // ============================================================
    // impl<T, ...> Patchable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patchable_trait_impl(&self) -> TokenStream2 {
        let patchable_trait = &self.patchable_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_bounded_types(patchable_trait);
        let assoc_type_decl = self.build_associate_type_declaration();

        let input_struct_name = self.struct_name;

        quote! {
            impl #impl_generics #patchable_trait
                for #input_struct_name #type_generics
            #where_clause {
                #assoc_type_decl
            }
        }
    }

    // ======================================================================
    // impl<T, ...> From<OriginalStruct<T, ...>> for OriginalStructPatch<...>
    // ======================================================================

    pub(crate) fn build_from_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let patch_type_generics = self.build_patch_type_generics();
        let where_clause = self.build_from_where_clause();

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

    // ============================================================
    // impl<T, ...> Patch for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patch_trait_impl(&self) -> TokenStream2 {
        let patch_trait = &self.patch_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_bounded_types(patch_trait);

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

    fn generate_patch_fields(&self) -> Vec<TokenStream2> {
        self.field_actions
            .iter()
            .map(|action| match action {
                FieldAction::Keep {
                    member: FieldMember::Named(name),
                    ty,
                } => {
                    quote! { #name : #ty }
                }
                FieldAction::Keep {
                    member: FieldMember::Unnamed(_),
                    ty,
                } => {
                    quote! { #ty }
                }
                FieldAction::Patch {
                    member: FieldMember::Named(name),
                    ty,
                } => {
                    quote! { #name : #ty :: Patch }
                }
                FieldAction::Patch {
                    member: FieldMember::Unnamed(_),
                    ty,
                } => {
                    quote! { #ty :: Patch }
                }
            })
            .collect()
    }

    fn generate_patch_method_body(&self) -> TokenStream2 {
        if self.field_actions.is_empty() {
            return quote! {};
        }

        let statements = self.field_actions.iter().map(|action| match action {
            FieldAction::Keep { member, .. } => {
                quote! {
                    self.#member = patch.#member;
                }
            }
            FieldAction::Patch { member, .. } => {
                quote! {
                    self.#member.patch(patch.#member);
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

            self.select_fields(quote! { #member: #expr }, quote! { #expr }, quote! {})
        });

        let body = quote! { #(#field_expressions),* };
        let body_ref = &body;

        self.select_fields(
            quote! { Self { #body_ref } },
            quote! { Self(#body_ref) },
            quote! { Self },
        )
    }

    fn collect_patch_generics(&self) -> Vec<Ident> {
        let mut generics = Vec::new();
        for param in self.generics.type_params() {
            if self.preserved_types.contains_key(&param.ident) {
                generics.push(param.ident.clone());
            }
        }
        generics
    }

    fn build_bounded_types(&self, bound: &TokenStream2) -> TokenStream2 {
        let mut bounded_types = Vec::new();
        for param in self.generics.type_params() {
            let t = &param.ident;
            match self.preserved_types.get(t) {
                Some(TypeUsage::Patchable) => {
                    bounded_types.push(quote! { #t: #bound + ::core::clone::Clone });
                }
                Some(TypeUsage::NotPatchable) => {
                    bounded_types.push(quote! { #t: ::core::clone::Clone });
                }
                None => {}
            }
        }

        self.extend_where_clause(bounded_types)
    }

    // ============================================================
    // type Patch = MyPatch<T, U, ...>
    // ============================================================

    fn build_associate_type_declaration(&self) -> TokenStream2 {
        let patch_type_generics = self.build_patch_type_generics();
        let state_name = &self.patch_struct_name;
        quote! {
            type Patch = #state_name #patch_type_generics;
        }
    }

    fn build_from_where_clause(&self) -> TokenStream2 {
        let patchable_trait = &self.patchable_trait;
        let mut bounded_types = Vec::new();
        for param in self.generics.type_params() {
            let t = &param.ident;
            if let Some(TypeUsage::Patchable) = self.preserved_types.get(t) {
                bounded_types.push(quote! { #t: #patchable_trait });
                bounded_types.push(quote! {
                    <#t as #patchable_trait>::Patch: ::core::convert::From<#t>
                });
            }
        }

        self.extend_where_clause(bounded_types)
    }

    fn build_patch_type_generics(&self) -> TokenStream2 {
        let patch_generic_params = self.collect_patch_generics();
        // Empty `<>` is legal in Rust, and adding or dropping the `<>` doesn't affect the
        // definition. For example, `struct A<>(i32)` and `struct A(i32)` have the
        // same HIR.
        quote! { <#(#patch_generic_params),*> }
    }

    fn extend_where_clause(&self, bounds: Vec<TokenStream2>) -> TokenStream2 {
        if let Some(where_clause) = &self.generics.where_clause {
            if bounds.is_empty() {
                return quote! { #where_clause };
            }

            let normalized_input_where_clause = if where_clause.predicates.empty_or_trailing() {
                quote! { #where_clause }
            } else {
                quote! { #where_clause, }
            };
            quote! {
                #normalized_input_where_clause
                #(#bounds),*
            }
        } else if !bounds.is_empty() {
            quote! {
                where #(#bounds),*
            }
        } else {
            quote! {}
        }
    }

    fn select_fields(
        &self,
        named: TokenStream2,
        unnamed: TokenStream2,
        unit: TokenStream2,
    ) -> TokenStream2 {
        match &self.fields {
            Fields::Named(_) => named,
            Fields::Unnamed(_) => unnamed,
            Fields::Unit => unit,
        }
    }
}

enum FieldMember {
    Named(Ident),
    Unnamed(Index),
}

impl ToTokens for FieldMember {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            FieldMember::Named(ident) => ident.to_tokens(tokens),
            FieldMember::Unnamed(index) => index.to_tokens(tokens),
        }
    }
}

enum FieldAction<'a> {
    Keep { member: FieldMember, ty: &'a Type },
    Patch { member: FieldMember, ty: &'a Type },
}

fn use_site_crate_path() -> TokenStream2 {
    let found_crate =
        crate_name("patchable").expect("patchable library should be present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => quote! { crate },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( ::#ident )
        }
    }
}

fn has_patchable_attr(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("patchable"))
}

fn has_serde_skip_attr(field: &Field) -> bool {
    #[inline]
    fn is_serde(attr: &Attribute) -> bool {
        attr.path().is_ident("serde")
    }

    #[inline]
    fn need_skip(metas: Punctuated<Meta, Token![,]>) -> bool {
        metas.iter().any(|e| {
            matches!(
                e,
                Meta::Path(p) if p.is_ident("skip") || p.is_ident("skip_serializing")
            )
        })
    }

    field.attrs.iter().any(|attr| {
        is_serde(attr)
            && attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(need_skip)
    })
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
