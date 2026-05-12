//! Per-frame time substrate: [`FrameClock`] and [`FrameClockView`].
//!
//! [`FrameClock`] samples the underlying [`Clock`](crate::scheduler::Clock) exactly
//! once at the start of each `App::run_frame` call (K04 axiom P3). All consumers in
//! that frame — animation tick, `AnimationController::value()`, post-frame
//! callbacks, layout cache hash keys — read the same `Instant` for the duration of
//! the frame.
//!
//! [`FrameClockView`] is an opaque copy snapshot returned by
//! `Window::frame_clock_view()`. Today it always reflects the App-wide clock; the
//! indirection is reserved so a future R-track / Wasm spec can introduce per-window
//! epoch divergence (tab visibility, iOS background scene) without a SemVer break.
//!
//! # Hot-path discipline
//!
//! `FrameClock` is `!Send` (lives on `App`, which is `!Send`). It wraps
//! `Arc<dyn Clock>` and never calls `Clock::now()` more than once per frame —
//! `begin_frame()` is the single sampling point, called by `App::run_frame`.
//!
//! # Outside-of-frame behavior
//!
//! `FrameClock::now()` and `FrameClockView::now()` are designed to be called
//! ONLY inside a frame. In `cfg(debug_assertions)` builds, calling either outside
//! a frame triggers a debug assertion. In release builds, both return the
//! last-sampled `Instant` (the most recent `begin_frame` value), preventing
//! a panic but signalling a bug via the `FrameProfile`.

use crate::scheduler::{Clock, Instant};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// App-level per-frame clock. Samples the underlying [`Clock`] once per
/// `App::run_frame` and serves the same `Instant` to every consumer for the
/// duration of the frame.
///
/// Lives on `App` and is `!Send` (never crosses threads). Layered on top of —
/// not parallel to — the existing `Arc<dyn Clock>` substrate.
///
/// Construction is `pub(crate)` (`App::new` owns the only `FrameClock`).
/// Read access is via the public methods on this struct or via the snapshot
/// [`FrameClockView`] returned by `Window::frame_clock_view()`.
///
/// # Determinism (axiom P3 + P9)
///
/// Because the clock is sampled once per frame, animation curves are reproducible
/// when driven by a `TestClock`; layout cache hash keys do not false-miss within a
/// frame; multiple `AnimationController::value()` reads return the same result.
///
/// Wasm-compatible because the underlying `Instant` is `web_time::Instant`
/// re-exported through [`crate::scheduler::Instant`].
pub struct FrameClock {
    /// Underlying time source. Injected at construction time; never mutated.
    clock: Arc<dyn Clock>,

    /// `Some(sampled_at)` while a frame is in flight (between `begin_frame()` and
    /// `end_frame()`); `None` during Idle. Drives [`FrameClock::in_frame()`].
    in_frame_sample: Option<Instant>,

    /// Most recent value sampled by `begin_frame()`. Always populated after the
    /// first frame; before the first frame, holds the construction-time sample.
    /// Used by [`FrameClock::now()`] when called outside a frame in release mode
    /// (debug builds assert instead).
    last_sampled: Instant,

    /// Time delta from the previous frame's `begin_frame` to the current.
    /// `Duration::ZERO` before the first frame and on the very first frame
    /// (no previous reference).
    last_delta: Duration,

    /// Monotonic frame counter. `0` before any frame has begun; incremented at
    /// the start of each `begin_frame()`. The first observable frame has
    /// `frame_index == 1`.
    frame_index: u64,

    /// `!Send + !Sync` marker — `FrameClock` lives on `App` and must not cross
    /// threads. The `Arc<dyn Clock>` field itself is `Send + Sync`, so without
    /// this marker the struct would inherit `Send`.
    _not_send: PhantomData<*const ()>,
}

impl FrameClock {
    /// Constructs a new `FrameClock` from the given [`Clock`] substrate.
    /// Samples the clock once at construction time to populate `last_sampled`,
    /// so `now()` in release builds has a sane initial value even when called
    /// before the first frame fires.
    ///
    /// `pub(crate)` — `App::new` owns the only `FrameClock`; downstream code
    /// reaches it via `App::frame_clock()`.
    pub(crate) fn new(clock: Arc<dyn Clock>) -> Self {
        let now = clock.now();
        Self {
            clock,
            in_frame_sample: None,
            last_sampled: now,
            last_delta: Duration::ZERO,
            frame_index: 0,
            _not_send: PhantomData,
        }
    }

    /// Begins a new frame. Called by `App::run_frame` at the start of the
    /// `PreFrame` phase. Samples `Clock::now()` exactly once, advances the
    /// frame index, and records the delta from the previous frame.
    ///
    /// After this call, [`in_frame()`](Self::in_frame) returns `true` and
    /// [`now()`](Self::now) returns the just-sampled `Instant`.
    ///
    /// `pub(crate)` — only `App::run_frame` (and the test-mode `TestApp::advance_frame`)
    /// may begin a frame.
    pub(crate) fn begin_frame(&mut self) {
        let now = self.clock.now();
        // First frame: no previous reference, delta stays zero.
        let delta = if self.frame_index == 0 {
            Duration::ZERO
        } else {
            now.saturating_duration_since(self.last_sampled)
        };
        self.last_sampled = now;
        self.last_delta = delta;
        self.in_frame_sample = Some(now);
        // Saturating add: `u64` overflow after 2^64 frames is theoretical, but
        // `wrapping_add` would silently corrupt telemetry; saturating preserves
        // ordering across overflow.
        self.frame_index = self.frame_index.saturating_add(1);
    }

    /// Ends the current frame. Called by `App::run_frame` after `PostFrame`
    /// completes. Flips [`in_frame()`](Self::in_frame) back to `false` but
    /// preserves `last_sampled`, `last_delta`, and `frame_index` so post-frame
    /// telemetry and panic-recovery paths can still read the panicked frame's
    /// state.
    ///
    /// `pub(crate)` — symmetrical with [`begin_frame()`](Self::begin_frame).
    pub(crate) fn end_frame(&mut self) {
        self.in_frame_sample = None;
    }

    /// Resets `in_frame()` to `false` without advancing any other state.
    /// Called by `App::abort_frame_after_panic` so post-panic code observes
    /// `Idle` rather than a stuck mid-frame state. Preserves `last_sampled`
    /// and `frame_index` — those stay "stuck dirty" per the K04 panic-safety
    /// contract (Decision D9 in the design spec).
    ///
    /// `pub(crate)` — only `App` panic-recovery paths invoke this.
    pub(crate) fn abort_frame(&mut self) {
        self.in_frame_sample = None;
    }

    /// Returns the `Instant` sampled at the start of the current frame.
    ///
    /// # Behavior outside a frame
    ///
    /// - In `cfg(debug_assertions)`: triggers a debug assertion (recommends
    ///   the caller migrate to reading non-frame time some other way).
    /// - In release: returns `last_sampled` (the most recent `begin_frame`
    ///   value), preventing a panic. The caller likely has a bug, but the
    ///   App stays responsive.
    #[inline]
    pub fn now(&self) -> Instant {
        match self.in_frame_sample {
            Some(t) => t,
            None => {
                debug_assert!(
                    false,
                    "FrameClock::now() called outside a frame (FramePhase::Idle); \
                     use App::frame_clock().last_sampled() if you really need \
                     non-frame time, or schedule the work via App::defer_to(...)"
                );
                self.last_sampled
            }
        }
    }

    /// Returns the monotonic frame counter.
    ///
    /// - `0` before the first frame has begun.
    /// - Incremented at the start of each `begin_frame()`. The first observable
    ///   frame has `frame_index == 1`.
    /// - Stable across `end_frame()` and panic recovery — preserves "stuck dirty"
    ///   semantics per the panic-safety contract.
    ///
    /// Telemetry consumers (`FrameProfile.frame_index`) read this directly.
    #[inline]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Returns the duration between the previous frame's `begin_frame` and the
    /// current frame's `begin_frame`. `Duration::ZERO` before the first frame
    /// and on the very first frame (no previous reference).
    ///
    /// Drives animation `AnimationController` per-frame advance.
    #[inline]
    pub fn delta(&self) -> Duration {
        self.last_delta
    }

    /// Returns `true` between [`begin_frame()`](Self::begin_frame) and the
    /// matching [`end_frame()`](Self::end_frame) / [`abort_frame()`](Self::abort_frame).
    ///
    /// `false` during `FramePhase::Idle`.
    #[inline]
    pub fn in_frame(&self) -> bool {
        self.in_frame_sample.is_some()
    }

    /// Returns the most recent `begin_frame` sample, even if no frame is
    /// currently in flight. Use sparingly — most code wants [`now()`](Self::now)
    /// instead.
    ///
    /// Stable across `end_frame()` and panic recovery (per the K04 panic-safety
    /// contract — `frame_clock.last_sampled` is in the "stuck dirty" set).
    #[inline]
    pub fn last_sampled(&self) -> Instant {
        self.last_sampled
    }

    /// Returns an opaque copy [`FrameClockView`] snapshot. Future per-window
    /// epoch divergence (R-track / Wasm) will adjust this method on `Window` to
    /// return a window-local view without changing this struct's API.
    #[inline]
    pub fn view(&self) -> FrameClockView {
        FrameClockView {
            sampled: self.in_frame_sample,
            last_sampled: self.last_sampled,
            frame_index: self.frame_index,
            delta: self.last_delta,
        }
    }
}

/// Opaque copy snapshot of a [`FrameClock`]. Returned by `Window::frame_clock_view()`.
///
/// Today always reflects the App-wide clock at the moment the view was taken;
/// the indirection is reserved so a future R-track / Wasm spec can introduce
/// per-window epoch divergence (tab visibility, iOS background scene) without a
/// SemVer break.
///
/// `Copy + Clone + Debug`. No lifetime parameter — the view is a self-contained
/// snapshot and may outlive the borrow that produced it. Consumers that need
/// fresh values must re-take the view via `Window::frame_clock_view()`.
#[derive(Copy, Clone, Debug)]
pub struct FrameClockView {
    /// `Some(sampled_at)` if a frame was in flight when the view was taken.
    sampled: Option<Instant>,
    /// Always populated; mirrors `FrameClock::last_sampled`.
    last_sampled: Instant,
    /// Mirrors `FrameClock::frame_index`.
    frame_index: u64,
    /// Mirrors `FrameClock::delta`.
    delta: Duration,
}

impl FrameClockView {
    /// Returns the `Instant` sampled at the start of the current frame (the
    /// frame in flight when this view was taken).
    ///
    /// # Behavior outside a frame
    ///
    /// Same as [`FrameClock::now()`] — debug-assert in debug builds, fall back
    /// to `last_sampled` in release builds.
    #[inline]
    pub fn now(&self) -> Instant {
        match self.sampled {
            Some(t) => t,
            None => {
                debug_assert!(
                    false,
                    "FrameClockView::now() called outside a frame; \
                     use last_sampled() if you really need non-frame time"
                );
                self.last_sampled
            }
        }
    }

    /// Mirrors [`FrameClock::frame_index()`].
    #[inline]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Mirrors [`FrameClock::delta()`].
    #[inline]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Mirrors [`FrameClock::in_frame()`].
    #[inline]
    pub fn in_frame(&self) -> bool {
        self.sampled.is_some()
    }

    /// Mirrors [`FrameClock::last_sampled()`].
    #[inline]
    pub fn last_sampled(&self) -> Instant {
        self.last_sampled
    }
}
