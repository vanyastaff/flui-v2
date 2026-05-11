# flui-v2

A Flutter-inspired GPU-accelerated UI framework for Rust, built on the foundation of [gpui-ce](https://github.com/gpui-ce/gpui-ce) (the community edition of Zed's GPUI).

## Vision

flui-v2 takes GPUI's proven GPU rendering foundation and evolves it toward a Flutter-like developer experience in Rust: composable widgets, declarative routing, animations, and accessibility — all with native performance.

## Architecture (three-tier)

```text
+-------------------------------------------------------+
|  C. ECOSYSTEM (community-writable)                    |
|  flui-widgets, flui-material, flui-cupertino,         |
|  flui-theme, flui-navigator, flui-a11y                |
+-------------------------------------------------------+
|  B. FRAMEWORK (Flutter developer experience)          |
|  flui-framework — PLANNED (Phase II-F)                |
|  Widget + Key + State + BuildCx + Provider            |
+-------------------------------------------------------+
|  A. ENGINE (substrate)                                |
|  flui-core (App + Entity + Element + Scene +          |
|             Window + Layout + Text + Gesture +        |
|             Animation)                                |
|  flui-platform (skeleton — Phase III)                 |
|  flui-macros (proc macros)                            |
+-------------------------------------------------------+
```

Hard fork of [gpui-ce](https://github.com/gpui-ce/gpui-ce) — see [ARCHITECTURE.md](.ai-factory/ARCHITECTURE.md) for the full rationale, the "2 structures + 1 cache" Framework-tier model, and Phase 0-K kernel-cleanup track.

## Workspace structure

```text
flui-v2/
  crates/
    flui-core/       # GPU rendering, entity system, layout (Taffy),
                     # text (cosmic-text), gesture, animation, platform backends
    flui-platform/   # Platform abstraction crate (skeleton — Phase III)
    flui-macros/     # Procedural macros (derive Render, IntoElement, etc.)
    flui-navigator/  # Routing: nested routes, transitions, guards, middleware
    flui-widgets/    # Widget library (skeleton — gated on Framework tier)
    flui-material/   # Material design widgets (skeleton)
    flui-theme/      # Theming (skeleton)
    flui-a11y/       # Accessibility / semantic tree (skeleton)
  examples/
    nav_demo/        # Navigation routing demo
    material_demo/   # Material widget demo
    animation_demo/  # Animation system demo
  tooling/
    lock-checks/     # Lock-behavior regression checks
  docs/superpowers/
    specs/           # Design docs (YYYY-MM-DD-<id>-<slug>-design.md)
```

## Quick start

**MSRV:** Rust 1.95 (edition 2024) — pinned via `rust-toolchain.toml`.

```toml
[dependencies]
flui-core = { git = "https://github.com/vanyastaff/flui-v2" }
flui-navigator = { git = "https://github.com/vanyastaff/flui-v2" }
```

```rust
extern crate flui_core as gpui;
use gpui::*;
use flui_navigator::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", home_page).transition(Transition::fade(200)));
        });
        // ...
    });
}
```

## Platform support

| Platform | Backend | Status |
|----------|---------|--------|
| macOS    | Metal   | Supported |
| Linux    | wgpu (Wayland + X11) | Supported |
| Windows  | DirectX | Supported |
| iOS/Android | - | Planned (Phase 3) |
| WASM     | Canvas  | Draft |

## Building

### Dependencies

**Linux (Fedora/RHEL):**
```sh
sudo dnf install wayland-devel libxkbcommon-devel fontconfig-devel \
    mesa-libEGL-devel libX11-devel vulkan-loader-devel
```

**Linux (Ubuntu/Debian):**
```sh
sudo apt install libwayland-dev libxkbcommon-dev libfontconfig-dev \
    libegl-dev libx11-dev libvulkan-dev
```

**macOS:** Xcode command line tools (`xcode-select --install`)

### Build & run

```sh
cargo build --workspace
cargo run -p nav_demo              # navigation demo
cargo run -p flui-core --example hello_world  # hello world
```

## Based on

- [gpui-ce](https://github.com/gpui-ce/gpui-ce) — Community edition of Zed's GPUI. flui-v2 is a **hard fork** (no upstream-sync commitment, no semver compatibility); breaking changes are the design goal. The `extern crate flui_core as gpui;` pattern shown above is a one-way migration aid for porting Zed-style code, not a compatibility contract.
- [gpui-navigator](https://github.com/vanyastaff/gpui-navigator) — Type-safe routing for GPUI

## Project status

- **Phase 0-K (Kernel Cleanup) — active.** Architectural debt repayment in `flui-core` before Framework tier (Phase II-F) work begins. Tracked in [`.ai-factory/ROADMAP.md`](.ai-factory/ROADMAP.md). Done so far: [K99 MSRV bump](docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md), [K15 re-entrancy contract](docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md), [K01 Provider rewrite](docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md), [K02 Element identity and Key](docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md), and [K03 Render to Build separation](docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md). Next critical-chain item: K04 Effect / Frame contract.
- **Phase II — engine completeness.** Gesture arena (S07), animation parity (S21) done; semantics (S08), canvas facade (S09), media query (S14) pending.
- **Phase II-F — Framework tier (Widget / State / setState).** Not started; gated on Phase 0-K critical chain completion.

## License

Apache-2.0
