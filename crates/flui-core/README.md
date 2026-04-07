# flui-core

Core crate of the flui framework — a Flutter-inspired GPU-accelerated UI framework for Rust.

Based on [gpui-ce](https://github.com/gpui-ce/gpui-ce), the community edition of Zed's GPUI framework.

## Features

- GPU-accelerated rendering (Metal on macOS, DirectX on Windows, wgpu on Linux)
- Taffy-based flexbox layout engine
- Declarative element/view system
- Cross-platform: macOS, Linux (Wayland + X11), Windows
- Integrated async task scheduler
- Animation support
