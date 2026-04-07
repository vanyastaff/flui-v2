use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn derive_render(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics flui_core::Render for #type_name #type_generics
        #where_clause
        {
            fn render(&mut self, _window: &mut flui_core::Window, _cx: &mut flui_core::Context<Self>) -> impl flui_core::Element {
                flui_core::Empty
            }
        }
    };

    r#gen.into()
}
