# K01 - Provider rewrite

**Branch:** `feature/K01-provider-rewrite` (planned; not created by this planning run)
**Created:** 2026-05-11
**Phase:** 0-K (Kernel Cleanup) - fifth spec in the critical chain after K99, K15, K07, and K05.
**Type:** API-breaking engine/provider refactor in `flui-core`.
**Tasks:** 32 checkbox tasks.

> **Design-first spec.** K01 replaces the thread-local provider stack with a per-Window inherited-value registry. The first phase must lock the exact API and invalidation semantics before implementation, because this touches `Window`, `Element` lifecycle contexts, view caching, and the future Framework `BuildCx` surface.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | K01 changes inherited-value scoping, nested provider behavior, cached-view invalidation, and window isolation. Unit and integration tests are required. |
| Logging | verbose during implementation, no committed hot-path logs | Temporary DEBUG diagnostics are useful while tracing invalidation, but committed provider reads/writes happen on layout/prepaint/paint paths and must not log per access. Keep only sparse non-hot-path diagnostics if needed. |
| Docs | yes (mandatory checkpoint) | K01 is API-breaking and unblocks SF03. The design spec, migration guide, rustdoc, ROADMAP, RESEARCH, AGENTS, and CHANGELOG need a docs checkpoint. |
| Roadmap linkage | linked | K01 is the next Phase 0-K critical-chain item and unblocks K02, K03, K04, and SF03. |

## Roadmap Linkage

**Milestone:** K01 Provider rewrite - per-Window InheritedRegistry, reactive (Phase 0-K Kernel Cleanup, critical chain).

**Rationale:** `.ai-factory/ROADMAP.md` names K01 as the next item after K05. K01 replaces `crates/flui-core/src/provider/stack.rs` with a per-Window registry, closes E1 from `docs/promt.md`, and gives SF03 the Provider substrate that Framework `BuildCx` will wrap later.

K01 must stay inside the current Tier A compatibility surface because `flui-framework` does not exist yet. It must not introduce the full Framework tier (`Widget`, `State`, reconciliation, dirty-list, or final `BuildCx` ergonomics). The plan should name the future SF03 wrapper path explicitly so K01 does not grow into a Framework implementation.

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `.ai-factory/ARCHITECTURE.md`, `docs/promt.md` section 4.4, and current provider/window code.

- K05 is complete. `LayoutCx`, `PrepaintCx`, and `PaintCx` now provide clean lifecycle access to element identity, bounds, `Window`, and `App`.
- Current Provider lives in `crates/flui-core/src/provider/stack.rs` as `thread_local! HashMap<TypeId, Vec<Box<dyn Any>>>`.
- `ProviderElement<T>` currently pushes in `request_layout` and pops in `paint`, which is fragile across panics and not window-isolated.
- Current provider reads are global free functions: `flui_core::read::<T>()` and `try_read::<T>()`. K01 should replace these with context/window-scoped reads.
- `crates/flui-widgets/src/lib.rs` already re-exports `InheritedValue`, `Provider`, `read`, and `try_read`; K01 must migrate downstream public shims, not only `flui-core`.
- Current view caching invalidates from `Context::notify()` and accessed-entity tracking. K01 needs a view-level invalidation bridge for provider dependents until K02/K04 introduce stronger element identity/frame contracts.
- K15 established that re-entry-sensitive work should use `cx.defer` / `Window::defer` rather than nested updates.

## Current State

| Area | Current shape | K01 concern |
|---|---|---|
| Provider storage | Thread-local global stacks in `provider/stack.rs` | Values leak across windows/tests conceptually and are not tied to draw lifecycle ownership. |
| Provider element | Pushes during `request_layout`, pops during `paint` | Panic between phases can corrupt the stack; phase scope is implicit and brittle. |
| Provider identity | `ProviderElement<T>::id()` currently returns `None` | Any registry keyed by provider `GlobalElementId` is invalid until K01 deliberately introduces or derives a stable provider scope key. |
| Read API | `read::<T>()` / `try_read::<T>()` free functions | No `Window`, `App`, view, or element identity context, so subscriptions cannot be registered correctly. |
| Reactivity | None | Provider changes do not notify dependents; cached child views can reuse stale output. |
| Cached views | `AnyViewState` stores accessed entities but no inherited dependencies | Provider subscriptions can disappear when a cached view reuses prepaint/paint output unless dependencies are stored and replayed. |
| Identity | `GlobalElementId` exists from current `ElementId` stack; K02 has not landed | K01 can use `GlobalElementId` and current view id, but must document that K02 improves key stability. |
| View invalidation | `WindowInvalidator` and `dirty_views` operate at view granularity | K01 should invalidate dependent views, not invent element-level dirty-list semantics before K02/K04. |
| Framework boundary | `flui-framework` absent | K01 should provide low-level registry operations and leave final `BuildCx::inherit<T>()` ergonomics to SF03. |
| Downstream public surface | `flui-widgets` re-exports the provider API | Workspace green is not enough if Tier C exports stale provider names or broken shims. |

## Implementation Inventory

Task 1 inventory findings:

| Surface | Current location | Current behavior | K01 action |
|---|---|---|---|
| `InheritedValue` trait | `crates/flui-core/src/provider/mod.rs` | Blanket-implemented for `Any + Clone + Send + Sync + 'static`; no equality or notification hook. | Extend or replace with explicit notification semantics before registry invalidation lands. |
| `Provider<T>` construction API | `crates/flui-core/src/provider/element.rs` | `Provider::new(value, child)` renders through `Component<Self>` and currently gives the provider element no identity. | Keep the construction role, add stable K01 scope identity, and document explicit-key requirements before K02. |
| Thread-local provider stack | `crates/flui-core/src/provider/stack.rs` | `thread_local!` map from `TypeId` to `Vec<Box<dyn Any>>`, with typed `push`, `pop`, `try_read`, and panic-on-missing `read`. | Retire as production storage; replace with per-`Window` `InheritedRegistry`. |
| Provider lifecycle caller | `crates/flui-core/src/provider/element.rs` | Pushes provider value in `request_layout`; pops in `paint`; `prepaint` relies on cross-phase stack state. | Replace with independent phase-scoped activation around child layout, prepaint, and paint. |
| Public `flui-core` exports | `crates/flui-core/src/lib.rs` | Re-exports `InheritedValue`, `Provider`, `read`, and `try_read`. | Re-export the new scoped provider API intentionally; remove or migrate stale free reads. |
| Downstream `flui-widgets` exports | `crates/flui-widgets/src/lib.rs` | Re-exports `InheritedValue`, `Provider`, `read`, and `try_read` from `flui_core`. | Migrate alongside `flui-core`; no stale Tier C shim is acceptable. |
| Current provider tests | `crates/flui-core/src/provider/stack.rs` | Unit tests only cover stack push/pop, shadowing, missing provider, and multiple types. | Replace with registry, identity, lifecycle, reactivity, caching, panic-safety, and multi-window tests. |
| Examples | `examples/*` | No direct provider `read` / `try_read` usage found during inventory. | Keep compile checks in verification to catch indirect public-surface breakage. |
| Docs and roadmap references | `.ai-factory/*`, `docs/**/*.md`, `AGENTS.md` | K01 is described as the next critical-chain item; existing docs mention broken Provider/thread-local stack as debt. | Add K01 spec and migration guide, then update status artifacts after code lands. |

## Target Design

K01 introduces a per-Window inherited-value registry under `crates/flui-core/src/provider/registry.rs`, but the design spec must freeze the exact data model before implementation. The plan must not treat the sketch below as a license to code the first convenient `Box<dyn Any>` map:

```rust
pub struct InheritedRegistry {
    entries: FxHashMap<ProviderScopeKey, InheritedEntry>,
    active_by_type: FxHashMap<TypeId, SmallVec<[ProviderScopeKey; 4]>>,
}

struct ProviderScopeKey {
    type_id: TypeId,
    scope_id: GlobalElementId,
}

struct InheritedEntry {
    value: ProviderValue,
    version: u64,
    dependents: SmallVec<[InheritedDependent; 4]>,
    seen_in_frame: bool,
}

struct InheritedDependent {
    element_id: GlobalElementId,
    view_id: EntityId,
    last_seen_version: u64,
}
```

The implementation may differ, but the spec must explicitly decide:

- **Provider identity:** `ProviderElement<T>::id()` is currently `None`. K01 must either introduce an explicit stable provider scope id or choose a registry model that does not depend on provider `GlobalElementId`. Sibling same-type providers and nested same-type providers must be distinguished.
- **Value semantics:** choose clone-returning reads, closure-borrowed reads, `Arc<T>` values, or another model. The choice must address allocation, lifetime safety, current `InheritedValue: Clone + Send + Sync + 'static`, and future `BuildCx::read<T>()` ergonomics.
- **Notification semantics:** decide whether unchanged values are detected by `PartialEq`, a custom `should_notify`, explicit provider versioning, or always-notify. This must be stable enough for Theme/MediaQuery/DefaultTextStyle later.
- **Active-scope semantics:** provider activation is scoped separately around child `request_layout`, child `prepaint`, and child `paint`; no guard may be held across lifecycle phases.
- **Dependency storage:** subscribing reads record enough information to invalidate the current view now and to migrate to element-level dirty lists after K02/K04.
- **Cached-view replay:** provider dependencies discovered while rendering an `AnyView` must survive cached reuse, just as accessed entities are replayed today. A cached view must not silently drop inherited subscriptions.
- **Removal semantics:** changed provider value, unchanged provider value, provider removal, and dependent subtree removal are different cases and must be tested separately.
- **Public API migration:** `flui-core` and `flui-widgets` public re-exports must move together, with a deliberate deprecation/removal policy for `read::<T>()` / `try_read::<T>()`.
- **Hot-path discipline:** no committed per-read/per-provide logs, no unbounded dependent growth, and no new `Rc<RefCell<...>>` on dispatch/tick/paint hot paths.

## Implementation Validation Notes

- Task 30 hot-path check: `rg` over `crates/flui-core/src/provider` found no production `dbg!`, `println!`, `eprintln!`, `log::`, or `tracing::` calls. `Rc<RefCell<...>>` appears only in `#[cfg(test)]` provider lifecycle probes.
- Provider reads clone the active `GlobalElementId` `Arc` into a stack-local `ProviderScopeKey` and clone the inherited value by contract. Provider value storage allocates only on first insert or changed-value replacement (`Box::new`), not on unchanged provider visits.
- Dependent lists and dirty-view collections use `SmallVec`; per-frame accessed dependency/provider vectors are capacity-reusing registry fields and are cleared at frame start.
- Registry cleanup is bounded by current registry entries and current-frame accessed dependency snapshots; stale providers are removed before live-provider dependent pruning so removal invalidation is not lost.

## Tasks

### Phase 1: Design and Inventory

- [x] Task 1: Inventory the current Provider API, internal callsites, and downstream re-exports.
  - Deliverable: plan/spec inventory of every public provider export, internal `stack::{push,pop,read,try_read}` caller, provider docs snippet, tests, examples, and downstream re-export.
  - Files to inspect: `crates/flui-core/src/provider/mod.rs`, `crates/flui-core/src/provider/stack.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/lib.rs`, `crates/flui-widgets/src/lib.rs`, `examples/*`, `docs/**/*.md`.
  - Logging requirements: no runtime logs. Record noteworthy command output in the spec or plan notes only.

- [x] Task 2: Author the K01 design spec.
  - Deliverable: `docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md`.
  - Must include: target registry model, provider identity decision, value/read semantics, notification semantics, cached-view dependency replay, provider removal semantics, nested provider behavior, view-level invalidation bridge, panic-safety policy, public API breakage, migration path, SF03 handoff, rejected alternatives, and known limitations.
  - Logging requirements: specify that provider operations are hot-path and should not log per read/write in committed code.

- [x] Task 3: Resolve provider identity and scope keys before implementing storage.
  - Deliverable: spec decision and code plan for how a `Provider<T>` receives a stable scope key, despite `ProviderElement<T>::id()` currently returning `None`.
  - Must cover: sibling same-type providers, nested same-type providers, source-location fallback, explicit provider keys if needed, what changes after K02, and behavior when a provider cannot get a stable id.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no runtime logs; identity failures should be deterministic errors, debug assertions, or documented non-subscribing fallbacks.

- [x] Task 4: Resolve inherited value storage, read, and notification semantics.
  - Deliverable: spec decision for whether reads return cloned values, borrowed values through closures, `Arc<T>`, or another representation; decide whether notification uses `PartialEq`, `should_notify`, explicit versioning, or always-notify.
  - Must cover: current `InheritedValue: Any + Clone + Send + Sync + 'static`, future Theme/MediaQuery use, hot-path allocation expectations, object safety, and API ergonomics for SF03 `BuildCx`.
  - Files: `crates/flui-core/src/provider/mod.rs`, `crates/flui-core/src/provider/element.rs`, new `crates/flui-core/src/provider/registry.rs`.
  - Logging requirements: no runtime logs; API rationale belongs in the spec and rustdoc.

- [x] Task 5: Define the inherited dependency model, including cached-view replay.
  - Deliverable: spec design for how subscribing reads record provider dependencies, how those dependencies are stored in `AnyViewState`, and how they are replayed when cached view prepaint/paint output is reused.
  - Must cover: relation to `App::detect_accessed_entities`, `Window::record_entities_accessed`, `dirty_views`, `WindowInvalidator`, and K02/K04 future element-level dirty lists.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs`, new `crates/flui-core/src/provider/registry.rs`.
  - Logging requirements: no runtime logs; use spec diagrams or tables instead of instrumentation.

- [x] Task 6: Classify panic-safety and lifecycle cleanup.
  - Deliverable: spec section describing current failure modes and the new phase-scoped guard behavior for provider activation, including what happens when child layout/prepaint/paint panics.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/element.rs`.
  - Logging requirements: no committed logs; use tests to prove cleanup rather than logging cleanup events.

- [x] Task 7: Freeze the review gates.
  - Deliverable: checklist in the spec/plan requiring `flui-arch-reviewer`, `migration-risk-adversary`, and `rust-api-migration-auditor` before commit. `wgpu-gpu-reviewer` is not required unless implementation unexpectedly touches scene/wgpu/Metal/DirectX/offscreen rendering.
  - Logging requirements: no runtime logs; review evidence belongs in PR notes or the plan.

### Phase 2: Registry Substrate

- [x] Task 8: Add the per-Window registry module using the spec-locked data model.
  - Deliverable: `InheritedRegistry`, provider scope keys, erased or typed value storage, dependent records, active stack operations, and focused unit tests for stack behavior.
  - Files: `crates/flui-core/src/provider/registry.rs` (new), `crates/flui-core/src/provider/mod.rs`.
  - Logging requirements: no per-operation logs. Temporary DEBUG logs may be used while developing tests and removed before commit.

- [x] Task 9: Add provider scope identity plumbing.
  - Deliverable: `ProviderElement<T>` and/or the registry can derive the chosen provider scope key consistently during request-layout, prepaint, and paint. If `ProviderElement<T>::id()` changes, document the public behavior and inspector implications.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no runtime logs; use debug assertions for identity-stack invariants.

- [x] Task 10: Attach the registry to `Window` and its frame lifecycle.
  - Deliverable: `Window` owns an `InheritedRegistry`, initializes it in `Window::new`, exposes narrow `pub(crate)` methods, and calls begin/end-frame hooks so stale provider/dependent cleanup has a deterministic lifecycle.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/provider/registry.rs`.
  - Logging requirements: no hot-path logs in `Window` accessors; if cleanup detects an invariant violation, prefer `debug_assert!` or test-only diagnostics.

- [x] Task 11: Implement phase-scoped provider activation.
  - Deliverable: `ProviderElement<T>` activates its provider value around child `request_layout`, `prepaint`, and `paint` separately, preserving nearest-provider-wins behavior while eliminating cross-phase push/pop state.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/provider/registry.rs`.
  - Logging requirements: no lifecycle logs in committed code; panic-safety is covered by tests.

- [x] Task 12: Replace thread-local stack storage.
  - Deliverable: remove or retire `provider/stack.rs`, stop exporting thread-local free reads, and update module rustdoc to describe per-window inherited values.
  - Files: `crates/flui-core/src/provider/stack.rs`, `crates/flui-core/src/provider/mod.rs`, `crates/flui-core/src/lib.rs`.
  - Logging requirements: no runtime logs; migration messages belong in docs or compile-time deprecation notes if any compatibility shim remains.

- [x] Task 13: Add scoped read and inherit APIs.
  - Deliverable: context/window-scoped APIs for non-subscribing read and subscribing inherit. The implementation should work from the low-level engine lifecycle now and leave final `BuildCx::read<T>()` / `BuildCx::inherit<T>()` wrappers to SF03.
  - Candidate files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/provider/mod.rs`.
  - Logging requirements: no per-read logs. If subscribing without a stable id is supported as a fallback, use at most debug-only diagnostics and document the behavior.

- [x] Task 14: Implement provider value change detection and dependent invalidation.
  - Deliverable: provider value updates bump versions only according to the chosen notification rule, collect affected dependents, and mark their views dirty through the existing window invalidation path without nested window/entity updates.
  - Files: `crates/flui-core/src/provider/registry.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs` if an effect hook is required.
  - Logging requirements: temporary verbose logs may trace provider id, type id, version, and dependent count during development; committed code must avoid hot-path logs and use tests/assertions for behavior.

- [x] Task 15: Implement provider removal and dependent-subtree cleanup semantics.
  - Deliverable: disappearing providers invalidate previous dependents exactly once, removed dependent subtrees are pruned, and stale records do not grow unbounded across frames.
  - Files: `crates/flui-core/src/provider/registry.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/view.rs`.
  - Logging requirements: no cleanup logs in hot paths; use unit/integration tests to prove bounded cleanup and invalidation behavior.

- [x] Task 16: Add cached-view provider dependency capture and replay.
  - Deliverable: `AnyViewState` stores inherited dependencies discovered during rendering; reused cached prepaint/paint output re-registers or preserves provider dependencies without rerendering the subtree.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/provider/registry.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no view-cache logs in committed code; use focused assertions and frame counters in tests.

- [x] Task 17: Add test-only registry inspection helpers.
  - Deliverable: `#[cfg(any(test, feature = "test-support"))]` helpers expose active scopes, provider versions, dependent counts, and dirty-view decisions for tests without widening production API.
  - Files: `crates/flui-core/src/provider/registry.rs`, `crates/flui-core/src/window.rs`, possibly `crates/flui-core/src/app/test_app.rs`.
  - Logging requirements: no production logs; test helpers return data structures for assertions.

### Phase 3: API Migration and Integration

- [x] Task 18: Update `flui-core` provider public exports and rustdoc.
  - Deliverable: `flui_core::Provider` remains the construction API, inherited read APIs point users at `Window`/lifecycle context methods, and old free-function docs are removed or replaced with explicit migration guidance.
  - Files: `crates/flui-core/src/provider/mod.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/lib.rs`, `crates/flui-core/src/prelude.rs` if present.
  - Logging requirements: no runtime logs; API migration guidance belongs in rustdoc and migration docs.

- [x] Task 19: Migrate `flui-widgets` public re-exports and downstream compile surface.
  - Deliverable: `crates/flui-widgets/src/lib.rs` re-exports the new provider API intentionally or stops re-exporting removed names with a documented migration note.
  - Files: `crates/flui-widgets/src/lib.rs`, any widget docs/examples that mention `read`, `try_read`, `Provider`, or `InheritedValue`.
  - Logging requirements: no runtime logs; downstream API decisions belong in rustdoc or migration docs.

- [x] Task 20: Update internal examples and tests that used free provider reads.
  - Deliverable: all provider tests and any examples compile against the new scoped API.
  - Files: `crates/flui-core/src/provider/*.rs`, `crates/flui-core/tests/**/*.rs`, `examples/**/*.rs`.
  - Logging requirements: tests use assertions, not log inspection.

- [x] Task 21: Preserve root, deferred, prompt, drag, tooltip, and inspector drawing behavior.
  - Deliverable: provider registry activation works wherever `AnyElement` can be drawn, including deferred draws and inspector elements, without leaking active scopes between roots, overlays, or windows.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/provider/element.rs`.
  - Logging requirements: no runtime logs; tests should assert isolation and cleanup.

- [x] Task 22: Preserve low-level Engine boundaries and avoid Framework drift.
  - Deliverable: implementation keeps K01 as the engine substrate, does not create `flui-framework`, and does not add final Framework `BuildCx`, `Widget`, `State`, reconciliation, or dirty-list APIs.
  - Files: `crates/flui-core/src/provider/*.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no runtime logs; architectural evidence belongs in the spec and review notes.

- [x] Task 23: Add the K01 migration guide.
  - Deliverable: `docs/superpowers/migrations/K01-provider-rewrite.md` explaining how to migrate from `read::<T>()` / `try_read::<T>()` to scoped read/inherit APIs, plus notes on provider identity before K02.
  - Files: `docs/superpowers/migrations/K01-provider-rewrite.md`.
  - Logging requirements: no runtime logs; include examples instead of diagnostic text.

- [x] Task 24: Record accepted limitations and follow-up hooks.
  - Deliverable: spec and migration docs clearly state what remains deferred to K02, K03, K04, and SF03, especially identity stability, final `BuildCx` ergonomics, and element-level dirty lists.
  - Files: `docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md`, `docs/superpowers/migrations/K01-provider-rewrite.md`, `.ai-factory/ROADMAP.md` if cross-track notes need tightening.
  - Logging requirements: documentation only; no runtime logs.

### Phase 4: Tests, Performance, and Verification

- [x] Task 25: Add registry unit tests.
  - Deliverable: tests for nearest-provider wins, nested same-type providers, multiple types, provider versions, notification suppression, provider removal, stale cleanup, and no active-scope leak after panic.
  - Files: `crates/flui-core/src/provider/registry.rs` or `crates/flui-core/src/provider/tests.rs`.
  - Logging requirements: assertions only; no log-dependent tests.

- [x] Task 26: Add provider identity and scope-key tests.
  - Deliverable: tests proving sibling providers do not collide, nested providers restore the outer value, source-location or explicit ids are stable enough for K01, and no-id fallback behavior matches the spec.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/tests/provider_identity.rs` if integration coverage is cleaner.
  - Logging requirements: assertions only; no runtime logs.

- [x] Task 27: Add provider element lifecycle integration tests.
  - Deliverable: tests that mount `Provider<T>` around child elements and prove reads work in request-layout, prepaint, and paint; missing provider behavior matches the spec.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/tests/provider_lifecycle.rs` if integration coverage is cleaner.
  - Logging requirements: assertions only; no runtime logs.

- [x] Task 28: Add reactivity, caching, and removal tests.
  - Deliverable: tests proving subscribing inherited reads dirty dependent views on provider changes, unchanged provider values do not dirty when notification rules suppress it, non-subscribing reads do not subscribe, cached views preserve inherited dependencies during reuse, and provider removal invalidates dependents.
  - Files: `crates/flui-core/tests/provider_reactivity.rs` or an existing test-support module.
  - Logging requirements: assertions and frame counters only; no log-dependent tests.

- [x] Task 29: Add panic-safety and multi-window isolation tests.
  - Deliverable: tests proving active provider scopes are restored after layout/prepaint/paint panic, separate windows do not share providers or dependents, and overlay/deferred draw paths do not leak active provider stacks.
  - Files: `crates/flui-core/tests/provider_isolation.rs` or focused modules under `crates/flui-core/src/provider/`.
  - Logging requirements: assertions only; panic tests should inspect state through test-only helpers, not logs.

- [x] Task 30: Add hot-path performance and allocation validation.
  - Deliverable: focused validation that provider read/inherit paths do not allocate in the common case, dependent cleanup is bounded, and no `Rc<RefCell<...>>` is introduced into provider read/inherit hot paths.
  - Files: `crates/flui-core/src/provider/registry.rs`, optional focused benches/tests under the existing test-support pattern.
  - Logging requirements: no runtime logs; record measurements in implementation notes or the spec.

- [x] Task 31: Update project docs and status artifacts.
  - Deliverable: after code lands, update K01 completion notes in `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, and `CHANGELOG.md` if present. Ensure K02 is named as the next critical-chain item.
  - Files: `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, `CHANGELOG.md`, plus spec/migration docs.
  - Logging requirements: documentation only; no runtime logs.

- [x] Task 32: Run verification.
  - Deliverable: formatting, focused provider tests, downstream widget compile surface, flui-core tests, and workspace compile checks are green or any failures are documented with exact blockers.
  - Commands: `cargo fmt --all -- --check`, `cargo test -p flui-core provider`, `cargo test -p flui-core --tests`, `cargo check -p flui-widgets --all-targets`, `cargo check --workspace --all-targets`.
  - Logging requirements: command output is summarized in the implementation notes; no code logging changes.

## Commit Plan

- **Commit 1** (after tasks 1-7): `docs(provider): specify K01 inherited registry`
- **Commit 2** (after tasks 8-13): `feat(provider): add scoped inherited registry`
- **Commit 3** (after tasks 14-17): `feat(provider): wire invalidation and cached dependencies`
- **Commit 4** (after tasks 18-24): `refactor(provider): migrate public inherited APIs`
- **Commit 5** (after tasks 25-29): `test(provider): cover inherited registry behavior`
- **Commit 6** (after task 30): `perf(provider): validate inherited hot path`
- **Commit 7** (after tasks 31-32): `docs(provider): document K01 migration`

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|---|---:|---:|---|
| Provider scope keys are unstable or collide before K02 lands | High | High | Tasks 3, 9, and 26 force an explicit identity decision and sibling/nested collision tests. |
| Clone-returning reads allocate or copy too much on the rebuild hot path | Medium | High | Task 4 must choose value semantics deliberately; Task 30 validates hot-path behavior. |
| Cached child views silently lose provider subscriptions | High | High | Tasks 5, 16, and 28 require dependency capture/replay and cached-view tests. |
| Provider removal leaves stale dependents or stale values alive | Medium | High | Tasks 15, 25, and 28 separate removal semantics from ordinary value changes. |
| Provider invalidation accidentally re-enters window/entity updates during draw | Medium | High | Use K15's defer guidance; tests must cover provider changes during active draw. |
| Downstream `flui-widgets` keeps stale provider re-exports | Medium | Medium | Task 19 and Task 32 include downstream public-surface migration and compile checks. |
| Nested provider activation leaks on panic | Medium | High | Phase-scoped guards plus Task 29 panic-safety tests. |
| Registry cleanup leaks dependents or provider entries over long sessions | Medium | Medium | Tasks 15, 17, 25, and 30 require cleanup tests, inspection hooks, and bounded-growth validation. |
| K01 drifts into full Framework `BuildCx` implementation | Medium | High | Tasks 7, 22, and 24 keep K01 as low-level substrate; SF03 owns final Framework ergonomics. |

## Out of Scope

- Do not create `flui-framework`.
- Do not implement final Framework `BuildCx`, `Widget`, `State`, reconciliation, or dirty-list APIs.
- Do not implement K02 `Key` / stable element identity beyond the minimum provider scope key required by K01.
- Do not change the frame/effect contract beyond the minimum provider invalidation bridge.
- Do not add platform code under `crates/flui-core/src/platform/**`.
- Do not add committed hot-path provider logs.
