// crates/flui-core/src/animation/controller.rs
//
// AnimationController — persistent, reactive animation state container.
// S21 phase 0 task 0.7 wires it onto the new foundation: it consumes a
// `Ticker` (Clock-aware time source) for determinism, embeds
// `LocalListeners` / `LocalStatusListeners` for the new
// `Animation<T>` listener model, and implements `Animation<f64>`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked under S21 phase 7

use std::time::Duration;

use crate::animation::animation::{
    Animation, ListenerCallback, ListenerId, StatusListenerCallback,
};
use crate::animation::curve::Linear;
use crate::animation::listeners::{LocalListeners, LocalStatusListeners};
use crate::animation::status::AnimationStatus;
use crate::animation::ticker::Ticker;
use crate::animation::{Curve, Simulation};
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
/// Embeds [`LocalListeners`] + [`LocalStatusListeners`] to satisfy the
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
    repeating: bool,
    simulation: Option<Box<dyn Simulation>>,
    sim_start_time: Option<Instant>,

    // S21 phase 0: clock-aware time source. `None` until `.attach(cx)` runs.
    ticker: Option<Ticker>,

    // S21 phase 0: listener storage for the new Animation<T> trait.
    listeners: LocalListeners,
    status_listeners: LocalStatusListeners,
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
            repeating: false,
            simulation: None,
            sim_start_time: None,
            ticker: None,
            listeners: LocalListeners::new(),
            status_listeners: LocalStatusListeners::new(),
        }
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

    /// Current animated value as `f32`. Recalculates from elapsed time on
    /// each call. Renamed from `value()` in S21 phase 0 — the bare
    /// `value()` method now belongs to the [`Animation<f64>`] trait impl
    /// (returns `f64` for Flutter parity).
    // TODO: consider per-frame caching if animated() is called multiple times.
    pub fn value(&self) -> f32 {
        if let (Some(sim), Some(start)) = (&self.simulation, self.sim_start_time) {
            let elapsed = self.now().saturating_duration_since(start).as_secs_f32();
            return sim.x(elapsed).clamp(self.lower_bound, self.upper_bound);
        }

        if let Some(start) = self.start_time {
            let duration = match self.status {
                AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
                _ => self.duration,
            };

            if duration.is_zero() {
                return match self.status {
                    AnimationStatus::Forward | AnimationStatus::Completed => self.upper_bound,
                    _ => self.lower_bound,
                };
            }

            let elapsed = self.now().saturating_duration_since(start).as_secs_f32();
            let raw_t = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
            let curved_t = self.curve.transform(raw_t);

            match self.status {
                AnimationStatus::Forward => {
                    self.start_value + curved_t * (self.upper_bound - self.start_value)
                }
                AnimationStatus::Reverse => {
                    self.start_value - curved_t * (self.start_value - self.lower_bound)
                }
                _ => self.value,
            }
        } else {
            self.value
        }
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

    // ========================================================================
    // Control methods (each calls cx.notify() so existing observe chains stay alive)
    // ========================================================================

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
        self.start_value = self.value();
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Forward);
        self.repeating = false;
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
        self.start_value = self.value();
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Reverse);
        self.repeating = false;
        self.notify_value();
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
        log::debug!(
            target: "flui_core::animation::controller",
            "AnimationController::repeat (was status={:?})",
            self.status
        );
        self.simulation = None;
        self.sim_start_time = None;
        self.start_value = self.lower_bound;
        self.start_time = Some(self.now());
        self.set_status(AnimationStatus::Forward);
        self.repeating = true;
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
        self.repeating = false;
        let next_status = if (self.value - self.upper_bound).abs() < 0.001 {
            AnimationStatus::Completed
        } else if (self.value - self.lower_bound).abs() < 0.001 {
            AnimationStatus::Dismissed
        } else {
            self.status
        };
        self.set_status(next_status);
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
        self.repeating = false;
        self.set_status(AnimationStatus::Dismissed);
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
        self.sim_start_time = Some(self.now());
        self.simulation = Some(Box::new(simulation));
        self.set_status(AnimationStatus::Forward);
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
