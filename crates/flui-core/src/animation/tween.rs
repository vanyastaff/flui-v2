// crates/flui-core/src/animation/tween.rs
//
// S21 phase 2: `Animatable<T>` trait + Tween family. Mirrors Flutter's
// `package:flutter/animation/Tween-class.html` surface — `Tween`,
// `ConstantTween`, `ReverseTween`, `CurveTween`, `IntTween`, `StepTween`,
// `ColorTween`, `SizeTween`, `RectTween`, `TweenSequence`,
// `FlippedTweenSequence`. The trait is `f64`-typed at the parametric `t`
// boundary (Flutter parity); concrete numeric/visual tweens cast at the
// `Lerp::lerp(_, _, t: f32)` boundary.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::marker::PhantomData;

use super::animation::Animation;
use super::curve::Curve;
use super::lerp::Lerp;
use crate::{Bounds, Hsla, Pixels, Size};

// ============================================================================
// Animatable<T> trait
// ============================================================================

/// An object that produces a value of type `T` from a parametric `t ∈ [0, 1]`.
///
/// **Flutter parity:** corresponds to
/// [`Animatable<T>`](https://api.flutter.dev/flutter/animation/Animatable-class.html).
///
/// # Object safety
///
/// Object-safe by design — `chain` lives on the [`AnimatableExt`] extension
/// trait so that `dyn Animatable<T>` remains usable (e.g. as
/// `Box<dyn Animatable<T>>` inside `TweenSequenceItem`).
pub trait Animatable<T>: 'static {
    /// Compute the value for parametric `t ∈ [0, 1]`. Implementors are
    /// responsible for clamping or interpreting out-of-range `t` as makes
    /// sense for their semantics (Tween clamps; CurveTween delegates to
    /// the curve which clamps).
    fn transform(&self, t: f64) -> T;

    /// Convenience: evaluate against an [`Animation<f64>`] by reading its
    /// current value and calling [`Animatable::transform`].
    fn evaluate(&self, animation: &dyn Animation<f64>) -> T {
        self.transform(animation.value())
    }
}

/// Extension methods on `Animatable<T>` that aren't object-safe (generic
/// methods would break `dyn Animatable<T>`). Use via `use AnimatableExt;`.
pub trait AnimatableExt<T>: Animatable<T> + Sized {
    /// Compose two animatables: the `parent: Animatable<f64>` runs first,
    /// then `self` applies on its output.
    ///
    /// Idiomatic use: `Tween::new(0.0, 100.0).chain(CurveTween { curve: EaseInOut })`
    /// builds a curve-aware tween that doesn't need a separate `CurvedAnimation`.
    fn chain<P: Animatable<f64>>(self, parent: P) -> ChainedAnimatable<P, Self, T> {
        ChainedAnimatable {
            parent,
            child: self,
            _t: PhantomData,
        }
    }
}

impl<T, A: Animatable<T> + Sized> AnimatableExt<T> for A {}

/// Composition of two animatables: `parent.transform(t)` produces an `f64`
/// that is fed into `child.transform(...)`.
pub struct ChainedAnimatable<P, C, T> {
    parent: P,
    child: C,
    _t: PhantomData<T>,
}

impl<P: Animatable<f64>, C: Animatable<T>, T: 'static> Animatable<T> for ChainedAnimatable<P, C, T> {
    fn transform(&self, t: f64) -> T {
        let intermediate = self.parent.transform(t);
        self.child.transform(intermediate)
    }
}

// ============================================================================
// Tween<T: Lerp>
// ============================================================================

/// Linear interpolation between two values of the same `Lerp`-able type.
///
/// **Flutter parity:** corresponds to
/// [`Tween<T>`](https://api.flutter.dev/flutter/animation/Tween-class.html).
#[derive(Clone, Debug)]
pub struct Tween<T: Lerp> {
    pub begin: T,
    pub end: T,
}

impl<T: Lerp> Tween<T> {
    /// Construct a new tween from `begin` to `end`.
    pub fn new(begin: T, end: T) -> Self {
        Self { begin, end }
    }

    /// Compute the interpolated value for `t ∈ [0, 1]` as `f32`. Kept for
    /// callers that already work in `f32` (notably `examples/animation_demo`
    /// which feeds `controller.value() -> f32` into the tween).
    ///
    /// For Flutter-parity `f64` access use [`Animatable::transform`].
    pub fn transform(&self, t: f32) -> T {
        if t <= 0.0 {
            self.begin.clone()
        } else if t >= 1.0 {
            self.end.clone()
        } else {
            self.begin.lerp(&self.end, t)
        }
    }
}

impl<T: Lerp + 'static> Animatable<T> for Tween<T> {
    fn transform(&self, t: f64) -> T {
        // Boundary-stable lerp: snap to endpoints rather than relying on
        // float arithmetic round-off.
        if t <= 0.0 {
            self.begin.clone()
        } else if t >= 1.0 {
            self.end.clone()
        } else {
            self.begin.lerp(&self.end, t as f32)
        }
    }
}

// ============================================================================
// ConstantTween / ReverseTween / CurveTween
// ============================================================================

/// Always returns the wrapped value, regardless of `t`. Useful as a
/// placeholder in chains and sequences.
#[derive(Clone, Debug)]
pub struct ConstantTween<T: Clone>(pub T);

impl<T: Clone + 'static> Animatable<T> for ConstantTween<T> {
    fn transform(&self, _t: f64) -> T {
        self.0.clone()
    }
}

/// Reversed variant: `transform(t) = lerp(end, begin, t)`. At `t = 0` returns
/// `end`; at `t = 1` returns `begin`.
#[derive(Clone, Debug)]
pub struct ReverseTween<T: Lerp> {
    pub begin: T,
    pub end: T,
}

impl<T: Lerp> ReverseTween<T> {
    pub fn new(begin: T, end: T) -> Self {
        Self { begin, end }
    }
}

impl<T: Lerp + 'static> Animatable<T> for ReverseTween<T> {
    fn transform(&self, t: f64) -> T {
        if t <= 0.0 {
            self.end.clone()
        } else if t >= 1.0 {
            self.begin.clone()
        } else {
            self.end.lerp(&self.begin, t as f32)
        }
    }
}

/// `Animatable<f64>` that applies a [`Curve`] to its parametric `t`.
/// Composes with other animatables via [`AnimatableExt::chain`]:
/// `Tween::new(0.0, 100.0).chain(CurveTween { curve: EaseInOut })`.
#[derive(Clone, Debug)]
pub struct CurveTween<C: Curve + Clone> {
    pub curve: C,
}

impl<C: Curve + Clone> CurveTween<C> {
    pub fn new(curve: C) -> Self {
        Self { curve }
    }
}

impl<C: Curve + Clone> Animatable<f64> for CurveTween<C> {
    fn transform(&self, t: f64) -> f64 {
        self.curve.transform(t as f32) as f64
    }
}

// ============================================================================
// Numeric tweens
// ============================================================================

/// Interpolation between two integers with `round`-to-nearest semantics.
#[derive(Copy, Clone, Debug)]
pub struct IntTween {
    pub begin: i64,
    pub end: i64,
}

impl IntTween {
    pub const fn new(begin: i64, end: i64) -> Self {
        Self { begin, end }
    }
}

impl Animatable<i64> for IntTween {
    fn transform(&self, t: f64) -> i64 {
        if t <= 0.0 {
            return self.begin;
        }
        if t >= 1.0 {
            return self.end;
        }
        let f = self.begin as f64 + (self.end as f64 - self.begin as f64) * t;
        f.round() as i64
    }
}

/// Interpolation between two integers with `floor` semantics — produces a
/// step-shaped output (each integer "lasts" `1 / (end - begin)` of the
/// timeline).
#[derive(Copy, Clone, Debug)]
pub struct StepTween {
    pub begin: i64,
    pub end: i64,
}

impl StepTween {
    pub const fn new(begin: i64, end: i64) -> Self {
        Self { begin, end }
    }
}

impl Animatable<i64> for StepTween {
    fn transform(&self, t: f64) -> i64 {
        if t <= 0.0 {
            return self.begin;
        }
        if t >= 1.0 {
            return self.end;
        }
        let f = self.begin as f64 + (self.end as f64 - self.begin as f64) * t;
        f.floor() as i64
    }
}

// ============================================================================
// Visual tweens — ColorTween, SizeTween, RectTween
// ============================================================================

/// Null-aware color interpolation (Flutter-parity `Color.lerp(null, b, t)`):
///
/// - `(Some(a), Some(b))` → standard HSL+alpha lerp.
/// - `(Some(a), None)` → `a` fades to fully-transparent same-hue.
/// - `(None, Some(b))` → fades in from transparent same-hue to `b`.
/// - `(None, None)` → `None`.
///
/// "Same-hue transparent" avoids the hue-flip artifact that a naive
/// `lerp(a, transparent_black, t)` would introduce.
#[derive(Clone, Debug, Default)]
pub struct ColorTween {
    pub begin: Option<Hsla>,
    pub end: Option<Hsla>,
}

impl ColorTween {
    pub fn new(begin: Option<Hsla>, end: Option<Hsla>) -> Self {
        Self { begin, end }
    }
}

impl Animatable<Option<Hsla>> for ColorTween {
    fn transform(&self, t: f64) -> Option<Hsla> {
        match (&self.begin, &self.end) {
            (None, None) => None,
            (Some(a), Some(b)) => {
                if t <= 0.0 {
                    Some(*a)
                } else if t >= 1.0 {
                    Some(*b)
                } else {
                    Some(a.lerp(b, t as f32))
                }
            }
            (Some(a), None) => {
                let mut transparent = *a;
                transparent.a = 0.0;
                Some(a.lerp(&transparent, t as f32))
            }
            (None, Some(b)) => {
                let mut transparent = *b;
                transparent.a = 0.0;
                Some(transparent.lerp(b, t as f32))
            }
        }
    }
}

/// Linear interpolation between two `Size<Pixels>`.
#[derive(Clone, Debug)]
pub struct SizeTween {
    pub begin: Size<Pixels>,
    pub end: Size<Pixels>,
}

impl SizeTween {
    pub fn new(begin: Size<Pixels>, end: Size<Pixels>) -> Self {
        Self { begin, end }
    }
}

impl Animatable<Size<Pixels>> for SizeTween {
    fn transform(&self, t: f64) -> Size<Pixels> {
        if t <= 0.0 {
            self.begin.clone()
        } else if t >= 1.0 {
            self.end.clone()
        } else {
            self.begin.lerp(&self.end, t as f32)
        }
    }
}

/// Linear interpolation between two `Bounds<Pixels>`. Lerps the origin and
/// size independently. Requires `Lerp for Bounds<Pixels>` — added below.
#[derive(Clone, Debug)]
pub struct RectTween {
    pub begin: Bounds<Pixels>,
    pub end: Bounds<Pixels>,
}

impl RectTween {
    pub fn new(begin: Bounds<Pixels>, end: Bounds<Pixels>) -> Self {
        Self { begin, end }
    }
}

impl Animatable<Bounds<Pixels>> for RectTween {
    fn transform(&self, t: f64) -> Bounds<Pixels> {
        if t <= 0.0 {
            self.begin.clone()
        } else if t >= 1.0 {
            self.end.clone()
        } else {
            self.begin.lerp(&self.end, t as f32)
        }
    }
}

// ============================================================================
// TweenSequence
// ============================================================================

/// One segment of a [`TweenSequence`]. The `weight` controls how much of the
/// total timeline this item occupies relative to other items in the same
/// sequence.
pub struct TweenSequenceItem<T: 'static> {
    pub tween: Box<dyn Animatable<T>>,
    pub weight: f64,
}

impl<T: 'static> TweenSequenceItem<T> {
    pub fn new(tween: Box<dyn Animatable<T>>, weight: f64) -> Self {
        Self { tween, weight }
    }
}

/// A sequence of weighted animatables that fire one after another along the
/// `[0, 1]` parametric timeline.
///
/// **Flutter parity:** corresponds to
/// [`TweenSequence`](https://api.flutter.dev/flutter/animation/TweenSequence-class.html).
///
/// Weights are normalized to `[0, 1]` at construction; the resulting boundaries
/// are stored as a cumulative array for `O(N)` `transform` lookup. For
/// typical `N ≤ 8` this is faster than binary search and simpler.
pub struct TweenSequence<T: 'static> {
    items: Vec<TweenSequenceItem<T>>,
    /// Cumulative end-points of each segment, normalized to `[0, 1]`.
    cumulative: Vec<f64>,
}

impl<T: 'static> TweenSequence<T> {
    /// Construct from a non-empty list of weighted items. Total weight must
    /// be positive; panics otherwise.
    pub fn new(items: Vec<TweenSequenceItem<T>>) -> Self {
        assert!(
            !items.is_empty(),
            "TweenSequence requires at least one item"
        );
        let total: f64 = items.iter().map(|i| i.weight).sum();
        assert!(
            total > 0.0 && total.is_finite(),
            "TweenSequence: total weight must be positive and finite"
        );
        let mut cumulative = Vec::with_capacity(items.len());
        let mut acc = 0.0_f64;
        for item in &items {
            acc += item.weight / total;
            cumulative.push(acc);
        }
        // Pin the final boundary at exactly 1.0 to absorb float drift.
        if let Some(last) = cumulative.last_mut() {
            *last = 1.0;
        }
        Self { items, cumulative }
    }

    /// Number of items in the sequence.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T: 'static> Animatable<T> for TweenSequence<T> {
    fn transform(&self, t: f64) -> T {
        let n = self.items.len();
        if t <= 0.0 {
            return self.items[0].tween.transform(0.0);
        }
        if t >= 1.0 {
            return self.items[n - 1].tween.transform(1.0);
        }
        for (i, &cum_end) in self.cumulative.iter().enumerate() {
            if t <= cum_end {
                let cum_start = if i == 0 { 0.0 } else { self.cumulative[i - 1] };
                let span = cum_end - cum_start;
                let local_t = if span <= 0.0 {
                    0.0
                } else {
                    (t - cum_start) / span
                };
                return self.items[i].tween.transform(local_t);
            }
        }
        // Defensive fallback (the `t >= 1.0` clamp above means we never
        // reach here under normal float arithmetic, but a NaN `t` could).
        self.items[n - 1].tween.transform(1.0)
    }
}

/// Reversed-order sequence: items are visited in reverse, and each item's
/// local `t` is also flipped (`1 - local_t`). Equivalent to running a
/// [`TweenSequence`] backward.
pub struct FlippedTweenSequence<T: 'static>(TweenSequence<T>);

impl<T: 'static> FlippedTweenSequence<T> {
    pub fn new(items: Vec<TweenSequenceItem<T>>) -> Self {
        Self(TweenSequence::new(items))
    }

    pub fn from_sequence(seq: TweenSequence<T>) -> Self {
        Self(seq)
    }
}

impl<T: 'static> Animatable<T> for FlippedTweenSequence<T> {
    fn transform(&self, t: f64) -> T {
        // Flip the timeline: feed (1 - t) into the underlying sequence.
        // Note this also reverses each segment's local progress, which
        // matches Flutter's `FlippedTweenSequence` semantics.
        let flipped = if t.is_nan() { 0.0 } else { 1.0 - t };
        self.0.transform(flipped)
    }
}

// ============================================================================
// Lerp for Bounds<Pixels>
// ============================================================================

impl Lerp for Bounds<Pixels> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Bounds {
            origin: self.origin.lerp(&other.origin, t),
            size: self.size.lerp(&other.size, t),
        }
    }
}

// Convenience: alias-style access to `Lerp` for callers who only use Tween.
// (No code change needed — `Lerp` is already public via `super::lerp`.)
#[allow(unused_imports)]
use super::lerp as _;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, px, size as size_fn};

    #[test]
    fn tween_inherent_transform_endpoints_and_midpoint() {
        let tween = Tween::new(0.0_f32, 100.0);
        assert_eq!(tween.transform(0.0_f32), 0.0);
        assert!((tween.transform(0.5_f32) - 50.0).abs() < 1e-3);
        assert_eq!(tween.transform(1.0_f32), 100.0);
    }

    #[test]
    fn tween_animatable_endpoints_and_midpoint() {
        let tween = Tween::new(0.0_f32, 100.0);
        assert_eq!(<Tween<f32> as Animatable<f32>>::transform(&tween, 0.0), 0.0);
        assert!(
            (<Tween<f32> as Animatable<f32>>::transform(&tween, 0.5) - 50.0).abs() < 1e-3
        );
        assert_eq!(<Tween<f32> as Animatable<f32>>::transform(&tween, 1.0), 100.0);
    }

    #[test]
    fn tween_clamps_out_of_range_input() {
        let tween = Tween::new(0.0_f32, 100.0);
        assert_eq!(<Tween<f32> as Animatable<f32>>::transform(&tween, -0.5), 0.0);
        assert_eq!(<Tween<f32> as Animatable<f32>>::transform(&tween, 1.5), 100.0);
    }

    #[test]
    fn reverse_tween_endpoints_swapped() {
        let tween = ReverseTween::new(0.0_f32, 100.0);
        assert_eq!(<ReverseTween<f32> as Animatable<f32>>::transform(&tween, 0.0), 100.0);
        assert_eq!(<ReverseTween<f32> as Animatable<f32>>::transform(&tween, 1.0), 0.0);
    }

    #[test]
    fn constant_tween_ignores_t() {
        let tween = ConstantTween(42i64);
        assert_eq!(<ConstantTween<i64> as Animatable<i64>>::transform(&tween, 0.0), 42);
        assert_eq!(<ConstantTween<i64> as Animatable<i64>>::transform(&tween, 0.5), 42);
        assert_eq!(<ConstantTween<i64> as Animatable<i64>>::transform(&tween, 1.0), 42);
    }

    #[test]
    fn curve_tween_applies_curve_at_f64_boundary() {
        use super::super::curve::EaseIn;
        let curve_tween = CurveTween::new(EaseIn);
        // EaseIn(0.5) = 0.25
        let v = <CurveTween<EaseIn> as Animatable<f64>>::transform(&curve_tween, 0.5);
        assert!((v - 0.25).abs() < 1e-3);
    }

    #[test]
    fn int_tween_rounds_to_nearest() {
        let tween = IntTween::new(0, 10);
        assert_eq!(tween.transform(0.0), 0);
        assert_eq!(tween.transform(0.49), 5);
        assert_eq!(tween.transform(0.5), 5); // 5.0 rounds to 5 (banker's rounding for .5 → even, but .0 is exact)
        assert_eq!(tween.transform(0.51), 5);
        assert_eq!(tween.transform(1.0), 10);
    }

    #[test]
    fn step_tween_floors() {
        let tween = StepTween::new(0, 10);
        assert_eq!(tween.transform(0.0), 0);
        assert_eq!(tween.transform(0.49), 4); // 4.9 → 4
        assert_eq!(tween.transform(0.99), 9); // 9.9 → 9
        assert_eq!(tween.transform(1.0), 10);
    }

    #[test]
    fn color_tween_both_some_lerps_in_hsla() {
        use crate::hsla;
        let begin = hsla(0.0, 1.0, 0.5, 1.0); // red
        let end = hsla(1.0 / 3.0, 1.0, 0.5, 1.0); // green (in 0..1 hue)
        let tween = ColorTween::new(Some(begin), Some(end));
        assert_eq!(tween.transform(0.0), Some(begin));
        assert_eq!(tween.transform(1.0), Some(end));
        let mid = tween.transform(0.5).unwrap();
        // Hue at midpoint should be between start and end:
        assert!((mid.h - 1.0 / 6.0).abs() < 1e-3);
    }

    #[test]
    fn color_tween_some_to_none_fades_alpha() {
        use crate::hsla;
        let begin = hsla(0.0, 1.0, 0.5, 1.0);
        let tween = ColorTween::new(Some(begin), None);
        let mid = tween.transform(0.5).unwrap();
        assert!((mid.a - 0.5).abs() < 1e-3, "alpha mid = {}", mid.a);
        // Hue stays the same — no hue flip.
        assert_eq!(mid.h, begin.h);
    }

    #[test]
    fn color_tween_none_to_none_stays_none() {
        let tween = ColorTween::new(None, None);
        assert_eq!(tween.transform(0.0), None);
        assert_eq!(tween.transform(0.5), None);
        assert_eq!(tween.transform(1.0), None);
    }

    #[test]
    fn size_tween_lerps_components() {
        let begin = size_fn(px(0.0), px(0.0));
        let end = size_fn(px(100.0), px(200.0));
        let tween = SizeTween::new(begin, end);
        let mid = tween.transform(0.5);
        assert!((mid.width.0 - 50.0).abs() < 1e-3);
        assert!((mid.height.0 - 100.0).abs() < 1e-3);
    }

    #[test]
    fn rect_tween_lerps_origin_and_size() {
        let begin = Bounds {
            origin: Point::new(px(0.0), px(0.0)),
            size: size_fn(px(0.0), px(0.0)),
        };
        let end = Bounds {
            origin: Point::new(px(10.0), px(20.0)),
            size: size_fn(px(100.0), px(200.0)),
        };
        let tween = RectTween::new(begin, end);
        let mid = tween.transform(0.5);
        assert!((mid.origin.x.0 - 5.0).abs() < 1e-3);
        assert!((mid.origin.y.0 - 10.0).abs() < 1e-3);
        assert!((mid.size.width.0 - 50.0).abs() < 1e-3);
        assert!((mid.size.height.0 - 100.0).abs() < 1e-3);
    }

    #[test]
    fn chain_curve_tween_then_value_tween() {
        use super::super::curve::EaseIn;
        // (Tween 0..100).chain(CurveTween(EaseIn))
        // At t=0.5: EaseIn(0.5) = 0.25 → Tween(0..100).transform(0.25) = 25
        let chained = Tween::new(0.0_f32, 100.0).chain(CurveTween::new(EaseIn));
        let v = <ChainedAnimatable<_, _, f32> as Animatable<f32>>::transform(&chained, 0.5);
        assert!((v - 25.0).abs() < 1.0);
    }

    #[test]
    fn tween_sequence_three_equal_segments() {
        let items = vec![
            TweenSequenceItem::new(Box::new(Tween::new(0.0_f32, 1.0)), 1.0),
            TweenSequenceItem::new(Box::new(Tween::new(1.0_f32, 0.0)), 1.0),
            TweenSequenceItem::new(Box::new(Tween::new(0.0_f32, 2.0)), 1.0),
        ];
        let seq = TweenSequence::new(items);
        // Three equal segments → boundaries at 1/3, 2/3, 1.
        // t = 1/6 → middle of first segment → Tween(0,1).transform(0.5) = 0.5
        let v = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 1.0 / 6.0);
        assert!((v - 0.5).abs() < 1e-3);
        // t = 1/2 → middle of second segment → Tween(1,0).transform(0.5) = 0.5
        let v2 = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 0.5);
        assert!((v2 - 0.5).abs() < 1e-3);
        // t = 5/6 → middle of third segment → Tween(0,2).transform(0.5) = 1.0
        let v3 = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 5.0 / 6.0);
        assert!((v3 - 1.0).abs() < 1e-3);
    }

    #[test]
    fn tween_sequence_weighted_segments() {
        let items = vec![
            TweenSequenceItem::new(Box::new(Tween::new(0.0_f32, 1.0)), 3.0),
            TweenSequenceItem::new(Box::new(Tween::new(1.0_f32, 2.0)), 1.0),
        ];
        let seq = TweenSequence::new(items);
        // Total weight 4 → first segment 0..0.75, second 0.75..1.
        // At t=0.75 (boundary): Tween(0,1).transform(1.0) = 1.0 OR Tween(1,2).transform(0.0) = 1.0
        let boundary = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 0.75);
        assert!((boundary - 1.0).abs() < 1e-3);
        // At t=0.875: middle of second segment → Tween(1,2).transform(0.5) = 1.5
        let mid = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 0.875);
        assert!((mid - 1.5).abs() < 1e-3);
    }

    #[test]
    fn tween_sequence_endpoints() {
        let items = vec![TweenSequenceItem::new(
            Box::new(Tween::new(0.0_f32, 100.0)),
            1.0,
        )];
        let seq = TweenSequence::new(items);
        assert_eq!(<TweenSequence<f32> as Animatable<f32>>::transform(&seq, 0.0), 0.0);
        assert_eq!(<TweenSequence<f32> as Animatable<f32>>::transform(&seq, 1.0), 100.0);
    }

    #[test]
    fn flipped_tween_sequence_runs_backward() {
        let items = vec![
            TweenSequenceItem::new(Box::new(Tween::new(0.0_f32, 1.0)), 1.0),
            TweenSequenceItem::new(Box::new(Tween::new(1.0_f32, 0.0)), 1.0),
        ];
        let flipped = FlippedTweenSequence::new(items);
        // At t=0 → equivalent to underlying at t=1 → second segment end → Tween(1,0).transform(1.0) = 0.0
        assert_eq!(<FlippedTweenSequence<f32> as Animatable<f32>>::transform(&flipped, 0.0), 0.0);
        // At t=1 → equivalent to underlying at t=0 → first segment start → Tween(0,1).transform(0.0) = 0.0
        assert_eq!(<FlippedTweenSequence<f32> as Animatable<f32>>::transform(&flipped, 1.0), 0.0);
    }

    #[test]
    #[should_panic(expected = "TweenSequence requires at least one item")]
    fn tween_sequence_panics_on_empty() {
        let _: TweenSequence<f32> = TweenSequence::new(Vec::new());
    }

    #[test]
    fn lerp_for_bounds_pixels() {
        let begin = Bounds {
            origin: Point::new(px(0.0), px(0.0)),
            size: size_fn(px(0.0), px(0.0)),
        };
        let end = Bounds {
            origin: Point::new(px(20.0), px(40.0)),
            size: size_fn(px(60.0), px(80.0)),
        };
        let mid = begin.lerp(&end, 0.5);
        assert!((mid.origin.x.0 - 10.0).abs() < 1e-3);
        assert!((mid.size.width.0 - 30.0).abs() < 1e-3);
    }

    // ------------------------------------------------------------------
    // proptest sweeps (S21 phase 6 partial)
    // ------------------------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        /// `Tween<f32>::transform` clamps to `[begin, end]` (or
        /// `[end, begin]` if begin > end) for any `t`.
        #[test]
        fn tween_f32_output_within_endpoint_bracket(
            begin in -1000.0_f32..=1000.0_f32,
            end in -1000.0_f32..=1000.0_f32,
            t in 0.0_f64..=1.0_f64,
        ) {
            let tween = Tween::new(begin, end);
            let v = <Tween<f32> as Animatable<f32>>::transform(&tween, t);
            let lo = begin.min(end);
            let hi = begin.max(end);
            // Float epsilon — allow tiny drift past the closed bracket.
            prop_assert!(
                v >= lo - 1e-3 && v <= hi + 1e-3,
                "Tween<f32>::transform({}) = {} outside bracket [{}, {}]",
                t, v, lo, hi
            );
        }

        /// `Tween<f32>::transform(0.0)` always returns exactly `begin`;
        /// `transform(1.0)` always returns exactly `end`. (Boundary
        /// pinning by the snap-to-endpoints code path.)
        #[test]
        fn tween_f32_endpoints_pinned_exactly(
            begin in -1000.0_f32..=1000.0_f32,
            end in -1000.0_f32..=1000.0_f32,
        ) {
            let tween = Tween::new(begin, end);
            prop_assert_eq!(
                <Tween<f32> as Animatable<f32>>::transform(&tween, 0.0),
                begin
            );
            prop_assert_eq!(
                <Tween<f32> as Animatable<f32>>::transform(&tween, 1.0),
                end
            );
        }

        /// `IntTween::transform` always returns an i64 inside the
        /// `[min(begin, end), max(begin, end)]` bracket.
        #[test]
        fn int_tween_output_within_bracket(
            begin in -1_000_000i64..=1_000_000i64,
            end in -1_000_000i64..=1_000_000i64,
            t in 0.0_f64..=1.0_f64,
        ) {
            let tween = IntTween::new(begin, end);
            let v = tween.transform(t);
            let lo = begin.min(end);
            let hi = begin.max(end);
            prop_assert!(v >= lo && v <= hi);
        }

        /// `StepTween::transform` always returns an i64 inside the bracket.
        /// Floor semantics → always <= rounded mid-result.
        #[test]
        fn step_tween_output_within_bracket(
            begin in -1_000_000i64..=1_000_000i64,
            end in -1_000_000i64..=1_000_000i64,
            t in 0.0_f64..=1.0_f64,
        ) {
            let tween = StepTween::new(begin, end);
            let v = tween.transform(t);
            let lo = begin.min(end);
            let hi = begin.max(end);
            prop_assert!(v >= lo && v <= hi);
        }

        /// `ConstantTween` ignores `t` entirely.
        #[test]
        fn constant_tween_value_invariant(
            value in -1000.0_f32..=1000.0_f32,
            t in 0.0_f64..=1.0_f64,
        ) {
            let tween = ConstantTween(value);
            let v = <ConstantTween<f32> as Animatable<f32>>::transform(&tween, t);
            prop_assert_eq!(v, value);
        }

        /// `Tween + ReverseTween` round-trip: applying both halves yields
        /// the original input within float epsilon.
        #[test]
        fn tween_reverse_tween_swap_endpoints(
            begin in -100.0_f32..=100.0_f32,
            end in -100.0_f32..=100.0_f32,
        ) {
            let forward = Tween::new(begin, end);
            let reverse = ReverseTween::new(begin, end);
            // forward.transform(0) = begin, reverse.transform(1) = begin.
            prop_assert!(
                (<Tween<f32> as Animatable<f32>>::transform(&forward, 0.0)
                    - <ReverseTween<f32> as Animatable<f32>>::transform(&reverse, 1.0))
                    .abs() < 1e-6
            );
        }

        /// `TweenSequence` endpoints: at `t = 0` always equals the first
        /// item's `transform(0)`; at `t = 1` always equals the last item's
        /// `transform(1)`. Holds for any positive-weight sequence.
        #[test]
        fn tween_sequence_endpoints_match_first_and_last_segment(
            n in 1usize..=5,
        ) {
            // Build a sequence of n equal-weight Tween(i as f32, (i+1) as f32) items.
            let items: Vec<TweenSequenceItem<f32>> = (0..n)
                .map(|i| {
                    TweenSequenceItem::new(
                        Box::new(Tween::new(i as f32, (i + 1) as f32)),
                        1.0,
                    )
                })
                .collect();
            let seq = TweenSequence::new(items);
            let v0 = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 0.0);
            let v1 = <TweenSequence<f32> as Animatable<f32>>::transform(&seq, 1.0);
            // First item starts at 0; last item ends at n.
            prop_assert!((v0 - 0.0).abs() < 1e-6);
            prop_assert!((v1 - n as f32).abs() < 1e-6);
        }

        /// `FlippedTweenSequence` is the inverse of its underlying sequence
        /// at the endpoints (transform(0) of flipped == transform(1) of inner).
        #[test]
        fn flipped_sequence_inverts_endpoints(
            n in 1usize..=4,
        ) {
            let make_items = || -> Vec<TweenSequenceItem<f32>> {
                (0..n)
                    .map(|i| {
                        TweenSequenceItem::new(
                            Box::new(Tween::new(i as f32, (i + 1) as f32)),
                            1.0,
                        )
                    })
                    .collect()
            };
            let inner = TweenSequence::new(make_items());
            let flipped = FlippedTweenSequence::new(make_items());
            let inner_at_one = <TweenSequence<f32> as Animatable<f32>>::transform(&inner, 1.0);
            let flipped_at_zero =
                <FlippedTweenSequence<f32> as Animatable<f32>>::transform(&flipped, 0.0);
            prop_assert!((inner_at_one - flipped_at_zero).abs() < 1e-6);
        }
    }
}
