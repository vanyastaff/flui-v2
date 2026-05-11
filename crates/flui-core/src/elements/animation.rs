use crate::scheduler::Instant;
use std::{rc::Rc, time::Duration};

use crate::{AnyElement, Element, ElementId, IntoElement};

pub use easing::*;
use smallvec::SmallVec;

/// An animation that can be applied to an element.
///
/// Renamed from `Animation` in S21 phase 0a to free the `Animation` symbol for the
/// new Flutter-parity `Animation<T>` trait that lands in `flui_core::animation` (Phase 0).
/// For higher-level declarative animations driven by listeners and a `Ticker`, see
/// `flui_core::animation::AnimationController` and the `animated()` wrapper. Widget-level
/// animation builders will arrive with the `flui-widgets` crate as a future S21 follow-up.
#[derive(Clone)]
pub struct ElementAnimation {
    /// The amount of time for which this animation should run
    pub duration: Duration,
    /// Whether to repeat this animation when it finishes
    pub oneshot: bool,
    /// A function that takes a delta between 0 and 1 and returns a new delta
    /// between 0 and 1 based on the given easing function.
    pub easing: Rc<dyn Fn(f32) -> f32>,
    /// An optional easing curve. Takes precedence over `easing` if set.
    /// Boxed `dyn Curve` (S21 phase 1 — `Curve` is now a trait, not an enum).
    pub curve: Option<Box<dyn crate::animation::Curve>>,
}

impl ElementAnimation {
    /// Create a new animation with the given duration.
    /// By default the animation will only run once and will use a linear easing function.
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            oneshot: true,
            easing: Rc::new(|t| t),
            curve: None,
        }
    }

    /// Set the animation to loop when it finishes.
    pub fn repeat(mut self) -> Self {
        self.oneshot = false;
        self
    }

    /// Set the easing function to use for this animation.
    /// The easing function will take a time delta between 0 and 1 and return a new delta
    /// between 0 and 1
    pub fn with_easing(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
        self.easing = Rc::new(easing);
        self
    }

    /// Set the easing curve. Takes precedence over `with_easing()` if both are set.
    /// Accepts any type implementing
    /// [`Curve`](crate::animation::Curve) — typically a constant from
    /// [`Curves`](crate::animation::Curves) (e.g.
    /// `Curves::EASE_IN_OUT`).
    pub fn curve<C: crate::animation::Curve>(mut self, curve: C) -> Self {
        self.curve = Some(Box::new(curve));
        self
    }
}

/// An extension trait for adding the animation wrapper to both Elements and Components
pub trait AnimationExt {
    /// Render this component or element with an animation
    fn with_animation(
        self,
        id: impl Into<ElementId>,
        animation: ElementAnimation,
        animator: impl Fn(Self, f32) -> Self + 'static,
    ) -> ElementAnimationElement<Self>
    where
        Self: Sized,
    {
        ElementAnimationElement {
            id: id.into(),
            element: Some(self),
            animator: Box::new(move |this, _, value| animator(this, value)),
            animations: smallvec::smallvec![animation],
        }
    }

    /// Render this component or element with a chain of animations
    fn with_animations(
        self,
        id: impl Into<ElementId>,
        animations: Vec<ElementAnimation>,
        animator: impl Fn(Self, usize, f32) -> Self + 'static,
    ) -> ElementAnimationElement<Self>
    where
        Self: Sized,
    {
        ElementAnimationElement {
            id: id.into(),
            element: Some(self),
            animator: Box::new(animator),
            animations: animations.into(),
        }
    }
}

impl<E: IntoElement + 'static> AnimationExt for E {}

/// A GPUI element that applies an animation to another element
pub struct ElementAnimationElement<E> {
    id: ElementId,
    element: Option<E>,
    animations: SmallVec<[ElementAnimation; 1]>,
    animator: Box<dyn Fn(E, usize, f32) -> E + 'static>,
}

impl<E> ElementAnimationElement<E> {
    /// Returns a new [`ElementAnimationElement<E>`] after applying the given function
    /// to the element being animated.
    pub fn map_element(mut self, f: impl FnOnce(E) -> E) -> ElementAnimationElement<E> {
        self.element = self.element.map(f);
        self
    }
}

impl<E: IntoElement + 'static> IntoElement for ElementAnimationElement<E> {
    type Element = ElementAnimationElement<E>;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct AnimationState {
    start: Instant,
    animation_ix: usize,
}

impl<E: IntoElement + 'static> Element for ElementAnimationElement<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        cx: &mut crate::LayoutCx<'_>,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let global_id = cx.global_id().cloned();
        cx.with_window_app(|window, cx| {
            // S21 phase 0.9: pull the current time from the active scheduler's
            // `Clock` instead of `Instant::now()` so element-level animations
            // become deterministic under `TestClock` (matches `AnimationController`'s
            // Ticker-driven elapsed-time path). Pre-compute once so the
            // `with_element_state` closure (which mutably borrows the App via
            // the inner `element.request_layout`) doesn't conflict with the
            // clock fetch.
            let now = cx
                .background_executor()
                .scheduler_executor()
                .scheduler()
                .clock()
                .now();
            let elapsed_since = |t: Instant| now.saturating_duration_since(t).as_secs_f32();

            window.with_element_state(global_id.as_ref().unwrap(), |state, window| {
                let mut state = state.unwrap_or_else(|| AnimationState {
                    start: now,
                    animation_ix: 0,
                });
                let animation_ix = state.animation_ix;

                let mut delta = elapsed_since(state.start)
                    / self.animations[animation_ix].duration.as_secs_f32();

                let mut done = false;
                if delta > 1.0 {
                    if self.animations[animation_ix].oneshot {
                        if animation_ix >= self.animations.len() - 1 {
                            done = true;
                        } else {
                            state.start = now;
                            state.animation_ix += 1;
                        }
                        delta = 1.0;
                    } else {
                        delta %= 1.0;
                    }
                }
                let delta = if let Some(ref curve) = self.animations[animation_ix].curve {
                    curve.transform(delta)
                } else {
                    (self.animations[animation_ix].easing)(delta)
                };

                debug_assert!(
                    (0.0..=1.0).contains(&delta),
                    "delta should always be between 0 and 1"
                );

                let element = self.element.take().expect("should only be called once");
                let mut element = (self.animator)(element, animation_ix, delta).into_any_element();

                if !done {
                    window.request_animation_frame();
                }

                let mut element_cx = crate::LayoutCx::new(window, cx, None, None);
                ((element.request_layout(&mut element_cx), element), state)
            })
        })
    }

    fn prepaint(
        &mut self,
        cx: &mut crate::PrepaintCx<'_>,
        element: &mut Self::RequestLayoutState,
    ) -> Self::PrepaintState {
        element.prepaint(cx);
    }

    fn paint(
        &mut self,
        cx: &mut crate::PaintCx<'_>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
    ) {
        element.paint(cx);
    }
}

/// Legacy free-function easings preserved for backward compatibility with
/// pre-S21 callers (`examples/learn/animation.rs`,
/// `examples/legacy/image_loading.rs`).
///
/// **Deprecated in S21 phase 1.8b** — prefer the trait-shaped curves in
/// [`crate::animation::Curves`]:
///
/// - `easing::linear` → [`Curves::LINEAR`](crate::animation::Curves::LINEAR)
/// - `easing::quadratic` → [`Curves::EASE_IN`](crate::animation::Curves::EASE_IN)
/// - `easing::ease_in_out` → [`Curves::EASE_IN_OUT`](crate::animation::Curves::EASE_IN_OUT)
/// - `easing::ease_out_quint` — no direct Curves equivalent; build a
///   [`CustomCurve`](crate::animation::CustomCurve) or use a `Cubic` approximation
/// - `easing::bounce` — Flutter uses
///   [`Curves::BOUNCE_IN_OUT`](crate::animation::Curves::BOUNCE_IN_OUT) for the
///   common case; the combinator wrapper has no Curves equivalent
/// - `easing::pulsating_between` — no Flutter equivalent; build a
///   [`CustomCurve`](crate::animation::CustomCurve)
///
/// The free functions here intentionally do NOT clamp their input (matching
/// pre-S21 behaviour); the new `Curve` trait clamps `t` to `[0, 1]` inside
/// `transform`. Keep these only while migrating call sites.
mod easing {
    use std::f32::consts::PI;

    /// The linear easing function, or delta itself.
    #[deprecated(
        note = "use `Curves::LINEAR.transform(t)` from `flui_core::animation::Curves` (S21 phase 1.8b)"
    )]
    pub fn linear(delta: f32) -> f32 {
        delta
    }

    /// The quadratic easing function, delta * delta.
    #[deprecated(
        note = "use `Curves::EASE_IN.transform(t)` from `flui_core::animation::Curves` (S21 phase 1.8b)"
    )]
    pub fn quadratic(delta: f32) -> f32 {
        delta * delta
    }

    /// The quadratic ease-in-out function, which starts and ends slowly but speeds up in the middle.
    #[deprecated(
        note = "use `Curves::EASE_IN_OUT.transform(t)` from `flui_core::animation::Curves` (S21 phase 1.8b)"
    )]
    pub fn ease_in_out(delta: f32) -> f32 {
        if delta < 0.5 {
            2.0 * delta * delta
        } else {
            let x = -2.0 * delta + 2.0;
            1.0 - x * x / 2.0
        }
    }

    /// The Quint ease-out function, which starts quickly and decelerates to a stop.
    #[deprecated(
        note = "no direct Curves equivalent; build a `CustomCurve` or `Cubic` approximation (S21 phase 1.8b)"
    )]
    pub fn ease_out_quint() -> impl Fn(f32) -> f32 {
        move |delta| 1.0 - (1.0 - delta).powi(5)
    }

    /// Apply the given easing function, first in the forward direction and then in the reverse direction.
    #[deprecated(
        note = "use `Curves::BOUNCE_IN_OUT` for the common bounce case; the combinator wrapper has no Curves equivalent (S21 phase 1.8b)"
    )]
    pub fn bounce(easing: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
        move |delta| {
            if delta < 0.5 {
                easing(delta * 2.0)
            } else {
                easing((1.0 - delta) * 2.0)
            }
        }
    }

    /// A custom easing function for pulsating alpha that slows down as it approaches 0.1.
    #[deprecated(
        note = "no Flutter / Curves equivalent; wrap an equivalent closure in `CustomCurve` (S21 phase 1.8b)"
    )]
    pub fn pulsating_between(min: f32, max: f32) -> impl Fn(f32) -> f32 {
        let range = max - min;

        move |delta| {
            // Use a combination of sine and cubic functions for a more natural breathing rhythm
            let t = (delta * 2.0 * PI).sin();
            let breath = (t * t * t + t) / 2.0;

            // Map the breath to our desired alpha range
            let normalized_alpha = (breath + 1.0) / 2.0;

            min + (normalized_alpha * range)
        }
    }
}
