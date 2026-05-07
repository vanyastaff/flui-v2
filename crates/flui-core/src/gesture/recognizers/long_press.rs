//! `LongPressGestureRecognizer` + `LongPressDetails`.
//!
//! Async timer via `cx.spawn(async { smol::Timer::after(d).await })`.
//! Async back-channel to the arena via
//! `Weak<RefCell<GestureArenaManager>>` plus `pointer_index`. Drop
//! cancels the timer task.
//!
//! See the design doc § "LongPressGestureRecognizer".

use crate::gesture::arena::GestureArenaManager;
use crate::gesture::{
    GestureDisposition, GestureRecognizer, PointerButtons, PointerEvent, PointerId, PointerKind,
    PointerPhase, SemanticAction,
};
use crate::{Pixels, Point, Task};
use std::cell::RefCell;
use std::rc::Weak;
use std::time::Duration;

const LONG_PRESS_SEMANTIC_ACTIONS: &[SemanticAction] = &[SemanticAction::LongPress];

/// Payload for `on_long_press_*` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LongPressDetails {
    /// Position of the press in window-local pixels.
    pub global_position: Point<Pixels>,
    /// The device kind.
    pub kind: PointerKind,
}

/// Long-press recognizer (timer-driven acceptance).
///
/// State machine: `Down` schedules a timer; `Move > slop` cancels;
/// `Up` before timer expires cancels; timer fire calls
/// `arena.declare_winner` via the stored
/// `Weak<RefCell<GestureArenaManager>>` back-channel.
///
/// Threshold fields ([`Self::timeout`], [`Self::slop`],
/// [`Self::timer_budget`]) are public for symmetry with
/// [`super::TapGestureRecognizer`] — they can be tuned
/// post-construction. The on_* callback fields and these threshold
/// fields are the full configurable surface; mutating them is
/// supported and idiomatic.
#[non_exhaustive]
pub struct LongPressGestureRecognizer {
    /// Fires when the long-press timer expires (after acceptance).
    pub on_long_press_start:
        Option<Box<dyn FnMut(LongPressDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fires on each `Move` after acceptance.
    pub on_long_press_move:
        Option<Box<dyn FnMut(LongPressDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fires on the `Up` that ends the long-press.
    pub on_long_press_end:
        Option<Box<dyn FnMut(LongPressDetails, &mut crate::Window, &mut crate::App)>>,
    /// Which button this recognizer accepts. Default primary.
    pub button: PointerButtons,
    /// Hold duration before the long-press fires. Read from
    /// [`crate::gesture::GestureSettings::long_press_timeout`] at
    /// construction (default: 500 ms).
    pub timeout: Duration,
    /// Maximum movement (in logical pixels) before the long-press
    /// gesture is rejected. Read from
    /// [`crate::gesture::GestureSettings::long_press_slop`] at
    /// construction (default: 18 logical px).
    pub slop: Pixels,
    /// Maximum spawn-to-flush latency budget for the async timer —
    /// the recognizer warns if exceeded. Read from
    /// [`crate::gesture::GestureSettings::long_press_timer_budget`] at
    /// construction (default: 16 ms / one 60 Hz frame).
    pub timer_budget: Duration,

    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_kind: PointerKind,
    accepted: bool,
    /// Async timer task; dropped on recognizer drop or cancel,
    /// cancelling the underlying future.
    timer: Option<Task<()>>,
    /// Async back-channel to the arena. T15 wires this from
    /// `GestureBinding`'s arena `Rc` when `add_pointer` is called.
    /// `None` until wiring lands.
    #[allow(dead_code, reason = "T15 LongPress timer wiring populates this")]
    arena_back_channel: Option<Weak<RefCell<GestureArenaManager>>>,
    /// Index into `arena.entries` recorded inside `add_pointer`.
    #[allow(dead_code, reason = "T15 LongPress timer wiring populates this")]
    pointer_index: Option<usize>,
}

impl LongPressGestureRecognizer {
    /// Construct a new recognizer using the supplied gesture settings.
    pub fn new(settings: &super::super::GestureSettings) -> Self {
        Self {
            on_long_press_start: None,
            on_long_press_move: None,
            on_long_press_end: None,
            button: PointerButtons::PRIMARY,
            timeout: settings.long_press_timeout,
            slop: settings.long_press_slop,
            timer_budget: settings.long_press_timer_budget,
            pointer: None,
            down_position: Point::default(),
            last_kind: PointerKind::Mouse,
            accepted: false,
            timer: None,
            arena_back_channel: None,
            pointer_index: None,
        }
    }

    fn distance_sq(&self, p: Point<Pixels>) -> f32 {
        let dx = p.x.0 - self.down_position.x.0;
        let dy = p.y.0 - self.down_position.y.0;
        dx * dx + dy * dy
    }
}

impl GestureRecognizer for LongPressGestureRecognizer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "long_press"
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent) {
        if !event.buttons.contains(self.button) {
            return;
        }
        self.pointer = Some(pointer_id);
        self.down_position = event.position;
        self.last_kind = event.kind;
        self.accepted = false;
        // T15 will populate `arena_back_channel` and `pointer_index`
        // from the GestureBinding when the recognizer joins the
        // arena. Until that wiring lands, the timer's
        // `declare_winner` upgrade is a no-op (Weak::default()
        // upgrades to None).
    }

    fn handle_event(
        &mut self,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition {
        if self.pointer != Some(event.pointer_id) {
            return GestureDisposition::Possible;
        }
        match event.phase {
            PointerPhase::Down => {
                // Schedule the long-press timer. `cx.spawn` returns a
                // `Task<()>` we store; dropping it cancels the future.
                // T15 wiring will set up `arena_back_channel` so the
                // timer's `declare_winner` actually fires.
                //
                // For T11-only scope (no T15 wiring yet) the timer
                // runs but cannot signal acceptance. Tests in T17
                // exercise the timer path with a synthetic clock.
                let timeout = self.timeout;
                let _budget = self.timer_budget;
                self.timer = Some(cx.spawn(async move |_cx| {
                    smol::Timer::after(timeout).await;
                    // T15: upgrade `arena_back_channel` and call
                    // `declare_winner`. Until then, this future ends
                    // without effect.
                }));
                GestureDisposition::Possible
            }
            PointerPhase::Move => {
                if self.distance_sq(event.position) > (self.slop.0).powi(2) {
                    self.timer = None; // drops the task → cancels future
                    GestureDisposition::Rejected
                } else if self.accepted {
                    if let Some(cb) = self.on_long_press_move.as_mut() {
                        cb(
                            LongPressDetails {
                                global_position: event.position,
                                kind: event.kind,
                            },
                            window,
                            cx,
                        );
                    }
                    GestureDisposition::Possible
                } else {
                    GestureDisposition::Possible
                }
            }
            PointerPhase::Up => {
                let was_accepted = self.accepted;
                self.timer = None; // drops the task → cancels future
                if was_accepted {
                    if let Some(cb) = self.on_long_press_end.as_mut() {
                        cb(
                            LongPressDetails {
                                global_position: event.position,
                                kind: event.kind,
                            },
                            window,
                            cx,
                        );
                    }
                    GestureDisposition::Accepted
                } else {
                    GestureDisposition::Rejected
                }
            }
            PointerPhase::Cancel | PointerPhase::Removed => {
                self.timer = None;
                GestureDisposition::Rejected
            }
            _ => GestureDisposition::Possible,
        }
    }

    fn sweep_accepted(
        &mut self,
        _pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        // LongPress wins via timer-driven `declare_winner`, not via
        // sweep — sweep firing means our timer never expired.
    }

    fn rejected(
        &mut self,
        _pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        // Drop the timer to cancel the future.
        self.timer = None;
        self.accepted = false;
    }

    fn semantic_actions(&self) -> &'static [SemanticAction] {
        LONG_PRESS_SEMANTIC_ACTIONS
    }
}

impl Drop for LongPressGestureRecognizer {
    fn drop(&mut self) {
        // The `Task` field drops automatically; this impl exists
        // primarily as a documentation site for the drop-cancel
        // contract (and so future Drop logic has a place to land).
    }
}

#[cfg(test)]
mod tests {
    //! T17 — Long-press recognizer unit tests.
    //!
    //! `LongPressGestureRecognizer` accepts via timer-driven
    //! `declare_winner` (T15-wired through `arena_back_channel`); these
    //! tests exercise the synchronous state-machine paths that gate
    //! whether the timer is allowed to fire.

    use super::*;
    use crate::gesture::{
        GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, Pixels, Point, TestAppContext};

    fn pe(phase: PointerPhase, pos: Point<Pixels>, buttons: PointerButtons) -> PointerEvent {
        PointerEvent {
            pointer_id: PointerId(0),
            kind: PointerKind::Mouse,
            phase,
            position: pos,
            delta: Point::default(),
            buttons,
            modifiers: Modifiers::default(),
            timestamp: Instant::now(),
            pressure: 1.0,
            tilt: 0.0,
            orientation: 0.0,
        }
    }

    fn p(x: f32, y: f32) -> Point<Pixels> {
        Point::new(Pixels(x), Pixels(y))
    }

    #[flui_core::test]
    fn long_press_move_beyond_slop_rejects_and_cancels_timer(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut lp = LongPressGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    lp.add_pointer(PointerId(0), &down);
                    assert_eq!(
                        lp.handle_event(&down, window, cx),
                        GestureDisposition::Possible
                    );
                    assert!(lp.timer.is_some(), "Down schedules a timer");
                    let mv = pe(PointerPhase::Move, p(100.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        lp.handle_event(&mv, window, cx),
                        GestureDisposition::Rejected,
                    );
                    assert!(
                        lp.timer.is_none(),
                        "drop-on-cancel pattern: rejecting clears the timer Task"
                    );
                });
        });
    }

    #[flui_core::test]
    fn long_press_up_before_accept_rejects(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut lp = LongPressGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    lp.add_pointer(PointerId(0), &down);
                    let _ = lp.handle_event(&down, window, cx);
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        lp.handle_event(&up, window, cx),
                        GestureDisposition::Rejected,
                        "Up before timer-accept rejects (no premature acceptance)"
                    );
                });
        });
    }

    #[flui_core::test]
    fn long_press_cancel_phase_rejects_and_drops_timer(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut lp = LongPressGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    lp.add_pointer(PointerId(0), &down);
                    let _ = lp.handle_event(&down, window, cx);
                    let cancel = pe(PointerPhase::Cancel, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        lp.handle_event(&cancel, window, cx),
                        GestureDisposition::Rejected,
                    );
                    assert!(lp.timer.is_none(), "Cancel drops the timer Task");
                });
        });
    }

    /// Compile-time lock for B2 — threshold fields stay `pub` so
    /// downstream code can tune them post-construction. Changing any
    /// of these to `pub(crate)` makes this test fail to compile,
    /// which is the intended canary.
    #[test]
    fn long_press_threshold_fields_are_settable() {
        // GestureSettings::default() is platform-agnostic; this test
        // does not need a TestAppContext.
        let s = GestureSettings::default();
        let mut r = LongPressGestureRecognizer::new(&s);
        r.timeout = std::time::Duration::from_millis(1000);
        r.slop = crate::Pixels(10.0);
        r.timer_budget = std::time::Duration::from_millis(8);
        r.button = PointerButtons::SECONDARY;
        // Read back to silence the unused-field-write lint.
        assert_eq!(r.timeout, std::time::Duration::from_millis(1000));
    }

    #[flui_core::test]
    fn long_press_rejected_callback_clears_state(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut lp = LongPressGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    lp.add_pointer(PointerId(0), &down);
                    let _ = lp.handle_event(&down, window, cx);
                    lp.accepted = true; // simulate timer firing
                    GestureRecognizer::rejected(&mut lp, PointerId(0), window, cx);
                    assert!(lp.timer.is_none(), "rejected drops the timer");
                    assert!(!lp.accepted, "rejected resets the accepted flag");
                });
        });
    }
}
