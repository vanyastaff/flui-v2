// CONTRACT (ADR-012 — Custom canvas paint):
//
// `canvas(prepaint, paint)` is the public low-level paint surface.
//
// `prepaint(bounds, window, app) -> T` runs during the prepaint phase and
// produces a `T` that is private to this element instance for this frame.
// `paint(bounds, T, window, app)` runs during `DrawPhase::Paint` and may
// call any `Window::paint_*` method (`paint_quad`, `paint_text`,
// `paint_path`, `paint_image`, …) any number of times.
//
// `canvas` does NOT compile shaders. Authors that need a custom shader
// compose pre-built effects via the styled API. A future "custom shader"
// element is a separate ADR with a distinct lifecycle.
//
// `canvas` participates in invalidation like any other element: its
// `prepaint`/`paint` re-runs when the owning view notifies. There is no
// per-`canvas` redraw request distinct from `Window::refresh()` /
// `cx.notify(view)` (ADR-001 / ADR-002 contracts).
//
// The `T` between `prepaint` and `paint` is private; authors MUST NOT
// depend on it surviving across frames.
//
// See: `docs/research/adr/ADR-012-custom-canvas-paint.md`.

use refineable::Refineable as _;

use crate::{
    App, Bounds, Element, ElementId, IntoElement, Pixels, Style, StyleRefinement, Styled, Window,
};

/// Construct a canvas element with the given paint callback.
/// Useful for adding short term custom drawing to a view.
pub fn canvas<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint: impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T> {
    Canvas {
        prepaint: Some(Box::new(prepaint)),
        paint: Some(Box::new(paint)),
        style: StyleRefinement::default(),
    }
}

/// A canvas element, meant for accessing the low level paint API without defining a whole
/// custom element
pub struct Canvas<T> {
    prepaint: Option<Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T>>,
    paint: Option<Box<dyn FnOnce(Bounds<Pixels>, T, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl<T: 'static> IntoElement for Canvas<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<T: 'static> Element for Canvas<T> {
    type RequestLayoutState = Style;
    type PrepaintState = Option<T>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        cx: &mut crate::LayoutCx<'_>,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        cx.with_window_app(|window, cx| {
            let mut style = Style::default();
            style.refine(&self.style);
            let layout_id = window.request_layout(style.clone(), [], cx);
            (layout_id, style)
        })
    }

    fn prepaint(
        &mut self,
        cx: &mut crate::PrepaintCx<'_>,
        _request_layout: &mut Style,
    ) -> Option<T> {
        let bounds = cx.bounds();
        cx.with_window_app(|window, cx| Some(self.prepaint.take().unwrap()(bounds, window, cx)))
    }

    fn paint(
        &mut self,
        cx: &mut crate::PaintCx<'_>,
        style: &mut Style,
        prepaint: &mut Self::PrepaintState,
    ) {
        let bounds = cx.bounds();
        let prepaint = prepaint.take().unwrap();
        cx.with_window_app(|window, cx| {
            style.paint(bounds, window, cx, |window, cx| {
                (self.paint.take().unwrap())(bounds, prepaint, window, cx)
            });
        });
    }
}

impl<T> Styled for Canvas<T> {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}
