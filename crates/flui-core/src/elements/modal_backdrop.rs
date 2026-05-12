//! ADR-018: canonical "modal blocks below" backdrop helper.
//!
//! A modal opened via [`crate::Window::defer_draw`] at [`crate::z::Z_MODAL`]
//! paints on top of every lower-priority overlay and wins clicks within
//! its own bounds, but clicks *outside* its bounds still reach elements
//! below (per ADR-018 decision 5: "There is no 'block input below' effect
//! inherent in `defer_draw` priority"). To actually block input below the
//! modal, an app pairs the modal content with a full-window transparent
//! backdrop that consumes pointer events.
//!
//! [`modal_backdrop`] returns exactly that backdrop, painted via
//! `Deferred::with_priority(Z_MODAL - 1)` so it sits just below the
//! modal content. The optional `on_dismiss` closure fires when the user
//! clicks outside the modal — the canonical "click-outside-to-close"
//! pattern.
//!
//! See `docs/research/adr/ADR-018-modal-overlay-layering.md`.

use crate::{
    AnyElement, App, Element, InteractiveElement, IntoElement, LayoutId, Styled, Window, deferred,
    div, transparent_black,
};

/// Returns a full-window transparent backdrop element that consumes
/// pointer events and sits at [`crate::z::Z_MODAL`] minus one. Wrap
/// the modal *content* in [`crate::deferred`] at `Z_MODAL` and pair
/// it with this backdrop for the canonical "modal blocks below"
/// composition:
///
/// ```no_compile
/// use flui_core::{deferred, elements::modal_backdrop, z};
///
/// fn my_modal(on_dismiss: impl Fn() + 'static) -> impl flui_core::IntoElement {
///     div()
///         .child(modal_backdrop(on_dismiss))
///         .child(deferred(my_modal_content()).with_priority(z::Z_MODAL))
/// }
/// ```
///
/// Per ADR-018 decision 4 modality is per-window, so a click in
/// another window is not blocked by this backdrop — that is the
/// engine's invariant, not a feature of this helper.
pub fn modal_backdrop<F>(on_dismiss: F) -> ModalBackdrop<F>
where
    F: 'static + Fn(&mut Window, &mut App),
{
    ModalBackdrop { on_dismiss }
}

/// Returned by [`modal_backdrop`]. Use [`crate::IntoElement`] to compose
/// into the tree. Carries the dismiss closure as a type parameter so
/// no allocation happens until the click fires.
pub struct ModalBackdrop<F> {
    on_dismiss: F,
}

impl<F> IntoElement for ModalBackdrop<F>
where
    F: 'static + Fn(&mut Window, &mut App),
{
    type Element = ModalBackdropElement;

    fn into_element(self) -> Self::Element {
        // ADR-018: full-window transparent backdrop. Captures
        // mouse-down + scroll-wheel so clicks/scrolls below are
        // blocked. The dismiss closure fires on mouse-down so the
        // "click outside to close" pattern is opt-in via the public
        // `modal_backdrop(on_dismiss)` constructor.
        let on_dismiss = self.on_dismiss;
        let backdrop = div().size_full().bg(transparent_black()).on_mouse_down(
            crate::MouseButton::Left,
            move |_, window, cx| {
                on_dismiss(window, cx);
                cx.stop_propagation();
            },
        );
        ModalBackdropElement {
            inner: Some(
                deferred(backdrop)
                    .with_priority(crate::z::Z_MODAL.saturating_sub(1))
                    .into_any_element(),
            ),
        }
    }
}

/// Erased element wrapper produced by `modal_backdrop().into_element()`.
/// Holds the dismiss-capable child as an `AnyElement` so consumers can
/// embed it like any other tree node.
pub struct ModalBackdropElement {
    inner: Option<AnyElement>,
}

impl Element for ModalBackdropElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, cx: &mut crate::LayoutCx<'_>) -> (LayoutId, ()) {
        let layout_id = self.inner.as_mut().unwrap().request_layout(cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        cx: &mut crate::PrepaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
    ) {
        self.inner.as_mut().unwrap().prepaint(cx);
    }

    fn paint(
        &mut self,
        cx: &mut crate::PaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
    ) {
        self.inner.as_mut().unwrap().paint(cx);
    }
}

impl IntoElement for ModalBackdropElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
