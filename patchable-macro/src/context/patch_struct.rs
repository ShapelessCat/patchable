use super::*;

impl<'a> MacroContext<'a> {
    // ============================================================
    // #[derive(::serde::Deserialize)]
    // struct InputTypePatch<T, ...> ...
    // ============================================================

    pub(crate) fn build_patch_struct(&self) -> TokenStream2 {
        let generic_params = self.build_patch_type_generics();
        let where_clause = self.build_where_clause_with_bound(&self.patchable_trait);
        let patch_fields = self.generate_patch_fields();
        let body = match &self.fields {
            Fields::Named(_) => quote! { #generic_params #where_clause { #(#patch_fields),* } },
            Fields::Unnamed(_) => quote! { #generic_params ( #(#patch_fields),* ) #where_clause; },
            Fields::Unit => quote! {;},
        };
        let patch_name = &self.patch_struct_name;
        let derive_attr = if IS_SERDE_ENABLED {
            quote! { #[derive(::serde::Deserialize)] }
        } else {
            quote! {}
        };

        quote! {
            #derive_attr
            pub struct #patch_name #body
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
}
