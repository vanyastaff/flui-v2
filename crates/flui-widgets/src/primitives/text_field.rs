use flui_core::{
    AnyElement, App, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, prelude::FluentBuilder,
};

use crate::InteractionState;

/// Visual configuration for the text field, provided by the design system.
#[derive(Clone, Debug)]
pub struct TextFieldVisuals {
    /// Color of the text cursor.
    pub cursor_color: Hsla,
    /// Color of selected text background.
    pub selection_color: Hsla,
    /// Color of the text content.
    pub text_color: Hsla,
    /// Color of placeholder text.
    pub placeholder_color: Hsla,
}

/// Headless text input primitive — basic text display + on_change.
///
/// **Note**: This is a stub implementation. Full cursor, selection, and IME
/// support will be added in a future phase.
///
/// Design systems use `.style()` to apply visual styling.
#[derive(flui_core::IntoElement)]
pub struct TextFieldBase {
    id: ElementId,
    value: SharedString,
    placeholder: Option<SharedString>,
    disabled: bool,
    focus_handle: Option<FocusHandle>,
    visuals: Option<TextFieldVisuals>,
    on_change: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_submit: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    style_fn: Option<Box<dyn FnOnce(InteractionState, AnyElement) -> AnyElement + 'static>>,
}

impl TextFieldBase {
    /// Create a new text field with the given ID and current value.
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            placeholder: None,
            disabled: false,
            focus_handle: None,
            visuals: None,
            on_change: None,
            on_submit: None,
            style_fn: None,
        }
    }

    /// Set placeholder text shown when value is empty.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Attach a focus handle.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Set visual configuration (cursor color, selection color, etc).
    pub fn visuals(mut self, visuals: TextFieldVisuals) -> Self {
        self.visuals = Some(visuals);
        self
    }

    /// Called when the text value changes.
    pub fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Called when the user presses Enter.
    pub fn on_submit(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        self
    }

    /// Apply visual styling from design system.
    pub fn style(
        mut self,
        f: impl FnOnce(InteractionState, AnyElement) -> AnyElement + 'static,
    ) -> Self {
        self.style_fn = Some(Box::new(f));
        self
    }
}

impl RenderOnce for TextFieldBase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = InteractionState {
            disabled: self.disabled,
            ..Default::default()
        };

        // Build text content display
        let text_content: AnyElement = if self.value.is_empty() {
            if let Some(placeholder) = self.placeholder {
                div()
                    .when_some(
                        self.visuals.as_ref().map(|v| v.placeholder_color),
                        |d, color| d.text_color(color),
                    )
                    .child(placeholder)
                    .into_any_element()
            } else {
                div().into_any_element()
            }
        } else {
            div()
                .when_some(self.visuals.as_ref().map(|v| v.text_color), |d, color| {
                    d.text_color(color)
                })
                .child(self.value.clone())
                .into_any_element()
        };

        if let Some(style_fn) = self.style_fn {
            let styled = style_fn(state, text_content);
            let mut el = div().id(self.id).child(styled);
            if let Some(ref fh) = self.focus_handle {
                el = el.track_focus(fh);
            }
            el.into_any_element()
        } else {
            let mut el = div().id(self.id).child(text_content);
            if let Some(ref fh) = self.focus_handle {
                el = el.track_focus(fh);
            }
            el.into_any_element()
        }
    }
}
