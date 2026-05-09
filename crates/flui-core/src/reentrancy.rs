//! Re-entrancy contract for the flui-core runtime.
//!
//! This module defines and enforces what is allowed when a callback dispatched
//! by the runtime calls back into the runtime — `update_window` from inside an
//! observer, `update_entity` from inside `did_update_widget`, recursive
//! `with_element_state`, double `prompt`, and so on. Spec: K15 (Phase 0-K
//! Kernel Cleanup, critical chain). Authoritative design document:
//! `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md`.
//!
//! # The contract
//!
//! Every callback class falls into one of three buckets:
//!
//! - **Synchronous** — re-entry into App / Window / Entity for a *different*
//!   target is allowed. Examples: `cx.update_window(other_window, ...)` from
//!   inside another `update_window`, observer callbacks reading state from
//!   sibling entities.
//! - **Forbidden** — re-entry for the *same* target produces a [`ReentryError`].
//!   Never a bare `RefCell::borrow_mut` panic, never an unstructured message
//!   from `EntityMap::double_lease_panic`. Examples: `cx.update_window(self, ...)`
//!   from inside `update_window` for the same window, recursive
//!   `with_element_state` for the same `(GlobalElementId, TypeId)` key, double
//!   `prompt`.
//! - **Queued** — work is admitted to the App effect queue and runs after the
//!   current update completes. The runtime does NOT auto-queue forbidden cases;
//!   queueing is exclusively via the user-visible escape hatches:
//!   - [`App::defer`](crate::App::defer) — schedule a closure for the next
//!     effect flush.
//!   - [`Window::defer`](crate::Window::defer) — same, with window context.
//!
//! # Per-callback class summary
//!
//! | Callback class | Same-target re-entry | Different-target | Notes |
//! |---|---|---|---|
//! | `cx.update_window(...)` | Forbidden ([`ReentryError::NestedWindowUpdate`]) | Synchronous | Detection via `App::window_update_stack.contains(&id)`. |
//! | `cx.update_entity(...)` | Forbidden ([`ReentryError::NestedEntityUpdate`]) | Synchronous | Detection via `App::currently_updating_entity == Some(id)` AND the unified `EntityMap::double_lease_panic` (which now uses [`ReentryError::NestedEntityUpdate`] Display). |
//! | Multi-entity cycle `A→B→A` | Forbidden ([`ReentryError::NestedEntityUpdate`]) | n/a | Caught by `EntityMap::double_lease_panic` because A's slot is empty when the inner re-entry attempts `lease`. Same Display as direct re-entry. |
//! | Observer / event-listener / release callback | Synchronous within callback | Synchronous | The internal `SubscriberSet::retain` snapshot pattern guarantees no concurrent mutation of the subscriber list. Nested same-target updates raise [`ReentryError`]; user is directed to `cx.defer`. |
//! | `observe_in` / `subscribe_in` | Forbidden (inner update returns `Err`) | Synchronous | The `.unwrap_or(false)` discard at the call site (`crate::app::context`) is preserved unchanged. The error itself is logged at `warn!` (Loose mode) or `error!` (Strict mode) inside `App::update_window_id` *before* the `Err` is returned to the discard site — so a re-entry inside `observe_in`/`subscribe_in` produces a `flui_core::reentrancy` log event, then is silently dropped at the closure boundary. Adding explicit `log::debug!` at the discard site is a follow-up if richer telemetry is needed. |
//! | `Window::with_element_state(...)` | Forbidden ([`ReentryError::ElementStateInUse`]) — panic shape | n/a | Replaces the bare `expect("reentrant…")` panic with a structured `ReentryError` Display message. |
//! | `Window::prompt(...)` | Forbidden ([`ReentryError::PromptInProgress`]) | n/a | `Window::prompt` widens to `Result<oneshot::Receiver<usize>, ReentryError>`. `AsyncWindowContext::prompt` widens to `Result<_, anyhow::Error>` and stops swallowing errors. |
//! | `cx.defer(...)` / `Window::defer(...)` | Queued (existing) | Queued | The escape hatches. Always allowed; never produce [`ReentryError`]. |
//! | Animation listener / Ticker tick | Synchronous within callback | Synchronous | Listener snapshot pattern (`animation/listeners.rs`). |
//! | Gesture recognizer event handler | Synchronous within recognizer | Synchronous | Recognizers have their own scoped `Rc<RefCell<...>>` (A7-audit-closed). |
//! | `AsyncApp::run_update` | Forbidden ([`ReentryError::AppBorrowed`]) | Synchronous | `try_borrow_mut()?` chain auto-converts via `From<BorrowMutError> for ReentryError`. |
//!
//! # Modes
//!
//! [`ReentryMode`] selects how re-entry is reported:
//!
//! - [`ReentryMode::Strict`] (default in `cfg(test)`) — `ReentryError` is logged
//!   at `error!` level. Suitable for tests so silent-pass bugs surface.
//! - [`ReentryMode::Loose`] (default in release) — `ReentryError` is logged at
//!   `warn!` level. The error is still produced; only the log level differs.
//!
//! Set the mode via [`App::set_reentry_mode`](crate::App::set_reentry_mode).
//!
//! # Logging
//!
//! All log events use the target `flui_core::reentrancy`. Set
//! `RUST_LOG=flui_core::reentrancy=trace` (or `=warn` / `=error`) to filter.
//!
//! # Known limitations (documented gaps — see design spec)
//!
//! 1. `AsyncApp` has 10+ direct `app.borrow_mut()` sites that remain
//!    unstructured. K07 (AppCell removal) redesigns this surface.
//! 2. `AsyncApp::as_mut` panics with `"Cannot as_mut with an async context"`
//!    — different panic class, not re-entry.
//! 3. `web` platform dispatcher re-entry exposure unverified.
//! 4. [`ReentryError::AppBorrowed`] does not carry a source location
//!    (`std::cell::BorrowMutError::location()` is nightly-only). Use
//!    `RUST_LOG=flui_core::reentrancy=warn` for callsite context via
//!    `#[track_caller]` on the `From` impl.

use std::any::TypeId;
use std::cell::BorrowMutError;

use thiserror::Error;

use crate::{EntityId, GlobalElementId, WindowId};

/// Structured error returned (or panicked with) when the runtime re-entrancy
/// contract is violated.
///
/// This enum is `#[non_exhaustive]`: K07 and follow-up Phase 0-K specs may
/// introduce additional variants without a major version bump. Match arms in
/// downstream code must include a wildcard arm.
///
/// `ReentryError: std::error::Error + Send + Sync + 'static`, so it converts
/// into [`anyhow::Error`] automatically via anyhow's blanket `From` impl —
/// there is no explicit `From<ReentryError> for anyhow::Error` here, and
/// adding one would conflict with the blanket.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ReentryError {
    /// `update_window` was called recursively for the same window. Use
    /// [`App::defer`](crate::App::defer) or [`Window::defer`](crate::Window::defer)
    /// to schedule the work for after the current update completes.
    #[error(
        "update_window called recursively for window {0:?}; use cx.defer or window.defer to schedule work"
    )]
    NestedWindowUpdate(WindowId),

    /// The entity is already leased; a recursive re-entry attempted to access
    /// it. Three concrete shapes funnel into this variant:
    ///
    /// 1. Direct: `A` calls `update_entity(A, ...)` from inside an outer
    ///    `update_entity(A, ...)`. Caught at `App::update_entity` before
    ///    `EntityMap::lease`.
    /// 2. Cycle: `A → B → A`. Caught by `EntityMap::double_lease_panic` at
    ///    the lease layer because `A`'s slot is empty during the inner
    ///    re-entry.
    /// 3. Read-while-leased: `read_entity(A)` while `update_entity(A, ...)`
    ///    is still on the stack. Also caught by `EntityMap::double_lease_panic`
    ///    via `EntityMap::read`.
    ///
    /// All three produce this same Display (the operation/type/lease vs
    /// read distinction is appended by `double_lease_panic` for diagnostic
    /// context but is not part of the structured contract).
    ///
    /// Use [`App::defer`](crate::App::defer) to schedule the work for after
    /// the current update completes.
    #[error(
        "entity {0:?} is already leased; recursive re-entry is forbidden — use cx.defer to schedule work"
    )]
    NestedEntityUpdate(EntityId),

    /// `Window::with_element_state` was called recursively for the same
    /// `(GlobalElementId, TypeId)` key. Element state is single-take — the
    /// `Option<S>` wrapping the state is empty during the recursive call.
    ///
    /// This variant is panic-shape (the runtime panics with this Display);
    /// the `with_element_state` API does not return `Result`.
    #[error("with_element_state called recursively for ({global_element_id:?}, {type_id:?})")]
    ElementStateInUse {
        /// The global element id whose state was already taken.
        global_element_id: GlobalElementId,
        /// The state type id (`TypeId::of::<S>()`).
        type_id: TypeId,
    },

    /// `Window::prompt` was called while another prompt is already awaiting
    /// the user's response. Wait for the previous prompt's `Receiver` to
    /// complete before opening a new prompt.
    #[error("prompt() called while another prompt is awaiting user response")]
    PromptInProgress,

    /// The `App` cell was already mutably borrowed when an async re-entry
    /// attempted to acquire it. Likely cause: a callback that has not yet
    /// returned called back into the runtime via [`AsyncApp`](crate::AsyncApp).
    ///
    /// Use [`App::defer`](crate::App::defer) to schedule the work for after
    /// the current update completes. This variant carries no source location
    /// in stable Rust because [`std::cell::BorrowMutError::location`] is
    /// nightly-only; the `#[track_caller]` annotation on the `From` impl gives
    /// the callsite context when logging is enabled.
    #[error("App was already mutably borrowed (callback re-entered the runtime; use cx.defer)")]
    AppBorrowed,
}

impl From<BorrowMutError> for ReentryError {
    #[track_caller]
    fn from(_: BorrowMutError) -> Self {
        Self::AppBorrowed
    }
}

/// Selects how the runtime reports re-entry contract violations.
///
/// This enum is `#[non_exhaustive]`: K07 (AppCell removal) may add a
/// `PanicLikeUpstream` compatibility variant under a feature flag; that
/// variant was deferred from K15 to keep the contract surface minimal.
///
/// Set the active mode via
/// [`App::set_reentry_mode`](crate::App::set_reentry_mode). The default is
/// [`ReentryMode::Loose`] in release builds and [`ReentryMode::Strict`] in
/// `cfg(test)` — the test default makes silent re-entry bugs in existing tests
/// surface as `error!` log events for easier diagnosis.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReentryMode {
    /// `ReentryError` is logged at `error!` level on the
    /// `flui_core::reentrancy` target. Default in `cfg(test)`.
    Strict,
    /// `ReentryError` is logged at `warn!` level on the
    /// `flui_core::reentrancy` target. Default in release.
    #[default]
    Loose,
}

/// Emit a `log` event for a re-entry admission, choosing the level from the
/// active [`ReentryMode`]. Always logs to target `flui_core::reentrancy`.
///
/// Used internally by `App::update_window_id` and `App::update_entity`
/// before the contract is enforced (return `Err` or panic). Note:
/// `EntityMap::double_lease_panic` does NOT call this helper — that path is
/// terminal (immediate `panic!`) and the panic Display itself carries the
/// `ReentryError::NestedEntityUpdate` text. Not part of the public surface.
pub(crate) fn log_reentry(mode: ReentryMode, err: &ReentryError) {
    match mode {
        ReentryMode::Strict => {
            log::error!(target: "flui_core::reentrancy", "{}", err);
        }
        ReentryMode::Loose => {
            log::warn!(target: "flui_core::reentrancy", "{}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reentry_error_is_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<ReentryError>();
    }

    #[test]
    fn reentry_error_display_format_matches_contract() {
        // Smoke check that the Display strings include the recommended escape
        // hatch — downstream tooling (logs, error reporting) may scan for
        // "cx.defer" / "window.defer" strings.
        let display = format!("{}", ReentryError::PromptInProgress);
        assert!(display.contains("prompt"));

        // Nested update messages must mention the queue escape hatch.
        // We use a dummy slotmap key via Default::default(); the exact ID
        // representation is not part of the stable contract.
        let win_err = format!("{}", ReentryError::NestedWindowUpdate(WindowId::default()));
        assert!(win_err.contains("cx.defer") || win_err.contains("window.defer"));

        let ent_err = format!("{}", ReentryError::NestedEntityUpdate(EntityId::default()));
        assert!(ent_err.contains("cx.defer"));
        // NestedEntityUpdate is operation-agnostic (used by both lease for
        // update AND read-while-leased) — its Display must NOT hard-code
        // "update_entity" anymore.
        assert!(
            ent_err.contains("already leased"),
            "Display should be operation-agnostic; got: {ent_err}"
        );
    }

    #[test]
    fn reentry_error_converts_into_anyhow() {
        // Confirm the anyhow blanket impl fires.
        let err: anyhow::Error = ReentryError::PromptInProgress.into();
        assert!(format!("{}", err).contains("prompt"));
    }

    #[test]
    fn borrow_mut_error_converts_to_app_borrowed() {
        use std::cell::RefCell;
        let cell = RefCell::new(0);
        let _outer = cell.borrow_mut();
        let inner_err = cell.try_borrow_mut().unwrap_err();
        let r: ReentryError = inner_err.into();
        assert!(matches!(r, ReentryError::AppBorrowed));
    }

    #[test]
    fn reentry_mode_default_is_loose() {
        assert_eq!(ReentryMode::default(), ReentryMode::Loose);
    }

    #[test]
    fn reentry_mode_is_copy() {
        let m = ReentryMode::Strict;
        let n = m;
        assert_eq!(m, n);
    }
}

// Behavioral integration tests use the in-crate `TestApp` harness (gated on
// `cfg(test)`). Tests for `update_window` re-entry, `with_element_state`
// re-entry, and `Window::prompt` re-entry require the visual-test harness
// (Window mocking, platform prompt mocking) which is out of scope for K15;
// those are deferred to K17 (Test harness simplification, audit-finding E)
// per the K15 design spec §"Known limitations".
#[cfg(test)]
mod behavioral_tests {
    use super::*;
    use crate::{AppContext, TestApp};

    /// Per-test counter entity used to trigger entity-side re-entry.
    struct Counter {
        count: u32,
    }

    #[test]
    fn set_reentry_mode_setter_round_trips() {
        let mut app = TestApp::new();

        // Default in cfg(test) is Strict.
        app.read(|cx| {
            assert_eq!(cx.reentry_mode, ReentryMode::Strict);
        });

        // Setter round-trips both ways.
        app.update(|cx| cx.set_reentry_mode(ReentryMode::Loose));
        app.read(|cx| {
            assert_eq!(cx.reentry_mode, ReentryMode::Loose);
        });

        app.update(|cx| cx.set_reentry_mode(ReentryMode::Strict));
        app.read(|cx| {
            assert_eq!(cx.reentry_mode, ReentryMode::Strict);
        });
    }

    /// Direct same-entity re-entry. Panic message must include the unified
    /// `ReentryError::NestedEntityUpdate` Display, NOT the legacy
    /// `"cannot update <T> while it is already being updated"` text.
    #[test]
    fn nested_update_entity_same_target_panics_with_structured_display() {
        let mut app = TestApp::new();
        let entity = app.new_entity(|_| Counter { count: 0 });

        // Capture a second handle for the recursive inner call.
        // `Entity<T>: Clone` (cheap Arc-bump under the hood).
        let entity_clone = entity.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.update_entity(&entity, |_counter, cx| {
                // Re-enter the SAME entity from within the update closure.
                // K15 contract: this must panic with structured Display.
                cx.update_entity(&entity_clone, |inner, _cx| {
                    inner.count += 1;
                });
            });
        }));

        let panic_payload = result.expect_err("nested update_entity must panic");
        let msg = panic_payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic_payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic payload>");

        assert!(
            msg.contains("already leased"),
            "panic message must use ReentryError::NestedEntityUpdate Display; got: {msg}"
        );
        assert!(
            msg.contains("cx.defer"),
            "panic message must point to cx.defer escape hatch; got: {msg}"
        );
    }

    /// Direct test of `EntityMap::double_lease_panic` unified message: the
    /// multi-entity-cycle case (`A → B → A`) routes through this panic and
    /// must produce the same structured Display as the App-level guard.
    #[test]
    fn entity_map_double_lease_uses_unified_reentry_display() {
        let mut app = TestApp::new();
        let entity_a = app.new_entity(|_| Counter { count: 0 });
        let entity_b = app.new_entity(|_| Counter { count: 0 });

        let entity_a_for_inner = entity_a.clone();

        // Cycle: outer update_entity(A) → inner update_entity(B) →
        // innermost update_entity(A). When the innermost re-entry attempts
        // to lease A, A's slot is empty (it's leased by the outer call).
        // EntityMap::double_lease_panic fires there with the unified
        // ReentryError::NestedEntityUpdate Display.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.update_entity(&entity_a, |_outer, cx| {
                cx.update_entity(&entity_b, |_middle, cx| {
                    cx.update_entity(&entity_a_for_inner, |_innermost, _cx| {
                        // Should never reach here.
                    });
                });
            });
        }));

        let panic_payload = result.expect_err("multi-entity cycle must panic");
        let msg = panic_payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic_payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic payload>");

        assert!(
            msg.contains("already leased"),
            "multi-entity-cycle panic must use unified ReentryError::NestedEntityUpdate Display; got: {msg}"
        );
        // Also verify the legacy text is NOT present.
        assert!(
            !msg.contains("while it is already being updated"),
            "legacy double_lease_panic message must be replaced by ReentryError; got: {msg}"
        );
        // The diagnostic context appended by `EntityMap::double_lease_panic`
        // (operation/type) should still be present so debug logs distinguish
        // `lease` from `read` paths.
        assert!(
            msg.contains("during update of type") || msg.contains("during read of type"),
            "operation/type context must be appended for diagnostic; got: {msg}"
        );
    }

    /// Different-target re-entry is allowed and runs synchronously.
    #[test]
    fn nested_update_entity_different_target_runs_synchronously() {
        let mut app = TestApp::new();
        let entity_a = app.new_entity(|_| Counter { count: 0 });
        let entity_b = app.new_entity(|_| Counter { count: 0 });

        let entity_b_for_inner = entity_b.clone();

        app.update_entity(&entity_a, |outer, cx| {
            outer.count = 100;
            // Different entity is fine.
            cx.update_entity(&entity_b_for_inner, |inner, _cx| {
                inner.count = 200;
            });
        });

        app.read_entity(&entity_a, |c, _| assert_eq!(c.count, 100));
        app.read_entity(&entity_b, |c, _| assert_eq!(c.count, 200));
    }

    /// `cx.defer` is the documented escape hatch and must NOT panic when
    /// scheduling work that would otherwise be a re-entry violation.
    #[test]
    fn cx_defer_avoids_reentry_panic() {
        use std::cell::Cell;
        use std::rc::Rc;

        let mut app = TestApp::new();
        let entity = app.new_entity(|_| Counter { count: 0 });
        let deferred_ran = Rc::new(Cell::new(false));

        let entity_clone = entity.clone();
        let deferred_ran_clone = deferred_ran.clone();

        app.update_entity(&entity, |_outer, cx| {
            // Instead of nested update_entity(self, ...), defer it.
            let entity_for_defer = entity_clone.clone();
            let ran = deferred_ran_clone.clone();
            cx.defer(move |cx| {
                cx.update_entity(&entity_for_defer, |c, _cx| {
                    c.count = 42;
                });
                ran.set(true);
            });
        });

        // After the outer update returns, deferred work runs in the next
        // effect flush. TestApp::update auto-flushes.
        app.update(|_cx| {});
        assert!(
            deferred_ran.get(),
            "deferred closure must run after outer update"
        );
        app.read_entity(&entity, |c, _| assert_eq!(c.count, 42));
    }
}
