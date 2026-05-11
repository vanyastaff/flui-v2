# K03 - Render to Build separation

**Branch:** `feature/k03-render-build-separation`
**Created:** 2026-05-11
**Phase:** 0-K Kernel Cleanup - seventh spec in the critical chain after K99, K15, K07, K05, K01, and K02.
**Type:** API-breaking engine/framework boundary cleanup in `flui-core`, with macro and docs fallout.
**Tasks:** 29 checkbox tasks.

> **Design-first spec.** K03 separates the existing mutable engine-view rendering model from the future pure Widget build model. The design spec must freeze the boundary before implementation, because the roadmap currently says "add `Widget::build`" while the architecture says final Framework APIs belong in `flui-framework`, not `flui-core`.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | K03 changes the core view/component trait vocabulary and must prove that existing `Render`, `RenderOnce`, `AnyView`, macros, identity, provider, and cached-view behavior keep working while the new pure-build path is introduced. |
| Logging | verbose during implementation, no committed hot-path logs | Temporary DEBUG diagnostics are useful while tracing adapter boundaries, but committed build/layout/prepaint/paint paths must not log per element or per frame. |
| Docs | yes (mandatory checkpoint) | K03 is API-breaking or at least API-shaping. It needs a design spec, migration guide, rustdoc updates, examples/docs cleanup, roadmap/research status updates, and changelog notes. |
| Roadmap linkage | linked | K03 is the next Phase 0-K critical-chain item and unblocks K04 plus Phase II-F SF01/SF07. |

## Roadmap Linkage

**Milestone:** K03 Render to Build separation - distinguish mutable engine `Render` views from pure Framework build semantics (Phase 0-K critical chain).

**Rationale:** `.ai-factory/ROADMAP.md` names K03 as the next critical-chain item after K02. It closes the `Render::render(&mut self)` semantic mismatch called out by the kernel audit and establishes the type-level boundary that SF01 and SF07 need.

K03 must not implement the full Framework tier. In particular, it must not add `State<W>`, reconciliation, dirty lists, `setState`, `InheritedWidget`, Theme/MediaQuery ergonomics, async widgets, or a widget catalogue. It may add the minimal pure-build substrate selected by the K03 design spec, but final `flui-framework` APIs stay assigned to Phase II-F unless the spec deliberately revises that boundary.

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `.ai-factory/ARCHITECTURE.md`, `docs/promt.md`, K02/K05 specs, and current `element`, `view`, `window`, macro, and widget-doc code.

- K99, K15, K07, K05, K01, K02, and K03 are complete. K04 follows.
- K05 made low-level element lifecycle calls use `LayoutCx`, `PrepaintCx`, and `PaintCx`, so K03 can stop treating build/render as another paint/layout lifecycle shape.
- K01 gives `Window` a per-window inherited registry. K03 may expose only the minimal build-time access the design approves; final ergonomic `BuildCx::inherit<T>()` remains SF03 unless explicitly scoped into K03.
- K02 gives `Key`, `ValueKey`, `GlobalKey`, normalized Local identity, and `ElementIdStack`. Any new pure-build adapter must use those identity rules rather than invent a second key model.
- Current `Render` is entity-backed and mutable: `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement`.
- Current `RenderOnce` is stateless but consumed: `fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement`.
- `Component<C: RenderOnce>` is a doc-hidden engine wrapper used by `derive(IntoElement)`. It is not the future Framework widget adapter.
- `crates/flui-widgets/src/widget.rs` currently maps Flutter `Widget.build()` to `RenderOnce::render()` / `Render::render()`, which is now strategically misleading.
- Tier C crates already depend on the current engine recipe model: `flui-widgets`, `flui-material`, and `flui-navigator` contain many `RenderOnce`, `IntoElement`, `use_state`, and `use_keyed_state` callsites. K03 must audit and compile-check these consumers, not only `flui-core`.

## Current State

| Area | Current shape | K03 concern |
|---|---|---|
| Root views | `Entity<V>` where `V: Render`; `AnyView` calls `view.update(cx, \|view, cx\| view.render(...))` | Mutable `Render` is the right engine-view API, but it is currently described as the whole widget model. |
| Stateless components | `RenderOnce` consumes `self`; `derive(IntoElement)` wraps `Component<Self>` | Useful compatibility path, but not equivalent to immutable `Widget::build(&self)`. |
| Element tree | `IntoElement` and `Element` remain the runtime substrate | K03 should not create a second render tree or a Flutter-style RenderObject tree. |
| Identity | `Component<C>` captures callsite and accepts explicit `Component::key(...)` | Any pure-build wrapper must preserve K02 Local/Value/Global key behavior and `#[track_caller]`. |
| Provider reads | `LayoutCx`, `PrepaintCx`, `PaintCx` expose `read_inherited` / `inherit` | K03 must not accidentally make lifecycle contexts into final Framework `BuildCx`. |
| Macros | `derive(Render)` and `derive(IntoElement)` exist in `flui-macros` | New pure-build derives or aliases need macro tests and a migration story. |
| Docs/examples | `creating_components.rs` and `flui-widgets/src/widget.rs` teach RenderOnce as Flutter `Widget.build()` | These docs must be split into "engine components today" vs "future framework widgets". |
| Tier C consumers | `flui-widgets`, `flui-material`, `flui-navigator` use `RenderOnce` and element-state helpers | K03 can break downstream crates even if `flui-core` compiles. |
| Crate layout | No `flui-framework` crate yet | K03 should not start Phase II-F wholesale. If the design chooses to add a tiny crate skeleton, it must justify the roadmap change. |

## Target Design Direction

The exact public names are frozen by `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md` before code. The preferred direction is:

1. Keep `Render` as the mutable, entity-backed engine view trait. It remains valid for Zed-style imperative UI and window roots.
2. Keep `RenderOnce` and `Component<C>` as the compatibility path for existing stateless engine recipes, at least through K03.
3. Introduce a minimal pure-build contract, if the spec confirms it belongs in K03. Candidate shape: a trait whose build method takes `&self` and a narrow build context, then produces an `IntoElement` bridge or a future-widget-compatible return type without heap allocation.
4. Keep final Framework APIs out of K03: no StateMap, reconciliation, dirty-list, setState, or widget catalogue.
5. Make naming explicit enough that future SF01 can define `flui-framework::Widget` without conflicting with engine `Render`, `RenderOnce`, or any K03 bridge.
6. Freeze object-safety and RPITIT implications before adapter work. A pure `build(&self) -> impl IntoElement` style API is likely generic-only unless the design adds an explicit erased wrapper.
7. Preserve hot-path behavior: no per-build allocation beyond what existing element construction already does, no object erasure unless the spec chooses it deliberately, and no committed per-frame logs.

## Key Design Questions To Freeze

| Question | Required decision before implementation |
|---|---|
| K03 scope | Does K03 add a minimal pure-build trait now, or only refactor/rename existing engine concepts and reserve final `Widget` for SF01? |
| Trait naming | If a new trait lands, is it called `Build`, `Widget`, `ViewBuild`, `BuildElement`, or something else that will not collide with `flui-framework::Widget`? |
| Build context | Does K03 introduce a narrow `BuildCx<'_>` in `flui-core`, or defer final `BuildCx` entirely to SF03? What can it expose from `Window`, `App`, and K01 inherited reads? |
| Return type | Does pure build return `impl IntoElement`, `impl Widget`, an erased wrapper, or a spec-specific adapter? |
| Object safety | Is the pure-build trait intentionally generic-only, or does K03 need an object-safe erased form for heterogeneous storage? What are the monomorphization/code-size tradeoffs? |
| Compatibility | Are `RenderOnce`, `derive(IntoElement)`, and `Component<C>` preserved, deprecated, aliased, or migrated? |
| Identity | How do pure-build boundaries participate in K02 Local/Value/Global key semantics and component callsite identity? |
| Macros | Is there a K03 derive macro for the pure-build trait, or is `derive(Widget)` deferred to SF01? |
| Root mounting | Can a pure-build object become a root view in K03, or does root mounting remain `Entity<V: Render>` until SF07? |
| Provider access | Are K01 inherited reads available during pure build, and if so are they subscribing or non-subscribing? |
| Crate graph | If any `flui-framework` precursor is created, how does the workspace avoid `flui-core -> flui-framework` upward dependencies and how are Tier C crates gated? |
| API breakage | Which public docstrings, prelude exports, examples, and downstream callsites change? |

## Review Gates

Before PR merge:

- `flui-arch-reviewer` for the `App` / `Entity` / `Context` / `Window` / `Element` / Framework boundary.
- `migration-risk-adversary` because K03 may rename, move, or split core traits and can silently break downstream UI code.
- `rust-api-migration-auditor` because public traits, macros, prelude exports, and crate-boundary decisions are affected.
- `wgpu-gpu-reviewer` is not required unless implementation unexpectedly touches `scene`, `platform/wgpu`, Metal, DirectX, shader, or offscreen rendering code.

## Commit Plan

- **Commit 1** (after tasks 1-9): `docs: specify k03 render build separation`
- **Commit 2** (after tasks 10-17): `feat: add render build separation substrate`
- **Commit 3** (after tasks 18-25): `test: cover k03 render build compatibility`
- **Commit 4** (after tasks 26-29): `docs: document k03 migration and status`

## Tasks

### Phase 1: Design, Inventory, and Scope Freeze

- [x] Task 1: Inventory current render/build surfaces and callsites.
  - Deliverable: table covering `Render`, `RenderOnce`, `IntoElement`, `Component<C>`, `AnyView`, `Entity<V>`, `Window::open_window`, `TestAppWindow`, `derive(Render)`, `derive(IntoElement)`, prelude exports, examples, and docs that mention widget/build semantics.
  - Files to inspect: `crates/flui-core/src/element.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/test_app.rs`, `crates/flui-core/src/prelude.rs`, `crates/flui-macros/src/*.rs`, `crates/flui-widgets/src/**/*.rs`, `crates/flui-material/src/**/*.rs`, `crates/flui-navigator/src/**/*.rs`, `examples/**/*.rs`, `crates/flui-core/examples/**/*.rs`.
  - Logging requirements: no runtime logs. Capture evidence in the design spec only.

- [x] Task 2: Audit Tier C consumer crates before selecting the API shape.
  - Deliverable: inventory of `RenderOnce`, `IntoElement`, `Component::key`, `Provider::new(_keyed)`, `Window::use_state`, and `Window::use_keyed_state` usage in `flui-widgets`, `flui-material`, and `flui-navigator`, with compatibility risks and migration notes.
  - Files to inspect: `crates/flui-widgets/src/**/*.rs`, `crates/flui-material/src/**/*.rs`, `crates/flui-navigator/src/**/*.rs`, plus their `Cargo.toml` files if feature or dependency edges are affected.
  - Logging requirements: no runtime logs. Record downstream risk evidence in the design spec or implementation notes.

- [x] Task 3: Author the K03 design spec before code.
  - Deliverable: `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md`.
  - Must include: scope boundary, trait naming, build context API, return type, object-safety/RPITIT strategy, macro strategy, identity behavior, provider access policy, root mounting policy, compatibility plan, migration plan, rejected alternatives, review gates, and known limitations.
  - Logging requirements: specify no committed per-element or per-frame logs in build/layout/prepaint/paint hot paths.

- [x] Task 4: Resolve the `flui-core` vs `flui-framework` boundary contradiction.
  - Deliverable: a design decision explaining whether K03 lands a minimal pure-build substrate in `flui-core`, a tiny `flui-framework` precursor, or documentation/API preparation only.
  - Must cover: `.ai-factory/ARCHITECTURE.md` rule that final Framework APIs live in `flui-framework`, roadmap text that currently says "both coexist in Engine", and the handoff to SF01/SF07.
  - Logging requirements: no runtime logs; rationale belongs in the spec.

- [x] Task 5: Freeze object-safety, RPITIT, and erasure strategy.
  - Deliverable: explicit design decision for whether the pure-build trait is generic-only, has an object-safe erased companion, or defers heterogeneous widget storage to SF01/SF07.
  - Must cover: `-> impl Trait` in trait methods under Rust 1.95, dyn compatibility, code-size/monomorphization risk, allocation policy, and whether any erased wrapper can stay off the rebuild hot path.
  - Files: candidate build module, `crates/flui-core/src/element.rs`, `crates/flui-core/src/lib.rs`, `crates/flui-macros/src/*.rs` if derives depend on the decision.
  - Logging requirements: no runtime logs; performance and API rationale belongs in the spec.

- [x] Task 6: Define the `Render` compatibility contract.
  - Deliverable: spec section and rustdoc plan that keeps `Render` as the mutable entity-backed view trait, including why `&mut self` remains correct for engine views but not for future immutable widgets.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs`.
  - Logging requirements: no runtime logs; use tests to prove behavior instead of diagnostics.

- [x] Task 7: Define the `RenderOnce` / `Component<C>` compatibility and migration path.
  - Deliverable: decision on whether to preserve, deprecate, alias, or wrap `RenderOnce` in K03, and how `Component<C>::key` continues to work with K02 identity.
  - Must cover: whether `#[deprecated]` is acceptable now, lint fallout for Tier C crates, derive-macro compatibility, and the explicit statement that `Component<C>` is still not the Framework `Widget` adapter.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-macros/src/derive_into_element.rs`, `crates/flui-core/examples/learn/creating_components.rs`, `crates/flui-widgets/src/**/*.rs`, `crates/flui-material/src/**/*.rs`.
  - Logging requirements: no runtime logs; compatibility risks documented in the spec.

- [x] Task 8: Define the pure-build context and provider policy.
  - Deliverable: exact API for the spec-selected build context, or explicit deferral to SF03.
  - Must cover: access to `Window`, `App`, `read_inherited`, `inherit`, current view id, element identity, re-entrancy expectations, and why the context is or is not final Framework `BuildCx`.
  - Files: candidate new module, `crates/flui-core/src/element.rs`, `crates/flui-core/src/provider/registry.rs`, `crates/flui-core/src/provider/element.rs`.
  - Logging requirements: no build-context access logs in committed code; invalid usage should use type boundaries, debug assertions, or tests.

- [x] Task 9: Freeze review gates and migration checklist.
  - Deliverable: checklist in the design spec requiring architecture, migration-risk, and Rust API review before implementation merge.
  - Logging requirements: no runtime logs; review evidence belongs in PR notes.

### Phase 2: Core API and Adapter Implementation

- [x] Task 10: Add the spec-selected pure-build module and public re-exports.
  - Deliverable: new trait/context/adapter types according to the K03 spec, with curated exports from `crates/flui-core/src/lib.rs` and `crates/flui-core/src/prelude.rs` if public.
  - Files: likely `crates/flui-core/src/element.rs` or a new focused module such as `crates/flui-core/src/build.rs`, plus `lib.rs` and `prelude.rs`.
  - Logging requirements: no runtime logs; API misuse should be compile-time where possible.

- [x] Task 11: Implement the pure-build to `IntoElement` bridge.
  - Deliverable: adapter that lets pure-build values produce the existing engine `Element` tree without adding a second tree, preserving `#[track_caller]` callsite identity and avoiding heap-erased hot-path dispatch unless the spec selects it.
  - Files: `crates/flui-core/src/element.rs`, candidate build module, `crates/flui-core/src/view.rs` if `AnyView` integration is needed.
  - Logging requirements: no per-build logs. Temporary DEBUG logs are allowed locally while tracing adapter entry/exit, but must not be committed.

- [x] Task 12: Wire K02 identity into the pure-build boundary.
  - Deliverable: pure-build boundaries can use Local, Value, and Global keys consistently with `Component<C>`, `ParentElement::child/children`, `IntoElement::into_any_element`, and `Provider::new_keyed`, including repeated sibling cases.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/element/identity.rs`, `crates/flui-core/src/provider/element.rs`.
  - Logging requirements: no identity logs in committed code; use debug assertions and focused tests.

- [x] Task 13: Preserve `Render`, `RenderOnce`, `IntoElement`, and `AnyView` behavior.
  - Deliverable: existing examples and tests keep compiling unless the K03 spec explicitly marks a break and gives a migration.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/view.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs`.
  - Logging requirements: no runtime logs; behavior is validated by tests and example builds.

- [x] Task 14: Update `flui-macros` for the selected K03 API.
  - Deliverable: macro changes for any new derive or compatibility alias selected by the spec; otherwise explicit tests proving existing derives keep working with the new boundary.
  - Must cover: compile-pass tests for current derives, compile-fail tests for invalid usage if K03 adds a derive/attribute, and whether a `trybuild`-style harness is worth adding or explicitly deferred.
  - Files: `crates/flui-macros/Cargo.toml`, `crates/flui-macros/src/flui_macros.rs`, `crates/flui-macros/src/derive_render.rs`, `crates/flui-macros/src/derive_into_element.rs`, `crates/flui-macros/tests/render_test.rs`, new macro tests if needed.
  - Logging requirements: no runtime logs; proc-macro failures should emit clear compiler errors.

- [x] Task 15: Guard workspace membership and tier dependency graph if a new crate lands.
  - Deliverable: if the K03 design creates a `flui-framework` precursor or any new crate/module boundary, update workspace membership and dependency direction deliberately; otherwise add a spec note that no Cargo graph changes are part of K03.
  - Must cover: root `Cargo.toml`, new crate `Cargo.toml` if any, `[lints] workspace = true`, `rust-version.workspace = true`, no `flui-core -> flui-framework` dependency, and Tier C dependency implications.
  - Files: `Cargo.toml`, candidate `crates/flui-framework/Cargo.toml`, `.ai-factory/ARCHITECTURE.md` only if the design changes the documented boundary.
  - Logging requirements: no runtime logs; graph validation belongs in command output and spec notes.

- [x] Task 16: Update root mounting and test harness surfaces only as scoped.
  - Deliverable: if the spec allows pure-build roots, add the narrow conversion path; otherwise explicitly keep `Window::open_window`, `WindowHandle<V>`, and `TestAppWindow<V>` constrained to `Render`.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/test_app.rs`, `crates/flui-core/src/view.rs`.
  - Logging requirements: no window-open logs; errors should remain structured `Result`/panic messages as today.

- [x] Task 17: Update prelude and public docs for the new vocabulary.
  - Deliverable: public rustdoc distinguishes `Render` views, `RenderOnce` engine recipes, low-level `Element`, and the K03 pure-build concept without claiming Phase II-F is complete.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-core/src/prelude.rs`, `crates/flui-core/src/lib.rs`, `crates/flui-core/README.md`.
  - Logging requirements: no runtime logs; docs include the no-hot-path-logging invariant where relevant.

### Phase 3: Tests and Compatibility Coverage

- [x] Task 18: Add compile/unit tests for the pure-build trait.
  - Deliverable: tests prove the selected pure-build method takes `&self`, can build an element tree, and does not require mutable widget/config state.
  - Files: new or existing tests under `crates/flui-core/src/element.rs`, `crates/flui-core/tests/`, and/or `crates/flui-macros/tests/`.
  - Logging requirements: tests may use local counters/assertions; no committed runtime logs.

- [x] Task 19: Add compatibility tests for existing `Render` root views.
  - Deliverable: `Render` still supports mutable entity-backed state, `Context<Self>`, `cx.notify()`, and current `AnyView` cached/non-cached paths.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/app/test_app.rs`, focused test module.
  - Logging requirements: no runtime logs; use state counters and draw calls for assertions.

- [x] Task 20: Add compatibility tests for `RenderOnce` and `Component<C>`.
  - Deliverable: `derive(IntoElement)` still wraps `Component<Self>`, callsite Local identity remains stable through `ParentElement::child/children` and `IntoElement::into_any_element`, `Component::key(...)` still controls explicit identity, and nested provider/state behavior from K01/K02 is not regressed.
  - Files: `crates/flui-core/src/element.rs`, `crates/flui-macros/tests/`, provider/identity focused tests.
  - Logging requirements: no runtime logs; duplicate/key behavior uses test assertions.

- [x] Task 21: Add provider/build-context boundary tests.
  - Deliverable: tests cover whatever inherited-read behavior the K03 spec selects, including a negative test or documented absence if final `BuildCx::inherit<T>()` is deferred.
  - Files: `crates/flui-core/src/provider/element.rs`, `crates/flui-core/src/provider/registry.rs`, new build-context tests if applicable.
  - Logging requirements: no provider logs in committed code; inspect registry state through test helpers.

- [x] Task 22: Add cached-view and deferred-draw regression coverage.
  - Deliverable: `AnyView::cached`, inherited dependency replay, identity stack restoration, and deferred draw snapshots still work when pure-build components appear inside the rendered tree.
  - Files: `crates/flui-core/src/view.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no cache hit/miss logs; use counters/ranges/test helpers.

- [x] Task 23: Add example build coverage.
  - Deliverable: at least one small learn/example path demonstrates the new pure-build vocabulary if it lands; existing `nav_demo`, `material_demo`, `animation_demo`, and core learn examples still compile.
  - Files: `crates/flui-core/examples/learn/creating_components.rs`, `examples/nav_demo/src/main.rs`, `examples/material_demo/src/main.rs`, `examples/animation_demo/src/main.rs`.
  - Logging requirements: example code should not add permanent diagnostic logs.

- [x] Task 24: Add Tier C compile coverage.
  - Deliverable: focused compile checks for crates most likely to break under K03 vocabulary/API changes before the full workspace test run.
  - Commands to run when implementation reaches validation: `cargo check -p flui-widgets --all-targets`, `cargo check -p flui-material --all-targets`, `cargo check -p flui-navigator --all-targets`, plus feature-gated variants if the design changes crate features.
  - Logging requirements: command output is verification evidence only; do not add source logs to make checks pass.

- [x] Task 25: Run focused and workspace validation.
  - Deliverable: `cargo fmt`, `cargo test -p flui-core`, `cargo test -p flui-macros`, Tier C compile checks from Task 24, relevant example checks, and then `cargo test --workspace` when feasible.
  - Files: workspace-wide validation, no source file target.
  - Logging requirements: command output is verification evidence only; do not add source logs to make tests pass.

### Phase 4: Documentation, Migration, and Status Updates

- [x] Task 26: Write the K03 migration guide.
  - Deliverable: `docs/superpowers/migrations/K03-render-build-separation.md`.
  - Must include: how to keep using `Render`, how to keep using or migrate from `RenderOnce`, when to use the new pure-build API, how keys work, what remains deferred to SF01/SF07, and examples of old/new code if public API changed.
  - Logging requirements: no runtime logs; migration guide should mention no committed hot-path logs.

- [x] Task 27: Update misleading widget/build docs.
  - Deliverable: docs no longer teach `RenderOnce::render()` / `Render::render()` as final Flutter `Widget.build()`. They explain current engine recipes versus future Framework widgets.
  - Files: `crates/flui-widgets/src/widget.rs`, `crates/flui-widgets/src/lib.rs`, `crates/flui-material/src/lib.rs`, `crates/flui-core/examples/learn/creating_components.rs`, `crates/flui-core/src/element.rs`, `crates/flui-core/docs/key_dispatch.md` if examples need vocabulary updates.
  - Logging requirements: no runtime logs; docs-only task.

- [x] Task 28: Update roadmap, research, AGENTS, and changelog status.
  - Deliverable: after implementation lands, mark K03 complete and K04 next in `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, and `CHANGELOG.md` if present/appropriate.
  - Files: `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, `CHANGELOG.md`.
  - Logging requirements: no runtime logs; status updates cite validation results and remaining limitations.

- [x] Task 29: Complete review gates and final API audit.
  - Deliverable: architecture, migration-risk, and Rust API review findings are addressed or explicitly accepted; public re-exports are curated; no accidental `flui-framework`/SF scope creep landed.
  - Files: changed files from tasks 10-28, plus `Cargo.toml` if crate/member changes occur.
  - Logging requirements: no runtime logs; review evidence belongs in PR notes or implementation notes.

## Done Criteria

- K03 design spec exists and resolves the `flui-core` / `flui-framework` boundary.
- Existing `Render`, `RenderOnce`, `IntoElement`, `Component<C>`, and `AnyView` behavior remains compatible or has a documented migration.
- Any new pure-build API has rustdoc, tests, and curated exports.
- Object-safety/RPITIT strategy is documented before adapter implementation, including whether heterogeneous widget storage is deferred.
- No full Framework tier behavior lands in K03: no StateMap, reconciliation, dirty-list, setState, Theme/MediaQuery ergonomics, async widgets, or widget catalogue.
- Provider, identity, cached-view, deferred-draw, and Tier C consumer regressions are covered.
- `cargo fmt`, focused tests, and workspace tests pass or any blocker is documented.
