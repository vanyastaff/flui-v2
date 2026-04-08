use flui_core::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div};
use smallvec::SmallVec;

/// Overlays children on top of each other using absolute positioning.
///
/// Equivalent to Flutter's `Stack`.
///
/// The first child is at the bottom, the last child is on top.
#[derive(flui_core::IntoElement)]
pub struct Stack {
    children: SmallVec<[AnyElement; 4]>,
}

impl Stack {
    /// Create a new empty Stack.
    pub fn new() -> Self {
        Self {
            children: SmallVec::new(),
        }
    }

    /// Add a child to the stack.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Add multiple children to the stack.
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children
            .extend(children.into_iter().map(|c| c.into_any_element()));
        self
    }
}

impl RenderOnce for Stack {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().relative().size_full().children(self.children)
    }
}
