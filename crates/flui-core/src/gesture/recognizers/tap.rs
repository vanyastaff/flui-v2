//! `TapGestureRecognizer` + `TapDetails` / `TapDownDetails` /
//! `TapUpDetails`.
//!
//! Primary / secondary / tertiary buttons; `request_focus_on_tap_down`
//! wired through the `on_focus_request` (S12 seam) hook;
//! `semantic_actions()` returns `&[SemanticAction::Tap]` (S08 seam).
//!
//! See the design doc § "TapGestureRecognizer".

use crate::gesture::{
    GestureDisposition, GestureRecognizer, GestureSettings, PointerButtons, PointerEvent,
    PointerId, PointerKind, PointerPhase, RecognizerLifecycle, SemanticAction,
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

    state: TapState,
    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_kind: PointerKind,
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
            state: TapState::Idle,
            pointer: None,
            down_position: Point::default(),
            last_kind: PointerKind::Mouse,
        }
    }

    /// Internal — drop tracked pointer state and return to `Idle` so
    /// the recognizer is ready for a fresh `add_pointer`. Called from
    /// every terminal path inside `handle_event`, `sweep_accepted`,
    /// and `rejected`.
    fn reset(&mut self) {
        self.state = TapState::Idle;
        self.pointer = None;
    }
}

impl GestureRecognizer for TapGestureRecognizer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "tap"
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent) {
        if self.state != TapState::Idle {
            return;
        }
        if !event.buttons.contains(self.button) {
            return;
        }
        self.pointer = Some(pointer_id);
        self.down_position = event.position;
        self.last_kind = event.kind;
        self.state = TapState::Down;
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
                if let Some(cb) = self.on_tap_down.as_mut() {
                    cb(
                        TapDownDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: event.kind,
                        },
                        window,
                        cx,
                    );
                }
                GestureDisposition::Possible
            }
            PointerPhase::Move => {
                let dx = event.position.x.0 - self.down_position.x.0;
                let dy = event.position.y.0 - self.down_position.y.0;
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
                if let Some(cb) = self.on_tap_up.as_mut() {
                    cb(
                        TapUpDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: event.kind,
                        },
                        window,
                        cx,
                    );
                }
                if let Some(cb) = self.on_tap.as_mut() {
                    cb(
                        TapDetails {
                            kind: event.kind,
                            global_position: event.position,
                        },
                        window,
                        cx,
                    );
                }
                // Eager-accept resolves the gesture; arena declares us
                // winner without further calls. Reset so the recognizer
                // can serve the next tap on the same element.
                self.reset();
                GestureDisposition::Accepted
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
        // Sweep — last competitor on Up that did not eager-accept
        // earlier. The arena declared us winner via sweep semantics;
        // fire `on_tap` once and reset. Guard against double-fire
        // by checking that we are still tracking a pointer (an
        // eager-accept Up would have reset us already).
        if self.pointer.is_some() {
            if let Some(cb) = self.on_tap.as_mut() {
                cb(
                    TapDetails {
                        kind: self.last_kind,
                        global_position: self.down_position,
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
        GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, Pixels, Point, TestAppContext};
    use std::cell::Cell;
    use std::rc::Rc;

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
    fn tap_down_then_up_within_slop_eagerly_accepts(cx: &mut TestAppContext) {
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
                    tap.add_pointer(PointerId(0), &down);
                    assert_eq!(
                        tap.handle_event(&down, window, cx),
                        GestureDisposition::Possible,
                        "Down stays Possible until Up"
                    );
                    let up = pe(PointerPhase::Up, p(11.0, 11.0), PointerButtons::default());
                    assert_eq!(
                        tap.handle_event(&up, window, cx),
                        GestureDisposition::Accepted,
                        "Up within slop eagerly Accepts"
                    );
                    assert_eq!(fired.get(), 1, "on_tap fires exactly once");
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
                    tap.add_pointer(PointerId(0), &down);
                    let _ = tap.handle_event(&down, window, cx);
                    // Move beyond touch_slop (default 18px); 100 > 18 on x.
                    let mv = pe(PointerPhase::Move, p(100.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        tap.handle_event(&mv, window, cx),
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
                    tap.add_pointer(PointerId(0), &down);
                    let _ = tap.handle_event(&down, window, cx);
                    let cancel = pe(PointerPhase::Cancel, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        tap.handle_event(&cancel, window, cx),
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
                    tap.add_pointer(PointerId(0), &down);
                    let up = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    // pointer is None → recognizer ignores the event
                    // (returns Possible, no callback).
                    assert_eq!(
                        tap.handle_event(&up, window, cx),
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
                    // Realistic flow: arena registers the recognizer via
                    // `add_pointer` on the Down, then sweeps on Up if
                    // no other recognizer eager-accepted. The recognizer
                    // expects `add_pointer` to have run before
                    // `sweep_accepted` — Copilot-G/H reset tightened the
                    // contract so an unrequested sweep is a no-op.
                    let down = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    tap.add_pointer(PointerId(0), &down);
                    tap.sweep_accepted(PointerId(0), window, cx);
                    assert_eq!(fired.get(), 1, "sweep_accepted fires on_tap once");
                    // After sweep, the recognizer is back at Idle, so a
                    // second sweep without a fresh add_pointer is a
                    // no-op (no double-fire).
                    tap.sweep_accepted(PointerId(0), window, cx);
                    assert_eq!(fired.get(), 1, "second sweep does not refire");
                });
        });
    }
}
