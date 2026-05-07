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

use crate::gesture::{
    GestureDisposition, GestureRecognizer, GestureSettings, PointerEvent, PointerId, PointerKind,
    PointerPhase,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScaleState {
    Idle,
    Possible,
    Accepted,
    Rejected,
}

/// Multi-pointer scale recognizer.
#[non_exhaustive]
pub struct ScaleGestureRecognizer {
    pub on_start: Option<Box<dyn FnMut(ScaleStartDetails, &mut crate::Window, &mut crate::App)>>,
    pub on_update: Option<Box<dyn FnMut(ScaleUpdateDetails, &mut crate::Window, &mut crate::App)>>,
    pub on_end: Option<Box<dyn FnMut(ScaleEndDetails, &mut crate::Window, &mut crate::App)>>,
    pub(crate) slop: Pixels,

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
            state: ScaleState::Idle,
            pointers: SmallVec::new(),
            initial_distance: 0.0,
            initial_angle: 0.0,
            initial_kind: PointerKind::Mouse,
        }
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
}

impl GestureRecognizer for ScaleGestureRecognizer {
    fn name(&self) -> &'static str {
        "scale"
    }

    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent) {
        if self.state == ScaleState::Rejected {
            return;
        }
        if !self.pointers.iter().any(|(id, _)| *id == pointer_id) {
            self.pointers.push((pointer_id, event.position));
        }
        if self.pointers.is_empty() {
            self.initial_kind = event.kind;
        }
        if self.pointers.len() >= 2 && self.state == ScaleState::Idle {
            self.state = ScaleState::Possible;
            self.initial_distance = self.pair_distance();
            self.initial_angle = self.pair_angle();
        }
    }

    fn handle_event(
        &mut self,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition {
        match event.phase {
            PointerPhase::Move => {
                self.update_pointer(event.pointer_id, event.position);
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
                self.pointers.retain(|(id, _)| *id != event.pointer_id);
                if self.state == ScaleState::Accepted && self.pointers.len() < 2 {
                    let cur_distance = self.pair_distance();
                    let cur_angle = self.pair_angle();
                    let scale = if self.initial_distance > f32::EPSILON {
                        cur_distance / self.initial_distance
                    } else {
                        1.0
                    };
                    let rotation = cur_angle - self.initial_angle;
                    if let Some(cb) = self.on_end.as_mut() {
                        cb(
                            ScaleEndDetails { scale, rotation },
                            window,
                            cx,
                        );
                    }
                    self.state = ScaleState::Idle;
                    return GestureDisposition::Accepted;
                }
                if self.pointers.is_empty() {
                    self.state = ScaleState::Idle;
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
        // we never crossed slop — no callbacks.
    }

    fn rejected(
        &mut self,
        _pointer_id: PointerId,
        _window: &mut crate::Window,
        _cx: &mut crate::App,
    ) {
        self.state = ScaleState::Rejected;
        self.pointers.clear();
    }
}
