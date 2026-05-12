# Flutter cross-walk — mapping flutter/flutter issues to flui-v2 ADRs

**Date:** 2026-05-12
**Status:** Derived artifact. Sources:
[`flutter-issues.md`](flutter-issues.md) (997 unique open issues across
10 UI-relevant labels) + [`adr/`](adr/) (ADRs 001–020).

## Purpose

ADRs 001–018 were authored primarily from the GPUI corpus. This document
maps the most-reacted Flutter issues to those ADRs, enriches the
evidence base for each, and flags Flutter themes that have **no
matching ADR yet** — those become candidates for future ADRs.

The table below is curated, not exhaustive. The cutoff is "reactions
≥ 5 or particularly architectural"; the long tail of one-off bugs is
not listed.

## Mapping

### ADR-001 — Invalidation scope

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#34864](https://github.com/flutter/flutter/issues/34864) | 35 | Screen flicker on Android during app-overview load — invalidation timing in the engine handover. |
| [#58425](https://github.com/flutter/flutter/issues/58425) | 0 | `RenderOpacity` sometimes omits its layer during animations, breaking caching — closest Flutter analogue to GPUI #50392. |
| [#69615](https://github.com/flutter/flutter/issues/69615) | 4 | Flare in `Stack` disappears when widget tree repaints — over-invalidation. |
| [#123123](https://github.com/flutter/flutter/issues/123123) | 2 | Partial screen flickering on Android — same family. |
| [#172079](https://github.com/flutter/flutter/issues/172079) | 2 | UI flickers on Linux Desktop on hover — hover-driven over-paint (also touches ADR-002). |

### ADR-002 — Hover / active invalidation

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#172079](https://github.com/flutter/flutter/issues/172079) | 2 | Linux Desktop hover flicker on icon buttons. Same shape: hover triggers more repaint than the visual change requires. |
| [#83046](https://github.com/flutter/flutter/issues/83046) | 0 | `AnimatedOpacity + BackdropFilter` lag on first trigger — pre-warming / invalidation cousin of ADR-002 (also touches ADR-017). |

### ADR-003 — Color / alpha pipeline

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#14288](https://github.com/flutter/flutter/issues/14288) | 197 | Antialiasing artefacts between same-colour adjacent geometry — classic disambiguation: this is a rasterizer property, *not* the source-over alpha contract. **Cross-reference, not a cause.** |
| [#65633](https://github.com/flutter/flutter/issues/65633) | 2 | `Color.withOpacity` on Arabic letters does not fully cover the word — alpha + text run boundary. |
| [#72249](https://github.com/flutter/flutter/issues/72249) | 0 | `ClipPath + CustomPaint + BlendMode.difference` artefacts on OnePlus — blend-mode behaviour. |
| [#167835](https://github.com/flutter/flutter/issues/167835) | 0 | `BlendMode.clear + MaskFilter.blur` render bug — blend semantics. |
| [#182066](https://github.com/flutter/flutter/issues/182066) | 2 | `FadeTransition` wrapping `CupertinoPopupSurface` breaks appearance on Impeller — alpha composition under filter. |
| [#31706](https://github.com/flutter/flutter/issues/31706) | 7 | `BackdropFilter` doesn't work as child of `Opacity` — also covered by ADR-017, but the root is alpha + filter ordering. |

### ADR-004 — Text slicing UTF-8 safety

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#79172](https://github.com/flutter/flutter/issues/79172) | 28 | `[web]` complex characters rendered incorrectly — shaping / slicing on the web target. |
| [#79931](https://github.com/flutter/flutter/issues/79931) | 12 | Chinese characters not vertically aligned in some fonts — metrics + run boundary. |
| [#180306](https://github.com/flutter/flutter/issues/180306) | 0 | CJK text-background rendering not in clear state — close relative of GPUI #49860. |
| [#115517](https://github.com/flutter/flutter/issues/115517) | 4 | Some emoji/text abnormal in different text orders — order-of-operations near grapheme boundaries. |

### ADR-005 — GPU device-loss recovery

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#111151](https://github.com/flutter/flutter/issues/111151) | 6 | "Engine and Embedders should gracefully handle GPU device loss" — direct twin of GPUI #23288. |
| [#177873](https://github.com/flutter/flutter/issues/177873) | 0 | Android Impeller-OpenGL ES SVG/VG corruption after Vulkan fallback — backend swap is the same loss-event family. |

### ADR-006 — Partial present / damage regions

Flutter's Skia/CanvasKit/Impeller pipeline already handles damage at
the engine level, so there are few direct equivalents. The closest:

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#94429](https://github.com/flutter/flutter/issues/94429) | 10 | `[web] [canvaskit]` resizing the browser is janky — partial-present-style symptom, root is CanvasKit's repaint scope on resize. |

### ADR-007 — Display lifecycle

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#32115](https://github.com/flutter/flutter/issues/32115) | 61 | Allow to set the logical pixel size programmatically — DPI knob the engine doesn't expose; adjacent to our scale-factor contract. |

### ADR-008 — Window chrome contract

Flutter's primary target is mobile, where window decorations are
platform-imposed. Desktop window-chrome issues are sparse; no
strong matches at the reactions threshold.

### ADR-009 — Input / IME pipeline

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#106789](https://github.com/flutter/flutter/issues/106789) | 12 | Hero animation on text field dismiss keyboard — IME visibility interacts with widget transitions, an `EditorCommand`-adjacent timing concern. |
| [#77023](https://github.com/flutter/flutter/issues/77023) | 21 | `[Web] [CanvasKit]` Load fonts as soon as detecting browser locale — keyboard/locale binding lifecycle. |

### ADR-010 — Local tab-index

Flutter has `FocusTraversalGroup` / `FocusNode` — its solution is
already hierarchical, like ours. No specific issue listed; the
contract holds the same way.

### ADR-011 — External drag-and-drop

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#1837](https://github.com/flutter/flutter/issues/1837) | 18 | `Draggable` feedback should animate back when drop fails — internal DnD, payload-shape adjacent to our `ExternalDropPayload` contract. |

### ADR-012 — Canvas custom paint

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#77485](https://github.com/flutter/flutter/issues/77485) | 14 | `Canvas.drawVertices` does not use anti-aliasing — Flutter's analogue of our low-level `Window::paint_*` capability surface. |
| [#105044](https://github.com/flutter/flutter/issues/105044) | 6 | Skia CanvasKit severe perf issue on Chrome/macOS — canvas backend performance. |
| [#94429](https://github.com/flutter/flutter/issues/94429) | 10 | `[web] [canvaskit]` resizing janky — same family. |
| [#77832](https://github.com/flutter/flutter/issues/77832) | 2 | `[web] [canvaskit]` resizing causes black dotted render — same family. |

### ADR-013 — Text rasterization strategy

This is the area where Flutter has the most signal. Many of the
top-reacted Flutter issues map here.

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#67034](https://github.com/flutter/flutter/issues/67034) | 60 | Fonts double-antialiased on desktop — exactly the symptom of an unselectable rasterization mode (TextRasterMode contract). |
| [#75832](https://github.com/flutter/flutter/issues/75832) | 22 | Font weight light (<400) does not render correctly on Flutter Web — rasterizer regression. |
| [#100964](https://github.com/flutter/flutter/issues/100964) | 19 | Emojis render no color on macOS Desktop — colour-glyph rasterization (explicit out-of-scope of ADR-013 today, but listed). |
| [#113026](https://github.com/flutter/flutter/issues/113026) | 12 | Warning ⚠️ emoji rendered incorrectly — same. |
| [#79172](https://github.com/flutter/flutter/issues/79172) | 28 | `[web]` complex characters rendered incorrectly — also touches ADR-004 (text slicing); listed in both. |
| [#79931](https://github.com/flutter/flutter/issues/79931) | 12 | Chinese vertical-alignment — metrics. |
| [#59798](https://github.com/flutter/flutter/issues/59798) | 9 | Hairline stroke AA across fractional pixel boundaries — pixel-grid alignment is exactly the "hinted vs subpixel" tension. |
| [#139854](https://github.com/flutter/flutter/issues/139854) | 5 | `[Web] [CanvasKit]` Chinese text with different font weights — rasterization + weight selection. |

### ADR-014 — Software rendering fallback

Flutter's Skia / Impeller / CanvasKit pipelines each have their own
software path; no direct twin of GPUI #45897 in the top-reacted set.

### ADR-015 — Image clip vs corner

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#86584](https://github.com/flutter/flutter/issues/86584) | 27 | `ListView` background overflows from parent `Container` — same family of "child paint exceeds container, container clip not applied". |
| [#117355](https://github.com/flutter/flutter/issues/117355) | 11 | Border of widget becomes blurry if centered — pixel-grid / fractional position; tangential. |
| [#184733](https://github.com/flutter/flutter/issues/184733) | 0 | Proposal to align widgets which use `BoxFit` — Flutter's `BoxFit` ≡ our `ObjectFit`; same alignment-meets-cover question. |

### ADR-016 — Wasm target gating

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#181848](https://github.com/flutter/flutter/issues/181848) | 0 | Occasional rendering issues with Flutter web with wasm — symptom of incomplete wasm-target gating in dependencies / engine. |
| [#180959](https://github.com/flutter/flutter/issues/180959) | 0 | Flutter Web `RuntimeEffect` rejects `dFdx`/`dFdy`/`fwidth` — wasm-target capability matrix. |

### ADR-017 — Window background blur

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#120028](https://github.com/flutter/flutter/issues/120028) | 7 | `BackdropFilter` inconsistent across edges with webview — blur + cross-context composition. |
| [#64828](https://github.com/flutter/flutter/issues/64828) | 3 | `ImageFilter.blur` bright edges and clip when resizing — same. |
| [#165422](https://github.com/flutter/flutter/issues/165422) | 3 | `BackdropFilter` with `ImageFilter.blur` rendering issue — same. |
| [#175537](https://github.com/flutter/flutter/issues/175537) | 1 | `BackdropFilter` inside `ShaderMask` — composition. |
| [#31706](https://github.com/flutter/flutter/issues/31706) | 7 | `BackdropFilter` doesn't work as child of `Opacity` — same. |
| [#83046](https://github.com/flutter/flutter/issues/83046) | 0 | `AnimatedOpacity + BackdropFilter` lag on first trigger — pre-warming. |

Pattern: Flutter's blur is widget-level (`BackdropFilter`), ours is
window-level. The cross-cutting concern is **filter + alpha
composition ordering**; we already capture this in ADRs 003 + 017.
There is no flui-v2 analogue of widget-level `BackdropFilter` today;
when one ships, this group becomes the evidence base for its ADR.

### ADR-018 — Modal & overlay layering

| Flutter issue | 👍 | Why it maps |
|---|---|---|
| [#182085](https://github.com/flutter/flutter/issues/182085) | 1 | Content of nested scroll view inside `Stack` escapes `Stack` boundaries — overlay clipping. |
| [#86584](https://github.com/flutter/flutter/issues/86584) | 27 | `ListView` background overflows — also listed for ADR-015; the clip-vs-layer tension covers both. |
| [#46070](https://github.com/flutter/flutter/issues/46070) | 2 | `Stack` relayout too pessimistic w.r.t. `Positioned` children — performance side of overlay layering. |

## Flutter themes with no matching flui-v2 ADR

These groups have visible reactions in the Flutter corpus but no
flui-v2 ADR covers them. They are candidates for ADR-019 onwards
when we choose to expand.

| Theme | Flutter issues |
|-------|----------------|
| **Scroll physics & bounce** | many `f: scrolling` issues; now scoped by [ADR-019](adr/ADR-019-scroll-physics.md). The implementation widget still has to be built. |
| **Status bar / system UI overlay** | [#64001](https://github.com/flutter/flutter/issues/64001) (54), [#54029](https://github.com/flutter/flutter/issues/54029) (25), [#119465](https://github.com/flutter/flutter/issues/119465) (24). Mobile-only; defer to mobile roadmap. |
| **iOS orientation handling** | [#73651](https://github.com/flutter/flutter/issues/73651) (17), [#71278](https://github.com/flutter/flutter/issues/71278) (25). Mobile-only. |
| **Design-system fidelity** (Material 3 Expressive, Liquid Glass) | [#168813](https://github.com/flutter/flutter/issues/168813) (765), [#170310](https://github.com/flutter/flutter/issues/170310) (670). Lives in `flui-material`, not engine; out of scope for engine-level ADRs. |
| **Layout API for advanced cases** | [#105511](https://github.com/flutter/flutter/issues/105511) (20) `MultiChildLayoutDelegate` replacement — layout-engine concern; we have Taffy under the hood, different model. |
| **API-breakage governance** | [#24722](https://github.com/flutter/flutter/issues/24722) (35) "breaking changes that would improve the overall API" — meta; for us this is K-series + ADR governance, already covered structurally. |

## How to use this document

- When working on an ADR action item, scan the relevant row for
  upstream Flutter context that informs the fix.
- When a Flutter issue ages and accumulates reactions, re-check
  whether it now warrants a new ADR. The flui-v2 overlay
  ([`flutter-issues-overlay.yaml`](flutter-issues-overlay.yaml))
  carries `adr` and `repro` markings for the issues listed here so
  the next snapshot regeneration surfaces drift.
- The Flutter snapshot is read-only here; this document is the
  cross-link.

## References

- [docs/research/flutter-issues.md](flutter-issues.md) — 997-issue snapshot.
- [docs/research/flutter-issues-overlay.yaml](flutter-issues-overlay.yaml) — per-issue triage overlay.
- [docs/research/adr/](adr/) — ADR 001–020.
- [docs/research/gpui-adr-candidates.md](gpui-adr-candidates.md) — original GPUI-derived theme grouping.
