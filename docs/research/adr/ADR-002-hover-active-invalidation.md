# ADR-002: Hover / active / pressed state — invalidation must be view-scoped, not window-scoped

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR; concrete
migrations are tracked as action items.
**Scope:** `flui-core/src/elements/div.rs`. Touches the patterns used by all
interactive elements built on `Div`.
**Builds on:** [ADR-001 — Invalidation scope](ADR-001-invalidation-scope.md).
**Drivers:** [zed-industries/zed#24405](https://github.com/zed-industries/zed/issues/24405),
[zed-industries/zed#38350](https://github.com/zed-industries/zed/issues/38350).

## Context

ADR-001 audited every `.refresh()` call in `crates/flui-core/src` and found 13
of them in `elements/div.rs`, all in discrete event handlers — hover, mouse
down/up, drag, tooltip. None of them violated ADR-001 (`refresh()` is the
explicitly-permitted full-window intent), but every one of them paid full
`draw_roots` cost for a visual change that affects only the element that just
got hovered, pressed, or showed a tooltip.

This is the same pattern behind the upstream symptoms in #24405 ("hover
tooltips bleed through panels") and #38350 ("hover reacts while window is in
background"): the engine repaints the whole window on every hover crossing, so
every overlapping listener participates. A scoped invalidation does not
*solve* those bugs by itself (they also need correct hit-test layering and
foreground-state filtering), but it cuts the contributing factor that makes
them so visible.

The mixed pattern is already there: the same file already uses
`cx.notify(current_view)` in ~6 places. The contract is just not written
down, so new code copy-pastes whichever neighbour was closest.

## Current behaviour (verified)

References below cite `crates/flui-core/src/elements/div.rs` at the commit this
ADR is written against.

### Already correct (use `cx.notify(current_view)`)

| Line | Pattern | Notes |
|------|---------|-------|
| [2474](../../../crates/flui-core/src/elements/div.rs#L2474) | secondary-key hover bounds change | Captures `current_view` from the surrounding scope. |
| [2624](../../../crates/flui-core/src/elements/div.rs#L2624) | mouse-move hover state | Already scoped. |
| [2647](../../../crates/flui-core/src/elements/div.rs#L2647) | hover-group state | Already scoped. |
| [2993](../../../crates/flui-core/src/elements/div.rs#L2993) | hover-style toggle | Already scoped. |
| [3045](../../../crates/flui-core/src/elements/div.rs#L3045) | group-hover-style toggle | Already scoped. |

### Candidates to migrate to `cx.notify(current_view)`

These call `window.refresh()` today but mutate state that lives in **one**
element (`pending_mouse_down`, `clicked_state`, modifier-driven highlight,
secondary-key UI). Repainting the whole window is over-invalidation.

| Line | Trigger | State mutated | Why scoped is correct |
|------|---------|---------------|-----------------------|
| [2461](../../../crates/flui-core/src/elements/div.rs#L2461) | modifiers-changed (secondary key) over inspector text | Hover-only inspector overlay | Affects only this element's text overlay |
| [2719](../../../crates/flui-core/src/elements/div.rs#L2719) | mouse-down on the element | `pending_mouse_down` (per-element `Rc<RefCell<…>>`) | Pressed visual is on this element |
| [2800](../../../crates/flui-core/src/elements/div.rs#L2800) | mouse-up capture, pending click commits | `pending_mouse_down` cleared | Same as above |
| [2808](../../../crates/flui-core/src/elements/div.rs#L2808) | mouse-up capture, pending click cancelled | `pending_mouse_down` cleared | Same as above |
| [2920](../../../crates/flui-core/src/elements/div.rs#L2920) | mouse-up that clears `clicked_state` | `clicked_state` per element | Active visual is on this element |
| [2941](../../../crates/flui-core/src/elements/div.rs#L2941) | mouse-down that sets `clicked_state` | `clicked_state` per element | Active visual is on this element |

Replacement pattern: capture `let current_view = window.current_view();`
immediately before the relevant `on_mouse_event` / `on_key_event` block, move
it into the closure, and call `cx.notify(current_view)` from inside.

### Must stay `window.refresh()`

These mutate **window-global** state and so genuinely require a full-window
repaint. They are not bugs.

| Line | Why window-global |
|------|---|
| [2684](../../../crates/flui-core/src/elements/div.rs#L2684) | Drop fires `cx.active_drag.take()` — `active_drag` is `App`-level; any view in any window that draws the drag preview must repaint. |
| [2750](../../../crates/flui-core/src/elements/div.rs#L2750) | Drag start sets `cx.active_drag = Some(...)` — same reason. |
| [3254](../../../crates/flui-core/src/elements/div.rs#L3254), [3255](../../../crates/flui-core/src/elements/div.rs#L3255), [3271](../../../crates/flui-core/src/elements/div.rs#L3271), [3421](../../../crates/flui-core/src/elements/div.rs#L3421), [3490](../../../crates/flui-core/src/elements/div.rs#L3490) | Tooltip helpers (`clear_active_tooltip`, `clear_active_tooltip_if_not_hoverable`, `handle_tooltip_mouse_move`) take only `&mut Window`. Tooltip is a window-level overlay drawn on top of all views; the helpers do not know which view requested it. Migrating these would require threading a view id through the helper API, which is a bigger refactor and is **out of scope** for this ADR. |

## Findings vs upstream issues

| Issue | Symptom | Effect of the proposed migration |
|-------|---------|----------------------------------|
| [zed-industries/zed#24405](https://github.com/zed-industries/zed/issues/24405) | Hover tooltips of underlying tabs bleed through a panel that overlaps them | The proposed migration does not fix the hit-test layering bug. It does cut the per-hover full-window repaint, which is what makes the bleed visible on every mouse-move. The real fix is hit-test ordering (separate future ADR). |
| [zed-industries/zed#38350](https://github.com/zed-industries/zed/issues/38350) | Hover events fire while the window is in the background | Same: scoped invalidation does not stop the events, it stops the over-paint amplifying them. Background-state filtering is a separate ADR. |

The honest claim: ADR-002 narrows the blast radius of every hover/click event.
It does not by itself resolve the upstream issues. That requires the
follow-up ADRs called out below.

## Decision (contract)

1. **Default rule: per-element state changes use `cx.notify(current_view)`.**
   Any code in `elements/*` that mutates state owned by a single element
   (pressed visual, hovered visual, focus highlight, modifier-driven overlay)
   **must** invalidate via `cx.notify(view)` and not `window.refresh()`.

2. **`window.refresh()` is reserved for window-global state.** The two
   currently-justified cases are `cx.active_drag` transitions and tooltip
   helpers whose signature does not yet carry a view id.

3. **`current_view` is captured at registration time, not at fire time.**
   The pattern is:
   ```rust
   let current_view = window.current_view();
   window.on_mouse_event({
       // …
       move |event, phase, window, cx| {
           // …
           cx.notify(current_view);
       }
   });
   ```
   Reading `window.current_view()` inside the closure would resolve to
   whatever view is current at *event dispatch time*, which is not the same
   thing.

4. **Mixed files are explicitly allowed during migration.** A single PR is
   not required to migrate every site; the rule applies to *new* code
   immediately and to existing sites as they are touched. The candidate list
   above is the authoritative work backlog.

## Consequences

- Every hover crossing, pressed visual, and modifier-driven overlay invalidates
  only the view that owns it. ADR-001's `present()` is still full-scene, so
  the platform-layer cost does not change yet — that is ADR-003's job. But
  `draw_roots`/layout cost drops to the view subtree's slice of work.
- The two `cx.active_drag` sites remain full-window by design.
- Tooltip helpers stay window-scoped until a separate refactor threads
  per-view identity through them.
- The contract is binding on future widget libraries built on `Div`.

## Out of scope (separate ADRs)

- **Hit-test layering / panels eating hover** (GPUI #24405). The scoping
  here cuts the symptom intensity but not the cause.
- **Pointer events while window is in background** (GPUI #38350). Needs an
  event-filtering ADR around `WindowFocusEvent` / background-state.
- **Tooltip helper API refactor** so tooltip mutations can carry a view id.
  Independent of ADR-002.
- **Partial present** (ADR-003 / GPUI #15166) — orthogonal to invalidation
  scope.

## Action items (tracked; no code lands with this ADR)

1. Migrate the six candidate sites above. Suggested grouping: one commit per
   logical block (modifiers handler; click `pending_mouse_down`; active
   state). Each commit should keep `cargo test -p flui-core --lib` green and
   should not introduce new clippy warnings.
2. Add a clippy-style lint or a `// LINT:` comment near `window.refresh()` in
   `elements/*` so future code does not silently regress the contract. (A
   real clippy lint is preferred but is a tooling decision deferred to the
   migration PR.)
3. Open a follow-up ADR for the tooltip helper refactor when somebody touches
   that area for an unrelated reason.

## References

### Upstream issues
- [zed-industries/zed#24405](https://github.com/zed-industries/zed/issues/24405) — hover bleeds through panels.
- [zed-industries/zed#38350](https://github.com/zed-industries/zed/issues/38350) — hover while window is in background.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — the outer contract this ADR specialises.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #5 (_Input / focus / hit-testing_) of which this ADR is a partial coverage.
- [docs/research/gpui-issues.md](../gpui-issues.md) — full GPUI snapshot.
