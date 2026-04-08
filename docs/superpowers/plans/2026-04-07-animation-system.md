# Animation System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a two-level animation system to flui-core: Curve enum, Lerp/Tween for interpolation, physics simulations (Spring/Friction/Gravity), AnimationController entity, and animated() convenience wrapper.

**Architecture:** New `src/animation/` module in flui-core with 7 files. Extends existing `AnimationExt` with Curve support. AnimationController is a pure state container — parent view drives rendering via `animated()` which auto-schedules frames.

**Tech Stack:** Rust, flui-core (elements/animation.rs existing system, Entity/Context for controller)

---

### Task 1: Curve enum

**Files:**
- Create: `crates/flui-core/src/animation/curve.rs`
- Create: `crates/flui-core/src/animation/mod.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `animation/curve.rs` with all variants and transform()**

```rust
// crates/flui-core/src/animation/curve.rs

use std::f32::consts::PI;
use std::sync::Arc;

/// Easing curve for animations.
///
/// Standard variants are zero-allocation. `Custom` uses `Arc` for shared ownership.
#[derive(Clone)]
pub enum Curve {
    // Standard
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,

    // Dramatic
    Bounce,
    Elastic,

    // Parametric
    Spring { damping: f32, stiffness: f32 },
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },

    // Composition
    /// Maps a sub-range of the timeline to 0.0..1.0.
    /// Used for staggered animations: `Interval { begin: 0.0, end: 0.33, .. }`.
    Interval { begin: f32, end: f32, curve: Box<Curve> },
    Reversed(Box<Curve>),

    // Custom
    Custom(Arc<dyn Fn(f32) -> f32 + Send + Sync>),
}

impl Curve {
    /// Transform `t` (0.0..=1.0) through this curve, returning a new value in 0.0..=1.0.
    pub fn transform(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::EaseIn => t * t,
            Curve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Curve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    let x = -2.0 * t + 2.0;
                    1.0 - x * x / 2.0
                }
            }
            Curve::EaseInCubic => t * t * t,
            Curve::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Curve::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Curve::Bounce => {
                let t = 1.0 - t;
                let n1 = 7.5625;
                let d1 = 2.75;
                let out = if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                };
                1.0 - out
            }
            Curve::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let p = 0.3;
                    -(2.0_f32.powf(10.0 * (t - 1.0))
                        * ((t - 1.0 - p / 4.0) * 2.0 * PI / p).sin())
                }
            }
            Curve::Spring { damping, stiffness } => {
                // Simplified spring: critically-damped approximation
                let omega = stiffness.sqrt();
                let zeta = damping / (2.0 * omega);
                if zeta < 1.0 {
                    // Underdamped
                    let wd = omega * (1.0 - zeta * zeta).sqrt();
                    1.0 - (-zeta * omega * t).exp()
                        * ((zeta * omega * t / wd).sin() + (wd * t).cos())
                } else {
                    // Critically/overdamped
                    1.0 - (1.0 + omega * t) * (-omega * t).exp()
                }
            }
            Curve::CubicBezier { x1, y1, x2, y2 } => {
                cubic_bezier_transform(t, *x1, *y1, *x2, *y2)
            }
            Curve::Interval { begin, end, curve } => {
                if t <= *begin {
                    0.0
                } else if t >= *end {
                    1.0
                } else {
                    let local_t = (t - begin) / (end - begin);
                    curve.transform(local_t)
                }
            }
            Curve::Reversed(inner) => inner.transform(1.0 - t),
            Curve::Custom(f) => f(t),
        }
    }
}

/// Solve cubic bezier curve using Newton's method.
fn cubic_bezier_transform(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Find parameter for x coordinate, then evaluate y
    let mut guess = t;
    for _ in 0..8 {
        let x = cubic_bezier_sample(guess, x1, x2) - t;
        if x.abs() < 1e-6 {
            break;
        }
        let dx = cubic_bezier_derivative(guess, x1, x2);
        if dx.abs() < 1e-6 {
            break;
        }
        guess -= x / dx;
    }
    cubic_bezier_sample(guess.clamp(0.0, 1.0), y1, y2)
}

fn cubic_bezier_sample(t: f32, a: f32, b: f32) -> f32 {
    // B(t) = 3(1-t)^2*t*a + 3(1-t)*t^2*b + t^3
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * a + 3.0 * mt * t2 * b + t3
}

fn cubic_bezier_derivative(t: f32, a: f32, b: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * a + 6.0 * mt * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert_eq!(Curve::Linear.transform(0.0), 0.0);
        assert_eq!(Curve::Linear.transform(0.5), 0.5);
        assert_eq!(Curve::Linear.transform(1.0), 1.0);
    }

    #[test]
    fn test_ease_in_boundaries() {
        assert_eq!(Curve::EaseIn.transform(0.0), 0.0);
        assert_eq!(Curve::EaseIn.transform(1.0), 1.0);
        assert!(Curve::EaseIn.transform(0.5) < 0.5); // ease-in is slow at start
    }

    #[test]
    fn test_ease_out_boundaries() {
        assert_eq!(Curve::EaseOut.transform(0.0), 0.0);
        assert_eq!(Curve::EaseOut.transform(1.0), 1.0);
        assert!(Curve::EaseOut.transform(0.5) > 0.5); // ease-out is fast at start
    }

    #[test]
    fn test_ease_in_out_boundaries() {
        assert_eq!(Curve::EaseInOut.transform(0.0), 0.0);
        assert_eq!(Curve::EaseInOut.transform(1.0), 1.0);
        assert!((Curve::EaseInOut.transform(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bounce_boundaries() {
        assert!((Curve::Bounce.transform(0.0)).abs() < 0.01);
        assert!((Curve::Bounce.transform(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_interval_before_begin() {
        let curve = Curve::Interval {
            begin: 0.3,
            end: 0.7,
            curve: Box::new(Curve::Linear),
        };
        assert_eq!(curve.transform(0.0), 0.0);
        assert_eq!(curve.transform(0.2), 0.0);
    }

    #[test]
    fn test_interval_after_end() {
        let curve = Curve::Interval {
            begin: 0.3,
            end: 0.7,
            curve: Box::new(Curve::Linear),
        };
        assert_eq!(curve.transform(0.8), 1.0);
        assert_eq!(curve.transform(1.0), 1.0);
    }

    #[test]
    fn test_interval_midpoint() {
        let curve = Curve::Interval {
            begin: 0.0,
            end: 0.5,
            curve: Box::new(Curve::Linear),
        };
        assert!((curve.transform(0.25) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_reversed() {
        let curve = Curve::Reversed(Box::new(Curve::EaseIn));
        // Reversed EaseIn = EaseOut behavior
        assert_eq!(curve.transform(0.0), 1.0);
        assert_eq!(curve.transform(1.0), 0.0);
    }

    #[test]
    fn test_custom() {
        let curve = Curve::Custom(Arc::new(|t| t * t * t));
        assert_eq!(curve.transform(0.0), 0.0);
        assert!((curve.transform(0.5) - 0.125).abs() < 0.01);
        assert_eq!(curve.transform(1.0), 1.0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(Curve::Linear.transform(-0.5), 0.0);
        assert_eq!(Curve::Linear.transform(1.5), 1.0);
    }
}
```

- [ ] **Step 2: Create `animation/mod.rs`**

```rust
// crates/flui-core/src/animation/mod.rs

mod curve;

pub use curve::Curve;
```

- [ ] **Step 3: Register in `lib.rs`**

Add `pub mod animation;` after the existing module declarations in `crates/flui-core/src/lib.rs`. Add `pub use animation::*;` in the re-exports section.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p flui-core -- animation::curve::tests 2>&1 | tail -15`
Expected: all tests pass

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

```bash
git add crates/flui-core/src/animation/
git add crates/flui-core/src/lib.rs
git commit -m "feat(animation): Curve enum with 15 variants + Interval for stagger"
```

---

### Task 2: Lerp trait + Tween

**Files:**
- Create: `crates/flui-core/src/animation/lerp.rs`
- Create: `crates/flui-core/src/animation/tween.rs`
- Modify: `crates/flui-core/src/animation/mod.rs`

- [ ] **Step 1: Create `lerp.rs`**

```rust
// crates/flui-core/src/animation/lerp.rs

use crate::{Hsla, Pixels, Point, Size};

/// Trait for types that can be linearly interpolated.
pub trait Lerp: Clone {
    /// Interpolate between `self` and `other` by factor `t` (0.0..=1.0).
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for f64 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t as f64
    }
}

impl Lerp for Pixels {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Pixels(self.0.lerp(&other.0, t))
    }
}

impl Lerp for Hsla {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Hsla {
            h: self.h + (other.h - self.h) * t,
            s: self.s + (other.s - self.s) * t,
            l: self.l + (other.l - self.l) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl Lerp for Point<Pixels> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Point {
            x: self.x.lerp(&other.x, t),
            y: self.y.lerp(&other.y, t),
        }
    }
}

impl Lerp for Size<Pixels> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Size {
            width: self.width.lerp(&other.width, t),
            height: self.height.lerp(&other.height, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn test_f32_lerp() {
        assert_eq!(0.0f32.lerp(&10.0, 0.0), 0.0);
        assert_eq!(0.0f32.lerp(&10.0, 0.5), 5.0);
        assert_eq!(0.0f32.lerp(&10.0, 1.0), 10.0);
    }

    #[test]
    fn test_pixels_lerp() {
        let a = px(0.0);
        let b = px(100.0);
        let mid = a.lerp(&b, 0.5);
        assert!((mid.0 - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_hsla_lerp() {
        let a = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 1.0 };
        let b = Hsla { h: 1.0, s: 1.0, l: 1.0, a: 1.0 };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.h - 0.5).abs() < 0.01);
        assert!((mid.s - 0.5).abs() < 0.01);
        assert!((mid.l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_point_lerp() {
        let a = Point { x: px(0.0), y: px(0.0) };
        let b = Point { x: px(100.0), y: px(200.0) };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.x.0 - 50.0).abs() < 0.01);
        assert!((mid.y.0 - 100.0).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Create `tween.rs`**

```rust
// crates/flui-core/src/animation/tween.rs

use super::lerp::Lerp;

/// Interpolates between two values of the same type.
///
/// # Example
/// ```ignore
/// let tween = Tween::new(0.0f32, 100.0);
/// assert_eq!(tween.transform(0.5), 50.0);
/// ```
#[derive(Clone, Debug)]
pub struct Tween<T: Lerp> {
    pub begin: T,
    pub end: T,
}

impl<T: Lerp> Tween<T> {
    /// Create a new tween from `begin` to `end`.
    pub fn new(begin: T, end: T) -> Self {
        Self { begin, end }
    }

    /// Get the interpolated value at `t` (0.0..=1.0).
    pub fn transform(&self, t: f32) -> T {
        self.begin.lerp(&self.end, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_tween() {
        let tween = Tween::new(0.0f32, 100.0);
        assert_eq!(tween.transform(0.0), 0.0);
        assert_eq!(tween.transform(0.5), 50.0);
        assert_eq!(tween.transform(1.0), 100.0);
    }

    #[test]
    fn test_reverse_tween() {
        let tween = Tween::new(100.0f32, 0.0);
        assert_eq!(tween.transform(0.0), 100.0);
        assert_eq!(tween.transform(1.0), 0.0);
    }
}
```

- [ ] **Step 3: Update `animation/mod.rs`**

```rust
mod curve;
mod lerp;
mod tween;

pub use curve::Curve;
pub use lerp::Lerp;
pub use tween::Tween;
```

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p flui-core -- animation::lerp::tests animation::tween::tests 2>&1 | tail -15`
Expected: all tests pass

```bash
git add crates/flui-core/src/animation/
git commit -m "feat(animation): Lerp trait + Tween<T> for typed interpolation"
```

---

### Task 3: Physics Simulations

**Files:**
- Create: `crates/flui-core/src/animation/simulation.rs`
- Modify: `crates/flui-core/src/animation/mod.rs`

- [ ] **Step 1: Create `simulation.rs`**

```rust
// crates/flui-core/src/animation/simulation.rs

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
```

- [ ] **Step 2: Update `animation/mod.rs`**

```rust
mod curve;
mod lerp;
mod simulation;
mod tween;

pub use curve::Curve;
pub use lerp::Lerp;
pub use simulation::{
    FrictionSimulation, GravitySimulation, Simulation, SpringDescription, SpringSimulation,
    Tolerance,
};
pub use tween::Tween;
```

- [ ] **Step 3: Verify and commit**

Run: `cargo test -p flui-core -- animation::simulation::tests 2>&1 | tail -15`
Expected: all tests pass

```bash
git add crates/flui-core/src/animation/
git commit -m "feat(animation): physics simulations — Spring, Friction, Gravity"
```

---

### Task 4: AnimationController + attach()

**Files:**
- Create: `crates/flui-core/src/animation/controller.rs`
- Modify: `crates/flui-core/src/animation/mod.rs`

- [ ] **Step 1: Create `controller.rs`**

```rust
// crates/flui-core/src/animation/controller.rs

use crate::animation::{Curve, Simulation};
use crate::scheduler::Instant;
use crate::{App, AppContext, Context, Entity};
use std::time::Duration;

/// Status of an animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationStatus {
    /// At lower bound, idle.
    #[default]
    Dismissed,
    /// Animating toward upper bound.
    Forward,
    /// Animating toward lower bound.
    Reverse,
    /// At upper bound, idle.
    Completed,
}

impl AnimationStatus {
    /// Whether the animation is currently running.
    pub fn is_animating(self) -> bool {
        matches!(self, Self::Forward | Self::Reverse)
    }
}

/// A persistent animation state container.
///
/// Does NOT tick itself — parent view drives rendering via [`animated()`](super::animated).
/// Create with [`AnimationController::new()`] and attach to a view with [`.attach(cx)`].
///
/// # Example
/// ```ignore
/// struct MyView {
///     fade: Entity<AnimationController>,
/// }
///
/// impl MyView {
///     fn new(cx: &mut Context<Self>) -> Self {
///         Self {
///             fade: AnimationController::new(Duration::from_millis(300))
///                 .curve(Curve::EaseInOut)
///                 .attach(cx),
///         }
///     }
/// }
/// ```
pub struct AnimationController {
    value: f32,
    status: AnimationStatus,
    duration: Duration,
    reverse_duration: Option<Duration>,
    lower_bound: f32,
    upper_bound: f32,
    curve: Curve,
    start_time: Option<Instant>,
    start_value: f32,
    repeating: bool,
    simulation: Option<Box<dyn Simulation>>,
    sim_start_time: Option<Instant>,
}

impl AnimationController {
    /// Create a new controller with the given duration.
    pub fn new(duration: Duration) -> Self {
        Self {
            value: 0.0,
            status: AnimationStatus::Dismissed,
            duration,
            reverse_duration: None,
            lower_bound: 0.0,
            upper_bound: 1.0,
            curve: Curve::Linear,
            start_time: None,
            start_value: 0.0,
            repeating: false,
            simulation: None,
            sim_start_time: None,
        }
    }

    /// Set the easing curve.
    pub fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// Set the lower bound (default: 0.0).
    pub fn lower_bound(mut self, v: f32) -> Self {
        self.lower_bound = v;
        self.value = v;
        self
    }

    /// Set the upper bound (default: 1.0).
    pub fn upper_bound(mut self, v: f32) -> Self {
        self.upper_bound = v;
        self
    }

    /// Set a different duration for reverse animation.
    pub fn reverse_duration(mut self, d: Duration) -> Self {
        self.reverse_duration = Some(d);
        self
    }

    /// Create an Entity and auto-observe the parent for re-render.
    ///
    /// This is the recommended way to create an AnimationController.
    /// Eliminates the `cx.new()` + `cx.observe()` + `.detach()` boilerplate.
    pub fn attach<V: 'static>(self, cx: &mut Context<V>) -> Entity<Self> {
        let entity = cx.new(|_| self);
        cx.observe(&entity, |_, _, cx| cx.notify()).detach();
        entity
    }

    // ========================================================================
    // State reading
    // ========================================================================

    /// Current animated value. Recalculates from elapsed time on each call.
    // TODO: consider per-frame caching if animated() is called multiple times
    pub fn value(&self) -> f32 {
        if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            let elapsed = start.elapsed().as_secs_f32();
            return sim.x(elapsed).clamp(self.lower_bound, self.upper_bound);
        }

        if let Some(start) = self.start_time {
            let duration = match self.status {
                AnimationStatus::Reverse => {
                    self.reverse_duration.unwrap_or(self.duration)
                }
                _ => self.duration,
            };

            if duration.is_zero() {
                return match self.status {
                    AnimationStatus::Forward | AnimationStatus::Completed => self.upper_bound,
                    _ => self.lower_bound,
                };
            }

            let elapsed = start.elapsed().as_secs_f32();
            let raw_t = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
            let curved_t = self.curve.transform(raw_t);

            let range = self.upper_bound - self.lower_bound;
            match self.status {
                AnimationStatus::Forward => self.start_value + curved_t * (self.upper_bound - self.start_value),
                AnimationStatus::Reverse => self.start_value - curved_t * (self.start_value - self.lower_bound),
                _ => self.value,
            }
        } else {
            self.value
        }
    }

    /// Whether the animation is currently running.
    pub fn is_animating(&self) -> bool {
        if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            return !sim.is_done(start.elapsed().as_secs_f32());
        }

        if let Some(start) = self.start_time {
            let duration = match self.status {
                AnimationStatus::Reverse => {
                    self.reverse_duration.unwrap_or(self.duration)
                }
                _ => self.duration,
            };
            let elapsed = start.elapsed();
            if elapsed >= duration {
                return self.repeating;
            }
            self.status.is_animating()
        } else {
            false
        }
    }

    /// Current animation status.
    pub fn status(&self) -> AnimationStatus {
        self.status
    }

    // ========================================================================
    // Control methods (each calls cx.notify())
    // ========================================================================

    /// Animate toward upper bound.
    pub fn forward(&mut self, cx: &mut Context<Self>) {
        self.simulation = None;
        self.sim_start_time = None;
        self.start_value = self.value();
        self.start_time = Some(Instant::now());
        self.status = AnimationStatus::Forward;
        self.repeating = false;
        cx.notify();
    }

    /// Animate toward lower bound.
    pub fn reverse(&mut self, cx: &mut Context<Self>) {
        self.simulation = None;
        self.sim_start_time = None;
        self.start_value = self.value();
        self.start_time = Some(Instant::now());
        self.status = AnimationStatus::Reverse;
        self.repeating = false;
        cx.notify();
    }

    /// Toggle direction: if forward/completed → reverse, otherwise → forward.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        match self.status {
            AnimationStatus::Forward | AnimationStatus::Completed => self.reverse(cx),
            _ => self.forward(cx),
        }
    }

    /// Animate forward and repeat indefinitely.
    pub fn repeat(&mut self, cx: &mut Context<Self>) {
        self.simulation = None;
        self.sim_start_time = None;
        self.start_value = self.lower_bound;
        self.start_time = Some(Instant::now());
        self.status = AnimationStatus::Forward;
        self.repeating = true;
        cx.notify();
    }

    /// Stop at current value.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.value = self.value();
        self.start_time = None;
        self.simulation = None;
        self.sim_start_time = None;
        self.repeating = false;
        self.status = if (self.value - self.upper_bound).abs() < 0.001 {
            AnimationStatus::Completed
        } else if (self.value - self.lower_bound).abs() < 0.001 {
            AnimationStatus::Dismissed
        } else {
            self.status
        };
        cx.notify();
    }

    /// Reset to lower bound.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.value = self.lower_bound;
        self.start_time = None;
        self.simulation = None;
        self.sim_start_time = None;
        self.repeating = false;
        self.status = AnimationStatus::Dismissed;
        cx.notify();
    }

    /// Drive animation with a physics simulation (spring, friction, gravity).
    pub fn animate_with(
        &mut self,
        simulation: impl Simulation + 'static,
        cx: &mut Context<Self>,
    ) {
        self.start_time = None;
        self.sim_start_time = Some(Instant::now());
        self.simulation = Some(Box::new(simulation));
        self.status = AnimationStatus::Forward;
        cx.notify();
    }
}
```

- [ ] **Step 2: Update `animation/mod.rs`**

```rust
mod controller;
mod curve;
mod lerp;
mod simulation;
mod tween;

pub use controller::{AnimationController, AnimationStatus};
pub use curve::Curve;
pub use lerp::Lerp;
pub use simulation::{
    FrictionSimulation, GravitySimulation, Simulation, SpringDescription, SpringSimulation,
    Tolerance,
};
pub use tween::Tween;
```

- [ ] **Step 3: Verify and commit**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

```bash
git add crates/flui-core/src/animation/
git commit -m "feat(animation): AnimationController + attach() helper"
```

---

### Task 5: animated() wrapper

**Files:**
- Create: `crates/flui-core/src/animation/animated.rs`
- Modify: `crates/flui-core/src/animation/mod.rs`

- [ ] **Step 1: Create `animated.rs`**

```rust
// crates/flui-core/src/animation/animated.rs

use crate::animation::controller::AnimationController;
use crate::{App, Entity, IntoElement, Window};

/// Render with an AnimationController, automatically scheduling frame updates.
///
/// Reads the controller's current value, calls `builder` with it, and
/// schedules the next frame if the animation is still running.
/// Users never need to call `window.request_animation_frame()` manually.
///
/// # Example
/// ```ignore
/// animated(&self.fade, window, cx, |opacity| {
///     div().opacity(opacity).child("Fading in...")
/// })
/// ```
pub fn animated<E: IntoElement>(
    controller: &Entity<AnimationController>,
    window: &mut Window,
    cx: &App,
    builder: impl FnOnce(f32) -> E,
) -> E {
    let ctrl = controller.read(cx);
    let value = ctrl.value();
    let animating = ctrl.is_animating();
    drop(ctrl);

    if animating {
        window.request_animation_frame();
    }

    builder(value)
}
```

- [ ] **Step 2: Update `animation/mod.rs`**

```rust
mod animated;
mod controller;
mod curve;
mod lerp;
mod simulation;
mod tween;

pub use animated::animated;
pub use controller::{AnimationController, AnimationStatus};
pub use curve::Curve;
pub use lerp::Lerp;
pub use simulation::{
    FrictionSimulation, GravitySimulation, Simulation, SpringDescription, SpringSimulation,
    Tolerance,
};
pub use tween::Tween;
```

- [ ] **Step 3: Verify and commit**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

```bash
git add crates/flui-core/src/animation/
git commit -m "feat(animation): animated() convenience wrapper"
```

---

### Task 6: Extend existing AnimationExt with Curve

**Files:**
- Modify: `crates/flui-core/src/elements/animation.rs`

- [ ] **Step 1: Add `.curve()` method to Animation struct**

In `crates/flui-core/src/elements/animation.rs`, the `Animation` struct (line 12) has field `pub easing: Rc<dyn Fn(f32) -> f32>`. Add a `curve` field and method:

Add field to `Animation` struct:
```rust
pub curve: Option<crate::animation::Curve>,
```

Initialize in `Animation::new()`:
```rust
curve: None,
```

Add method:
```rust
/// Set the easing curve. Overrides `with_easing()` if both are set.
pub fn curve(mut self, curve: crate::animation::Curve) -> Self {
    self.curve = Some(curve);
    self
}
```

In `AnimationElement::request_layout()` (line 163), change:
```rust
let delta = (self.animations[animation_ix].easing)(delta);
```
to:
```rust
let delta = if let Some(ref curve) = self.animations[animation_ix].curve {
    curve.transform(delta)
} else {
    (self.animations[animation_ix].easing)(delta)
};
```

- [ ] **Step 2: Verify and commit**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

```bash
git add crates/flui-core/src/elements/animation.rs
git commit -m "feat(animation): extend AnimationExt with Curve enum support"
```

---

### Task 7: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Run all animation tests**

Run: `cargo test -p flui-core -- animation:: 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 2: Check full workspace**

Run: `cargo check --workspace 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 3: Commit and push**

```bash
git push origin main
```
