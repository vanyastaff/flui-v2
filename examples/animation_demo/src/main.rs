//! Animation System Demo
//!
//! Demonstrates AnimationController, Curve, Tween, and animated() wrapper.

extern crate flui_core;

use std::time::Duration;

use flui_core::{
    div, hsla, prelude::*, px, size, App, Application, Bounds, Context, Entity, IntoElement,
    ParentElement, Render, Styled, Window, WindowBounds, WindowOptions, TitlebarOptions,
};
use flui_core::animation::{AnimationController, Curve, Tween, animated};
use flui_material::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("flui Animation Demo".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| AnimationDemo::new(cx)),
        )
        .unwrap();
    });
}

struct AnimationDemo {
    fade: Entity<AnimationController>,
    slide: Entity<AnimationController>,
    color: Entity<AnimationController>,
    bounce: Entity<AnimationController>,
}

impl AnimationDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            fade: AnimationController::new(Duration::from_millis(600))
                .curve(Curve::EaseInOut)
                .attach(cx),
            slide: AnimationController::new(Duration::from_millis(800))
                .curve(Curve::EaseOutCubic)
                .attach(cx),
            color: AnimationController::new(Duration::from_millis(1000))
                .curve(Curve::EaseInOut)
                .attach(cx),
            bounce: AnimationController::new(Duration::from_millis(1200))
                .curve(Curve::Bounce)
                .attach(cx),
        }
    }
}

impl Render for AnimationDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fade = self.fade.clone();
        let slide = self.slide.clone();
        let color_anim = self.color.clone();
        let bounce = self.bounce.clone();

        MaterialApp::new()
            .theme_mode(ThemeMode::Dark)
            .child(
                Scaffold::new()
                    .app_bar(AppBar::new("Animation Demo"))
                    .body(
                        column()
                            .p(px(24.))
                            .gap(px(24.))
                            // Fade
                            .child(
                                row().items_center().gap(px(16.))
                                    .child(
                                        animated(&self.fade, window, cx, |v| {
                                            div()
                                                .w(px(200.)).h(px(60.)).rounded(px(12.))
                                                .bg(hsla(262. / 360., 0.52, 0.47, 1.0))
                                                .opacity(v)
                                                .flex().items_center().justify_center()
                                                .text_color(hsla(0., 0., 1., 1.0))
                                                .child("Fade (EaseInOut)")
                                        })
                                    )
                                    .child(make_controls("fade", fade))
                            )
                            // Slide
                            .child(
                                row().items_center().gap(px(16.))
                                    .child(
                                        animated(&self.slide, window, cx, |v| {
                                            let offset = Tween::new(px(-200.), px(0.)).transform(v);
                                            div()
                                                .ml(offset)
                                                .w(px(200.)).h(px(60.)).rounded(px(12.))
                                                .bg(hsla(145. / 360., 0.63, 0.42, 1.0))
                                                .flex().items_center().justify_center()
                                                .text_color(hsla(0., 0., 1., 1.0))
                                                .child("Slide (EaseOutCubic)")
                                        })
                                    )
                                    .child(make_controls("slide", slide))
                            )
                            // Color tween
                            .child(
                                row().items_center().gap(px(16.))
                                    .child(
                                        animated(&self.color, window, cx, |v| {
                                            let bg = Tween::new(
                                                hsla(0., 0.74, 0.40, 1.0),
                                                hsla(262. / 360., 0.52, 0.47, 1.0),
                                            ).transform(v);
                                            div()
                                                .w(px(200.)).h(px(60.)).rounded(px(12.))
                                                .bg(bg)
                                                .flex().items_center().justify_center()
                                                .text_color(hsla(0., 0., 1., 1.0))
                                                .child("Color Tween")
                                        })
                                    )
                                    .child(make_controls("color", color_anim))
                            )
                            // Bounce
                            .child(
                                row().items_center().gap(px(16.))
                                    .child(
                                        animated(&self.bounce, window, cx, |v| {
                                            let h = Tween::new(px(20.), px(60.)).transform(v);
                                            div()
                                                .w(px(200.)).h(h).rounded(px(12.))
                                                .bg(hsla(40. / 360., 0.95, 0.55, 1.0))
                                                .flex().items_center().justify_center()
                                                .text_color(hsla(0., 0., 0., 1.0))
                                                .child("Bounce!")
                                        })
                                    )
                                    .child(make_controls("bounce", bounce))
                            ),
                    ),
            )
    }
}

fn make_controls(prefix: &str, ctrl: Entity<AnimationController>) -> impl IntoElement {
    let ctrl2 = ctrl.clone();
    let ctrl3 = ctrl.clone();
    row()
        .gap(px(8.))
        .child(
            FilledButton::new(format!("{prefix}-fwd"), "Forward")
                .on_click({
                    let c = ctrl;
                    move |_, _, cx| { c.update(cx, |c, cx| c.forward(cx)); }
                }),
        )
        .child(
            OutlinedButton::new(format!("{prefix}-rev"), "Reverse")
                .on_click({
                    let c = ctrl2;
                    move |_, _, cx| { c.update(cx, |c, cx| c.reverse(cx)); }
                }),
        )
        .child(
            TextButton::new(format!("{prefix}-reset"), "Reset")
                .on_click({
                    let c = ctrl3;
                    move |_, _, cx| { c.update(cx, |c, cx| c.reset(cx)); }
                }),
        )
}
