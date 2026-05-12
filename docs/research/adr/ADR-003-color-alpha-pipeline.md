# ADR-003: Color / alpha pipeline — CPU blending must match GPU source-over

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/color.rs`, `flui-core/src/platform/wgpu/wgpu_renderer.rs`.
**Drivers:** [zed-industries/zed#55972](https://github.com/zed-industries/zed/issues/55972),
[zed-industries/zed#33050](https://github.com/zed-industries/zed/issues/33050).
**Out of direct scope (but referenced):**
[flutter/flutter#14288](https://github.com/flutter/flutter/issues/14288) — antialiasing edge
artefacts; rasterizer-level, not blend-level.

## Context

Upstream GPUI accumulated two long-lived issues that theme authors and widget
authors keep rediscovering:

- **#33050** — `Rgba::blend` does not produce the result of canonical
  source-over alpha compositing. Stacking two semi-transparent rectangles
  visibly diverges from what every other renderer (Skia, Cairo, web browsers)
  produces.
- **#55972** — the visible effect of the same bug, plus its consequences for
  theming: two semi-transparent layers' opacities are *added* and clamped at
  1.0, so a `0.5` layer over a `0.5` layer becomes fully opaque.

Both reduce to the same root cause: the **CPU-side** blend function uses a
formula that is correct for RGB but wrong for the resulting alpha. The GPU
pipeline uses the right blend mode, so the bug only appears when a theme or a
widget pre-composes colors in Rust before handing them to the renderer.

flui-v2 inherited the function verbatim. This ADR fixes the contract before
any user code starts depending on the buggy behaviour.

## Current behaviour (verified)

References below cite the commit this ADR is written against.

### GPU side — correct

[`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs:627`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L627):

```rust
let blend_mode = match alpha_mode {
    wgpu::CompositeAlphaMode::PreMultiplied =>
        wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
    _ => wgpu::BlendState::ALPHA_BLENDING,
};
```

This is the canonical source-over formula. On the GPU, layers compose
correctly: a 50 % red over a 50 % red yields the right alpha and the right
visual result. The `premultiplied_alpha: u32` flag carried in the uniform
buffer at [wgpu_renderer.rs:20](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L20)
tells the shader which input convention to use.

### CPU side — incorrect

[`crates/flui-core/src/color.rs:56`](../../../crates/flui-core/src/color.rs#L56):

```rust
impl Rgba {
    pub fn blend(&self, other: Rgba) -> Self {
        if other.a >= 1.0 {
            other
        } else if other.a <= 0.0 {
            *self
        } else {
            Rgba {
                r: (self.r * (1.0 - other.a)) + (other.r * other.a),
                g: (self.g * (1.0 - other.a)) + (other.g * other.a),
                b: (self.b * (1.0 - other.a)) + (other.b * other.a),
                a: self.a,            // ← BUG: dst alpha is preserved,
                                      //   not composed.
            }
        }
    }
}
```

Canonical source-over for non-premultiplied RGBA, where `other` is the
*source* and `self` is the destination, is:

```
out.a   = src.a + dst.a * (1 - src.a)
out.rgb = (src.rgb * src.a + dst.rgb * dst.a * (1 - src.a)) / out.a   // when out.a > 0
```

Our `out.rgb` is a simplified form that is **only** correct when `self.a` is
already 1.0 (a fully-opaque destination). The moment two semi-transparent
colors are composed, both the alpha *and* the RGB drift away from the
canonical result.

[`crates/flui-core/src/color.rs:492`](../../../crates/flui-core/src/color.rs#L492):

```rust
impl Hsla {
    pub fn blend(self, other: Hsla) -> Hsla {
        let alpha = other.a;
        if alpha >= 1.0 {
            other
        } else if alpha <= 0.0 {
            self
        } else {
            let converted_self  = Rgba::from(self);
            let converted_other = Rgba::from(other);
            let blended_rgb     = converted_self.blend(converted_other);
            Hsla::from(blended_rgb)
        }
    }
}
```

`Hsla::blend` routes through `Rgba::blend` and inherits the same bug, plus an
extra lossy HSLA ↔ RGBA round-trip.

### Where the CPU blend is reached

The CPU `blend` and friends (`opacity`, `alpha`, `fade_out`) are used by:

- Theme authors when stacking semi-transparent surface tokens (e.g. computing
  a hover background from a base + a `surface_hover` overlay).
- Widget recipes that pre-compose a colour at build time rather than letting
  the GPU stack two layers.

In both cases the diverging alpha is visible because the *next* layer the GPU
draws on top of the precomposed colour now treats the dst alpha as
`self.a`, which is too low.

## Findings vs upstream issues

| Issue | Where it shows up in flui-v2 |
|-------|-------------------------------|
| [zed-industries/zed#33050](https://github.com/zed-industries/zed/issues/33050) | `Rgba::blend` at [color.rs:58](../../../crates/flui-core/src/color.rs#L58) reproduces the same divergence from canonical source-over alpha. |
| [zed-industries/zed#55972](https://github.com/zed-industries/zed/issues/55972) | Same `Rgba::blend` root cause; user-visible symptoms (theme color coupling, gradient transition breakage) depend on whether downstream code uses GPU or CPU compositing — both code paths exist. |
| [flutter/flutter#14288](https://github.com/flutter/flutter/issues/14288) | **Not the same bug.** Flutter's antialiasing seam between same-colour adjacent geometry is a rasterizer property of Skia/Impeller, not a blend formula. Listed here to disambiguate. |

## Decision (contract)

1. **Canonical source-over is the only supported alpha model.** All blending
   in flui-core, both CPU and GPU, follows:

   ```
   out.a   = src.a + dst.a * (1 - src.a)
   out.rgb = (src.rgb * src.a + dst.rgb * dst.a * (1 - src.a)) / out.a
            // RGB is undefined when out.a == 0; callers must check.
   ```

2. **`Rgba` and `Hsla` are non-premultiplied (straight) alpha.** Public color
   types stay straight; conversions to premultiplied happen only at the
   shader uniform / texture upload boundary.

3. **GPU and CPU agree.** A round-trip "blend N layers on the CPU into one
   `Rgba`, then push that `Rgba` to the GPU" must produce the same pixels as
   "let the GPU blend the N layers itself", within float rounding. This is a
   testable invariant — the action items below propose a unit test for it.

4. **`opacity(f)` and `alpha(f)` keep their existing semantics.** They scale
   alpha multiplicatively (`opacity`) or replace it (`alpha`). They do not
   compose two layers; that is `blend`'s job. The bug was in `blend`, not in
   these.

5. **`Rgba::blend` zero-alpha output is RGB = (0,0,0).** When the canonical
   formula divides by zero, we return `(0, 0, 0, 0)` instead of NaN. This is
   a contract, not an implementation detail — callers compare alphas to
   `<= 0.0` and short-circuit on it.

## Consequences

- Pre-composed theme colours start producing the same alpha as the GPU does.
  Existing themes that *relied* on the buggy alpha will visually shift; this
  is the right shift, not a regression, and matches every other modern
  renderer.
- Widgets that called `Hsla::blend` repeatedly to stack overlay tones will
  produce slightly different output. The migration is a known, bounded
  visual change, not a behaviour discontinuity.
- The fix is **purely arithmetic** — no dependency change, no GPU code
  change.

## Out of scope (separate ADRs)

- **Edge antialiasing artefacts** (Flutter #14288). Rasterizer-level. Will
  get its own ADR if/when we own the scene painter geometry path.
- **Color spaces / sRGB vs linear** (related to the surface format detection
  at [wgpu_renderer.rs:265](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L265)).
  Worth a separate ADR; we currently do not specify whether `Rgba` values
  are sRGB or linear, and this matters once HDR / wide-gamut output lands.
- **Gradient interpolation** (GPUI #55972 mentions gradient transition
  breakage). Distinct concern: gradient stops interpolate in HSLA, which
  has its own issues; this ADR fixes only the per-pixel blend.

## Action items (tracked; no code lands with this ADR)

1. Rewrite `Rgba::blend` to the canonical formula. Add a property-style
   test that stacks N random colours both on the CPU and as a description
   of what the GPU would do, and asserts pixel-equality within `1e-6`.
2. Audit existing uses of `Hsla::blend` / `Rgba::blend` in `flui-core`,
   `flui-theme`, `flui-material`, `flui-widgets`. Any caller that relied on
   the buggy alpha must be flagged.
3. Document the alpha convention in a `// CONTRACT:` comment block at the
   top of [color.rs](../../../crates/flui-core/src/color.rs) pointing back
   to this ADR.
4. Open a follow-up ADR for sRGB vs linear color space once a concrete
   visual issue requires it.

## References

### Upstream issues
- [zed-industries/zed#55972](https://github.com/zed-industries/zed/issues/55972) — opacity / theme color coupling.
- [zed-industries/zed#33050](https://github.com/zed-industries/zed/issues/33050) — gpui blending vs simple alpha composite.
- [flutter/flutter#14288](https://github.com/flutter/flutter/issues/14288) — referenced for disambiguation only.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-002-hover-active-invalidation.md](ADR-002-hover-active-invalidation.md)
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #2 (_Color / alpha pipeline_), covered by this ADR.
