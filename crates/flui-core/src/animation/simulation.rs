// crates/flui-core/src/animation/simulation.rs

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked separately

/// Threshold for determining when a simulation is "close enough" to its target.
#[derive(Clone, Debug)]
pub struct Tolerance {
    pub distance: f32,
    pub velocity: f32,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            distance: 0.001,
            velocity: 0.001,
        }
    }
}

/// A physics simulation that produces position and velocity over time.
///
/// `'static` supertrait added in S21 review-fix Tier 3 — `AnimationController`
/// stores `Option<Box<dyn Simulation>>` (defaulting to
/// `Box<dyn Simulation + 'static>` per Rust's lifetime-elision rule), and
/// `animate_with` accepts `impl Simulation + 'static`. Making the supertrait
/// explicit prevents external impls from accidentally introducing borrowed
/// lifetimes that the controller cannot store.
pub trait Simulation: Send + Sync + 'static {
    /// Position at time `t` (seconds).
    fn x(&self, t: f32) -> f32;
    /// Velocity at time `t` (seconds).
    fn dx(&self, t: f32) -> f32;
    /// Whether the simulation is effectively complete at time `t`.
    fn is_done(&self, t: f32) -> bool;
}

// ============================================================================
// SpringDescription
// ============================================================================

/// Parameters for a damped spring.
#[derive(Clone, Debug)]
pub struct SpringDescription {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDescription {
    /// Create a spring from a damping ratio.
    /// - ratio < 1.0: underdamped (oscillates)
    /// - ratio = 1.0: critically damped (fastest without oscillation)
    /// - ratio > 1.0: overdamped (slow return)
    pub fn with_damping_ratio(mass: f32, stiffness: f32, ratio: f32) -> Self {
        Self {
            mass,
            stiffness,
            damping: ratio * 2.0 * (mass * stiffness).sqrt(),
        }
    }
}

// ============================================================================
// SpringSimulation
// ============================================================================

/// Damped harmonic oscillator. Used for snap-back, bounce, overscroll.
///
/// Solves: m*x'' + c*x' + k*(x - end) = 0
pub struct SpringSimulation {
    end: f32,
    /// Initial offset from end
    offset: f32,
    /// Initial velocity
    velocity: f32,
    /// Angular frequency
    omega: f32,
    /// Damping ratio
    zeta: f32,
    tolerance: Tolerance,
}

impl SpringSimulation {
    pub fn new(spring: SpringDescription, start: f32, end: f32, velocity: f32) -> Self {
        Self::with_tolerance(spring, start, end, velocity, Tolerance::default())
    }

    pub fn with_tolerance(
        spring: SpringDescription,
        start: f32,
        end: f32,
        velocity: f32,
        tolerance: Tolerance,
    ) -> Self {
        let omega = (spring.stiffness / spring.mass).sqrt();
        let zeta = spring.damping / (2.0 * spring.mass * omega);
        Self {
            end,
            offset: start - end,
            velocity,
            omega,
            zeta,
            tolerance,
        }
    }
}

impl Simulation for SpringSimulation {
    fn x(&self, t: f32) -> f32 {
        if self.zeta < 1.0 {
            // Underdamped
            let wd = self.omega * (1.0 - self.zeta * self.zeta).sqrt();
            let decay = (-self.zeta * self.omega * t).exp();
            let a = self.offset;
            let b = (self.velocity + self.zeta * self.omega * self.offset) / wd;
            self.end + decay * (a * (wd * t).cos() + b * (wd * t).sin())
        } else {
            // Critically damped or overdamped
            let decay = (-self.omega * t).exp();
            let a = self.offset;
            let b = self.velocity + self.omega * self.offset;
            self.end + decay * (a + b * t)
        }
    }

    fn dx(&self, t: f32) -> f32 {
        if self.zeta < 1.0 {
            let wd = self.omega * (1.0 - self.zeta * self.zeta).sqrt();
            let decay = (-self.zeta * self.omega * t).exp();
            let a = self.offset;
            let b = (self.velocity + self.zeta * self.omega * self.offset) / wd;
            let cos_part = (wd * t).cos();
            let sin_part = (wd * t).sin();
            decay
                * ((-self.zeta * self.omega) * (a * cos_part + b * sin_part)
                    + wd * (-a * sin_part + b * cos_part))
        } else {
            let decay = (-self.omega * t).exp();
            let b = self.velocity + self.omega * self.offset;
            decay * (b - self.omega * (self.offset + b * t))
        }
    }

    fn is_done(&self, t: f32) -> bool {
        (self.x(t) - self.end).abs() < self.tolerance.distance
            && self.dx(t).abs() < self.tolerance.velocity
    }
}

// ============================================================================
// FrictionSimulation
// ============================================================================

/// Exponential deceleration. Used for fling momentum.
///
/// Position decays exponentially: x(t) = x0 + v0 * (1 - e^(-drag*t)) / drag
pub struct FrictionSimulation {
    position: f32,
    velocity: f32,
    drag: f32,
    tolerance: Tolerance,
}

impl FrictionSimulation {
    pub fn new(drag: f32, position: f32, velocity: f32) -> Self {
        Self::with_tolerance(drag, position, velocity, Tolerance::default())
    }

    pub fn with_tolerance(drag: f32, position: f32, velocity: f32, tolerance: Tolerance) -> Self {
        Self {
            position,
            velocity,
            drag: drag.max(0.001),
            tolerance,
        }
    }

    /// Final resting position.
    pub fn final_x(&self) -> f32 {
        self.position + self.velocity / self.drag
    }
}

impl Simulation for FrictionSimulation {
    fn x(&self, t: f32) -> f32 {
        self.position + self.velocity * (1.0 - (-self.drag * t).exp()) / self.drag
    }

    fn dx(&self, t: f32) -> f32 {
        self.velocity * (-self.drag * t).exp()
    }

    fn is_done(&self, t: f32) -> bool {
        self.dx(t).abs() < self.tolerance.velocity
    }
}

// ============================================================================
// BoundedFrictionSimulation
// ============================================================================

/// `FrictionSimulation` clamped to a `[min, max]` range. Once the inner
/// friction simulation would carry the position past either bound, this
/// wrapper pins to the bound and reports `is_done`.
///
/// **Flutter parity:** corresponds to
/// `BoundedFrictionSimulation` from `package:flutter/physics`.
pub struct BoundedFrictionSimulation {
    inner: FrictionSimulation,
    min: f32,
    max: f32,
}

impl BoundedFrictionSimulation {
    /// Construct a friction simulation clamped to `[min, max]`.
    ///
    /// Panics if `min > max` — this is a programmer-error invariant, not a
    /// runtime condition, and silently swapping bounds would mask bugs at the
    /// caller. Asserted unconditionally (not `debug_assert!`) so release
    /// builds also enforce the contract.
    pub fn new(drag: f32, position: f32, velocity: f32, min: f32, max: f32) -> Self {
        assert!(
            min <= max,
            "BoundedFrictionSimulation: min ({min}) must be <= max ({max})"
        );
        Self {
            inner: FrictionSimulation::new(drag, position, velocity),
            min,
            max,
        }
    }
}

impl Simulation for BoundedFrictionSimulation {
    fn x(&self, t: f32) -> f32 {
        self.inner.x(t).clamp(self.min, self.max)
    }

    fn dx(&self, t: f32) -> f32 {
        // Use inclusive comparisons to keep `dx` and `is_done` consistent:
        // when `is_done(t) == true` (the position has reached a bound), the
        // reported velocity is zero. Without the inclusive bound, a particle
        // starting at exactly `min` or `max` would report non-zero velocity
        // even though `is_done` already says we've pinned.
        let raw = self.inner.x(t);
        if raw <= self.min || raw >= self.max {
            // We've hit a bound — velocity is effectively zero (clamped).
            0.0
        } else {
            self.inner.dx(t)
        }
    }

    fn is_done(&self, t: f32) -> bool {
        let raw = self.inner.x(t);
        // Done when either the inner friction has settled or we've pinned at
        // a bound.
        self.inner.is_done(t) || raw <= self.min || raw >= self.max
    }
}

// ============================================================================
// GravitySimulation
// ============================================================================

/// Constant acceleration. Used for throwing/falling.
///
/// x(t) = x0 + v0*t + 0.5*a*t^2
pub struct GravitySimulation {
    acceleration: f32,
    position: f32,
    velocity: f32,
    end: f32,
    tolerance: Tolerance,
}

impl GravitySimulation {
    pub fn new(acceleration: f32, position: f32, velocity: f32, end: f32) -> Self {
        Self {
            acceleration,
            position,
            velocity,
            end,
            tolerance: Tolerance::default(),
        }
    }
}

impl Simulation for GravitySimulation {
    fn x(&self, t: f32) -> f32 {
        self.position + self.velocity * t + 0.5 * self.acceleration * t * t
    }

    fn dx(&self, t: f32) -> f32 {
        self.velocity + self.acceleration * t
    }

    fn is_done(&self, t: f32) -> bool {
        let past_end = if self.acceleration > 0.0 {
            self.x(t) >= self.end
        } else {
            self.x(t) <= self.end
        };
        past_end || (self.x(t) - self.end).abs() < self.tolerance.distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_converges() {
        let spring = SpringDescription::with_damping_ratio(1.0, 100.0, 1.0);
        let sim = SpringSimulation::new(spring, 0.0, 1.0, 0.0);
        // At t=0, should be at start
        assert!((sim.x(0.0) - 0.0).abs() < 0.01);
        // After enough time, should converge to end
        assert!((sim.x(2.0) - 1.0).abs() < 0.01);
        assert!(sim.is_done(2.0));
    }

    #[test]
    fn test_spring_underdamped_oscillates() {
        let spring = SpringDescription::with_damping_ratio(1.0, 100.0, 0.3);
        let sim = SpringSimulation::new(spring, 0.0, 1.0, 0.0);
        // Underdamped should overshoot
        let mut overshot = false;
        for i in 0..100 {
            let t = i as f32 * 0.05;
            if sim.x(t) > 1.01 {
                overshot = true;
                break;
            }
        }
        assert!(overshot, "Underdamped spring should overshoot target");
    }

    #[test]
    fn test_friction_decelerates() {
        let sim = FrictionSimulation::new(2.0, 0.0, 100.0);
        assert!((sim.x(0.0) - 0.0).abs() < 0.01);
        assert!(sim.dx(0.0) > sim.dx(1.0)); // velocity decreases
        assert!(sim.is_done(10.0)); // eventually stops
    }

    #[test]
    fn test_friction_final_position() {
        let sim = FrictionSimulation::new(2.0, 0.0, 100.0);
        // final_x should match x at large t
        assert!((sim.final_x() - sim.x(100.0)).abs() < 0.1);
    }

    #[test]
    fn test_bounded_friction_clamps_at_max() {
        // Drag is small, velocity is large → friction would carry past max.
        // The bound clamps the position.
        let sim = BoundedFrictionSimulation::new(0.5, 0.0, 100.0, 0.0, 10.0);
        // At t = 5, raw friction position is well past 10 — clamp to 10.
        assert!((sim.x(5.0) - 10.0).abs() < 1e-3);
        assert!(sim.is_done(5.0), "bounded sim done at the upper bound");
        // dx at the bound is reported as zero (clamped).
        assert_eq!(sim.dx(5.0), 0.0);
    }

    #[test]
    fn test_bounded_friction_within_bounds() {
        // Velocity small enough that friction settles within bounds.
        let sim = BoundedFrictionSimulation::new(5.0, 0.0, 10.0, 0.0, 100.0);
        // Mid-flight — within bounds.
        assert!(sim.x(0.5) > 0.0);
        assert!(sim.x(0.5) < 100.0);
        // Velocity is the inner friction's velocity (not clamped).
        assert!(sim.dx(0.5) > 0.0);
    }

    #[test]
    fn test_gravity_parabolic() {
        let sim = GravitySimulation::new(9.8, 0.0, 0.0, 100.0);
        assert!((sim.x(0.0) - 0.0).abs() < 0.01);
        // x(1) = 0.5 * 9.8 * 1 = 4.9
        assert!((sim.x(1.0) - 4.9).abs() < 0.01);
    }
}
