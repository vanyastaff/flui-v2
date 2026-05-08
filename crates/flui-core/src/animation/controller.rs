// crates/flui-core/src/animation/controller.rs

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked separately

use crate::animation::status::AnimationStatus;
use crate::animation::{Curve, Simulation};
use crate::scheduler::Instant;
use crate::{AppContext, Context, Entity};
use std::time::Duration;

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
                AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
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
            return !sim.is_done(start.elapsed().as_secs_f32());
        }

        if let Some(start) = self.start_time {
            let duration = match self.status {
                AnimationStatus::Reverse => self.reverse_duration.unwrap_or(self.duration),
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
    pub fn animate_with(&mut self, simulation: impl Simulation + 'static, cx: &mut Context<Self>) {
        self.start_time = None;
        self.sim_start_time = Some(Instant::now());
        self.simulation = Some(Box::new(simulation));
        self.status = AnimationStatus::Forward;
        cx.notify();
    }
}
