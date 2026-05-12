# ADR-012: Custom canvas paint — `canvas(prepaint, paint)` already covers low-level drawing

**Date:** 2026-05-12
**Status:** Draft — documents an existing capability and scopes the gap
that remains. No code changes land with this ADR.
**Scope:** `flui-core/src/elements/canvas.rs`, the scene API consumed by
its `paint` callback (`Window::paint_quad`, `paint_text`, etc.).
**Drivers:** [zed-industries/zed#43273](https://github.com/zed-industries/zed/issues/43273).

## Context

GPUI #43273 asks for a `<canvas>`-style element where the author runs
arbitrary drawing code inside a sized box. The request reads:

> Like a `<canvas>` web element.

There are two reasonable readings of that one-liner:

- **(A)** A low-level paint hook: an element that gives the author a
  `Bounds` and lets them issue draw calls (paint_quad, paint_text,
  paint_path) into the scene during the paint phase. This is the
  "imperative drawing" reading.
- **(B)** A shader hook: an element that runs a custom WGSL / Metal /
  HLSL shader against its bounds and composites the output. This is
  the "GPU effect" reading.

flui-v2 already implements **(A)** — see
[`elements/canvas.rs:9`](../../../crates/flui-core/src/elements/canvas.rs#L9):

```rust
pub fn canvas<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint:    impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T> { /* ... */ }
```

The element accepts a prepaint closure (returns a `T`) and a paint
closure (consumes it). It is styleable via `Styled` and behaves like any
other `Element` in the tree. Reading **(A)** is already a closed gap.

Reading **(B)** is not implemented — there is no API for an element to
supply a shader source that the renderer compiles, caches, and runs
against its bounds. The shader pipeline currently lives entirely behind
`platform/wgpu/wgpu_renderer.rs` and is not exposed to widget code.

This ADR fixes the contract of **(A)** so widget authors know what they
are guaranteed, and declares **(B)** as a separate, future ADR.

## Current behaviour (verified)

[`crates/flui-core/src/elements/canvas.rs:1`](../../../crates/flui-core/src/elements/canvas.rs#L1)
is 89 lines total. The element:

1. Runs `prepaint(bounds, window, app)` during the prepaint phase and
   stores the returned `T` until paint.
2. Runs `paint(bounds, T, window, app)` during the paint phase. The
   author calls `window.paint_quad(...)` / `paint_text(...)` /
   `paint_path(...)` etc. from inside this closure.
3. Styles like any other element via `StyleRefinement` (size, margin,
   background colour, ...).

The paint primitives live on `Window` — see
[`window.rs:3373`](../../../crates/flui-core/src/window.rs#L3373)
and surrounding for the `push_layer`/`paint_*` API.

Reading **(B)** — a shader-source-as-input element — has no entry point.
A grep for `wgpu::ShaderModule`/`ShaderSource` outside
`platform/wgpu/wgpu_renderer.rs` returns no widget-facing matches.

## Findings vs upstream issues

| Issue | Reading | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#43273](https://github.com/zed-industries/zed/issues/43273) | **(A)** Low-level imperative paint inside a sized box. | **no** — `canvas(prepaint, paint)` already covers this. |
| [zed-industries/zed#43273](https://github.com/zed-industries/zed/issues/43273) | **(B)** Custom-shader element. | **yes** — no widget-facing entry point. |

The issue title is ambiguous; both interpretations live behind it.

## Decision (contract)

1. **`canvas(prepaint, paint)` is the public low-level paint surface.**
   The author receives a `Bounds<Pixels>` and a `&mut Window`; the
   paint closure runs during `DrawPhase::Paint`. The output is layered
   into the scene like any other element. `prepaint` is for state
   that depends on layout but must outlive the closure (e.g. shaped
   text laid out by the system).

2. **`canvas` is allowed to call any `Window::paint_*` method.**
   The author may issue arbitrary numbers of paint primitives. There
   is no per-`canvas` cap on draw calls — it is the author's
   responsibility to keep counts reasonable for performance.

3. **`canvas` does NOT compile shaders.** Authors who need a custom
   shader compose pre-built effects (`shadow`, `blur`, ...) via the
   styled API. A future "user-shader" API is out of scope for this
   ADR (see below).

4. **`canvas` participates in invalidation like any other element.**
   Its `prepaint` / `paint` re-runs when the owning view notifies
   (ADR-001 / ADR-002 contract). There is no per-`canvas` "redraw"
   request distinct from `Window::refresh()` / `cx.notify(view)`.

5. **The `T` between `prepaint` and `paint` is private to the element.**
   Authors must not depend on it surviving across frames; each frame
   is a fresh closure pair.

## Consequences

- The existing `canvas` element gets a documented contract; widget
  authors stop guessing what is guaranteed.
- Apps that need imperative drawing (custom charts, mini-maps,
  hand-rolled progress bars, debug overlays) are unblocked today.
- A future "custom shader" element has a clear distinction in its
  ADR — it is **not** an extension of `canvas`; it is a new element
  with a different lifecycle (shader module compiled once, used
  many frames).

## Out of scope (separate ADRs)

- **Custom-shader element** (reading **(B)** above). Needs its own
  ADR, covering: shader module caching, hot-reload during
  development, capability matrix across wgpu/Metal/DirectX, shader
  binding-group layout, lifetime against device-loss (ADR-005).
- **Effect composition** (filters, blur, blend modes between layers).
  The scene API has primitives for some of this; a unified contract
  is its own ADR.
- **GPU-backed text rendering with custom shaders** (e.g. SDF text,
  outline text). Future text-pipeline ADR.
- **Canvas inside scroll views.** The interaction between `canvas`
  and viewport clipping deserves a note, but the rule is the same
  as for any other element — the document is silent for now.

## Action items (tracked; no code lands with this ADR)

1. Add a `// CONTRACT:` comment block at the top of
   [`elements/canvas.rs`](../../../crates/flui-core/src/elements/canvas.rs)
   pointing back to this ADR.
2. Add a documentation example to the rustdoc of `canvas()` showing the
   prepaint/paint split and one realistic use (e.g. a sparkline).
3. Open a separate ADR for the custom-shader element when there is a
   concrete user that needs it — premature now.

## References

### Upstream issues
- [zed-industries/zed#43273](https://github.com/zed-industries/zed/issues/43273) — `<canvas>`-style element request.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — invalidation interplay (decision 4).
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md) — referenced for the future custom-shader ADR's resilience constraints.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #6 (_Drag-and-drop / custom paint_), partial coverage.
