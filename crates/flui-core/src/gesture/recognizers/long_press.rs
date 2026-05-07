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
    pub(crate) timeout: Duration,
    pub(crate) slop: Pixels,
    pub(crate) timer_budget: Duration,

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
    arena_back_channel: Option<Weak<RefCell<GestureArenaManager>>>,
    /// Index into `arena.entries` recorded inside `add_pointer`.
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
