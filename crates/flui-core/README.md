# flui-core

Core crate of the flui framework — a Flutter-inspired GPU-accelerated UI framework for Rust.

Based on [gpui-ce](https://github.com/gpui-ce/gpui-ce), the community edition of Zed's GPUI framework.

## Features

- GPU-accelerated rendering (Metal on macOS, DirectX on Windows, wgpu on Linux)
- Taffy-based flexbox layout engine
- Declarative element/view system
- Mutable `Render` views plus immutable `ElementBuilder` recipes for K03 render/build separation
- Cross-platform: macOS, Linux (Wayland + X11), Windows
- Integrated async task scheduler
- Animation support

## Render and Build Vocabulary

`Render` is the mutable, entity-backed engine view trait used for window roots
and cached views. `RenderOnce` remains the compatibility path for consuming
stateless engine recipes. `ElementBuilder` is the K03 pure-build substrate for
immutable recipes that build the existing element tree through `build_element`.

The final Flutter-style `Widget`, `State`, reconciliation, and `BuildCx`
surface belongs to the planned `flui-framework` crate.
