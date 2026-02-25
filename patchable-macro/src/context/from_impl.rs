use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, WherePredicate, parse_quote};

use crate::context::MacroContext;

impl<'a> MacroContext<'a> {
    // ======================================================================
    // impl<T, ...> From<OriginalStruct<T, ...>> for OriginalStructPatch<...>
    // ======================================================================

    pub(crate) fn build_from_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_where_clause_for_from_impl();

        let input_struct_name = self.struct_name;
        let patch_struct_type = &self.patch_struct_type;
        let from_method_body = self.build_from_method_body();

        quote! {
            impl #impl_generics ::core::convert::From<#input_struct_name #type_generics>
                for #patch_struct_type
            #where_clause {
                #[inline(always)]
                fn from(value: #input_struct_name #type_generics) -> Self {
                    #from_method_body
                }
            }
        }
    }

    fn build_from_method_body(&self) -> TokenStream2 {
        match &self.fields {
            Fields::Named(_) => {
                let field_initializers = self.field_actions.iter().map(|action| {
                    let member = action.member();
                    let value = action.build_initializer_expr();
                    quote! { #member: #value }
                });
                quote! { Self { #(#field_initializers),* } }
            }
            Fields::Unnamed(_) => {
                let field_values = self
                    .field_actions
                    .iter()
                    .map(|action| action.build_initializer_expr());
                quote! { Self(#(#field_values),*) }
            }
            Fields::Unit => {
                debug_assert!(self.field_actions.is_empty());
                quote! { Self }
            }
        }
    }

    fn build_where_clause_for_from_impl(&self) -> Option<syn::WhereClause> {
        let patchable_trait = &self.patchable_trait;
        let trait_bounds: Vec<WherePredicate> = self
            .iter_patchable_type_params()
            .flat_map(|ty| {
                [
                    parse_quote! { #ty: #patchable_trait },
                    parse_quote! { #ty::Patch: ::core::convert::From<#ty> },
                ]
            })
            .collect();
        self.extend_where_clause(&trait_bounds)
    }
}
