use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Ident;

use crate::context::{MacroContext, TypeUsage};

impl<'a> MacroContext<'a> {
    pub(super) fn build_where_clause_with_bound(&self, bound: &TokenStream2) -> TokenStream2 {
        self.build_where_clause_for_patchable_types(|ty| quote! { #ty: #bound, })
    }

    pub(super) fn build_where_clause_for_patchable_types<F>(&self, build_bounds: F) -> TokenStream2
    where
        F: Fn(&Ident) -> TokenStream2,
    {
        let bounded_types: Vec<_> = self
            .iter_patchable_type_params()
            .map(build_bounds)
            .collect();
        self.extend_where_clause(bounded_types)
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

    fn extend_where_clause(&self, bounds: Vec<TokenStream2>) -> TokenStream2 {
        match (&self.generics.where_clause, bounds.is_empty()) {
            (None, true) => quote! {},
            (None, false) => quote! { where #(#bounds)* },
            (Some(where_clause), true) => quote! { #where_clause },
            (Some(where_clause), false) => {
                let sep = (!where_clause.predicates.trailing_punct()).then_some(quote! {,});
                quote! { #where_clause #sep #(#bounds)* }
            }
        }
    }
}
