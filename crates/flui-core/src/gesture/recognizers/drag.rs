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
        ///
        /// Configure callbacks via [`Self::on_start`] /
        /// [`Self::on_update`] / [`Self::on_end`]; configure
        /// thresholds via [`Self::with_pan_slop`] /
        /// [`Self::with_button`]. The internal `DragImpl` storage is
        /// `pub(crate)` to keep the field set out of the public
        /// semver surface — settings flow through these builder
        /// methods instead.
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

            /// Override the slop (in logical pixels) at which the
            /// drag is accepted. Default comes from the
            /// [`GestureSettings`] passed to [`Self::new`] — typically
            /// 18 logical pixels.
            pub fn with_pan_slop(mut self, slop: crate::Pixels) -> Self {
                self.inner.pan_slop = slop;
                self
            }

            /// Override which button arms this recognizer. Default
            /// [`PointerButtons::PRIMARY`].
            pub fn with_button(mut self, button: PointerButtons) -> Self {
                self.inner.button = button;
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

#[cfg(test)]
mod tests {
    //! T17 — Drag-family recognizer unit tests (Pan / Horizontal / Vertical).

    use super::*;
    use crate::gesture::{
        GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, Pixels, Point, TestAppContext};
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

    fn pt(x: f32, y: f32) -> Point<Pixels> {
        Point::new(Pixels(x), Pixels(y))
    }

    /// Compile-time + behaviour lock for B2 — drag-family thresholds
    /// must be configurable through `with_pan_slop` / `with_button`
    /// builder methods. Changing those to non-`pub` (or removing
    /// them) breaks this test. Behaviour: the configured slop wins
    /// over the `GestureSettings` default.
    #[flui_core::test]
    fn drag_threshold_builders_override_default_slop(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    // Default slop is 18 logical px. Override to 100 px
                    // — a 50-px move that *would* cross default slop
                    // should now stay Possible.
                    let mut pan = PanGestureRecognizer::new(&GestureSettings::default())
                        .with_pan_slop(crate::Pixels(100.0))
                        .with_button(PointerButtons::PRIMARY);
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    pan.add_pointer(PointerId(0), &down);
                    let _ = pan.handle_event(&down, window, cx);
                    let mv = pe(PointerPhase::Move, pt(50.0, 50.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        pan.handle_event(&mv, window, cx),
                        GestureDisposition::Possible,
                        "with_pan_slop(100) keeps a 50-px move below threshold",
                    );
                });
        });
    }

    #[flui_core::test]
    fn pan_below_slop_stays_possible_then_accepts_on_slop_crossing(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let starts = Rc::new(Cell::new(0u32));
                    let pan = PanGestureRecognizer::new(&GestureSettings::default()).on_start({
                        let starts = Rc::clone(&starts);
                        move |_d, _w, _c| {
                            starts.set(starts.get() + 1);
                        }
                    });
                    let mut pan = pan;
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    pan.add_pointer(PointerId(0), &down);
                    let _ = pan.handle_event(&down, window, cx);
                    // Stay below the 18px default slop.
                    let mv1 = pe(PointerPhase::Move, pt(5.0, 5.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        pan.handle_event(&mv1, window, cx),
                        GestureDisposition::Possible
                    );
                    assert_eq!(starts.get(), 0, "no on_start while below slop");
                    // Cross slop → Accepted, on_start fires.
                    let mv2 = pe(PointerPhase::Move, pt(50.0, 50.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        pan.handle_event(&mv2, window, cx),
                        GestureDisposition::Accepted
                    );
                    assert_eq!(starts.get(), 1, "on_start fires once on slop crossing");
                });
        });
    }

    #[flui_core::test]
    fn horizontal_drag_rejects_orthogonal_motion(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut hdrag =
                        HorizontalDragGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    hdrag.add_pointer(PointerId(0), &down);
                    let _ = hdrag.handle_event(&down, window, cx);
                    // Vertical motion: dy=100, dx=0 → axis_rejected.
                    let mv = pe(PointerPhase::Move, pt(0.0, 100.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        hdrag.handle_event(&mv, window, cx),
                        GestureDisposition::Rejected,
                    );
                });
        });
    }

    #[flui_core::test]
    fn horizontal_drag_accepts_aligned_motion(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ =
                cx.open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                    .unwrap()
                    .update(cx, |_, window, cx| {
                        let starts = Rc::new(Cell::new(0u32));
                        let mut hdrag =
                            HorizontalDragGestureRecognizer::new(&GestureSettings::default())
                                .on_start({
                                    let starts = Rc::clone(&starts);
                                    move |_d, _w, _c| {
                                        starts.set(starts.get() + 1);
                                    }
                                });
                        let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                        hdrag.add_pointer(PointerId(0), &down);
                        let _ = hdrag.handle_event(&down, window, cx);
                        // Horizontal motion: dx=50, dy=0.
                        let mv = pe(PointerPhase::Move, pt(50.0, 0.0), PointerButtons::PRIMARY);
                        assert_eq!(
                            hdrag.handle_event(&mv, window, cx),
                            GestureDisposition::Accepted,
                        );
                        assert_eq!(starts.get(), 1);
                    });
        });
    }

    #[flui_core::test]
    fn vertical_drag_rejects_orthogonal_motion(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut vdrag = VerticalDragGestureRecognizer::new(&GestureSettings::default());
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    vdrag.add_pointer(PointerId(0), &down);
                    let _ = vdrag.handle_event(&down, window, cx);
                    let mv = pe(PointerPhase::Move, pt(100.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        vdrag.handle_event(&mv, window, cx),
                        GestureDisposition::Rejected,
                    );
                });
        });
    }

    #[flui_core::test]
    fn pan_update_emits_correct_delta(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let last_delta: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
                    let mut pan =
                        PanGestureRecognizer::new(&GestureSettings::default()).on_update({
                            let last = Rc::clone(&last_delta);
                            move |d, _w, _c| {
                                last.set((d.delta.x.0, d.delta.y.0));
                            }
                        });
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    pan.add_pointer(PointerId(0), &down);
                    let _ = pan.handle_event(&down, window, cx);
                    // Cross slop → Accepted.
                    let m1 = pe(PointerPhase::Move, pt(40.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        pan.handle_event(&m1, window, cx),
                        GestureDisposition::Accepted
                    );
                    // Now in Accepted state — next Move fires on_update
                    // with delta from the snapshot stored on accept.
                    let m2 = pe(PointerPhase::Move, pt(60.0, 5.0), PointerButtons::PRIMARY);
                    let _ = pan.handle_event(&m2, window, cx);
                    let (dx, dy) = last_delta.get();
                    assert!(
                        (dx - 20.0).abs() < 1e-3,
                        "expected delta.x = 20.0, got {}",
                        dx
                    );
                    assert!(
                        (dy - 5.0).abs() < 1e-3,
                        "expected delta.y = 5.0, got {}",
                        dy
                    );
                });
        });
    }

    #[flui_core::test]
    fn pan_end_fires_with_well_formed_velocity(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    // VelocityTracker timestamps come from
                    // `crate::scheduler::Instant::now()`, which the test
                    // dispatcher freezes during synchronous updates. We
                    // therefore can't assert on the magnitude of the
                    // velocity in a unit test — only that on_end fires
                    // and the velocity field is well-formed (non-NaN).
                    // The bench fixture (T22) measures real-clock
                    // velocity behaviour end-to-end.
                    let ends = Rc::new(Cell::new(0u32));
                    let velocity_ok = Rc::new(Cell::new(false));
                    let mut pan = PanGestureRecognizer::new(&GestureSettings::default()).on_end({
                        let ends = Rc::clone(&ends);
                        let ok = Rc::clone(&velocity_ok);
                        move |d, _w, _c| {
                            ends.set(ends.get() + 1);
                            let vx = d.velocity.pixels_per_second.x;
                            let vy = d.velocity.pixels_per_second.y;
                            ok.set(!vx.is_nan() && !vy.is_nan());
                        }
                    });
                    let down = pe(PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    pan.add_pointer(PointerId(0), &down);
                    let _ = pan.handle_event(&down, window, cx);
                    // Cross slop → Accepted.
                    let m1 = pe(PointerPhase::Move, pt(40.0, 0.0), PointerButtons::PRIMARY);
                    assert_eq!(
                        pan.handle_event(&m1, window, cx),
                        GestureDisposition::Accepted
                    );
                    // A few more samples (no real time passes; positions
                    // still feed the tracker).
                    for x in [60.0_f32, 80.0, 100.0] {
                        let mv = pe(PointerPhase::Move, pt(x, 0.0), PointerButtons::PRIMARY);
                        let _ = pan.handle_event(&mv, window, cx);
                    }
                    let up = pe(PointerPhase::Up, pt(100.0, 0.0), PointerButtons::default());
                    assert_eq!(
                        pan.handle_event(&up, window, cx),
                        GestureDisposition::Accepted,
                    );
                    assert_eq!(ends.get(), 1, "on_end fires exactly once");
                    assert!(velocity_ok.get(), "velocity must be non-NaN");
                });
        });
    }
}
