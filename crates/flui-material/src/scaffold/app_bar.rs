use flui_core::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
    prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;

/// Material Design 3 App Bar (top bar).
#[derive(flui_core::IntoElement)]
pub struct AppBar {
    title: SharedString,
    leading: Option<AnyElement>,
    actions: Vec<AnyElement>,
}

impl AppBar {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            leading: None,
            actions: Vec::new(),
        }
    }

    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for AppBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let surface = theme.color_scheme.surface;
        let on_surface = theme.color_scheme.on_surface;
        let outline = theme.color_scheme.outline_variant;
        let title_size = theme.text.title_large.size;

        div()
            .flex()
            .items_center()
            .h(px(64.))
            .px(theme.spacing.lg)
            .gap(theme.spacing.sm)
            .bg(surface)
            .border_b_1()
            .border_color(outline)
            .when_some(self.leading, |d, l| d.child(l))
            .child(
                div()
                    .flex_grow()
                    .text_color(on_surface)
                    .text_size(title_size)
                    .child(self.title),
            )
            .children(self.actions)
    }
}
