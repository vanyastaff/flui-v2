# ADR-008: Window chrome — `WindowOptions` invariants and drag-region semantics

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/platform.rs` (`WindowOptions`), per-platform window
creation paths (`platform/{windows,mac,linux/{x11,wayland}}/window.rs`),
custom-title-bar drag region computation.
**Drivers:**
[zed-industries/zed#52067](https://github.com/zed-industries/zed/issues/52067),
[zed-industries/zed#27500](https://github.com/zed-industries/zed/issues/27500).

## Context

Two upstream issues encode the same gap from opposite ends of the window
"chrome" surface:

- **#52067** — setting `is_minimizable: false` *greys out* the minimize
  button but the user can still minimize the window via the system menu
  (Alt+Space → Minimize on Windows, equivalent on macOS).
- **#27500** — on macOS, clicking-and-dragging on a button that sits inside
  the title bar drags the whole window, because the title-bar's drag region
  is computed by area instead of by hit-tree.

Both expose the same root failure mode: **the engine treats a chrome
constraint as a hint to the OS, not as an invariant on its own
event-handling**. The OS then either lets the user bypass the constraint
or routes a parent gesture through a child element.

flui-v2 inherits the same shape: `is_minimizable: bool` is plumbed through
to `WS_MINIMIZEBOX` on Windows and equivalents on macOS/Linux, but no
flui-side guard rejects an `SC_MINIMIZE` `WM_SYSCOMMAND` if the user
manually invokes it.

## Current behaviour (verified)

References cite the commit this ADR is written against.

### `WindowOptions` surface

[`crates/flui-core/src/platform.rs:1392`](../../../crates/flui-core/src/platform.rs#L1392):

```rust
pub struct WindowOptions {
    // ...
    pub is_movable: bool,
    pub is_resizable: bool,
    pub is_minimizable: bool,
    // ...
}
```

Defaults at [`platform.rs:1537`](../../../crates/flui-core/src/platform.rs#L1537):
all three are `true`.

### Windows path

[`crates/flui-core/src/platform/windows/window.rs:431`](../../../crates/flui-core/src/platform/windows/window.rs#L431):

```rust
if params.is_resizable {
    dwstyle |= WS_THICKFRAME | WS_MAXIMIZEBOX;
}
if params.is_minimizable {
    dwstyle |= WS_MINIMIZEBOX;
}
```

These styles change the **appearance** of the title-bar buttons (grey them
out when the flag is `false`) but do not block the system menu. There is
no `WM_SYSCOMMAND` interception for `SC_MINIMIZE` / `SC_MAXIMIZE` / `SC_MOVE`
gated on these flags. The keyboard shortcut path (Win+Down, Win+Up, Alt+Space)
goes through `WM_SYSCOMMAND` and is therefore unaffected.

### macOS path

Searching for `is_minimizable` in `crates/flui-core/src/platform/mac/window.rs`
shows the value is read at window creation and translated into `NSWindow`
styleMask flags, with no later filtering of the menu/key path.

### Linux paths

X11 and Wayland accept the flags as hints to the compositor / WM (XdG
`xdg_toplevel.set_*_callback`) but enforcement depends on the WM.

### Drag-region

Custom title bars route mouse events to a custom drag region. There is
no engine-side hook that *excludes* hit-tested children from the drag
region — every pointer-down inside the title bar bounds is treated as a
window-move gesture, which is exactly the GPUI #27500 path.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#52067](https://github.com/zed-industries/zed/issues/52067) | `is_minimizable: false` greys out the button but minimization via system menu still works. | **likely yes**. Windows path only sets `WS_MINIMIZEBOX`; no `WM_SYSCOMMAND` filter exists. Same shape on macOS/Linux (the flag is a hint, no second-line enforcement). |
| [zed-industries/zed#27500](https://github.com/zed-industries/zed/issues/27500) | Clicking-and-dragging on a button inside the title bar drags the window. | **likely yes**. Drag-region currently treats the title-bar area as a single hit zone; no per-child hit-tree consultation before the drag intent is consumed. |

## Decision (contract)

1. **`WindowOptions` flags are invariants enforced by flui-core, not just
   hints to the platform.** When `is_minimizable: false` is set, no code
   path inside or outside flui-core may end with the window minimized.
   The engine must filter system menu commands, keyboard shortcuts, and
   programmatic API calls equally.

2. **Per-platform implementation must include a "second line".** Setting
   the platform-level flag (greyed-out button, missing menu entry) is
   the *first* line. Intercepting the system-menu / keyboard /
   programmatic invocation is the *second*. Both are required.

3. **The same applies to `is_resizable` and `is_movable`.** A
   non-resizable window must reject resize gestures from any source
   (keyboard shortcut, snap layouts, programmatic `set_bounds` that
   changes size). A non-movable window must reject drag-to-move from
   any source.

4. **Drag-region is computed *after* the per-child hit-test, not
   instead of it.** A pointer-down within the title bar's bounds that
   lands on a child element with a `mouse_down` listener delivers the
   event to the child first; window-move is the fall-through for the
   bare title-bar surface. Programmatic opt-in (e.g. a child explicitly
   declaring `.window_drag()`) re-enables the move gesture on that
   child.

5. **`WindowOptions::default()` keeps everything `true`.** The contract
   only fires when a caller explicitly opts out. Default behaviour is
   unchanged.

6. **Programmatic API (`Window::minimize`, etc.) is also gated.** A
   future `minimize()` method on `Window` must respect `is_minimizable`
   and return an `Err` (or no-op with a `log::warn!`) if the option is
   `false`. The constraint does not have a back door.

## Consequences

- Apps that set `is_minimizable: false` actually get a non-minimizable
  window, matching every other modern toolkit.
- Custom title bars produce native-feeling button clicks; click-drag
  on a title-bar button no longer accidentally drags the window.
- The cost is one filter per option per platform — small but
  unavoidable. Test coverage is the bottleneck because the keyboard
  shortcut path is platform-specific.

## Out of scope (separate ADRs)

- **Custom window decorations entirely** (drawing the close/min/max
  buttons ourselves). Touched but not specified here.
- **Programmatic `Window::minimize` / `Window::maximize` /
  `Window::move_to`** API surface. Mentioned in decision point 6
  but the full API design is a separate ADR.
- **Snap-layouts / aero-snap** on Windows 11 / equivalents on macOS
  Stage Manager. The contract here implies they must respect the
  flags; how they are intercepted is platform glue.

## Action items (tracked; no code lands with this ADR)

1. Audit Windows `WM_SYSCOMMAND` handling in
   [`platform/windows/events.rs`](../../../crates/flui-core/src/platform/windows/events.rs)
   and add an interception that filters `SC_MINIMIZE` / `SC_MAXIMIZE` /
   `SC_MOVE` / `SC_SIZE` against `WindowOptions`. Default reject is
   `MA_NOACTIVATE` (i.e. ignore the message).
2. Audit macOS minimize / zoom / titlebar drag paths in
   [`platform/mac/window.rs`](../../../crates/flui-core/src/platform/mac/window.rs)
   and gate them on the flags.
3. Add hit-tree-aware drag-region in the title-bar element so
   `mouse_down`-bearing children win the gesture.
4. Add a test in the test platform that creates a window with
   `is_minimizable: false` and verifies a synthetic system-menu invocation
   does not minimize it.

## References

### Upstream issues
- [zed-industries/zed#52067](https://github.com/zed-industries/zed/issues/52067) — `is_minimizable: false` not enforced.
- [zed-industries/zed#27500](https://github.com/zed-industries/zed/issues/27500) — title-bar buttons drag the window.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md)
- [docs/research/adr/ADR-007-display-lifecycle.md](ADR-007-display-lifecycle.md) — sibling on the "external state about the window" axis.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #4 (_Window / display lifecycle_), continued.
