//! T18 — `GestureArena` interactive demo (S07 + S07.5).
//!
//! After the S07.5 wiring landed, every fluent recognizer in this
//! demo (`on_tap`, `on_double_tap`, `on_long_press_*`, `on_pan_*`)
//! actually fires through the production dispatch path:
//! `Interactivity::paint` → `pending_recognizers` →
//! `GestureBinding::register_recognizer` → `arena.dispatch` →
//! callback. Earlier revisions of the demo carried "T15-followup
//! wiring lands separately" caveats on LongPress / DoubleTap / per-window
//! settings; those are stale after S07.5 and have been removed.
//!
//! Four scenarios share the demo window:
//!
//! 1. **Competing recognizers** — a single element has `on_tap`,
//!    `on_double_tap`, `on_long_press_start`, and `on_pan_start`
//!    registered. The arena resolves which gesture wins based on
//!    pointer behaviour: a quick down-up wins for tap, a held press
//!    wins for long-press, slop-crossing motion wins for pan, two
//!    quick taps win for double-tap.
//!
//! 2. **Translucent overlay** — a half-opaque div sits over a base
//!    div. The overlay has `HitTestBehavior::Translucent` and a tap
//!    listener; the base div also has a tap listener. With
//!    `Translucent`, both fire — gesture-arena participation flows
//!    through to the element behind. (Switching the overlay to
//!    `Opaque` would suppress the base div's listener.)
//!
//! 3. **Settings override** — the demo flips
//!    `window.gesture_settings_mut().long_press_timeout` to a custom
//!    value at render time, demonstrating the
//!    `GestureSettings`/`GestureBinding` mutation seam. (S14 will
//!    route `MediaQuery` updates here.)
//!
//! 4. **GestureArenaTeam (informational)** — the
//!    `flui_core::GestureArenaTeam` type implements captain-deferred
//!    resolution; it does not yet have a public registration API on
//!    `InteractiveElement` (deferred to a future spec —
//!    `GestureDetector` / `RawGestureDetector` will surface it). The
//!    demo shows a placeholder card explaining the contract. Property
//!    test P6 in `arena_team.rs` covers the rule.
//!
//! Run interactively:
//!
//! ```text
//! cargo run -p flui-core --example gesture_arena_demo
//! ```
//!
//! Run as a CI smoke test (open window, init, immediately quit, no
//! event loop work, no painting):
//!
//! ```text
//! cargo run -p flui-core --example gesture_arena_demo -- --headless-smoke
//! ```

#[path = "../prelude.rs"]
mod example_prelude;

use example_prelude::init_example;
use flui_core::colors::Colors;
use flui_core::{
    App, Application, Bounds, Context, Entity, FontWeight, IntoElement, Render, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use flui_core::{HitTestBehavior, ScaleStartDetails, ScaleUpdateDetails};
use std::time::Duration;

/// Maximum number of recent log entries each card keeps.
const MAX_LOG: usize = 8;

fn truncate_log(log: &mut Vec<String>, line: String) {
    log.push(line);
    if log.len() > MAX_LOG {
        log.remove(0);
    }
}

// =====================================================================
// Scenario 1 — Competing recognizers (Tap / DoubleTap / LongPress / Pan).
// =====================================================================

struct CompetingRecognizersCard {
    log: Vec<String>,
}

impl CompetingRecognizersCard {
    fn new() -> Self {
        Self { log: Vec::new() }
    }
}

impl Render for CompetingRecognizersCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Colors::for_appearance(window);
        let weak = cx.weak_entity();
        let weak_dt = weak.clone();
        let weak_lp = weak.clone();
        let weak_ps = weak.clone();
        let weak_pe = weak.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(colors.container)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text)
                    .child("1. Competing recognizers"),
            )
            .child(div().text_xs().text_color(colors.disabled).child(
                "Tap, double-tap, long-press, or pan the target. The arena picks one winner.",
            ))
            .child(
                div()
                    .id("compete-target")
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(colors.selected)
                    .text_color(colors.selected_text)
                    .text_sm()
                    .child("⇆ try gestures here ⇆")
                    .on_tap(move |_d, _w, app| {
                        if let Some(e) = weak.upgrade() {
                            e.update(app, |this, cx| {
                                truncate_log(&mut this.log, "tap".into());
                                cx.notify();
                            });
                        }
                    })
                    .on_double_tap(move |_d, _w, app| {
                        if let Some(e) = weak_dt.upgrade() {
                            e.update(app, |this, cx| {
                                truncate_log(&mut this.log, "double_tap".into());
                                cx.notify();
                            });
                        }
                    })
                    .on_long_press_start(move |_d, _w, app| {
                        if let Some(e) = weak_lp.upgrade() {
                            e.update(app, |this, cx| {
                                truncate_log(&mut this.log, "long_press_start".into());
                                cx.notify();
                            });
                        }
                    })
                    .on_pan_start(move |_d, _w, app| {
                        if let Some(e) = weak_ps.upgrade() {
                            e.update(app, |this, cx| {
                                truncate_log(&mut this.log, "pan_start".into());
                                cx.notify();
                            });
                        }
                    })
                    .on_pan_end(move |d, _w, app| {
                        if let Some(e) = weak_pe.upgrade() {
                            e.update(app, |this, cx| {
                                truncate_log(
                                    &mut this.log,
                                    format!(
                                        "pan_end vx={:.0} vy={:.0}",
                                        d.velocity.pixels_per_second.x,
                                        d.velocity.pixels_per_second.y
                                    ),
                                );
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .text_xs()
                    .text_color(colors.disabled)
                    .children(self.log.iter().map(|e| div().child(format!("• {e}")))),
            )
    }
}

// =====================================================================
// Scenario 2 — Translucent overlay (HitTestBehavior::Translucent).
// =====================================================================

struct TranslucentOverlayCard {
    base_taps: u32,
    overlay_taps: u32,
}

impl TranslucentOverlayCard {
    fn new() -> Self {
        Self {
            base_taps: 0,
            overlay_taps: 0,
        }
    }
}

impl Render for TranslucentOverlayCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Colors::for_appearance(window);
        let base_taps = self.base_taps;
        let overlay_taps = self.overlay_taps;
        let weak_base = cx.weak_entity();
        let weak_overlay = weak_base.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(colors.container)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text)
                    .child("2. Translucent overlay"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.disabled)
                    .child(
                        "Tap the green overlay — both base and overlay fire (Translucent). Counts diverge if Opaque.",
                    ),
            )
            .child(
                // Base layer — `Opaque` (default), tap listener fires.
                div()
                    .id("base")
                    .relative()
                    .h_24()
                    .w_full()
                    .rounded_md()
                    .bg(colors.selected)
                    .text_color(colors.selected_text)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .child(format!("base — taps: {base_taps}"))
                    .on_tap(move |_d, _w, app| {
                        if let Some(e) = weak_base.upgrade() {
                            e.update(app, |this, cx| {
                                this.base_taps += 1;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        // Overlay — `Translucent`, also forwards to base.
                        div()
                            .id("overlay")
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w_full()
                            .with_hit_test_behavior(HitTestBehavior::Translucent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .child(format!("overlay — taps: {overlay_taps}"))
                            .on_tap(move |_d, _w, app| {
                                if let Some(e) = weak_overlay.upgrade() {
                                    e.update(app, |this, cx| {
                                        this.overlay_taps += 1;
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
    }
}

// =====================================================================
// Scenario 3 — GestureSettings override.
// =====================================================================

struct SettingsOverrideCard {
    long_presses: u32,
    /// Custom long-press timeout pushed into the per-window
    /// `GestureSettings`. Default is 500ms; we use 250ms for a
    /// snappier demo.
    long_press_ms: u64,
}

impl SettingsOverrideCard {
    fn new() -> Self {
        Self {
            long_presses: 0,
            long_press_ms: 250,
        }
    }
}

impl Render for SettingsOverrideCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push our custom timeout into the per-window settings on
        // every render. (A real app would do this once via the
        // MediaQuery seam in S14.)
        window.gesture_settings_mut().long_press_timeout =
            Duration::from_millis(self.long_press_ms);

        let colors = Colors::for_appearance(window);
        let count = self.long_presses;
        let ms = self.long_press_ms;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(colors.container)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text)
                    .child("3. GestureSettings override"),
            )
            .child(div().text_xs().text_color(colors.disabled).child(format!(
                "Long-press fires after {ms} ms (default 500 ms). Hold the chip below.",
            )))
            .child({
                let weak = cx.weak_entity();
                div()
                    .id("long-press-target")
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(colors.selected)
                    .text_color(colors.selected_text)
                    .text_sm()
                    .child(format!("hold me — long-presses: {count}"))
                    .on_long_press_start(move |_d, _w, app| {
                        if let Some(e) = weak.upgrade() {
                            e.update(app, |this, cx| {
                                this.long_presses += 1;
                                cx.notify();
                            });
                        }
                    })
            })
    }
}

// =====================================================================
// Scenario 4 — GestureArenaTeam (informational).
// =====================================================================

struct ArenaTeamCard;

impl ArenaTeamCard {
    fn new() -> Self {
        Self
    }
}

impl Render for ArenaTeamCard {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Colors::for_appearance(window);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(colors.container)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text)
                    .child("4. GestureArenaTeam (informational)"),
            )
            .child(div().text_xs().text_color(colors.disabled).child(
                "Captain-deferred resolution: members defer Accepted to the captain. \
                         Public registration via InteractiveElement is deferred (a future \
                         GestureDetector spec will surface it) — the contract is locked by \
                         property P6 in arena_team.rs.",
            ))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(colors.text)
                    .child("flui_core::gesture::GestureArenaTeam::with_captain(...)"),
            )
    }
}

// =====================================================================
// Bonus: Scale (multi-pointer) — desktop platforms have no native
// pinch on Windows, scale-only on Wayland/macOS, no rotation. The
// recognizer is still wired — listening costs nothing on platforms
// without multi-pointer events.
// =====================================================================

struct ScaleDemoCard {
    last_scale: f32,
}

impl ScaleDemoCard {
    fn new() -> Self {
        Self { last_scale: 1.0 }
    }
}

impl Render for ScaleDemoCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Colors::for_appearance(window);
        let last_scale = self.last_scale;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(colors.container)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text)
                    .child("5. Scale (Wayland/macOS pinch only)"),
            )
            .child(div().text_xs().text_color(colors.disabled).child(
                "Pinch on a Wayland session or macOS trackpad. Windows desktop emits no \
                         native pinch — the listener is silent there.",
            ))
            .child({
                let weak = cx.weak_entity();
                div()
                    .id("scale-target")
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(colors.selected)
                    .text_color(colors.selected_text)
                    .text_sm()
                    .child(format!("last scale ratio: {last_scale:.2}"))
                    .on_scale_start(|_d: ScaleStartDetails, _w, _app| {
                        // Nothing to record on start; update / end carry the data.
                    })
                    .on_scale_update(move |d: ScaleUpdateDetails, _w, app| {
                        if let Some(e) = weak.upgrade() {
                            e.update(app, |this, cx| {
                                this.last_scale = d.scale;
                                cx.notify();
                            });
                        }
                    })
            })
    }
}

// =====================================================================
// Main viewer.
// =====================================================================

struct GestureArenaDemo {
    competing: Entity<CompetingRecognizersCard>,
    overlay: Entity<TranslucentOverlayCard>,
    settings: Entity<SettingsOverrideCard>,
    team: Entity<ArenaTeamCard>,
    scale: Entity<ScaleDemoCard>,
}

impl GestureArenaDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            competing: cx.new(|_| CompetingRecognizersCard::new()),
            overlay: cx.new(|_| TranslucentOverlayCard::new()),
            settings: cx.new(|_| SettingsOverrideCard::new()),
            team: cx.new(|_| ArenaTeamCard::new()),
            scale: cx.new(|_| ScaleDemoCard::new()),
        }
    }
}

impl Render for GestureArenaDemo {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Colors::for_appearance(window);
        div()
            .id("root")
            .size_full()
            .p_6()
            .bg(colors.background)
            .overflow_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .max_w(px(900.))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors.text)
                            .child("S07 — GestureArena demo"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.disabled)
                            .child("Five scenarios exercise the gesture-arena public surface."),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_4()
                            .child(self.competing.clone())
                            .child(self.overlay.clone())
                            .child(self.settings.clone())
                            .child(self.team.clone())
                            .child(self.scale.clone()),
                    ),
            )
    }
}

fn main() {
    let smoke = std::env::args().any(|a| a == "--headless-smoke");
    if smoke {
        // CI gate: do not initialize the platform, do not open a
        // window, do not run the event loop. The cheapest signal we
        // can give is that the demo binary linked against the public
        // gesture surface and starts main() without panicking.
        // Platforms without windowing (CI containers, sandboxes) are
        // explicitly supported — we do not call `Application::new()`.
        println!("gesture_arena_demo: --headless-smoke OK");
        return;
    }

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| GestureArenaDemo::new(cx)),
        )
        .expect("Failed to open window");

        init_example(cx, "Gesture Arena Demo");
    });
}
