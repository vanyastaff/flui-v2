# ADR-005: GPU device-loss — recovery contract and known gaps

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/platform/wgpu/**`, `flui-core/src/platform/windows/{platform,events,directx_renderer,directx_devices,directx_atlas,direct_write}.rs`,
`flui-core/src/platform/{linux/x11,linux/wayland}/window.rs`.
**Drivers:**
[zed-industries/zed#23288](https://github.com/zed-industries/zed/issues/23288),
[zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469),
[zed-industries/zed#52085](https://github.com/zed-industries/zed/issues/52085),
[flutter/flutter#111151](https://github.com/flutter/flutter/issues/111151).

## Context

GPU device loss is the most user-visible failure mode of a GPU-backed UI
framework: a driver update, a suspend/resume cycle, a thermal event, or a
malfunctioning extension causes the GPU to revoke the device handle. Without
graceful recovery, the app either crashes (best case — restartable) or
silently hangs (worst case — user state is gone).

This ADR is unusual among the others in this folder: flui-v2 **already has**
most of the recovery infrastructure (inherited from upstream). The job of
ADR-005 is to write down what we have, what guarantees it offers, what it
does not handle, and what an upcoming caller is allowed to assume.

The four upstream issues bundled under this ADR are not the same event:

- **#23288** — feature request for graceful recovery from driver crashes,
  suspend/resume, thermal events. The "umbrella" issue.
- **#30469** — Linux Wayland: turning the monitor off and back on closes
  Zed. Symptom of **output disconnect**, not device loss.
- **#52085** — Windows: updating the NVIDIA driver crashes Zed. Pure
  device-loss case.
- **flutter/flutter#111151** — Flutter engine analogue of #23288.

The dissolution into separate concerns is important; recovery from device
loss does not by itself fix monitor disconnect, and conflating them was part
of why upstream issues have stayed open so long.

## Current behaviour (verified)

References below cite the commit this ADR is written against.

### wgpu path (macOS, Linux, transitively Web)

**Detection.** [`platform/wgpu/wgpu_context.rs:52`](../../../crates/flui-core/src/platform/wgpu/wgpu_context.rs#L52):

```rust
let device_lost = Arc::new(AtomicBool::new(false));
device.set_device_lost_callback({
    let device_lost = Arc::clone(&device_lost);
    move |reason, _msg| {
        if reason != wgpu::DeviceLostReason::Destroyed {
            device_lost.store(true, Ordering::Relaxed);
        }
    }
});
```

Filters out `Destroyed` (normal teardown). All other reasons — `Unknown`,
`ReplacedAdapter`, etc. — set the flag. The flag is shared with every
renderer via [`wgpu_renderer.rs:140`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L140).

**Surface-error handling.** [`platform/wgpu/wgpu_renderer.rs:1068`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1068):

```rust
Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
    let surface_config = self.surface_config.clone();
    let resources = self.resources_mut();
    resources.surface.configure(&resources.device, &surface_config);
    return;
}
Err(wgpu::SurfaceError::Timeout) => return,
Err(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other) => {
    *self.last_error.lock().unwrap() = Some("Surface texture error".to_string());
    return;
}
```

`Lost`/`Outdated` triggers a surface reconfigure (which is what wgpu's docs
recommend). `OutOfMemory`/`Other` records a message and returns — **does
not currently trigger full recovery.** This is a known gap.

**Recovery.** [`platform/wgpu/wgpu_renderer.rs:1639`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1639):
`Renderer::recover(&window)` is a 60+ line routine that drops all GPU
resources, sleeps 350 ms ("copied from windows :shrug:" per the
in-code comment), rebuilds `WgpuContext`, recreates the surface, and
hands the renderer's atlas the new device/queue via
[`wgpu_atlas.rs:71`](../../../crates/flui-core/src/platform/wgpu/wgpu_atlas.rs#L71).
Coordinated across multiple windows: the first window to call it rebuilds
the shared context, subsequent windows adopt it.

**Platform glue.**

- [`platform/linux/x11/window.rs:1608`](../../../crates/flui-core/src/platform/linux/x11/window.rs#L1608)
  polls `renderer.device_lost()` from the event loop.
- [`platform/linux/wayland/window.rs:1341`](../../../crates/flui-core/src/platform/linux/wayland/window.rs#L1341)
  does the same on Wayland.

### DirectX path (Windows)

Independent code path because Windows uses Direct3D 11 via `windows-rs`, not
wgpu.

- [`platform/windows/directx_devices.rs:24`](../../../crates/flui-core/src/platform/windows/directx_devices.rs#L24)
  defines `try_to_recover_from_device_lost`, a retry/backoff helper.
- [`platform/windows/platform.rs:318`](../../../crates/flui-core/src/platform/windows/platform.rs#L318)
  checks `check_device_lost(directx_device)` on each frame attempt and calls
  `handle_gpu_device_lost` on a positive result.
- [`platform/windows/directx_renderer.rs:227`](../../../crates/flui-core/src/platform/windows/directx_renderer.rs#L227)
  is the per-renderer recovery (`handle_device_lost`), which delegates to
  the same `try_to_recover_from_device_lost` helper.
- [`platform/windows/direct_write.rs:1254`](../../../crates/flui-core/src/platform/windows/direct_write.rs#L1254)
  uses the same helper when text rasterization fails because of a lost
  device.
- [`platform/windows/directx_atlas.rs:58`](../../../crates/flui-core/src/platform/windows/directx_atlas.rs#L58)
  exposes `handle_device_lost(device, device_context)` to rebuild atlas
  textures on the new device.
- [`platform/windows/events.rs:1128`](../../../crates/flui-core/src/platform/windows/events.rs#L1128)
  handles a custom `WM_GPUI_GPU_DEVICE_LOST` window message routed by the
  platform thread.

### What is **not** handled today

1. **`SurfaceError::OutOfMemory` and `Other` do not trigger recovery.** The
   wgpu path records a message and returns. The frame is dropped silently.
   Whether the device is actually lost is not re-checked.
2. **The 350 ms post-loss sleep is a magic number** copied from the Windows
   path. There is no documented justification and no test that it is
   sufficient on a slower system / under load.
3. **Output disconnect (#30469) is not handled.** Wayland's
   `wl_output::done` removal does not trigger renderer recovery in our
   code; we depend on the compositor to fire a configure event with the
   new size, which only happens when output remains attached.
4. **No state-preservation contract.** Recovery rebuilds the renderer and
   the atlas, but **what happens to user state (Entities, view state, text
   selection, scroll positions) after recovery** is not written down. By
   inspection: state survives because it lives in `App`/`Window`, not in
   `Renderer`. This needs to be a contract, not an accident.
5. **No backpressure on recovery failure.** If `recover()` fails — surface
   creation fails, instance creation fails, sleep is interrupted — the
   error bubbles to the caller, but there is no documented retry strategy
   and no fallback (software renderer, headless mode, exit with code).
6. **No platform-level diagnostic** other than `log::warn!`. End users
   facing repeated device losses get no actionable surface.

## Findings vs upstream issues

| Issue | Symptom | Coverage in flui-v2 today |
|-------|---------|---------------------------|
| [zed-industries/zed#23288](https://github.com/zed-industries/zed/issues/23288) | App becomes unresponsive on GPU device loss; needs graceful recovery. | **partial — large parts exist**. Detection, surface reconfigure, full context recovery, multi-window coordination, and per-platform glue are present. Gaps 1–6 above remain. |
| [zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469) | Wayland: turning the monitor off closes the app. | **not addressed** — this is output disconnect, not device loss. Tracked as out-of-scope below. |
| [zed-industries/zed#52085](https://github.com/zed-industries/zed/issues/52085) | Windows: NVIDIA driver update crashes Zed. | **mostly covered** by the DirectX `try_to_recover_from_device_lost` path. Verify by running the driver-update scenario once gap 2 (sleep timing) is removed. |
| [flutter/flutter#111151](https://github.com/flutter/flutter/issues/111151) | Engine should gracefully handle GPU device loss. | Same theme; this ADR is the flui-v2-side reply. |

## Decision (contract)

1. **`Renderer::device_lost()` is the single source of truth for "is the
   GPU currently usable from this renderer".** The platform glue must
   consult it once per frame, before issuing draw calls.

2. **Recovery is a `Renderer::recover(&window)` call.** Callers must not
   try to rebuild surfaces, devices, queues, or atlases by hand. New
   renderer surfaces created during recovery may be opaque or transparent
   depending on the surface config; the contract is to match the previous
   surface, not to renegotiate.

3. **User state survives device loss.** `App`, `Window`, `Entity` values,
   focus, scroll positions, text selection, and animation state are
   preserved across `recover()`. Anything stored on `Renderer` or its
   atlas is rebuildable from those.

4. **Recovery is best-effort idempotent across windows.** The first window
   to call `recover()` rebuilds the shared `WgpuContext`; subsequent windows
   in the same loss event adopt the new context. The flag-store/load is
   `SeqCst` so concurrent observation is well-defined.

5. **`Lost`/`Outdated` is a normal frame; `OutOfMemory`/`Other` is not.**
   Today the wgpu path treats `OutOfMemory` and `Other` as a logged
   no-op; ADR-005 declares that these branches must trigger recovery (or
   shutdown) in the future — they may signal device loss as well as
   transient resource exhaustion, and the current behaviour silently
   loses frames.

6. **Output disconnect is a separate ADR.** Today, neither wgpu nor
   DirectX recovery handles monitor removal. Recovery on a missing output
   would be incoherent; that path needs its own design.

## Consequences

- Code that owns a `Renderer` is allowed to treat `device_lost() == true`
  as a hint to schedule recovery on the platform thread. It is not
  allowed to call `recover()` from inside a paint pass.
- A future fallback renderer (software / headless) attaches at the
  `recover()` boundary, not as a per-frame fallback.
- Tests that touch device loss must drive `set_device_lost_callback`
  explicitly — the contract above is what is tested, not the underlying
  callback wiring.
- The "350 ms sleep" remains in code with a documented `// MAGIC:`
  comment until replaced; the ADR makes its lack of justification
  explicit.

## Out of scope (separate ADRs)

- **Monitor / output disconnect** (GPUI #30469). Needs a Wayland
  `wl_output` lifecycle ADR; in flui-v2 also affects the bookkeeping in
  `AppContext::displays`, which is GPUI #46378.
- **Software renderer fallback.** Listed as gap 5; deserves its own ADR
  with a feature-flag and capability matrix.
- **Diagnostic UX on repeated loss.** User-facing messaging is a UX/
  product decision distinct from the contract.
- **Headless / offscreen renderer** for testing under simulated device
  loss. Cross-references the wgpu-gpu-reviewer agent's territory.

## Action items (tracked; no code lands with this ADR)

1. Add a comment block at the top of
   [`platform/wgpu/wgpu_renderer.rs`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs)
   pointing to this ADR and naming the contract.
2. Replace the `OutOfMemory | Other` no-op at
   [`wgpu_renderer.rs:1079`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1079)
   with a path that sets `device_lost = true` (driving the existing
   `recover()` flow) and logs the kind of error distinctly.
3. Replace the literal `350` with a named constant
   `POST_DEVICE_LOSS_STABILIZATION_DELAY` in
   [`wgpu_renderer.rs:1666`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1666),
   and add a `// MAGIC:` comment explaining its origin until the value is
   either justified empirically or replaced with a probe loop.
4. Write a `flui-core` integration-style test that drives device loss
   through `set_device_lost_callback`, calls `recover()`, and asserts the
   `App`/`Window` state survives unchanged.
5. Open a separate ADR for output-disconnect (#30469 and #46378 sibling).

## References

### Upstream issues
- [zed-industries/zed#23288](https://github.com/zed-industries/zed/issues/23288) — umbrella device-loss recovery proposal.
- [zed-industries/zed#52085](https://github.com/zed-industries/zed/issues/52085) — Windows driver update crash.
- [zed-industries/zed#30469](https://github.com/zed-industries/zed/issues/30469) — Wayland monitor off (referenced for disambiguation).
- [flutter/flutter#111151](https://github.com/flutter/flutter/issues/111151) — Flutter engine analogue.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-002-hover-active-invalidation.md](ADR-002-hover-active-invalidation.md)
- [docs/research/adr/ADR-003-color-alpha-pipeline.md](ADR-003-color-alpha-pipeline.md)
- [docs/research/adr/ADR-004-text-slicing-utf8-safety.md](ADR-004-text-slicing-utf8-safety.md)
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #7 (_Resilience: GPU device-loss_).
