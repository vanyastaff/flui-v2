//! `ScaleGestureRecognizer` + `ScaleStartDetails` /
//! `ScaleUpdateDetails` / `ScaleEndDetails`.
//!
//! ≥2 pointers; focal point + scale + rotation.
//!
//! **Rotation is always 0.0 on current desktop platforms** —
//! `PinchEvent.delta` is scale-only, and Windows desktop has no
//! native pinch at all (see the design doc's "Explicit gaps"
//! matrix). The recognizer state machine carries rotation so future
//! multi-pointer touch input on Wayland's
//! `pointer-gestures-unstable-v1` can populate it without a
//! breaking change.
//!
//! See the design doc § "ScaleGestureRecognizer".

use crate::Modifiers;
use crate::gesture::{
    AllowedButtonsFilter, DeliveredEvent, GestureDisposition, GestureRecognizer, GestureSettings,
    PointerButtons, PointerId, PointerKind, PointerPhase, RecognizerLifecycle,
};
use crate::{Pixels, Point};
use smallvec::SmallVec;

/// Payload for `on_scale_start` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ScaleStartDetails {
    /// Average position of all active pointers.
    pub focal_point: Point<Pixels>,
    /// Number of active pointers (always ≥ 2).
    pub pointer_count: usize,
    /// The kind of the first pointer (devices in a multi-pointer
    /// gesture should match in practice).
    pub kind: PointerKind,
}

/// Payload for `on_scale_update` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ScaleUpdateDetails {
    /// Current focal point (average of active pointers).
    pub focal_point: Point<Pixels>,
    /// Multiplicative scale relative to the start (1.0 == no change).
    pub scale: f32,
    /// Rotation in radians relative to the start. Always 0.0 on
    /// current desktop platforms.
    pub rotation: f32,
    /// Number of active pointers.
    pub pointer_count: usize,
    /// The kind of the first pointer.
    pub kind: PointerKind,
}

/// Payload for `on_scale_end` callbacks.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ScaleEndDetails {
    /// Final scale at end.
    pub scale: f32,
    /// Final rotation at end (radians).
    pub rotation: f32,
}

/// Scale state machine. The previous design carried a terminal
/// `Rejected` variant, but the recognizer must reset to `Idle`
/// after every resolution so the same instance can serve subsequent
/// gestures (Copilot review G/H). Rejection now expresses itself by
/// transitioning to `Idle` together with clearing pointer storage.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScaleState {
    Idle,
    Possible,
    Accepted,
}

/// Multi-pointer scale recognizer.
///
/// Threshold field [`Self::slop`] is public for symmetry with
/// [`super::TapGestureRecognizer::touch_slop`] — it can be tuned
/// post-construction.
#[non_exhaustive]
pub struct ScaleGestureRecognizer {
    /// Fires when the gesture is accepted (≥ 2 pointers crossed slop).
    /// Carries the focal point and pointer count at acceptance time.
    pub on_start: Option<Box<dyn FnMut(ScaleStartDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fires on every pointer-Move while the gesture is active.
    /// Carries the current scale ratio (relative to the initial
    /// pointer pair distance) and rotation in radians (always 0.0
    /// on current desktop platforms).
    pub on_update: Option<Box<dyn FnMut(ScaleUpdateDetails, &mut crate::Window, &mut crate::App)>>,
    /// Fires when the active pointer count drops below 2 (the
    /// gesture cannot continue without a pair). Carries the final
    /// scale and rotation snapshot.
    pub on_end: Option<Box<dyn FnMut(ScaleEndDetails, &mut crate::Window, &mut crate::App)>>,
    /// Minimum pointer-pair distance change (in logical pixels) before
    /// the gesture is accepted. Read from
    /// [`crate::gesture::GestureSettings::touch_slop`] at construction.
    pub slop: Pixels,
    /// Optional `(buttons, modifiers) -> bool` predicate evaluated by
    /// `GestureBinding::register_recognizer` before
    /// the recognizer joins the arena. `None` (the default) admits
    /// every event.
    pub allowed_buttons_filter: Option<AllowedButtonsFilter>,

    state: ScaleState,
    /// Active pointers, indexed by `PointerId`.
    pointers: SmallVec<[(PointerId, Point<Pixels>); 4]>,
    initial_distance: f32,
    initial_angle: f32,
    initial_kind: PointerKind,
}

impl ScaleGestureRecognizer {
    /// Construct a new recognizer using the supplied gesture settings.
    pub fn new(settings: &GestureSettings) -> Self {
        Self {
            on_start: None,
            on_update: None,
            on_end: None,
            slop: settings.touch_slop,
            allowed_buttons_filter: None,
            state: ScaleState::Idle,
            pointers: SmallVec::new(),
            initial_distance: 0.0,
            initial_angle: 0.0,
            initial_kind: PointerKind::Mouse,
        }
    }

    /// Fluent setter for [`Self::allowed_buttons_filter`]. The closure
    /// is evaluated by `GestureBinding::register_recognizer`
    /// at registration time; on `false` the recognizer never enters
    /// the arena (Decision D10).
    pub fn with_allowed_buttons_filter(
        mut self,
        f: impl Fn(PointerButtons, Modifiers) -> bool + 'static,
    ) -> Self {
        self.allowed_buttons_filter = Some(AllowedButtonsFilter::new(f));
        self
    }

    fn focal_point(&self) -> Point<Pixels> {
        if self.pointers.is_empty() {
            return Point::default();
        }
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        for (_, p) in self.pointers.iter() {
            sum_x += p.x.0;
            sum_y += p.y.0;
        }
        let n = self.pointers.len() as f32;
        Point::new(Pixels(sum_x / n), Pixels(sum_y / n))
    }

    fn pair_distance(&self) -> f32 {
        if self.pointers.len() < 2 {
            return 0.0;
        }
        let a = self.pointers[0].1;
        let b = self.pointers[1].1;
        let dx = a.x.0 - b.x.0;
        let dy = a.y.0 - b.y.0;
        (dx * dx + dy * dy).sqrt()
    }

    fn pair_angle(&self) -> f32 {
        if self.pointers.len() < 2 {
            return 0.0;
        }
        let a = self.pointers[0].1;
        let b = self.pointers[1].1;
        (b.y.0 - a.y.0).atan2(b.x.0 - a.x.0)
    }

    fn update_pointer(&mut self, pointer_id: PointerId, position: Point<Pixels>) {
        if let Some(slot) = self.pointers.iter_mut().find(|(id, _)| *id == pointer_id) {
            slot.1 = position;
        }
    }

    /// Snapshot the current scale ratio from the active pointer pair.
    /// Reads `self.pointers` directly (not a snapshot copy), so call
    /// **before** removing a lifted pointer when constructing
    /// [`ScaleEndDetails`] — Copilot review F.
    fn current_scale(&self) -> f32 {
        if self.initial_distance > f32::EPSILON {
            self.pair_distance() / self.initial_distance
        } else {
            1.0
        }
    }

    /// Internal — drop tracked pointers and return to `Idle` so the
    /// recognizer is ready for a fresh multi-pointer gesture. Called
    /// from every terminal path (gesture-end, slop fail, Cancel,
    /// arena `rejected`).
    fn reset(&mut self) {
        self.state = ScaleState::Idle;
        self.pointers.clear();
        self.initial_distance = 0.0;
        self.initial_angle = 0.0;
        self.initial_kind = PointerKind::Mouse;
    }
}

impl GestureRecognizer for ScaleGestureRecognizer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "scale"
    }

    fn allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter> {
        self.allowed_buttons_filter.as_ref()
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: DeliveredEvent<'_>) {
        // Skip duplicate registrations — a pointer that re-enters the
        // arena (e.g. via a sanitizer-synthesized re-Down after orphan
        // detection) keeps its existing position; the next Move
        // updates it.
        if self.pointers.iter().any(|(id, _)| *id == pointer_id) {
            return;
        }
        // Single atomic transition: capture the "this is the first
        // pointer" signal *before* the push so it stays meaningful.
        // The previous version checked `is_empty()` after the push,
        // which made the test trivially false and silently kept
        // `initial_kind` at its `Mouse` default.
        let is_first_pointer = self.pointers.is_empty();
        self.pointers.push((pointer_id, event.local_position));
        if is_first_pointer {
            self.initial_kind = event.kind();
        }
        if self.pointers.len() >= 2 && self.state == ScaleState::Idle {
            self.state = ScaleState::Possible;
            self.initial_distance = self.pair_distance();
            self.initial_angle = self.pair_angle();
        }
    }

    fn handle_event(
        &mut self,
        event: DeliveredEvent<'_>,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition {
        match event.phase() {
            PointerPhase::Move => {
                self.update_pointer(event.pointer_id(), event.local_position);
                if self.pointers.len() < 2 {
                    return GestureDisposition::Possible;
                }
                let cur_distance = self.pair_distance();
                let cur_angle = self.pair_angle();

                if self.state == ScaleState::Possible {
                    let delta = (cur_distance - self.initial_distance).abs();
                    if delta > self.slop.0 {
                        self.state = ScaleState::Accepted;
                        // Snapshot before borrowing self.on_start mutably.
                        let focal_point = self.focal_point();
                        let pointer_count = self.pointers.len();
                        let kind = self.initial_kind;
                        if let Some(cb) = self.on_start.as_mut() {
                            cb(
                                ScaleStartDetails {
                                    focal_point,
                                    pointer_count,
                                    kind,
                                },
                                window,
                                cx,
                            );
                        }
                        return GestureDisposition::Accepted;
                    }
                    return GestureDisposition::Possible;
                }

                if self.state == ScaleState::Accepted {
                    let scale = if self.initial_distance > f32::EPSILON {
                        cur_distance / self.initial_distance
                    } else {
                        1.0
                    };
                    let rotation = cur_angle - self.initial_angle;
                    let focal_point = self.focal_point();
                    let pointer_count = self.pointers.len();
                    let kind = self.initial_kind;
                    if let Some(cb) = self.on_update.as_mut() {
                        cb(
                            ScaleUpdateDetails {
                                focal_point,
                                scale,
                                rotation,
                                pointer_count,
                                kind,
                            },
                            window,
                            cx,
                        );
                    }
                }
                GestureDisposition::Possible
            }
            PointerPhase::Up | PointerPhase::Cancel | PointerPhase::Removed => {
                // Detect "the lifted pointer is one of the active
                // pair" *before* mutating `self.pointers`, then
                // snapshot `scale`/`rotation` *before* the retain so
                // the end callback reports the final values rather
                // than the post-retain `pair_distance() = 0` (Copilot
                // review F). After the snapshot is captured we
                // remove the pointer and check whether the gesture is
                // ending (count < 2) or merely losing one finger of
                // a > 2 group.
                let was_tracked = self
                    .pointers
                    .iter()
                    .any(|(id, _)| *id == event.pointer_id());
                let pre_retain_scale = self.current_scale();
                let pre_retain_rotation = self.pair_angle() - self.initial_angle;
                self.pointers.retain(|(id, _)| *id != event.pointer_id());
                if !was_tracked {
                    return GestureDisposition::Possible;
                }
                if self.state == ScaleState::Accepted && self.pointers.len() < 2 {
                    if let Some(cb) = self.on_end.as_mut() {
                        cb(
                            ScaleEndDetails {
                                scale: pre_retain_scale,
                                rotation: pre_retain_rotation,
                            },
                            window,
                            cx,
                        );
                    }
                    // Resolve via reset so the recognizer is ready
                    // for the next multi-pointer sequence.
                    self.reset();
                    return GestureDisposition::Accepted;
                }
                if self.pointers.is_empty() {
                    self.reset();
                }
                GestureDisposition::Possible
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
        // Scale wins via eager-accept on slop-crossing; sweep means
        // we never crossed slop — no callbacks. Reset so the next
        // multi-pointer sequence starts clean.
        self.reset();
    }

    fn rejected(
        &mut self,
        _pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        self.reset();
    }

    fn lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle> {
        Some(self)
    }
}

impl RecognizerLifecycle for ScaleGestureRecognizer {
    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.slop = settings.touch_slop;
    }
}

#[cfg(test)]
mod tests {
    //! T17 — Scale recognizer unit tests.

    use super::*;
    use crate::gesture::{
        DeliveredEvent, GestureSettings, PointerButtons, PointerEvent, PointerId, PointerKind,
        PointerPhase,
    };
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, TestAppContext};
    use std::cell::Cell;
    use std::rc::Rc;

    fn de(event: &PointerEvent) -> DeliveredEvent<'_> {
        DeliveredEvent::at_event_position(event)
    }

    fn pe(
        id: u64,
        phase: PointerPhase,
        pos: Point<Pixels>,
        buttons: PointerButtons,
    ) -> PointerEvent {
        let now = Instant::now();
        PointerEvent {
            pointer_id: PointerId(id),
            kind: PointerKind::Touch,
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

    fn pt(x: f32, y: f32) -> Point<Pixels> {
        Point::new(Pixels(x), Pixels(y))
    }

    /// Compile-time lock for B2 — threshold field `slop` stays `pub`.
    #[test]
    fn scale_threshold_fields_are_settable() {
        let s = GestureSettings::default();
        let mut r = ScaleGestureRecognizer::new(&s);
        r.slop = Pixels(25.0);
        assert_eq!(r.slop.0, 25.0);
    }

    #[flui_core::test]
    fn scale_single_pointer_does_not_engage(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let starts = Rc::new(Cell::new(0u32));
                    let mut scale = ScaleGestureRecognizer::new(&GestureSettings::default());
                    {
                        let starts = Rc::clone(&starts);
                        scale.on_start = Some(Box::new(move |_d, _w, _c| {
                            starts.set(starts.get() + 1);
                        }));
                    }
                    let d = pe(0, PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    scale.add_pointer(PointerId(0), de(&d));
                    let mv = pe(
                        0,
                        PointerPhase::Move,
                        pt(50.0, 50.0),
                        PointerButtons::PRIMARY,
                    );
                    assert_eq!(
                        scale.handle_event(de(&mv), window, cx),
                        GestureDisposition::Possible,
                        "single-pointer scale must not engage"
                    );
                    assert_eq!(starts.get(), 0);
                });
        });
    }

    #[flui_core::test]
    fn scale_two_pointers_diverging_accepts_after_slop(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let starts = Rc::new(Cell::new(0u32));
                    let mut scale = ScaleGestureRecognizer::new(&GestureSettings::default());
                    {
                        let starts = Rc::clone(&starts);
                        scale.on_start = Some(Box::new(move |d, _w, _c| {
                            starts.set(starts.get() + 1);
                            // After Move that crosses slop, focal_point
                            // is the average of the *current* pointer
                            // positions: (0,0) and (150,0) → (75,0).
                            assert!(
                                (d.focal_point.x.0 - 75.0).abs() < 1e-3,
                                "expected focal.x=75, got {}",
                                d.focal_point.x.0
                            );
                            assert_eq!(d.pointer_count, 2);
                            // Regression lock for the scale
                            // `initial_kind` bug: the recognizer must
                            // carry the kind of the first pointer
                            // through to `ScaleStartDetails.kind`.
                            // The previous `is_empty()` check after
                            // `pointers.push(...)` was unreachable, so
                            // `initial_kind` stayed at its `Mouse`
                            // default for any device.
                            assert_eq!(
                                d.kind,
                                PointerKind::Touch,
                                "ScaleStartDetails.kind must mirror the \
                                 first pointer (Touch in this test); the \
                                 default Mouse means the initial_kind \
                                 atomic-write fix regressed",
                            );
                        }));
                    }
                    // p0 at (0,0), p1 at (100,0) → initial distance 100.
                    let d0 = pe(0, PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    let d1 = pe(
                        1,
                        PointerPhase::Down,
                        pt(100.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    scale.add_pointer(PointerId(0), de(&d0));
                    scale.add_pointer(PointerId(1), de(&d1));
                    // Move p1 outward to 150 → new distance 150 → delta 50 > slop 18.
                    let m1 = pe(
                        1,
                        PointerPhase::Move,
                        pt(150.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    assert_eq!(
                        scale.handle_event(de(&m1), window, cx),
                        GestureDisposition::Accepted,
                    );
                    assert_eq!(starts.get(), 1);
                });
        });
    }

    #[flui_core::test]
    fn scale_update_computes_correct_zoom_ratio(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let last_scale: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
                    let mut scale = ScaleGestureRecognizer::new(&GestureSettings::default());
                    {
                        let last = Rc::clone(&last_scale);
                        scale.on_update = Some(Box::new(move |d, _w, _c| {
                            last.set(d.scale);
                        }));
                    }
                    let d0 = pe(0, PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    let d1 = pe(
                        1,
                        PointerPhase::Down,
                        pt(100.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    scale.add_pointer(PointerId(0), de(&d0));
                    scale.add_pointer(PointerId(1), de(&d1));
                    // Slop crossing → Accepted (no on_update yet).
                    let m1a = pe(
                        1,
                        PointerPhase::Move,
                        pt(150.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    let _ = scale.handle_event(de(&m1a), window, cx);
                    // Now in Accepted; move p1 to 200 → distance 200, ratio 2.0.
                    let m1b = pe(
                        1,
                        PointerPhase::Move,
                        pt(200.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    let _ = scale.handle_event(de(&m1b), window, cx);
                    let s = last_scale.get();
                    assert!(
                        (s - 2.0).abs() < 1e-3,
                        "expected scale = 2.0 after distance 100→200, got {}",
                        s
                    );
                });
        });
    }

    #[flui_core::test]
    fn scale_end_fires_when_pointer_count_drops_below_two(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let ends = Rc::new(Cell::new(0u32));
                    let mut scale = ScaleGestureRecognizer::new(&GestureSettings::default());
                    {
                        let ends = Rc::clone(&ends);
                        scale.on_end = Some(Box::new(move |_d, _w, _c| {
                            ends.set(ends.get() + 1);
                        }));
                    }
                    let d0 = pe(0, PointerPhase::Down, pt(0.0, 0.0), PointerButtons::PRIMARY);
                    let d1 = pe(
                        1,
                        PointerPhase::Down,
                        pt(100.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    scale.add_pointer(PointerId(0), de(&d0));
                    scale.add_pointer(PointerId(1), de(&d1));
                    // Cross slop to enter Accepted.
                    let m1 = pe(
                        1,
                        PointerPhase::Move,
                        pt(150.0, 0.0),
                        PointerButtons::PRIMARY,
                    );
                    let _ = scale.handle_event(de(&m1), window, cx);
                    // Lift p1 → pointer_count drops to 1 → on_end fires.
                    let up = pe(
                        1,
                        PointerPhase::Up,
                        pt(150.0, 0.0),
                        PointerButtons::default(),
                    );
                    assert_eq!(
                        scale.handle_event(de(&up), window, cx),
                        GestureDisposition::Accepted,
                    );
                    assert_eq!(ends.get(), 1);
                });
        });
    }
}
