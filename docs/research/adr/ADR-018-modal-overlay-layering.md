# ADR-018: Modal & overlay layering — defer_draw priority and per-window modal scope

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/window.rs` (`defer_draw`, modal helpers),
the hit-test pipeline in `gesture/`.
**Drivers:**
[zed-industries/zed#52013](https://github.com/zed-industries/zed/issues/52013),
[zed-industries/zed#52448](https://github.com/zed-industries/zed/issues/52448),
[zed-industries/zed#54017](https://github.com/zed-industries/zed/issues/54017) (related).

## Context

Three upstream issues describe layering and modality gaps:

- **#52013** — a folder-picker overlay renders *behind* a Remote Projects
  modal in the same window. Z-index of overlays is not ordered correctly.
- **#52448** — an "About Zed" modal in window A blocks the file-navigator
  click in window B. Modality is global instead of per-window.
- **#54017** — a macOS floating window does not become key when the app
  is already frontmost. Window-level layering interacts with the platform
  key-window state.

All three reduce to the same model question: **what is the layering
contract for elements drawn *above* the main tree, and what is the
scope of a "modal" interruption?** flui-v2 has the primitives but not
the contract.

## Current behaviour (verified)

[`crates/flui-core/src/window.rs:3337`](../../../crates/flui-core/src/window.rs#L3337):

```rust
pub fn defer_draw(
    &mut self,
    element: AnyElement,
    absolute_offset: Point<Pixels>,
    priority: usize,
    content_mask: Option<ContentMask<Pixels>>,
) {
    // ... prepaint-phase only; element painted later in priority order
}
```

— overlay elements register here during `prepaint`. They are painted
later, ordered by `priority`. Higher priority paints on top. This is
the z-index mechanism, just spelled differently from CSS.

[`window.rs:2772`](../../../crates/flui-core/src/window.rs#L2772):
`deferred_draw_traversal_order` returns the indices sorted by
priority, so paint order matches the contract.

What is **not** documented:

- Whether hit-test traversal uses the **same** priority order
  (highest-priority overlay should win the hit), or whether it
  walks the element tree in document order (which would let an
  underlying element steal the click).
- Whether two overlays at the same priority follow document order
  or insertion order.
- Whether a window-modal overlay blocks input to the *rest of the
  same window*, blocks input to *all windows of the app*, or
  blocks input to *one specific subtree*.

Multi-window: `App` already holds an entity map shared across
windows; `Window` is per-window. Whether a modal in one `Window`
fires app-level effects (like consuming key events from another
`Window`) is decided by how the keymap dispatcher walks the focus
chain — see `key_dispatch.rs`.

## Findings vs upstream

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#52013](https://github.com/zed-industries/zed/issues/52013) | Folder picker renders behind modal. | **likely yes — pending audit**. The fix is "register both via `defer_draw` with matching priorities; the later-priority one wins". If app code mixes deferred and non-deferred for overlays, the latter loses. |
| [zed-industries/zed#52448](https://github.com/zed-industries/zed/issues/52448) | Modal in window A blocks navigation in window B. | **likely yes — by design accident**. We do not have a "modal" primitive; what apps call a "modal" is just a high-priority `defer_draw`. The platform has no concept of inter-window modal scope. |
| [zed-industries/zed#54017](https://github.com/zed-industries/zed/issues/54017) | macOS floating window does not become key. | **out of scope here** — the bug is in macOS `NSWindow` level handling, ADR-007 sibling. Listed for context. |

## Decision (contract)

1. **`defer_draw(priority)` is the only overlay layering mechanism.**
   No "z-index" field on `Style`; no parallel "portal" API. Apps that
   need overlays use `defer_draw` and a priority constant from a
   shared scheme (see decision 2).

2. **The priority space is partitioned by convention:**

   | Range | Use |
   |-------|-----|
   | `0..1000` | In-tree visual layering (drop shadows above siblings, etc.) |
   | `1000..10000` | Tooltips, hover popovers |
   | `10000..100000` | Drop-down menus, autocomplete |
   | `100000..1000000` | Modals / dialogs |
   | `1000000..` | Drag preview, top-most system overlays |

   These are conventions, not enforced numbers. Widget libraries
   define named constants (`Z_TOOLTIP`, `Z_MODAL`) so call sites
   are readable.

3. **Hit-test traversal uses the same priority order as paint.**
   The element drawn on top is the element that wins the click,
   even when document order would have placed a sibling earlier.
   This is the rule users expect from every other UI framework
   and matches CSS `z-index` for `pointer-events`.

4. **Modality is per-window, not per-app.** A modal opened in
   window A blocks input only within window A. Window B continues
   to dispatch input independently. This closes #52448.

5. **There is no "block input below" effect inherent in
   `defer_draw` priority.** A high-priority overlay paints on top
   and wins clicks within its bounds, but clicks *outside* its
   bounds still reach the elements below. Apps that want
   below-blocking modality wrap the modal in a full-window
   transparent backdrop element that consumes pointer events; the
   backdrop is the explicit blocker.

6. **Two overlays at the same priority follow insertion order
   (last wins).** Deterministic, matches `deferred_draws.push`
   semantics in the existing code.

7. **The hit-test walk through deferred draws happens during
   pointer dispatch, not as a separate query.** Implementation
   detail; the contract for callers is "highest-priority overlay
   under the pointer wins".

## Consequences

- Folder pickers, autocompletes, tooltips, modals all share one
  ordering primitive; #52013-style "X renders behind Y" reduces
  to "X has lower priority than Y".
- Apps that want true blocking modals add a transparent backdrop
  — explicit, testable, scoped to one window.
- Multi-window apps work as users expect: a modal does not freeze
  the whole app. #52448 closes.
- macOS floating-key behaviour (#54017) is independent and stays
  with the platform/mac ADR family.

## Out of scope (separate ADRs)

- **`NSWindow` level / key-window state** (#54017). Platform glue.
- **Focus-trap inside a modal** (keyboard-focus cycles inside the
  modal subtree). Touches ADR-010 (local tab-index) — a focus
  scope is the natural primitive; deferred until a widget needs
  it.
- **Inert / non-interactive backdrops on the web target.** Web
  has `inert` attribute; reuse it when implementing the web
  backdrop element.
- **Animation of overlay open/close transitions.** Visual style
  decision; not a layering contract.

## Action items (tracked; no code lands with this ADR)

1. Verify that the hit-test walk consults `defer_draw` priority
   before document order. Audit
   [`gesture/dispatch.rs`](../../../crates/flui-core/src/gesture/dispatch.rs)
   — if it walks the bounds tree in element-document order, add a
   pre-pass over `deferred_draws` sorted by descending priority.
2. Publish a `flui_core::z` module with named priority constants
   (`Z_TOOLTIP`, `Z_DROPDOWN`, `Z_MODAL`, `Z_DRAG_PREVIEW`) so
   widget code stops inventing magic numbers.
3. Add a documented `modal_backdrop()` helper widget that paints
   a transparent full-window quad at priority `Z_MODAL - 1` and
   consumes pointer events — the canonical "modal blocks below"
   pattern.
4. Add a test that opens a modal in window A and asserts a click
   in window B still reaches its target.

## References

### Upstream issues
- [zed-industries/zed#52013](https://github.com/zed-industries/zed/issues/52013) — folder picker behind modal.
- [zed-industries/zed#52448](https://github.com/zed-industries/zed/issues/52448) — modal blocks other windows.
- [zed-industries/zed#54017](https://github.com/zed-industries/zed/issues/54017) — referenced for context only.

### Internal
- [docs/research/adr/ADR-002-hover-active-invalidation.md](ADR-002-hover-active-invalidation.md) — neighbour on the hit-test family.
- [docs/research/adr/ADR-010-local-tab-index.md](ADR-010-local-tab-index.md) — focus-trap-inside-modal sits next to this.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #5 (_Input / focus / hit-testing_), continued.
