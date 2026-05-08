// crates/flui-core/src/animation/combinator.rs
//
// S21 phase 3: Animation<T> combinators — `AlwaysStoppedAnimation`,
// `ProxyAnimation`, `ReverseAnimation`, `CompoundAnimation` (+
// `AnimationMin` / `AnimationMax` / `AnimationMean`), `TrainHoppingAnimation`.
//
// All listener-forwarding combinators follow the same shape established by
// `CurvedAnimation` in phase 1.6: subscribe to parent(s) on construction;
// fan parent notifications into our own `LocalListeners` /
// `LocalStatusListeners`; release parent subscriptions in `Drop`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::animation::{Animation, ListenerCallback, ListenerId, StatusListenerCallback};
use super::listeners::{LocalListeners, LocalStatusListeners};
use super::status::AnimationStatus;

// ============================================================================
// AlwaysStoppedAnimation<T>
// ============================================================================

/// Animation that always returns a fixed value and never notifies.
///
/// **Flutter parity:** corresponds to
/// [`AlwaysStoppedAnimation<T>`](https://api.flutter.dev/flutter/animation/AlwaysStoppedAnimation-class.html).
/// Status is hard-coded to `AnimationStatus::Forward` (Flutter default).
/// Listeners can be added but will never fire — `add_listener` returns a
/// fresh `ListenerId` for API parity, and `remove_listener` is a no-op.
pub struct AlwaysStoppedAnimation<T: Clone + 'static> {
    value: T,
}

impl<T: Clone + 'static> AlwaysStoppedAnimation<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T: Clone + 'static> Animation<T> for AlwaysStoppedAnimation<T> {
    fn value(&self) -> T {
        self.value.clone()
    }

    fn status(&self) -> AnimationStatus {
        AnimationStatus::Forward
    }

    fn add_listener(&self, _listener: ListenerCallback) -> ListenerId {
        // No-op: nothing ever fires. Return a fresh ID for API parity so the
        // caller's `remove_listener(id)` later is structurally valid.
        ListenerId::next()
    }

    fn remove_listener(&self, _id: ListenerId) {}

    fn add_status_listener(&self, _listener: StatusListenerCallback) -> ListenerId {
        ListenerId::next()
    }

    fn remove_status_listener(&self, _id: ListenerId) {}
}

// ============================================================================
// ProxyAnimation<T>
// ============================================================================

/// Animation whose parent can be swapped at runtime via [`set_parent`].
/// On swap, the proxy unsubscribes from the old parent, subscribes to the
/// new one, and re-fires its own listeners (so consumers re-render against
/// the new value).
///
/// **Flutter parity:** corresponds to
/// [`ProxyAnimation`](https://api.flutter.dev/flutter/animation/ProxyAnimation-class.html).
/// Phase 3 ships the always-has-parent variant — Flutter's
/// "kAlwaysDismissedAnimation" fallback for the no-parent case can be
/// added in a follow-up if a real consumer surfaces.
///
/// [`set_parent`]: ProxyAnimation::set_parent
pub struct ProxyAnimation<T: 'static> {
    parent: RefCell<Rc<dyn Animation<T>>>,
    parent_value_id: Cell<ListenerId>,
    parent_status_id: Cell<ListenerId>,
    listeners: Rc<LocalListeners>,
    status_listeners: Rc<LocalStatusListeners>,
}

impl<T: 'static> ProxyAnimation<T> {
    pub fn new(parent: Rc<dyn Animation<T>>) -> Self {
        let listeners = Rc::new(LocalListeners::new());
        let status_listeners = Rc::new(LocalStatusListeners::new());
        let value_id = subscribe_to_parent_value(parent.as_ref(), &listeners);
        let status_id = subscribe_to_parent_status(parent.as_ref(), &status_listeners);
        Self {
            parent: RefCell::new(parent),
            parent_value_id: Cell::new(value_id),
            parent_status_id: Cell::new(status_id),
            listeners,
            status_listeners,
        }
    }

    /// Replace the parent. Unsubscribes from the old, subscribes to the new,
    /// and re-fires our listeners + status listeners since value/status may
    /// have changed.
    pub fn set_parent(&self, new_parent: Rc<dyn Animation<T>>) {
        log::debug!(
            target: "flui_core::animation::combinator",
            "ProxyAnimation::set_parent — swapping parent"
        );
        let old = self.parent.replace(Rc::clone(&new_parent));
        old.remove_listener(self.parent_value_id.get());
        old.remove_status_listener(self.parent_status_id.get());

        let value_id = subscribe_to_parent_value(new_parent.as_ref(), &self.listeners);
        let status_id = subscribe_to_parent_status(new_parent.as_ref(), &self.status_listeners);
        self.parent_value_id.set(value_id);
        self.parent_status_id.set(status_id);

        // Fan-out: value and status both change with the swap.
        self.listeners.notify();
        self.status_listeners.notify(new_parent.status());
    }
}

impl<T: 'static> Drop for ProxyAnimation<T> {
    fn drop(&mut self) {
        let parent = self.parent.borrow();
        parent.remove_listener(self.parent_value_id.get());
        parent.remove_status_listener(self.parent_status_id.get());
    }
}

impl<T: Clone + 'static> Animation<T> for ProxyAnimation<T> {
    fn value(&self) -> T {
        self.parent.borrow().value()
    }

    fn status(&self) -> AnimationStatus {
        self.parent.borrow().status()
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
// ReverseAnimation
// ============================================================================

/// Inverts a `f64`-valued animation along the unit interval. `value()`
/// returns `1.0 - parent.value()`; status is flipped
/// (`Forward ↔ Reverse`, `Dismissed ↔ Completed`).
///
/// **Flutter parity:** corresponds to
/// [`ReverseAnimation`](https://api.flutter.dev/flutter/animation/ReverseAnimation-class.html).
/// Operates on `Animation<f64>` only — Flutter's variant is also
/// `Animation<double>`-only because "reverse" is undefined for arbitrary `T`.
pub struct ReverseAnimation {
    parent: Rc<dyn Animation<f64>>,
    parent_value_id: Cell<Option<ListenerId>>,
    parent_status_id: Cell<Option<ListenerId>>,
    listeners: Rc<LocalListeners>,
    status_listeners: Rc<LocalStatusListeners>,
}

impl ReverseAnimation {
    pub fn new(parent: Rc<dyn Animation<f64>>) -> Self {
        let listeners = Rc::new(LocalListeners::new());
        let status_listeners = Rc::new(LocalStatusListeners::new());
        let value_id = subscribe_to_parent_value(parent.as_ref(), &listeners);
        // Status listener: flip the status before fanning out.
        let status_listeners_clone = Rc::clone(&status_listeners);
        let status_id = parent.add_status_listener(Rc::new(move |s| {
            status_listeners_clone.notify(reverse_status(s));
        }));
        Self {
            parent,
            parent_value_id: Cell::new(Some(value_id)),
            parent_status_id: Cell::new(Some(status_id)),
            listeners,
            status_listeners,
        }
    }
}

impl Drop for ReverseAnimation {
    fn drop(&mut self) {
        if let Some(id) = self.parent_value_id.take() {
            self.parent.remove_listener(id);
        }
        if let Some(id) = self.parent_status_id.take() {
            self.parent.remove_status_listener(id);
        }
    }
}

impl Animation<f64> for ReverseAnimation {
    fn value(&self) -> f64 {
        1.0 - self.parent.value()
    }

    fn status(&self) -> AnimationStatus {
        reverse_status(self.parent.status())
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

fn reverse_status(s: AnimationStatus) -> AnimationStatus {
    match s {
        AnimationStatus::Forward => AnimationStatus::Reverse,
        AnimationStatus::Reverse => AnimationStatus::Forward,
        AnimationStatus::Dismissed => AnimationStatus::Completed,
        AnimationStatus::Completed => AnimationStatus::Dismissed,
        // Future variants pass through (AnimationStatus is #[non_exhaustive]).
        other => other,
    }
}

// ============================================================================
// CompoundAnimation<f64> + AnimationMin / AnimationMax / AnimationMean
// ============================================================================

/// Combines two parent animations into a derived `Animation<f64>`. The
/// `combine` closure produces the derived value from the two parents'
/// values; the combined status follows the rule documented on
/// [`CompoundAnimation::status`].
///
/// **Flutter parity:** loose correspondence to
/// [`CompoundAnimation<T>`](https://api.flutter.dev/flutter/animation/CompoundAnimation-class.html).
/// Flutter caches `_lastStatus` and `_nextStatus` for nuanced transitions —
/// our impl uses a simpler rule "the more active parent wins" which covers
/// the common cases (Min/Max/Mean usage).
pub struct CompoundAnimation<F>
where
    F: Fn(f64, f64) -> f64 + 'static,
{
    first: Rc<dyn Animation<f64>>,
    second: Rc<dyn Animation<f64>>,
    combine: F,
    first_value_id: Cell<Option<ListenerId>>,
    first_status_id: Cell<Option<ListenerId>>,
    second_value_id: Cell<Option<ListenerId>>,
    second_status_id: Cell<Option<ListenerId>>,
    listeners: Rc<LocalListeners>,
    status_listeners: Rc<LocalStatusListeners>,
}

impl<F> CompoundAnimation<F>
where
    F: Fn(f64, f64) -> f64 + 'static,
{
    pub fn new(first: Rc<dyn Animation<f64>>, second: Rc<dyn Animation<f64>>, combine: F) -> Self {
        let listeners = Rc::new(LocalListeners::new());
        let status_listeners = Rc::new(LocalStatusListeners::new());

        let first_value_id = subscribe_to_parent_value(first.as_ref(), &listeners);
        let second_value_id = subscribe_to_parent_value(second.as_ref(), &listeners);

        // For status: combined status is computed at read time, but we still
        // need to fan listeners on either parent's transition. Capture both
        // current statuses so we can compute the combined and notify.
        let first_clone = Rc::clone(&first);
        let second_clone_for_first = Rc::clone(&second);
        let status_listeners_for_first = Rc::clone(&status_listeners);
        let first_status_id = first.add_status_listener(Rc::new(move |_| {
            let combined =
                combined_status(first_clone.status(), second_clone_for_first.status());
            status_listeners_for_first.notify(combined);
        }));

        let first_clone_for_second = Rc::clone(&first);
        let second_clone = Rc::clone(&second);
        let status_listeners_for_second = Rc::clone(&status_listeners);
        let second_status_id = second.add_status_listener(Rc::new(move |_| {
            let combined =
                combined_status(first_clone_for_second.status(), second_clone.status());
            status_listeners_for_second.notify(combined);
        }));

        Self {
            first,
            second,
            combine,
            first_value_id: Cell::new(Some(first_value_id)),
            first_status_id: Cell::new(Some(first_status_id)),
            second_value_id: Cell::new(Some(second_value_id)),
            second_status_id: Cell::new(Some(second_status_id)),
            listeners,
            status_listeners,
        }
    }
}

impl<F> Drop for CompoundAnimation<F>
where
    F: Fn(f64, f64) -> f64 + 'static,
{
    fn drop(&mut self) {
        if let Some(id) = self.first_value_id.take() {
            self.first.remove_listener(id);
        }
        if let Some(id) = self.first_status_id.take() {
            self.first.remove_status_listener(id);
        }
        if let Some(id) = self.second_value_id.take() {
            self.second.remove_listener(id);
        }
        if let Some(id) = self.second_status_id.take() {
            self.second.remove_status_listener(id);
        }
    }
}

impl<F> Animation<f64> for CompoundAnimation<F>
where
    F: Fn(f64, f64) -> f64 + 'static,
{
    fn value(&self) -> f64 {
        (self.combine)(self.first.value(), self.second.value())
    }

    /// Combined status priority: Forward > Reverse > Completed > Dismissed.
    /// If either parent is animating (Forward/Reverse), the combined status
    /// is animating; if both are idle but at different bounds, the
    /// "more advanced" one (Completed) wins.
    fn status(&self) -> AnimationStatus {
        combined_status(self.first.status(), self.second.status())
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

fn combined_status(a: AnimationStatus, b: AnimationStatus) -> AnimationStatus {
    use AnimationStatus::*;
    if matches!(a, Forward) || matches!(b, Forward) {
        Forward
    } else if matches!(a, Reverse) || matches!(b, Reverse) {
        Reverse
    } else if matches!(a, Completed) || matches!(b, Completed) {
        Completed
    } else {
        Dismissed
    }
}

/// `Animation<f64>` whose value is `min(first, second)`.
///
/// **Flutter parity:** corresponds to
/// [`AnimationMin<T>`](https://api.flutter.dev/flutter/animation/AnimationMin-class.html)
/// (we ship the `f64` specialization — generic `Min<T: PartialOrd>` would
/// be straightforward as a follow-up).
pub fn animation_min(
    first: Rc<dyn Animation<f64>>,
    second: Rc<dyn Animation<f64>>,
) -> CompoundAnimation<impl Fn(f64, f64) -> f64> {
    CompoundAnimation::new(first, second, |a, b| a.min(b))
}

/// `Animation<f64>` whose value is `max(first, second)`.
pub fn animation_max(
    first: Rc<dyn Animation<f64>>,
    second: Rc<dyn Animation<f64>>,
) -> CompoundAnimation<impl Fn(f64, f64) -> f64> {
    CompoundAnimation::new(first, second, |a, b| a.max(b))
}

/// `Animation<f64>` whose value is `(first + second) / 2`.
pub fn animation_mean(
    first: Rc<dyn Animation<f64>>,
    second: Rc<dyn Animation<f64>>,
) -> CompoundAnimation<impl Fn(f64, f64) -> f64> {
    CompoundAnimation::new(first, second, |a, b| (a + b) * 0.5)
}

// ============================================================================
// TrainHoppingAnimation
// ============================================================================

/// Listens to two `Animation<f64>` parents simultaneously; once their values
/// cross, "hops" to the second and disposes the first. After the hop, the
/// animation behaves identically to the second parent. The hop is one-shot
/// — it never hops back.
///
/// **Flutter parity:** corresponds to
/// [`TrainHoppingAnimation`](https://api.flutter.dev/flutter/animation/TrainHoppingAnimation-class.html).
/// Used by route-transition swaps where mid-flight the destination animation
/// takes over from the source.
pub struct TrainHoppingAnimation {
    first: RefCell<Option<Rc<dyn Animation<f64>>>>,
    second: Rc<dyn Animation<f64>>,
    first_value_id: Cell<Option<ListenerId>>,
    first_status_id: Cell<Option<ListenerId>>,
    second_value_id: Cell<Option<ListenerId>>,
    second_status_id: Cell<Option<ListenerId>>,
    /// Sign of `first.value() - second.value()` at construction. The hop
    /// fires when the sign of the current difference flips.
    initial_sign: Cell<f64>,
    listeners: Rc<LocalListeners>,
    status_listeners: Rc<LocalStatusListeners>,
}

impl TrainHoppingAnimation {
    pub fn new(first: Rc<dyn Animation<f64>>, second: Rc<dyn Animation<f64>>) -> Rc<Self> {
        let initial = (first.value() - second.value()).signum();
        let listeners = Rc::new(LocalListeners::new());
        let status_listeners = Rc::new(LocalStatusListeners::new());

        let this = Rc::new(Self {
            first: RefCell::new(Some(Rc::clone(&first))),
            second: Rc::clone(&second),
            first_value_id: Cell::new(None),
            first_status_id: Cell::new(None),
            second_value_id: Cell::new(None),
            second_status_id: Cell::new(None),
            initial_sign: Cell::new(initial),
            listeners,
            status_listeners,
        });

        // First parent: listen for value changes that may trigger a hop.
        let this_for_first = Rc::downgrade(&this);
        let first_value_id = first.add_listener(Rc::new(move || {
            if let Some(s) = this_for_first.upgrade() {
                s.maybe_hop_and_notify_value();
            }
        }));
        this.first_value_id.set(Some(first_value_id));

        let listeners_clone = Rc::clone(&this.listeners);
        let first_status_id = first.add_status_listener(Rc::new(move |_| {
            // Status from first only matters until we hop. Forward via value
            // listeners (status fan-out happens through second after hop).
            let _ = &listeners_clone; // referenced to keep alive
        }));
        this.first_status_id.set(Some(first_status_id));

        // Second parent: always forwards (it's the eventual owner).
        let listeners_for_second = Rc::clone(&this.listeners);
        let second_value_id = second.add_listener(Rc::new(move || {
            listeners_for_second.notify();
        }));
        this.second_value_id.set(Some(second_value_id));

        let status_listeners_for_second = Rc::clone(&this.status_listeners);
        let second_status_id = second.add_status_listener(Rc::new(move |s| {
            status_listeners_for_second.notify(s);
        }));
        this.second_status_id.set(Some(second_status_id));

        this
    }

    /// Called from the first parent's value listener. Checks whether the
    /// values have crossed; if so, disposes the first parent's subscription
    /// and switches to second-only operation.
    fn maybe_hop_and_notify_value(&self) {
        // If we've already hopped, nothing to check.
        if self.first.borrow().is_none() {
            return;
        }
        let first = self.first.borrow().as_ref().map(Rc::clone);
        if let Some(first) = first {
            let current_sign = (first.value() - self.second.value()).signum();
            // A hop fires when:
            // - the initial sign was non-zero (parents started apart), AND
            // - the current sign is opposite (or zero — exact crossing).
            let initial = self.initial_sign.get();
            if initial != 0.0 && current_sign != initial {
                log::debug!(
                    target: "flui_core::animation::combinator",
                    "TrainHoppingAnimation: hop fired — disposing first parent"
                );
                if let Some(id) = self.first_value_id.take() {
                    first.remove_listener(id);
                }
                if let Some(id) = self.first_status_id.take() {
                    first.remove_status_listener(id);
                }
                self.first.replace(None);
            } else {
                // No hop yet: forward first's value change to our listeners
                // (we are still effectively "showing" the first parent).
                self.listeners.notify();
                return;
            }
        }
        // After the hop (or if already hopped), notify with current value
        // (which is now second.value()).
        self.listeners.notify();
    }
}

impl Drop for TrainHoppingAnimation {
    fn drop(&mut self) {
        if let Some(first) = self.first.borrow_mut().take() {
            if let Some(id) = self.first_value_id.take() {
                first.remove_listener(id);
            }
            if let Some(id) = self.first_status_id.take() {
                first.remove_status_listener(id);
            }
        }
        if let Some(id) = self.second_value_id.take() {
            self.second.remove_listener(id);
        }
        if let Some(id) = self.second_status_id.take() {
            self.second.remove_status_listener(id);
        }
    }
}

impl Animation<f64> for TrainHoppingAnimation {
    fn value(&self) -> f64 {
        match self.first.borrow().as_ref() {
            Some(f) => f.value(),
            None => self.second.value(),
        }
    }

    fn status(&self) -> AnimationStatus {
        match self.first.borrow().as_ref() {
            Some(f) => f.status(),
            None => self.second.status(),
        }
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
// Internal helpers
// ============================================================================

fn subscribe_to_parent_value<T: 'static>(
    parent: &dyn Animation<T>,
    listeners: &Rc<LocalListeners>,
) -> ListenerId {
    let listeners = Rc::clone(listeners);
    parent.add_listener(Rc::new(move || {
        listeners.notify();
    }))
}

fn subscribe_to_parent_status<T: 'static>(
    parent: &dyn Animation<T>,
    status_listeners: &Rc<LocalStatusListeners>,
) -> ListenerId {
    let status_listeners = Rc::clone(status_listeners);
    parent.add_status_listener(Rc::new(move |status| {
        status_listeners.notify(status);
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal `Animation<f64>` for tests — value / status are settable.
    /// Notifies on every set.
    struct TestSource {
        value: Cell<f64>,
        status: Cell<AnimationStatus>,
        listeners: LocalListeners,
        status_listeners: LocalStatusListeners,
    }

    impl TestSource {
        fn new(value: f64, status: AnimationStatus) -> Rc<Self> {
            Rc::new(Self {
                value: Cell::new(value),
                status: Cell::new(status),
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

    // ------------------------------------------------------------------
    // AlwaysStoppedAnimation
    // ------------------------------------------------------------------

    #[test]
    fn always_stopped_returns_constant_value_and_forward_status() {
        let a = AlwaysStoppedAnimation::new(0.42_f64);
        assert_eq!(<AlwaysStoppedAnimation<f64> as Animation<f64>>::value(&a), 0.42);
        assert_eq!(<AlwaysStoppedAnimation<f64> as Animation<f64>>::status(&a), AnimationStatus::Forward);
    }

    #[test]
    fn always_stopped_listeners_are_no_op() {
        let a = AlwaysStoppedAnimation::new(0.0_f64);
        let counter = Rc::new(Cell::new(0u32));
        let counter_in = Rc::clone(&counter);
        let id = a.add_listener(Rc::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));
        // No mechanism causes notification; the counter never increments.
        a.remove_listener(id); // no-op, must not panic.
        assert_eq!(counter.get(), 0);
    }

    // ------------------------------------------------------------------
    // ProxyAnimation
    // ------------------------------------------------------------------

    #[test]
    fn proxy_passes_through_initial_parent() {
        let parent = TestSource::new(0.5, AnimationStatus::Forward);
        let proxy = ProxyAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        assert_eq!(<ProxyAnimation<f64> as Animation<f64>>::value(&proxy), 0.5);
        assert_eq!(<ProxyAnimation<f64> as Animation<f64>>::status(&proxy), AnimationStatus::Forward);
    }

    #[test]
    fn proxy_forwards_parent_notifications() {
        let parent = TestSource::new(0.0, AnimationStatus::Forward);
        let proxy = ProxyAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        let counter = Rc::new(Cell::new(0u32));
        let counter_in = Rc::clone(&counter);
        proxy.add_listener(Rc::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));
        parent.set_value(0.3);
        parent.set_value(0.6);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn proxy_set_parent_swaps_subscription_and_refires() {
        let p1 = TestSource::new(0.1, AnimationStatus::Forward);
        let p2 = TestSource::new(0.9, AnimationStatus::Reverse);
        let proxy = ProxyAnimation::new(p1.clone() as Rc<dyn Animation<f64>>);
        let counter = Rc::new(Cell::new(0u32));
        let counter_in = Rc::clone(&counter);
        proxy.add_listener(Rc::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));

        // Sanity: p1 fires.
        p1.set_value(0.2);
        assert_eq!(counter.get(), 1);

        // Swap to p2 — set_parent itself fires once.
        proxy.set_parent(p2.clone() as Rc<dyn Animation<f64>>);
        assert!(counter.get() >= 2);

        // Now p1's notifications must NOT propagate.
        let before = counter.get();
        p1.set_value(0.3);
        assert_eq!(
            counter.get(),
            before,
            "after swap, old parent must not propagate"
        );

        // p2 notifications DO propagate.
        let before = counter.get();
        p2.set_value(0.95);
        assert_eq!(counter.get(), before + 1);

        // value() reads from new parent.
        assert!(
            (<ProxyAnimation<f64> as Animation<f64>>::value(&proxy) - 0.95).abs() < 1e-9
        );
    }

    #[test]
    fn proxy_drop_releases_parent_listener() {
        let parent = TestSource::new(0.0, AnimationStatus::Forward);
        assert_eq!(parent.listeners.len(), 0);
        let proxy = ProxyAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        assert_eq!(parent.listeners.len(), 1);
        drop(proxy);
        assert_eq!(parent.listeners.len(), 0);
    }

    // ------------------------------------------------------------------
    // ReverseAnimation
    // ------------------------------------------------------------------

    #[test]
    fn reverse_inverts_value() {
        let parent = TestSource::new(0.3, AnimationStatus::Forward);
        let rev = ReverseAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        assert!((<ReverseAnimation as Animation<f64>>::value(&rev) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn reverse_flips_status() {
        let parent = TestSource::new(0.0, AnimationStatus::Forward);
        let rev = ReverseAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        assert_eq!(<ReverseAnimation as Animation<f64>>::status(&rev), AnimationStatus::Reverse);

        parent.set_status(AnimationStatus::Completed);
        assert_eq!(<ReverseAnimation as Animation<f64>>::status(&rev), AnimationStatus::Dismissed);

        parent.set_status(AnimationStatus::Reverse);
        assert_eq!(<ReverseAnimation as Animation<f64>>::status(&rev), AnimationStatus::Forward);

        parent.set_status(AnimationStatus::Dismissed);
        assert_eq!(<ReverseAnimation as Animation<f64>>::status(&rev), AnimationStatus::Completed);
    }

    #[test]
    fn reverse_status_listener_receives_flipped_status() {
        let parent = TestSource::new(0.0, AnimationStatus::Forward);
        let rev = ReverseAnimation::new(parent.clone() as Rc<dyn Animation<f64>>);
        let captured: Rc<Cell<Option<AnimationStatus>>> = Rc::new(Cell::new(None));
        let captured_in = Rc::clone(&captured);
        rev.add_status_listener(Rc::new(move |s| captured_in.set(Some(s))));

        parent.set_status(AnimationStatus::Forward);
        assert_eq!(captured.get(), Some(AnimationStatus::Reverse));

        parent.set_status(AnimationStatus::Completed);
        assert_eq!(captured.get(), Some(AnimationStatus::Dismissed));
    }

    // ------------------------------------------------------------------
    // CompoundAnimation / Min / Max / Mean
    // ------------------------------------------------------------------

    #[test]
    fn animation_min_returns_smaller() {
        let a = TestSource::new(0.3, AnimationStatus::Forward);
        let b = TestSource::new(0.7, AnimationStatus::Forward);
        let combined = animation_min(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        assert!((combined.value() - 0.3).abs() < 1e-9);
    }

    #[test]
    fn animation_max_returns_larger() {
        let a = TestSource::new(0.3, AnimationStatus::Forward);
        let b = TestSource::new(0.7, AnimationStatus::Forward);
        let combined = animation_max(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        assert!((combined.value() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn animation_mean_returns_average() {
        let a = TestSource::new(0.2, AnimationStatus::Forward);
        let b = TestSource::new(0.8, AnimationStatus::Forward);
        let combined = animation_mean(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        assert!((combined.value() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn compound_status_priority_forward_wins() {
        let a = TestSource::new(0.0, AnimationStatus::Reverse);
        let b = TestSource::new(0.0, AnimationStatus::Forward);
        let combined = animation_min(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        assert_eq!(combined.status(), AnimationStatus::Forward);
    }

    #[test]
    fn compound_status_both_dismissed() {
        let a = TestSource::new(0.0, AnimationStatus::Dismissed);
        let b = TestSource::new(0.0, AnimationStatus::Dismissed);
        let combined = animation_min(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        assert_eq!(combined.status(), AnimationStatus::Dismissed);
    }

    #[test]
    fn compound_value_listener_fires_on_either_parent() {
        let a = TestSource::new(0.0, AnimationStatus::Forward);
        let b = TestSource::new(0.0, AnimationStatus::Forward);
        let combined = animation_mean(
            a.clone() as Rc<dyn Animation<f64>>,
            b.clone() as Rc<dyn Animation<f64>>,
        );
        let counter = Rc::new(Cell::new(0u32));
        let counter_in = Rc::clone(&counter);
        combined.add_listener(Rc::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));

        a.set_value(0.5);
        b.set_value(0.5);
        assert_eq!(counter.get(), 2);
    }

    // ------------------------------------------------------------------
    // TrainHoppingAnimation
    // ------------------------------------------------------------------

    #[test]
    fn train_hopping_starts_with_first_value() {
        let first = TestSource::new(0.2, AnimationStatus::Forward);
        let second = TestSource::new(0.8, AnimationStatus::Forward);
        let train = TrainHoppingAnimation::new(
            first.clone() as Rc<dyn Animation<f64>>,
            second.clone() as Rc<dyn Animation<f64>>,
        );
        assert!((train.value() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn train_hopping_hops_when_first_crosses_second() {
        let first = TestSource::new(0.2, AnimationStatus::Forward); // first < second initially
        let second = TestSource::new(0.8, AnimationStatus::Forward);
        let train = TrainHoppingAnimation::new(
            first.clone() as Rc<dyn Animation<f64>>,
            second.clone() as Rc<dyn Animation<f64>>,
        );

        // Drive the first across the second's value (0.9 > 0.8 → sign flips).
        first.set_value(0.9);

        // After the hop, value() reads from second.
        assert!((train.value() - 0.8).abs() < 1e-9);

        // Further first updates should be ignored — value tracks second.
        first.set_value(0.5);
        assert!((train.value() - 0.8).abs() < 1e-9);

        // Second updates DO propagate.
        second.set_value(0.6);
        assert!((train.value() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn train_hopping_hop_releases_first_subscription() {
        let first = TestSource::new(0.2, AnimationStatus::Forward);
        let second = TestSource::new(0.8, AnimationStatus::Forward);
        // Sanity counts before the train animation latches on.
        assert_eq!(first.listeners.len(), 0);
        let train = TrainHoppingAnimation::new(
            first.clone() as Rc<dyn Animation<f64>>,
            second.clone() as Rc<dyn Animation<f64>>,
        );
        assert_eq!(first.listeners.len(), 1);

        // Hop: first crosses second (0.9 > 0.8).
        first.set_value(0.9);
        assert_eq!(first.listeners.len(), 0, "first parent listener must be released after hop");
        // Second parent still subscribed.
        assert_eq!(second.listeners.len(), 1);

        drop(train);
        // After drop, second parent's listener is also released.
        assert_eq!(second.listeners.len(), 0);
    }
}
