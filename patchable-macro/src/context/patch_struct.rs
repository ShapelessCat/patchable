use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Fields;

use crate::{IS_SERDE_ENABLED, context::MacroContext};

impl<'a> MacroContext<'a> {
    // ============================================================
    // #[derive(::serde::Deserialize)]
    // struct InputTypePatch<T, ...> ...
    // ============================================================

    pub(crate) fn build_patch_struct(&self) -> TokenStream2 {
        let derive_attr = IS_SERDE_ENABLED.then_some(quote! { #[derive(::serde::Deserialize)] });
        let patch_struct_type = &self.patch_struct_type;

        let bounded_types = self.build_trait_bounds(&self.patchable_trait);
        let where_clause = if bounded_types.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounded_types),* }
        };
        let patch_fields = self.field_actions.iter().map(|action| action.build_field());
        let body = match &self.fields {
            Fields::Named(_) => quote! { #where_clause { #(#patch_fields),* } },
            Fields::Unnamed(_) => quote! { ( #(#patch_fields),* ) #where_clause; },
            Fields::Unit => quote! {;},
        };

        quote! {
            #derive_attr
            pub struct #patch_struct_type #body
        }
    }
}
