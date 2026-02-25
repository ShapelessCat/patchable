use proc_macro2::TokenStream as TokenStream2;
use syn::{Ident, WhereClause, WherePredicate, parse_quote};

use crate::context::{MacroContext, TypeUsage};

impl<'a> MacroContext<'a> {
    pub(super) fn build_trait_bounds(&self, bound: &TokenStream2) -> Vec<WherePredicate> {
        self.iter_patchable_type_params()
            .map(|ty| parse_quote! { #ty: #bound })
            .collect()
    }

    pub(super) fn extend_where_clause(
        &self,
        trait_bounds: &[WherePredicate],
    ) -> Option<WhereClause> {
        let mut where_clause = self.generics.where_clause.clone();
        if !trait_bounds.is_empty() {
            where_clause
                .get_or_insert_with(|| parse_quote! { where })
                .predicates
                .extend(trait_bounds.iter().cloned());
        }
        where_clause
    }

    pub(super) fn iter_patchable_type_params(&self) -> impl Iterator<Item = &Ident> + '_ {
        self.generics.type_params().filter_map(|param| {
            matches!(
                self.preserved_types.get(&param.ident),
                Some(TypeUsage::Patchable)
            )
            .then_some(&param.ident)
        })
    }
}
