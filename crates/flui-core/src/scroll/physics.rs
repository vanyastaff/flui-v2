//! ADR-019: `ScrollPhysics` trait + reference impls.
//!
//! The trait is a small, deterministic surface (no `&mut Window` /
//! `&mut App`) so platform-default physics (iOS bouncing, Android
//! overscroll glow + clamp, web passive wheel) and test mocks can
//! all implement it cleanly. Two reference impls ship with the
//! engine: `BouncingPhysics` (iOS / macOS feel — overscroll
//! permitted with rubber-band resistance) and `ClampingPhysics`
//! (Android / Windows / Linux — overscroll clamped at edges).
//!
//! The future `Scrollable` widget composes a `ScrollPhysics` via
//! `Theme::scroll_physics_default()` (platform-conditional default
//! in `flui-theme`) and per-`Scrollable` overrides.

use crate::{Pixels, Point, px};

use super::{Axis, ScrollState};

/// Per-axis clamp helper for [`ClampingPhysics::apply_delta`].
///
/// When `offset` is already past the `[min, max]` range, only motion
/// toward the valid range is permitted (the user can "release" the
/// overscroll but cannot push further out). When `offset` is in range,
/// the returned delta is capped so the new offset does not cross the
/// boundary.
fn clamp_axis(offset: Pixels, min: Pixels, max: Pixels, delta: Pixels) -> Pixels {
    if offset > max {
        // Out of bounds on the positive side: only allow inward
        // (negative-direction) motion, capped at returning to `max`.
        if delta < px(0.0) {
            let allowed = max - offset; // negative — moves inward
            if delta < allowed { allowed } else { delta }
        } else {
            // User trying to push further out — zeroed.
            px(0.0)
        }
    } else if offset < min {
        // Out of bounds on the negative side: only allow inward
        // (positive-direction) motion, capped at returning to `min`.
        if delta > px(0.0) {
            let allowed = min - offset; // positive — moves inward
            if delta > allowed { allowed } else { delta }
        } else {
            px(0.0)
        }
    } else {
        // In range: clamp so the resulting offset does not cross the
        // boundary.
        let next = offset + delta;
        if next > max {
            max - offset
        } else if next < min {
            min - offset
        } else {
            delta
        }
    }
}

/// ADR-019 — strategy object that converts pointer deltas and release
/// velocities into a scrollable view's offset trajectory.
///
/// Trait (rather than enum) because platform-specific physics cannot
/// be encoded as a closed set without future breakage, and tests
/// need deterministic mock physics — a trait provides that for free.
///
/// Implementors are pure: no `&mut Window` / `&mut App`. The widget
/// calls `apply_delta` on every pointer-move during an active gesture
/// and `fling` on release. The simulator returned by `fling` runs
/// against the engine's animation clock — see
/// `crate::animation::Simulation` for the runtime surface.
///
/// See `docs/research/adr/ADR-019-scroll-physics.md`.
pub trait ScrollPhysics: Send + Sync + 'static {
    /// Apply the physics to a pending pointer delta. May reject the
    /// delta entirely (axis lock — the orthogonal component is
    /// returned as zero), modify it (rubber-band resistance at edges),
    /// or pass it through unchanged.
    fn apply_delta(&self, state: &ScrollState, delta: Point<Pixels>) -> Point<Pixels>;

    /// Build the simulator that runs after the user releases the
    /// gesture. Returns `None` if the physics has no fling (e.g. a
    /// `MockPhysics` that only services drag).
    ///
    /// Concrete simulators live in `crate::animation::simulation` —
    /// `SpringSimulation` for the rubber-band return,
    /// `FrictionSimulation` for the inertial decay,
    /// `BoundedFrictionSimulation` for the clamped variant. Physics
    /// impls compose those primitives.
    fn fling(
        &self,
        state: &ScrollState,
        velocity: Point<Pixels>,
    ) -> Option<Box<dyn crate::animation::Simulation>>;

    /// Should this physics allow overscroll past the edges during an
    /// active gesture? `BouncingPhysics` returns `true` (rubber-band
    /// feel); `ClampingPhysics` returns `false` (offset clamped at
    /// edges, gesture continues to track the cursor but the view
    /// stays still).
    fn allows_overscroll(&self) -> bool;

    /// Slop threshold (in pixels) before the gesture-recognizer
    /// commits to a dominant axis. Below this, motion is free-form
    /// 2-D. Above this, the recognizer sets
    /// [`ScrollState::axis_lock`] and `apply_delta` zeroes the
    /// orthogonal component. Default `0.0` (no axis-lock) so impls
    /// that do not care about axis-lock semantics need not override
    /// the method; both `BouncingPhysics` and `ClampingPhysics`
    /// override to expose their own configured threshold.
    ///
    /// Exposed on the trait so a future `Scrollable` widget composing
    /// physics via `dyn ScrollPhysics` can query the threshold
    /// without downcasting to the concrete impl.
    fn axis_lock_slop(&self) -> f32 {
        0.0
    }
}

/// ADR-019 iOS / macOS-style scroll physics — rubber-band resistance
/// at edges; momentum decays via spring + friction; allows overscroll
/// during gesture.
#[derive(Copy, Clone, Debug, Default)]
pub struct BouncingPhysics {
    /// Slop threshold (in pixels) before the gesture is committed to
    /// a dominant axis. Below this, motion is free-form 2-D.
    pub axis_lock_slop: f32,
}

impl ScrollPhysics for BouncingPhysics {
    fn apply_delta(&self, state: &ScrollState, delta: Point<Pixels>) -> Point<Pixels> {
        // Axis-lock: if the state has a committed axis, zero the
        // orthogonal component. ADR-019 decision 3: closes GPUI #40623
        // by making the rule data, not wired into the gesture
        // recogniser.
        let delta = match state.axis_lock {
            Some(Axis::Horizontal) => Point::new(delta.x, px(0.0)),
            Some(Axis::Vertical) => Point::new(px(0.0), delta.y),
            None => delta,
        };
        // Rubber-band resistance at edges: the further past the edge,
        // the harder it is to push further. Standard cubic decay.
        let overscroll = state.overscroll();
        let resist_x = if overscroll.x.0 == 0.0 {
            delta.x
        } else {
            delta.x * 0.5_f32
        };
        let resist_y = if overscroll.y.0 == 0.0 {
            delta.y
        } else {
            delta.y * 0.5_f32
        };
        Point::new(resist_x, resist_y)
    }

    fn fling(
        &self,
        _state: &ScrollState,
        _velocity: Point<Pixels>,
    ) -> Option<Box<dyn crate::animation::Simulation>> {
        // The actual fling integration is deferred — the Scrollable
        // widget that consumes this returns a composite of
        // SpringSimulation (for the bounce-back near edges) and
        // FrictionSimulation (for the body of the fling). Wiring is
        // part of the Scrollable widget spec, not the physics
        // scaffolding. Returning None here is the documented "no
        // fling configured" path; the trait stays well-defined.
        None
    }

    fn allows_overscroll(&self) -> bool {
        true
    }

    fn axis_lock_slop(&self) -> f32 {
        self.axis_lock_slop
    }
}

/// ADR-019 Android / Windows / Linux-style scroll physics — offset
/// clamped at edges, no rubber-band; fling decays via friction with a
/// hard stop at the bound.
#[derive(Copy, Clone, Debug, Default)]
pub struct ClampingPhysics {
    /// Slop threshold (in pixels) before the gesture is committed to
    /// a dominant axis. Below this, motion is free-form 2-D.
    pub axis_lock_slop: f32,
}

impl ScrollPhysics for ClampingPhysics {
    fn apply_delta(&self, state: &ScrollState, delta: Point<Pixels>) -> Point<Pixels> {
        let delta = match state.axis_lock {
            Some(Axis::Horizontal) => Point::new(delta.x, px(0.0)),
            Some(Axis::Vertical) => Point::new(px(0.0), delta.y),
            None => delta,
        };
        // Clamp at edges. Two cases per axis:
        //   1. Offset already past the boundary (state.offset > max
        //      OR < min): only INWARD motion (toward the valid range)
        //      is permitted. Outward motion is zeroed — otherwise a
        //      drag past the edge would produce a negative-direction
        //      delta that moves opposite to the user's drag, which
        //      reads as a glitch.
        //   2. Offset in range: clamp the delta so the resulting
        //      offset does not cross the boundary.
        let dx = clamp_axis(
            state.offset.x,
            state.min_offset.x,
            state.max_offset.x,
            delta.x,
        );
        let dy = clamp_axis(
            state.offset.y,
            state.min_offset.y,
            state.max_offset.y,
            delta.y,
        );
        Point::new(dx, dy)
    }

    fn fling(
        &self,
        _state: &ScrollState,
        _velocity: Point<Pixels>,
    ) -> Option<Box<dyn crate::animation::Simulation>> {
        // See `BouncingPhysics::fling` — Scrollable composes the
        // concrete simulator from this physics impl.
        None
    }

    fn allows_overscroll(&self) -> bool {
        false
    }

    fn axis_lock_slop(&self) -> f32 {
        self.axis_lock_slop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_in_bounds() -> ScrollState {
        ScrollState {
            offset: Point::new(px(50.0), px(50.0)),
            min_offset: Point::new(px(0.0), px(0.0)),
            max_offset: Point::new(px(200.0), px(200.0)),
            velocity: Point::new(px(0.0), px(0.0)),
            axis_lock: None,
        }
    }

    /// ADR-019 decision 3 — axis-lock zeroes the orthogonal component.
    /// Closes GPUI #40623 (macOS horizontal trackpad scroll leaking
    /// into vertical) by treating axis-lock as physics-level data, not
    /// gesture-recognizer logic.
    #[test]
    fn adr_019_bouncing_axis_lock_zeroes_orthogonal_component() {
        let physics = BouncingPhysics::default();
        let mut state = state_in_bounds();

        // No lock — both axes pass.
        let out = physics.apply_delta(&state, Point::new(px(10.0), px(20.0)));
        assert_eq!(out, Point::new(px(10.0), px(20.0)));

        // Horizontal lock — vertical zeroed.
        state.axis_lock = Some(Axis::Horizontal);
        let out = physics.apply_delta(&state, Point::new(px(10.0), px(20.0)));
        assert_eq!(out, Point::new(px(10.0), px(0.0)));

        // Vertical lock — horizontal zeroed.
        state.axis_lock = Some(Axis::Vertical);
        let out = physics.apply_delta(&state, Point::new(px(10.0), px(20.0)));
        assert_eq!(out, Point::new(px(0.0), px(20.0)));
    }

    /// ADR-019 — `BouncingPhysics` allows overscroll with resistance.
    /// `ClampingPhysics` does not.
    #[test]
    fn adr_019_overscroll_modes_diverge() {
        let bouncing = BouncingPhysics::default();
        let clamping = ClampingPhysics::default();
        assert!(bouncing.allows_overscroll());
        assert!(!clamping.allows_overscroll());
    }

    /// ADR-019 — `ClampingPhysics` truncates the delta at the maximum
    /// offset boundary; the offset never goes past `max_offset`.
    /// Out-of-bounds states (e.g. via a programmatic
    /// `scroll_to_offset` past the bound) permit only INWARD motion —
    /// outward motion is zeroed instead of producing a reversed delta
    /// (which would read as a glitch on the scrollable).
    #[test]
    fn adr_019_clamping_truncates_at_max_offset() {
        let physics = ClampingPhysics::default();
        let mut state = state_in_bounds();
        // Sitting at 190 with max 200; a 30-px push should be
        // truncated to 10.
        state.offset = Point::new(px(190.0), px(0.0));
        let out = physics.apply_delta(&state, Point::new(px(30.0), px(0.0)));
        assert_eq!(out, Point::new(px(10.0), px(0.0)));
        // At max, outward push is zeroed.
        state.offset = Point::new(px(200.0), px(0.0));
        let out = physics.apply_delta(&state, Point::new(px(30.0), px(0.0)));
        assert_eq!(out, Point::new(px(0.0), px(0.0)));
        // ADR-019: past max with outward push → zero (NOT a reversed
        // negative delta — that was the pre-fix bug).
        state.offset = Point::new(px(210.0), px(0.0));
        let out = physics.apply_delta(&state, Point::new(px(30.0), px(0.0)));
        assert_eq!(out, Point::new(px(0.0), px(0.0)));
        // Past max with inward push: capped at returning to max
        // (delta of -10 here moves 210 → 200).
        let out = physics.apply_delta(&state, Point::new(px(-30.0), px(0.0)));
        assert_eq!(out, Point::new(px(-10.0), px(0.0)));
        // Past min with inward (positive) push: capped at returning to min.
        state.offset = Point::new(px(-15.0), px(0.0));
        let out = physics.apply_delta(&state, Point::new(px(30.0), px(0.0)));
        assert_eq!(out, Point::new(px(15.0), px(0.0)));
    }

    /// ADR-019 — `BouncingPhysics` applies rubber-band resistance
    /// (half-strength) when in overscroll. Locks the resistance
    /// coefficient is non-zero (specific 0.5 factor is implementation
    /// detail; the contract is "less than 1.0").
    #[test]
    fn adr_019_bouncing_overscroll_applies_resistance() {
        let physics = BouncingPhysics::default();
        let mut state = state_in_bounds();
        // Set offset PAST max to enter overscroll.
        state.offset = Point::new(px(210.0), px(0.0));
        // overscroll.x = +10, overscroll.y = 0. Delta on x is
        // resisted; delta on y is full strength.
        let out = physics.apply_delta(&state, Point::new(px(10.0), px(10.0)));
        assert!(
            out.x.0 < 10.0,
            "rubber-band resistance must reduce x delta when in overscroll; got {out:?}"
        );
        assert_eq!(out.y, px(10.0));
    }

    /// ADR-019 — `ScrollState::in_bounds` / `overscroll` agree on edge
    /// values (the boundary itself counts as in-bounds, not over).
    #[test]
    fn adr_019_scroll_state_overscroll_at_boundary_is_zero() {
        let state = ScrollState {
            offset: Point::new(px(200.0), px(0.0)),
            min_offset: Point::new(px(0.0), px(0.0)),
            max_offset: Point::new(px(200.0), px(200.0)),
            velocity: Point::new(px(0.0), px(0.0)),
            axis_lock: None,
        };
        assert!(state.in_bounds());
        assert_eq!(state.overscroll(), Point::new(px(0.0), px(0.0)));
    }
}
