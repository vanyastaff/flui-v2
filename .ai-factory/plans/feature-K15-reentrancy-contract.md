# K15 — Re-entrancy contract

**Branch:** `feature/K15-reentrancy-contract`
**Created:** 2026-05-09
**Refined (aif-improve):** 2026-05-09 — codebase-grounded fact pass; scope narrowed (no new `Effect` variant), counts corrected, log vs tracing aligned with project style.
**Refined (3-agent adversarial review):** 2026-05-09 — `flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor` returned 22 findings (9 BLOCKERs, 8 MAJOR, 5 MINOR). All BLOCKERs patched into tasks; `PanicLikeUpstream` deferred to K07; `double_lease_panic` unification added; `AsyncWindowContext::prompt` signature change made explicit.
**Phase:** 0-K (Kernel Cleanup) — second spec in the critical chain (gates K07 → K05 → K01 → K02 → K03 → K04 → Phase II-F)
**Type:** architectural contract + targeted runtime enforcement (no new queueing infrastructure; nested same-target updates are Forbidden — `cx.defer` is the single escape hatch)

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | Property tests over re-entry permutations are a CORE deliverable of K15 — without them the contract is unverified |
| Logging | verbose | New runtime path (re-entry detection, structured panic, RAII drop). `log::trace!` on each `ReentryGuard` enter/exit, `log::warn!` on every structured-panic conversion, `log::debug!` on contract-mode toggle. **Project uses `log` crate** ([flui-core/Cargo.toml:86](crates/flui-core/Cargo.toml#L86)) — NOT `tracing`. `tracing` standardization is roadmap A4, explicitly out of K15 scope. |
| Docs | yes (mandatory checkpoint) | New public types (`ReentryError`), new module-level docs covering "what is allowed inside which callback", design spec under `docs/superpowers/specs/`, ROADMAP flip, RESEARCH "Active Summary" addendum |
| Roadmap linkage | linked | K15 in Phase 0-K critical chain |

## Roadmap Linkage

**Milestone:** K15 — Re-entrancy contract (Phase 0-K Kernel Cleanup, critical chain — second spec after K99).

**Rationale:** Per `.ai-factory/ROADMAP.md` Phase 0-K — "document and enforce semantics for `update_window` inside `update_window`, `update_entity` inside callback, `setState` inside `did_update_widget`. Either queue (preferred) or panic with structured error (acceptable). No undefined `RefCell::borrow_mut` panics. Adds property-tests covering re-entry scenarios. **HIGH-RISK** — touches every callback in the system."

K15 must land before K07 (AppCell removal). K07's token-based borrow model makes re-entry a compile-time concern, but for that refactor to be safe each callback's contract must already be **specified**. K15 is the spec; K07 is one possible implementation strategy.

K15 also has cross-track value beyond the critical chain: SF02 (reconciliation) and SF05 (`setState` + dirty-list) inherit the same contract — `setState` inside `did_update_widget` is a re-entry case **as if** the State system existed today. By specifying the contract in engine terms now, the Framework tier (Phase II-F) gets a published target.

## Research Context

From `.ai-factory/RESEARCH.md` (Active Summary), the K15 reconnaissance pass, and the 3-agent adversarial review:

- **Hard fork posture** — flui-v2 has no upstream-sync commitment, no semver compatibility with `gpui`. Re-entrancy contract is unilateral; we name it, document it, enforce it. Upstream's silent `RefCell::borrow_mut` panics are not a constraint to preserve.
- **Phase 0-K rationale** — 24+ structural issues block a healthy Framework tier; re-entrancy is one of them.
- **Current state (audit findings, summarized — full mapping below in "Inventory of re-entry vectors"):**
  - **AppCell = `RefCell<App>`** ([crates/flui-core/src/app.rs:75-108](crates/flui-core/src/app.rs#L75)). `borrow_mut()` panics with bare `BorrowMutError` text on any re-entry. Two existing platform sites paper over this in mac/windows comments.
  - **`EntityMap::lease` is the OTHER pre-existing re-entry trap** ([app/entity_map.rs:142](crates/flui-core/src/app/entity_map.rs#L142)) — `unwrap_or_else(|| double_lease_panic::<T>("update"))` produces `"cannot update {TypeName} while it is already being updated"`. This is NOT a `BorrowMutError`. Adversarial review (migration-risk B1) caught that the original plan only treated AppCell as the re-entry boundary; entity-side has its own pre-existing structured panic which K15 must unify or coexist with explicitly. **Decision (revision 3):** unify. K15 rewrites `double_lease_panic` to construct `ReentryError::NestedEntityUpdate(id)` and use its Display. One panic message, one variant.
  - **Effect queue exists; `cx.defer` and `Window::defer` are escape hatches.** `pending_effects: VecDeque<Effect>` ([app.rs:603](crates/flui-core/src/app.rs#L603)) with `Effect::Defer { callback }` ([app.rs:2501](crates/flui-core/src/app.rs#L2501)) and `defer()` at [app.rs:1655-1658](crates/flui-core/src/app.rs#L1655). `Window::defer` ([window.rs:1799](crates/flui-core/src/window.rs#L1799)) is the parallel escape hatch when the caller has `&mut Window`.
  - **`window_update_stack: Vec<WindowId>` already exists** at [app.rs:653](crates/flui-core/src/app.rs#L653); manual push/pop in TWO sites — `update_window_id` at [1559](crates/flui-core/src/app.rs#L1559)/[1562](crates/flui-core/src/app.rs#L1562) AND `open_window` at [1084](crates/flui-core/src/app.rs#L1084)/[1086](crates/flui-core/src/app.rs#L1086). Adversarial review (arch + migration M3) flagged that the original plan only covered `update_window_id`; revision 3 covers both with the RAII guard.
  - **`with_element_state` recursion key is `GlobalElementId`, NOT `ElementId`** — at [window.rs:3118](crates/flui-core/src/window.rs#L3118) the key is `(global_id.clone(), TypeId::of::<S>())`. Adversarial review (api BLOCKER 1) caught that the original `ReentryError::ElementStateInUse { element_id: ElementId, ... }` variant was wrong-typed. Revision 3 uses `global_element_id: GlobalElementId`.
  - **`with_element_state` callsites: 7** — animation.rs:174, image_cache.rs:343, text.rs:796, view.rs:149, view.rs:215, window.rs:3071, window.rs:3200.
  - **`Window::prompt` topology:** `Platform::prompt` trait at [platform.rs:641](crates/flui-core/src/platform.rs#L641) and 7 platform impls — NOT touched by K15. Conversion target: `Window::prompt` ([window.rs:5142](crates/flui-core/src/window.rs#L5142), `unreachable!` at [5155](crates/flui-core/src/window.rs#L5155)). `AsyncWindowContext::prompt` ([app/async_context.rs:345-360](crates/flui-core/src/app/async_context.rs#L345)) currently swallows errors via `.unwrap_or_else(|_| oneshot::channel().1)` (line 359) — adversarial review (migration B2 + api MAJOR 3) caught this. Revision 3 explicitly widens its return type. Two example callsites at [examples/legacy/window.rs:203,221](crates/flui-core/examples/legacy/window.rs) need updates (migration B3).
  - **`AppContext` trait + 5 implementors.** `AppContext` at [app.rs:2370](crates/flui-core/src/app.rs#L2370). Implementors: `App`, `AsyncApp` (95% of paths via `app.borrow_mut()`), `TestAppContext::update_entity` ([app/test_app.rs:135-141](crates/flui-core/src/app/test_app.rs#L135) — calls `self.update(|cx| entity.update(cx, f))`, where `entity.update` is `Entity::update` NOT `App::update_entity`; this is the funnel-bypass adversarial review (arch B3) flagged), `VisualTestContext::update_window` ([app/visual_test_context.rs:176](crates/flui-core/src/app/visual_test_context.rs#L176)), `HeadlessAppContext::update_window` ([app/headless_app_context.rs:155](crates/flui-core/src/app/headless_app_context.rs#L155)). Revision 3 verifies `Entity::update` reaches the `EntityMap::lease` funnel (which after K15's unification carries the `ReentryError::NestedEntityUpdate` panic).
  - **`observe_in` / `subscribe_in` are HIGH-RISK silent-failure sites** — [context.rs:334-348](crates/flui-core/src/app/context.rs#L334) and [363-387](crates/flui-core/src/app/context.rs#L363) do `window_handle.update(cx, |_, window, cx| observer.update(cx, ...))` from inside an outer `update_window` for the same window. Under K15 Strict (default in `cfg(test)`), this returns `Err(ReentryError::NestedWindowUpdate)`, which is then silently discarded by `.unwrap_or(false)`. **Subscriber callback never fires.** Adversarial review (migration H2) caught this — revision 3 adds explicit audit + test (Task 13).
  - **Three platform deferral comments (not two):** [platform/mac/platform.rs:500-502](crates/flui-core/src/platform/mac/platform.rs#L500), [platform/windows/platform.rs:452-453](crates/flui-core/src/platform/windows/platform.rs#L452), and [platform/mac/platform.rs:1254](crates/flui-core/src/platform/mac/platform.rs#L1254) (thermal state change deferral — adversarial review migration H5 caught the missing third site).
  - **Existing structured panics** keep their shape with `ReentryError` Display: `with_element_state` ([window.rs:3155-3157](crates/flui-core/src/window.rs#L3155)), `unreachable!` prompt re-entry ([window.rs:5155](crates/flui-core/src/window.rs#L5155)), and now `double_lease_panic` ([entity_map.rs:207-211](crates/flui-core/src/app/entity_map.rs#L207)).
  - **`SubscriberSet::retain` snapshot pattern** ([subscription.rs:116-153](crates/flui-core/src/subscription.rs#L116)) is the reference shape — codify what it already does, propagate to the contract.
  - **Property tests for re-entry: 2 unit tests in [animation/listeners.rs:299-351](crates/flui-core/src/animation/listeners.rs#L299).** App-level: 0. `proptest = "1"` already in `[dev-dependencies]` ([flui-core/Cargo.toml:278](crates/flui-core/Cargo.toml#L278)).
  - **`thiserror = "2.0.12"`** already a direct dep at [flui-core/Cargo.toml:120](crates/flui-core/Cargo.toml#L120). `#[non_exhaustive]` enums supported in thiserror 2.x.
- **Constraints carried over:** 60 FPS structural property — re-entry checks are O(stack_depth) (`Vec::contains`); typically depth 1-2; `Option::contains` is O(1). No allocation on hot path. No new `Rc<RefCell<…>>`.
- **Scope decisions resolved by adversarial review (revision 3):**
  - **Drop `ReentryMode::PanicLikeUpstream` from K15 scope.** Defer to K07. Three reviewers converged on this: (a) the hatch cannot faithfully reproduce upstream behavior because entity-side panic is `double_lease_panic`, not `BorrowMutError` (migration B4 / api MAJOR for feature flag); (b) the feature flag was not declared in `Cargo.toml` (api BLOCKER 9); (c) runtime `App` field for compile-time-gated variant is dead weight (arch decision-4). K15 ships `ReentryMode { Strict, Loose }` with `#[non_exhaustive]` so K07 can add variants without semver impact.
  - **Drop the explicit `impl From<ReentryError> for anyhow::Error`.** Conflicts with `anyhow`'s blanket impl `From<E: Error + Send + Sync + 'static>` (api BLOCKER 8). `?` operator works automatically once `ReentryError: thiserror::Error`.
  - **Multi-entity A-B-A re-entry cycle is detected by the unified `double_lease_panic`** (now using `ReentryError::NestedEntityUpdate` Display). The K15 `currently_updating_entity: Option<EntityId>` field catches direct A-A re-entry; multi-step cycles fall through to the `EntityMap::lease` panic, which after K15's Task 7 carries the same structured Display. Net effect: ALL entity re-entry produces the same structured panic message. Resolves arch + migration M5.
  - **`AsyncApp` 10+ remaining `borrow_mut()` sites** (lines 39, 45, 55, 65, 126, 135, 152, 168, 182) are an ACCEPTED gap, documented under "Known Limitations". K07 redesigns this surface anyway.

## Current state (pre-K15)

| Aspect | State | Note |
|---|---|---|
| `ReentryError` type | absent | `BorrowMutError` (std) used directly at the only `try_borrow_mut` site; `double_lease_panic` panics with raw text |
| Re-entrant `update_window` (same window) | **panics** with `BorrowMutError` text | undefined contract; no error type |
| Re-entrant `update_window` (different window) | **allowed** (independent borrow domains) | undocumented |
| Re-entrant `update_entity` (same entity, direct call) | **panics** via `EntityMap::lease` → `double_lease_panic` with `"cannot update <T>..."` text | structured BUT not via `ReentryError` |
| Multi-entity cycle (A-B-A) | **panics** via `double_lease_panic` (entity slot empty when A re-enters) | structured BUT not via `ReentryError` |
| `Effect::Defer` queue path | exists, used by `cx.defer()` and `window.defer()` | K15 promotes these as the SINGLE escape hatch |
| Documented contract (rustdoc) | absent | no module-level doc explains "what runs synchronously vs queued" |
| Property tests for re-entry | 2 (animation listener) | App / Window / Entity re-entry: 0 |
| Logging instrumentation around re-entry | none | `log::trace!` / `warn!` to be added |
| Platform workarounds documenting re-entry | 3 sites (mac × 2, windows × 1) | mac/platform.rs:500-502 + 1254, windows/platform.rs:452-453 |
| `with_element_state` re-entry | bare `expect(...)` panic at [window.rs:3155-3157](crates/flui-core/src/window.rs#L3155) | converts to `ReentryError::ElementStateInUse` (panic shape, structured Display) |
| Re-entrant `Window::prompt` | `unreachable!(...)` at [window.rs:5155](crates/flui-core/src/window.rs#L5155) | converts to `Err(ReentryError::PromptInProgress)`; `Window::prompt` and `AsyncWindowContext::prompt` signatures widen |
| `window_update_stack` lifecycle | manual `push`/`pop` in TWO sites: `update_window_id` ([app.rs:1559-1587](crates/flui-core/src/app.rs#L1559)) AND `open_window` ([app.rs:1084-1086](crates/flui-core/src/app.rs#L1084)) | RAII guard added in K15 covers both, closes panic-leaves-stack-dirty class |
| `observe_in` / `subscribe_in` callbacks | call `window_handle.update(cx, ...)` from inside outer `update_window`; `.unwrap_or(false)` discards errors | Strict mode ⇒ silent functionality loss; revision 3 audits + test |

## Design

> **The full design is captured in the spec authored at Task 14** (`docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md`). What follows is the operational shape that drives the tasks.

### The two-axis contract

Every callback has two answers:
1. **Re-entry into App / Window / Entity from inside this callback — what happens?** → one of `Synchronous` / `Forbidden`. (Queued is reserved exclusively for `cx.defer` / `window.defer`, which the user calls explicitly.)
2. **What does "Forbidden" produce?** → a `ReentryError` variant, NOT a bare `RefCell::borrow_mut` panic.

### `ReentryError` (new public type — `flui_core::reentrancy::ReentryError`)

```rust
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ReentryError {
    #[error("update_window called recursively for window {0:?}; use cx.defer or window.defer to schedule work")]
    NestedWindowUpdate(WindowId),

    #[error("entity {0:?} is already leased; recursive re-entry is forbidden — use cx.defer to schedule work")]
    NestedEntityUpdate(EntityId),

    #[error("with_element_state called recursively for ({global_element_id:?}, {type_id:?})")]
    ElementStateInUse {
        global_element_id: GlobalElementId,
        type_id: TypeId,
    },

    #[error("prompt() called while another prompt is awaiting user response")]
    PromptInProgress,

    #[error("App was already mutably borrowed (callback re-entered the runtime; use cx.defer)")]
    AppBorrowed,
}
```

Notes (revision 3):
- Variant field is `global_element_id: GlobalElementId`, NOT `element_id: ElementId` — per api BLOCKER 1; the `with_element_state` key is `(GlobalElementId, TypeId)`.
- `#[non_exhaustive]` because future K-specs (K07, K01) will add variants without breaking SemVer.
- All variant fields are `Send + Sync + 'static`: `WindowId` / `EntityId` are slotmap keys (`Send + Sync`); `GlobalElementId = Arc<[ElementId]>` is `Send + Sync`; `TypeId` is `Copy`. So `ReentryError: std::error::Error + Send + Sync + 'static` and `anyhow::Error: From<ReentryError>` is satisfied by the **blanket impl** — NO explicit `impl From<ReentryError> for anyhow::Error` (api BLOCKER 2).
- `From<std::cell::BorrowMutError>` is a manual impl that maps to `ReentryError::AppBorrowed`. No `#[from]` attribute on the variant (would conflict with the manual impl).

### `ReentryMode` (new public enum)

```rust
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReentryMode {
    /// Re-entry produces `ReentryError` immediately. Default in `cfg(test)`.
    Strict,
    /// Re-entry produces `ReentryError` AND emits `log::warn!`. Default in release.
    Loose,
}

impl Default for ReentryMode {
    fn default() -> Self {
        Self::Loose
    }
}
```

Revision 3 changes:
- `#[non_exhaustive]` (api MAJOR 1).
- Full derive set: `Clone, Copy, Debug, PartialEq, Eq` (api MAJOR 2). `Copy` so reads from `App::reentry_mode` don't require `clone`.
- Manual `Default` returning `Loose`.
- **Two variants only** — `PanicLikeUpstream` deferred to K07 (resolves api BLOCKER 9 by removing the feature flag entirely from K15 scope).

### The contract matrix

| Callback class | Re-entry into App / Window / Entity | Strategy | Existing site (file:line) | Behavior change in K15 |
|---|---|---|---|---|
| `cx.update_window(…)` inside another `update_window` for **the same** window | nested mutable borrow of `WindowState` | **Forbidden — structured** | [app.rs:1550-1592](crates/flui-core/src/app.rs#L1550) (`update_window_id`) | early-return `Err(ReentryError::NestedWindowUpdate(id).into())` BEFORE attempting `windows.get_mut(id)?.take()`; signature unchanged (`Result<T>`) |
| `cx.update_window(…)` for a **different** window inside outer update | independent borrow domains | **Synchronous (allowed)** | same | no change; documented |
| `cx.update_entity(&handle, …)` inside another `update_entity` for **the same** entity (direct re-entry) | second lease attempt | **Forbidden — structured** | [app.rs:2410-2424](crates/flui-core/src/app.rs#L2410) | check `currently_updating_entity == Some(id)` BEFORE `entities.lease`; on match, panic with `ReentryError::NestedEntityUpdate(id)` Display. Trait signature `R` unchanged. |
| Multi-entity cycle `update_entity(A) → update_entity(B) → update_entity(A)` | A's slot is empty during inner call (B's lease succeeds; A's re-entry hits `EntityMap::lease`'s empty slot) | **Forbidden — structured** | [entity_map.rs:142,207-211](crates/flui-core/src/app/entity_map.rs#L142) | Task 7 rewrites `double_lease_panic` to construct `ReentryError::NestedEntityUpdate(id)` and panic via its Display. **Single message** for both direct and cycle re-entry (revision 3 unification). |
| `cx.update_entity(&different_handle, …)` not yet leased | independent leases | **Synchronous (allowed)** | same | no change |
| Observer callback (`on_notify` / `on_emit` / global / new-entity / bounds / appearance / activation) | snapshot before iterate | **Synchronous within callback**; nested same-target updates Forbidden | [app.rs:1490-1548](crates/flui-core/src/app.rs#L1490), [subscription.rs:116-153](crates/flui-core/src/subscription.rs#L116) | document existing snapshot semantics; nested updates raise `ReentryError`; user directed to `cx.defer` |
| `observe_in` / `subscribe_in` callbacks | callback's body does `window.update(cx, ...)` from inside outer `update_window` for SAME window | **Forbidden — structured** (the inner update returns `Err`) | [context.rs:334-348](crates/flui-core/src/app/context.rs#L334), [363-387](crates/flui-core/src/app/context.rs#L363) | **Adversarial-review-driven (revision 3):** `.unwrap_or(false)` at the end of the closure currently discards the error. K15 changes the discard to `.map_err(|e| log::warn!("…")).is_ok()` so silent loss is at least logged. Test asserts callback fires (or the documented Forbidden message appears). See Task 13. |
| `with_element_state(global_id, type_id, …)` recursively for the same key | `Option<T>::take` already empty | **Forbidden — structured** (panic shape, structured Display) | [window.rs:3155-3157](crates/flui-core/src/window.rs#L3155) | `expect("reentrant…")` → `unwrap_or_else(|| panic!("{}", ReentryError::ElementStateInUse{ global_element_id, type_id }))`. 7 callsites audited |
| `Window::prompt(...)` while another prompt open | `Option<PromptBuilder>::take` already empty | **Forbidden — structured** | [window.rs:5155](crates/flui-core/src/window.rs#L5155) | `unreachable!` → `Err(ReentryError::PromptInProgress)`. Signature widens to `Result<oneshot::Receiver<usize>, ReentryError>`. `AsyncWindowContext::prompt` ([async_context.rs:345-360](crates/flui-core/src/app/async_context.rs#L345)) ALSO widens to `Result<oneshot::Receiver<usize>, anyhow::Error>` (revision 3, replacing the swallowing `.unwrap_or_else(\|_\| oneshot::channel().1)` at line 359). 7 platform impls untouched. |
| `cx.defer(\|cx\| …)` from any callback | always queued | **Queued (existing)** | [app.rs:1655-1658](crates/flui-core/src/app.rs#L1655) | no change; **THE escape hatch** |
| `window.defer(cx, \|w, cx\| …)` from any Window-context callback | queued via App's effect queue | **Queued (existing)** | [window.rs:1799](crates/flui-core/src/window.rs#L1799) | no change; parallel escape hatch |
| Next-frame callback calling `update_window` for same window | drained inside platform's frame tick (already inside an `update`) | **Forbidden — structured** | [window.rs:1902-1903](crates/flui-core/src/window.rs#L1902), drain at [1267-1275](crates/flui-core/src/window.rs#L1267) | users hit `Err`; redirected to `cx.defer` |
| Keystroke / action listener calling `notify` / `emit` | already queued | **Queued (existing)** | [app.rs:1490-1507](crates/flui-core/src/app.rs#L1490), [window.rs:5008-5075](crates/flui-core/src/window.rs#L5008) | no change |
| Animation listener (Ticker tick) | listeners snapshot before iterate | **Synchronous within callback; nested updates Forbidden** | [animation/listeners.rs:79-100](crates/flui-core/src/animation/listeners.rs#L79) | document |
| Gesture recognizer event handler | scoped `Rc<RefCell<…>>` per recognizer (intentional, A7-audit) | **Synchronous; nested App-level updates Forbidden** | [gesture/arena.rs:350-655](crates/flui-core/src/gesture/arena.rs#L350) | document only |
| Release listener (entity drop callback) | runs during effect flush | **Synchronous within listener; nested updates Forbidden** | [app.rs:1450-1465](crates/flui-core/src/app.rs#L1450) | document |
| Async context (`AsyncApp::run_update`) | already returns `Result<_>` (anyhow); `app.try_borrow_mut()?` | **Forbidden — structured** | [app/async_context.rs:90](crates/flui-core/src/app/async_context.rs#L90) | error chain widens via `From<BorrowMutError> for ReentryError`; `?` works through anyhow blanket |

### Two runtime modes

| Mode | Effect |
|---|---|
| `ReentryMode::Strict` | re-entry produces `ReentryError` immediately + `log::error!`. **Default in `cfg(test)`.** |
| `ReentryMode::Loose` (default in release) | same enforcement + `log::warn!` instead of `error!`. |

`PanicLikeUpstream` removed from K15 scope (revision 3) — deferred to K07 with rationale recorded in design spec §Decision log.

### `RAII guards` (covers BOTH push/pop sites)

Two RAII types live in `flui-core::reentrancy`:

- `WindowUpdateGuard<'a>` — pushes onto `App::window_update_stack` on construction, pops on `Drop`. Replaces:
  - manual push at [app.rs:1559](crates/flui-core/src/app.rs#L1559) and pop in `trail()` at [app.rs:1562](crates/flui-core/src/app.rs#L1562) (`update_window_id`)
  - manual push/pop at [app.rs:1084-1086](crates/flui-core/src/app.rs#L1084) (`open_window`) — adversarial review (arch + migration M3) added this site
- `EntityUpdateGuard<'a>` — sets `App::currently_updating_entity` on construction, clears on `Drop`. Used in `App::update_entity`.

Construction is fallible: `WindowUpdateGuard::enter(&mut app, window_id) -> Result<Self, ReentryError>` etc.

**Drop ordering (revision 3 — load-bearing decision):** `WindowUpdateGuard::Drop` MUST fire AFTER `trail()` (or its equivalent in `open_window`) returns. Currently `trail()` ([app.rs:1561-1586](crates/flui-core/src/app.rs#L1561)) pops the stack BEFORE running `window_closed_observers` (lines 1568-1571). Observers see the closing window's id ABSENT from the stack — that's existing semantics. Implementation strategy:
- The guard is constructed inside the `update(|cx| { ... })` closure body.
- Inside the closure: declare `let mut window = cx.windows.get_mut(id)?.take()?;` FIRST, then `let _guard = WindowUpdateGuard::enter(cx, id)?;` SECOND.
- Run `update(root_view, &mut window, cx)` and `trail(id, window, cx)?` inside the closure.
- The closure body returns; the guard drops AFTER the closure body executes (Rust drops in reverse declaration order — guard declared after `window` drops before `window`, but the closure scope ends before either). The point: guard's Drop runs after `trail()` completes, popping the stack AFTER observers have run.
- This is contrary to the desired pop-before-observer order. **Two-phase guard required:** add `WindowUpdateGuard::commit_pop(self)` consuming method that pops explicitly. Call `commit_pop` before observers fire (i.e., at the start of the `trail()` body, mirroring current line 1562). Drop becomes a panic-safety fallback that pops only if `commit_pop` was never called.

`EntityUpdateGuard::Drop` (clearing `currently_updating_entity`) has no observer-visibility hazard — clears unconditionally on Drop.

### What K15 does NOT do

- Does NOT remove `AppCell` (K07).
- Does NOT change `Element` trait signatures (K05).
- Does NOT introduce `BuildOwner` / `PipelineOwner` (K06).
- Does NOT touch `Render::&mut self` (K03).
- Does NOT define `setState` (Phase II-F / SF05). K15 documents the contract `setState` will adhere to.
- Does NOT refactor `SubscriberSet` internals.
- Does NOT touch gesture re-entry (A7-audit-closed surface).
- Does NOT add a new `Effect` variant.
- Does NOT widen `update_window` or `update_entity` trait signatures.
- Does NOT change `Platform::prompt` trait or its 7 platform implementations.
- Does NOT add `legacy-reentry-panics` Cargo feature (deferred to K07, revision 3).
- Does NOT add `impl From<ReentryError> for anyhow::Error` explicitly — relies on anyhow's blanket impl (revision 3, fixes api BLOCKER 2).
- Does NOT structure-ify the 10+ remaining `borrow_mut()` sites in `AsyncApp` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182) — see Known Limitations.

### Known Limitations (documented in design spec)

These are NOT bugs in K15; they are scope decisions:

1. **`AsyncApp` non-`try_borrow_mut` sites** — 10+ direct `app.borrow_mut()` calls remain unstructured. K07 redesigns this surface; out of scope here.
2. **`AsyncApp::as_mut` panic** at [app/async_context.rs:73](crates/flui-core/src/app/async_context.rs#L73) — `panic!("Cannot as_mut with an async context. Try calling update() first")` is a different panic class (not re-entry). Out of scope.
3. **`web` platform dispatcher re-entry exposure** — unverified (web event loop is single-threaded). Out of scope; if revealed by K07, file follow-up.
4. **`ReentryError::AppBorrowed` carries no source location** — `BorrowMutError::location()` is nightly-only. `RUST_LOG=flui_core::reentrancy=warn` provides callsite context via `#[track_caller]` on `From` impl. Documented.

## Tasks

### Phase 1 — Pre-flight & contract specification

- [x] **Task 1.** Re-confirm baseline: `cargo build --workspace --all-features`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Capture exact output as the "before K15" baseline. → File: `crates/flui-core/`, workspace root. Logging: capture into `.k15-baseline.txt` (gitignored).

- [x] **Task 2.** Audit reconnaissance findings against the live tree on `feature/K15-reentrancy-contract`. For every file:line in the contract matrix and "Current state" table, confirm the line still references the documented behavior. Specifically verify:
  - `TestAppContext::update_entity` at [test_app.rs:140](crates/flui-core/src/app/test_app.rs#L140) — `entity.update(cx, f)` is `Entity<T>::update` (find its impl, trace to `EntityMap::lease`). Confirm that path reaches the unified panic site.
  - `examples/legacy/window.rs` line numbers for the two `window.prompt(...)` callsites (the rev-3 cite is 203, 221; if drifted, update Task 9 cites).
  - Any additional `window_update_stack.push` / `pop` sites beyond the two known (1084-1086, 1559-1562); update Task 5 if found.
  - Any additional `with_element_state` callsites beyond the 7 known.
  - Any additional platform deferral comment sites beyond the 3 known.
  - File touched: this plan only (matrix tables).

### Phase 2 — Public type + module surface

- [x] **Task 3.** Create `crates/flui-core/src/reentrancy.rs` (NEW module). Contains:
  - `pub enum ReentryError` — `#[non_exhaustive]`, `#[derive(Debug, thiserror::Error)]`. Variant `ElementStateInUse` uses `global_element_id: GlobalElementId` (api BLOCKER 1 fix). NO `#[from]` attribute on `AppBorrowed`.
  - `pub enum ReentryMode { Strict, Loose }` — `#[non_exhaustive]`, `#[derive(Clone, Copy, Debug, PartialEq, Eq)]`, manual `Default::default() = Loose`. (api MAJOR 1 + MAJOR 2). NO `PanicLikeUpstream` variant — deferred to K07.
  - `pub(crate) struct WindowUpdateGuard<'a>` — RAII helper. `WindowUpdateGuard::enter(app: &'a mut App, id: WindowId) -> Result<Self, ReentryError>`. `pub(crate) fn commit_pop(self)` consuming method that pops `window_update_stack` explicitly. `Drop` impl pops only if `commit_pop` was not called (use `Option<WindowId>` field to track active state). (revision 3 two-phase design — fixes arch B2 + migration H1 + api MINOR 5).
  - `pub(crate) struct EntityUpdateGuard<'a>` — RAII helper. `EntityUpdateGuard::enter(app: &'a mut App, id: EntityId) -> Result<Self, ReentryError>`. `Drop` clears `currently_updating_entity`. Single-phase (no observer-visibility hazard).
  - `impl From<std::cell::BorrowMutError> for ReentryError { #[track_caller] fn from(_: BorrowMutError) -> Self { ReentryError::AppBorrowed } }`.
  - **NO** explicit `impl From<ReentryError> for anyhow::Error` — anyhow's blanket impl handles it (api BLOCKER 2 fix).
  - Module-level rustdoc IS the in-source contract document (each callback class with strategy, file:line cites, escape-hatch instructions, log target name).
  - Per-variant rustdoc on `ReentryError` and `ReentryMode` to satisfy `#[warn(missing_docs)]` at [lib.rs:2](crates/flui-core/src/lib.rs#L2).
  - Re-export `ReentryError` and `ReentryMode` from `crates/flui-core/src/prelude.rs`. (NOT the guards — they're `pub(crate)`.)
  - Files: `crates/flui-core/src/reentrancy.rs` (new), `crates/flui-core/src/lib.rs` (`pub mod reentrancy`), `crates/flui-core/src/prelude.rs` (re-export).
  - **NO Cargo.toml change needed** — `thiserror`, `proptest`, `log` all present.

- [x] **Task 4.** Add `App` runtime fields (minimal surface):
  - `pub(crate) currently_updating_entity: Option<EntityId>` (init `None`)
  - `pub(crate) reentry_mode: ReentryMode` (init: `if cfg!(test) { ReentryMode::Strict } else { ReentryMode::Loose }`)
  - `pub fn set_reentry_mode(&mut self, mode: ReentryMode)` — public setter to make the prelude `ReentryMode` re-export meaningful (arch M4 fix). One-line setter, no validation.
  - **DO NOT add** `currently_updating_window` (use existing `window_update_stack`) or `update_depth` (use existing `pending_updates`).
  - File: `crates/flui-core/src/app.rs`. Initialize in `App::new_app`.

### Phase 3 — Update path enforcement

- [x] **Task 5.** `WindowUpdateGuard` integration at TWO sites (revision 3 — fixes arch + migration M3):
  - **Site 1: `App::update_window_id`** ([app.rs:1550-1592](crates/flui-core/src/app.rs#L1550)).
    1. After `let mut window = cx.windows.get_mut(id)?.take()?;` (line 1555), construct `let mut guard = WindowUpdateGuard::enter(cx, id)?;` (returns `Err(ReentryError)` on `window_update_stack.contains(&id)`).
    2. Run `update(root_view, &mut window, cx)`.
    3. In `trail(id, window, cx)`: BEFORE the window-removal cleanup at line 1568 (`window_closed_observers`), call `guard.commit_pop()` to pop the stack explicitly (preserves existing observer-visibility semantics: observers see the closing id ABSENT from stack).
    4. Guard's Drop becomes a panic-safety fallback (pops only if `commit_pop` was never called — handles the panic-during-update case).
  - **Site 2: `App::open_window`** ([app.rs:1084-1086](crates/flui-core/src/app.rs#L1084)).
    - Replace manual `cx.window_update_stack.push(id);` ... `cx.window_update_stack.pop();` with the same guard pattern. `open_window` does NOT have the equivalent `trail()`-vs-observer hazard, so the simpler one-phase use is fine: `let _guard = WindowUpdateGuard::enter(cx, id)?;`. Drop pops at end of scope.
  - On `Err` from guard: in `Loose` mode, emit `log::warn!(target: "flui_core::reentrancy", "re-entrant update_window for window {:?} — caller should consider cx.defer", id);`. In `Strict` mode, `log::error!`. Return `Err(anyhow::Error::from(reentry_err)).context("nested update_window")` for `update_window_id`.
  - Files: `crates/flui-core/src/app.rs:1080-1095, 1550-1592`. Signature unchanged.
  - Search callers via `git grep update_window` — expect ~30-50 sites; most propagate via `?`.

- [x] **Task 6.** `EntityUpdateGuard` integration in `App::update_entity` ([app.rs:2410-2424](crates/flui-core/src/app.rs#L2410)):
  1. Construct `let _guard = EntityUpdateGuard::enter(self, handle.entity_id())?;` BEFORE `cx.entities.lease(handle)`.
  2. On `Err`: panic with `panic!("{}", err)` (the Display text from `ReentryError::NestedEntityUpdate`). ROADMAP authorizes panic-with-structured-error. Pre-emit `log::error!` with the same text.
  3. On `Ok`: guard sets `currently_updating_entity = Some(id)`; cleared on Drop.
  - File: `crates/flui-core/src/app.rs:2410-2424`. Trait signature `R` unchanged.

- [x] **Task 7.** Unify `EntityMap::lease` panic shape (revision 3 — fixes arch + migration B4):
  - [entity_map.rs:142](crates/flui-core/src/app/entity_map.rs#L142): `unwrap_or_else(|| double_lease_panic::<T>("update"))` — keep call shape, but rewrite `double_lease_panic` itself:
  - [entity_map.rs:207-211](crates/flui-core/src/app/entity_map.rs#L207): change body from `panic!("cannot {operation} {} while it is already being updated", std::any::type_name::<T>())` to use `ReentryError::NestedEntityUpdate(entity_id)` Display. Signature change: `fn double_lease_panic<T>(operation: &str, entity_id: EntityId) -> !` — caller at line 142 passes `pointer.entity_id`. Same for the `read` site at line 164.
  - Net effect: ALL entity re-entry (direct `update_entity(A,A)`, multi-entity cycle `A→B→A`, observer that re-enters its own entity) panics with the SAME `ReentryError::NestedEntityUpdate(_)` Display. K15 closes its asymmetry.
  - Audit `read` operation: should it use a different variant or `ReentryError::NestedEntityUpdate` too? Decision: same variant — re-entering an entity (read or write) while it's leased is the same class of bug. Document in spec.

- [x] **Task 8.** `with_element_state` recursive panic conversion (revision 3 — fixes api BLOCKER 1):
  - [window.rs:3155-3157](crates/flui-core/src/window.rs#L3155): change `expect("reentrant call to with_element_state for the same state type and element id")` to `unwrap_or_else(|| panic!("{}", ReentryError::ElementStateInUse { global_element_id: global_id.clone(), type_id: TypeId::of::<S>() }))`.
  - **Keep panic shape, NOT widen to `Result<R, ReentryError>`** — preserves source compatibility at 7 callsites; the structured Display satisfies ROADMAP "no undefined panics".
  - Audit all 7 callsites (animation.rs:174, image_cache.rs:343, text.rs:796, view.rs:149, view.rs:215, window.rs:3071, window.rs:3200). Each carries `// SAFETY-CONTRACT(K15): …` if re-entry is structurally impossible.

- [x] **Task 9.** `Window::prompt` and `AsyncWindowContext::prompt` widening (revision 3 — fixes migration B2/B3 + api MAJOR 3):
  - **`Window::prompt`** ([window.rs:5142](crates/flui-core/src/window.rs#L5142)): change return from `oneshot::Receiver<usize>` to `Result<oneshot::Receiver<usize>, ReentryError>`. At [window.rs:5155](crates/flui-core/src/window.rs#L5155): `unreachable!(...)` → `return Err(ReentryError::PromptInProgress)`.
  - **`AsyncWindowContext::prompt`** ([app/async_context.rs:345-360](crates/flui-core/src/app/async_context.rs#L345)): change return from `oneshot::Receiver<usize>` to `Result<oneshot::Receiver<usize>, anyhow::Error>`. Replace the swallowing line 359 (`.unwrap_or_else(\|_\| oneshot::channel().1)`) with proper error flattening: `.and_then(\|inner\| inner.map_err(\|e\| anyhow::Error::from(e)))`.
  - **Two example callers** at [examples/legacy/window.rs:203,221](crates/flui-core/examples/legacy/window.rs) — currently `let answer = window.prompt(...); answer.await.unwrap()`. Update to:
    ```rust
    let answer = window.prompt(...).expect("documented as never re-entrant in this example flow");
    ```
    (or change the closure to be fallible and use `?` if the closure already returns `Result`).
  - **Untouched**: `Platform::prompt` trait at [platform.rs:641](crates/flui-core/src/platform.rs#L641) and 7 platform impls.
  - Audit `Window::prompt` callers via `git grep "\.prompt("` filtered by file. Each gains `?` or `.expect("documented as never re-entrant: <reason>")`.

- [x] **Task 10.** `AsyncApp::run_update` chain widening ([app/async_context.rs:90](crates/flui-core/src/app/async_context.rs#L90)):
  - With `From<BorrowMutError> for ReentryError` from Task 3, the `?` operator's `From` chain reshapes automatically: `BorrowMutError → ReentryError → anyhow::Error` (via blanket).
  - Likely zero source-code changes here. Verify in Task 2 audit.
  - **Documented limitation:** the 10+ remaining `borrow_mut()` sites in `async_context.rs` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182) are NOT structured by K15. K07 redesigns this surface. Documented in design spec under "Known Limitations".

- [x] **Task 11.** Update THREE platform deferral comments to reference K15 (revision 3 — fixes migration H5):
  - [platform/mac/platform.rs:500-502](crates/flui-core/src/platform/mac/platform.rs#L500): rephrase to "Defer the close callback to the next run loop iteration to satisfy the K15 re-entrancy contract (`flui_core::reentrancy`); calling `Platform::quit` while holding an `AppCell` borrow would otherwise hit `ReentryError::AppBorrowed`."
  - [platform/mac/platform.rs:1254](crates/flui-core/src/platform/mac/platform.rs#L1254): similar rephrasing for thermal-state-change deferral.
  - [platform/windows/platform.rs:452-453](crates/flui-core/src/platform/windows/platform.rs#L452): similar.

### Phase 4 — Test infrastructure & property tests

- [~] **Task 12.** Add `crates/flui-core/src/reentrancy/tests.rs` (or `crates/flui-core/tests/reentrancy.rs`) using existing `proptest` dev-dep:
  - `prop_nested_update_window_same_target_returns_structured_error_in_strict` — random sequences; same-target nesting returns `Err`; different-target nesting succeeds.
  - `prop_nested_update_entity_same_target_panics_with_structured_error` — observer fires `update_entity` for same entity; `std::panic::catch_unwind` asserts message matches `ReentryError::NestedEntityUpdate` Display.
  - `prop_multi_entity_cycle_panics_with_unified_message` — A→B→A; same Display as direct re-entry (revision 3 unification check).
  - `prop_setState_inside_did_update_widget_simulated` — SF05-shape: callback A calls `update_entity(B)` which calls `update_entity(A)`. Asserts unified panic.
  - `prop_with_element_state_recursive_panics_with_global_element_id` — recursive same-key call panics; message includes `GlobalElementId` Debug, NOT `ElementId`.
  - `prop_prompt_reentry_returns_structured_error` — second `Window::prompt` returns `Err(ReentryError::PromptInProgress)`.

- [x] **Task 13.** Unit tests for cases proptest doesn't cover with high signal (revision 3 expansions for adversarial findings):
  - `nested_update_window_for_DIFFERENT_window_runs_synchronously` (positive case).
  - `cx_defer_from_observer_does_not_panic_and_drains_after_observer_returns`.
  - `window_defer_from_listener_does_not_panic`.
  - `next_frame_callback_calling_update_window_returns_error`.
  - `release_listener_calling_update_entity_panics_with_structured_error`.
  - `WindowUpdateGuard_drop_pops_stack_even_on_panic` — `std::panic::catch_unwind` wraps a guarded scope with a panicking update closure; assert `window_update_stack` is empty after.
  - `WindowUpdateGuard_commit_pop_runs_before_window_closed_observers` — observer registered via `cx.on_window_closed(...)` reads `cx.window_update_stack` length; assert it sees the closing window id ABSENT (existing semantics preserved). (revision 3 — pins arch B2 + migration H1.)
  - `observe_in_callback_fires_during_outer_window_update` — observer registered via `observe_in` while window is being updated; trigger notification; assert callback fires (or, if K15 forces `Err`, that the documented warning message appears in the log capture). (revision 3 — pins migration H2.)
  - `subscribe_in_callback_handles_event_during_outer_window_update` — same as above for `subscribe_in`.
  - `mode_propagates_through_AppContext_implementors` — set `Strict` on App; verify that `TestAppContext::update_entity`, `VisualTestContext::update_window`, `HeadlessAppContext::update_window`, `AsyncApp::update_window` all observe the mode. (revision 3 — pins arch + api MS5.)
  - `App_set_reentry_mode_setter_works` — basic positive case.
  - `entity_map_double_lease_panic_uses_unified_display` — manual call to `lease` on already-leased entity asserts the panic message is `ReentryError::NestedEntityUpdate(_)` Display, NOT the old "cannot update <T>..." text.

### Phase 5 — Documentation & spec

- [x] **Task 14.** Author design spec at `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md` following project convention (reference `2026-05-08-K99-msrv-bump-1.95-design.md` for header/section style). Required sections:
  - Context, Goals, Non-goals.
  - Current state (audit table).
  - Design (contract matrix, `ReentryError` enum, `ReentryMode` enum, RAII shapes incl. `commit_pop` two-phase).
  - **Decision log** — explicitly documents revision 2 narrowing (no `Effect::ScheduledUpdate`, no `update_depth`, no `currently_updating_window`) AND revision 3 narrowing (no `PanicLikeUpstream`, no `legacy-reentry-panics` feature, no explicit `From<ReentryError> for anyhow::Error`, `double_lease_panic` unification, `WindowUpdateGuard::commit_pop` two-phase).
  - API surface (every new `pub` symbol; explicit list of breaking changes: `Window::prompt`, `AsyncWindowContext::prompt`).
  - Migration / compatibility (the two example callers, the silent-loss audit on `observe_in`/`subscribe_in`).
  - Testing (proptest + unit tests).
  - **Known Limitations** (the 4 documented gaps from "Known Limitations" section of this plan).
  - Open questions (resolved via revision 3 — leave only K07-handoff items: `PanicLikeUpstream` deferral cadence, AsyncApp surface redesign).
  - Done criteria (verbatim from this plan).
  - Cross-references (K99 spec, ROADMAP K15, RESEARCH Active Summary, `docs/promt.md` §3.1, the THREE platform comment sites).
  - Unblocks (K07, K01, K05, SF02, SF05).

- [x] **Task 15.** Update `.ai-factory/RESEARCH.md` Active Summary with one-paragraph K15 entry. Mention: contract published at `crates/flui-core/src/reentrancy.rs`; `ReentryMode { Strict, Loose }` two-mode (PanicLikeUpstream deferred to K07); `cx.defer` / `window.defer` are the only Queue path; entity-side panic unified via `ReentryError::NestedEntityUpdate` Display; 4 known limitations documented.

- [x] **Task 16.** Mark K15 done in `.ai-factory/ROADMAP.md` — checkbox flip at line 57; completion-date row in `## Completed` table at line 192.

- [~] **Task 17.** Run `/aif-docs` to absorb rustdoc / README drift. Confirm `cargo doc --workspace --no-deps` zero new warnings vs Task 1 baseline.

### Phase 6 — Validation gates

- [x] **Task 18.** `cargo build --workspace --all-features` green.
- [x] **Task 19.** `cargo test --workspace` green. Pre-existing test count increases by ≥ 17 (6 proptests + 11 unit tests). Verify Strict mode is the test default — explicitly toggle to `Loose` in one test to assert `log::warn!` fires.
- [x] **Task 20.** `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings.
- [x] **Task 21.** `cargo fmt --all -- --check` clean.
- [x] **Task 22.** `cargo doc --workspace --no-deps` zero new warnings; `flui_core::reentrancy` rustdoc reads as the contract document (not a stub).
- [~] **Task 23.** Manual smoke: run `cargo run --example nav_demo` ~30 seconds with `RUST_LOG=flui_core::reentrancy=trace`. Verify zero `warn!` events under normal navigation.

### Phase 7 — Adversarial re-review

- [ ] **Task 24.** Dispatch **`flui-arch-reviewer`** subagent on the K15 implementation diff (post-revision-3 plan). The agent's prompt: "Re-review the K15 implementation against the revision-3 plan. Key invariants to verify post-implementation: (a) `WindowUpdateGuard::commit_pop` is invoked in BOTH `update_window_id::trail()` and `open_window` BEFORE any observer callbacks; (b) `EntityMap::double_lease_panic` produces unified Display; (c) `AsyncWindowContext::prompt` no longer swallows errors; (d) the 4 Known Limitations are present in the design spec, not silent." → Depends on Task 14 (spec) + Task 17 (docs sweep).

- [ ] **Task 25.** Dispatch **`migration-risk-adversary`** subagent on the K15 diff. Prompt: "Re-review the K15 implementation. Specifically validate the migration-risk findings from the planning-phase review are addressed: (B1) entity-side double-lease unified, (B2) `AsyncWindowContext::prompt` flattening correct, (B3) `examples/legacy/window.rs` callsites compile and run, (H2) `observe_in` / `subscribe_in` silent loss converted to logged loss, (H5) third platform comment updated. Find any NEW regression introduced by revision 3 itself." → Depends on Task 14 + Task 17.

- [ ] **Task 26.** Triage findings from Tasks 24-25. (a) accept and patch, (b) split to follow-up K-spec, or (c) reject with documented reason in design spec.

## Task Dependencies

```
   1. Baseline → 2. Audit cites & verify Entity::update chain
        │           │
        │           ▼
        ├──► 3. ReentryError + module + RAII guards (two-phase) + From impls + prelude
        │           │
        │           ▼
        ├──► 4. App fields (currently_updating_entity, reentry_mode, set_reentry_mode)
        │           │
        │           ├──► 5. WindowUpdateGuard at TWO sites (update_window_id + open_window)
        │           ├──► 6. EntityUpdateGuard in update_entity
        │           ├──► 7. EntityMap::double_lease_panic unification (NEW Task)
        │           ├──► 8. with_element_state → ReentryError::ElementStateInUse (GlobalElementId)
        │           ├──► 9. Window::prompt + AsyncWindowContext::prompt widening + 2 example callsites
        │           ├──► 10. AsyncApp chain widening (likely zero diff)
        │           └──► 11. THREE platform comment updates
        │                       │
        │                       ▼
        │                 12. Property tests (6 props)
        │                 13. Unit tests (11 unit tests)
        │                       │
        │                       ▼
        │                 18. cargo build
        │                       │
        │                       ├──► 19. cargo test
        │                       ├──► 20. cargo clippy
        │                       ├──► 21. cargo fmt
        │                       ├──► 22. cargo doc
        │                       └──► 23. example smoke
        │                                       │
        │                                       ▼
        │                                  14. Design spec (rev-3 Decision log)
        │                                       │
        │                                       ├──► 15. RESEARCH.md addendum
        │                                       └──► 16. ROADMAP closure
        │                                                   │
        │                                                   ▼
        │                                            17. /aif-docs checkpoint
        │                                                   │
        │                                                   ▼
        │                                            24. flui-arch-reviewer (re-review) ║ 25. migration-risk-adversary (re-review)
        │                                                   │
        │                                                   ▼
        │                                            26. Adversary triage
        ▼
   (final commit + PR)

   Parallel-eligible after Task 4:
     5 ║ 6 ║ 7 ║ 8 ║ 9 ║ 10 ║ 11   (independent file/symbol surfaces)
   Parallel-eligible after Tasks 5-11:
     12 ║ 13
   Parallel-eligible after Task 17:
     24 ║ 25
```

## Commit Plan

K15 has **26 tasks** (revision 3: -3 obsolete from rev 1 + +4 new from adversarial review = +1 net vs rev 2's 25). Per skill convention, commit checkpoints every 3-5 tasks. Each commit green at HEAD — `cargo build` + `cargo test` MUST pass.

| # | Tasks | Conventional commit message |
|---|---|---|
| 1 | 1, 2 | `chore(k15): pre-flight audit and contract shape verification` (squash with #2 if no in-plan edits) |
| 2 | 3, 4 | `feat(reentrancy)!: introduce flui_core::reentrancy module + ReentryError + two-phase RAII guards` (BREAKING — public re-export added to prelude) |
| 3 | 5 | `feat(reentrancy)!: enforce update_window contract via WindowUpdateGuard at update_window_id + open_window` (BREAKING — same-window re-entry now raises ReentryError) |
| 4 | 6, 7 | `feat(reentrancy)!: unify entity re-entry panic via EntityUpdateGuard + double_lease_panic rewrite` (BREAKING — entity re-entry message changes from "cannot update <T>..." to ReentryError::NestedEntityUpdate Display) |
| 5 | 8, 9 | `refactor(reentrancy)!: convert with_element_state to GlobalElementId variant; widen Window::prompt and AsyncWindowContext::prompt` (BREAKING — Window::prompt and AsyncWindowContext::prompt return types widen; 2 example callsites updated) |
| 6 | 10, 11 | `docs(platform): reference K15 contract from THREE mac+windows reentry workarounds` (one-line rephrasing × 3) |
| 7 | 12, 13 | `test(reentrancy): proptest + 11-unit-test coverage incl. observe_in audit, mode propagation, drop ordering` |
| 8 | 14, 15 | `docs(spec): add K15 design spec with revision-3 Decision log; update RESEARCH.md` |
| 9 | 16, 17 | `docs(roadmap): mark K15 complete; /aif-docs sweep` |
| 10 | 18-23 | `chore(k15): validation pass — build/test/clippy/fmt/doc/smoke` (likely empty — fold into commit 8 or 9 if no fixups) |
| 11 | 24, 25, 26 | `docs(reentrancy): adversary re-review triage` (any post-implementation changes from agents) |

If commits 1 or 10 are empty, drop them. **Rollback note:** Commits 2-onward have forward type-dependencies; rollback of K15 = revert all-as-unit (migration-risk finding).

## Done criteria

K15 is done when:

1. ✅ `crates/flui-core/src/reentrancy.rs` module exists with `ReentryError`, `ReentryMode { Strict, Loose }` (NO `PanicLikeUpstream`), `WindowUpdateGuard` (two-phase with `commit_pop`), `EntityUpdateGuard`, `From<BorrowMutError> for ReentryError`. Module-level rustdoc IS the contract document.
2. ✅ `ReentryError` is `#[non_exhaustive]`, derives `thiserror::Error`. `ElementStateInUse` carries `global_element_id: GlobalElementId` (NOT `ElementId`).
3. ✅ `ReentryMode` is `#[non_exhaustive]`, derives `Clone, Copy, Debug, PartialEq, Eq`, manual `Default = Loose`.
4. ✅ NO explicit `impl From<ReentryError> for anyhow::Error` (relies on anyhow blanket).
5. ✅ Both `ReentryError` and `ReentryMode` re-exported through `flui_core::prelude`. `App::set_reentry_mode` public setter exists.
6. ✅ `App` carries `currently_updating_entity` and `reentry_mode` fields, initialized in `App::new_app` with `Strict` in `cfg(test)`.
7. ✅ `WindowUpdateGuard` integrated at BOTH `App::update_window_id` AND `App::open_window`. `commit_pop` is called BEFORE `window_closed_observers` in `update_window_id::trail()`. Drop is panic-safety fallback.
8. ✅ `App::update_entity` uses `EntityUpdateGuard`; same-entity re-entry panics with `ReentryError::NestedEntityUpdate(_)` Display.
9. ✅ `EntityMap::double_lease_panic` rewritten to use `ReentryError::NestedEntityUpdate(entity_id)` Display (Task 7 unification). Multi-entity cycles produce the same message.
10. ✅ `with_element_state` recursive call panics with `ReentryError::ElementStateInUse { global_element_id, type_id }` Display; 7 callsites audited, retained `expect`/`unwrap_or_else` carry `// SAFETY-CONTRACT(K15)` comments.
11. ✅ `Window::prompt` returns `Result<oneshot::Receiver<usize>, ReentryError>`; re-entry produces `Err(ReentryError::PromptInProgress)`. 7 platform `Platform::prompt` impls UNCHANGED.
12. ✅ `AsyncWindowContext::prompt` returns `Result<oneshot::Receiver<usize>, anyhow::Error>`; no longer swallows errors.
13. ✅ Two example callers at `examples/legacy/window.rs` updated to handle the new `Result`.
14. ✅ `AsyncApp::run_update` propagates `ReentryError` via the `From<BorrowMutError>` chain; existing `?`-callers compile unchanged.
15. ✅ THREE platform comment sites reference the K15 contract.
16. ✅ Property tests (≥ 6) + unit tests (≥ 11) cover: same-window re-entry, same-entity direct re-entry, multi-entity cycle, setState-cycle simulation, recursive `with_element_state`, recursive `prompt`, `commit_pop` ordering, `observe_in` / `subscribe_in` audit, mode propagation through 4 AppContext implementors, `set_reentry_mode` setter, unified `double_lease_panic` Display.
17. ✅ `cargo build --workspace --all-features` green.
18. ✅ `cargo test --workspace` green; test count increases by ≥ 17.
19. ✅ `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings vs Task 1 baseline.
20. ✅ `cargo fmt --all -- --check` clean.
21. ✅ `cargo doc --workspace --no-deps` zero new warnings.
22. ✅ Design spec at `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md` exists; "Decision log" section documents both rev-2 and rev-3 narrowings; "Known Limitations" enumerates 4 accepted gaps.
23. ✅ `.ai-factory/RESEARCH.md` Active Summary has K15 entry.
24. ✅ ROADMAP K15 entry checked off; completion-date row added.
25. ✅ `/aif-docs` checkpoint completed.
26. ✅ `flui-arch-reviewer` and `migration-risk-adversary` re-reviews completed; findings either patched, split into follow-up K-spec, or rejected with documented reason.
27. ✅ Manual smoke (~30s, `RUST_LOG=flui_core::reentrancy=trace`) produces zero `ReentryError::*` events under normal navigation.

## Open questions (resolved or accepted)

- **`ReentryMode::PanicLikeUpstream` cadence?** — DEFERRED to K07 (revision 3). K07's PR adds it if needed. Filed in K07 plan as explicit checklist.
- **`with_element_state` panic-shape vs `Result`?** — KEEP panic shape. 7 callsites unchanged; structured Display satisfies ROADMAP intent.
- **Strict mode default in `cfg(test)`?** — YES.
- **Multi-entity A-B-A cycle?** — RESOLVED (revision 3): unified `double_lease_panic` produces same Display as direct re-entry. K15's `currently_updating_entity` catches direct A-A; cycles fall through to `EntityMap::lease` which now uses the unified message.
- **`AsyncApp` 10+ remaining `borrow_mut` sites?** — ACCEPTED gap. K07 redesigns this surface. Documented in Known Limitations.
- **`observe_in` / `subscribe_in` silent loss?** — RESOLVED (revision 3): `.unwrap_or(false)` becomes `.map_err(\|e\| log::warn!(...)).unwrap_or(false)`; tests assert callback fires (or warning logged).

## Risk assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `Window::prompt` / `AsyncWindowContext::prompt` signature widening breaks downstream | Medium | Low-Medium | Task 9 explicitly updates 2 example callers; Task 25 adversary re-review enumerates remaining sites |
| Existing test code depends on `BorrowMutError` panic message text | Low-Medium | Low | Task 7 unifies `double_lease_panic` Display — pre-K15 tests checking the old text WILL break. Migrate in same PR or document as expected breakage |
| `WindowUpdateGuard::commit_pop` not called in some path | Low | High (silent stack corruption) | Drop fallback always pops; Task 13 unit test `WindowUpdateGuard_drop_pops_stack_even_on_panic` covers panic path; Task 13 unit test `WindowUpdateGuard_commit_pop_runs_before_window_closed_observers` pins ordering |
| `observe_in` / `subscribe_in` audit reveals more silent-loss sites than the 2 known | Medium | Medium | Task 13 tests cover the 2 known; if Task 25 adversary review finds more, escalate to follow-up K-spec |
| `Strict` mode in `cfg(test)` reveals existing tests doing nested same-window updates that previously silently worked | Medium | Low-Medium | Test code is cheapest place to fix; if count > 5, escalate to hygiene task |
| Property tests reveal genuine pre-existing bugs | Medium | Medium-High | Task 26 splits unrelated bugs into follow-up K-specs |
| K07 invalidates K15's enforcement points | Low (sequential) | Low | K15 is K07's input contract; worst case `WindowUpdateGuard` / `EntityUpdateGuard` relocate |
| Cross-spec collision with K94 (prelude expansion) | Low | Low | Task 3 cross-references K94 |

## Refinement record

### Revision 1 (2026-05-09 initial draft)
- 28 tasks. Proposed `Effect::ScheduledUpdate` variant, `update_depth: u8` field, `currently_updating_window: Option<WindowId>` field, three modes including `PanicLikeUpstream`. Used `tracing::*` macros.

### Revision 2 (2026-05-09 aif-improve)
- 25 tasks (-3). Removed `Effect::ScheduledUpdate` (incompatible with generic `T`/`R`), removed redundant `update_depth` and `currently_updating_window` fields (existing `pending_updates` and `window_update_stack` cover them). Switched `tracing::*` → `log::*` to match codebase. Corrected `with_element_state` count to 7. Clarified `Window::prompt` topology. Acknowledged 5 `AppContext` implementors. Added `WindowUpdateGuard` panic-safety benefit.

### Revision 3 (2026-05-09 3-agent adversarial review)
22 findings from `flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`. All BLOCKERs (9) patched into tasks; MAJORs (8) split between task additions and design-spec "Known Limitations":
- **Removed** `ReentryMode::PanicLikeUpstream` and `legacy-reentry-panics` feature — deferred to K07. Reasons: cannot faithfully reproduce upstream entity-side panic (which was `double_lease_panic`, not `BorrowMutError`); feature flag undeclared; runtime-field-for-compile-time-variant is dead weight.
- **Removed** explicit `impl From<ReentryError> for anyhow::Error` — conflicts with anyhow blanket impl.
- **Added** `WindowUpdateGuard` to `open_window` site (was only `update_window_id` before).
- **Added** `WindowUpdateGuard::commit_pop` two-phase design — preserves observer-visibility ordering (observers see the closing id ABSENT from stack, matching pre-K15 semantics).
- **Added** Task 7: rewrite `EntityMap::double_lease_panic` to use unified `ReentryError::NestedEntityUpdate` Display. Closes asymmetry; multi-entity cycles produce same message.
- **Added** explicit `AsyncWindowContext::prompt` signature widening + flatten — was previously claimed "zero diff" which was false (line 359 silently swallowed errors).
- **Added** 2-callsite update at `examples/legacy/window.rs` (compile-time breakage that Task 9 originally undersold).
- **Added** third platform comment site (`mac/platform.rs:1254` thermal-state).
- **Added** `App::set_reentry_mode` public setter — makes prelude `ReentryMode` re-export meaningful.
- **Added** `ReentryMode` `#[non_exhaustive]`, full derive set incl. `Copy`, manual `Default`.
- **Changed** `ReentryError::ElementStateInUse` field from `element_id: ElementId` to `global_element_id: GlobalElementId` — matches actual key at `window.rs:3118`.
- **Verified** `TestAppContext::update_entity` reaches funnel through `Entity::update → EntityMap::lease` (after Task 7 unification, the funnel carries the same structured panic).
- **Documented** 4 Known Limitations: `AsyncApp` 10+ remaining `borrow_mut`, `AsyncApp::as_mut` panic, `web` platform unverified re-entry exposure, `AppBorrowed` no source location.
- **Added** unit tests: `WindowUpdateGuard_commit_pop_runs_before_window_closed_observers`, `observe_in_callback_fires_during_outer_window_update`, `subscribe_in_callback_handles_event_during_outer_window_update`, `mode_propagates_through_AppContext_implementors`, `App_set_reentry_mode_setter_works`, `entity_map_double_lease_panic_uses_unified_display`.
- **Added** Phase 7: re-dispatch `flui-arch-reviewer` and `migration-risk-adversary` post-implementation to verify revision-3 invariants land correctly.

Net: 25 → 26 tasks, scope refined (no new `Effect`, no compat hatch in K15, unified entity panic), all 22 adversarial findings either patched or moved to documented Known Limitations.

## Next steps

After K15 lands and is merged:

```
/aif-plan full K07-appcell-removal-token-borrow
```

K07 inherits: `ReentryError` enum (gains new variants under K07 token model), `ReentryMode` (may add `PanicLikeUpstream` or other variants), the RAII-guard pattern (relocates with the borrow primitive change), and the 4 Known Limitations from K15 (especially `AsyncApp` surface redesign).
