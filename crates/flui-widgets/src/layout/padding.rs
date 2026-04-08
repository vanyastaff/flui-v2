use flui_core::{AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, div};

use super::EdgeInsets;

/// Adds padding around a child widget.
///
/// Equivalent to Flutter's `Padding` widget.
///
/// # Example
///
/// ```ignore
/// Padding::all(px(16.)).child(Text::new("Hello"))
/// Padding::symmetric(px(16.), px(8.)).child(my_widget)
/// ```
#[derive(flui_core::IntoElement)]
pub struct Padding {
    insets: EdgeInsets,
    child: Option<AnyElement>,
}

impl Padding {
    /// Create padding with the given edge insets.
    pub fn new(insets: EdgeInsets) -> Self {
        Self {
            insets,
            child: None,
        }
    }

    /// Uniform padding on all sides.
    pub fn all(value: Pixels) -> Self {
        Self::new(EdgeInsets::all(value))
    }

    /// Symmetric horizontal and vertical padding.
    pub fn symmetric(horizontal: Pixels, vertical: Pixels) -> Self {
        Self::new(EdgeInsets::symmetric(horizontal, vertical))
    }

    /// Individual padding for each edge.
    pub fn only(left: Pixels, top: Pixels, right: Pixels, bottom: Pixels) -> Self {
        Self::new(EdgeInsets::only(left, top, right, bottom))
    }

    /// Set the child widget.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Padding {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut d = div()
            .pl(self.insets.left)
            .pr(self.insets.right)
            .pt(self.insets.top)
            .pb(self.insets.bottom);
        if let Some(child) = self.child {
            d = d.child(child);
        }
        d
    }
}
