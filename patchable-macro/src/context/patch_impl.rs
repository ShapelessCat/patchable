use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::MacroContext;

impl<'a> MacroContext<'a> {
    // ============================================================
    // impl<T, ...> Patch for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patch_trait_impl(&self) -> TokenStream2 {
        let patch_trait = &self.patch_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let extra_trait_bounds = self.build_trait_bounds(patch_trait);
        let where_clause = self.extend_where_clause(&extra_trait_bounds);

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

    fn generate_patch_method_body(&self) -> TokenStream2 {
        let statements = self
            .field_actions
            .iter()
            .enumerate()
            .map(|(patch_index, action)| {
                action.build_update_statement(&self.patch_trait, patch_index)
            });

        quote! { #(#statements)* }
    }
}
