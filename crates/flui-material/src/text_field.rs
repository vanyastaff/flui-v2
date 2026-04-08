use flui_core::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;
use flui_widgets::TextFieldBase;

/// Material Design 3 Text Field input decoration.
#[derive(Default)]
pub struct InputDecoration {
    pub label: Option<SharedString>,
    pub helper_text: Option<SharedString>,
    pub prefix_icon: Option<AnyElement>,
    pub suffix_icon: Option<AnyElement>,
    pub filled: bool,
}

impl InputDecoration {
    pub fn new() -> Self { Self::default() }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self { self.label = Some(label.into()); self }
    pub fn helper_text(mut self, text: impl Into<SharedString>) -> Self { self.helper_text = Some(text.into()); self }
    pub fn filled(mut self) -> Self { self.filled = true; self }
}

/// Material Design 3 TextField.
///
/// Wraps `TextFieldBase` with M3 styling + `InputDecoration`.
#[derive(flui_core::IntoElement)]
pub struct TextField {
    id: ElementId,
    value: SharedString,
    placeholder: Option<SharedString>,
    disabled: bool,
    decoration: InputDecoration,
    on_change: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
}

impl TextField {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            placeholder: None,
            disabled: false,
            decoration: InputDecoration::default(),
            on_change: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self { self.placeholder = Some(text.into()); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn decoration(mut self, dec: InputDecoration) -> Self { self.decoration = dec; self }
    pub fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f)); self
    }
}

impl RenderOnce for TextField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();

        let mut base = TextFieldBase::new(self.id, self.value);
        if let Some(placeholder) = self.placeholder {
            base = base.placeholder(placeholder);
        }
        base = base.disabled(self.disabled);
        if let Some(on_change) = self.on_change {
            base = base.on_change(on_change);
        }

        let base_el = base.style(move |_state, child| {
            let mut container = div()
                .flex()
                .items_center()
                .gap(theme.spacing.sm)
                .px(theme.spacing.lg)
                .py(theme.spacing.md)
                .rounded(theme.shape.extra_small)
                .text_color(theme.color_scheme.on_surface)
                .when(self.decoration.filled, |d| {
                    d.bg(theme.color_scheme.surface_container_high)
                })
                .when(!self.decoration.filled, |d| {
                    d.border_1().border_color(theme.color_scheme.outline)
                })
                .child(child);

            container.into_any_element()
        });

        // Wrap with label and helper text
        let mut wrapper = div().flex().flex_col().gap_1();

        if let Some(label) = self.decoration.label {
            wrapper = wrapper.child(
                div()
                    .text_color(theme.color_scheme.on_surface_variant)
                    .text_size(theme.text.body_small.size)
                    .child(label),
            );
        }

        wrapper = wrapper.child(base_el);

        if let Some(helper) = self.decoration.helper_text {
            wrapper = wrapper.child(
                div()
                    .text_color(theme.color_scheme.on_surface_variant)
                    .text_size(theme.text.body_small.size)
                    .child(helper),
            );
        }

        wrapper
    }
}
