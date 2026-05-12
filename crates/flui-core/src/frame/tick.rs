//! Animation tick scheduling: sealed [`TickTarget`] trait, [`TickTargetId`]
//! handle, and [`TickOutcome`] return value.
//!
//! The `AnimationTick` phase of each frame walks an active-target set populated
//! by [`TickTarget`] implementors (initially only `AnimationController`). Inactive
//! targets are NOT visited — the cost of an animation-free frame is one
//! `FxHashSet::is_empty()` check (Task 30 wires the set on `App`).
//!
//! [`TickTarget`] is sealed via the crate-private `seal::Sealed` supertrait so
//! Tier-C cannot inject arbitrary tick targets in K04. Future SF08 (async widgets)
//! and audio / spring / particle controllers will add `impl TickTarget` for their
//! types additively; opening the trait (removing the sealing supertrait) is itself
//! an additive change.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::scheduler::Instant;

/// Outcome of one [`TickTarget::tick`] call. Drives the active-target set
/// retain policy used by the `AnimationTick` phase (Task 30).
///
/// # Stability
///
/// `#[non_exhaustive]` — future specs may add a third state (e.g. `Suspended`)
/// for animations paused by `MediaQuery.disableAnimations` or background-scene
/// throttling. Downstream code must include a wildcard arm.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TickOutcome {
    /// Keep this target in the active set for the next frame. The target
    /// has more work to do — typical for animations still mid-curve.
    Continue,

    /// Remove this target from the active set. The target is finished
    /// (animation reached its target value, simulation settled, etc.). The
    /// `AnimationTick` walker drops the entry after this tick.
    Done,
}

/// Opaque identifier for a [`TickTarget`]. Unique within the process — each
/// new target allocates a fresh `u64` from a monotonic atomic counter.
///
/// Used as the key in `App::active_animations` (Task 30) so the active set is
/// `FxHashSet<TickTargetId>` rather than a heap-allocated trait-object set.
/// The actual target objects live in their owning `Entity<T>`; the App's
/// active-set entry is the lookup key.
///
/// # Stability
///
/// `Copy + Clone + Debug + PartialEq + Eq + Hash + Ord` for use as map / set
/// keys. The inner `u64` is `pub(crate)` so downstream code cannot synthesize
/// or transmute IDs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TickTargetId(pub(crate) u64);

impl TickTargetId {
    /// Allocate a fresh, process-unique [`TickTargetId`].
    ///
    /// Backed by an atomic counter — safe to call from any thread, though
    /// in practice every K04 tick-target lives on the main thread (App is
    /// `!Send`).
    ///
    /// `pub(crate)` — only sealed implementors in `flui-core` may construct
    /// new IDs.
    pub(crate) fn allocate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        // `Relaxed` is sufficient — we only need atomicity, not ordering
        // across IDs.
        //
        // Counter starts at 1 so `0` is effectively a reserved sentinel
        // (never returned under non-overflowed conditions). On `u64`
        // wrap-around — which would take ~584 years at 1 ID / nanosecond —
        // a post-overflow `0` is theoretically possible; the cost of
        // adding `fetch_update` to skip `0` is not worth paying for a
        // never-occurring case.
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the underlying counter value. Internal — telemetry only.
    ///
    /// Wired by Task 30 (active-set telemetry). The `dead_code` allow is part
    /// of the K04 staged rollout — accessor lands first, walker consumes next.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}

/// Sealed trait for objects that participate in the per-frame
/// [`AnimationTick`](super::FramePhase::AnimationTick) phase.
///
/// In K04 the only implementor is `AnimationController`. Future K04+ specs
/// (SF08 async widgets, audio / spring / particle controllers) will add
/// implementations additively without an API break — sealing keeps Tier-C
/// from injecting arbitrary impls before the contract is widely battle-tested.
///
/// # Contract
///
/// - `id()` returns a stable, unique [`TickTargetId`] for the lifetime of the
///   target. Implementors store one allocated via [`TickTargetId::allocate()`]
///   in their constructor.
/// - `tick(now)` advances any internal state that needs to observe per-frame
///   wall-clock progress, and returns whether the target wants to stay in
///   the active set ([`TickOutcome::Continue`]) or be dropped
///   ([`TickOutcome::Done`]).
/// - `now` is the [`FrameClock::now()`](super::clock::FrameClock::now) value
///   sampled for the current frame. Per axiom P3, every consumer in the
///   frame sees the same `Instant`.
///
/// # Re-entry
///
/// The K15 contract applies: `tick()` MUST NOT call `cx.update_window` on
/// the same window that issued the tick. The walker (Task 30) provides only
/// `&mut self`, not `&mut App`, so the trait shape mechanically prevents the
/// most common re-entry mistake. Any side-effect a tick body wants to emit
/// (notify, refresh, persist) must go through `cx.defer_to(...)` in the
/// surrounding walker code, not inside `tick()`.
pub trait TickTarget: sealed::Sealed {
    /// Advances the target with the current frame's clock sample. Returns
    /// whether the target stays in the active set after this tick.
    ///
    /// `frame_index` is the monotonic `FrameClock::frame_index()` value for
    /// the current frame. Implementors that cache per-frame state should
    /// key the cache on this counter rather than on `now` — under
    /// `TestClock` two frames can share the same wall-clock `Instant`
    /// (no advance between ticks), which would defeat an `Instant`-keyed
    /// cache.
    fn tick(&mut self, frame_index: u64, now: Instant) -> TickOutcome;

    /// Returns the target's stable identifier. The same value is returned on
    /// every call for the lifetime of `self`.
    fn id(&self) -> TickTargetId;
}

/// Sealing module — only types declared in `flui-core` may implement
/// [`TickTarget`] (per K04 design decision D11). Adding new implementors
/// outside this crate is a hard compile error until SF08 lifts the seal.
mod sealed {
    pub trait Sealed {}

    // The only K04 implementor.
    impl Sealed for crate::animation::AnimationController {}
}
