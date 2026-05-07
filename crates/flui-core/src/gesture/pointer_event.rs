//! `PointerEvent`, `PointerKind`, `PointerPhase`, `PointerId`,
//! `PointerButtons`. The normalized wire format produced by
//! [`crate::gesture::dispatch`] from `PlatformInput`.
//!
//! See the design doc at
//! `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! § "Design — PointerEvent" for the full surface.

use crate::scheduler::Instant;
use crate::{Modifiers, Pixels, Point};

/// A platform-reported pressure value with its raw device range.
///
/// `value` is the platform's raw pressure reading; `min` / `max` are the
/// device's reported range. Different devices report different ranges:
/// a Wacom pen may report `0..=8192`, a Force Touch trackpad reports
/// `0.0..=1.0`. Use [`Self::normalize`] to obtain a `[0.0, 1.0]` value
/// relative to the device's own range — that is the value gesture
/// recognizers should compare against threshold settings, **never** the
/// raw `value` field directly. Comparing raw `value` against a fixed
/// constant produces semantically different results across devices.
///
/// `#[non_exhaustive]` reserves space for future per-platform fields
/// (e.g. tangential pressure on stylus). Construction goes through the
/// platform-side conversion helpers in [`crate::gesture::dispatch`];
/// downstream users observe `PressureSample` only by reading the
/// `pressure: Option<PressureSample>` field on [`PointerEvent`].
///
/// **Auto-trait posture:** `Copy + Clone + Debug + PartialEq`. **Not**
/// `Eq` or `Hash` because the `f32` fields make those derivations
/// unsound (NaN does not equal itself).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PressureSample {
    /// Raw platform-reported pressure value. Always falls in
    /// `[min, max]` for honest platforms.
    pub value: f32,
    /// Minimum value the platform can report for this device.
    /// Often `0.0`; never assume it.
    pub min: f32,
    /// Maximum value the platform can report for this device.
    /// Often `1.0`; never assume it (Wacom pens commonly report
    /// `8192.0` or `4096.0`).
    pub max: f32,
}

impl PressureSample {
    /// Normalize the raw `value` against the device's `[min, max]`
    /// range, clamped to `[0.0, 1.0]`.
    ///
    /// Returns `0.0` if `max <= min` (degenerate range; defensive
    /// fallback rather than producing NaN). Threshold comparisons in
    /// gesture recognizers should be against this normalized value,
    /// never the raw `value` field.
    pub fn normalize(self) -> f32 {
        let range = self.max - self.min;
        if range > 0.0 {
            ((self.value - self.min) / range).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

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
    /// A stylus flipped to its eraser side. Distinct from `Stylus`
    /// because tablet apps commonly map eraser strokes to a
    /// foreground/background-erase brush. Not emitted by any current
    /// platform (S20 territory).
    InvertedStylus,
    /// The synthetic device behind native pan-zoom-rotate gestures
    /// (macOS trackpad two-finger gestures). Distinct from `Mouse`
    /// because pressure / wheel / button semantics differ. **Note for
    /// Windows:** trackpad cursor movement still emits `Mouse`;
    /// `Trackpad` is reserved for the dedicated pan-zoom synthetic
    /// device path that emits [`super::PointerPanZoomEvent`].
    Trackpad,
    /// The platform did not report a recognizable device kind.
    /// Recognizers can choose to gate on `kind != Unknown` for safety.
    Unknown,
}

/// The origin of a [`PointerEvent`].
///
/// `#[non_exhaustive]` — future variants `ResamplerSynthesized` (S07.7
/// pre-arena resampling) and `SemanticsSynthesized` (S08
/// accessibility-driven synthetic events) will be added.
///
/// Used to filter or distinguish events depending on whether they came
/// directly from the platform or were synthesized by a higher-level
/// pipeline component. A boolean flag was rejected in favour of an
/// enum because the resampler and semantics paths are semantically
/// distinct from sanitizer-synthesized hover Enter/Exit events.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[non_exhaustive]
pub enum PointerEventProvenance {
    /// Emitted directly by the platform layer.
    #[default]
    Platform,
    /// Synthesized by [`crate::gesture::dispatch::PointerSanitizer`]:
    /// per-target hover Enter/Exit, or orphan-Cancel events.
    SanitizerSynthesized,
    // S07.7 will add: ResamplerSynthesized,
    // S08 will add: SemanticsSynthesized,
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
    /// Time at which this event was *delivered* into the dispatcher.
    /// For platform-emitted events: equal to the underlying
    /// `PlatformInput`'s arrival time. For synthesized events: the
    /// boundary time at which the synthesis ran (e.g. resampler sample
    /// boundary, semantics-synthesis tick). Compare with
    /// [`Self::source_timestamp`] to recover the originating event time.
    pub timestamp: Instant,
    /// Time at which the *originating* platform event was produced.
    /// For non-synthesized events: equal to [`Self::timestamp`]. For
    /// resampler / semantics synthesized events: the timestamp of the
    /// underlying input the synthesis was based on.
    /// [`crate::gesture::velocity_tracker::VelocityTracker`] consumers
    /// (drag recognizers) MUST use `source_timestamp` so velocity
    /// estimates remain truthful across synthesis boundaries.
    pub source_timestamp: Instant,
    /// Origin of this event — platform vs synthesized by which
    /// pipeline stage. See [`PointerEventProvenance`].
    pub provenance: PointerEventProvenance,
    /// Optional contact pressure with the platform's raw range.
    ///
    /// `None` for devices that report no pressure (most desktop mouse
    /// events). `Some(_)` for stylus, touch, and macOS Force Touch
    /// (which surfaces through `MousePressureEvent` and is mapped to a
    /// `PressureSample { value, min: 0.0, max: 1.0 }` here).
    /// Recognizers MUST normalize via [`PressureSample::normalize`]
    /// before comparing against thresholds; comparing `value` directly
    /// against a fixed constant gives semantically different results
    /// across devices with different ranges.
    pub pressure: Option<PressureSample>,
    /// Stylus tilt (radians). Zero for non-stylus pointers (always
    /// today; reserved for forward-compat).
    pub tilt: f32,
    /// Stylus rotation (radians). Zero for non-stylus pointers.
    pub orientation: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S07.5b T8 — `PressureSample::normalize` is platform-agnostic:
    /// a Wacom 8192-level pen at half pressure and a Force-Touch
    /// trackpad at half pressure both report `0.5` after normalization,
    /// so a recognizer threshold of `0.4` means the same physical
    /// effort regardless of device range.
    #[test]
    fn pressure_sample_normalize_correct_for_wacom_range() {
        let wacom_half = PressureSample {
            value: 4096.0,
            min: 0.0,
            max: 8192.0,
        };
        assert!((wacom_half.normalize() - 0.5).abs() < 1e-6);

        let force_touch_half = PressureSample {
            value: 0.5,
            min: 0.0,
            max: 1.0,
        };
        assert!((force_touch_half.normalize() - 0.5).abs() < 1e-6);

        // Out-of-range values clamp.
        let over = PressureSample {
            value: 9000.0,
            min: 0.0,
            max: 8192.0,
        };
        assert_eq!(over.normalize(), 1.0);

        // Degenerate range yields 0.0 (no NaN propagation).
        let degenerate = PressureSample {
            value: 0.5,
            min: 1.0,
            max: 1.0,
        };
        assert_eq!(degenerate.normalize(), 0.0);
    }
}
