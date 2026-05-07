---
spec_id: S07
title: gesture-arena
phase: II
depends_on: [S01a.3, S01a.4, S01b, S01c, S01d, S02a]
blocks: [S08, S12, S14]
status: draft
date: 2026-05-06
---

# S07 — GestureArena

## Context

`flui-core` currently routes pointer-class input through the implicit
hitbox infrastructure committed during paint. Each `InteractiveElement`
attaches `mouse_*` listeners directly
([`crates/flui-core/src/elements/div.rs:1723-1742`](../../../crates/flui-core/src/elements/div.rs#L1723-L1742)),
and `Window::dispatch_event`
([`crates/flui-core/src/window.rs:4074`](../../../crates/flui-core/src/window.rs#L4074))
calls `self.dispatch_mouse_event` after coalescing the position into
`self.mouse_position`. There is no explicit hit-test pass before
propagation; instead, listeners fire in paint order, and the
[`HitTest`](../../../crates/flui-core/src/window.rs#L540-L544) struct
(`SmallVec<[HitboxId; 8]>` plus `hover_hitbox_count`) is queried
out-of-band by `is_hovered`/`should_handle_scroll` for hover-state
styling.

The result is a working, but **incomplete**, input model:

- There is no notion of competing recognizers. Two overlapping elements
  that both want to claim a tap have no shared arbitration; whichever
  one calls `cx.stop_propagation()` first wins, and that order is
  determined by paint order, not by intent.
- There are no high-level recognizers (tap, double-tap, long-press,
  drag, scale). Each consumer of `Interactivity` reimplements them on
  top of `mouse_down` / `mouse_up` / `mouse_move`.
- There is no normalized `PointerEvent`. The platform input enum is
  mouse-shaped (`MouseDownEvent`, `MouseMoveEvent`, …); touch arrives
  as mouse events on macOS trackpad / Wayland; stylus has no
  representation at all.
- There is no `PointerSanitizer`. Orphaned `Down` events (e.g. from
  modal switches, focus loss) and duplicate `Down` events leak through
  the dispatch chain.
- There is no per-Window owner of input policy (`GestureSettings`).
  Slop thresholds, double-tap windows, and long-press timings are
  hard-coded inside individual recognizers when consumers write them.

This spec adds an explicit `HitTestResult`/`HitTestEntry` protocol with
`HitTestBehavior`, a normalized `PointerEvent` (and a separate
`PointerSignalEvent` for scroll / magnify), a Flutter-style
`GestureBinding` per `Window`, a `GestureArenaManager` with eager-accept
semantics, a captain-deferred `GestureArenaTeam`, a
`PointerSanitizer`, a shared `VelocityTracker`, and five competing
recognizers (`Tap`, `DoubleTap`, `LongPress`, `Drag` (free, horizontal,
vertical), `Scale`). The new infrastructure is **additive**: existing
`mouse_*` listeners and the imperative `cx.active_drag` (`AnyDrag`)
flow keep firing in parallel at every commit checkpoint.

This is row **S07** of the Phase II Flutter-parity track in
[`docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`](2026-04-13-flui-core-roadmap.md).
Closing it ticks roadmap Gap **B** ("GestureArena with competing
recognizers — medium") and unblocks the seam contracts for **S08**
(semantics — `semantic_actions()` hook), **S12** (focus traversal —
`on_focus_request()` hook), and **S14** (MediaQuery completeness —
`gesture_settings_mut()` accessor).

S07 is **explicitly not** a platform-layer migration; nothing under
`crates/flui-core/src/platform/**` moves. All new code lands under
`crates/flui-core/src/gesture/**` with a clear path to
`crates/flui-platform/` (or a future `crates/flui-gesture/` sibling)
when Phase III drives a real platform-abstraction boundary.

## Goals

1. Add an explicit
   [`HitTestResult`](#hit-test-result-and-entry)/`HitTestEntry`
   protocol to `Window` with **`HitTestBehavior` (`Opaque` |
   `Translucent` | `DeferToChild`)**, on top of the existing implicit
   hitbox infrastructure
   ([`Window::mouse_hit_test`](../../../crates/flui-core/src/window.rs#L578)).
   The new protocol reuses committed `Hitbox`es; it does not replace
   them.

2. Introduce a normalized [`PointerEvent`](#pointer-event) (`Mouse` |
   `Touch` | `Stylus`, unique `PointerId(u64)`) carrying position,
   delta, buttons, modifiers, timestamp, pressure, **tilt**,
   **orientation**. Plus a separate
   [`PointerSignalEvent`](#pointer-signal-event) (`Scroll` | `Magnify`)
   that bypasses the arena.

3. Ship a Flutter-style
   [`GestureBinding`](#gesture-binding) (per-`Window` owner) holding
   the `GestureArenaManager`, a configurable
   [`GestureSettings`](#gesture-settings), and a
   [`PointerSanitizer`](#pointer-sanitizer) (synthesizes `Cancel` for
   orphaned `Down`, rejects duplicate `Down`).

4. Implement five competing recognizers ([`Tap`](#tap-recognizer),
   [`DoubleTap`](#double-tap-recognizer),
   [`LongPress`](#long-press-recognizer),
   [`Drag` (`free` + `horizontal` + `vertical`)](#drag-recognizer),
   [`Scale` (multi-pointer)](#scale-recognizer)), backed by a shared
   [`VelocityTracker`](#velocity-tracker). Each recognizer consumes
   `&GestureSettings` and exposes `semantic_actions()` (S08 seam) and
   `on_focus_request()` (S12 seam) hooks.

5. Surface the recognizer registry via
   [`Interactivity` fluent builders](#interactivity-fluent-builders)
   (`with_hit_test_behavior`, `on_tap`, `on_long_press_*`, `on_pan_*`,
   `on_horizontal_drag_*`, `on_scale_*`) without breaking existing raw
   `on_mouse_*` / `on_click` listeners or the imperative `cx.active_drag`
   (`AnyDrag`) flow.

## Non-goals

1. **Multi-touch hardware support beyond what the platform layer
   surfaces today.** Real touch on macOS trackpad and Wayland already
   arrives as
   [`PinchEvent`](../../../crates/flui-core/src/interactive.rs#L478-L494)
   (linux+macos `cfg`-gated, scale-only). Full multi-finger touch on
   Windows desktop is **not** on the platform layer yet. The Scale
   recognizer state machine supports more than two pointers, but the
   arena will only ever see up to two on current hardware.

2. **Full stylus parity.**
   [`MousePressureEvent`](../../../crates/flui-core/src/interactive.rs#L192-L204)
   is macOS-trackpad-only and carries no tilt / orientation / azimuth.
   `PointerKind::Stylus` is added as a `#[non_exhaustive]` variant so
   future platform support is non-breaking, but the platform-side
   wire-up to real stylus events is **deferred** (S20 desktop-gaps
   cleanup or a dedicated stylus spec).

3. **Pinch rotation on desktop.** `PinchEvent.delta` is `f32` (scale
   only). The Scale recognizer state machine carries a rotation field
   for future multi-pointer touch input but emits zero rotation on
   current desktop platforms; Windows currently has no native pinch at
   all. See
   [Explicit gaps](#explicit-gaps) for the matrix.

4. **Rewriting any platform-side input plumbing.**
   `crates/flui-core/src/platform/**` is unchanged — S07 only normalizes
   events on the way in via `From<PlatformInput>` conversions. The
   platform module continues to emit the existing
   [`PlatformInput`](../../../crates/flui-core/src/interactive.rs#L658-L682)
   variants verbatim.

5. **Replacing the implicit `Hitbox` infrastructure used during paint.**
   The new `HitTestResult` is **additive** and reuses committed
   hitboxes via the existing `Window::insert_hitbox` /
   `mouse_hit_test` machinery. Style-bearing concepts (`is_hovered`,
   `should_handle_scroll`,
   [`HitboxBehavior`](../../../crates/flui-core/src/window.rs#L648))
   remain orthogonal to `HitTestBehavior`.

6. **Spatial-index hit-test (BVH / quadtree / R-tree).** The current
   `SmallVec<[HitboxId; 8]>` linear scan is sufficient for trees of
   ~8–16 hitboxes typical of `flui-core` consumers. Spatial indexing
   is deferred to a P-track perf milestone and has its own row in the
   roadmap if/when measurements show it matters (typical breakpoint:
   trees > 100 hitboxes).

7. **Inertia / fling animation post-gesture-end.** The
   [`Velocity`](#velocity) payload is provided to recognizer end
   callbacks; physics integration into `AnimationController` is **S11**
   (Physics simulations).

8. **Pointer event pooling / zero-allocation dispatch on the hot
   path.** S07 keeps allocations bounded but does not pool
   `PointerEvent` objects; that is deferred to a P-track perf milestone
   if measurements justify it. See
   [Performance budgets](#performance-budgets) for the bounded-alloc
   contract.

9. **Introducing `tracing`, `criterion`, `dhat`, or `tracing-tracy`
   workspace-wide.** Logging stays on `log` + `kv_unstable_serde`
   (already present at
   [`Cargo.toml:77`](../../../crates/flui-core/Cargo.toml#L77)). The
   bench fixture follows the existing `examples/bench/*.rs` pattern
   (paths_bench, shadow, pattern, data_table). These cross-cutting
   choices belong to **A4** (Tracing standardization) and **T4**
   (Criterion benchmark suite) tracks of the roadmap.

10. **New widget types.** `GestureDetector` does not exist as a widget
    in S07 — `Interactivity` is the widget seam, and a dedicated
    `GestureDetector` widget belongs to `flui-widgets` (currently
    out-of-scope per
    [README.md](../../../README.md)).

11. **Mobile platform integration.** S17 (iOS) and S18 (Android) own
    that, post-S08 (semantics) and S16 (headless renderer). S07 only
    closes the desktop-side gap. The `PointerKind` enum is shaped to
    accept mobile-touch sources without a breaking change (`Touch`
    variant is already in the initial enum).

12. **Accessibility action plumbing.** S08 owns the semantics
    protocol; S07 only exposes the `semantic_actions()` default-empty
    hook on `GestureRecognizer` so S08 can populate it without a
    breaking change.

## Current state

### Pointer-class input today

The platform layer emits
[`PlatformInput`](../../../crates/flui-core/src/interactive.rs#L658-L682)
on every input event. The mouse-shaped variants are:

```text
KeyDown / KeyUp / ModifiersChanged       — keyboard, untouched by S07.
MouseDown(MouseDownEvent)
MouseUp(MouseUpEvent)
MousePressure(MousePressureEvent)        — #[cfg]-implicit; macOS only in practice.
MouseMove(MouseMoveEvent)
MouseExited(MouseExitEvent)
ScrollWheel(ScrollWheelEvent)
Pinch(PinchEvent)                        — #[cfg(any(linux, macos))], scale-only.
FileDrop(FileDropEvent)                  — synthesizes mouse events; reused by S07 dispatch.
```

There is no `PointerEvent`, no `PointerId`, no `PointerKind`. There is
no `PointerSignalEvent`. `MouseClickEvent` is **synthesized** by
`InteractiveElement` from a `MouseDown` followed by a `MouseUp` on the
same hitbox.

### `Window` event flow today

[`Window::dispatch_event`](../../../crates/flui-core/src/window.rs#L4074-L4193)
is the single entry point from the platform layer. It:

1. Tracks input modality (mouse vs keyboard) for hover suppression
   (`InputModality`), refreshing the window if the modality changed.
2. Resets `cx.propagate_event = true` and `self.default_prevented = false`.
3. Coalesces `position` / `modifiers` into the per-`Window` cache for
   each pointer-bearing variant.
4. Translates `PlatformInput::FileDrop` into synthetic
   `MouseMove`/`MouseUp` events (and manipulates `cx.active_drag`).
5. Routes the event to `dispatch_mouse_event` (for mouse-class inputs)
   or `dispatch_key_event` (for keyboard).
6. Records input-rate telemetry via `self.input_rate_tracker`.
7. Returns `DispatchEventResult { propagate, default_prevented }`.

`dispatch_mouse_event` then runs the existing paint-order propagation
chain through `Interactivity`'s registered listeners (mouse_down,
mouse_up, mouse_move, scroll_wheel, pinch, click). Listener firing
honors the implicit `HitboxBehavior` flags (`Normal` / `BlockMouse` /
`BlockMouseExceptScroll`) committed during paint via
`Window::insert_hitbox`.

The committed
[`Window::mouse_hit_test`](../../../crates/flui-core/src/window.rs#L578)
field is a `HitTest { ids: SmallVec<[HitboxId; 8]>, hover_hitbox_count:
usize }` (private,
[`crates/flui-core/src/window.rs:540-544`](../../../crates/flui-core/src/window.rs#L540-L544)).
It is consulted out-of-band by `HitboxId::is_hovered` and
`HitboxId::should_handle_scroll`
([`window.rs:570-593`](../../../crates/flui-core/src/window.rs#L570-L593))
for style decisions. There is no explicit `Window::hit_test(position)`
returning a typed result.

### `Interactivity` listener registry today

[`Interactivity`](../../../crates/flui-core/src/elements/div.rs#L1691-L1752)
holds the following pointer-class listener vectors (excerpted):

```rust
pub(crate) mouse_down_listeners:        Vec<MouseDownListener>,
pub(crate) mouse_up_listeners:          Vec<MouseUpListener>,
pub(crate) mouse_pressure_listeners:    Vec<MousePressureListener>,
pub(crate) mouse_move_listeners:        Vec<MouseMoveListener>,
pub(crate) scroll_wheel_listeners:      Vec<ScrollWheelListener>,
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) pinch_listeners:             Vec<PinchListener>,
pub(crate) click_listeners:             Vec<ClickListener>,
pub(crate) aux_click_listeners:         Vec<ClickListener>,
pub(crate) drag_listener:               Option<(Arc<dyn Any>, DragListener)>,
pub(crate) hover_listener:              Option<Box<dyn Fn(&bool, &mut Window, &mut App)>>,
pub(crate) hitbox_behavior:             HitboxBehavior,
```

`click_listeners` and `aux_click_listeners` are synthesized inside
`InteractiveElement` from `mouse_down` → `mouse_up` on the same hitbox.
`drag_listener` plus
[`AnyDrag`](../../../crates/flui-core/src/app.rs#L2557-L2572) (set
imperatively via `cx.active_drag = Some(AnyDrag { … })` from a raw
`on_mouse_down`) form the imperative drag-and-drop API. `hover_listener`
fires on `is_hovered` boundary transitions.

S07 **adds**, but does not replace, this registry. Each listener vector
keeps its existing semantics; the new `gesture_recognizers` vector
fires in parallel after T6 hit-test conversion and T20 sanitization.

### Logger baseline

`flui-core` uses
[`log = "0.4.16"` with `kv_unstable_serde`](../../../crates/flui-core/Cargo.toml#L77).
`tracing` is **not** in the workspace. Existing platform code uses
`log::warn!` (e.g.
[`executor.rs:317`](../../../crates/flui-core/src/executor.rs#L317),
the wgpu renderer module). S07 follows this convention and standardizes
on the kv field schema documented in
[Architectural decisions log § Logging](#log-vs-tracing).

### Bench infrastructure

There is no `criterion` in the workspace. The existing
[`crates/flui-core/examples/bench/{data_table,paths_bench,pattern,shadow}.rs`](../../../crates/flui-core/examples/bench/)
pattern is the project convention for perf fixtures (declared as
`[[example]]` blocks in
[`Cargo.toml:320-334`](../../../crates/flui-core/Cargo.toml#L320-L334)).
T22 follows this pattern. When the **T4** roadmap row lands and adopts
`criterion` workspace-wide, the T22 bench thresholds become reference
baselines.

### Async timer pattern

`cx.spawn(async { smol::Timer::after(d).await })` is the documented
pattern for async timers throughout `flui-core` (used by the
animation controller and several executor sites). T11 (LongPress
recognizer) uses this. The recognizer's `Drop` impl cancels orphan
timers via the existing
`Task` cancellation semantics (drop the `Task` future to cancel).

### Object-safe trait templates

[`Box<dyn Action>`](../../../crates/flui-core/src/action.rs#L117-L134)
(`Action: Any + Send`) and
[`Box<dyn Simulation>`](../../../crates/flui-core/src/animation/simulation.rs#L22-L27)
(`Simulation: Send + Sync`) are working precedents for boxed-trait
object safety in `flui-core`. `GestureRecognizer` follows the same
shape but **drops `Sync`**, since recognizer state self-mutates from
inside the arena callback chain on the main thread only.

### Public re-export discipline

[`crates/flui-core/src/lib.rs`](../../../crates/flui-core/src/lib.rs#L91-L233)
contains ~140 explicit per-symbol re-exports plus ~29 module-level
glob re-exports. Per **S01a.3**, the `pub use platform::{…}` block is
explicit and must not be globbed; the other globs (`pub use action::*`,
`pub use animation::*`, …) are tracked by **A2** for a future cleanup.
T3 + T19 add a new explicit per-symbol block:

```rust
// In lib.rs, near the existing re-export blocks.

// Gesture / pointer / hit-test (S07) — core types and arena facade.
// `GestureArena`, `GestureArenaEntry`, `GestureArenaManager`, and
// `PointerSanitizer` are deliberately omitted — they are pub(crate)
// and have no public consumer surface today. Add them here only when
// a real consumer use case exists.
pub use gesture::{
    GestureArenaTeam, GestureBinding, GestureDisposition,
    GestureRecognizer, GestureSettings, HitTestBehavior, HitTestEntry,
    HitTestResult, PointerButtons, PointerEvent, PointerId, PointerKind,
    PointerPhase, PointerSignalEvent, PositionSample, SemanticAction,
    Velocity, VelocityTracker,
};

// Gesture concrete recognizers (S07). Kept in a separate block to
// match the existing flat-path re-export convention (`platform::*`
// uses one symbol per import line per group; nested sub-paths in a
// single `use` are avoided per S01a.3 hygiene).
pub use gesture::recognizers::{
    DoubleTapDetails, DoubleTapGestureRecognizer,
    DragEndDetails, DragStartDetails, DragUpdateDetails,
    HorizontalDragGestureRecognizer, LongPressDetails,
    LongPressGestureRecognizer, PanGestureRecognizer, ScaleEndDetails,
    ScaleGestureRecognizer, ScaleStartDetails, ScaleUpdateDetails,
    TapDetails, TapDownDetails, TapGestureRecognizer, TapUpDetails,
    VerticalDragGestureRecognizer,
};
```

Plus `pub mod gesture;` near the existing public-mod declarations
(e.g. next to `pub mod animation;`). Every name is enumerated; no
`pub use gesture::*;` glob. The fully-qualified path
`flui_core::gesture::TapGestureRecognizer` is also reachable via the
`pub mod gesture;` declaration; the **flat** path
(`flui_core::TapGestureRecognizer`) is canonical for downstream code.
The crate-level rustdoc on `gesture/mod.rs` notes this convention.

### MSRV / dependencies / lints

- **Edition:** 2024, **MSRV:** 1.85
  ([`Cargo.toml:21`](../../../Cargo.toml#L21)).
- **Existing relevant deps:** `smallvec = "1.6"`, `proptest = "1"`,
  `log = "0.4.16"` (kv-enabled), `parking_lot = "0.12.1"`,
  `circular-buffer = "1.0"`, `smol = "2.0"`. No new workspace deps
  added by S07.
- **Workspace lints:**
  [`Cargo.toml:55-67`](../../../Cargo.toml#L55-L67) sets
  `clippy::dbg_macro = "deny"`, `redundant_clone = "deny"`,
  `declare_interior_mutable_const = "deny"`, `disallowed_methods =
  "deny"`. `clippy.toml` enforces `smol::process::Command::*` over
  `std::process::Command::*`. S07 introduces no `dbg!`, no manual
  `Clone::clone()` round-trips, and no static `RefCell` constants.
- **`#![warn(missing_docs)]`** is on at
  [`lib.rs:2`](../../../crates/flui-core/src/lib.rs#L2). Every new
  public item carries rustdoc.

### Lock-checks baseline

The
[`tooling/lock-checks`](../../../tooling/lock-checks) crate scans for
`unimplemented!()` / `todo!()` / `unreachable!()` and for glob imports
in `crates/flui-core/src/platform/**`. S07 introduces no new stubs
(T3's empty modules contain no `unimplemented!()`) and no glob imports.
The `check-stubs` and `check-platform-imports` baselines remain
unchanged.

## Design

### Module layout

```
crates/flui-core/src/gesture/
├── mod.rs                  # public re-exports + crate-internal pub(crate) wiring
├── pointer_event.rs        # PointerEvent, PointerKind, PointerPhase, PointerId, PointerButtons
├── pointer_signal.rs       # PointerSignalEvent (Scroll | Magnify, #[non_exhaustive])
├── hit_test.rs             # HitTestEntry, HitTestResult, HitTestBehavior
├── gesture_settings.rs     # GestureSettings (#[non_exhaustive], Flutter defaults)
├── binding.rs              # GestureBinding (per-Window owner)
├── dispatch.rs             # PlatformInput → PointerEvent conversion + PointerSanitizer
├── arena.rs                # GestureArenaManager, GestureArena, GestureArenaEntry, GestureDisposition
├── arena_team.rs           # GestureArenaTeam (captain-deferred)
├── recognizer.rs           # GestureRecognizer trait + SemanticAction enum
├── velocity_tracker.rs     # VelocityTracker, Velocity, PositionSample
└── recognizers/
    ├── mod.rs              # re-exports of the five recognizers + Details types
    ├── tap.rs              # TapGestureRecognizer + TapDetails
    ├── double_tap.rs       # DoubleTapGestureRecognizer + DoubleTapDetails
    ├── long_press.rs       # LongPressGestureRecognizer + LongPressDetails
    ├── drag.rs             # PanGestureRecognizer, HorizontalDragGestureRecognizer, VerticalDragGestureRecognizer
    └── scale.rs            # ScaleGestureRecognizer + ScaleStart/Update/EndDetails
```

`pub mod gesture;` lands next to `pub mod animation;` in
[`crates/flui-core/src/lib.rs`](../../../crates/flui-core/src/lib.rs#L13).

### `PointerEvent` <a id="pointer-event"></a>

```rust
// crates/flui-core/src/gesture/pointer_event.rs

use std::time::Instant;
use crate::{Modifiers, Pixels, Point};

/// A unique, monotonically-increasing identifier for a single pointer
/// from the time it enters the window until the time it leaves.
///
/// On mouse-only platforms, the same `PointerId` is reused across
/// down/up sequences (one mouse cursor = one pointer); on multi-touch
/// platforms a new `PointerId` is allocated for each touch contact.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PointerId(pub(crate) u64);

/// The kind of input device that produced a [`PointerEvent`].
///
/// `#[non_exhaustive]` so future device kinds (e.g. eye-tracking) are
/// non-breaking additions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[non_exhaustive]
pub enum PointerKind {
    /// A standard mouse cursor.
    #[default]
    Mouse,
    /// A multi-touch contact (finger). Only emitted on platforms that
    /// surface real touch events (currently macOS trackpad + Wayland;
    /// Windows desktop touch is deferred — see Explicit gaps).
    Touch,
    /// A stylus / pen contact. The platform layer does not currently
    /// emit this variant — it is reserved for forward-compatibility
    /// with S20 desktop-gaps cleanup. The `tilt` and `orientation`
    /// fields on `PointerEvent` are zero for non-stylus pointers.
    Stylus,
}

/// The lifecycle phase of a [`PointerEvent`].
///
/// `#[non_exhaustive]` so future phases (e.g. `PanZoomStart` for trackpad
/// gestures) are non-breaking additions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum PointerPhase {
    /// The pointer device became known to the application (cursor entered
    /// the window, finger touched the screen, …). Carries no contact
    /// information yet.
    Added,
    /// The pointer is now in contact (button pressed, finger down).
    Down,
    /// The pointer moved while in contact.
    Move,
    /// The pointer left contact (button released, finger up).
    Up,
    /// The platform / sanitizer cancelled this gesture sequence (e.g.
    /// focus loss, modal switch, orphan-Down sanitization).
    Cancel,
    /// The pointer device left the application.
    Removed,
    /// Hover-only motion; no contact, no buttons. Mouse-class only.
    Hover,
    /// The pointer entered a new hit-test target during hover. Synthesized
    /// from `Hover` by `dispatch.rs` (frame-to-frame diff).
    Enter,
    /// The pointer left a hit-test target during hover. Synthesized from
    /// `Hover` by `dispatch.rs`.
    Exit,
}

/// A bitfield of currently-pressed buttons.
///
/// Modeled after Flutter's `kPrimaryButton` / `kSecondaryButton` /
/// `kTertiaryButton` constants; values match Flutter's Dart layer.
///
/// The inner `u32` is `pub(crate)` to prevent downstream code from
/// constructing arbitrary bit patterns that bypass the documented
/// constant set. Use the associated constants (`PRIMARY`,
/// `SECONDARY`, `TERTIARY`) plus `bits()` / `contains()` /
/// `is_empty()` for inspection.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PointerButtons(pub(crate) u32);

impl PointerButtons {
    /// The primary button (left mouse, single-finger tap, stylus tip).
    pub const PRIMARY:   Self = Self(0x01);
    /// The secondary button (right mouse, two-finger touch, stylus barrel).
    pub const SECONDARY: Self = Self(0x02);
    /// The tertiary button (middle mouse, three-finger touch).
    pub const TERTIARY:  Self = Self(0x04);

    /// Raw bit-pattern. Use this only for `serde` round-tripping or
    /// FFI; for normal logic prefer `contains()`.
    pub fn bits(self) -> u32                   { self.0 }
    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) != 0 }
    pub fn is_empty(self) -> bool              { self.0 == 0 }
}

/// A normalized pointer event, produced from [`PlatformInput`] by
/// [`crate::Window::dispatch_event`] and consumed by recognizers.
///
/// Construction goes through `From<PlatformInput>` impls in
/// `gesture/dispatch.rs`; users do not construct `PointerEvent`
/// directly. The struct is `#[non_exhaustive]` so adding fields
/// (`azimuth` for stylus, `device_id` for multi-monitor pointers in
/// future S20 work) is non-breaking.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PointerEvent {
    /// Per-pointer unique identifier (stable across `Down`→`Up`).
    pub pointer_id:  PointerId,
    /// The device kind that produced this event.
    pub kind:        PointerKind,
    /// The lifecycle phase.
    pub phase:       PointerPhase,
    /// Position in window-local logical pixels.
    pub position:    Point<Pixels>,
    /// Movement delta since the previous event for the same pointer.
    pub delta:       Point<Pixels>,
    /// Currently-pressed buttons. `is_empty()` for hover/exit phases.
    pub buttons:     PointerButtons,
    /// Currently-held keyboard modifiers (snapshot at event time).
    pub modifiers:   Modifiers,
    /// Wall-clock timestamp at the time the platform layer produced
    /// the underlying [`PlatformInput`].
    pub timestamp:   Instant,
    /// Normalized 0.0..=1.0 contact pressure. Mouse-class events have
    /// `pressure = 0.0` for `Up`/`Hover`/`Removed` and `1.0` for
    /// `Down`/`Move`. Real pressure values arrive only via
    /// `MousePressureEvent` (macOS-trackpad-only today).
    pub pressure:    f32,
    /// Stylus tilt (radians). Zero for non-stylus pointers (always
    /// today; reserved for forward-compat).
    pub tilt:        f32,
    /// Stylus rotation (radians). Zero for non-stylus pointers.
    pub orientation: f32,
}
```

The `From<PlatformInput>` impls live in `gesture/dispatch.rs`, not on
the type itself, because conversion requires the `Window`'s
per-pointer state (`PointerId` allocation, prior `position` for
`delta`, `pressure` synthesis) and produces `Option<PointerEvent>`
(some `PlatformInput` variants — keyboard, file-drop in non-mouse
phases — produce `None`). The conversion signature is:

```rust
// gesture/dispatch.rs

/// Per-`Window` pointer-state cache consumed by `PointerSanitizer::convert`.
/// `pub(crate)` — internal implementation detail of the gesture
/// dispatch path, not part of the public API.
pub(crate) struct WindowPointerState {
    pub(crate) last_position:  Point<Pixels>,
    pub(crate) modifiers:      Modifiers,
    pub(crate) next_pointer_id: u64,
    pub(crate) prior_hit_test: Option<HitTestResult>,
}

impl PointerSanitizer {
    pub(crate) fn convert(
        &mut self,
        input: &PlatformInput,
        window_state: &mut WindowPointerState,
    ) -> SmallVec<[PointerEvent; 2]> { … }
}
```

Returns `SmallVec` because hover→down transitions and orphan-cancel
synthesis can produce two events from a single platform input (e.g. a
synthetic `Cancel` followed by the real `Down`). Both
`PointerSanitizer` and `WindowPointerState` are `pub(crate)` — they
have no public consumer use case and exposing them would commit
`flui-core` to their layout via the type-name re-export alone.

### `PointerSignalEvent` <a id="pointer-signal-event"></a>

Scroll-wheel and pinch-magnify events bypass the gesture arena. They
are non-competitive — there is no "winner" of a scroll wheel tick — and
they fire directly on the deepest hit-test target with `Translucent`
propagation per
[`HitTestBehavior`](#hit-test-behavior). This matches Flutter's
`PointerSignalEvent` separation.

```rust
// crates/flui-core/src/gesture/pointer_signal.rs

use std::time::Instant;
use crate::{Modifiers, Pixels, Point};
use super::{PointerId, PointerKind};

/// A non-competitive signal from a pointer device (scroll, magnify).
/// Bypasses the gesture arena entirely.
///
/// `#[non_exhaustive]` to admit future signals (e.g. `Smart-zoom`,
/// `Force-press`) without breaking changes.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum PointerSignalEvent {
    /// A scroll-wheel / two-finger-pan tick.
    Scroll {
        pointer_id: PointerId,
        kind:       PointerKind,
        position:   Point<Pixels>,
        delta:      Point<Pixels>,
        modifiers:  Modifiers,
        timestamp:  Instant,
    },
    /// A pinch-magnify tick. `scale_delta` is multiplicative (1.0 == no
    /// change). Rotation is **always 0.0** on current desktop platforms;
    /// the field exists for forward-compat with multi-pointer touch.
    Magnify {
        pointer_id:   PointerId,
        kind:         PointerKind,
        position:     Point<Pixels>,
        scale_delta:  f32,
        rotation_rad: f32,
        modifiers:    Modifiers,
        timestamp:    Instant,
    },
}
```

`From<&ScrollWheelEvent>` and `From<&PinchEvent>` (the latter
`#[cfg]`-gated to `linux`+`macos`) live in `gesture/dispatch.rs`.

### `HitTestEntry` and `HitTestResult` <a id="hit-test-result-and-entry"></a>

```rust
// crates/flui-core/src/gesture/hit_test.rs

use smallvec::SmallVec;
use crate::{HitboxId, Pixels, Point};

/// One target identified during a hit-test pass, ordered front-to-back
/// (deepest paint wins index 0).
///
/// `#[non_exhaustive]` so future fields (e.g. `paint_layer`,
/// `clip_rect`) are non-breaking additions.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HitTestEntry {
    /// The `HitboxId` of the committed hitbox that matched.
    pub hitbox_id:        HitboxId,
    /// The hit-test position in window-local pixels (same as the source
    /// `PointerEvent.position`; carried for recognizers that need it).
    pub position:         Point<Pixels>,
    /// The behavior of this entry — controls whether propagation
    /// continues past it.
    pub behavior:         HitTestBehavior,
}

/// The ordered set of entries produced by [`crate::Window::hit_test`].
/// Front-to-back; index 0 is the deepest hitbox under the pointer.
#[derive(Clone, Debug, Default)]
pub struct HitTestResult {
    pub(crate) entries: SmallVec<[HitTestEntry; 8]>,
}

impl HitTestResult {
    /// Iterate front-to-back (deepest first).
    pub fn iter(&self) -> impl Iterator<Item = &HitTestEntry> { self.entries.iter() }
    /// Number of hit-test entries.
    pub fn len(&self) -> usize { self.entries.len() }
    /// `true` iff no hitbox was hit.
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

/// How a hit-test entry interacts with propagation.
///
/// `#[non_exhaustive]` so future behaviors (e.g. `OpaqueExceptScroll`)
/// are non-breaking additions.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum HitTestBehavior {
    /// Receives events; stops propagation. Default for
    /// `InteractiveElement` consistent with Flutter's default.
    #[default]
    Opaque,
    /// Receives events and forwards them to the next entry behind it.
    Translucent,
    /// Does not receive events itself; defers to its children. If no
    /// child matches, falls through to the next entry behind it.
    DeferToChild,
}
```

The new `Window::hit_test(position) -> HitTestResult` walks the
existing committed `mouse_hit_test` ids in front-to-back order,
filtering through `HitboxBehavior::BlockMouse` /
`BlockMouseExceptScroll` for backward compatibility, and pairs each
remaining `HitboxId` with the `HitTestBehavior` recorded by the
matching `Interactivity` (or `Opaque` if no `Interactivity` claimed
that hitbox — the painted-only case). The exact lookup map (per-frame
`HashMap<HitboxId, HitTestBehavior>` populated during paint) is a
private implementation detail; `T6` documents it inline in `window.rs`.

`HitTestBehavior` is **orthogonal** to `HitboxBehavior` (which lives in
[`crates/flui-core/src/window.rs:648`](../../../crates/flui-core/src/window.rs#L648-L661)
and controls hover-style `is_hovered` semantics). They serve different
purposes:

| Concept           | Owner              | Affects                               |
|-------------------|--------------------|---------------------------------------|
| `HitboxBehavior`  | paint-time hitbox  | `is_hovered`, `should_handle_scroll` (style decisions) |
| `HitTestBehavior` | gesture entry      | Arena participation + listener propagation |

A single `Interactivity` may carry both: e.g. an overlay sets
`HitboxBehavior::BlockMouseExceptScroll` (so style `is_hovered` returns
`false` for elements behind it) **and** `HitTestBehavior::Translucent`
(so gesture recognizers behind it still join the arena for the same
pointer). T14 wires `with_hit_test_behavior` independently of the
existing `occlude` / `block_mouse_except_scroll` builders.

### `GestureSettings` <a id="gesture-settings"></a>

```rust
// crates/flui-core/src/gesture/gesture_settings.rs

use std::time::Duration;
use crate::Pixels;

/// Per-window tunable thresholds for gesture recognition.
///
/// `#[non_exhaustive]` so future thresholds are non-breaking. Use the
/// `Default` impl (Flutter-parity defaults) and overwrite individual
/// fields:
///
/// ```ignore
/// let mut settings = flui_core::GestureSettings::default();
/// settings.long_press_timeout = std::time::Duration::from_millis(800);
/// window.gesture_binding_mut().set_settings(settings);
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct GestureSettings {
    /// Maximum movement before a tap is rejected. Flutter default: 18px.
    pub touch_slop:               Pixels,
    /// Slop along the locked axis for axis-locked drags. Flutter
    /// default: 18px.
    pub pan_slop:                 Pixels,
    /// Maximum interval between two taps to count as a double-tap.
    /// Flutter default: 300ms.
    pub double_tap_timeout:       Duration,
    /// Minimum interval between two taps (avoids quad-emit on jittery
    /// hardware). Flutter default: 40ms.
    pub double_tap_min_time:      Duration,
    /// Hold duration before a long-press fires. Flutter default: 500ms.
    pub long_press_timeout:       Duration,
    /// Maximum movement before a long-press is rejected. Flutter
    /// default: 18px.
    pub long_press_slop:          Pixels,
    /// VelocityTracker max sample window age. Flutter default: 100ms.
    pub velocity_tracker_window:  Duration,
    /// VelocityTracker maximum sample buffer size. Flutter default: 20.
    pub velocity_tracker_samples: usize,
    /// Maximum spawn-to-flush latency budget for the LongPress async
    /// timer (the recognizer panics-or-warns if exceeded). Default:
    /// 16ms (one 60Hz frame).
    pub long_press_timer_budget:  Duration,
}

impl Default for GestureSettings { … }   // Flutter parity values listed above.
```

### `GestureBinding` <a id="gesture-binding"></a>

A `GestureBinding` is the per-`Window` owner of the arena, settings,
and sanitizer. `Window` carries it as a private field; user code
reaches it via `window.gesture_binding()` /
`window.gesture_binding_mut()` (where `window: &mut Window` is the
parameter passed to listener closures and `Render::render`
implementations — there is no `cx.window()` accessor on `App`).

```rust
// crates/flui-core/src/gesture/binding.rs

use crate::App;
use super::{GestureArenaManager, GestureSettings, PointerSanitizer};

/// Per-window owner of the gesture arena, the configurable
/// `GestureSettings`, and the `PointerSanitizer`.
///
/// One instance lives inside every `Window`; access it via
/// `window.gesture_binding()` and `window.gesture_binding_mut()`.
///
/// Auto-trait posture: `!Send + !Sync` (transitively via the arena's
/// `Rc<RefCell<dyn GestureRecognizer>>`). Per-`Window` types are
/// main-thread-only by construction; do **not** wrap a
/// `GestureBinding` in `Arc` — the borrow-check failure points at
/// the `Rc` directly.
pub struct GestureBinding {
    arena:     GestureArenaManager,
    settings:  GestureSettings,
    sanitizer: PointerSanitizer,
}

impl GestureBinding {
    pub(crate) fn new() -> Self { … }

    /// Borrow the configured gesture settings. Cheap.
    pub fn settings(&self) -> &GestureSettings           { &self.settings }
    /// Mutate settings. Wired to `window.gesture_settings_mut()`
    /// (the S14 `MediaQuery::gesture_settings` seam).
    pub fn settings_mut(&mut self) -> &mut GestureSettings { &mut self.settings }

    /// Number of pointers currently competing in any open arena.
    /// Read-only observer for tests and debug rendering.
    pub fn active_pointer_count(&self) -> usize { self.arena.arena_count() }

    /// Number of recognizers competing for `pointer_id`'s arena, or
    /// 0 if no arena is open for that pointer.
    pub fn arena_entry_count(&self, pointer_id: PointerId) -> usize {
        self.arena.entry_count(pointer_id)
    }

    // The full `GestureArenaManager` is intentionally pub(crate)-only.
    // External callers cannot mutate arena state directly; the dispatch
    // flow inside `Window::dispatch_event` is the single source of
    // truth for arena transitions.
    pub(crate) fn arena_mut(&mut self) -> &mut GestureArenaManager { &mut self.arena }
    pub(crate) fn arena(&self) -> &GestureArenaManager             { &self.arena }
    pub(crate) fn sanitizer_mut(&mut self) -> &mut PointerSanitizer { &mut self.sanitizer }
}
```

`Window::gesture_binding(&self) -> &GestureBinding` and
`Window::gesture_binding_mut(&mut self)` are added in T21. The S14
seam is `Window::gesture_settings_mut()` (a thin shortcut returning
`&mut GestureSettings` directly so MediaQuery can write into it
without exposing `GestureBinding` internals).

### `PointerSanitizer` <a id="pointer-sanitizer"></a>

The sanitizer runs between
[`Window::dispatch_event`](../../../crates/flui-core/src/window.rs#L4074)
event coalescing and the existing `dispatch_mouse_event` chain. Its
contract:

1. **Synthesize `Cancel` for orphan `Down`.** If a `Down` arrives for a
   `PointerId` that is already known to be down (no intervening `Up`),
   the sanitizer emits a synthetic `PointerEvent { phase: Cancel, … }`
   for the prior `Down` first, then forwards the new `Down`. This
   matches Flutter's `_PointerEventConverter` behavior on focus-loss
   boundaries.

2. **Reject duplicate `Down`.** If a `Down` arrives for a `PointerId`
   that is already in the down state with the same `position` and
   `buttons` (within slop), the duplicate is dropped silently.

3. **Clamp out-of-bounds positions.** Positions outside the window
   bounds (which can arrive on Wayland during decoration drag) are
   clamped to the window rect.

4. **Hover diff.** On `Hover`, the sanitizer compares the current
   hit-test result with the previous one and synthesizes `Exit` events
   for entries that were in the prior result but are no longer hit,
   and `Enter` events for newly-hit entries. The bare `Hover` is
   forwarded last for any element that wants raw motion.

The sanitizer is per-`Window`; per-pointer state lives in a
`SmallVec<[(PointerId, PointerState); 4]>` private to the sanitizer
(linear scan; pointer counts are bounded by hardware contact limits —
typically ≤ 2 on desktop, ≤ 4 on multi-touch).

Logging: every sanitized output emits a `log::trace!` with kv fields
`pointer_id`, `phase`, `synthesized` (bool), `reason` (string slice).
`Cancel` synthesis additionally logs at `log::warn!` level so the
condition shows up in default log filtering.

### `GestureRecognizer` trait

```rust
// crates/flui-core/src/gesture/recognizer.rs

use crate::FocusHandle;
use super::{GestureDisposition, PointerEvent, PointerId};

/// One competitor in the gesture arena.
///
/// **Object-safety:** verified by `dyn GestureRecognizer` use in
/// `GestureArenaEntry`, by a doc-test in this module, and by
/// `rust-api-migration-auditor` review (see T2). The trait is
/// `?Sync` (main-thread-only) — recognizers self-mutate from inside
/// arena callbacks.
///
/// **Drop guarantee:** dropping a recognizer must cancel any in-flight
/// asynchronous work (e.g. LongPress timers). Implementations MUST
/// store `Task` handles such that dropping the recognizer drops the
/// `Task` and cancels its future. The arena verifies this at run-time
/// by tracking outstanding callbacks; a callback after recognizer
/// `Drop` is logged at `warn` and discarded.
pub trait GestureRecognizer {
    /// A short human-readable name (e.g. `"tap"`, `"long_press"`).
    /// Used in `log::*` `kv` fields.
    fn name(&self) -> &'static str;

    /// The recognizer is being added to the arena for `pointer_id`.
    /// Recognizers track per-pointer state internally; the arena
    /// manager is the system of record for which recognizers care
    /// about which pointers.
    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent);

    /// A new event arrived for a tracked pointer. Recognizers may
    /// **eagerly accept** by returning
    /// `GestureDisposition::Accepted` or **eagerly reject** with
    /// `GestureDisposition::Rejected`. Returning
    /// `GestureDisposition::Possible` keeps the recognizer in the
    /// arena.
    ///
    /// **Trait contract:** implementations MUST NOT call
    /// `cx.stop_propagation()` from inside `handle_event`. The arena
    /// declares its winner via `GestureDisposition::Accepted`, and the
    /// existing raw-listener chain (`on_mouse_*` / `on_click` /
    /// `AnyDrag`) is preserved by the dispatcher resetting
    /// `cx.propagate_event = true` after the arena pass. Calling
    /// `stop_propagation` from a recognizer would silently break the
    /// existing `cx.active_drag` flow on `Down` events.
    fn handle_event(
        &mut self,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition;

    /// Sweep — fire delegated callbacks if this recognizer "won" the
    /// arena via sweep semantics (last man standing on `Up`). Called
    /// by the arena manager exactly once per pointer when the arena
    /// resolves; recognizers that already returned `Accepted` from
    /// `handle_event` will not see a `sweep_accepted` call.
    fn sweep_accepted(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    );

    /// The arena resolved against this recognizer. Recognizers MUST
    /// reset any in-flight visual state (LongPress feedback, cursor
    /// styling) without firing user callbacks.
    fn rejected(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    );

    /// **S08 seam.** The set of semantic actions this recognizer
    /// surfaces to the accessibility tree. Default empty; S08 will
    /// populate Tap/DoubleTap/LongPress recognizers' overrides.
    fn semantic_actions(&self) -> &'static [SemanticAction] { &[] }

    /// **S12 seam.** The focus handle this recognizer wishes to
    /// claim on accept (e.g. a button claims focus on tap-down).
    /// Default `None`; `TapGestureRecognizer` overrides when
    /// `request_focus_on_tap_down` is set.
    fn on_focus_request(&self) -> Option<FocusHandle> { None }
}

/// Semantic-action enum (S08 seam — default-empty here, populated in S08).
///
/// `#[non_exhaustive]` so S08 may add `Increment`, `Decrement`,
/// `Move`, etc. without a breaking change.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SemanticAction {
    Tap,
    DoubleTap,
    LongPress,
}
```

The `?Sync` bound is encoded by **omitting** `: Sync` from the trait
bound. We do NOT add `: Send` either — recognizers are owned by their
parent `Interactivity`, which is itself `!Send` because it holds
`Box<dyn FnMut(...)>` callbacks that capture the per-`Window`
`AppContext`. Same auto-trait posture as `Element` in `flui-core`
today.

### `GestureArena` and `GestureArenaManager`

```rust
// crates/flui-core/src/gesture/arena.rs

use std::cell::RefCell;
use std::rc::Rc;
use smallvec::SmallVec;
use super::{GestureRecognizer, PointerEvent, PointerId};

/// One competitor entry in a `GestureArena`. Holds an `Rc<RefCell<…>>`
/// to the recognizer because recognizers self-mutate from inside the
/// arena callback chain (eager-accept may run user code that mutates
/// the recognizer state).
///
/// **A7 audit comment:** the `Rc<RefCell<dyn GestureRecognizer>>` is
/// bounded to the gesture subsystem internals. Public surface
/// (`Interactivity::on_tap` and friends) takes
/// `Box<dyn GestureRecognizer>` and the arena promotes to
/// `Rc<RefCell<...>>` internally. The auto-trait set on the public
/// API is therefore **not** affected by this internal interior
/// mutability.
pub struct GestureArenaEntry {
    pub(crate) recognizer: Rc<RefCell<dyn GestureRecognizer>>,
}

/// One arena per active pointer. `entries` is registration order;
/// the captain is `entries[0]` (sweep on Up declares the first
/// registered the winner if no recognizer eagerly accepted).
#[derive(Default)]
pub struct GestureArena {
    pub(crate) entries:        SmallVec<[GestureArenaEntry; 4]>,
    pub(crate) winner:         Option<usize>,   // index into entries on accept
    pub(crate) is_open:        bool,
    pub(crate) is_held:        bool,
}

/// The disposition returned by `GestureRecognizer::handle_event` and
/// recorded by the arena manager.
///
/// `#[non_exhaustive]` to admit future dispositions (e.g. `Hold` for
/// gesture-yield semantics) without breaking changes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GestureDisposition {
    /// "I want this gesture sequence; declare me the winner now."
    /// All other recognizers in the arena are notified `rejected`.
    Accepted,
    /// "I cannot win this gesture sequence; remove me from the arena."
    /// Other recognizers continue competing.
    Rejected,
    /// "I might still win — keep me in the arena."
    Possible,
}

/// One per `GestureBinding` per `Window`.
#[derive(Default)]
pub struct GestureArenaManager {
    pub(crate) arenas: SmallVec<[(PointerId, GestureArena); 4]>,
}

impl GestureArenaManager {
    /// Open an arena for `pointer_id` if none exists; insert
    /// `recognizer` at the back of the entries list (registration
    /// order).
    pub(crate) fn add(
        &mut self,
        pointer_id: PointerId,
        recognizer: Rc<RefCell<dyn GestureRecognizer>>,
    ) { … }

    /// Dispatch an event to all entries in `pointer_id`'s arena. If
    /// any returns `Accepted`, declare it winner and notify the rest
    /// `rejected`. If any returns `Rejected`, drop it.
    pub(crate) fn dispatch(
        &mut self,
        pointer_id: PointerId,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) { … }

    /// Sweep — called by `dispatch.rs` on `Up`. If no winner has been
    /// declared, declare the first remaining entry the winner via
    /// `sweep_accepted`. Then close the arena.
    pub(crate) fn sweep(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) { … }

    /// Hold semantics — keep the arena open past `Up` until the
    /// caller (e.g. DoubleTap waiting for a second tap) calls
    /// `release`. Used by recognizers that span multiple Down/Up
    /// sequences.
    pub(crate) fn hold(&mut self, pointer_id: PointerId)    { … }
    pub(crate) fn release(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) { … }

    /// Forcefully close the arena and notify every remaining entry
    /// `rejected`. Called by the sanitizer on `Cancel`.
    pub(crate) fn cancel(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) { … }

    /// Async back-channel for recognizers whose `Accepted` decision
    /// fires from outside `handle_event` (e.g. `LongPressGestureRecognizer`'s
    /// timer). Stored as a `Weak<RefCell<GestureArenaManager>>` on
    /// the recognizer; dropped weakly so a Window-close cancels the
    /// pending acceptance harmlessly.
    ///
    /// `recognizer_index` is the index into the arena's `entries`
    /// vector that the recognizer recorded inside `add_pointer`.
    pub(crate) fn declare_winner(
        &mut self,
        pointer_id: PointerId,
        recognizer_index: usize,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) { … }
}
```

**Resolution semantics.**

```text
                            arena.dispatch(event)
                           /         |          \
                          v          v           v
             Accepted by   no eager     Rejected by
             recognizer X   accept       recognizer Y
                  |          |               |
                  v          v               v
        winner = X    keep Y in arena    drop Y from arena
        notify all
        others Rejected

                       on Up phase:
                 winner already declared? ── yes ──▶ done
                              │ no
                              ▼
                       sweep: winner = entries[0]
                              │
                              ▼
                       sweep_accepted on entries[0]
                       rejected on rest
```

This is the canonical Flutter arena behavior (eager-accept wins;
sweep declares first-registered on `Up` if no one accepted).

### `GestureArenaTeam`

A team is a captain-led group of recognizers that **defer** disposition
to their captain. The captain is the only one that may declare
`Accepted`; team members may declare `Rejected` to leave the team but
not `Accepted`. Teams are useful when several recognizers want to
present a coordinated front (e.g. a row of buttons each with its own
`Tap` recognizer that should defer to a parent `Drag` if the user
panned across multiple buttons).

```rust
// crates/flui-core/src/gesture/arena_team.rs

use std::cell::RefCell;
use std::rc::Rc;
use smallvec::SmallVec;
use super::{GestureDisposition, GestureRecognizer, PointerId};

#[non_exhaustive]
pub struct GestureArenaTeam {
    pub(crate) captain: Rc<RefCell<dyn GestureRecognizer>>,
    pub(crate) members: SmallVec<[Rc<RefCell<dyn GestureRecognizer>>; 2]>,
}

impl GestureArenaTeam {
    /// Create a new team with `captain` as the captain recognizer.
    /// The captain is the only recognizer in the team that may
    /// declare `GestureDisposition::Accepted`; team members that
    /// return `Accepted` are coerced to `Possible` by the team.
    pub fn with_captain(captain: Box<dyn GestureRecognizer>) -> Self {
        Self {
            captain: Rc::new(RefCell::new(captain)) as Rc<RefCell<dyn GestureRecognizer>>,
            members: SmallVec::new(),
        }
    }

    /// Add a member recognizer to the team.
    pub fn add_member(&mut self, member: Box<dyn GestureRecognizer>) {
        self.members.push(
            Rc::new(RefCell::new(member)) as Rc<RefCell<dyn GestureRecognizer>>,
        );
    }

    /// Resolve a member's reported disposition. Members that report
    /// `Accepted` are converted to `Possible` (deferred to captain);
    /// captain's `Accepted` resolves the entire team.
    pub(crate) fn resolve_member(
        &self,
        member: &Rc<RefCell<dyn GestureRecognizer>>,
        reported: GestureDisposition,
    ) -> GestureDisposition { … }
}
```

The team type is only used by callers that explicitly opt in (no
`Interactivity` builder for it in T14; teams are advanced and rare).
S07 ships the type and its semantics with a `Box`-accepting
constructor that hides the internal `Rc<RefCell<…>>` plumbing. S08
will use it for semantics-driven coordination across siblings.

### `VelocityTracker` <a id="velocity-tracker"></a>

Flutter's `VelocityTracker` uses a `LeastSquaresSolver` over a weighted
quadratic fit. We replicate the same shape:

```rust
// crates/flui-core/src/gesture/velocity_tracker.rs

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crate::{Pixels, Point};

/// One position sample with its timestamp.
///
/// `#[non_exhaustive]` so future fields (e.g. `pointer_id` for
/// multi-pointer LSQ fits) are non-breaking additions.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct PositionSample {
    pub position:  Point<Pixels>,
    pub timestamp: Instant,
}

/// The result of a `VelocityTracker::estimate()` call.
///
/// On insufficient samples, `Velocity::default()` is returned (zero
/// vector). `VelocityTracker::estimate()` guarantees non-NaN output;
/// `is_zero()` is safe to call on its result.
///
/// `#[non_exhaustive]` so future fields (e.g. `acceleration`) are
/// non-breaking additions.
#[derive(Copy, Clone, Debug, Default)]
#[non_exhaustive]
pub struct Velocity {
    pub pixels_per_second: Point<f32>,
}

impl Velocity {
    /// Returns `true` if both velocity components are exactly zero.
    /// `VelocityTracker::estimate()` returns `Velocity::default()`
    /// (zero) on insufficient samples, and asserts non-NaN on its
    /// output via `debug_assert!(!.. .is_nan())` — so this method
    /// always reflects a meaningful velocity check.
    pub fn is_zero(self) -> bool {
        self.pixels_per_second.x == 0.0 && self.pixels_per_second.y == 0.0
    }
}

/// Bounded least-squares velocity estimator. Drops samples older than
/// `GestureSettings::velocity_tracker_window`; caps the buffer at
/// `GestureSettings::velocity_tracker_samples`.
pub struct VelocityTracker {
    samples:     VecDeque<PositionSample>,
    max_samples: usize,
    max_age:     Duration,
}

impl VelocityTracker {
    pub fn new(settings: &super::GestureSettings) -> Self { … }
    pub fn add_position(&mut self, sample: PositionSample) { … }
    /// Weighted least-squares quadratic fit. Returns `Velocity::default()`
    /// if fewer than 3 in-window samples are available.
    pub fn estimate(&self) -> Velocity { … }
    pub fn reset(&mut self) { … }
}
```

The math itself is a direct port of Flutter's
`LeastSquaresSolver::solve` (see Common pitfalls below for the
weight-function gotcha). The solver is documented inline in
`velocity_tracker.rs` with a 30-line ASCII diagram of the weight
function.

### Concrete recognizers

For each recognizer, the surface is an `*Details` struct (passed to the
user callback) plus a constructor on the recognizer type. `Details`
fields are extracted from the relevant `PointerEvent` plus VelocityTracker
output (for end-state recognizers like Drag).

#### `TapGestureRecognizer` <a id="tap-recognizer"></a>

All `*Details` structs and the recognizer struct itself are
`#[non_exhaustive]` so future field additions (pressure, tilt for
S20 stylus support; per-button modifiers; etc.) are non-breaking.
The recognizer's callback fields stay `pub` so `Interactivity::on_tap`
fluent builders can install closures via field assignment.
Construction outside `flui-core` goes through `Default::default()`
plus per-field overrides — direct struct-literal construction is
forbidden by `#[non_exhaustive]`.

```rust
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapDownDetails {
    pub global_position: Point<Pixels>,
    pub local_position:  Point<Pixels>,
    pub kind:            PointerKind,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapUpDetails  { … }       // mirrors TapDownDetails

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TapDetails    { pub kind: PointerKind, pub global_position: Point<Pixels> }

#[non_exhaustive]
pub struct TapGestureRecognizer {
    pub on_tap_down: Option<Box<dyn FnMut(TapDownDetails, &mut crate::Window, &mut crate::App)>>,
    pub on_tap_up:   Option<Box<dyn FnMut(TapUpDetails,   &mut crate::Window, &mut crate::App)>>,
    pub on_tap:      Option<Box<dyn FnMut(TapDetails,     &mut crate::Window, &mut crate::App)>>,
    pub on_tap_cancel: Option<Box<dyn FnMut(&mut crate::Window, &mut crate::App)>>,
    pub button:                       PointerButtons,    // primary/secondary/tertiary
    pub request_focus_on_tap_down:    Option<crate::FocusHandle>,
    settings:                         GestureSettings,
    // Async back-channel to the arena (LongPress is the primary user;
    // Tap stores it for symmetry but does not invoke from a timer).
    arena_back_channel:               Weak<RefCell<GestureArenaManager>>,
    pointer_index:                    Option<usize>,  // index inside arena.entries
}
```

The same `#[non_exhaustive]` discipline is applied uniformly to all
remaining `*Details` and recognizer structs (`DoubleTapDetails`,
`LongPressDetails`, `DragStartDetails`, `DragUpdateDetails`,
`DragEndDetails`, `ScaleStartDetails`, `ScaleUpdateDetails`,
`ScaleEndDetails`, `DoubleTapGestureRecognizer`,
`LongPressGestureRecognizer`, `PanGestureRecognizer`,
`HorizontalDragGestureRecognizer`, `VerticalDragGestureRecognizer`,
`ScaleGestureRecognizer`).

`semantic_actions()` returns `&[SemanticAction::Tap]`.
`on_focus_request()` returns `request_focus_on_tap_down.clone()`.

State machine: enters `Possible` on `Down`; transitions to `Accepted`
on `Up` if the displacement from `Down` is within `touch_slop`;
rejects on `Move` if displacement exceeds slop, on `Cancel`, or on
`Up` of a different `pointer_id`.

#### `DoubleTapGestureRecognizer` <a id="double-tap-recognizer"></a>

State machine spans two Down/Up sequences:

```text
Idle ── Down ──▶ FirstDown ── Up (within slop, < min_time?) ──▶ FirstUp
                                          │ Up (> min_time, < timeout)
                                          ▼
                                       AwaitSecond  ── Down ──▶ SecondDown
                                          │ timeout or Move beyond slop
                                          ▼
                                        Reject
SecondDown ── Up (within slop) ──▶ Accepted
            ── Move > slop or Up too late ──▶ Rejected
```

Uses arena `hold` between FirstUp and SecondDown so the arena keeps the
sequence open across the gap.

#### `LongPressGestureRecognizer` <a id="long-press-recognizer"></a>

Uses `cx.spawn(async { smol::Timer::after(d).await })` to schedule the
timeout; stores the resulting `Task` so dropping the recognizer
cancels the timer.

```text
Down ──▶ schedule timer (long_press_timeout)
        │
        ├── Move ≤ slop  ──▶ continue waiting
        ├── Move > slop  ──▶ Rejected (drop timer)
        ├── Up           ──▶ Rejected (drop timer)
        └── timer fired  ──▶ Accepted, fire on_long_press_start;
                              continue tracking Move/Up for
                              on_long_press_move / on_long_press_end
```

Because the timer fires from inside `cx.spawn`, the arena's
`handle_event` does not see the acceptance directly. The recognizer
holds a `Weak<RefCell<GestureArenaManager>>` (its `arena_back_channel`
field, captured from the binding when `add_pointer` is called) and a
`pointer_index` (the index recorded inside `entries` when the arena
admitted the recognizer). From inside the timer future the recognizer
upgrades the `Weak` to `Rc` and calls
`arena.declare_winner(pointer_id, pointer_index, window, cx)`.

The `Weak` is **load-bearing**: if the `Window` (and therefore the
`GestureBinding`) is dropped before the timer fires, the upgrade
returns `None` and the recognizer no-ops. This avoids
callback-after-Window-drop crashes.

The operation is bounded by the `long_press_timer_budget`
(`GestureSettings`) — if the spawn-to-flush latency exceeds the
budget, the recognizer logs at `log::warn!` and self-cancels (drops
the upgrade attempt without calling `declare_winner`). T17 verifies
both paths (timer-fires-while-window-alive and
timer-fires-while-window-dropped).

Note: the timer future is launched via `window.spawn(...)` (which is
the per-`Window` future-spawning wrapper around `cx.spawn`); this
guarantees the future is cancelled on `Window::drop` and that the
`Weak` upgrade fails cleanly. The recognizer stores the returned
`Task` in `Option<Task<()>>` and drops it on recognizer-Drop to
cancel the timer eagerly.

#### `Drag` recognizers <a id="drag-recognizer"></a>

Three flavors with slightly different acceptance criteria:

- `PanGestureRecognizer` — accepts on any `Move > pan_slop` (no axis
  lock).
- `HorizontalDragGestureRecognizer` — accepts on `|delta.x| > pan_slop &&
  |delta.x| > 2 * |delta.y|`. Rejects on the inverse (vertical-leaning
  motion).
- `VerticalDragGestureRecognizer` — symmetrical.

All three feed a per-pointer `VelocityTracker` and emit `on_drag_start`
on accept, `on_drag_update` on each subsequent `Move`, and
`on_drag_end` with `velocity = velocity_tracker.estimate()` on `Up`.

The module rustdoc explicitly documents coexistence with
`cx.active_drag` (`AnyDrag`):

> Pan recognizers fire **independently** of the imperative
> `cx.active_drag = Some(AnyDrag { … })` flow used for drag-and-drop
> data transfer. Both can be active simultaneously: the Pan recognizer
> reports start/update/end via `on_pan_*` callbacks, while the
> `AnyDrag` flow drives the visual drag preview and drop-target
> notification. T15 wires them in parallel; tests in T16 / T17 verify
> the two coexist without firing-order regressions.

#### `ScaleGestureRecognizer` <a id="scale-recognizer"></a>

Tracks ≥2 active pointers; emits `on_scale_start` when the second
pointer's `Down` arrives (within slop window of the first), then
`on_scale_update` on each `Move` of either pointer with:

- `focal_point` — average of all active pointer positions
- `scale` — current pointer-pair distance / initial pointer-pair
  distance (1.0 == no scale change)
- `rotation_rad` — current angle of the pointer-pair vector minus
  initial angle (radians, **always 0.0 on current desktop platforms**)

`on_scale_end` fires when any tracked pointer goes `Up`.

The recognizer's rustdoc explicitly documents:

```text
- Windows desktop emits no native pinch (no PinchEvent variant for
  Windows). On Windows, multi-pointer Scale will only fire if the
  consumer manually injects a second touch contact via test scaffolding.
- Linux/macOS PinchEvent has `delta: f32` only — no rotation. Rotation
  is therefore always 0.0 from PinchEvent-sourced sequences. Multi-finger
  rotation requires the Wayland `pointer-gestures-unstable-v1` extension
  (deferred — see Explicit gaps).
```

### `Window::dispatch_event` integration

The post-S07 dispatch flow:

```text
Window::dispatch_event(PlatformInput) ──┐
                                        │ existing modality / mouse_position / cx.propagate_event reset
                                        │
                                        ▼
                              gesture::dispatch::translate(input)
                                     │
                                     ├──▶ Some(PointerSignalEvent) ──▶ Window::dispatch_signal(...)
                                     │                                       │
                                     │                                       ▼
                                     │                          existing scroll_wheel / pinch listener
                                     │                          chain (HitTestBehavior::Translucent
                                     │                          honored via the new HitTestResult)
                                     │
                                     └──▶ SmallVec<PointerEvent>
                                                │ for each event:
                                                ▼
                                  Window::hit_test(event.position) ─▶ HitTestResult
                                                │
                                                ▼
                              PointerSanitizer::process(event, prior) ──▶ SmallVec<PointerEvent>
                                                │ (synthesizes Cancel / Enter / Exit / Hover diff)
                                                ▼
                                  for each sanitized event:
                                    1. arena.dispatch(event, window, cx)   ── new (arena pass)
                                    2. cx.propagate_event = true;          ── reset boundary
                                    3. existing dispatch_mouse_event(...)  ── unchanged
                                       (raw on_mouse_*, on_click,
                                        AnyDrag flow, hover styles)
```

The two-pass design (arena dispatch, then existing listener chain)
preserves the firing order and count of every existing listener. The
arena dispatch is **isolated** from the existing listener chain by an
explicit `cx.propagate_event = true` reset between the two passes. This
guarantees that a recognizer cannot suppress raw `on_mouse_*` listeners
via `stop_propagation()`, preserving the `cx.active_drag` / `AnyDrag`
contract that depends on `on_mouse_down` always firing.

The `GestureRecognizer::handle_event` rustdoc explicitly forbids
calling `cx.stop_propagation()` from inside the trait implementation
(see the trait contract above). The arena's winner is declared via
`GestureDisposition::Accepted`, not via propagation control. T2 review
established this rule as a load-bearing backward-compat invariant.

### `Interactivity` fluent builders <a id="interactivity-fluent-builders"></a>

T14 extends `Interactivity` with two new fields:

```rust
// crates/flui-core/src/elements/div.rs (excerpt)
pub(crate) gesture_recognizers: SmallVec<[Box<dyn GestureRecognizer>; 4]>,
pub(crate) hit_test_behavior:    HitTestBehavior,
```

And the following fluent builders on `InteractiveElement` (placed
alongside `mouse_down`/`mouse_up`/`on_click`/`on_drag` to preserve
discoverability):

```rust
pub fn with_hit_test_behavior(mut self, behavior: HitTestBehavior) -> Self;

pub fn on_tap         (mut self, f: impl FnMut(TapDetails,         &mut Window, &mut App) + 'static) -> Self;
pub fn on_double_tap  (mut self, f: impl FnMut(DoubleTapDetails,   &mut Window, &mut App) + 'static) -> Self;
pub fn on_long_press_start (mut self, f: impl FnMut(LongPressDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_long_press_move  (mut self, f: impl FnMut(LongPressDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_long_press_end   (mut self, f: impl FnMut(LongPressDetails, &mut Window, &mut App) + 'static) -> Self;

pub fn on_pan_start         (mut self, f: impl FnMut(DragStartDetails,  &mut Window, &mut App) + 'static) -> Self;
pub fn on_pan_update        (mut self, f: impl FnMut(DragUpdateDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_pan_end           (mut self, f: impl FnMut(DragEndDetails,    &mut Window, &mut App) + 'static) -> Self;
pub fn on_horizontal_drag_start  (mut self, f: impl FnMut(DragStartDetails,  &mut Window, &mut App) + 'static) -> Self;
pub fn on_horizontal_drag_update (mut self, f: impl FnMut(DragUpdateDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_horizontal_drag_end    (mut self, f: impl FnMut(DragEndDetails,    &mut Window, &mut App) + 'static) -> Self;
pub fn on_vertical_drag_start    (mut self, f: impl FnMut(DragStartDetails,  &mut Window, &mut App) + 'static) -> Self;
pub fn on_vertical_drag_update   (mut self, f: impl FnMut(DragUpdateDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_vertical_drag_end      (mut self, f: impl FnMut(DragEndDetails,    &mut Window, &mut App) + 'static) -> Self;

pub fn on_scale_start  (mut self, f: impl FnMut(ScaleStartDetails,  &mut Window, &mut App) + 'static) -> Self;
pub fn on_scale_update (mut self, f: impl FnMut(ScaleUpdateDetails, &mut Window, &mut App) + 'static) -> Self;
pub fn on_scale_end    (mut self, f: impl FnMut(ScaleEndDetails,    &mut Window, &mut App) + 'static) -> Self;
```

All callback signatures match the existing `AnyMouseListener`
shape (`Box<dyn FnMut(&dyn Any, DispatchPhase, &mut Window, &mut
App)>`) — the `Window` is needed for `window.refresh()` after
state changes inside the callback, and `App` is needed for
`cx.spawn`, `cx.notify`, and `cx.active_drag` access.

Each `on_X_*` builder appends a recognizer to `gesture_recognizers` if
no recognizer of that type exists yet, or installs the callback on the
existing recognizer. The recognizers consume `&GestureSettings` from
the parent `Window` at paint time (the `Interactivity::paint` site
injects them via `Rc::clone(&window.gesture_binding.settings)`).

## API surface

S07 introduces the following **public** items in `flui-core`:

### Types (struct / enum)

| Name | Module | `#[non_exhaustive]` | Notes |
|---|---|---|---|
| `PointerId` | `gesture::pointer_event` | n/a (newtype) | `pub`. Inner `u64` is `pub(crate)` |
| `PointerKind` | `gesture::pointer_event` | yes | `pub`. `Mouse | Touch | Stylus` |
| `PointerPhase` | `gesture::pointer_event` | yes | `pub`. 9 variants |
| `PointerButtons` | `gesture::pointer_event` | n/a (bitfield newtype) | `pub`. Inner `u32` is `pub(crate)`; access via `bits()` / `contains()` / consts |
| `PointerEvent` | `gesture::pointer_event` | yes | `pub`. All-`pub` fields, `Clone + Debug` |
| `PointerSignalEvent` | `gesture::pointer_signal` | yes | `pub`. `Scroll | Magnify` |
| `HitTestEntry` | `gesture::hit_test` | yes | `pub`. Owned by `HitTestResult` |
| `HitTestResult` | `gesture::hit_test` | no | `pub`. Iterator surface (sealed by private `entries`) |
| `HitTestBehavior` | `gesture::hit_test` | yes | `pub`. `Opaque | Translucent | DeferToChild` |
| `GestureSettings` | `gesture::gesture_settings` | yes | `pub`. All thresholds |
| `GestureBinding` | `gesture::binding` | n/a | `pub`. Owns arena+settings+sanitizer; `arena_mut()` is `pub(crate)`-only |
| `GestureArena` | `gesture::arena` | n/a | **`pub(crate)`**. Internal — exposed only via `pub(crate) GestureBinding::arena()` |
| `GestureArenaEntry` | `gesture::arena` | n/a | **`pub(crate)`**. Internal |
| `GestureArenaManager` | `gesture::arena` | n/a | **`pub(crate)`**. Internal |
| `GestureArenaTeam` | `gesture::arena_team` | yes | `pub`. Constructor `with_captain(Box<dyn GestureRecognizer>)` |
| `GestureDisposition` | `gesture::arena` | yes | `pub`. `Accepted | Rejected | Possible` |
| `PointerSanitizer` | `gesture::dispatch` | n/a | **`pub(crate)`**. Internal — no public methods; not in the re-export block |
| `WindowPointerState` | `gesture::dispatch` | n/a | **`pub(crate)`**. Internal sanitizer-state cache |
| `Velocity` | `gesture::velocity_tracker` | yes | `pub`. Public-field struct |
| `VelocityTracker` | `gesture::velocity_tracker` | n/a | `pub`. LSQ-fit estimator |
| `PositionSample` | `gesture::velocity_tracker` | yes | `pub`. Used as `VelocityTracker::add_position` argument |
| `SemanticAction` | `gesture::recognizer` | yes | `pub`. `Tap | DoubleTap | LongPress` (S08 will extend) |
| `TapGestureRecognizer` | `gesture::recognizers::tap` | yes | `pub`. + `TapDetails`, `TapDownDetails`, `TapUpDetails` (all `#[non_exhaustive]`) |
| `DoubleTapGestureRecognizer` | `gesture::recognizers::double_tap` | yes | `pub`. + `DoubleTapDetails` (`#[non_exhaustive]`) |
| `LongPressGestureRecognizer` | `gesture::recognizers::long_press` | yes | `pub`. + `LongPressDetails` (`#[non_exhaustive]`) |
| `PanGestureRecognizer` | `gesture::recognizers::drag` | yes | `pub`. + `DragStartDetails`, `DragUpdateDetails`, `DragEndDetails` (all `#[non_exhaustive]`) |
| `HorizontalDragGestureRecognizer` | `gesture::recognizers::drag` | yes | `pub`. Same Details types as Pan |
| `VerticalDragGestureRecognizer` | `gesture::recognizers::drag` | yes | `pub`. Same Details types as Pan |
| `ScaleGestureRecognizer` | `gesture::recognizers::scale` | yes | `pub`. + `ScaleStartDetails`, `ScaleUpdateDetails`, `ScaleEndDetails` (all `#[non_exhaustive]`) |

### Traits

| Name | Module | Object-safety | Notes |
|---|---|---|---|
| `GestureRecognizer` | `gesture::recognizer` | yes (verified by doc-test in T7 + T2 review) | `?Sync`, `?Send` |

### Methods on existing types

| Owner | Method | Visibility |
|---|---|---|
| `Window` | `hit_test(&self, position: Point<Pixels>) -> HitTestResult` | `pub` |
| `Window` | `gesture_binding(&self) -> &GestureBinding` | `pub` |
| `Window` | `gesture_binding_mut(&mut self) -> &mut GestureBinding` | `pub` |
| `Window` | `gesture_settings_mut(&mut self) -> &mut GestureSettings` | `pub` (S14 seam shortcut) |
| `InteractiveElement` | All `on_tap`, `on_double_tap`, `on_long_press_*`, `on_pan_*`, `on_horizontal_drag_*`, `on_vertical_drag_*`, `on_scale_*` builders | `pub` |
| `InteractiveElement` | `with_hit_test_behavior(behavior: HitTestBehavior) -> Self` | `pub` |
| `Interactivity` | private fields `gesture_recognizers`, `hit_test_behavior` | `pub(crate)` |

### Re-exports

T3 + T19 add the explicit per-symbol `pub use gesture::{ … }` block in
`crates/flui-core/src/lib.rs` (full enumeration in
[Current state § Public re-export discipline](#public-re-export-discipline)).
**No** glob `pub use gesture::*;` is introduced, in line with the
S01a.3 explicit-re-export discipline. The block is documented as the
S07 entry per the existing convention.

### What S07 does **not** add to the public surface

- **No new error types.** Arena, sanitizer, and recognizer methods are
  infallible at the public API. Cross-cutting **A3** (Error-type
  unification) reasoning: introducing `GestureError` here would
  proliferate `Box<dyn Error>` patterns that A3 wants to consolidate.
  Instead, invariant violations panic in debug-mode (`debug_assert!`)
  and log-and-continue in release.
- **No public macros.** No `gesture!` / `recognize!` macros. The
  fluent builder pattern is sufficient.
- **No public free functions.** All entry points are methods on
  `Window` or builders on `InteractiveElement`.
- **No new feature flags.** S07 ships unconditionally on all targets.
  The PinchEvent `#[cfg]` gate is preserved exactly as today (the
  `From<&PinchEvent>` impl carries the same gate).

## Migration / Compatibility

### Backward-compatibility contract

**No existing public API symbol changes name, signature, or semantics
under S07.** Concretely:

1. **Existing `Interactivity` listener vectors** (`mouse_down_listeners`,
   `mouse_up_listeners`, `mouse_pressure_listeners`,
   `mouse_move_listeners`, `scroll_wheel_listeners`, `pinch_listeners`,
   `click_listeners`, `aux_click_listeners`, `drag_listener`,
   `hover_listener`) are **untouched**. Their firing order, count, and
   payload shape are identical pre- and post-S07. Tests in T16/T17 do
   not gate on this — the existing
   [`crates/flui-core/src/elements/div.rs`](../../../crates/flui-core/src/elements/div.rs)
   and
   [`crates/flui-core/src/interactive.rs`](../../../crates/flui-core/src/interactive.rs)
   test suites must remain green at every commit checkpoint as a
   load-bearing regression gate.

2. **The `cx.active_drag` / `AnyDrag` flow** is untouched. T15
   explicitly preserves the order: `dispatch_mouse_event` runs after
   `arena.dispatch`, so any code that sets `cx.active_drag` from
   inside an `on_mouse_down` listener still does so on every event
   (the arena dispatch in front of it neither reads nor writes
   `active_drag`). The S07 demo (T18) includes a
   "drag-and-drop alongside Pan recognizer" scenario.

3. **`HitboxBehavior` semantics** (`Normal` / `BlockMouse` /
   `BlockMouseExceptScroll`) are unchanged. `is_hovered` and
   `should_handle_scroll` keep their existing behavior. The new
   `HitTestBehavior` is **orthogonal** (per the table in
   [Design § HitTestEntry/HitTestResult](#hit-test-result-and-entry)).

4. **`PlatformInput` enum** is **not** changed. T4's normalized
   `PointerEvent` is constructed **from** existing `PlatformInput`
   variants, not in place of them. The platform layer continues to
   emit `PlatformInput`. Conversion is a one-way edge.

5. **`Window::dispatch_event`** signature is unchanged. The function
   body grows the new translation step **before** the existing
   `dispatch_mouse_event` call.

6. **`MouseClickEvent`** synthesis logic in `InteractiveElement` is
   unchanged. The new `Tap` recognizer fires in parallel with the
   existing click listeners; it does not replace them.

### Per-checkpoint compatibility verification

Each commit checkpoint must produce a workspace that passes
`cargo test --workspace --all-targets` and `cargo clippy --workspace
--all-targets -- -D warnings`. The plan checkpoint table (B–F in the
plan file) is structured to keep this property at every point:

| Checkpoint | What lands | What still works |
|---|---|---|
| B (after T3,T4,T5,T6,T20) | Pointer events + hit-test pass + sanitizer | All existing tests (no recognizers wired yet) |
| C (after T7,T8,T21,T9) | Arena, binding, settings, velocity tracker | Existing tests (arena unused; binding owned but inert) |
| D (after T10,T11,T12,T13) | Five recognizers exist and pass unit tests | Existing tests (recognizers not wired into Interactivity yet) |
| E (after T14,T15) | Interactivity fluent builders + dispatch wiring | Existing tests (raw mouse_* listeners still fire in parallel) |
| F (after T16,T17,T22,T23,T18,T19) | Tests, bench, demo, rustdoc, roadmap | Existing tests (now reinforced by new tests) |

### Downstream-crate impact

Outside `flui-core`, only the explicit re-export list grows. No other
crate (`flui-widgets`, `flui-navigator`, `flui-animate`, `flui-a11y`,
`flui-theme`, `flui-material`, `flui-macros`) is touched.
`examples/nav_demo`, `examples/material_demo`, `examples/animation_demo`
do not depend on the new types and their `cargo check` outputs
identically before and after.

### `cargo-semver-checks` outlook

When **R2** lands (`cargo-semver-checks` in CI), S07's additions are
all `MajorVersion::None` (additive symbols, no signature changes). The
`#[non_exhaustive]` discipline on every public enum and `GestureSettings`
guarantees that Phase III stylus / mobile work can extend variants and
fields without bumping major.

## Testing strategy

### Unit tests (T16, T17, T23)

- **`crates/flui-core/tests/gesture_arena_lifecycle.rs`** (T16):
  - `arena_single_accept_short_circuits_others`
  - `arena_eager_accept_during_event_dispatch`
  - `arena_sweep_first_registered_wins_on_up`
  - `arena_dispose_recognizer_mid_sequence_no_callback_after_drop`
  - `arena_hold_keeps_arena_open_past_up`
  - `arena_release_resumes_normal_resolution`
  - `arena_team_captain_resolves_member_accept_to_possible`
  - `arena_team_captain_accept_resolves_team`
  - `arena_cancel_synthesizes_rejected_for_all_entries`

- **`crates/flui-core/tests/gesture_recognizers.rs`** (T17):
  - Tap: primary/secondary/tertiary button matrix
  - Tap: rejection on Move > slop
  - Tap: rejection on second pointer arriving
  - DoubleTap: two-tap acceptance within window
  - DoubleTap: rejection on > timeout, on < min_time, on Move > slop
  - LongPress: synthetic clock acceptance after timeout
  - LongPress: rejection on Move > slop, on Up before timeout
  - LongPress: drop cancels timer (callback after drop count assertion)
  - Drag: pan slop acceptance, axis-rejection for HorizontalDrag /
    VerticalDrag
  - Drag: velocity at end (sanity-check the LSQ output for a known
    sample sequence)
  - Scale: focal-point and scale math for two synthetic pointers

- **`crates/flui-core/tests/gesture_arena_proptest.rs`** (T23): six
  properties over arena and team state machines.
  - **P1** (lifecycle invariant): every entry that joins the arena
    receives exactly one of `sweep_accepted` or `rejected` before the
    arena closes.
  - **P2** (eager-accept dominance): if any recognizer returns
    `Accepted`, no recognizer subsequently receives `sweep_accepted`.
  - **P3** (sweep semantics): if no recognizer ever returns `Accepted`,
    the entry at index 0 of `entries` at sweep time receives
    `sweep_accepted` and all others receive `rejected`.
  - **P4** (cancel-on-cancel): a `Cancel` event causes every remaining
    entry to receive `rejected` exactly once and no `sweep_accepted`
    callbacks for that pointer.
  - **P5** (team captain conversion): an `Accepted` reported by a team
    member is observed by the arena as `Possible`; an `Accepted`
    reported by the captain resolves the team.
  - **P6** (no callback after drop): dropping a recognizer mid-arena
    cannot cause any further callback (`sweep_accepted` or `rejected`)
    on it.

  All six properties run without GPU or `Window` instances (pure
  arena + recognizer logic), so `cargo-llvm-cov` (T1 of the testing
  track) covers them without GPU CI legs.

### Integration tests (none in T-track)

S07 does not add integration tests; the listener-level integration is
covered by existing tests in
[`crates/flui-core/src/elements/div.rs`](../../../crates/flui-core/src/elements/div.rs)
which must remain green at every checkpoint.

### Bench fixture (T22)

`crates/flui-core/examples/bench/gesture_arena_bench.rs` is a
runnable example matching the existing
[`paths_bench`](../../../crates/flui-core/examples/bench/paths_bench.rs)
pattern. Three sub-benchmarks with explicit pass/fail thresholds:

```text
hit_test_8deep:
  Build a synthetic 8-deep nested hitbox tree; run 1_000_000
  Window::hit_test queries; report mean per-query latency.
  Threshold: < 2µs/query (M2-class, release profile).

arena_tick:
  Construct an arena with 8 mock recognizers, each returning
  Possible until a configurable event count, then Accepted; dispatch
  1_000_000 PointerEvents through it; report mean per-event-recognizer
  latency.
  Threshold: < 1.25µs/event-recognizer.

full_frame_120hz:
  Wire the full pipeline (hit-test + arena + recognizer dispatch) into
  a synthetic 60-element InteractiveElement tree; simulate a 1-second
  120Hz drag sequence (240 events); report p99 frame time.
  Threshold: < 8ms p99.
```

The bench runs as `cargo run -p flui-core --release --example
gesture_arena_bench`. Failure of any threshold is a checkpoint-blocking
event for the corresponding commit.

### Demo (T18)

`crates/flui-core/examples/learn/gesture_arena_demo.rs` follows the
existing
[`interactive_elements`](../../../crates/flui-core/examples/learn/interactive_elements.rs)
pattern. Four scenarios in tabbed panes:

1. **Competing recognizers** — overlapping panes with `Tap`,
   `DoubleTap`, `LongPress`, `Pan` recognizers; status bar shows which
   recognizer accepted each gesture.
2. **Captain-team** — three sibling buttons with their own `Tap`s,
   wrapped in a `Pan` captain; verify panning across buttons defers to
   pan, not tap.
3. **Translucent overlay** — a hit-test-translucent overlay that
   forwards taps to the element behind it.
4. **Settings override** — a settings panel that mutates
   `window.gesture_settings_mut()` live (e.g. drag the
   `long_press_timeout` slider and observe the recognizer behavior
   change).

The example accepts a `--headless-smoke` CLI flag that runs each
scenario with synthetic events and asserts on the recognizer outputs,
making the demo runnable in CI as a regression gate. This matches the
existing pattern used by `examples/learn/animation.rs`.

### Lock-checks

`cargo run -p lock-checks -- check-stubs` and
`… check-platform-imports` must remain zero-diff at every checkpoint.
Since S07 introduces no new files under
`crates/flui-core/src/platform/**` and no new `unimplemented!()`,
`todo!()`, or `unreachable!()` sites, both checks remain trivially
green for the existing scan paths.

**Caveat raised in T2 review:** `tooling/lock-checks` currently scans
only `crates/flui-core/src/platform/**`. Stubs accidentally introduced
inside `crates/flui-core/src/gesture/**` would not be caught. T3
explicitly forbids `unimplemented!()`/`todo!()`/`unreachable!()` in
the new module via inline rustdoc on every empty stub file (the
empty modules contain only `//!` doc comments, not stub bodies).
Extending `lock-checks` to scan `gesture/**` is an optional follow-up
(documented as a nit in [Open questions § 11](#open-questions)) —
not a blocker for S07.

## Open questions

These were resolved during **T2 architectural review** by
`flui-arch-reviewer` and `rust-api-migration-auditor`. Resolutions
are inlined here; the landing PR for Phase A captures both reviews
verbatim in the commit body.

### Blockers (resolved before any code lands)

1. **Arena-vs-`AnyDrag` `cx.stop_propagation()` regression** —
   resolved. The dispatch flow at the T6/T15 integration point now
   explicitly resets `cx.propagate_event = true` between the arena
   pass and the existing `dispatch_mouse_event` call. The
   `GestureRecognizer::handle_event` rustdoc forbids calling
   `cx.stop_propagation()` from inside the trait — the arena's
   winner is declared via `GestureDisposition::Accepted`, not via
   propagation control. See [Design § Window::dispatch_event
   integration](#windowdispatch_event-integration) for the updated
   flow diagram and trait contract.

2. **`GestureRecognizer` method signatures missing `&mut Window`** —
   resolved. `handle_event`, `sweep_accepted`, `rejected`, and
   `arena.dispatch`/`sweep`/`cancel`/`release`/`declare_winner` all
   take `(&mut Window, &mut App)` to match the existing
   `AnyMouseListener` signature
   ([`window.rs:531-532`](../../../crates/flui-core/src/window.rs#L531-L532))
   and to allow `window.refresh()` from inside a recognizer
   callback.

3. **`GestureArena`/`GestureArenaEntry`/`GestureArenaManager` had no
   `pub` methods or fields** — resolved. All three are now
   `pub(crate)` (see API surface table). `GestureBinding::arena()`
   and `arena_mut()` are also `pub(crate)`. Two thin
   `pub` observers (`active_pointer_count`, `arena_entry_count`)
   replace the previously-`pub` arena handle for test introspection.

4. **`PointerSanitizer` had no public consumer surface** — resolved.
   It is now `pub(crate)` and removed from the re-export block.
   `WindowPointerState` (introduced as the sanitizer's per-`Window`
   state cache) is also `pub(crate)`.

5. **Phantom `DragGestureRecognizer` in re-export block** —
   resolved. Removed; the three recognizers are
   `PanGestureRecognizer`, `HorizontalDragGestureRecognizer`,
   `VerticalDragGestureRecognizer`.

6. **Missing `PositionSample` in re-export block** — resolved.
   Added; required because `VelocityTracker::add_position` is `pub`
   and takes `PositionSample` by value.

### Should-fixes (resolved before any code lands)

7. **`PointerButtons.0: pub u32` over-exposed** — resolved. Inner
   field is `pub(crate)`; `pub fn bits(self) -> u32` added for
   inspection. The `PRIMARY` / `SECONDARY` / `TERTIARY` constants
   plus `contains()` / `is_empty()` are the consumer API.

8. **Missing `#[non_exhaustive]` on data structs** — resolved.
   Applied to `PointerEvent`, `HitTestEntry`, `Velocity`,
   `PositionSample`, all 10 `*Details` types, and all 5
   recognizer structs (so future fields like `pressure`, `tilt`,
   per-button modifiers are non-breaking additions).

9. **`GestureArenaTeam::new` took `Rc<RefCell<…>>`** — resolved.
   Replaced with `with_captain(Box<dyn GestureRecognizer>)` and
   `add_member(Box<dyn GestureRecognizer>)`. Internal
   `Rc<RefCell<…>>` plumbing is hidden.

10. **LongPress arena back-channel was unspecified** — resolved.
    Recognizers store
    `Weak<RefCell<GestureArenaManager>>` (the `arena_back_channel`
    field) plus a `pointer_index: Option<usize>` recorded inside
    `add_pointer`. From inside the spawned timer future the
    recognizer upgrades the `Weak`; if the upgrade fails (Window
    dropped), the recognizer no-ops. See
    [Design § LongPressGestureRecognizer](#long-press-recognizer).

11. **`cx.window().gesture_binding()` was a phantom accessor** —
    resolved. All call-site references replaced with
    `window.gesture_binding()` /
    `window.gesture_binding_mut()` /
    `window.gesture_settings_mut()` (where `window: &mut Window`
    comes from the enclosing closure parameter or
    `Render::render(&mut self, window, cx)`).

12. **Re-export block nested sub-paths deviated from S01a.3** —
    resolved. Split into two flat blocks: the gesture-core block
    and a separate `pub use gesture::recognizers::{ … }` block.

### Pre-existing design choices (no change)

13. **`GestureRecognizer` is `?Send + ?Sync`** — confirmed correct
    by both reviewers. Per-`Window` callback registry is
    main-thread-only; matches existing `Interactivity` posture
    (which also captures `!Send + !Sync` callback boxes).

14. **`HitTestBehavior` strictly orthogonal to `HitboxBehavior`** —
    confirmed. The two-table cross-reference in
    [Design § HitTestEntry/HitTestResult](#hit-test-result-and-entry)
    documents the difference. Both rustdocs cross-reference each
    other to soften the discoverability hazard noted by the
    auditor.

15. **`PointerEvent` is `Clone + Debug`, not `Copy`** — confirmed
    pending T22 measurement. Auditor agreed the bench-driven
    decision is appropriate; non-`Copy` does not constrain
    ergonomics today.

16. **`PointerSignalEvent` placement** — confirmed. Existing
    `scroll_wheel_listeners` and `pinch_listeners` keep their
    binary-compatible signatures; the new `PointerSignalEvent` is
    a **wire** type, not a listener API.

17. **Scale recognizer is 2-pointer-only** — confirmed.
    Documented in [Explicit gaps](#explicit-gaps).

### Remaining nits (resolved during implementation, not blocking T1)

18. **Auto-trait documentation** — `GestureBinding` rustdoc now
    explicitly notes `!Send + !Sync` posture. Implementers must
    add equivalent rustdoc on `GestureArena`, `GestureArenaTeam`
    (per the `// SAFETY: !Send intentional` pattern).

19. **`GestureEvent` (existing trait at
    [`interactive.rs:21`](../../../crates/flui-core/src/interactive.rs#L21))
    naming overlap** — keep both. The existing `GestureEvent` is
    the platform-input marker trait (e.g. `impl GestureEvent for
    PinchEvent`); the new `Gesture*` types are the higher-level
    arena/recognizer facade. Cross-reference in `gesture::mod.rs`
    rustdoc to disambiguate. Discoverability hazard mitigated;
    not a Blocker.

20. **Extending `lock-checks check-stubs` to scan
    `gesture/**`** — optional follow-up, not a S07 gate. Tracked
    as a future tooling improvement adjacent to **T1** (Code
    coverage).

21. **CI matrix coverage for T22 bench** — scheduled CI only
    (matches **T2** `cargo-fuzz` posture), align with **T4**
    Criterion-suite policy when that lands.

22. **WASM `smol::Timer` behavior** — confirmed inherited from
    existing `executor.rs` pattern. T11 LongPress uses the same
    spawn shape as the animation controller; no new wasm-target
    work needed in S07.

## Architectural decisions log

### Threading model

**Decision:** `GestureRecognizer: ?Sync` (main-thread only).

**Rationale:** Recognizers self-mutate from inside arena callbacks
(eager-accept may run user code that mutates the recognizer state).
Cross-task `Send` is not needed — `Interactivity` already lives in
`!Send` territory (it captures `&mut Window` and `&mut App` in callback
boxes). Matching the GPUI-derived single-threaded UI runtime model.

**Trade-off:** Multi-threaded gesture pre-processing (e.g. velocity
estimation on a background task) is impossible without explicit
opt-in. We accept this; it maps to Flutter's main-thread-only
`GestureBinding`.

### Logging <a id="log-vs-tracing"></a>

**Decision:** `log` crate + `kv_unstable_serde` (no `tracing`).

**Rationale:** Matches existing `flui-core` convention
([`executor.rs:317`](../../../crates/flui-core/src/executor.rs#L317),
the wgpu renderer module use `log::warn!` already). `tracing` would
introduce a new workspace dep and require A4-track decisions on span
hierarchy, sampling, and exporter integration. Those are the wrong
discussions to have in a feature spec.

**kv-field schema** (consumed across all gesture-subsystem log calls):

| Field name      | Type      | Description                                  |
|-----------------|-----------|----------------------------------------------|
| `pointer_id`    | u64       | The `PointerId.0` value                      |
| `recognizer`    | &str      | `GestureRecognizer::name()`                  |
| `phase`         | &str      | `Debug` of `PointerPhase`                    |
| `arena_state`   | &str      | `"open" | "held" | "closed" | "cancelled"`  |
| `widget_id`     | Option<u64> | `HitboxId.0` (if known)                    |
| `synthesized`   | bool      | True for sanitizer-emitted events           |
| `reason`        | &str      | Human-readable reason (sanitizer cancel)    |
| `disposition`   | &str      | `"accepted" | "rejected" | "possible"`      |

**Trade-off:** Discoverability in production tools (Tracy, Tokio
Console, etc.) is limited until A4 lands. Migration plan: when A4
picks `tracing`, every `log::trace!` / `log::debug!` site swaps to
`tracing::trace!` / `tracing::debug!` mechanically; the kv fields
trivially become span fields.

### Error policy

**Decision:** Infallible at the public API surface; no new error
types.

**Rationale:** Aligns with cross-cutting **A3** (Error-type
unification — avoid `Box<dyn Error>` proliferation). Arena ops cannot
fail in-band: invalid recognizer state is a programming bug
(`debug_assert!`), not a runtime error. Sanitizer "rejection" is not
an error — it's a sanitization outcome, captured in logs.

**Trade-off:** Out-of-bounds events (e.g. `pointer_id` not in any
arena) are silently dropped with a `log::warn!`. Tests in T16 verify
no panic on that path.

### Interior mutability

**Decision:** `Rc<RefCell<dyn GestureRecognizer>>` in arena entries;
every `RefCell` carries an A7 audit comment.

**Rationale:** Required because recognizers self-mutate from inside
arena callbacks. Public surface (the `Box<dyn GestureRecognizer>` that
`Interactivity::on_tap` accepts) is the boxed-trait analog used by
`Box<dyn Action>`
([`action.rs:117`](../../../crates/flui-core/src/action.rs#L117)) and
`Box<dyn Simulation>`
([`animation/simulation.rs:22`](../../../crates/flui-core/src/animation/simulation.rs#L22)).
Internally the arena promotes to `Rc<RefCell<…>>` because multiple
arena codepaths need shared mutable access.

**Audit posture (A7):** the `Rc<RefCell<…>>` is **not** part of the
public API. It lives behind `pub(crate)` arena types. The
`auto-trait` set on the public `GestureRecognizer` and on
`Interactivity` is therefore unaffected by this internal mutability.
T7 + T8 carry explicit inline `// A7-audit: …` comments on every such
site.

**Trade-off:** Borrow-check failures during arena dispatch are caught
at runtime, not compile time. T16 / T23 stress this with high-fanout
arena tests.

### Public enum extensibility

**Decision:** `#[non_exhaustive]` on all new public enums:
`PointerKind`, `PointerPhase`, `HitTestBehavior`,
`PointerSignalEvent`, `GestureDisposition`, `SemanticAction`. Plus
`GestureSettings` is `#[non_exhaustive]` as a struct.

**Rationale:** Aligns with cross-cutting **A8** — adding `Stylus`,
`Hover`, future scroll granularities, future semantic actions, future
sanitizer reasons is non-breaking.

**Trade-off:** Pattern-match callers must include `_ => …` arms (the
non-exhaustive contract). Documentation in each enum explicitly notes
this and gives a recommended fallback.

### Allocations on hot path

**Decision:** Zero allocations in `Window::dispatch_event` →
`gesture::dispatch::translate` → `arena.dispatch`.

**Constraints:**

- `PointerSanitizer::convert` returns `SmallVec<[PointerEvent; 2]>`
  (inline storage 2 — sufficient for `Cancel + Down` synthesis).
- `GestureArena.entries: SmallVec<[GestureArenaEntry; 4]>` (inline
  storage 4 — typical max competitor count is 3 — Tap + LongPress +
  Pan).
- `GestureArenaManager.arenas: SmallVec<[(PointerId, GestureArena); 4]>`
  (inline storage 4 — typical max active pointer count is 2 on
  desktop, 4 on multi-touch).
- `VelocityTracker.samples: VecDeque<PositionSample>` is allocated
  **once** per recognizer (bounded by
  `GestureSettings::velocity_tracker_samples`).
- `HitTestResult.entries: SmallVec<[HitTestEntry; 8]>` matches the
  existing `HitTest.ids: SmallVec<[HitboxId; 8]>` shape.

T22 enforces this with a **debug_assertions-only** allocation counter
that records the heap allocations made during the bench loop and
asserts zero across all three sub-benchmarks. The counter uses the
existing
[`profiling`](../../../crates/flui-core/Cargo.toml#L84) crate's
no-op-by-default hooks; release builds carry no overhead.

### `PointerEvent` / `PointerSignalEvent` split

**Decision:** Two distinct types; signals bypass the arena.

**Rationale:** Matches Flutter's `PointerSignal` separation. Scroll
and magnify do not "compete" — there is no winner of a scroll wheel
tick. They go to dedicated listeners on the deepest hit-test target
(plus translucent forwarding) without arena resolution.

**Trade-off:** Recognizers cannot react to scroll. If we later need a
"scroll-to-zoom" recognizer (a common UX pattern on Windows), it
would consume `PointerSignalEvent::Scroll` directly via a new
`SignalRecognizer` trait. Out of scope for S07; tracked as Phase III
item.

### Backward compatibility

**Decision:** Mechanical, not best-effort. Raw `on_mouse_*` /
`on_click` / `AnyDrag` continue to fire in parallel with the arena.

**Rationale:** Existing `flui-core` consumers (and the `examples/*`
demos) cannot regress on this PR. The arena is **additive**: T15
explicitly preserves the existing `dispatch_mouse_event` chain after
arena dispatch.

**Trade-off:** A small overhead per pointer event (the arena pass
runs even if no `Interactivity` registered any recognizers). T22
measures this — empty-arena `arena_tick` is < 100ns/event, well below
the `< 1.25µs/event-recognizer` threshold.

## Performance budgets <a id="performance-budgets"></a>

Verified by **T22** (`cargo run -p flui-core --release --example
gesture_arena_bench`):

| Sub-bench | Operation | Budget | Rationale |
|---|---|---|---|
| `hit_test_8deep` | Single hit-test query in 8-deep nested tree | < **2µs**/query | Frame-budget headroom. Linear scan over `SmallVec<[HitboxId; 8]>` plus per-id `HashMap<HitboxId, HitTestBehavior>` lookup. |
| `arena_tick` | Single PointerEvent through arena with 8 competing recognizers | < **1.25µs**/event-recognizer | < 5% of an 8.33ms 120Hz frame budget for 8 recognizers and 1 event. |
| `full_frame_120hz` | Full pipeline (hit-test + arena step + recognizer dispatch) per frame | < **8ms** p99 | Flutter parity reference: Flutter targets 16ms at 60Hz; we target half that at 120Hz. |
| `empty_arena_tick` | Single PointerEvent with no recognizers (regression gate) | < **100ns**/event | Validates that the new dispatch path adds negligible overhead when no `Interactivity::on_tap` is in play. |

**Reference hardware:** Apple M2-class. The CI runners (GitHub
Actions) are 1.5-2× slower; thresholds in CI are scaled by 2× per
existing project convention.

**Allocation budget:** zero allocations on the dispatch hot path
(post-recognizer-construction). VelocityTracker samples are inserted
into a pre-allocated `VecDeque`; arena entries live in inline-storage
`SmallVec`. The debug-assertions allocation counter in T22 enforces
this.

## Cross-cutting roadmap interactions

| Cross-cutting | This plan's contract |
|---|---|
| **A2** — Audit remaining glob re-exports | S07 adds an explicit per-symbol `pub use gesture::{ … }` block (no glob). Does **not** convert any of the existing ~29 globs (out of A2's scope). |
| **A3** — Error-type unification | S07 introduces no new error types; arena, sanitizer, and recognizer methods are infallible at the public API. Aligns with A3's preference to avoid `Box<dyn Error>` proliferation. |
| **A4** — Tracing standardization | S07 uses `log` + `kv` provisionally; this design doc defines the kv-field schema (Architectural decisions § Logging) so an A4-driven migration to `tracing` is mechanical. |
| **A5** — Feature flag matrix discipline | S07 introduces no new feature flags. The `#[cfg(any(linux, macos))]` gate on `From<&PinchEvent>` is preserved exactly as today. |
| **A6** — `[workspace.dependencies]` migration | S07 adds no new workspace dependencies. When A6 lands, `smallvec`, `parking_lot`, `circular-buffer`, `proptest`, and `log` migrate together. |
| **A7** — Interior-mutability surface reduction | Every `Rc<RefCell<dyn GestureRecognizer>>` use carries an explicit `// A7-audit: …` comment. Public types are opaque newtype-wrapped (`GestureBinding`, `GestureArenaManager`) so the auto-trait set on the public surface stays curated. |
| **A8** — `#[non_exhaustive]` audit | All public enums introduced here (`PointerKind`, `PointerPhase`, `HitTestBehavior`, `PointerSignalEvent`, `GestureDisposition`, `SemanticAction`) carry `#[non_exhaustive]`, plus `GestureSettings` (struct). |
| **A9** — Crate-boundary review | S07's `gesture/` module is a clear extraction candidate for a future `flui-gesture` sibling crate (post-S02b). The module is self-contained except for `PointerEvent`'s use of `Modifiers` / `Pixels` / `Point` (which would re-export from `flui-core` when extracted). |
| **T1** — Code coverage in CI | Tests structured to support `cargo-llvm-cov`: pure-logic property tests (T23), recognizer unit tests (T17), arena lifecycle tests (T16) all run without GPU/runtime deps. |
| **T2** — `cargo-fuzz` targets | A future fuzz target on `PointerSanitizer::convert(input, state)` is straightforward — input bytes → `PlatformInput` → asserts no panic. Out of scope for S07. |
| **T3** — Property-based tests with `proptest` | T23 adds property tests for arena & team state machines (six properties listed in Testing strategy). When T3 expands proptest coverage, the gesture state machines become reference targets. |
| **T4** — Criterion benchmark suite | S07 does **not** introduce `criterion`. Instead, T22 follows the existing `examples/bench/*.rs` pattern with explicit pass/fail thresholds. When T4 lands and adopts `criterion`, T22 fixtures become reference baselines. |
| **T5** — Mutation testing pilot | The arena state machine (`arena.rs`) is a strong candidate for `cargo-mutants` once T5 lands. The six P1–P6 properties exercise enough state transitions that mutation testing gives reasonable kill rates. |
| **T6** — Visual regression suite | S07 adds no GPU-bearing tests. T22 bench runs in headless mode; T18 demo accepts `--headless-smoke` for non-GPU CI. |
| **S08** — Semantics protocol | `GestureRecognizer::semantic_actions()` is a default-empty hook. S08 will populate Tap/DoubleTap/LongPress recognizers' overrides via the `SemanticAction` enum without a breaking change (it's `#[non_exhaustive]`). |
| **S09** — Canvas facade | S07 does not interact with the canvas facade. `HitTestResult` carries no canvas-coordinate concept; positions are window-local pixels (matching the existing scene primitives). |
| **S11** — Physics simulations | The `Velocity` payload is the seam for `S11`'s `Spring` / `Friction` / `Gravity` / `ScrollPhysics` integration with `AnimationController`. No coupling needed in S07; consumer code calls `velocity.pixels_per_second` directly. |
| **S12** — Focus traversal | `GestureRecognizer::on_focus_request()` is a default-empty hook. `Tap` returns `Some(focus_handle)` when `request_focus_on_tap_down` is set; `DoubleTap` returns `None`. S12 will plug `FocusTraversalPolicy` and read this hook on accept. |
| **S13** — Text parity | S07 does not interact with text rendering. Text-selection drag (a likely S13 item) will instantiate a `Pan` recognizer behind the scenes. |
| **S14** — MediaQuery completeness | `GestureSettings` is owned by `GestureBinding` and is mutable via `window.gesture_settings_mut()`. S14 will route `MediaQueryData::gesture_settings` here as the canonical seam. |
| **S15** — Asset bundle | No interaction. |
| **R2** — `cargo-semver-checks` | All S07 additions are semver-additive (no signature changes). When R2 lands, S07 changes pass clean. |

## Explicit gaps

These gaps are intentional in S07 and tracked for follow-up:

| Gap | Why it's deferred | Where it gets closed |
|---|---|---|
| **Stylus tilt / orientation / azimuth** | `MousePressureEvent` is macOS-trackpad-only force-touch and carries no tilt. `PointerKind::Stylus` and the `tilt`/`orientation` fields exist for forward-compat but are zero on all current platforms. | Platform-layer work in `crates/flui-platform/` once **S02b–S06** unfreeze + a dedicated stylus spec or **S20** desktop-gaps cleanup. |
| **Pinch rotation on desktop** | `PinchEvent.delta` is `f32` (scale only). Recognizer state machine carries rotation for future multi-pointer touch input but emits `0.0` on desktop today. | Wayland `pointer-gestures-unstable-v1` extension (deferred); macOS `NSEventTypeMagnify` does not carry rotation either, so this is a **multi-finger touch** unblock, not a desktop one. |
| **Windows native pinch** | `PinchEvent` is `#[cfg(any(linux, macos))]`. Windows desktop trackpad does not currently produce pinch events into `PlatformInput`. | **S20** desktop-gaps cleanup (Windows pinch via `RegisterTouchWindow` + `WM_GESTURE`). |
| **Spatial-index hit-test** | Current `SmallVec<[HitboxId; 8]>` linear scan is O(n); BVH/quadtree upgrade is unjustified at typical tree sizes. | A **P-track** perf milestone gated on `Window::hit_test` profiling showing > 5% of frame budget on > 100-hitbox trees. |
| **Pointer event pooling** | Zero-allocation path is preserved by reference passing; explicit pool of `PointerEvent` objects is unjustified at typical event rates. | A **P-track** perf milestone if `arena_tick` exceeds `< 1.25µs/event-recognizer` budget on real workloads. |
| **`tracing` migration** | Current `log` + `kv` is the right call until **A4** picks the workspace policy. | **A4** track. |
| **Trackpad-specific multi-finger gestures** (3-finger swipe, 4-finger pinch) | Wayland `pointer-gestures-unstable-v1` and macOS `NSPanGestureRecognizer` expose these; flui platform layer does not surface them yet. | A future spec (post-S20). |
| **Gesture-yield / disposition `Hold`** | The `GestureDisposition` enum is `#[non_exhaustive]` to admit a future `Hold` variant that lets a recognizer postpone arena resolution beyond `sweep`. | Out of scope for S07; future spec if needed for advanced UX. |
| **`PointerSignalEvent` recognizer interface** | Scroll / magnify currently bypass the arena entirely (they're non-competitive). A future `SignalRecognizer` trait would let consumers compete on signals (e.g. scroll-to-zoom on Windows). | Phase III item. |

## Common pitfalls

This section captures known traps that bit similar implementations
(Flutter's `gestures/` layer, GPUI's input pipeline) and that
implementers should watch for.

### `VelocityTracker` weight function

The `LeastSquaresSolver` weight is **not** linear in `(now - t)`. It
is a Gaussian-ish exponential (`exp(-((now - t) / horizon)^2)`).
A naïve linear weight overweights recent samples and produces
flickery `velocity.is_zero()` results at low motion rates. The
implementation in `velocity_tracker.rs` ports Flutter's exact weight
verbatim, with a 30-line ASCII diagram explaining the curve.

### Drop-cancel discipline for LongPress timer

`LongPressGestureRecognizer` schedules a timer via `cx.spawn(async {
Timer::after(d).await; … })`. The returned `Task` must be **stored as
a struct field** so dropping the recognizer drops the `Task` and
cancels the future. A common bug is to do `let _ = cx.spawn(…)` and
hope the future "knows" to cancel — it does not. T17 includes a
"callback after drop count assertion" test that fails if the timer
fires after the recognizer is dropped.

### Arena dispatch reentrancy

A recognizer's `handle_event` may run user code (the `on_tap` callback
when accepting). That user code may call back into
`Interactivity` builders or modify the recognizer registry on a
sibling element. This is **fine** — the arena dispatch operates on a
snapshot of `entries` at call time, not on the live registry. T16
verifies this with a "callback mutates sibling tree" test.

### `Translucent` propagation order

`HitTestBehavior::Translucent` means "I receive events **and** the
next entry behind me does too". Crucially, the **arena** stays
per-pointer; both translucent entries register their recognizers in
the same arena. That is the difference from "two opaque entries":
one arena, two competitors, real arena resolution. T15 documents
this with an inline ASCII diagram.

### `cx.active_drag` ordering

The `AnyDrag` flow expects to fire from inside a raw `on_mouse_down`
listener. Because T15 places the arena dispatch **before** the
existing `dispatch_mouse_event` chain, a recognizer that calls
`cx.stop_propagation()` from inside `handle_event` could swallow the
`on_mouse_down` and leave `cx.active_drag = None`. Avoid this by:
- Pan recognizers do **not** call `stop_propagation` on `Move` events.
- The fluent builder `on_pan_start` documentation calls out this
  contract explicitly.

### Sanitizer hover-diff cost

The `Hover` phase synthesizes `Enter`/`Exit` by diffing the current
`HitTestResult` against the previous one. A naïve implementation
allocates two `HashSet`s. The provided implementation uses a
2-pointer scan over the two `SmallVec<[HitTestEntry; 8]>` lists
(both sorted by paint depth), giving O(n+m) time and zero
allocations. T22 `arena_tick` includes a hover-heavy variant to
catch regressions.

### Recognizer `SmallVec` overflow

`Interactivity.gesture_recognizers: SmallVec<[Box<dyn
GestureRecognizer>; 4]>` inlines four. If a consumer registers more
(rare in practice), the `SmallVec` heap-allocates. This is not a
correctness concern but does count against the zero-alloc budget on
the registration path (not the dispatch path). T22 does not include
this in its dispatch-hot-path budget.

### `From<PlatformInput>` lossiness

Some `PlatformInput` variants don't map to `PointerEvent` at all
(`KeyDown`, `KeyUp`, `ModifiersChanged`). Some map to
`PointerSignalEvent` instead (`ScrollWheel`, `Pinch`). Some are
1:N (`FileDrop` already synthesizes `MouseMove`/`MouseUp` in the
existing code; the new conversion does not duplicate that work).
The conversion is therefore `Option<…>` / `SmallVec<…>`-shaped, not
plain `From`. The `dispatch.rs` module documents the full mapping
table inline.

## Migration guide

This is a forward-looking guide for consumers of `flui-core`. Since
S07 is purely additive, the migration is opt-in: existing code keeps
working unchanged.

### Adopting `on_tap` in place of `on_click`

```rust
// Before: synthesized click via on_mouse_down + on_mouse_up.
div()
    .on_click(|_, _, cx| { /* … */ })
    .on_mouse_down(MouseButton::Left, |evt, _, _| { /* … */ })

// After: declarative tap recognizer (preferred for new code).
div()
    .on_tap(|details, window, cx| {
        // details: TapDetails { kind, global_position }
        // window: &mut Window, cx: &mut App — match the existing
        // raw on_mouse_* listener signature.
        cx.do_thing();
        window.refresh();
    })
```

`on_click` continues to fire in parallel; for a one-off migration,
adopt `on_tap` and drop `on_click` afterward.

### Adopting `on_pan_*` for gesture-driven dragging

```rust
// Before: imperative drag via on_mouse_down + cx.active_drag.
div()
    .on_mouse_down(MouseButton::Left, |evt, _, cx| {
        cx.active_drag = Some(AnyDrag {
            value: Arc::new(my_value),
            view: my_view.into(),
            cursor_offset: evt.position,
            cursor_style: None,
        });
    })

// After: declarative pan recognizer (gestures only — no drag preview).
div()
    .on_pan_start(|details, window, cx| { /* slop crossed */ })
    .on_pan_update(|details, window, cx| { /* update — has delta */ })
    .on_pan_end(|details, window, cx| { /* end — has velocity */ })
```

Note: the **AnyDrag preview rendering** is orthogonal. If you need
both a Pan recognizer **and** a draggable preview, register both:

```rust
div()
    .on_mouse_down(MouseButton::Left, |evt, _, cx| {
        cx.active_drag = Some(AnyDrag { /* … */ });
    })
    .on_pan_start(|details, window, cx| { /* report start to data model */ })
    .on_pan_update(|details, window, cx| { /* update model */ })
```

### Tuning thresholds

```rust
// Anywhere with `&mut Window`:
window.gesture_settings_mut().long_press_timeout = Duration::from_millis(800);
```

This is the **S14 seam** — when MediaQuery completeness lands, the
same field becomes routable via accessibility settings.

### Translucent overlays

```rust
div()
    .with_hit_test_behavior(HitTestBehavior::Translucent)
    .on_tap(|_, _window, _cx| { /* fires for taps under the overlay */ })
```

Without `with_hit_test_behavior`, `Interactivity` defaults to
`HitTestBehavior::Opaque` (matching today's behavior).

### Integrating physics (post-S11)

```rust
div().on_pan_end(|details, window, cx| {
    // Pre-S11: inertia is consumer's problem.
    // Post-S11: the SAME details.velocity drives a Spring or Friction sim:
    let sim = Friction::new(initial_position, details.velocity);
    window.start_animation(sim, cx);
})
```

S11 will land `Friction::new(Point<Pixels>, Velocity)` — same
`Velocity` payload. No churn for S07 consumers.

### Custom recognizer (advanced)

Implementing your own `GestureRecognizer` is supported but **niche**.
The trait surface is small (`name`, `add_pointer`, `handle_event`,
`sweep_accepted`, `rejected`, plus optional `semantic_actions` and
`on_focus_request`). The most common need (a recognizer with custom
slop) is met by parameterizing one of the five built-in recognizers
via `GestureSettings`.

Implementer's checklist:

- [ ] `name()` returns a `&'static str` (used in log kv).
- [ ] `add_pointer` initializes per-pointer state.
- [ ] `handle_event` is **pure** in the eager-accept case (no user
  callback fired); user callbacks fire from `sweep_accepted` /
  inside the `Accepted` branch of `handle_event`.
- [ ] Drop guard: store `Task` handles such that recognizer drop
  cancels in-flight work.
- [ ] `// A7-audit: …` comment on every interior-mutability site.

## Done criteria

This spec is complete when **all** of the following are true on a
clean checkout of the landing PR series (Phase A through F):

1. `crates/flui-core/src/gesture/` exists with the module layout
   from [Design § Module layout](#module-layout). Every module
   compiles with no `unimplemented!()` / `todo!()` / `unreachable!()`
   sites (verified by `cargo run -p lock-checks -- check-stubs`
   producing zero diff).

2. The explicit per-symbol `pub use gesture::{ … }` block is added
   to [`crates/flui-core/src/lib.rs`](../../../crates/flui-core/src/lib.rs)
   alongside the existing per-symbol blocks. **No** glob
   `pub use gesture::*;` is introduced.

3. `Window::hit_test(position) -> HitTestResult`,
   `Window::gesture_binding()`, `Window::gesture_binding_mut()`, and
   `Window::gesture_settings_mut()` exist and are documented with
   rustdoc.

4. `Interactivity` carries `gesture_recognizers` and
   `hit_test_behavior` fields (both `pub(crate)`). `InteractiveElement`
   exposes `with_hit_test_behavior` plus all `on_X_*` builders listed
   in [Design § Interactivity fluent builders](#interactivity-fluent-builders).

5. All five recognizers (`Tap`, `DoubleTap`, `LongPress`, `Drag`-family,
   `Scale`) exist, with their `Details` types, and are unit-tested
   (T17 file).

6. T16 (arena lifecycle) and T23 (proptest) both pass `cargo test
   --workspace`. T22 bench passes its three sub-budgets in `--release`
   profile on M2-class hardware (CI thresholds 2× scaled). T18 demo
   passes `--headless-smoke`.

7. `cargo doc -p flui-core --no-deps` produces non-empty rustdoc for
   every new public item. The `#![warn(missing_docs)]` lint is
   green.

8. `cargo clippy --workspace --all-targets -- -D warnings` is green.

9. `cargo run -p lock-checks -- check-stubs` and
   `cargo run -p lock-checks -- check-platform-imports` produce
   zero-diff results against committed baselines.

10. [`docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`](2026-04-13-flui-core-roadmap.md)
    has the **S07 GestureArena** Phase II row marked `[x]`, and an
    entry has been appended to the Completed table with today's date
    (T19).

11. [`.ai-factory/DESCRIPTION.md`](../../../.ai-factory/DESCRIPTION.md)
    "Core Features" section has a bullet for the gesture arena
    surface (T19).

12. Existing tests in
    [`crates/flui-core/src/elements/div.rs`](../../../crates/flui-core/src/elements/div.rs)
    and
    [`crates/flui-core/src/interactive.rs`](../../../crates/flui-core/src/interactive.rs)
    pass with **identical output** at every commit checkpoint
    (mechanical backward compatibility — no firing-order regressions).

13. Architectural review by `flui-arch-reviewer` and
    `rust-api-migration-auditor` (T2) is recorded in this spec's
    [Open questions](#open-questions) section (resolutions inline)
    before any code lands. T1 commits the design doc; T2 commits the
    review resolution edits.

14. Roadmap Gap **B** ("GestureArena with competing recognizers —
    medium") in
    [§2 of the roadmap](2026-04-13-flui-core-roadmap.md) is marked
    `done`.

## Test log

To be filled in during implementation. Expected entries (per Phase):

- **Phase A:** `flui-arch-reviewer` review timing + summary;
  `rust-api-migration-auditor` review timing + summary.
- **Phase B:** `cargo build --workspace` runtime delta (Linux /
  macOS / Windows). `cargo test --workspace` baseline diff (must be
  zero — Phase B adds no recognizers, only pointer/hit-test
  plumbing).
- **Phase C:** `cargo test --workspace` for any T7/T8/T21/T9 unit
  tests added inline (full per-module unit tests come in T16).
- **Phase D:** Per-recognizer test counts and their per-test wall
  clocks.
- **Phase E:** Final dispatch-flow smoke test on the existing
  `examples/learn/interactive_elements.rs` (must produce identical
  visual output pre- and post-T15).
- **Phase F:** T22 sub-bench numbers on macOS aarch64 (M-class) +
  Linux x86_64 (CI runner) + Windows x86_64 (CI runner). T23
  proptest seed coverage. T18 `--headless-smoke` runtime.

## Follow-ups after S07 lands

1. **S08** (Semantics protocol) populates
   `GestureRecognizer::semantic_actions()` overrides for Tap,
   DoubleTap, LongPress; adds `Increment`, `Decrement`, `Move`
   variants to `SemanticAction` (non-breaking via
   `#[non_exhaustive]`).

2. **S11** (Physics simulations) consumes `Velocity` from
   `on_pan_end` / `on_horizontal_drag_end` /
   `on_vertical_drag_end` / `on_scale_end` to drive `Spring`,
   `Friction`, `Gravity`, `ScrollPhysics`.

3. **S12** (Focus traversal) reads
   `GestureRecognizer::on_focus_request()` on accept and routes
   the returned `FocusHandle` through `FocusTraversalPolicy`.

4. **S14** (MediaQuery completeness) wires
   `MediaQueryData::gesture_settings` to
   `window.gesture_settings_mut()`.

5. **A4** (Tracing standardization), when it lands, swaps every
   `log::trace!` / `log::debug!` / `log::warn!` site in
   `crates/flui-core/src/gesture/**` to the equivalent
   `tracing::*` macro. The kv-field schema documented in
   [Architectural decisions § Logging](#log-vs-tracing) trivially
   becomes span fields.

6. **T4** (Criterion benchmark suite), when it lands, ports T22's
   three sub-benches (`hit_test_8deep`, `arena_tick`,
   `full_frame_120hz`) to Criterion's `Bencher` API. Thresholds
   become Criterion's regression-detection baselines.

7. **A9** (Crate-boundary review for `flui-core`) considers
   `crates/flui-core/src/gesture/**` for extraction into a
   `crates/flui-gesture/` sibling once **S02b** unfreezes. The
   module is self-contained except for `Modifiers` / `Pixels` /
   `Point` (re-exportable from `flui-core`).

8. **S20** (Desktop platform-gaps cleanup) closes the stylus
   tilt/orientation/azimuth gap and the Windows native-pinch gap
   listed in [Explicit gaps](#explicit-gaps).
