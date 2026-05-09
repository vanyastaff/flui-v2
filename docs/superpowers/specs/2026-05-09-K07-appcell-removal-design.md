# K07 — AppCell removal (token-based borrow model)

**Status:** Design (revision 3 — second adversarial review absorbed; pending implementation).
**Revision history:**
- rev 1 (2026-05-09) — initial design, Candidate B locked.
- rev 2 (2026-05-09) — absorbed first Task 6 findings; 8 BLOCKERs + 12 MAJORs + 10 MINORs.
- rev 3 (2026-05-09) — absorbed SECOND Task 6 findings (rev 2 itself reviewed by 4 agents: 3 adversarial + 1 quality). **rev 2 introduced new BLOCKERs while fixing rev 1's** — most critically: (a) `unsafe impl UnwindSafe` is `error[E0199]` compile error since `UnwindSafe` is NOT an unsafe trait; (b) the entire "UnwindSafe regression" narrative was based on a factual error about std — `UnsafeCell<T>: UnwindSafe` automatically (only `!RefUnwindSafe` has a negative impl); (c) the `catch_unwind(AssertUnwindSafe(...))` pattern in Task 14a leaks `pending_updates` via `resume_unwind` through `App::update`'s frame; (d) `Q4 panic!("{}", e)` produces `String` payload, losing `ReentryError` type identity at `catch_unwind` boundary. **Major rev 3 corrections:** Drop manual `impl UnwindSafe` (auto-traited automatically); Replace `catch_unwind` pattern with raw-pointer field-projection guard (no unwind, no leak); Q4 use `panic_any(...)` for type-preservation OR document recovery-not-intended; Q2 fix `AsyncContextAsMut` Display to be context-agnostic; All "RAII guard" stale text updated; "4 unsafe blocks" → "2 unsafe blocks + N plain trait impls"; Miri CI job task added; criterion bench added; `static_assertions` explicit in `[dev-dependencies]`. 4 reviewers cross-confirmed convergence.
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
   - Limitation #6 (panic-leak on `currently_updating_entity` / `window_update_stack`): closed via plan Task 14a — **raw-pointer field-projection guards** with `Drop` running `unsafe { *self.ptr = self.prev }`. The pointer is derived from `&mut field` but stored in the guard as `*mut`, releasing the borrow conflict that defeated K15's RAII attempts AND avoiding the `catch_unwind`/`resume_unwind` path that would leak `pending_updates`.
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
2. **Native K15 fit** — cell flag *is* `ReentryError::AppBorrowed`; deletes the `From<BorrowMutError>` conversion as dead code; K15's inline `window_update_stack` push/pop and `currently_updating_entity` replace/restore patterns remain orthogonal and keep their behavior. (rev 2 correction: rev 1 incorrectly referenced `WindowUpdateGuard`/`EntityUpdateGuard` RAII types that do not exist in the codebase — K15 rejected RAII guards during implementation.)
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
//! # Auto-trait invariants (rev 3 — corrected from rev 1+rev 2)
//!
//! `AppCell: !Send + !Sync` (enforced by `PhantomData<*const ()>`).
//! `AppRef<'_>: !Send + !Sync` (transitively).
//! `AppRefMut<'_>: !Send + !Sync` (transitively).
//!
//! **`UnwindSafe` is automatic. No manual impl needed.** Rev 1 and rev 2 both
//! claimed manual intervention was required, but the rust-api-migration-auditor
//! verified against std source (`core/src/panic/unwind_safe.rs:181-202` in
//! 1.95 toolchain) that:
//! - `UnwindSafe` is a `pub auto trait UnwindSafe {}` with NO negative impl
//!   for `UnsafeCell<T>` or `RefCell<T>`.
//! - `RefUnwindSafe` is a `pub auto trait RefUnwindSafe {}` with the ONLY
//!   negative impl: `impl<T: ?Sized> !RefUnwindSafe for UnsafeCell<T>`.
//!
//! Therefore:
//! - Pre-K07 `AppCell` (wrapping `RefCell<App>`): `UnwindSafe` (auto), `!RefUnwindSafe`
//!   (via `UnsafeCell<T>` inside `RefCell<T>`).
//! - Post-K07 `AppCell` (wrapping `UnsafeCell<App>`): `UnwindSafe` (auto, identical
//!   to pre-K07), `!RefUnwindSafe` (auto, also identical to pre-K07).
//!
//! **There is no regression.** The compile-time assertion below LOCKS
//! `AppCell: UnwindSafe` so that any future code change accidentally introducing
//! a `!UnwindSafe` field (e.g., a closure) is caught at compile time:
//!
//! ```ignore
//! // Locks the auto-impl. No manual impl needed.
//! static_assertions::assert_impl_all!(AppCell: std::panic::UnwindSafe);
//! ```
//!
//! **`RefUnwindSafe` decision (rev 3):** K07 keeps `AppCell: !RefUnwindSafe`
//! (matches pre-K07). NO manual `impl RefUnwindSafe`. Reasoning: `RefUnwindSafe`
//! semantics are about shared (`&AppCell`) references surviving panic. Since
//! `AppCell` is `!Sync`, multi-thread panic-safety is moot. Within a single
//! thread, `AppRef<'_>` (shared borrow) IS `RefUnwindSafe`-flavored at use site,
//! but the cell type itself stays `!RefUnwindSafe` to match pre-K07 baseline.
//! If a future K-spec needs `RefUnwindSafe` for a specific `catch_unwind` pattern,
//! that's a deliberate widening at that point — not a K07 concern.

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

// rev 3 — NO manual UnwindSafe / RefUnwindSafe impls.
//
// UnwindSafe: auto-trait, no negative impl on UnsafeCell or RefCell.
// AppCell already implements UnwindSafe automatically. Rev 2's manual
// `unsafe impl UnwindSafe` was BOTH a compile error (E0199 — UnwindSafe
// is not an unsafe trait) AND logically unnecessary (auto-impl already
// covers it). The compile-time assertion in #[cfg(test)] locks the
// behavior as a regression guard.
//
// RefUnwindSafe: AppCell remains !RefUnwindSafe (auto-trait via
// UnsafeCell's negative impl). This matches pre-K07 RefCell<App>
// behavior. NO manual impl — adding one would be a deliberate widening
// beyond pre-K07 parity, and K07's mandate is to preserve, not extend.

#[derive(Clone, Copy, Debug)]   // rev 2 — Debug added (was missing in rev 1)
enum BorrowState {
    Free,
    Mut,
    Shared(NonZeroU32),
}

impl AppCell {
    // rev 2 — there is NO standalone `AppCell::new(app) -> Rc<Self>`
    // constructor because `App.this: Weak<AppCell>` requires
    // `Rc::new_cyclic` integration. The cell is constructed inline
    // in `App::new_app` via:
    //
    //     let cell = Rc::new_cyclic(|this: &Weak<AppCell>| AppCell {
    //         app: UnsafeCell::new(App {
    //             this: this.clone(),
    //             // … other fields …
    //         }),
    //         borrowed: Cell::new(BorrowState::Free),
    //         _not_send: PhantomData,
    //     });
    //
    // No public constructor is exposed. (rev 1 sketch had `pub(crate)
    // fn new` — removed because the cyclic-init pattern requires
    // Rc::new_cyclic at the call site.)

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

// rev 2 — NO #[track_caller] on Drop impls. Verified: Rust's Drop
// glue does not propagate caller Location; the attribute is a no-op
// on Drop::drop. (Rust Reference / RFC 2091.) Logging from Drop
// emits the Drop-glue's internal location, not the borrow callsite.
// Acceptable: Drop is symmetric to acquire; the acquire-side
// `borrow*` methods carry `#[track_caller]` and supply callsite info.
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
    use static_assertions::{assert_impl_all, assert_not_impl_any};

    // R2 — !Send + !Sync invariant (matches pre-K07 RefCell<App>).
    assert_not_impl_any!(AppCell: Send, Sync);
    assert_not_impl_any!(AppRef<'static>: Send, Sync);
    assert_not_impl_any!(AppRefMut<'static>: Send, Sync);

    // R3 — UnwindSafe is auto-impl'd (no negative impl on UnsafeCell or
    // RefCell in std). Lock the behavior so any future code change
    // accidentally introducing a !UnwindSafe field is caught at compile
    // time. Pre-K07 RefCell<App>: UnwindSafe (auto); post-K07
    // UnsafeCell<App>: UnwindSafe (auto). Identical.
    assert_impl_all!(AppCell: std::panic::UnwindSafe);

    // R3b — RefUnwindSafe is NOT impl'd (UnsafeCell has negative impl).
    // Pre-K07 RefCell<App>: !RefUnwindSafe (via UnsafeCell inside);
    // post-K07 UnsafeCell<App>: !RefUnwindSafe (directly). Lock the
    // negative side too.
    assert_not_impl_any!(AppCell: std::panic::RefUnwindSafe);
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
- **Task 14a.** **(K15 Limitation #6 closure — rev 3.)** Raw-pointer field-projection guards (one for `currently_updating_entity` in `update_entity`; one for `window_update_stack` in `update_window_id` and `open_window`). The guard struct holds `*mut FieldType` and `prev: FieldType`; Drop writes `prev` back through the pointer. Single `unsafe` block per guard's Drop. Property tests: `prop_currently_updating_entity_restored_after_panic`, `prop_window_update_stack_restored_after_panic`, `prop_pending_updates_zero_after_panic_through_update_entity`.

### Phase 4 (5 PARALLEL tasks)

- **Tasks 15-19.** `AsyncApp` / `TestAppContext` / `TestApp` / `HeadlessAppContext` / `VisualTestContext` — file-disjoint, parallel-safe.

### Phase 5 (sequential after Phase 4)

- **Tasks 20-24.** `elements/`, `subscription.rs`, `executor.rs` (test fixture only), `platform/*` (with K15 deferral comment updates), `examples/`.
- **Task 25.** Final scan — zero `BorrowMutError`, zero `TRACK_THREAD_BORROWS`, zero AppCell-derived `borrow*` hits via narrow-pattern grep.

## K15 contract preservation table

> **Rev 2 correction (BLOCKER A2 + B):** rev 1 referenced `WindowUpdateGuard::commit_pop` and `EntityUpdateGuard::enter` as pre-existing types. **They do not exist in the codebase.** K15's plan documented these RAII types but the actual implementation rejected them ("RAII guards … conflict with Rust borrow rules" per app.rs:2488-2491) and uses inline push/pop. Verified via `grep -r 'WindowUpdateGuard\|EntityUpdateGuard\|commit_pop' crates/flui-core/src/` returning **zero matches**.

| K15 contract element | Post-K07 status | Implementation site | Notes |
|---|---|---|---|
| `ReentryError::NestedWindowUpdate(WindowId)` | unchanged | app.rs:1594 (early-return check) | Pre-`update` early `Err` return; cell flag is orthogonal |
| `ReentryError::NestedEntityUpdate(EntityId)` | unchanged | app.rs:2469 (early-panic check) + `EntityMap::double_lease_panic` unification | Pre-`update` panic with structured Display |
| `ReentryError::ElementStateInUse { global_element_id, type_id }` | unchanged | window.rs:3155 area | `with_element_state` is its own panic shape |
| `ReentryError::PromptInProgress` | unchanged | window.rs:5142 area | `Window::prompt` Result shape preserved |
| `ReentryError::AppBorrowed` | **now produced DIRECTLY by `AppCell::try_borrow_mut`** | app/cell.rs (new) | `From<BorrowMutError>` impl DELETED (semver MAJOR — see §"Semver impact (rev 2)") |
| `ReentryMode { Strict, Loose }` | unchanged | reentrancy.rs | `PanicLikeUpstream` deferred decision: DROP per Q3 |
| `window_update_stack` inline push/pop | **panic-safety added via Task 14a** (raw-pointer guard) | app.rs:1605 push, 1611 pop (in `trail`); app.rs:1109 push, 1111 pop (in `open_window`) | rev 3: raw-pointer field-projection guard restores stack length on panic without catching/resuming through `App::update`'s frame. Steady-state inline pattern preserved |
| `currently_updating_entity` inline replace/restore | **remains inline** for the steady-state path; **panic-safety added via Task 14a** (raw-pointer field-projection guard) | app.rs:2497 replace, 2504 restore | rev 1 proposed `EntityScope { app: &mut App }` RAII — does not compile. rev 2 proposed `catch_unwind(AssertUnwindSafe(...))` — leaks `pending_updates` via `resume_unwind` through `App::update`'s frame (skips `finish_update`), AND clears the entity guard while leaving the entity slot leaked (worse than K15 baseline). **rev 3:** raw-pointer guard `Guard { ptr: *mut Option<EntityId>, prev }` with Drop running `unsafe { *self.ptr = self.prev }`. Holds `*mut`, not `&mut` — no borrow conflict with closure's `cx`. Drop runs on panic via standard stack-unwind; no `catch_unwind` needed; `pending_updates` semantics fully preserved |
| `EntityMap::double_lease_panic` unified Display | unchanged | app/entity_map.rs:142, 207 | Same |
| `cx.defer` / `Window::defer` escape hatches | unchanged | app.rs:1655, window.rs:1799 | Same |
| `observe_in` / `subscribe_in` callback discard | **unchanged** (silent-loss class — K15 documented as accepted) | context.rs:334, 363 | rev 1 omitted this row; rev 2 adds. Behavior: K15 logs `warn!` but `.unwrap_or(false)` discards the error. K07 preserves verbatim |
| `borrow_mut_error_converts_to_app_borrowed` test | **DELETED in same commit as `From<BorrowMutError>`** (Task 9, commit 5) | reentrancy.rs:253-259 | rev 1 missed this. The test exercises the impl being deleted; deletion is mandatory, not optional |
| K15 11 reentrancy tests | **all 11 pass under K07** | Various locations | Required by plan Task 26 done criterion 11 |

## Decisions on Q1-Q12 (all resolved — rev 2 amendments)

| Q | Decision | Rationale |
|---|---|---|
| Q1 | Candidate B locked | Phase 1 spike + 3-agent research convergence |
| Q2 | **Keep panic shape on ALL FIVE `as_mut` sites; structure Display via `ReentryError::AsyncContextAsMut`** (rev 2: rev 1 only covered async_context.rs:73 — adversarial review found 4 more sites). Sites: `async_context.rs:73` (AsyncApp), `async_context.rs:412` (AsyncWindowContext), `test_context.rs:68` (TestAppContext), `headless_app_context.rs:230` (HeadlessAppContext), `visual_test_context.rs:430` (VisualTestContext). All 5 use `std::panic::panic_any(ReentryError::AsyncContextAsMut)` so the panic payload is `Box<ReentryError>` (typed) rather than `String` (formatted Display). `catch_unwind` callers can `downcast_ref::<ReentryError>()` to distinguish. **rev 3 amendment**: `AsyncContextAsMut` Display rephrased context-agnostic ("`AppContext::as_mut` is forbidden in async/test/headless context types; use the equivalent `update(...)` method to acquire mutable access") so it is accurate when raised from `HeadlessAppContext`, `TestAppContext`, `VisualTestContext`, or `AsyncWindowContext`. Document the variant in `ReentryError`'s rustdoc as a "Panic-only variant" (never returned from a Result-bearing method). | 5-site coverage; typed panic payload via `panic_any`; trait shape preserved; misleading "AsyncApp" message replaced with context-agnostic phrasing |
| Q3 | DROP `PanicLikeUpstream`; document obsolete | Cell flag *is* `AppBorrowed`; no `BorrowMutError` to mimic |
| Q4 | **`AsyncApp::app()` (PRIVATE) widens to `Result<Rc<AppCell>, ReentryError::AppGoneAway>`. Public AsyncApp methods retain current panic semantics by absorbing the error inside the method body** — the cascade is NOT propagated to public method signatures. Specifically: (a) `AsyncApp` methods that ALREADY return `Result<T>` (e.g., `update_window`, `read_window`) propagate via `?`; (b) methods that return `T` (e.g., `new`, `update_entity`, `read_global`) use `match self.app() { Ok(rc) => …, Err(e) => std::panic::panic_any(e) }` — typed `Box<ReentryError>` payload, NOT `panic!("{}", e)` (which would produce `String`). `catch_unwind` callers can `downcast_ref::<ReentryError>()` to distinguish `AppGoneAway`. **rev 3 amendment**: replaced `unwrap_or_else(\|e\| panic!("{}", e))` with `panic_any(e)` to preserve type identity at the unwind boundary. **Like Q2's `AsyncContextAsMut`, `AppGoneAway` becomes a panic-payload variant for non-Result methods AND a returnable error for Result methods.** Document the dual nature in rustdoc. **Known limitation (rev 3 documented):** panic-source location for non-Result methods points to the `panic_any` line in `async_context.rs`, not the caller's site. `#[track_caller]` does not propagate through the absorbing match arm. Acceptable parity with pre-K07 `expect("...")` which had the same property. | Preserves public API; typed panic payload via `panic_any` where un-widenable; structured Result where already widenable |
| Q5 | `log::trace!` (project style) | Project uses `log` crate; new feature flag overkill |
| Q6 | **PR-blocking scoped Miri** for `app/cell.rs` only — `cargo +nightly miri test -p flui-core cell` with default Stacked Borrows. **Tree Borrows (`MIRIFLAGS=-Zmiri-tree-borrows`) added as a separate non-blocking gate** (cargo-miri logs as scheduled CI job, not PR-blocking). Rationale: Tree Borrows is the active research successor; UnsafeCell projection behavior is changing. Run both during dev, gate only the stable model. | Soundness gate for new `unsafe`; bounded cost |
| Q7 | NO split — single PR | Candidate B is signature-compatible; no review-overhead benefit from split |
| Q8 | Keep `AppContext::as_mut` trait shape | Widening breaks 5 implementors + downstream |
| Q9 | `UnsafeCell<App>` by-value via `Rc::new_cyclic` preserves Drop. **Manual `unsafe impl UnwindSafe for AppCell {}` required** (rev 2 — UnsafeCell<T>: !UnwindSafe by default; without manual impl, K07 regresses pre-K07 RefCell<App>: UnwindSafe). | Matches K12 invariant; restores pre-K07 UnwindSafe behavior |
| Q10 | `Application: Clone` remains absent | Out of K07 scope |
| Q11 | K07-only CHANGELOG entry; K99/K15 backfill in separate PR | PR scope discipline |
| Q12 | `K07 — AppCell removal (Phase 0-K, third spec)` | K15 PR #9 style; architectural change |

## Open questions

**EMPTY.** All 12 open questions resolved with documented rationale in §"Decisions on Q1-Q12 (all resolved)" above. Adversarial review (Task 6, completed before rev 2) raised 8 BLOCKERs / 12 MAJORs / 10 MINORs — all absorbed into spec rev 2 with no new open questions.

## Semver impact (rev 2 — explicit per adversarial review)

K07 introduces multiple semver breakages. cargo-semver-checks (roadmap R2, post-K07) treats `pub` items as part of the public surface even when `#[doc(hidden)]` (as of v0.34+). The following breaks are documented for the future R2 spec:

| Change | Type | Notes |
|---|---|---|
| `AppCell::try_borrow_mut` return type widens from `Result<_, BorrowMutError>` to `Result<_, ReentryError>` | **MAJOR** | `AppCell` is `pub` (doc-hidden) — counted by cargo-semver-checks |
| `AppCell` struct internals change (field `app: RefCell<App>` → `app: UnsafeCell<App>` + new fields) | **MAJOR (layout-level)** | Anyone using `transmute` or `size_of::<AppCell>()` breaks; vanishingly unlikely in practice |
| `AppRef<'a>` / `AppRefMut<'a>` internal field change (wraps `Ref/RefMut` → wraps `&App/&mut App`) | **MAJOR (layout-level)** | Same — both are `pub` (doc-hidden) |
| `impl From<std::cell::BorrowMutError> for ReentryError` DELETED | **MAJOR** | This was a `pub` impl on a `pub` enum. Downstream code calling `ReentryError::from(some_borrow_mut_error)` breaks. flui-core not yet published, so impact is internal. CHANGELOG must explicitly flag |
| `ReentryError::AppGoneAway` new variant | **MINOR** (forward-compatible) | Enum is `#[non_exhaustive]` |
| `ReentryError::AsyncContextAsMut` new variant | **MINOR** (forward-compatible) | Same |
| `AsyncApp::app()` (PRIVATE) widens return type | **NONE** (private method) | Cascade absorbed inside AsyncApp; public methods keep panic semantics for non-Result cases (Q4) |
| Public AsyncApp methods that return `T` (e.g., `new`, `update_entity`) | **NONE** (semantics preserved via panic-Display) | Q4 keeps panic for non-Result methods; Display Text changes (`expect("app was released…")` → `panic!("{}", AppGoneAway)`) — affects `catch_unwind` payload type |
| `panic!` payload type changes from `&'static str` and `BorrowMutError` to `Box<ReentryError>` (via `panic_any`) | **OBSERVABLE** | Any `catch_unwind` that downcasts payload to `BorrowMutError` or specific `&str` breaks. K07 contract (rev 3): panic payloads are typed `Box<ReentryError>` for K07-introduced panic sites; old payloads (`&'static str`, `BorrowMutError`) gone. `catch_unwind` callers should `downcast_ref::<ReentryError>()` for K07 panics |

**CHANGELOG (Task 32a) MUST flag the From<BorrowMutError> deletion and try_borrow_mut return-type widening as semver MAJOR breaks.** Migration guide (Task 32b) MUST show:
- Before/after for any code calling `ReentryError::from(borrow_mut_error)`.
- `catch_unwind` callers that downcast payload — must switch from downcasting `BorrowMutError` / `&'static str` to downcasting `ReentryError` (typed via `panic_any`).
- `From<BorrowMutError> for ReentryError` deletion is acceptable as a pre-1.0 unpublished crate decision (no third-party downstream); no `#[deprecated]` cycle needed.

## Re-export rule (rev 2 — explicit per adversarial review)

`crates/flui-core/src/app.rs` MUST use `pub use cell::{AppCell, AppRef, AppRefMut}` (not `pub(crate) use`). Rationale: `lib.rs:125 pub use app::*` does not re-export `pub(crate)` items. The `HeadlessAppContext.app: Rc<AppCell>` field (and analogous `pub Rc<AppCell>` fields on `TestAppContext`, `VisualTestContext`) are `pub` and name `AppCell` in their type — external consumers of the `test-support` feature need `flui_core::AppCell` to be reachable. Plan Task 8 corrected to require `pub use`.

## `pub Rc<AppCell>` on test contexts — preserved (rev 2 — explicit per adversarial review)

`HeadlessAppContext.app`, `TestAppContext.app`, `VisualTestContext.app` are all `pub Rc<AppCell>` (gated on `#[cfg(any(test, feature = "test-support"))]`). Downstream test code may write `cx.app.borrow_mut()` directly. Post-K07 this continues to work via `Deref<Target=App>` on `AppRefMut<'_>`. The new `AppCell` keeps the same method names; downstream test-support consumers see the same callable surface. **Risk: low** (the test-support API is internal-discipline only, no third-party crate exists yet).

## `unsafe` audit (rev 3 — re-corrected)

| Block | Location | SAFETY justification | Miri test |
|---|---|---|---|
| 1 | `try_borrow` `let app_ref = unsafe { &*self.app.get() }` | `borrowed` flag transitioned to `Shared(_)` before access; no `Mut` coexists; `app_ref` lifetime ≤ `AppRef::Drop` ≤ cell lifetime; one root reference; Stacked Borrows: projection is taken from the unique root reference `cell.app` with no aliased `&mut` from a different root in scope. | `prop_borrow_then_borrow_drop_releases` |
| 2 | `try_borrow_mut` `let app_mut = unsafe { &mut *self.app.get() }` | `borrowed` flag transitioned to `Mut`; no `Shared` or `Mut` coexists by enum exhaustiveness; lifetime same as #1; one root reference. | `prop_borrow_mut_then_borrow_mut_returns_app_borrowed` |
| 3* | `Guard { ptr, prev }` raw-pointer field-projection in Task 14a (`update_entity` and `update_window_id`) | `*self.ptr = self.prev` in `Drop`. SAFETY: `ptr: *mut Option<EntityId>` is derived from `&mut cx.currently_updating_entity` where `cx: &mut App` is alive for the entire enclosing `self.update(\|cx\| { … })` closure body; the guard is dropped at the end of the closure scope (or during unwind), strictly before `cx` exits scope. No aliased reference exists because the guard holds only the raw pointer, not a borrow. Single-threaded `!Send + !Sync` cell — no concurrent access. | `prop_currently_updating_entity_restored_after_panic` |

**\*Block 3 lives in `app.rs`, not `app/cell.rs`** — it's part of Task 14a's panic-safety fix, not the cell primitive. Counted in K07's total `unsafe` audit but in a different module. Two analogous guards exist (one for `currently_updating_entity` in `update_entity`, one for `window_update_stack` in `update_window_id` — same SAFETY argument, distinct callsites).

**Two `unsafe` blocks in `app/cell.rs`** (the cell primitive itself); **two `unsafe` blocks in `app.rs`** (Task 14a panic-safety guards). Total: 4 `unsafe` blocks across K07 implementation, all single-line `*self.ptr` writes or `&mut *cell.app.get()` projections.

**NO `unsafe impl` blocks for auto-traits.** Rev 2 mistakenly added `unsafe impl UnwindSafe` / `unsafe impl RefUnwindSafe`; both are compile errors (E0199 — these traits aren't unsafe) AND unnecessary (UnwindSafe is auto-impl'd; RefUnwindSafe stays `!` to match pre-K07).

Saturation overflow on `BorrowState::Shared(u32::MAX)` is a `match` arm returning `Err(ReentryError::AppBorrowed)` — NOT `unsafe`.

The `AppRef::Drop` and `AppRefMut::Drop` impls contain NO `unsafe` blocks — they only manipulate the safe `Cell<BorrowState>` API.

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
- **Adopting `qcell::TLCell` post-K07.** If hand-rolled cell maintenance becomes burdensome (e.g., new `unsafe` blocks proliferate), `qcell::TLCell` is a **functional alternative requiring API surface changes** (rev 2 — rev 1 wrongly called it "drop-in replacement"). `TLCellOwner<Marker>` is an ambient capability type, not storable as `Weak<AppCell>` — adopting it would re-shape the `App.this` back-pointer pattern. Re-evaluate after K05/K01 land if maintenance pressure mounts.

## Decision log

### Revision 1 (2026-05-09 — this document)

Phase 1 spike + 3-agent research dispatch resulted in Candidate B lock. Rejected alternatives:
- Candidate A (pass-through `&mut App`): DEFER — ~1500-2500 LoC migration, multi-week. Mixing with K07 violates PR scope.
- Candidate C (GhostCell branded `'id`): REJECT — HRTB poisons trait objects (the 7 callback typedefs at app.rs:243-250); `AsyncApp` cannot exist with branded `'id`; widget authors writing custom `Element` impls see `'id` in trait method signatures (ecosystem barrier).
- `qcell` / `ghost-cell` dependency: REJECT — Phase 0-K minimizes deps. Hand-rolled cell is cheaper for the same functional power.
- Sharding (Slint/Dioxus/Floem/Servo pattern): OUT OF SCOPE. K06 territory.

All 12 open questions (Q1-Q12) resolved with rationale in §"Decisions on Q1-Q12 (all resolved)".

### Revision 2 (2026-05-09 — post-Task 6 adversarial review)

Three reviewers dispatched in parallel; total 8 BLOCKERs / 12 MAJORs / 10 MINORs absorbed.

**Cross-confirmed BLOCKERs (multi-reviewer agreement):**

1. **`WindowUpdateGuard` / `EntityUpdateGuard` / `commit_pop` types do not exist** (arch + verified via grep returning zero matches in `crates/flui-core/src/`). K15's plan documented these RAII types, but the implementation rejected them per the inline comment at app.rs:2488-2491 ("RAII guards conflicted with Rust borrow rules"). K15 uses inline push/pop on `window_update_stack` (app.rs:1605, 1611) and inline replace/restore on `currently_updating_entity` (app.rs:2497, 2504). Spec rev 1 K15 contract preservation table FABRICATED these types. **Rev 2 fix:** preservation table corrected to reflect actual inline patterns.

2. **`UnwindSafe` claim INVERTED** (arch + api). Pre-K07: `RefCell<App>: UnwindSafe` UNCONDITIONALLY (std special-cases). Post-K07: `UnsafeCell<App>: !UnwindSafe` UNCONDITIONALLY (std special-cases). Rev 1 claimed "AppCell: UnwindSafe IFF App: UnwindSafe" — wrong direction. **Rev 2 fix:** manual `unsafe impl UnwindSafe for AppCell {}` and `unsafe impl RefUnwindSafe for AppCell {}` required, with safety comment mirroring std's RefCell impl. Two new entries in unsafe audit (count 3 → 4).

3. **`EntityScope { app: &'a mut App }` does not compile** (arch + migration). The proposed Task 14a RAII guard would require a `&mut App` borrow that aliases the closure's `cx: &mut App` — same conflict K15 had. **Rev 2 fix:** Task 14a re-scoped from `EntityScope` RAII to `std::panic::catch_unwind(AssertUnwindSafe(\|\| { … }))` pattern around the closure body; field restoration runs after catch.

4. **Test `borrow_mut_error_converts_to_app_borrowed` orphans** at reentrancy.rs:253-259 (migration + api). Test exercises the `From<BorrowMutError>` impl being deleted. Plan Task 9 said "if any" — must be MANDATORY. **Rev 2 fix:** preservation table explicitly lists test for deletion in same commit as From impl (commit 5).

**Unique BLOCKERs:**

5. **`#[track_caller]` on `Drop` is no-op** (api). Rust drop glue does not propagate caller location. Rev 1 plan Task 8 required it. **Rev 2 fix:** Drop impls explicitly do NOT carry `#[track_caller]`; only the acquire-side `borrow*` methods do. Documented in code comments and §"Type surface".

6. **`AsyncApp::app()` cascade not enumerated** (api). Method is private but widening Result cascades to 10+ public AsyncApp methods. **Rev 2 fix:** Q4 amended — private widening absorbed inside AsyncApp; public methods retain current panic semantics via `unwrap_or_else(|e| panic!("{}", e))` for non-Result cases, propagate `?` for Result cases. `AppGoneAway` is dual-purpose: returnable in Result paths, panic-Display in T-return paths.

7. **Four other `as_mut` panic sites** not covered by Q2 (migration). Sites: async_context.rs:412 (AsyncWindowContext), test_context.rs:68 (TestAppContext), headless_app_context.rs:230 (HeadlessAppContext), visual_test_context.rs:430 (VisualTestContext). **Rev 2 fix:** Q2 expanded to all 5 sites; plan Task 15 expanded to migrate all 5.

8. **`AppCell::new` standalone constructor incompatible with `Rc::new_cyclic`** (arch + migration). `App.this: Weak<AppCell>` requires cyclic init; rev 1 sketch showed `pub(crate) fn new(app) -> Rc<Self>` which cannot set `App.this`. **Rev 2 fix:** removed standalone constructor; spec sketch shows `Rc::new_cyclic` integration in `App::new_app` directly.

**MAJOR fixes:**

- M9 (api). `From<BorrowMutError>` deletion is semver MAJOR break, not "becomes redundant". Rev 2 added §"Semver impact (rev 2 — explicit per adversarial review)" with full table. CHANGELOG (Task 32a) flags explicitly.
- M10 (api). Re-export must be `pub use cell::{...}`, not `pub(crate) use`. Rev 2 added §"Re-export rule (rev 2 — explicit per adversarial review)". Plan Task 8 corrected.
- M11 (migration). `pub Rc<AppCell>` on test contexts exposes raw cell access. Rev 2 added §"`pub Rc<AppCell>` on test contexts — preserved" — works via Deref preservation; risk: low.
- M12 (api). `AsyncContextAsMut` variant is panic-only Display, never returnable in `Result`. Rev 2 documents this dual-nature in `ReentryError` rustdoc.
- M13 (migration + api). `BorrowState` lacks `Debug` for `unreachable!("{:?}", other)`. Rev 2 added `#[derive(Clone, Copy, Debug)]`.

**MINOR fixes:**

- TLCell "drop-in replacement" softened to "functional alternative requiring API surface changes" (Future considerations).
- "3 unsafe blocks total" corrected to "4 unsafe blocks total" (audit table now shows 2 projection blocks + 2 unsafe impl blocks).
- `migration-risk-adversary` finding #22 (Cargo.lock policy vs static_assertions) is a FALSE POSITIVE — `static_assertions v1.1.0` is already in lockfile transitively via postage v0.5.0 (verified by `rust-api-migration-auditor`). No policy violation.

**Known false positives / disputed findings:**

- `migration-risk-adversary` BLOCKER M2 (`read_global`/`try_read_global` use `borrow_mut` semantically wrong) — partially valid: under K07 with `BorrowState`, these sites COULD be `try_borrow()` (shared) for cleaner semantics. Spec rev 2 calls this out as a follow-up cleanup (NOT blocking K07). Documented in plan Task 15 as optional refinement.

- `flui-arch-reviewer` BLOCKER A1 (Key Principle #8 paint/dispatch hot path) — partially valid: spec rev 2 keeps the permissive interpretation BUT adds plan Task 14b: explicit grep audit for `try_borrow_mut` reachability from `Window::draw` / `dispatch_event`. If grep finds reachable sites, escalate to Known Limitation. If zero, spec stays.

**Net rev 2 changes:**

- Spec sections rewritten/added: K15 contract preservation table (corrected), Auto-trait invariants (UnwindSafe inverted), Type surface (Rc::new_cyclic, no AppCell::new, Debug on BorrowState, no #[track_caller] on Drop), unsafe audit (4 blocks not 3), Q2 (5 sites not 1), Q4 (cascade policy specified), Q9 (manual UnwindSafe impl), §"Semver impact (rev 2)", §"Re-export rule (rev 2)", §"`pub Rc<AppCell>` on test contexts — preserved".
- Plan changes: Task 8 (Drop no #[track_caller], Debug on BorrowState, manual UnwindSafe impl, pub use not pub(crate) use); Task 9 (mandatory test deletion); Task 14a (catch_unwind + AssertUnwindSafe pattern, NOT EntityScope); Task 14b (NEW — paint/dispatch hot-path audit); Task 15 (5 as_mut sites); Task 26 (added catch_unwind tests).

### Revision 3 (2026-05-09 — second adversarial review absorbed)

Rev 2 itself reviewed by 4 agents (3 adversarial: `flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`; 1 quality: general-purpose). Rev 2 introduced new BLOCKERs while fixing rev 1's. **4 reviewers cross-confirmed convergence on these critical fixes:**

**BLOCKER A (api-auditor definitive — fact-corrects rev 1+rev 2):**
- `unsafe impl UnwindSafe` is `error[E0199]` — `UnwindSafe` is NOT an unsafe trait.
- The "UnwindSafe regression" narrative is false. api-auditor verified `core/src/panic/unwind_safe.rs:181-202` (1.95 toolchain): `UnwindSafe` is `pub auto trait` with NO negative impl on `UnsafeCell`. Only `RefUnwindSafe` has a negative impl on `UnsafeCell`. There IS NO regression.
- Pre-K07 `RefCell<App>: UnwindSafe` (auto), `!RefUnwindSafe` (via UnsafeCell inside).
- Post-K07 `UnsafeCell<App>: UnwindSafe` (auto), `!RefUnwindSafe` (UnsafeCell negative impl).
- **rev 3 fix:** Drop the manual `unsafe impl UnwindSafe` block entirely. Drop the `unsafe impl RefUnwindSafe` block (would have widened pre-K07 contract — out of K07 mandate). Add compile-time `assert_impl_all!(AppCell: UnwindSafe)` and `assert_not_impl_any!(AppCell: RefUnwindSafe)` regression guards.

**BLOCKER B (arch + migration cross-confirmed):**
- Rev 2's `catch_unwind(AssertUnwindSafe(...))` pattern in Task 14a leaks `pending_updates` via `resume_unwind` propagating through `App::update`'s frame, which skips `finish_update`. After ONE caught panic, `pending_updates` stays at +1 forever; `flush_effects` guard `if pending_updates == 1` never fires; effects queue freezes. Silent permanent regression.
- Additionally: catch_unwind clears the entity guard while leaving the entity slot leaked — worse than K15's documented behavior.
- **rev 3 fix:** Replace `catch_unwind(AssertUnwindSafe(...))` pattern with raw-pointer field-projection guard pattern (suggested by arch-reviewer's MAJOR 2). The guard holds `*mut FieldType + prev: FieldType` — Drop runs `*self.ptr = self.prev` during stack unwind without crossing `App::update`'s frame. `pending_updates` semantics fully preserved; no `catch_unwind` / `resume_unwind`. New `unsafe` block per guard's Drop, but bounded and SAFETY-commented.

**BLOCKER C (arch + migration cross-confirmed):**
- Compile-error window between Task 9 (deletes `From<BorrowMutError>`) and Task 15 (updates `async_context.rs:96 .map_err(ReentryError::from)`).
- **rev 3 fix:** Plan reordering — line 96 update moved to Task 9 (atomic with the deletion). Task 15 keeps the bulk AsyncApp migration but drops the line-96 obligation.

**BLOCKER D (api-auditor):**
- Q4 `panic!("{}", e)` produces `String` payload; `catch_unwind.downcast_ref::<ReentryError>()` returns `None`. Type identity lost at unwind boundary.
- **rev 3 fix:** Replace `panic!("{}", e)` with `std::panic::panic_any(e)` for typed `Box<ReentryError>` payload. Same change for Q2 (`AsyncContextAsMut` panics).

**MAJOR fixes (rev 3):**
- Q2 `AsyncContextAsMut` Display rephrased context-agnostic ("`AppContext::as_mut` is forbidden in async/test/headless context types") instead of misleading "AsyncApp" wording.
- "4 unsafe blocks" → 4 (corrected accounting): 2 in `app/cell.rs` (projections) + 2 in `app.rs` (Task 14a guards). NO `unsafe impl` blocks (rev 2 mistakenly added two — both compile errors).
- `static_assertions` MUST be added to `[dev-dependencies]` of `crates/flui-core/Cargo.toml` explicitly (in lockfile transitively but not in manifest).
- Miri PR-blocking requires CI job — added explicit task: append step to `.github/workflows/ci.yml` with `dtolnay/rust-toolchain@nightly` + `cargo +nightly miri test -p flui-core cell`.
- `#[track_caller]` does not propagate through `panic_any(e)` for non-Result methods. Documented as known DX limitation matching pre-K07 `expect()` parity.

**MINOR fixes (rev 3):**
- "RAII guards now feasible" → "raw-pointer field-projection guards" (Goal #4, migration plan, all references).
- `.expect_or_panic_with_display()` invented pseudocode → `match self.app() { Ok(rc) => rc, Err(e) => panic_any(e) }` actual idiom.
- `AsyncContextAsMut` "Panic-only variant" annotation explicitly added to code sketch rustdoc.
- Goals/Current-state/Risks "3-5 unsafe blocks" reconciled to "4 (2 cell projections + 2 Task 14a guards)".
- `From<BorrowMutError>` pre-1.0 unpublished justification noted explicitly.
- `debug_assert!` in `Drop` impls preserved BUT explicitly noted as abort-on-double-panic-during-unwind risk; mitigation: `#[cfg(debug_assertions)]` gate so production builds skip the assertion.

**Quality findings absorbed (general-purpose review):**
- Saturation guard on `BorrowState::Shared(u32::MAX)` retained as defensive code, but corresponding proptest dropped (test was theatre per general-purpose review).
- Audit pending: investigate whether ANY callsite genuinely needs `try_borrow` (shared) vs `try_borrow_mut` (exclusive). If zero, drop `BorrowState::Shared` entirely. Added Task 7a (NEW).
- Criterion bench `bench_borrow_mut_acquire_release` — added Task 26b (NEW).
- `App.this: Weak<AppCell>` is K06 shard-resistance point — added to Future Considerations.
- Xilem thread-local pattern — added one-line note in Future Considerations alongside K06 sharding.

**False positives (rev 2 rejected upon rev 3 verification):**
- Rev 2 claimed "Cargo.lock policy violation" with `static_assertions` — actually transitively present (verified by api-auditor).
- Rev 2 said `#[track_caller]` on Drop is no-op — confirmed correct, kept the rev 2 fix.

**Net rev 3 changes:**
- Spec sections rewritten: Auto-trait invariants (no manual impls; auto-trait analysis), Type surface (no unsafe impl), unsafe audit (4 blocks all single-line projections/writes), K15 contract preservation table (raw-pointer guards), Q2 + Q4 (panic_any), §"Semver impact" (typed payload), §"Decisions on Q1-Q12" Q9 (NO manual UnwindSafe impl).
- Plan changes: Task 8 (no unsafe impl, no manual UnwindSafe; assertion-based; debug_assert! cfg-gated); Task 9 (line-96 update atomic); Task 14a (raw-pointer guards, NOT catch_unwind); Task 14b (unconditionally PR-blocking, audit output committed); Task 26 (panic_any tests, pending_updates tests, Display tests for new variants); NEW Task 7a (try_borrow shared-call audit); NEW Task 8a (Cargo.toml dev-deps); NEW Task 26b (criterion bench); NEW Task 27a (Miri CI job).
- Open Questions: still EMPTY — Q1-Q12 all resolved with rev 3 amendments.

51 → **55 tasks** (+1 try_borrow audit, +1 Cargo.toml, +1 criterion bench, +1 Miri CI job).
