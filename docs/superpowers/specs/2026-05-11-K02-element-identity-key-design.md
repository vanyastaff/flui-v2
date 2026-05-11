# K02 - Element Identity and Key Design

**Date:** 2026-05-11
**Phase:** 0-K Kernel Cleanup
**Status:** Draft implementation contract
**Plan:** `.ai-factory/plans/feature-K02-element-identity-key.md`

## Summary

K02 stabilizes the engine identity substrate in `flui-core` without introducing the
Framework tier. The design keeps `ElementId` as the low-level path segment used by
`GlobalElementId`, state retention, provider scopes, dispatch ids, and caches. It adds a
first-class `Key` API as the user-facing identity constructor for future Framework widgets and
for current keyed engine APIs.

The important split is:

- `Key`: public opaque identity intent, constructed as Local, Value, or Global.
- `ElementId`: normalized engine path segment stored in `GlobalElementId`.
- `GlobalElementId`: full identity path, still cheap to clone/hash via `Arc<[ElementId]>`.

K02 is intentionally not a reconciliation engine. SF01/SF02 own `Widget`, `BuildCx`, dirty
lists, keyed child matching, and cross-tree GlobalKey moves.

## Current Inventory

| Surface | Current role | K02 action |
|---|---|---|
| `Element::id(&self) -> Option<ElementId>` | Low-level optional path segment | Preserve as compatibility API; document that returned `CodeLocation` is normalized by the stack. |
| `ElementId` | Public enum in `window.rs` | Move to an Element-owned identity module and extend with normalized Local/Global variants. |
| `GlobalElementId` | `Arc<[ElementId]>` in `element.rs` | Preserve representation and construction from normalized stack segments. |
| `Window::element_id_stack` | Raw `SmallVec<[ElementId; 32]>` | Replace with `ElementIdStack`, which owns path plus local occurrence/duplicate state. |
| `Window::with_id` | Pushes explicit id for callback scope | Keep accepting `impl Into<ElementId>`; `Key` converts into `ElementId`. |
| `Window::with_global_id` | Reads current stack as `GlobalElementId` | Preserve behavior against normalized stack. |
| `Window::with_element_namespace` | Pushes namespace segment for state | Keep accepting `impl Into<ElementId>` and normalizing Local fallbacks. |
| `Window::use_state` | Uses caller `CodeLocation` | Normalize to `Local { source_location, occurrence }` in the current parent namespace. |
| `Window::use_keyed_state` | Uses explicit caller key | Keep explicit keys for loop/reorder-stable state. |
| `Window::with_element_state` | Stores state by `(GlobalElementId, TypeId)` | Preserve storage key; normalized ids improve collision behavior. |
| `AnyView::cached` | View-only cache consumer | Preserve behavior in K02; extract only internal identity substrate needed later if scoped. |
| K01 `Provider::new_keyed` | Provider-specific explicit id | Keep API and accept `Key` through `Into<ElementId>`. |
| K01 `Provider::new` | Source-location fallback | Normalize fallback through the same Local occurrence rules. |
| `derive(IntoElement)` / `Component<C>` | Macro-generated components | Preserve existing `Component<C>` ids and make source-location ids normalize uniformly. |
| `flui_core::*` re-exports | Public API surface | Preserve curated exports by re-exporting identity types from `element`. |

## Identity Model

`ElementId` remains the canonical engine segment type. Existing value-like variants remain valid:

- `Integer(u64)`
- `Name(SharedString)`
- `Uuid(Uuid)`
- `FocusHandle(FocusId)`
- `NamedInteger(SharedString, u64)`
- `Path(Arc<std::path::Path>)`
- `NamedChild(Arc<ElementId>, SharedString)`
- `View(EntityId)`

K02 adds normalized identity classes:

- `Local { source_location: Location<'static>, occurrence: u32 }`
- `Local(LocalElementId)`
- `GlobalKey(GlobalKey)`

`CodeLocation(Location<'static>)` remains as a compatibility input. It should not appear in newly
constructed global paths after it has passed through `ElementIdStack::push`; the stack rewrites it
to `Local`.

`Key` is introduced as:

- `Key::local()` - caller-location fallback, using `#[track_caller]`.
- `Key::value(value)` - explicit reorder-stable value identity, backed by `ValueKey` conversions
  for the existing value-like `ElementId` variants.
- `Key::global(value)` - reserved global identity substrate, represented by `GlobalKey`.

The API deliberately avoids erased `dyn Hash` / `dyn Eq` value keys. `Key::value` does not accept
Local or Global identity inputs. For K02, supported values are integers, strings, `SharedString`,
`Uuid`, `FocusHandle`, paths, tuples, and `NamedChild` composed from `ValueKey`.

## Local Key Semantics

Local identity is a fallback for elements that do not provide an explicit value/global key. It is
stable for a fixed tree shape but not reorder-stable.

Algorithm:

1. Each parent identity namespace owns a map from `Location<'static>` to next occurrence.
2. Pushing `ElementId::CodeLocation(location)` allocates the current occurrence from the parent map.
3. The pushed segment becomes `ElementId::Local(LocalElementId)`.
4. A new empty child namespace is pushed for that element's children.
5. Popping the element restores the parent namespace.

Consequences:

- Two same-callsite siblings get `occurrence = 0`, `occurrence = 1`, etc.
- Reordered loop children using Local identity can reuse state incorrectly by position. Users must use
  value/global keys for reorder-sensitive lists.
- Source locations must be stored in release builds because they are part of runtime identity, not only
  diagnostics.

## Duplicate Key Diagnostics

Duplicate explicit sibling keys are programmer errors. In debug builds, `ElementIdStack` records explicit
non-Local sibling segments in the current parent namespace and asserts if the same segment is pushed twice
within one lifecycle pass.

Release builds do not allocate duplicate tracking sets and preserve existing behavior. This keeps the hot
path predictable while making invalid identity trees loud during development.

Lifecycle repeat handling:

- Layout, prepaint, and paint each start a fresh identity pass at root entry points.
- A keyed child appearing once in layout and once in prepaint is not a duplicate.
- A keyed child appearing twice under the same parent during one phase is a duplicate.
- Deferred draw snapshots include the identity stack resolver state captured at scheduling time, so
  deferred prepaint/paint continues in the same parent namespace instead of reconstructing only raw path
  segments.

## Deferred Draws

`DeferredDraw` must store the full `ElementIdStack`, not just the path. This preserves:

- the normalized ancestor path,
- pending local occurrence counters for the current namespace,
- debug-only explicit sibling key sets,
- child namespace depth.

Deferred prepaint and paint restore the snapshot before entering the deferred closure and clear the window
stack after the pass. Root phase resets must not erase the deferred snapshot immediately before replay.

## Provider and State Migration

K01 Provider scopes already salt by provider value type. K02 keeps that rule and improves the base identity:

- `Provider::new` continues to use caller location, but it normalizes through Local occurrence rules.
- `Provider::new_keyed` accepts the existing `impl Into<ElementId>` path and therefore accepts `Key`.
- Same-callsite sibling providers no longer collide when they are siblings in one parent namespace.
- Reorder-stable provider identity still requires explicit value/global keys.

`Window::use_state` follows the same Local occurrence model as `Provider::new`. `Window::use_keyed_state`
and `Window::with_element_state` continue to be the explicit/stable alternatives.

## Cache Scope

K02 preserves `AnyView::cached` behavior and the K01 inherited dependency replay semantics. A generalized
public stateless element cache wrapper is deferred to SF02/SF05 unless the implementation can extract a
small internal helper without widening API surface.

Cache work in K02 is limited to ensuring cache keys and replay paths consume normalized `GlobalElementId`
values. Cache hit/miss logging is not committed in layout/prepaint/paint hot paths.

## Public API and Migration

Breaking surface:

- `ElementId` moves modules but remains re-exported from `flui_core::*`.
- `ElementId::CodeLocation` remains accepted but is a compatibility input.
- New code should prefer `Key::local()`, `Key::value(...)`, or `Key::global(...)` at API boundaries that
  are intended to model user identity. Moving `Component<C: RenderOnce>` boundaries can be keyed with
  `Component::key(...)`; value keys inside an unkeyed moving component do not key the component itself.

Non-breaking bridge:

- Existing `id(...)`, `with_id(...)`, `use_keyed_state(...)`, and `Provider::new_keyed(...)` callsites keep
  compiling through `Into<ElementId>`.
- Existing `Display`, `Debug`, `Hash`, and `Eq` behavior remains deterministic for map keys and diagnostics.

## Rejected Alternatives

- Rename `ElementId` to `Key` immediately. This creates unnecessary downstream churn and blurs engine path
  segments with user identity intent.
- Make `Key` an erased typed value container. Rust cannot soundly implement erased equality/hash without a
  narrow type-id protocol and careful collision rules; K02 does not need that risk.
- Track Local occurrence globally per frame. Local identity must be scoped by parent namespace, otherwise
  unrelated subtrees influence each other's ids.
- Commit per-element identity logs. Identity resolution is a hot path; tests and debug assertions are the
  right feedback channel.

## Review Gates

Before PR merge:

- `flui-arch-reviewer` for core runtime architecture and K-track consistency.
- `migration-risk-adversary` because identity refactoring can silently regress state/provider/cache reuse.
- `rust-api-migration-auditor` because K02 adds public identity types and changes public enum shape.

`wgpu-gpu-reviewer` is not required unless the implementation unexpectedly touches scene, wgpu, Metal,
DirectX, shader, or offscreen rendering code.

## Known Limitations

- Local occurrence identity is positional and not reorder-stable.
- GlobalKey cross-tree reparenting is represented but not implemented as a reconciliation feature.
- Full Framework `Widget` identity and keyed child matching remain SF01/SF02 work.
