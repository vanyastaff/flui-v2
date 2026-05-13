//! Fail: #[widget()] (empty meta list) must be rejected explicitly.
//!
//! Before the 2026-05-12 fix, empty `#[widget()]` was silently treated
//! as `#[widget(key)]` because `parse_nested_meta` calls its closure
//! zero times for an empty meta list and returns Ok. The fix adds an
//! explicit "seen_key" check that requires the `key` argument to
//! appear at least once.

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct Foo {
    #[widget()]
    key: Option<Key>,
}

fn main() {}
