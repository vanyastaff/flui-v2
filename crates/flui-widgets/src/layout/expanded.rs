use flui_core::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div};

/// A widget that expands to fill available space along the parent's main axis.
///
/// Must be used inside a `row()` or `column()` (flex container).
///
/// Equivalent to Flutter's `Expanded`.
#[derive(flui_core::IntoElement)]
pub struct Expanded {
    flex: u32,
    child: Option<AnyElement>,
}

impl Expanded {
    /// Create an Expanded with flex factor 1.
    pub fn new() -> Self {
        Self {
            flex: 1,
            child: None,
        }
    }

    /// Set the flex factor (default: 1).
    pub fn flex(mut self, value: u32) -> Self {
        self.flex = value;
        self
    }

    /// Set the child widget.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Expanded {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut d = div().flex_grow().flex_shrink().min_w_0().min_h_0();
        if let Some(child) = self.child {
            d = d.child(child);
        }
        d
    }
}

/// A widget that takes a proportional amount of space without forcing expansion.
///
/// Equivalent to Flutter's `Flexible` with `FlexFit::Loose`.
#[derive(flui_core::IntoElement)]
pub struct Flexible {
    flex: u32,
    child: Option<AnyElement>,
}

impl Flexible {
    /// Create a Flexible with flex factor 1.
    pub fn new() -> Self {
        Self {
            flex: 1,
            child: None,
        }
    }

    /// Set the flex factor.
    pub fn flex(mut self, value: u32) -> Self {
        self.flex = value;
        self
    }

    /// Set the child widget.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Flexible {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut d = div().flex_grow().flex_shrink_0();
        if let Some(child) = self.child {
            d = d.child(child);
        }
        d
    }
}
