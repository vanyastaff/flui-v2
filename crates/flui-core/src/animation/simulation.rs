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
pub trait Simulation: Send + Sync {
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
    pub fn new(
        spring: SpringDescription,
        start: f32,
        end: f32,
        velocity: f32,
    ) -> Self {
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
            decay * ((-self.zeta * self.omega) * (a * cos_part + b * sin_part)
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

    pub fn with_tolerance(
        drag: f32,
        position: f32,
        velocity: f32,
        tolerance: Tolerance,
    ) -> Self {
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
    pub fn new(
        acceleration: f32,
        position: f32,
        velocity: f32,
        end: f32,
    ) -> Self {
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
    fn test_gravity_parabolic() {
        let sim = GravitySimulation::new(9.8, 0.0, 0.0, 100.0);
        assert!((sim.x(0.0) - 0.0).abs() < 0.01);
        // x(1) = 0.5 * 9.8 * 1 = 4.9
        assert!((sim.x(1.0) - 4.9).abs() < 0.01);
    }
}
