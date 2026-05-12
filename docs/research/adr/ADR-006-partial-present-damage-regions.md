# ADR-006: Partial present — design space for damage-region API

**Date:** 2026-05-12
**Status:** Draft — scoping ADR. Defines the design problem and the
constraints; defers the implementation choice. No code changes land
with this ADR.
**Scope:** `flui-core/src/platform.rs` (the `PlatformWindow` trait),
`flui-core/src/scene.rs`, every backend that implements `PlatformWindow::draw`.
**Drivers:** [zed-industries/zed#15166](https://github.com/zed-industries/zed/issues/15166).
**Builds on:** [ADR-001 — Invalidation scope](ADR-001-invalidation-scope.md),
item 4 of which deferred this work.

## Context

ADR-001 nailed down what `refresh()` / `invalidate_view()` /
`request_animation_frame()` mean for **building** a frame. It deliberately
left **presenting** the frame as full-scene: `Window::present()` calls
`platform_window.draw(&self.rendered_frame.scene)` with the entire scene,
and the platform compositor sees a full-window damage by default.

Upstream GPUI #15166 catalogues exactly this: every modern compositor
expects a damage / present region per submitted frame, and without one the
display server re-blits the whole window even when only a caret blinks.
The Vulkan, EGL and Metal hooks are well-known
(`VK_KHR_incremental_present`, `EXT_swap_buffers_with_damage`,
`CALayer::setNeedsDisplayInRect`). They are *not* hard to call individually
— what is hard is choosing a single API shape on top of all of them.

This ADR is a **scoping** document, not a contract for code. The goal is to
make the next implementer's life cheaper: state the constraints, name the
shape, list the alternatives we have rejected and the ones we have not yet.

## Current behaviour (verified)

References below cite the commit this ADR is written against.

[`crates/flui-core/src/platform.rs:668`](../../../crates/flui-core/src/platform.rs#L668):

```rust
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    // ...
    fn draw(&self, scene: &Scene);
    // ...
}
```

Every backend implements this:

- [`platform/wgpu/wgpu_renderer.rs:1037`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1037) — `Renderer::draw(scene)`.
- [`platform/mac/metal_renderer.rs:440`](../../../crates/flui-core/src/platform/mac/metal_renderer.rs#L440) — Metal path.
- [`platform/windows/window.rs:918`](../../../crates/flui-core/src/platform/windows/window.rs#L918) — DirectX path.
- [`platform/linux/{x11,wayland}/window.rs`](../../../crates/flui-core/src/platform/linux) — wgpu via the X11/Wayland surface.
- [`platform/web/window.rs:666`](../../../crates/flui-core/src/platform/web/window.rs#L666) — WebGL/WebGPU.
- [`platform/test/window.rs:288`](../../../crates/flui-core/src/platform/test/window.rs#L288) — no-op for the test platform.

None of these consult a damage region. The `present()` call at
[`window.rs:2512`](../../../crates/flui-core/src/window.rs#L2512) hands the
entire `Scene` to the platform.

The invalidation side has the information that would feed a damage region:
`WindowInvalidator::dirty_views: FxHashSet<EntityId>`
([window.rs:112](../../../crates/flui-core/src/window.rs#L112)) and the
ancestor-marking pass at
[`window.rs:1573`](../../../crates/flui-core/src/window.rs#L1573). What is
*missing* is the bridge from "this view is dirty" to "this rectangle on the
surface must be repainted".

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#15166](https://github.com/zed-industries/zed/issues/15166) | Compositor repaints the whole window on every present because no damage region is supplied. | **yes**. Already documented in [ADR-001](ADR-001-invalidation-scope.md). |

## Design problem

Three things must be decided together; deciding one before the others
locks in the wrong tradeoffs.

**(A) Where does damage live in the model?** The candidates are:

- A field on `Scene` (or `rendered_frame.scene`): a list of dirty
  rectangles in device coordinates. Tight coupling between scene
  construction and the platform API.
- A separate argument to `PlatformWindow::draw`:
  `fn draw(&self, scene: &Scene, damage: Option<&Damage>)`. Loose coupling;
  cheap to feature-flag; symmetric with the current "full surface" default
  (`damage = None`).
- A method on `Window` that the platform queries:
  `window.consume_damage() -> Damage`. Avoids new trait params; harder to
  reason about ordering.

**(B) How is damage produced?** Candidates:

- From the bounds-tree update during `prepaint`. Bounds of every dirty
  view union into the damage region. This is the most truthful answer
  and lines up with the existing `bounds_tree.rs`.
- From a coarser approximation: union the bounding boxes of `dirty_views`
  in the dispatch tree. Cheaper but over-damages.
- From the layout engine via a "what changed since last frame" diff.
  Cleanest model; needs persistent layout state we do not yet keep.

**(C) What does the platform do with `None`?** Default behaviour is
"full present" — backward-compatible. This is the path we want;
`None` is the explicit "I don't have damage info" case for the
recovery / first-paint / window-resize frame.

The three choices interact: choosing (A) "field on Scene" forces (B)
"computed during scene construction", which is currently *during*
`draw_roots`, not after `prepaint`. Choosing (A) "parameter to draw"
makes (B) flexible.

## Decision (constraints, not implementation)

This ADR fixes the **constraints** any implementation must satisfy. The
concrete shape can change as long as these hold.

1. **`PlatformWindow::draw` may grow a damage parameter; `draw(scene)`
   stays valid as a default.** Backward-compatible introduction is
   mandatory; existing call sites must compile without change.

2. **Damage is opt-in per backend.** A backend that does not implement
   damage falls back to full present. There is no platform-level
   requirement to support damage; the contract is "if you supply it, we
   use it; otherwise we present full".

3. **Damage is expressed in device pixels relative to the surface
   origin.** Not view coordinates; not logical pixels; not normalized.
   This matches what every platform API actually wants.

4. **Damage is an additive hint, never an exclusion.** A backend is
   allowed to repaint more than the damage region (e.g. on a hardware
   path that always does full-surface anyway). It must never repaint
   *less* than the damage region.

5. **A frame with damage == empty is a no-present.** If nothing is
   damaged, the platform must not present. This is the steady-state
   for an idle window.

6. **First-paint, window resize, and post-`recover()` frames present
   with no damage (i.e. full).** These are the explicit cases where the
   compositor must re-blit everything.

7. **The damage producer is `Window`, not the caller.** Callers continue
   to call `refresh()` / `invalidate_view()` / `request_animation_frame()`;
   they do not pass damage rectangles. ADR-001 keeps its contract.

## Consequences

- The cheapest path to "we honour damage" becomes a single new optional
  parameter on `draw`, fed by a `Window::collect_damage_rect()` helper
  that reads the bounds tree at `prepaint` end. The full implementation
  fits inside one feature-flagged code path; the rest of the codebase
  is unaware.
- The wgpu backend is the natural first implementer because it talks to
  the platform compositor via `wgpu::Surface::get_current_texture`; the
  damage hint is a runtime hint passed to whatever the wgpu surface
  config supports. Metal/DirectX/Wayland-direct paths follow the same
  shape.
- ADR-001's "full-scene `present()` is by design" is now scoped: it
  remains the default. Partial present is a layered improvement, not a
  contract change.

## Out of scope (separate ADRs)

- **Overdraw** (GPUI #8043). Independent. Overdraw is a scene-painter
  property; damage is a present-time property. They are often confused
  but solve different problems.
- **Subpixel scroll / smooth scroll**. Often discussed alongside damage
  because both touch the surface presentation pipeline; orthogonal in
  practice.
- **Variable refresh rate / GPU vsync coordination**. Distinct ADR.
- **The damage producer's accuracy**. This ADR is satisfied by an
  over-damaging producer (covers what changed *and* more); a tight
  producer is a follow-up optimization, not a contract.

## Action items (tracked; no code lands with this ADR)

1. Sketch the helper signature in a one-page proposal:
   `Window::collect_damage_rect(&self) -> Option<Bounds<DevicePixels>>`.
   Decide between bounds-tree union and dirty-view ancestor union as the
   producer.
2. Land the trait change as a non-breaking default:
   `fn draw_with_damage(&self, scene: &Scene, damage: Option<&Damage>) { self.draw(scene) }`,
   keeping the existing `draw` as the no-damage entry.
3. Implement the wgpu path first; the rest stay on the default until they
   are touched for another reason.
4. Add a `Damage::FULL` and `Damage::EMPTY` sentinel so the rules in
   decision points 5 and 6 are expressible as types, not implicit
   conventions.

## References

### Upstream issues
- [zed-industries/zed#15166](https://github.com/zed-industries/zed/issues/15166) — missing damage / present regions cause whole-window compositor repaint.
- [zed-industries/zed#50392](https://github.com/zed-industries/zed/issues/50392) — referenced via ADR-001 (the layout/invalidation cousin).
- [zed-industries/zed#8043](https://github.com/zed-industries/zed/issues/8043) — referenced for disambiguation (overdraw is *not* damage).

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — closes its action item 4.
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md) — the post-`recover()` "full present" rule in decision point 6 references that ADR's contract.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #1 (_Rendering / GPU pipeline_), partial coverage by this ADR (present-time only).
