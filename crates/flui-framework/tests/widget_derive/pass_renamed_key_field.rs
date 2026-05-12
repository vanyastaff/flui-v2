//! Pass (T4.5): `#[widget(key)]` works on a field with a non-default
//! name. Guards against the derive implicitly assuming the field is
//! named `key`. The attribute, not the field name, marks identity.

use flui_framework::{Key, Widget};

#[derive(Widget)]
struct WithRenamedKey {
    #[widget(key)]
    id: Option<Key>,
}

fn main() {
    let r = WithRenamedKey {
        id: Some(Key::local()),
    };
    // Widget::key must read `self.id`, not `self.key` (which doesn't
    // exist on this struct). If the macro hardcoded the field name to
    // `key`, this would fail to compile.
    assert!(<WithRenamedKey as flui_framework::Widget>::key(&r).is_some());

    let r_none = WithRenamedKey { id: None };
    assert!(<WithRenamedKey as flui_framework::Widget>::key(&r_none).is_none());
}
