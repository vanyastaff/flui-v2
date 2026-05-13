//! Fail: `#[widget = "..."]` (name-value form) must be rejected with
//! a targeted "expected #[widget(key)]" diagnostic instead of an opaque
//! `parse_nested_meta` parse error. Bug S3, PR #18 Copilot review.

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct BadAttr {
    #[widget = "key"]
    key: Option<Key>,
}

fn main() {}
