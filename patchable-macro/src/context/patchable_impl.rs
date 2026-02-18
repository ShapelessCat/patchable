use super::*;

impl<'a> MacroContext<'a> {
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

    pub(super) fn build_patch_type_generics(&self) -> TokenStream2 {
        let patch_generic_params = self.iter_preserved_type_params();
        // Empty `<>` is legal in Rust, and adding or dropping the `<>` doesn't affect the
        // definition. For example, `struct A<>(i32)` and `struct A(i32)` have the
        // same HIR.
        quote! { <#(#patch_generic_params),*> }
    }

    // ===========================================
    // Helper functions for building where clauses
    // ===========================================

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
