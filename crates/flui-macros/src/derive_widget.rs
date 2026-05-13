//! `#[derive(Widget)]` proc-macro implementation.
//!
//! Generates `impl flui_framework::Widget` for a user struct. Per the
//! SF01 design spec (frozen 2026-05-12) and Amendment 1 (2026-05-12),
//! the macro:
//!
//! - Accepts struct inputs only (rejects `enum`/`union` with a
//!   `compile_error!` pointing at the input span).
//! - Recognises a single optional `#[widget(key)]` field attribute that
//!   marks the field carrying the widget's identity key. The field type
//!   MUST be `Option<Key>` (syntactic variants are accepted — the macro
//!   compares the terminal path segment, not the full path).
//! - Generates `fn key(&self) -> Option<&::flui_framework::Key>` ONLY
//!   when a `#[widget(key)]` field is present; otherwise the
//!   trait-default `None` applies.
//! - Always generates `fn build(&self) -> impl
//!   ::flui_framework::IntoWidget { ::flui_framework::Empty }` per
//!   Amendment 1 — `Widget::build` is a required method and `Empty` is
//!   the sealed null widget for trivial bodies.
//! - Threads generic parameters and where-clauses through the impl.
//! - Uses absolute path prefix `::flui_framework::` in generated code so
//!   the macro works regardless of the user's imports — provided the
//!   caller has `flui-framework` in their dependency graph.
//!
//! See `crates/flui-framework/tests/widget_derive_compile.rs` and
//! `crates/flui-framework/tests/widget_derive/*.rs` for trybuild
//! compile-pass and compile-fail fixtures.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, GenericArgument, PathArguments, Type, parse_macro_input};

/// Entry point for `#[derive(Widget)]`.
pub fn derive_widget(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    // SF01 spec: only `struct` inputs are accepted. The
    // `get_simple_attribute_field` helper silently returns `None` for
    // enum/union, so we do the explicit reject ourselves and span the
    // diagnostic at the input type identifier so the user's cursor
    // lands on the enum/union name rather than the derive invocation.
    let fields = match &ast.data {
        Data::Struct(data_struct) => &data_struct.fields,
        Data::Enum(_) | Data::Union(_) => {
            return syn::Error::new_spanned(&ast.ident, "Widget derive only supports structs")
                .into_compile_error()
                .into();
        }
    };

    // Locate the single optional `#[widget(key)]` field. Iterate every
    // field directly (rather than `get_simple_attribute_field`) because
    // we need both the `Ident` and the `Type` for validation.
    let key_field = match locate_key_field(fields) {
        Ok(maybe_field) => maybe_field,
        Err(err) => return err.into_compile_error().into(),
    };

    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    // Generate `fn key` only when an attribute-marked field is present;
    // otherwise rely on the trait's default `None` impl.
    let key_method = if let Some(field_ident) = key_field {
        Some(quote! {
            fn key(&self) -> ::core::option::Option<&::flui_framework::Key> {
                self.#field_ident.as_ref()
            }
        })
    } else {
        None
    };

    // Amendment 1: `Widget::build` is required. The derive always
    // supplies a trivial body returning `Empty`.
    let build_method = quote! {
        fn build(&self) -> impl ::flui_framework::IntoWidget {
            ::flui_framework::Empty
        }
    };

    let r#gen = quote! {
        #[automatically_derived]
        impl #impl_generics ::flui_framework::Widget for #type_name #type_generics
        #where_clause
        {
            #key_method
            #build_method
        }
    };

    r#gen.into()
}

/// Walk the struct's fields and return the `Ident` of the single
/// optional `#[widget(key)]`-marked field, validating the type along
/// the way.
///
/// Errors:
/// - `Err` if more than one field bears `#[widget(key)]`.
/// - `Err` if the `#[widget(key)]` field's type is not `Option<Key>`
///   (terminal-segment match — see `is_option_key_type`).
/// - `Err` if the `#[widget(key)]`-bearing field is unnamed (tuple
///   struct field) — the macro can only emit `self.<ident>.as_ref()`
///   for named fields.
///
/// Returns `Ok(None)` when no `#[widget(key)]` attribute is present at
/// all — that is the common path for stateless widgets, and the trait's
/// default `Widget::key -> None` covers them.
fn locate_key_field(fields: &syn::Fields) -> syn::Result<Option<syn::Ident>> {
    let mut found: Option<syn::Ident> = None;

    for field in fields {
        let mut field_marks_key = false;
        for attr in &field.attrs {
            if check_widget_key_attribute(attr)? {
                field_marks_key = true;
            }
        }
        if !field_marks_key {
            continue;
        }

        if let Some(prev) = &found {
            return Err(syn::Error::new_spanned(
                field,
                format!(
                    "only one #[widget(key)] field is allowed; previously seen on `{}`",
                    prev
                ),
            ));
        }

        // Require named field — tuple-struct fields would force the
        // derive to emit `self.0.as_ref()` which is fragile and
        // surprising. SF01 only sanctions named-field structs for
        // identity.
        let Some(ident) = field.ident.clone() else {
            return Err(syn::Error::new_spanned(
                field,
                "#[widget(key)] requires a named field; tuple struct fields are not supported",
            ));
        };

        if !is_option_key_type(&field.ty) {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "#[widget(key)] field must be `Option<Key>` (terminal path segment must be `Option<...Key>`; aliases are not resolved by the macro)",
            ));
        }

        found = Some(ident);
    }

    Ok(found)
}

/// Inspect a single attribute and decide whether it represents a
/// well-formed `#[widget(key)]` marker.
///
/// Returns:
/// - `Ok(false)` — the attribute is not a `#[widget(...)]` attribute at
///   all; the caller should skip it.
/// - `Ok(true)` — the attribute is exactly `#[widget(key)]` and marks
///   the field as the identity-key carrier.
/// - `Err(...)` — the attribute IS a `#[widget(...)]` attribute but is
///   malformed. Cases producing errors:
///   - `#[widget]` (bare path) or `#[widget = "..."]` (name-value):
///     rejected with a targeted "expected #[widget(key)]" diagnostic
///     so the user does not see an opaque `parse_nested_meta` parse
///     error (Bug S3, fixed in response to PR #18 Copilot review).
///   - `#[widget()]` (empty meta list): rejected explicitly so a user
///     who forgot the `key` argument gets a clear diagnostic instead
///     of silently inheriting key behavior (Bug S1, fixed 2026-05-12).
///   - `#[widget(<unknown>)]` or `#[widget(key, <unknown>)]`: rejected
///     with a span pointing at the unknown sub-argument so the user
///     sees the typo (Bug S2, fixed 2026-05-12).
fn check_widget_key_attribute(attr: &syn::Attribute) -> syn::Result<bool> {
    if !attr.path().is_ident("widget") {
        return Ok(false);
    }

    // The attribute MUST be in list form `#[widget(...)]`. Reject
    // `#[widget]` (path) and `#[widget = ...]` (name-value) with a
    // targeted diagnostic — `parse_nested_meta` on these forms would
    // otherwise surface as a generic "expected `(`" parse error.
    if !matches!(&attr.meta, syn::Meta::List(_)) {
        return Err(syn::Error::new_spanned(
            attr,
            "expected #[widget(key)]; the `widget` attribute requires arguments in parentheses",
        ));
    }

    let mut seen_key = false;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("key") {
            seen_key = true;
            Ok(())
        } else {
            Err(meta.error("unknown #[widget(...)] argument; expected `key`"))
        }
    })?;

    if !seen_key {
        return Err(syn::Error::new_spanned(
            attr,
            "#[widget(...)] requires the `key` argument; write `#[widget(key)]`",
        ));
    }

    Ok(true)
}

/// True if the type is `Option<T>` whose `T`'s terminal path segment is
/// `Key`. Accepts `Option<Key>`, `Option<flui_framework::Key>`,
/// `Option<::flui_framework::Key>`, `Option<crate::Key>`, etc. Does NOT
/// resolve aliases — `Option<MyAlias>` where `MyAlias = Key` is
/// rejected by design (proc-macros run before name resolution; alias
/// support would require structural inspection that the macro cannot
/// perform reliably).
fn is_option_key_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    // Require last segment to be `Option`.
    let Some(option_segment) = type_path.path.segments.last() else {
        return false;
    };

    if option_segment.ident != "Option" {
        return false;
    }

    let PathArguments::AngleBracketed(args) = &option_segment.arguments else {
        return false;
    };

    // Find the single generic-type argument.
    let mut type_arg = None;
    for arg in &args.args {
        if let GenericArgument::Type(inner) = arg {
            if type_arg.is_some() {
                return false;
            }
            type_arg = Some(inner);
        }
    }

    let Some(inner) = type_arg else {
        return false;
    };

    let Type::Path(inner_path) = inner else {
        return false;
    };

    inner_path
        .path
        .segments
        .last()
        .map(|seg| seg.ident == "Key")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_option_key_type;
    use syn::parse_quote;

    #[test]
    fn option_key_accepted() {
        assert!(is_option_key_type(&parse_quote!(Option<Key>)));
        assert!(is_option_key_type(&parse_quote!(
            Option<flui_framework::Key>
        )));
        assert!(is_option_key_type(&parse_quote!(
            Option<::flui_framework::Key>
        )));
        assert!(is_option_key_type(&parse_quote!(Option<crate::Key>)));
    }

    #[test]
    fn non_option_rejected() {
        assert!(!is_option_key_type(&parse_quote!(Key)));
        assert!(!is_option_key_type(&parse_quote!(String)));
        assert!(!is_option_key_type(&parse_quote!(Vec<Key>)));
    }

    #[test]
    fn option_of_non_key_rejected() {
        assert!(!is_option_key_type(&parse_quote!(Option<String>)));
        assert!(!is_option_key_type(&parse_quote!(Option<i32>)));
        assert!(!is_option_key_type(&parse_quote!(Option<KeyValue>)));
    }
}
