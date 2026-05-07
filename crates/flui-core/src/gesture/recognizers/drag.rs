//! `PanGestureRecognizer` (free pan), `HorizontalDragGestureRecognizer`
//! (axis-locked horizontal), `VerticalDragGestureRecognizer`
//! (axis-locked vertical) + shared `DragStartDetails` /
//! `DragUpdateDetails` / `DragEndDetails`.
//!
//! Each recognizer feeds a per-pointer `VelocityTracker` and emits
//! velocity at end. Coexists with the imperative `cx.active_drag` /
//! `AnyDrag` flow — both can be active simultaneously; the dispatcher
//! resets `cx.propagate_event = true` between the arena pass and
//! `dispatch_mouse_event` so `on_mouse_down` always fires.
//!
//! See the design doc § "Drag recognizers".

use crate::gesture::{
    GestureDisposition, GestureRecognizer, GestureSettings, PointerButtons, PointerEvent,
    PointerId, PointerKind, PointerPhase, PositionSample, Velocity, VelocityTracker,
};
use crate::scheduler::Instant;
use crate::{Pixels, Point};

/// Payload for `on_*_drag_start` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DragStartDetails {
    /// Position where the drag was first detected (post-slop).
    pub global_position: Point<Pixels>,
    /// The device kind.
    pub kind: PointerKind,
}

/// Payload for `on_*_drag_update` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DragUpdateDetails {
    /// Current position in window-local pixels.
    pub global_position: Point<Pixels>,
    /// Movement delta since the previous update.
    pub delta: Point<Pixels>,
    /// The device kind.
    pub kind: PointerKind,
}

/// Payload for `on_*_drag_end` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DragEndDetails {
    /// Final velocity at the time of `Up`.
    pub velocity: Velocity,
    /// Final position.
    pub global_position: Point<Pixels>,
    /// The device kind.
    pub kind: PointerKind,
}

/// Axis lock for the drag recognizer family.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragAxis {
    Free,
    Horizontal,
    Vertical,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragState {
    Idle,
    Possible,
    Accepted,
    Rejected,
}

struct DragImpl {
    axis: DragAxis,
    pan_slop: Pixels,
    button: PointerButtons,
    state: DragState,
    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_position: Point<Pixels>,
    last_kind: PointerKind,
    velocity_tracker: VelocityTracker,

    on_start: Option<Box<dyn FnMut(DragStartDetails, &mut crate::Window, &mut crate::App)>>,
    on_update: Option<Box<dyn FnMut(DragUpdateDetails, &mut crate::Window, &mut crate::App)>>,
    on_end: Option<Box<dyn FnMut(DragEndDetails, &mut crate::Window, &mut crate::App)>>,
}

impl DragImpl {
    fn new(axis: DragAxis, settings: &GestureSettings) -> Self {
        Self {
            axis,
            pan_slop: settings.pan_slop,
            button: PointerButtons::PRIMARY,
            state: DragState::Idle,
            pointer: None,
            down_position: Point::default(),
            last_position: Point::default(),
            last_kind: PointerKind::Mouse,
            velocity_tracker: VelocityTracker::new(settings),
            on_start: None,
            on_update: None,
            on_end: None,
        }
    }

    fn axis_passes_slop(&self, dx: f32, dy: f32) -> bool {
        let slop = self.pan_slop.0;
        match self.axis {
            DragAxis::Free => (dx * dx + dy * dy) > slop * slop,
            DragAxis::Horizontal => dx.abs() > slop && dx.abs() > 2.0 * dy.abs(),
            DragAxis::Vertical => dy.abs() > slop && dy.abs() > 2.0 * dx.abs(),
        }
    }

    fn axis_rejected(&self, dx: f32, dy: f32) -> bool {
        let slop = self.pan_slop.0;
        match self.axis {
            DragAxis::Free => false,
            DragAxis::Horizontal => dy.abs() > slop && dy.abs() > 2.0 * dx.abs(),
            DragAxis::Vertical => dx.abs() > slop && dx.abs() > 2.0 * dy.abs(),
        }
    }
}

macro_rules! impl_drag_recognizer {
    ($name:ident, $name_str:expr, $axis:expr) => {
        /// Drag recognizer (axis is determined by the type).
        #[non_exhaustive]
        pub struct $name {
            inner: DragImpl,
        }
        impl $name {
            /// Construct a new recognizer using the supplied gesture
            /// settings.
            pub fn new(settings: &GestureSettings) -> Self {
                Self {
                    inner: DragImpl::new($axis, settings),
                }
            }

            /// Set the start callback. Returns the recognizer for
            /// chaining.
            pub fn on_start(
                mut self,
                f: impl FnMut(DragStartDetails, &mut crate::Window, &mut crate::App) + 'static,
            ) -> Self {
                self.inner.on_start = Some(Box::new(f));
                self
            }

            /// Set the update callback.
            pub fn on_update(
                mut self,
                f: impl FnMut(DragUpdateDetails, &mut crate::Window, &mut crate::App) + 'static,
            ) -> Self {
                self.inner.on_update = Some(Box::new(f));
                self
            }

            /// Set the end callback.
            pub fn on_end(
                mut self,
                f: impl FnMut(DragEndDetails, &mut crate::Window, &mut crate::App) + 'static,
            ) -> Self {
                self.inner.on_end = Some(Box::new(f));
                self
            }
        }

        impl GestureRecognizer for $name {
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn name(&self) -> &'static str {
                $name_str
            }

            fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent) {
                if self.inner.state != DragState::Idle {
                    return;
                }
                if !event.buttons.contains(self.inner.button) {
                    return;
                }
                self.inner.pointer = Some(pointer_id);
                self.inner.down_position = event.position;
                self.inner.last_position = event.position;
                self.inner.last_kind = event.kind;
                self.inner.state = DragState::Possible;
                self.inner.velocity_tracker.reset();
                self.inner
                    .velocity_tracker
                    .add_position(PositionSample::new(event.position, Instant::now()));
            }

            fn handle_event(
                &mut self,
                event: &PointerEvent,
                window: &mut crate::Window,
                cx: &mut crate::App,
            ) -> GestureDisposition {
                if self.inner.pointer != Some(event.pointer_id) {
                    return GestureDisposition::Possible;
                }
                match event.phase {
                    PointerPhase::Move => {
                        let dx = event.position.x.0 - self.inner.down_position.x.0;
                        let dy = event.position.y.0 - self.inner.down_position.y.0;
                        self.inner
                            .velocity_tracker
                            .add_position(PositionSample::new(event.position, Instant::now()));

                        if self.inner.state == DragState::Possible {
                            if self.inner.axis_rejected(dx, dy) {
                                self.inner.state = DragState::Rejected;
                                return GestureDisposition::Rejected;
                            }
                            if self.inner.axis_passes_slop(dx, dy) {
                                self.inner.state = DragState::Accepted;
                                if let Some(cb) = self.inner.on_start.as_mut() {
                                    cb(
                                        DragStartDetails {
                                            global_position: event.position,
                                            kind: event.kind,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                                self.inner.last_position = event.position;
                                return GestureDisposition::Accepted;
                            }
                            return GestureDisposition::Possible;
                        }

                        // DragState::Accepted — fire on_update.
                        if let Some(cb) = self.inner.on_update.as_mut() {
                            let delta = Point::new(
                                event.position.x - self.inner.last_position.x,
                                event.position.y - self.inner.last_position.y,
                            );
                            cb(
                                DragUpdateDetails {
                                    global_position: event.position,
                                    delta,
                                    kind: event.kind,
                                },
                                window,
                                cx,
                            );
                        }
                        self.inner.last_position = event.position;
                        GestureDisposition::Possible
                    }
                    PointerPhase::Up => {
                        if self.inner.state == DragState::Accepted {
                            let velocity = self.inner.velocity_tracker.estimate();
                            if let Some(cb) = self.inner.on_end.as_mut() {
                                cb(
                                    DragEndDetails {
                                        velocity,
                                        global_position: event.position,
                                        kind: event.kind,
                                    },
                                    window,
                                    cx,
                                );
                            }
                            self.inner.state = DragState::Idle;
                            GestureDisposition::Accepted
                        } else {
                            self.inner.state = DragState::Rejected;
                            GestureDisposition::Rejected
                        }
                    }
                    PointerPhase::Cancel | PointerPhase::Removed => {
                        self.inner.state = DragState::Rejected;
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
                // Drag wins by eager-accept; sweep means slop was
                // never crossed — no callbacks to fire.
            }

            fn rejected(
                &mut self,
                _pointer_id: PointerId,
                _window: &mut crate::Window,
                _cx: &mut crate::App,
            ) {
                self.inner.state = DragState::Rejected;
            }
        }
    };
}

impl_drag_recognizer!(PanGestureRecognizer, "pan", DragAxis::Free);
impl_drag_recognizer!(
    HorizontalDragGestureRecognizer,
    "horizontal_drag",
    DragAxis::Horizontal
);
impl_drag_recognizer!(
    VerticalDragGestureRecognizer,
    "vertical_drag",
    DragAxis::Vertical
);
