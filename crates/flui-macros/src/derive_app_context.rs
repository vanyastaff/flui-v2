use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use crate::get_simple_attribute_field;

pub fn derive_app_context(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let Some(app_variable) = get_simple_attribute_field(&ast, "app") else {
        return quote! {
            compile_error!("Derive must have an #[app] attribute to detect the &mut App field");
        }
        .into();
    };

    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics flui_core::AppContext for #type_name #type_generics
        #where_clause
        {
            fn new<T: 'static>(
                &mut self,
                build_entity: impl FnOnce(&mut flui_core::Context<'_, T>) -> T,
            ) -> flui_core::Entity<T> {
                self.#app_variable.new(build_entity)
            }

            fn reserve_entity<T: 'static>(&mut self) -> flui_core::Reservation<T> {
                self.#app_variable.reserve_entity()
            }

            fn insert_entity<T: 'static>(
                &mut self,
                reservation: flui_core::Reservation<T>,
                build_entity: impl FnOnce(&mut flui_core::Context<'_, T>) -> T,
            ) -> flui_core::Entity<T> {
                self.#app_variable.insert_entity(reservation, build_entity)
            }

            fn update_entity<T, R>(
                &mut self,
                handle: &flui_core::Entity<T>,
                update: impl FnOnce(&mut T, &mut flui_core::Context<'_, T>) -> R,
            ) -> R
            where
                T: 'static,
            {
                self.#app_variable.update_entity(handle, update)
            }

            fn as_mut<'y, 'z, T>(
                &'y mut self,
                handle: &'z flui_core::Entity<T>,
            ) -> flui_core::GpuiBorrow<'y, T>
            where
                T: 'static,
            {
                self.#app_variable.as_mut(handle)
            }

            fn read_entity<T, R>(
                &self,
                handle: &flui_core::Entity<T>,
                read: impl FnOnce(&T, &flui_core::App) -> R,
            ) -> R
            where
                T: 'static,
            {
                self.#app_variable.read_entity(handle, read)
            }

            fn update_window<T, F>(&mut self, window: flui_core::AnyWindowHandle, f: F) -> flui_core::Result<T>
            where
                F: FnOnce(flui_core::AnyView, &mut flui_core::Window, &mut flui_core::App) -> T,
            {
                self.#app_variable.update_window(window, f)
            }

            fn read_window<T, R>(
                &self,
                window: &flui_core::WindowHandle<T>,
                read: impl FnOnce(flui_core::Entity<T>, &flui_core::App) -> R,
            ) -> flui_core::Result<R>
            where
                T: 'static,
            {
                self.#app_variable.read_window(window, read)
            }

            fn background_spawn<R>(&self, future: impl std::future::Future<Output = R> + Send + 'static) -> flui_core::Task<R>
            where
                R: Send + 'static,
            {
                self.#app_variable.background_spawn(future)
            }

            fn read_global<G, R>(&self, callback: impl FnOnce(&G, &flui_core::App) -> R) -> R
            where
                G: flui_core::Global,
            {
                self.#app_variable.read_global(callback)
            }
        }
    };

    r#gen.into()
}
