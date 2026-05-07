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
