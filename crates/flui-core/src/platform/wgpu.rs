mod cosmic_text_system;
mod wgpu_atlas;
mod wgpu_context;
// `WgpuHeadlessRenderer` is gated on BOTH (test | test-support) AND
// not-wasm. Keep both predicates here so the submodule is not compiled
// under wasm32 + test-support (where `WgpuContext::new_headless` is
// cfg'd out and the symbol would fail to resolve).
#[cfg(all(any(test, feature = "test-support"), not(target_family = "wasm")))]
mod wgpu_headless_renderer;
mod wgpu_renderer;

pub use cosmic_text_system::*;
// Re-export the wgpu crate for downstream consumers that need to
// construct `wgpu::SurfaceConfiguration` / `wgpu::Backends` etc.
// without taking a direct dep. Currently unused inside flui-core
// itself, but part of the public extension surface.
#[allow(unused_imports)]
pub use wgpu;
pub use wgpu_atlas::*;
pub use wgpu_context::*;
#[cfg(all(any(test, feature = "test-support"), not(target_family = "wasm")))]
pub use wgpu_headless_renderer::WgpuHeadlessRenderer;
pub use wgpu_renderer::{GpuContext, WgpuRenderer, WgpuSurfaceConfig};
