# ADR Action-Items Rollout Plan

**Date:** 2026-05-12
**Branch:** serene-hertz-c903e6 (existing worktree; no new branch)
**Plan type:** Full / cross-cutting rollout
**Source artefact:** `docs/research/adr/ADR-001…020`
**Scope owner:** flui-core (with platform-glue spillover in
`platform/{wgpu,mac,windows,linux/{x11,wayland},web}`)

## Goal

Convert the 19 active ADRs in `docs/research/adr/` (ADR-001 is closed) into
trackable engineering work. Each ADR carries an "Action items" section that
the ADR explicitly **does not implement** ("no code lands with this ADR").
This plan turns those action items into a phased rollout with clear
dependencies, commit checkpoints, and a per-task verification surface.

The ADRs themselves are frozen contracts — they tell us **what** must hold.
This plan tells us **how and when** to land the code that satisfies them.

## Non-goals

- Re-litigating ADR decisions. Each ADR is the source of truth for its
  contract; this plan does not propose contract changes.
- Implementing the future widget layer (Scrollable, modal_backdrop,
  custom-shader element). Those are out-of-scope per their own ADRs.
- Re-opening upstream issue fixes that the ADRs explicitly disambiguated
  (e.g. ADR-002 narrows hover blast radius but does not fix #24405 hit-test
  layering itself).
- Touching files outside the ADR-named scopes. Cross-task drift will be
  rejected at review.

## Settings

- **Testing:** yes — every code-touching task carries explicit test
  requirements (unit, property, integration, or visual-regression). Tasks
  that are pure documentation are explicitly noted as "no tests".
- **Logging:** verbose — debug-level instrumentation on every new code path
  (device-loss recovery, display-change observers, classifier passes,
  fallback selection). Logs name the ADR they enforce.
- **Docs:** yes — mandatory documentation checkpoint at completion via
  `/aif-docs`. Each task documents what it changes; the docs pass folds the
  contract notes into the public rustdoc surface.

## Roadmap Linkage

Milestone: "none"
Rationale: ADR action items span Engine (flui-core), Platform glue, and
cross-cutting tracks (A-track API hygiene, P-track perf, R-track release).
A single roadmap milestone does not fit. Once Phase 0-K critical chain is
fully landed, the Engine half of these ADRs becomes eligible for inclusion
in Phase II Engine-completeness work, and the GPU items (ADR-006, ADR-014,
ADR-020) feed P1/P5 in the Performance track. Per `/aif-verify --strict`
guidance, this is a WARN, not a failure.

## Research Context

The 20 ADRs codify the contracts the flui-v2 engine must hold to avoid
re-living the GPUI / Flutter long-tail issue clusters. Each ADR is
independently scoped to one theme; cross-references between them are
explicit. Action items inside each ADR are intentionally small and ordered
so future implementation sessions can pick one off without re-loading the
full context. ADR-001 is fully closed (its four action items landed
together with the ADR text). The remaining 19 ADRs carry between 2 and 5
action items each, ranging from "add a comment block" to "implement an
opaque-pass classifier in the scene builder".

This plan groups the action items by **risk profile and surface area**, not
by ADR number. Documentation passes ship first because they are free,
catch regressions early, and let later tasks reference the published
contract notes. Bug-class fixes ship next because their blast radius is
small and the perf wins are immediate. Larger architectural moves
(opaque-pass split, partial present, scroll physics) ship last because
they cross more files and need visual-regression infrastructure to merge
safely.

## Task Checklist

### Phase A — Foundation
- [x] Task 1: Add `// CONTRACT:` doc blocks across all ADR-touched files.

### Phase B — Bug-class fixes
- [x] Task 2: ADR-002 hover/active migration in `div.rs`.
- [x] Task 3: ADR-003 canonical source-over `Rgba::blend`.
- [x] Task 4: ADR-004 UTF-8 boundary assertions in `text_system`.

### Phase C — Resilience: device loss & display lifecycle
- [x] Task 5: ADR-005 device-loss gap closures.
- [x] Task 6: ADR-007 display lifecycle observers.

### Phase D — Window chrome & input enforcement
- [x] Task 7: ADR-008 `WindowOptions` invariants + drag-region hit-tree.
- [x] Task 8: ADR-009 `EditorCommand` enum + IME selector bridge.

### Phase E — Surface existing capabilities
- [x] Task 9: ADR-010 surface `cx.tab_group()` + tests.
  - **Finding:** `Window::with_tab_group(Option<isize>, ...)` already existed as the stable public sugar (named `with_tab_group` not `tab_group` per file's `with_text_style` convention) — annotated with ADR-010 reference.
  - **Implementation/contract drift flagged:** `TabStopMap` orders entries strictly by lexicographic comparison (negative `tab_index` comes BEFORE non-negatives), which contradicts ADR-010 decision 2's "negatives come after every non-negative" wording. The new test `adr_010_negative_tab_index_locked_to_lexicographic_order` locks the actual implementation behavior and documents the conflict. **Follow-up:** ADR-010 text or implementation needs reconciliation in a separate task (likely an ADR text update, since the ADR explicitly claims to "document current behaviour"). Tracked here so a future ADR-update PR can flip the test in lockstep with the spec change.
- [x] Task 10: ADR-012 `canvas` rustdoc example.

### Phase F — External DnD payload
- [x] Task 11: ADR-011 `ExternalDropPayload` migration (breaking).
  - **Landed:** `ExternalDropPayload` enum (`#[non_exhaustive]`, 6 variants: `Paths`/`Urls`/`Text`/`Html`/`Mime`/`Mixed`) + `Eq/PartialEq` derives; `FileDropEvent` renamed to `ExternalDropEvent` with the field `paths` → `payload`; soft-migration `pub type FileDropEvent = ExternalDropEvent` alias for out-of-tree callers; `DropAcceptFilter` type alias; `ClipboardEntry::ExternalPaths` → `ClipboardEntry::ExternalDrop(ExternalDropPayload)` (also `#[non_exhaustive]`); all 5 platform glue files (mac/pasteboard, mac/window, windows/clipboard, windows/window, linux/{x11,wayland}/client, web/events) updated to emit `Paths` payload via the new typed enum; 3 regression tests.
  - **Deferred follow-ups** (per ADR-011 #4/#5):
    - Wider per-platform MIME negotiation: macOS (`NSURLPboardType`, `NSStringPboardType`, `public.html`), Windows (`CF_INETURL`, `CF_UNICODETEXT`, `CF_HTML`), X11 (XDND types beyond `text/uri-list`), Wayland (`wl_data_offer::accept` for non-URI types), Web (full `DataTransfer.types` walk).
    - `DropAcceptFilter` wiring to `Div::on_drop` API — currently the type is defined but not consumed by the listener path.
    - Sandbox example (`examples/dnd_payload`) with URL drag from Firefox/Chrome/Safari/Edge — requires cross-browser manual testing.

### Phase G — Text rasterization + software-rendering pacing
- [x] Task 12: ADR-013 `TextRasterMode` + per-style override.
  - **Landed:** `TextRasterMode` enum (`#[non_exhaustive]`, 4 variants); `TextStyle::raster_mode` field with `Subpixel` default; `RenderGlyphParams::raster_mode` field so the per-glyph atlas key is mode-aware; `TextRasterMode::resolve_with_fallback` documented fallback chain (BiLevel→Grayscale, Hinted→Subpixel); 4 regression tests including the cascade through `Refineable`.
  - **Deferred:** per-platform text systems (CoreText, DirectWrite, cosmic-text) honouring the resolved mode at glyph rasterization time — engine `paint_text` currently hard-codes `Subpixel` (matches pre-ADR cache identity, no churn) so wiring the live style cascade → platform rasterization is a per-platform follow-up.
- [x] Task 13: ADR-014 `RendererKind` + software-rendering frame budget.
  - **Landed:** `RendererKind` enum (`#[non_exhaustive]`, `Hardware`/`Software`, default `Hardware`); `Platform::renderer_kind` trait method with `Hardware` default; `App::renderer_kind()` public accessor; `WgpuContext::renderer_kind` override that maps `wgpu::DeviceType::Cpu` → `Software`; 1 regression test on the test platform; `EditorCommand` and `RendererKind` added to the explicit re-export list at crate root.
  - **Deferred:** wiring `Platform::renderer_kind` on `LinuxPlatform<P>` to forward to `WgpuContext::renderer_kind` (requires touching `LinuxClient` trait); per-platform event-loop frame-budget pacing (60 fps hardware / 30 fps software per ADR decision 3 — needs touching `FrameClock` + each platform's event loop); `AnimationController` consulting the kind for per-frame tick interval; one-shot startup `log::info!` on Software detection.

### Phase H — Image clip-vs-corner + wasm gating
- [ ] Task 14: ADR-015 `push_rounded_clip` + `img.rs` two-step paint.
- [ ] Task 15: ADR-016 wasm gating policy + closure audit.

### Phase I — Background blur + modal/overlay layering
- [ ] Task 16: ADR-017 X11/KDE/Deepin background blur xprops.
- [ ] Task 17: ADR-018 `flui_core::z` module + hit-test priority audit.

### Phase J — Scroll physics scaffolding
- [ ] Task 18: ADR-019 `ScrollPhysics` trait + reference implementations.

### Phase K — Partial present scaffolding
- [ ] Task 19: ADR-006 `draw_with_damage` default + wgpu implementation.

### Phase L — Opaque-pass + depth pipeline
- [ ] Task 20: ADR-020 opaque-pass classifier + second wgpu pipeline.

## Phase structure

| Phase | Theme | Tasks | Dependencies |
|-------|-------|-------|--------------|
| **A** | Foundation — `// CONTRACT:` doc pass | 1 | none |
| **B** | Bug-class fixes (colour, text slicing, hover scope) | 2, 3, 4 | A |
| **C** | Device-loss & display lifecycle resilience | 5, 6 | A |
| **D** | Window chrome & input enforcement | 7, 8 | A |
| **E** | Surfacing existing capabilities (tab order, canvas docs) | 9, 10 | A |
| **F** | External DnD payload migration | 11 | A |
| **G** | Text rasterization & software-rendering pacing | 12, 13 | A |
| **H** | Image clip-vs-corner + wasm gating | 14, 15 | A |
| **I** | Background blur + modal/overlay layering | 16, 17 | D |
| **J** | Scroll physics scaffolding (types only) | 18 | G |
| **K** | Partial present scaffolding (trait default) | 19 | C |
| **L** | Opaque-pass + depth pipeline | 20 | A, C, K |

Phases A-H are independent of each other and may run in any order or in
parallel branches. Phase I depends on D (chrome enforcement primitives).
Phase J depends on G (frame-budget hook + raster-mode interplay). Phase K
depends on C (post-`recover()` full-present rule). Phase L depends on
A + C + K (depth attachment recreates on device-loss + composes with
damage region).

---

## Tasks

### Phase A — Foundation: contract documentation pass

#### Task 1: Add `// CONTRACT:` doc blocks across all ADR-touched files

**Deliverable:** One doc-only commit per logical area (color, text, gpu,
input, window, scene, etc.). Every file named in an ADR's "Action items"
list as needing a doc comment gets one, pointing back to the ADR by
filename and section.

**Files to touch (with their ADR refs):**
- `crates/flui-core/src/color.rs` → ADR-003 #3
- `crates/flui-core/src/text_system/line_wrapper.rs` → ADR-004 #4
- `crates/flui-core/src/text_system.rs` → ADR-013 #3 (per-platform capability matrix)
- `crates/flui-core/src/platform/wgpu/wgpu_renderer.rs` → ADR-005 #1
- `crates/flui-core/src/platform.rs:246` → ADR-007 #5 (`displays()` blocking-populate)
- `crates/flui-core/src/platform.rs:1615` → ADR-017 #2 (background-appearance capability matrix)
- `crates/flui-core/src/tab_stop.rs` → ADR-010 #2
- `crates/flui-core/src/elements/canvas.rs` → ADR-012 #1
- `crates/flui-core/Cargo.toml` → ADR-016 #4 (wasm gating policy)

**Logging:** none — pure documentation, no runtime change.

**Tests:** none — documentation only. Run `cargo doc -p flui-core` to
verify the rustdoc renders.

**Dependencies:** none — landable today.

**Verification:** `cargo doc -p flui-core --no-deps` builds clean;
`flui-arch-reviewer` agent run on the doc diff returns clean.

---

### Phase B — Bug-class fixes

#### Task 2: ADR-002 hover/active migration in `div.rs`

**ADR ref:** ADR-002 (Hover/active state must use per-view invalidation).

**Deliverable:** Migrate the six candidate sites in
`crates/flui-core/src/elements/div.rs` from `window.refresh()` to
`cx.notify(current_view)`:
- `div.rs:2461` (modifiers-changed)
- `div.rs:2719` (mouse-down sets `pending_mouse_down`)
- `div.rs:2800` (mouse-up commits)
- `div.rs:2808` (mouse-up cancels)
- `div.rs:2920` (mouse-up clears `clicked_state`)
- `div.rs:2941` (mouse-down sets `clicked_state`)

Capture `let current_view = window.current_view();` at registration time
(not fire time, per decision 3). Three logical commits suggested:
modifiers; click `pending_mouse_down`; active state. Add a `// LINT:`
comment near remaining `window.refresh()` call sites in `elements/*` so
future code does not silently regress (decision 4).

**Files:** `crates/flui-core/src/elements/div.rs`.

**Logging:** debug-level trace at each new `cx.notify(current_view)` site
("hover migration: notifying view {view_id}") gated behind a
`tracing::trace_span!`.

**Tests:** an `elements::div::tests` test that wires a synthetic
mouse-event sequence (down → up → leave) and asserts only the target
view's dirty flag flips, not the window's. `cargo test -p flui-core --lib`
must stay green.

**Dependencies:** Task 1 (the `// LINT:` comment uses the ADR-002 contract
note).

**Verification:** `cargo clippy -p flui-core --all-targets -- -D warnings`
clean; existing `flui-core` test suite passes.

---

#### Task 3: ADR-003 canonical source-over `Rgba::blend`

**ADR ref:** ADR-003 (Color/alpha pipeline — CPU blending must match GPU
source-over).

**Deliverable:**
1. Rewrite `Rgba::blend` at `crates/flui-core/src/color.rs:56` to the
   canonical formula from decision point 1. `Hsla::blend` at `color.rs:492`
   inherits the fix automatically.
2. Implement decision 5 (zero-alpha → `(0,0,0,0)`, no NaN).
3. Add a property-style unit test that stacks N random colours both on the
   CPU and as a description of what the GPU would do (using
   `BlendState::ALPHA_BLENDING` semantics), asserts pixel-equality within
   `1e-6`.
4. Audit existing uses of `Hsla::blend` / `Rgba::blend` across
   `flui-core`, `flui-theme`, `flui-material`, `flui-widgets`. Document
   any caller that relied on the buggy alpha (flag with a `// FIXME(adr-003):`
   comment).

**Files:** `crates/flui-core/src/color.rs`, audit-only reads of
`crates/flui-{theme,material,widgets}/src/**`.

**Logging:** none — pure arithmetic; no new code path.

**Tests:** new `color::tests::canonical_source_over_*` module with
proptest sweep (1 000 random colour pairs × 10 stack depths). Existing
colour tests must continue to pass; visual changes are expected and
acceptable per the ADR's "consequences" section.

**Dependencies:** Task 1 (the `// CONTRACT:` block in `color.rs`).

**Verification:** `cargo test -p flui-core --lib color::`; the audit log
attaches to the commit message.

---

#### Task 4: ADR-004 UTF-8 boundary assertions in `text_system`

**ADR ref:** ADR-004 (Text slicing — UTF-8 boundary safety).

**Deliverable:**
1. Add `debug_assert!(text.is_char_boundary(byte_index))` at
   `crates/flui-core/src/text_system/line.rs:211` and `:212`.
2. Add `debug_assert!(text.is_char_boundary(glyph.index))` at
   `crates/flui-core/src/text_system/line_layout.rs:150` with a
   `// CONTRACT:` comment naming the platform shaper (CoreText /
   DirectWrite / HarfBuzz) as the producer.
3. Add a property-style test in `line_wrapper.rs::tests` that feeds the
   truncation pipeline a corpus of mixed-script strings (ASCII + CJK +
   Latin + emoji ZWJ + regional indicators), spanning every break the
   existing tests miss, and asserts no panic for any prefix width.

**Files:** `crates/flui-core/src/text_system/{line,line_layout,line_wrapper}.rs`.

**Logging:** none — assertions are silent in release; debug builds get the
panic message.

**Tests:** the new `line_wrapper.rs::tests::mixed_script_truncation`
proptest. Existing `text_system` tests must stay green in both debug and
release modes.

**Dependencies:** Task 1 (the `line_wrapper.rs` `// CONTRACT:` block).

**Verification:** `cargo test -p flui-core --lib text_system::`; debug
build assertions fire on a deliberately corrupted index (smoke check that
the assert is actually reached).

---

### Phase C — Resilience: device loss & display lifecycle

#### Task 5: ADR-005 device-loss gap closures

**ADR ref:** ADR-005 (GPU device-loss — recovery contract and known gaps).

**Deliverable:**
1. Replace the `SurfaceError::OutOfMemory | Other` no-op at
   `crates/flui-core/src/platform/wgpu/wgpu_renderer.rs:1079` with a path
   that sets `device_lost = true` (driving the existing `recover()` flow)
   and logs the error kind distinctly (per decision 5).
2. Replace the literal `350` at `wgpu_renderer.rs:1666` with a named
   constant `POST_DEVICE_LOSS_STABILIZATION_DELAY` and add a `// MAGIC:`
   comment documenting its origin.
3. Write a `flui-core` integration-style test that drives device loss via
   `set_device_lost_callback`, calls `Renderer::recover(&window)`, and
   asserts `App`/`Window` state survives unchanged (per decision 3: "user
   state survives device loss").

**Files:** `crates/flui-core/src/platform/wgpu/wgpu_renderer.rs`,
`crates/flui-core/tests/device_loss.rs` (new — alongside the existing
`gesture_dispatch_integration.rs` integration test).

**Logging:** distinct `tracing::warn!` per `SurfaceError` arm, identifying
the kind. The one-shot startup `log::info!` for renderer-kind detection is
in Task 13, not here.

**Tests:** the new integration test runs under the **S01b headless wgpu
harness** (`WgpuContext::new_headless`) gated by `--features test-support`
so it works on `lavapipe` in CI and on real GPUs locally. The test platform
backend (no real GPU) skips this test entirely. Existing wgpu tests must
stay green on macOS, Linux, and Windows-via-wgpu paths.

**Dependencies:** Task 1 (the `wgpu_renderer.rs` ADR comment block).

**Verification:** `cargo test -p flui-core --features test-support`;
manual smoke on at least one wgpu host. The migration-risk-adversary agent
should review the recovery diff.

---

#### Task 6: ADR-007 display lifecycle observers

**ADR ref:** ADR-007 (Display lifecycle — `displays()`, DPI changes,
output disconnect).

**Deliverable:**
1. Add the two trait methods `Platform::on_displays_changed(callback)`
   and `Window::observe_display_change(callback)` with default
   implementations that never fire (decision 3 + contract expressed in
   types before any backend wires them).
2. Implement the Wayland binding for `wl_output` add/remove that drives
   `on_displays_changed`. Document the registry event mapping at the
   call site.
3. Implement the X11 XRandR notification path that drives
   `observe_display_change` independently of `bounds_changed`.
4. Add a `Window::observe_display_change` test in the test platform that
   simulates output removal and asserts the window survives (decision 5;
   closes #30469's repro pattern under test).

**Files:** `crates/flui-core/src/platform.rs`,
`crates/flui-core/src/window.rs`,
`crates/flui-core/src/platform/linux/wayland/{client,window}.rs`,
`crates/flui-core/src/platform/linux/x11/{client,window}.rs`,
`crates/flui-core/src/platform/test/{platform,window}.rs`.

**Logging:** `tracing::debug!` at every observer fire site with the
display id and event type. Wayland and X11 paths log at the registry
binding edge so output add/remove timing is observable.

**Tests:** new test-platform driver that simulates output add → remove
→ readd; asserts the window's `display_id` updates and a single
`bounds_changed` fires on rebind to primary.

**Dependencies:** Task 1 (the `platform.rs` ADR-007 `// CONTRACT:` block).

**Verification:** `cargo test -p flui-core --lib`; manual smoke on a
Wayland desktop with `wlr-randr` toggling outputs, and an X11 desktop with
`xrandr --auto`. The flui-arch-reviewer agent should review the trait
addition (new public surface).

---

### Phase D — Window chrome & input enforcement

#### Task 7: ADR-008 `WindowOptions` invariants + drag-region hit-tree

**ADR ref:** ADR-008 (Window chrome — `WindowOptions` invariants and
drag-region semantics).

**Deliverable:**
1. Audit `WM_SYSCOMMAND` handling in
   `crates/flui-core/src/platform/windows/events.rs`; add an interception
   that filters `SC_MINIMIZE` / `SC_MAXIMIZE` / `SC_MOVE` / `SC_SIZE`
   against the active `WindowOptions`. Default reject is ignore.
2. Audit macOS minimize / zoom / titlebar drag paths in
   `crates/flui-core/src/platform/mac/window.rs`; gate them on the flags.
3. Add hit-tree-aware drag-region in the title-bar element so
   `mouse_down`-bearing children win the gesture (decision 4).
4. Add a test platform smoke test that creates a window with
   `is_minimizable: false` and verifies a synthetic system-menu invocation
   does not minimize it. Add `Window::minimize` / `Window::maximize` gates
   per decision 6.

**Files:** `crates/flui-core/src/platform.rs` (the `Window` programmatic
gates),
`crates/flui-core/src/platform/windows/{events,window}.rs` (`WM_SYSCOMMAND`
filter + drag-region),
`crates/flui-core/src/platform/mac/window.rs` (minimize/zoom gates +
drag-region),
`crates/flui-core/src/platform/linux/{x11,wayland}/window.rs` (drag-region;
WM/compositor hint gates),
`crates/flui-core/src/gesture/hit_test.rs` (hit-tree-aware drag-region
delivery — the per-child `mouse_down`-bearing widget wins before the
title-bar fall-through). There is no `elements/title_bar.rs` element in
flui-core today; the drag-region computation lives inside each platform
window file, so this task's "second line" enforcement happens there.

**Logging:** `tracing::warn!` when a gated programmatic call is rejected;
`tracing::debug!` when a synthetic system-menu command is filtered.

**Tests:** test-platform unit test for the programmatic gate; manual
smoke on Windows (Alt+Space → Minimize) and macOS (dock right-click →
Minimize).

**Dependencies:** Task 1.

**Verification:** `cargo test -p flui-core`; the migration-risk-adversary
agent should review the platform-glue diff per memory feedback on
pre-PR review triple-launch.

---

#### Task 8: ADR-009 `EditorCommand` enum + `InputHandler::handle_editor_command`

**ADR ref:** ADR-009 (Input / IME pipeline — `doCommandBySelector` must
honour selectors).

**Deliverable:**
1. Define `EditorCommand` enum in `crates/flui-core/src/platform.rs`
   seeded from the Cocoa `StandardKeyBindingResponding` protocol (the
   ADR's action item 1 lists the variants).
2. Add `InputHandler::handle_editor_command(&mut self, command,
   window, cx) -> bool` with a default `false` (decision 2 + backwards
   compatible).
3. Rewrite the handler at
   `crates/flui-core/src/platform/mac/window.rs:2423` to read the
   selector argument, look it up in a static table, and dispatch through
   `handle_editor_command`. Fallback to the current key-down path on
   unknown selectors or on the handler returning `false`.
4. Add tests that drive `ctrl-W` and `ctrl-A` through a mock
   `InputHandler` and assert `EditorCommand::DeleteWordBackward` /
   `MoveToBeginningOfLine` is observed.

**Files:** `crates/flui-core/src/platform.rs`,
`crates/flui-core/src/platform/mac/window.rs`.

**Logging:** `tracing::trace!` at the bridge entry naming the
selector and the routed `EditorCommand` variant; `tracing::debug!` on
fallback to keymap path. Helps debug `DefaultKeyBinding.dict` mismatches.

**Tests:** new `input_handler::tests` module with mock-selector roundtrip
on macOS path only (cross-platform Windows/Linux IME bridges are
out-of-scope per ADR-009 decision 5).

**Dependencies:** Task 1.

**Verification:** `cargo test -p flui-core` on macOS host; manual smoke
with a custom `~/Library/KeyBindings/DefaultKeyBinding.dict`. The
rust-api-migration-auditor agent should review the new public `EditorCommand`
enum (semver-relevant).

---

### Phase E — Surface existing capabilities

#### Task 9: ADR-010 surface `cx.tab_group()` + tests

**ADR ref:** ADR-010 (Local tab-index — already present, contract to be
made explicit).

**Deliverable:**
1. Surface `cx.tab_group(tab_index, |cx| { ... })` as the public sugar
   over the existing `begin_group` / `end_group` `pub fn` on the
   `pub(crate)` `TabStopMap`. The helper is the API authors compose with.
2. Add tests in `tab_stop.rs::tests` that cover all six decision points:
   hierarchical lexicographic ordering, `tab_index = 0` (document order),
   negative `tab_index` (after non-negative), `tab_stop = false`
   (programmatic-only), group-boundary fall-through, wrap on `next`/`prev`.
3. When `flui-a11y` gains real code (out of scope here), expose
   `TabStopMap::iter()` for AT traversal — leave a `// FUTURE:` marker.

**Files:** `crates/flui-core/src/tab_stop.rs`,
`crates/flui-core/src/window.rs` (the `cx.tab_group` helper),
`crates/flui-core/src/lib.rs` (public re-export if needed).

**Logging:** none — no runtime change beyond a thin wrapper.

**Tests:** the six new `tab_stop::tests::*` cases plus a doctest on the
public `cx.tab_group` helper showing a nested-group example.

**Dependencies:** Task 1.

**Verification:** `cargo test -p flui-core --lib tab_stop::`;
`cargo doc -p flui-core --no-deps`.

---

#### Task 10: ADR-012 `canvas` rustdoc example

**ADR ref:** ADR-012 (Custom canvas paint — `canvas(prepaint, paint)`
already covers low-level drawing).

**Deliverable:**
1. Add a documentation example to the rustdoc of `canvas()` at
   `crates/flui-core/src/elements/canvas.rs:9` showing the prepaint /
   paint split and one realistic use (e.g. a sparkline drawing
   `Window::paint_path` from prepaint-laid-out points).
2. The `// CONTRACT:` block itself ships in Task 1; this task is the
   doctest + example pair.

**Files:** `crates/flui-core/src/elements/canvas.rs`.

**Logging:** none.

**Tests:** the new doctest. `cargo test --doc -p flui-core canvas`.

**Dependencies:** Task 1.

**Verification:** `cargo doc -p flui-core --no-deps` renders cleanly with
the example visible.

---

### Phase F — External DnD payload

#### Task 11: ADR-011 `ExternalDropPayload` migration

**ADR ref:** ADR-011 (External drag-and-drop — payloads beyond file paths).

**Deliverable:**
1. Lock the final variant list for `ExternalDropPayload`: `Paths`, `Urls`,
   `Text`, `Html { html, text }`, `Mime { kind, bytes }`, `Mixed` (per ADR
   decision 2, with `Mixed` boxing the recursive variant).
2. Rename `FileDropEvent` → `ExternalDropEvent` and refactor the field
   `paths: ExternalPaths` to `payload: ExternalDropPayload`. Update
   `ClipboardEntry::ExternalPaths` to share the enum (decision 4).
3. Specify `DropAcceptFilter` as a `Fn(&ExternalDropPayload) -> bool`
   per `Div::on_drop`; helper macros `accepts_paths!()` / `accepts_urls!()`
   may follow but are not required here.
4. Wire per-platform DnD glue:
   - macOS: `NSDraggingInfo::pasteboard.types`
   - Windows: `IDataObject::EnumFormatEtc`
   - X11: `XdndAware` types negotiation
   - Wayland: `wl_data_offer::accept`
5. Add a manual sandbox test (`examples/dnd_payload`) that prints the
   payload kind for URL drags from Firefox/Chrome/Safari.

**Files:** `crates/flui-core/src/interactive.rs`,
`crates/flui-core/src/platform.rs` (the `ClipboardEntry` variant rename),
`crates/flui-core/src/platform/{mac,windows,linux/x11,linux/wayland,web}/**`
DnD glue, new `examples/dnd_payload/`.

**Logging:** `tracing::debug!` at the platform DnD ingest boundary naming
the advertised MIME types and the resolved variant.

**Tests:** an `interactive::tests::dropped_event_roundtrip` that
constructs each variant and round-trips through the listener; the manual
sandbox example for cross-browser smoke.

**Dependencies:** Task 1; benefits from running after Task 8 because
both touch platform-glue files (mac/window.rs).

**Verification:** `cargo test -p flui-core`; manual smoke per platform.
Breaking API change — the rust-api-migration-auditor agent must review
(rename + new public enum) per memory feedback on pre-PR triple-review.

---

### Phase G — Text rasterization + software-rendering pacing

#### Task 12: ADR-013 `TextRasterMode` + per-style override

**ADR ref:** ADR-013 (Text rasterization strategy — single hard-coded
path today, contract for tomorrow).

**Deliverable:**
1. Lock the `TextRasterMode` variant list: `Subpixel` (default),
   `Grayscale`, `BiLevel`, `Hinted`. Cross-reference Skrifa's
   `Outlines::hinted` API and CoreText's `kCT*` constants for naming.
2. Add `raster_mode: TextRasterMode` to `TextStyle`. Inherit through the
   existing style cascade (decision 2).
3. Update the per-glyph cache key to include the mode (decision 5).
4. Implement per-platform fallback chain (decision 3): `BiLevel` →
   `Grayscale` when unsupported; `Hinted` → `Subpixel` when unsupported.
5. Add a fallback test: request `BiLevel` on a backend that does not
   support it, assert the rendered glyphs match `Grayscale` exactly.

**Files:** `crates/flui-core/src/text_system.rs` (the new enum + cascade),
`crates/flui-core/src/style.rs` (the `TextStyle` field),
`crates/flui-core/src/window.rs` (the `paint_text` consumer),
`crates/flui-core/src/platform/mac/text_system.rs`,
`crates/flui-core/src/platform/wgpu/cosmic_text_system.rs`,
`crates/flui-core/src/platform/windows/direct_write.rs`.

**Logging:** `tracing::debug!` once at startup per renderer listing
which `TextRasterMode` variants are supported; per-style cache misses
log the resolved mode at `trace!` level (rate-limited via a sampling
filter to avoid flooding under text-heavy redraws).

**Tests:** the fallback test, plus a property test verifying
`TextRasterMode::default() == Subpixel`. Visual-regression on a small
"Hello World" snapshot for each mode where supported.

**Dependencies:** Task 1 (the `text_system.rs` capability matrix
comment).

**Verification:** `cargo test -p flui-core`; manual smoke on each
desktop platform. rust-api-migration-auditor reviews the new public enum
plus `TextStyle` field addition.

---

#### Task 13: ADR-014 `RendererKind` + software-rendering frame budget

**ADR ref:** ADR-014 (Software rendering fallback — accept, reject, or
expose?).

**Deliverable:**
1. Add `RendererKind` enum (`Hardware`, `Software`) to
   `crates/flui-core/src/platform.rs` and expose `App::renderer_kind()`.
   Populate from `wgpu::Adapter::get_info().device_type == DeviceType::Cpu`
   (and the equivalent on DirectX / Metal).
2. Plumb the frame budget into the platform-specific event loop:
   Linux X11/Wayland timer interval, Windows composition-clock
   subscription, macOS CVDisplayLink-or-equivalent. Default: 60 fps on
   hardware, 30 fps on software (decision 3).
3. `AnimationController` consults `renderer_kind()` and does not
   schedule per-frame ticks faster than the budget allows (decision 4).
4. One-shot `log::info!("Software renderer detected (...); frame budget
   reduced to 30 fps")` at startup when software is selected.
5. Manual smoke on a Linux box without Vulkan (or with `WGPU_BACKEND=gl`
   forcing GL) — verify CPU usage drops below the issue's report.

**Files:** `crates/flui-core/src/platform.rs` (new `RendererKind` enum +
`App::renderer_kind`),
`crates/flui-core/src/app.rs`,
`crates/flui-core/src/frame/{clock,tick}.rs` (K04's `FrameClock` — the
30-vs-60 fps policy is expressed here as a `FrameClock`-level cadence, not
duplicated in `animation/`),
`crates/flui-core/src/animation/controller.rs` (consults the
`FrameClock` cadence; does not own the policy),
`crates/flui-core/src/platform/{mac,windows,linux/x11,linux/wayland}/platform.rs`,
`crates/flui-core/src/platform/wgpu/wgpu_context.rs`.

**Logging:** the one-shot `info!`; `tracing::debug!` on every adapter
re-classification (post-`recover()` per ADR-005 interaction).

**Tests:** unit test that mocks `RendererKind::Software` and asserts the
`AnimationController` tick interval is 33 ms (30 fps), not 16 ms (60 fps).

**Dependencies:** Task 1; benefits from running after Task 5 so the
`recover()` adapter re-classification is in place.

**Verification:** `cargo test -p flui-core`; manual smoke on
`WGPU_BACKEND=gl`. wgpu-gpu-reviewer agent reviews the adapter
classification surface.

---

### Phase H — Image clip-vs-corner + wasm gating

#### Task 14: ADR-015 `push_rounded_clip` + `img.rs` two-step paint

**ADR ref:** ADR-015 (ObjectFit::Cover with rounded corners — clip
outside the image).

**Deliverable:**
1. Add `Window::push_rounded_clip(bounds, corner_radii)` and the
   corresponding `pop_layer` semantics; extend the layer stack to track
   the clip shape if it does not already.
2. Update `crates/flui-core/src/elements/img.rs:490` to use the new
   path when corner radii are non-zero; keep the single-call fast path
   when they are zero (decision 3).
3. Visual-regression snapshot: image larger than the container,
   `ObjectFit::Cover`, 32 px rounded corner — assert the overflow is
   clipped against the container shape, not the image shape.
4. The `// CONTRACT:` block on `img`'s paint method ships in Task 1.

**Files:** `crates/flui-core/src/window.rs`,
`crates/flui-core/src/elements/img.rs`, plus the visual-regression
infrastructure in `crates/flui-core/tests/golden/`.

**Logging:** `tracing::trace!` at the new `push_rounded_clip` site
naming the radii and bounds for debugging clip-shape stack imbalances.

**Tests:** the new visual-regression test under the S01b golden harness
on macOS, Linux-wgpu, Windows. Existing img tests must produce the
same output for `ObjectFit::Contain` / `None` (decision 4: identical
output before and after).

**Dependencies:** Task 1; uses the S01b golden infrastructure.

**Verification:** `cargo test -p flui-core --features test-support`;
golden snapshots regenerated and committed. wgpu-gpu-reviewer reviews
the layer-stack change.

---

#### Task 15: ADR-016 wasm gating policy + closure audit

**ADR ref:** ADR-016 (Wasm target dependency gating — keep `imp` and
native crates out).

**Deliverable:**
1. Add a `cargo check --target wasm32-unknown-unknown -p flui-core` job
   to CI (`.github/workflows/wasm-check.yml`). First failure is
   informational; once green, becomes blocking.
2. Audit every `wasm_bindgen::Closure` / `wasm_bindgen::closure::Closure`
   site in flui-core. Document the lifetime of each closure with a
   comment block; convert recursive invocations to
   `wasm_bindgen_futures::spawn_local` where possible.
3. Move any wasm-specific dependency under a
   `[target.'cfg(target_family = "wasm")'.dependencies]` block in
   `crates/flui-core/Cargo.toml`. Audit the current cross-target block
   for unused wasm-only crates.
4. The `// CONTRACT:` comment in `Cargo.toml` ships in Task 1.

**Files:** `crates/flui-core/Cargo.toml`,
`crates/flui-core/src/platform/web/**`,
`.github/workflows/wasm-check.yml` (new or extended).

**Logging:** none — Cargo-level + build-time only.

**Tests:** the new CI job. No runtime tests at this stage (the wasm
integration test surface deserves its own follow-up plan once a real
`hello_world` wasm target ships).

**Dependencies:** Task 1.

**Verification:** the CI job goes from red → green; `cargo check
--target wasm32-unknown-unknown -p flui-core` runs clean locally.

---

### Phase I — Background blur + modal/overlay layering

#### Task 16: ADR-017 X11/KDE/Deepin background blur xprops

**ADR ref:** ADR-017 (Window background blur — X11/KDE/Deepin xprops fill
in an existing API).

**Deliverable:**
1. Implement the `Blurred` branch in
   `crates/flui-core/src/platform/linux/x11/window.rs`:
   set `_KDE_NET_WM_BLUR_BEHIND_REGION` and
   `_NET_WM_DEEPIN_BLUR_REGION_ROUNDED` on
   `set_background_appearance(Blurred)`. Clear them on any other variant.
2. `background_appearance()` getter reflects the *actually applied*
   state (decision 3); on a compositor without blur support, the getter
   returns `Transparent` (or `Opaque` per the surface config), not the
   requested `Blurred`.
3. Manual test on a KDE X11 session — verify the blur is visible.
4. The capability matrix comment in `platform.rs:1615` ships in Task 1.

**Files:** `crates/flui-core/src/platform/linux/x11/window.rs`.

**Logging:** `tracing::debug!` at the xprop set/clear site naming the
property and value; `tracing::info!` once on first `Blurred` request when
the compositor is not in the supported list (KDE, Deepin) — helps users
understand why blur did not appear.

**Tests:** no automated test (compositor-dependent). Manual KDE X11
verification + smoke on a non-KDE WM to confirm graceful fall-through.

**Dependencies:** Task 1, Task 7 (chrome enforcement primitives — both
touch platform/linux/x11/window.rs).

**Verification:** manual smoke on KDE Plasma X11; getter returns
`Transparent` on i3 / GNOME-on-X.

---

#### Task 17: ADR-018 `flui_core::z` module + hit-test priority audit

**ADR ref:** ADR-018 (Modal & overlay layering — `defer_draw` priority
and per-window modal scope).

**Deliverable:**
1. Audit `crates/flui-core/src/gesture/dispatch.rs` — verify the
   hit-test walk consults `defer_draw` priority before document order;
   if it walks the bounds tree in element-document order, add a
   pre-pass over `deferred_draws` sorted by descending priority
   (decision 3).
2. Publish a `flui_core::z` module with named priority constants
   `Z_TOOLTIP`, `Z_DROPDOWN`, `Z_MODAL`, `Z_DRAG_PREVIEW` using the
   ranges in ADR-018 decision 2.
3. Add a documented `modal_backdrop()` helper widget that paints a
   transparent full-window quad at priority `Z_MODAL - 1` and consumes
   pointer events — the canonical "modal blocks below" pattern.
4. Add a test that opens a modal in window A and asserts a click in
   window B still reaches its target (decision 4: per-window modality).

**Files:** `crates/flui-core/src/gesture/dispatch.rs`,
`crates/flui-core/src/z.rs` (new module),
`crates/flui-core/src/elements/modal_backdrop.rs` (new),
`crates/flui-core/src/lib.rs` (re-exports).

**Logging:** `tracing::trace!` at hit-test traversal naming the
priority of the winning overlay; `tracing::debug!` at
`modal_backdrop` mount/unmount.

**Tests:** the per-window modality test (uses two test-platform windows);
a unit test that verifies the priority-sorted hit-test pre-pass; a
doctest on `modal_backdrop` showing typical usage.

**Dependencies:** Task 1.

**Verification:** `cargo test -p flui-core`. The flui-arch-reviewer
agent reviews the new public `z` module and `modal_backdrop` widget.

---

### Phase J — Scroll physics scaffolding

#### Task 18: ADR-019 `ScrollPhysics` trait + reference implementations

**ADR ref:** ADR-019 (Scroll physics — scoping document for the future
`Scrollable` widget).

**Deliverable (types only; no `Scrollable` widget yet):**
1. Add the `ScrollPhysics` trait, `ScrollState`, `BouncingPhysics`, and
   `ClampingPhysics` to `flui-core` as published types. No consumer
   yet; the types exist to pre-empt API divergence when the widget
   arrives.
2. Wire `Theme::scroll_physics_default()` selection in `flui-theme`.
   Default is platform-conditional (`BouncingPhysics` on macOS,
   `ClampingPhysics` elsewhere) per ADR decision 2.
3. Extend `UniformListScrollHandle::scroll_to_item` with an `animated:
   bool` parameter; existing call sites pass `false`. The animated path
   delegates to a fresh `Simulation` from the current physics (composes
   with the existing `simulation.rs` spring + friction primitives).
4. Audit `crates/flui-core/src/gesture/recognizers/drag.rs` for
   axis-lock semantics on wheel/trackpad inputs; document where
   `ScrollPhysics::apply_delta` takes over (no recognizer change yet;
   this is the ADR-019 #4 audit only).

**Files:** `crates/flui-core/src/scroll/{physics,state}.rs` (new),
`crates/flui-core/src/elements/uniform_list.rs`,
`crates/flui-theme/src/lib.rs`,
`crates/flui-core/src/gesture/recognizers/drag.rs` (audit + comments only).

**Logging:** `tracing::trace!` at `apply_delta` and `fling` boundaries
naming the physics impl and the input velocity/state.

**Tests:** unit tests for each reference physics (`BouncingPhysics`
edge bounce, `ClampingPhysics` clamp at extremes); a property test
asserting `apply_delta` is monotonic in the input delta sign.

**Dependencies:** Task 1, Task 12 (raster mode + frame budget interplay
via ADR-019 decision 7), Task 13 (`renderer_kind` consumption in
`Simulation` ticks).

**Verification:** `cargo test -p flui-core`. rust-api-migration-auditor
reviews the new public trait + types.

---

### Phase K — Partial present scaffolding

#### Task 19: ADR-006 `draw_with_damage` default + wgpu implementation

**ADR ref:** ADR-006 (Partial present — design space for damage-region API).

**Deliverable:**
1. Sketch the helper signature
   `Window::collect_damage_rect(&self) -> Option<Bounds<DevicePixels>>`.
   Decide between bounds-tree union (decision B-1 from the ADR) and
   dirty-view ancestor union; the choice is documented in the commit
   message and a comment block.
2. Land the trait change as a non-breaking default on `PlatformWindow`:
   `fn draw_with_damage(&self, scene: &Scene, damage: Option<&Damage>) {
   self.draw(scene) }` — existing `draw` stays the no-damage entry.
3. Implement the wgpu path first
   (`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs`). The rest of
   the backends stay on the default until touched for another reason.
4. Add a `Damage::FULL` and `Damage::EMPTY` sentinel so decision 5 and 6
   are expressible as types, not implicit conventions.

**Files:** `crates/flui-core/src/platform.rs` (new
`PlatformWindow::draw_with_damage` default method),
`crates/flui-core/src/window.rs` (the `Window::collect_damage_rect` helper
that reads the bounds tree at `prepaint` end),
`crates/flui-core/src/scene.rs` (new types: `Damage`, `Damage::FULL`,
`Damage::EMPTY` — colocated with `PrimitiveBatch` and the new
`Pass` enum from Task 20),
`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs` (first backend to
honour the hint).

**Logging:** `tracing::debug!` at `collect_damage_rect` naming the
producer (bounds-tree vs dirty-view union) and the resulting damage
bounds; `tracing::trace!` at the wgpu surface present call naming
whether damage was applied or skipped (empty / full / partial).

**Tests:** unit test on `collect_damage_rect` with a synthetic
bounds-tree; visual-regression test confirming partial-present output
matches full-present for the same scene (pixel-equality).

**Dependencies:** Task 1, Task 5 (post-`recover()` full-present rule
from ADR-006 decision 6).

**Verification:** `cargo test -p flui-core --features test-support`;
manual smoke on a Wayland compositor that exposes damage via
`wp_presentation_feedback`. wgpu-gpu-reviewer reviews the wgpu path.

---

### Phase L — Opaque-pass + depth pipeline

#### Task 20: ADR-020 opaque-pass classifier + second wgpu pipeline

**ADR ref:** ADR-020 (Overdraw strategy — opaque-pass + depth reject as
future work).

**Deliverable:**
1. Add a `Pass` enum (`Opaque` / `Transparent`) to scene types and a
   `Scene::classify_passes()` step that runs after layout + paint
   registration, before the GPU upload. Classification rules per
   decisions 2-5 (alpha-1.0 fill, non-AA-edge interior → opaque; text,
   corner-fringe, partial-alpha → transparent; bias toward correctness).
2. Add a second wgpu pipeline descriptor with depth-stencil attached
   (`Depth32Float`, `depth_compare: Less`, `depth_write_enabled: true`),
   sharing the same shader source as the existing transparent pipeline.
3. Allocate a depth texture matching the surface size; recreate on
   resize and on device-loss (compose with ADR-005 / Task 5). Skip
   allocation on the test platform (no real GPU).
4. Add a visual-regression test that renders the `creating_components`
   example with opaque-pass on / off and asserts pixel-for-pixel
   equality (the perf win must be invisible to users).
5. Measure with a CPU/GPU profiler before and after; record the delta
   in the commit message. The frame-budget heuristic from ADR-014
   (Task 13) re-tunes on software fallback after this lands.

**Files:** `crates/flui-core/src/scene.rs` (new `Pass` enum colocated with
the `Damage` type from Task 19 and the existing `PrimitiveBatch`; new
`Scene::classify_passes()` step),
`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs` (second pipeline +
opaque-pass render pass),
`crates/flui-core/src/platform/wgpu/wgpu_context.rs` (depth-texture
allocation; recreates on resize and on device-loss in concert with the
ADR-005 `recover()` path landed in Task 5),
`crates/flui-core/tests/golden/` (visual regression — sits alongside the
existing S01b harness; no new harness).

**Logging:** `tracing::debug!` at `classify_passes` reporting opaque /
transparent counts per frame (rate-limited via sampling at 1/60); a
`tracing::info!` one-shot at first frame naming the depth format and
pipeline pair.

**Tests:** the visual-regression test; a unit test for the classifier
edge cases (rounded-corner opaque interior emits two draws per
decision 4); a Criterion benchmark recording the GPU-time delta on a
sample scene.

**Dependencies:** Task 1, Task 5 (depth-texture recreate on device-loss),
Task 19 (composes orthogonally with damage-region; both shrink GPU
load by different mechanisms).

**Verification:** `cargo test -p flui-core --features test-support`;
golden snapshots regenerated; manual GPU profiler capture. The
wgpu-gpu-reviewer + migration-risk-adversary agents review the pipeline
addition.

---

## Commit Plan

This plan has 20 tasks. Commit checkpoints every 3-5 tasks per the
plan format rule. Suggested commit groupings (each ends with green
`cargo test -p flui-core --lib`):

| Checkpoint | Tasks | Commit message (conventional commit form) |
|------------|-------|------------------------------------------|
| **C1** | 1 | `docs(adr): add // CONTRACT: blocks for ADR-002…020` |
| **C2** | 2, 3, 4 | `fix(adr-002,003,004): hover scope, source-over blend, utf-8 slicing asserts` |
| **C3** | 5, 6 | `feat(adr-005,007): device-loss gap closures + display lifecycle observers` |
| **C4** | 7, 8 | `feat(adr-008,009): WindowOptions enforcement + EditorCommand IME bridge` |
| **C5** | 9, 10 | `feat(adr-010,012): cx.tab_group public sugar + canvas rustdoc example` |
| **C6** | 11 | `feat(adr-011)!: ExternalDropPayload migration (breaking)` |
| **C7** | 12, 13 | `feat(adr-013,014): TextRasterMode + RendererKind frame budget` |
| **C8** | 14, 15 | `feat(adr-015): push_rounded_clip + chore(adr-016): wasm CI gating` |
| **C9** | 16, 17 | `feat(adr-017,018): X11/KDE blur + flui_core::z modality primitives` |
| **C10** | 18 | `feat(adr-019): ScrollPhysics trait scaffolding (types only)` |
| **C11** | 19 | `feat(adr-006): draw_with_damage trait default + wgpu damage path` |
| **C12** | 20 | `perf(adr-020): opaque-pass classifier + depth pipeline` |

Each checkpoint is reviewable independently. C6 carries a `!` for the
breaking rename (`FileDropEvent` → `ExternalDropEvent`). C11 and C12
should be flagged for the wgpu-gpu-reviewer agent. C3, C4, C6, C11, C12
should run the **pre-PR review-agent triple launch** per memory feedback
(flui-arch-reviewer + migration-risk-adversary + rust-api-migration-auditor
in one message).

## Out-of-scope (deferred to future plans)

These items are recognised in the ADRs but explicitly **not** in this
plan's scope:

- The `Scrollable` widget itself (ADR-019 says scaffolding only).
- The custom-shader element (ADR-012 "reading B").
- Per-region partial blur on X11 (`PartialBlur { region }`, ADR-017).
- Focus-trap inside modals + RTL focus traversal (ADR-018, ADR-010
  cross-cutters).
- Wayland portable blur protocol (no protocol exists; ADR-017).
- Compositor-level hidden-surface culling beyond the opaque-pass
  classifier (ADR-020 out-of-scope).
- Tooltip helper view-id refactor (ADR-002 explicit deferral).
- Software-renderer fallback specifically for headless / CI (ADR-014
  out-of-scope; covered by S01b harness for the present).
- Per-display rasterization mode (ADR-013 out-of-scope).

Any of these earns its own ADR + plan when a concrete user materialises.

## Next Steps

1. Run `/aif-implement` against this plan (manual or autonomous mode).
2. Per checkpoint C3, C4, C6, C11, C12, dispatch the pre-PR
   review-agent triple in one message (flui-arch-reviewer +
   migration-risk-adversary + rust-api-migration-auditor).
3. After each checkpoint, the documentation pass via `/aif-docs` folds
   the ADR contract notes into the public rustdoc.
4. After Task 20 completes, run a full Criterion benchmark sweep and
   record the perf delta against the pre-rollout baseline; feed the
   result into the P1 frame-budget instrumentation track.
