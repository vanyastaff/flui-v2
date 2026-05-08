// crates/flui-core/src/animation/curve.rs
//
// S21 phase 1: `Curve` is now a TRAIT, not an enum. Concrete curve types
// (Linear, EaseIn, Cubic, BounceIn/Out/InOut, ElasticIn/Out/InOut, Interval,
// Threshold, SawTooth, FlippedCurve, Reversed, Split, CustomCurve) implement
// it. `Curves` (an empty marker struct with associated `pub const` items)
// is the named catalogue mirroring Flutter's `Curves.linear` / `Curves.bounceOut`
// / `Curves.fastOutSlowIn` / etc.
//
// The 2D / parametric curve family (Curve2D, CatmullRomCurve, CatmullRomSpline,
// ThreePointCubic, ParametricCurve<T>) lives in `curve_2d.rs` and is added
// in a follow-up commit; this file ships the 1D foundation that AnimationController
// and ElementAnimation consume.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::f32::consts::PI;
use std::sync::Arc;

// ============================================================================
// Curve trait
// ============================================================================

/// A parametric easing curve mapping the unit interval onto the unit interval
/// (with elastic curves the only documented exception — they may overshoot
/// `[0, 1]`).
///
/// **Flutter parity:** corresponds to
/// [`Curve`](https://api.flutter.dev/flutter/animation/Curve-class.html).
///
/// # Implementation contract
///
/// - `transform_internal` is the only required method. The default
///   `transform` clamps the input to `[0, 1]` and delegates.
/// - `derivative_at` is optional and returns `None` by default — curves
///   without a closed-form derivative (custom closures, Catmull-Rom splines,
///   sawtooth, etc.) leave it unimplemented; consumers fall back to numerical
///   differentiation (S21 phase 4 `AnimationController::velocity()`).
/// - `clone_box` exists so `Box<dyn Curve>` is `Clone`. Implementors typically
///   `Box::new(*self)` for `Copy` zero-sized types or `Box::new(self.clone())`.
///
/// # Object safety
///
/// `dyn Curve` is object-safe; storage as `Box<dyn Curve>` is the canonical
/// owned form (used by `AnimationController.curve`).
///
/// `Send + Sync` is a supertrait — animations may be passed between
/// threads in async pipelines (the animation runtime itself is single-threaded,
/// but the curve definitions can be).
pub trait Curve: Send + Sync + 'static {
    /// Transform `t` (clamped to `[0, 1]`) through this curve.
    fn transform(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        self.transform_internal(t)
    }

    /// Implementation-side hook. Receives `t` already clamped to `[0, 1]`.
    fn transform_internal(&self, t: f32) -> f32;

    /// Optional analytical derivative of [`Curve::transform`] at `t`.
    /// Curves with a closed-form derivative implement this; the default
    /// returns `None`, signalling consumers to use numerical differentiation.
    /// Used by `AnimationController::velocity()` in S21 phase 4.
    fn derivative_at(&self, _t: f32) -> Option<f32> {
        None
    }

    /// Box-clone helper — required for `Box<dyn Curve>: Clone`.
    fn clone_box(&self) -> Box<dyn Curve>;
}

impl Clone for Box<dyn Curve> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// ============================================================================
// Standard 1D easing curves
// ============================================================================

macro_rules! simple_curve {
    ($(#[$meta:meta])* $name:ident, |$t:ident| $body:expr) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Default, Debug)]
        pub struct $name;

        impl Curve for $name {
            fn transform_internal(&self, $t: f32) -> f32 {
                $body
            }
            fn clone_box(&self) -> Box<dyn Curve> {
                Box::new(*self)
            }
        }
    };
    ($(#[$meta:meta])* $name:ident, derivative |$dt:ident| $deriv:expr, |$t:ident| $body:expr) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Default, Debug)]
        pub struct $name;

        impl Curve for $name {
            fn transform_internal(&self, $t: f32) -> f32 {
                $body
            }
            fn derivative_at(&self, $dt: f32) -> Option<f32> {
                Some($deriv)
            }
            fn clone_box(&self) -> Box<dyn Curve> {
                Box::new(*self)
            }
        }
    };
}

simple_curve! {
    /// `t` — the identity curve.
    Linear,
    derivative |_t| 1.0,
    |t| t
}

simple_curve! {
    /// Quadratic ease-in: `t²`.
    EaseIn,
    derivative |t| 2.0 * t,
    |t| t * t
}

simple_curve! {
    /// Quadratic ease-out: `1 - (1 - t)²`.
    EaseOut,
    derivative |t| 2.0 * (1.0 - t),
    |t| 1.0 - (1.0 - t) * (1.0 - t)
}

simple_curve! {
    /// Quadratic ease-in-out (slow at boundaries, fast in the middle).
    EaseInOut,
    |t| {
        if t < 0.5 {
            2.0 * t * t
        } else {
            let x = -2.0 * t + 2.0;
            1.0 - x * x / 2.0
        }
    }
}

simple_curve! {
    /// Cubic ease-in: `t³`.
    EaseInCubic,
    derivative |t| 3.0 * t * t,
    |t| t * t * t
}

simple_curve! {
    /// Cubic ease-out.
    EaseOutCubic,
    derivative |t| 3.0 * (1.0 - t) * (1.0 - t),
    |t| 1.0 - (1.0 - t).powi(3)
}

simple_curve! {
    /// Cubic ease-in-out.
    EaseInOutCubic,
    |t| {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }
}

simple_curve! {
    /// Strongly decelerating curve — Flutter's `Curves.decelerate`.
    /// Maps `t -> 1 - (1 - t)²` then re-shaped; in Flutter it's
    /// `1 - (1 - t)² * (1 - t)`. We follow Flutter's formula.
    Decelerate,
    |t| {
        let inv = 1.0 - t;
        1.0 - inv * inv
    }
}

// Bounce family — three concrete types for in / out / inOut variants.

#[doc(hidden)]
fn bounce_out_impl(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
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
    }
}

simple_curve! {
    /// Bounce ease-in (slow build, then bounces in toward 1.0).
    BounceIn,
    |t| 1.0 - bounce_out_impl(1.0 - t)
}

simple_curve! {
    /// Bounce ease-out (decaying bounce toward 1.0).
    BounceOut,
    |t| bounce_out_impl(t)
}

simple_curve! {
    /// Bounce ease-in-out (mirror of `BounceOut` around 0.5).
    BounceInOut,
    |t| {
        if t < 0.5 {
            (1.0 - bounce_out_impl(1.0 - 2.0 * t)) * 0.5
        } else {
            (1.0 + bounce_out_impl(2.0 * t - 1.0)) * 0.5
        }
    }
}

// ============================================================================
// Cubic (parametric Bézier-derived curve)
// ============================================================================

/// Parametric cubic curve defined by two control points
/// `(x1, y1)`, `(x2, y2)` — corresponds to Flutter's
/// [`Cubic`](https://api.flutter.dev/flutter/animation/Cubic-class.html)
/// and to CSS `cubic-bezier(...)`.
#[derive(Copy, Clone, Debug)]
pub struct Cubic {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Cubic {
    /// Construct a new cubic curve from two control points.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

impl Curve for Cubic {
    fn transform_internal(&self, t: f32) -> f32 {
        cubic_bezier_transform(t, self.x1, self.y1, self.x2, self.y2)
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

fn cubic_bezier_transform(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
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

// ============================================================================
// Elastic family with parametric period (Flutter parity)
// ============================================================================

const DEFAULT_ELASTIC_PERIOD: f32 = 0.4;

/// Elastic ease-in. `period` controls oscillation frequency
/// (Flutter default: 0.4).
#[derive(Copy, Clone, Debug)]
pub struct ElasticIn {
    pub period: f32,
}

impl Default for ElasticIn {
    fn default() -> Self {
        Self {
            period: DEFAULT_ELASTIC_PERIOD,
        }
    }
}

impl Curve for ElasticIn {
    fn transform_internal(&self, t: f32) -> f32 {
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let s = self.period / 4.0;
        let t = t - 1.0;
        -(2.0_f32.powf(10.0 * t)) * ((t - s) * (2.0 * PI) / self.period).sin()
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

/// Elastic ease-out.
#[derive(Copy, Clone, Debug)]
pub struct ElasticOut {
    pub period: f32,
}

impl Default for ElasticOut {
    fn default() -> Self {
        Self {
            period: DEFAULT_ELASTIC_PERIOD,
        }
    }
}

impl Curve for ElasticOut {
    fn transform_internal(&self, t: f32) -> f32 {
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let s = self.period / 4.0;
        2.0_f32.powf(-10.0 * t) * ((t - s) * (2.0 * PI) / self.period).sin() + 1.0
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

/// Elastic ease-in-out.
#[derive(Copy, Clone, Debug)]
pub struct ElasticInOut {
    pub period: f32,
}

impl Default for ElasticInOut {
    fn default() -> Self {
        Self {
            period: DEFAULT_ELASTIC_PERIOD,
        }
    }
}

impl Curve for ElasticInOut {
    fn transform_internal(&self, t: f32) -> f32 {
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let s = self.period / 4.0;
        let t = t * 2.0 - 1.0;
        if t < 0.0 {
            -0.5 * 2.0_f32.powf(10.0 * t) * ((t - s) * (2.0 * PI) / self.period).sin()
        } else {
            2.0_f32.powf(-10.0 * t) * ((t - s) * (2.0 * PI) / self.period).sin() * 0.5 + 1.0
        }
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

// ============================================================================
// Composition primitives
// ============================================================================

/// Maps a sub-range of the timeline `[begin, end]` to `[0, 1]`, useful for
/// staggered animations with one controller. Outside the range, the curve
/// returns 0 (before `begin`) or 1 (after `end`).
///
/// `C` is the inner curve type — typically a zero-sized struct from the
/// `Curves` catalogue. For runtime-chosen curves use `Box<dyn Curve>`
/// (which itself implements `Curve`).
pub struct Interval<C: Curve> {
    pub begin: f32,
    pub end: f32,
    pub curve: C,
}

impl<C: Curve + Clone> Clone for Interval<C> {
    fn clone(&self) -> Self {
        Self {
            begin: self.begin,
            end: self.end,
            curve: self.curve.clone(),
        }
    }
}

impl<C: Curve> std::fmt::Debug for Interval<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interval")
            .field("begin", &self.begin)
            .field("end", &self.end)
            .finish()
    }
}

impl<C: Curve + Clone> Curve for Interval<C> {
    fn transform_internal(&self, t: f32) -> f32 {
        if t <= self.begin {
            0.0
        } else if t >= self.end {
            1.0
        } else {
            let local_t = (t - self.begin) / (self.end - self.begin);
            self.curve.transform(local_t)
        }
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(self.clone())
    }
}

/// Constant-zero up to `threshold`, then jumps to 1. Useful for triggers.
#[derive(Copy, Clone, Debug)]
pub struct Threshold(pub f32);

impl Curve for Threshold {
    fn transform_internal(&self, t: f32) -> f32 {
        if t < self.0 { 0.0 } else { 1.0 }
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

/// Repeats a sawtooth (rising 0→1) `count` times across `[0, 1]`.
#[derive(Copy, Clone, Debug)]
pub struct SawTooth(pub u32);

impl Curve for SawTooth {
    fn transform_internal(&self, t: f32) -> f32 {
        if self.0 == 0 {
            return t;
        }
        let count = self.0 as f32;
        let t = t * count;
        t - t.floor()
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
}

/// Flips a curve along the y-axis: `1 - inner.transform(1 - t)`. Equivalent
/// to using a curve in the reverse direction without rebinding the underlying
/// timeline.
#[derive(Clone, Debug)]
pub struct FlippedCurve<C: Curve + Clone>(pub C);

impl<C: Curve + Clone> Curve for FlippedCurve<C> {
    fn transform_internal(&self, t: f32) -> f32 {
        1.0 - self.0.transform(1.0 - t)
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(self.clone())
    }
}

/// Reverses a curve along the x-axis: `inner.transform(1 - t)`.
#[derive(Clone, Debug)]
pub struct Reversed<C: Curve + Clone>(pub C);

impl<C: Curve + Clone> Curve for Reversed<C> {
    fn transform_internal(&self, t: f32) -> f32 {
        self.0.transform(1.0 - t)
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(self.clone())
    }
}

/// Splits the timeline at `split_point`: applies `begin_curve` on `[0, split_point]`,
/// then `end_curve` on `[split_point, 1]`. Both halves are remapped to `[0, 1]`
/// internally before being passed to the inner curves.
pub struct Split<A: Curve + Clone, B: Curve + Clone> {
    pub split_point: f32,
    pub begin_curve: A,
    pub end_curve: B,
}

impl<A: Curve + Clone, B: Curve + Clone> Clone for Split<A, B> {
    fn clone(&self) -> Self {
        Self {
            split_point: self.split_point,
            begin_curve: self.begin_curve.clone(),
            end_curve: self.end_curve.clone(),
        }
    }
}

impl<A: Curve + Clone, B: Curve + Clone> std::fmt::Debug for Split<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Split")
            .field("split_point", &self.split_point)
            .finish()
    }
}

impl<A: Curve + Clone, B: Curve + Clone> Curve for Split<A, B> {
    fn transform_internal(&self, t: f32) -> f32 {
        if t < self.split_point {
            self.begin_curve.transform(t / self.split_point)
        } else {
            self.end_curve
                .transform((t - self.split_point) / (1.0 - self.split_point))
        }
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(self.clone())
    }
}

// ============================================================================
// CustomCurve — user-supplied closure
// ============================================================================

/// Curve defined by an arbitrary user closure. Stored as `Arc<dyn Fn>` so the
/// curve is `Clone + Send + Sync`.
///
/// Note: `derivative_at` returns `None` for custom curves — consumers
/// (`AnimationController::velocity`) fall back to numerical differentiation.
#[derive(Clone)]
pub struct CustomCurve(pub Arc<dyn Fn(f32) -> f32 + Send + Sync + 'static>);

impl CustomCurve {
    pub fn new<F: Fn(f32) -> f32 + Send + Sync + 'static>(f: F) -> Self {
        Self(Arc::new(f))
    }
}

impl std::fmt::Debug for CustomCurve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomCurve(<fn>)")
    }
}

impl Curve for CustomCurve {
    fn transform_internal(&self, t: f32) -> f32 {
        (self.0)(t)
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Curves catalogue (Flutter parity)
// ============================================================================

/// Named-curve catalogue. Mirrors Flutter's
/// [`Curves`](https://api.flutter.dev/flutter/animation/Curves-class.html)
/// static surface. Constants are zero-sized (or small `Cubic` parameter
/// holders) — no monomorphization explosion at call sites.
///
/// Naming uses Rust's idiomatic `SCREAMING_SNAKE_CASE` for `pub const`
/// items (e.g. `Curves::FAST_OUT_SLOW_IN` for Flutter's `Curves.fastOutSlowIn`).
pub struct Curves;

#[allow(non_upper_case_globals)] // Some constants follow Flutter naming exactly.
impl Curves {
    pub const LINEAR: Linear = Linear;
    pub const EASE_IN: EaseIn = EaseIn;
    pub const EASE_OUT: EaseOut = EaseOut;
    pub const EASE_IN_OUT: EaseInOut = EaseInOut;
    pub const EASE_IN_CUBIC: EaseInCubic = EaseInCubic;
    pub const EASE_OUT_CUBIC: EaseOutCubic = EaseOutCubic;
    pub const EASE_IN_OUT_CUBIC: EaseInOutCubic = EaseInOutCubic;
    pub const DECELERATE: Decelerate = Decelerate;

    pub const BOUNCE_IN: BounceIn = BounceIn;
    pub const BOUNCE_OUT: BounceOut = BounceOut;
    pub const BOUNCE_IN_OUT: BounceInOut = BounceInOut;

    pub const ELASTIC_IN: ElasticIn = ElasticIn {
        period: DEFAULT_ELASTIC_PERIOD,
    };
    pub const ELASTIC_OUT: ElasticOut = ElasticOut {
        period: DEFAULT_ELASTIC_PERIOD,
    };
    pub const ELASTIC_IN_OUT: ElasticInOut = ElasticInOut {
        period: DEFAULT_ELASTIC_PERIOD,
    };

    /// Material Design's standard Cubic curve. Slow start, steady end.
    pub const FAST_OUT_SLOW_IN: Cubic = Cubic::new(0.4, 0.0, 0.2, 1.0);

    /// Material Design's "slow out, fast in" — emphasis-incoming curves.
    pub const SLOW_MIDDLE: Cubic = Cubic::new(0.15, 0.85, 0.85, 0.15);

    /// "Ease" — CSS-flavored Cubic.
    pub const EASE: Cubic = Cubic::new(0.25, 0.1, 0.25, 1.0);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_identity() {
        assert_eq!(Linear.transform(0.0), 0.0);
        assert_eq!(Linear.transform(0.5), 0.5);
        assert_eq!(Linear.transform(1.0), 1.0);
    }

    #[test]
    fn linear_derivative_is_one() {
        assert_eq!(Linear.derivative_at(0.5), Some(1.0));
    }

    #[test]
    fn ease_in_boundaries_and_shape() {
        assert_eq!(EaseIn.transform(0.0), 0.0);
        assert_eq!(EaseIn.transform(1.0), 1.0);
        assert!(EaseIn.transform(0.5) < 0.5, "ease-in is slow at start");
    }

    #[test]
    fn ease_out_boundaries_and_shape() {
        assert_eq!(EaseOut.transform(0.0), 0.0);
        assert_eq!(EaseOut.transform(1.0), 1.0);
        assert!(EaseOut.transform(0.5) > 0.5, "ease-out is fast at start");
    }

    #[test]
    fn ease_in_out_midpoint_at_one_half() {
        assert_eq!(EaseInOut.transform(0.0), 0.0);
        assert_eq!(EaseInOut.transform(1.0), 1.0);
        assert!((EaseInOut.transform(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn bounce_out_endpoints() {
        assert!(BounceOut.transform(0.0).abs() < 0.01);
        assert!((BounceOut.transform(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn bounce_in_out_endpoints() {
        assert!(BounceInOut.transform(0.0).abs() < 0.01);
        assert!((BounceInOut.transform(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn elastic_in_endpoints() {
        let c = ElasticIn::default();
        assert_eq!(c.transform(0.0), 0.0);
        assert_eq!(c.transform(1.0), 1.0);
    }

    #[test]
    fn elastic_period_is_parametric() {
        let c1 = ElasticOut { period: 0.4 };
        let c2 = ElasticOut { period: 0.2 };
        // Different periods produce different mid-curve values.
        assert!((c1.transform(0.3) - c2.transform(0.3)).abs() > 1e-3);
    }

    #[test]
    fn cubic_endpoints_pin() {
        let c = Curves::FAST_OUT_SLOW_IN;
        assert!(c.transform(0.0).abs() < 1e-3);
        assert!((c.transform(1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn interval_before_begin_zero() {
        let c = Interval {
            begin: 0.3,
            end: 0.7,
            curve: Linear,
        };
        assert_eq!(c.transform(0.0), 0.0);
        assert_eq!(c.transform(0.2), 0.0);
    }

    #[test]
    fn interval_after_end_one() {
        let c = Interval {
            begin: 0.3,
            end: 0.7,
            curve: Linear,
        };
        assert_eq!(c.transform(0.8), 1.0);
        assert_eq!(c.transform(1.0), 1.0);
    }

    #[test]
    fn interval_midpoint_remaps() {
        let c = Interval {
            begin: 0.0,
            end: 0.5,
            curve: Linear,
        };
        assert!((c.transform(0.25) - 0.5).abs() < 0.01);
    }

    #[test]
    fn threshold_jumps() {
        let c = Threshold(0.5);
        assert_eq!(c.transform(0.0), 0.0);
        assert_eq!(c.transform(0.49), 0.0);
        assert_eq!(c.transform(0.5), 1.0);
        assert_eq!(c.transform(1.0), 1.0);
    }

    #[test]
    fn sawtooth_repeats() {
        let c = SawTooth(2);
        // 2 sawteeth across [0, 1] → midpoint of each tooth at 0.25, 0.75
        assert!((c.transform(0.25) - 0.5).abs() < 1e-5);
        assert!((c.transform(0.75) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn reversed_inverts() {
        let c = Reversed(EaseIn);
        // Reversed(EaseIn) at t=0 → EaseIn.transform(1) = 1
        assert!((c.transform(0.0) - 1.0).abs() < 0.01);
        assert!(c.transform(1.0).abs() < 0.01);
    }

    #[test]
    fn flipped_curve_endpoints() {
        let c = FlippedCurve(EaseIn);
        // FlippedCurve(EaseIn) at t=0 → 1 - EaseIn(1) = 0
        assert!((c.transform(0.0) - 0.0).abs() < 0.01);
        // FlippedCurve(EaseIn) at t=1 → 1 - EaseIn(0) = 1
        assert!((c.transform(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn split_remaps_each_half_to_unit() {
        let c = Split {
            split_point: 0.5,
            begin_curve: Linear,
            end_curve: EaseIn,
        };
        // First half is Linear over [0, 0.5] remapped to [0, 1]:
        // t=0      → Linear(0)    = 0
        // t=0.25   → Linear(0.5)  = 0.5
        // t=0.49   → Linear(0.98) = 0.98
        assert_eq!(c.transform(0.0), 0.0);
        assert!((c.transform(0.25) - 0.5).abs() < 0.01);
        assert!((c.transform(0.49) - 0.98).abs() < 1e-5);
        // At t = split_point the second curve takes over starting at its t=0
        // (Flutter parity — split is intentionally allowed to be discontinuous;
        // the user is responsible for picking compatible curves).
        // Second half is EaseIn over [0.5, 1.0] remapped to [0, 1]:
        // t=1   → EaseIn(1) = 1
        assert!(c.transform(0.5).abs() < 1e-5); // EaseIn(0) = 0 — proves discontinuity
        assert_eq!(c.transform(1.0), 1.0);
    }

    #[test]
    fn custom_curve() {
        let c = CustomCurve::new(|t| t * t * t);
        assert_eq!(c.transform(0.0), 0.0);
        assert!((c.transform(0.5) - 0.125).abs() < 0.01);
        assert_eq!(c.transform(1.0), 1.0);
    }

    #[test]
    fn input_clamped_to_unit_interval() {
        assert_eq!(Linear.transform(-0.5), 0.0);
        assert_eq!(Linear.transform(1.5), 1.0);
    }

    #[test]
    fn box_dyn_curve_is_clone() {
        let c: Box<dyn Curve> = Box::new(EaseInOut);
        let c2 = c.clone();
        assert!((c.transform(0.5) - c2.transform(0.5)).abs() < 1e-9);
    }

    #[test]
    fn curves_catalogue_constants_resolve() {
        // Smoke test: the catalogue constants compile and produce the
        // expected boundary values.
        assert_eq!(Curves::LINEAR.transform(0.5), 0.5);
        assert_eq!(Curves::EASE_IN.transform(1.0), 1.0);
        assert!((Curves::FAST_OUT_SLOW_IN.transform(1.0) - 1.0).abs() < 1e-3);
        assert_eq!(Curves::ELASTIC_IN.transform(0.0), 0.0);
        assert_eq!(Curves::ELASTIC_IN.transform(1.0), 1.0);
    }
}
