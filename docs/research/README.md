# `docs/research/` — overview

Research artifacts derived from upstream GPUI and Flutter, plus the
ADRs (Architecture Decision Records) that codify what flui-v2
contracts must hold to avoid repeating upstream pain.

## How this folder fits together

```
gpui-issues.md            ← snapshot (43 open + 199 closed)
gpui-issues-overlay.yaml  ← per-issue triage (source of truth)
gpui-adr-candidates.md    ← theme groupings derived from triage

flutter-issues.md         ← snapshot (997 unique open, filtered labels)
flutter-issues-overlay.yaml
flutter-cross-walk.md     ← Flutter → ADR mapping
gpui-closed-cross-walk.md ← GPUI closed → ADR reading list
gpui-unknown-audit.md     ← log of the 2026-05-12 unknown-repro audit

mobile-roadmap.md         ← Android / iOS expansion plan

adr/ADR-001…020           ← 20 contracts, each scoped to one theme
```

The two **overlays** are the only files written by hand. Snapshots
regenerate from `scripts/fetch-{gpui,flutter}-issues.sh`.

## ADR index

| # | Topic | Drivers |
|---|-------|---------|
| [001](adr/ADR-001-invalidation-scope.md) | Invalidation scope — `refresh` / `notify` / `on_next_frame` contract | GPUI #50392, #15166, #56294 |
| [002](adr/ADR-002-hover-active-invalidation.md) | Hover/active state must use per-view invalidation | GPUI #24405, #38350 |
| [003](adr/ADR-003-color-alpha-pipeline.md) | CPU `Rgba::blend` must match canonical source-over | GPUI #55972, #33050 |
| [004](adr/ADR-004-text-slicing-utf8-safety.md) | `text_system` slice indices must come from a documented source | GPUI #49860 |
| [005](adr/ADR-005-gpu-device-loss.md) | GPU device-loss recovery contract + 6 known gaps | GPUI #23288, #52085, Flutter #111151 |
| [006](adr/ADR-006-partial-present-damage-regions.md) | Future damage-region API constraints | GPUI #15166 |
| [007](adr/ADR-007-display-lifecycle.md) | `displays()` / DPI / output disconnect observers | GPUI #46378, #21851, #30469 |
| [008](adr/ADR-008-window-chrome-contract.md) | `WindowOptions` flags are invariants, not hints | GPUI #52067, #27500 |
| [009](adr/ADR-009-input-ime-contract.md) | `EditorCommand` enum; `doCommandBySelector` honours the selector | GPUI #52550 |
| [010](adr/ADR-010-local-tab-index.md) | Hierarchical tab order — capability already present | GPUI #34796 |
| [011](adr/ADR-011-external-drag-drop.md) | Drag-drop payload is a typed enum, not just paths | GPUI #52110 |
| [012](adr/ADR-012-custom-canvas-paint.md) | `canvas(prepaint, paint)` is the low-level paint surface | GPUI #43273 |
| [013](adr/ADR-013-text-rasterization-strategy.md) | `TextRasterMode` enum + per-platform fallback | GPUI #55214, Flutter #67034 |
| [014](adr/ADR-014-software-rendering-fallback.md) | `RendererKind` + per-kind frame budget | GPUI #45897 |
| [015](adr/ADR-015-image-clip-vs-corner.md) | Rounded corners are a CLIP on the container, not the image | GPUI #44339 |
| [016](adr/ADR-016-wasm-target-gating.md) | Wasm dependency-gating policy | GPUI #52715 |
| [017](adr/ADR-017-window-background-blur.md) | `WindowBackgroundAppearance::Blurred` capability matrix | GPUI #14590 |
| [018](adr/ADR-018-modal-overlay-layering.md) | `defer_draw(priority)` + per-window modality | GPUI #52013, #52448 |
| [019](adr/ADR-019-scroll-physics.md) | `ScrollPhysics` trait scoping | GPUI #40623, Flutter `f: scrolling` |
| [020](adr/ADR-020-opaque-pass-overdraw.md) | Opaque/transparent pass split + depth reject | GPUI #8043 |

20 ADRs + 1 code change ([commit ed98186](../../) — `on_next_frame`
debug guard in `crates/flui-core/src/window.rs`).

## How a typical ADR is structured

1. **Context** — why the topic exists, which upstream issues drive it.
2. **Current behaviour (verified)** — direct `file:line` references to
   the flui-v2 code that holds (or breaks) the invariant today.
3. **Findings vs upstream** — table of upstream issue → reproduction
   status in flui-v2 (`yes` / `partial` / `no` / `unknown` / `n-a`).
4. **Decision (contract)** — numbered points binding on new code.
5. **Consequences** — what the contract enables and what it costs.
6. **Out of scope** — disambiguation from neighbouring themes.
7. **Action items** — tracked work; **no code lands with the ADR
   itself**. Code changes happen in their own commits.
8. **References** — upstream issues + internal cross-links.

## Triage status snapshot (43 open GPUI issues)

| repro | count |
|-------|-------|
| `yes` (we reproduce the bug) | 8 |
| `partial` (contract written, fix in backlog) | 18 |
| `no` (defended / already OK) | 5 |
| `n-a` (out of scope) | 9 |
| `unknown` (need a Linux machine) | 3 |

ADR coverage: 30 issues marked `adr: yes`, 2 `maybe`, the rest `no`/`n-a`.
See [gpui-unknown-audit.md](gpui-unknown-audit.md) for the audit that
moved 9 of the previously-`unknown` items to a resolved category.

## Maintenance

- **Re-fetching the snapshot:** `bash scripts/fetch-gpui-issues.sh`
  (requires `gh`, `jq`, `python` for YAML→JSON of the overlay).
  Same shape for Flutter via `scripts/fetch-flutter-issues.sh`.
- **Adding triage:** edit the overlay YAML (not the generated MD).
  Regenerate the MD via the script.
- **Adding an ADR:** copy any existing one as template; preserve the
  section layout; cross-link from `gpui-adr-candidates.md` if it
  appeared there, and add a row to this README's ADR index.

## Author note

The 19 ADRs in this folder do **not** implement fixes — they fix the
**contracts** that the fixes must satisfy. Action items inside each
ADR are the concrete code work. Each action item is intentionally
small and scoped so future implementation sessions can pick one off
without re-loading the full context.
