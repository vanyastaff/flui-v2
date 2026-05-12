# ADR-007: Display lifecycle — `displays()`, DPI changes, output disconnect

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/app.rs` (`App::displays`, `primary_display`),
`flui-core/src/platform.rs` (`Platform::displays`, `PlatformDisplay`),
`flui-core/src/window.rs` (`Window::scale_factor`, `Window::bounds_changed`,
`Window::display_id`), Linux Wayland/X11 platform glue.
**Drivers:**
[zed-industries/zed#46378](https://github.com/zed-industries/zed/issues/46378),
[zed-industries/zed#21851](https://github.com/zed-industries/zed/issues/21851),
[zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469).
**Sibling of:** [ADR-005 — GPU device-loss](ADR-005-gpu-device-loss.md) (output
disconnect is the non-GPU half of the same "external state changed" family).

## Context

Three upstream issues describe the same underlying gap from three angles:

- **#46378** — calling `AppContext::displays()` before the first window is
  created returns an empty list on Wayland, so layer-shell windows that need
  to pick an output at creation time cannot.
- **#21851** — moving a window between X11 outputs does not update its DPI
  scale.
- **#30469** — turning a monitor off on Wayland kills the app.

All three live on the lifecycle of `PlatformDisplay` and its connection to
`Window`. None of them is GPU-related (that is ADR-005). The model
question is **how long is a display "valid" from the caller's perspective**,
and **what events does the caller get when it stops being valid**.

flui-v2 inherited the GPUI implementation of the read side (`displays()`,
`scale_factor`) but no observer side, which is exactly the shape that
produced the upstream bugs.

## Current behaviour (verified)

References below cite the commit this ADR is written against.

### Read side — present

[`crates/flui-core/src/platform.rs:246`](../../../crates/flui-core/src/platform.rs#L246):

```rust
fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;
```

[`crates/flui-core/src/app.rs:1144`](../../../crates/flui-core/src/app.rs#L1144):

```rust
pub fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
    self.platform.displays()
}
```

Each call dispatches straight to the platform. There is no caching, no
copy-on-write, no version counter.

[`crates/flui-core/src/window.rs:2140`](../../../crates/flui-core/src/window.rs#L2140):

```rust
pub fn scale_factor(&self) -> f32 { self.scale_factor }
```

Stored on `Window`; refreshed by `bounds_changed`.

[`crates/flui-core/src/window.rs:1990`](../../../crates/flui-core/src/window.rs#L1990):

```rust
pub fn bounds_changed(&mut self, cx: &mut App) {
    self.scale_factor  = self.platform_window.scale_factor();
    self.viewport_size = self.platform_window.content_size();
    self.display_id    = self.platform_window.display().map(|d| d.id());
    self.refresh();
    self.bounds_observers
        .clone()
        .retain(&(), |callback| callback(self, cx));
}
```

`bounds_changed` is the **only** path that refreshes `scale_factor` and
`display_id`. It runs when the platform sends a resize/configure callback.
Whether the platform sends one when the window crosses an output boundary
without changing size is platform-defined — and varies.

### Observer side — absent

A grep for `displays_observers`, `on_displays_changed`, `observe_displays`,
`on_display_change` returns no matches. There is currently **no API for
code to subscribe to "the set of displays changed"** — no insertion event,
no removal event, no per-display scale-factor change event.

`bounds_observers` exists on `Window`, but it fires on the same callback
that refreshes `scale_factor`; it does not differentiate "I moved" from
"the monitor I sit on changed DPI" from "the monitor I sit on went away".

### Wayland output handling (verified)

[`crates/flui-core/src/platform/linux/wayland/client.rs`](../../../crates/flui-core/src/platform/linux/wayland/client.rs)
binds `wl_output`/`wl_registry` proxies. The handler processes `wl_output`
geometry and mode events, but a grep for `output_removed` / `global_remove`
on the same path indicates output removal is observed at registry level
and *not* propagated as a `PlatformDisplay` removal event to higher code.

### X11 output handling

[`crates/flui-core/src/platform/linux/x11/window.rs`](../../../crates/flui-core/src/platform/linux/x11/window.rs)
uses XRandR; output cross-over without resize is the GPUI #21851 path.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#46378](https://github.com/zed-industries/zed/issues/46378) | Wayland: `displays()` returns empty until a window is created. | **likely yes**. `App::displays` is a pass-through to the platform; the Wayland platform binds `wl_output` only after the event loop runs. The exact pre-window timing has not been tested under our shell setup, but no caching or one-shot wait exists in our code. |
| [zed-industries/zed#21851](https://github.com/zed-industries/zed/issues/21851) | X11: DPI scale stays stale when a window moves between outputs without a resize. | **partial**. `scale_factor` is only refreshed in `bounds_changed`. If the X11 platform emits a configure for the cross-over (winit-style), we update; if it does not (raw XRandR), we miss it. We do not have a separate display-change hook. |
| [zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469) | Wayland: monitor off → app exits. | **unknown but plausible**. The compositor stops sending frame events on a missing output; the renderer's `device_lost` path (ADR-005) does not fire because the device is fine. The platform code path that handles `wl_output` removal is not threaded into any higher-level observer; what exactly happens depends on the surface keeping a ref to a destroyed output. |

The pattern is consistent: **read works, observe does not**. Every
upstream issue in this ADR's scope falls out of the missing observer
side.

## Decision (contract)

1. **`App::displays()` is a snapshot, not a subscription.** Callers must
   not rely on the result staying valid; the snapshot is good for the
   current message-loop turn only.

2. **`PlatformDisplay::id` is stable across the lifetime of that display.**
   Different displays have different ids; the same display keeps its id
   while it remains connected. Reconnect after disconnect is a new id —
   identity is by connection, not by physical hardware.

3. **The platform layer must surface display add/remove and per-display
   scale-factor change.** Today there is no API for this. ADR-007
   declares the API shape but leaves implementation to a follow-up:

   - `Platform::on_displays_changed(callback: Box<dyn FnMut()>)` —
     coarse hook firing whenever the snapshot returned by `displays()`
     would differ from the previous snapshot.
   - `Window::observe_display_change(callback: Box<dyn FnMut(&Window,
     &mut App)>)` — fires when this window's bound display id changes
     **or** when that display's scale factor changes, distinct from
     `bounds_observers` which fires on size.

4. **`Window::scale_factor` is updated either through `bounds_changed`
   (today) or through the new per-display observer (future).** The
   eventually-consistent guarantee is: between any two consecutive
   frames, `scale_factor` reflects what the platform currently reports
   for the window's display.

5. **Output disconnect does not implicitly kill a window.** If a
   window's bound display goes away, the window is reattached to the
   `primary_display`, its bounds are reset to the new display, and a
   single `bounds_changed` is emitted. The user-visible window stays
   alive. This is the inverse of GPUI #30469.

6. **`displays()` must be callable before any window exists.** Today
   this is decided by the platform implementation; the contract says
   "if your platform needs an event loop tick to populate the output
   list, you must drive that tick on demand from `displays()` rather
   than return an empty list". The platform may block briefly to
   satisfy this. This closes GPUI #46378.

## Consequences

- The `Platform` trait grows two methods; existing platform
  implementations are not breaking because callers do not yet exist.
- Wayland and X11 platform glue gets a clear acceptance test: "remove
  an output → the still-connected window survives" and "add an output
  → `on_displays_changed` fires within the next event loop turn".
- Code that reads `scale_factor` may continue to do so via the cached
  `Window` field; the contract guarantees freshness per-frame, not
  per-instruction.
- A future _layer-shell-on-Wayland_ widget can rely on
  `displays()` being meaningful before window creation.

## Out of scope (separate ADRs)

- **GPU device-loss recovery.** Already covered by ADR-005. Output
  disconnect and device loss can coincide (compositor restart, driver
  crash); the recovery paths run sequentially, each handling its half.
- **Virtual displays / projection / overlay** (e.g. AirPlay, Miracast).
  These trigger `on_displays_changed` but their stability semantics
  differ; a separate ADR covers them.
- **HDR / wide-gamut display metadata.** Surface format choice and
  per-display colour-space negotiation; orthogonal.
- **Per-display refresh-rate hints** (VRR). Separate present-pipeline
  concern.

## Action items (tracked; no code lands with this ADR)

1. Add the two trait methods `Platform::on_displays_changed` and
   `Window::observe_display_change` with default implementations that
   never fire, so the contract is expressed in the type system before
   any backend wires them up.
2. Implement the Wayland binding for `wl_output` add/remove that drives
   `on_displays_changed`. Document the registry event mapping at the
   call site.
3. Implement the X11 XRandR notification path that drives
   `observe_display_change` independently of `bounds_changed`.
4. Add a `Window::observe_display_change` test in the test platform
   that simulates output removal and asserts the window survives
   (closes #30469's repro pattern under test).
5. Document the "blocking populate" requirement of `displays()` in a
   `// CONTRACT:` comment at [`platform.rs:246`](../../../crates/flui-core/src/platform.rs#L246).

## References

### Upstream issues
- [zed-industries/zed#46378](https://github.com/zed-industries/zed/issues/46378) — Wayland `displays()` empty pre-window.
- [zed-industries/zed#21851](https://github.com/zed-industries/zed/issues/21851) — X11 DPI scale stale on monitor cross-over.
- [zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469) — Wayland monitor off kills app.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md) — sibling for the non-GPU half of "external state changed".
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #4 (_Window / display lifecycle_), partial coverage by this ADR.
