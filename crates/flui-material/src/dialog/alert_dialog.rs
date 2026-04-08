use flui_core::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;
use flui_widgets::DialogBase;

use crate::buttons::{FilledButton, TextButton};

/// Material Design 3 Alert Dialog.
#[derive(flui_core::IntoElement)]
pub struct AlertDialog {
    visible: bool,
    title: SharedString,
    content: SharedString,
    confirm_label: SharedString,
    cancel_label: Option<SharedString>,
    on_confirm: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_dismiss: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl AlertDialog {
    pub fn new(title: impl Into<SharedString>, content: impl Into<SharedString>) -> Self {
        Self {
            visible: false,
            title: title.into(),
            content: content.into(),
            confirm_label: "OK".into(),
            cancel_label: None,
            on_confirm: None,
            on_dismiss: None,
        }
    }

    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self }
    pub fn confirm_label(mut self, label: impl Into<SharedString>) -> Self { self.confirm_label = label.into(); self }
    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self { self.cancel_label = Some(label.into()); self }

    pub fn on_confirm(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Box::new(f)); self
    }

    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f)); self
    }
}

impl RenderOnce for AlertDialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();

        let title = self.title.clone();
        let content = self.content.clone();
        let confirm_label = self.confirm_label.clone();
        let cancel_label = self.cancel_label.clone();

        let mut dialog = DialogBase::new("alert-dialog").visible(self.visible);

        if let Some(on_dismiss) = self.on_dismiss {
            dialog = dialog.on_dismiss(on_dismiss);
        }

        let on_confirm = self.on_confirm;

        dialog.content(
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.lg)
                .w(flui_core::px(312.))
                .max_w(flui_core::px(560.))
                .rounded(theme.shape.extra_large)
                .bg(theme.color_scheme.surface_container_high)
                .p(theme.spacing.xl)
                // Title
                .child(
                    div()
                        .text_color(theme.color_scheme.on_surface)
                        .text_size(theme.text.headline_small.size)
                        .child(title),
                )
                // Content
                .child(
                    div()
                        .text_color(theme.color_scheme.on_surface_variant)
                        .text_size(theme.text.body_medium.size)
                        .child(content),
                )
                // Actions
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(theme.spacing.sm)
                        .when_some(cancel_label, |d, label| {
                            d.child(TextButton::new("dialog-cancel", label))
                        })
                        .child({
                            let mut btn = FilledButton::new("dialog-confirm", confirm_label);
                            if let Some(on_confirm) = on_confirm {
                                btn = btn.on_click(move |_, window, cx| on_confirm(window, cx));
                            }
                            btn
                        }),
                ),
        )
    }
}
