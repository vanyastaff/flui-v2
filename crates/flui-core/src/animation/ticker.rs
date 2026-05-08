// crates/flui-core/src/animation/ticker.rs
//
// S21 phase 0 task 0.6: clock-driven elapsed-time source for animations.
// Decouples animation timing from `std::time::Instant::now()` so production
// code uses the platform's `RealClock` and tests inject `TestClock`. This is
// the substrate that unblocks deterministic golden tests for animation
// outputs (T6 in the roadmap).

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::scheduler::{Clock, Instant};

// ============================================================================
// Ticker
// ============================================================================

/// A clock-driven elapsed-time source for animations.
///
/// **Flutter parity:** corresponds to `Ticker` from
/// `package:flutter/scheduler.dart`, minus the per-frame callback. Our
/// per-frame re-arm lives in `animated()` (which has `Window` access);
/// the Ticker handles only the timing/clock concern.
///
/// # Construction
///
/// Construct directly with [`Ticker::new`] from any
/// `Arc<dyn `[`Clock`](crate::scheduler::Clock)`>`. The canonical clock
/// source is the active scheduler's clock, reachable from a
/// `Context<V>` / `App` via `cx.background_executor().scheduler().clock()`.
///
/// # Lifecycle
///
/// A Ticker is created idle. [`Ticker::start`] flips it to active, recording
/// the clock time at the start. [`Ticker::stop`] flips it back to idle and
/// settles the [`TickerFuture`] returned by `start`.
///
/// `start` may be called repeatedly — each call cancels the previous future
/// (settling it as [`TickerCanceled`]) and resets the start timestamp.
pub struct Ticker {
    clock: Arc<dyn Clock>,
    started_at: Cell<Option<Instant>>,
    active_future: RefCell<Option<Rc<Cell<TickerFutureState>>>>,
}

impl Ticker {
    /// Create a new idle ticker bound to `clock`.
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            started_at: Cell::new(None),
            active_future: RefCell::new(None),
        }
    }

    /// Mark the ticker as active starting at the current clock time.
    ///
    /// Returns a [`TickerFuture`] whose state will resolve to
    /// [`TickerFutureState::Completed`] when [`Ticker::stop`] is called with
    /// `canceled = false`, or [`TickerFutureState::Canceled`] otherwise.
    ///
    /// If a previous start is still pending, that future is canceled before
    /// the new one begins — matches Flutter's "only one active TickerFuture
    /// per Ticker" invariant.
    pub fn start(&self) -> TickerFuture {
        if let Some(prev) = self.active_future.borrow_mut().take() {
            prev.set(TickerFutureState::Canceled);
        }
        let state = Rc::new(Cell::new(TickerFutureState::Pending));
        *self.active_future.borrow_mut() = Some(Rc::clone(&state));

        let now = self.clock.now();
        self.started_at.set(Some(now));

        log::trace!(
            target: "flui_core::animation::ticker",
            "Ticker::start (clock now = {:?})",
            now
        );
        TickerFuture { state }
    }

    /// Stop the ticker. The active [`TickerFuture`] is settled —
    /// [`TickerFutureState::Completed`] when `canceled` is false,
    /// [`TickerFutureState::Canceled`] otherwise. Calling `stop` on an
    /// already-idle ticker is a no-op.
    pub fn stop(&self, canceled: bool) {
        let was_active = self.started_at.get().is_some();
        self.started_at.set(None);
        if let Some(fut) = self.active_future.borrow_mut().take() {
            fut.set(if canceled {
                TickerFutureState::Canceled
            } else {
                TickerFutureState::Completed
            });
        }
        if was_active {
            log::trace!(
                target: "flui_core::animation::ticker",
                "Ticker::stop canceled={}",
                canceled
            );
        }
    }

    /// Whether the ticker is currently active.
    pub fn is_active(&self) -> bool {
        self.started_at.get().is_some()
    }

    /// Time elapsed since the most recent [`Ticker::start`], or
    /// [`Duration::ZERO`] if not active. Uses
    /// [`Instant::saturating_duration_since`] so a clock that briefly goes
    /// backwards (e.g. NTP correction) does not panic.
    pub fn elapsed(&self) -> Duration {
        match self.started_at.get() {
            Some(start) => self.clock.now().saturating_duration_since(start),
            None => Duration::ZERO,
        }
    }

    /// Current clock time. Use this when an animation type needs a snapshot
    /// time-origin (e.g. `AnimationController.start_time = Some(ticker.now())`)
    /// without taking on the ticker's own start/stop machinery.
    pub fn now(&self) -> Instant {
        self.clock.now()
    }
}

// ============================================================================
// TickerFuture
// ============================================================================

/// Future-shaped handle returned by [`Ticker::start`].
///
/// **Phase 0 caveat:** this type does **not** yet implement [`std::future::Future`].
/// It exposes synchronous status accessors (`is_pending`/`is_completed`/
/// `is_canceled`) so callers can poll state imperatively. A proper `Future`
/// impl is deferred until a concrete consumer needs `await` semantics
/// (route transitions in S21 phase 4 / S22 hero transitions). This keeps
/// phase 0 free of `Waker` plumbing without locking the API down to the wrong
/// shape.
pub struct TickerFuture {
    state: Rc<Cell<TickerFutureState>>,
}

impl TickerFuture {
    /// Whether the future is still pending (the ticker has not been stopped).
    pub fn is_pending(&self) -> bool {
        matches!(self.state.get(), TickerFutureState::Pending)
    }

    /// Whether the future settled with [`TickerFutureState::Completed`].
    pub fn is_completed(&self) -> bool {
        matches!(self.state.get(), TickerFutureState::Completed)
    }

    /// Whether the future settled with [`TickerFutureState::Canceled`].
    pub fn is_canceled(&self) -> bool {
        matches!(self.state.get(), TickerFutureState::Canceled)
    }
}

/// Settled state of a [`TickerFuture`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TickerFutureState {
    Pending,
    Completed,
    Canceled,
}

// ============================================================================
// TickerCanceled
// ============================================================================

/// Sentinel error returned (in phase 4+) when a [`TickerFuture`] is awaited
/// and the underlying ticker was stopped with `canceled = true`.
///
/// Phase 0 ships the type for parity with Flutter's `TickerCanceled`; the
/// phase that wires `Future` semantics will use it as the error variant.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TickerCanceled;

impl std::fmt::Display for TickerCanceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ticker canceled")
    }
}

impl std::error::Error for TickerCanceled {}

// ============================================================================
// TickerProvider
// ============================================================================

/// Source of [`Ticker`]s. Phase 0 ships the trait but no implementation —
/// direct construction via [`Ticker::new`] is the canonical path. A future
/// phase (or `flui-widgets`) may implement this on `Context<V>` /
/// `Window` / view-state types as Flutter's `SingleTickerProviderStateMixin`
/// equivalent.
///
/// **`pub(crate)`** until the first concrete impl ships. Demoted in the S21
/// review-fix Tier 3 pass — exposing a contract before any consumer proves
/// the right shape risks locking in the wrong API. Promote to `pub` in the
/// commit that wires the first implementation.
#[allow(dead_code)] // future-proofing — first impl lands with widget-layer integration
pub(crate) trait TickerProvider {
    /// Hand out a fresh idle ticker.
    fn create_ticker(&self) -> Ticker;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::scheduler::TestClock;

    fn test_ticker() -> (Ticker, Arc<TestClock>) {
        let clock = Arc::new(TestClock::new());
        let ticker = Ticker::new(Arc::clone(&clock) as Arc<dyn Clock>);
        (ticker, clock)
    }

    #[test]
    fn idle_ticker_is_inactive() {
        let (ticker, _) = test_ticker();
        assert!(!ticker.is_active());
        assert_eq!(ticker.elapsed(), Duration::ZERO);
    }

    #[test]
    fn start_marks_active_and_resets_elapsed_to_zero() {
        let (ticker, _) = test_ticker();
        let _fut = ticker.start();
        assert!(ticker.is_active());
        assert_eq!(ticker.elapsed(), Duration::ZERO);
    }

    #[test]
    fn elapsed_advances_with_clock() {
        let (ticker, clock) = test_ticker();
        let _fut = ticker.start();
        clock.advance(Duration::from_millis(250));
        assert_eq!(ticker.elapsed(), Duration::from_millis(250));
    }

    #[test]
    fn stop_settles_future_completed() {
        let (ticker, _) = test_ticker();
        let fut = ticker.start();
        assert!(fut.is_pending());
        ticker.stop(false);
        assert!(fut.is_completed());
        assert!(!ticker.is_active());
        assert_eq!(ticker.elapsed(), Duration::ZERO);
    }

    #[test]
    fn stop_settles_future_canceled() {
        let (ticker, _) = test_ticker();
        let fut = ticker.start();
        ticker.stop(true);
        assert!(fut.is_canceled());
    }

    #[test]
    fn second_start_cancels_previous_future() {
        let (ticker, _) = test_ticker();
        let fut1 = ticker.start();
        let fut2 = ticker.start();
        assert!(fut1.is_canceled(), "previous future canceled by new start");
        assert!(fut2.is_pending());
    }

    #[test]
    fn now_delegates_to_clock() {
        let (ticker, clock) = test_ticker();
        let t0 = ticker.now();
        clock.advance(Duration::from_millis(100));
        let t1 = ticker.now();
        assert!(t1 > t0);
        assert_eq!(t1.saturating_duration_since(t0), Duration::from_millis(100));
    }

    #[test]
    fn ticker_canceled_error_displays() {
        let err = TickerCanceled;
        assert_eq!(err.to_string(), "ticker canceled");
    }
}
