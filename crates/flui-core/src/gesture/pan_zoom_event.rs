//! `PointerPanZoomEvent` and `PanZoomPhase` — native trackpad
//! pan-zoom-rotate gesture event family.
//!
//! Distinct from [`super::PointerSignalEvent::Magnify`] (which is a
//! scalar-only delta) because pan-zoom carries pan + scale + rotation
//! tuples. Lives as a sibling type to `PointerEvent` and
//! `PointerSignalEvent` rather than as variants on `PointerPhase`
//! because the rich payload would require ~3 `Option`-typed fields on
//! every `PointerEvent` (~99% of which would be `None` for non-trackpad
//! input). Flutter splits these the same way.
//!
//! **Platform support today:** no platform layer wires this through to
//! recognizers. The type identity is committed for forward-compat with
//! S20 desktop-gaps cleanup (macOS native pan-zoom emission). Once that
//! lands, `ScaleGestureRecognizer` will consume `PointerPanZoomEvent`
//! directly rather than reconstructing scale from `PinchEvent`.

use crate::scheduler::Instant;
use crate::{Modifiers, Pixels, Point};

use super::pointer_event::{PointerEventProvenance, PointerId, PointerKind};

/// The lifecycle phase of a [`PointerPanZoomEvent`].
///
/// Pan-zoom gestures are a single sequence per contact: `Start` →
/// zero-or-more `Update` → `End`. Cancellation is signaled by `End`
/// with zero-magnitude pan/scale/rotation deltas.
///
/// `#[non_exhaustive]` reserves space for future cancellation-distinct
/// variants if a platform-specific signal arrives.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum PanZoomPhase {
    /// Initial contact. Subsequent `Update` events report deltas
    /// relative to this start position.
    Start,
    /// Mid-gesture update with non-zero pan, scale, or rotation delta
    /// since the previous event in the same sequence.
    Update,
    /// Final event in the sequence (contact released or platform
    /// declared the gesture done).
    End,
}

/// A native trackpad pan-zoom-rotate gesture event.
///
/// Emitted by platforms that surface pan + zoom + rotation as one
/// composite gesture (macOS two-finger trackpad gesture). Distinct
/// from a scalar [`super::PointerSignalEvent::Magnify`].
///
/// `#[non_exhaustive]` reserves space for future per-platform fields.
/// Construction is platform-side only; downstream observers consume
/// `PointerPanZoomEvent` through the dispatcher.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PointerPanZoomEvent {
    /// The device kind that produced this gesture. Typically
    /// [`PointerKind::Trackpad`].
    pub kind: PointerKind,
    /// Per-pointer identifier (the synthetic trackpad-gesture device
    /// gets its own `PointerId`).
    pub pointer_id: PointerId,
    /// Time at which this event was delivered into the dispatcher.
    /// See [`super::PointerEvent::timestamp`] for the synthesis-vs-source
    /// distinction.
    pub timestamp: Instant,
    /// Time at which the originating platform event was produced.
    /// For non-synthesized events: equal to [`Self::timestamp`].
    pub source_timestamp: Instant,
    /// Origin of this event — platform vs synthesized.
    pub provenance: PointerEventProvenance,
    /// Position at gesture start (window-local). Updates do not move
    /// the position; pan is reported separately via [`Self::pan`].
    pub position: Point<Pixels>,
    /// Pan delta in window-local pixels accumulated since gesture
    /// start. Zero on `Start`; cumulative on subsequent events.
    pub pan: Point<Pixels>,
    /// Scale factor accumulated since gesture start. `1.0` = no zoom.
    /// Multiplicative — `2.0` means 2× zoom from start.
    pub scale: f32,
    /// Rotation in radians accumulated since gesture start. Positive =
    /// counter-clockwise. `0.0` if the platform does not emit rotation.
    pub rotation: f32,
    /// Currently-held keyboard modifiers (snapshot at event time).
    pub modifiers: Modifiers,
    /// Lifecycle phase of this event in the gesture sequence.
    pub phase: PanZoomPhase,
}
