use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::Fields;

use crate::{
    IS_SERDE_ENABLED,
    context::{FieldAction, FieldMember, MacroContext},
};

impl<'a> MacroContext<'a> {
    // ============================================================
    // #[derive(::serde::Deserialize)]
    // struct InputTypePatch<T, ...> ...
    // ============================================================

    pub(crate) fn build_patch_struct(&self) -> TokenStream2 {
        let derive_attr = IS_SERDE_ENABLED.then_some(quote! { #[derive(::serde::Deserialize)] });
        let patch_struct_type = &self.patch_struct_type;

        let where_clause = self.build_where_clause_with_bound(&self.patchable_trait);
        let patch_fields = self.generate_patch_fields();
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

    fn generate_patch_fields(&self) -> Vec<TokenStream2> {
        let patchable_trait = &self.patchable_trait;
        self.field_actions
            .iter()
            .map(|action| match action {
                FieldAction::Keep { member, ty } => match member {
                    FieldMember::Named(name) => quote! { #name : #ty },
                    FieldMember::Unnamed(_) => quote! { #ty },
                },
                FieldAction::Patch { member, ty } => {
                    let field = match member {
                        FieldMember::Named(name) => {
                            quote! { #name : <#ty as #patchable_trait>::Patch }
                        }
                        FieldMember::Unnamed(_) => quote! { <#ty as #patchable_trait>::Patch },
                    };
                    if IS_SERDE_ENABLED {
                        let bound =
                            quote! { <#ty as #patchable_trait>::Patch: ::serde::de::DeserializeOwned };
                        let bound_string = bound.to_string();
                        let bound_lit = syn::LitStr::new(&bound_string, Span::call_site());
                        quote! {
                            #[serde(bound(deserialize = #bound_lit))]
                            #field
                        }
                    } else {
                        quote! { #field }
                    }
                }
            })
            .collect()
    }

    // ============================================================
    // impl<T, ...> Patchable for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patchable_trait_impl(&self) -> TokenStream2 {
        let patchable_trait = &self.patchable_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let input_struct_name = self.struct_name;
        let where_clause = self.build_where_clause_with_bound(patchable_trait);
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
