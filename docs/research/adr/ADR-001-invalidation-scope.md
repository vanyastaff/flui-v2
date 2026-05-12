# ADR-001: Invalidation scope — what `refresh`, `request_animation_frame`, and `notify` actually re-do

**Date:** 2026-05-12
**Status:** Draft — documents current behaviour and fixes the contract.
**Scope:** `flui-core` — `Window`, `WindowInvalidator`, draw loop.
**Related K-specs:** K02 (element identity), K03 (render/build separation).
**Issues that motivate this:** see _References_ below.

## Context

Both GPUI and Flutter accumulated a long tail of issues where users observe that a
visually local change (animation tick, hover state, one-cell text edit) triggers
work that is far larger than the change itself: CPU stays high while an animated
spinner is on screen, GPU is loaded heavily during steady text typing, damage
regions are not communicated to the compositor and so the display server re-blits
the whole window every frame.

We inherited the same model — entity-based dirty set + scene-wide present — and
should document the contract before we add features on top of it. Without an
explicit contract, future modules (animation, scrolling, listeners) silently
encode assumptions that turn into the same long tail of issues.

## Current behaviour (verified)

References below cite `crates/flui-core/src/window.rs` at the commit this ADR is
written against.

| Surface | Effect | File:line |
|---|---|---|
| `WindowInvalidator { dirty: bool, dirty_views: FxHashSet<EntityId>, draw_phase }` | Single per-window dirty flag plus a set of entities considered dirty. | [window.rs:112](../../../crates/flui-core/src/window.rs#L112) |
| `WindowInvalidator::invalidate_view(entity, cx)` | Inserts entity into `dirty_views`, sets `dirty = true` outside paint, pushes a `Notify` effect. | [window.rs:131](../../../crates/flui-core/src/window.rs#L131) |
| `Window::refresh()` | Outside paint, sets `dirty = true`. Does **not** touch `dirty_views` — the entire window is considered dirty. | [window.rs:1621](../../../crates/flui-core/src/window.rs#L1621) |
| `Window::on_next_frame(callback)` | Appends to `next_frame_callbacks`. The list is drained after the frame is composited. | [window.rs:1911](../../../crates/flui-core/src/window.rs#L1911) |
| `Window::request_animation_frame()` | Calls `on_next_frame` that calls `cx.notify(current_view())`. So a continuous animation re-notifies the current view every frame. | [window.rs:1921](../../../crates/flui-core/src/window.rs#L1921) |
| `Window::mark_view_dirty(view_id)` | Walks the dispatch tree ancestor path and inserts each ancestor into `dirty_views` until an already-dirty ancestor is found. | [window.rs:1560](../../../crates/flui-core/src/window.rs#L1560) |
| `Window::invalidate_entities()` | Drains `WindowInvalidator::dirty_views` and feeds each entity through `mark_view_dirty`. | [window.rs:2481](../../../crates/flui-core/src/window.rs#L2481) |
| `Window::draw(cx)` | Calls `invalidate_entities()`, then **unconditionally** runs `draw_roots(cx)` (full layout + paint of the root). Clears `dirty_views`. Sets `needs_present = true`. | [window.rs:2379](../../../crates/flui-core/src/window.rs#L2379) |
| `Window::present()` | Calls `platform_window.draw(&self.rendered_frame.scene)` — the whole scene is handed to the platform layer. No damage / present regions are passed. | [window.rs:2490](../../../crates/flui-core/src/window.rs#L2490) |

### What this means in practice

1. **Rebuild scope is per-view-path.** When a view notifies, every ancestor view
   in its dispatch path is marked dirty. The first already-dirty ancestor short-
   circuits the walk — so the path is at worst from notifier to root.

2. **Layout + paint scope is window-wide.** `draw()` always calls `draw_roots`
   on the single root element with the full viewport. The `dirty_views` set is
   currently used by `inherited_registry` / `mark_view_dirty` bookkeeping but
   not as a gate that lets `draw_roots` skip a subtree.

3. **Present scope is full-scene.** `platform_window.draw(scene)` redraws the
   whole window surface. No damage region, no partial present.

4. **Frame callbacks are stored on `Window`.** `on_next_frame` is a global
   per-window queue; there is no checked precondition forbidding registration
   from inside paint. (See _Open question 1_.)

## Findings vs upstream issues

| Issue | Upstream symptom | Repro in flui-v2 today |
|---|---|---|
| [GPUI #50392](https://github.com/zed-industries/zed/issues/50392) — animated spinner triggers full layout recalculation | Steady 30-40 % CPU while a small progress indicator is visible. | **partial**. We avoid the worst case (`refresh()` would do it) because animation uses `notify(view)`, but `draw()` still re-runs root layout+paint every animation tick. |
| [GPUI #15166](https://github.com/zed-industries/zed/issues/15166) — missing damage / present regions cause whole-window compositor repaint | Display server re-blits the whole surface on every frame. | **yes**. `present()` is full-scene with no damage hint. |
| [GPUI #56294](https://github.com/zed-industries/zed/issues/56294) — registering `on_next_frame` during `paint` shifts content by 1 px on resize | Frame callback registered from `paint` interferes with the next configure event. | **possibly**. We allow `on_next_frame` from any phase; no `debug_assert` guards it. Symptom-specific to Wayland configure, but contract is unspecified. |
| [GPUI #8043](https://github.com/zed-industries/zed/issues/8043) — overdraw 5-6× per pixel | RenderDoc shows the same pixel painted by 5-6 draws. | **out of scope here** — overdraw is a scene-painter property, not an invalidation property. Tracked separately (see _Out of scope_). |
| [Flutter #14288](https://github.com/flutter/flutter/issues/14288) — antialiasing artefacts when same-colour edges abut | Visible seams when re-rendering | **out of scope here** — rasterizer property. |

## Decision (contract)

The invalidation contract for `flui-core` as of this ADR is:

1. **Two scopes exist by name, not yet by effect.** `refresh()` is a full-window
   intent. `invalidate_view(entity)` / `request_animation_frame()` are
   per-view intents. Today both end up running `draw_roots` on the entire root.
   The names are kept because callers want to express intent; the gap between
   intent and effect is acknowledged here, not papered over.

2. **`refresh()` is the safe default for non-animation state changes.** For an
   animation that runs every frame, callers **must** use
   `request_animation_frame()` (per-view) and never `refresh()` (window). This
   is binding on internal modules (`animation::*`, scroll, gesture).

3. **`invalidate_view` walks ancestors.** Callers may insert any descendant
   entity; the path-to-root walk is performed by the engine. Callers must not
   insert ancestors manually as a "shortcut" — that breaks the
   already-dirty short-circuit.

4. **`on_next_frame` runs after the frame is composited.** Its callback observes
   the next frame, not the current one. Registering during `paint` is currently
   permitted but **discouraged**; we will add a debug-only guard in a follow-up
   (see _Action items_) so that pre-K-series Wayland-style 1 px shifts cannot
   silently appear in flui-v2 apps.

5. **`present()` is currently full-scene by design.** This ADR does not promise
   partial present. It promises that when partial present is introduced (likely
   a follow-up driven by GPUI #15166 + #50392), the entrypoint will not change
   for callers — only the implementation behind `platform_window.draw`.

## Consequences

- Modules that drive continuous redraws (animation tickers, marquee, blinking
  caret) **must** use `request_animation_frame()` and **must not** call
  `refresh()` in their per-frame path. We will grep the workspace and fix any
  violation as part of the follow-up (_Action items_).
- We accept GPU/CPU cost similar to current GPUI behaviour until partial
  layout + partial present land. That is honest, not aspirational.
- Future ADRs that propose `mark_needs_paint(rect)`, damage region tracking,
  or layout-subtree caching build on the names defined here; they do not
  redefine them.

## Out of scope (separate ADRs)

- **Overdraw / opaque-pass ordering** (GPUI #8043). Property of the scene
  painter, not the invalidator.
- **Partial present / damage regions** (GPUI #15166). Needs a platform-layer
  ADR around `platform_window.draw` shape change.
- **Layout subtree caching** (GPUI #50392 deep fix). Needs an element-tier ADR
  about layout reuse; depends on K02 identity stability.
- **Frame callback safety in `paint`** (GPUI #56294). Will get its own short
  ADR once the debug guard is added and we know the right phase mask.

## Audit at the time of this ADR

A workspace grep for `.refresh()` was performed against
`crates/flui-core/src` to validate decision points 2 and 3.

- **`crates/flui-core/src/animation/**`: zero hits.** Animation modules use
  `request_animation_frame()` and listener notifications; the per-tick
  contract is already respected.
- **`crates/flui-core/src/elements/div.rs`: 13 hits**, all in discrete event
  handlers (hover-enter/leave, mouse-down/up, drag listeners, tooltip
  show/hide). Discrete events do not violate the ADR contract, but each
  hover crossing triggers a full-window `draw_roots` pass. This is
  consistent with GPUI's behaviour and contributes to the symptom in
  [zed-industries/zed#24405](https://github.com/zed-industries/zed/issues/24405)
  and [zed-industries/zed#38350](https://github.com/zed-industries/zed/issues/38350).
  Migrating these call sites to `invalidate_view(self_view)` is **out of scope
  for this ADR** and is queued as ADR-002 ("hover / active state
  invalidation").
- **`crates/flui-core/src/elements/uniform_list.rs`: 2 hits** for `scroll_to_item`.
  Acceptable — scroll target change is a structural change, not a per-frame loop.
- **`crates/flui-core/src/elements/text.rs`: 2 hits** for selection mouse-down.
  Same category as div hover.
- **`crates/flui-core/src/window.rs`: 8 hits** in `focus()`, `blur()`,
  `bounds_changed`, `hovered`, drag end, root replacement, modality change.
  All discrete; acceptable.
- **`crates/flui-core/src/app.rs`: 2 hits** in drag start / cursor-style
  update during drag. Discrete; acceptable.

Result: decision point 2 ("animation uses `request_animation_frame`, not
`refresh`") is held by the current code. Decision point 4
(`on_next_frame` must not be called from `paint`) is now enforced by a
debug-only assert added with this ADR — see action item 2 below.

## Action items (tracked, not blocking this ADR)

1. ~~Audit `crates/flui-core/src/animation/**` and verify no path calls
   `refresh()` per tick.~~ **Done in this ADR** — clean.
2. ~~Add a `#[cfg(debug_assertions)]` guard inside `on_next_frame` that warns
   when called during `DrawPhase::Paint`.~~ **Done.** `WindowInvalidator` now
   has `debug_assert_not_paint`, called from the entry of `Window::on_next_frame`.
   `cargo test -p flui-core --lib` (410 tests) and `cargo clippy -p flui-core
   --all-targets -- -D warnings` both pass — no existing code paths violate the
   new contract.
3. ~~Open ADR-002 — "hover / active state invalidation".~~ **Done.** See
   [ADR-002 — Hover / active / pressed state invalidation](ADR-002-hover-active-invalidation.md).
   The contract is fixed; the actual `div.rs` migration is its own action
   item, not landed here.
4. ~~Open a separate spec file for partial present once K-series finishes.~~
   **Done.** See [ADR-006 — Partial present / damage regions](ADR-006-partial-present-damage-regions.md).
   That ADR is a scoping document, not an implementation; the work itself
   remains queued.

## References

### Upstream issues
- [zed-industries/zed#50392](https://github.com/zed-industries/zed/issues/50392) — animation triggers full layout recalculation.
- [zed-industries/zed#15166](https://github.com/zed-industries/zed/issues/15166) — missing damage / present regions.
- [zed-industries/zed#56294](https://github.com/zed-industries/zed/issues/56294) — `on_next_frame` registered in `paint` shifts content by 1 px.
- [zed-industries/zed#8043](https://github.com/zed-industries/zed/issues/8043) — referenced for scope only; not addressed here.
- [flutter/flutter#14288](https://github.com/flutter/flutter/issues/14288) — referenced for scope only; not addressed here.

### Internal context
- [docs/research/gpui-issues.md](../gpui-issues.md) — full GPUI snapshot.
- [docs/research/flutter-issues.md](../flutter-issues.md) — Flutter snapshot.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — this ADR is part of theme #1 (_Rendering / GPU pipeline_), narrowed to invalidation only.
- [docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md](../../superpowers/specs/2026-05-11-K02-element-identity-key-design.md) — identity substrate this contract sits on top of.
- [docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md](../../superpowers/specs/2026-05-11-K03-render-build-separation-design.md) — build/render seam; this ADR sits underneath it.
