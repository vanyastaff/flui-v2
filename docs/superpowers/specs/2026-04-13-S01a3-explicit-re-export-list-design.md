---
spec_id: S01a.3
title: explicit-re-export-list
phase: I
depends_on: [S01a.1]
blocks: [S01d, S02]
status: draft
date: 2026-04-13
---

# S01a.3 — explicit-re-export-list

## Context

Third atomic sub-step of S01a (see [roadmap](2026-04-13-flui-core-roadmap.md)).

[`crates/flui-core/src/lib.rs:117`](../../crates/flui-core/src/lib.rs#L117)
currently has `pub use platform::*;` — a blanket glob re-export that leaks
every `pub` item in `platform.rs` and its nested `pub use` chains
(`app_menu::*`, `keyboard::*`, `keystroke::*`) straight to the `flui_core`
crate root.

This is a review hazard: any spec that adds or promotes a `pub` item in
`platform.rs` automatically makes it part of `flui_core`'s top-level
public API with no explicit approval step. S02 (the actual `flui-platform`
crate extraction) will add and move many items in this file, and without
an explicit list we lose the ability to audit what's crossing the
workspace boundary.

S01a.3 replaces the glob with an enumerated list, grouped by `#[cfg]`
predicate, with explicit `#[doc(hidden)]` markers on the handful of
macro-support items that were already annotated at the definition site.
No new `pub` items are added. No items are removed from the public API
shape.

## Goals

1. Replace `lib.rs:117 pub use platform::*;` with a hand-written list of
   every symbol the glob currently exposes, organized by `#[cfg]` block.
2. Preserve every target-gate and feature-gate exactly. A symbol that is
   only reachable on `target_os = "macos"` stays only reachable on
   `target_os = "macos"`.
3. Demote the four known macro-support items (`RunnableVariant`,
   `TimerResolutionGuard`, `PlatformDispatcher`, `RunnableMeta`) to
   `#[doc(hidden)] pub use`, matching their existing `#[doc(hidden)]`
   attribute at the definition site.
4. Verify that every workspace sibling continues to compile on Linux AND
   macOS. Windows verification is explicitly deferred to after S01a.4.
5. Add a crate-root comment explaining that `platform::*` is the only
   glob being fixed in S01a.3 and why (S02 extraction prep), leaving the
   other ~29 `pub use <mod>::*;` lines in `lib.rs` untouched by this spec.

## Non-goals

- Not touching any other `pub use <mod>::*;` at `lib.rs`. There are roughly
  29 other glob re-exports at `lib.rs:87-139` (`action::*`, `animation::*`,
  `app::*`, `scene::*`, `style::*`, `window::*`, etc.). Only `platform::*`
  is fixed because only `platform::*` is relevant to S02's extraction.
  A comment documents the decision.
- Not adding new docs. `#![warn(missing_docs)]` compliance is unchanged —
  each symbol's rustdoc lives at the definition site, not at the
  re-export.
- Not modifying any symbol's visibility at the definition site. `pub` in
  `platform.rs` stays `pub`. Only the crate-root re-export is rewritten.
- Not deleting any currently-reachable symbol. If it reaches the crate
  root via the glob today, it reaches the crate root via the enumerated
  list after S01a.3.
- Not un-hiding items that are already `#[doc(hidden)]` at the definition
  site. The demotions in this spec mirror existing annotations — they
  don't create new ones.
- Not attempting to verify the Windows build. That's S01a.4's problem.
  S01a.3 states that verification is deferred and lists it as a known
  gap.
- Not covering `flui_core::platform::*` sub-module paths. If a downstream
  user reaches `flui_core::platform::mac::X`, they get a compile error
  because `mod platform` is private — that's already true today, not a
  change from S01a.3.

## Current state

### Items reached through `pub use platform::*;` (enumerated live)

The list below was produced by
`rg '^pub (fn|struct|enum|trait|type|const|use|mod) ' crates/flui-core/src/platform.rs`
plus grep on the three nested `pub use` chains. Line numbers are as of the
commit immediately before S01a.3 lands. The implementation phase
regenerates the list against the current HEAD and commits the new
`lib.rs:117` block in one go.

#### Always-on (no cfg)

**Traits:**
- [`Platform`](../../crates/flui-core/src/platform.rs#L204)
- [`PlatformDisplay`](../../crates/flui-core/src/platform.rs#L349)
- [`PlatformWindow`](../../crates/flui-core/src/platform.rs#L566)
- [`PlatformTextSystem`](../../crates/flui-core/src/platform.rs#L725)
- [`PlatformAtlas`](../../crates/flui-core/src/platform.rs#L973)
- [`ScreenCaptureSource`](../../crates/flui-core/src/platform.rs#L406)
- [`ScreenCaptureStream`](../../crates/flui-core/src/platform.rs#L420)
- [`InputHandler`](../../crates/flui-core/src/platform.rs#L1232)

**Structs:**
- `DisplayId`, `SourceMetadata`, `ScreenCaptureFrame`, `WindowControls`,
  `Tiling`, `RequestFrameOptions`, `NoopTextSystem`, `AtlasTextureList<T>`,
  `AtlasTile`, `AtlasTextureId`, `TileId`, `PlatformInputHandler`,
  `UTF16Selection`, `WindowOptions`, `WindowParams`, `TitlebarOptions`,
  `PathPromptOptions`, `ClipboardItem`, `ClipboardString`, `Image`.

**Enums:**
- `ThermalState`, `ResizeEdge`, `WindowDecorations`, `Decorations`,
  `WindowBounds`, `WindowKind`, `WindowAppearance`,
  `WindowBackgroundAppearance`, `TextRenderingMode`, `PromptLevel`,
  `PromptButton`, `CursorStyle`, `ClipboardEntry`, `ImageFormat`,
  `AtlasKey`, `AtlasTextureKind`.

**Functions:**
- `current_platform`, `background_executor`, `application`, `headless`.

**From `app_menu::*`:** `Menu`, `OsMenu`, `SystemMenuType`, `MenuItem`,
`OwnedOsMenu`, `OwnedMenu`, `OwnedMenuItem`, `OsAction`.

**From `keyboard::*`:** `PlatformKeyboardLayout`, `PlatformKeyboardMapper`,
`DummyKeyboardMapper`.

**From `keystroke::*`:** `AsKeystroke`, `Keystroke`, `KeybindingKeystroke`,
`InvalidKeystrokeError`, `KEYSTROKE_PARSE_EXPECTED_MESSAGE`, `Modifiers`,
`Capslock`.

#### Target-gated

- `#[cfg(target_os = "macos")]` → `MacPlatform`
- `#[cfg(target_os = "windows")]` → `WindowsPlatform`
- `#[cfg(target_family = "wasm")]` → `single_threaded_web`, `web_init`
- `#[cfg(any(target_os = "linux", target_os = "freebsd"))]` →
  `guess_compositor`
- `#[cfg(all(target_os = "linux", feature = "wayland"))]` →
  `pub mod layer_shell` — this is a **module** that reaches crate root
  (see Design §3 below for the decision).

#### Feature-gated

- `#[cfg(any(test, feature = "test-support"))]` →
  `PlatformHeadlessRenderer`, `TestDispatcher`, `TestScreenCaptureSource`,
  `TestScreenCaptureStream`, `current_headless_renderer`.
- `#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]` →
  `VisualTestPlatform`.

#### Already `#[doc(hidden)]` at definition site (mirror the annotation)

- `RunnableVariant` — defined with `#[doc(hidden)]` at
  [`platform.rs:692`](../../crates/flui-core/src/platform.rs#L692).
- `TimerResolutionGuard` — same, at
  [`platform.rs:695`](../../crates/flui-core/src/platform.rs#L695).
- `PlatformDispatcher` — same, at
  [`platform.rs:700`](../../crates/flui-core/src/platform.rs#L700) with
  an explicit comment about macro generation.
- `RunnableMeta` — re-exported from `scheduler::RunnableMeta` at
  [`platform.rs:47`](../../crates/flui-core/src/platform.rs#L47); the
  definition in `scheduler/` is also `pub` but intended as macro
  plumbing. Mirror as `#[doc(hidden)]` here.

#### Candidates for demotion to `pub(crate)` or `#[doc(hidden)]` — NOT touched in S01a.3

Left `pub` as-is for this spec; a future cleanup spec may demote them:

- `get_gamma_correction_ratios` at
  [`platform.rs:889`](../../crates/flui-core/src/platform.rs#L889) —
  marked `#[allow(dead_code)]`, unreferenced outside `flui-core`. Stays
  `pub` because S01a.3 is non-breaking.
- `KEYSTROKE_PARSE_EXPECTED_MESSAGE` — a `pub const` error string.
  Probably should be `pub(crate)`. Not touched.
- `DummyKeyboardMapper` — test/fallback. Not touched.
- `NoopTextSystem` — headless scaffolding. Not touched.

Documenting these here so a future spec doesn't re-discover them.

### Sibling consumers of `flui_core::*`

The blast radius of the enumerated list is every workspace sibling that
imports from `flui_core`. Confirmed live consumers of glob-importing:

- [`flui-navigator/src/widgets.rs:34`](../../crates/flui-navigator/src/widgets.rs#L34)
  — `use flui_core::*;`. Blanket glob. Any symbol the S01a.3 list forgets
  is a compile error here.

Named imports (lower blast radius but still verified):

- `flui-widgets/**` — many `use flui_core::{...}` named imports.
- `flui-material/**` — many `use flui_core::{...}` including
  `WindowAppearance`.
- `flui-animate/**`, `flui-theme/**`, `flui-a11y/**` — named imports.

S01a.3 does NOT exhaustively grep all of these before writing the list —
instead it treats the sibling `cargo check` as the verification gate.
If the enumerated list drops a symbol any sibling needs, the check fails
and the list is patched in the same PR.

## Design

### Step 1 — draft the `lib.rs:117` replacement block

The block replaces the single line `pub use platform::*;` with an
organized, comment-delimited enumeration. Exact draft:

```rust
// --- crate-level re-exports from the platform module ---
//
// S01a.3 replaces `pub use platform::*;` with an explicit list so that
// any new `pub` item in `platform.rs` must be explicitly routed through
// this block before it becomes part of the `flui_core` public surface.
// This is the prerequisite for S02 extracting the platform subtree into
// the sibling `flui-platform` crate.
//
// NOTE: the ~29 other `pub use <mod>::*;` lines in this file intentionally
// remain glob-re-exports. They will be converted case-by-case if and when
// their modules are extracted; S01a.3 is scoped to `platform::*` only.

// Core platform traits (always-on)
pub use platform::{
    Platform, PlatformDisplay, PlatformWindow, PlatformTextSystem,
    PlatformAtlas,
    ScreenCaptureSource, ScreenCaptureStream, ScreenCaptureFrame,
    SourceMetadata,
    InputHandler,
};

// Display & window types
pub use platform::{
    DisplayId, ThermalState,
    WindowOptions, WindowParams, WindowBounds, WindowKind,
    WindowAppearance, WindowBackgroundAppearance, WindowControls,
    WindowDecorations, Decorations, Tiling, TitlebarOptions,
    ResizeEdge, RequestFrameOptions,
    TextRenderingMode,
    PromptLevel, PromptButton, PathPromptOptions, CursorStyle,
};

// Input & clipboard
pub use platform::{
    PlatformInputHandler, UTF16Selection,
    ClipboardItem, ClipboardEntry, ClipboardString,
    ImageFormat, Image,
};

// Atlas / rendering primitives
pub use platform::{
    AtlasKey, AtlasTextureList, AtlasTile, AtlasTextureId, TileId,
    AtlasTextureKind,
    NoopTextSystem,
};

// Free functions
pub use platform::{
    current_platform, application, headless, background_executor,
};

// app_menu (originally glob-re-exported via platform::app_menu::*)
pub use platform::{
    Menu, OsMenu, SystemMenuType, MenuItem,
    OwnedOsMenu, OwnedMenu, OwnedMenuItem, OsAction,
};

// keyboard (originally glob-re-exported via platform::keyboard::*)
pub use platform::{
    PlatformKeyboardLayout, PlatformKeyboardMapper, DummyKeyboardMapper,
};

// keystroke (originally glob-re-exported via platform::keystroke::*)
pub use platform::{
    AsKeystroke, Keystroke, KeybindingKeystroke, InvalidKeystrokeError,
    KEYSTROKE_PARSE_EXPECTED_MESSAGE,
    Modifiers, Capslock,
};

// Macro-support plumbing — hidden from rustdoc to match the annotations
// at the definition site in `platform.rs`.
#[doc(hidden)]
pub use platform::{RunnableVariant, TimerResolutionGuard, PlatformDispatcher, RunnableMeta};

// Test-support-only items
#[cfg(any(test, feature = "test-support"))]
pub use platform::{
    PlatformHeadlessRenderer,
    TestDispatcher, TestScreenCaptureSource, TestScreenCaptureStream,
    current_headless_renderer,
};

// Target-gated platform impls
#[cfg(target_os = "macos")]
pub use platform::MacPlatform;

#[cfg(target_os = "windows")]
pub use platform::WindowsPlatform;

#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use platform::VisualTestPlatform;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use platform::guess_compositor;

#[cfg(target_family = "wasm")]
pub use platform::{single_threaded_web, web_init};
```

Total: ~85-100 lines including comments and `#[cfg]` attributes.

### Step 2 — `pub mod layer_shell` decision

`platform.rs:22` declares `pub mod layer_shell` gated on
`#[cfg(all(target_os = "linux", feature = "wayland"))]`. Today the glob
`pub use platform::*;` does NOT re-export modules — modules are excluded
from glob re-export. So `flui_core::layer_shell::*` is currently reachable
only through `flui_core::platform::layer_shell::*` — which requires
`platform` to be reachable, and `mod platform;` is private at
`lib.rs:40`.

Verification: `cargo doc --no-deps` on a Linux+wayland build should show
`flui_core::layer_shell` as a reachable path if and only if it's
explicitly re-exported.

**Decision:** add an explicit cfg-gated re-export to preserve the status
quo if `layer_shell` is already reachable, or leave it alone if it isn't.
Implementation runs `cargo doc --no-deps --features wayland` on Linux
before the rewrite to determine which. If reachable today, the new
`lib.rs` block adds:

```rust
#[cfg(all(target_os = "linux", feature = "wayland"))]
pub use platform::layer_shell;
```

If not, no change.

### Step 3 — verification procedure

1. `cargo doc --no-deps -p flui-core` on Linux **before** the rewrite.
   Save the generated HTML file listing (`target/doc/flui_core/index.html`)
   for diff comparison.
2. Apply the rewrite.
3. `cargo doc --no-deps -p flui-core` **after** the rewrite.
4. `diff -r target/doc/flui_core/ target/doc-before/` — must show zero
   meaningful differences in published items (timestamps in generated
   HTML are filtered out).
5. Repeat steps 1-4 on macOS.
6. `cargo check -p flui-core -p flui-navigator -p flui-widgets -p flui-material -p flui-theme -p flui-a11y -p flui-animate`
   on Linux. Any compile error → the enumerated list is incomplete, patch
   and re-run.
7. Same on macOS.
8. Optional but recommended: `cargo check --features inspector -p flui-core`,
   `cargo check --no-default-features -p flui-core`,
   `cargo check --all-features -p flui-core` on Linux to exercise
   alternative feature combinations. The enumerated list must be
   cfg-correct under all of these.

Windows verification is explicitly **deferred to post-S01a.4**. The reason
is verified locally: debug-mode Windows build of `flui-core` currently
fails with 257 errors for reasons unrelated to S01a.3, so S01a.3 can't
check its own effect on Windows. The Windows verification becomes part of
S01a.4's done criteria (which will green the Windows build).

### Step 4 — sanity assertions

Add a `compile_tests` section in the test log:

- `static_assertions::assert_impl_all!(Platform: ?Sized);` — confirms the
  trait is still reachable at its advertised path.
- Same for `PlatformWindow`, `PlatformDisplay`, `PlatformTextSystem`,
  `PlatformAtlas`.
- Assert no change in `TestDispatcher` public API.

These are not runtime tests; they run at compile time and prove the paths
still resolve.

## API surface

**Zero net change** relative to current `pub use platform::*;`. The same
set of items is reachable at `flui_core::<name>`. Four of them gain
`#[doc(hidden)]` at the re-export site, matching their existing
annotation at the definition site — so rustdoc already hides them, and
the change is cosmetic from rustdoc's perspective but enforces consistency
at the API-boundary layer.

Verified via `cargo doc --no-deps` before/after diff.

## Migration / Compatibility

**Zero breaking change** to `flui-core`'s public API. All consumers — both
workspace siblings and any external users — continue to see the same
symbol set at the same paths.

`flui-navigator/src/widgets.rs:34 use flui_core::*;` continues to work
because the enumerated list reproduces the full set of reachable names.

## Testing strategy

1. **`cargo doc --no-deps` diff** on Linux + macOS: identical output
   except for ordering and internal HTML timestamps.
2. **`cargo check` across workspace siblings** on Linux + macOS: green.
3. **Compile-time assertions** (`static_assertions::assert_impl_all!`) on
   every key trait path.
4. **Feature matrix spot checks** on Linux: `--no-default-features`,
   `--features inspector`, `--features test-support`, `--all-features`.
5. **No new tests in `flui-core`** — S01a.3 adds no runtime code.

## Open questions

- **`pub mod layer_shell` re-export status** — resolved by live
  `cargo doc` inspection at implementation time, not by pre-commit
  reasoning. Whichever side is currently correct, S01a.3 preserves it.
- **Post-deletion count** — S01a.2 deletes `scap_screen_capture` module,
  which means `platform.rs:34 pub mod scap_screen_capture;` is gone by
  the time S01a.3 runs. Good — one less module to decide about. S01a.3's
  enumerated list does NOT include `scap_screen_capture` because S01a.2
  already removed it.
- **Windows debug build** — verification deferred to after S01a.4 lands.
  If S01a.4 turns up a Windows-specific symbol the enumerated list
  doesn't include, the fix is a single-line addition to S01a.3's block,
  not a rollback.
- **`flui_core::PlatformInput` and related non-`platform::*` items** —
  these come through other modules (`input::*`, `interactive::*`) and
  are out of scope for S01a.3. Noted so the spec's reader doesn't
  confuse the scope.

## Done criteria

- [ ] `crates/flui-core/src/lib.rs:117` no longer contains
      `pub use platform::*;`.
- [ ] The replacement block is committed with comments explaining the
      rationale and the `platform::*`-only scope.
- [ ] `cargo doc --no-deps -p flui-core` on Linux + macOS shows no public
      API delta (documented in test log).
- [ ] `cargo check -p flui-core -p flui-navigator -p flui-widgets -p flui-material -p flui-theme -p flui-a11y -p flui-animate`
      green on Linux + macOS.
- [ ] `cargo check -p flui-core --no-default-features` green on Linux.
- [ ] `cargo check -p flui-core --features inspector` green on Linux.
- [ ] `cargo check -p flui-core --features test-support` green on Linux +
      macOS.
- [ ] `cargo check -p flui-core --all-features` green on Linux + macOS.
- [ ] `static_assertions::assert_impl_all!` block added proving the five
      core traits still resolve at the expected paths.
- [ ] `layer_shell` re-export decision made and documented (add or skip
      based on pre-rewrite `cargo doc` inspection).
- [ ] Windows verification explicitly marked deferred to post-S01a.4 in
      the PR description.
- [ ] `#[doc(hidden)] pub use platform::{RunnableVariant, TimerResolutionGuard, PlatformDispatcher, RunnableMeta};`
      added.
- [ ] Commit is a single atomic PR touching only `lib.rs`.
- [ ] No sibling `Cargo.toml` change.

## Test log

To be filled during implementation.

### `cargo doc --no-deps` diff

| Target | Before file count | After file count | Delta |
|---|---|---|---|
| Linux | TBD | TBD | TBD |
| macOS | TBD | TBD | TBD |

### `cargo check` sibling canary

| Crate | Linux | macOS |
|---|---|---|
| flui-core | TBD | TBD |
| flui-navigator | TBD | TBD |
| flui-widgets | TBD | TBD |
| flui-material | TBD | TBD |
| flui-theme | TBD | TBD |
| flui-a11y | TBD | TBD |
| flui-animate | TBD | TBD |

### Feature matrix

| Features | Linux | macOS |
|---|---|---|
| default | TBD | TBD |
| --no-default-features | TBD | TBD |
| --features inspector | TBD | TBD |
| --features test-support | TBD | TBD |
| --all-features | TBD | TBD |

### Symbol count

- Symbols reachable at `flui_core::*` before: TBD
- Symbols reachable at `flui_core::*` after: TBD (must equal before)

## Follow-ups after S01a.3 lands

- **S01d unblocked.** S01d's `WebWindowInner` `#[doc(hidden)]` facade and
  `PlatformScreenCaptureFrame` opaque newtype both need the explicit
  re-export list to avoid leaking into the crate root.
- **Windows verification after S01a.4.** Once S01a.4 repairs the debug
  Windows build, re-run the sibling check on Windows and amend S01a.3's
  test log (or open a tiny follow-up PR if a Windows-specific symbol is
  missing from the list).
- **Future cleanup spec**: may demote `get_gamma_correction_ratios`,
  `KEYSTROKE_PARSE_EXPECTED_MESSAGE`, `DummyKeyboardMapper`, `NoopTextSystem`
  to `pub(crate)`. Not S01a.3's job.
- **Other glob re-exports at `lib.rs`**: `action::*`, `animation::*`,
  `scene::*`, `style::*`, `window::*`, etc. (29 total). Each will be
  converted if and when its module sees significant churn. No spec
  currently plans to do them en masse.
