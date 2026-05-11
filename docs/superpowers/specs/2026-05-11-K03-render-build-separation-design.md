# K03 - Render to Build Separation Design

**Date:** 2026-05-11
**Phase:** 0-K Kernel Cleanup
**Status:** Draft implementation contract
**Plan:** `.ai-factory/plans/feature-k03-render-build-separation.md`

## Summary

K03 separates the current engine-level render model from the future Framework-tier widget build
model without starting Phase II-F. `Render` remains the mutable, entity-backed view trait used for
window roots and view caching. `RenderOnce` and `Component<C>` remain the compatibility path for
existing stateless engine recipes and derive output. K03 adds one deliberately narrow engine
substrate: `ElementBuilder`, a generic-only pure element recipe trait whose `build` method takes
`&self` and returns the existing `Element` tree through `IntoElement`.

This is not `flui-framework::Widget`. The Framework tier still owns `Widget`, `State`,
`StatefulWidget`, `BuildCx`, reconciliation, dirty lists, `setState`, inherited-widget ergonomics,
Theme/MediaQuery APIs, and widget catalogues. K03 only makes the type-level distinction explicit so
SF01/SF07 can build the final Framework API on stable Engine concepts.

## Scope Boundary

In scope:

- Keep `Render` as the mutable engine view trait.
- Keep `RenderOnce` and `Component<C>` source-compatible.
- Add a narrow pure-build engine recipe trait named `ElementBuilder`.
- Add an adapter named `BuildElement<B>` plus `build_element(builder)` helper to bridge
  `ElementBuilder` values into the existing `Element` tree.
- Add `ElementBuildCx<'_>` as an engine-only context for build-time access to `Window`, `App`,
  current identity, and K01 inherited values.
- Preserve K02 Local/Value/Global key semantics through the new adapter.
- Add focused tests for the new adapter and compatibility tests for the existing paths.
- Update docs so current engine recipes are no longer described as final Flutter `Widget.build`.

Out of scope:

- Creating `flui-framework`.
- Defining public `Widget`, `StatefulWidget`, `WidgetState`, or final `BuildCx`.
- Implementing reconciliation, state maps, dirty lists, `setState`, `did_update_widget`, or
  `dispose`.
- Adding Theme, MediaQuery, Localizations, async widget, or widget catalogue APIs.
- Changing root window mounting to accept non-`Render` roots.
- Adding object-safe heterogeneous widget storage.

## Current Inventory

| Surface | Current role | K03 action |
|---|---|---|
| `Element` (`crates/flui-core/src/element.rs`) | Low-level layout/prepaint/paint runtime substrate | Preserve unchanged; new build adapter compiles into this tree. |
| `IntoElement` (`element.rs`) | Conversion contract into a concrete `Element` | Preserve; `BuildElement<B>` implements it explicitly. |
| `ParentElement::child/children` (`element.rs`) | Child insertion helpers that preserve `#[track_caller]` through `IntoElement` | Preserve; K03 adapter must work as an `IntoElement` child. |
| `Render` (`element.rs`) | Mutable entity-backed view trait: `render(&mut self, &mut Window, &mut Context<Self>)` | Preserve and document as engine view API, not immutable widget build. |
| `RenderOnce` (`element.rs`) | Consuming stateless engine recipe: `render(self, &mut Window, &mut App)` | Preserve without deprecation in K03. |
| `Component<C: RenderOnce>` (`element.rs`) | Macro-generated engine wrapper with callsite or explicit key identity | Preserve; keep doc-hidden and not the Framework widget adapter. |
| `AnyView` (`crates/flui-core/src/view.rs`) | Erased `Entity<V: Render>` element with cache/deferred provider replay | Preserve; pure-build values can appear inside rendered element trees, not as roots. |
| `Entity<V>` -> `AnyView` (`view.rs`) | Allows `Entity<V: Render>` to become an element | Preserve. |
| `App::open_window` (`crates/flui-core/src/app.rs`) | Opens roots constrained to `V: Render` | Preserve the bound in K03. |
| `WindowHandle<V>` (`window.rs`/`app.rs`) | Typed handle to a root render view | Preserve `V: Render` root semantics. |
| `TestAppWindow<V>` (`crates/flui-core/src/app/test_app.rs`) | Test harness root constrained to `V: Render` | Preserve the bound in K03. |
| K01 `Provider<T>` (`crates/flui-core/src/provider/element.rs`) | Engine inherited-value element; also implements `RenderOnce` | Preserve; `ElementBuildCx` delegates to the same registry policy. |
| K02 `Key` / `ElementIdStack` (`element/identity.rs`, `window.rs`) | Normalized Local/Value/Global identity substrate | Reuse for `BuildElement<B>::key`. |
| `derive(IntoElement)` (`crates/flui-macros/src/derive_into_element.rs`) | Emits `IntoElement` with `type Element = flui_core::Component<Self>` | Preserve; no K03 derive macro is added. |
| `derive(Render)` (`crates/flui-macros/src/derive_render.rs`) | Emits an empty `Render` impl | Preserve; add compatibility coverage where practical. |
| `flui_core` re-exports (`lib.rs`, `prelude.rs`) | Curated public surface including `Render`, `RenderOnce`, `IntoElement`, keys, lifecycle contexts | Add only curated K03 exports; no blanket exports. |
| `crates/flui-widgets/src/widget.rs` | Maps Flutter `Widget.build()` to `RenderOnce::render` / `Render::render` | Update; this is strategically misleading after K03. |
| `crates/flui-widgets/src/lib.rs` | Mentions visual styling on top using the `build()` pattern | Update terminology. |
| `crates/flui-core/examples/learn/creating_components.rs` | Teaches function, `RenderOnce`, and `Render` component styles | Update to "engine recipes today" vs "future Framework widgets". |
| `flui-material` components | Many `RenderOnce` + `derive(IntoElement)` widgets | Preserve compatibility; docs can mention engine recipes. |
| `flui-navigator` | Uses `Render` route components and `window.use_keyed_state` caches | Preserve compatibility; no root/build migration in K03. |
| Examples (`examples/*`, `crates/flui-core/examples/*`) | Root views use `Render`; helpers return `impl IntoElement` | Preserve and compile-check. |

## Tier C Consumer Audit

Tier C is already coupled to the current engine recipe vocabulary:

| Crate | Current usage | K03 compatibility decision |
|---|---|---|
| `flui-widgets` | `ButtonBase`, `DialogBase`, `ScrollBase`, `TextFieldBase`, `VirtualListBase`, `Padding`, `SizedBox`, `Expanded`, `Flexible`, and `Stack` implement `RenderOnce` and usually derive `IntoElement`. | Keep source-compatible; no deprecation lint in K03. Update docs that call this final `Widget.build`. |
| `flui-material` | Material components such as `MaterialApp`, `Scaffold`, `AppBar`, buttons, `Card`, `Divider`, `AlertDialog`, and `TextField` implement `RenderOnce`. | Keep source-compatible; compile-check crate after adapter work. |
| `flui-navigator` | Route components use `Render`; outlets and transitions return `impl IntoElement`; route caching uses `window.use_state` / `window.use_keyed_state`. | Keep root and route component APIs on `Render`; no pure-build route root support in K03. |
| Examples | Demo roots use `Render`; many helper functions return `impl IntoElement`; core learn docs teach `RenderOnce`. | Keep compiling; update learning docs vocabulary. |

The audit found no Tier C requirement for object-safe heterogeneous widget storage in K03. The
existing route and component APIs can remain on `Render`, `RenderOnce`, and `IntoElement` while the
new `ElementBuilder` path proves immutable recipe semantics.

## Core Design

### `Render`

`Render` remains:

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

The `&mut self` receiver is still correct for engine views because views are entity-backed runtime
objects. They own mutable state, receive `Context<Self>`, call `cx.notify()`, and can participate in
`AnyView` caching. K03 documentation must stop presenting this as the final immutable widget build
model.

### `RenderOnce` and `Component<C>`

`RenderOnce` remains:

```rust
pub trait RenderOnce: 'static {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;
}
```

K03 does not deprecate `RenderOnce`. Deprecating it now would create noisy fallout across Tier C
before the Framework replacement exists. `Component<C>` remains doc-hidden and remains the
`derive(IntoElement)` target. Its `Component::key` behavior continues to use K02 identity.

### `ElementBuilder`

K03 introduces:

```rust
pub trait ElementBuilder: 'static {
    fn build(&self, cx: &mut ElementBuildCx<'_>) -> impl IntoElement;
}
```

This trait models a pure engine element recipe: the recipe object is borrowed immutably, while the
context carries the runtime handles needed to produce the existing `Element` tree. It is intentionally
not called `Widget`, and it does not define state or reconciliation hooks.

### `BuildElement<B>`

`BuildElement<B>` is the explicit adapter from `ElementBuilder` to `IntoElement`:

```rust
#[track_caller]
pub fn build_element<B: ElementBuilder>(builder: B) -> BuildElement<B>;
```

The adapter owns the builder value, captures the caller location for Local identity, supports
`key(impl Into<ElementId>)`, implements `Element`, and stores the built child element for prepaint
and paint. It does not add a second render tree.

K03 intentionally does not add a blanket `impl<T: ElementBuilder> IntoElement for T`. The explicit
adapter avoids coherence surprises for downstream types that already implement `IntoElement` and
keeps the new path visibly distinct from the future Framework widget path.

## `flui-core` vs `flui-framework` Boundary Decision

K03 lands a minimal `flui-core` substrate, not a `flui-framework` precursor crate.

Rationale:

- The architecture says final Framework APIs live in `flui-framework`; K03 respects that.
- Creating a tiny `flui-framework` crate before SF01 would force dependency and naming decisions
  without enough Framework design context.
- A core-local `ElementBuilder` is useful immediately because it clarifies that `&self` element
  recipes are not the same thing as mutable `Render` views.
- The name leaves `flui_framework::Widget` and `flui_framework::BuildCx` available for SF01/SF03.
- `flui-core` does not depend upward on any Framework or Ecosystem crate.

No Cargo workspace membership changes are part of K03.

## Object-Safety, RPITIT, and Erasure

`ElementBuilder` uses RPITIT:

```rust
fn build(&self, cx: &mut ElementBuildCx<'_>) -> impl IntoElement;
```

This is intentionally generic-only and not dyn-compatible. K03 does not add `Box<dyn ElementBuilder>`
or an erased `AnyWidget` equivalent.

Decision:

- Generic-only `ElementBuilder` is acceptable because K03 is an engine recipe bridge, not a
  heterogeneous widget storage layer.
- RPITIT keeps the hot path allocation-free for the common case.
- Monomorphization cost is limited to concrete builder types, matching existing `RenderOnce` and
  `IntoElement` usage.
- Object-safe heterogeneous storage is deferred to SF01/SF07, where reconciliation and state storage
  can make one coherent erasure decision.
- No erased wrapper may be introduced in K03 unless a later spec amendment proves it stays off the
  rebuild hot path.

## `ElementBuildCx`

`ElementBuildCx<'_>` is an engine context, not final Framework `BuildCx`.

It exposes:

- `global_id()` for the current build adapter identity, when available.
- `window()` and `app()` reborrows for existing engine APIs.
- `with_window_app(...)` for APIs that need both handles.
- `read_inherited<T>()` for non-subscribing K01 reads.
- `inherit<T>()` for subscribing K01 reads when a stable element id and active view are present.

It does not expose:

- `setState`
- reconciliation state
- `did_update_widget`
- `dispose`
- child diffing
- Theme/MediaQuery/Localizations ergonomics

Provider subscriptions made through `ElementBuildCx::inherit` attach to the `BuildElement<B>`
namespace identity. Missing stable identity or active view returns `None` in release and trips debug
assertions, matching the K01 lifecycle-context policy.

## Identity Behavior

`BuildElement<B>` mirrors `Component<C>` identity rules:

- `BuildElement::new` / `build_element` captures `Location::caller()` with `#[track_caller]`.
- `BuildElement::key(...)` overrides the callsite fallback with an explicit `ElementId`.
- `Key::value(...)` and `Key::global(...)` work through `Into<ElementId>`.
- The adapter pushes a type-name namespace for `B` while building and traversing the produced child.
- Repeated same-callsite adapters get K02 Local occurrences.
- Reordered adapters should use value/global keys to retain inner state/provider identity.

This preserves the existing "Element tree plus identity stack" model; no Flutter-style Widget tree
or RenderObject tree is introduced.

## Root Mounting Policy

Window roots remain `V: Render` in K03:

- `App::open_window<V: Render>`
- `WindowHandle<V>`
- `TestApp::open_window<V: Render>`
- `TestAppWindow<V>`
- `AnyView::from(Entity<V>)`

A pure-build value may be used inside a render tree through `build_element(builder)`. It cannot be a
root view until SF07 defines the Framework mounting story.

## Macro Strategy

K03 does not add `derive(Widget)` or `derive(ElementBuilder)`.

`derive(IntoElement)` continues to emit `Component<Self>` for `RenderOnce` implementers. `derive(Render)`
continues to target `Render`. K03 adds tests and docs around compatibility rather than changing macro
expansion.

## Logging and Performance Policy

No committed per-element, per-build, per-layout, per-prepaint, or per-paint logs are allowed in K03.
Temporary local diagnostics are acceptable while tracing adapter boundaries, but they must be removed
before commit. Runtime misuse should be expressed through type boundaries, clear panic/debug-assert
messages, and tests.

The new adapter must not allocate beyond existing element construction. It must not box the builder,
erase build output, or allocate a second runtime tree.

## Rejected Alternatives

| Alternative | Rejection reason |
|---|---|
| Rename `Render` to `Build` | Breaks the correct mutable engine view model and still does not create Framework semantics. |
| Make `Render::render` take `&self` | Invalid for entity-backed mutable views and `Context<Self>` workflows. |
| Deprecate `RenderOnce` in K03 | Creates Tier C churn before a replacement Framework API exists. |
| Add `flui_core::Widget` | Conflicts with the planned `flui-framework::Widget` ownership boundary. |
| Create `flui-framework` in K03 | Pulls Phase II-F decisions into a kernel cleanup task. |
| Add erased `AnyWidget` in K03 | Premature object-safety decision and likely hot-path allocation. |
| Blanket-implement `IntoElement` for every `ElementBuilder` | Creates downstream coherence traps and hides the boundary K03 is trying to make explicit. |

## Migration Plan

Existing code keeps working:

- Keep using `Render` for window roots and mutable engine views.
- Keep using `RenderOnce` plus `#[derive(IntoElement)]` for existing stateless engine recipes.
- Keep using `Component::key`, `Provider::new_keyed`, `Window::use_state`, and
  `Window::use_keyed_state` as today.

New code may use `ElementBuilder` when it wants immutable recipe semantics without Framework state:

```rust
struct LabelRow {
    label: SharedString,
}

impl ElementBuilder for LabelRow {
    fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
        div().child(self.label.clone())
    }
}

let row = build_element(LabelRow { label: "Name".into() });
```

Use `.key(Key::value(...))` on `BuildElement<B>` for repeated or reordered builder boundaries.

## Review Gates

Before merge, K03 requires:

- Architecture review for the spec and any changes touching `App`, `Entity`, `Context`, `Window`,
  `Element`, or Framework boundary concepts.
- Migration-risk review for public trait, adapter, docs, and downstream behavior changes.
- Rust API migration review for new public types, re-exports, RPITIT use, object-safety decision,
  and macro compatibility.
- GPU review only if implementation unexpectedly touches `scene`, platform renderers, shader,
  pipeline, offscreen, or GPU determinism code.

## Validation Plan

- Focused `flui-core` tests for `ElementBuilder`, `BuildElement`, identity, provider reads, and
  compatibility with existing render paths.
- Macro compatibility coverage for `derive(Render)` and `derive(IntoElement)`.
- Tier C compile checks for `flui-widgets`, `flui-material`, and `flui-navigator`.
- Example checks for the current demos and the updated learn example.
- Workspace validation after focused checks are green.

## Known Limitations

- `ElementBuilder` is not object-safe.
- `ElementBuilder` is not the final Framework widget trait.
- Pure-build roots are not supported.
- No reconciliation or state retention is added for builder values beyond existing element identity
  and `Window::use_state` / `use_keyed_state` mechanics.
- Provider ergonomics remain low-level; final `BuildCx::inherit<T>()` belongs to SF03.
