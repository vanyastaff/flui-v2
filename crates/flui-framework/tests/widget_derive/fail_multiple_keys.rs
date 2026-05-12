//! Fail: only one #[widget(key)] field is allowed.

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct TwoKeys {
    #[widget(key)]
    first: Option<Key>,
    #[widget(key)]
    second: Option<Key>,
}

fn main() {}
