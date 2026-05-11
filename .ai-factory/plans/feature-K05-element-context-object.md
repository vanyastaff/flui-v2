# K05 - Element trait context object

**Branch:** `feature/K05-element-context-object`
**Created:** 2026-05-10
**Refined (aif-improve):** 2026-05-10 - second-pass gap check added `AnyElement` public-helper migration, panic-safety classification for element id stack / dispatch tree scope, `ProviderElement` thread-local stack coverage, proc-macro/example surface checks, and review gates for runtime/API migration risk.
**Refined (aif-improve round 2):** 2026-05-11 - accepted deep-pass refinements: exact Element inventory wording; spec date corrected to 2026-05-11; derived/nested context API requirement added; `Interactivity` helper migration split out; `Window` root/deferred/inspector and `TestAppContext::draw` callsites made explicit; workspace all-targets check added.
**Phase:** 0-K (Kernel Cleanup) - fourth spec in the critical chain after K99, K15, and K07.
**Type:** API-breaking engine refactor of `flui_core::Element` method signatures.
**Tasks:** 25 checkbox tasks.

> **Design-first spec.** K05 changes the low-level custom `Element` API used across `flui-core`. Phase 1 authors the design spec and locks the exact `LayoutCx`, `PrepaintCx`, and `PaintCx` surface before code migration starts.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | Every built-in and external custom `Element` implementation is affected. Compile coverage alone is not enough; K05 needs targeted lifecycle tests for layout, prepaint, paint, child traversal, and id/inspector propagation. |
| Logging | verbose during implementation, no committed hot-path logs | Use temporary/debug diagnostics while migrating, but do not add runtime logs to layout/prepaint/paint hot paths. Any committed logging must be outside per-frame hot paths and use the existing `log` crate. |
| Docs | yes (mandatory checkpoint) | K05 is API-breaking. The design spec, migration guide, rustdoc, ROADMAP, RESEARCH, AGENTS, and CHANGELOG need explicit updates. |
| Roadmap linkage | linked | K05 is the next Phase 0-K critical-chain item and unblocks K01-K04 plus internal-org K06/K08/K10/K11. |

## Roadmap Linkage

**Milestone:** K05 Element trait -> context object (Phase 0-K Kernel Cleanup, critical chain).

**Rationale:** `.ai-factory/ROADMAP.md` names K05 as the next critical-chain item after K07: replace `Element` methods that take 6-7 arguments with `&mut LayoutCx<'_>`, `&mut PrepaintCx<'_>`, and `&mut PaintCx<'_>`. This closes E5/E6 from `docs/promt.md` and gives K01-K04 cleaner borrow surfaces.

K05 must stay inside Tier A (`flui-core`). It must not create Framework-tier APIs (`Widget`, `BuildCx`, `State`, `Provider` rewrite) and must not grow `crates/flui-core/src/platform/**`.

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `docs/promt.md` section 4.3, and the post-K07 handoff.

- K07 is merged and AppCell is now a custom `UnsafeCell<App>` + `BorrowState` primitive. K05 inherits the monolithic `&mut App` model and must not assume App sharding yet.
- K05 introduces context objects for the engine Element lifecycle only. Framework `BuildCx` remains a later SF/K item.
- `docs/promt.md` section 4.3 proposes `PaintCx`, `LayoutCx`, and `PrepaintCx` as the fix for the current Element parameter explosion.
- Current `Element` shape in `crates/flui-core/src/element.rs` passes `Option<&GlobalElementId>`, `Option<&InspectorElementId>`, `Bounds<Pixels>`, mutable phase state, `&mut Window`, and `&mut App` directly into trait methods.
- `Drawable<E>` owns lifecycle state and currently computes global ids, inspector ids, bounds, and dispatch tree nodes before passing raw arguments into `Element`.
- A reconnaissance scan found 21 production `Element` impls plus 2 test-only `CustomElement` impls. The heaviest migrations are `Div`, `List`, `UniformList`, `Img`, `InteractiveText`, `AnyView`, `ProviderElement`, and the `Interactivity` helper layer in `div.rs`.

## Current State

| Area | Current shape | K05 concern |
|---|---|---|
| `Element` trait | `request_layout(id, inspector_id, window, cx)`, `prepaint(id, inspector_id, bounds, state, window, cx)`, `paint(id, inspector_id, bounds, state, prepaint, window, cx)` | 6-7 argument signatures leak lifecycle plumbing into every custom element. |
| `Drawable<E>` | Stores `global_id`, `inspector_id`, `bounds`, request-layout state, prepaint state, and dispatch node in `ElementDrawPhase` | Natural owner for constructing context objects. |
| `AnyElement` | Public child/root helpers still take `&mut Window` and `&mut App` | Needs deliberate migration path so parent elements can compose without reintroducing argument plumbing. |
| Built-in elements | Many impls ignore most arguments and manually thread `window`/`cx` to children | Migration should reduce boilerplate without broad behavior changes. |
| Interactivity helper layer | `Interactivity::{request_layout, prepaint, paint}` still accepts raw id/inspector/bounds/window/app bundles | Must migrate with `Div`/`Img`/`Svg`/`UniformList` or K05 only moves the parameter explosion one layer down. |
| Window lifecycle callsites | `Window` drives root, prompt/drag/tooltip, deferred draws, and inspector elements through `AnyElement` helpers | Root/deferred/inspector paths must be migrated explicitly, not left to compile-error discovery. |
| Provider | `ProviderElement` pushes the thread-local provider stack during request-layout and pops during paint | K05 must preserve this lifecycle exactly until K01 replaces Provider. |
| K07 AppCell | Contexts carry `&mut App`; App borrow sharding is not present | Do not invent sub-cell sharding in K05. Document partial/sharded borrow follow-up for K06/K01 if needed. |

## Target Design

K05 introduces explicit context objects in the Element lifecycle:

```rust
pub struct LayoutCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
}

pub struct PrepaintCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
    bounds: Bounds<Pixels>,
}

pub struct PaintCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
    bounds: Bounds<Pixels>,
}
```

Exact fields may be private, but accessors must be public and documented:

- `window(&mut self) -> &mut Window` and `app(&mut self) -> &mut App` for escape-hatch compatibility.
- `global_id(&self) -> Option<&GlobalElementId>` and `inspector_id(&self) -> Option<&InspectorElementId>`.
- `bounds(&self) -> Bounds<Pixels>` on `PrepaintCx` and `PaintCx`.
- Small convenience delegates only where they remove real boilerplate (`request_layout`, `layout_bounds`, `paint_quad`, `paint_path`, child traversal helpers). Do not clone the entire `Window` API into context types.
- A deliberate derived/nested-context API is required for internal cases that need to call another `Element` with adjusted lifecycle metadata, such as `InteractiveText` delegating to `StyledText` with `global_id = None` while preserving the current inspector id and bounds. The design spec must choose an explicit shape (`with_global_id`, `with_bounds`, `for_child`, or equivalent) instead of ad-hoc local reconstruction.

The new trait shape:

```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<ElementId>;
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>>;
    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(
        &mut self,
        cx: &mut PrepaintCx<'_>,
        request_layout: &mut Self::RequestLayoutState,
    ) -> Self::PrepaintState;
    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
    );
}
```

## Tasks

### Phase 1: Design and Inventory

- [x] Task 1: Produce an exact K05 surface inventory.
  - Deliverable: a short committed or plan-embedded inventory of all production and test-only `impl Element for ...` sites, all `AnyElement::{request_layout,prepaint,paint,layout_as_root,prepaint_at,prepaint_as_root}` callers, and all external crates/examples affected by signature changes.
  - Files to inspect: `crates/flui-core/src/element.rs`, `crates/flui-core/src/elements/*.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/key_dispatch.rs`, `crates/flui-navigator/src/*.rs`, `examples/*`.
  - Logging requirements: no committed runtime logs. Record command outputs in the plan/spec if useful, not in hot-path code.

- [x] Task 2: Author the K05 design spec.
  - Deliverable: `docs/superpowers/specs/2026-05-11-K05-element-context-object-design.md`.
  - Must include: target trait signatures, exact context type API, derived/nested-context API for adjusted ids/bounds, visibility/export policy, migration strategy, K07 AppCell inheritance, why K05 does not introduce `BuildCx`, no-allocation/no-hot-path-log constraints, compatibility breakage, and known limitations.
  - Logging requirements: document that context creation is allocation-free and adds no hot-path logging.

- [x] Task 3: Classify lifecycle panic-safety before changing code.
  - Deliverable: spec section or plan note describing current behavior for `window.element_id_stack` push/pop and `dispatch_tree.push_node/pop_node` if an `Element` panics during request-layout, prepaint, or paint.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no runtime logs. If K05 chooses to add guards, tests must prove restoration without logging.

- [x] Task 4: Freeze the review gates.
  - Deliverable: design spec review checklist naming `flui-arch-reviewer`, `migration-risk-adversary`, and `rust-api-migration-auditor`; `wgpu-gpu-reviewer` is only required if K05 touches scene, wgpu, Metal, DirectX, shader, or offscreen-rendering code.
  - Logging requirements: no runtime logs; review evidence goes in PR notes or the plan.

### Phase 2: Core Context Object Layer

- [x] Task 5: Add context object types and documented accessors.
  - Deliverable: `LayoutCx<'_>`, `PrepaintCx<'_>`, and `PaintCx<'_>` in `crates/flui-core/src/element.rs` or a dedicated `crates/flui-core/src/element/cx.rs` submodule, with rustdoc, no heap allocation, and explicit helpers for derived/nested contexts where ids or bounds must be adjusted.
  - Files: `crates/flui-core/src/element.rs`, optional `crates/flui-core/src/element/cx.rs`, `crates/flui-core/src/lib.rs` explicit re-exports if these types are part of public API.
  - Logging requirements: no committed runtime logs in context constructors or accessors.

- [x] Task 6: Refactor the `Element` trait to accept context objects.
  - Deliverable: new `Element` signatures for `request_layout`, `prepaint`, and `paint`, with module rustdoc updated away from raw argument plumbing.
  - Files: `crates/flui-core/src/element.rs`.
  - Logging requirements: no runtime logs; compile errors are the migration driver.

- [x] Task 7: Refactor `Drawable<E>` lifecycle to construct and pass contexts.
  - Deliverable: `Drawable::request_layout`, `Drawable::prepaint`, and `Drawable::paint` construct the right context object at the existing lifecycle boundary while preserving `ElementDrawPhase`, id stack behavior, inspector id creation, bounds computation, and dispatch node activation.
  - Files: `crates/flui-core/src/element.rs`.
  - Logging requirements: no hot-path logs; panic-safety guard behavior, if changed, must be tested rather than logged.

- [x] Task 8: Refactor `ElementObject` and `AnyElement` APIs deliberately.
  - Deliverable: object-erased calls keep an ergonomic child/root traversal surface without reintroducing 6-7 arg trait calls. Decide and implement whether public helpers take context objects directly or keep compatibility wrappers around `&mut Window` + `&mut App`.
  - Files: `crates/flui-core/src/element.rs`, downstream callers from Task 1.
  - Logging requirements: no runtime logs in traversal helpers.

- [x] Task 9: Update public exports and macro assumptions.
  - Deliverable: `LayoutCx`, `PrepaintCx`, and `PaintCx` are available from the intended public path; `flui-macros` generated `IntoElement` code still compiles and does not assume old `Element` method signatures.
  - Files: `crates/flui-core/src/lib.rs`, `crates/flui-macros/src/derive_into_element.rs`, any prelude/re-export file found in Task 1.
  - Logging requirements: no runtime logs.

### Phase 3: Built-in Element Migration

- [x] Task 10: Migrate core wrapper elements.
  - Deliverable: `Component<C>`, `AnyElement`, and `Empty` implement the new context-object trait correctly.
  - Files: `crates/flui-core/src/element.rs`.
  - Logging requirements: no runtime logs.

- [x] Task 11: Migrate `AnyView` and view rendering integration.
  - Deliverable: `AnyView` request-layout/prepaint/paint behavior remains unchanged, including cached view rendering and context/entity handoff.
  - Files: `crates/flui-core/src/view.rs`.
  - Logging requirements: no hot-path logs; keep existing diagnostics only.

- [x] Task 12: Migrate provider element without changing Provider semantics.
  - Deliverable: `ProviderElement<T>` uses contexts and preserves `stack::push` during request-layout and `stack::pop` during paint exactly until K01 replaces Provider.
  - Files: `crates/flui-core/src/provider/element.rs`.
  - Logging requirements: no runtime logs.

- [x] Task 13: Migrate simple leaf and media elements.
  - Deliverable: `&'static str`, `SharedString`, `StyledText`, `InteractiveText`, `Svg`, `Surface`, `Canvas`, `Img`, and `ElementAnimationElement<E>` compile and preserve behavior.
  - Files: `crates/flui-core/src/elements/text.rs`, `svg.rs`, `surface.rs`, `canvas.rs`, `img.rs`, `animation.rs`.
  - Logging requirements: no hot-path logs; preserve existing asset/image logging only.

- [x] Task 14: Migrate container and layout-heavy elements.
  - Deliverable: `Div`, `Stateful<E>`, `Anchored`, `Deferred`, `ImageCacheElement`, `List`, and `UniformList` use context objects while preserving child traversal, interactivity, hitbox registration, focus assignment, and layout behavior.
  - Files: `crates/flui-core/src/elements/div.rs`, `anchored.rs`, `deferred.rs`, `image_cache.rs`, `list.rs`, `uniform_list.rs`.
  - Logging requirements: no hot-path logs. Any temporary migration logs must be removed before commit.

- [x] Task 14a: Migrate the `Interactivity` helper layer.
  - Deliverable: `Interactivity::{request_layout, prepaint, paint}` accept/use lifecycle contexts or narrowly-scoped context-derived values instead of the old raw `global_id`, `inspector_id`, `bounds`, `window`, and `cx` bundle. `Div`, `Img`, `Svg`, and `UniformList` must not keep a parallel K05-era plumbing API alive through this helper.
  - Files: `crates/flui-core/src/elements/div.rs`, plus callers in `img.rs`, `svg.rs`, and `uniform_list.rs`.
  - Logging requirements: no hot-path logs. Preserve existing debug/test-only behavior such as `debug_bounds` without adding new runtime diagnostics.

- [x] Task 15: Migrate test-only custom elements and internal examples.
  - Deliverable: `CustomElement` impls in tests and examples compile against the new API and demonstrate the intended author ergonomics.
  - Files: `crates/flui-core/src/key_dispatch.rs`, `examples/*`, any doctests found by Task 1.
  - Logging requirements: no runtime logs.

- [x] Task 16: Update navigator and ecosystem callsites affected by `AnyElement`.
  - Deliverable: `flui-navigator` compiles if `AnyElement` helper signatures changed, with no Framework-tier API introduced.
  - Files: `crates/flui-navigator/src/*.rs`, `examples/nav_demo/src/main.rs`.
  - Logging requirements: no new logs unless existing navigator diagnostics require a small compatibility message outside hot paths.

- [x] Task 16a: Migrate `Window` lifecycle and test-harness callsites.
  - Deliverable: root element prepaint/paint, prompt/drag/tooltip element paths, deferred draw prepaint/paint, inspector element drawing, and `TestAppContext::draw` use the finalized `AnyElement`/`Drawable` context API correctly.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/test_context.rs`, and any adjacent visual/headless test helpers found by Task 1.
  - Logging requirements: no runtime logs. Panic-safety behavior for restored element id stacks / dispatch nodes must be tested or documented, not logged.

### Phase 4: Tests and Verification

- [x] Task 17: Add focused Element context regression tests.
  - Deliverable: tests that prove `LayoutCx`, `PrepaintCx`, and `PaintCx` expose global id, inspector id, bounds, `Window`, and `App` as intended; child traversal still works; focus assignment behavior through `AnyElement::prepaint` is preserved.
  - Files: a new or existing test module under `crates/flui-core/src/element.rs` or `crates/flui-core/src/element/tests.rs`.
  - Logging requirements: tests may use assertions, not runtime logs.

- [x] Task 18: Add or update panic-safety tests if K05 changes lifecycle cleanup.
  - Deliverable: if any guard/cleanup behavior is introduced for id stack or dispatch tree scope, tests prove restoration after an element panic. If K05 keeps current behavior, document the limitation in the spec instead.
  - Files: `crates/flui-core/src/element.rs` tests and `crates/flui-core/src/reentrancy.rs` only if integration with K15/K07 is needed.
  - Logging requirements: no runtime logs.

- [x] Task 19: Run targeted compile and test checks.
  - Deliverable: green targeted checks before broad workspace validation.
  - Commands: `cargo check --workspace --all-targets`, `cargo fmt --all -- --check`, `cargo test -p flui-core element --tests`, `cargo test -p flui-core key_dispatch --tests`, `cargo test -p flui-core provider --tests`, `cargo check -p flui-navigator --all-targets`.
  - Logging requirements: capture failures in the implementation notes; do not add code logs to hide test failures.

- [x] Task 20: Run full validation.
  - Deliverable: green workspace verification or documented blocker.
  - Commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -q -p lock-checks -- check-stubs`, `cargo run -q -p lock-checks -- check-platform-imports`.
  - Logging requirements: no runtime log additions for validation-only issues.

### Phase 5: Documentation, Review, and Handoff

- [x] Task 21: Write the K05 migration guide.
  - Deliverable: `docs/superpowers/migrations/K05-element-context-object.md` with old-vs-new custom `Element` examples and a concise guide for external element authors.
  - Files: `docs/superpowers/migrations/K05-element-context-object.md`.
  - Logging requirements: no runtime logs.

- [x] Task 22: Update project context docs.
  - Deliverable: ROADMAP flips K05 to done when implementation is complete; RESEARCH gets a K05 status entry; AGENTS and DESCRIPTION/ARCHITECTURE are updated only where K05 changes structural context; CHANGELOG records the breaking API change.
  - Files: `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, `.ai-factory/DESCRIPTION.md`, `.ai-factory/ARCHITECTURE.md`, `CHANGELOG.md`.
  - Logging requirements: no runtime logs.

- [x] Task 23: Complete review gates and PR handoff.
  - Deliverable: review findings from `flui-arch-reviewer`, `migration-risk-adversary`, and `rust-api-migration-auditor` are either fixed or explicitly triaged; PR description lists breaking changes, migration guide, tests run, and known limitations.
  - Files: plan/spec/PR notes plus any code touched by review fixes.
  - Logging requirements: no runtime logs unless a review finding identifies a real missing diagnostic outside hot paths.

  Review gate note: this Codex session did not spawn specialized subagents because the active tool policy requires an explicit user request for delegated/subagent work. A local review pass applied the same three lenses:
  - `flui-arch-reviewer`: K05 stays inside Tier A (`flui-core`), does not grow `platform/**`, does not introduce Framework `BuildCx`, and keeps the context API phase-scoped.
  - `migration-risk-adversary`: old public `Element` lifecycle signatures and `Interactivity` raw lifecycle bundles are removed; `Window` root/deferred/inspector/test draw paths and external examples compile through the new context surface.
  - `rust-api-migration-auditor`: breaking custom-Element API is documented in `CHANGELOG.md` and `docs/superpowers/migrations/K05-element-context-object.md`; `LayoutCx`, `PrepaintCx`, and `PaintCx` are reachable from `flui_core` and the prelude.

## Commit Plan

- **Commit 1** (after tasks 1-4): `docs: design K05 element context object migration`
- **Commit 2** (after tasks 5-9): `refactor(flui-core)!: add element lifecycle context objects`
- **Commit 3** (after tasks 10-16a): `refactor(flui-core)!: migrate built-in elements to lifecycle contexts`
- **Commit 4** (after tasks 17-23): `test(docs): verify and document K05 element context migration`

## Done Criteria

- `Element::request_layout`, `Element::prepaint`, and `Element::paint` no longer expose raw id/inspector/window/app/bounds argument bundles.
- `LayoutCx`, `PrepaintCx`, and `PaintCx` are documented, exported from the chosen public path, and allocation-free.
- Built-in `Element` implementations and test-only custom elements compile with the new API.
- `AnyElement` child/root helpers remain ergonomic and do not recreate the original trait parameter explosion.
- `Interactivity` and `Window` lifecycle callsites do not preserve hidden old-shape argument plumbing after the public `Element` trait changes.
- Existing layout/prepaint/paint behavior is preserved, including provider push/pop, focus assignment, interactivity, hitboxes, inspector ids, and deferred draws.
- No new committed hot-path logging or allocation is introduced.
- K05 design spec and migration guide exist.
- Targeted checks and full validation are green, or any blocker is explicitly documented.
- Review gates for runtime architecture, migration risk, and Rust public API risk are satisfied.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---:|---:|---|
| Context objects accidentally become a second `BuildCx` | Medium | High | Design spec explicitly limits K05 to Element lifecycle contexts; Framework `BuildCx` remains later. |
| Borrow aliasing through `cx.window()` and `cx.app()` blocks child traversal | Medium | High | Keep accessors as narrow reborrows; migrate parent elements early; prefer small delegates only where they avoid aliasing churn. |
| `AnyElement` public helper migration becomes the real breaking surface | Medium | Medium | Task 8 handles it before built-in element migration; migration guide includes examples. |
| `Interactivity` preserves the old plumbing under a new trait surface | Medium | High | Task 14a migrates the helper layer explicitly before validation. |
| Root/deferred/inspector element drawing misses context conversion | Medium | High | Task 16a isolates `Window` and `TestAppContext::draw` callsites. |
| Provider stack lifecycle changes before K01 | Low | High | Task 12 is isolated and tests must cover provider behavior if existing coverage is weak. |
| Panic cleanup changes introduce subtle regressions | Medium | Medium | Task 3 classifies current behavior; Task 18 tests any deliberate cleanup change. |
| Hot-path logs or convenience wrappers add overhead | Low | Medium | Settings forbid committed hot-path logs; context types stay small and allocation-free. |
| External custom Elements need manual migration | High | Medium | K05 is API-breaking by design; migration guide and CHANGELOG make the change explicit. |

## Known Limitations

- K05 does not shard `App`, `Window`, or pipeline ownership. Context objects carry the current monolithic `&mut App` and `&mut Window` model inherited from K07.
- K05 does not rewrite Provider. `ProviderElement` keeps the existing thread-local stack until K01.
- K05 does not introduce Framework `Widget`, `Key`, `State`, `BuildCx`, reconciliation, or dirty-list semantics.
- K05 may expose follow-up pressure for K06 ownership split if context methods reveal unavoidable `Window`/`App` aliasing pain.

## Next Step

K05 implementation is complete on `feature/K05-element-context-object`. Next handoff step is PR review/PR creation with the breaking-change notes from `CHANGELOG.md` and the validation list above.
