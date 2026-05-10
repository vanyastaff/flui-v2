//! Borrow cell for the single-threaded `App` runtime.
//!
//! `AppCell` is a single-mutable-borrow cell. Recursive mutable borrows produce
//! `ReentryError::AppBorrowed` via the K15 contract. Use `cx.defer` or
//! `Window::defer` to schedule work that must touch `App` after the current
//! callback returns.
//!
//! Dropping a borrow guard during panic releases the borrow flag, but it does
//! not undo partial mutations to `App`. That matches the previous runtime-cell
//! behavior: after a panicking closure, the runtime is in a best-effort
//! consistent state and the borrow machinery itself is not permanently poisoned.

use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    num::NonZeroU32,
    ops::{Deref, DerefMut},
};

use crate::{App, reentrancy::ReentryError};

const LOG_TARGET: &str = "flui_core::app::cell";

/// A borrow-tracking cell for the flui-core [`App`].
#[doc(hidden)]
pub struct AppCell {
    pub(super) app: UnsafeCell<App>,
    pub(super) borrowed: Cell<BorrowState>,
    pub(super) _not_send: PhantomData<*const ()>,
}

/// Current borrow state for [`AppCell`].
///
/// Transitions:
/// - `Free -> Shared(1)` when `try_borrow` succeeds.
/// - `Shared(n) -> Shared(n + 1)` when another shared borrow succeeds.
/// - `Shared(1) -> Free` when the last shared guard drops.
/// - `Free -> Mut` when `try_borrow_mut` succeeds.
/// - `Mut -> Free` when the mutable guard drops.
///
/// `Shared(u32::MAX)` is treated as contention and returns
/// `ReentryError::AppBorrowed` instead of overflowing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BorrowState {
    Free,
    Mut,
    Shared(NonZeroU32),
}

impl AppCell {
    /// Borrows the app immutably, panicking with a typed [`ReentryError`] on
    /// contention.
    #[doc(hidden)]
    #[track_caller]
    pub fn borrow(&self) -> AppRef<'_> {
        match self.try_borrow() {
            Ok(app) => app,
            Err(err) => std::panic::panic_any(err),
        }
    }

    /// Attempts to borrow the app immutably.
    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow(&self) -> Result<AppRef<'_>, ReentryError> {
        match self.borrowed.get() {
            BorrowState::Free => {
                self.borrowed
                    .set(BorrowState::Shared(NonZeroU32::new(1).unwrap()));
            }
            BorrowState::Shared(count) => {
                let Some(next) = count.get().checked_add(1).and_then(NonZeroU32::new) else {
                    log::warn!(
                        target: LOG_TARGET,
                        "borrow denied at {}: shared borrow count saturated",
                        std::panic::Location::caller(),
                    );
                    return Err(ReentryError::AppBorrowed);
                };
                self.borrowed.set(BorrowState::Shared(next));
            }
            BorrowState::Mut => {
                log::warn!(
                    target: LOG_TARGET,
                    "borrow denied at {}: app is mutably borrowed",
                    std::panic::Location::caller(),
                );
                return Err(ReentryError::AppBorrowed);
            }
        }

        // SAFETY: the borrow flag has been transitioned to `Shared(_)`, so no
        // mutable reference can coexist with this shared reference. The
        // reference is created from the single `cell.app.get()` root and is
        // stored inside `AppRef`, whose `Drop` releases one shared borrow before
        // the cell can be borrowed mutably again. This mirrors the RefCell
        // proof shape: dynamic state guards the projection's lifetime.
        let app = unsafe { &*self.app.get() };
        let guard = AppRef { app, cell: self };

        log::trace!(
            target: LOG_TARGET,
            "borrow acquired at {}",
            std::panic::Location::caller(),
        );

        Ok(guard)
    }

    /// Borrows the app mutably, panicking with a typed [`ReentryError`] on
    /// contention.
    #[doc(hidden)]
    #[track_caller]
    pub fn borrow_mut(&self) -> AppRefMut<'_> {
        match self.try_borrow_mut() {
            Ok(app) => app,
            Err(err) => std::panic::panic_any(err),
        }
    }

    /// Attempts to borrow the app mutably.
    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow_mut(&self) -> Result<AppRefMut<'_>, ReentryError> {
        match self.borrowed.get() {
            BorrowState::Free => self.borrowed.set(BorrowState::Mut),
            BorrowState::Mut | BorrowState::Shared(_) => {
                log::warn!(
                    target: LOG_TARGET,
                    "borrow_mut denied at {}: app is already borrowed",
                    std::panic::Location::caller(),
                );
                return Err(ReentryError::AppBorrowed);
            }
        }

        // SAFETY: the borrow flag has been transitioned to `Mut`, so neither a
        // shared nor another mutable reference can coexist with this projection.
        // The mutable reference is created from the single `cell.app.get()`
        // root and is stored inside `AppRefMut`, whose `Drop` resets the state
        // to `Free` before any later borrow can succeed.
        let app = unsafe { &mut *self.app.get() };
        let guard = AppRefMut { app, cell: self };

        log::trace!(
            target: LOG_TARGET,
            "borrow_mut acquired at {}",
            std::panic::Location::caller(),
        );

        Ok(guard)
    }
}

/// Immutable app borrow guard.
#[doc(hidden)]
pub struct AppRef<'a> {
    app: &'a App,
    cell: &'a AppCell,
}

impl Deref for AppRef<'_> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
    }
}

impl Drop for AppRef<'_> {
    fn drop(&mut self) {
        match self.cell.borrowed.get() {
            BorrowState::Shared(count) if count.get() > 1 => {
                self.cell.borrowed.set(BorrowState::Shared(
                    NonZeroU32::new(count.get() - 1).unwrap(),
                ));
            }
            BorrowState::Shared(_) => self.cell.borrowed.set(BorrowState::Free),
            other => {
                log::warn!(
                    target: LOG_TARGET,
                    "borrow dropped while state was {other:?}",
                );
                #[cfg(debug_assertions)]
                if !std::thread::panicking() {
                    unreachable!("AppRef dropped while borrow state was {other:?}");
                }
                self.cell.borrowed.set(BorrowState::Free);
            }
        }

        log::trace!(target: LOG_TARGET, "borrow dropped");
    }
}

/// Mutable app borrow guard.
#[doc(hidden)]
pub struct AppRefMut<'a> {
    app: &'a mut App,
    cell: &'a AppCell,
}

impl Deref for AppRefMut<'_> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
    }
}

impl DerefMut for AppRefMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.app
    }
}

impl Drop for AppRefMut<'_> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if !std::thread::panicking() {
            debug_assert!(
                matches!(self.cell.borrowed.get(), BorrowState::Mut),
                "AppRefMut dropped while borrow state was {:?}",
                self.cell.borrowed.get(),
            );
        }

        self.cell.borrowed.set(BorrowState::Free);
        log::trace!(target: LOG_TARGET, "borrow_mut dropped");
    }
}

#[cfg(test)]
mod tests;
