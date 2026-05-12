//! Pass: bare leaf widget with no key field, no children.
//! The derive must emit only the build body (returning Empty) and rely
//! on the default Widget::key returning None.

use flui_framework::Widget;

#[derive(Widget)]
struct Leaf;

fn main() {
    let leaf = Leaf;
    // Widget::key default impl returns None when no #[widget(key)]
    // field is present.
    assert!(<Leaf as flui_framework::Widget>::key(&leaf).is_none());
}
