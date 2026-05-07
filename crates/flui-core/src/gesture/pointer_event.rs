//! `PointerEvent`, `PointerKind`, `PointerPhase`, `PointerId`,
//! `PointerButtons`. The normalized wire format produced by
//! [`crate::gesture::dispatch`] from `PlatformInput`.
//!
//! See the design doc at
//! `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! § "Design — PointerEvent" for the full surface.

use crate::scheduler::Instant;
use crate::{Modifiers, Pixels, Point};

/// A unique, monotonically-increasing identifier for a single pointer
/// from the time it enters the window until the time it leaves.
///
/// On mouse-only platforms, the same `PointerId` is reused across
/// down/up sequences (one mouse cursor = one pointer); on multi-touch
/// platforms a new `PointerId` is allocated for each touch contact.
///
/// The inner `u64` is `pub(crate)` — construction goes through
/// `crate::gesture::dispatch`, which holds the per-`Window` allocator.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PointerId(pub(crate) u64);

impl PointerId {
    /// Raw inner value. For logging (`log::*` `kv` field
    /// `pointer_id`) and `serde` round-tripping; not for normal logic.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// The kind of input device that produced a [`PointerEvent`].
///
/// `#[non_exhaustive]` so future device kinds (e.g. eye-tracking) are
/// non-breaking additions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[non_exhaustive]
pub enum PointerKind {
    /// A standard mouse cursor.
    #[default]
    Mouse,
    /// A multi-touch contact (finger). Only emitted on platforms that
    /// surface real touch events (currently macOS trackpad + Wayland;
    /// Windows desktop touch is deferred — see the design doc's
    /// "Explicit gaps" matrix).
    Touch,
    /// A stylus / pen contact. The platform layer does not currently
    /// emit this variant — it is reserved for forward-compatibility
    /// with S20 desktop-gaps cleanup. The `tilt` and `orientation`
    /// fields on [`PointerEvent`] are zero for non-stylus pointers.
    Stylus,
}

/// The lifecycle phase of a [`PointerEvent`].
///
/// `#[non_exhaustive]` so future phases (e.g. `PanZoomStart` for
/// trackpad gestures) are non-breaking additions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum PointerPhase {
    /// The pointer device became known to the application (cursor
    /// entered the window, finger touched the screen, …). Carries no
    /// contact information yet.
    Added,
    /// The pointer is now in contact (button pressed, finger down).
    Down,
    /// The pointer moved while in contact.
    Move,
    /// The pointer left contact (button released, finger up).
    Up,
    /// The platform / sanitizer cancelled this gesture sequence (e.g.
    /// focus loss, modal switch, orphan-`Down` sanitization).
    Cancel,
    /// The pointer device left the application surface (mouse cursor
    /// exited the window, multi-touch finger lifted off, stylus left
    /// the digitizer area). **Per-target** leave events are
    /// synthesized as [`Self::Exit`] instead — the two phases are
    /// orthogonal and recognizers must distinguish them:
    /// - `Removed` = "the device is gone"; recognizers should drop
    ///   any in-flight gesture for this pointer.
    /// - `Exit`    = "the pointer moved off this target"; the device
    ///   is still present, possibly hovering a sibling.
    Removed,
    /// Hover-only motion; no contact, no buttons. Mouse-class only.
    Hover,
    /// The pointer entered a new hit-test target during hover.
    /// Synthesized from `Hover` by `crate::gesture::dispatch` (frame-to-frame diff).
    Enter,
    /// The pointer left a hit-test target during hover. Synthesized
    /// from `Hover` by `crate::gesture::dispatch`. Distinct from
    /// [`Self::Removed`] — see that variant's note for the per-target
    /// vs per-device distinction.
    Exit,
}

/// A bitfield of currently-pressed buttons.
///
/// Modeled after Flutter's `kPrimaryButton` / `kSecondaryButton` /
/// `kTertiaryButton` constants; values match Flutter's Dart layer.
///
/// The inner `u32` is `pub(crate)` to prevent downstream code from
/// constructing arbitrary bit patterns that bypass the documented
/// constant set. Use the associated constants ([`Self::PRIMARY`],
/// [`Self::SECONDARY`], [`Self::TERTIARY`]) plus [`Self::bits`] /
/// [`Self::contains`] / [`Self::is_empty`] for inspection.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PointerButtons(pub(crate) u32);

impl PointerButtons {
    /// The primary button (left mouse, single-finger tap, stylus tip).
    pub const PRIMARY: Self = Self(0x01);
    /// The secondary button (right mouse, two-finger touch, stylus
    /// barrel).
    pub const SECONDARY: Self = Self(0x02);
    /// The tertiary button (middle mouse, three-finger touch).
    pub const TERTIARY: Self = Self(0x04);

    /// Raw bit-pattern. Use this only for `serde` round-tripping or
    /// FFI; for normal logic prefer [`Self::contains`].
    pub fn bits(self) -> u32 {
        self.0
    }

    /// `true` iff `other`'s bits are all set in `self`.
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0 && other.0 != 0
    }

    /// `true` iff no buttons are pressed.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Combine two button bitfields (set union).
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A normalized pointer event, produced from `PlatformInput` by
/// [`crate::gesture::dispatch`] and consumed by recognizers.
///
/// Construction goes through the conversion helpers in
/// [`crate::gesture::dispatch`]; users do not construct `PointerEvent`
/// directly. The struct is `#[non_exhaustive]` so adding fields
/// (`azimuth` for stylus, `device_id` for multi-monitor pointers in
/// future S20 work) is non-breaking.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PointerEvent {
    /// Per-pointer unique identifier (stable across `Down`→`Up`).
    pub pointer_id: PointerId,
    /// The device kind that produced this event.
    pub kind: PointerKind,
    /// The lifecycle phase.
    pub phase: PointerPhase,
    /// Position in window-local logical pixels.
    pub position: Point<Pixels>,
    /// Movement delta since the previous event for the same pointer.
    pub delta: Point<Pixels>,
    /// Currently-pressed buttons. `is_empty()` for hover/exit phases.
    pub buttons: PointerButtons,
    /// Currently-held keyboard modifiers (snapshot at event time).
    pub modifiers: Modifiers,
    /// Wall-clock timestamp at the time the platform layer produced
    /// the underlying `PlatformInput`.
    pub timestamp: Instant,
    /// Normalized 0.0..=1.0 contact pressure. Mouse-class events have
    /// `pressure = 0.0` for `Up`/`Hover`/`Removed` and `1.0` for
    /// `Down`/`Move`. Real pressure values arrive only via
    /// `MousePressureEvent` (macOS-trackpad-only today).
    pub pressure: f32,
    /// Stylus tilt (radians). Zero for non-stylus pointers (always
    /// today; reserved for forward-compat).
    pub tilt: f32,
    /// Stylus rotation (radians). Zero for non-stylus pointers.
    pub orientation: f32,
}
