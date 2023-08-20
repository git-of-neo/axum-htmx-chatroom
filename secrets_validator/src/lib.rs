extern crate proc_macro;

use quote::quote;

#[proc_macro]
pub fn check_env(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    check_env_inner(input.into()).into()
}

fn check_env_inner(_input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if let Err(_) = dotenvy::var("DATABASE_URL") {
        let msg = "DATABASE_URL not set";
        return quote! {compile_error!(#msg);};
    };

    quote! {}
}
