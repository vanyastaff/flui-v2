// crates/flui-core/src/animation/listeners.rs
//
// S21 phase 0: listener-storage mixins that animation types embed to satisfy
// the [`Animation<T>`](crate::animation::Animation) trait's listener methods.
// Mirrors Flutter's `AnimationLocalListenersMixin`,
// `AnimationLocalStatusListenersMixin`, `AnimationLazyListenerMixin`,
// `AnimationEagerListenerMixin`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::cell::RefCell;

use smallvec::SmallVec;

use crate::animation::animation::{ListenerCallback, ListenerId, StatusListenerCallback};
use crate::animation::status::AnimationStatus;

// ============================================================================
// LocalListeners — value-change listener storage
// ============================================================================

/// Storage helper for value-change listeners. Embedded in animation types
/// (controllers, combinators) to satisfy
/// [`Animation::add_listener`](crate::animation::Animation::add_listener) /
/// [`Animation::remove_listener`].
///
/// **Flutter parity:** corresponds to `AnimationLocalListenersMixin`.
///
/// # Re-entrancy semantics
///
/// [`LocalListeners::notify`] snapshots the listener list (by cloning
/// [`Rc`] handles, cheap) before iterating. If a callback adds a listener,
/// the new listener is **not** invoked in the current dispatch but will be
/// invoked on the next. If a callback removes a listener that has not yet
/// fired in the current dispatch, it is **skipped** — matching Flutter's
/// `_listeners.contains(listener)` guard inside `notifyListeners`.
#[derive(Default)]
pub struct LocalListeners {
    inner: RefCell<SmallVec<[(ListenerId, ListenerCallback); 4]>>,
}

impl LocalListeners {
    /// Create an empty listener set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a callback, returning its opaque ID for later removal.
    ///
    /// The first attach (transition from empty to non-empty) is the natural
    /// point at which a [`LazyListenable`] implementor would start its
    /// underlying ticker; consult [`LocalListeners::is_empty`] before pushing.
    pub fn add(&self, listener: ListenerCallback) -> ListenerId {
        let id = ListenerId::next();
        self.inner.borrow_mut().push((id, listener));
        log::trace!(
            target: "flui_core::animation::listeners",
            "LocalListeners::add id={:?} count={}",
            id,
            self.inner.borrow().len()
        );
        id
    }

    /// Remove a previously-registered callback. No-op if the ID is unknown.
    ///
    /// The last detach (transition to empty) is the natural point at which a
    /// [`LazyListenable`] implementor would stop its ticker.
    pub fn remove(&self, id: ListenerId) {
        let mut vec = self.inner.borrow_mut();
        if let Some(pos) = vec.iter().position(|(stored, _)| *stored == id) {
            vec.remove(pos);
            log::trace!(
                target: "flui_core::animation::listeners",
                "LocalListeners::remove id={:?} count={}",
                id,
                vec.len()
            );
        }
    }

    /// Notify every registered listener. See module-level docs for re-entrancy
    /// guarantees.
    pub fn notify(&self) {
        // Cheap snapshot: clone the listener handles. `ListenerCallback` is
        // a newtype around `Rc<dyn Fn>` — `Clone` delegates to `Rc::clone`.
        let snapshot: SmallVec<[(ListenerId, ListenerCallback); 4]> = self
            .inner
            .borrow()
            .iter()
            .map(|(id, cb)| (*id, cb.clone()))
            .collect();
        for (id, cb) in &snapshot {
            // Guard against listeners removed by a previous callback in this
            // very dispatch — match Flutter's `_listeners.contains(...)` check.
            let still_present = self.inner.borrow().iter().any(|(stored, _)| stored == id);
            if still_present {
                (cb.0)();
            }
        }
    }

    /// Whether any listener is registered.
    #[allow(dead_code)] // used by tests + future LazyListenable consumers
    pub fn is_empty(&self) -> bool {
        self.inner.borrow().is_empty()
    }

    /// Number of registered listeners.
    #[allow(dead_code)] // used by tests + future LazyListenable consumers
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }
}

// ============================================================================
// LocalStatusListeners — status-change listener storage
// ============================================================================

/// Storage helper for status-change listeners. Embedded alongside
/// [`LocalListeners`] in animation types that expose
/// [`Animation::add_status_listener`](crate::animation::Animation::add_status_listener).
///
/// **Flutter parity:** corresponds to `AnimationLocalStatusListenersMixin`.
///
/// Re-entrancy semantics match [`LocalListeners`].
#[derive(Default)]
pub struct LocalStatusListeners {
    inner: RefCell<SmallVec<[(ListenerId, StatusListenerCallback); 4]>>,
}

impl LocalStatusListeners {
    /// Create an empty listener set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a status-change callback.
    pub fn add(&self, listener: StatusListenerCallback) -> ListenerId {
        let id = ListenerId::next();
        self.inner.borrow_mut().push((id, listener));
        log::trace!(
            target: "flui_core::animation::listeners",
            "LocalStatusListeners::add id={:?} count={}",
            id,
            self.inner.borrow().len()
        );
        id
    }

    /// Remove a previously-registered status callback. No-op if the ID is
    /// unknown.
    pub fn remove(&self, id: ListenerId) {
        let mut vec = self.inner.borrow_mut();
        if let Some(pos) = vec.iter().position(|(stored, _)| *stored == id) {
            vec.remove(pos);
            log::trace!(
                target: "flui_core::animation::listeners",
                "LocalStatusListeners::remove id={:?} count={}",
                id,
                vec.len()
            );
        }
    }

    /// Notify every registered status listener with the given status.
    pub fn notify(&self, status: AnimationStatus) {
        let snapshot: SmallVec<[(ListenerId, StatusListenerCallback); 4]> = self
            .inner
            .borrow()
            .iter()
            .map(|(id, cb)| (*id, cb.clone()))
            .collect();
        for (id, cb) in &snapshot {
            let still_present = self.inner.borrow().iter().any(|(stored, _)| stored == id);
            if still_present {
                (cb.0)(status);
            }
        }
    }

    /// Whether any status listener is registered.
    #[allow(dead_code)] // used by tests + future LazyListenable consumers
    pub fn is_empty(&self) -> bool {
        self.inner.borrow().is_empty()
    }

    /// Number of registered status listeners.
    #[allow(dead_code)] // used by tests + future LazyListenable consumers
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }
}

// ============================================================================
// LazyListenable — lazy ticker hooks
// ============================================================================

/// Hooks fired when listener count crosses the empty/non-empty boundary.
///
/// Animation types that drive an underlying ticker (notably
/// [`AnimationController`](crate::animation::AnimationController)) implement
/// this to start ticking when the first listener attaches and stop when the
/// last detaches — matches Flutter's `AnimationLazyListenerMixin`.
///
/// Phase 0 ships the trait. Phase 0 task 0.6 wires the production
/// [`Ticker`](crate::animation::Ticker) to the hooks; phase 0 task 0.7 wires
/// `AnimationController` itself.
///
/// **Sealed:** external crates cannot implement this trait — the seal
/// supertrait is `crate::seal::Sealed`. The hooks only make sense for animation
/// types that own a `LocalListeners` instance and a `Ticker`, both of which
/// are crate-internal. If a future external use case appears, the trait can
/// be unsealed via a follow-up commit.
#[allow(dead_code)] // future-proofing for S21-followup widget-layer integration
pub trait LazyListenable: crate::seal::Sealed {
    /// Called when the first listener attaches to a previously-empty notifier.
    /// Implementors typically use this to start their ticker.
    fn did_register_listener(&self);

    /// Called when the last listener detaches (notifier transitions back to
    /// empty). Implementors typically stop their ticker here.
    fn did_unregister_listener(&self);
}

// ============================================================================
// EagerListenable — explicit disposal hooks
// ============================================================================

/// Hooks fired on explicit disposal.
///
/// Animation types that own non-RAII resources (manual subscriptions,
/// timers wired through callbacks, retained Entity handles) implement this
/// to release them in `dispose()` — matches Flutter's
/// `AnimationEagerListenerMixin`.
///
/// Implementors must guarantee `dispose` is idempotent — calling it twice
/// is a no-op on the second call. This is a stronger contract than Flutter's
/// (which assumes single-call) but it is cheap to honour and prevents
/// double-free panics from accidentally-shared dispose paths.
///
/// **Sealed:** see the note on [`LazyListenable`].
#[allow(dead_code)] // future-proofing for S21-followup widget-layer integration
pub trait EagerListenable: crate::seal::Sealed {
    /// Release any non-RAII resources. Must be idempotent.
    fn dispose(&mut self);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn empty_listeners_default_state() {
        let listeners = LocalListeners::new();
        assert!(listeners.is_empty());
        assert_eq!(listeners.len(), 0);
    }

    #[test]
    fn add_and_remove_round_trip() {
        let listeners = LocalListeners::new();
        let counter = Rc::new(Cell::new(0));
        let counter_in = Rc::clone(&counter);

        let id = listeners.add(ListenerCallback::new(move || {
            counter_in.set(counter_in.get() + 1);
        }));
        assert_eq!(listeners.len(), 1);

        listeners.notify();
        assert_eq!(counter.get(), 1);

        listeners.remove(id);
        assert!(listeners.is_empty());

        listeners.notify();
        assert_eq!(counter.get(), 1, "removed listener must not fire");
    }

    #[test]
    fn remove_unknown_id_is_no_op() {
        let listeners = LocalListeners::new();
        let id = listeners.add(ListenerCallback::new(|| {}));
        listeners.remove(id);
        // Removing a stale ID a second time must not panic / double-remove.
        listeners.remove(id);
        assert!(listeners.is_empty());
    }

    #[test]
    fn reentrant_add_during_notify_fires_only_on_next_dispatch() {
        let listeners = Rc::new(LocalListeners::new());
        let outer_count = Rc::new(Cell::new(0));
        let inner_count = Rc::new(Cell::new(0));

        let listeners_in = Rc::clone(&listeners);
        let inner_count_for_outer = Rc::clone(&inner_count);
        let outer_count_in = Rc::clone(&outer_count);
        listeners.add(ListenerCallback::new(move || {
            outer_count_in.set(outer_count_in.get() + 1);
            // Re-entrant add — must NOT fire in the current dispatch.
            let inner_count_inner = Rc::clone(&inner_count_for_outer);
            listeners_in.add(ListenerCallback::new(move || {
                inner_count_inner.set(inner_count_inner.get() + 1);
            }));
        }));

        listeners.notify();
        assert_eq!(outer_count.get(), 1);
        assert_eq!(inner_count.get(), 0, "newcomer must not fire mid-dispatch");

        // Second dispatch: outer + inner both fire.
        listeners.notify();
        assert_eq!(outer_count.get(), 2);
        assert_eq!(inner_count.get(), 1);
    }

    #[test]
    fn reentrant_remove_during_notify_skips_target() {
        let listeners = Rc::new(LocalListeners::new());
        let a_count = Rc::new(Cell::new(0));
        let b_count = Rc::new(Cell::new(0));

        // Listener A removes listener B during dispatch.
        let id_b: Rc<Cell<Option<ListenerId>>> = Rc::new(Cell::new(None));
        let listeners_in = Rc::clone(&listeners);
        let a_count_in = Rc::clone(&a_count);
        let id_b_for_a = Rc::clone(&id_b);
        listeners.add(ListenerCallback::new(move || {
            a_count_in.set(a_count_in.get() + 1);
            if let Some(id) = id_b_for_a.get() {
                listeners_in.remove(id);
            }
        }));

        let b_count_in = Rc::clone(&b_count);
        let id = listeners.add(ListenerCallback::new(move || {
            b_count_in.set(b_count_in.get() + 1);
        }));
        id_b.set(Some(id));

        listeners.notify();
        assert_eq!(a_count.get(), 1);
        assert_eq!(b_count.get(), 0, "B must be skipped after A removes it");
    }

    #[test]
    fn status_listeners_round_trip() {
        let listeners = LocalStatusListeners::new();
        let captured: Rc<Cell<Option<AnimationStatus>>> = Rc::new(Cell::new(None));
        let captured_in = Rc::clone(&captured);
        let id = listeners.add(StatusListenerCallback::new(move |status| {
            captured_in.set(Some(status));
        }));

        listeners.notify(AnimationStatus::Forward);
        assert_eq!(captured.get(), Some(AnimationStatus::Forward));

        listeners.notify(AnimationStatus::Completed);
        assert_eq!(captured.get(), Some(AnimationStatus::Completed));

        listeners.remove(id);
        listeners.notify(AnimationStatus::Reverse);
        assert_eq!(
            captured.get(),
            Some(AnimationStatus::Completed),
            "removed status listener must not fire"
        );
    }

    #[test]
    fn listener_ids_are_unique_and_non_zero() {
        let id1 = ListenerId::next();
        let id2 = ListenerId::next();
        assert_ne!(id1, id2);
    }
}
