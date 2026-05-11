# K05 - Element Trait Context Object Design

**Date:** 2026-05-11
**Phase:** 0-K Kernel Cleanup
**Status:** Implemented
**Plan:** `.ai-factory/plans/feature-K05-element-context-object.md`

## Summary

K05 replaces the low-level `Element` lifecycle method parameter bundles with explicit
context objects:

- `LayoutCx<'_>`
- `PrepaintCx<'_>`
- `PaintCx<'_>`

The goal is not to introduce Framework-tier `BuildCx`, Provider rewrite, Widget identity, or
stateful reconciliation. K05 stays in Tier A (`flui-core`) and narrows the engine `Element`
surface so K01-K04 can build on clean lifecycle borrow points.

This is API-breaking for custom `Element` implementations.

## Motivation

The current `Element` trait exposes lifecycle plumbing directly:

```rust
fn request_layout(
    &mut self,
    id: Option<&GlobalElementId>,
    inspector_id: Option<&InspectorElementId>,
    window: &mut Window,
    cx: &mut App,
) -> (LayoutId, Self::RequestLayoutState);

fn prepaint(
    &mut self,
    id: Option<&GlobalElementId>,
    inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    request_layout: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut App,
) -> Self::PrepaintState;

fn paint(
    &mut self,
    id: Option<&GlobalElementId>,
    inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    request_layout: &mut Self::RequestLayoutState,
    prepaint: &mut Self::PrepaintState,
    window: &mut Window,
    cx: &mut App,
);
```

This spreads the same 4-7 lifecycle arguments across every built-in element and every custom
element author. It also makes follow-up work harder:

- K01 Provider rewrite needs a clean place to hang inherited-value reads/subscriptions.
- K02 identity/key work needs a clear boundary for `GlobalElementId` propagation.
- K03 build separation needs engine lifecycle calls to stop looking like app callbacks.
- K04 frame/effect work needs lifecycle phase access to be explicit.

K05 turns lifecycle state into named context objects while preserving the existing single-tree
engine model.

## Non-Goals

- Do not introduce `flui-framework`.
- Do not introduce `Widget`, `BuildCx`, `State`, `Key`, reconciliation, dirty lists, or
  Framework-tier Provider ergonomics.
- Do not rewrite `provider/stack.rs`; K01 owns Provider.
- Do not shard `App`, `Window`, or pipeline ownership; K06/K01 can revisit that.
- Do not add new platform code under `crates/flui-core/src/platform/**`.
- Do not add committed hot-path logging in layout, prepaint, or paint.

## Current Inventory

### Element Trait Boundary

`crates/flui-core/src/element.rs` owns the public trait and object-erased traversal:

| Item | Location | Role |
|---|---:|---|
| `Element` trait | `element.rs:51` | Public low-level element lifecycle surface |
| `ElementObject` | `element.rs:305` | Object-safe erased lifecycle dispatch |
| `Drawable<E>` | `element.rs:323` | Stores lifecycle phase state and computes ids/bounds |
| `AnyElement` | `element.rs:593` | Erased element handle used by parents, views, and Window |
| `AnyElement::request_layout` | `element.rs:613` | Child layout traversal |
| `AnyElement::prepaint` | `element.rs:619` | Child prepaint traversal and focus assignment |
| `AnyElement::paint` | `element.rs:632` | Child paint traversal |
| `AnyElement::layout_as_root` | `element.rs:637` | Root/item measurement traversal |
| `AnyElement::prepaint_at` | `element.rs:648` | Offset child/root prepaint traversal |
| `AnyElement::prepaint_as_root` | `element.rs:659` | Combined layout + prepaint boundary |

### Production Element Implementations

The production surface contains 21 `Element` implementations:

| # | Implementation | File |
|---:|---|---|
| 1 | `Component<C>` | `crates/flui-core/src/element.rs` |
| 2 | `AnyElement` | `crates/flui-core/src/element.rs` |
| 3 | `Empty` | `crates/flui-core/src/element.rs` |
| 4 | `ElementAnimationElement<E>` | `crates/flui-core/src/elements/animation.rs` |
| 5 | `Anchored` | `crates/flui-core/src/elements/anchored.rs` |
| 6 | `Deferred` | `crates/flui-core/src/elements/deferred.rs` |
| 7 | `Canvas<T>` | `crates/flui-core/src/elements/canvas.rs` |
| 8 | `Div` | `crates/flui-core/src/elements/div.rs` |
| 9 | `Stateful<E>` | `crates/flui-core/src/elements/div.rs` |
| 10 | `ImageCacheElement` | `crates/flui-core/src/elements/image_cache.rs` |
| 11 | `Img` | `crates/flui-core/src/elements/img.rs` |
| 12 | `List` | `crates/flui-core/src/elements/list.rs` |
| 13 | `Surface` | `crates/flui-core/src/elements/surface.rs` |
| 14 | `Svg` | `crates/flui-core/src/elements/svg.rs` |
| 15 | `UniformList` | `crates/flui-core/src/elements/uniform_list.rs` |
| 16 | `&'static str` | `crates/flui-core/src/elements/text.rs` |
| 17 | `SharedString` | `crates/flui-core/src/elements/text.rs` |
| 18 | `StyledText` | `crates/flui-core/src/elements/text.rs` |
| 19 | `InteractiveText` | `crates/flui-core/src/elements/text.rs` |
| 20 | `AnyView` | `crates/flui-core/src/view.rs` |
| 21 | `ProviderElement<T>` | `crates/flui-core/src/provider/element.rs` |

There are also 2 test-only `CustomElement` implementations in
`crates/flui-core/src/key_dispatch.rs`.

### Helper Layers That Must Move Too

`Interactivity::{request_layout, prepaint, paint}` in `crates/flui-core/src/elements/div.rs`
currently accepts the same old raw bundles. K05 must migrate this layer with the elements that
depend on it (`Div`, `Img`, `Svg`, `UniformList`) or the parameter explosion will simply move
one helper call lower.

### Window and Test Harness Entrypoints

`Window` drives element traversal through `AnyElement` helpers in these paths:

- root element prepaint and paint
- prompt element prepaint and paint
- active drag element prepaint and paint
- tooltip element prepaint and paint
- deferred draw prepaint and paint
- inspector prepaint and paint

Key locations:

- `crates/flui-core/src/window.rs:2474`
- `crates/flui-core/src/window.rs:2503`
- `crates/flui-core/src/window.rs:2591`
- `crates/flui-core/src/window.rs:2652`
- `crates/flui-core/src/window.rs:5532`
- `crates/flui-core/src/app/test_context.rs:840`

## Target API

The public trait becomes:

```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<ElementId>;

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>>;

    fn request_layout(
        &mut self,
        cx: &mut LayoutCx<'_>,
    ) -> (LayoutId, Self::RequestLayoutState);

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

### Context Types

The context objects are public, but fields stay private:

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

Minimum public accessors:

```rust
impl<'a> LayoutCx<'a> {
    pub fn global_id(&self) -> Option<&GlobalElementId>;
    pub fn inspector_id(&self) -> Option<&InspectorElementId>;
    pub fn window(&mut self) -> &mut Window;
    pub fn app(&mut self) -> &mut App;
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R;
}

impl<'a> PrepaintCx<'a> {
    pub fn global_id(&self) -> Option<&GlobalElementId>;
    pub fn inspector_id(&self) -> Option<&InspectorElementId>;
    pub fn bounds(&self) -> Bounds<Pixels>;
    pub fn window(&mut self) -> &mut Window;
    pub fn app(&mut self) -> &mut App;
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R;
}

impl<'a> PaintCx<'a> {
    pub fn global_id(&self) -> Option<&GlobalElementId>;
    pub fn inspector_id(&self) -> Option<&InspectorElementId>;
    pub fn bounds(&self) -> Bounds<Pixels>;
    pub fn window(&mut self) -> &mut Window;
    pub fn app(&mut self) -> &mut App;
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R;
}
```

`with_window_app` is the compatibility escape hatch for existing APIs that still need both
`&mut Window` and `&mut App` at the same time, including `RenderOnce::render`, callbacks, and
some `Window` helpers. It avoids forcing custom element authors into repeated manual reborrows.

### Derived / Nested Contexts

K05 needs an explicit derived-context API. Internal cases such as `InteractiveText` delegate to
`StyledText` with `global_id = None` while preserving inspector id and bounds. This must not be
done by reconstructing ad-hoc local structs in element code.

Required shape:

```rust
impl<'a> LayoutCx<'a> {
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut LayoutCx<'_>) -> R,
    ) -> R;
}

impl<'a> PrepaintCx<'a> {
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut PrepaintCx<'_>) -> R,
    ) -> R;

    pub fn with_bounds<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut PrepaintCx<'_>) -> R,
    ) -> R;
}

impl<'a> PaintCx<'a> {
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut PaintCx<'_>) -> R,
    ) -> R;

    pub fn with_bounds<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut PaintCx<'_>) -> R,
    ) -> R;
}
```

If implementation shows a better name (`for_child`, `with_metadata`, etc.), it is acceptable as
long as the behavior is explicit and no element manually rebuilds context structs outside
`element.rs`.

### Convenience Delegates

K05 may add small delegates where they reduce boilerplate:

- `LayoutCx::request_layout(...)`
- `PrepaintCx::layout_bounds(...)`
- `PaintCx::paint_quad(...)`
- `PaintCx::paint_path(...)`

It must not clone the whole `Window` API onto the context types. The context objects are lifecycle
accessors, not a replacement for `Window`.

## AnyElement Migration

`AnyElement` child traversal should take lifecycle contexts:

```rust
impl AnyElement {
    pub fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> LayoutId;
    pub fn prepaint(&mut self, cx: &mut PrepaintCx<'_>) -> Option<FocusHandle>;
    pub fn paint(&mut self, cx: &mut PaintCx<'_>);
}
```

Root/item measurement helpers should also move to contexts where they are used inside element
trees:

```rust
pub fn layout_as_root(
    &mut self,
    available_space: Size<AvailableSpace>,
    cx: &mut LayoutCx<'_>,
) -> Size<Pixels>;

pub fn prepaint_at(
    &mut self,
    origin: Point<Pixels>,
    cx: &mut PrepaintCx<'_>,
) -> Option<FocusHandle>;
```

`Window` may keep small `pub(crate)` boundary helpers that construct root contexts because the
root has no parent context. Those helpers should live in `element.rs`/`window.rs` internals and
must not preserve the old public custom-element shape.

## Drawable Migration

`Drawable<E>` remains the owner of lifecycle state:

- `ElementDrawPhase::RequestLayout` still stores `layout_id`, `global_id`, `inspector_id`, and
  request-layout state.
- `ElementDrawPhase::Prepaint` still stores `node_id`, `bounds`, request-layout state, and
  prepaint state.
- `Drawable::request_layout` computes `GlobalElementId` and `InspectorElementId`, constructs
  `LayoutCx`, and calls `Element::request_layout`.
- `Drawable::prepaint` computes `bounds`, pushes the dispatch node, constructs `PrepaintCx`, and
  calls `Element::prepaint`.
- `Drawable::paint` activates the stored dispatch node, constructs `PaintCx`, and calls
  `Element::paint`.

No heap allocation is introduced by context construction. The context structs are stack values
wrapping existing mutable borrows and small metadata.

## Interactivity Migration

`Interactivity` currently carries the old surface forward:

```rust
pub fn request_layout(
    &mut self,
    global_id: Option<&GlobalElementId>,
    inspector_id: Option<&InspectorElementId>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(Style, &mut Window, &mut App) -> LayoutId,
) -> LayoutId
```

K05 must move this helper layer to contexts or context-derived values so built-in elements do not
retain a second old-shape API after the trait changes. `Div`, `Img`, `Svg`, and `UniformList`
are the primary callers.

## Panic-Safety Classification

Current behavior is not a general panic-recovery contract for element lifecycle panics:

- `Drawable::request_layout` pushes `window.element_id_stack` before calling the element and pops
  only after the call returns.
- `Drawable::prepaint` pushes `window.element_id_stack`, pushes a dispatch node, and pops only
  after the element returns.
- `Window::with_id` and `Window::with_element_namespace` also push/pop without a guard.
- `Drawable::paint` pushes `window.element_id_stack` and relies on normal return to pop it.

K05 classifies this as an existing limitation rather than silently widening the runtime contract.
The implementation may add cleanup only if it can do so with a small, well-reviewed change and
targeted tests. Otherwise, the K05 implementation must document the limitation and avoid hiding
panic behavior behind `catch_unwind` or new unsafe code.

K07's App bookkeeping panic restoration remains unchanged and is not redefined by K05.

## Public Surface and Re-Exports

`flui-core` currently re-exports `element::*` from `crates/flui-core/src/lib.rs`. K05 should keep
the new context objects reachable through the same explicit public path as `Element`.

`crates/flui-core/src/prelude.rs` should be updated to include the context types only if custom
element authors need them when implementing `Element`. The likely answer is yes for:

- `LayoutCx`
- `PrepaintCx`
- `PaintCx`

No blanket re-export changes outside the existing `element::*` path are needed.

## Migration Sketch

Old:

```rust
fn prepaint(
    &mut self,
    global_id: Option<&GlobalElementId>,
    inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    state: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut App,
) -> Self::PrepaintState {
    self.child.prepaint(window, cx);
    window.insert_hitbox(bounds, HitboxBehavior::Normal)
}
```

New:

```rust
fn prepaint(
    &mut self,
    cx: &mut PrepaintCx<'_>,
    state: &mut Self::RequestLayoutState,
) -> Self::PrepaintState {
    self.child.prepaint(cx);
    cx.window().insert_hitbox(cx.bounds(), HitboxBehavior::Normal)
}
```

Nested metadata adjustment:

```rust
cx.with_global_id(None, |cx| {
    self.text.prepaint(cx, state);
});
```

## Test Strategy

K05 needs targeted tests in addition to workspace compilation:

- context accessors expose the expected `global_id`, `inspector_id`, and `bounds`;
- `AnyElement::prepaint` still returns a newly-assigned `FocusHandle` when focus appears during
  prepaint;
- child traversal through context objects preserves layout/prepaint/paint ordering;
- `InteractiveText` or an equivalent synthetic element can delegate to a nested element with
  adjusted `global_id`;
- Provider push/pop behavior remains unchanged until K01;
- if panic cleanup behavior is changed, tests must prove id-stack/dispatch-stack restoration.

Required validation:

```text
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo test -p flui-core element --tests
cargo test -p flui-core key_dispatch --tests
cargo test -p flui-core provider --tests
cargo check -p flui-navigator --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p lock-checks -- check-stubs
cargo run -q -p lock-checks -- check-platform-imports
```

## Review Gates

Before opening or merging the K05 PR:

- `flui-arch-reviewer` reviews the design and implementation because K05 touches `Element`,
  `Window`, and core runtime lifecycle.
- `migration-risk-adversary` reviews migration completeness because K05 is API-breaking and
  touches every built-in custom element.
- `rust-api-migration-auditor` reviews the public API change and re-export/prelude surface.
- `wgpu-gpu-reviewer` is required only if implementation touches `scene.rs`, wgpu, Metal,
  DirectX, shaders, pipeline cache, or offscreen rendering.

## Known Limitations

- Context objects still carry monolithic `&mut Window` and `&mut App`. K05 does not split borrow
  domains.
- Provider remains thread-local stack based until K01.
- Root boundary code still has to construct contexts from `&mut Window` and `&mut App`; this is
  not a Framework `BuildCx`.
- Element lifecycle panics are classified but not automatically caught by K05.

## Done Criteria

- `Element` lifecycle methods use context objects.
- Built-in production `Element` implementations and test-only custom elements compile.
- `AnyElement`, `Drawable`, `Interactivity`, `Window` root/deferred/inspector paths, and
  `TestAppContext::draw` no longer depend on the old public trait shape.
- No committed hot-path logging or heap allocation is introduced by context construction.
- Migration guide and changelog entries document the breaking change.
- Targeted and full validation pass.
