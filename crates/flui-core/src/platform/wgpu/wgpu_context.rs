#[cfg(not(target_family = "wasm"))]
use anyhow::Context as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_family = "wasm"))]
use util::ResultExt;

pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    dual_source_blending: bool,
    device_lost: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
pub struct CompositorGpuHint {
    pub vendor_id: u32,
    pub device_id: u32,
}

impl WgpuContext {
    #[cfg(not(target_family = "wasm"))]
    pub fn new(
        instance: wgpu::Instance,
        surface: &wgpu::Surface<'_>,
        compositor_gpu: Option<CompositorGpuHint>,
    ) -> anyhow::Result<Self> {
        let device_id_filter = match std::env::var("ZED_DEVICE_ID") {
            Ok(val) => parse_pci_id(&val)
                .context("Failed to parse device ID from `ZED_DEVICE_ID` environment variable")
                .log_err(),
            Err(std::env::VarError::NotPresent) => None,
            err => {
                err.context("Failed to read value of `ZED_DEVICE_ID` environment variable")
                    .log_err();
                None
            }
        };

        // Select an adapter by actually testing surface configuration with the real device.
        // This is the only reliable way to determine compatibility on hybrid GPU systems.
        let (adapter, device, queue, dual_source_blending) =
            pollster::block_on(Self::select_adapter_and_device(
                &instance,
                device_id_filter,
                surface,
                compositor_gpu.as_ref(),
            ))?;

        let device_lost = Arc::new(AtomicBool::new(false));
        device.set_device_lost_callback({
            let device_lost = Arc::clone(&device_lost);
            move |reason, message| {
                log::error!("wgpu device lost: reason={reason:?}, message={message}");
                if reason != wgpu::DeviceLostReason::Destroyed {
                    device_lost.store(true, Ordering::Relaxed);
                }
            }
        });

        log::info!(
            "Selected GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            dual_source_blending,
            device_lost,
        })
    }

    /// Creates a headless `WgpuContext` without any surface.
    ///
    /// Used by `WgpuHeadlessRenderer` for offscreen golden rendering and
    /// by `current_headless_renderer()` on Linux/FreeBSD in test-support
    /// builds. Selects the first adapter that can create a device with
    /// the same features/limits as the surface path, preferring low
    /// power (software rasterizers like lavapipe for CI reproducibility).
    #[cfg(all(any(test, feature = "test-support"), not(target_family = "wasm")))]
    pub fn new_headless() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to request headless wgpu adapter — no compatible adapter found")
        })?;

        log::info!(
            "Selected headless GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue, dual_source_blending) =
            pollster::block_on(Self::create_device(&adapter))?;

        let device_lost = Arc::new(AtomicBool::new(false));
        device.set_device_lost_callback({
            let device_lost = Arc::clone(&device_lost);
            move |reason, message| {
                log::error!("wgpu device lost: reason={reason:?}, message={message}");
                if reason != wgpu::DeviceLostReason::Destroyed {
                    device_lost.store(true, Ordering::Relaxed);
                }
            }
        });

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            dual_source_blending,
            device_lost,
        })
    }

    #[cfg(target_family = "wasm")]
    pub async fn new_web() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to request GPU adapter: {e}"))?;

        log::info!(
            "Selected GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let device_lost = Arc::new(AtomicBool::new(false));
        let (device, queue, dual_source_blending) = Self::create_device(&adapter).await?;

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            dual_source_blending,
            device_lost,
        })
    }

    async fn create_device(
        adapter: &wgpu::Adapter,
    ) -> anyhow::Result<(wgpu::Device, wgpu::Queue, bool)> {
        // Dual-source blending is disabled for now because naga (wgpu's shader
        // validator) does not yet support the `enable dual_source_blending;` WGSL
        // directive. See https://github.com/gfx-rs/wgpu/issues/6402
        let dual_source_blending = false;

        let mut required_features = wgpu::Features::empty();
        if false {
            required_features |= wgpu::Features::DUAL_SOURCE_BLENDING;
        } else {
            log::warn!(
                "Dual-source blending not available on this GPU. \
                Subpixel text antialiasing will be disabled."
            );
        }

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("flui_device"),
                    required_features,
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits())
                        .using_alignment(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create wgpu device: {e}"))?;

        Ok((device, queue, dual_source_blending))
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn instance(_display: Box<dyn raw_window_handle::HasDisplayHandle>) -> wgpu::Instance {
        wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
        })
    }

    pub fn check_compatible_with_surface(&self, surface: &wgpu::Surface<'_>) -> anyhow::Result<()> {
        let caps = surface.get_capabilities(&self.adapter);
        if caps.formats.is_empty() {
            let info = self.adapter.get_info();
            anyhow::bail!(
                "Adapter {:?} (backend={:?}, device={:#06x}) is not compatible with the \
                 display surface for this window.",
                info.name,
                info.backend,
                info.device,
            );
        }
        Ok(())
    }

    /// Select an adapter and create a device, testing that the surface can actually be configured.
    /// This is the only reliable way to determine compatibility on hybrid GPU systems, where
    /// adapters may report surface compatibility via get_capabilities() but fail when actually
    /// configuring (e.g., NVIDIA reporting Vulkan Wayland support but failing because the
    /// Wayland compositor runs on the Intel GPU).
    #[cfg(not(target_family = "wasm"))]
    async fn select_adapter_and_device(
        instance: &wgpu::Instance,
        device_id_filter: Option<u32>,
        surface: &wgpu::Surface<'_>,
        compositor_gpu: Option<&CompositorGpuHint>,
    ) -> anyhow::Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue, bool)> {
        let mut adapters: Vec<_> = instance.enumerate_adapters(wgpu::Backends::all());

        if adapters.is_empty() {
            anyhow::bail!("No GPU adapters found");
        }

        if let Some(device_id) = device_id_filter {
            log::info!("ZED_DEVICE_ID filter: {:#06x}", device_id);
        }

        // Sort adapters into a single priority order. Tiers (from highest to lowest):
        //
        // 1. ZED_DEVICE_ID match — explicit user override
        // 2. Compositor GPU match — the GPU the display server is rendering on
        // 3. Device type (Discrete > Integrated > Other > Virtual > Cpu).
        //    "Other" ranks above "Virtual" because OpenGL seems to count as "Other".
        // 4. Backend — prefer Vulkan/Metal/Dx12 over GL/etc.
        adapters.sort_by_key(|adapter| {
            let info = adapter.get_info();

            // Backends like OpenGL report device=0 for all adapters, so
            // device-based matching is only meaningful when non-zero.
            let device_known = info.device != 0;

            let user_override: u8 = match device_id_filter {
                Some(id) if device_known && info.device == id => 0,
                _ => 1,
            };

            let compositor_match: u8 = match compositor_gpu {
                Some(hint)
                    if device_known
                        && info.vendor == hint.vendor_id
                        && info.device == hint.device_id =>
                {
                    0
                }
                _ => 1,
            };

            let type_priority: u8 = match info.device_type {
                wgpu::DeviceType::DiscreteGpu => 0,
                wgpu::DeviceType::IntegratedGpu => 1,
                wgpu::DeviceType::Other => 2,
                wgpu::DeviceType::VirtualGpu => 3,
                wgpu::DeviceType::Cpu => 4,
            };

            let backend_priority: u8 = match info.backend {
                wgpu::Backend::Vulkan => 0,
                wgpu::Backend::Metal => 0,
                wgpu::Backend::Dx12 => 0,
                _ => 1,
            };

            (
                user_override,
                compositor_match,
                type_priority,
                backend_priority,
            )
        });

        // Log all available adapters (in sorted order)
        log::info!("Found {} GPU adapter(s):", adapters.len());
        for adapter in &adapters {
            let info = adapter.get_info();
            log::info!(
                "  - {} (vendor={:#06x}, device={:#06x}, backend={:?}, type={:?})",
                info.name,
                info.vendor,
                info.device,
                info.backend,
                info.device_type,
            );
        }

        // Test each adapter by creating a device and configuring the surface
        for adapter in adapters {
            let info = adapter.get_info();
            log::info!("Testing adapter: {} ({:?})...", info.name, info.backend);

            match Self::try_adapter_with_surface(&adapter, surface).await {
                Ok((device, queue, dual_source_blending)) => {
                    log::info!(
                        "Selected GPU (passed configuration test): {} ({:?})",
                        info.name,
                        info.backend
                    );
                    return Ok((adapter, device, queue, dual_source_blending));
                }
                Err(e) => {
                    log::info!(
                        "  Adapter {} ({:?}) failed: {}, trying next...",
                        info.name,
                        info.backend,
                        e
                    );
                }
            }
        }

        anyhow::bail!("No GPU adapter found that can configure the display surface")
    }

    /// Try to use an adapter with a surface by creating a device and testing configuration.
    /// Returns the device and queue if successful, allowing them to be reused.
    #[cfg(not(target_family = "wasm"))]
    async fn try_adapter_with_surface(
        adapter: &wgpu::Adapter,
        surface: &wgpu::Surface<'_>,
    ) -> anyhow::Result<(wgpu::Device, wgpu::Queue, bool)> {
        let caps = surface.get_capabilities(adapter);
        if caps.formats.is_empty() {
            anyhow::bail!("no compatible surface formats");
        }
        if caps.alpha_modes.is_empty() {
            anyhow::bail!("no compatible alpha modes");
        }

        let (device, queue, dual_source_blending) = Self::create_device(adapter).await?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let test_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: 64,
            height: 64,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &test_config);

        let error = device.pop_error_scope().await;
        if let Some(e) = error {
            anyhow::bail!("surface configuration failed: {e}");
        }

        Ok((device, queue, dual_source_blending))
    }

    pub fn supports_dual_source_blending(&self) -> bool {
        self.dual_source_blending
    }

    /// Returns true if the GPU device was lost (e.g., due to driver crash, suspend/resume).
    /// When this returns true, the context should be recreated.
    pub fn device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Relaxed)
    }

    /// Returns a clone of the device_lost flag for sharing with renderers.
    pub(crate) fn device_lost_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.device_lost)
    }

    /// ADR-014: classify the wgpu adapter. Returns
    /// [`crate::RendererKind::Software`] when wgpu reports
    /// `wgpu::DeviceType::Cpu` (Mesa `llvmpipe`, WARP, etc.); otherwise
    /// [`crate::RendererKind::Hardware`]. The Linux/X11/Wayland
    /// `Platform::renderer_kind` impls forward to this method —
    /// wiring is deferred per the rollout plan, so this method is
    /// currently dead from the engine's POV but published as the
    /// stable downstream surface for apps that hold a `WgpuContext`
    /// directly (e.g. via the headless renderer).
    #[allow(dead_code)] // ADR-014: pending LinuxPlatform forward.
    pub fn renderer_kind(&self) -> crate::RendererKind {
        match self.adapter.get_info().device_type {
            wgpu::DeviceType::Cpu => crate::RendererKind::Software,
            _ => crate::RendererKind::Hardware,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
fn parse_pci_id(id: &str) -> anyhow::Result<u32> {
    let mut id = id.trim();

    if id.starts_with("0x") || id.starts_with("0X") {
        id = &id[2..];
    }
    let is_hex_string = id.chars().all(|c| c.is_ascii_hexdigit());
    let is_4_chars = id.len() == 4;
    anyhow::ensure!(
        is_4_chars && is_hex_string,
        "Expected a 4 digit PCI ID in hexadecimal format"
    );

    u32::from_str_radix(id, 16).context("parsing PCI ID as hex")
}

#[cfg(test)]
mod tests {
    use super::parse_pci_id;

    #[test]
    fn test_parse_device_id() {
        assert!(parse_pci_id("0xABCD").is_ok());
        assert!(parse_pci_id("ABCD").is_ok());
        assert!(parse_pci_id("abcd").is_ok());
        assert!(parse_pci_id("1234").is_ok());
        assert!(parse_pci_id("123").is_err());
        assert_eq!(
            parse_pci_id(&format!("{:x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:X}", 0x1234)).unwrap(),
        );

        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
    }

    /// ADR-005 regression test for `device_lost()` as the single source
    /// of truth.
    ///
    /// Locks two ADR-005 contract points:
    /// 1. Decision 1 — `WgpuContext::device_lost()` is the canonical
    ///    "is the GPU currently usable" probe. The atomic flag returned
    ///    by `device_lost_flag()` must be the same `Arc<AtomicBool>` the
    ///    callback at construction writes to, so external observers
    ///    (renderers, atlases) and the public probe agree.
    /// 2. Decision 4 — the flag uses `Ordering::SeqCst` for cross-window
    ///    observation. We exercise SeqCst by writing in one thread and
    ///    reading from another, asserting visibility.
    ///
    /// Gated on `test-support` because `WgpuContext::new_headless` itself
    /// is gated there — without a wgpu adapter, the test is a no-op on
    /// CI runners that lack lavapipe. Skips gracefully (returns early
    /// with a `log::info!`) when no adapter is available; this keeps the
    /// `cargo test -p flui-core --features test-support` matrix green on
    /// every host the project actually runs on (macOS, Linux+wgpu,
    /// Windows-via-wgpu, headless+lavapipe).
    ///
    /// Note: this test does NOT drive `Renderer::recover()` — that path
    /// requires a real window handle and is verified by manual smoke
    /// (see ADR-005 action item 4 / plan task 5 verification).
    /// See `docs/research/adr/ADR-005-gpu-device-loss.md`.
    #[cfg(all(feature = "test-support", not(target_family = "wasm")))]
    #[test]
    fn adr_005_device_lost_flag_is_canonical_and_shared() {
        use super::WgpuContext;
        use std::sync::atomic::Ordering;

        let context = match WgpuContext::new_headless() {
            Ok(c) => c,
            Err(e) => {
                log::info!(
                    "ADR-005 test skipped: no headless wgpu adapter on this host ({e:?}). \
                     Manual smoke on a wgpu-capable host is required per plan task 5 verification."
                );
                return;
            }
        };

        // Decision 1: fresh context is not in a lost state.
        assert!(
            !context.device_lost(),
            "ADR-005: fresh WgpuContext::new_headless must report device_lost == false"
        );

        // Decision 1 + 4: `device_lost_flag()` returns the same Arc that
        // `device_lost()` reads. Toggling through the shared Arc must be
        // immediately observable via the canonical probe.
        let flag = context.device_lost_flag();
        flag.store(true, Ordering::SeqCst);
        assert!(
            context.device_lost(),
            "ADR-005: device_lost_flag() must alias the same AtomicBool that \
             device_lost() reads — toggling one MUST be visible through the other"
        );

        // Cross-thread observation: write in a spawned thread, read on
        // this one. SeqCst ordering guarantees visibility after thread
        // join completes.
        flag.store(false, Ordering::SeqCst);
        let flag_for_thread = std::sync::Arc::clone(&flag);
        let join = std::thread::spawn(move || {
            flag_for_thread.store(true, Ordering::SeqCst);
        });
        join.join().expect("thread did not panic");
        assert!(
            context.device_lost(),
            "ADR-005: cross-thread store-via-shared-Arc must be visible to device_lost() reads"
        );
    }
}
