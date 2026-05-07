//! `TapGestureRecognizer` + `TapDetails` / `TapDownDetails` /
//! `TapUpDetails`.
//!
//! Primary / secondary / tertiary buttons; `request_focus_on_tap_down`
//! wired through the `on_focus_request` (S12 seam) hook;
//! `semantic_actions()` returns `&[SemanticAction::Tap]` (S08 seam).
//!
//! See the design doc § "TapGestureRecognizer".

use crate::gesture::{
    GestureDisposition, GestureRecognizer, PointerButtons, PointerEvent, PointerId, PointerKind,
    PointerPhase, SemanticAction,
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

/// State machine: tap starts on `Down`; rejects on `Move > slop` or
/// `Up` of a different `pointer_id`; eagerly accepts on `Up` within
/// slop.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TapState {
    Idle,
    Down,
    Accepted,
    Rejected,
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
                    self.state = TapState::Rejected;
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
                self.state = TapState::Accepted;
                GestureDisposition::Accepted
            }
            PointerPhase::Cancel | PointerPhase::Removed => {
                if let Some(cb) = self.on_tap_cancel.as_mut() {
                    cb(window, cx);
                }
                self.state = TapState::Rejected;
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
        // Sweep — last competitor on Up. Fire callbacks as if we
        // accepted (the arena declared us winner).
        if self.state != TapState::Accepted {
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
            self.state = TapState::Accepted;
        }
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
        self.state = TapState::Rejected;
    }

    fn semantic_actions(&self) -> &'static [SemanticAction] {
        TAP_SEMANTIC_ACTIONS
    }

    fn on_focus_request(&self) -> Option<FocusHandle> {
        self.request_focus_on_tap_down.clone()
    }
}
