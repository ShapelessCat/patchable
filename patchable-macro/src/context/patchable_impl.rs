use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::context::MacroContext;

impl<'a> MacroContext<'a> {
    // ============================================================
    // impl<T, ...> Patchable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patchable_trait_impl(&self) -> TokenStream2 {
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let patchable_trait = &self.patchable_trait;
        let input_struct_name = self.struct_name;
        let extra_trait_bounds = self.build_trait_bounds(patchable_trait);
        let where_clause = self.extend_where_clause(&extra_trait_bounds);
        let patch_struct_type = &self.patch_struct_type;

        quote! {
            impl #impl_generics #patchable_trait
                for #input_struct_name #type_generics
            #where_clause {
                type Patch = #patch_struct_type;
            }
        }
    }
}
