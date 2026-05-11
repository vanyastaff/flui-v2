# K02 - Element identity and Key

**Branch:** `featrure/k02-element-identity-key` (current PR branch name)
**Created:** 2026-05-11
**Refined:** 2026-05-11 - second-pass `/aif-improve` review against current `element`, `window`, `view`, provider, and macro code.
**Phase:** 0-K (Kernel Cleanup) - sixth spec in the critical chain after K99, K15, K07, K05, and K01.
**Type:** API-breaking engine identity refactor in `flui-core`.
**Tasks:** 36 checkbox tasks.

> **Design-first spec.** K02 is not just "add an enum named `Key`". It must stabilize the low-level identity substrate that K01 Provider, current `Window::with_element_state`, cached views, and the future Framework reconciliation layer will all depend on. The design spec must freeze `Key` / `ElementId` semantics before implementation starts.

## Refinement Notes

Second-pass code analysis found four plan-critical risks that are now folded into the tasks below:

- Identity stack state is not just a `Vec` of path segments. K02 must specify how sibling occurrence counters and duplicate-key diagnostics behave across layout, prepaint, paint, cached replay, and deferred draws.
- Duplicate-key detection can easily false-positive because the same element is visited in multiple lifecycle phases. The design must distinguish "same keyed element repeated across phases" from "two sibling elements with the same key in one phase".
- `derive(IntoElement)` and `Component<C: RenderOnce>` are real integration points. A plan that only changes hand-written elements can pass focused tests and still leave macro-generated components with broken Local identity.
- Generalized stateless caching is required by the roadmap, but it must be implemented as a bounded engine substrate. If the design review shows a public wrapper is too large for K02, the spec must preserve `AnyView::cached` behavior and land only the internal reusable cache primitive needed by SF02/SF05.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | K02 changes element identity, state reuse, cache reuse, provider scope stability, and future reconciliation contracts. Focused unit and integration tests are required. |
| Logging | verbose during implementation, no committed hot-path logs | Temporary DEBUG diagnostics are useful while tracing identity paths and cache reuse, but committed layout/prepaint/paint paths must not log per element. |
| Docs | yes (mandatory checkpoint) | K02 is API-breaking and unblocks K03, K04, SF01, and SF02. It requires a design spec, migration guide, rustdoc, roadmap/research status updates after implementation, and changelog notes. |
| Roadmap linkage | linked | K02 is the next Phase 0-K critical-chain item after K01 and provides the identity substrate for Framework `Widget` / `Key` / reconciliation. |

## Roadmap Linkage

**Milestone:** K02 Element identity and Key - stable element identity via Local / Value / Global keys (Phase 0-K Kernel Cleanup, critical chain).

**Rationale:** `.ai-factory/ROADMAP.md` names K02 as the next item after K01. K02 closes the "no Widget identity / Key" kernel blocker, removes K01's provider same-callsite limitation, and gives SF01/SF02 a stable identity model to build on.

K02 must stay inside Tier A (`flui-core`) as an identity substrate. It must not create `flui-framework`, final `Widget`, `State`, `BuildCx`, or the full Framework reconciliation engine. The plan should explicitly hand those APIs to SF01/SF02.

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `.ai-factory/ARCHITECTURE.md`, `docs/promt.md` section 4.5, K01 spec/plan, and current `element` / `view` / `window` code.

- K01 is complete. `Window` owns the inherited registry; `Provider<T>` has source-location fallback identity plus `Provider::new_keyed`; cached views replay inherited dependencies.
- Current identity is split awkwardly: `ElementId` lives in `crates/flui-core/src/window.rs`; `GlobalElementId` lives in `crates/flui-core/src/element.rs`; `Element::id(&self) -> Option<ElementId>` controls whether lifecycle contexts receive a global id.
- `GlobalElementId` is currently `Arc<[ElementId]>` assembled from `Window::element_id_stack`.
- `Window::with_element_state` stores state by `(GlobalElementId, TypeId)` and is the current survival mechanism across frames.
- `Window::use_state` uses `ElementId::CodeLocation(Location::caller())`; repeated callsites in loops require manual `use_keyed_state`.
- `AnyView::cached` is view-only. `AnyViewState` stores prepaint/paint ranges, accessed entities, and K01 inherited dependencies. Stateless element subtrees do not have the same reusable cache substrate.
- `Provider::new` already uses source-location fallback identity, and `Provider::new_keyed` covers repeated same-type providers until K02. K02 should converge Provider identity on the new key model rather than leaving a provider-only special case.
- `docs/promt.md` section 4.5 sketches `KeyKind::{Local, Value, Global}` but incorrectly references a non-existent `Component<Widget>` adapter. K02 must correct the kernel substrate without importing that Framework assumption into `flui-core`.

## Current State

| Area | Current shape | K02 concern |
|---|---|---|
| Element identity API | `Element::id(&self) -> Option<ElementId>` | `id` is a low-level segment API, not a Framework `Key` model. Missing ids mean no `GlobalElementId` and no durable state/cache identity. |
| Identity type location | `ElementId` in `window.rs`; `GlobalElementId` in `element.rs` | Identity is part of Element semantics but is physically tied to `Window`, making the API boundary unclear. |
| Identity stack | `Window::element_id_stack: SmallVec<[ElementId; 32]>` | No sibling occurrence accounting, no duplicate-key diagnostics, and no explicit Local/Value/Global classification. |
| Local identity | `ElementId::CodeLocation(Location<'static>)` used by `use_state` and K01 Provider | Source-location identity collides for repeated same-callsite siblings unless users pass explicit ids. |
| Value identity | String/integer/uuid/path/focus variants on `ElementId` | Useful, but not modeled as a user-facing `ValueKey` contract for future Framework reconciliation. |
| Global identity | No first-class `GlobalKey` type | Cross-tree identity is not represented separately from plain UUID/name ids. |
| State reuse | `Window::with_element_state` keyed by `(GlobalElementId, TypeId)` | Works only when callers already have a correct global id; cannot diagnose key collisions well. |
| View cache | `AnyView::cached(style)` stores `AnyViewState` under the view's global id | Caching is tied to `AnyView`, so future stateless Framework widgets cannot reuse it without duplicating logic. |
| Provider identity | `Provider::new` uses source-location fallback, `Provider::new_keyed` uses explicit `ElementId` | K02 should remove or narrow K01's "same-callsite loop needs explicit provider key" limitation. |
| Frame retention | `Frame::finish` carries accessed element states from `rendered_frame` into `next_frame` by `(GlobalElementId, TypeId)` | K02 must keep state retention and stale-state pruning correct when global ids gain key classes and local occurrence data. |
| Deferred draws | `DeferredDraw` stores `element_id_stack` and later restores it for prepaint/paint reuse | K02 must snapshot the full identity resolver state, not only raw path segments, if sibling counters or duplicate detectors become stateful. |
| Macro wrappers | `derive(IntoElement)` expands to `Component<Self>` with `#[track_caller]`; `Component<C>` namespaces children by `type_name::<C>()` | K02 must define Local identity for macro-generated components and avoid relying only on hand-written `Element` implementations. |
| Framework boundary | `flui-framework` absent | K02 should publish engine identity primitives only. SF01/SF02 own the public Widget trait and reconciliation algorithm. |

## Target Design Direction

The exact design is locked by the K02 design spec before code. The preferred implementation direction is:

1. Extract identity types out of `window.rs` into an Element-owned module, for example `crates/flui-core/src/element/identity.rs` or `crates/flui-core/src/element/key.rs`, while preserving curated `flui_core::*` re-exports.
2. Introduce a first-class `Key` model with explicit Local / Value / Global semantics. Do not create a fake configurable API that cannot actually preserve equality and hashing correctly.
3. Keep a migration bridge from existing `ElementId` constructors and variants. The spec must decide whether `ElementId` becomes a compatibility alias/wrapper, is renamed to `Key`, or remains the low-level path segment while `Key` becomes the higher-level public type.
4. Add deterministic Local-key disambiguation for repeated same-callsite siblings. The likely shape is source location plus sibling occurrence within the parent identity namespace, but the spec must define exactly where the occurrence is counted. Local occurrence identity should be treated as a fallback, not as reorder-stable identity; reorder-stable lists require Value or Global keys.
5. Add duplicate-key diagnostics for sibling identity collisions. Debug builds should fail loudly for invalid duplicate keys; release behavior must be specified and tested rather than accidental.
6. Generalize view caching into a reusable keyed element-cache substrate so `AnyView::cached` becomes one consumer rather than the only cacheable element shape.
7. Keep state and provider behavior correct while identity changes. `Window::with_element_state`, K01 provider scope keys, cached inherited dependency replay, inspector ids, focus, dispatch tree, and deferred draws must all remain coherent.
8. Treat identity resolution as frame/scope state. If K02 introduces sibling counters, duplicate-key sets, or cache namespaces, the plan must specify begin-frame cleanup, phase-local reset, and deferred-draw snapshot/restore behavior.

## Key Design Questions To Freeze

| Question | Required decision before implementation |
|---|---|
| `Key` vs `ElementId` | Decide whether to rename, wrap, alias, or split the concepts. Avoid two overlapping public identity systems with unclear precedence. |
| Local key stability | Define whether Local is source location only, source location plus sibling occurrence, or source location plus parent-local counter. State exactly how loops and reorders behave. |
| Value key representation | Define how user-provided typed values become hashable/equatable without unsound `dyn Hash` / `dyn Eq` shortcuts. |
| Global key semantics | Define whether Global keys are unique IDs only, cross-tree move handles, or just reserved substrate for SF02. |
| Duplicate handling | Debug panic vs structured error vs release fallback. Must be deterministic and documented. |
| Lifecycle phase repeats | Define why layout/prepaint/paint visits of the same element do not count as duplicate siblings, and how tests prove that. |
| Deferred identity snapshots | Decide what state must be copied into `DeferredDraw` beyond the raw element path if counters or duplicate-key sets exist. |
| Macro-generated wrappers | Decide how `derive(IntoElement)` and `Component<C: RenderOnce>` participate in Local identity and source-location tracking. |
| Public API breakage | Decide whether `Element::id` becomes `Element::key`, keeps its name with new semantics, or gets a transitional default. |
| Cache invalidation | Decide cache keys, invalidation inputs, and relation to K01 provider dependency replay. |
| Cache scope | Decide whether K02 lands a public keyed stateless cache wrapper or only an internal substrate plus `AnyView::cached` refactor. |
| Hot-path allocation | Define where allocation is acceptable. Per-element layout/prepaint/paint identity resolution must avoid steady-state allocations. |

## Tasks

### Phase 1: Design, Inventory, and Review Gates

- [x] Task 1: Inventory current identity surfaces and callsites.
  - Deliverable: inventory table covering `Element::id`, `ElementId`, `GlobalElementId`, `Window::with_id`, `Window::with_global_id`, `Window::with_element_namespace`, `Window::use_state`, `Window::use_keyed_state`, `Window::with_element_state`, `AnyView::cached`, K01 `Provider::new_keyed`, all `fn id(&self) -> Option<ElementId>` implementations, and public re-exports.
  - Files to inspect: `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/lib.rs`, `crates/flui-core/src/prelude.rs`, `crates/flui-core/src/elements/**/*.rs`, `crates/flui-widgets/src/lib.rs`, `examples/**/*.rs`.
  - Logging requirements: no runtime logs. Record command evidence in the design spec/plan notes only.

- [x] Task 2: Author the K02 design spec before code.
  - Deliverable: `docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md`.
  - Must include: identity model, `Key`/`ElementId` decision, Local/Value/Global semantics, sibling occurrence algorithm, duplicate handling, lifecycle-phase repeat semantics, deferred-draw identity snapshots, global id path construction, state/cache/provider migration, public API breakage, migration strategy, SF01/SF02 handoff, rejected alternatives, and known limitations.
  - Logging requirements: specify that identity resolution and cache checks are hot-path operations and must not log per element in committed code.

- [x] Task 3: Decide the `Key` / `ElementId` public model.
  - Deliverable: spec section and code plan for whether `ElementId` is renamed, wrapped, aliased, or kept as a low-level compatibility segment beneath a new `Key`.
  - Must cover: existing constructors from `usize`, `String`, `SharedString`, `Uuid`, `FocusHandle`, `Path`, `Location`, tuple forms, `NamedChild`, display/debug output, serde considerations if any, and how downstream code migrates.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/element.rs`, candidate new identity module.
  - Logging requirements: no runtime logs; API rationale belongs in spec and rustdoc.

- [x] Task 4: Define Local key generation and sibling collision behavior.
  - Deliverable: design for source-location based local identity, occurrence counters, parent namespace boundaries, loops, reordered children, duplicate keys, debug/release behavior, and test examples.
  - Must address: `#[track_caller]` storage in release builds when Local keys need it, current debug-only `source_location` fields, interaction with inspector source locations, and the explicit limitation that Local occurrence fallback is not reorder-stable.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/elements/div.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no per-element logs. Use debug assertions or structured test-only diagnostics for duplicate-key detection.

- [x] Task 5: Define Value and Global key semantics.
  - Deliverable: design for user-provided value keys and global keys that avoids unsound erased equality/hash behavior.
  - Must cover: typed `ValueKey<T>` vs existing `ElementId` variants, `Arc<str>` / `SharedString` common paths, `Uuid`, `FocusHandle`, global-key uniqueness, and whether cross-tree moves are implemented now or explicitly deferred to SF02.
  - Files: candidate identity module, `crates/flui-core/src/window.rs`, `crates/flui-core/src/elements/div.rs`.
  - Logging requirements: no runtime logs; collision and invalid global-key cases should be documented and asserted/tested.

- [x] Task 6: Define generalized keyed element caching.
  - Deliverable: spec section explaining how `AnyView::cached` is refactored into a reusable cache substrate for keyed stateless element subtrees.
  - Must cover: cache key inputs, prepaint/paint range reuse, accessed entity replay, K01 inherited provider access/dependency replay, bounds/content-mask/text-style invalidation, refresh/inspector bypass, and how future Framework stateless widgets consume it. If public keyed stateless caching is too large for K02, the spec must choose a smaller internal substrate and state what remains for SF02/SF05.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/element.rs`.
  - Logging requirements: no cache hit/miss logs in committed hot paths; use tests and optional local diagnostics during development only.

- [x] Task 7: Freeze review gates.
  - Deliverable: checklist in the spec/plan requiring `flui-arch-reviewer`, `migration-risk-adversary`, and `rust-api-migration-auditor` before PR. `wgpu-gpu-reviewer` is not required unless implementation unexpectedly touches scene/wgpu/Metal/DirectX/offscreen rendering.
  - Logging requirements: no runtime logs; review evidence belongs in PR notes or implementation notes.

### Phase 2: Identity Types and Stack Plumbing

- [x] Task 8: Extract identity types into an Element-owned module.
  - Deliverable: identity types move out of `window.rs` into a focused module while preserving public re-exports and minimizing unrelated `Window` churn.
  - Files: new `crates/flui-core/src/element/identity.rs` or `crates/flui-core/src/element/key.rs` (final path per spec), `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/lib.rs`.
  - Logging requirements: no runtime logs; this is structural code movement plus rustdoc.

- [x] Task 9: Add the first-class `Key` API and compatibility conversions.
  - Deliverable: `Key` plus Local/Value/Global support according to the spec, conversion helpers for existing `ElementId` use, display/debug/hash/equality behavior, and rustdoc examples.
  - Files: identity module, `crates/flui-core/src/lib.rs`, `crates/flui-core/src/prelude.rs` if identity types are part of prelude.
  - Logging requirements: no runtime logs; invalid conversions should be compile-time unavailable where possible or deterministic panics/assertions where unavoidable.

- [x] Task 10: Migrate `Element::id` semantics to the spec-selected key API.
  - Deliverable: `Element` exposes the chosen identity method (`id`, `key`, or transitional pair) and all built-in element implementations compile against it.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/elements/**/*.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/key_dispatch.rs`, `crates/flui-core/src/view.rs`.
  - Logging requirements: no runtime logs; migration errors should be caught by compiler/tests.

- [x] Task 11: Add an identity stack manager to `Window`.
  - Deliverable: replace ad hoc `SmallVec<[ElementId; 32]>` manipulation with a narrow identity-stack helper that can push explicit keys, derive local keys, track sibling occurrences, distinguish sibling duplicates from lifecycle-phase repeats, and restore state panic-safely.
  - Files: `crates/flui-core/src/window.rs`, identity module, `crates/flui-core/src/element.rs`.
  - Logging requirements: no push/pop logs. Use `debug_assert!` for stack balance and duplicate-key invariants.

- [x] Task 12: Update `GlobalElementId` construction and representation.
  - Deliverable: `GlobalElementId` is built from the new identity segments, preserves cheap clone/equality/hash behavior, and remains usable as a key in `FxHashMap`.
  - Files: `crates/flui-core/src/element.rs` or identity module, `crates/flui-core/src/window.rs`, `crates/flui-core/src/provider/registry.rs`.
  - Logging requirements: no runtime logs; display formatting should be deterministic for diagnostics.

- [x] Task 13: Preserve lifecycle context identity access.
  - Deliverable: `LayoutCx::global_id`, `PrepaintCx::global_id`, and `PaintCx::global_id` keep working with the new identity model, including root/deferred/inspector paths.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no lifecycle logs; identity availability is asserted through tests.

- [x] Task 14: Update `Window::with_id`, `with_global_id`, `with_element_namespace`, `use_state`, and `use_keyed_state`.
  - Deliverable: public state APIs accept the new key model, source-location state identity uses the K02 Local-key rules, explicit keyed state remains available for loops/reorder-sensitive cases, and frame cleanup still preserves only accessed `(GlobalElementId, TypeId)` entries.
  - Files: `crates/flui-core/src/window.rs`, identity module, tests under `window.rs` or focused identity tests.
  - Logging requirements: no runtime logs; duplicate or unstable key behavior should use debug assertions and tests.

### Phase 3: Built-in Element Migration and Provider Convergence

- [x] Task 15: Migrate built-in elements that carry explicit identity.
  - Deliverable: `Div`, text elements, `UniformList`, `Surface`, `Img`, image cache, animation elements, key-dispatch test elements, and provider elements use the new identity/key API.
  - Files: `crates/flui-core/src/elements/div.rs`, `crates/flui-core/src/elements/text.rs`, `crates/flui-core/src/elements/uniform_list.rs`, `crates/flui-core/src/elements/surface.rs`, `crates/flui-core/src/elements/img.rs`, `crates/flui-core/src/elements/image_cache.rs`, `crates/flui-core/src/elements/animation.rs`, `crates/flui-core/src/key_dispatch.rs`, `crates/flui-core/src/provider/element.rs`.
  - Logging requirements: no runtime logs; use compile errors and identity tests to validate migration.

- [x] Task 16: Make `Component<C: RenderOnce>` identity behavior explicit.
  - Deliverable: decide and implement whether components participate in Local identity, remain namespace-only wrappers, or get explicit keys. Audit `derive(IntoElement)` expansion, `#[track_caller]` propagation, and component source-location behavior. Document why `Component<C: RenderOnce>` is not the Framework `Widget` adapter.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-macros/src/derive_into_element.rs`.
  - Logging requirements: no runtime logs; behavior is covered by tests and rustdoc.

- [x] Task 17: Converge K01 Provider identity on K02 keys.
  - Deliverable: `Provider::new` / `Provider::new_keyed` use the K02 key model; same-type providers from repeated callsites no longer rely on provider-only identity semantics beyond what the spec deliberately keeps.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/provider/registry.rs`, provider tests.
  - Logging requirements: no provider identity logs in committed code; assert scope keys through test-only helpers.

- [x] Task 18: Preserve inspector and debug source-location behavior.
  - Deliverable: inspector element ids and source navigation still work after identity extraction, and Local-key source locations do not accidentally disappear in release builds if they are needed for identity.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/elements/div.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no runtime logs; inspector behavior should be validated by focused tests or compile-gated assertions.

- [x] Task 19: Preserve focus, hit-test, dispatch, and deferred draw behavior.
  - Deliverable: focus handles, dispatch tree nodes, hitbox ownership, deferred draws, tooltips, prompts, drag overlays, and root elements keep coherent global ids and do not leak identity stack entries. If the identity resolver contains sibling counters, duplicate sets, or cache namespaces, `DeferredDraw` must snapshot/restore the required state, not only the raw element path.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/elements/div.rs`.
  - Logging requirements: no dispatch/paint logs; tests should assert state/identity behavior directly.

### Phase 4: Generalized Cached Element Substrate

- [x] Task 20: Scope reusable cache state from `AnyViewState`.
  - Deliverable: K02 design keeps `AnyView::cached` as the cache consumer, preserves prepaint/paint ranges, accessed entities, inherited provider accesses, inherited dependencies, and cache key inputs, and defers broader public cache extraction to SF02/SF05.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, candidate new cache helper module if the spec chooses one.
  - Logging requirements: no cache logs in committed code; use tests and counters.

- [x] Task 21: Implement a keyed stateless element cache wrapper or equivalent substrate.
  - Deliverable: per the K02 spec, no public keyed stateless cache wrapper lands in Tier A. `AnyView::cached` behavior remains unchanged and consumes normalized identity; the public wrapper is deferred to SF02/SF05 with reconciliation.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no per-cache-access logs; use debug assertions for missing keys.

- [x] Task 22: Preserve `AnyView::cached` on the scoped cache substrate.
  - Deliverable: current `AnyView::cached` behavior remains compatible, nested cache rerenders reset layout/prepaint identity passes, and existing entity access replay plus K01 provider dependency replay remain covered.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no committed cache hit/miss logs.

- [x] Task 23: Add cache invalidation and collision tests.
  - Deliverable: focused K02 coverage includes duplicate-key debug behavior and lifecycle-phase repeat behavior; existing K01 cached-provider tests cover provider removal/value-change invalidation and inherited dependency replay. Public stateless element cache reuse remains deferred with the wrapper.
  - Files: focused tests in `crates/flui-core/src/view.rs`, `crates/flui-core/src/element.rs`, or integration tests under `crates/flui-core/tests/` if existing conventions support them.
  - Logging requirements: assertions and counters only; no log-dependent tests.

### Phase 5: Tests, Migration Docs, and Verification

- [x] Task 24: Add identity unit tests.
  - Deliverable: tests for `Key` equality/hash/display, Local key construction, Value key conversions, Global key uniqueness, `GlobalElementId` path formatting, and compatibility conversions from existing explicit ids.
  - Files: identity module tests, `crates/flui-core/src/window.rs` tests if compatibility constructors remain there.
  - Logging requirements: assertions only.

- [x] Task 25: Add lifecycle identity integration tests.
  - Deliverable: tests proving keyed elements receive stable `GlobalElementId` in layout/prepaint/paint, repeated lifecycle visits of the same keyed element do not trip sibling duplicate detection, unkeyed elements behave according to the spec, nested namespaces compose correctly, and identity stack is restored after panic.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`, or focused integration tests.
  - Logging requirements: assertions only; panic tests inspect state through test-only helpers.

- [x] Task 26: Add state retention and reorder tests.
  - Deliverable: tests proving `with_element_state` / `use_state` / `use_keyed_state` retain or reset state according to Local/Value/Global key semantics, including insertion, deletion, reorder, and duplicate-key cases. Tests must show Local occurrence fallback is deterministic but not reorder-stable, while explicit Value/Global keys preserve state across supported reorders.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/tests/identity_state.rs` if integration coverage is cleaner.
  - Logging requirements: assertions and frame counters only.

- [x] Task 27: Add provider identity regression tests.
  - Deliverable: tests proving K01 provider dependencies still work after K02 identity changes, same-callsite providers follow the new key rules, provider removal invalidation still fires, and cached views replay inherited dependencies.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/provider/registry.rs`, provider integration tests.
  - Logging requirements: assertions only; no provider logs.

- [x] Task 28: Add built-in element compatibility tests.
  - Deliverable: focused tests or compile checks for elements with explicit ids: `Div`, `UniformList`, text/image/surface/animation wrappers, focus and dispatch test elements.
  - Files: existing element test modules plus `crates/flui-core/src/key_dispatch.rs` tests.
  - Logging requirements: assertions only.

- [x] Task 29: Add hot-path performance and allocation validation.
  - Deliverable: validation that steady-state identity resolution, duplicate-key tracking, cache lookup, provider identity lookup, and state lookup do not allocate per element on layout/prepaint/paint beyond known first-insert/capacity growth cases.
  - Files: identity module, `crates/flui-core/src/window.rs`, `crates/flui-core/src/view.rs`, optional focused bench/test-support helpers.
  - Logging requirements: no committed runtime logs; record measurements in implementation notes.

- [x] Task 30: Add the K02 migration guide.
  - Deliverable: `docs/superpowers/migrations/K02-element-identity-key.md`.
  - Must include: migrating explicit ids to keys, handling repeated Local-key callsites, provider `new_keyed` guidance after K02, state retention/reorder examples, cache behavior notes, and what remains deferred to SF01/SF02.
  - Logging requirements: documentation only.

- [x] Task 31: Update rustdoc and public examples.
  - Deliverable: rustdoc for `Element`, `Key`/`ElementId`, `GlobalElementId`, `Window::use_state`, `Window::use_keyed_state`, cache APIs, and Provider identity notes. Examples compile or are marked `ignore` only when they intentionally require future Framework APIs.
  - Files: `crates/flui-core/src/element.rs`, identity module, `crates/flui-core/src/window.rs`, `crates/flui-core/src/provider/element.rs`, examples if affected.
  - Logging requirements: documentation only.

- [x] Task 32: Update downstream public surfaces.
  - Deliverable: `flui-widgets` and examples compile against the K02 identity API; no stale re-export points to removed identity names.
  - Files: `crates/flui-widgets/src/lib.rs`, `examples/**/*.rs`, possibly `README.md` snippets if they mention ids.
  - Logging requirements: no runtime logs.

- [x] Task 33: Update project status artifacts after implementation.
  - Deliverable: mark K02 complete and K03 next in `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, and `CHANGELOG.md` if implementation lands.
  - Files: `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, `CHANGELOG.md`.
  - Logging requirements: documentation only.

- [x] Task 34: Run specialized review gates.
  - Deliverable: run `flui-arch-reviewer`, `migration-risk-adversary`, and `rust-api-migration-auditor` on the spec and implementation diff; triage all BLOCKER/MAJOR findings before PR.
  - Files: review notes may live in PR comments or implementation notes; do not add noisy generated reports unless needed.
  - Logging requirements: no runtime logs; review evidence is external/documentary.

- [x] Task 35: Run verification.
  - Deliverable: formatting, focused identity/provider/cache tests, flui-core tests, downstream widget compile surface, and workspace check are green or exact blockers are documented.
  - Commands: `cargo fmt --all -- --check`, `cargo test -p flui-core identity`, `cargo test -p flui-core provider`, `cargo test -p flui-core --tests`, `cargo check -p flui-widgets --all-targets`, `cargo check --workspace --all-targets`.
  - Logging requirements: command output is summarized in implementation notes; no code logging changes.

- [x] Task 36: Final API and hot-path audit.
  - Deliverable: audit public exports, docs, and hot-path code before PR. Confirm no new `Rc<RefCell<...>>` on dispatch/tick/paint identity paths, no committed per-element logs, no broad platform changes, no scattered direct `element_id_stack.push/pop` after the stack-manager migration, no Framework-tier types in `flui-core`, and no unclassified `unimplemented!()` / `unreachable!()` changes.
  - Commands: `rg -n "Rc<RefCell|dbg!|println!|eprintln!|tracing::|log::|unimplemented!|todo!|unreachable!|element_id_stack\\.(push|pop)" crates/flui-core/src`, plus targeted diff review.
  - Logging requirements: audit output is summarized in PR notes; no runtime logs.

## Commit Plan

- **Commit 1** (after tasks 1-7): `docs(identity): specify K02 key model`
- **Commit 2** (after tasks 8-14): `feat(identity): add key substrate`
- **Commit 3** (after tasks 15-19): `refactor(identity): migrate engine elements`
- **Commit 4** (after tasks 20-23): `feat(identity): generalize keyed cache reuse`
- **Commit 5** (after tasks 24-29): `test(identity): cover key and cache semantics`
- **Commit 6** (after tasks 30-33): `docs(identity): document K02 migration`
- **Commit 7** (after tasks 34-36): `chore(identity): verify K02 gates`

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|---|---:|---:|---|
| `Key` and `ElementId` become two overlapping identity systems | High | High | Tasks 2-3 force a single public model decision before code. |
| Local source-location keys collide in loops or repeated same-callsite siblings | High | High | Task 4 defines occurrence/collision semantics; Tasks 24-26 test loops, reorder, and duplicates. |
| Typed value keys are implemented with unsound erased equality/hash | Medium | High | Task 5 must choose a representation that preserves `Eq`/`Hash` correctness without ad hoc trait-object hacks. |
| Identity changes break `with_element_state` and state retention | High | High | Tasks 14, 25, and 26 focus on state APIs and lifecycle identity. |
| K01 Provider scope keys regress or lose cached dependency replay | Medium | High | Tasks 17 and 27 explicitly cover provider convergence and K01 regression tests. |
| Generalized cache silently drops accessed entities or provider dependencies | Medium | High | Tasks 20-23 require shared cache state and replay tests. |
| Duplicate-key diagnostics false-positive across lifecycle phases | Medium | High | Tasks 2, 11, 23, and 25 require phase-repeat semantics and tests. |
| Deferred draws restore only raw path segments and lose resolver state | Medium | High | Tasks 2, 19, and 23 require snapshot/restore design and replay tests. |
| Generalized stateless cache grows beyond K02 scope | Medium | Medium | Tasks 6 and 21 require a bounded design decision and allow internal substrate without a broad public wrapper. |
| K02 drifts into full Framework reconciliation | Medium | High | Scope gates in Tasks 2, 7, 16, 33, and 36 keep Widget/State/BuildCx/SF02 out of `flui-core`. |
| Inspector/debug source locations disappear in release-sensitive Local keys | Medium | Medium | Task 4 and Task 18 distinguish identity source data from inspector-only debug metadata. |
| Hot-path identity resolution allocates per element | Medium | High | Task 29 validates steady-state allocation behavior; Task 36 audits hot-path changes. |
| Public API breakage is under-documented | Medium | High | Tasks 30-33 require migration docs, rustdoc, downstream compile checks, and status updates. |

## Out of Scope

- Do not create `flui-framework`.
- Do not implement final Framework `Widget`, `State`, `BuildCx`, `InheritedWidget`, or `setState` APIs.
- Do not implement the full SF02 reconciliation algorithm. K02 may define the identity substrate SF02 will consume, but SF02 owns reconciliation behavior.
- Do not change the frame/effect contract beyond what identity/cache correctness requires.
- Do not add platform code under `crates/flui-core/src/platform/**`.
- Do not rewrite `Window` broadly; K06 owns Window decomposition.
- Do not introduce committed per-element, per-cache, or per-provider hot-path logs.
- Do not preserve gpui-ce API compatibility for its own sake; flui-v2 is a hard fork, but breakage must be documented.

## Definition Of Done

- K02 design spec exists and passes specialized review.
- Public identity model is unambiguous: users can tell when to use Local, Value, and Global keys.
- `GlobalElementId` construction is stable, cheap to clone/hash, and tested across layout/prepaint/paint.
- `Window::with_element_state`, `use_state`, and `use_keyed_state` follow K02 semantics and have reorder/collision tests.
- Provider identity and cached inherited dependency replay still pass K01 regression tests.
- Deferred/root/overlay identity stack snapshots are correct if K02 adds resolver state beyond raw path segments.
- Duplicate-key diagnostics distinguish sibling collisions from normal layout/prepaint/paint repeats.
- `AnyView::cached` is backed by reusable cache machinery or the spec explicitly justifies a narrower substrate.
- Migration guide explains old explicit ids, new keys, repeated callsites, provider keys, and cache behavior.
- `cargo fmt --all -- --check`, focused tests, `cargo test -p flui-core --tests`, `cargo check -p flui-widgets --all-targets`, and `cargo check --workspace --all-targets` are green or blockers are documented.
