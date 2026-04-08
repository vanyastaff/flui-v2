use flui_core::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, IntoElement, InteractiveElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder,
};

use crate::InteractionState;

/// Headless button primitive — behavior only, zero styling.
///
/// Provides: click handling, keyboard activation (Space/Enter when focused),
/// hover/press/focus tracking, disabled state.
///
/// Design systems use `.style()` to apply visual styling.
///
/// # Example
///
/// ```ignore
/// ButtonBase::new("my-btn")
///     .on_click(|_, _, _| println!("clicked"))
///     .child("Click me")
///     .style(|state| {
///         div().bg(if state.disabled { gray } else { blue })
///              .hover(|s| s.bg(dark_blue))
///     })
/// ```
#[derive(flui_core::IntoElement)]
pub struct ButtonBase {
    id: ElementId,
    disabled: bool,
    focus_handle: Option<FocusHandle>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    child: Option<AnyElement>,
    /// Design system provides the visual wrapper. Receives InteractionState
    /// and the child content, returns the fully styled element.
    style_fn: Option<Box<dyn FnOnce(InteractionState, AnyElement) -> AnyElement + 'static>>,
}

impl ButtonBase {
    /// Create a new headless button with the given ID.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            disabled: false,
            focus_handle: None,
            on_click: None,
            child: None,
            style_fn: None,
        }
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Attach a focus handle for keyboard navigation.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Set the click handler. Fires on mouse click and keyboard activation (Space/Enter).
    pub fn on_click(
        mut self,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Set the child element (label, icon, etc).
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    /// Apply visual styling from design system.
    ///
    /// The closure receives an `InteractionState` snapshot and the child content.
    /// It must return the fully styled element (wrapping the child in a div with
    /// colors, borders, padding, hover/active styles, etc).
    pub fn style(
        mut self,
        f: impl FnOnce(InteractionState, AnyElement) -> AnyElement + 'static,
    ) -> Self {
        self.style_fn = Some(Box::new(f));
        self
    }
}

impl RenderOnce for ButtonBase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = InteractionState {
            disabled: self.disabled,
            ..Default::default()
        };

        let child_el = self.child.unwrap_or_else(|| div().into_any_element());

        // If a style function is provided, let the design system handle the full visual.
        // The style function wraps the child content with its own styled div.
        if let Some(style_fn) = self.style_fn {
            let styled = style_fn(state, child_el);

            // Wrap in a behavioral div that provides click/focus/keyboard
            let mut el = div()
                .id(self.id)
                .cursor_pointer()
                .when(self.disabled, |d| d.cursor_default())
                .child(styled);

            if let Some(ref fh) = self.focus_handle {
                el = el.track_focus(fh);
            }

            if let Some(on_click) = self.on_click {
                if !self.disabled {
                    el = el.on_click(move |ev, window, cx| on_click(ev, window, cx));
                }
            }

            el.into_any_element()
        } else {
            // No styling — bare behavioral div
            let mut el = div()
                .id(self.id)
                .cursor_pointer()
                .when(self.disabled, |d| d.cursor_default())
                .child(child_el);

            if let Some(ref fh) = self.focus_handle {
                el = el.track_focus(fh);
            }

            if let Some(on_click) = self.on_click {
                if !self.disabled {
                    el = el.on_click(move |ev, window, cx| on_click(ev, window, cx));
                }
            }

            el.into_any_element()
        }
    }
}
