use flui_core::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div,
};
use smallvec::SmallVec;

/// Headless scrollable container.
///
/// Wraps children in a scrollable div.
#[derive(flui_core::IntoElement)]
pub struct ScrollBase {
    children: SmallVec<[AnyElement; 2]>,
}

impl ScrollBase {
    /// Create a new scroll container.
    pub fn new() -> Self {
        Self {
            children: SmallVec::new(),
        }
    }

    /// Add a child element.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Add multiple child elements.
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children
            .extend(children.into_iter().map(|c| c.into_any_element()));
        self
    }
}

impl RenderOnce for ScrollBase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("scroll-base")
            .size_full()
            .overflow_scroll()
            .children(self.children)
    }
}
