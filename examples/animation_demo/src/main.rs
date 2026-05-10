//! Animation System Demo — S21 phase 7.6 showcase
//!
//! Eight sections covering the Flutter-parity animation surface that landed
//! in S21:
//!
//!   1. Curves Catalogue       — eight easing curves on a single play button
//!   2. Tween Family           — IntTween, ColorTween, SizeTween, ConstantTween
//!   3. TweenSequence          — multi-segment opacity → size → colour chain
//!   4. Controller Polish      — animate_to / animate_back / fling / velocity
//!   5. Simulation-driven      — SpringSimulation via animate_with
//!   6. Listener Channel       — raw add_listener / add_status_listener
//!   7. CurvedAnimation        — explanatory note (decorator vs. controller.curve())
//!   8. Combinators            — explanatory note (Proxy / Compound / TrainHopping)
//!
//! The "explanatory note" sections (7, 8) describe surface that requires
//! `Rc<dyn Animation<f64>>` sources — they are meant for widget-layer
//! composition and are exercised via unit tests in
//! `crates/flui-core/src/animation/{combinator.rs, curved_animation.rs}`.

extern crate flui_core;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use flui_core::animation::{
    Animatable, AnimatableExt, Animation, AnimationBehavior, AnimationController, AnimationStatus,
    AnimationStyle, ColorTween, ConstantTween, Curve, CurveTween, Curves, IntTween,
    ListenerCallback, ListenerId, SizeTween, SpringDescription, SpringSimulation,
    StatusListenerCallback, Tween, TweenSequence, TweenSequenceItem, animated,
};
use flui_core::{
    App, Application, Bounds, Context, Entity, IntoElement, ParentElement, Pixels, Render, Size,
    Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla, prelude::*, px, size,
};
use flui_material::*;

// ============================================================================
// App entry
// ============================================================================

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("flui Animation Demo (S21)".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(AnimationDemo::new),
        )
        .unwrap();
    });
}

// ============================================================================
// Demo state
// ============================================================================

struct AnimationDemo {
    // Section 1 — Curves catalogue (one shared controller drives all eight tiles)
    curves_ctrl: Entity<AnimationController>,
    // Section 2 — Tween family
    int_ctrl: Entity<AnimationController>,
    step_ctrl: Entity<AnimationController>,
    color_ctrl: Entity<AnimationController>,
    size_ctrl: Entity<AnimationController>,
    // Section 3 — TweenSequence
    seq_ctrl: Entity<AnimationController>,
    // Section 4 — Controller polish (animate_to / animate_back / fling)
    polish_ctrl: Entity<AnimationController>,
    // Section 5 — Simulation-driven
    spring_ctrl: Entity<AnimationController>,
    // Section 6 — Listener channel + status
    listener_ctrl: Entity<AnimationController>,
    tick_count: Rc<Cell<u32>>,
    last_status: Rc<Cell<Option<AnimationStatus>>>,
    listener_id: Cell<Option<ListenerId>>,
    status_listener_id: Cell<Option<ListenerId>>,
}

impl AnimationDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let curves_ctrl = AnimationController::new(Duration::from_millis(1500)).attach(cx);
        let int_ctrl = AnimationController::new(Duration::from_millis(1200))
            .curve(Curves::EASE_IN_OUT)
            .attach(cx);
        let step_ctrl = AnimationController::new(Duration::from_millis(1500))
            .curve(Curves::LINEAR)
            .attach(cx);
        let color_ctrl = AnimationController::new(Duration::from_millis(1200))
            .curve(Curves::EASE_IN_OUT)
            .attach(cx);
        let size_ctrl = AnimationController::new(Duration::from_millis(1200))
            .curve(Curves::EASE_OUT_CUBIC)
            .attach(cx);
        let seq_ctrl = AnimationController::new(Duration::from_millis(2400))
            .curve(Curves::LINEAR)
            .attach(cx);
        let polish_ctrl = AnimationController::new(Duration::from_millis(700))
            .curve(Curves::EASE_OUT_CUBIC)
            .attach(cx);
        let spring_ctrl = AnimationController::new(Duration::from_millis(800))
            .curve(Curves::LINEAR)
            .attach(cx);
        let listener_ctrl = AnimationController::new(Duration::from_millis(1500))
            .curve(Curves::EASE_IN_OUT)
            .attach(cx);

        // Wire raw listeners onto `listener_ctrl` for Section 6.
        let tick_count = Rc::new(Cell::new(0u32));
        let last_status = Rc::new(Cell::new(None::<AnimationStatus>));

        let tick_count_for_listener = Rc::clone(&tick_count);
        let listener_id = listener_ctrl
            .read(cx)
            .add_listener(ListenerCallback::new(move || {
                tick_count_for_listener.set(tick_count_for_listener.get() + 1);
            }));

        let last_status_for_listener = Rc::clone(&last_status);
        let status_listener_id =
            listener_ctrl
                .read(cx)
                .add_status_listener(StatusListenerCallback::new(move |status| {
                    last_status_for_listener.set(Some(status));
                }));

        Self {
            curves_ctrl,
            int_ctrl,
            step_ctrl,
            color_ctrl,
            size_ctrl,
            seq_ctrl,
            polish_ctrl,
            spring_ctrl,
            listener_ctrl,
            tick_count,
            last_status,
            listener_id: Cell::new(Some(listener_id)),
            status_listener_id: Cell::new(Some(status_listener_id)),
        }
    }
}

// On Drop, clean up the raw listener subscriptions (Section 6).
impl Drop for AnimationDemo {
    fn drop(&mut self) {
        // The Entity is gone from `cx` by this point in normal teardown,
        // but the listener IDs are still tracked here for completeness.
        let _ = self.listener_id.take();
        let _ = self.status_listener_id.take();
    }
}

// ============================================================================
// Render
// ============================================================================

impl Render for AnimationDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        MaterialApp::new().theme_mode(ThemeMode::Dark).child(
            Scaffold::new()
                .app_bar(AppBar::new("Animation Demo (S21 surface)"))
                .body(
                    column()
                        .p(px(20.))
                        .gap(px(20.))
                        .child(section_curves_catalogue(self, window, cx))
                        .child(section_tween_family(self, window, cx))
                        .child(section_tween_sequence(self, window, cx))
                        .child(section_controller_polish(self, window, cx))
                        .child(section_simulation(self, window, cx))
                        .child(section_listener_channel(self, window, cx))
                        .child(section_curved_animation_note())
                        .child(section_combinators_note()),
                ),
        )
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn section_card(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    column()
        .p(px(16.))
        .gap(px(12.))
        .rounded(px(8.))
        .bg(hsla(220. / 360., 0.10, 0.18, 1.0))
        .child(
            div()
                .text_size(px(16.))
                .text_color(hsla(0., 0., 1., 1.0))
                .child(title),
        )
        .child(body)
}

fn note_card(title: &'static str, body: &'static str) -> impl IntoElement {
    column()
        .p(px(16.))
        .gap(px(8.))
        .rounded(px(8.))
        .bg(hsla(220. / 360., 0.10, 0.14, 1.0))
        .child(
            div()
                .text_size(px(16.))
                .text_color(hsla(60. / 360., 0.55, 0.75, 1.0))
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(hsla(0., 0., 0.85, 1.0))
                .child(body),
        )
}

fn play_button(
    id: &'static str,
    label: &'static str,
    ctrl: Entity<AnimationController>,
) -> impl IntoElement {
    FilledButton::new(id, label).on_click(move |_, _, cx| {
        ctrl.update(cx, |c, cx| {
            c.reset(cx);
            c.forward(cx);
        });
    })
}

fn reverse_button(
    id: &'static str,
    label: &'static str,
    ctrl: Entity<AnimationController>,
) -> impl IntoElement {
    OutlinedButton::new(id, label).on_click(move |_, _, cx| {
        ctrl.update(cx, |c, cx| c.reverse(cx));
    })
}

fn repeat_button(
    id: &'static str,
    label: &'static str,
    ctrl: Entity<AnimationController>,
) -> impl IntoElement {
    TextButton::new(id, label).on_click(move |_, _, cx| {
        ctrl.update(cx, |c, cx| {
            c.reset(cx);
            c.repeat(cx);
        });
    })
}

fn stop_button(
    id: &'static str,
    label: &'static str,
    ctrl: Entity<AnimationController>,
) -> impl IntoElement {
    TextButton::new(id, label).on_click(move |_, _, cx| {
        ctrl.update(cx, |c, cx| c.stop(cx));
    })
}

// ============================================================================
// Section 1 — Curves Catalogue
// ============================================================================

fn section_curves_catalogue(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    type CurveEntry = (&'static str, fn(f32) -> f32);
    let curves: [CurveEntry; 8] = [
        ("Linear", |t| Curves::LINEAR.transform(t)),
        ("EaseInOut", |t| Curves::EASE_IN_OUT.transform(t)),
        ("EaseOutCubic", |t| Curves::EASE_OUT_CUBIC.transform(t)),
        ("EaseInOutCubic", |t| Curves::EASE_IN_OUT_CUBIC.transform(t)),
        ("BounceOut", |t| Curves::BOUNCE_OUT.transform(t)),
        ("ElasticOut", |t| Curves::ELASTIC_OUT.transform(t)),
        ("FastOutSlowIn", |t| Curves::FAST_OUT_SLOW_IN.transform(t)),
        ("Decelerate", |t| Curves::DECELERATE.transform(t)),
    ];

    let mut grid = column().gap(px(8.));
    for chunk in curves.chunks(4) {
        let mut r = row().gap(px(8.));
        for (label, curve_fn) in chunk {
            r = r.child(curve_tile(demo, window, cx, label, *curve_fn));
        }
        grid = grid.child(r);
    }

    section_card(
        "1. Curves Catalogue — 8 easing curves driven by one controller",
        column().gap(px(12.)).child(grid).child(
            row()
                .gap(px(8.))
                .child(play_button("curves-fwd", "Play", demo.curves_ctrl.clone()))
                .child(reverse_button(
                    "curves-rev",
                    "Reverse",
                    demo.curves_ctrl.clone(),
                ))
                .child(repeat_button(
                    "curves-loop",
                    "Loop",
                    demo.curves_ctrl.clone(),
                ))
                .child(stop_button("curves-stop", "Stop", demo.curves_ctrl.clone())),
        ),
    )
}

fn curve_tile(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
    label: &'static str,
    curve_fn: fn(f32) -> f32,
) -> impl IntoElement {
    column()
        .gap(px(4.))
        .child(animated(&demo.curves_ctrl, window, cx, move |t| {
            let eased = curve_fn(t);
            let bar_width = px(40.0 + 100.0 * eased);
            column()
                .gap(px(4.))
                .child(div().h(px(8.)).w(bar_width).rounded(px(4.)).bg(hsla(
                    262. / 360.,
                    0.55,
                    0.55,
                    1.0,
                )))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(hsla(0., 0., 0.75, 1.0))
                        .child(label),
                )
        }))
}

// ============================================================================
// Section 2 — Tween Family
// ============================================================================

fn section_tween_family(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    let int_tween = IntTween::new(0, 100);
    let step_tween = IntTween::new(0, 4); // 0..=4 discrete jumps
    let color_tween = ColorTween::new(
        Some(hsla(0. / 360., 0.7, 0.5, 1.0)),
        Some(hsla(262. / 360., 0.55, 0.55, 1.0)),
    );
    let size_tween = SizeTween::new(Size::new(px(20.), px(20.)), Size::new(px(120.), px(60.)));
    // ConstantTween demo: a value that never changes (tuple-struct ctor).
    let constant: ConstantTween<f64> = ConstantTween(0.5);

    section_card(
        "2. Tween Family — IntTween, StepTween, ColorTween, SizeTween, ConstantTween",
        column()
            .gap(px(12.))
            .child(
                row()
                    .gap(px(16.))
                    .items_center()
                    .child(animated(&demo.int_ctrl, window, cx, move |t| {
                        let n = int_tween.transform(t as f64);
                        column()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .text_color(hsla(0., 0., 1., 1.0))
                                    .child(format!("{n}")),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(hsla(0., 0., 0.7, 1.0))
                                    .child("IntTween 0→100 (eased)"),
                            )
                    }))
                    .child(animated(&demo.step_ctrl, window, cx, move |t| {
                        let step = step_tween.transform(t as f64);
                        column()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .text_color(hsla(40. / 360., 0.95, 0.6, 1.0))
                                    .child(format!("step = {step}")),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(hsla(0., 0., 0.7, 1.0))
                                    .child("IntTween 0→4 / linear (discrete)"),
                            )
                    }))
                    .child(
                        column()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .text_color(hsla(0., 0., 0.85, 1.0))
                                    .child(format!("{:.2}", constant.transform(0.0))),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(hsla(0., 0., 0.7, 1.0))
                                    .child("ConstantTween (never animates)"),
                            ),
                    ),
            )
            .child(
                row()
                    .gap(px(16.))
                    .items_center()
                    .child(animated(&demo.color_ctrl, window, cx, move |t| {
                        let bg = color_tween
                            .transform(t as f64)
                            .unwrap_or(hsla(0., 0., 0.5, 1.0));
                        div()
                            .w(px(120.))
                            .h(px(60.))
                            .rounded(px(8.))
                            .bg(bg)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(hsla(0., 0., 1., 1.0))
                            .child("ColorTween")
                    }))
                    .child(animated(&demo.size_ctrl, window, cx, move |t| {
                        let s = size_tween.transform(t as f64);
                        div()
                            .w(s.width)
                            .h(s.height)
                            .rounded(px(6.))
                            .bg(hsla(145. / 360., 0.55, 0.55, 1.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(hsla(0., 0., 1., 1.0))
                            .text_size(px(12.))
                            .child("SizeTween")
                    })),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(play_button("tw-int-play", "Int", demo.int_ctrl.clone()))
                    .child(play_button("tw-step-play", "Step", demo.step_ctrl.clone()))
                    .child(play_button("tw-col-play", "Color", demo.color_ctrl.clone()))
                    .child(play_button("tw-size-play", "Size", demo.size_ctrl.clone()))
                    .child(reverse_button(
                        "tw-int-rev",
                        "Reverse Int",
                        demo.int_ctrl.clone(),
                    ))
                    .child(reverse_button(
                        "tw-col-rev",
                        "Reverse Color",
                        demo.color_ctrl.clone(),
                    )),
            ),
    )
}

// ============================================================================
// Section 3 — TweenSequence (multi-segment)
// ============================================================================

fn section_tween_sequence(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    // Three-segment width animation with different per-segment easings via
    // CurveTween chain composition. Total weight: 1 + 1 + 2 = 4 so segments
    // occupy 25%, 25%, 50% of the timeline.
    let seq_width: TweenSequence<Pixels> = TweenSequence::new(vec![
        TweenSequenceItem::new(
            Box::new(Tween::new(px(40.), px(160.)).chain(CurveTween::new(Curves::EASE_IN))),
            1.0,
        ),
        TweenSequenceItem::new(
            Box::new(Tween::new(px(160.), px(80.)).chain(CurveTween::new(Curves::EASE_OUT))),
            1.0,
        ),
        TweenSequenceItem::new(
            Box::new(Tween::new(px(80.), px(220.)).chain(CurveTween::new(Curves::EASE_IN_OUT))),
            2.0,
        ),
    ]);

    section_card(
        "3. TweenSequence — three width segments (1:1:2 weight) with per-segment easing",
        column()
            .gap(px(12.))
            .child(animated(&demo.seq_ctrl, window, cx, move |t| {
                let w = seq_width.transform(t as f64);
                div()
                    .h(px(40.))
                    .w(w)
                    .rounded(px(6.))
                    .bg(hsla(40. / 360., 0.95, 0.55, 1.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(hsla(0., 0., 0.1, 1.0))
                    .text_size(px(12.))
                    .child("Sequence")
            }))
            .child(
                row()
                    .gap(px(8.))
                    .child(play_button("seq-play", "Play", demo.seq_ctrl.clone()))
                    .child(reverse_button("seq-rev", "Reverse", demo.seq_ctrl.clone()))
                    .child(repeat_button("seq-loop", "Loop", demo.seq_ctrl.clone()))
                    .child(stop_button("seq-stop", "Stop", demo.seq_ctrl.clone())),
            ),
    )
}

// ============================================================================
// Section 4 — Controller polish (animate_to / animate_back / fling)
// ============================================================================

fn section_controller_polish(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    // Read live velocity for the readout.
    let velocity = demo.polish_ctrl.read(cx).velocity();

    let polish_for_to_25 = demo.polish_ctrl.clone();
    let polish_for_to_50 = demo.polish_ctrl.clone();
    let polish_for_to_100 = demo.polish_ctrl.clone();
    let polish_for_back = demo.polish_ctrl.clone();
    let polish_for_fling_pos = demo.polish_ctrl.clone();
    let polish_for_fling_neg = demo.polish_ctrl.clone();

    section_card(
        "4. Controller polish — animate_to, animate_back, fling, velocity",
        column()
            .gap(px(12.))
            .child(animated(&demo.polish_ctrl, window, cx, |v| {
                // Map controller value [0..1] to a position bar.
                let bar_width = px(20.0 + 240.0 * v);
                column()
                    .gap(px(6.))
                    .child(div().h(px(28.)).w(bar_width).rounded(px(6.)).bg(hsla(
                        200. / 360.,
                        0.65,
                        0.55,
                        1.0,
                    )))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(hsla(0., 0., 0.75, 1.0))
                            .child(format!("value = {v:.3}")),
                    )
            }))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(hsla(60. / 360., 0.55, 0.7, 1.0))
                    .child(format!("velocity (live) = {velocity:.3} units/sec")),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(
                        FilledButton::new("polish-to-25", "animate_to(0.25)").on_click(
                            move |_, _, cx| {
                                polish_for_to_25.update(cx, |c, cx| {
                                    c.animate_to(0.25, AnimationStyle::default(), cx);
                                });
                            },
                        ),
                    )
                    .child(
                        FilledButton::new("polish-to-50", "animate_to(0.5)").on_click(
                            move |_, _, cx| {
                                polish_for_to_50.update(cx, |c, cx| {
                                    c.animate_to(0.5, AnimationStyle::default(), cx);
                                });
                            },
                        ),
                    )
                    .child(
                        FilledButton::new("polish-to-100", "animate_to(1.0)").on_click(
                            move |_, _, cx| {
                                polish_for_to_100.update(cx, |c, cx| {
                                    c.animate_to(1.0, AnimationStyle::default(), cx);
                                });
                            },
                        ),
                    )
                    .child(
                        OutlinedButton::new("polish-back", "animate_back(0.0)").on_click(
                            move |_, _, cx| {
                                polish_for_back.update(cx, |c, cx| {
                                    c.animate_back(0.0, AnimationStyle::default(), cx);
                                });
                            },
                        ),
                    )
                    .child(TextButton::new("polish-fling-pos", "fling(+2.0)").on_click(
                        move |_, _, cx| {
                            polish_for_fling_pos.update(cx, |c, cx| {
                                c.fling(2.0, AnimationBehavior::default(), cx);
                            });
                        },
                    ))
                    .child(TextButton::new("polish-fling-neg", "fling(-2.0)").on_click(
                        move |_, _, cx| {
                            polish_for_fling_neg.update(cx, |c, cx| {
                                c.fling(-2.0, AnimationBehavior::default(), cx);
                            });
                        },
                    )),
            ),
    )
}

// ============================================================================
// Section 5 — Simulation-driven (SpringSimulation)
// ============================================================================

fn section_simulation(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    let spring_for_to_one = demo.spring_ctrl.clone();
    let spring_for_to_zero = demo.spring_ctrl.clone();
    let spring_for_overshoot = demo.spring_ctrl.clone();

    section_card(
        "5. Simulation-driven — SpringSimulation via animate_with",
        column()
            .gap(px(12.))
            .child(animated(&demo.spring_ctrl, window, cx, |v| {
                let bar = px(20.0 + 240.0 * v);
                div()
                    .h(px(28.))
                    .w(bar)
                    .rounded(px(6.))
                    .bg(hsla(310. / 360., 0.7, 0.6, 1.0))
            }))
            .child(
                row()
                    .gap(px(8.))
                    .child(FilledButton::new("spring-to-1", "Spring → 1.0").on_click(
                        move |_, _, cx| {
                            spring_for_to_one.update(cx, |c, cx| {
                                let spring = SpringDescription::with_damping_ratio(1.0, 100.0, 0.5);
                                c.animate_with(
                                    SpringSimulation::new(spring, c.value(), 1.0, 0.0),
                                    cx,
                                );
                            });
                        },
                    ))
                    .child(OutlinedButton::new("spring-to-0", "Spring → 0.0").on_click(
                        move |_, _, cx| {
                            spring_for_to_zero.update(cx, |c, cx| {
                                let spring = SpringDescription::with_damping_ratio(1.0, 100.0, 0.5);
                                c.animate_with(
                                    SpringSimulation::new(spring, c.value(), 0.0, 0.0),
                                    cx,
                                );
                            });
                        },
                    ))
                    .child(
                        TextButton::new("spring-overshoot", "Underdamped → 1.0").on_click(
                            move |_, _, cx| {
                                spring_for_overshoot.update(cx, |c, cx| {
                                    let spring =
                                        SpringDescription::with_damping_ratio(1.0, 200.0, 0.25);
                                    c.animate_with(
                                        SpringSimulation::new(spring, c.value(), 1.0, 4.0),
                                        cx,
                                    );
                                });
                            },
                        ),
                    ),
            ),
    )
}

// ============================================================================
// Section 6 — Listener Channel + Status state machine
// ============================================================================

fn section_listener_channel(
    demo: &AnimationDemo,
    window: &mut Window,
    cx: &mut Context<AnimationDemo>,
) -> impl IntoElement {
    let ticks = demo.tick_count.get();
    let status_label = match demo.last_status.get() {
        None => "—".to_string(),
        Some(AnimationStatus::Forward) => "Forward".to_string(),
        Some(AnimationStatus::Reverse) => "Reverse".to_string(),
        Some(AnimationStatus::Dismissed) => "Dismissed".to_string(),
        Some(AnimationStatus::Completed) => "Completed".to_string(),
        Some(other) => format!("{other:?}"),
    };
    let status_color = match demo.last_status.get() {
        Some(AnimationStatus::Forward) => hsla(145. / 360., 0.55, 0.55, 1.0),
        Some(AnimationStatus::Reverse) => hsla(40. / 360., 0.95, 0.55, 1.0),
        Some(AnimationStatus::Completed) => hsla(200. / 360., 0.65, 0.55, 1.0),
        Some(AnimationStatus::Dismissed) => hsla(0. / 360., 0.0, 0.6, 1.0),
        _ => hsla(0., 0., 0.5, 1.0),
    };

    section_card(
        "6. Listener Channel — raw add_listener + add_status_listener",
        column()
            .gap(px(12.))
            .child(animated(&demo.listener_ctrl, window, cx, |v| {
                div()
                    .h(px(28.))
                    .w(px(20.0 + 240.0 * v))
                    .rounded(px(6.))
                    .bg(hsla(170. / 360., 0.55, 0.55, 1.0))
            }))
            .child(
                row()
                    .gap(px(16.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(hsla(0., 0., 0.85, 1.0))
                            .child(format!("listener fired {ticks} times")),
                    )
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(status_color)
                            .text_color(hsla(0., 0., 1., 1.0))
                            .text_size(px(12.))
                            .child(format!("status = {status_label}")),
                    ),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(play_button(
                        "lis-fwd",
                        "Forward",
                        demo.listener_ctrl.clone(),
                    ))
                    .child(reverse_button(
                        "lis-rev",
                        "Reverse",
                        demo.listener_ctrl.clone(),
                    ))
                    .child(repeat_button(
                        "lis-loop",
                        "Loop",
                        demo.listener_ctrl.clone(),
                    ))
                    .child(stop_button("lis-stop", "Stop", demo.listener_ctrl.clone())),
            ),
    )
}

// ============================================================================
// Section 7 — CurvedAnimation note
// ============================================================================

fn section_curved_animation_note() -> impl IntoElement {
    note_card(
        "7. CurvedAnimation (decorator)",
        "CurvedAnimation wraps any Rc<dyn Animation<f64>> and applies a Curve \
on top — `controller.curve(c)` is the high-level shortcut for it. Use the \
decorator directly when composing combinators (Reverse / Compound) and \
need separate forward/reverse curves via `with_reverse(...)`. Exercised \
in `crates/flui-core/src/animation/curved_animation.rs` unit tests.",
    )
}

// ============================================================================
// Section 8 — Combinators note
// ============================================================================

fn section_combinators_note() -> impl IntoElement {
    note_card(
        "8. Combinators — ProxyAnimation / ReverseAnimation / Compound / TrainHopping",
        "Combinators take Rc<dyn Animation<f64>> sources and are intended \
for widget-internal composition (route transitions, gesture-driven \
hand-off). They are not designed to consume an Entity-owned \
AnimationController directly — listener-mixin storage is crate-internal. \
See `crates/flui-core/src/animation/combinator.rs` for tests covering \
Proxy parent swap, Reverse value/status flip, animation_min/max/mean, \
and TrainHopping crossover-driven hop.",
    )
}
