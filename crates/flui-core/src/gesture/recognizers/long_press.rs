//! `LongPressGestureRecognizer` + `LongPressDetails`.
//!
//! Async timer via `cx.spawn(async { smol::Timer::after(d).await })`.
//! Async back-channel to the arena via
//! `Weak<RefCell<GestureArenaManager>>` plus `pointer_index`. Drop
//! cancels the timer task.
//!
//! See the design doc § "LongPressGestureRecognizer".

use crate::Modifiers;
use crate::gesture::arena::ArenaBackChannel;
use crate::gesture::{
    AllowedButtonsFilter, DeliveredEvent, GestureDisposition, GestureRecognizer, GestureSettings,
    PointerButtons, PointerId, PointerKind, PointerPhase, RecognizerLifecycle, SemanticAction,
};
use crate::{AppContext, Pixels, Point, Task};
use smallvec::SmallVec;
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
    /// Optional `(buttons, modifiers) -> bool` predicate evaluated by
    /// [`crate::gesture::GestureBinding::register_recognizer`] before
    /// the recognizer joins the arena. `None` (the default) admits
    /// every event whose `buttons` contain [`Self::button`].
    pub allowed_buttons_filter: Option<AllowedButtonsFilter>,

    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_kind: PointerKind,
    accepted: bool,
    /// Async timer task; dropped on recognizer drop or cancel,
    /// cancelling the underlying future.
    timer: Option<Task<()>>,
    /// Async back-channel to the arena. Populated at registration time
    /// by `GestureBinding::register_recognizer` via the
    /// [`RecognizerLifecycle::set_arena_back_channel`] hook. Defaults
    /// to [`ArenaBackChannel::empty`] so a recognizer constructed
    /// outside the binding (e.g. directly in unit tests) silently
    /// no-ops the timer's `declare_winner` call instead of panicking.
    arena_back_channel: ArenaBackChannel,
    /// Per-pointer arena entry slots recorded at registration time.
    ///
    /// Single-shot LongPress holds at most one pointer in flight in
    /// practice, so the inline storage of `1` covers the common case
    /// without a heap allocation. Multi-pointer recognizers built on
    /// the same back-channel hook (S07.6 MultiTap) carry the same
    /// shape with a larger inline budget.
    pointer_indexes: SmallVec<[(PointerId, usize); 1]>,
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
            allowed_buttons_filter: None,
            pointer: None,
            down_position: Point::default(),
            last_kind: PointerKind::Mouse,
            accepted: false,
            timer: None,
            arena_back_channel: ArenaBackChannel::empty(),
            pointer_indexes: SmallVec::new(),
        }
    }

    /// Fluent setter for [`Self::allowed_buttons_filter`]. The closure
    /// is evaluated by [`crate::gesture::GestureBinding::register_recognizer`]
    /// at registration time; on `false` the recognizer never enters
    /// the arena (Decision D10).
    pub fn with_allowed_buttons_filter(
        mut self,
        f: impl Fn(PointerButtons, Modifiers) -> bool + 'static,
    ) -> Self {
        self.allowed_buttons_filter = Some(AllowedButtonsFilter::new(f));
        self
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

    fn allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter> {
        self.allowed_buttons_filter.as_ref()
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: DeliveredEvent<'_>) {
        if !event.buttons().contains(self.button) {
            return;
        }
        self.pointer = Some(pointer_id);
        self.down_position = event.local_position;
        self.last_kind = event.kind();
        self.accepted = false;
        // T15 will populate `arena_back_channel` and `pointer_index`
        // from the GestureBinding when the recognizer joins the
        // arena. Until that wiring lands, the timer's
        // `declare_winner` upgrade is a no-op (Weak::default()
        // upgrades to None).
    }

    fn handle_event(
        &mut self,
        event: DeliveredEvent<'_>,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition {
        if self.pointer != Some(event.pointer_id()) {
            return GestureDisposition::Possible;
        }
        match event.phase() {
            PointerPhase::Down => {
                // Schedule the long-press timer. `cx.spawn` returns a
                // `Task<()>` we store; dropping it cancels the future.
                //
                // S07.5 T5: the timer upgrades the per-window arena
                // back-channel injected at registration time
                // (`set_arena_back_channel`), looks up its own
                // `Rc<RefCell<...>>` from the arena to fire the user
                // callback, and declares itself the winner. The
                // back-channel is `Weak`, so a window-tear-down race
                // (timer fires after `Window` drops) becomes a no-op
                // upgrade.
                let timeout = self.timeout;
                let pointer_id = event.pointer_id();
                // Callback global_position uses window-local because
                // user code expects window-space coordinates here.
                let entry_position = event.global_position();
                let entry_kind = event.kind();
                let back_channel = self.arena_back_channel.clone();
                // Look up this pointer's entry slot recorded during
                // `set_arena_back_channel`. Multi-pointer recognizers
                // share the back-channel hook surface, so the lookup
                // is keyed on `pointer_id` (the matching slot is
                // `(pid, idx)`).
                let entry_index = self
                    .pointer_indexes
                    .iter()
                    .find(|(pid, _)| *pid == pointer_id)
                    .map(|(_, idx)| *idx);
                let window_handle = window.window_handle();
                // S07.5 T5 — use `BackgroundExecutor::timer` so the
                // test harness's virtual clock (driven by
                // `cx.executor().advance_clock`) wakes the timer.
                // `smol::Timer::after` would observe wall-clock time
                // and never fire under `advance_clock`.
                let timer_future = cx.background_executor().timer(timeout);
                self.timer = Some(cx.spawn(async move |async_cx| {
                    timer_future.await;
                    let Some(idx) = entry_index else {
                        log::trace!(
                            target: "flui::gesture::long_press",
                            recognizer = "long_press",
                            phase = "timer_fire",
                            lifecycle = "cancel";
                            "long_press timer fired without entry index (registration not via GestureBinding)"
                        );
                        return;
                    };
                    let Some(arena_rc) = back_channel.upgrade() else {
                        log::trace!(
                            target: "flui::gesture::long_press",
                            recognizer = "long_press",
                            phase = "timer_fire",
                            lifecycle = "cancel";
                            "back-channel upgrade returned None (window dropped)"
                        );
                        return;
                    };
                    let _ = async_cx.update_window(window_handle, |_, window, cx| {
                        log::debug!(
                            target: "flui::gesture::long_press",
                            recognizer = "long_press",
                            phase = "timer_fire",
                            lifecycle = "accept",
                            pointer_id = format!("{:?}", pointer_id);
                            "long_press timer fired; declaring winner via back-channel"
                        );
                        // Snapshot the recognizer's Rc inside a
                        // short-lived borrow so we can release the
                        // arena borrow before calling user code.
                        let recognizer_rc = {
                            let arena = arena_rc.borrow();
                            arena
                                .arenas
                                .iter()
                                .find(|(pid, _)| *pid == pointer_id)
                                .and_then(|(_, a)| a.entries.get(idx))
                                .map(|e| std::rc::Rc::clone(&e.recognizer))
                        };
                        let Some(rec_rc) = recognizer_rc else {
                            return;
                        };
                        // Mark accepted + fire on_long_press_start.
                        // Drop the borrow before declare_winner so
                        // declare_winner's own arena re-borrow does
                        // not panic.
                        {
                            let mut rec = rec_rc.borrow_mut();
                            if let Some(lp) =
                                rec.as_any_mut().downcast_mut::<LongPressGestureRecognizer>()
                            {
                                lp.accepted = true;
                                if let Some(cb) = lp.on_long_press_start.as_mut() {
                                    cb(
                                        LongPressDetails {
                                            global_position: entry_position,
                                            kind: entry_kind,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }
                        back_channel.declare_winner(pointer_id, idx, window, cx);
                    });
                }));
                GestureDisposition::Possible
            }
            PointerPhase::Move => {
                if self.distance_sq(event.local_position) > (self.slop.0).powi(2) {
                    self.timer = None; // drops the task → cancels future
                    GestureDisposition::Rejected
                } else if self.accepted {
                    if let Some(cb) = self.on_long_press_move.as_mut() {
                        cb(
                            LongPressDetails {
                                global_position: event.global_position(),
                                kind: event.kind(),
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
                                global_position: event.global_position(),
                                kind: event.kind(),
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
        pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        // Drop the timer to cancel the future and clear the entry
        // slot for this pointer (single-shot LongPress only ever
        // tracks one in-flight pointer, but staying defensive
        // matches the per-pointer storage shape).
        self.timer = None;
        self.accepted = false;
        self.pointer_indexes
            .retain(|(pid, _)| *pid != pointer_id);
    }

    fn semantic_actions(&self) -> &'static [SemanticAction] {
        LONG_PRESS_SEMANTIC_ACTIONS
    }

    fn lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle> {
        Some(self)
    }
}

impl RecognizerLifecycle for LongPressGestureRecognizer {
    fn needs_back_channel(&self) -> bool {
        true
    }

    fn set_arena_back_channel(
        &mut self,
        pointer_id: PointerId,
        back_channel: ArenaBackChannel,
        entry_index: usize,
    ) {
        log::trace!(
            target: "flui::gesture::long_press",
            recognizer = "long_press",
            lifecycle = "set_back_channel",
            pointer_id = format!("{:?}", pointer_id),
            entry_index = entry_index;
            "long_press back-channel injected at registration"
        );
        self.arena_back_channel = back_channel;
        // Replace any stale entry for this pointer (defensive: a
        // re-Down on the same pointer mid-arena is unexpected but
        // would otherwise leave a duplicate slot here).
        self.pointer_indexes
            .retain(|(pid, _)| *pid != pointer_id);
        self.pointer_indexes.push((pointer_id, entry_index));
    }

    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.timeout = settings.long_press_timeout;
        self.slop = settings.long_press_slop;
        self.timer_budget = settings.long_press_timer_budget;
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
        DeliveredEvent, GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind,
        PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, Modifiers, Pixels, Point, TestAppContext};

    fn de(event: &PointerEvent) -> DeliveredEvent<'_> {
        DeliveredEvent::at_event_position(event)
    }

    fn pe(phase: PointerPhase, pos: Point<Pixels>, buttons: PointerButtons) -> PointerEvent {
        let now = Instant::now();
        PointerEvent {
            pointer_id: PointerId(0),
            kind: PointerKind::Mouse,
            phase,
            position: pos,
            delta: Point::default(),
            buttons,
            modifiers: Modifiers::default(),
            timestamp: now,
            source_timestamp: now,
            provenance: crate::gesture::PointerEventProvenance::Platform,
            pressure: None,
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
                    lp.add_pointer(PointerId(0), de(&down));
                    assert_eq!(
                        lp.handle_event(de(&down), window, cx),
                        GestureDisposition::Possible
                    );
                    assert!(lp.timer.is_some(), "Down schedules a timer");
                    let mv = pe(PointerPhase::Move, p(100.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        lp.handle_event(de(&mv), window, cx),
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
                    lp.add_pointer(PointerId(0), de(&down));
                    let _ = lp.handle_event(de(&down), window, cx);
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        lp.handle_event(de(&up), window, cx),
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
                    lp.add_pointer(PointerId(0), de(&down));
                    let _ = lp.handle_event(de(&down), window, cx);
                    let cancel = pe(PointerPhase::Cancel, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        lp.handle_event(de(&cancel), window, cx),
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
                    lp.add_pointer(PointerId(0), de(&down));
                    let _ = lp.handle_event(de(&down), window, cx);
                    lp.accepted = true; // simulate timer firing
                    GestureRecognizer::rejected(&mut lp, PointerId(0), window, cx);
                    assert!(lp.timer.is_none(), "rejected drops the timer");
                    assert!(!lp.accepted, "rejected resets the accepted flag");
                });
        });
    }
}
