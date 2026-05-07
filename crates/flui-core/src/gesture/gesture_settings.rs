//! `GestureSettings` — per-`Window` tunable thresholds for gesture
//! recognition.
//!
//! Flutter-parity defaults; mutable via `window.gesture_settings_mut()`
//! (the S14 MediaQuery seam).
//!
//! See the design doc § "GestureSettings".

use crate::Pixels;
use std::time::Duration;

/// Per-window tunable thresholds for gesture recognition.
///
/// `#[non_exhaustive]` so future thresholds are non-breaking. Use the
/// `Default` impl (Flutter-parity defaults) and overwrite individual
/// fields:
///
/// ```ignore
/// // window: &mut Window
/// window.gesture_settings_mut().long_press_timeout =
///     std::time::Duration::from_millis(800);
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct GestureSettings {
    /// Maximum movement before a tap is rejected. Flutter default: 18 logical px.
    pub touch_slop: Pixels,
    /// Slop along the locked axis for axis-locked drags. Flutter
    /// default: 18 logical px.
    pub pan_slop: Pixels,
    /// Maximum interval between two taps to count as a double-tap.
    /// Flutter default: 300 ms.
    pub double_tap_timeout: Duration,
    /// Minimum interval between two taps (avoids quad-emit on jittery
    /// hardware). Flutter default: 40 ms.
    pub double_tap_min_time: Duration,
    /// Hold duration before a long-press fires. Flutter default: 500 ms.
    pub long_press_timeout: Duration,
    /// Maximum movement before a long-press is rejected. Flutter
    /// default: 18 logical px.
    pub long_press_slop: Pixels,
    /// `VelocityTracker` max sample window age. Flutter default: 100 ms.
    pub velocity_tracker_window: Duration,
    /// `VelocityTracker` maximum sample buffer size. Flutter default: 20.
    pub velocity_tracker_samples: usize,
    /// Maximum spawn-to-flush latency budget for the LongPress async
    /// timer (the recognizer warns if exceeded). Default: 16 ms (one
    /// 60 Hz frame).
    pub long_press_timer_budget: Duration,
}

impl Default for GestureSettings {
    fn default() -> Self {
        Self {
            touch_slop: Pixels(18.0),
            pan_slop: Pixels(18.0),
            double_tap_timeout: Duration::from_millis(300),
            double_tap_min_time: Duration::from_millis(40),
            long_press_timeout: Duration::from_millis(500),
            long_press_slop: Pixels(18.0),
            velocity_tracker_window: Duration::from_millis(100),
            velocity_tracker_samples: 20,
            long_press_timer_budget: Duration::from_millis(16),
        }
    }
}
