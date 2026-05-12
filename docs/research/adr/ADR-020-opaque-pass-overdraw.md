# ADR-020: Overdraw strategy — opaque-pass + depth reject as future work

**Date:** 2026-05-12
**Status:** Draft — scoping ADR. Defines the design space; defers
the implementation. No code changes land with this ADR.
**Scope:** `flui-core/src/platform/wgpu/wgpu_renderer.rs` pipeline
descriptors, `flui-core/src/scene.rs` quad/path ordering, and the
future opaque-pass classifier in the scene builder.
**Drivers:** [zed-industries/zed#8043](https://github.com/zed-industries/zed/issues/8043).
**Related:** [ADR-001](ADR-001-invalidation-scope.md) explicitly
carved overdraw out of scope. [ADR-006](ADR-006-partial-present-damage-regions.md)
addresses the *present-time* half of "draw less"; this ADR is the
*build-time* half.
**Surfaced by:** [`gpui-unknown-audit.md`](../gpui-unknown-audit.md)
on 2026-05-12.

## Context

GPUI #8043 (filed 2024) shows a RenderDoc "pass overdrawn" capture
of a Zed window: most "ordinary" pixels are painted **5–6 times**
per frame. The scene walks back-to-front; every overlapping
primitive writes the framebuffer; alpha blending is enabled even
for tiles that are fully opaque and would overwrite everything
beneath. This is GPU power and battery burned for no visual
contribution.

The fix on a modern GPU is well-known: split the scene into an
**opaque pass** drawn **front-to-back with depth writes**, plus a
**transparent pass** drawn back-to-front with depth tests (no
writes), under the existing alpha-blend pipeline. WebRender, Skia,
Impeller, and every modern game engine use a variant of this
shape. The cost is one classifier in the scene builder and a
depth-stencil attachment on the render pipeline.

flui-v2's audit on 2026-05-12 confirmed we reproduce the bug
verbatim:
[`wgpu_renderer.rs:674`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L674)
sets `cull_mode: None` and
[`wgpu_renderer.rs:679`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L679)
sets `depth_stencil: None`. No opaque-pass classification exists
in `scene.rs`; every quad goes through the same pipeline.

This ADR is **scoping**, not implementation — overdraw is a known
performance gap, the fix is a non-trivial engine change, and we
must agree on the contract before changing pipelines. The work
itself stays a future task.

## Current behaviour (verified)

References cite the commit this ADR is written against.

### Pipeline descriptor — no depth, no cull

[`wgpu_renderer.rs:670`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L670):

```rust
primitive: wgpu::PrimitiveState {
    topology,
    strip_index_format: None,
    front_face: wgpu::FrontFace::Ccw,
    cull_mode: None,            // ← no back-face culling
    polygon_mode: wgpu::PolygonMode::Fill,
    unclipped_depth: false,
    conservative: false,
},
depth_stencil: None,            // ← no depth attachment
```

Every overlapping primitive participates in the framebuffer write.

### Render pass — no depth attachment

[`wgpu_renderer.rs:1160`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1160),
[`:1198`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1198),
[`:1568`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1568)
all set `depth_stencil_attachment: None`. Even if we wanted to
write depth, the pass would refuse it.

### Scene order

`scene.rs` produces a back-to-front order. The order is correct
for transparent content; it is wasteful for opaque content. The
scene has no concept of "this quad is fully opaque so the layers
beneath it can be skipped".

## Findings vs upstream

| Issue | 👍 | Repro in flui-v2 today |
|---|---|---|
| [zed-industries/zed#8043](https://github.com/zed-industries/zed/issues/8043) | n/a | **yes** — verified by the 2026-05-12 audit. |

## Decision (constraints, not implementation)

1. **Two passes per frame: opaque, then transparent.** The opaque
   pass writes depth and runs front-to-back; the transparent pass
   tests depth but does not write, runs back-to-front under the
   current alpha-blend pipeline. Existing pipelines stay; a new
   "opaque" pipeline is added with `depth_stencil:
   Some(DepthStencilState { format: Depth32Float, depth_write_enabled:
   true, depth_compare: Less, ... })`.

2. **Opacity classification happens in the scene builder, not at
   the GPU.** A quad / path / image is classified as opaque iff
   its solid background has `alpha == 1.0` *and* no `corner_radii`
   produce anti-aliased edges that would partially blend with the
   layer beneath. Anti-aliased corners spill alpha at the edge
   pixels — those pixels must go through the transparent pass.

3. **Text is transparent.** Subpixel and grayscale text glyphs
   carry alpha and run in the transparent pass. ADR-013's
   `TextRasterMode::BiLevel` could eventually qualify glyphs for
   the opaque pass, but the default classification is
   "transparent" until proven otherwise.

4. **Border radii do not block opacity.** A quad with rounded
   corners that has a fully opaque fill still draws its interior
   through the opaque pass with a rectangular bounds smaller than
   the visible quad. The corner pixels go through the transparent
   pass. This is a coverage detail; the contract is that the
   classifier may issue *two draws* for one quad (rect-interior
   opaque + corner-fringe transparent) when this helps.

5. **The classifier never breaks correctness.** When in doubt
   (semi-transparent backdrop, mix-blend mode, unknown filter
   effect), the quad is transparent. A misclassification toward
   "transparent" loses the perf win for that quad; a
   misclassification toward "opaque" produces a visual bug.
   We bias hard toward correctness.

6. **Depth precision is `Depth32Float`.** With many UI quads at
   near-identical Z values, 16-bit depth is not enough. `Depth32Float`
   is universal across our wgpu backends (Vulkan / Metal / DirectX /
   WebGPU all guarantee it).

7. **Backwards compatibility: the change is invisible to widget
   authors.** No public API gains a "this is opaque" flag.
   Authors continue to set background colours with arbitrary alpha;
   the classifier infers.

8. **Composes with ADR-006 (partial present).** Damage-region
   pruning runs *after* the opaque/transparent classification —
   the two halves are orthogonal optimizations; both reduce GPU
   load by different mechanisms.

## Consequences

- A measurable GPU-load drop for any UI with overlapping opaque
  surfaces (a typical editor / IDE / dashboard).
- One extra render pipeline (opaque-pass shader path). Same
  shader source; different pipeline descriptor.
- A small build-time cost in the scene builder for the
  classifier — measured in microseconds per frame.
- Test infrastructure: a visual-regression test must compare
  before / after pixel-for-pixel, because misclassifications
  surface as silent visual bugs.

## Out of scope (separate ADRs)

- **Stencil-based effects** (rounded clipping via stencil instead
  of fragment-shader masking). Independent from the
  opaque/transparent split.
- **Hidden-surface culling at the layer level** (window-occluded
  surfaces). Compositor-level optimization, not engine.
- **Per-quad MSAA toggling**. We already set
  `multisample: { count: sample_count }` uniformly — orthogonal.
- **Reading back depth for picking / hit-test acceleration**. The
  hit-test pipeline (`gesture/`) is bounds-tree based; using the
  depth buffer would be a separate ADR.

## Action items (tracked; no code lands with this ADR)

1. Add a `Pass` enum to scene types (`Opaque | Transparent`) and
   a `Scene::classify_passes()` step that runs after layout +
   paint registration, before the GPU upload.
2. Add a second wgpu pipeline descriptor with depth-stencil
   attached, using the same shader source. Both pipelines share
   the vertex/index buffer layout.
3. Allocate a depth texture matching the surface size; recreate
   on resize and on device-loss (ADR-005). Skip allocation on
   the test platform (`platform/test/window.rs`) where there is
   no real GPU.
4. Add a visual-regression test that renders the
   `creating_components` example, opaque-pass on / off, and
   asserts pixel-for-pixel equality.
5. Measure with a CPU/GPU profiler before and after; record the
   delta in the action-item closure comment. ADR-014's frame
   budget heuristic should re-tune on software fallback after
   this lands.

## References

### Upstream
- [zed-industries/zed#8043](https://github.com/zed-industries/zed/issues/8043) — overdraw 5-6×.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — declared overdraw out of scope; this ADR closes that hand-off.
- [docs/research/adr/ADR-006-partial-present-damage-regions.md](ADR-006-partial-present-damage-regions.md) — orthogonal present-time optimization.
- [docs/research/adr/ADR-013-text-rasterization-strategy.md](ADR-013-text-rasterization-strategy.md) — `BiLevel` mode interaction with the opaque-pass classifier.
- [docs/research/adr/ADR-014-software-rendering-fallback.md](ADR-014-software-rendering-fallback.md) — frame budget interaction.
- [docs/research/gpui-unknown-audit.md](../gpui-unknown-audit.md) — surfaced this gap.
