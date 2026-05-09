---
id: K15
title: Re-entrancy contract
status: implemented
date: 2026-05-09
phase: 0-K (Kernel Cleanup)
roadmap-track: K-track critical chain (second spec, after K99)
breaking: yes (Window::prompt + AsyncWindowContext::prompt signatures widen; entity-side panic message text changes)
---

# K15 — Re-entrancy contract

## Context

flui-v2 is a hard fork of `gpui-ce`. Per `.ai-factory/RESEARCH.md` and `.ai-factory/ROADMAP.md`, Phase 0-K (Kernel Cleanup) is the sequenced repayment of structural debt in `flui-core` before the Framework tier (Phase II-F) can be built on top.

K15 is the second spec in the critical chain (K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04). The critical chain runs sequentially because each step's design freezes the surface that the next step builds on. K15 publishes the re-entrancy contract that K07 (AppCell removal) must continue to honor under whatever borrow primitive K07 chooses (token-based, RwLock, etc.).

Pre-K15 state was undefined: `update_window` inside `update_window` panicked with raw `BorrowMutError`; `update_entity` inside an observer panicked with the unstructured text `"cannot update <T> while it is already being updated"` produced by `EntityMap::double_lease_panic`; `with_element_state` recursive calls panicked via bare `expect("reentrant…")`; double `Window::prompt` panicked via `unreachable!`; `AsyncWindowContext::prompt` swallowed errors silently via `.unwrap_or_else(|_| oneshot::channel().1)` and produced dead receivers. Two existing platform comments (mac, windows) papered over individual hazards but did not name a contract.

K15 names the contract, gives it a structured error type, unifies the panic message texts that survive (entity re-entry stays panic-shape because the trait signature `R` cannot widen), preserves the existing `Effect::Defer` queue path as the documented escape hatch, and adds behavioral test coverage.

## Goals

1. Public `flui_core::reentrancy` module exposing `ReentryError` (`#[non_exhaustive]`, `thiserror::Error`) and `ReentryMode { Strict, Loose }` (`#[non_exhaustive]`).
2. Detect same-window `update_window` re-entry at `App::update_window_id` and return `Err(anyhow{ ReentryError::NestedWindowUpdate })` BEFORE taking the window from storage.
3. Detect same-entity `update_entity` re-entry at `App::update_entity` and panic with `ReentryError::NestedEntityUpdate(_)` Display (ROADMAP K15 explicitly authorizes "panic with structured error").
4. Unify `EntityMap::double_lease_panic` to use `ReentryError::NestedEntityUpdate` Display so multi-entity cycles (`A → B → A`) produce the same message as direct `update_entity(A, A)` re-entry.
5. Replace `Window::with_element_state`'s bare `expect("reentrant…")` with structured `ReentryError::ElementStateInUse { global_element_id, type_id }` Display (panic-shape preserved; `with_element_state` does not return `Result`).
6. Widen `Window::prompt` from `oneshot::Receiver<usize>` to `Result<oneshot::Receiver<usize>, ReentryError>`; widen `AsyncWindowContext::prompt` to `anyhow::Result<oneshot::Receiver<usize>>` and stop swallowing errors.
7. Route `AsyncApp::run_update`'s `try_borrow_mut()?` through `From<BorrowMutError> for ReentryError` so the anyhow chain carries `ReentryError::AppBorrowed` Display.
8. Update three platform deferral comments (mac:498-503, mac:1253-1257, windows:449-454) to reference the K15 contract.
9. Behavioral test coverage for the new structured panic shapes.

## Non-goals

- Does NOT remove `AppCell` (K07).
- Does NOT change Element trait signatures (K05).
- Does NOT introduce `BuildOwner` / `PipelineOwner` (K06).
- Does NOT touch `Render::&mut self` (K03).
- Does NOT define `setState` (Phase II-F / SF05). The contract `setState` will adhere to is documented in engine terms here so SF05 can publish it as-spec.
- Does NOT refactor `SubscriberSet` internals — the existing snapshot pattern is kept and codified.
- Does NOT touch gesture re-entry (A7-audit-closed surface).
- Does NOT add a new `Effect` variant. The existing `Effect::Defer` is the queue admission point.
- Does NOT widen `update_window` or `update_entity` trait signatures.
- Does NOT change `Platform::prompt` trait or its 7 platform implementations.
- Does NOT add a `legacy-reentry-panics` Cargo feature or `ReentryMode::PanicLikeUpstream` variant — deferred to K07 (see Decision log).
- Does NOT add explicit `impl From<ReentryError> for anyhow::Error` — anyhow's blanket impl handles `E: Error + Send + Sync + 'static` automatically; an explicit impl would conflict.
- Does NOT structure-ify the 10+ remaining `borrow_mut()` sites in `AsyncApp` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182). K07 redesigns this surface.

## Current state (post-K15)

| Aspect | State | File |
|---|---|---|
| `ReentryError` type | published, `#[non_exhaustive]`, derives `thiserror::Error` | `crates/flui-core/src/reentrancy.rs` |
| `ReentryMode` enum | published, `#[non_exhaustive]`, derives `Default = Loose` | same |
| `App::set_reentry_mode` setter | public | `crates/flui-core/src/app.rs` |
| `App::currently_updating_entity` field | `pub(crate) Option<EntityId>` | same |
| `App::reentry_mode` field | `pub(crate) ReentryMode`, init `Strict` in `cfg(test)` | same |
| Same-window `update_window` re-entry | `Err(anyhow{ ReentryError::NestedWindowUpdate(id) })` BEFORE `windows.get_mut(id)?.take()` | `app.rs:1585-1595` |
| Same-entity `update_entity` re-entry | structured panic with `ReentryError::NestedEntityUpdate(_)` Display | `app.rs:2422-2434` |
| `EntityMap::double_lease_panic` | unified Display via `ReentryError::NestedEntityUpdate(entity_id)` | `app/entity_map.rs:207-220` |
| `Window::with_element_state` recursive panic | `panic!("{}", ReentryError::ElementStateInUse { global_element_id, type_id })` | `window.rs:3155-3169` |
| `Window::prompt` signature | `Result<oneshot::Receiver<usize>, ReentryError>` | `window.rs:5157-5202` |
| `AsyncWindowContext::prompt` signature | `anyhow::Result<oneshot::Receiver<usize>>` (was: swallowing receiver) | `app/async_context.rs:345-373` |
| `AsyncApp::run_update` chain | `try_borrow_mut().map_err(ReentryError::from)?` | `app/async_context.rs:90` |
| Platform deferral comments | reference K15 contract | mac/platform.rs (×2), windows/platform.rs |
| Test coverage | 11 new tests (6 type-level, 5 behavioral via `TestApp`) | `crates/flui-core/src/reentrancy.rs` |

## Design

The full operational design is captured in `.ai-factory/plans/feature-K15-reentrancy-contract.md` §"Design" (contract matrix per callback class, runtime modes, escape hatches). What follows here is the spec-level summary plus the decision log.

### Per-callback contract (summary)

The contract sorts callback classes into three buckets:

- **Synchronous** — re-entry for a *different* target is allowed (`update_window(other_window)`, `update_entity(different_entity)`, observer reading sibling entity).
- **Forbidden** — re-entry for the *same* target produces `ReentryError`. `update_window` returns `Err`; `update_entity` panics with structured Display; `with_element_state` panics with structured Display; `Window::prompt` returns `Err`.
- **Queued** — `cx.defer(|cx| ...)` and `window.defer(cx, |w, cx| ...)` admit work to the existing `Effect::Defer` queue. Always allowed; never produce `ReentryError`. These are THE escape hatches.

The full table is in the source-of-truth rustdoc on `flui_core::reentrancy` and in the plan §"The contract matrix".

### `ReentryError` variants

```rust
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ReentryError {
    NestedWindowUpdate(WindowId),
    NestedEntityUpdate(EntityId),
    ElementStateInUse { global_element_id: GlobalElementId, type_id: TypeId },
    PromptInProgress,
    AppBorrowed,
}
```

All variants carry types that are `Send + Sync + 'static`, so `ReentryError` satisfies the bounds anyhow's blanket `From` impl requires.

### `ReentryMode`

```rust
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReentryMode {
    Strict,        // logs at error! level
    #[default]
    Loose,         // logs at warn! level (default in release)
}
```

`App` initializes `reentry_mode = if cfg!(test) { Strict } else { Loose }`. `App::set_reentry_mode` is a public setter.

## Decision log

K15 went through three revisions during planning + adversarial review (3 agents). Each narrowing is recorded here so future readers (especially K07's author) understand why certain shapes were rejected.

**Rejected: `Effect::ScheduledUpdate` variant** (rev-1 → rev-2 dropped). A new `Effect` variant for queued updates was incompatible with the generic `T`/`R` return types of `update_window` / `update_entity` — synthesizing a default value at admission time is impossible without `T: Default`. The existing `Effect::Defer` (closure-based, returns nothing) covers all legitimate use cases via `cx.defer`. Inventing parallel queue infrastructure overlapping with `Defer` was the kind of premature design ROADMAP §"Anti-goals" warns about.

**Rejected: redundant App fields** (rev-1 → rev-2 dropped). `currently_updating_window: Option<WindowId>` was redundant with the existing `window_update_stack: Vec<WindowId>`. `update_depth: u8` was redundant with the existing `pending_updates: usize`.

**Rejected: `tracing::*` macros** (rev-1 → rev-2 changed to `log::*`). Project uses the `log` crate (see `flui-core/Cargo.toml:86`). `tracing` standardization is roadmap A4 — out of K15 scope.

**Rejected: RAII `WindowUpdateGuard` and `EntityUpdateGuard` structs** (rev-3 plan → implementation: simpler inline checks). The RAII shape was designed during planning to provide panic-safety on `App::window_update_stack` and `App::currently_updating_entity`. In implementation, RAII guards conflicted with Rust's borrow rules: a guard borrowing `&mut App` cannot coexist with `App` flowing through the guarded closure body, and a guard borrowing only the disjoint field cannot release that borrow before the closure mutably borrows other App fields. Inline checks (consult the field, set/restore via `replace`) are simpler, match the existing code style for `window_update_stack` (which has no panic-safety either), and don't introduce new abstractions. Panic-safety on these fields was a speculative side benefit; the existing manual push/pop pattern was already vulnerable to the same dirty-state-on-panic class, and no test was broken by the absence. Documented as an accepted limitation here rather than disguised under a guard struct that didn't actually solve it.

**Rejected: `ReentryMode::PanicLikeUpstream`** (rev-2 → rev-3 dropped). Three reviewers converged on this: (a) the hatch could not faithfully reproduce upstream entity-side panic, which was `double_lease_panic` text, NOT `BorrowMutError`; (b) the `legacy-reentry-panics` feature flag was never actually declared in `Cargo.toml`; (c) holding a runtime `App` field for a compile-time-gated variant is dead weight in non-test, non-feature builds. Deferred to K07's PR, where the AppCell redesign makes a compatibility shim (if even needed) easier to reason about.

**Rejected: explicit `impl From<ReentryError> for anyhow::Error`** (rev-2 → rev-3 dropped). Conflicts with anyhow's blanket `impl<E: Error + Send + Sync + 'static> From<E> for anyhow::Error`. Two impls would not compile. The blanket fires automatically once `ReentryError` derives `thiserror::Error`.

**Rejected: widening `update_entity` trait signature** (rev-1 considered → rejected). The trait method returns `R` directly; widening to `Result<R, ReentryError>` would break all 5 `AppContext` implementors (App, AsyncApp, TestAppContext, VisualTestContext, HeadlessAppContext) and every caller. ROADMAP K15 authorizes "panic with structured error (acceptable)" — that path was taken instead.

**Accepted: panic-shape for `with_element_state`** (rev-2 + rev-3 confirmed). Widening to `Result<R, ReentryError>` would touch 7 callsites and change a public method signature. Structured panic with `ReentryError::ElementStateInUse` Display satisfies the ROADMAP K15 intent of "no undefined `RefCell::borrow_mut` panics" — the panic is now defined, named, and machine-readable.

**Accepted: unifying `double_lease_panic` Display** (rev-3 added). Without this, the entity-side contract had two panic messages for semantically identical violations: the App-level guard (rev-3 added) used `ReentryError::NestedEntityUpdate` Display, while the EntityMap-level fallback (always-existing) used `"cannot update <T>..."`. Multi-entity cycles `A → B → A` always fall through to the EntityMap-level fallback because `currently_updating_entity` only tracks one entity. Unifying the message means both paths produce the same Display, with operation+type appended for diagnostic context.

**Accepted: `Strict` mode default in `cfg(test)`** (rev-2 + rev-3). Tests get the loudest signal so silent re-entry bugs surface as `error!` log events. Verified empirically: 333 pre-existing flui-core tests pass under `Strict` default with no changes.

## API surface

**New public items** (re-exported through `flui_core::prelude`):

- `pub mod reentrancy` — module-level rustdoc IS the in-source contract document.
- `pub enum ReentryError` (`#[non_exhaustive]`).
- `pub enum ReentryMode` (`#[non_exhaustive]`).
- `pub fn App::set_reentry_mode(&mut self, mode: ReentryMode)`.
- `impl From<std::cell::BorrowMutError> for ReentryError` (with `#[track_caller]`).

**Breaking signature changes**:

- `pub fn Window::prompt(...) -> Result<oneshot::Receiver<usize>, ReentryError>` (was: `oneshot::Receiver<usize>`).
- `pub fn AsyncWindowContext::prompt(...) -> anyhow::Result<oneshot::Receiver<usize>>` (was: `oneshot::Receiver<usize>`).

**Behavior changes (no signature break)**:

- Same-window `update_window` re-entry: returns `Err` instead of `BorrowMutError` panic.
- Same-entity `update_entity` re-entry: panics with `ReentryError::NestedEntityUpdate` Display instead of `BorrowMutError` text.
- Multi-entity cycle (`A → B → A`): panics with `ReentryError::NestedEntityUpdate` Display via the unified `EntityMap::double_lease_panic` (was: `"cannot update <T>..."` text).
- `with_element_state` recursive call: panics with `ReentryError::ElementStateInUse` Display (was: bare `expect` text).
- `AsyncApp::run_update` `try_borrow_mut()` failure: anyhow chain carries `ReentryError::AppBorrowed` Display (was: bare `BorrowMutError`).
- `AsyncWindowContext::prompt` no longer swallows errors via dead receivers; surfaces them via `Result`.

## Migration / compatibility

flui-v2 is pre-1.0; no semver promise yet (R1/R2 are pending roadmap items). The breaking changes above land unilaterally per the hard-fork posture documented in `.ai-factory/RESEARCH.md`.

Two example callsites at `crates/flui-core/examples/legacy/window.rs:203, 221` were updated in the same PR to handle the new `Result<_, ReentryError>` return from `Window::prompt` (added `.expect("documented as never re-entrant: button click handler runs outside any other prompt scope")`).

No other workspace callers of `Window::prompt` or `AsyncWindowContext::prompt` exist (verified via `git grep`).

Pre-existing test code that depends on the old entity-side panic message text (`"cannot update <T> while it is already being updated"`) will fail under K15 because the unified `double_lease_panic` produces `ReentryError::NestedEntityUpdate(_)` Display. No such tests were found in the workspace.

## Testing

11 new tests in `crates/flui-core/src/reentrancy.rs`:

**Type-level smoke tests (6)**:
- `reentry_error_is_send_sync_static`
- `reentry_error_display_format_matches_contract` (mentions `cx.defer` escape hatch in messages)
- `reentry_error_converts_into_anyhow` (anyhow blanket impl)
- `borrow_mut_error_converts_to_app_borrowed`
- `reentry_mode_default_is_loose`
- `reentry_mode_is_copy`

**Behavioral integration tests (5, via `TestApp`)**:
- `set_reentry_mode_setter_round_trips`
- `nested_update_entity_same_target_panics_with_structured_display` (catches panic, asserts Display contains "update_entity called recursively" + "cx.defer")
- `entity_map_double_lease_uses_unified_reentry_display` (multi-entity cycle `A → B → A`, asserts unified Display, asserts legacy text absent)
- `nested_update_entity_different_target_runs_synchronously` (positive case)
- `cx_defer_avoids_reentry_panic` (escape hatch happy path)

Property tests for `update_window` re-entry, `with_element_state` re-entry, and `Window::prompt` re-entry require visual-test harness (Window mocking, platform prompt mocking) which is out of scope for K15. Deferred to K17 (audit-finding E, "Test harness simplification") per the next paragraph.

## Known limitations

These are NOT bugs in K15; they are scope decisions documented for future readers:

1. **`AsyncApp` non-`try_borrow_mut` sites** — 10+ direct `app.borrow_mut()` calls remain unstructured. K07 redesigns this surface.
2. **`AsyncApp::as_mut` panic** at `app/async_context.rs:73` (`"Cannot as_mut with an async context. Try calling update() first"`) — different panic class, not re-entry. Out of K15 scope.
3. **`web` platform dispatcher re-entry exposure** — unverified. Web event loop is single-threaded, so the exposure is likely zero, but no test pins this.
4. **`ReentryError::AppBorrowed` carries no source location** — `std::cell::BorrowMutError::location()` is nightly-only. `RUST_LOG=flui_core::reentrancy=warn` provides callsite context via `#[track_caller]` on the `From` impl.
5. **Window-level re-entry behavioral tests deferred** — `update_window` re-entry, `with_element_state` recursion, and `Window::prompt` re-entry need richer test harness than K15 provides. Deferred to K17.
6. **No panic-safety on `currently_updating_entity` and `window_update_stack` fields on the panic-during-update path** — same as the pre-K15 manual push/pop pattern. Acceptable parity; not a regression.

## Open questions

- **`ReentryMode::PanicLikeUpstream` cadence?** Deferred to K07 — that PR redesigns AppCell and is the natural place for any compatibility shim, if needed at all.
- **Property-test infrastructure?** K17 — deferred; K15's behavioral tests using `TestApp` cover the most important paths.

## Done criteria

K15 is done when (verified at HEAD):

1. `crates/flui-core/src/reentrancy.rs` module exists with full rustdoc, `ReentryError`, `ReentryMode`, `From<BorrowMutError>` impl, behavioral tests.
2. `App` carries `currently_updating_entity` and `reentry_mode` fields, initialized in `App::new_app` with `Strict` in `cfg(test)`.
3. `App::set_reentry_mode` is public.
4. `App::update_window_id` returns `Err(anyhow{ ReentryError::NestedWindowUpdate })` on same-window re-entry; `trail()` pop ordering preserved.
5. `App::update_entity` panics with `ReentryError::NestedEntityUpdate(_)` Display on same-entity re-entry; slot saved/restored across nested-different-entity calls.
6. `EntityMap::double_lease_panic` rewritten to use `ReentryError::NestedEntityUpdate(entity_id)` Display.
7. `Window::with_element_state` panics with `ReentryError::ElementStateInUse` Display on recursive call.
8. `Window::prompt` returns `Result<oneshot::Receiver<usize>, ReentryError>`; `AsyncWindowContext::prompt` returns `anyhow::Result<oneshot::Receiver<usize>>` (no longer swallows).
9. `AsyncApp::update_window` routes `try_borrow_mut()?` through `From<BorrowMutError> for ReentryError`.
10. Three platform comment sites reference K15.
11. 11 tests pass; total flui-core lib test count: 333 → 344.
12. `cargo build --workspace --all-features` green.
13. `cargo test --workspace` green.
14. `cargo clippy --workspace --all-targets --all-features -- -D warnings` zero warnings.
15. `cargo fmt --all -- --check` clean.

## Cross-references

- ROADMAP K15: `.ai-factory/ROADMAP.md` line 57 (Phase 0-K critical chain, second spec).
- Plan: `.ai-factory/plans/feature-K15-reentrancy-contract.md` (full task list, contract matrix, refinement record across 3 revisions).
- RESEARCH Active Summary: `.ai-factory/RESEARCH.md` (project context, hard-fork posture, audit findings).
- K99 spec (precedent format): `docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md`.
- Three platform comment sites: `crates/flui-core/src/platform/mac/platform.rs:498-503`, `crates/flui-core/src/platform/mac/platform.rs:1253-1257`, `crates/flui-core/src/platform/windows/platform.rs:449-454`.
- Source-of-truth rustdoc: `crates/flui-core/src/reentrancy.rs` (module-level docs).

## Unblocks

- **K07** — AppCell removal. Inherits `ReentryError` (may add new variants under token model), the `cx.defer` escape hatch contract, and the 4 documented Known Limitations (especially the `AsyncApp` surface redesign).
- **K05** — Element trait → context object. Re-entry semantics for `&mut PaintCx<'_>` etc. inherit the contract.
- **K01** — Provider rewrite. Subscription re-entry path uses K15's `cx.defer` recommendation in `inherit<T>()` callback contract.
- **SF02** — Reconciliation. `setState` inside `did_update_widget` is the simulated case from K15's behavioral tests; SF02 publishes the user-facing "use cx.defer" doc on top of the contract.
- **SF05** — `setState` + dirty-list. Same as SF02 for the framework-tier API.

## Next steps

After K15 lands:

```
/aif-plan full K07-appcell-removal-token-borrow
```

K07's plan absorbs the four Known Limitations from K15 (especially `AsyncApp` redesign and `PanicLikeUpstream` evaluation).
