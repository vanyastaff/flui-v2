// crates/flui-core/src/animation/animation.rs
//
// S21 phase 0: the Flutter-parity `Animation<T>` trait. Foundation for every
// concrete animation type (`AnimationController`, `CurvedAnimation`,
// `ProxyAnimation`, etc.) that lands in subsequent phases.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::animation::status::AnimationStatus;

/// An opaque listener identifier returned by [`Animation::add_listener`] /
/// [`Animation::add_status_listener`] and accepted by the matching `remove_*`
/// calls.
///
/// **Deviation from Flutter parity:** Flutter's `ChangeNotifier` keys
/// listeners by `VoidCallback` identity. Rust closures have no equality, so
/// this trait returns an opaque ID instead. Callers must remember the ID if
/// they want to detach a specific listener.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ListenerId(NonZeroU64);

impl ListenerId {
    /// Allocate a fresh, monotonically-increasing ID. Internal-only — animation
    /// types call this through their listener-storage mixins.
    pub(crate) fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let raw = COUNTER.fetch_add(1, Ordering::Relaxed);
        // SAFETY: COUNTER starts at 1 and only increments. Reaching u64::MAX
        // takes ~585 years at 1 billion IDs/second; we panic if we ever do.
        Self(NonZeroU64::new(raw).expect("ListenerId counter overflowed u64"))
    }
}

/// A value-change listener callback.
///
/// Internally stores `Rc<dyn Fn>` so [`LocalListeners::notify`] can clone
/// references into a snapshot before dispatching — the snapshot is what makes
/// re-entrant `add_listener` / `remove_listener` from within a callback safe.
/// The inner `Rc` is `pub(crate)` to keep the storage strategy out of the
/// public contract — future versions can switch to a different handle type
/// (e.g. `Arc` or a custom slab-allocated handle) without breaking callers.
/// External code constructs callbacks via [`ListenerCallback::new`].
#[derive(Clone)]
pub struct ListenerCallback(pub(crate) Rc<dyn Fn() + 'static>);

impl ListenerCallback {
    /// Wrap a closure as a listener callback.
    pub fn new<F: Fn() + 'static>(f: F) -> Self {
        Self(Rc::new(f))
    }
}

/// A status-change listener callback. Same shape as [`ListenerCallback`] but
/// receives the new [`AnimationStatus`] when invoked.
#[derive(Clone)]
pub struct StatusListenerCallback(pub(crate) Rc<dyn Fn(AnimationStatus) + 'static>);

impl StatusListenerCallback {
    /// Wrap a closure as a status-change listener callback.
    pub fn new<F: Fn(AnimationStatus) + 'static>(f: F) -> Self {
        Self(Rc::new(f))
    }
}

/// A value of type `T` that may change over time, paired with an observable
/// [`AnimationStatus`].
///
/// **Flutter parity:** corresponds to
/// [`Animation<T>`](https://api.flutter.dev/flutter/animation/Animation-class.html).
///
/// # Listener model
///
/// Implementors typically embed a [`LocalListeners`](crate::animation::LocalListeners)
/// (and [`LocalStatusListeners`](crate::animation::LocalStatusListeners))
/// mixin to satisfy the listener methods. The expected semantics, matching
/// Flutter:
///
/// - Listeners fire **after** the value/status has updated.
/// - The list is snapshotted before dispatch. Adding or removing listeners
///   from inside a callback does **not** affect the current iteration but
///   does take effect on subsequent dispatches.
/// - Listeners are not `Send`. Animations are single-threaded UI primitives;
///   crossing threads with an animation handle is undefined.
///
/// # Object safety
///
/// `dyn Animation<T>` is object-safe; downstream combinators (CurvedAnimation,
/// ProxyAnimation, etc.) store parents as `Rc<dyn Animation<f64>>`.
///
/// The compile-time check at the bottom of this module pins the property —
/// any future trait extension that breaks object safety will fail to compile.
pub trait Animation<T>: 'static {
    /// The current value of the animation.
    fn value(&self) -> T;

    /// The current status of the animation.
    fn status(&self) -> AnimationStatus;

    /// Subscribe to value-change notifications.
    ///
    /// Returns an opaque [`ListenerId`] that the caller must keep if it wants
    /// to detach the listener via [`Animation::remove_listener`]. Dropping the
    /// ID without calling `remove_listener` simply leaves the listener
    /// registered — no leak in the GC sense, but the callback will keep firing
    /// for the lifetime of `self`.
    fn add_listener(&self, listener: ListenerCallback) -> ListenerId;

    /// Unsubscribe a previously-registered value listener. No-op if the ID is
    /// unknown (already removed, or from a different animation).
    fn remove_listener(&self, id: ListenerId);

    /// Subscribe to status-change notifications. Same ownership semantics as
    /// [`Animation::add_listener`].
    fn add_status_listener(&self, listener: StatusListenerCallback) -> ListenerId;

    /// Unsubscribe a previously-registered status listener.
    fn remove_status_listener(&self, id: ListenerId);

    /// Whether the status is [`AnimationStatus::Dismissed`].
    fn is_dismissed(&self) -> bool {
        matches!(self.status(), AnimationStatus::Dismissed)
    }

    /// Whether the status is [`AnimationStatus::Completed`].
    fn is_completed(&self) -> bool {
        matches!(self.status(), AnimationStatus::Completed)
    }

    /// Whether the status is [`AnimationStatus::Forward`] or
    /// [`AnimationStatus::Completed`] — convenient for "is the animation
    /// pointing at its upper bound right now."
    fn is_forward_or_completed(&self) -> bool {
        matches!(
            self.status(),
            AnimationStatus::Forward | AnimationStatus::Completed
        )
    }
}

// ============================================================================
// Compile-time object-safety check
// ============================================================================
//
// Phase 0 establishes the convention. Any future change to the `Animation`
// trait that breaks object safety (e.g. an associated type, a `Self`-bound
// method without `where Self: Sized`) will fail this check at compile time.
//
// Subsequent phases that extend the trait MUST keep this check passing.

#[doc(hidden)]
fn _object_safe(_: &dyn Animation<f64>) {}

#[doc(hidden)]
fn _object_safe_boxed(_: Box<dyn Animation<f64>>) {}

#[doc(hidden)]
fn _object_safe_rc(_: Rc<dyn Animation<f64>>) {}
