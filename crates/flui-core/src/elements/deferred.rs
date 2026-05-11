use crate::{AnyElement, Element, IntoElement, LayoutId};

/// Builds a `Deferred` element, which delays the layout and paint of its child.
pub fn deferred(child: impl IntoElement) -> Deferred {
    Deferred {
        child: Some(child.into_any_element()),
        priority: 0,
    }
}

/// An element which delays the painting of its child until after all of
/// its ancestors, while keeping its layout as part of the current element tree.
pub struct Deferred {
    child: Option<AnyElement>,
    priority: usize,
}

impl Deferred {
    /// Sets the `priority` value of the `deferred` element, which
    /// determines the drawing order relative to other deferred elements,
    /// with higher values being drawn on top.
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Element for Deferred {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, cx: &mut crate::LayoutCx<'_>) -> (LayoutId, ()) {
        let layout_id = self.child.as_mut().unwrap().request_layout(cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        cx: &mut crate::PrepaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
    ) {
        cx.with_window_app(|window, _cx| {
            let child = self.child.take().unwrap();
            let element_offset = window.element_offset();
            window.defer_draw(child, element_offset, self.priority, None)
        })
    }

    fn paint(
        &mut self,
        _cx: &mut crate::PaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
    ) {
    }
}

impl IntoElement for Deferred {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Deferred {
    /// Sets a priority for the element. A higher priority conceptually means painting the element
    /// on top of deferred draws with a lower priority (i.e. closer to the viewer).
    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}
