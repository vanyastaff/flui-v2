# K01 Provider Rewrite Migration

K01 replaces the old thread-local provider stack with a per-`Window` inherited-value registry. This is an intentional breaking change: provider reads now need lifecycle context so flui can record dependents and invalidate cached views correctly.

## What Changed

Removed from the public surface:

```rust
flui_core::read::<T>()
flui_core::try_read::<T>()
flui_widgets::read::<T>()
flui_widgets::try_read::<T>()
```

Kept:

```rust
flui_core::Provider
flui_core::InheritedValue
flui_widgets::Provider
flui_widgets::InheritedValue
```

Added low-level Engine APIs:

```rust
cx.read_inherited::<T>() // non-subscribing lookup
cx.inherit::<T>()        // subscribing lookup
window.read_inherited::<T>()
```

`cx.inherit::<T>()` is available on `LayoutCx`, `PrepaintCx`, and `PaintCx`. SF03 will wrap this in the future Framework `BuildCx`.

## Value Requirements

Inherited values now require `PartialEq`:

```rust
pub trait InheritedValue: Any + Clone + PartialEq + Send + Sync + 'static {}
```

Equality suppresses unnecessary provider invalidation. If the value is large, wrap it in a cheap clone handle such as `Arc<T>`.

## Before And After

Old global lookup:

```rust
let theme = flui_core::try_read::<Theme>();
```

New non-subscribing lookup inside element lifecycle code:

```rust
let theme = cx.read_inherited::<Theme>();
```

Old panic-on-missing lookup:

```rust
let theme = flui_core::read::<Theme>();
```

New explicit handling:

```rust
let theme = cx
    .read_inherited::<Theme>()
    .expect("Theme provider is required above this element");
```

Use `inherit` when the current element/view should be invalidated after the nearest provider changes. Use `read_inherited` for one-shot lookups that should not subscribe.

## Repeated Providers

`Provider::new(value, child)` uses the callsite as a K01 identity fallback. If you construct repeated same-type providers from the same source location, give them explicit keys:

```rust
Provider::new_keyed(("theme-row", index), theme, child)
```

This limitation exists only until K02 introduces proper framework-level `Key` semantics.

## Cached Views

Cached views now preserve inherited dependencies. A view that used `cx.inherit::<Theme>()` will not silently lose that dependency when its prepaint/paint output is reused from cache.

## Deferred To Later Work

- K02 owns stable `Key` semantics beyond K01 explicit provider keys.
- K03 and K04 own the next runtime cleanup steps.
- SF03 owns final Framework `BuildCx::read<T>()` / `BuildCx::inherit<T>()` ergonomics.
