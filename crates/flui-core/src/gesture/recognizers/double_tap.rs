//! `DoubleTapGestureRecognizer` + `DoubleTapDetails`.
//!
//! See the design doc § "DoubleTapGestureRecognizer".

use crate::gesture::{
    GestureDisposition, GestureRecognizer, PointerButtons, PointerEvent, PointerId, PointerKind,
    PointerPhase, SemanticAction,
};
use crate::scheduler::Instant;
use crate::{Pixels, Point};
use std::time::Duration;

const DOUBLE_TAP_SEMANTIC_ACTIONS: &[SemanticAction] = &[SemanticAction::DoubleTap];

/// Payload for `on_double_tap` callback.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DoubleTapDetails {
    /// Position of the second tap in window-local pixels.
    pub global_position: Point<Pixels>,
    /// The device kind.
    pub kind: PointerKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DoubleTapState {
    Idle,
    FirstDown,
    AwaitSecond,
    SecondDown,
    Rejected,
}

/// Two-tap recognizer.
#[non_exhaustive]
pub struct DoubleTapGestureRecognizer {
    /// Fires when both taps complete within the configured window.
    pub on_double_tap:
        Option<Box<dyn FnMut(DoubleTapDetails, &mut crate::Window, &mut crate::App)>>,
    /// Which button this recognizer accepts. Default primary.
    pub button: PointerButtons,
    pub(crate) touch_slop: Pixels,
    pub(crate) double_tap_timeout: Duration,
    pub(crate) double_tap_min_time: Duration,

    state: DoubleTapState,
    pointer: Option<PointerId>,
    first_up_time: Option<Instant>,
    first_position: Point<Pixels>,
    last_kind: PointerKind,
}

impl DoubleTapGestureRecognizer {
    /// Construct a new recognizer using the supplied gesture settings.
    pub fn new(settings: &super::super::GestureSettings) -> Self {
        Self {
            on_double_tap: None,
            button: PointerButtons::PRIMARY,
            touch_slop: settings.touch_slop,
            double_tap_timeout: settings.double_tap_timeout,
            double_tap_min_time: settings.double_tap_min_time,
            state: DoubleTapState::Idle,
            pointer: None,
            first_up_time: None,
            first_position: Point::default(),
            last_kind: PointerKind::Mouse,
        }
    }

    fn distance_sq(&self, p: Point<Pixels>) -> f32 {
        let dx = p.x.0 - self.first_position.x.0;
        let dy = p.y.0 - self.first_position.y.0;
        dx * dx + dy * dy
    }
}

impl GestureRecognizer for DoubleTapGestureRecognizer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "double_tap"
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent) {
        if !event.buttons.contains(self.button) {
            return;
        }
        match self.state {
            DoubleTapState::Idle => {
                self.pointer = Some(pointer_id);
                self.first_position = event.position;
                self.last_kind = event.kind;
                self.state = DoubleTapState::FirstDown;
            }
            DoubleTapState::AwaitSecond => {
                // The second tap must arrive after `min_time` and
                // before `timeout`, and within slop of the first.
                let now = Instant::now();
                let elapsed = self
                    .first_up_time
                    .map(|t| now.saturating_duration_since(t))
                    .unwrap_or_default();
                if elapsed < self.double_tap_min_time
                    || elapsed > self.double_tap_timeout
                    || self.distance_sq(event.position) > (self.touch_slop.0).powi(2)
                {
                    self.state = DoubleTapState::Rejected;
                    return;
                }
                self.pointer = Some(pointer_id);
                self.last_kind = event.kind;
                self.state = DoubleTapState::SecondDown;
            }
            _ => {}
        }
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
        match (self.state, event.phase) {
            (DoubleTapState::FirstDown, PointerPhase::Move) => {
                if self.distance_sq(event.position) > (self.touch_slop.0).powi(2) {
                    self.state = DoubleTapState::Rejected;
                    return GestureDisposition::Rejected;
                }
                GestureDisposition::Possible
            }
            (DoubleTapState::FirstDown, PointerPhase::Up) => {
                self.first_up_time = Some(Instant::now());
                self.state = DoubleTapState::AwaitSecond;
                GestureDisposition::Possible
            }
            (DoubleTapState::SecondDown, PointerPhase::Move) => {
                if self.distance_sq(event.position) > (self.touch_slop.0).powi(2) {
                    self.state = DoubleTapState::Rejected;
                    return GestureDisposition::Rejected;
                }
                GestureDisposition::Possible
            }
            (DoubleTapState::SecondDown, PointerPhase::Up) => {
                if let Some(cb) = self.on_double_tap.as_mut() {
                    cb(
                        DoubleTapDetails {
                            global_position: event.position,
                            kind: event.kind,
                        },
                        window,
                        cx,
                    );
                }
                self.state = DoubleTapState::Idle;
                GestureDisposition::Accepted
            }
            (_, PointerPhase::Cancel) => {
                self.state = DoubleTapState::Rejected;
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
        // Sweep on Up of the first tap is meaningless for DoubleTap —
        // we need the second tap. The arena will hold the arena
        // open via `arena.hold` in T15 wiring; sweep without held
        // means we missed our chance.
    }

    fn rejected(
        &mut self,
        _pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        self.state = DoubleTapState::Rejected;
    }

    fn semantic_actions(&self) -> &'static [SemanticAction] {
        DOUBLE_TAP_SEMANTIC_ACTIONS
    }
}

#[cfg(test)]
mod tests {
    //! T17 — Double-tap recognizer unit tests.

    use super::*;
    use crate::gesture::{
        GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{
        self as flui_core, AppContext as _, Context as _, Modifiers, Pixels, Point,
        TestAppContext,
    };
    use std::cell::Cell;
    use std::rc::Rc;

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

    /// Build a recognizer whose double-tap window is permissive enough
    /// for synthetic-time tests: `min_time = 0`, `timeout = 10s`.
    fn permissive_dt() -> DoubleTapGestureRecognizer {
        let mut s = GestureSettings::default();
        s.double_tap_min_time = std::time::Duration::from_millis(0);
        s.double_tap_timeout = std::time::Duration::from_secs(10);
        DoubleTapGestureRecognizer::new(&s)
    }

    #[flui_core::test]
    fn double_tap_two_quick_taps_accept(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let mut dt = permissive_dt();
                    {
                        let fired = Rc::clone(&fired);
                        dt.on_double_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    // First tap.
                    let d1 = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d1);
                    assert_eq!(dt.handle_event(&d1, window, cx), GestureDisposition::Possible);
                    let u1 = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        dt.handle_event(&u1, window, cx),
                        GestureDisposition::Possible,
                        "first Up still Possible (waiting for second tap)"
                    );
                    // Second tap.
                    let d2 = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d2);
                    assert_eq!(dt.handle_event(&d2, window, cx), GestureDisposition::Possible);
                    let u2 = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        dt.handle_event(&u2, window, cx),
                        GestureDisposition::Accepted,
                    );
                    assert_eq!(fired.get(), 1, "on_double_tap fired exactly once");
                });
        });
    }

    #[flui_core::test]
    fn double_tap_second_tap_outside_slop_is_rejected(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let fired = Rc::new(Cell::new(0u32));
                    let mut dt = permissive_dt();
                    {
                        let fired = Rc::clone(&fired);
                        dt.on_double_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    let d1 = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d1);
                    let _ = dt.handle_event(&d1, window, cx);
                    let u1 = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    let _ = dt.handle_event(&u1, window, cx);
                    // Second tap arrives 200px away → outside slop.
                    let d2 = pe(PointerPhase::Down, p(200.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d2);
                    // Once add_pointer rejected, subsequent events stay
                    // in the rejected state and never fire on_double_tap.
                    let u2 = pe(PointerPhase::Up, p(200.0, 0.0), PointerButtons::default());
                    let _ = dt.handle_event(&u2, window, cx);
                    assert_eq!(fired.get(), 0, "out-of-slop second tap rejected");
                });
        });
    }

    #[flui_core::test]
    fn double_tap_second_tap_after_timeout_is_rejected(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    // 1ms timeout — sleeping 5ms guarantees the second
                    // tap arrives past the window.
                    let mut s = GestureSettings::default();
                    s.double_tap_min_time = std::time::Duration::from_millis(0);
                    s.double_tap_timeout = std::time::Duration::from_millis(1);
                    let fired = Rc::new(Cell::new(0u32));
                    let mut dt = DoubleTapGestureRecognizer::new(&s);
                    {
                        let fired = Rc::clone(&fired);
                        dt.on_double_tap = Some(Box::new(move |_d, _w, _c| {
                            fired.set(fired.get() + 1);
                        }));
                    }
                    let d1 = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d1);
                    let _ = dt.handle_event(&d1, window, cx);
                    let u1 = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    let _ = dt.handle_event(&u1, window, cx);
                    // Sleep past the 1ms window. Real-clock sleep on the
                    // test thread is fine — TestAppContext does not pause
                    // wall-clock time.
                    std::thread::sleep(std::time::Duration::from_millis(15));
                    let d2 = pe(PointerPhase::Down, p(0.0, 0.0), PointerButtons::PRIMARY);
                    dt.add_pointer(PointerId(0), &d2);
                    let u2 = pe(PointerPhase::Up, p(0.0, 0.0), PointerButtons::default());
                    let _ = dt.handle_event(&u2, window, cx);
                    assert_eq!(fired.get(), 0, "second tap past timeout rejected");
                });
        });
    }
}
