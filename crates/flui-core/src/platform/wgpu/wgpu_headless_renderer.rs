//! Headless wgpu renderer for offscreen golden tests.
//!
//! Implements `PlatformHeadlessRenderer` on Linux/FreeBSD. This is a
//! **skeleton** landed by S01b — the struct exists, the constructor
//! creates a real headless `WgpuContext` (so adapter selection is
//! tested every time the binary loads), and an atlas is wired up.
//! The `render_scene_to_image` method produces a solid-color
//! placeholder image today.
//!
//! **A pixel-accurate Scene → PNG implementation is pending.** It
//! requires duplicating ~500 LoC of `WgpuRenderer`'s internal rendering
//! loop — bind group allocation, pipeline construction,
//! instance buffer grow/shrink, path intermediate textures, MSAA
//! resolution, offscreen texture readback with
//! `COPY_BYTES_PER_ROW_ALIGNMENT`-padded rows — plus careful
//! verification on a real Linux/Mesa environment. S01b lands the
//! skeleton so S02 and later specs have a stable symbol to point at;
//! a follow-up PR (tracked in `docs/lock-coverage-gaps.md`) fills in
//! the rendering body and commits the initial golden reference PNGs.
//!
//! See `docs/superpowers/specs/2026-04-13-S01b-lock-wgpu-headless-and-golden-design.md`
//! for the full design. The key constraints the follow-up must
//! satisfy:
//!
//! - Offscreen texture format: `Bgra8Unorm` (matches the surface
//!   path's first preference, avoids sRGB gamma drift).
//! - Pipeline `BlendState`: explicit `PREMULTIPLIED_ALPHA_BLENDING`
//!   (not derived from `CompositeAlphaMode`, which doesn't exist
//!   offscreen).
//! - Readback pattern: `queue.submit(...)` →
//!   `buffer.slice(..).map_async(MapMode::Read, cb)` →
//!   `device.poll(PollType::Wait)` → `block_on(rx)`. The callback
//!   registration MUST happen before polling, and `device.poll`
//!   must run on the same thread as the sync wait.
//! - Row alignment: `padded_bpr = align(width * 4, COPY_BYTES_PER_ROW_ALIGNMENT)`.
//!   Readback strips the padding row-by-row before constructing
//!   the `RgbaImage`.
//! - B/R channel swap on readback since `Bgra8Unorm` → `RgbaImage`.
//! - Environment scrubbing: the golden test harness unsets
//!   `ZED_FONTS_GAMMA`, `ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST`,
//!   `ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST` before rendering — they
//!   are baked into the uniform buffer and would skew golden output
//!   on any dev machine that exports them.

// Module-level cfg gating lives at the `mod wgpu_headless_renderer;`
// declaration in `platform/wgpu.rs` to avoid the pattern where this
// file's `#![cfg(not(target_family = "wasm"))]` inner attribute would
// empty the module under wasm + test-support, leaving the `pub use`
// at the declaration site with an unresolved symbol.

use super::{WgpuAtlas, WgpuContext};
use flui_core::{DevicePixels, PlatformAtlas, PlatformHeadlessRenderer, Scene, Size};
use std::sync::Arc;

/// A headless wgpu renderer.
///
/// Constructed via `WgpuHeadlessRenderer::new()`, which performs real
/// wgpu adapter selection and device creation. Safe to drop and
/// recreate between tests.
pub struct WgpuHeadlessRenderer {
    #[allow(dead_code)]
    context: WgpuContext,
    atlas: Arc<WgpuAtlas>,
}

impl WgpuHeadlessRenderer {
    /// Creates a new headless renderer by building a `WgpuContext` via
    /// `WgpuContext::new_headless()` and allocating a sprite atlas on
    /// top of the resulting device/queue.
    pub fn new() -> anyhow::Result<Self> {
        let context = WgpuContext::new_headless()?;
        let atlas = Arc::new(WgpuAtlas::new(
            Arc::clone(&context.device),
            Arc::clone(&context.queue),
        ));
        Ok(Self { context, atlas })
    }
}

impl PlatformHeadlessRenderer for WgpuHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        _scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        // TODO(S01b-followup): implement the pixel-accurate renderer
        // body. See the module doc comment for the constraints that
        // the implementation must satisfy.
        //
        // For now, return a solid-black image so the trait contract
        // is satisfied and consumers can wire up to the renderer
        // without a compile-time blocker. Any golden test that
        // renders through this stub will trivially diff against a
        // captured reference, which is correct: a stub is an
        // unverifiable baseline and the follow-up PR will bless
        // real references at the same time as it replaces this
        // body.
        let width = size.width.0.max(0) as u32;
        let height = size.height.0.max(0) as u32;
        let mut buf = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            buf.extend_from_slice(&[0, 0, 0, 255]);
        }
        image::RgbaImage::from_raw(width, height, buf)
            .ok_or_else(|| anyhow::anyhow!("failed to construct placeholder RgbaImage"))
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}
