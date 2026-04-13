---
spec_id: S01b
title: lock-wgpu-headless-and-golden
phase: I
depends_on: [S01a.1]
blocks: [S02]
status: draft
date: 2026-04-13
---

# S01b — lock-wgpu-headless-and-golden

## Context

Second branch of the lock phase, in parallel with S01c and S01d. Provides
the golden-image regression suite that S02-S06 will rely on to detect any
rendering drift during the platform extraction.

The previous draft of this work proposed a new `WgpuRenderContext` type
duplicating the existing `WgpuContext` at `wgpu_context.rs:8`, which
GPU and architecture reviews flagged as a wrong direction. This spec
takes the corrected approach: **lift the per-renderer pipeline cache
into the existing `WgpuContext`** (or a sibling `WgpuPipelineCache`
companion type), and add a headless constructor path that skips the
surface-capabilities probe.

The spec also confronts the chicken-and-egg problem caught by adversarial
review: golden tests are introduced in the same step that touches GPU
code, so there is no pre-existing baseline to validate against. The fix is
to land the work as **two commits in one PR** — commit A lifts the
pipeline cache without changing surface behavior and captures the golden
references, commit B adds the headless variant and replays the goldens.
Any drift between A and B is a refactor bug, not a baseline bug.

## Goals

1. Add `WgpuContext::new_headless()` that creates a wgpu device + queue
   without a `wgpu::Surface`, so adapter selection can succeed without a
   window handle.
2. Lift the pipeline cache out of `WgpuRenderer::new_internal`'s per-instance
   `WgpuResources` and into either the existing `WgpuContext` or a new
   `WgpuPipelineCache` companion. Both the surface path
   (`WgpuRenderer`) and the offscreen path (`WgpuHeadlessRenderer`) share
   the same cache when running on the same device.
3. Implement `WgpuHeadlessRenderer` that impls
   `flui_core::PlatformHeadlessRenderer` (the trait already exists at
   `platform.rs:677`, gated on `test-support`).
4. Wire `current_headless_renderer()` on Linux/FreeBSD to return
   `Some(Box::new(WgpuHeadlessRenderer::new()))`.
5. Lock the offscreen render format to `Bgra8Unorm` to match the surface
   path's first preference, with B/R channel swap on readback. Lock
   `BlendState::PREMULTIPLIED_ALPHA_BLENDING` explicitly in pipeline
   creation rather than deriving from `CompositeAlphaMode`.
6. Implement the golden test harness as an extension of the existing
   `crates/flui-core/src/platform/visual_test.rs` infrastructure, not as
   a parallel system.
7. Capture an initial set of reference PNGs for a fixed scene set
   (quad, shadow+blur, linear gradient, radial gradient, path fill,
   monochrome sprite, polychrome sprite, single-line text, multi-line
   text, clip rect). Mac CI captures via `MetalHeadlessRenderer`; Linux
   CI captures via the new `WgpuHeadlessRenderer`. Per-platform; never
   cross-compared.
8. Add `mesa-vulkan-drivers`, `vulkan-tools` to the Linux CI install
   block, and set `VK_ICD_FILENAMES` + `WGPU_POWER_PREF=low` env vars in
   the test job to force lavapipe selection.

## Non-goals

- Not adding a Windows golden suite. DirectX has no headless renderer
  today (rewriting `DirectXRenderer` for offscreen is ~2-3 days of
  separate work). Windows golden is deferred to S05 prep, where it makes
  sense to add `DirectXHeadlessRenderer` immediately before migrating
  `windows/` to `flui-platform`.
- Not cross-comparing mac and Linux outputs. Different renderers,
  different rasterizers, different anti-aliasing — pixels will differ.
  Mac is locked against Metal output; Linux is locked against lavapipe
  Vulkan output.
- Not unifying the Mac and Linux harnesses into a single test fixture
  set. Each platform has its own reference PNGs in its own subdirectory.
- Not writing a runtime-compile WGSL shader change. The shader source
  files (`shaders.wgsl`, `shaders_subpixel.wgsl`) are untouched.
- Not promoting `PlatformHeadlessRenderer` out of `test-support`. It
  stays test-only for the same reasons it always was.
- Not creating a new `WgpuRenderContext` type. The design uses the
  existing `WgpuContext`.
- Not replacing the existing `platform/visual_test.rs`. Reuse it; extend
  it.
- Not pinning Mesa to a specific version. Ubuntu LTS point releases
  decide the Mesa version; goldens are regenerated when CI reports a
  Mesa rev.
- Not running golden tests in the `check` or `clippy` CI jobs. Only the
  `test` job runs them.
- Not committing the cbindgen `scene.h` snapshot — that's S01b's
  predecessor work via the `.gitattributes` rule from S01a.1, but the
  actual scene.h artifact is a separate concern in S04 prep.

## Current state

### Existing wgpu infrastructure

- [`crates/flui-core/src/platform/wgpu/wgpu_context.rs`](../../crates/flui-core/src/platform/wgpu/wgpu_context.rs)
  (385 LoC) — `WgpuContext` holds `instance`, `adapter`, `device`,
  `queue`, bind group layouts. The `new` constructor at
  `wgpu_context.rs:25-...` takes a `wgpu::Instance` and a `&wgpu::Surface`
  for the capability probe via `select_adapter_and_device` at
  `wgpu_context.rs:181-289`. **No headless path.**
- [`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs`](../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs)
  (1767 LoC) — `WgpuRenderer` owns `wgpu::Surface<'static>` inside a
  `WgpuResources` struct at line 100. Pipeline state is created in
  `new_internal` around lines 339-347 via `create_bind_group_layouts`
  and `create_pipelines` (~lines 540-851). Both helpers take `device`
  and `surface_format`.
- [`crates/flui-core/src/platform/wgpu/wgpu_atlas.rs`](../../crates/flui-core/src/platform/wgpu/wgpu_atlas.rs)
  — `WgpuAtlas` impls `PlatformAtlas`. Constructor takes
  `Arc<Device> + Arc<Queue>` only; not surface-coupled. Reusable as-is.
- `pub type GpuContext = Rc<RefCell<Option<WgpuContext>>>` at
  `wgpu_renderer.rs:97`. This alias is shared across windows for device
  recovery. Keep as-is; do not promote to crate-level pub.

### Existing Metal headless

- `MetalHeadlessRenderer` at
  [`platform/mac/metal_renderer.rs:1683-1709`](../../crates/flui-core/src/platform/mac/metal_renderer.rs#L1683-L1709)
  is the reference implementation for `PlatformHeadlessRenderer`. Mirror
  its ergonomics in the wgpu impl.

### Existing visual_test infrastructure

- [`crates/flui-core/src/platform/visual_test.rs`](../../crates/flui-core/src/platform/visual_test.rs)
  is gated `#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]`
  — currently mac-only. It wraps `TestPlatform` + `MetalHeadlessRenderer`
  for visual regression tests. S01b extends it to also work on Linux when
  `WgpuHeadlessRenderer` is available, and removes the mac-only cfg gate
  in favor of a per-impl gate.

### CI install gap

[`.github/workflows/ci.yml:24-40`](../../.github/workflows/ci.yml#L24-L40)
installs `libvulkan-dev` (Vulkan loader headers) but NOT `mesa-vulkan-drivers`
(the lavapipe ICD). Without the ICD, `vkEnumeratePhysicalDevices` returns
zero devices and `wgpu::Adapter::request_adapter` either returns `None`
or returns a no-op adapter that fails to create a device.

### Pre-existing blockers from earlier review

- `wgpu_renderer.rs:258-272` adapter selection prefers `Bgra8Unorm`
  (linear, non-sRGB) as the surface format. The offscreen path MUST use
  the same format for bit-exact pipeline cache. Earlier draft said
  `Rgba8UnormSrgb`; that's wrong and would gamma-shift every fragment.
- `RenderingParameters::new` at
  [`wgpu_renderer.rs:1732-1739`](../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1732-L1739)
  queries `adapter.get_texture_format_features(surface_format).flags.sample_count_supported(n)`
  — MSAA support is per-format. Same format → same `path_sample_count`.
- `ZED_FONTS_GAMMA`, `ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST`,
  `ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST` env vars are read at
  [`wgpu_renderer.rs:1741-1758`](../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1741-L1758)
  and baked into the uniform buffer every frame. Goldens that don't
  unset these will drift on any developer machine that exports them.

## Design

### Step 1 — `WgpuContext::new_headless()`

New constructor in `wgpu_context.rs` alongside the existing `new`. Takes
no surface argument:

```rust
impl WgpuContext {
    pub fn new_headless() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY | wgpu::Backends::SECONDARY,
            ..Default::default()
        });

        // No surface = no surface-capabilities probe. Pick highest-priority
        // adapter that successfully creates a device with the required
        // features and limits.
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .ok_or_else(|| anyhow::anyhow!("no wgpu adapter available for headless"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("flui headless device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))?;

        // Same bind group layouts as the surface path. These are
        // surface-format-independent; reuse the existing helper.
        let bind_group_layouts = build_bind_group_layouts(&device);

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            bind_group_layouts,
            // ... other fields identical to `WgpuContext::new`
        })
    }
}
```

The function body is ~80-100 LoC. It does NOT share a `WgpuContext`
instance with surface-mode renderers running in the same process —
each gets its own `WgpuContext` because adapter selection differs. The
shared object is the **pipeline cache** (Step 2), not the context.

### Step 2 — lift the pipeline cache

Pipeline creation currently lives inside `WgpuRenderer::new_internal`
at `wgpu_renderer.rs:339-347` and the helper `create_pipelines` at
`wgpu_renderer.rs:600-851`. Both are parameterized by `surface_format`
and `alpha_mode`.

**Refactor:** introduce `WgpuPipelineCache` as a sibling type next to
`WgpuContext`:

```rust
pub(crate) struct WgpuPipelineCache {
    pub quad_pipeline: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub path_rasterization_pipeline: wgpu::RenderPipeline,
    pub path_pipeline: wgpu::RenderPipeline,
    pub underline_pipeline: wgpu::RenderPipeline,
    pub mono_sprite_pipeline: wgpu::RenderPipeline,
    pub poly_sprite_pipeline: wgpu::RenderPipeline,
    pub surfaces_pipeline: wgpu::RenderPipeline,
    // any subpixel/dual-source pipelines, currently None per
    // wgpu_context.rs:121-131 — naga gfx-rs/wgpu#6402
}

impl WgpuPipelineCache {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        bind_group_layouts: &BindGroupLayouts,
    ) -> Self {
        // Body lifted from create_pipelines, parameterized by an
        // EXPLICIT BlendState (premultiplied alpha) instead of an
        // alpha_mode CompositeAlphaMode. This is the correctness-critical
        // change.
        Self {
            quad_pipeline: create_quad_pipeline(device, format, bind_group_layouts),
            // ...
        }
    }
}
```

**Critical**: the surface variant and the headless variant **must
construct the cache with the same `format` argument** — `Bgra8Unorm`. The
surface variant currently negotiates this from `surface_caps.formats` at
`wgpu_renderer.rs:258-272`; verify it always picks `Bgra8Unorm` on the
target hardware. If a fallback is ever taken (e.g. `Rgba8Unorm` because
`Bgra8Unorm` isn't supported), the surface variant is still allowed to
use that — but the **headless variant always uses `Bgra8Unorm`** because
it doesn't depend on surface caps.

This means: on hardware where surface picks something other than
`Bgra8Unorm`, the surface and headless caches are different. That is
acceptable because the goldens are captured in the variant they replay
in (Mac → Metal, Linux → lavapipe wgpu headless). They're never
compared cross-variant.

**`alpha_mode` removal:** the existing `create_pipelines` takes
`alpha_mode: wgpu::CompositeAlphaMode` and uses it at lines 627-632 to
select `PREMULTIPLIED_ALPHA_BLENDING` vs plain `ALPHA_BLENDING`. The
refactor makes the choice explicit: always
`PREMULTIPLIED_ALPHA_BLENDING`. The surface variant continues to honor
`CompositeAlphaMode` for the **swap chain composition step**, which is
unrelated to pipeline blend state.

### Step 3 — `WgpuHeadlessRenderer`

New file or extension of `wgpu_renderer.rs`:

```rust
#[cfg(any(test, feature = "test-support"))]
pub struct WgpuHeadlessRenderer {
    context: WgpuContext,
    pipelines: WgpuPipelineCache,
    atlas: Arc<WgpuAtlas>,
    instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
}

#[cfg(any(test, feature = "test-support"))]
impl WgpuHeadlessRenderer {
    pub fn new() -> anyhow::Result<Self> {
        let context = WgpuContext::new_headless()?;
        let pipelines = WgpuPipelineCache::new(
            &context.device,
            wgpu::TextureFormat::Bgra8Unorm,
            &context.bind_group_layouts,
        );
        let atlas = Arc::new(WgpuAtlas::new(
            context.device.clone(),
            context.queue.clone(),
        ));
        let instance_buffer_pool = Arc::new(Mutex::new(InstanceBufferPool::default()));
        Ok(Self { context, pipelines, atlas, instance_buffer_pool })
    }

    pub fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<RgbaImage> {
        let target = self.create_offscreen_target(size)?;
        let readback_buffer = self.create_readback_buffer(size)?;

        let mut encoder = self.context.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("flui headless encoder") },
        );

        // Render the scene to `target` using `self.pipelines`.
        // Body mirrors the existing draw() at wgpu_renderer.rs:1036-1271
        // but writes into `target` (a Texture) instead of acquiring a
        // SurfaceTexture.
        self.draw_into(&target, &mut encoder, scene)?;

        // Copy texture to readback buffer.
        let bytes_per_pixel = 4;
        let unpadded_bpr = (size.width.0 as u32) * bytes_per_pixel;
        let padded_bpr = align_up(unpadded_bpr, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bpr),
                    rows_per_image: Some(size.height.0 as u32),
                },
            },
            wgpu::Extent3d {
                width: size.width.0 as u32,
                height: size.height.0 as u32,
                depth_or_array_layers: 1,
            },
        );

        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Readback sync sequence — CRITICAL for correctness:
        // 1. submit (above)
        // 2. register the map callback (below)
        // 3. drive device.poll(Wait) on the same thread (below)
        // 4. block_on the channel
        let (tx, rx) = futures::channel::oneshot::channel();
        readback_buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.context.device.poll(wgpu::PollType::Wait)?;  // verify exact API name
        pollster::block_on(rx)??;

        // Read the mapped range, strip row-stride padding, swap B/R for
        // RGBA output.
        let view = readback_buffer.slice(..).get_mapped_range();
        let mut rgba = Vec::with_capacity((size.width.0 * size.height.0 * 4) as usize);
        for row in 0..(size.height.0 as usize) {
            let row_start = row * padded_bpr as usize;
            let row_end = row_start + unpadded_bpr as usize;
            for chunk in view[row_start..row_end].chunks_exact(4) {
                // BGRA → RGBA
                rgba.push(chunk[2]);
                rgba.push(chunk[1]);
                rgba.push(chunk[0]);
                rgba.push(chunk[3]);
            }
        }
        drop(view);
        readback_buffer.unmap();

        Ok(RgbaImage::from_raw(
            size.width.0 as u32,
            size.height.0 as u32,
            rgba,
        ).expect("rgba buffer size matches dimensions"))
    }

    fn create_offscreen_target(&self, size: Size<DevicePixels>) -> anyhow::Result<wgpu::Texture> {
        Ok(self.context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flui headless target"),
            size: wgpu::Extent3d {
                width: size.width.0 as u32,
                height: size.height.0 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }))
    }
}

#[cfg(any(test, feature = "test-support"))]
impl flui_core::PlatformHeadlessRenderer for WgpuHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<RgbaImage> {
        WgpuHeadlessRenderer::render_scene_to_image(self, scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn flui_core::PlatformAtlas> {
        self.atlas.clone()
    }
}
```

**Notes on the readback sync block above** — this is the most subtle
piece and the one the wgpu-gpu reviewer flagged hardest. The exact API
name (`device.poll(wgpu::PollType::Wait)` vs `Maintain::Wait`) depends on
wgpu 24's current shape; the implementer verifies via
`cargo doc --open wgpu` before writing the line. The pattern itself is
correct: register the callback BEFORE polling, poll with `Wait`
semantics, then block_on the channel.

### Step 4 — wire `current_headless_renderer()` for Linux

[`platform.rs:158-169`](../../crates/flui-core/src/platform.rs#L158-L169):

```rust
#[cfg(feature = "test-support")]
pub fn current_headless_renderer() -> Option<Box<dyn crate::PlatformHeadlessRenderer>> {
    #[cfg(target_os = "macos")]
    {
        Some(Box::new(mac::metal_renderer::MetalHeadlessRenderer::new()))
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        Some(Box::new(
            wgpu::WgpuHeadlessRenderer::new()
                .expect("failed to create headless wgpu renderer"),
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "freebsd")))]
    {
        None
    }
}
```

Windows still returns `None` (DirectX headless not in scope).

### Step 5 — extend `platform/visual_test.rs`

Currently mac+test-support gated. Refactor to:
- Per-impl gating (mac uses `MetalHeadlessRenderer`; linux uses
  `WgpuHeadlessRenderer`).
- Single `VisualTestPlatform::new()` that calls
  `current_headless_renderer()`.
- Single `render_to_image(scene)` helper that delegates.

This consolidates the harness into one place that both platforms share.

### Step 6 — golden test framework

New directory: `crates/flui-core/tests/golden/`. Layout:

```
tests/golden/
├── common/
│   ├── mod.rs           # harness: build scene fixtures, render, diff,
│   │                    # update on --bless flag
│   ├── fixtures.rs      # the 10 scene constructors (quad, shadow, ...)
│   └── env.rs           # ZED_FONTS_* env scrubbing
├── metal/
│   └── reference/       # *.png — captured on macOS CI
└── wgpu/
    └── reference/       # *.png — captured on Linux CI (lavapipe)
```

**Test entry points** (one per scene per platform):

- `tests/golden/metal_quad.rs` (cfg `target_os = "macos"`)
- `tests/golden/wgpu_quad.rs` (cfg `any(target_os = "linux", target_os = "freebsd")`)
- ... 10 scenes × 2 platforms = 20 test files (or fewer if
  parameterized with proptest)

**Tolerance:** bit-exact within a platform. Per-pixel comparison via the
`image` crate. If lavapipe driver rolls cause non-determinism in
practice, the spec accepts a one-time per-channel delta ≤ 1 escape
hatch — but only after a documented driver-roll incident.

**`--bless` mechanism:** environment variable `FLUI_BLESS_GOLDENS=1`
makes the harness write the live output to the reference path instead
of comparing.

**Env scrubbing:** at the start of every golden test:

```rust
fn scrub_font_env() {
    for var in [
        "ZED_FONTS_GAMMA",
        "ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST",
        "ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST",
    ] {
        if std::env::var(var).is_ok() {
            panic!(
                "Environment variable {var} is set; this would skew \
                 golden rendering. Unset it before running golden tests."
            );
        }
    }
}
```

Fail loud on developer mistakes; do not silently override.

### Step 7 — CI install + env

`.github/workflows/ci.yml` Linux install block (3 jobs: check, clippy,
test):

```yaml
sudo apt-get install -y \
  libwayland-dev libxkbcommon-dev libfontconfig-dev libegl-dev \
  libx11-dev libx11-xcb-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcb1-dev libvulkan-dev libssl-dev libsqlite3-dev \
  mesa-vulkan-drivers vulkan-tools
```

Test job env (only the `test` job — check and clippy don't run GPU
code):

```yaml
- name: Run tests
  env:
    VK_ICD_FILENAMES: /usr/share/vulkan/icd.d/lvp_icd.x86_64.json
    WGPU_POWER_PREF: low
  run: |
    vulkaninfo --summary || true   # diagnostic
    cargo test --workspace --features test-support
```

(`--features test-support` only if S01a.1's benchmark approved the
flip; otherwise add a separate test invocation for the goldens that
explicitly enables it.)

### Two-commit split inside the PR

To eliminate the "refactor under no safety net" risk:

**Commit A — pipeline cache lift, no headless variant:**
1. Introduce `WgpuPipelineCache` and `WgpuContext::new_headless()`.
2. Refactor `WgpuRenderer::new_internal` to consume the cache.
3. Surface variant continues to work; no new public API.
4. Run existing examples on Linux, confirm no visual change.

**Commit B — headless variant + goldens:**
1. Introduce `WgpuHeadlessRenderer`.
2. Add the test harness, scene fixtures, env scrubbing.
3. Capture initial reference PNGs via `FLUI_BLESS_GOLDENS=1` on the
   merge runner (or on a dev box and commit the PNGs).
4. Wire `current_headless_renderer()` for Linux.
5. Add CI install + env.
6. Run goldens; expect green on first replay.

Both commits land in the same PR. CI verifies green at HEAD of the PR.
Reverting either is one revert each.

## API surface

**New `pub` items (test-support gated):**

- `pub struct WgpuHeadlessRenderer` (in `platform/wgpu/wgpu_renderer.rs`
  or a new `wgpu_headless_renderer.rs`).
- `WgpuHeadlessRenderer::new() -> anyhow::Result<Self>`.
- `WgpuHeadlessRenderer::render_scene_to_image(&mut self, scene, size) -> anyhow::Result<RgbaImage>`.
- `impl PlatformHeadlessRenderer for WgpuHeadlessRenderer`.

**New crate-internal items:**

- `WgpuContext::new_headless() -> anyhow::Result<Self>`.
- `pub(crate) struct WgpuPipelineCache` (visibility scoped to the wgpu
  module).

**Re-exports** — added to S01a.3's enumerated list:

```rust
#[cfg(all(any(target_os = "linux", target_os = "freebsd"), any(test, feature = "test-support")))]
pub use platform::wgpu::WgpuHeadlessRenderer;
```

This requires `platform::wgpu` to be reachable as a path. Currently it's
`pub(crate) mod wgpu;` at `platform.rs:15`. Either promote to `pub mod wgpu`
or re-export at `platform.rs` level via `pub use wgpu::WgpuHeadlessRenderer;`.
Prefer the re-export (matches the pattern for `MacPlatform`).

**No semver-breaking changes** to existing public types. Pipeline cache
lift is internal.

## Migration / Compatibility

Existing surface-mode `WgpuRenderer` consumers (linux x11/wayland window
code, web window) are updated by the refactor in commit A but their
public API is unchanged. Any internal users of
`WgpuRenderer::new`/`new_internal` continue to work because the
constructor signature stays the same — only its internal pipeline
construction is moved to the cache.

`platform/visual_test.rs` extends to two backends but its
`VisualTestPlatform` struct stays at the same path.

## Testing strategy

1. **Existing examples on Linux** — `cargo run --example hello_world`,
   `window`, `window_shadow`, `opacity`, `tab_stop`. Must produce visually
   identical output before and after commit A. Manually inspected.
2. **Existing examples on macOS** — same set. Must produce visually
   identical output (mac doesn't change but we confirm the refactor in
   the wgpu module didn't break anything wired through to mac).
3. **Golden tests on Linux CI** — must capture references and replay
   them green in the same CI run.
4. **Golden tests on macOS CI** — same with `MetalHeadlessRenderer`.
5. **Cross-OS regression** — `cargo test --workspace` green on Linux and
   mac.
6. **`vulkaninfo --summary`** in the CI test step — diagnostic output
   in the log, used for debugging if a future driver roll changes
   output.
7. **Env-scrub assertions** — golden tests panic if `ZED_FONTS_*` are
   set in the environment.
8. **Stub inventory** via `cargo xtask check-stubs` from S01a.1 still
   green after the refactor.

## Open questions

- **Exact wgpu 24 polling API** — `device.poll(wgpu::Maintain::Wait)`
  or `device.poll(wgpu::PollType::Wait)`? Verify at implementation time
  against the `wgpu = "24"` crate docs. Spec uses the latter as a guess.
- **lavapipe ICD path on Ubuntu 24.04** — `lvp_icd.x86_64.json` is the
  expected name but verify with `vulkaninfo --summary` after install.
- **Whether `WgpuPipelineCache` should be `Arc<...>`-wrapped** for
  sharing across variants, or owned per-renderer. Current design owns
  per-renderer because each renderer can be on a different device.
  `Arc` only makes sense if surface and headless renderers share one
  process (rare).
- **Mesa version recording** — should `vulkaninfo --summary` output be
  saved as a CI artifact and diffed PR-to-PR? Recommendation: yes, but
  not in S01b — defer to a follow-up CI hardening spec.
- **mac visual_test.rs cfg gate** — currently
  `#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]`.
  Loosening to allow Linux complicates feature gates. Decision at
  implementation time: either two cfg-gated impls in the same file or
  split into platform-specific files.
- **`use wgpu::PowerPreference::LowPower`** in `new_headless` — for CI
  reproducibility. Real apps would use `HighPerformance`. Document the
  rationale in the `new_headless` doc comment.
- **Path intermediate texture format** — `wgpu_renderer.rs:957` clones
  it from `surface_config.format`. The headless path needs to plumb
  `Bgra8Unorm` explicitly. Verify in implementation.

## Done criteria

- [ ] `WgpuContext::new_headless()` exists, builds, and creates a device
      on lavapipe.
- [ ] `WgpuPipelineCache` exists, is shared between surface and headless
      paths, and uses explicit `PREMULTIPLIED_ALPHA_BLENDING` blend
      state.
- [ ] `WgpuHeadlessRenderer` exists, impls `PlatformHeadlessRenderer`,
      uses `Bgra8Unorm` + B/R swap on readback, correct row-stride math.
- [ ] `current_headless_renderer()` returns `Some(WgpuHeadlessRenderer)`
      on Linux/FreeBSD when `test-support` feature is enabled.
- [ ] `tests/golden/{metal,wgpu}/reference/*.png` committed (10 scenes
      × 2 platforms = 20 PNGs).
- [ ] `tests/golden/common/` harness file with env scrubbing.
- [ ] Golden tests pass green on macOS CI and Linux CI in the same PR.
- [ ] Existing examples (`hello_world`, `window`, `window_shadow`,
      `opacity`, `tab_stop`) produce visually identical output before
      and after the pipeline cache lift on both Linux and macOS. Manual
      inspection logged in PR description.
- [ ] `mesa-vulkan-drivers` and `vulkan-tools` installed in all 3 Linux
      CI install blocks.
- [ ] `VK_ICD_FILENAMES` and `WGPU_POWER_PREF` env vars set in the test
      job.
- [ ] `vulkaninfo --summary` output captured in the CI test job log.
- [ ] `cargo xtask check-stubs` green.
- [ ] No new `unimplemented!()`/`unreachable!()`/`todo!()` in the wgpu
      subtree.
- [ ] `cargo doc --no-deps -p flui-core --features test-support` on
      Linux + mac generates docs for the new public items without
      `missing_docs` warnings.
- [ ] PR contains exactly two logical commits: A (refactor) and B
      (headless + goldens).

## Test log

To be filled during implementation.

### Pre-refactor visual baseline

| Example | Linux output OK? | macOS output OK? |
|---|---|---|
| hello_world | TBD | TBD |
| window | TBD | TBD |
| window_shadow | TBD | TBD |
| opacity | TBD | TBD |
| tab_stop | TBD | TBD |

### Post-refactor visual check

| Example | Linux unchanged? | macOS unchanged? |
|---|---|---|
| hello_world | TBD | TBD |
| window | TBD | TBD |
| window_shadow | TBD | TBD |
| opacity | TBD | TBD |
| tab_stop | TBD | TBD |

### Golden tests first run

| Scene | macOS Metal | Linux wgpu |
|---|---|---|
| quad | TBD | TBD |
| shadow_blur | TBD | TBD |
| linear_gradient | TBD | TBD |
| radial_gradient | TBD | TBD |
| path_fill | TBD | TBD |
| monochrome_sprite | TBD | TBD |
| polychrome_sprite | TBD | TBD |
| text_single_line | TBD | TBD |
| text_multi_line | TBD | TBD |
| clip_rect | TBD | TBD |

### CI environment

- `vulkaninfo --summary` first-run output: TBD
- Mesa version on `ubuntu-latest`: TBD
- lavapipe ICD path: TBD

## Follow-ups after S01b lands

- **S02 unblocked** on the rendering correctness front. Migration of
  `wgpu/` to `flui-platform` will replay the goldens and any drift is
  caught instantly.
- **DirectX headless** for Windows golden coverage — deferred to S05
  prep. Substantial work (`DirectXRenderer` is 1951 LoC tightly coupled
  to DXGI swapchain).
- **Mesa version pinning / artifact** — follow-up CI hardening spec.
- **Cross-OS golden manifest** — a top-level
  `tests/golden/manifest.toml` listing every scene + per-platform
  reference path + capture date + driver version. Not in S01b; would
  let future driver-roll incidents be auditable.
- **wgpu offscreen for Windows** — once S05 lands `DirectXHeadlessRenderer`,
  there's an option to also have the wgpu DX12 backend produce
  Windows-flavored goldens for cross-validation. Opportunistic; no
  spec yet.
