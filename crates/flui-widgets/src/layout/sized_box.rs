use flui_core::{
    AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, div,
};

/// A box with a specific size, optionally containing a child.
///
/// Equivalent to Flutter's `SizedBox`.
#[derive(flui_core::IntoElement)]
pub struct SizedBox {
    width: Option<Pixels>,
    height: Option<Pixels>,
    child: Option<AnyElement>,
}

impl SizedBox {
    /// Create a sized box with explicit width and height.
    pub fn new(width: Pixels, height: Pixels) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
            child: None,
        }
    }

    /// A zero-size box — useful as a spacer or placeholder.
    pub fn shrink() -> Self {
        Self {
            width: None,
            height: None,
            child: None,
        }
    }

    /// A box that expands to fill all available space.
    pub fn expand() -> Self {
        Self {
            width: None,
            height: None,
            child: None,
        }
    }

    /// A square box with equal width and height.
    pub fn square(size: Pixels) -> Self {
        Self::new(size, size)
    }

    /// Set the child widget.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    /// Set the width.
    pub fn width(mut self, w: Pixels) -> Self {
        self.width = Some(w);
        self
    }

    /// Set the height.
    pub fn height(mut self, h: Pixels) -> Self {
        self.height = Some(h);
        self
    }
}

impl RenderOnce for SizedBox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut d = div();
        if let Some(w) = self.width {
            d = d.w(w);
        }
        if let Some(h) = self.height {
            d = d.h(h);
        }
        // expand() variant: fill all space
        if self.width.is_none() && self.height.is_none() && self.child.is_none() {
            d = d.size_full();
        }
        if let Some(child) = self.child {
            d = d.child(child);
        }
        d
    }
}
