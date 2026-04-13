//! Material Design 3 Demo
//!
//! Demonstrates MaterialApp, Scaffold, AppBar, all 5 button variants,
//! Card, Divider, and theme switching.

extern crate flui_core;

use flui_core::{
    App, Application, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use flui_material::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("flui Material Demo".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| AppRoot),
        )
        .unwrap();
    });
}

struct AppRoot;

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        MaterialApp::new().theme_mode(ThemeMode::Dark).child(
            Scaffold::new()
                .app_bar(AppBar::new("flui Material Demo"))
                .body(demo_content()),
        )
    }
}

fn demo_content() -> impl IntoElement {
    column()
        .p(px(24.))
        .gap(px(24.))
        .child(button_section())
        .child(Divider::horizontal())
        .child(card_section())
}

fn button_section() -> impl IntoElement {
    column()
        .gap(px(16.))
        .child(div().text_size(px(20.)).child("Buttons"))
        .child(
            row()
                .gap(px(12.))
                .items_center()
                .child(
                    FilledButton::new("filled-btn", "Filled")
                        .on_click(|_, _, _| println!("Filled clicked")),
                )
                .child(
                    ElevatedButton::new("elevated-btn", "Elevated")
                        .on_click(|_, _, _| println!("Elevated clicked")),
                )
                .child(
                    OutlinedButton::new("outlined-btn", "Outlined")
                        .on_click(|_, _, _| println!("Outlined clicked")),
                )
                .child(
                    TextButton::new("text-btn", "Text")
                        .on_click(|_, _, _| println!("Text clicked")),
                )
                .child(
                    IconButton::new("icon-btn", div().child("X"))
                        .on_click(|_, _, _| println!("Icon clicked")),
                ),
        )
        .child(
            row()
                .gap(px(12.))
                .items_center()
                .child(FilledButton::new("disabled-filled", "Disabled").disabled(true))
                .child(OutlinedButton::new("disabled-outlined", "Disabled").disabled(true)),
        )
}

fn card_section() -> impl IntoElement {
    column()
        .gap(px(16.))
        .child(div().text_size(px(20.)).child("Cards"))
        .child(
            row()
                .gap(px(16.))
                .child(
                    Card::elevated().child(
                        div()
                            .child("Elevated Card")
                            .child(div().text_size(px(12.)).child("With shadow")),
                    ),
                )
                .child(Card::filled().child(div().child("Filled Card")))
                .child(Card::outlined().child(div().child("Outlined Card"))),
        )
}
