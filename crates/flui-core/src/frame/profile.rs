//! Frame telemetry: [`FrameProfile`] (always-on, ~32 bytes) and
//! [`FrameProfileDetailed`] (flag-gated, full per-phase Durations).
//!
//! `FrameProfile` is in the [`prelude`](crate::prelude) and is populated by every
//! `App::run_frame` call. `FrameProfileDetailed` is populated only when
//! `App::set_profiling_enabled(true)`; default is `cfg!(debug_assertions)`.
//!
//! Per `docs/promt.md` §3.1, the committed dispatch / tick / paint paths must NOT
//! pay measurement cost in release builds unless profiling is explicitly enabled.
//!
//! `FrameProfile` layers on top of, not in place of, the existing
//! `measure("frame duration", ...)` macro at `window.rs:1294` and
//! `#[profiling::function]` annotations.

use std::time::Duration;

use smallvec::SmallVec;

use super::{FramePhase, FramePhaseSet};

/// Always-on per-frame telemetry. Populated by every `App::run_frame` call.
///
/// Sized for cheapness — roughly 32 bytes — so reading it on every frame from a
/// debug HUD or inspector imposes negligible overhead. Detailed per-phase
/// timings live on [`FrameProfileDetailed`] which is allocated lazily and only
/// populated when `App::set_profiling_enabled(true)`.
///
/// # Stability
///
/// `#[non_exhaustive]` — future K22 inspector / R-track work may add fields
/// (dropped-frame estimation, GPU queue depth, paint primitive breakdown).
/// Downstream code must construct via [`FrameProfile::default()`] and never
/// rely on field ordering.
///
/// # Hot-path discipline
///
/// Population happens once per `App::run_frame`. Phase code does NOT update
/// this struct field-by-field on the hot path — values are written at frame
/// boundaries in `run_frame`.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameProfile {
    /// Monotonic frame counter — matches `FrameClock::frame_index()` at the
    /// moment the frame ran. `0` before the first frame, advances at every
    /// `App::run_frame` start.
    pub frame_index: u64,

    /// Total wall-clock duration of the frame, from `PreFrame` entry to
    /// `PostFrame` exit. `Duration::ZERO` in release builds when profiling is
    /// disabled — the measurement is skipped to honor the hot-path discipline.
    pub frame_duration_total: Duration,

    /// Number of [`TickTarget`](super::tick::TickTarget) entries the
    /// `AnimationTick` phase visited this frame. Cheap (one `len()` on the
    /// active-set).
    pub active_animations: usize,

    /// Scene-primitive count produced by `Paint`. Sampled from the active
    /// `Window`'s scene at the end of `Paint`. `0` outside `App::run_frame`.
    pub primitive_count: u32,

    /// Set of phases that exceeded their advisory deadline this frame. The
    /// `EffectFlush` interleave uses break-and-requeue and does NOT set bits
    /// here — its requeue count is tracked in [`FrameProfileDetailed`] instead.
    pub overruns: FramePhaseSet,

    /// Reserved for K22 drift detection. Always `0` in K04 — populated by a
    /// later spec that observes the gap between `FrameClock::frame_index()` and
    /// expected display-vsync ticks.
    pub dropped_frames: u32,
}

impl FrameProfile {
    /// Returns `true` if any non-effect phase exceeded its advisory deadline in
    /// the recorded frame.
    #[inline]
    pub fn had_overrun(&self) -> bool {
        !self.overruns.is_empty()
    }
}

/// Flag-gated detailed per-frame telemetry. Populated only when
/// `App::set_profiling_enabled(true)` (default `cfg!(debug_assertions)`).
///
/// Read via `App::frame_profile_detailed() -> Option<&FrameProfileDetailed>`.
/// Returns `None` when profiling is disabled — the cost of populating the
/// per-phase `Duration` array is avoided in release builds by default.
///
/// # Stability
///
/// `#[non_exhaustive]` — future K22 / R-track specs may add custom-metric
/// registries.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameProfileDetailed {
    /// Per-phase durations, indexed by [`FramePhase::as_index()`].
    /// Size is exactly [`FramePhase::COUNT`].
    ///
    /// Phases that did not run (e.g. `Build` while reserved as no-op in K04)
    /// contain `Duration::ZERO`.
    pub per_phase: [Duration; FramePhase::COUNT],

    /// Total number of effects drained during interleaved `EffectFlush` work
    /// in this frame.
    pub effect_drain_count: u32,

    /// Number of effects requeued because the `EffectFlush` budget was exceeded
    /// at a phase boundary. Non-zero indicates the application is generating
    /// effects faster than the deadline allows.
    pub effect_drain_requeued: u32,

    /// Phases (with measured overshoot) that exceeded their advisory deadline.
    /// Mirrors `FrameProfile.overruns` but carries the actual overshoot
    /// magnitude. Sized to handle up to 4 simultaneous overruns inline before
    /// spilling to the heap — frames with more overruns than that are already
    /// pathological.
    pub deadline_overrun: SmallVec<[(FramePhase, Duration); 4]>,
}

impl FrameProfileDetailed {
    /// Returns the recorded duration of `phase`. `Duration::ZERO` if the phase
    /// did not run in this frame (e.g. reserved `Build`).
    #[inline]
    pub fn phase_duration(&self, phase: FramePhase) -> Duration {
        self.per_phase[phase.as_index() as usize]
    }

    /// Resets the struct to all zeros / empty. Called by `App::run_frame` at
    /// the start of each frame when profiling is enabled.
    ///
    /// Wired by `App::run_frame` (Task 23). The `dead_code` allow is part of
    /// the K04 staged rollout — every accessor lands first, then `run_frame`
    /// consumes them.
    #[allow(dead_code)]
    pub(crate) fn reset(&mut self) {
        self.per_phase = [Duration::ZERO; FramePhase::COUNT];
        self.effect_drain_count = 0;
        self.effect_drain_requeued = 0;
        self.deadline_overrun.clear();
    }
}
