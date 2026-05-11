# K02 Element Identity and Key Migration Guide

K02 adds the engine identity substrate that future Framework reconciliation will use. It keeps
existing `ElementId` callsites compiling, but introduces `Key` as the preferred user-facing way to
express identity intent.

Design spec:
[`2026-05-11-K02-element-identity-key-design.md`](../specs/2026-05-11-K02-element-identity-key-design.md).

## What Changed

- `ElementId` moved out of `window.rs` into the Element identity module and is still re-exported
  from `flui_core::*`.
- `Key` is now available with Local, Value, and Global identity forms.
- `ElementId::CodeLocation` is now an input form. The window identity stack normalizes it into a
  parent-scoped `ElementId::Local(LocalElementId)` segment with a sibling occurrence counter.
- `Key::value` accepts `ValueKey` conversions, not arbitrary `ElementId` or `Key` values. This keeps
  Local and Global identity from being accidentally labeled as reorder-stable Value identity.
- `Window` no longer stores a raw `SmallVec<[ElementId; 32]>`; it uses an internal identity stack
  that tracks Local occurrence and debug duplicate-key diagnostics.
- `Provider::new_keyed`, `Window::with_id`, `Window::with_element_namespace`, and
  `Window::use_keyed_state` accept `Key` through `Into<ElementId>`.

## Choosing a Key

Use Local identity only when sibling order is fixed:

```rust
let key = Key::local();
```

Local keys are caller-location based and disambiguated by occurrence under the current parent. They
are deterministic for the same tree shape, but they are not reorder-stable.

Use Value identity for lists, tabs, rows, providers, component boundaries, or state that belongs to data:

```rust
window.use_keyed_state(Key::value(("row", row_id)), cx, |_, _| RowState::new());
Provider::new_keyed(Key::value(("theme", account_id)), theme, child);
```

Use Global identity only when you intentionally need the reserved global-key substrate:

```rust
let key = Key::global("app-shell");
```

K02 stores and compares global keys. Cross-tree move/reparent semantics are deferred to SF02.

## Repeated Call Sites

Before K02, two same-callsite Local ids could collide unless callers supplied explicit ids. After K02,
the identity stack assigns occurrence numbers within the parent namespace:

```rust
// Same callsite, two sibling Local ids:
// Local(location, 0), Local(location, 1)
for item in items {
    row(item);
}
```

This fixes collisions, not reordering. If `items` can be inserted, removed, or reordered, use
`Key::value(item.id)`.

## State

`Window::use_state` now uses caller location plus sibling occurrence. It remains convenient for fixed
tree shapes:

```rust
let hover = window.use_state(cx, |_, _| HoverState::default());
```

For movable children, migrate to `use_keyed_state`:

```rust
let row_state = window.use_keyed_state(Key::value(("row", row.id)), cx, |_, _| {
    RowState::default()
});
```

If state lives inside a repeated `RenderOnce` component, key the component boundary too:

```rust
div().child(Row { row }.into_element().key(("row", row.id)))
```

## Provider

`Provider::new` still uses caller-location fallback identity. It now benefits from Local occurrence
normalization for repeated sibling providers.

Use `Provider::new_keyed(Key::value(...))` when provider identity must follow data across reorder or
conditional movement:

```rust
Provider::new_keyed(Key::value(("theme", account_id)), theme, child)
```

Provider value-change invalidation, provider removal cleanup, and cached-view inherited dependency
replay are unchanged from K01.

## Duplicate Keys

Debug builds assert when two explicit sibling keys collide in the same lifecycle pass:

```rust
div().children([
    row(Key::value("same")),
    row(Key::value("same")), // debug assertion
])
```

Normal layout/prepaint/paint repeats do not count as duplicates. Release builds do not keep the
debug duplicate-tracking sets on the hot path; duplicate explicit sibling keys are therefore a
caller bug that can alias state/provider/cache identity instead of producing a release panic. Treat
debug duplicate-key failures as correctness issues, not as optional diagnostics.

## Cache Behavior

`AnyView::cached` behavior is preserved. K02 ensures cached views consume normalized global ids and
continue to replay K01 inherited provider dependencies. A public stateless element cache wrapper is
deferred to SF02/SF05 so it can be designed with the Framework reconciliation API instead of becoming
a premature Tier-A surface.

## Component<C>

`Component<C: RenderOnce>` remains an engine wrapper used by `derive(IntoElement)`. It is not the
Framework Widget adapter. K02 gives `Component` Local identity and an explicit `Component::key(...)`
escape hatch for repeated/reordered component boundaries. Framework Widget identity is SF01/SF02 work.
