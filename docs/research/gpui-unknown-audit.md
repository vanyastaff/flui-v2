# GPUI open issues — `unknown` repro audit (2026-05-12)

Twelve open issues carried `repro: unknown` in the overlay before
this audit. Each was reviewed against the flui-v2 code; this
document records the verdict and the evidence so the change is
traceable.

## Per-issue verdicts

### Closed by code check

#### #56314 — Wayland init `unreachable!` panic

- **Verdict:** `repro: no`.
- **Evidence:** Grep for `unreachable!` / `todo!` / `unimplemented!` in
  `crates/flui-core/src/platform/linux/` returns only four
  `unreachable!` sites in
  [`wayland/client.rs`](../../crates/flui-core/src/platform/linux/wayland/client.rs)
  at lines 1900, 1927, 1943, 1955 — all are exhaustive matches on
  `wl_pointer::Axis`, which is a closed enum in the
  `wayland-client` crate. Safe. The upstream panic site
  `crates/gpui_linux/src/linux.rs:55` does not have a counterpart
  in our tree (we split X11 and Wayland into separate modules).

#### #14110 — Disable rounded window corners option

- **Verdict:** `repro: yes`.
- **Evidence:** Grep for `round.*corner` / `disable.*round` /
  `rounded_window` in
  [`crates/flui-core/src/platform.rs`](../../crates/flui-core/src/platform.rs)
  returns zero matches in `WindowOptions`. The field that would
  control this does not exist. Adding a `bool square_corners`
  field when a UX case appears is a minor change; no ADR.

#### #8043 — Overdraw 5-6× per pixel

- **Verdict:** `repro: yes`. **New gap surfaced.**
- **Evidence:**
  [`wgpu_renderer.rs:674`](../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L674)
  sets `cull_mode: None`;
  [`wgpu_renderer.rs:679`](../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L679)
  sets `depth_stencil: None`. No front-to-back opaque pass, no
  depth-rejection, no stencil. Painting walks the scene
  back-to-front and writes every pixel of every overlapping
  primitive. The 5-6× factor from upstream RenderDoc traces is
  the expected outcome of this pipeline shape.
- **Follow-up:** This is *not* covered by any existing ADR.
  ADR-001 explicitly carved out "overdraw" as out of scope, and
  ADR-006 (partial present) is a different layer. Worth a new
  ADR-020 — see _Surfaced gaps_ below.

### Reclassified as not-our-scope (`n-a`)

#### #44529 — UI 1 px shift after login

- **Verdict:** `repro: n-a`.
- **Evidence:** Zed-specific UI — the title-bar avatar/name region
  that appears after account login. flui-v2 has no account flow.

#### #19805 — X11 wrap-guides under 1.5× scale

- **Verdict:** `repro: n-a`.
- **Evidence:** "Wrap guides" are an editor concept that lives in
  `settings.json["wrap_guides"]` in Zed. flui-v2 has no
  analogue element.

#### #54017 — macOS floating window does not become key

- **Verdict:** `repro: n-a`.
- **Evidence:** macOS `NSWindow` level / key-window state. macOS
  is not our current priority; the bug would only matter when we
  invest in mac.

#### #35903 — Window drag flaky on macOS

- **Verdict:** `repro: n-a`.
- **Evidence:** Same family as #54017 — macOS platform glue not
  on the current path.

### Reclassified to `partial` (architecturally covered, no benchmark)

#### #21341 — Shadow behind transparent windows

- **Verdict:** `repro: partial`.
- **Evidence:** We have shadow rendering paths (`paint_shadow` and
  related in `window.rs`) but have not exercised
  `transparent + maximized` to confirm whether the shadow is
  drawn behind the surface. Design-level question; not
  blocking.

#### #37727 — Windows text typing high GPU usage

- **Verdict:** `repro: partial`.
- **Evidence:** Structurally covered by ADR-001 (invalidation
  scope), ADR-006 (partial present), ADR-013 (`TextRasterMode`),
  and ADR-015 (clip layer overhead). A real benchmark against
  VSCode on the same Windows machine has not been run.

### Cannot resolve without target machine — kept `unknown`

#### #48103 — Wayland drag/resize not working for standalone apps

- **Evidence:** API exists —
  [`wayland/window.rs:1391`](../../crates/flui-core/src/platform/linux/wayland/window.rs#L1391)
  `start_window_move` calls `xdg_toplevel::_move`,
  [`window.rs:1399`](../../crates/flui-core/src/platform/linux/wayland/window.rs#L1399)
  `start_window_resize` calls `xdg_toplevel::resize`. Whether the
  upstream symptom reproduces depends on the compositor and is
  not testable from Windows.

#### #33956 — X11 cursor lag on 4K

- **Evidence:** X11 event handling is in place
  (`platform/linux/x11/{client,event,window}.rs`). Cursor-lag
  observation requires a 4K X11 machine.

#### #30469 — Wayland monitor off kills app

- **Evidence:** Already covered by ADR-007 (output disconnect
  contract); the `unknown` here is about actual repro on a
  Wayland machine. Left as `unknown` for the same machine-access
  reason; the contract is the binding part.

## Surfaced gaps

The audit found one issue (#8043 — overdraw) that is **not**
covered by any existing ADR and reproduces in our pipeline. This
becomes a candidate for **ADR-020 — opaque-pass / overdraw
strategy** in a future research turn.

The action items on existing ADRs that the audit re-confirmed:

- ADR-001 → add a real benchmark (no current one).
- ADR-013 → audit `TextRasterMode` fallback against Windows
  text-typing perf.
- ADR-015 → measure clip-layer cost.

## Distribution before/after

| repro    | before | after |
|----------|--------|-------|
| yes      | 7      | 8     |
| partial  | 17     | 18    |
| no       | 4      | 5     |
| n-a      | 5      | 9     |
| unknown  | 12 *(initial 18; 6 closed in prior turns)* | 3 |

Three remaining `unknown` all need a Linux machine; no further
desktop-Windows audit closes them.

## References

- [docs/research/gpui-issues-overlay.yaml](gpui-issues-overlay.yaml)
- [docs/research/gpui-issues.md](gpui-issues.md)
- ADRs 001, 006, 013, 015 (referenced above).
