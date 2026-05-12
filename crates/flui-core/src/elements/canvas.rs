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
/// Useful for adding short-term custom drawing to a view.
///
/// # ADR-012: prepaint / paint split
///
/// `canvas` is the public low-level paint surface (see the
/// `// CONTRACT (ADR-012)` block at the top of this file). The two
/// closures express the engine's prepaint → paint pipeline:
///
/// - **`prepaint(bounds, window, app) -> T`** runs during the prepaint
///   phase. Use it for layout-dependent work that must run *before*
///   paint can start — laying out shaped text, computing path metrics,
///   sampling animation curves at the current frame time. The returned
///   `T` is stored on the element until paint and then dropped; it does
///   not survive across frames.
/// - **`paint(bounds, T, window, app)`** runs during paint. Issue any
///   number of `Window::paint_*` calls from inside.
///
/// # Example — sparkline
///
/// A minimal sparkline that lays out point positions in prepaint and
/// strokes them as a path in paint. Each frame the value buffer is read
/// fresh from the surrounding view; `canvas` participates in the same
/// invalidation pipeline as any other element (ADR-001 / ADR-002).
///
/// ```no_compile
/// use flui_core::{canvas, point, px, Bounds, Path, Pixels, Window};
///
/// fn sparkline(samples: Vec<f32>) -> impl flui_core::IntoElement {
///     canvas(
///         // Prepaint: convert sample values into points laid out
///         // across the canvas bounds. The resulting Vec lives until
///         // paint runs, then drops.
///         move |bounds: Bounds<Pixels>, _window, _app| {
///             let n = samples.len().max(1);
///             let step = bounds.size.width / px(n as f32);
///             let max = samples.iter().cloned().fold(0.0_f32, f32::max).max(1.0);
///             samples
///                 .iter()
///                 .enumerate()
///                 .map(|(i, &v)| {
///                     point(
///                         bounds.origin.x + step * px(i as f32),
///                         bounds.origin.y + bounds.size.height * px(1.0 - v / max),
///                     )
///                 })
///                 .collect::<Vec<_>>()
///         },
///         // Paint: stroke the path. The engine handles damage / clipping
///         // / scaling around this closure.
///         |_bounds, points, window, _app| {
///             let mut path = Path::new(points[0]);
///             for p in &points[1..] {
///                 path.line_to(*p);
///             }
///             window.paint_path(path, flui_core::Hsla::black());
///         },
///     )
/// }
/// ```
///
/// (Marked `no_compile` because `flui-core` sets `doctest = false` in
/// `Cargo.toml` — the example is rustdoc-rendered but not type-checked.
/// The signatures above match the public API at the time of writing.)
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
