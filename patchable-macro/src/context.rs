//! # Macro Context
//!
//! Context and logic for the `Patchable` derive macro.
//!
//! This module contains the [`MacroContext`] struct, which analyzes the input struct and generates
//! the code for:
//! 1. The companion patch struct (state struct).
//! 2. The `Patchable` trait implementation.

use std::collections::{HashMap, HashSet};

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
struct TypeUsage {
    used_in_keep: bool,
}

pub(crate) struct MacroContext<'a> {
    /// The name of the struct on which the `#[derive(Patchable)]` macro is applied.
    struct_name: &'a Ident,
    /// The generics definition of the target struct.
    generics: &'a Generics,
    /// The fields of the target struct.
    fields: &'a Fields,
    /// Mapping from preserved type to its usage flag.
    preserved_types: HashMap<&'a Ident, TypeUsage>,
    /// Patchable field types that should implement `Patchable`.
    patchable_field_types: Vec<&'a Type>,
    /// The list of actions to perform for each field when generating the `patch` method and the
    /// state struct.
    ///
    /// This determines whether a field is copied directly (`Keep`) or recursively patched
    /// (`Patch`).
    field_actions: Vec<FieldAction<'a>>,
    /// The name of the generated companion state struct (e.g., `MyStructState`).
    state_struct_name: Ident,
    crate_path: TokenStream2,
}

impl<'a> MacroContext<'a> {
    pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        let Data::Struct(DataStruct { fields, .. }) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "This `Patchable` derive macro can only be applied on structs",
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
                "`Patchable` does not support borrowed fields",
            ));
        }

        let mut preserved_types: HashMap<&Ident, TypeUsage> = HashMap::new();
        let mut patchable_field_types = Vec::new();
        let mut field_actions = vec![];

        let stateful_fields: Vec<&Field> = fields
            .iter()
            .filter(|f| !has_serde_skip_attr(f) && !is_phantom_data(f))
            .collect();

        for (index, field) in stateful_fields.iter().enumerate() {
            let member = if let Some(field_name) = field.ident.as_ref() {
                FieldMember::Named(field_name.clone())
            } else {
                FieldMember::Unnamed(Index::from(index))
            };

            let field_type = &field.ty;

            if has_patchable_attr(field) {
                patchable_field_types.push(field_type);

                field_actions.push(FieldAction::Patch {
                    member,
                    ty: field_type,
                });
            } else {
                for type_name in collect_used_simple_types(field_type) {
                    preserved_types
                        .entry(type_name)
                        .and_modify(|usage| usage.used_in_keep = true)
                        .or_insert(TypeUsage { used_in_keep: true });
                }
                field_actions.push(FieldAction::Keep {
                    member,
                    ty: field_type,
                });
            };
        }

        Ok(MacroContext {
            struct_name: &input.ident,
            generics: &input.generics,
            fields,
            preserved_types,
            field_actions,
            state_struct_name: quote::format_ident!("{}State", &input.ident),
            crate_path: use_site_crate_path(),
            patchable_field_types,
        })
    }

    // ============================================================
    // #[derive(::core::clone::Clone, ::serde::Deserialize)]
    // struct InputTypeState<T, ...> ...
    // ============================================================

    pub(crate) fn build_state_struct(&self) -> TokenStream2 {
        let state_generic_params = self.collect_state_generics();
        // Empty `<>` is legal in Rust, and add or drop the `<>` doesn't affect the
        // definition. For example, `struct A<>(i32)` and `struct A(i32)` have the
        // same HIR.
        let generic_params = quote! { <#(#state_generic_params),*> };

        let mut bounds = Vec::new();
        let patchable_generic_params = self.collect_patchable_generic_params();
        for param in self.generics.type_params() {
            if patchable_generic_params.contains(&param.ident) {
                bounds.push(quote! { #param: ::core::clone::Clone });
            }
        }

        let crate_root = &self.crate_path;
        let mut seen_patchable_bounds = HashSet::new();
        for field_type in &self.patchable_field_types {
            let key = field_type.to_token_stream().to_string();
            if seen_patchable_bounds.insert(key) {
                bounds.push(quote! { #field_type: #crate_root :: Patchable });
                if !self.is_generic_param_type(field_type) {
                    bounds.push(quote! {
                        for<'patchable_de> <#field_type as #crate_root :: Patchable>::Patch:
                            ::serde::Deserialize<'patchable_de>
                    });
                }
            }
        }
        let where_clause = if bounds.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounds),* }
        };

        let state_fields = self.generate_state_fields();
        let body = match &self.fields {
            Fields::Named(_) => quote! { #generic_params #where_clause { #(#state_fields),* } },
            Fields::Unnamed(_) => quote! { #generic_params #where_clause ( #(#state_fields),* ); },
            Fields::Unit => quote! {;},
        };
        let state_name = &self.state_struct_name;
        quote! {
            #[derive(::core::clone::Clone, ::serde::Deserialize)]
            pub struct #state_name #body
        }
    }

    // ============================================================
    // impl<T, ...> Patchable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patchable_trait_impl(&self) -> TokenStream2 {
        let crate_root = &self.crate_path;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_bounds();
        let assoc_type_decl = self.build_associate_type_declaration();

        let input_struct_name = &self.struct_name;

        let patch_param_name = if self.field_actions.is_empty() {
            quote! { _state }
        } else {
            quote! { state }
        };

        let patch_method_body = self.generate_patch_method_body();
        quote! {
            impl #impl_generics #crate_root :: Patchable
                for #input_struct_name #type_generics
            #where_clause {
                #assoc_type_decl

                #[inline(always)]
                fn patch(&mut self, #patch_param_name: Self::Patch) {
                    #patch_method_body
                }
            }
        }
    }

    fn generate_state_fields(&self) -> Vec<TokenStream2> {
        let crate_root = &self.crate_path;
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
                    let patch_type = if self.is_generic_param_type(ty) {
                        quote! { #ty :: Patch }
                    } else {
                        quote! { <#ty as #crate_root :: Patchable>::Patch }
                    };
                    quote! { #name : #patch_type }
                }
                FieldAction::Patch {
                    member: FieldMember::Unnamed(_),
                    ty,
                } => {
                    let patch_type = if self.is_generic_param_type(ty) {
                        quote! { #ty :: Patch }
                    } else {
                        quote! { <#ty as #crate_root :: Patchable>::Patch }
                    };
                    quote! { #patch_type }
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
                    self.#member = state.#member;
                }
            }
            FieldAction::Patch { member, .. } => {
                quote! {
                    self.#member.patch(state.#member);
                }
            }
        });

        quote! {
            #(#statements)*
        }
    }

    fn collect_state_generics(&self) -> Vec<Ident> {
        let mut generics = Vec::new();
        let patchable_generic_params = self.collect_patchable_generic_params();
        for param in self.generics.type_params() {
            if self.preserved_types.contains_key(&param.ident)
                || patchable_generic_params.contains(&param.ident)
            {
                generics.push(param.ident.clone());
            }
        }
        generics
    }

    fn collect_patchable_generic_params(&self) -> HashSet<&Ident> {
        let mut patchable_generic_params = HashSet::new();
        for field_type in &self.patchable_field_types {
            for type_name in collect_used_simple_types(field_type) {
                patchable_generic_params.insert(type_name);
            }
        }
        patchable_generic_params
    }

    fn is_generic_param_type(&self, ty: &Type) -> bool {
        let Type::Path(tp) = ty else {
            return false;
        };
        if tp.qself.is_some() || tp.path.segments.len() != 1 {
            return false;
        }
        let segment = &tp.path.segments[0];
        if !matches!(segment.arguments, PathArguments::None) {
            return false;
        }
        self.generics
            .type_params()
            .any(|param| param.ident == segment.ident)
    }

    fn build_bounds(&self) -> TokenStream2 {
        let mut bounds = Vec::new();
        let patchable_generic_params = self.collect_patchable_generic_params();
        for param in self.generics.type_params() {
            let t = &param.ident;
            if patchable_generic_params.contains(t)
                || self
                    .preserved_types
                    .get(t)
                    .is_some_and(|usage| usage.used_in_keep)
            {
                bounds.push(quote! { #t: ::core::clone::Clone });
            }
        }

        let crate_root = &self.crate_path;
        let mut seen_patchable_bounds = HashSet::new();
        for field_type in &self.patchable_field_types {
            let key = field_type.to_token_stream().to_string();
            if seen_patchable_bounds.insert(key) {
                bounds.push(quote! { #field_type: #crate_root :: Patchable });
                if !self.is_generic_param_type(field_type) {
                    bounds.push(quote! {
                        for<'patchable_de> <#field_type as #crate_root :: Patchable>::Patch:
                            ::serde::Deserialize<'patchable_de>
                    });
                }
            }
        }

        if let Some(where_clause) = &self.generics.where_clause {
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

    // ============================================================
    // type Patch = MyState<T, U, ...>
    // ============================================================

    fn build_associate_type_declaration(&self) -> TokenStream2 {
        let mut args = Vec::new();
        let patchable_generic_params = self.collect_patchable_generic_params();
        for param in self.generics.type_params() {
            let t = &param.ident;
            if self.preserved_types.contains_key(t) || patchable_generic_params.contains(t) {
                args.push(quote! { #t });
            }
        }
        let state_name = &self.state_struct_name;
        quote! {
            type Patch = #state_name <#(#args),*>;
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
        crate_name("patchable").expect("patchable library should present in `Cargo.toml`");
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

fn is_phantom_data(field: &Field) -> bool {
    matches!(
        &field.ty,
        Type::Path(p)
        if p.path.segments.first().is_some_and(|s| s.ident == "PhantomData")
    )
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
