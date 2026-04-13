use flui_core::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;
use flui_widgets::ButtonBase;

/// Material Design 3 Text Button.
///
/// No background, no border — just text with primary color.
#[derive(flui_core::IntoElement)]
pub struct TextButton {
    id: ElementId,
    label: SharedString,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl TextButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            on_click: None,
        }
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for TextButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let disabled = self.disabled;

        let mut base = ButtonBase::new(self.id)
            .disabled(self.disabled)
            .child(self.label);
        if let Some(on_click) = self.on_click {
            base = base.on_click(on_click);
        }

        base.style(move |_state, child| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .px(theme.spacing.md)
                .py(theme.spacing.sm)
                .rounded(theme.shape.full)
                .when(disabled, |d| {
                    d.text_color(theme.color_scheme.on_surface.opacity(0.38))
                })
                .when(!disabled, |d| {
                    d.text_color(theme.color_scheme.primary)
                        .hover(|s| s.bg(theme.color_scheme.primary.opacity(0.08)))
                })
                .child(child)
                .into_any_element()
        })
    }
}
