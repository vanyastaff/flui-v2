//! Navigation Demo for flui-navigator
//!
//! Demonstrates basic routing with transitions between pages.

#![allow(clippy::needless_pass_by_ref_mut)]

use flui_core::{
    App, Application, Bounds, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
    prelude::*, px, rgb, size,
};
use flui_navigator::{Navigator, Route, RouteParams, Transition, init_router, router_view};

fn main() {
    Application::new().run(|cx: &mut App| {
        init_router(cx, |router| {
            router.add_route(
                Route::new("/", home_page)
                    .name("home")
                    .transition(Transition::fade(200)),
            );
            router.add_route(
                Route::new("/about", about_page)
                    .name("about")
                    .transition(Transition::slide_left(300)),
            );
            router.add_route(
                Route::new("/contact", contact_page)
                    .name("contact")
                    .transition(Transition::slide_left(300)),
            );
        });

        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("flui-navigator Demo".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| AppView),
        )
        .unwrap();
    });
}

struct AppView;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .child(nav_bar())
            .child(div().flex_1().child(router_view(window, cx)))
    }
}

fn nav_bar() -> impl IntoElement {
    div()
        .flex()
        .gap(px(8.))
        .p(px(12.))
        .bg(rgb(0x313244))
        .child(nav_button("Home", "/"))
        .child(nav_button("About", "/about"))
        .child(nav_button("Contact", "/contact"))
}

fn nav_button(label: &str, path: &str) -> impl IntoElement {
    let path = path.to_string();
    let label = SharedString::from(label.to_string());
    div()
        .px(px(16.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(rgb(0x45475a))
        .hover(|s| s.bg(rgb(0x585b70)))
        .cursor_pointer()
        .text_color(rgb(0xcdd6f4))
        .child(label)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            Navigator::push(cx, path.as_str());
        })
}

fn home_page(_window: &mut Window, _cx: &mut App, _params: &RouteParams) -> flui_core::AnyElement {
    page_content("Home", "Welcome to flui-v2!", hsla(0.38, 0.74, 0.66, 1.0))
}

fn about_page(_window: &mut Window, _cx: &mut App, _params: &RouteParams) -> flui_core::AnyElement {
    page_content(
        "About",
        "flui-v2 is a Flutter-inspired GPU-accelerated UI framework for Rust.",
        hsla(0.62, 0.87, 0.76, 1.0),
    )
}

fn contact_page(
    _window: &mut Window,
    _cx: &mut App,
    _params: &RouteParams,
) -> flui_core::AnyElement {
    page_content(
        "Contact",
        "github.com/vanyastaff/flui-v2",
        hsla(0.9, 0.76, 0.74, 1.0),
    )
}

fn page_content(title: &str, body: &str, accent: flui_core::Hsla) -> flui_core::AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .gap(px(16.))
        .child(
            div()
                .text_color(accent)
                .text_size(px(32.))
                .child(SharedString::from(title.to_string())),
        )
        .child(
            div()
                .text_color(rgb(0xbac2de))
                .text_size(px(16.))
                .child(SharedString::from(body.to_string())),
        )
        .into_any_element()
}
