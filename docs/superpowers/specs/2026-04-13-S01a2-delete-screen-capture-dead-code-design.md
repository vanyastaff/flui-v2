---
spec_id: S01a.2
title: delete-screen-capture-dead-code
phase: I
depends_on: [S01a.1]
blocks: [S02]
status: draft
date: 2026-04-13
---

# S01a.2 — delete-screen-capture-dead-code

## Context

Second atomic sub-step of S01a (see [roadmap](2026-04-13-flui-core-roadmap.md)).
The `screen-capture` feature is referenced at 18+ sites across `flui-core`
but is **not declared** in `crates/flui-core/Cargo.toml`. Every
`#[cfg(feature = "screen-capture")]` gate resolves to *always false*, which
means the gated code has been unreachable since the feature was introduced.
`scap` and `scap_screen_capture` are effectively dead code.

Per adversarial review, the choice is: **declare the feature** (adds
`scap = { version = "0.0.8", optional = true }` — a 0.0.x unstable crate
that pulls `pipewire`, `dbus`, `screencapturekit`, `windows-capture` as
transitive system-lib deps), or **delete the dead branches**. The user
approved **delete**.

The traits `ScreenCaptureSource` and `ScreenCaptureStream` stay in place as
**future extension points** so that a later spec can re-introduce screen
capture with a deliberate dep choice without moving symbols.

## Goals

1. Remove every `#[cfg(feature = "screen-capture")]` gate inside
   `crates/flui-core/src/**` and delete the two module files that only
   exist under the gate (`platform/scap_screen_capture.rs` and
   `platform/mac/screen_capture.rs`).
2. Collapse `PlatformScreenCaptureFrame` to its unit-type form (the
   `not(feature = "screen-capture")` branch, which is the only branch that
   has ever compiled).
3. Keep the public traits `ScreenCaptureSource`, `ScreenCaptureStream`, the
   `ScreenCaptureFrame` wrapper struct, the `SourceMetadata` struct, and the
   `Platform::screen_capture_sources()` / `Platform::is_screen_capture_supported()`
   method surface intact — no semver change to `flui-core`'s public API.
4. Verify that every workspace sibling still compiles on Linux and macOS.
5. Update the `S01a.1` stub inventory (via `cargo xtask check-stubs --bless`)
   to reflect any `unreachable!()` or `unimplemented!()` counts that shift
   when dead blocks vanish.

## Non-goals

- Not deleting `ScreenCaptureSource`, `ScreenCaptureStream`,
  `ScreenCaptureFrame`, or `SourceMetadata`. These are retained as
  **future extension points** with zero current impls (other than
  `TestScreenCaptureSource` / `TestScreenCaptureStream`, which live in the
  test platform and are unaffected).
- Not removing `App::screen_capture_sources()` (public method at
  `app.rs:1147`). It continues to delegate to `Platform::screen_capture_sources()`,
  which now always returns the default "not supported" error from the
  trait's default impl at `platform.rs:230`.
- Not touching `platform/test/*` — the `TestScreenCaptureSource` /
  `TestScreenCaptureStream` types are test infrastructure, not production
  screen capture, and must be preserved intact.
- Not adding any new feature, not pinning any new dependency.
- Not removing `scap` from `Cargo.lock` manually — it was never declared,
  so it isn't in the lockfile.
- Not modifying any non-core crate.

## Current state

### Sites to delete (verified via grep)

**Feature gates** (`#[cfg(feature = "screen-capture")]` or
`#[cfg(not(feature = "screen-capture"))]`) at:

- [`crates/flui-core/src/platform.rs:31`](../../crates/flui-core/src/platform.rs#L31) — opens the
  `scap_screen_capture` module gate.
- [`crates/flui-core/src/platform.rs:38`](../../crates/flui-core/src/platform.rs#L38) — opens the
  `PlatformScreenCaptureFrame = scap::frame::Frame;` branch.
- [`crates/flui-core/src/platform.rs:41`](../../crates/flui-core/src/platform.rs#L41) — opens the
  `not(feature = "screen-capture")` → `()` branch (the only one that
  compiles today; collapses to unconditional).
- [`crates/flui-core/src/platform.rs:43`](../../crates/flui-core/src/platform.rs#L43) — opens the
  `all(target_os = "macos", feature = "screen-capture")` →
  `CVImageBuffer` branch.
- [`crates/flui-core/src/platform/mac.rs:13`](../../crates/flui-core/src/platform/mac.rs#L13) —
  gates the `mod screen_capture;` module declaration.
- [`crates/flui-core/src/platform/mac/platform.rs:593`](../../crates/flui-core/src/platform/mac/platform.rs#L593) and
  [`:599`](../../crates/flui-core/src/platform/mac/platform.rs#L599) —
  gate the `MacPlatform`'s `screen_capture_sources` /
  `is_screen_capture_supported` override methods.
- [`crates/flui-core/src/platform/windows/platform.rs:490`](../../crates/flui-core/src/platform/windows/platform.rs#L490) and
  [`:495`](../../crates/flui-core/src/platform/windows/platform.rs#L495) —
  gate the `WindowsPlatform`'s override methods.
- [`crates/flui-core/src/platform/linux/headless/client.rs:67`](../../crates/flui-core/src/platform/linux/headless/client.rs#L67) —
  gates a headless no-op override.
- [`crates/flui-core/src/platform/linux/platform.rs:58`](../../crates/flui-core/src/platform/linux/platform.rs#L58),
  [`:63`](../../crates/flui-core/src/platform/linux/platform.rs#L63),
  [`:286`](../../crates/flui-core/src/platform/linux/platform.rs#L286),
  [`:291`](../../crates/flui-core/src/platform/linux/platform.rs#L291) —
  gate the Linux platform's dispatcher-level overrides.
- [`crates/flui-core/src/platform/linux/wayland/client.rs:705`](../../crates/flui-core/src/platform/linux/wayland/client.rs#L705) —
  gates the Wayland client's override.
- [`crates/flui-core/src/platform/linux/x11/client.rs:1493`](../../crates/flui-core/src/platform/linux/x11/client.rs#L1493) and
  [`:1498`](../../crates/flui-core/src/platform/linux/x11/client.rs#L1498) —
  gate the X11 client's overrides.

**Files to delete entirely:**

- [`crates/flui-core/src/platform/scap_screen_capture.rs`](../../crates/flui-core/src/platform/scap_screen_capture.rs) —
  317 LoC of `scap` integration. The file imports `use scap::Target;`
  unconditionally at line 8; since the `scap` crate is nowhere in
  `Cargo.toml`, this file has not compiled for the entire time the feature
  has been broken. Deleting it is definitionally safe.
- [`crates/flui-core/src/platform/mac/screen_capture.rs`](../../crates/flui-core/src/platform/mac/screen_capture.rs) —
  referenced by the `#[cfg(feature = "screen-capture")] mod screen_capture;`
  at `mac.rs:13`. Contains `MacScreenCaptureSource` / `MacScreenCaptureStream`
  using `core-video` + `screencapturekit`. Also dead.

Total sites: **18 cfg gates across 11 files** + **2 module files deleted**.

### What stays untouched

- **`crates/flui-core/src/platform.rs:392-426`**: `SourceMetadata` struct,
  `pub trait ScreenCaptureSource`, `pub trait ScreenCaptureStream`,
  `pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame)`. All
  four remain `pub` at the same paths.
- **`crates/flui-core/src/platform.rs:225-234`**: `Platform` trait's
  `is_screen_capture_supported` (default `false`) and
  `screen_capture_sources` (default "not supported" error). No change.
- **`crates/flui-core/src/app.rs:1145-1150`**: `App::screen_capture_sources`
  method. No change. Runtime behavior is that it now always receives the
  default error from every platform's trait impl.
- **`crates/flui-core/src/platform/test/platform.rs`**,
  **`crates/flui-core/src/platform/test/window.rs`**: `TestScreenCaptureSource`
  / `TestScreenCaptureStream` are test infrastructure and completely
  unaffected. They're test platform impls of the trait, not gated on the
  feature.
- **`crates/flui-core/src/platform/visual_test.rs:118`**: the visual test
  platform's `screen_capture_sources` override returns an empty list; not
  gated; stays.
- **`crates/flui-core/src/app/test_context.rs:371`**:
  `set_screen_capture_sources(sources: Vec<TestScreenCaptureSource>)`. Test
  context helper; unaffected.

### Imports to preserve

`app.rs:50` imports `ScreenCaptureSource`. Stays.

`platform/test.rs:11` re-exports `TestScreenCaptureSource`,
`TestScreenCaptureStream`. Stays.

`platform.rs:91` re-exports those same test types behind
`#[cfg(any(test, feature = "test-support"))]`. Stays.

## Design

### Step 1 — collapse `PlatformScreenCaptureFrame`

Replace `platform.rs:36-44`:

```rust
#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "freebsd",),
    feature = "screen-capture"
))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = ();
#[cfg(all(target_os = "macos", feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = core_video::image_buffer::CVImageBuffer;
```

With:

```rust
/// Placeholder type for platform screen-capture frames.
///
/// Screen capture is currently not implemented in flui-core. The
/// `ScreenCaptureSource` / `ScreenCaptureStream` traits exist as future
/// extension points; when a concrete backend is reintroduced, this alias
/// will be replaced with the real frame type.
pub(crate) type PlatformScreenCaptureFrame = ();
```

Doc comment is required to pass `#![warn(missing_docs)]` on the promoted
alias — we add it here even though the type is `pub(crate)`, because
`ScreenCaptureFrame(pub PlatformScreenCaptureFrame)` exposes it indirectly
through a public field.

### Step 2 — remove the module gate

Delete `platform.rs:30-34`:

```rust
#[cfg(all(
    feature = "screen-capture",
    any(target_os = "windows", target_os = "linux", target_os = "freebsd",)
))]
pub mod scap_screen_capture;
```

Delete the entire file `platform/scap_screen_capture.rs`.

Delete `platform/mac.rs:13`:

```rust
#[cfg(feature = "screen-capture")]
mod screen_capture;
```

Delete the entire file `platform/mac/screen_capture.rs`.

### Step 3 — delete the override methods

For each of the following sites, delete the `#[cfg(...)]` attribute AND the
entire method body that follows it (until the next method boundary). The
platform falls back to the trait default.

- `mac/platform.rs:593` — `fn is_screen_capture_supported(&self) -> bool`
  override (returns true). After deletion, the default (`false`) applies.
- `mac/platform.rs:599` — `fn screen_capture_sources(&self) -> ...` override.
  After deletion, the default error applies.
- `windows/platform.rs:490`, `:495` — same two overrides for
  `WindowsPlatform`.
- `linux/headless/client.rs:67` — headless override (already a no-op).
- `linux/platform.rs:58`, `:63`, `:286`, `:291` — Linux dispatcher
  overrides for both wayland and x11 cases.
- `linux/wayland/client.rs:705` — Wayland client screen capture.
- `linux/x11/client.rs:1493`, `:1498` — X11 client screen capture.

**Verification for each site:** after deleting the cfg'd method, run
`cargo check -p flui-core` and confirm the `impl Platform for <XxxPlatform>`
block still compiles. The trait's default impl is called in the absence of
an override.

### Step 4 — update stub inventory

`unreachable!()` site at `crates/flui-core/src/platform/linux/wayland/client.rs:705`
— the cfg'd code block contains a `// TODO(screen-capture)` comment plus
some fallback logic; after deletion the count may shift. Same for X11 client.

Run `cargo xtask check-stubs --bless` and commit the updated fixture at
`docs/fixtures/platform-expected-stubs.toml` as part of the same commit.
Document the delta in the spec's test log.

## API surface

**Zero change to the public surface.**

Specifically preserved, bit-for-bit:

- `pub trait ScreenCaptureSource` with its three methods (`metadata`,
  `stream`, `is_primary`).
- `pub trait ScreenCaptureStream` with its one method (`poll_frame`).
- `pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame)` — still
  has a public field, but the inner type is now unconditionally `()`. Any
  caller constructing `ScreenCaptureFrame(())` continues to compile.
- `pub struct SourceMetadata`.
- `Platform::is_screen_capture_supported(&self) -> bool` (default `false`).
- `Platform::screen_capture_sources(&self) -> oneshot::Receiver<...>`
  (default error).
- `App::screen_capture_sources(&self) -> oneshot::Receiver<...>`.

**Removed (internal only):**

- Module `platform/scap_screen_capture` (pub but never compiled).
- Module `platform/mac/screen_capture` (private to mac).
- Three per-platform method override bodies.

The pub module `scap_screen_capture` had a pub function `scap_screen_sources`
that external code could in theory have called — but since the feature was
never declared, no external code has ever compiled against it. Removing the
module is definitionally not a breaking change.

## Migration / Compatibility

**Internal workspace siblings:** `flui-widgets`, `flui-material`,
`flui-navigator`, `flui-a11y`, `flui-theme`, `flui-animate` — none
currently enable the `screen-capture` feature (it doesn't exist). None
should break.

**External consumers of `flui-core`** (there are none yet at 0.1.0, but
being careful): anybody importing `flui_core::ScreenCaptureSource` still
compiles. Anybody importing `flui_core::scap_screen_capture::scap_screen_sources`
breaks — but this import requires enabling a feature that doesn't exist,
so the code has never compiled upstream either.

**Cargo.lock:** unchanged — `scap` was never a declared dependency so it's
not locked.

**`Cargo.toml`:** unchanged. No feature is added or removed (the feature
was never declared in the first place).

## Testing strategy

1. `cargo check -p flui-core` — all three targets (Linux, macOS — the
   developer's Windows 11 will fail because of S01a.4's 257 errors, so
   Windows is explicitly not a verification target for S01a.2).
2. `cargo check -p flui-widgets -p flui-material -p flui-navigator -p flui-a11y -p flui-theme -p flui-animate` —
   sibling canary, Linux + macOS. Must remain green.
3. `cargo xtask check-stubs` — runs the new xtask from S01a.1 and confirms
   the post-delete stub counts match the re-blessed fixture.
4. `cargo test --workspace` — all existing tests still pass.
5. Manual spot-check: `grep -rn 'feature = "screen-capture"' crates/flui-core/`
   returns zero matches.
6. Manual spot-check: `grep -rn 'scap::' crates/flui-core/` returns zero
   matches (if it doesn't, we missed a usage).

## Open questions

- **`# TODO(screen-capture)` comments inside now-deleted blocks** — some
  sites (e.g. `linux/wayland/client.rs:710` per roadmap §1) have TODO
  comments referring to screen capture. If the entire surrounding block is
  removed, the comment goes with it. If the comment is outside the
  cfg-gated block but refers to it, it becomes stale. Sweep for
  `TODO.*screen.capture` during implementation and remove or rewrite.
- **`mac/platform.rs:593-599`** override body: it likely constructs
  `ScapCaptureSource` instances via the now-deleted module. Deleting it
  may leave dangling `use` statements at the top of the file that need
  cleanup.
- **`windows/platform.rs:490-495`** has an `unimplemented!()` at line 474
  that is independent of screen capture (dock-menu). Not affected, but
  verify the cfg attribute on `:490` doesn't accidentally extend to
  `:474`.

## Done criteria

- [ ] All 18 `#[cfg(feature = "screen-capture")]` gates removed from
      `crates/flui-core/src/**`.
- [ ] `platform/scap_screen_capture.rs` deleted.
- [ ] `platform/mac/screen_capture.rs` deleted.
- [ ] `platform/mac.rs:13` `mod screen_capture;` line deleted.
- [ ] `platform.rs:36-44` collapsed to the unconditional unit-type form
      with doc comment.
- [ ] `grep -rn 'screen-capture' crates/flui-core/` returns zero matches
      (both `feature = "screen-capture"` and any bare references).
- [ ] `grep -rn 'scap::' crates/flui-core/` returns zero matches.
- [ ] `cargo check -p flui-core` green on Linux and macOS.
- [ ] `cargo check -p flui-widgets -p flui-material -p flui-navigator -p flui-a11y -p flui-theme -p flui-animate`
      green on Linux and macOS.
- [ ] `cargo test --workspace` green on Linux and macOS.
- [ ] `cargo xtask check-stubs --bless` run; updated fixture committed in
      the same PR; delta recorded in test log.
- [ ] `ScreenCaptureSource`, `ScreenCaptureStream`, `ScreenCaptureFrame`,
      `SourceMetadata` still exist at the same paths.
- [ ] `App::screen_capture_sources` still exists with the same signature.
- [ ] `Platform::is_screen_capture_supported` and
      `Platform::screen_capture_sources` still exist with their default
      impls unchanged.
- [ ] Commit is a single atomic PR.

## Test log

To be filled during implementation.

### Stub inventory delta

```
$ cargo xtask check-stubs --bless
# Expected output: N fewer unreachable!() sites, N fewer unimplemented!() sites
# Captured delta: TBD
```

### Sibling canary results

| Crate | Linux | macOS |
|---|---|---|
| flui-widgets | TBD | TBD |
| flui-material | TBD | TBD |
| flui-navigator | TBD | TBD |
| flui-a11y | TBD | TBD |
| flui-theme | TBD | TBD |
| flui-animate | TBD | TBD |

### Final site count verification

```
$ grep -rn 'screen-capture\|scap::' crates/flui-core/
# Expected: empty
# Actual: TBD
```

## Follow-ups after S01a.2 lands

- **S02 unblocked** on the screen-capture front — flui-platform extraction
  no longer has to preserve the broken feature.
- **Future spec**: if screen capture is ever reintroduced, it goes into its
  own separate spec with a deliberate `scap` (or alternative) dep decision
  and CI coverage from day one. The trait surface is already in place and
  does not need to be re-designed.
