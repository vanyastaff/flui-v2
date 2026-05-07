# flui-v2 Project Status & Architecture

## Quick Overview
Flutter-inspired GPU-accelerated UI framework for Rust, built on gpui-ce (Zed's GPUI).
- **Current Latest Work** (as of 2026-04-13): Animation system nearly complete (Curve enums, Lerp, physics simulations, tweens, damping)  
- **Architecture**: 5-layer stack (Application → navigator → widgets/animate/a11y → core → platform backends)
- **Multi-platform**: macOS (Metal), Windows (DirectX), Linux (wgpu + Wayland/X11)

## Layer-by-Layer Status

### Layer 1: Platform Backends
**Status**: ✅ Largely complete
- Metal (macOS), DirectX (Windows), wgpu (Linux)
- Wayland + X11 support on Linux
- Web/WASM in draft  

### Layer 2: flui-core (GPU rendering, elements, layout)
**Status**: ✅ ~90% complete, mature
- **What's done**: GPU rendering pipeline, entity system, Views, Elements, layout (Taffy), styling, input handling, async executor, text system, platform abstraction
- **Recent**: Animation controller + AnimationExt with Curves, physics-based simulations (Spring, Friction, Gravity)
- **Lingering TODOs**:
  - Default font selection on Linux (hardcoded `.SystemUIFont`)
  - Focus/tab group ordering (`.keys()` returns arbitrary order)
  - Some platform-specific edge cases documented in code

### Layer 3: Widget System & Animation
**flui-widgets** (Layout + Primitives): ~40% complete
- **Done**: Layout primitives (column, row, flex, padding, stack)
- **Partially done**: Widget base classes (ButtonBase, CheckboxBase, SliderBase, etc) — structure exists, logic in progress
- **Missing**: Full implementation of interaction handling, keyboard navigation, state management for each widget
- **Architecture**: Headless (zero styling) + design system applies visuals via `.style()`

**flui-animate**: ~5% (skeleton only)
- Comment says "will extend flui-core's Animation with higher-level primitives"
- Blocked on: Needs design decision on what "higher-level" means

**flui-a11y**: ~5% (skeleton only)  
- Placeholder for semantic tree, ARIA roles, keyboard nav helpers
- Not started

### Layer 4: flui-navigator (Routing)
**Status**: ✅ Appears functional
- Type-safe routing, nested routes, transitions, guards, middleware
- Features: guard, middleware, transition, cache (LRU), logging/tracing

### Layer 5: Design Systems
- **flui-material** (Material Design): Exists but not inspected
- **flui-theme**: Exists but not inspected

## Known Gaps & Incomplete Work

### High Priority (Blocking Usability)
1. **flui-widgets implementation**: Primitive widgets have stubs — need full interaction logic
2. **flui-animate design**: Need clear scope (higher-level choreography? gesture-driven animation binding?)
3. **Default font strategy**: Linux hardcoded to `.SystemUIFont` (not portable)

### Medium Priority (Nice-to-Have)
4. **flui-a11y**: Semantic tree + screen reader support (planned Phase 2+)
5. **iOS/Android support**: Planned Phase 3
6. **WASM platform**: Draft status, not production-ready

### Low Priority (Refinement)
7. Tab/focus group ordering via HashMap (should use ordered map)
8. Various platform-specific edge cases in window handling
