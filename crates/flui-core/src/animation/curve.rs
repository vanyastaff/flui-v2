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

/// `Box<dyn Curve>` itself implements `Curve`. This unlocks runtime-composed
/// curve composition primitives (`Interval<Box<dyn Curve>>`,
/// `Reversed<Box<dyn Curve>>`, `FlippedCurve<Box<dyn Curve>>`,
/// `Split<Box<dyn Curve>, Box<dyn Curve>>`) — without this impl, those generic
/// types are stack-only and cannot accept a runtime-chosen inner curve.
///
/// Added in the S21 review-fix pass; the previous code merely promised this
/// in a doc-comment without actually implementing it.
impl Curve for Box<dyn Curve> {
    fn transform(&self, t: f32) -> f32 {
        (**self).transform(t)
    }
    fn transform_internal(&self, t: f32) -> f32 {
        (**self).transform_internal(t)
    }
    fn derivative_at(&self, t: f32) -> Option<f32> {
        (**self).derivative_at(t)
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        (**self).clone_box()
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
    /// Formula: `1 - (1 - t)²` (quadratic ease-out).
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
///
/// Fields are private — `x1` and `x2` MUST be in `[0, 1]` for the Newton-Raphson
/// solver to converge correctly (CSS cubic-bezier constraint). Construct via
/// [`Cubic::new`], which `debug_assert!`s the constraint, or read components
/// via the field accessors. Made private in the S21 review-fix pass; previously
/// `pub` fields permitted illegal state without any validation.
#[derive(Copy, Clone, Debug)]
pub struct Cubic {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl Cubic {
    /// Construct a new cubic curve from two control points. In debug builds
    /// asserts that `x1, x2 ∈ [0, 1]`.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        // const fn debug_assert! is restricted; rely on assert! which is
        // const-stable from Rust 1.79.
        assert!(x1 >= 0.0 && x1 <= 1.0, "Cubic: x1 must be in [0, 1]");
        assert!(x2 >= 0.0 && x2 <= 1.0, "Cubic: x2 must be in [0, 1]");
        Self { x1, y1, x2, y2 }
    }

    /// Read the first control point's `x` coordinate.
    pub const fn x1(&self) -> f32 {
        self.x1
    }
    /// Read the first control point's `y` coordinate.
    pub const fn y1(&self) -> f32 {
        self.y1
    }
    /// Read the second control point's `x` coordinate.
    pub const fn x2(&self) -> f32 {
        self.x2
    }
    /// Read the second control point's `y` coordinate.
    pub const fn y2(&self) -> f32 {
        self.y2
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
// Spring (curve-based; replaces the pre-S21 `Curve::Spring` enum variant)
// ============================================================================

/// Damped-harmonic-oscillator curve. Successor to the pre-S21 `Curve::Spring`
/// enum variant — kept in the trait API so `controller.curve(Spring { ... })`
/// continues to work without forcing callers to switch to
/// `animate_with(SpringSimulation)`.
///
/// Solves the underdamped, critically-damped, or overdamped response of
/// `m·x'' + c·x' + k·(x − 1) = 0` over `t ∈ [0, 1]` with `m = 1`,
/// `k = stiffness`, `c = damping`. For richer parametrisation (initial
/// velocity, custom mass, configurable rest position) use
/// `SpringSimulation` via `AnimationController::animate_with`.
///
/// Invalid parameters (`stiffness <= 0.0`, non-finite values) fall back to
/// `Curves::LINEAR` rather than emitting `NaN`/`Inf`; controllers that need
/// strict validation should use `SpringSimulation`.
#[derive(Copy, Clone, Debug)]
pub struct Spring {
    pub damping: f32,
    pub stiffness: f32,
}

impl Spring {
    /// Construct a new spring curve.
    pub const fn new(damping: f32, stiffness: f32) -> Self {
        Self { damping, stiffness }
    }
}

impl Curve for Spring {
    fn transform_internal(&self, t: f32) -> f32 {
        // Guard invalid parameters: a non-positive stiffness collapses the
        // model (omega == 0 ⇒ zeta == NaN/Inf). Falling back to linear keeps
        // `Curve::transform` finite for misuse without panicking.
        if !self.stiffness.is_finite() || self.stiffness <= 0.0 || !self.damping.is_finite() {
            return t;
        }
        let omega = self.stiffness.sqrt();
        let zeta = self.damping / (2.0 * omega);
        if zeta < 1.0 {
            // Underdamped — oscillatory response.
            let wd = omega * (1.0 - zeta * zeta).sqrt();
            1.0 - (-zeta * omega * t).exp() * ((zeta * omega * t / wd).sin() + (wd * t).cos())
        } else if zeta > 1.0 {
            // Overdamped — two distinct real roots `r1 = -ω(ζ − √(ζ²−1))`,
            // `r2 = -ω(ζ + √(ζ²−1))`. With initial conditions
            // `x(0) = 0`, `x'(0) = 0` (rest position at 1), the response is
            //   x(t) = 1 - (r2·e^{r1 t} - r1·e^{r2 t}) / (r2 - r1).
            let s = (zeta * zeta - 1.0).sqrt();
            let r1 = -omega * (zeta - s);
            let r2 = -omega * (zeta + s);
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        } else {
            // Critically damped.
            1.0 - (1.0 + omega * t) * (-omega * t).exp()
        }
    }
    fn clone_box(&self) -> Box<dyn Curve> {
        Box::new(*self)
    }
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

    /// Default-tuned damped spring (S21 review-fix successor to the pre-S21
    /// `Curve::Spring` enum variant). Mass = 1, stiffness = 100, damping = 10
    /// → critically-damped fast settle.
    pub const SPRING: Spring = Spring::new(10.0, 100.0);
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

    // ------------------------------------------------------------------
    // proptest sweeps (S21 phase 6 partial)
    // ------------------------------------------------------------------
    //
    // Property-based invariants that strengthen the example-based tests
    // above. proptest is already a flui-core dep (Cargo.toml line 92).
    // Cases default to 256 — sufficient for fast curve-eval coverage; the
    // exhaustive sweep + criterion benches + animation-frame goldens land
    // as a follow-up after this partial Phase 6.

    use proptest::prelude::*;

    /// All curves must produce a finite output for any `t` in the unit
    /// interval — no NaN, no Inf, no panic. This is the weakest invariant
    /// every curve must obey.
    fn assert_finite(name: &str, value: f32, t: f32) {
        assert!(
            value.is_finite(),
            "{} produced non-finite {} at t={}",
            name,
            value,
            t
        );
    }

    proptest! {
        /// Boundary pinning: every monotone-bounded curve maps
        /// `t = 0 → 0` and `t = 1 → 1` (within float epsilon).
        /// Excludes elastic curves (which can equal 0 / 1 exactly at the
        /// endpoints but the formula uses a special-case branch — covered
        /// by the example-based tests).
        #[test]
        fn monotone_curves_pin_endpoints(
            _ in 0..1usize  // dummy — proptest! requires at least one input
        ) {
            for (name, c) in monotone_curves() {
                let v0 = c.transform(0.0);
                let v1 = c.transform(1.0);
                prop_assert!(
                    v0.abs() < 1e-3,
                    "{} transform(0) = {} (expected ~0)",
                    name, v0
                );
                prop_assert!(
                    (v1 - 1.0).abs() < 1e-3,
                    "{} transform(1) = {} (expected ~1)",
                    name, v1
                );
            }
        }

        /// Every curve in the standard catalogue produces finite output for
        /// any `t ∈ [0, 1]`.
        #[test]
        fn all_curves_produce_finite_output_in_unit_interval(
            t in 0.0_f32..=1.0_f32
        ) {
            for (name, c) in all_named_curves() {
                let v = c.transform(t);
                assert_finite(name, v, t);
            }
        }

        /// Output stays in `[0, 1]` for non-overshooting curves
        /// (Linear / EaseIn / EaseOut / EaseInOut / EaseInCubic /
        /// EaseOutCubic / EaseInOutCubic / Decelerate / BounceIn /
        /// BounceOut / BounceInOut / Cubic-with-monotone-control-points).
        /// Elastic family + parametric Cubic with overshooting controls are
        /// EXPLICITLY allowed to escape `[0, 1]` — Flutter parity.
        #[test]
        fn non_overshoot_curves_stay_in_unit_interval(
            t in 0.0_f32..=1.0_f32
        ) {
            for (name, c) in non_overshoot_curves() {
                let v = c.transform(t);
                prop_assert!(
                    (-1e-3..=1.0 + 1e-3).contains(&v),
                    "{} transform({}) = {} (expected in [0,1] within 1e-3)",
                    name, t, v
                );
            }
        }

        /// Linear is genuinely linear: transform(t) == t (within
        /// float epsilon).
        #[test]
        fn linear_is_identity(t in 0.0_f32..=1.0_f32) {
            prop_assert!((Linear.transform(t) - t).abs() < 1e-6);
        }

        /// Reversed(c) at t equals c at (1 - t).
        #[test]
        fn reversed_satisfies_reflection(t in 0.0_f32..=1.0_f32) {
            let inner = EaseIn;
            let reversed = Reversed(inner);
            let lhs = reversed.transform(t);
            let rhs = inner.transform(1.0 - t);
            prop_assert!((lhs - rhs).abs() < 1e-6);
        }

        /// FlippedCurve(c) at t equals 1 - c(1 - t).
        #[test]
        fn flipped_satisfies_reflection(t in 0.0_f32..=1.0_f32) {
            let inner = EaseInOut;
            let flipped = FlippedCurve(inner);
            let lhs = flipped.transform(t);
            let rhs = 1.0 - inner.transform(1.0 - t);
            prop_assert!((lhs - rhs).abs() < 1e-6);
        }

        /// Threshold: zero before threshold, one at-or-after. Always exactly.
        #[test]
        fn threshold_is_step_shaped(
            threshold in 0.0_f32..=1.0_f32,
            t in 0.0_f32..=1.0_f32,
        ) {
            let curve = Threshold(threshold);
            let v = curve.transform(t);
            if t < threshold {
                prop_assert_eq!(v, 0.0);
            } else {
                prop_assert_eq!(v, 1.0);
            }
        }

        /// SawTooth(n).transform(t) for n >= 1 always lies in [0, 1).
        /// (Endpoints t=0 and t=1 both map to 0 by the periodicity rule;
        /// every other value is < 1.)
        #[test]
        fn sawtooth_output_in_half_open_unit_interval(
            count in 1u32..=8u32,
            t in 0.0_f32..=1.0_f32,
        ) {
            let curve = SawTooth(count);
            let v = curve.transform(t);
            prop_assert!(v >= 0.0 && v < 1.0 + 1e-6);
        }

        /// Curves catalogue: LINEAR, EASE_IN, EASE_OUT, EASE_IN_OUT,
        /// FAST_OUT_SLOW_IN — all weakly monotone-non-decreasing on
        /// `[0, 1]` (any pair t1 < t2 implies transform(t1) <= transform(t2)
        /// within float epsilon).
        #[test]
        fn standard_curves_are_weakly_monotone(
            t1 in 0.0_f32..=1.0_f32,
            t2 in 0.0_f32..=1.0_f32,
        ) {
            // Order the inputs.
            let (lo, hi) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
            let curves: &[(&str, &dyn Curve)] = &[
                ("Linear", &Curves::LINEAR),
                ("EaseIn", &Curves::EASE_IN),
                ("EaseOut", &Curves::EASE_OUT),
                ("EaseInOut", &Curves::EASE_IN_OUT),
                ("FastOutSlowIn", &Curves::FAST_OUT_SLOW_IN),
            ];
            for (name, curve) in curves {
                let v_lo = curve.transform(lo);
                let v_hi = curve.transform(hi);
                prop_assert!(
                    v_hi >= v_lo - 1e-3,
                    "{}: transform({}) = {} > transform({}) = {} — not weakly monotone",
                    name, lo, v_lo, hi, v_hi
                );
            }
        }

        /// CurveTween (used in chain) applied to `Linear` is the identity
        /// at the f64 boundary.
        #[test]
        fn curve_tween_with_linear_is_identity_in_f64(t in 0.0_f64..=1.0_f64) {
            use super::super::tween::{Animatable, CurveTween};
            let ct = CurveTween::new(Linear);
            let v = <CurveTween<Linear> as Animatable<f64>>::transform(&ct, t);
            prop_assert!((v - t).abs() < 1e-6);
        }
    }

    /// Returns named curves whose output is bounded to `[0, 1]` (non-elastic,
    /// non-overshooting).
    fn non_overshoot_curves() -> Vec<(&'static str, Box<dyn Curve>)> {
        vec![
            ("Linear", Box::new(Linear)),
            ("EaseIn", Box::new(EaseIn)),
            ("EaseOut", Box::new(EaseOut)),
            ("EaseInOut", Box::new(EaseInOut)),
            ("EaseInCubic", Box::new(EaseInCubic)),
            ("EaseOutCubic", Box::new(EaseOutCubic)),
            ("EaseInOutCubic", Box::new(EaseInOutCubic)),
            ("Decelerate", Box::new(Decelerate)),
            ("BounceIn", Box::new(BounceIn)),
            ("BounceOut", Box::new(BounceOut)),
            ("BounceInOut", Box::new(BounceInOut)),
            ("FastOutSlowIn", Box::new(Curves::FAST_OUT_SLOW_IN)),
        ]
    }

    /// Returns named curves that pin `(0, 0)` and `(1, 1)` — the strongest
    /// "well-formed easing" set.
    fn monotone_curves() -> Vec<(&'static str, Box<dyn Curve>)> {
        non_overshoot_curves()
    }

    /// Every named curve, including elastic / overshooting ones.
    fn all_named_curves() -> Vec<(&'static str, Box<dyn Curve>)> {
        let mut v = non_overshoot_curves();
        v.push(("ElasticIn", Box::new(ElasticIn::default())));
        v.push(("ElasticOut", Box::new(ElasticOut::default())));
        v.push(("ElasticInOut", Box::new(ElasticInOut::default())));
        v
    }
}
