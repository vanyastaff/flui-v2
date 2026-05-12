//! Fail: #[widget(key)] field must be Option<Key>.

use flui_framework::Widget;

#[derive(Widget)]
struct WrongKeyType {
    #[widget(key)]
    key: String,
}

fn main() {}
