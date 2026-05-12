# ADR-015: ObjectFit::Cover with rounded corners — clip outside the image, not inside

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/elements/img.rs`, `flui-core/src/style.rs`
(`ObjectFit`), `flui-core/src/window.rs` (`push_layer` /
`paint_image` / rounded-clip surface).
**Drivers:** [zed-industries/zed#44339](https://github.com/zed-industries/zed/issues/44339).

## Context

GPUI #44339 reports that an `img` element with `ObjectFit::Cover` plus
rounded corners renders incorrectly: the **image itself** is rounded,
not the **visible area**. When `ObjectFit::Cover` is in effect, the
image extends past the container; the user expected rounded corners on
the container edge, not on a sub-region of the image that may not even
overlap the container's corners.

The root cause is a layering decision: should rounded corners be a
property of the *image draw* (current behaviour) or of the *clip layer
that surrounds the image* (the correct behaviour)?

flui-v2 ports the same logic verbatim. The img element computes a
`new_bounds` from `ObjectFit::Cover` that can exceed the container; it
then applies `corner_radii` to **`new_bounds`**, not to the container
bounds.

## Current behaviour (verified)

[`crates/flui-core/src/elements/img.rs:490`](../../../crates/flui-core/src/elements/img.rs#L490):

```rust
let new_bounds = self
    .style
    .object_fit
    .get_bounds(bounds, data.size(layout_state.frame_index));
let corner_radii = style
    .corner_radii
    .to_pixels(window.rem_size())
    .clamp_radii_for_quad_size(new_bounds.size);
window
    .paint_image(
        new_bounds,
        corner_radii,
        data,
        layout_state.frame_index,
        self.style.grayscale,
    )
    .log_err();
```

Note:

1. `new_bounds` is the image rectangle after `ObjectFit::Cover`
   stretches it — possibly *larger than* the container `bounds`.
2. `corner_radii` is computed against `new_bounds.size`, not against
   the container.
3. `paint_image(new_bounds, corner_radii, ...)` rounds the corners
   of the image itself.

Result: the image's *own* corners are rounded, but the parts of the
image that overflow the container are still drawn full-square. The
container's true corners stay sharp.

For `ObjectFit::Contain` / `ObjectFit::None` this accident produces the
right output because the image fits inside the container. For
`ObjectFit::Cover` / `ObjectFit::Fill` it is visibly wrong.

[`crates/flui-core/src/window.rs:3379`](../../../crates/flui-core/src/window.rs#L3379):

```rust
.push_layer(clipped_bounds.scale(scale_factor));
```

`push_layer` accepts only rectangular clip bounds — no corner radii.
So even if `img` switched to a clip layer, the layer would still be
square.

## Findings vs upstream

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#44339](https://github.com/zed-industries/zed/issues/44339) | `ObjectFit::Cover` + rounded corners → image's own edges are rounded; overflow stays visible. | **yes**. `img.rs:490` ports the same composition order; `Window::push_layer` has no rounded-corner variant. |

## Decision (contract)

1. **Rounded corners on a styled element are a clip on the bounding
   box, not a property of any single draw.** Adding `rounded_2xl()`
   to an `img` rounds the container, not the image.

2. **`Window::push_layer` grows a rounded-corner variant** —
   `Window::push_rounded_clip(bounds, corner_radii)`. Existing
   rectangular `push_layer` stays as a fast path.

3. **`img` paints in two steps** when corner radii are non-zero:

   ```text
   window.push_rounded_clip(container_bounds, corner_radii);
   window.paint_image(new_bounds, Corners::all(0.0), data, ...);
   window.pop_layer();
   ```

   The image draw no longer carries its own corner radii in this
   path. The fast path (no rounded corners) still calls
   `paint_image(bounds, …)` directly without a layer.

4. **`ObjectFit::Cover` / `ObjectFit::Fill` are the only modes where
   the difference is observable**, but the rule applies uniformly to
   keep the contract simple. `ObjectFit::Contain` and
   `ObjectFit::None` will produce identical output before and after.

5. **The rule generalises beyond `img`.** Any element whose paint
   exceeds its layout bounds (a future `video`, `webview`,
   user-provided `canvas` extending past its style box) clips
   against the container's rounded shape via the same mechanism.

6. **`corner_radii` semantics on `paint_image` itself remain
   available** as a low-level escape hatch for callers that want to
   round the image draw directly. The img element no longer uses
   that variant when a container clip is the intent.

## Consequences

- The visible result matches every modern UI framework's behaviour
  for `background-image`, `<img>`, and CSS `overflow: hidden` plus
  `border-radius`.
- A small extra cost is paid for `ObjectFit::Cover` + rounded
  corners (one push/pop layer) — measured in microseconds per
  paint.
- `Window::push_layer` keeps its existing shape; `push_rounded_clip`
  is additive.
- Future video / webview / GPU-effect elements inherit the
  contract for free.

## Out of scope (separate ADRs)

- **Per-corner border decoration** (different colours on each side,
  inner shadows). Independent.
- **CSS `background-position` semantics** when an image is used as
  a background instead of an `<img>`. Same root mechanism, but the
  style-cascade integration is a separate ADR.
- **Anti-aliasing of the rounded clip edge** (which is the actual
  rasterization detail). Touched by ADR-013's text rasterization
  family but lives in the shader.

## Action items (tracked; no code lands with this ADR)

1. Add `Window::push_rounded_clip(bounds, corner_radii)` and the
   corresponding `pop_layer` semantics. Verify the layer stack
   already tracks the clip shape; if not, extend it.
2. Update [`img.rs:490`](../../../crates/flui-core/src/elements/img.rs#L490)
   to use the new path when corner radii are non-zero; keep the
   single-call fast path when they are zero.
3. Add a visual-regression test (or a `flui_visual_test` snapshot)
   with an image larger than the container, `ObjectFit::Cover`,
   and a 32 px rounded corner — assert the overflow is clipped.
4. Document the rule in a `// CONTRACT:` comment block on `img`'s
   paint method.

## References

### Upstream issues
- [zed-industries/zed#44339](https://github.com/zed-industries/zed/issues/44339) — `ObjectFit::Cover` + rounded corners produce the wrong clip.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — invalidates the same way as any other element.
- [docs/research/adr/ADR-012-custom-canvas-paint.md](ADR-012-custom-canvas-paint.md) — same rule applies to user-painted canvases that exceed their bounds.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #1 (_Rendering / GPU pipeline_).
