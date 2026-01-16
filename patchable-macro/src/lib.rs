use proc_macro::TokenStream;

use quote::quote;
use syn::{self, DeriveInput};

mod context;

#[proc_macro_derive(Patchable, attributes(patchable))]
pub fn derive_state_and_patchable_impl(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse_macro_input!(input as DeriveInput);
    match context::MacroContext::new(&input) {
        Ok(ctx) => {
            let state_struct_def = ctx.build_state_struct();
            let patchable_trait_impl = ctx.build_patchable_trait_impl();

            quote! {
                const _: () = {
                    #state_struct_def
                    #[automatically_derived]
                    #patchable_trait_impl
                };
            }
            .into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}
