//! `PointerSignalEvent` — non-competitive pointer signals (scroll wheel,
//! magnify) that bypass the gesture arena entirely.
//!
//! See the design doc at
//! `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! § "Design — PointerSignalEvent".

use super::{PointerId, PointerKind};
use crate::scheduler::Instant;
use crate::{Modifiers, Pixels, Point};

/// A non-competitive signal from a pointer device (scroll, magnify).
/// Bypasses the gesture arena entirely.
///
/// Recognizers do not compete on signals — there is no winner of a
/// scroll-wheel tick. Signals are dispatched directly to the deepest
/// hit-test target with `Translucent` propagation per
/// `HitTestBehavior`.
///
/// `#[non_exhaustive]` to admit future signals (e.g. smart-zoom,
/// force-press) without breaking changes.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum PointerSignalEvent {
    /// A scroll-wheel / two-finger-pan tick.
    Scroll {
        /// The pointer that produced this signal. Mouse cursor on
        /// desktop; per-touch on multi-touch platforms.
        pointer_id: PointerId,
        /// The device kind.
        kind: PointerKind,
        /// Position in window-local logical pixels.
        position: Point<Pixels>,
        /// Scroll delta in window-local logical pixels.
        delta: Point<Pixels>,
        /// Currently-held keyboard modifiers.
        modifiers: Modifiers,
        /// Wall-clock timestamp at platform-emit time.
        timestamp: Instant,
    },
    /// A pinch-magnify tick. `scale_delta` is multiplicative
    /// (1.0 == no change). `rotation_rad` is **always 0.0** on current
    /// desktop platforms; the field exists for forward-compat with
    /// multi-pointer touch (Wayland's
    /// `pointer-gestures-unstable-v1`).
    Magnify {
        /// The pointer that produced this signal.
        pointer_id: PointerId,
        /// The device kind.
        kind: PointerKind,
        /// Position in window-local logical pixels (focal point).
        position: Point<Pixels>,
        /// Multiplicative scale delta. 1.0 == no change.
        scale_delta: f32,
        /// Rotation delta in radians. **Always 0.0 on current desktop
        /// platforms.**
        rotation_rad: f32,
        /// Currently-held keyboard modifiers.
        modifiers: Modifiers,
        /// Wall-clock timestamp at platform-emit time.
        timestamp: Instant,
    },
}
