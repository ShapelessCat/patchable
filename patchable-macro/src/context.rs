//! # Macro Context
//!
//! Context and logic for the `Patchable` derive macro.
//!
//! This module contains the [`MacroContext`] struct, which analyzes the input struct and generates
//! the code for:
//! 1. The companion patch struct (state struct).
//! 2. The `Patchable` trait implementation.

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
    /// The name of the struct on which the `#[derive(Patchable)]` macro is applied.
    struct_name: &'a Ident,
    /// The generics definition of the target struct.
    generics: &'a Generics,
    /// The fields of the target struct.
    fields: &'a Fields,
    /// Mapping from preserved type to its usage flag.
    preserved_types: HashMap<&'a Ident, TypeUsage>,
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
            panic!("This `Patchable` derive macro can only be applied on structs");
        };

        assert!(
            input
                .generics
                .params
                .iter()
                .all(|g| !matches!(g, GenericParam::Lifetime(_))),
            "`Patchable` does not support borrowed fields"
        );

        let mut preserved_types: HashMap<&Ident, TypeUsage> = HashMap::new();
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
                let Some(type_name) = get_abstract_simple_type_name(field_type) else {
                    return Err(syn::Error::new_spanned(
                        field_type,
                        "Only a simple generic type can be used", // TODO: remove this limit
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

        Ok(MacroContext {
            struct_name: &input.ident,
            generics: &input.generics,
            fields,
            preserved_types,
            field_actions,
            state_struct_name: quote::format_ident!("{}State", &input.ident),
            crate_path: use_site_crate_path(),
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
        for param in self.generics.type_params() {
            if let Some(TypeUsage::Patchable) = self.preserved_types.get(&param.ident) {
                let crate_root = &self.crate_path;
                bounds.push(quote! { #param: #crate_root :: Patchable });
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
        for param in self.generics.type_params() {
            if self.preserved_types.contains_key(&param.ident) {
                generics.push(param.ident.clone());
            }
        }
        generics
    }

    fn build_bounds(&self) -> TokenStream2 {
        let mut bounds = Vec::new();
        for param in self.generics.type_params() {
            let t = &param.ident;
            match self.preserved_types.get(t) {
                Some(TypeUsage::Patchable) => {
                    let crate_root = &self.crate_path;
                    bounds.push(quote! { #t: #crate_root :: Patchable + ::core::clone::Clone });
                }
                Some(TypeUsage::NotPatchable) => {
                    bounds.push(quote! { #t: ::core::clone::Clone });
                }
                None => {}
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
        for param in self.generics.type_params() {
            let t = &param.ident;
            if self.preserved_types.contains_key(t) {
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
