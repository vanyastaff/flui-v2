# ADR-013: Text rasterization strategy — single hard-coded path today, contract for tomorrow

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/text_system.rs`, per-platform text glue
(`platform/mac/text_system.rs`, `platform/wgpu/cosmic_text_system.rs`,
`platform/windows/direct_write.rs`).
**Drivers:** [zed-industries/zed#55214](https://github.com/zed-industries/zed/issues/55214).
**Related:** [Flutter #67034](https://github.com/flutter/flutter/issues/67034)
(desktop fonts double-antialiased — symptom of an unselectable rasterization
strategy).

## Context

GPUI #55214 asks for control over text rasterization: today GPUI (and
flui-v2 by inheritance) always render text with subpixel positioning and
grayscale anti-aliasing. For certain workloads — code editors at low DPR,
CJK at small sizes, or terminal-style text — the user wants **bi-level**
(no AA) or **metrics-hinted** rendering instead, to get sharp pixels
aligned to the pixel grid.

The current code on macOS forces both subpixel positioning and AA at the
CoreText level
([`platform/mac/text_system.rs:442`](../../../crates/flui-core/src/platform/mac/text_system.rs#L442)):

```rust
cx.set_allows_antialiasing(true);
cx.set_should_antialias(true);
cx.set_allows_font_subpixel_positioning(true);
```

The wgpu / cosmic-text path on Linux follows the same default. The wgpu
shader pipeline has a separate compile path
([`platform/wgpu/shaders_subpixel.wgsl`](../../../crates/flui-core/src/platform/wgpu/shaders_subpixel.wgsl))
that the renderer can switch into when
`is_subpixel_rendering_supported()` returns true, but the *positioning*
choice is hard-coded — there is no "render this `Text` with bi-level"
path that a widget author can request.

The upstream issue notes the request is blocked on Skrifa supporting
hinting modes. That is an external dependency we share: flui-v2 already
pulls in cosmic-text / fontdb / Skrifa for Linux, and we will inherit
whatever they support when they support it.

## Current behaviour (verified)

[`crates/flui-core/src/platform.rs:671`](../../../crates/flui-core/src/platform.rs#L671):

```rust
fn is_subpixel_rendering_supported(&self) -> bool;
```

— a single boolean exposed on `PlatformWindow`. It governs whether the
wgpu pipeline picks the subpixel-AA shader; it does **not** let a widget
author choose the rasterization strategy.

`Window::paint_text` (called at
[`window.rs:3704`](../../../crates/flui-core/src/window.rs#L3704))
consults the boolean once and falls back to grayscale AA when it is
false. No per-`TextStyle` strategy override.

`TextStyle` (the public per-run/per-span style type) currently exposes
font family, size, weight, decoration, etc., but no
`rasterization` / `font_render_mode` field.

## Findings vs upstream

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#55214](https://github.com/zed-industries/zed/issues/55214) | No way to request bi-level or hinted text rasterization. | **yes — by omission**. The strategy is hard-coded; the public API gives no way to ask for anything else. |
| [flutter/flutter#67034](https://github.com/flutter/flutter/issues/67034) | Desktop fonts look double-antialiased. | **referenced**. Symptom of the same gap: when the platform compositor also AAs, the result is over-smooth. A future ADR on per-display rasterization may handle this; bi-level is part of the answer. |

## Decision (contract)

1. **`TextRasterMode` is the public per-style strategy enum.** Variants
   (provisional; final list lives in the action item):

   ```rust
   pub enum TextRasterMode {
       Subpixel,   // current default: subpixel positioning + grayscale AA
       Grayscale,  // pixel-aligned positioning + grayscale AA
       BiLevel,    // pixel-aligned positioning + 1-bit alpha (no AA)
       Hinted,     // metrics-hinted glyph dimensions, grayscale AA
   }
   ```

   `TextRasterMode::default()` returns `Subpixel`.

2. **`TextStyle` grows a `raster_mode: TextRasterMode` field.** Unset =
   inherit from the parent style; root inherits the platform default.

3. **Per-platform capability matrix is published.** Not every backend
   supports every mode (cosmic-text on Linux currently has no
   bi-level path; CoreText supports all four). A backend that does
   not support a mode renders the **next-best** supported variant
   silently — `BiLevel` falls through to `Grayscale`, `Hinted` falls
   through to `Subpixel`. The user-visible effect is "best-effort
   honoured"; the contract is "no panic and no silent black box".

4. **`is_subpixel_rendering_supported()` stays for backwards
   compatibility but is no longer the only switch.** When the
   platform returns `false`, requests for `Subpixel` fall through to
   `Grayscale`. This generalises the boolean into the new enum.

5. **Per-glyph cache key includes the mode.** The atlas keys text by
   (font, size, raster_mode); different modes produce different
   glyph bitmaps and must not share atlas slots.

6. **The contract is layered.** A widget library can pick a mode at
   the style level; an app-level "force bi-level on this view"
   setting reaches the same enum; future Theme-aware defaults set
   the mode at the root of a Theme scope.

## Consequences

- Apps that need sharp pixel-aligned text (code editor at 1× DPR,
  retro UIs, terminal panes) have an explicit knob.
- Glyph atlas memory may grow when the same view mixes modes; this
  is a cost the user chose by setting the mode.
- Skrifa / cosmic-text dependency upgrade lands the missing modes
  one by one without changing the public API.

## Out of scope (separate ADRs)

- **Per-display rasterization** (different mode on internal vs
  external display). Touches ADR-007 (display lifecycle); deserves
  its own when there is demand.
- **HDR / wide-gamut text colour**. Independent.
- **Variable-font axis control** (italic, weight, optical size).
  Style-level concern; not rasterization mode.
- **Emoji / colour-bitmap glyphs**. Different code path; the
  rasterization-mode enum does not apply.

## Action items (tracked; no code lands with this ADR)

1. Lock the `TextRasterMode` variant list. Cross-reference Skrifa's
   `Outlines::hinted` API and CoreText's `kCTFontUIFontEmphasized*` /
   `kCTFontTextEmphasis*` constants for naming consistency.
2. Add `raster_mode: TextRasterMode` to `TextStyle`. Inheritance
   through the existing style cascade keeps the change additive.
3. Define the per-platform capability matrix in a comment block at
   the top of [`text_system.rs`](../../../crates/flui-core/src/text_system.rs).
4. Add a fallback test: request `BiLevel` on a backend that does not
   support it, assert the rendered glyphs match `Grayscale` exactly.
5. Re-evaluate when Skrifa publishes hinting (`Outlines::hinted_glyph`
   stabilises) — this unblocks the `Hinted` variant on Linux/web.

## References

### Upstream issues
- [zed-industries/zed#55214](https://github.com/zed-industries/zed/issues/55214) — bi-level / metrics-hinted text request.
- [flutter/flutter#67034](https://github.com/flutter/flutter/issues/67034) — desktop fonts double-antialiased.

### Internal
- [docs/research/adr/ADR-004-text-slicing-utf8-safety.md](ADR-004-text-slicing-utf8-safety.md) — text-system sibling.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #3 (_Text rendering_), continued.
