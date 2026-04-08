use flui_core::{Div, Styled, div};

/// Create a vertical flex container.
///
/// Equivalent to Flutter's `Column`.
pub fn column() -> Div {
    div().flex().flex_col()
}
