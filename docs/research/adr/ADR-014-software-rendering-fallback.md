# ADR-014: Software rendering fallback — accept, reject, or expose?

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/platform/wgpu/wgpu_context.rs` adapter
selection, the platform glue that calls into it on Linux X11/Wayland
and Windows-via-wgpu, eventual main-loop pacing.
**Drivers:** [zed-industries/zed#45897](https://github.com/zed-industries/zed/issues/45897).

## Context

GPUI #45897 reports that on Linux systems where Vulkan is unavailable,
Zed still runs — through Mesa's `llvmpipe` software backend — but it
**uses all CPU cores at 100 % whenever the window has focus**, draining
a laptop battery in twenty minutes. The bug is not "software rendering
is unusable" (it is functionally fine), it is "software rendering plus
an event loop that paints every available frame burns the CPU".

flui-v2 inherits the same adapter request shape:

[`crates/flui-core/src/platform/wgpu/wgpu_context.rs:89`](../../../crates/flui-core/src/platform/wgpu/wgpu_context.rs#L89):

```rust
backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
flags: wgpu::InstanceFlags::default(),
```

and at [line 94](../../../crates/flui-core/src/platform/wgpu/wgpu_context.rs#L94):

```rust
let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
    // ...
    force_fallback_adapter: false,
}));
```

`force_fallback_adapter: false` means wgpu picks the best available
adapter and is allowed to fall through to a software backend if no
hardware adapter is found. Without a Vulkan driver, on a Linux box
with GL, wgpu happily accepts `llvmpipe` and we end up in the GPUI
#45897 scenario.

The decision is not "should software rendering work" — it should — it
is "what does the caller experience when it does, and how does the
renderer prevent the 100 % CPU symptom".

## Current behaviour (verified)

Adapter selection is uniform: VULKAN + GL backends, no fallback flag,
no software-detection branch. The renderer has no main-loop pacing
based on whether the adapter is hardware or software; whatever the
event loop decides to draw, it draws.

A grep for `is_fallback_adapter`, `AdapterInfo::driver`, or
`device_type == DeviceType::Cpu` in `flui-core/src/platform` returns no
matches. We do not currently classify the running adapter.

## Findings vs upstream

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#45897](https://github.com/zed-industries/zed/issues/45897) | No Vulkan → 100 % CPU on all cores while window is focused. | **likely yes**. We accept any adapter wgpu hands us, including llvmpipe; we do not throttle the event loop based on adapter type; we paint every frame regardless. The exact CPU number depends on `request_animation_frame` density, which is fine on a hardware GPU but burns on software. |

## Decision (contract)

1. **Software rendering is an accepted runtime mode.** Refusing to start
   when no hardware adapter exists is worse than degraded performance
   — accessibility / remote / headless / CI cases all depend on
   software rendering working.

2. **The adapter type is exposed.** `App::renderer_kind() ->
   RendererKind { Hardware, Software }`. Callers do not have to
   inspect it, but apps that want to disable expensive effects or
   skip animations on software can do so.

3. **The default frame budget tightens automatically on software.**
   The event loop targets a longer frame interval (target 30 fps
   instead of 60) when `renderer_kind() == Software`. This is the
   actual fix for the 100 % CPU symptom: we still paint, but the
   compositor and the CPU have idle time between frames.

4. **Animations check `renderer_kind()`.** A future
   `AnimationController` consults the renderer kind and **does not
   schedule per-frame ticks faster than the budget allows**.
   `request_animation_frame()` continues to work; the underlying
   pacing is the change. This composes with ADR-001.

5. **A diagnostic message is logged once at startup** when the
   adapter is software. Not on every frame; the contract is
   user-respectful telemetry, not a console flood.

6. **`force_fallback_adapter: false` stays.** We do not *prefer*
   software; we only *accept* it. The hardware path remains the
   primary.

## Consequences

- A user on a Mesa-only Linux box on battery still works — at
  ~30 fps, with quieter fans and a multi-hour battery.
- CI machines that run flui-v2 examples under llvmpipe get
  predictable frame timing.
- An app can pre-render heavy visuals on hardware and skip them on
  software via the `renderer_kind()` API.
- The 100 % CPU symptom of #45897 disappears for unmodified apps,
  not just for apps that opted into the fix.

## Out of scope (separate ADRs)

- **VRR / refresh-rate adaptation.** The frame budget here is a
  fallback floor, not a frame-rate negotiation system.
- **Headless mode** (no surface at all). Adjacent topic; the test
  platform already has it. A future ADR may unify headless and
  software-on-surface.
- **GPU device-loss** while running on software. The recovery path
  in ADR-005 applies the same way; the adapter classification
  re-runs on `recover()`.
- **Per-window renderer-kind override** (force-software for one
  view). Use case is unclear; deferred.

## Action items (tracked; no code lands with this ADR)

1. Add `RendererKind` to `flui-core/src/platform.rs` and expose
   `App::renderer_kind`. Populate it from
   `wgpu::Adapter::get_info().device_type == DeviceType::Cpu` (and
   the equivalent on DirectX / Metal).
2. Plumb the frame budget into the platform-specific event loop —
   on Linux X11/Wayland the timer interval, on Windows the
   composition-clock subscription. Default budget: 60 fps on
   hardware, 30 fps on software.
3. Add a one-shot `log::info!("Software renderer detected (…); frame budget reduced to 30 fps")`
   at startup when `renderer_kind() == Software`.
4. Add a manual smoke test on a Linux box without Vulkan (or with
   `WGPU_BACKEND=gl` forcing GL) — verify CPU usage drops below
   what the issue reports.

## References

### Upstream issues
- [zed-industries/zed#45897](https://github.com/zed-industries/zed/issues/45897) — 100 % CPU when Vulkan unavailable.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — `request_animation_frame` composes with the frame-budget rule.
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md) — recovery path re-classifies the adapter.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #1 (_Rendering / GPU pipeline_), continued.
