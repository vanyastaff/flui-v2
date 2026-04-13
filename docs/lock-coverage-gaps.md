# Lock-phase coverage gaps

Behaviors and platforms that the S01 lock phase does NOT pin. Future
specs listed against each gap own the fix.

This file is the canonical list of "what's not locked" so that new
contributors can audit what silent regressions are possible.

## Golden rendering via `WgpuHeadlessRenderer` (S01b follow-up)

**Status:** skeleton only.

S01b landed `WgpuContext::new_headless()` (fully implemented) and
`WgpuHeadlessRenderer` (struct + trait impl + `current_headless_renderer()`
wiring on Linux/FreeBSD). The constructor performs real wgpu adapter
selection and device creation, so "can we even create a headless
device on lavapipe" is exercised every test run.

But the `render_scene_to_image` method currently returns a solid-black
placeholder `RgbaImage`. The full pixel-accurate body — bind group
allocation, pipeline construction with explicit
`BlendState::PREMULTIPLIED_ALPHA_BLENDING`, instance buffer
grow/shrink, path intermediate textures, MSAA resolve, offscreen
texture readback with `COPY_BYTES_PER_ROW_ALIGNMENT`-padded rows, B/R
channel swap on `Bgra8Unorm` → `RgbaImage` — is deferred to a
follow-up that requires a Linux/Mesa environment for verification.

The follow-up also commits the initial reference PNGs for the golden
scene set (quad, shadow+blur, linear/radial gradient, path fill,
monochrome/polychrome sprite, text single/multi-line, clip rect).
Reference images must be captured on a lavapipe build (never a
hardware GPU) so CI replay is bit-stable.

**Owner:** Linux-capable contributor or CI-driven regen via a
scheduled workflow. Until then, the Linux-side golden coverage of
the migration (S02 → S03 wgpu+linux) falls back to manual visual
inspection of examples.

**Mac Metal goldens** via `MetalHeadlessRenderer` are NOT affected by
this deferral — they remain fully functional and are the primary
rendering regression guard on mac CI. Golden tests for the Metal
path land in the same follow-up PR as the wgpu body so both platforms
get their initial fixtures at once.


## Web platform — never compiled in CI

**Status:** broken.

`crates/flui-core/src/platform/web/events.rs:12` imports
`crate::window::WebWindowInner`, which does not resolve to any defined
symbol. The expected re-export does not exist — the actual type is at
`crates/flui-core/src/platform/web/window.rs:45` as
`pub(crate) struct WebWindowInner`, and should be reached via
`super::window::WebWindowInner` from inside `platform/web/`.

The entire web subtree is gated on `target_family = "wasm"` and
**CI never compiles for `wasm32-unknown-unknown`** — the
`.github/workflows/ci.yml` matrix is `ubuntu-latest + macos-latest`
with no wasm target. `Cargo.toml` has no
`[target.'cfg(target_family = "wasm")'.dependencies]` block either,
so even if the target were added, the web code would need
`wasm-bindgen`, `web-sys`, `js-sys`, etc. to be wired up first.

**Owner:** **S06 (web migration)**. The facade redesign and CI wasm
coverage are folded into that migration spec so the same PR that
moves the web code to `flui-platform` also stands up its build
environment for the first time.

**S01 scope:** deferred. S01b's golden tests and S01c's behavior
pinning do not cover the web backend.

## IME composition

**Status:** not locked.

`TestPlatform` has no IME hookup, so S01c cannot pin IME composition
behavior per platform. The most fragile platform code (mac NSTextView
delegate, Windows WM_IME_* messages, Wayland zwp_text_input_v3, X11
XIM) is free-running during the migration.

**Owner:** S03 (Linux Wayland/X11 integration tests), S04 (mac
integration tests), S05 (Windows integration tests) — each migration
step adds real-platform IME assertions as part of its verification
surface.

## Drag and drop

**Status:** not locked.

`FileDropEvent` is defined and dispatched through `PlatformInput`, but
S01c does not assert drag-and-drop through the test platform (the test
platform has no drag simulation). Each real platform has its own
drag-accept code path (mac NSDraggingDestination, Win32 IDropTarget,
Wayland/X11 XDND).

**Owner:** S03/S04/S05 per-platform tests.

## Display-link / vsync timing

**Status:** not locked.

`AnimationController` uses `scheduler::Clock` which S01c may exercise
through `TestDispatcher`, but frame-pacing via the actual platform
display link (mac `CVDisplayLink`, Win32 DWM timing, Wayland
`wp_presentation`, X11 frame clocks) is not pinned.

**Owner:** S03/S04/S05 per-platform smoke tests. Out of scope for the
golden rendering suite (S01b) because golden tests render to a
framebuffer, not against a real vsync.

## Mac find-pasteboard and Linux primary selection

**Status:** not locked.

`TestPlatform`'s in-memory clipboard is a single buffer — there is no
primary-selection or find-pasteboard abstraction in the test platform.
S01c asserts the clipboard round-trip for the default buffer only.

**Owner:** S03 (Linux primary selection via x11/wayland) and S04 (mac
find-pasteboard via NSPasteboard.find).

## Mac `NSServices` menu integration

**Status:** not locked.

Real-platform mac menu behaviors (ServicesMenu integration, edit
actions, NSResponder forwarding) are exercised only on a real mac via
a running `NSApplication`. `TestPlatform`'s menu model is stubbed.

**Owner:** S04.

## Wayland special protocols

**Status:** not locked.

- `xdg-activation-v1` — app focus stealing, startup notification
- `ext-session-lock-v1` — session lock surfaces
- `wp-fractional-scale-v1` — fractional DPI scaling
- `zwp-text-input-v3` — IME (see above)
- `wlr-layer-shell-unstable-v1` — already exercised by the
  `layer_shell` example, but the example is **not** in the CI smoke
  set because GitHub runners lack a Wayland compositor.

**Owner:** S03. Wayland-specific tests need either a containerized
compositor (weston + xvfb-headless) or real hardware.

## `layer_shell` example

**Status:** not in CI.

`crates/flui-core/examples/legacy/layer_shell.rs` requires a running
Wayland compositor, which GitHub's `ubuntu-latest` runner does not
provide. The example is excluded from S01c's smoke test set.

**Owner:** S03 (wayland migration step) may introduce a headless
compositor container in CI.

## Release-mode Windows build

**Status:** not locked.

S01a.4 repaired `cargo build -p flui-core` in **debug mode** only.
Release mode still requires `fxc.exe` + the Windows SDK for the HLSL
shader compilation in `build.rs:181-421`. Adding Windows to CI with
release-mode shader compilation is deferred to S05 (Windows migration)
where the FXC environment setup is worth the investment.

**Owner:** S05 — Windows migration spec adds the SDK install step to
CI and starts running `cargo build --release -p flui-core` there.

## Custom cursors (runtime)

**Status:** partially locked.

`CursorStyle` propagation through `TestPlatform::set_cursor_style` can
be asserted, but the actual platform cursor rendering (including the
`CursorStyle::None` trap at `platform/mac/platform.rs:1020`) requires
a real display server. `mac/platform.rs:1020 CursorStyle::None => unreachable!()`
is a trap: any code path that sends `CursorStyle::None` to the mac
platform will panic in production.

**Owner:** S04 explicitly includes a cursor-style matrix test. Until
then, the trap stays — tracked in
`docs/fixtures/platform-expected-stubs.txt` as the
`mac/platform.rs:1 unreachable!()` entry.
