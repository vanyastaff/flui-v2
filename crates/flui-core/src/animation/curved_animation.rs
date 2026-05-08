// crates/flui-core/src/animation/curved_animation.rs
//
// S21 phase 1 task 1.6: `CurvedAnimation` — applies a `Curve` to a parent
// `Animation<f64>` and exposes the result as another `Animation<f64>`.
// The forward curve runs while the parent's status is `Forward` or
// `Completed`; the optional `reverse_curve` (defaults to the forward curve)
// runs while the parent's status is `Reverse` or `Dismissed`.
//
// This is the Animation<T>-decorator-pattern reference impl. Phase 3
// combinators (`ProxyAnimation`, `ReverseAnimation`, `CompoundAnimation`,
// `TrainHoppingAnimation`) follow the same listener-forwarding shape:
// subscribe to parent on construction; mirror parent's notifications to
// our own listener storage; unsubscribe in `Drop`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::cell::Cell;
use std::rc::Rc;

use crate::animation::animation::{
    Animation, ListenerCallback, ListenerId, StatusListenerCallback,
};
use crate::animation::curve::Curve;
use crate::animation::listeners::{LocalListeners, LocalStatusListeners};
use crate::animation::status::AnimationStatus;

/// Decorator: applies a [`Curve`] to a parent [`Animation<f64>`].
///
/// **Flutter parity:** corresponds to
/// [`CurvedAnimation`](https://api.flutter.dev/flutter/animation/CurvedAnimation-class.html).
///
/// # Forward / reverse semantics
///
/// - When the parent's status is `Forward` or `Completed`, `value()` returns
///   `forward_curve.transform(parent.value())`.
/// - When the parent's status is `Reverse` or `Dismissed`, `value()` returns
///   `reverse_curve.transform(parent.value())`.
/// - If `reverse_curve` is not specified, it defaults to the forward curve
///   (a clone of the same boxed `dyn Curve`).
///
/// # Listener forwarding
///
/// On construction the decorator subscribes to the parent's value and
/// status listeners. Whenever the parent fires, the decorator re-fires
/// its own listener storage. On drop, the parent subscriptions are
/// removed — preventing dangling notifications.
///
/// # Lifetime management
///
/// The parent is held as `Rc<dyn Animation<f64>>` because animation
/// notifiers are not `Send` and the decorator must keep the parent alive
/// for its own lifetime. Wrap an `AnimationController` accessed via
/// `Entity::read(cx)` into an `Rc<dyn Animation<f64>>` for use here — see
/// the example in the module docs once Phase 7 lands.
pub struct CurvedAnimation {
    parent: Rc<dyn Animation<f64>>,
    forward_curve: Box<dyn Curve>,
    reverse_curve: Box<dyn Curve>,
    listeners: Rc<LocalListeners>,
    status_listeners: Rc<LocalStatusListeners>,
    parent_value_id: Cell<Option<ListenerId>>,
    parent_status_id: Cell<Option<ListenerId>>,
}

impl CurvedAnimation {
    /// Wrap `parent` with `curve` (used in both directions). The decorator
    /// eagerly subscribes to the parent — listener forwarding is active
    /// from construction.
    pub fn new(parent: Rc<dyn Animation<f64>>, curve: impl Curve) -> Self {
        let forward_curve: Box<dyn Curve> = Box::new(curve);
        let reverse_curve = forward_curve.clone();
        Self::new_with_reverse_boxed(parent, forward_curve, reverse_curve)
    }

    /// Wrap `parent` with separate forward / reverse curves.
    pub fn with_reverse(
        parent: Rc<dyn Animation<f64>>,
        curve: impl Curve,
        reverse_curve: impl Curve,
    ) -> Self {
        let forward_curve: Box<dyn Curve> = Box::new(curve);
        let reverse_curve: Box<dyn Curve> = Box::new(reverse_curve);
        Self::new_with_reverse_boxed(parent, forward_curve, reverse_curve)
    }

    fn new_with_reverse_boxed(
        parent: Rc<dyn Animation<f64>>,
        forward_curve: Box<dyn Curve>,
        reverse_curve: Box<dyn Curve>,
    ) -> Self {
        let listeners = Rc::new(LocalListeners::new());
        let status_listeners = Rc::new(LocalStatusListeners::new());
        let value_id = {
            let listeners = Rc::clone(&listeners);
            parent.add_listener(ListenerCallback::new(move || {
                listeners.notify();
            }))
        };
        let status_id = {
            let status_listeners = Rc::clone(&status_listeners);
            parent.add_status_listener(StatusListenerCallback::new(move |status| {
                status_listeners.notify(status);
            }))
        };
        log::debug!(
            target: "flui_core::animation::curved_animation",
            "CurvedAnimation created (parent listener ids: value={:?}, status={:?})",
            value_id,
            status_id
        );
        Self {
            parent,
            forward_curve,
            reverse_curve,
            listeners,
            status_listeners,
            parent_value_id: Cell::new(Some(value_id)),
            parent_status_id: Cell::new(Some(status_id)),
        }
    }
}

impl Drop for CurvedAnimation {
    fn drop(&mut self) {
        if let Some(id) = self.parent_value_id.take() {
            self.parent.remove_listener(id);
        }
        if let Some(id) = self.parent_status_id.take() {
            self.parent.remove_status_listener(id);
        }
        log::trace!(
            target: "flui_core::animation::curved_animation",
            "CurvedAnimation dropped — parent listeners released"
        );
    }
}

impl Animation<f64> for CurvedAnimation {
    fn value(&self) -> f64 {
        let parent_value = self.parent.value();
        // Curve::transform takes f32 (1D math kept f32 per the Architecture
        // Overview). Widen back to f64 at the boundary.
        let t = parent_value as f32;
        let curve = match self.parent.status() {
            AnimationStatus::Reverse | AnimationStatus::Dismissed => &self.reverse_curve,
            _ => &self.forward_curve,
        };
        curve.transform(t) as f64
    }

    fn status(&self) -> AnimationStatus {
        self.parent.status()
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::animation::curve::{EaseIn, EaseOut, Linear};

    /// Minimal `Animation<f64>` impl for tests — a settable holder we can
    /// drive deterministically.
    struct TestSource {
        value: Cell<f64>,
        status: Cell<AnimationStatus>,
        listeners: LocalListeners,
        status_listeners: LocalStatusListeners,
    }

    impl TestSource {
        fn new() -> Rc<Self> {
            Rc::new(Self {
                value: Cell::new(0.0),
                status: Cell::new(AnimationStatus::Forward),
                listeners: LocalListeners::new(),
                status_listeners: LocalStatusListeners::new(),
            })
        }

        fn set_value(&self, v: f64) {
            self.value.set(v);
            self.listeners.notify();
        }

        fn set_status(&self, s: AnimationStatus) {
            self.status.set(s);
            self.status_listeners.notify(s);
        }
    }

    impl Animation<f64> for TestSource {
        fn value(&self) -> f64 {
            self.value.get()
        }
        fn status(&self) -> AnimationStatus {
            self.status.get()
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

    #[test]
    fn forward_curve_applied_in_forward_status() {
        let parent = TestSource::new();
        parent.set_status(AnimationStatus::Forward);
        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, EaseIn);

        parent.set_value(0.5);
        // EaseIn(0.5) = 0.25
        assert!((curved.value() - 0.25).abs() < 1e-3);
    }

    #[test]
    fn reverse_curve_applied_in_reverse_status_when_explicit() {
        let parent = TestSource::new();
        parent.set_status(AnimationStatus::Reverse);
        let curved = CurvedAnimation::with_reverse(
            parent.clone() as Rc<dyn Animation<f64>>,
            EaseIn,  // forward
            EaseOut, // reverse
        );

        parent.set_value(0.5);
        // EaseOut(0.5) = 1 - 0.25 = 0.75
        assert!((curved.value() - 0.75).abs() < 1e-3);
    }

    #[test]
    fn reverse_curve_defaults_to_forward_when_not_specified() {
        let parent = TestSource::new();
        parent.set_status(AnimationStatus::Reverse);
        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, EaseIn);

        parent.set_value(0.5);
        // Defaults to forward curve in reverse: EaseIn(0.5) = 0.25
        assert!((curved.value() - 0.25).abs() < 1e-3);
    }

    #[test]
    fn value_change_on_parent_fires_decorator_listeners() {
        let parent = TestSource::new();
        parent.set_status(AnimationStatus::Forward);
        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, Linear);

        let counter = Rc::new(Cell::new(0u32));
        let counter_in = Rc::clone(&counter);
        curved.add_listener(ListenerCallback::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));

        parent.set_value(0.3);
        parent.set_value(0.6);
        parent.set_value(0.9);

        assert_eq!(counter.get(), 3);
    }

    #[test]
    fn status_change_on_parent_fires_decorator_status_listeners() {
        let parent = TestSource::new();
        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, Linear);

        let captured: Rc<Cell<Option<AnimationStatus>>> = Rc::new(Cell::new(None));
        let captured_in = Rc::clone(&captured);
        curved.add_status_listener(StatusListenerCallback::new(move |status| {
            captured_in.set(Some(status));
        }));

        parent.set_status(AnimationStatus::Completed);
        assert_eq!(captured.get(), Some(AnimationStatus::Completed));
    }

    #[test]
    fn drop_releases_parent_listeners() {
        let parent = TestSource::new();
        // Sanity: before constructing the CurvedAnimation, the parent has
        // no listeners.
        assert_eq!(parent.listeners.len(), 0);
        assert_eq!(parent.status_listeners.len(), 0);

        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, Linear);
        assert_eq!(parent.listeners.len(), 1);
        assert_eq!(parent.status_listeners.len(), 1);

        drop(curved);
        assert_eq!(parent.listeners.len(), 0);
        assert_eq!(parent.status_listeners.len(), 0);
    }

    #[test]
    fn status_passes_through_unchanged() {
        let parent = TestSource::new();
        let curved = CurvedAnimation::new(parent.clone() as Rc<dyn Animation<f64>>, Linear);

        parent.set_status(AnimationStatus::Completed);
        assert_eq!(curved.status(), AnimationStatus::Completed);

        parent.set_status(AnimationStatus::Reverse);
        assert_eq!(curved.status(), AnimationStatus::Reverse);
    }
}
