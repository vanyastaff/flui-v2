# K03 Render to Build Separation Migration Guide

## Summary

K03 keeps the existing engine APIs working and adds a narrow immutable recipe
path in `flui-core`:

- `Render` remains the mutable, entity-backed view trait for roots and cached
  views.
- `RenderOnce` remains the consuming stateless engine recipe path.
- `Component<C: RenderOnce>` remains the `derive(IntoElement)` adapter.
- New `ElementBuilder` values build from `&self` through
  `build_element(builder)`.

This is not the final `flui-framework::Widget` API. `Widget`, `State`,
reconciliation, dirty lists, `setState`, and final `BuildCx` remain deferred to
Phase II-F.

## Keep Using `Render`

Use `Render` for window roots and mutable engine views:

```rust
impl Render for Counter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(format!("{}", self.value))
    }
}
```

No K03 migration is required for existing roots. `App::open_window`,
`WindowHandle<V>`, `AnyView`, and the test window helpers remain constrained to
`V: Render`.

## Keep Using `RenderOnce`

Existing stateless recipes can stay on `RenderOnce`:

```rust
#[derive(IntoElement)]
struct Label {
    text: SharedString,
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().child(self.text)
    }
}
```

`RenderOnce` is not deprecated in K03 because Tier C crates still use it heavily.
It is still an engine recipe, not the final Framework widget trait.

## Use `ElementBuilder` For Immutable Recipes

Use `ElementBuilder` when the recipe should build from borrowed immutable
configuration:

```rust
struct Label {
    text: SharedString,
}

impl ElementBuilder for Label {
    fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
        div().child(self.text.clone())
    }
}

let label = build_element(Label { text: "Hello".into() });
```

K03 intentionally uses an explicit `build_element(...)` adapter. There is no
blanket `impl<T: ElementBuilder> IntoElement for T`, so existing manual
`IntoElement` impls do not run into coherence surprises.

## Keys And Identity

`BuildElement<B>` supports `.key(...)`, mirroring `Component<C>::key(...)`:

```rust
build_element(Label { text })
    .key(Key::value(("row-label", row_id)))
```

Use value keys for repeated or reordered builder boundaries. Local callsite
identity remains fine for fixed tree shapes.

## Provider Reads

`ElementBuildCx` exposes the same low-level inherited-value distinction as the
K01 lifecycle contexts:

- `cx.read_inherited::<T>()` reads without subscribing.
- `cx.inherit::<T>()` reads and subscribes the current rendered view through the
  build boundary's stable element identity.

Final ergonomic Framework provider APIs remain SF03 work.

## What Remains Deferred

K03 does not add:

- `flui-framework`
- `Widget`
- `StatefulWidget`
- `State<W>` / `WidgetState<W>`
- reconciliation
- dirty-list scheduling
- `setState`
- pure-build roots
- object-safe heterogeneous widget storage

## Logging Policy

Do not add committed per-build, per-layout, per-prepaint, or per-paint logs while
migrating. Use tests and clear type boundaries instead of hot-path diagnostics.
