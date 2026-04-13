---
title: flui-core roadmap — platform extraction + Flutter-level feature parity
date: 2026-04-13
status: approved
type: roadmap
spec_id: ROADMAP
---

# flui-core Roadmap

Master index for the multi-phase effort to (1) extract all platform code from
`flui-core` into a dedicated `flui-platform` crate without losing any existing
functionality, (2) fill the Flutter-level gaps in core subsystems
(gestures, semantics, canvas, filters, physics, text, media query, assets),
and (3) add new platform embeddings (iOS, Android, web rendering, headless).

Widgets (`flui-widgets`, `flui-material`, etc.) are **out of scope** until the
roadmap is complete.

## 1. Current state (snapshot 2026-04-13)

### What `flui-core` contains today

- GPUI-derived runtime: `App`, `Entity`, `Context`, `Window`, `Element`, `View`
- Painting primitives: `scene`, `path_builder`, `style`, `taffy`, `text_system`
- Runtime: `scheduler`, `executor`, `queue`, `animation` (including
  `AnimationController`)
- Input pipeline: `input`, `interactive`, `key_dispatch`, `keymap`, `tab_stop`
- Platforms inside core at `crates/flui-core/src/platform/`:
  `mac` (Metal), `linux` (x11 + wayland), `windows` (Direct3D 11),
  `web` (skeleton), `wgpu` (Linux/FreeBSD shared backend), `test`, `visual_test`
- Locale / brightness / assets / media_query / a11y trait skeleton

### What the old `../flui` (v1) contributed as lessons

- Multi-crate layering (`flui-foundation`, `flui-engine`, `flui-rendering`,
  `flui-painting`, `flui-tree`, `flui-semantics`, `flui-layer`, `flui-platform`,
  `flui-interaction`, `flui-scheduler`, `flui-reactivity`, `flui-view`,
  `flui-animation`) proved too complex and was abandoned.
- **Lesson:** do not replicate Flutter's *internal* architecture on top of
  GPUI. Replicate Flutter's **feature surface** and user-visible API concepts,
  but keep the single-level engine that GPUI already provides.
- The v1 `flui-platform` layout (single crate that aggregated
  `android/ios/linux/macos/web/windows/headless/winit`) is a good template for
  the new flui-platform crate in v2.

### Scope of code that will move

Deep analysis (see `## 3. Migration strategy`) reports:

- **42,936 LoC across 80 files** under `crates/flui-core/src/platform/**`
- Per-platform breakdown:
  - `mac/` — 8,889 LoC
  - `windows/` — 10,439 LoC
  - `linux/` — 11,657 LoC (x11 + wayland + headless stub)
  - `wgpu/` — 2,876 LoC (shared renderer for Linux/FreeBSD)
  - `web/` — 1,849 LoC
  - `test/` + `visual_test.rs` — 1,350 LoC
  - top-level (`keystroke`, `keyboard`, `app_menu`, `layer_shell`,
    `scap_screen_capture`) — 1,308 LoC

- **24 `unimplemented!()` sites across 5 files** (verified via grep, not the 72
  figure that earlier drafts cited): `platform/test/platform.rs` (12),
  `platform/test/window.rs` (8), `platform/windows/platform.rs` (2),
  `platform/windows/window.rs` (1), `platform/mac/metal_atlas.rs` (1).
- **0 `todo!()` sites** inside `platform/**`.
- **12 `unreachable!()` sites** that need separate classification (some are
  legitimate invariant guards, e.g. wayland protocol fallbacks; at least one is
  a trap-in-waiting, e.g. `platform/mac/platform.rs:1020 CursorStyle::None =>
  unreachable!()`). These are **not** in the same category as the
  `unimplemented!()` stubs and S01a must classify them distinctly.
- **~26 comment-level `// TODO` markers** scattered through platform code. These
  are intent-to-fix notes, not runtime stubs.
- The inventory produced in S01a supersedes earlier numeric claims.

## 2. Gap analysis vs Flutter (A–J)

Assessment of the distance from Flutter's user-visible feature set. Sized
"small / medium / large" relative to the effort inside `flui-core`.

| Cat | Area | Gap |
|---|---|---|
| **A** | Platforms (embedding) | iOS — full; Android — full; Web — large (skeleton only); Headless — full; Windows/Linux/macOS — small (TODO cleanup) |
| **B** | Input & gestures | GestureArena with competing recognizers — medium; full Focus tree / traversal — small; MouseRegion — small |
| **C** | Rendering & painting | Unified Canvas facade — medium; ImageFilter / ColorFilter / BackdropFilter — medium; SaveLayer offscreen — medium |
| **D** | Text | StrutStyle / FontFeatures / FontVariations / selection rendering / IME composition — medium |
| **E** | Animation & scheduler | Ticker — already there; physics simulations (Spring/Friction/Gravity) — medium |
| **F** | Accessibility / semantics | SemanticsNode tree + SemanticsOwner + actions + protocol hooks in core — large |
| **G** | Scheduling / async | Priority queue and frame-budget gaps — small |
| **H** | MediaQuery / localization / SystemChrome | Accessibility flags, gestureSettings, SystemChrome — medium |
| **I** | Assets / IO | Resolution-aware bundle variants — small |
| **J** | Test infra | WidgetTester / golden test harness / headless renderer — large |

Non-goals (explicitly **not** in this roadmap):

- Dart VM / platform channels. We are native-only.
- Replicating Flutter's internal layer tree — GPUI's scene already solves this.
- Widgets, routing, theming, material — all gated on completing this roadmap.
- DevTools / inspector / performance overlay.

## 3. Crate layout decision

Current structure keeps all platform code inside `flui-core`. It will move to
a single new sibling crate, **`flui-platform`**, following the v1 template.

Rationale:

1. Keeps the number of new crates to one (not a zoo of
   `flui-core-ios`, `flui-core-android`, `flui-core-headless`).
2. Allows one place to look for "how a platform is implemented".
3. Avoids layering / multi-level architecture that sank v1: the platform trait
   contract stays in `flui-core`, implementations live in `flui-platform`,
   dependency flows one-way (`flui-platform -> flui-core`).
4. Feature flags (`macos`, `windows`, `linux`, `wayland`, `x11`, `web`, `ios`,
   `android`, `headless`, `test-support`) select which backends compile.

### Final workspace shape after roadmap

```
crates/
├── flui-core             # runtime, traits, scene, text, scheduler, input
├── flui-platform         # NEW: all platform embeddings under one roof
├── flui-macros
├── flui-animate          # untouched by this roadmap
├── flui-navigator        # untouched
├── flui-a11y             # untouched (consumes semantics API from core)
├── flui-theme            # untouched
├── flui-material         # untouched
└── flui-widgets          # untouched — gated on roadmap completion
```

Not in this roadmap (deferred until after widgets land): `flui-cli`,
`flui-build`, `flui-test`, `flui-golden`, `flui-devtools`.

## 4. Migration strategy — "Lock-First, Step-by-Step"

The primary constraint is **no functionality loss during migration**. A big-bang
extraction of 42k LoC across native toolchains is rejected. So is a parallel
"new in flui-platform, old in flui-core" dual-track. So is the asymmetric
"desktop stays in core, new platforms go to flui-platform" approach.

The chosen strategy:

1. **Lock behavior before moving code** — write pinning tests (golden rendering,
   event dispatch, scheduler determinism, shader ABI size, example smoke)
   against the pre-extraction tree. These tests must stay green at every step.
2. **Stabilize the public API surface** that platform code needs *before*
   extracting. The current coupling includes several private types from core
   (e.g. `crate::window::WebWindowInner`, `pub(crate) scheduler::TestScheduler`,
   Scene primitive iteration) — these become `pub` in core as part of the lock
   step.
3. **Move one vertical slice per commit**, in dependency order, running the
   lock tests after each slice. Any red = rollback, diagnose, retry.

### Pre-migration API stabilization (applied in S01a and S01d)

Verified against the tree — the earlier draft of this list was ground-truth
wrong on six of seven items. The real work:

**Already `pub` today, no promotion needed** (removed from scope):

- `Scene::batches()` — already `pub fn` at `scene.rs:158`.
- `PrimitiveBatch` and its variants — already `pub enum` at `scene.rs:463`.
  Opportunistic change: add `#[non_exhaustive]` (source-compat, no layout
  impact).
- `scheduler::{Clock, TestScheduler, SessionId}` — already `pub` at
  `scheduler/clock.rs:8`, `scheduler/test_scheduler.rs:34`,
  `scheduler/mod.rs:121`.
- `command::{new_command, new_std_command}` — already `pub` at
  `local_util.rs:114` and reachable via `pub use local_util::{command, ...}`
  at `lib.rs:114`.
- Core Scene primitives (`Quad`, `Shadow`, `Underline`, `MonochromeSprite`,
  `PolychromeSprite`, `SubpixelSprite`, `PathVertex`) — already `pub` and
  already `#[repr(C)]` at `scene.rs:485+`. Layout is already frozen on the
  public semver surface; no new module needed.

**Real work for S01a (ground truth and hygiene):**

- Convert `pub use platform::*;` at `lib.rs:117` to an **explicit re-export
  list**. Today's glob silently leaks every new `pub` item in `platform.rs`
  to the crate root with no review step. This is a prerequisite for any later
  spec that adds public items in the platform subtree.
- Fix the `screen-capture` feature landmine: `platform.rs:31-44` gate code on
  `feature = "screen-capture"` but `Cargo.toml` does not declare the feature.
  Either declare it (`screen-capture = ["dep:scap"]`) or delete the dead code.
- Add `#[non_exhaustive]` to `PrimitiveBatch`.
- Enumerate every `use crate::*;` and `use crate::<mod>::*;` inside
  `crates/flui-core/src/platform/**` (at least 6 sites: `direct_write.rs:26`,
  `directx_renderer.rs:23`, `windows/{window,util,events,platform}.rs`). For
  each glob, list the concrete symbols used so the extraction in S02+ can
  verify each survives cross-crate.
- Verify debug-mode Windows build currently compiles: the
  `include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"))` at
  `directx_renderer.rs:1750` is unconditional, but the file is generated only
  by release builds (`build.rs:184 #[cfg(not(debug_assertions))]`). This may
  be silently broken today and S01a must record the status.
- Commit a snapshot of the cbindgen-generated `scene.h` as a fixture.
  Subsequent specs diff against it to catch silent ABI drift during the mac
  migration in S04.
- Decide `test-support` CI posture: currently `ci.yml` runs `cargo test
  --workspace` without `--features test-support`, so any test gated on that
  feature is skipped. S01a either enables it in CI or explicitly marks it as
  deferred.
- Install `mesa-vulkan-drivers` (lavapipe) on the Linux CI runner so
  `wgpu` can actually execute in software without a display. Without this the
  golden tests in S01b will panic at adapter selection time on
  `ubuntu-latest`.

**Real work for S01d (facades for extraction):**

- `WebWindowInner` — keep `pub(crate)` in its real location at
  `platform/web/window.rs:45` (the `crate::window::WebWindowInner` path cited
  by the earlier deep analysis does not exist). Introduce a
  `#[doc(hidden)] pub mod __platform_internals` facade exposing only the
  callback-registration accessors that `platform/web/events.rs` needs. Do NOT
  promote the raw type — it holds `Rc<RefCell<…>>` interior state whose
  auto-traits should not become part of the semver surface.
- `PlatformScreenCaptureFrame` — wrap in an opaque newtype rather than
  promoting the target-gated type alias directly. Three different concrete
  types across target configurations would otherwise become part of the
  public API.
- Decide the platform submodule visibility strategy for extraction: keep
  `pub(crate) mod {mac,linux,windows,wgpu,web}` and expose via an explicit
  re-export list, or promote the submodules and rely on them being the public
  integration surface. Document the choice so S02 can implement it without
  re-opening the discussion.

**Not in S01 at all** (removed from scope — these were errors in the earlier
draft):

- There is no `Uniforms` struct in flui-core to stabilize. The cbindgen export
  list at `build.rs:60` mentions the name, but no Rust source defines it; it
  is likely a Metal-shader-side construct. Shader-visible structs that DO
  exist (`GlobalParams`, `GammaParams`, `SurfaceParams`, `PathSprite`,
  `PathRasterizationVertex`) are private to `wgpu_renderer.rs:16-69`. If a
  shared shader ABI module is needed, it is a decision for S04 when mac
  migration actually needs cross-crate cbindgen.
- No `skip-shader-build` feature. It was proposed to skip FXC on Windows CI,
  but `build.rs:184` already gates `compile_shaders()` on
  `#[cfg(not(debug_assertions))]` — `cargo check` in the default dev profile
  already skips FXC. The feature would be solving a non-problem.
- No new `WgpuRenderContext` type that duplicates the existing `WgpuContext`
  at `platform/wgpu/wgpu_context.rs:8`. The real refactor (in S01b) is to
  lift the pipeline cache out of the per-renderer `WgpuResources` struct and
  into either `WgpuContext` itself or a sibling `WgpuPipelineCache`.

### Behavior-pinning tests (applied across S01b and S01c)

**S01b — rendering locks:**

1. **Golden rendering on macOS** — `MetalHeadlessRenderer` (already exists at
   `platform/mac/metal_renderer.rs:1683`) against a reference PNG set. Mac CI.
2. **Golden rendering on Linux** — new `WgpuHeadlessRenderer` (introduced in
   S01b) against a reference PNG set. Linux CI. Uses lavapipe in software.
   Offscreen texture format locked to `Bgra8Unorm` with B/R channel swap on
   readback to match the surface path's pipeline cache exactly.
3. **Shader ABI size assertions** — `const` assertions on
   `std::mem::size_of::<Quad>()`, `Shadow`, `Underline`, `MonochromeSprite`,
   `PolychromeSprite` against frozen expected values. Captured once on a
   known-good target triple, recorded with the triple for which they are
   valid.

**S01c — input, lifecycle, example locks:**

4. **PlatformWindow event dispatch chain** — synthetic input per variant
   (MouseDown, MouseUp, MouseMove, MouseExit, KeyDown, KeyUp,
   ModifiersChanged, ScrollWheel, FileDrop) → `on_input` callback receives it
   with the correct modifier state. One test per variant, not a blanket
   single test.
5. **Focus / tab-stop traversal smoke test** — at least one positive and one
   negative case per platform.
6. **Keyboard layout round-trip** — synthetic scancode → `Keystroke` matches
   expected across platforms.
7. **Clipboard read/write smoke test** — write a known string, read it back,
   verify identity. Includes primary selection on Linux and find-pasteboard on
   macOS.
8. **Window lifecycle smoke test** — `minimize`, `maximize`,
   `toggle_fullscreen`, `is_maximized`, `close` each exercised once per
   platform; regression catches API changes.
9. **TestDispatcher determinism** — verify gaps relative to existing tests in
   `scheduler/tests.rs` (which already contains a 1000-iteration `test_many`
   pattern). Add only what is missing, not a duplicate.
10. **Real example smoke** — run a named, fixed subset of `examples/legacy/*`
    (at least `hello_world`, `window`, `window_shadow`, `opacity`,
    `tab_stop`). Verify window opened and first frame rendered via a
    test-platform hook — not just exit code 0 (panics during shutdown can
    still exit 0 on some platforms). `layer_shell` on Linux is skipped in CI
    because GitHub runners lack a Wayland compositor; note as a known gap.

### Expected-stub inventory (applied in S01a)

A file `docs/platform-expected-stubs.md` classifies every runtime-panic
annotation inside `crates/flui-core/src/platform/**`:

- **24 `unimplemented!()` sites** (the real count, verified via grep):
  `platform/test/platform.rs` 12 sites, `platform/test/window.rs` 8 sites,
  `platform/windows/platform.rs` 2 sites, `platform/windows/window.rs` 1 site,
  `platform/mac/metal_atlas.rs` 1 site. These are "expected stubs" — test-only
  or platform-inapplicable (Windows has no dock, mac metal_atlas has a rare
  unsupported texture format).
- **12 `unreachable!()` sites** classified separately: some are legitimate
  protocol-level invariants (wayland fallback arms in
  `linux/wayland/client.rs`, directx adapter enumeration fallbacks in
  `windows/directx_devices.rs`), one is a trap waiting to fire
  (`platform/mac/platform.rs:1020 CursorStyle::None => unreachable!()` — any
  refactor that sends `CursorStyle::None` into the mac platform will panic in
  production). The inventory documents each one's intent.
- **~26 comment-level `// TODO`** markers: these are intent-to-fix notes, not
  runtime-affecting. They are listed but not guarded by the CI check.

The CI grep check matches `unimplemented!(` and `unreachable!(` patterns (not
`todo!` — there are none) against a stable path-only index, not path:line, so
unrelated PRs that shift line numbers do not false-positive. Any new site that
isn't in the inventory fails CI. Updating the inventory requires touching the
docs file in the same commit as the code.

### Step-by-step migration order

- **Step 0 — Lock.** Split into four atomic sub-steps, each one a single
  rollback-able commit:
  - **S01a — Ground truth & cleanup.** Correct the stub inventory (24 real
    sites, not the fabricated 72), fix the `screen-capture` feature landmine,
    convert `pub use platform::*;` to an explicit re-export list, add
    `#[non_exhaustive]` to `PrimitiveBatch`, enumerate `use crate::*;` globs in
    platform code, verify or repair debug-mode Windows compilation, commit
    cbindgen `scene.h` snapshot as fixture, decide `test-support` CI posture,
    install lavapipe on Linux CI. Zero new runtime code; zero refactors.
  - **S01b — WgpuHeadlessRenderer + golden infra.** Add
    `WgpuContext::new_headless()` that skips the surface-capabilities probe,
    lift the pipeline cache out of per-renderer `WgpuResources` into the
    shared context, introduce `WgpuHeadlessRenderer` reusing it. Lock
    offscreen format to `Bgra8Unorm` + B/R swap to match the surface path's
    pipeline cache. Explicit `BlendState::PREMULTIPLIED_ALPHA_BLENDING` in
    pipeline creation instead of deriving from `CompositeAlphaMode`. Correct
    readback sync sequence (submit → map_async callback → device.poll(Wait) →
    block_on(rx)). Row-stride math with `COPY_BYTES_PER_ROW_ALIGNMENT = 256`.
    Golden test harness that unsets `ZED_FONTS_GAMMA`,
    `ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST`,
    `ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST` before rendering. Reuse the
    existing `platform/visual_test.rs` infrastructure rather than writing a
    parallel one. Mac + Linux golden suites.
  - **S01c — Behavior pinning (non-rendering).** Event dispatch per input
    variant, focus/tab-stop smoke, keyboard layout, clipboard, window
    lifecycle, example smoke with real window-open detection. Pure test
    additions; no production-code changes.
  - **S01d — Extraction facades.** `#[doc(hidden)]` facade for
    `WebWindowInner` (keep `pub(crate)`, expose a callback accessor).
    Opaque newtype wrapper for `PlatformScreenCaptureFrame`. Decide and
    implement the platform submodule visibility strategy (explicit re-exports
    vs promoting `pub mod`). Prep for S02 to consume.

  Each S01 sub-step is a single commit. All four must land (and their tests
  must remain green) before S02 begins.

- **Step 1 — flui-platform skeleton + test platform.** (spec S02) Create the
  crate, move `test/` and `visual_test.rs` only. Smallest and safest because
  it has no native deps.
- **Step 2 — wgpu + linux.** (spec S03) Move `wgpu/` and `linux/{x11,wayland,
  headless}` together because wgpu is the Linux renderer. Carry target-deps
  and the naga build.rs logic across.
- **Step 3 — macOS.** (spec S04) Move `mac/` and the cbindgen + Metal shader
  compilation pipeline. Build.rs now depends on `flui-core` for
  `shader_abi` struct definitions — this is the first cross-crate cbindgen
  setup and is the highest technical risk in the migration.
- **Step 4 — Windows.** (spec S05) Move `windows/` and the FXC.exe +
  embed-resource build.rs. Requires Windows SDK in CI.
- **Step 5 — Web + top-level files.** (spec S06) Move `web/`, `keystroke.rs`,
  `keyboard.rs`, `app_menu.rs`, `layer_shell.rs`, `scap_screen_capture.rs`.
  After this the `flui-core/src/platform/` directory is deleted; `flui-core`
  re-exports from `flui-platform`.

### Rejected strategies

- **Big bang.** Highest chance of silent functionality loss; rollback is
  impractical.
- **Parallel dual-track.** Creates duplicated trait paths, confuses IDEs and
  clippy, and still has the same rollback problem at cut-over.
- **Asymmetric (desktop stays in core).** Permanent technical debt; new
  contributors won't know where a platform lives.

## 5. Ordered spec list (master index)

Each row below corresponds to a standalone design document that will live
alongside this roadmap in `docs/superpowers/specs/`. Specs are written in the
order listed; each can be brainstormed and approved independently.

### Phase I — Migration (blocks everything else)

| Spec | Title | Depends on | Summary |
|---|---|---|---|
| **S01a.1** | `lock-inventory-and-hygiene` | — | Move stub inventory + `use crate::*;` import survey into `tooling/xtask` as `check-stubs` / `check-platform-imports` subcommands (follow existing `package_conformity` pattern). Add `.gitattributes` rule `docs/fixtures/*.h text eol=lf`. Benchmark `cargo test --features test-support` runtime delta on Linux + mac and decide whether to flip the CI default. No new runtime code. |
| **S01a.2** | `delete-screen-capture-dead-code` | S01a.1 | Delete the `screen-capture` feature and all its references. The feature is declared nowhere in `Cargo.toml` but referenced at `platform.rs:31-44`, `windows/platform.rs:488-500`, `linux/x11/client.rs:1498-1504`, `mac/screen_capture.rs`, `scap_screen_capture.rs`. Delete the dead branches, collapse `PlatformScreenCaptureFrame` to `()`, keep `ScreenCaptureSource`/`ScreenCaptureStream`/`ScreenCaptureFrame` traits as future extension points. Verify all workspace siblings still build. |
| **S01a.3** | `explicit-re-export-list` | S01a.1 | Replace `lib.rs:117 pub use platform::*;` with an enumerated `~95-100` symbol list grouped by `#[cfg]` predicate. Preserve every target gate exactly. Demote macro-support items (`PlatformDispatcher`, `RunnableVariant`, `TimerResolutionGuard`, `RunnableMeta`) to `#[doc(hidden)] pub use`. Verification: `cargo doc --no-deps` before/after diff and `cargo check` across all workspace siblings (esp. `flui-navigator` which does `use flui_core::*;`) on Linux + mac. Windows verification deferred to after S01a.4. Rationale comment: this is the only glob fixed now because `platform::*` is the prerequisite for S02; the ~29 other globs at `lib.rs` stay. |
| **S01a.4** | `fix-debug-windows-build` | S01a.1 | Repair the debug-mode Windows compilation, currently broken with 257 errors. Root causes: missing `Win32_Media` feature on the `windows` crate dep, `windows::core::w!` path resolution, `use crate::*;` globs in `platform/windows/{direct_write, directx_renderer, events, platform, util, window}.rs` failing to resolve (`DirectXDevices`, `DirectXAtlas`, `WindowsWindowInner`, `HWND`, `SafeHwnd`, `logical_point`, `with_dll_library`, `WM_GPUI_*` constants). Add `Win32_Media` to the `windows` dep features; replace the globs with explicit imports per file. Verify: `cargo build -p flui-core` on a Windows machine exits 0. Does NOT block S01b or S01c. Blocks S02. |
| **S01b** | `lock-wgpu-headless-and-golden` | S01a.1 | `WgpuContext::new_headless`, pipeline cache lift, `WgpuHeadlessRenderer`, `Bgra8Unorm` lock, correct readback pattern, golden harness, mac + Linux golden suites. |
| **S01c** | `lock-behavior-non-rendering` | S01a.1 | Event dispatch per variant, focus/tab-stop smoke, keyboard layout, clipboard, window lifecycle, real example smoke. Pure test additions. |
| **S01d** | `lock-extraction-facades` | S01a.3 | `WebWindowInner` `#[doc(hidden)]` facade, `PlatformScreenCaptureFrame` opaque newtype, submodule visibility strategy for extraction. |
| **S02** | `flui-platform-crate-skeleton` | S01a.1, S01a.2, S01a.3, S01a.4, S01b, S01c, S01d | Create `crates/flui-platform`, move `test/` + `visual_test.rs`, set up re-exports for backwards compatibility. |
| **S03** | `platform-migration-wgpu-linux` | S02 | Move `wgpu/` + `linux/{x11,wayland,headless}` + Linux target-deps + naga build.rs. |
| **S04** | `platform-migration-macos` | S03 | Move `mac/` + cbindgen cross-crate setup + Metal shader build.rs. |
| **S05** | `platform-migration-windows` | S04 | Move `windows/` + FXC shader compilation + embed-resource build.rs. |
| **S06** | `platform-migration-web` | S05 | Move `web/` + top-level files (keystroke, keyboard, app_menu, layer_shell, scap_screen_capture). Delete `flui-core/src/platform/`. |

### Phase II — New core subsystems

| Spec | Title | Gap | Summary |
|---|---|---|---|
| **S07** | `gesture-arena` | B | `GestureArena` with competing recognizers (tap, double-tap, long-press, drag, scale, horizontal/vertical drag), hit-test protocol. |
| **S08** | `semantics-protocol` | F | `SemanticsNode` tree, `SemanticsOwner`, actions (tap/scroll/dismiss/increase/decrease), roles/hints/labels, hooks in Element and Window so that `flui-a11y` can plug in. |
| **S09** | `canvas-facade` | C | Unified `Canvas` API over existing `scene` + `path_builder`, including `saveLayer`, clips, transforms, blend modes. |
| **S10** | `image-filters` | C | `ImageFilter` (blur, matrix), `ColorFilter`, `BackdropFilter`, `MaskFilter` with shader support. Depends on S09. |
| **S11** | `physics-simulations` | E | `Spring`, `Friction`, `Gravity`, `ScrollPhysics` integrated with `AnimationController`. |
| **S12** | `focus-traversal` | B | Directional traversal, `FocusTraversalPolicy`, `FocusScope` groups on top of existing `tab_stop`. |
| **S13** | `text-parity` | D | `StrutStyle`, `TextDecoration`, `FontFeatures`, `FontVariations`, selection rendering, IME composition preview. |
| **S14** | `media-query-complete` | H | Accessibility flags (highContrast, disableAnimations, accessibleNavigation), gestureSettings, SystemChrome (orientation, overlays). |
| **S15** | `asset-bundle` | I | Resolution-aware variants, locale variants, structured manifest format. |

### Phase III — New platforms

| Spec | Title | Gap | Summary |
|---|---|---|---|
| **S16** | `platform-headless-renderer` | A | Cross-platform wgpu-offscreen backend, reusable golden-test infrastructure in `flui-platform`. |
| **S17** | `platform-ios` | A | UIKit + Metal (reuses `mac/metal_renderer` via shared module) + IMKit + UIAccessibility. |
| **S18** | `platform-android` | A | JNI/NDK Surface + Choreographer + InputMethod + AccessibilityNodeProvider. |
| **S19** | `platform-web-rendering` | A | Web/WASM: wgpu → WebGPU/WebGL2, canvas integration, IME, clipboard API, fetch-based assets. |
| **S20** | `platform-gaps-cleanup` | A | Close remaining TODOs on Windows/Linux/macOS (IME edges, fractional scaling, wayland session lock, etc.), cross-check against S01 inventory. |

### Dependency graph

```
         ┌─ S01a.2 ─┐
         ├─ S01a.3 ─┼─ S01d ─┐
S01a.1 ──┼─ S01a.4 ─┤         │
         ├─ S01b ───┤         │
         └─ S01c ───┴─────────┴─ S02 ─ S03 ─ S04 ─ S05 ─ S06 ─┬─ S07..S15 (parallelizable)
                                                               │
                                                               └─ S16 ─ (S17, S18, S19 parallel) ─ S20
```

S01a was split after adversarial review revealed that the initial "single
lock spec" bundled nine unrelated hygiene tasks plus a rendering refactor
plus a 257-error Windows repair. The four S01a.x sub-specs are each a single
reviewable PR:

- **S01a.1** lays down infrastructure (xtask subcommands, `.gitattributes`,
  test-support benchmark).
- **S01a.2** deletes the dead `screen-capture` code path (it was never
  reachable — the feature is referenced but undeclared).
- **S01a.3** replaces the `pub use platform::*;` glob with an explicit list
  — the only API-visible change in the S01a family.
- **S01a.4** repairs the currently-broken debug-mode Windows build (257
  errors, verified locally). Does not block S01b/S01c but blocks S02.

S01b, S01c and S01d then depend on S01a.1 (S01d additionally on S01a.3 for
the re-export list it extends). All must be green before S02.

Notes:

- Inside Phase II, most specs are independent. Exceptions: **S10 depends on
  S09** (filters sit on top of Canvas), and **S08 should land before S17/S18**
  so that mobile platforms have a place to plug accessibility into.
- Phase III specs S17, S18, S19 are independent once S16 provides the headless
  renderer baseline.

## 6. Spec document format

Each spec S## is its own file at
`docs/superpowers/specs/<date>-<spec-id>-<slug>-design.md` with the following
sections:

- **Context** — why, links back to this roadmap row.
- **Goals** — 3–5 concrete, verifiable bullet points.
- **Non-goals** — explicit anti-scope.
- **Current state** — `file:line` references, LoC/file list for migrations.
- **Design** — modules, traits, types, data flow (for subsystems); structural
  diff + API change list (for migrations).
- **API surface** — public types / functions introduced; breaking changes
  flagged.
- **Migration / Compatibility** — what breaks, what re-exports stay for
  backwards compatibility.
- **Testing strategy** — specific tests that gate completion: golden / unit /
  integration / example smoke.
- **Open questions** — resolved when writing the implementation plan.
- **Done criteria** — checklist for that spec.

## 7. Done criteria

### Phase I done when

1. `flui-core/src/platform/` does not exist.
2. `crates/flui-platform/` contains 100% of the previously-existing platform
   functionality.
3. All golden tests introduced in S01 are green on macOS, Windows, and Linux.
4. All `crates/flui-core/examples/legacy/*` examples build and run (smoke check
   — exit code 0).
5. The expected-stub inventory from S01 matches the actual `unimplemented!()`
   sites in the tree — no silent regressions.
6. CI is green on all three desktop platforms.
7. `cargo build -p flui-core` and `cargo build -p flui-platform` compile
   independently.

### Phase II (per-spec)

1. Public API documented with rustdoc and at least one runnable example.
2. Unit tests cover the core logic.
3. Gap-analysis row in section 2 of this roadmap is marked "done".
4. S01 lock tests remain green.

### Phase III (per-platform)

1. A minimal hello-world equivalent runs on the target platform.
2. `PlatformWindow::on_input` receives touch/mouse/keyboard correctly.
3. Rendering golden test "quad + shadow + text" matches reference output.
4. Semantics: basic actions (tap, scroll) reach the platform a11y API.
5. IME composition range renders correctly.
6. Stub inventory updated — any temporary `unimplemented!()` is documented.

## 8. Risks (updated after adversarial review of the draft S01)

1. **Scene layout is already frozen on the semver surface.** `Quad`, `Shadow`,
   `Underline`, `MonochromeSprite`, `PolychromeSprite`, `SubpixelSprite`,
   `PathVertex` are already `pub` and already `#[repr(C)]`, and they are
   fields of `pub` wrappers. Any future field reorder is both a breaking API
   change and a breaking shader change (via mac cbindgen-generated
   `scene.h`). Mitigation: S01a commits the cbindgen output as a diffable
   fixture; S01b golden tests catch any rendering-level drift.
2. **Web platform reaches private `WebWindowInner`.** The earlier draft cited
   a wrong module path (`crate::window::WebWindowInner`); the actual type is
   at `platform/web/window.rs:45`. The entire `platform/web/` subtree is gated
   on `target_family = "wasm"` and is never compiled in CI today — which
   means there is currently NO compile gate for any change to this code.
   Mitigation: S01d introduces a `#[doc(hidden)]` facade; S01a or S01c adds
   a `wasm32-unknown-unknown` build job to CI.
3. **WgpuRenderer refactor has no pre-existing regression net.** S01b both
   introduces the golden tests and refactors the wgpu pipeline cache. If the
   refactor itself introduces a drift, there is no earlier baseline to catch
   it. Mitigation: S01b splits into two commits — commit A ("lift pipeline
   cache, keep surface path") followed by commit B ("add headless variant and
   golden tests"). Golden PNGs are captured from commit A's unchanged surface
   output and then replayed against commit B's output. Any drift between the
   two is a refactor bug.
4. **Ubuntu-latest CI has no display and no GPU.** The Linux golden suite
   must run through lavapipe (software Vulkan). `ubuntu-latest` runners do
   NOT have `mesa-vulkan-drivers` installed by default. Mitigation: S01a adds
   the install step. Golden PNGs are captured on lavapipe and replayed on
   lavapipe — never mixed with hardware captures.
5. **`test-support` feature is not currently enabled in CI.** `cargo test
   --workspace` without `--features test-support` silently skips every test
   gated on that feature, including `PlatformHeadlessRenderer`-based tests.
   Mitigation: S01a's ground-truth work decides whether CI enables it and
   commits the choice to `ci.yml`.
6. **macOS cbindgen cross-crate.** Metal shader struct definitions are
   extracted from `flui-core` types during the `flui-platform` build. This is
   the first cross-crate cbindgen setup and is the most likely step to
   require iteration. Mitigation: S01a commits the current cbindgen output as
   a fixture; S04 diffs the post-extraction output against it.
7. **Debug-mode Windows build may already be broken.** The
   `include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"))` at
   `directx_renderer.rs:1750` is unconditional, but `build.rs:184` only runs
   FXC under `#[cfg(not(debug_assertions))]`. If nothing generates the file
   in debug, the build fails. Nobody has tested this recently because Windows
   is not in CI. Mitigation: S01a verifies the current state on a
   `windows-latest` runner and either repairs or documents it before adding
   any Windows CI entry.
8. **`screen-capture` feature is declared nowhere.** `platform.rs:31-44` gate
   code on `feature = "screen-capture"`, but `Cargo.toml` does not declare
   the feature — making the whole screen-capture path dead code today. Any
   claim that migration "preserves screen-capture functionality" is
   preserving a broken state. Mitigation: S01a fixes or removes.
9. **Blanket `pub use platform::*;` re-export.** `lib.rs:117` leaks every new
   `pub` item in `platform.rs` to the crate root with no review step.
   Mitigation: S01a converts to an explicit re-export list as a prerequisite
   for any spec that adds public items in the platform subtree.
10. **Scheduler determinism test may already exist.** `scheduler/tests.rs`
    already contains a 1000-iteration test pattern. Mitigation: S01c first
    surveys existing tests and adds only what is missing; duplicate coverage
    is rejected.
11. **Adapter selection is surface-coupled** at
    `wgpu_context.rs:181-289`. The headless path needs a new
    `WgpuContext::new_headless` that skips the surface-capabilities probe;
    the design estimate for S01b must include this code (~100 LoC) or the
    split is infeasible. Mitigation: called out explicitly in S01b scope.
12. **Windows FXC.exe environment.** Shader compilation relies on the Windows
    SDK. Mitigation: Windows in CI is deferred to S05; S01a only verifies
    debug-mode compilation (which already bypasses FXC), not release
    compilation.

## 9. Open questions (resolved per-spec)

- **S08 semantics protocol shape** — how deep does the SemanticsNode tree
  integrate with the Element tree? Snapshot vs live?
- **S11 physics** — include in core now, or defer until the scroll widget
  needs it? Current plan: include, documented open in S11.
- **S14 SystemChrome** — keep in `flui-core` or spin off as
  `flui-system-chrome`? Current plan: keep in core.
- **S17/S18 mobile shaders** — reuse Metal from mac for iOS; need to decide
  whether Android uses the wgpu renderer or a native GLES path. Resolved in
  S17/S18.
