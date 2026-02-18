use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Index;

use crate::context::{FieldAction, FieldMember, MacroContext};

impl<'a> MacroContext<'a> {
    // ============================================================
    // impl<T, ...> Patch for OriginalStruct<T, ...
    // ============================================================

    pub(crate) fn build_patch_trait_impl(&self) -> TokenStream2 {
        let patch_trait = &self.patch_trait;
        let (impl_generics, type_generics, _) = self.generics.split_for_impl();
        let where_clause = self.build_where_clause_with_bound(patch_trait);

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
        if self.field_actions.is_empty() {
            return quote! {};
        }

        let statements = self
            .field_actions
            .iter()
            .enumerate()
            .map(|(patch_index, action)| match action {
                FieldAction::Keep { member, .. } => {
                    let patch_member = patch_member(member, patch_index);
                    quote! {
                        self.#member = patch.#patch_member;
                    }
                }
                FieldAction::Patch { member, .. } => {
                    let patch_member = patch_member(member, patch_index);
                    quote! {
                        self.#member.patch(patch.#patch_member);
                    }
                }
            });

        quote! {
            #(#statements)*
        }
    }
}

fn patch_member(member: &FieldMember<'_>, patch_index: usize) -> TokenStream2 {
    match member {
        FieldMember::Named(name) => quote! { #name },
        FieldMember::Unnamed(_) => {
            let index = Index::from(patch_index);
            quote! { #index }
        }
    }
}
