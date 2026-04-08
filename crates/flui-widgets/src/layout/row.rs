use flui_core::{Div, Styled, div};

/// Create a horizontal flex container.
///
/// Equivalent to Flutter's `Row`.
pub fn row() -> Div {
    div().flex().flex_row()
}
