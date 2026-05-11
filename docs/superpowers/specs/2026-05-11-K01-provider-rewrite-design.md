# K01 Provider Rewrite Design

**Date:** 2026-05-11
**Status:** Implementation design locked; review gates required before commit.
**Track:** Phase 0-K Kernel Cleanup
**Depends on:** K99 MSRV 1.95, K15 re-entrancy contract, K07 app ownership primitive, K05 lifecycle context objects
**Unblocks:** K02 Key and stable element identity, K03 Render `&mut self` semantics, K04 layout cache, SF03 Framework Provider and `BuildCx`

## Summary

K01 replaces the current thread-local provider stack with a per-`Window` inherited-value registry. The registry is an Engine-tier substrate only: it gives `flui-core` deterministic provider scoping, view-level dependency tracking, invalidation, cached-view replay, removal cleanup, and panic-safe lifecycle activation. It does not create `flui-framework`, `Widget`, `State`, reconciliation, or final `BuildCx` ergonomics.

The current implementation is intentionally simple but architecturally broken:

- Values live in a process/thread-local stack, not in the owning `Window`.
- `ProviderElement<T>` pushes in `request_layout` and pops in `paint`, so a panic or skipped phase can corrupt the stack.
- `read::<T>()` and `try_read::<T>()` are free functions with no `Window`, current view, or element identity, so they cannot subscribe dependents.
- Cached views replay accessed entities but not inherited dependencies.
- `ProviderElement<T>::id()` returns `None`, so a registry keyed by element identity would collide or be impossible unless K01 adds a deliberate scope key.

K01 fixes those problems at the low-level runtime boundary and leaves the higher-level Flutter-like API to SF03.

## Goals

- Store inherited values per `Window`, not per thread.
- Scope provider activation separately for layout, prepaint, and paint.
- Give each provider instance a stable K01 scope key.
- Preserve nearest-provider-wins behavior for nested providers of the same type.
- Support both non-subscribing reads and subscribing inherited reads.
- Dirty dependent views when provider values change or providers disappear.
- Preserve provider dependencies when `AnyView` output is reused from cache.
- Keep provider logic inside Tier A (`flui-core`) without growing Framework-tier concepts.
- Migrate `flui-core` and `flui-widgets` public re-exports together.
- Avoid committed per-read/per-provider logs on hot paths.

## Non-Goals

- Do not create `flui-framework`.
- Do not introduce final `BuildCx`, `Widget`, `State`, reconciliation, `Key`, or element-level dirty-list APIs.
- Do not solve all identity problems before K02.
- Do not add platform code or touch renderer backends.
- Do not preserve the old global free-function provider API for compatibility at the cost of correctness.

## Current Inventory

| Surface | Location | Current behavior | K01 decision |
|---|---|---|---|
| `InheritedValue` | `crates/flui-core/src/provider/mod.rs` | Blanket trait for `Any + Clone + Send + Sync + 'static`. | Add notification semantics; keep clone-returning reads for K01. |
| `Provider<T>` | `crates/flui-core/src/provider/element.rs` | `Provider::new(value, child)` renders through `Component<Self>`. | Keep constructor role; add stable scope identity. |
| Provider stack | `crates/flui-core/src/provider/stack.rs` | `thread_local!` map from `TypeId` to typed stacks. | Remove from production path; registry replaces it. |
| Stack callers | `crates/flui-core/src/provider/element.rs` | Push in layout, pop in paint. | Replace with phase-scoped `Window` registry activation. |
| Public exports | `crates/flui-core/src/lib.rs` | Re-exports `InheritedValue`, `Provider`, `read`, `try_read`. | Export new scoped API intentionally; remove stale free reads. |
| Widget re-exports | `crates/flui-widgets/src/lib.rs` | Re-exports the same provider names from `flui_core`. | Migrate at the same time as `flui-core`. |
| Tests | `provider/stack.rs` unit tests | Stack-only tests. | Replace with registry, identity, lifecycle, cache, invalidation, and isolation tests. |
| Examples | `examples/*` | No direct free provider reads found. | Workspace checks still cover public-surface drift. |

## Target Model

Each `Window` owns one `InheritedRegistry`. The registry stores provider entries by type plus provider scope identity, maintains an active-provider stack per type during each lifecycle phase, and records dependency snapshots for subscribing reads.

Conceptual model:

```rust
pub(crate) struct InheritedRegistry {
    entries: FxHashMap<ProviderScopeKey, InheritedEntry>,
    active_by_type: FxHashMap<TypeId, SmallVec<[GlobalElementId; 4]>>,
    accessed_providers: Vec<ProviderScopeKey>,
    accessed_dependencies: Vec<InheritedDependency>,
}

pub(crate) struct ProviderScopeKey {
    type_id: TypeId,
    scope_id: GlobalElementId,
}

struct InheritedEntry {
    value: Box<dyn Any + Send + Sync>,
    version: u64,
    dependents: SmallVec<[InheritedDependent; 4]>,
}

struct InheritedDependent {
    element_id: GlobalElementId,
    view_id: EntityId,
    last_seen_version: u64,
}

pub(crate) struct InheritedDependency {
    provider: ProviderScopeKey,
    provider_version: u64,
    dependent_element: GlobalElementId,
    dependent_view: EntityId,
}
```

The concrete code may split this into smaller structs, but it must preserve these invariants:

- A provider entry is identified by `TypeId + GlobalElementId`.
- Active lookup is type-indexed and stack-ordered.
- Reads never consult global/thread-local state.
- Subscribing reads record the dependent element and current view.
- Dependency snapshots can be copied into cached view state and replayed later.
- Removal cleanup is bounded by frame access tracking, not by unbounded historical growth.

## Provider Identity

K01 introduces provider scope identity directly on `Provider<T>`.

`Provider::new(value, child)` is annotated with `#[track_caller]` and derives a source-location fallback identity. `Provider::new_keyed(key, value, child)` accepts an explicit key for repeated providers, loops, and callsites that would otherwise collide. The rendered `ProviderElement<T>` returns `Some(scope_element_id)` from `id()`, so existing `Drawable` plumbing can produce a `GlobalElementId`.

The provider registry key is:

```text
(TypeId::of::<T>(), provider_global_element_id)
```

This distinguishes:

- Different provider value types at the same source location.
- Nested providers of the same type at different positions in the global element stack.
- Sibling providers of the same type when they come from different callsites or explicit keys.

Same-type sibling providers constructed from the same callsite in a loop must use `Provider::new_keyed`. K01 documents this as an accepted pre-K02 limitation. K02 will replace this fallback with proper `Key`/sibling identity semantics.

If a provider cannot obtain a `GlobalElementId`, it may still scope its child for non-subscribing reads during the current phase, but subscribing reads must not be recorded. In debug builds this should trip a deterministic assertion because a missing provider id means reactive provider semantics cannot be correct.

## Value And Read Semantics

K01 keeps clone-returning reads. This matches the current `InheritedValue: Clone` contract and avoids exposing borrowed references from erased registry storage across user code. Heavy inherited values should be wrapped by callers in `Arc<T>` or another cheap clone handle.

`InheritedValue` gains equality-based notification semantics:

```rust
pub trait InheritedValue: Any + Clone + PartialEq + Send + Sync + 'static {}
```

The `PartialEq` bound is an intentional API break. Always-notify is rejected because providers can be visited every frame; unconditional version bumps would dirty dependents repeatedly and make cached-view behavior noisy or unstable.

K01 deliberately does not add a `should_notify` method to the blanket-implemented trait. On stable Rust, a blanket impl for all compatible values prevents downstream types from overriding the default method, so such a hook would look configurable while actually being fixed. If later Theme/MediaQuery values need custom notification policy, SF03 or a follow-up Engine task should add an explicit policy wrapper rather than a misleading default trait method.

K01 exposes two low-level read families:

- Non-subscribing read: returns the nearest active `T` for the current window and type, but records no dependent.
- Subscribing inherit: returns the nearest active `T` and records `(provider, provider_version, dependent_element, current_view)`.

The exact method names should be explicit and scoped. The preferred Engine-tier shape is context/window methods rather than global free functions:

```rust
impl Window {
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T>;
}

impl LayoutCx<'_> {
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T>;
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T>;
}

impl PrepaintCx<'_> {
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T>;
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T>;
}

impl PaintCx<'_> {
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T>;
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T>;
}
```

SF03 can wrap these with `BuildCx::read<T>()`, `BuildCx::inherit<T>()`, `Theme::of(cx)`, and `MediaQuery::of(cx)` after the Framework tier exists.

## Notification And Versioning

Every provider entry has a monotonically increasing `version`. On phase activation:

1. If the provider entry does not exist, insert `value` at version `0`.
2. If the entry exists and `new_value == old_value`, keep the existing version and value semantics stable.
3. If the values differ, replace the stored value, increment the version, and schedule previous dependents for invalidation.

Invalidation happens at view granularity through the existing `WindowInvalidator` path. K01 must not introduce nested entity/window updates during draw. If a provider changes while a draw is active, dependents are queued for the next draw through the same invalidator mechanism other view dirtiness uses.

Dependents are deduplicated by `(dependent_element, dependent_view)` for each provider entry. Re-reading the same inherited value updates `last_seen_version` instead of appending a duplicate.

## Active Scope Semantics

Provider activation is phase-scoped:

- During child `request_layout`, the provider is active only for that call.
- During child `prepaint`, the provider is active only for that call.
- During child `paint`, the provider is active only for that call.

No provider guard may span multiple lifecycle phases. This is the central fix for the current push-in-layout/pop-in-paint bug.

Nested provider behavior is stack-based per `TypeId`:

```text
Provider<A outer>
  child sees outer A
  Provider<A inner>
    child sees inner A
  after inner scope, child sees outer A again
```

The active stack must be restored if child layout, prepaint, or paint panics. The implementation can use a small guard or `catch_unwind`/resume-unwind wrapper as long as it does not leave stale active scopes after a panic. Tests must prove cleanup in all three phases.

Provider activation is attached to the `ProviderElement<T>` itself, so the same behavior applies when that element is drawn as a root child, tooltip, prompt, inspector subtree, drag/deferred subtree, or ordinary child. K01 does not stretch an ancestor provider's active scope across a deferred draw that is executed after the ancestor has finished painting; deferred elements that need inherited values should contain their own provider or capture the value before deferring. This preserves the pre-K01 lifecycle shape while removing thread-local leakage.

## Dependency Capture

Subscribing reads require three pieces of runtime context:

- The active provider scope key.
- The dependent element `GlobalElementId`.
- The current view `EntityId`.

`LayoutCx`, `PrepaintCx`, and `PaintCx` already expose `global_id()` after K05. `Window` already tracks the current rendered view through `with_rendered_view`. K01 should add narrow internal helpers rather than exposing broad registry access.

When `inherit::<T>()` is called:

1. Look up the nearest active provider for `T`.
2. Clone the value to return.
3. If dependent identity and current view are available, record the dependency.
4. Add or update the provider entry's dependent record.
5. Append the dependency snapshot to the current frame's inherited-dependency access list.

If the dependent has no stable element id, the read can return the value but must not subscribe. Debug assertions should make this easy to catch in provider tests and during Framework integration.

## Cached View Replay

`AnyViewState` currently preserves accessed entities and replays them when cached prepaint/paint output is reused. Provider dependencies need the same treatment.

K01 extends cached view state with inherited dependency snapshots discovered while rendering that view. When cached output is reused:

- The cached inherited dependencies are replayed into the registry access list.
- The corresponding provider dependent records remain live.
- The provider entries referenced by reused cached output are counted as accessed for provider-removal cleanup.

This prevents a cached child view from silently losing its inherited subscriptions just because it did not rerender this frame.

The provider dependency replay path should be structurally parallel to `accessed_entities` replay, but it should not reuse entity tracking types. Provider dependencies have different invalidation and cleanup semantics.

## Removal And Cleanup Semantics

Provider cleanup distinguishes four cases:

| Case | Version change | Dependent invalidation | Cleanup |
|---|---:|---|---|
| Provider value unchanged | No | No | Mark provider as accessed this frame. |
| Provider value changed | Yes | Invalidate previous dependents. | Keep provider entry and update value/version. |
| Provider disappeared | Entry removed | Invalidate previous dependents once. | Remove provider entry and active/dependent records. |
| Dependent subtree disappeared | No | No immediate invalidation needed. | Prune stale dependent records when they are not replayed/accessed. |

Frame cleanup uses accessed-provider and accessed-dependency snapshots. Providers that were neither activated nor replayed through a cached view by the end of the frame are considered removed and their dependents are invalidated once. Dependent records that are not refreshed over cleanup windows must be pruned so registry memory does not grow unbounded.

Removal invalidation is view-level. K02/K04 can later make this more precise with stable element identities and dirty lists.

## Panic Safety

Current failure mode:

```text
request_layout: push provider
prepaint: child runs with provider still active
paint: pop provider
```

If any phase panics or is skipped, the stack can remain corrupted. K01 eliminates cross-phase state and restores active stacks immediately after each child call.

Required tests:

- Panic during child `request_layout` leaves no active provider scope.
- Panic during child `prepaint` leaves no active provider scope.
- Panic during child `paint` leaves no active provider scope.
- Nested providers restore the outer provider after an inner panic.

The panic tests should assert registry state through test-only helpers, not by inspecting logs.

## Public API Migration

`flui_core::Provider` remains the provider construction primitive.

`flui_core::read::<T>()` and `flui_core::try_read::<T>()` are removed or replaced with compile-time migration guidance. Keeping global compatibility shims would reintroduce the exact bug K01 is fixing because they have no window, view, or element identity.

`flui-widgets` must migrate in the same change:

- Either re-export the new scoped provider names intentionally.
- Or stop re-exporting provider reads and point users to `flui_core`/future `flui_framework`.

The migration guide must include examples for:

- Replacing `try_read::<Theme>()` inside lifecycle code with `cx.read_inherited::<Theme>()`.
- Replacing reactive reads with `cx.inherit::<Theme>()`.
- Adding `Provider::new_keyed` for repeated same-type providers from the same callsite.

## Engine Boundary And SF03 Handoff

K01 is an Engine substrate. It may add methods to `Window` and K05 lifecycle contexts because those are already Tier A runtime types. It must not add Framework names or behavior:

- No `BuildCx`.
- No `Widget`.
- No `State`.
- No reconciliation.
- No Framework dirty-list API.

SF03 should consume K01 by wrapping the low-level scoped methods in Framework-level ergonomics and by replacing the source-location provider fallback with proper `Key` integration once K02 lands.

## Rejected Alternatives

| Alternative | Rejection reason |
|---|---|
| Keep the thread-local stack and add cleanup guards | Still not window-owned and still cannot record subscriptions correctly. |
| Always notify dependents on every provider visit | Causes frame-to-frame invalidation churn and undermines cached views. |
| Store provider values as `Arc<dyn Any>` and return `Arc<T>` only | Forces an API style change on all inherited values and makes simple copy values less ergonomic. K01 can still support cheap clones through user-provided `Arc<T>`. |
| Borrow values directly from the registry | Hard to make lifetime-safe through erased storage and user callbacks without over-complicating the low-level API. |
| Wait for K02 before fixing Provider | Provider is the next critical-chain blocker; K01 can use explicit keys and source-location fallback, then K02 improves identity. |
| Put Provider in `flui-framework` only | `flui-framework` does not exist yet, and current Engine code already exposes a broken provider API that must be retired. |

## Hot-Path Discipline

Provider reads and provider activation are layout/prepaint/paint hot-path operations.

Committed code must not log per read, per activation, per dependent update, or per cached replay. Temporary local diagnostics are acceptable while implementing, but they must be removed before commit. Invariant failures should use assertions or test-only inspection helpers.

Implementation constraints:

- No new `Rc<RefCell<...>>` on provider read/inherit paths.
- Use capacity-preserving vectors/maps where practical.
- Use `SmallVec` for common small stacks/dependent lists.
- Deduplicate dependents to prevent repeated inherited reads from growing memory.
- Keep registry access behind narrow `Window`/context APIs.

## Test Matrix

| Area | Required coverage |
|---|---|
| Registry basics | Multiple types, nearest provider wins, nested same-type providers, version bumps, unchanged values suppress notification. |
| Identity | Source-location fallback, explicit keyed providers, nested same-type scopes, same-callsite repeated provider limitation. |
| Lifecycle | Reads work in layout, prepaint, and paint; missing provider behavior is deterministic. |
| Reactivity | Subscribing reads dirty dependent views on provider change; non-subscribing reads do not. |
| Cached views | Cached view reuse preserves inherited dependencies and provider liveness. |
| Removal | Disappearing providers invalidate previous dependents once; stale dependents are pruned. |
| Panic safety | Active scope cleanup after layout/prepaint/paint panic. |
| Isolation | Separate windows do not share providers, active scopes, or dependents. |
| Public surface | `flui-core` and `flui-widgets` compile with migrated exports. |
| Hot path | No unbounded dependent growth; common reads do not allocate beyond the chosen clone semantics. |

## Review Gates

These gates are required before committing K01 docs/code:

- `flui-arch-reviewer`: required for this spec and for runtime changes touching `Window`, `Element`, lifecycle contexts, and provider internals.
- `migration-risk-adversary`: required because K01 removes a public API and rewrites provider storage/lifecycle behavior.
- `rust-api-migration-auditor`: required because `InheritedValue` and public provider exports change.
- `wgpu-gpu-reviewer`: not required unless implementation unexpectedly touches `scene`, wgpu, Metal, DirectX, shader, pipeline, or offscreen rendering code.

Review findings must be resolved before final verification. Evidence belongs in PR notes or plan notes, not runtime logs.

## Accepted Limitations

- Same-type providers created repeatedly from the same callsite require explicit keys until K02.
- K01 invalidates at view granularity, not at future Framework element-dirty-list granularity.
- K01 exposes Engine-level context methods, not final Framework `BuildCx` ergonomics.
- Clone-returning reads remain the default; heavy values should use cheap clone handles such as `Arc<T>`.
- The registry is not a general dependency graph. It tracks inherited values only.

## Implementation Order

1. Add `provider::registry` data structures and unit tests.
2. Add provider scope identity to `Provider<T>`.
3. Attach the registry to `Window` and frame lifecycle.
4. Activate providers independently around layout, prepaint, and paint.
5. Replace free provider reads with scoped context/window methods.
6. Add notification/invalidation and removal cleanup.
7. Extend cached view state to replay inherited dependencies.
8. Migrate `flui-core`, `flui-widgets`, examples, and tests.
9. Add migration docs and status artifact updates.
10. Run focused and workspace verification.
