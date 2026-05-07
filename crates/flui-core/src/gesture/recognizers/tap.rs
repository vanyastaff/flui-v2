//! `TapGestureRecognizer` + `TapDetails` / `TapDownDetails` /
//! `TapUpDetails`.
//!
//! Primary / secondary / tertiary buttons; `request_focus_on_tap_down`
//! wired through the `on_focus_request` (S12 seam) hook;
//! `semantic_actions()` returns `&[SemanticAction::Tap]` (S08 seam).
//!
//! See the design doc § "TapGestureRecognizer".

use crate::Modifiers;
use crate::gesture::{
    AllowedButtonsFilter, DeliveredEvent, GestureDisposition, GestureRecognizer, GestureSettings,
    PointerButtons, PointerId, PointerKind, PointerPhase, RecognizerLifecycle, SemanticAction,
};
use crate::{FocusHandle, Pixels, Point};

const TAP_SEMANTIC_ACTIONS: &[SemanticAction] = &[SemanticAction::Tap];

/// Payload for `on_tap_down` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapDownDetails {
    /// Position of the down event in window-local pixels.
    pub global_position: Point<Pixels>,
    /// Position of the down event in element-local pixels (filled by
    /// the dispatcher; defaults to `global_position` until T14 wires
    /// element-local mapping).
    pub local_position: Point<Pixels>,
    /// The device kind that produced the event.
    pub kind: PointerKind,
}

/// Payload for `on_tap_up` callbacks. Mirrors [`TapDownDetails`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapUpDetails {
    /// Position of the up event in window-local pixels.
    pub global_position: Point<Pixels>,
    /// Position of the up event in element-local pixels.
    pub local_position: Point<Pixels>,
    /// The device kind that produced the event.
    pub kind: PointerKind,
}

/// Payload for `on_tap` callbacks (fires on completed tap).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapDetails {
    /// The device kind that produced the tap.
    pub kind: PointerKind,
    /// Position of the tap in window-local pixels.
    pub global_position: Point<Pixels>,
}

/// State machine — two states only. The previous design carried
/// terminal `Accepted` / `Rejected` variants, but the recognizer
/// must reset to `Idle` after any resolution so the same instance
/// can serve subsequent gestures (Copilot review G/H). Keeping
/// terminal states stuck the recognizer permanently.
///
/// Lifecycle:
/// - `Idle` — waiting for a `Down` (via `add_pointer`).
/// - `Down` — observing the in-flight tap; transitions back to
///   `Idle` on slop reject, on `Up` (eager-accept), or on
///   `arena.rejected` / `Cancel` / `Removed`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TapState {
    Idle,
    Down,
}

/// Single-tap recognizer.
///
/// Fluent-builder construction: use [`Self::new`] and assign callback
/// fields directly (`tap.on_tap = Some(...)`). The struct is
/// `#[non_exhaustive]` to admit future fields.
#[non_exhaustive]
pub struct TapGestureRecognizer {
    /// Fired on the initial `Down` (before arena resolution).
    pub on_tap_down: Option<Box<dyn FnMut(TapDownDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fired on the `Up` that completes the tap.
    pub on_tap_up: Option<Box<dyn FnMut(TapUpDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fired on the `Up` that completes the tap, after `on_tap_up`.
    pub on_tap: Option<Box<dyn FnMut(TapDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fired when the tap is cancelled (Cancel event or rejected by
    /// arena).
    pub on_tap_cancel: Option<Box<dyn FnMut(&mut crate::Window, &mut crate::App)>>,
    /// Which button this recognizer accepts. Default
    /// [`PointerButtons::PRIMARY`].
    pub button: PointerButtons,
    /// Maximum movement before the tap is rejected (touch-slop).
    /// Read from `GestureSettings::touch_slop` at construction.
    pub touch_slop: Pixels,
    /// Optional focus handle to claim on tap-down. Surfaces via
    /// [`GestureRecognizer::on_focus_request`] (S12 seam).
    pub request_focus_on_tap_down: Option<FocusHandle>,
    /// Optional `(buttons, modifiers) -> bool` predicate evaluated by
    /// [`crate::gesture::GestureBinding::register_recognizer`] before
    /// the recognizer joins the arena. `None` (the default) admits
    /// every event whose `buttons` contain [`Self::button`].
    pub allowed_buttons_filter: Option<AllowedButtonsFilter>,

    state: TapState,
    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_kind: PointerKind,
    /// `Up`-event details captured when the pointer is released so
    /// they can be replayed to `on_tap_up` / `on_tap` from
    /// [`Self::sweep_accepted`] after the arena resolves. `None`
    /// before any `Up` and after a sweep fires (cleared by
    /// `reset()`).
    pending_up: Option<(Point<Pixels>, Point<Pixels>, PointerKind)>,
}

impl TapGestureRecognizer {
    /// Construct a new recognizer using the supplied gesture
    /// settings. Callback fields default to `None`.
    pub fn new(settings: &super::super::GestureSettings) -> Self {
        Self {
            on_tap_down: None,
            on_tap_up: None,
            on_tap: None,
            on_tap_cancel: None,
            button: PointerButtons::PRIMARY,
            touch_slop: settings.touch_slop,
            request_focus_on_tap_down: None,
            allowed_buttons_filter: None,
            state: TapState::Idle,
            pointer: None,
            down_position: Point::default(),
            last_kind: PointerKind::Mouse,
            pending_up: None,
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

    /// Internal — drop tracked pointer state and return to `Idle` so
    /// the recognizer is ready for a fresh `add_pointer`. Called from
    /// every terminal path inside `handle_event`, `sweep_accepted`,
    /// and `rejected`.
    fn reset(&mut self) {
        self.state = TapState::Idle;
        self.pointer = None;
        self.pending_up = None;
    }
}

impl GestureRecognizer for TapGestureRecognizer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "tap"
    }

    fn allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter> {
        self.allowed_buttons_filter.as_ref()
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: DeliveredEvent<'_>) {
        if self.state != TapState::Idle {
            return;
        }
        if !event.buttons().contains(self.button) {
            return;
        }
        self.pointer = Some(pointer_id);
        self.down_position = event.local_position;
        self.last_kind = event.kind();
        self.state = TapState::Down;
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
                if let Some(cb) = self.on_tap_down.as_mut() {
                    cb(
                        TapDownDetails {
                            global_position: event.global_position(),
                            local_position: event.local_position,
                            kind: event.kind(),
                        },
                        window,
                        cx,
                    );
                }
                GestureDisposition::Possible
            }
            PointerPhase::Move => {
                let dx = event.local_position.x.0 - self.down_position.x.0;
                let dy = event.local_position.y.0 - self.down_position.y.0;
                let dist_sq = dx * dx + dy * dy;
                let slop = self.touch_slop.0;
                if dist_sq > slop * slop {
                    // Self-declared rejection: arena will not call
                    // `rejected` back on us, so we own the reset.
                    self.reset();
                    GestureDisposition::Rejected
                } else {
                    GestureDisposition::Possible
                }
            }
            PointerPhase::Up => {
                // **Flutter-parity contract.** Tap MUST NOT eagerly
                // accept on `Up`. If it did, a competing
                // `DoubleTapGestureRecognizer` on the same element
                // could never observe the second `Down` — the arena
                // would have already declared Tap the winner and
                // `rejected` the DoubleTap. Instead we stash the Up
                // event here, return `Possible`, and let the arena
                // resolve via `sweep` (immediate when no recognizer
                // holds the arena, deferred until
                // `arena.release` fires when DoubleTap is in play).
                // `sweep_accepted` replays this stored payload to
                // `on_tap_up` / `on_tap`. `rejected` (which fires
                // when DoubleTap wins the second-tap race) clears
                // it instead.
                self.pending_up =
                    Some((event.global_position(), event.local_position, event.kind()));
                GestureDisposition::Possible
            }
            PointerPhase::Cancel | PointerPhase::Removed => {
                if let Some(cb) = self.on_tap_cancel.as_mut() {
                    cb(window, cx);
                }
                self.reset();
                GestureDisposition::Rejected
            }
            _ => GestureDisposition::Possible,
        }
    }

    fn sweep_accepted(
        &mut self,
        _pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        // Arena declared us the winner via sweep semantics — either
        // the immediate sweep right after `Up` (no recognizer holds
        // the arena) or a deferred sweep triggered by
        // `arena.release` after a `DoubleTap` competitor's
        // `double_tap_timeout` expires without a second tap.
        //
        // We fire `on_tap_up` and `on_tap` with the payload we
        // stashed in [`Self::handle_event`]'s `Up` branch. If
        // `pending_up` is `None`, the recognizer was registered but
        // never saw a corresponding `Up` (e.g. arena cancelled
        // mid-flight) — fire nothing rather than synthesizing a
        // bogus position from `down_position`.
        if let Some((global, local, kind)) = self.pending_up.take() {
            if let Some(cb) = self.on_tap_up.as_mut() {
                cb(
                    TapUpDetails {
                        global_position: global,
                        local_position: local,
                        kind,
                    },
                    window,
                    cx,
                );
            }
            if let Some(cb) = self.on_tap.as_mut() {
                cb(
                    TapDetails {
                        kind,
                        global_position: global,
                    },
                    window,
                    cx,
                );
            }
        }
        self.reset();
    }

    fn rejected(
        &mut self,
        _pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        if let Some(cb) = self.on_tap_cancel.as_mut() {
            cb(window, cx);
        }
        self.reset();
    }

    fn semantic_actions(&self) -> &'static [SemanticAction] {
        TAP_SEMANTIC_ACTIONS
    }

    fn on_focus_request(&self) -> Option<FocusHandle> {
        self.request_focus_on_tap_down.clone()
    }

    fn lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle> {
        Some(self)
    }
}

impl RecognizerLifecycle for TapGestureRecognizer {
    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.touch_slop = settings.touch_slop;
    }
}

#[cfg(test)]
mod tests {
    //! T17 — Tap recognizer unit tests.

    use super::*;
    use crate::gesture::{
        DeliveredEvent, GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind,
        PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, Pixels, Point, TestAppContext};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Wrap a synthetic [`PointerEvent`] for delivery to the
    /// [`GestureRecognizer`] trait. Tests assume identity transform
    /// (no per-target inverse) — local position equals window position.
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
            // Synthetic mouse events without real pressure data.
            pressure: None,
            tilt: 0.0,
            orientation: 0.0,
        }
    }

    fn p(x: f32, y: f32) -> Point<Pixels> {
        Point::new(Pixels(x), Pixels(y))
    }

    #[flui_core::test]
    fn tap_down_then_up_stays_possible_until_sweep(cx: &mut TestAppContext) {
        // Flutter-parity contract: Tap does NOT eager-accept on `Up`
        // — it returns `Possible` and waits for the arena's sweep to
        // declare it the winner. `on_tap_up` / `on_tap` only fire
        // from `sweep_accepted`. Eager acceptance would lock out a
        // competing `DoubleTap` recognizer from ever seeing the
        // second `Down`.
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    {
                        let fired = Rc::clone(&fired);
                        tap.on_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    let down = pe(PointerPhase::Down, p(10.0, 10.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    assert_eq!(
                        tap.handle_event(de(&down), window, cx),
                        GestureDisposition::Possible,
                        "Down stays Possible until Up"
                    );
                    let up = pe(PointerPhase::Up, p(11.0, 11.0), PointerButtons::default());
                    assert_eq!(
                        tap.handle_event(de(&up), window, cx),
                        GestureDisposition::Possible,
                        "Up within slop stays Possible — arena sweep decides the winner",
                    );
                    assert_eq!(fired.get(), 0, "on_tap does not fire from handle_event");
                    tap.sweep_accepted(PointerId(0), window, cx);
                    assert_eq!(
                        fired.get(),
                        1,
                        "on_tap fires exactly once from sweep_accepted with the stored Up payload"
                    );
                });
        });
    }

    /// Regression lock for the user-reported "double_tap never
    /// fires, every gesture goes to tap" bug. Before this fix Tap
    /// eagerly accepted on `Up`, which made the arena declare Tap
    /// the winner and `rejected` the DoubleTap — DoubleTap could
    /// never observe the second `Down`. After the fix Tap returns
    /// `Possible`, the arena holds for `double_tap_timeout`, and
    /// `rejected` (called when DoubleTap eager-accepts the second
    /// Up) clears the pending payload so `on_tap` never fires.
    #[flui_core::test]
    fn tap_rejected_after_pending_up_does_not_fire_callback(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let cancels = Rc::new(Cell::new(0u32));
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    {
                        let fired = Rc::clone(&fired);
                        tap.on_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    {
                        let cancels = Rc::clone(&cancels);
                        tap.on_tap_cancel = Some(Box::new(move |_w, _c| {
                            cancels.set(cancels.get() + 1);
                        }));
                    }

                    // Tap registers + sees Up + would-be winner if not
                    // for a competing recognizer.
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    let _ = tap.handle_event(de(&down), window, cx);
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    let _ = tap.handle_event(de(&up), window, cx);

                    // DoubleTap eats the second Up — arena calls
                    // `rejected` on Tap. on_tap MUST NOT fire from
                    // a subsequent `sweep_accepted` because there
                    // is no pending payload anymore.
                    GestureRecognizer::rejected(&mut tap, PointerId(0), window, cx);
                    tap.sweep_accepted(PointerId(0), window, cx);

                    assert_eq!(fired.get(), 0, "on_tap must NOT fire after rejection");
                    assert_eq!(cancels.get(), 1, "on_tap_cancel fires on rejection");
                });
        });
    }

    #[flui_core::test]
    fn tap_move_beyond_slop_rejects(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    let _ = tap.handle_event(de(&down), window, cx);
                    // Move beyond touch_slop (default 18px); 100 > 18 on x.
                    let mv = pe(PointerPhase::Move, p(100.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        tap.handle_event(de(&mv), window, cx),
                        GestureDisposition::Rejected,
                        "Move past slop yields Rejected"
                    );
                });
        });
    }

    #[flui_core::test]
    fn tap_cancel_calls_on_tap_cancel_and_rejects(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let cancels = Rc::new(Cell::new(0u32));
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    {
                        let cancels = Rc::clone(&cancels);
                        tap.on_tap_cancel = Some(Box::new(move |_w, _c| {
                            cancels.set(cancels.get() + 1);
                        }));
                    }
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    let _ = tap.handle_event(de(&down), window, cx);
                    let cancel = pe(PointerPhase::Cancel, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        tap.handle_event(de(&cancel), window, cx),
                        GestureDisposition::Rejected,
                    );
                    assert_eq!(cancels.get(), 1, "on_tap_cancel fired once");
                });
        });
    }

    #[flui_core::test]
    fn tap_secondary_button_does_not_register(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    {
                        let fired = Rc::clone(&fired);
                        tap.on_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    // Default `tap.button` is PRIMARY; SECONDARY-only Down
                    // must not arm the recognizer.
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::SECONDARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    // pointer is None → recognizer ignores the event
                    // (returns Possible, no callback).
                    assert_eq!(
                        tap.handle_event(de(&up), window, cx),
                        GestureDisposition::Possible,
                    );
                    assert_eq!(fired.get(), 0, "PRIMARY-only tap ignored SECONDARY Down");
                });
        });
    }

    #[flui_core::test]
    fn tap_sweep_accepted_fires_on_tap_when_arena_declares_winner(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let mut tap = TapGestureRecognizer::new(&GestureSettings::default());
                    {
                        let fired = Rc::clone(&fired);
                        tap.on_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    // Realistic flow: arena registers the recognizer
                    // via `add_pointer` on the `Down`, the `Up`
                    // stashes `pending_up` and returns `Possible`,
                    // then sweep declares Tap the winner — either
                    // immediately (no DoubleTap competitor) or
                    // deferred via `arena.release` after
                    // `double_tap_timeout` if there was one.
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), de(&down));
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    let _ = tap.handle_event(de(&up), window, cx);
                    tap.sweep_accepted(PointerId(0), window, cx);
                    assert_eq!(fired.get(), 1, "sweep_accepted fires on_tap once");
                    // After sweep, the recognizer is back at Idle and
                    // `pending_up` is cleared, so a second sweep
                    // without a fresh add_pointer + Up is a no-op
                    // (no double-fire).
                    tap.sweep_accepted(PointerId(0), window, cx);
                    assert_eq!(fired.get(), 1, "second sweep does not refire");
                });
        });
    }
}
