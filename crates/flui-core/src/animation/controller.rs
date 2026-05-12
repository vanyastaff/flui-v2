// crates/flui-core/src/animation/controller.rs
//
// AnimationController — persistent, reactive animation state container.
// S21 phase 0 task 0.7 wires it onto the new foundation: it consumes a
// `Ticker` (Clock-aware time source) for determinism, embeds
// `LocalListeners` / `LocalStatusListeners` for the new
// `Animation<T>` listener model, and implements `Animation<f64>`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked under S21 phase 7

use std::cell::Cell;
use std::time::Duration;

use crate::animation::animation::{
    Animation, ListenerCallback, ListenerId, StatusListenerCallback,
};
use crate::animation::behavior::{AnimationBehavior, AnimationStyle};
use crate::animation::curve::Linear;
use crate::animation::listeners::{LocalListeners, LocalStatusListeners};
use crate::animation::simulation::FrictionSimulation;
use crate::animation::status::AnimationStatus;
use crate::animation::ticker::Ticker;
use crate::animation::{Curve, Simulation};
use crate::frame::tick::{TickOutcome, TickTarget, TickTargetId};
use crate::scheduler::Instant;
use crate::{AppContext, Context, Entity};

/// A persistent animation state container.
///
/// Does NOT tick itself — parent view drives rendering via
/// [`animated()`](super::animated). Create with [`AnimationController::new`]
/// and attach to a view with [`AnimationController::attach`].
///
/// **Flutter parity:** corresponds to
/// [`AnimationController`](https://api.flutter.dev/flutter/animation/AnimationController-class.html).
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
///
/// # Time source
///
/// Once attached, the controller pulls elapsed time from a [`Ticker`] which
/// in turn consults the active [`Clock`](crate::scheduler::Clock) (production
/// `RealClock`, tests `TestClock`). Pre-attach instances fall back to
/// `web_time::Instant::now()` — they should not drive real animations,
/// `attach(cx)` is the only sanctioned construction path.
///
/// # Listener model
///
/// Embeds `LocalListeners` + `LocalStatusListeners` to satisfy the
/// [`Animation<f64>`] listener methods. Listeners fire after every state
/// transition (forward / reverse / repeat / stop / reset / animate_with).
/// On every notification the controller also calls `cx.notify()` so existing
/// `cx.observe(&entity, ...)` chains keep working unchanged — consumers
/// pick ONE of the two; subscribing to BOTH is supported but does not
/// double-fire (each side fires once per state transition).
pub struct AnimationController {
    value: f32,
    status: AnimationStatus,
    duration: Duration,
    reverse_duration: Option<Duration>,
    lower_bound: f32,
    upper_bound: f32,
    curve: Box<dyn Curve>,
    start_time: Option<Instant>,
    start_value: f32,
    /// Per-segment target value. `None` means "use upper_bound for Forward,
    /// lower_bound for Reverse" (existing forward/reverse semantics). `Some`
    /// is set by `animate_to` / `animate_back` for an explicit target.
    /// Cleared on `stop` / `reset` / `repeat` / `animate_with`.
    target_value: Option<f32>,
    repeating: bool,
    simulation: Option<Box<dyn Simulation>>,
    sim_start_time: Option<Instant>,

    // S21 phase 0: clock-aware time source. `None` until `.attach(cx)` runs.
    ticker: Option<Ticker>,

    // S21 phase 0: listener storage for the new Animation<T> trait.
    listeners: LocalListeners,
    status_listeners: LocalStatusListeners,

    // S21 phase 4: behaviour + style overrides.
    behavior: AnimationBehavior,
    /// Per-segment duration override. `None` means "use the controller's
    /// default `duration` / `reverse_duration`". Cleared on `stop` / `reset`.
    style_duration: Option<Duration>,
    /// Per-segment curve override. `None` means "use the controller's default
    /// `curve`". Cleared on `stop` / `reset`.
    style_curve: Option<Box<dyn Curve>>,

    // K04 Task 29: stable [`TickTargetId`] for the animation-tick walker
    // (Task 30 wires `App::active_animations: FxHashSet<TickTargetId>`).
    // Allocated once in `new()` and never mutated.
    tick_target_id: TickTargetId,

    /// K04 Task 31: per-frame cache for [`Self::value()`].
    ///
    /// The [`AnimationTick`](crate::frame::FramePhase::AnimationTick) walker
    /// records `Some(frame_now)` here at every tick. While set, the first
    /// `value()` call in the frame computes the curve / simulation against
    /// `frame_now` and caches the result in [`Self::value_cache`]; subsequent
    /// reads return the cache. `None` outside a frame (pre-attach or after
    /// the animation settles) — `value()` falls back to the legacy
    /// ticker-based path.
    ///
    /// `Cell<Option<Instant>>` because `value()` is `&self`; the cache must
    /// be set from a `&self` method on the tick path.
    last_tick_instant: Cell<Option<Instant>>,

    /// K04 Task 31: cached `value()` result keyed by the `Instant` at which
    /// it was computed. Hits when the next `value()` call sees the same
    /// `Instant` (per axiom P3: a single frame samples one `Instant`).
    ///
    /// Invalidated implicitly — every new `tick()` may overwrite
    /// `last_tick_instant`, and any cached entry whose key no longer matches
    /// is recomputed on the next read.
    value_cache: Cell<Option<(Instant, f32)>>,
}

impl AnimationController {
    /// Create a new controller with the given duration. Call `.attach(cx)`
    /// before driving real animations — pre-attach controllers do not have
    /// a Clock and will fall back to `Instant::now()`.
    pub fn new(duration: Duration) -> Self {
        Self {
            value: 0.0,
            status: AnimationStatus::Dismissed,
            duration,
            reverse_duration: None,
            lower_bound: 0.0,
            upper_bound: 1.0,
            curve: Box::new(Linear),
            start_time: None,
            start_value: 0.0,
            target_value: None,
            repeating: false,
            simulation: None,
            sim_start_time: None,
            ticker: None,
            listeners: LocalListeners::new(),
            status_listeners: LocalStatusListeners::new(),
            behavior: AnimationBehavior::default(),
            style_duration: None,
            style_curve: None,
            tick_target_id: TickTargetId::allocate(),
            last_tick_instant: Cell::new(None),
            value_cache: Cell::new(None),
        }
    }

    /// Override the [`AnimationBehavior`] used for this controller. Phase 4
    /// ships the field; `MediaQueryData.disableAnimations` integration lands
    /// alongside S14.
    pub fn with_behavior(mut self, behavior: AnimationBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    /// Replace the controller-level defaults from an [`AnimationStyle`].
    /// Each `Some(...)` field overrides the corresponding default; `None`
    /// fields are left as-is. Useful when constructing a controller from a
    /// theme.
    pub fn with_style(mut self, style: AnimationStyle) -> Self {
        if let Some(d) = style.duration {
            self.duration = d;
        }
        if let Some(d) = style.reverse_duration {
            self.reverse_duration = Some(d);
        }
        if let Some(c) = style.curve {
            self.curve = c;
        }
        // reverse_curve is currently unused (controller has no separate
        // reverse curve field today; CurvedAnimation handles per-direction
        // curves). Stored for parity but does not affect controller value.
        let _ = style.reverse_curve;
        self
    }

    /// Set the easing curve. Accepts any concrete type implementing
    /// [`Curve`] — typically a constant from
    /// [`Curves`](crate::animation::curve::Curves) (e.g.
    /// `Curves::EASE_IN_OUT`) or a custom struct.
    pub fn curve<C: Curve>(mut self, curve: C) -> Self {
        self.curve = Box::new(curve);
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

    /// Create an Entity, inject the active scheduler's [`Clock`](crate::scheduler::Clock)
    /// via a fresh [`Ticker`], and auto-observe the parent for re-render.
    ///
    /// This is the recommended way to construct an `AnimationController` —
    /// it's the only path that wires Clock injection. Pre-`attach`
    /// controllers do not have a Ticker and fall back to `Instant::now()`
    /// for elapsed-time computation.
    pub fn attach<V: 'static>(self, cx: &mut Context<V>) -> Entity<Self> {
        let mut this = self;
        // The outer `BackgroundExecutor` wraps `crate::scheduler::BackgroundExecutor`;
        // peel one layer to reach the active `Scheduler` and its `Clock`.
        let clock = cx
            .background_executor()
            .scheduler_executor()
            .scheduler()
            .clock();
        this.ticker = Some(Ticker::new(clock));
        let entity = cx.new(|_| this);
        cx.observe(&entity, |_, _, cx| cx.notify()).detach();
        entity
    }

    // ========================================================================
    // Internal: clock-aware "now"
    // ========================================================================

    /// Current time from the injected Clock (post-`attach`) or
    /// `web_time::Instant::now()` (pre-`attach` fallback).
    fn now(&self) -> Instant {
        match &self.ticker {
            Some(t) => t.now(),
            None => Instant::now(),
        }
    }

    // ========================================================================
    // State reading
    // ========================================================================

    /// Current animated value as `f32`. Renamed from `value()` in S21 phase 0
    /// — the bare `value()` method now belongs to the [`Animation<f64>`] trait
    /// impl (returns `f64` for Flutter parity).
    ///
    /// # K04 per-frame caching (Task 31, axiom P3)
    ///
    /// While the [`AnimationTick`](crate::frame::FramePhase::AnimationTick)
    /// walker has set [`Self::last_tick_instant`] for the current frame, the
    /// first `value()` call computes against that `Instant` and caches the
    /// result; subsequent reads within the frame return the cache. This
    /// makes multi-read sites (e.g. an animated view that reads `value()`
    /// from both `prepaint` and `paint`) observably consistent and removes
    /// the per-read `Clock::now()` cost.
    ///
    /// Outside a frame — pre-`attach`, or before the first tick of a new
    /// segment — falls back to the legacy `self.now()` (ticker- or
    /// wall-clock-based) read path with no caching.
    pub fn value(&self) -> f32 {
        // Resolve the "effective now". Prefer the AnimationTick-supplied
        // `last_tick_instant` (K04 P3: identical for every consumer in the
        // frame); fall back to the ticker for value() reads that happen
        // outside an AnimationTick (e.g. PreFrame, between frames).
        let now = self.last_tick_instant.get().unwrap_or_else(|| self.now());

        // Cache fast path: same `Instant` ⇒ identical result. Skipped when
        // no frame `Instant` is available (cache key would degenerate to a
        // per-call wall-clock read).
        if let Some((cached_at, val)) = self.value_cache.get()
            && cached_at == now
        {
            return val;
        }

        let computed = if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            let elapsed = now.saturating_duration_since(start).as_secs_f32();
            sim.x(elapsed).clamp(self.lower_bound, self.upper_bound)
        } else if let Some(start) = self.start_time {
            // Resolve duration: per-segment override > reverse_duration (when
            // status is Reverse) > controller default.
            let duration = self.style_duration.unwrap_or_else(|| match self.status {
                AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
                _ => self.duration,
            });

            // Resolve target: per-segment animate_to/animate_back override >
            // bound based on direction.
            let target = self.target_value.unwrap_or_else(|| match self.status {
                AnimationStatus::Forward | AnimationStatus::Completed => self.upper_bound,
                AnimationStatus::Reverse | AnimationStatus::Dismissed => self.lower_bound,
            });

            if duration.is_zero() {
                target
            } else {
                let elapsed = now.saturating_duration_since(start).as_secs_f32();
                let raw_t = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
                // Resolve curve: per-segment override > controller default.
                let curved_t = match &self.style_curve {
                    Some(c) => c.transform(raw_t),
                    None => self.curve.transform(raw_t),
                };
                self.start_value + curved_t * (target - self.start_value)
            }
        } else {
            self.value
        };

        // Only cache when we have a frame `Instant` — otherwise the cache
        // would never hit (each call resamples wall-clock).
        if self.last_tick_instant.get().is_some() {
            self.value_cache.set(Some((now, computed)));
        }
        computed
    }

    /// Whether the animation is currently running.
    pub fn is_animating(&self) -> bool {
        if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            let elapsed = self.now().saturating_duration_since(start).as_secs_f32();
            return !sim.is_done(elapsed);
        }

        if let Some(start) = self.start_time {
            let duration = match self.status {
                AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
                _ => self.duration,
            };
            let elapsed = self.now().saturating_duration_since(start);
            if elapsed >= duration {
                return self.repeating;
            }
            self.status.is_animating()
        } else {
            false
        }
    }

    /// Current animation status. (Inherent accessor preserved for callers
    /// that prefer struct-method access; the same value is exposed via
    /// [`Animation::status`].)
    pub fn current_status(&self) -> AnimationStatus {
        self.status
    }

    // ========================================================================
    // Internal: status transitions + listener fan-out
    // ========================================================================

    fn set_status(&mut self, new_status: AnimationStatus) {
        if self.status != new_status {
            log::debug!(
                target: "flui_core::animation::controller",
                "AnimationController status: {:?} -> {:?}",
                self.status,
                new_status
            );
            self.status = new_status;
            self.status_listeners.notify(new_status);
        }
    }

    fn notify_value(&self) {
        self.listeners.notify();
    }

    /// K04 Task 30: register this controller in `App::active_animations` so
    /// the next [`FramePhase::AnimationTick`](crate::frame::FramePhase::AnimationTick)
    /// phase walks it. Called from every `forward` / `reverse` / `animate_*`
    /// / `repeat` / `fling` entry.
    ///
    /// Idempotent: re-registering an already-active controller is a no-op
    /// (the `HashMap::insert` overwrites the weak handle with an equivalent
    /// one).
    fn register_for_tick(&self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.active_animations.insert(self.tick_target_id, weak);
    }

    /// K04 Task 30: drop this controller from `App::active_animations`. Called
    /// from `stop` / `reset` so settled controllers stop costing one
    /// `is_animating()` check per frame.
    ///
    /// Idempotent: removing an absent entry is a no-op.
    fn unregister_for_tick(&self, cx: &mut Context<Self>) {
        cx.active_animations.remove(&self.tick_target_id);
    }

    // ========================================================================
    // Control methods (each calls cx.notify() so existing observe chains stay alive)
    // ========================================================================

    /// Internal: clear ad-hoc per-segment overrides (target value + style
    /// override). Called from every method that starts a new segment so the
    /// previous segment's `animate_to` / `with_style` ad-hoc state does not
    /// leak.
    fn clear_segment_overrides(&mut self) {
        self.target_value = None;
        self.style_duration = None;
        self.style_curve = None;
    }

    /// Animate toward upper bound.
    pub fn forward(&mut self, cx: &mut Context<Self>) {
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::forward (was status={:?}, value={})",
            self.status,
            self.value()
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.clear_segment_overrides();
        self.start_value = self.value();
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Forward);
        self.repeating = false;
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Animate toward lower bound.
    pub fn reverse(&mut self, cx: &mut Context<Self>) {
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::reverse (was status={:?}, value={})",
            self.status,
            self.value()
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.clear_segment_overrides();
        self.start_value = self.value();
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Reverse);
        self.repeating = false;
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Animate from the current value to `target`. Direction is inferred:
    /// `Forward` when `target >= current`, `Reverse` when `target < current`.
    /// The `style` parameter overrides the controller's default duration and
    /// curve for this segment only — pass [`AnimationStyle::default`] for
    /// the controller defaults.
    ///
    /// **Flutter parity:** corresponds to `AnimationController.animateTo`.
    pub fn animate_to(&mut self, target: f32, style: AnimationStyle, cx: &mut Context<Self>) {
        let target = target.clamp(self.lower_bound, self.upper_bound);
        let current = self.value();
        let next_status = if target >= current {
            AnimationStatus::Forward
        } else {
            AnimationStatus::Reverse
        };
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::animate_to(target={}) — was value={} status={:?}",
            target,
            current,
            self.status
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.target_value = Some(target);
        self.style_duration = style.duration;
        self.style_curve = style.curve;
        // reverse_curve / reverse_duration from style are not consumed here —
        // a single animate_to segment uses ONE direction's parameters.
        let _ = (style.reverse_duration, style.reverse_curve);
        self.start_value = current;
        self.start_time = Some(self.now());
        self.set_status(next_status);
        self.repeating = false;
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Animate from the current value to `target` with explicit `Reverse`
    /// status (regardless of whether `target` is below `current`). Useful
    /// for status-driven UI transitions where the consumer wants the
    /// animation to be observably "reversing" even if the target is above
    /// the current value.
    ///
    /// **Flutter parity:** corresponds to `AnimationController.animateBack`.
    pub fn animate_back(&mut self, target: f32, style: AnimationStyle, cx: &mut Context<Self>) {
        let target = target.clamp(self.lower_bound, self.upper_bound);
        let current = self.value();
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::animate_back(target={}) — was value={} status={:?}",
            target,
            current,
            self.status
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.target_value = Some(target);
        self.style_duration = style.duration.or(style.reverse_duration);
        self.style_curve = style.curve.or(style.reverse_curve);
        self.start_value = current;
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Reverse);
        self.repeating = false;
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Drive the controller with a velocity-based fling. Phase 4 ships a
    /// simple friction-based fling — passing a custom [`Simulation`] via
    /// [`AnimationController::animate_with`] gives the consumer full
    /// control. The `behavior` parameter is stored but not yet consumed
    /// by the simulation choice (S14 wires it to `MediaQueryData.disableAnimations`).
    ///
    /// **Flutter parity:** corresponds to `AnimationController.fling`.
    pub fn fling(&mut self, velocity: f32, behavior: AnimationBehavior, cx: &mut Context<Self>) {
        // Override behavior for this fling — does not change the controller's
        // default behaviour.
        let _ = behavior; // future: switch friction/spring constants per behaviour.
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::fling(velocity={}) — was value={}",
            velocity,
            self.value()
        );
        let drag = 8.0_f32; // empirically chosen for "feels natural" flings.
        let sim = FrictionSimulation::new(drag, self.value(), velocity);
        self.animate_with(sim, cx);
    }

    /// Current animation velocity. Three-branch implementation:
    ///
    /// 1. If a [`Simulation`] is driving the controller, returns
    ///    `simulation.dx(elapsed)`.
    /// 2. Else, if the active curve has an analytical derivative
    ///    (`Curve::derivative_at` returns `Some`), returns
    ///    `derivative * (target - start) / duration_secs`.
    /// 3. Else, falls back to a numerical central finite-differences
    ///    estimate (epsilon = 1e-3) and emits a `log::trace!`.
    pub fn velocity(&self) -> f32 {
        // Branch 1: active simulation.
        if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            let elapsed = self.now().saturating_duration_since(start).as_secs_f32();
            return sim.dx(elapsed);
        }

        let Some(start) = self.start_time else {
            return 0.0;
        };

        // Compute current `t` and the active duration / target / curve.
        let duration = self.style_duration.unwrap_or_else(|| match self.status {
            AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
            _ => self.duration,
        });
        if duration.is_zero() {
            return 0.0;
        }
        let elapsed = self.now().saturating_duration_since(start).as_secs_f32();
        let raw_t = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);

        let target = self.target_value.unwrap_or_else(|| match self.status {
            AnimationStatus::Forward | AnimationStatus::Completed => self.upper_bound,
            AnimationStatus::Reverse | AnimationStatus::Dismissed => self.lower_bound,
        });
        let span = target - self.start_value;
        let dur_secs = duration.as_secs_f32();
        let active_curve: &dyn Curve = match &self.style_curve {
            Some(c) => c.as_ref(),
            None => self.curve.as_ref(),
        };

        // Branch 2: analytical derivative.
        if let Some(d) = active_curve.derivative_at(raw_t) {
            return d * span / dur_secs;
        }

        // Branch 3: numerical central differences with epsilon = 1e-3.
        log::trace!(
            target: "flui_core::animation::controller",
            "AnimationController::velocity falling back to numerical derivative for non-analytical curve"
        );
        const EPS: f32 = 1e-3;
        let t_lo = (raw_t - EPS).clamp(0.0, 1.0);
        let t_hi = (raw_t + EPS).clamp(0.0, 1.0);
        let v_lo = active_curve.transform(t_lo);
        let v_hi = active_curve.transform(t_hi);
        let dt = t_hi - t_lo;
        if dt > 0.0 {
            ((v_hi - v_lo) / dt) * span / dur_secs
        } else {
            0.0
        }
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
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::repeat (was status={:?})",
            self.status
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.clear_segment_overrides();
        self.start_value = self.lower_bound;
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Forward);
        self.repeating = true;
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Stop at current value.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        let value_now = self.value();
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::stop (was status={:?}, value={})",
            self.status,
            value_now
        );
        self.value = value_now;
        self.start_time = None;
        self.simulation = None;
        self.sim_start_time = None;
        self.clear_segment_overrides();
        self.repeating = false;
        let next_status = if (self.value - self.upper_bound).abs() < 0.001 {
            AnimationStatus::Completed
        } else if (self.value - self.lower_bound).abs() < 0.001 {
            AnimationStatus::Dismissed
        } else {
            self.status
        };
        self.set_status(next_status);
        self.unregister_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Reset to lower bound.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::reset (was status={:?})",
            self.status
        );
        self.value = self.lower_bound;
        self.start_time = None;
        self.simulation = None;
        self.sim_start_time = None;
        self.clear_segment_overrides();
        self.repeating = false;
        self.set_status(AnimationStatus::Dismissed);
        self.unregister_for_tick(cx);
        self.notify_value();
        cx.notify();
    }

    /// Drive animation with a physics simulation (spring, friction, gravity).
    pub fn animate_with(&mut self, simulation: impl Simulation + 'static, cx: &mut Context<Self>) {
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::animate_with (was status={:?})",
            self.status
        );
        self.start_time = None;
        self.clear_segment_overrides();
        self.sim_start_time = Some(self.now());
        self.simulation = Some(Box::new(simulation));
        self.set_status(AnimationStatus::Forward);
        self.register_for_tick(cx);
        self.notify_value();
        cx.notify();
    }
}

// ============================================================================
// Animation<f64> impl — Flutter-parity trait surface
// ============================================================================

impl Animation<f64> for AnimationController {
    /// Current value as `f64` (Flutter parity). Internally widens the
    /// existing `f32` curve / simulation math at the trait boundary —
    /// lossless widening.
    fn value(&self) -> f64 {
        self.value() as f64
    }

    fn status(&self) -> AnimationStatus {
        self.status
    }

    fn add_listener(&self, listener: ListenerCallback) -> ListenerId {
        self.listeners.add(listener)
    }

    fn remove_listener(&self, id: ListenerId) {
        self.listeners.remove(id);
    }

    fn add_status_listener(&self, listener: StatusListenerCallback) -> ListenerId {
        self.status_listeners.add(listener)
    }

    fn remove_status_listener(&self, id: ListenerId) {
        self.status_listeners.remove(id);
    }
}

// K04 Task 29: sealed [`TickTarget`] impl. The `AnimationTick` phase walker
// (Task 30) calls `tick(frame_clock.now())` once per frame for every active
// target. The controller's `value()` recomputes from `now()` on demand
// (Task 31 caches per-frame), so `tick` itself does not need to mutate the
// stored value — it only reports whether the controller still wants to be
// walked next frame.
impl TickTarget for AnimationController {
    fn tick(&mut self, now: Instant) -> TickOutcome {
        // K04 Task 31: seed the per-frame cache key so subsequent `value()`
        // reads within this frame return the cached result rather than
        // resampling the wall clock. Per axiom P3, every consumer in the
        // frame sees the same `Instant`.
        self.last_tick_instant.set(Some(now));
        // Invalidate the cached *value* so the next `value()` read computes
        // fresh against the new frame's `Instant`. The cache will repopulate
        // on first read; we don't recompute here to avoid paying for views
        // that ignore the controller this frame.
        self.value_cache.set(None);

        // The controller is "still animating" while a curve or simulation is
        // mid-flight. When neither is active, the active set drops this
        // target via `TickOutcome::Done`.
        if self.is_animating() {
            TickOutcome::Continue
        } else {
            TickOutcome::Done
        }
    }

    #[inline]
    fn id(&self) -> TickTargetId {
        self.tick_target_id
    }
}
