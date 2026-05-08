# flui-v2

A Flutter-inspired GPU-accelerated UI framework for Rust, built on the foundation of [gpui-ce](https://github.com/gpui-ce/gpui-ce) (the community edition of Zed's GPUI).

## Vision

flui-v2 takes GPUI's proven GPU rendering foundation and evolves it toward a Flutter-like developer experience in Rust: composable widgets, declarative routing, animations, and accessibility — all with native performance.

## Architecture (5 layers)

```
+-------------------------------------------------------+
|  Layer 5: Application                                  |
|  Your app code, examples, demos                        |
+-------------------------------------------------------+
|  Layer 4: flui-navigator                               |
|  Type-safe routing, transitions, guards, middleware     |
+-------------------------------------------------------+
|  Layer 3: flui-widgets / flui-a11y                     |
|  Widget library, accessibility                          |
+-------------------------------------------------------+
|  Layer 2: flui-core                                    |
|  Entity system, views, elements, layout (Taffy),       |
|  styling, input, async executor                        |
+-------------------------------------------------------+
|  Layer 1: Platform backends                            |
|  Metal (macOS), DirectX (Windows), wgpu (Linux),       |
|  Wayland, X11                                          |
+-------------------------------------------------------+
```

## Workspace structure

```
flui-v2/
  crates/
    flui-core/       # GPU rendering, element system, layout, platform backends
    flui-macros/     # Procedural macros (derive Render, IntoElement, etc.)
    flui-navigator/  # Routing: nested routes, transitions, guards, middleware
    flui-widgets/    # Widget library (planned: Button, Input, Modal, Theme)
    flui-a11y/       # Accessibility / semantic tree (planned)
  examples/
    nav_demo/        # Navigation routing demo
```

## Quick start

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

- [gpui-ce](https://github.com/gpui-ce/gpui-ce) - Community edition of Zed's GPUI
- [gpui-navigator](https://github.com/vanyastaff/gpui-navigator) - Type-safe routing for GPUI

## License

Apache-2.0
