---
spec_id: S01a.4
title: fix-debug-windows-build
phase: I
depends_on: [S01a.1]
blocks: [S02]
status: draft
date: 2026-04-13
---

# S01a.4 — fix-debug-windows-build

## Context

Fourth atomic sub-step of S01a (see
[roadmap](2026-04-13-flui-core-roadmap.md)). Verified locally on
2026-04-13: `cargo build -p flui-core` on Windows 11 (debug profile,
default features) fails with **257 errors and 44 warnings**. The repair
has nothing to do with shader compilation (`build.rs:184` correctly gates
FXC on `#[cfg(not(debug_assertions))]`, and `directx_renderer.rs` also
correctly gates its `include!(shaders_bytes.rs)` — the GPU reviewer
confirmed this during S01a brainstorming).

The actual root causes are much more mundane: a missing `windows` crate
feature, a broken `windows::core` path, and `use crate::*;` globs in the
`platform/windows/**` subtree that are failing to resolve across 6+ files.

S01a.4 is a **repair spec** with a categorized error breakdown and a
mechanical fix procedure. It is not a design spec — there is nothing to
design; the debug build used to work and needs to work again. The spec's
value is (1) documenting the error classes so the implementer doesn't
have to re-derive them, (2) committing to a verification loop, and (3)
explicitly scoping the work so it doesn't balloon into a rewrite of the
Windows platform layer.

Does not block S01b or S01c. **Blocks S02** (the flui-platform extraction
cannot start migrating Windows code that doesn't compile in the current
mode).

## Goals

1. `cargo build -p flui-core` on Windows 11 with default features (debug
   profile) exits 0. Warnings may remain; errors must be zero.
2. `cargo check -p flui-core` on Windows 11 with
   `--no-default-features`, `--features test-support`, `--features inspector`,
   `--all-features` all exit 0.
3. Root causes are fixed, not papered over. No `#[cfg]` suppressions, no
   stubs that panic, no feature gates that hide the problem.
4. The stub inventory from S01a.1 remains accurate: no new
   `unimplemented!()`, `unreachable!()`, or `todo!()` sites introduced by
   the repair.
5. Every `use crate::*;` glob inside `crates/flui-core/src/platform/windows/**`
   that S01a.3's explicit re-export list didn't make reachable is either
   replaced by an explicit import list, or the necessary items are
   surfaced at an appropriate module level to make the glob resolve.
6. After S01a.4 lands, Windows can be added to CI in a later spec without
   additional repair work.

## Non-goals

- Not adding Windows to CI. That's a later spec (possibly S01a.5 or part
  of the follow-up after the S01a family lands). S01a.4 proves Windows
  compiles; a separate step wires it into CI.
- Not touching macOS or Linux code paths except incidentally (if a shared
  file needs a trivial change, document it; don't refactor).
- Not refactoring the Windows platform layer. Mechanical imports only.
- Not upgrading the `windows` crate major version. Stay on `0.61`.
- Not adding Windows release-mode build verification (that requires
  `fxc.exe` and the Windows SDK; deferred to S05 when we migrate Windows
  to `flui-platform`).
- Not repairing `key_dispatch.rs`, `action.rs`, or any non-platform
  module. If these have `unimplemented!()` sites, they stay.
- Not fixing shader compilation for Windows debug (already correctly
  skipped by `build.rs:184`).

## Current state — error categories

Captured from `cargo build -p flui-core 2>&1` on
`c:/Users/vanya/RustroverProjects/flui-v2` at commit immediately after
S01a.1/.2/.3 land. Error count on the pre-repair tree: **257 errors, 44
warnings**. Representative samples follow; the implementer re-runs the
build and works through each category until zero.

### Category 1 — Missing `windows` crate feature `Win32_Media` (1 root cause, ~2-3 errors)

**Site:**
[`crates/flui-core/src/platform/windows/dispatcher.rs:15`](../../crates/flui-core/src/platform/windows/dispatcher.rs#L15):

```rust
use windows::Win32::{
    ...,
    Media::{timeBeginPeriod, timeEndPeriod},
    ...,
};
```

**Error:**
```
E0432: unresolved import `windows::Win32::Media`
note: found an item that was configured out
      the item is gated behind the `Win32_Media` feature
```

**Fix:** add `"Win32_Media"` to the `features` list of the `windows` dep
in `crates/flui-core/Cargo.toml` (the Windows target block starting
around line 202, current feature list visible at `Cargo.toml:206-252`).
Alphabetical placement between `Win32_Graphics_Imaging` and
`Win32_Networking_WinSock`.

**Cascade:** one added feature resolves one error. Re-run to confirm.

### Category 2 — `windows::core::w!` path not resolving (20+ errors across 5 files)

**Sites** (partial list from grep):
- `windows/window.rs:669`, `:673`, `:677`, `:1309` — 4 sites with
  `windows::core::w!("...")`.
- `windows/platform.rs:1032`, `:1170`, `:1171` — 3 sites.
- `windows/direct_write.rs:1878`, `:1600`, `:1601` — 3 sites including
  `windows::core::Result` and `windows::core::Error`.
- Several more at `-Read` time in implementation.

**Error shape:**
```
E0433: failed to resolve: could not find `core` in `windows`
```

**Root cause:** the `windows` crate at 0.61 re-exports its `core` module
through the `windows-core` crate (which is declared separately at
`Cargo.toml:253`: `windows-core = "0.61"`). The `windows::core` path
works only when certain features are enabled or the module is otherwise
reachable. In the current Cargo.toml state, it is not.

**Possible fixes** (implementer picks one and sticks with it, then
re-runs):

- **(A)** Replace every `windows::core::X` reference with `windows_core::X`.
  The `windows-core` crate is already in Cargo.toml at the same version
  and provides `w!`, `Result`, `Error`, `HSTRING`, `PCWSTR`, `PCSTR`.
  This is a mechanical `sed`-style edit, ~20 lines changed, no behavior
  change.
- **(B)** Add the `windows` feature that enables the `core` submodule
  re-export. Research what that feature is — possibly `"Win32"` or an
  implicit umbrella. If such a feature exists and is free (no runtime
  cost), this is simpler but discovery-dependent.

**Recommendation:** **Option A**. `windows-core` is already imported and
intended as the public API surface for these constants. The `windows::core`
alias is a convenience that has broken with some feature change upstream;
going through the explicit crate is more stable.

### Category 3 — `use crate::*;` globs in `platform/windows/**` not resolving

**Files with the broken glob** (from S01a.1's import survey, 6 files, 12
total glob sites — each file has BOTH `use crate::*;` AND
`use flui_core::*;`):
- `windows/direct_write.rs:26-27`
- `windows/directx_renderer.rs:23-24`
- `windows/events.rs:20-21`
- `windows/platform.rs:30-31`
- `windows/util.rs:17-18`
- `windows/window.rs:29-30`

**Symbols that fail to resolve through the glob** (representative list
from the build output; full list collected during implementation):

- **Intra-windows types** not reaching crate root:
  - `HWND`, `SafeHwnd`
  - `DirectXAtlas`, `DirectXDevices`
  - `WindowsWindowInner`
  - `RawShaderBytes`, `ShaderModule`, `ShaderTarget` (from
    `windows/directx_renderer/shader_resources` or similar)
- **Intra-windows free functions**:
  - `logical_point`
  - `with_dll_library`
  - `try_to_recover_from_device_lost`
- **Intra-windows constants**:
  - `WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD`
  - `WM_GPUI_CLOSE_ONE_WINDOW`
  - `WM_GPUI_CLOSE_ALL_WINDOWS`
  - `WM_GPUI_DOCK_MENU_ACTION`
  - `WM_GPUI_KEYBOARD_LAYOUT_CHANGED`
  - `WM_GPUI_GPU_DEVICE_LOST`

**Root cause:** `pub(crate) mod windows;` at `platform.rs:12` gates the
entire windows subtree as crate-internal. Items inside are `pub` at the
module level but are NOT re-exported to crate root. The
`use crate::*;` + `use flui_core::*;` pattern assumes the symbols reach
crate root, but they don't — most of them are only reachable via
`crate::platform::windows::<item>`.

**Error shapes:**
```
E0432: unresolved imports `crate::HWND`, `crate::SafeHwnd`,
       `crate::WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD`
       help: consider importing this struct through its public re-export instead:
             crate::windows::HWND
E0425: cannot find type `DirectXAtlas` in this scope
       help: consider importing this struct through its public re-export
             use crate::windows::DirectXAtlas;
E0422: cannot find struct `DirectXDevices` in this scope
E0408: variable `WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD` is not bound in all patterns
       (consequence of the earlier unresolved import cascading into
        match arms in window_proc dispatch)
```

**Note:** the compiler's own help text suggests `crate::windows::HWND`
— which means the `windows` module IS reachable as `crate::windows`.
This is the key insight. The pattern
`pub(crate) mod windows` at `platform.rs:12` combined with
`pub(crate) use windows::*;` somewhere (probably in `platform.rs` itself
or `platform/windows.rs`) is what makes `crate::windows::X` work. But
`crate::X` does not.

**Fix procedure:**

For each of the 6 files, replace `use crate::*;` and `use flui_core::*;`
with an explicit import list. Discovery method:

1. Comment out both glob lines.
2. Run `cargo build -p flui-core`.
3. Collect every `E0412`/`E0422`/`E0425`/`E0433` "cannot find X in this
   scope" error.
4. For each missing name, determine its source module and add an explicit
   `use` statement at the top of the file.
5. Repeat until the file compiles.

Because there are 6 files and the intersection of symbols used is
substantial, expect ~50-150 `use` statements across the 6 files combined.
The implementation PR commits the expanded lists verbatim.

**Side-effect of this fix:** the resulting `use` lists form an import
audit for each Windows file. S01a.1's `check-platform-imports` xtask
subcommand remains useful but will have fewer glob sites to report after
S01a.4 lands.

### Category 4 — `crate::directx_renderer::shader_resources` path does not exist

**Site:**
[`crates/flui-core/src/platform/windows/directx_renderer.rs:22`](../../crates/flui-core/src/platform/windows/directx_renderer.rs#L22):

```rust
use crate::directx_renderer::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};
```

**Error:**
```
E0433: failed to resolve: unresolved import
       help: a similar path exists: `windows::directx_renderer`
```

**Root cause:** same as Category 3 — the `use crate::*;` conventions in
this file assume `directx_renderer` is at the crate root, but it's at
`crate::platform::windows::directx_renderer`. The file is importing from
itself via the wrong path.

**Fix:** change to
`use crate::platform::windows::directx_renderer::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};`
OR (preferred) `use self::shader_resources::{...};` since the types are
in a submodule of the same file.

Verify the submodule exists — grep `mod shader_resources` inside
`directx_renderer.rs`. If it's an inline `mod shader_resources { ... }`,
`self::` is the right path. If it's a separate file, the path is
`use crate::platform::windows::directx_renderer::shader_resources::*;`.

### Category 5 — Unused-variable warnings cascading from Category 3

After Category 3 is fixed, the 44 warnings about
`unused variable: WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD` (and similar)
should resolve automatically — they're symptoms of the match arms not
being able to bind the constants, which happens because the imports
failed.

If the warnings persist after Categories 1-4, they are real unused
constants and get cleaned up as a final sweep. Do not silence them with
`#[allow(unused_variables)]`.

## Design

The "design" is a repair procedure:

### Step 1 — Cargo.toml feature add

Add `"Win32_Media"` to the `windows` dep features list in
`crates/flui-core/Cargo.toml`. Alphabetical placement. One commit hunk.

Verify: `cargo build -p flui-core 2>&1 | grep -c "Win32_Media"` returns 0.

### Step 2 — `windows::core` path sweep

Replace every `windows::core::` with `windows_core::` across the Windows
platform tree. Command template (developer runs locally, not committed
as a script):

```bash
# Dry run:
rg 'windows::core::' crates/flui-core/src/platform/windows/

# Apply (manual edits, verify each):
# For each hit, change `windows::core::X` → `windows_core::X`
```

Verify: `rg 'windows::core::' crates/flui-core/src/platform/windows/`
returns zero matches after the sweep.

Verify: `cargo build -p flui-core 2>&1 | grep -c "could not find \`core\` in"`
returns 0.

### Step 3 — Expand the 6 Windows file globs

For each file in `platform/windows/{direct_write, directx_renderer,
events, platform, util, window}.rs`:

1. Delete lines `use crate::*;` and `use flui_core::*;`.
2. Run `cargo build -p flui-core`.
3. For each "cannot find X in this scope" error, add an explicit `use`
   statement. Prefer `use super::X;` for same-directory items,
   `use crate::platform::windows::submod::X;` for nested ones, and
   `use crate::X;` for genuinely crate-root items (the S01a.3 enumerated
   list decides which names qualify).
4. Repeat until the file compiles.
5. Move to the next file.

Expected final state: 0 glob imports in `platform/windows/**`, every
name in the file resolvable via an explicit `use`.

This is mechanical but tedious. Budget 2-4 hours for the full six-file
sweep. The implementer should commit each file's expanded imports as a
separate logical chunk within the same PR (or as separate commits in a
stack that all land together).

### Step 4 — `directx_renderer::shader_resources` path fix

One-line edit. Preferred form: `use self::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};`.

### Step 5 — Warning sweep

After Steps 1-4, re-run `cargo build -p flui-core` and collect remaining
warnings. For each:

- If genuinely unused: delete the unused item.
- If suppressed by an intentional glob that's now gone: add explicit
  `#[allow(...)]` at the narrowest scope possible.
- If a true warning exposed by Windows-specific code that was never
  compiled under CI: fix it.

Warnings must drop to zero OR have a justification comment.

### Step 6 — Run the S01a.1 stub check

```
cargo xtask check-stubs
```

If the path counts have shifted (e.g. because dead code gets removed
during the warning sweep), run `--bless` and commit the updated fixture.

### Step 7 — Feature matrix verification

```
cargo check -p flui-core
cargo check -p flui-core --no-default-features
cargo check -p flui-core --features test-support
cargo check -p flui-core --features inspector
cargo check -p flui-core --all-features
```

All must exit 0 on Windows 11.

### Step 8 — Sibling verification on Windows

```
cargo check -p flui-navigator -p flui-widgets -p flui-material \
           -p flui-theme -p flui-a11y -p flui-animate
```

Must exit 0. If any sibling breaks, the fix is in the sibling's import
pattern — not in `flui-core`. Document any sibling change in the PR.

## API surface

**Zero change** to the public API.

The `windows` crate's `core` submodule is an implementation detail of
the upstream crate. Switching from `windows::core::*` to
`windows_core::*` is a code-level change invisible to consumers of
`flui-core`.

Expanding glob imports to explicit imports is visibly a diff inside
windows files but does not affect any `pub` surface.

Adding `Win32_Media` to the features list expands the `windows` crate's
surface inside `flui-core` (enables previously-gated items), but none
of those items are re-exported to downstream.

## Migration / Compatibility

Zero breaking changes. Windows is not currently in CI; no downstream
consumer exists that depends on today's broken state.

## Testing strategy

1. **`cargo build -p flui-core` on Windows 11** — must exit 0. Captured
   before/after build logs attached to the PR.
2. **Error count drop** — before: 257 errors, 44 warnings. After: 0
   errors, 0 or documented warnings.
3. **Feature matrix** on Windows 11 (see Step 7 above).
4. **Sibling canary** on Windows 11 (see Step 8 above).
5. **Re-run the same commands on Linux and macOS** — the changes must
   NOT break Linux/macOS. `cargo check` green on both.
6. **Stub inventory** via `cargo xtask check-stubs` green (either
   unchanged counts, or updated fixture committed in the same PR).

**No new tests are added.** The existing test suite on Linux/mac is the
safety net for cross-platform regressions. The Windows fix is functional
validation by build, not unit tests.

## Open questions

- **Exact `windows::core` replacement strategy** — Option A (swap to
  `windows_core::` prefix) vs Option B (find and enable the right
  `windows` crate feature). Implementation picks one; recommendation is
  A for stability.
- **Whether `HWND` needs to be brought to crate root** — if the glob
  expansion reveals that every Windows file re-imports `HWND` from
  `crate::platform::windows::HWND`, and the type is frequently used, a
  `pub use platform::windows::HWND;` at `platform.rs` (behind
  `#[cfg(target_os = "windows")]`) might be cleaner. Decision deferred
  to the implementer; the spec accepts either.
- **Remaining warnings after the fix** — cannot be pre-enumerated. Spec
  commits to "zero or justified" as the bar.
- **Whether S01a.4 should also add Windows to CI** — no. CI addition is
  a separate, trivial follow-up PR that needs a `windows-latest` runner
  slot. Keeping it separate makes rollback atomic.
- **Release-mode Windows build** — still requires FXC.exe + Windows SDK.
  Not in scope for S01a.4. A later spec (probably part of S05) tackles
  release-mode + adds Windows to CI with the SDK install.

## Done criteria

- [ ] `cargo build -p flui-core` exits 0 on Windows 11, default features,
      debug profile.
- [ ] `cargo check -p flui-core --no-default-features` exits 0.
- [ ] `cargo check -p flui-core --features test-support` exits 0.
- [ ] `cargo check -p flui-core --features inspector` exits 0.
- [ ] `cargo check -p flui-core --all-features` exits 0.
- [ ] `cargo check -p flui-navigator -p flui-widgets -p flui-material -p flui-theme -p flui-a11y -p flui-animate`
      exits 0 on Windows 11.
- [ ] Zero `windows::core::` references remain under
      `crates/flui-core/src/platform/windows/`.
- [ ] Zero `use crate::*;` or `use flui_core::*;` glob imports remain
      under `crates/flui-core/src/platform/windows/`.
- [ ] `Win32_Media` feature added to `windows` dep in `Cargo.toml`.
- [ ] `cargo xtask check-stubs` green (either unchanged, or fixture
      re-blessed in the same PR with documented delta).
- [ ] `cargo check -p flui-core` on Linux and macOS still green (no
      regression from the repair).
- [ ] Warning count is zero OR documented in the PR description with
      justification per warning.
- [ ] Before/after `cargo build -p flui-core` logs attached to the PR
      (even if truncated).
- [ ] Commit is a single PR.

## Test log

To be filled during implementation.

### Error-category progress

| Category | Initial error count | After fix | Status |
|---|---|---|---|
| 1. `Win32_Media` missing | ~2 | 0 | TBD |
| 2. `windows::core::` paths | ~20 | 0 | TBD |
| 3. Windows file globs | ~200+ | 0 | TBD |
| 4. `shader_resources` path | ~3 | 0 | TBD |
| 5. Cascading warnings | ~44 | TBD | TBD |

### Feature matrix (Windows 11)

| Features | Status |
|---|---|
| default | TBD |
| --no-default-features | TBD |
| --features test-support | TBD |
| --features inspector | TBD |
| --all-features | TBD |

### Sibling canary (Windows 11)

| Crate | Status |
|---|---|
| flui-navigator | TBD |
| flui-widgets | TBD |
| flui-material | TBD |
| flui-theme | TBD |
| flui-a11y | TBD |
| flui-animate | TBD |

### Cross-platform regression

| Target | Before | After |
|---|---|---|
| Linux `cargo check` | green | TBD |
| macOS `cargo check` | green | TBD |

### Remaining warnings

TBD — enumerate or confirm zero.

## Follow-ups after S01a.4 lands

- **S02 unblocked** on the Windows front — the flui-platform extraction
  can now move Windows code without tripping over a pre-existing broken
  build.
- **Add Windows to CI** — small separate spec. Install `windows-latest`
  runner, `cargo check` + `cargo clippy`, NOT `cargo test` (no display
  server), explicitly opt out of release-mode shader compilation via the
  existing `#[cfg(not(debug_assertions))]` gate. No Windows SDK install
  needed for debug-mode check.
- **Release-mode Windows** — deferred to S05 (when migrating Windows
  code to flui-platform). Requires FXC.exe via Windows SDK install in
  CI; non-trivial runner config.
- **`HWND` crate-root hoist** — optional cleanup; if the glob expansion
  shows `HWND` imported ~20 times across Windows files, consider a
  target-gated `pub use platform::windows::HWND;` at the crate root.
  Out of scope for S01a.4 itself.
