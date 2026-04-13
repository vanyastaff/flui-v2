use flui_core::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;
use flui_widgets::ButtonBase;

/// Material Design 3 Filled Button.
///
/// Primary action button with solid background color.
#[derive(flui_core::IntoElement)]
pub struct FilledButton {
    id: ElementId,
    label: SharedString,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    leading_icon: Option<AnyElement>,
}

impl FilledButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            on_click: None,
            leading_icon: None,
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

    pub fn leading_icon(mut self, icon: impl IntoElement) -> Self {
        self.leading_icon = Some(icon.into_any_element());
        self
    }
}

impl RenderOnce for FilledButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let disabled = self.disabled;

        let label_el = div()
            .flex()
            .items_center()
            .gap_2()
            .when_some(self.leading_icon, |d, icon| d.child(icon))
            .child(self.label);

        let mut base = ButtonBase::new(self.id)
            .disabled(self.disabled)
            .child(label_el);

        if let Some(on_click) = self.on_click {
            base = base.on_click(on_click);
        }

        base.style(move |_state, child| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .px(theme.spacing.xl)
                .py(theme.spacing.sm)
                .rounded(theme.shape.full)
                .when(disabled, |d| {
                    d.bg(theme.color_scheme.on_surface.opacity(0.12))
                        .text_color(theme.color_scheme.on_surface.opacity(0.38))
                })
                .when(!disabled, |d| {
                    d.bg(theme.color_scheme.primary)
                        .text_color(theme.color_scheme.on_primary)
                        .hover(|s| s.opacity(0.92))
                })
                .child(child)
                .into_any_element()
        })
    }
}
