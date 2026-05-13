//! Pass: generic struct. The derive must thread `<T: 'static>`
//! parameters and any where-clauses through the generated impl.

use flui_framework::Widget;

#[derive(Widget)]
struct Generic<T: 'static> {
    value: T,
}

fn main() {
    let g = Generic { value: 42_i32 };
    assert!(<Generic<i32> as flui_framework::Widget>::key(&g).is_none());
}
