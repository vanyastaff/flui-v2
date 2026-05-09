# K07 — AppCell removal (token-based borrow model)

**Status:** Design (revision 1 — pending adversarial-review revision-2 from Task 6).
**Phase:** 0-K (Kernel Cleanup) — third spec in the critical chain (K99 → K15 → **K07** → K05 → K01 → K02 → K03 → K04).
**Plan:** [`.ai-factory/plans/feature-K07-appcell-removal-token-borrow.md`](../../../.ai-factory/plans/feature-K07-appcell-removal-token-borrow.md) (rev 4).
**Type:** structural refactor of the App ownership primitive. Replaces `AppCell = RefCell<App>` (marked "Strongly consider removing after stabilization" in `app.rs:73-74`) with a hand-rolled, K15-aware borrow primitive. **HIGH-RISK** — touches every callback path, but signature-compatible by design.
**Cross-refs:** K99 spec (`2026-05-08-K99-msrv-bump-1.95-design.md`); K15 spec (`2026-05-09-K15-reentrancy-contract-design.md`); ROADMAP K07 entry; RESEARCH Active Summary; `docs/promt.md` §E3; ARCHITECTURE Key Principles 1, 6, 8, 11; K12 (drop-order codification — Q9 cross-link); K06 (Window decomposition — sharding successor); K17 (test harness simplification — K15 Limitation #5 cross-link).

## Context

flui-v2 inherits `AppCell = RefCell<App>` from the `gpui-ce` fork. The cell is wrapped in `Rc<AppCell>` and shared across `Application` (the public top-level handle), `App.this: Weak<AppCell>` (internal back-pointer), `AsyncApp` (async context), and the four test-context types (`TestAppContext`, `TestApp`, `HeadlessAppContext`, `VisualTestContext`). Re-entrancy was previously undefined: any nested `update_window` / `update_entity` would trigger `RefCell::borrow_mut` and panic with bare `BorrowMutError` text. K15 (the immediately preceding Phase 0-K spec) structured the contract — `ReentryError` enum, `WindowUpdateGuard` / `EntityUpdateGuard` RAII, `ReentryMode { Strict, Loose }`, unified `EntityMap::double_lease_panic` Display — but explicitly deferred the borrow primitive itself to K07 (K15 spec line 240: *"K07 — AppCell removal. Inherits `ReentryError` (may add new variants under token model)"*).

K07 closes E3 from `docs/promt.md` ("AppCell = RefCell<App> with TODO 'remove after stabilization' (HIGH-RISK)") and unblocks K05 (Element trait → context object), K01 (Provider rewrite), K02 (Widget identity / Key), K03 (Widget::build separation), K04 (Effect/Frame contract), and the Framework tier (Phase II-F SF04 — State<W> + StateMap; SF05 — setState + dirty-list).

Phase 1 of the plan dispatched three parallel research agents (UI-framework comparison, Rust borrow-primitive comparison, local candidate spike) plus the `rust-api-migration-auditor` review. The convergent recommendation, backed by concrete file:line analysis of the three candidates in §"Design choice — three candidates", is **Candidate B (single-borrow guard)**.

## Goals

1. Replace `AppCell = RefCell<App>` with `flui_core::app::cell::AppCell` — a hand-rolled cell whose `try_borrow_mut` returns `Result<AppRefMut<'_>, ReentryError>` directly, eliminating the `BorrowMutError → ReentryError` conversion K15 introduced.
2. Preserve the `Application(Rc<AppCell>)` and `App.this: Weak<AppCell>` topology so that 100+ migration callsites are signature-compatible.
3. Discharge K15's six Known Limitations (with explicit per-limitation closure status — see §"K15 contract preservation" below):
   - Limitation #1 (10+ unstructured `app.borrow_mut()` in `async_context.rs`): convert to `try_borrow_mut().map(...)?` shape.
   - Limitation #2 (`AsyncApp::as_mut` raw panic): structure via `ReentryError::AsyncContextAsMut` Display.
   - Limitation #3 (`web` platform dispatcher exposure): verified via existing test platform; smoke gate in plan Task 41.
   - Limitation #4 (`AppBorrowed` no source location): `#[track_caller]` on every `borrow*` method; `RUST_LOG=flui_core::app::cell=trace` for callsite context.
   - Limitation #5 (Window-level reentry behavioral tests): deferred to K17 (test harness simplification) per K15's own deferral. K07 does NOT extend the harness.
   - Limitation #6 (panic-leak on `currently_updating_entity` / `window_update_stack`): closed via plan Task 14a — RAII guards now feasible because the borrow primitive no longer flows `&mut App` through the closure body.
4. Establish Miri-verified `unsafe` discipline for the cell module, scoped to ~3-5 `unsafe` blocks all in `crates/flui-core/src/app/cell.rs`.
5. Preserve K15's re-entrancy contract verbatim — every existing `ReentryError::*` variant produces the same Display under K07.

## Non-goals

(Verbatim from plan §"What K07 explicitly does NOT do".)

- Does NOT remove `RefCell<Keymap>`, `RefCell<Arena>`, `RefCell<Window>`, or any of the field-level `RefCell` instances inside `App`. Those are out of scope; K07 owns only the App-level cell.
- Does NOT change `Element` trait method signatures (K05).
- Does NOT introduce `BuildOwner` / `PipelineOwner` / `SemanticsOwner` (K06).
- Does NOT touch `Render::&mut self` (K03).
- Does NOT refactor the Provider system (K01).
- Does NOT introduce `Key` (K02) or Widget identity.
- Does NOT touch the pending-effects queue or change frame phases (K04).
- Does NOT widen `Element` or `Render` trait signatures (K05).
- Does NOT change `Platform::*` trait surface.
- Does NOT add `tracing` (A4 is roadmap, out of K07).
- Does NOT touch gesture re-entry surface (A7-audit-closed).
- Does NOT modify `Cargo.lock` (workspace policy frozen).
- Does NOT introduce sharded ownership (Slint / Dioxus / Floem / Servo pattern). Sharding is K06+ territory. K07 stays monolithic by explicit choice.
- Does NOT redesign the AsyncApp surface to enqueue-then-poll (Candidate A's elegance). Partial structuring (Limitation #1) only.
- Does NOT add a new dependency (`qcell`, `ghost-cell`, `generativity`). Hand-rolled is the deliberate choice.
- Does NOT bypass Miri verification of the new `unsafe`. PR-blocking scoped Miri (per Q6 resolution).

## Current state (post-K15)

| Aspect | State | File:line |
|---|---|---|
| Public top-level handle | `pub struct Application(Rc<AppCell>)` | [app.rs:139](../../../crates/flui-core/src/app.rs#L139) |
| Internal back-pointer | `App::this: Weak<AppCell>` | [app.rs:585](../../../crates/flui-core/src/app.rs#L585) |
| `App::new_app` | returns `Rc<AppCell>` | [app.rs:684](../../../crates/flui-core/src/app.rs#L684) |
| Async context | `AsyncApp { app: Weak<AppCell>, … }` | [async_context.rs:23](../../../crates/flui-core/src/app/async_context.rs#L23) |
| Test contexts | `Rc<AppCell>` field shape (4 types) | [test_context.rs:32](../../../crates/flui-core/src/app/test_context.rs#L32), [test_app.rs:41/322](../../../crates/flui-core/src/app/test_app.rs#L41), [headless_app_context.rs:40](../../../crates/flui-core/src/app/headless_app_context.rs#L40), [visual_test_context.rs:23](../../../crates/flui-core/src/app/visual_test_context.rs#L23) |
| Borrow API | `AppCell::borrow_mut() -> AppRefMut<'_>`, `try_borrow_mut() -> Result<…, BorrowMutError>` | [app.rs:75-108](../../../crates/flui-core/src/app.rs#L75) |
| Re-entry detection | K15 contract via `WindowUpdateGuard` + `EntityUpdateGuard` + `double_lease_panic` unified Display | [reentrancy.rs](../../../crates/flui-core/src/reentrancy.rs) |
| AppCell-derived callsites (post-spike recount) | **103 narrow-pattern + 10 storage + 33 symbol references** | `.k07-recon.txt` |
| `App.this.upgrade()` patterns | **5 distinct sites** (NOT ~30 as plan rev 1-3 claimed) | app.rs:215, context.rs:74,110,166, test_context.rs:660 |
| Unstructured `app.borrow_mut()` in `AsyncApp` | 10+ at lines 39, 45, 55, 65, 126, 135, 152, 168, 182 | K15 Limitation #1 |
| `AsyncApp::as_mut` panic | raw `panic!` at line 73 | K15 Limitation #2 |
| `option_env!("TRACK_THREAD_BORROWS")` debug shim | 5 locations in `app.rs:75-135` | K15 Limitation #3 (impl-side) |
| K15 Limitation #6 panic-leak | `currently_updating_entity` (app.rs:2497) and `window_update_stack` not RAII-restored on panic | K15 spec line 202 + inline comment app.rs:2483 |
| AppCell `#[doc(hidden)]` | yes | [app.rs:74](../../../crates/flui-core/src/app.rs#L74) — type hidden, methods are `pub` |
| `K15` test count baseline | 344 passed + 1 ignored | `.k07-baseline.txt` |
| Existing `unsafe` block count in flui-core | 801 (FFI-heavy, mostly platform/) | K07 adds 3-5 in `app/cell.rs` (+0.5%) |

## Design choice — three candidates

### Candidate A — Pass-through `&mut App` (no cell)

`Rc<AppCell>` deleted; `App` owned by run-loop top-level closure; passed by `&mut` through every call chain. `AsyncApp` becomes enqueue-only — every operation produces `Future<T>` resolved by the run-loop.

**API blast radius:** maximal. `Application(Rc<AppCell>)` becomes `Application(Option<App>)`; `App.this` deleted. `AsyncApp` (`async_context.rs:22-26`) loses its `Weak<AppCell>` field; methods change return type from `T`/`Result<T>` to `Receiver<T>`/`Task<T>`. Every async-side closure (currently sync) becomes async.

**Migration cost:** ~1500-2500 LoC, intellectual throughout. Concrete: `AsyncWindowContext::update` (`async_context.rs:292-295`), `update_root` (:298-303), `on_next_frame` (:306-310), `read_global` (:313-319), `update_global` (:322-333), all `VisualContext` methods (:467-501) all change shape.

**HRTB cost:** none directly — but every closure passed to `AsyncApp` becomes `FnOnce(&mut App) -> R + 'static`, breaking captures of transient `&App` borrows.

**`unsafe` count:** zero. Pure ownership solution.

**K15 fit:** eliminates `ReentryError::AppBorrowed` by construction (no cell). `WindowUpdateGuard` / `EntityUpdateGuard` still required (same-target re-entry detection lives on `App` itself, not the cell).

**`App.this: Weak<AppCell>`:** removed. 5 upgrade sites need rewiring; in particular `test_context.rs:660` (`cx.this.upgrade().unwrap()`) materializes an `Rc<AppCell>` for the test harness — fundamentally breaks under A.

**Verdict — DEFER.** Async-surface rewrite is itself a multi-week project. Mixing it with K07 violates K07 PR scope discipline. Recommended as a follow-up R-spec or future K-spec ("K07b: AsyncApp surface redesign").

### Candidate B — Single-borrow guard (`UnsafeCell<App>` + `BorrowState` flag)

Custom `AppCell { app: UnsafeCell<App>, borrowed: Cell<BorrowState>, _not_send: PhantomData<*const ()> }`; `borrowed` is a small enum with `Free / Mut / Shared(NonZeroU32)` variants; `borrow*` methods return RAII guards (`AppRef`, `AppRefMut`) whose `Drop` clears the flag. Same callsite ergonomics as today.

**API blast radius:** minimal. `AppCell::borrow()` / `borrow_mut()` / `try_borrow_mut()` keep their signatures. `Application::run` unchanged. `AsyncApp::app: Weak<AppCell>` unchanged. The 103 narrow-pattern AppCell callsites compile as-is. `try_borrow_mut` return type widens from `Result<_, BorrowMutError>` to `Result<_, ReentryError>` — the only callsite-level change is K15's `.map_err(ReentryError::from)?` becomes `.?` (cleaner). All 5 `App.this.upgrade()` sites compile unchanged.

**Migration cost:** ~200 LoC for the new primitive in `app/cell.rs`; 0 callsite-LoC. Mechanical.

**HRTB cost:** none.

**`unsafe` count:** 3-5 blocks, all in `app/cell.rs`: `borrow()` projection, `borrow_mut()` projection, optional `try_borrow_mut()` projection (or thin wrapper), guard `Drop` flag clear. Auditable in one file. Comparable to `std::cell::RefCell`'s internals.

**K15 fit:** native. `AppCell::try_borrow_mut` returns `Err(ReentryError::AppBorrowed)` directly — the K15 `From<BorrowMutError>` impl becomes redundant (deleted in Task 9). `WindowUpdateGuard` / `EntityUpdateGuard` are ORTHOGONAL to the cell flag and remain necessary: cell flag = "is App borrowed at all", K15 = "is THIS specific window/entity already being updated". Different concerns, both stay.

**`App.this: Weak<AppCell>`:** unchanged. `Rc::new_cyclic` at app.rs:700 continues to work (cell still inside `Rc`). All 5 upgrade sites compile unchanged.

**Verdict — LOCKED.** See §"Recommended candidate (LOCKED)".

### Candidate C — GhostCell-style branded `AppToken<'id>`

`AppCell<'id>` stores `App` behind `UnsafeCell`; access requires `&mut AppToken<'id>` where `'id` is a unique invariant lifetime constructed once via `Application::run`'s top-level HRTB closure. The brand poisons every type that stores an `AppCell`.

**API blast radius:** catastrophic. Every `pub` method on `App`, every `AppContext` trait method, every callback typedef (the 7 type aliases at app.rs:243-250) sprouts a `'id` lifetime. `Application::run` becomes `for<'id> FnOnce(AppToken<'id>, &mut App<'id>) -> _`. **`AsyncApp` cannot exist** — `AsyncApp` outlives any single `'id` brand by definition (`async_context.rs:15-20` documents "static lifetime so it can be held across `await` points"). The async surface either falls back to Candidate A's enqueue model (compounding the migration) or gets entirely rewritten.

**Migration cost:** >5000 LoC, intellectual throughout.

**HRTB cost:** pervasive. Every `Fn`/`FnMut` accepting an `AppToken` becomes `for<'id> Fn...`. The 7 callback typedefs at app.rs:243-250 become HRTB type aliases — Rust supports HRTB in `where` clauses but not directly in `type` aliases without TAIT. `AppContext` trait gets `'id` parameter on every method or becomes a GAT trait. Trait-object stored callbacks (the typical `Box<dyn FnMut(&mut App)>`) become very awkward — must existentialize the brand or thread it through everywhere.

**`unsafe` count:** ~2 blocks for the brand invariant (PhantomData<fn(&'id ()) -> &'id ()> is safe; `unsafe` is in the `with_token` constructor that asserts uniqueness). Confined to one module, but the `'id` brand is conceptually load-bearing across the entire crate.

**K15 fit:** token presence proves "I have access" but not "no one else has access for THIS window/entity" — K15's same-target detection is unaffected. `WindowUpdateGuard` / `EntityUpdateGuard` remain necessary. `ReentryError::AppBorrowed` may become unreachable (the brand statically prevents two simultaneous `&mut App` references), but the other variants stay.

**`App.this: Weak<AppCell>`:** cannot be expressed cleanly. `Weak<AppCell>` requires a stable type; `AppCell<'id>` makes the field unstorable across `'id` regions.

**Verdict — REJECT.** RustBelt-proven sound, but ergonomically incompatible with retained-mode UI. The `ghost-cell` crate's tracking issues confirm the same in production: trait objects, async, library boundaries all suffer from `'id` propagation. Cost vastly exceeds the benefit.

### Recommended candidate (LOCKED — revision 4 post-spike)

**Candidate B** — hand-rolled `UnsafeCell<App>` + `BorrowState` flag returning `ReentryError` directly.

**Lock rationale:**

1. **Lowest migration risk** — signature-compatible; 103 callsites unchanged after the cell rewrite.
2. **Native K15 fit** — cell flag *is* `ReentryError::AppBorrowed`; deletes the `From<BorrowMutError>` conversion as dead code; `WindowUpdateGuard` / `EntityUpdateGuard` remain orthogonal and keep their behavior.
3. **One-file `unsafe`** — auditable in `app/cell.rs` (~200 LoC), Miri-clean within reasonable effort.
4. **No new dependencies** — Phase 0-K minimizes deps (see K92 dep-update spec). Rejected `qcell`/`ghost-cell` dep-add for marginal gain.
5. **Leaves room for future Candidate A** — if the AsyncApp surface ever needs full enqueue-then-poll redesign, that's a follow-up spec ("K07b" or post-K07 hygiene). K07 itself stays scoped.
6. **Confirmed by 3 independent research dispatches:** UI-framework comparison (Druid/Iced/Bevy patterns inapplicable, Slint sharding is K06 territory), Rust borrow-primitive comparison (hand-rolled wrapper recommended over qcell/ghost-cell for this exact use case), local file:line spike (3 candidates analyzed against actual code).

**Rejected alternatives (with documented reasons):**

- **Candidate A** — DEFER. Multi-week refactor; mixing with K07 violates PR scope. K15 Limitation #1 (10+ unstructured) partially addressed by Candidate B's `Result<_, ReentryError>` shape.
- **Candidate C** — REJECT. HRTB poisons trait objects; AsyncApp doesn't compose; widget authors writing custom `Element` see `'id` in trait method signatures (barrier to ecosystem).
- **`qcell` / `ghost-cell` / `generativity` dep** — REJECT. Adds dep weight; `qcell::TLCell` adds global-marker constraint. Hand-rolled cell is cheaper.
- **Sharding (Slint/Dioxus/Floem/Servo pattern)** — OUT OF SCOPE for K07. K06 handles per-domain owners. K07 monolithic by explicit choice; does NOT preclude sharding.

## Detailed type surface

### New module: `crates/flui-core/src/app/cell.rs`

```rust
//! K07 — single-borrow App cell with structured re-entry detection.
//!
//! Replaces `AppCell = RefCell<App>` (gpui-ce inheritance, marked
//! "Strongly consider removing after stabilization" in the original).
//! K07 keeps the type name `AppCell`, the `Rc<AppCell>` topology, and
//! the `borrow_mut() -> AppRefMut<'_>` API shape for migration
//! compatibility — but the internals use `UnsafeCell<App>` + a small
//! `BorrowState` flag, and `try_borrow_mut` returns `Result<_,
//! ReentryError>` directly (NOT `Result<_, BorrowMutError>` — the K15
//! `From<BorrowMutError>` conversion has been removed).
//!
//! # Contract
//!
//! Recursive `borrow_mut` produces `ReentryError::AppBorrowed` (K15
//! contract). Use `cx.defer` / `window.defer` to schedule work that
//! must touch App from inside another `&mut App` scope.
//!
//! # Drop-on-panic
//!
//! Guards' `Drop` releases the borrow flag, BUT does NOT roll back
//! partial mutations to App. Same as pre-K07 `RefCell<App>` semantics.
//! For `catch_unwind`-based recovery, set
//! `App::set_reentry_mode(ReentryMode::Loose)` and accept potential
//! false-positive `NestedEntityUpdate` on the next `update_entity`
//! after a caught panic.
//!
//! # Auto-trait invariants
//!
//! `AppCell: !Send + !Sync` (enforced by `PhantomData<*const ()>`).
//! `AppRef<'_>: !Send + !Sync` (transitively).
//! `AppRefMut<'_>: !Send + !Sync` (transitively).
//! `AppCell: UnwindSafe` IFF `App: UnwindSafe` (preserves pre-K07
//! `RefCell<App>` behavior). Compile-time assertions in `#[cfg(test)]`
//! module pin all four invariants.

use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::reentrancy::ReentryError;

#[doc(hidden)]
pub struct AppCell {
    app: UnsafeCell<crate::App>,
    borrowed: Cell<BorrowState>,
    /// Pins `!Send + !Sync` (matches pre-K07 RefCell<App> auto-trait set
    /// because `App: !Send + !Sync` already).
    _not_send: PhantomData<*const ()>,
}

#[derive(Clone, Copy)]
enum BorrowState {
    Free,
    Mut,
    Shared(NonZeroU32),
}

impl AppCell {
    pub(crate) fn new(app: crate::App) -> Rc<Self> {
        Rc::new(Self {
            app: UnsafeCell::new(app),
            borrowed: Cell::new(BorrowState::Free),
            _not_send: PhantomData,
        })
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn borrow(&self) -> AppRef<'_> {
        match self.try_borrow() {
            Ok(r) => r,
            Err(e) => panic!("{}", e),
        }
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn borrow_mut(&self) -> AppRefMut<'_> {
        match self.try_borrow_mut() {
            Ok(r) => r,
            Err(e) => panic!("{}", e),
        }
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow(&self) -> Result<AppRef<'_>, ReentryError> {
        let next = match self.borrowed.get() {
            BorrowState::Free => BorrowState::Shared(NonZeroU32::new(1).unwrap()),
            BorrowState::Shared(n) => match n.checked_add(1) {
                Some(m) => BorrowState::Shared(m),
                // Saturation guard — structurally impossible
                // (u32::MAX simultaneous shared borrows would
                // exhaust the address space first), but pinned
                // as a regression guard. Returns AppBorrowed
                // for symmetry with the Mut path.
                None => return Err(ReentryError::AppBorrowed),
            },
            BorrowState::Mut => return Err(ReentryError::AppBorrowed),
        };
        self.borrowed.set(next);
        log::trace!(target: "flui_core::app::cell", "shared borrow acquired");
        // SAFETY: borrow flag has been transitioned to Shared(_),
        // ruling out simultaneous Mut borrow. Reference lifetime
        // is bounded by AppRef's lifetime, which is strictly less
        // than the cell's lifetime; the guard's Drop clears the
        // flag before the cell is dropped (no use-after-free).
        // Stacked Borrows: the projection `&*cell.app.get()` is
        // taken from the unique root reference `cell.app`; no
        // aliased &mut from a different root exists in scope.
        let app_ref = unsafe { &*self.app.get() };
        Ok(AppRef { cell: self, app: app_ref })
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow_mut(&self) -> Result<AppRefMut<'_>, ReentryError> {
        match self.borrowed.get() {
            BorrowState::Free => {
                self.borrowed.set(BorrowState::Mut);
                log::trace!(target: "flui_core::app::cell", "mut borrow acquired");
                // SAFETY: borrow flag has been transitioned to Mut,
                // ruling out any simultaneous Shared or Mut borrow.
                // Reference lifetime is bounded by AppRefMut's
                // lifetime, which is strictly less than the cell's
                // lifetime; the guard's Drop clears the flag before
                // any subsequent borrow can be issued. Stacked
                // Borrows: the projection `&mut *cell.app.get()` is
                // the unique active reference (no Shared can coexist
                // because `borrowed` enforces it). No aliased &
                // exists in scope (every AppRef holds the same
                // borrow-flag-discipline contract).
                let app_mut = unsafe { &mut *self.app.get() };
                Ok(AppRefMut { cell: self, app: app_mut })
            }
            BorrowState::Mut | BorrowState::Shared(_) => {
                log::warn!(
                    target: "flui_core::app::cell",
                    "AppCell already borrowed; emitting ReentryError::AppBorrowed (consider cx.defer)"
                );
                Err(ReentryError::AppBorrowed)
            }
        }
    }
}

#[doc(hidden)]
pub struct AppRef<'a> {
    cell: &'a AppCell,
    app: &'a crate::App,
}

impl Deref for AppRef<'_> {
    type Target = crate::App;
    fn deref(&self) -> &crate::App {
        self.app
    }
}

impl Drop for AppRef<'_> {
    fn drop(&mut self) {
        let next = match self.cell.borrowed.get() {
            BorrowState::Shared(n) => match NonZeroU32::new(n.get() - 1) {
                Some(m) => BorrowState::Shared(m),
                None => BorrowState::Free,
            },
            // Unreachable: an AppRef exists IFF the flag is Shared(_).
            other => unreachable!("AppRef Drop on inconsistent state {:?}", other),
        };
        self.cell.borrowed.set(next);
        log::trace!(target: "flui_core::app::cell", "shared borrow released");
    }
}

#[doc(hidden)]
pub struct AppRefMut<'a> {
    cell: &'a AppCell,
    app: &'a mut crate::App,
}

impl Deref for AppRefMut<'_> {
    type Target = crate::App;
    fn deref(&self) -> &crate::App {
        self.app
    }
}

impl DerefMut for AppRefMut<'_> {
    fn deref_mut(&mut self) -> &mut crate::App {
        self.app
    }
}

impl Drop for AppRefMut<'_> {
    fn drop(&mut self) {
        // Always transitions Mut → Free (an AppRefMut exists IFF
        // the flag is Mut, by construction).
        debug_assert!(matches!(self.cell.borrowed.get(), BorrowState::Mut));
        self.cell.borrowed.set(BorrowState::Free);
        log::trace!(target: "flui_core::app::cell", "mut borrow released");
    }
}

#[cfg(test)]
mod auto_trait_tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    // R2 — !Send + !Sync invariant (matches pre-K07 RefCell<App>).
    assert_not_impl_any!(AppCell: Send, Sync);
    assert_not_impl_any!(AppRef<'static>: Send, Sync);
    assert_not_impl_any!(AppRefMut<'static>: Send, Sync);

    // R3 — UnwindSafe preservation. Pre-K07 baseline:
    // RefCell<App>: UnwindSafe IFF App: UnwindSafe.
    // Post-K07: same (UnsafeCell<App>: UnwindSafe IFF App: UnwindSafe).
    // No assertion needed — the auto-trait inference is automatic.
    // Documented for review.
}
```

The above is the canonical sketch. Final implementation will reduce duplication via internal helpers and may inline `try_borrow` inside `try_borrow_mut` if codegen is identical.

### Existing public surface — preserved

| Symbol | Pre-K07 | Post-K07 | Change |
|---|---|---|---|
| `Application(Rc<AppCell>)` | `pub struct` | `pub struct` | none |
| `Application::new` etc. | unchanged | unchanged | none |
| `App` struct | unchanged | unchanged | none |
| `App.this: Weak<AppCell>` | `pub(crate)` | `pub(crate)` | none |
| `App::new_app(...) -> Rc<AppCell>` | `pub(crate)` | `pub(crate)` | none |
| `AppCell::borrow` / `borrow_mut` | `pub`, returns Ref/Mut | `pub`, returns AppRef/AppRefMut (same names) | internals |
| `AppCell::try_borrow_mut` | `pub`, `Result<_, BorrowMutError>` | `pub`, `Result<_, ReentryError>` | **return type widens** |
| `AppRef<'_>` / `AppRefMut<'_>` | `pub` (doc-hidden), wraps `Ref<App>`/`RefMut<App>` | `pub` (doc-hidden), wraps `&App`/`&mut App` | internals |
| `AppContext` trait | unchanged | unchanged | none |
| `AsyncApp::app: Weak<AppCell>` | `pub(crate)` | `pub(crate)` | none |
| `AsyncApp::app() -> Rc<AppCell>` | `expect`-panic on Weak::upgrade fail | `Result<Rc<AppCell>, ReentryError::AppGoneAway>` (Q4 decision) | **signature widens** |
| `AsyncApp::as_mut` | raw `panic!(…)` | `panic!(ReentryError::AsyncContextAsMut)` | **panic Display structured** |
| `AppContext::as_mut` (trait method) | returns `GpuiBorrow<'a, T>` | returns `GpuiBorrow<'a, T>` (Q8 — no widening) | none |

### New variants on `ReentryError`

```rust
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ReentryError {
    // K15 (existing)
    NestedWindowUpdate(WindowId),
    NestedEntityUpdate(EntityId),
    ElementStateInUse { global_element_id: GlobalElementId, type_id: TypeId },
    PromptInProgress,
    AppBorrowed,

    // K07 (new)
    /// Q4 decision — AsyncApp's Weak::upgrade returned None because
    /// the App is mid-drop. Replaces the raw `expect("app was
    /// released before async operation completed")` panic at
    /// async_context.rs:32.
    #[error("App was released before async operation completed (caller held an AsyncApp past App::drop)")]
    AppGoneAway,

    /// Q2 decision — AsyncApp::as_mut is fundamentally
    /// incompatible with the trait shape (returns
    /// GpuiBorrow<'a, T>, no Result). Pre-K07: raw `panic!(
    /// "Cannot as_mut with an async context. Try calling
    /// update() first")`. Post-K07: panic with structured Display.
    #[error("AppContext::as_mut called on AsyncApp; use AsyncApp::update(...) to acquire mutable access")]
    AsyncContextAsMut,
}
```

`#[non_exhaustive]` already on the enum (K15 invariant). Adding `AppGoneAway` and `AsyncContextAsMut` is forward-compatible — no semver impact beyond what K15 already declared.

The K15 `impl From<std::cell::BorrowMutError> for ReentryError` block (reentrancy.rs:161-163) is DELETED. Plan Task 9 covers the surface (rustdoc updates + the import line at reentrancy.rs:76).

## Migration plan

(Plan tasks 11-25 carry the file-by-file migration. Summary here.)

### Phase 3 (App / Application — sequential)

- **Task 11.** `App::new_app` returns the new `Rc<AppCell>` (signature-compatible).
- **Task 12.** `Application` (lines 139-241) — every `self.0.borrow*()` mass-replace; semantics preserved.
- **Task 13.** `App.this.upgrade()` consumers (5 distinct sites) — compile unchanged.
- **Task 14.** Audit `pending_effects` queue / drain pathway for RefCell-panic-shape reliance.
- **Task 14a.** **(K15 Limitation #6 closure.)** RAII guard for `currently_updating_entity` and verify `WindowUpdateGuard` panic-safety. Property test `prop_currently_updating_entity_restored_after_panic`.

### Phase 4 (5 PARALLEL tasks)

- **Tasks 15-19.** `AsyncApp` / `TestAppContext` / `TestApp` / `HeadlessAppContext` / `VisualTestContext` — file-disjoint, parallel-safe.

### Phase 5 (sequential after Phase 4)

- **Tasks 20-24.** `elements/`, `subscription.rs`, `executor.rs` (test fixture only), `platform/*` (with K15 deferral comment updates), `examples/`.
- **Task 25.** Final scan — zero `BorrowMutError`, zero `TRACK_THREAD_BORROWS`, zero AppCell-derived `borrow*` hits via narrow-pattern grep.

## K15 contract preservation table

| K15 contract element | Post-K07 status | Notes |
|---|---|---|
| `ReentryError::NestedWindowUpdate(WindowId)` | unchanged | `WindowUpdateGuard` orthogonal to cell flag |
| `ReentryError::NestedEntityUpdate(EntityId)` | unchanged | Same-entity check at app.rs:2469 + `EntityMap::double_lease_panic` unification |
| `ReentryError::ElementStateInUse { global_element_id, type_id }` | unchanged | `with_element_state` is its own panic shape |
| `ReentryError::PromptInProgress` | unchanged | `Window::prompt` Result shape preserved |
| `ReentryError::AppBorrowed` | **now produced directly by `AppCell::try_borrow_mut`** | `From<BorrowMutError>` impl deleted |
| `ReentryMode { Strict, Loose }` | unchanged | `PanicLikeUpstream` deferred decision: DROP per Q3 |
| `WindowUpdateGuard::commit_pop` two-phase | unchanged | Cell flag is orthogonal; guard sequence preserved |
| `EntityUpdateGuard::enter` | unchanged | Same |
| `EntityMap::double_lease_panic` unified Display | unchanged | Same |
| `cx.defer` / `Window::defer` escape hatches | unchanged | Same |
| K15 11 reentrancy tests | **all 11 pass under K07** | Required by plan Task 26 done criterion 11 |

## Decisions on Q1-Q12 (all resolved)

| Q | Decision | Rationale |
|---|---|---|
| Q1 | Candidate B locked | Phase 1 spike + 3-agent research convergence |
| Q2 | Keep panic shape; structure Display via `ReentryError::AsyncContextAsMut` | Q8 widening too costly; structured panic = K15 pattern |
| Q3 | DROP `PanicLikeUpstream`; document obsolete | Cell flag *is* `AppBorrowed`; no `BorrowMutError` to mimic |
| Q4 | Add `ReentryError::AppGoneAway`; `AsyncApp::app()` returns `Result` | Structured replaces raw `expect`-panic |
| Q5 | `log::trace!` (project style) | Project uses `log` crate; new feature flag overkill |
| Q6 | **PR-blocking scoped Miri** for `app/cell.rs` only | Soundness gate for new `unsafe`; bounded cost |
| Q7 | NO split — single PR | Candidate B is signature-compatible; no review-overhead benefit from split |
| Q8 | Keep `AppContext::as_mut` trait shape | Widening breaks 5 implementors + downstream |
| Q9 | `UnsafeCell<App>` by-value preserves Drop | Matches K12 invariant |
| Q10 | `Application: Clone` remains absent | Out of K07 scope |
| Q11 | K07-only CHANGELOG entry; K99/K15 backfill in separate PR | PR scope discipline |
| Q12 | `K07 — AppCell removal (Phase 0-K, third spec)` | K15 PR #9 style; architectural change |

## Open questions

**EMPTY.** All 12 open questions resolved with documented rationale in §"Decisions on Q1-Q12 (all resolved)" above. Any new question surfaced by adversarial review (plan Task 6) becomes a revision-2 entry.

## `unsafe` audit (rev 1 sketch — finalized in revision-2 after adversarial review)

| Block | Location | SAFETY justification | Miri test |
|---|---|---|---|
| 1 | `try_borrow` `let app_ref = unsafe { &*self.app.get() }` | `borrowed` flag transitioned to `Shared(_)` before access; no `Mut` coexists; `app_ref` lifetime ≤ `AppRef::Drop` ≤ cell lifetime; one root reference | `prop_borrow_then_borrow_drop_releases` |
| 2 | `try_borrow_mut` `let app_mut = unsafe { &mut *self.app.get() }` | `borrowed` flag transitioned to `Mut`; no `Shared` or `Mut` coexists by enum exhaustiveness; lifetime same as #1; one root reference | `prop_borrow_mut_then_borrow_mut_returns_app_borrowed` |
| 3 | `AppRef::Drop` flag clear | Single-threaded `!Send + !Sync` cell, `Cell<BorrowState>` is interior-mut-safe; `unreachable!` on inconsistent state catches logic bugs | `prop_panic_during_borrow_releases` |
| 4 | `AppRefMut::Drop` flag clear | Same as #3, plus `debug_assert!(Mut)` | Same |

Three blocks total in the rev-1 sketch (not 5). Saturation overflow (`Shared(u32::MAX) → ?`) is a `match` arm returning `Err` — not `unsafe`.

## Compile-time auto-trait tests (R2/R3/R4)

```rust
// R2 — !Send + !Sync invariant
static_assertions::assert_not_impl_any!(AppCell: Send, Sync);
static_assertions::assert_not_impl_any!(AppRef<'static>: Send, Sync);
static_assertions::assert_not_impl_any!(AppRefMut<'static>: Send, Sync);

// R3 — UnwindSafe preservation: documented (auto-trait inference matches pre-K07).

// R4 — #[track_caller] on every borrow* method (manually verified, see source).
```

Adds `static_assertions = "1"` to `[dev-dependencies]` (single-use crate, ~50 LoC, no transitive deps). Already-implicit dep candidate; verify via Task 1.

## Testing strategy

- **Property tests (Task 26):**
  - `prop_borrow_mut_then_borrow_mut_returns_app_borrowed_in_strict`
  - `prop_drop_releases_borrow`
  - `prop_panic_during_borrow_releases_borrow`
  - `prop_borrow_share_count_caps` (saturation guard)
  - `prop_currently_updating_entity_restored_after_panic` (R5 / Limitation #6)
  - **Port forward all 11 K15 reentrancy tests** to the new primitive.
- **AsyncApp behavioral tests (Task 28):**
  - `async_app_update_after_app_drop_returns_app_gone_away`
  - `async_app_as_mut_after_drop_returns_structured_error`
  - `async_app_borrow_mut_propagates_reentry_error`
- **Compile-time auto-trait tests (Task 8):** `static_assertions::assert_not_impl_any!` for `AppCell` / `AppRef` / `AppRefMut`.
- **Miri (Task 27):** PR-blocking scoped pass — `cargo +nightly miri test -p flui-core cell` MUST be green for the cell module's tests.

## Known Limitations (post-K07)

1. **Runtime borrow check not eliminated** — Candidate B keeps the `O(1)` flag check. ROADMAP Key Principle #8 ("no `Rc<RefCell<…>>` on hot paths") is interpreted permissively: the flag check is sub-microsecond, not a hot-path concern. Strict elimination would require Candidate A (deferred).
2. **AsyncApp full surface redesign deferred** — K15 Limitation #1 (10+ unstructured) partially resolved via `try_borrow_mut() -> Result<_, ReentryError>`. Full enqueue-then-poll redesign is a future spec.
3. **3-5 `unsafe` blocks added** — confined to one file; SAFETY-commented; Miri-verified. Marginal +0.5% over the 801 existing FFI-heavy unsafe sites.
4. **Cargo-semver-checks (R2 in roadmap) will flag changes to `pub struct AppCell` internals** — `#[doc(hidden)]` is "soft private" for semver purposes. Documented for the future R2 spec.
5. **K05 may need partial / sharded borrows** — out of K07 scope. K05's plan inherits the monolithic-cell choice; sharding is K06+ territory.

## Done criteria

(Plan Done Criteria 1-24, listed verbatim. K07 closes when all 24 are checked. See plan §"Done criteria".)

## Cross-references

- **K99** spec — MSRV bump prerequisite.
- **K15** spec — re-entrancy contract being preserved verbatim. Six Known Limitations enumerated; K07 closes #1 (partial), #2, #4, #6 directly; #3 verified via smoke; #5 deferred to K17.
- **ROADMAP** Phase 0-K K07 entry.
- **RESEARCH** Active Summary — Phase 0-K rationale.
- **`docs/promt.md`** §E3 — original audit finding.
- **ARCHITECTURE** Key Principles 1, 6, 8, 11.
- **K12** — drop-order codification (Q9 cross-link).
- **K06** — Window decomposition + ownership split (sharding successor).
- **K17** — test harness simplification (Limitation #5 destination).

## Unblocks

- **K05** — Element trait → context object.
- **K01** — Provider rewrite.
- **K02** — Widget identity / Key.
- **K03** — Widget::build separation.
- **K04** — Effect / Frame contract.
- **SF04** — State<W> + StateMap (Framework tier).
- **SF05** — setState + dirty-list (Framework tier).

## Risks (4-tier — see plan §"Risk assessment (rev 4 — post-spike)")

- **Tier 1 (PR-blocking):** R1 Stacked Borrows, R2 !Send+!Sync, R3 UnwindSafe, R4 #[track_caller], R5 panic-leak (Task 14a), R6 BorrowState transitions.
- **Tier 2 (review-gating):** R7 Miri CI policy (RESOLVED to PR-blocking via Q6), R8 cargo-semver-checks (deferred), AsyncApp regressions, K15 test regressions, web-platform regressions, drop-order.
- **Tier 3 (future-coupling, documented):** R9 K05 partial borrows, R10 Phase III `App: Send`, R11 mental model, R12 drop-on-panic.
- **Tier 4 (honest limitations):** L1 runtime check not eliminated, L2 AsyncApp redesign deferred, L3 +3-5 `unsafe`.

## Future considerations

(Per plan rev 4 R9/R10/R12 mitigation.)

- **R9 — K05 partial borrows.** When K05 introduces `&mut PaintCx<'_>` / `&mut LayoutCx<'_>`, K05's design may need sub-cell sharding (Slint/Dioxus pattern) or temp `&mut App` field-projection. K07's monolithic AppCell does NOT preclude (a). Document the deferred decision.
- **R10 — Phase III multi-threaded UI.** `_not_send: PhantomData<*const ()>` permanently blocks `App: Send`. If Phase III ever wants `App: Send` (iOS UIKit Main + Background Renderer; Android UI thread + GL thread), AppCell needs full redesign (`Mutex<App>` or thread-affinity assertion). Not blocker now.
- **R12 — Drop-on-panic.** Module rustdoc warning: "App is in best-effort consistent state after a panicking closure; for `catch_unwind`-based recovery, set `ReentryMode::Loose` and accept potential false-positive `NestedEntityUpdate` on the next `update_entity` after a caught panic."
- **Sharding deferred to K06.** Per-domain owners (BuildOwner / PipelineOwner / SemanticsOwner) will shard the App-level borrow domain. K07 stays monolithic by explicit choice.
- **Adopting `qcell::TLCell` post-K07.** If hand-rolled cell maintenance becomes burdensome (e.g., new `unsafe` blocks proliferate), `qcell::TLCell` is a drop-in replacement that adds 1 dep but removes ~150 LoC of `unsafe`. Re-evaluate after K05/K01 land.

## Decision log

### Revision 1 (2026-05-09 — this document)

Phase 1 spike + 3-agent research dispatch resulted in Candidate B lock. Rejected alternatives:
- Candidate A (pass-through `&mut App`): DEFER — ~1500-2500 LoC migration, multi-week. Mixing with K07 violates PR scope.
- Candidate C (GhostCell branded `'id`): REJECT — HRTB poisons trait objects (the 7 callback typedefs at app.rs:243-250); `AsyncApp` cannot exist with branded `'id`; widget authors writing custom `Element` impls see `'id` in trait method signatures (ecosystem barrier).
- `qcell` / `ghost-cell` dependency: REJECT — Phase 0-K minimizes deps. Hand-rolled cell is cheaper for the same functional power.
- Sharding (Slint/Dioxus/Floem/Servo pattern): OUT OF SCOPE. K06 territory.

All 12 open questions (Q1-Q12) resolved with rationale in §"Decisions on Q1-Q12 (all resolved)".

### Revision 2 (TBD — post-Task 6 adversarial review)

Will absorb findings from `flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor` per plan Task 6. Each finding categorized BLOCKER / MAJOR / MINOR; BLOCKERs patched into the spec, MAJORs split between spec patches and Known Limitations, MINORs deferred.
