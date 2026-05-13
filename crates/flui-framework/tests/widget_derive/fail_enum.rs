//! Fail: derive(Widget) must reject enum inputs.

use flui_framework::Widget;

#[derive(Widget)]
enum NotAStruct {
    A,
    B,
}

fn main() {}
