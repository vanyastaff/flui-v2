//! Pass: widget with the canonical `#[widget(key)] key: Option<Key>`
//! field. The derive must emit both fn key (returning self.key.as_ref())
//! and fn build (returning Empty).

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct WithKey {
    #[widget(key)]
    key: Option<Key>,
}

fn main() {
    let none = WithKey { key: None };
    assert!(<WithKey as flui_framework::Widget>::key(&none).is_none());

    let some = WithKey {
        key: Some(Key::local()),
    };
    assert!(<WithKey as flui_framework::Widget>::key(&some).is_some());
}
