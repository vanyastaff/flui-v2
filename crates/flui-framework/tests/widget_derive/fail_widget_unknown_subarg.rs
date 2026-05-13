//! Fail: #[widget(key, <unknown>)] must surface a diagnostic.
//!
//! Before the 2026-05-12 fix, `#[widget(key, unknown)]` was silently
//! ignored because the `parse_nested_meta` closure returned an Err on
//! the second iteration, the `.is_ok()` check converted that to false,
//! and the field was treated as not having the attribute at all. The
//! fix propagates the error so the user sees the `meta.error("unknown
//! #[widget(...)] argument; expected `key`")` diagnostic.

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct Foo {
    #[widget(key, oops_typo)]
    id: Option<Key>,
}

fn main() {}
