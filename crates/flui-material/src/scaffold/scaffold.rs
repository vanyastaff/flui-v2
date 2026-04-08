use flui_core::{
    AnyElement, App, IntoElement, InteractiveElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use flui_theme::ActiveTheme;

use super::AppBar;

/// Material Design 3 Scaffold.
///
/// Provides the basic visual structure: AppBar, body, optional FAB.
#[derive(flui_core::IntoElement)]
pub struct Scaffold {
    app_bar: Option<AppBar>,
    body: Option<AnyElement>,
    floating_action_button: Option<AnyElement>,
}

impl Scaffold {
    pub fn new() -> Self {
        Self {
            app_bar: None,
            body: None,
            floating_action_button: None,
        }
    }

    pub fn app_bar(mut self, app_bar: AppBar) -> Self {
        self.app_bar = Some(app_bar);
        self
    }

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn floating_action_button(mut self, fab: impl IntoElement) -> Self {
        self.floating_action_button = Some(fab.into_any_element());
        self
    }
}

impl RenderOnce for Scaffold {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let bg = theme.color_scheme.background;

        let mut container = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .relative();

        if let Some(app_bar) = self.app_bar {
            container = container.child(app_bar);
        }

        if let Some(body) = self.body {
            container = container.child(
                div()
                    .id("scaffold-body")
                    .flex_grow()
                    .overflow_scroll()
                    .child(body),
            );
        }

        if let Some(fab) = self.floating_action_button {
            container = container.child(
                div()
                    .absolute()
                    .bottom(px(16.))
                    .right(px(16.))
                    .child(fab),
            );
        }

        container
    }
}
