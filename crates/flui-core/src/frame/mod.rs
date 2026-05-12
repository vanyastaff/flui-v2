//! K04 effect/frame contract substrate.
//!
//! This module formalizes the App scheduler's frame pipeline as a typed, observable
//! seven-phase state machine with placement-aware effect placement, an App-level
//! [`FrameClock`], advisory per-phase deadlines, panic-safe phase wind-down, and a
//! two-layer telemetry surface ([`FrameProfile`] + [`FrameProfileDetailed`]).
//!
//! Authoritative spec:
//! `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md`.
//! Plan: `.ai-factory/plans/feature-K04-effect-frame-contract.md`.
//!
//! # The seven-phase contract
//!
//! ```text
//! Idle → PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame → Idle
//!                                                                                              │
//!                                       (EffectFlush interleaved at every phase boundary)      ▼
//! ```
//!
//! - [`FramePhase`] is `#[non_exhaustive]`; SF05 / R-track / future specs add variants
//!   additively without a SemVer break.
//! - [`FramePhase::Build`] is reserved as a no-op slot for SF05's `BuildOwner::flush_dirty()`.
//! - `EffectFlush` is not a phase — it is interleaved at phase boundaries, draining
//!   pending effects whose [`DeferPlacement`] matches the boundary.
//!
//! # 10-Year Contract Axioms
//!
//! The ten axioms (P1–P10) that anchor this design are cross-platform-convergent
//! invariants from Flutter `SchedulerBinding`, Compose `MonotonicFrameClock` +
//! Snapshot, HTML rAF + event loop, SwiftUI `CADisplayLink` + run-loop observer,
//! React Scheduler lanes, Bevy ECS Schedule v3, and Glenn Fiedler's injected-clock
//! determinism canon. See the design spec for the full table.
//!
//! The two axioms most relevant to consumers of this module:
//!
//! - **P3** — Time is sampled once per logical frame; everything in that frame sees
//!   the same `Instant`. [`FrameClock::now()`](clock::FrameClock::now) is the only
//!   sanctioned time source inside a frame. Direct `Instant::now()` in committed
//!   phase code is a lint.
//! - **P5** — Re-entry is admissible only through the documented queue.
//!   `cx.defer` / `cx.defer_to(placement, ...)` is the single K15 escape hatch.
//!
//! # Hot-path discipline
//!
//! Per `docs/promt.md` §3.1, the committed dispatch / tick / paint paths must NOT
//! log per element or per frame. Only the deadline-overrun `WARN` (rate-limited at
//! the phase level) and [`FrameProfile`]-flagged instrumentation are allowed in
//! committed code.
//!
//! [`FrameClock`]: clock::FrameClock
//! [`FrameProfile`]: profile::FrameProfile
//! [`FrameProfileDetailed`]: profile::FrameProfileDetailed

pub mod clock;
pub mod profile;
pub mod tick;

#[cfg(test)]
mod tests;

/// Logical frame phase executed by `App::run_frame`.
///
/// Phases observe each other in the order declared by the discriminants
/// (`Idle = 0`, `PreFrame = 1`, ..., `PostFrame = 7`). Per axiom **P2**, the order
/// is a logical contract — the implementation may collapse code paths internally,
/// but the observable order is binding and tests assert against it via
/// `App::current_phase()`, not via internal field inspection.
///
/// `EffectFlush` is **not** a variant of this enum. It is interleaved at phase
/// boundaries, draining `pending_effects` whose [`DeferPlacement`] matches the
/// boundary. See the module-level docs for the boundary table.
///
/// # Stability
///
/// `#[non_exhaustive]` — SF05 (`BuildOwner::flush_dirty()` fills [`Build`]),
/// R-track (`HotReload`), and other future specs may add variants. Downstream
/// match expressions must include a wildcard arm.
///
/// # Discriminants
///
/// `#[repr(u8)]` with stable discriminants. The discriminant order is the
/// observable phase order and must NEVER be reshuffled without a SemVer break.
/// Telemetry (`FrameProfile`, `FrameProfileDetailed.per_phase`) indexes arrays by
/// this discriminant.
///
/// [`Build`]: FramePhase::Build
#[non_exhaustive]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FramePhase {
    /// No frame is in flight. Application code can run; `cx.defer` callbacks with
    /// placement [`Idle`](DeferPlacement::Idle) and [`EndOfUpdate`](DeferPlacement::EndOfUpdate)
    /// drain here. The window invalidator can be set dirty without restriction.
    Idle = 0,

    /// Pre-frame callbacks run. The single platform-fired `request_frame`
    /// callback at this point would today fire `next_frame_callbacks` BEFORE
    /// `window.draw()` (see `window.rs:1275-1284`); K04 routes that drain
    /// through this phase. App-level `App::on_pre_frame` callbacks and
    /// Window-level `Window::on_pre_frame` callbacks both drain here.
    ///
    /// Forbidden: layout reads, paint, nested `update_window` on same window
    /// (K15 still applies). Allowed defers: [`PostFrame`](DeferPlacement::PostFrame),
    /// [`NextFrameStart`](DeferPlacement::NextFrameStart).
    PreFrame = 1,

    /// Animation tick. `App::active_animations` set is walked once; each
    /// [`TickTarget`](tick::TickTarget) is advanced with the per-frame
    /// `FrameClock::now()` (axiom P3). `TickTargets` returning
    /// `TickOutcome::Done` are removed from the active set.
    ///
    /// Forbidden: `AnimationController::start`/`stop` synchronously (queue via
    /// defer), `setState` (queue via defer in SF05).
    AnimationTick = 2,

    /// **Reserved no-op slot for SF05.** Phase enters and exits immediately in
    /// K04 — no work, no overrun. SF05 will fill this with
    /// `BuildOwner::flush_dirty()`. Reserving the slot now (rather than appending
    /// it in SF05) keeps the discriminant order stable; downstream tooling that
    /// indexes per-phase arrays does not need to migrate when SF05 lands.
    Build = 3,

    /// Layout (Taffy + mediaquery). Lookup-only against the future K20 layout
    /// cache; no scene primitive generation. Today's `Window::draw` body's
    /// prepaint pass runs here and in the subsequent [`Prepaint`].
    Layout = 4,

    /// Prepaint: bounds finalization, hitbox registration, `Interactivity::paint`,
    /// deferred-draw resolution. Internal `Window::DrawPhase::Prepaint` is a
    /// strict sub-state of this phase.
    ///
    /// Forbidden: GPU encode (deferred to [`Paint`]).
    Prepaint = 5,

    /// Paint: scene primitives generated, focus listener fan-out runs,
    /// `Window::present()` (`PlatformWindow::draw`) and `Window::complete_frame()`
    /// fire here. Internal `Window::DrawPhase::Paint` and `Window::DrawPhase::Focus`
    /// are strict sub-states of this phase.
    ///
    /// Forbidden: layout (already resolved); effect push (queue via defer).
    Paint = 6,

    /// Post-frame callbacks run. Inspector readout, telemetry export, App-level
    /// `App::on_post_frame` callbacks, Window-level `Window::on_post_frame`
    /// callbacks (the new K04 API, distinct from today's misleading-named
    /// `on_next_frame`), and SF08 future-settle callbacks drain here.
    /// `pending_effects` with placement [`PostFrame`](DeferPlacement::PostFrame)
    /// also drains here.
    ///
    /// Forbidden: element mutation, layout, paint (deferred to next frame).
    PostFrame = 7,
}

impl FramePhase {
    /// Number of variants in the enum. Used for sizing per-phase telemetry arrays
    /// (e.g. `FrameProfileDetailed.per_phase: [Duration; FramePhase::COUNT]`).
    ///
    /// Under `#[non_exhaustive]`, future specs adding variants bump `COUNT`.
    /// Downstream code that indexes per-phase arrays must use this constant
    /// rather than hardcoding the size.
    pub const COUNT: usize = 8;

    /// Returns the discriminant as a `u8` (`Idle = 0`, ..., `PostFrame = 7`).
    /// Stable; matches the `#[repr(u8)]` discriminant.
    #[inline]
    pub const fn as_index(self) -> u8 {
        self as u8
    }
}

/// Bitset of [`FramePhase`] values, used by `FrameProfile.overruns` to record
/// which phases exceeded their advisory deadline in the current frame without
/// heap allocation.
///
/// Represented as a `u32` for headroom — `FramePhase::COUNT` is 8 today but
/// `#[non_exhaustive]` may grow it. 32 bits leaves room for ~24 additional
/// variants without changing the storage type.
///
/// All operations are `O(1)`, branchless, allocation-free, and `const`-compatible
/// where useful.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FramePhaseSet(u32);

impl FramePhaseSet {
    /// Returns an empty set.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns `true` if `phase` is in the set.
    #[inline]
    pub const fn contains(&self, phase: FramePhase) -> bool {
        (self.0 & (1u32 << phase.as_index())) != 0
    }

    /// Inserts `phase` into the set. No-op if already present.
    #[inline]
    pub fn insert(&mut self, phase: FramePhase) {
        self.0 |= 1u32 << phase.as_index();
    }

    /// Removes `phase` from the set. No-op if absent.
    #[inline]
    pub fn remove(&mut self, phase: FramePhase) {
        self.0 &= !(1u32 << phase.as_index());
    }

    /// Returns `true` if the set has no phases.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Clears the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Number of phases in the set.
    #[inline]
    pub const fn len(&self) -> u32 {
        self.0.count_ones()
    }
}

/// Placement of a deferred callback in the seven-phase frame pipeline.
///
/// `Effect::Defer` carries a `DeferPlacement` field; `App::defer(f)` routes to
/// [`EndOfUpdate`](Self::EndOfUpdate) for back-compat (preserves today's
/// observable behavior for all 36+ existing `cx.defer` callsites).
/// `App::defer_to(placement, f)` is the new placement-aware API.
///
/// # Stability
///
/// `#[non_exhaustive]` — SF05 will add `BeforeBuild`, SF08 will add `AfterSettle`,
/// etc. Downstream match expressions must include a wildcard arm.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum DeferPlacement {
    /// **Default** — drains at the next phase boundary inside the current frame.
    /// Identical to today's `App::defer(f)` observable behavior. Every existing
    /// `cx.defer` callsite continues to work unchanged via this placement.
    #[default]
    EndOfUpdate,

    /// Drains at the start of the next frame's [`PreFrame`](FramePhase::PreFrame)
    /// phase. Use for work that wants the next frame's `FrameClock::now()` rather
    /// than the current frame's.
    NextFrameStart,

    /// Drains in the [`PostFrame`](FramePhase::PostFrame) phase of the current
    /// (or next, if outside a frame) frame. Use for work that wants to observe
    /// the resolved layout / painted scene before running.
    PostFrame,

    /// Drains in [`Idle`](FramePhase::Idle). May coalesce across multiple Idle
    /// entries — there is no guarantee that an `Idle`-placed callback runs before
    /// the next frame fires. Use for non-time-critical work (cache eviction,
    /// background bookkeeping).
    Idle,
}
