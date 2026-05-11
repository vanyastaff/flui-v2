# K05 Element Lifecycle Context Migration

K05 replaces the raw `Element` lifecycle argument bundles with small context
objects. This is an API-breaking cleanup for custom low-level elements.

## What Changed

Old custom elements received identity, bounds, `Window`, and `App` as separate
parameters:

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
    state: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut App,
) -> Self::PrepaintState;
```

New custom elements receive phase-specific contexts:

```rust
fn request_layout(
    &mut self,
    cx: &mut LayoutCx<'_>,
) -> (LayoutId, Self::RequestLayoutState);

fn prepaint(
    &mut self,
    cx: &mut PrepaintCx<'_>,
    state: &mut Self::RequestLayoutState,
) -> Self::PrepaintState;

fn paint(
    &mut self,
    cx: &mut PaintCx<'_>,
    state: &mut Self::RequestLayoutState,
    prepaint: &mut Self::PrepaintState,
);
```

## Common Rewrite

Use context accessors for identity and bounds:

```rust
let global_id = cx.global_id();
let inspector_id = cx.inspector_id();
let bounds = cx.bounds(); // prepaint and paint only
```

Use `with_window_app` for existing APIs that still need both handles:

```rust
fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, ()) {
    cx.with_window_app(|window, cx| {
        let layout_id = window.request_layout(Style::default(), [], cx);
        (layout_id, ())
    })
}
```

`window()` and `app()` are available for narrow one-handle reborrows, but
`with_window_app` is usually the least surprising migration for existing code.

## AnyElement Traversal

`AnyElement` child traversal now takes lifecycle contexts too:

```rust
child.request_layout(cx);
child.prepaint(cx);
child.paint(cx);
```

Root/internal boundaries in `Window` construct contexts for you. Custom element
authors should normally pass along the context they already received instead of
manually rebuilding one.

## Notes

- `LayoutCx`, `PrepaintCx`, and `PaintCx` are exported from `flui_core`.
- K05 does not add panic recovery for lifecycle panics. Existing id-stack and
  dispatch-stack panic limitations remain documented in the K05 design spec.
- `Provider` keeps its current stack behavior until K01.
- Framework-tier `BuildCx`, `Widget`, `Key`, and reconciliation are not part of
  K05.
