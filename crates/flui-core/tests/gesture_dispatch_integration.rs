//! S07.5 T10 — End-to-end integration test for the gesture dispatch
//! pipeline.
//!
//! Each test paints a `div()` with a fluent recognizer builder
//! (`on_tap`, `on_pan_start`, `on_long_press_*`, `on_double_tap`) and
//! drives raw `MouseDownEvent` / `MouseUpEvent` / `MouseMoveEvent`
//! sequences through `Window::dispatch_event` via the
//! `VisualTestContext::simulate_*` helpers. Every test asserts that
//! the recognizer's user-visible callback fires, locking the
//! paint → `pending_recognizers` → `register_recognizer` →
//! `arena.dispatch` → callback chain end-to-end.
//!
//! Cfg-gated on the `test-support` feature so workspace builds without
//! it stay clean (the integration `tests/` target inherits cargo's
//! default features but explicitly listing the cfg makes the
//! dependency on `TestAppContext` and the `simulate_*` helpers
//! unambiguous).
//!
//! Cover matrix (one test per S07-public recognizer family):
//! - `on_tap` — locks the eager-accept path on `Up`.
//! - `on_pan_*` (start) — locks the slop-crossing acceptance.
//! - `on_long_press_start` — locks the timer-driven back-channel
//!   (S07.5 T5) plus the `RecognizerLifecycle::set_arena_back_channel`
//!   wiring (S07.5 T4).
//! - `on_double_tap` — locks `arena.hold` / scheduled release (S07.5
//!   T6) plus the second-tap acceptance path.

#![cfg(feature = "test-support")]

use flui_core::{
    self as flui_core, Context, IntoElement, Modifiers, Point, Render, Styled, TestAppContext,
    Window, div, prelude::*, px,
};
use std::cell::Cell;
use std::rc::Rc;

/// A view that paints a single full-window `div` configured by the
/// caller-supplied closure. Used as the root view in every integration
/// test below; the closure runs at `render` time and is the place to
/// hang fluent gesture builders. Keeping the element type as `Div`
/// (not `Stateful<Div>`) avoids forcing every test to pick an `id`,
/// since the gesture recognizers under test all live on
/// `InteractiveElement` (no stateful identity required).
struct GestureTestView {
    builder: Box<
        dyn FnMut(flui_core::Div, &mut Window, &mut Context<GestureTestView>) -> flui_core::Div,
    >,
}

impl GestureTestView {
    fn new<F>(builder: F) -> Self
    where
        F: FnMut(flui_core::Div, &mut Window, &mut Context<GestureTestView>) -> flui_core::Div
            + 'static,
    {
        Self {
            builder: Box::new(builder),
        }
    }
}

impl Render for GestureTestView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let element = div().size_full();
        (self.builder)(element, window, cx)
    }
}

#[flui_core::test]
fn on_tap_callback_fires_through_dispatch(cx: &mut TestAppContext) {
    let fired = Rc::new(Cell::new(0u32));
    let fired_for_view = fired.clone();
    let (_view, cx) = cx.add_window_view(move |_window, _cx| {
        GestureTestView::new(move |element, _window, _cx| {
            let f = fired_for_view.clone();
            element.on_tap(move |_, _, _| {
                f.set(f.get() + 1);
            })
        })
    });
    cx.simulate_click(Point::new(px(20.0), px(20.0)), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        fired.get(),
        1,
        "on_tap must fire exactly once for a single Down/Up sequence \
         (paint → pending_recognizers → register_recognizer → arena.dispatch → callback)"
    );
}

#[flui_core::test]
fn on_pan_start_fires_on_slop_crossing(cx: &mut TestAppContext) {
    let started = Rc::new(Cell::new(0u32));
    let started_for_view = started.clone();
    let (_view, cx) = cx.add_window_view(move |_window, _cx| {
        GestureTestView::new(move |element, _window, _cx| {
            let s = started_for_view.clone();
            element.on_pan_start(move |_, _, _| {
                s.set(s.get() + 1);
            })
        })
    });
    cx.simulate_mouse_down(
        Point::new(px(20.0), px(20.0)),
        flui_core::MouseButton::Left,
        Modifiers::default(),
    );
    // First Move below slop — must not fire.
    cx.simulate_mouse_move(
        Point::new(px(25.0), px(22.0)),
        flui_core::MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(
        started.get(),
        0,
        "on_pan_start must NOT fire below pan_slop"
    );
    // Second Move clearly above 18 px slop on x-axis.
    cx.simulate_mouse_move(
        Point::new(px(80.0), px(20.0)),
        flui_core::MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    assert_eq!(
        started.get(),
        1,
        "on_pan_start must fire exactly once on slop crossing"
    );
}

#[flui_core::test]
fn on_long_press_start_fires_through_back_channel(cx: &mut TestAppContext) {
    use std::time::Duration;

    let started = Rc::new(Cell::new(0u32));
    let started_for_view = started.clone();
    let (_view, cx) = cx.add_window_view(move |window, _cx| {
        // Tighten the long-press timeout so the test does not stall
        // for half a second. This also locks the per-window settings
        // flow (S07.5 T9) — `register_recognizer` reads
        // `window.gesture_settings_mut()` at registration time, so
        // overrides set here actually take effect for the freshly
        // built recognizer.
        window.gesture_settings_mut().long_press_timeout = Duration::from_millis(20);
        GestureTestView::new(move |element, _window, _cx| {
            let s = started_for_view.clone();
            element.on_long_press_start(move |_, _, _| {
                s.set(s.get() + 1);
            })
        })
    });
    cx.simulate_mouse_down(
        Point::new(px(20.0), px(20.0)),
        flui_core::MouseButton::Left,
        Modifiers::default(),
    );
    // Drive the executor past the 20 ms timeout — the timer task
    // upgrades the back-channel and fires `declare_winner`, which
    // sets `accepted = true` and dispatches `on_long_press_start`
    // through the `update_window` path. Pump the executor twice
    // because the spawn → smol::Timer → update_window → callback
    // chain crosses two `await` points before the user closure runs.
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(5));
    cx.run_until_parked();
    assert_eq!(
        started.get(),
        1,
        "on_long_press_start must fire exactly once after the timer \
         expires (S07.5 T5 — arena_back_channel wiring)"
    );
}

#[flui_core::test]
fn on_double_tap_fires_through_arena_hold(cx: &mut TestAppContext) {
    use std::time::Duration;
    let fired = Rc::new(Cell::new(0u32));
    let fired_for_view = fired.clone();
    let (_view, cx) = cx.add_window_view(move |window, _cx| {
        // Loosen the timing window: `simulate_click` fires Down/Up
        // back-to-back at ~µs intervals, well below the default
        // 40 ms `double_tap_min_time` debounce. Setting it to 0 lets
        // the back-to-back synthetic clicks pass the gate. Locks the
        // S07.5 T9 per-window settings flow at the same time
        // (`register_recognizer` reads these overrides at registration).
        window.gesture_settings_mut().double_tap_min_time = Duration::from_millis(0);
        window.gesture_settings_mut().double_tap_timeout = Duration::from_secs(1);
        GestureTestView::new(move |element, _window, _cx| {
            let f = fired_for_view.clone();
            element.on_double_tap(move |_, _, _| {
                f.set(f.get() + 1);
            })
        })
    });
    // First click — registers the DoubleTap recognizer, opens the
    // arena, and (via S07.5 T6) the dispatcher calls `arena.hold`
    // because DoubleTap returns `true` from `needs_arena_hold`.
    cx.simulate_click(Point::new(px(20.0), px(20.0)), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        fired.get(),
        0,
        "single click must not fire on_double_tap (held arena waits for the second tap)"
    );
    // Second click within the configured `double_tap_timeout`.
    cx.simulate_click(Point::new(px(20.0), px(20.0)), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        fired.get(),
        1,
        "two consecutive clicks must trigger on_double_tap exactly once \
         (S07.5 T6 — arena.hold / release wiring)"
    );
}
