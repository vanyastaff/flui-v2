---
spec_id: S01d
title: lock-extraction-facades
phase: I
depends_on: [S01a.3]
blocks: [S02]
status: draft
date: 2026-04-13
---

# S01d — lock-extraction-facades

## Context

Fourth and final atomic sub-step of S01a. Where S01a.1/.2/.3/.4 cleaned
up ground truth, killed dead code, replaced the platform glob with an
explicit list, and repaired the Windows build, S01d is the last
preparation step before S02 starts the actual extraction.

S01d resolves two outstanding decisions that S02 needs answered:

1. **How will the new `flui-platform` crate access flui-core's platform
   trait contracts?** Today `pub(crate) mod {mac, linux, windows, wgpu,
   web}` at `platform.rs:6-18` keeps every platform implementation as
   crate-internal. After extraction, those impls live in `flui-platform`
   and need to import the Platform / PlatformWindow / etc. traits from
   `flui-core` AND construct types defined in `flui-core`'s submodules.
   The visibility shape has to be decided once before extraction starts.

2. **`WebWindowInner` facade.** The roadmap's deep analysis flagged
   `crate::window::WebWindowInner` as a private type imported across the
   `events.rs` boundary. S01d either confirms the path is currently
   broken (web is never built in CI), in which case we leave it to S06
   when web migrates, or it pre-introduces a `#[doc(hidden)]` callback
   facade that S06 can plug into without re-architecting.

S01d is the **smallest** of the four S01a sub-steps. Most of its work is
documentation and decision-making, with at most a small amount of
visibility/facade code if the decisions land that way.

## Goals

1. Decide and document the module visibility strategy for
   `crates/flui-core/src/platform/{mac, linux, windows, wgpu, web}`
   after extraction. Pick one of:
   - **(A)** Promote each `pub(crate) mod` to `pub mod` and let
     `flui-platform` consume them via `flui_core::platform::<os>::*`.
   - **(B)** Add named `pub use` re-exports inside `platform.rs` for
     every type `flui-platform` needs.
   - **(C)** Introduce a `pub(crate) trait PlatformImpl` private to
     `flui-core` that the `flui-platform` types implement via a sealed
     mechanism.
2. If the decision requires any `pub(crate) → pub` promotions in
   flui-core, apply them as part of S01d and update S01a.3's
   enumerated re-export list to include them.
3. Verify the current state of `platform/web/events.rs:12 use crate::window::WebWindowInner;`
   — does it compile today? If yes, classify why; if no, document the
   broken state in `docs/lock-coverage-gaps.md`.
4. If web compiles today, introduce a `#[doc(hidden)]`
   `WebWindowCallbacks` (or similar) facade so S06 can keep web
   working without depending on a private type at extraction time.
5. Decide and document what happens to the `ScreenCaptureFrame`
   wrapper after S01a.2 collapses `PlatformScreenCaptureFrame` to
   `()` — leave as `pub struct ScreenCaptureFrame(pub ())`, or
   collapse to a unit type, or make opaque.

## Non-goals

- Not creating `flui-platform`. That's S02.
- Not moving any code out of `flui-core`. That's S02-S06.
- Not refactoring the Platform trait. The contract stays as-is.
- Not adding any new Platform implementation methods.
- Not unifying any cross-platform code paths. Each backend stays in
  its current form.
- Not re-introducing screen capture. S01a.2 already deleted the dead
  feature; S01d only addresses the leftover `ScreenCaptureFrame`
  shape.
- Not fixing any `unimplemented!()` sites.
- Not running golden tests.

## Current state

### Module visibility today

[`crates/flui-core/src/platform.rs:6-18`](../../crates/flui-core/src/platform.rs#L6-L18):

```rust
#[cfg(target_os = "macos")]
pub(crate) mod mac;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) mod linux;

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(any(target_os = "linux", target_os = "freebsd", target_family = "wasm"))]
pub(crate) mod wgpu;

#[cfg(target_family = "wasm")]
pub(crate) mod web;
```

All five are `pub(crate)`. Everything they define is invisible outside
the `flui-core` crate.

Today, `flui-core` re-exports the concrete `XxxPlatform` types via:

```rust
#[cfg(target_os = "macos")]
pub use mac::MacPlatform;

#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;
```

That's enough to let `Application::with_platform(Rc::new(MacPlatform::new(false)))`
work from a downstream crate. But every supporting type
(`MetalRenderer`, `MetalAtlas`, `MacWindow`, `MacDispatcher`,
`MacTextSystem`, etc.) is unreachable from outside `flui-core`.

After extraction, those supporting types must live somewhere the
`flui-platform` crate can construct them, which means **either** they
move to `flui-platform` entirely (and `flui-core` just owns the
traits), **or** they stay in `flui-core` with promoted visibility
(and `flui-platform` consumes them via `flui_core::platform::<os>::*`).

### `WebWindowInner` reference today

[`crates/flui-core/src/platform/web/events.rs:12`](../../crates/flui-core/src/platform/web/events.rs#L12):

```rust
use crate::window::WebWindowInner;
```

Verified by grep: `WebWindowInner` actually lives at
[`crates/flui-core/src/platform/web/window.rs:45`](../../crates/flui-core/src/platform/web/window.rs#L45)
as `pub(crate) struct WebWindowInner`. The path
`crate::window::WebWindowInner` does NOT match the actual location
unless there's a re-export from the crate-level `mod window;` at
`lib.rs:63` — which there isn't.

**Conclusion:** the import in `events.rs:12` is broken in the static
sense. The reason it has not been caught is that `mod web;` at
`platform.rs:17-18` is gated on `target_family = "wasm"`, and CI never
compiles for `wasm32-unknown-unknown`.

S01d records this in `docs/lock-coverage-gaps.md` and decides whether
to fix it now or defer.

### `ScreenCaptureFrame` after S01a.2

S01a.2 collapses `PlatformScreenCaptureFrame` to
`pub(crate) type PlatformScreenCaptureFrame = ();`. That makes
`pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame)` →
`pub struct ScreenCaptureFrame(pub ())`. The `pub ()` field is
useless (anyone can construct `ScreenCaptureFrame(())` but the wrapped
value carries no information).

Two cleanup options for S01d:

- **Leave it.** Cosmetic ugliness, zero work, no breaking change.
- **Make opaque.** Change to `pub struct ScreenCaptureFrame { /* private */ }`.
  Construct via `pub(crate) fn new() -> Self`. Breaking change for
  anyone who matches on `ScreenCaptureFrame(_)` — but there are no
  such consumers since the feature has never worked.

S01d picks option **make opaque** — small breaking change, clean
shape, no regression risk because nothing constructs the type today.

## Design

### Decision 1 — module visibility strategy

Three options compared:

| Option | flui-core change | flui-platform reach | Maintenance |
|---|---|---|---|
| **A. `pub mod`** | `pub(crate) mod mac` → `pub mod mac` (etc.) | `flui_core::platform::mac::*` | Smallest churn now; biggest semver surface forever — every type inside the OS subtree becomes part of the crate's public API |
| **B. Named re-exports in `platform.rs`** | Add ~30-50 `pub use mac::TypeName;` lines (per OS) | `flui_core::TypeName` | Medium churn; controlled semver surface (only listed types are public); has to be kept in sync with the actual types `flui-platform` needs |
| **C. Sealed `PlatformImpl` trait** | New trait + extension methods | `flui_core::PlatformImpl::*` | Largest churn; cleanest semver story; requires `flui-platform` types to be designed against the trait surface from day one |

**Recommendation:** **Option B** — named re-exports.

**Why not A:** promoting `pub(crate) mod mac` to `pub mod mac` would
drag every internal type, function, and module reachable from
`mac/{mod.rs, platform.rs, dispatcher.rs, display.rs, display_link.rs,
window.rs, keyboard.rs, events.rs, text_system.rs, metal_renderer.rs,
metal_atlas.rs, pasteboard.rs, screen_capture.rs, open_type.rs,
window_appearance.rs}` into the crate-level public API. ~50+ types and
functions per OS × 4 OS subtrees = 200+ items to commit to forever. The
cost-of-ownership is unacceptable for a roadmap that will iterate on
the platform layer.

**Why not C:** the sealed trait approach is theoretically the cleanest,
but it requires the `flui-platform` types to be designed against the
trait from day one. They aren't — they're existing concrete types with
inline impls. Refactoring all of them to fit a new trait surface is
weeks of work that isn't on the critical path. C is the right answer
for a green-field design; for a brown-field migration, it's
over-engineering.

**Option B specifics:** S01d does NOT add the re-exports themselves —
that's S02's job, because the actual list of types `flui-platform`
needs depends on which files move where. Instead, S01d documents the
**process**:

1. When S02 creates `flui-platform`, it scans the `mac/`, `linux/`,
   `windows/`, `wgpu/`, `web/` subtrees and lists every type/function
   that needs to be reachable from outside `flui-core` (the union of
   "things `flui-platform` will move" minus "things that stay in
   `flui-core`").
2. For each item that stays in `flui-core` but must be reachable from
   `flui-platform`, S02 adds a named `pub use mac::TypeName;` (or
   `linux::`, etc.) inside `platform.rs` AND updates S01a.3's
   enumerated re-export list at `lib.rs:117` to include them.
3. For each item that moves to `flui-platform`, no `flui-core` change
   is needed — the type's new home is `flui_platform::<os>::TypeName`.

**Outcome documented in S01d:** decision is "Option B — named re-exports
added incrementally during S02-S06 as each backend moves." S01d itself
adds **zero** new re-exports. The decision is the deliverable.

### Decision 2 — `WebWindowInner` reference

`platform/web/events.rs:12 use crate::window::WebWindowInner;` does not
resolve. The web subtree doesn't compile in CI, so this has been
silently broken for some time.

Two paths:

- **(a) Fix it now.** Change the import to
  `use super::window::WebWindowInner;` and verify with
  `cargo check --target wasm32-unknown-unknown` (which itself requires
  some `wasm-bindgen` / `web-sys` deps that may not be in `Cargo.toml`).
  This is more work than expected because **the wasm target isn't
  fully wired**.
- **(b) Document and defer to S06.** S06 owns the web migration; it
  will inherit the broken state and fix it then. S01d records the
  brokenness in `docs/lock-coverage-gaps.md`.

**Recommendation: (b) defer to S06.** S01d already has enough scope.
Fixing the wasm build is a multi-step task that includes adding wasm
deps (`wasm-bindgen`, `web-sys`, `js-sys`) to `Cargo.toml` under
`[target.'cfg(target_family = "wasm")'.dependencies]` (which currently
has none), and then chasing down whatever else is broken. That's S06's
problem.

The "WebWindowCallbacks facade" idea from the original roadmap is
also deferred to S06 — there's no point designing a facade for a
broken type until the type's environment compiles.

**S01d action:** add an entry to `docs/lock-coverage-gaps.md`:

```markdown
## Web platform — never compiled in CI

Status: broken — `platform/web/events.rs:12` imports
`crate::window::WebWindowInner` which does not resolve to a defined
symbol. The path expects a re-export from the crate-level `mod window;`
at `lib.rs:63` that does not exist.

Verified by: grep on `WebWindowInner`. Actual definition is at
`platform/web/window.rs:45` (`pub(crate) struct WebWindowInner`).
The `crate::window::*` path is wrong; the correct path would be
`crate::platform::web::window::WebWindowInner` or
`super::window::WebWindowInner`.

Fix is deferred to S06 (web migration), which also adds wasm32 deps to
Cargo.toml so the web subtree compiles in CI for the first time.
```

### Decision 3 — `ScreenCaptureFrame` opacity

After S01a.2, the type degenerates to `pub struct ScreenCaptureFrame(pub ())`.
S01d cleans this up:

```rust
/// Opaque platform-specific screen capture frame.
///
/// Currently a placeholder — `flui-core` has no screen capture
/// implementation. The type exists as a future extension point: when
/// a backend is reintroduced, this struct will gain real fields and
/// `pub fn new(...)` constructors. Today it is uninhabited from outside
/// `flui-core`.
pub struct ScreenCaptureFrame {
    _private: PlatformScreenCaptureFrame,
}
```

The `_private` field uses the (now `()`) `PlatformScreenCaptureFrame`
type alias to keep the indirection in place. External code can no
longer construct `ScreenCaptureFrame(())` because the field is
private. This is a **breaking change** for any external code that does
construct it — but S01a.2 already documented that no such code exists,
because the feature has never worked.

Add a `#[allow(dead_code)]` if rustc complains about the unused field
— it has documentation value as a placeholder for the future shape.

S01a.3's enumerated re-export list already exports
`ScreenCaptureFrame`, so no `lib.rs` change is needed.

## API surface

**Changes:**

- `pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame)` →
  `pub struct ScreenCaptureFrame { _private: PlatformScreenCaptureFrame }`.
  Public field removed; replaced with private placeholder. Breaking
  change for tuple-struct constructors and pattern matches — none
  exist today.

**No other public API changes.**

**Documentation additions:**

- `docs/lock-coverage-gaps.md` gets a "Web platform" entry (or extends
  the existing entry from S01c).
- `docs/superpowers/specs/2026-04-13-S01d-lock-extraction-facades-design.md`
  (this file) — committed as the design record.
- A new comment block in `crates/flui-core/src/platform.rs` describing
  the "Option B" re-export strategy that S02-S06 will follow:

```rust
// --- Platform module visibility strategy (decided by S01d) ---
//
// The five OS-specific submodules (`mac`, `linux`, `windows`, `wgpu`,
// `web`) are `pub(crate)`. Their internals are NOT part of the public
// API; the only exported items are the concrete `XxxPlatform` types
// re-exported below.
//
// When code from these modules moves to the future `flui-platform`
// crate, any type that must remain in `flui-core` AND be reachable
// from `flui-platform` will be added as a named `pub use` re-export
// here, and the symbol added to the explicit list at `lib.rs:117`.
//
// Adding a new public re-export is a semver event. Do not add one
// without a documented reason in the relevant migration spec
// (S02-S06).
```

## Migration / Compatibility

- **`ScreenCaptureFrame` field removal:** breaking for tuple-struct
  use; zero impact in practice.
- **No internal workspace changes** — siblings don't construct
  `ScreenCaptureFrame`.
- **No other compatibility concerns.**

## Testing strategy

S01d adds no new tests. Existing tests remain green:

1. `cargo check -p flui-core` on Linux + macOS — unaffected by
   `ScreenCaptureFrame` field swap.
2. `cargo check -p flui-widgets -p flui-material -p flui-navigator -p flui-a11y -p flui-theme -p flui-animate`
   — sibling canary remains green.
3. `cargo test --workspace --features test-support` — still green.
4. `cargo xtask check-stubs` — unchanged.
5. `cargo doc -p flui-core` — `ScreenCaptureFrame`'s rustdoc renders
   the new placeholder docstring.

## Open questions

- **Was `WebWindowInner` ever reachable?** If a previous wasm build
  worked, there must have been a re-export at some earlier point.
  Worth a `git log` archaeology run during implementation, but not
  blocking.
- **Should the platform.rs strategy comment also mention S07-S20?**
  No — those are subsystem additions, not extractions, and don't need
  the re-export pattern. Comment scopes to the migration phase.
- **Sealed-trait variant for the future** — Option C is rejected for
  S01d/S02-S06, but if `flui-platform` ever wants to add a third-party
  backend (e.g. winit-based fallback), the sealed-trait approach
  becomes attractive. Documenting the rejection now means a future
  spec can revisit with full context.
- **`#[allow(dead_code)]` on the `_private` field** — does rustc need
  it? Verify at impl time.

## Done criteria

- [ ] `crates/flui-core/src/platform.rs:31-44` (post-S01a.2 location of
      `ScreenCaptureFrame`) updated to opaque struct with `_private`
      field.
- [ ] `pub struct ScreenCaptureFrame` has a doc comment explaining its
      placeholder status.
- [ ] `crates/flui-core/src/platform.rs` near the `mod {mac, linux,
      windows, wgpu, web};` declarations has the visibility-strategy
      comment block.
- [ ] `docs/lock-coverage-gaps.md` has a "Web platform" entry
      explaining the broken `WebWindowInner` import path and pointing
      to S06 as the fix owner.
- [ ] `cargo check -p flui-core -p flui-widgets -p flui-material -p flui-navigator -p flui-a11y -p flui-theme -p flui-animate`
      green on Linux and macOS.
- [ ] `cargo test --workspace --features test-support` green on Linux
      and macOS.
- [ ] `cargo xtask check-stubs` (from S01a.1) green.
- [ ] `cargo doc -p flui-core --no-deps` green; `ScreenCaptureFrame`
      rustdoc renders.
- [ ] Single atomic commit.

## Test log

To be filled during implementation.

### Sibling canary

| Crate | Linux | macOS |
|---|---|---|
| flui-core | TBD | TBD |
| flui-widgets | TBD | TBD |
| flui-material | TBD | TBD |
| flui-navigator | TBD | TBD |
| flui-a11y | TBD | TBD |
| flui-theme | TBD | TBD |
| flui-animate | TBD | TBD |

### `ScreenCaptureFrame` doc render

```
$ cargo doc -p flui-core --no-deps
$ open target/doc/flui_core/struct.ScreenCaptureFrame.html
# Expected: docstring visible, no fields shown publicly
```

## Follow-ups after S01d lands

- **S02 unblocked.** All four S01a.x sub-steps are done; S01b and S01c
  golden + behavior locks are in place; the visibility strategy is
  documented; the `flui-platform` crate skeleton can be created.
- **S06 inherits the web platform repair.** The `docs/lock-coverage-gaps.md`
  entry tells whoever picks up S06 exactly what's broken and why.
- **No further S01.x specs.** S01a.1 + S01a.2 + S01a.3 + S01a.4 + S01b +
  S01c + S01d == complete lock phase.
