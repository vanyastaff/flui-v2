# flui-v2

## Overview

flui-v2 is a Flutter-inspired, GPU-accelerated UI framework for Rust, built on the foundation of [gpui-ce](https://github.com/gpui-ce/gpui-ce) (the community edition of Zed's GPUI runtime). It evolves the proven GPUI rendering pipeline toward a Flutter-like developer experience: composable widgets, declarative routing, animations, and accessibility, while keeping native desktop performance via Metal, DirectX, and wgpu backends.

The project is currently in Phase I of a multi-phase roadmap focused on (1) extracting all platform-specific code from `flui-core` into a dedicated `flui-platform` crate without losing functionality, (2) closing Flutter-level gaps in core subsystems (gestures, semantics, canvas, filters, physics, text, media query, assets), and (3) eventually adding new platform embeddings (iOS, Android, web rendering, headless). Higher-level widget crates (`flui-widgets`, `flui-material`) are intentionally out of scope until the runtime stabilizes.

## Core Features

- GPU-accelerated rendering via wgpu (Linux/FreeBSD), Metal (macOS), and Direct3D 11 (Windows).
- Entity / View / Element runtime derived from GPUI: `App`, `Entity`, `Context`, `Window`.
- Painting primitives: `scene`, `path_builder`, `style`, Taffy-based layout, text system (cosmic-text / swash).
- Async runtime: `scheduler`, `executor`, `queue`, `animation` with `AnimationController`.
- Input pipeline: `input`, `interactive`, `key_dispatch`, `keymap`, `tab_stop`, plus `gesture` (S07 + S07.5 + S07.5b) — Flutter-style competing recognizers (`Tap`, `DoubleTap`, `LongPress`, `Pan`/`HorizontalDrag`/`VerticalDrag`, `Scale`), per-`Window` `GestureBinding`, explicit hit-test protocol with `HitTestBehavior` (`Opaque`/`Translucent`/`DeferToChild`), and a `VelocityTracker` (Flutter-LSQ port). After S07.5: DoubleTap holds the arena past the first `Up` and releases on a per-pointer timeout, LongPress accepts via a timer-driven arena back-channel, and the `RecognizerLifecycle` trait is the canonical extensibility seam for new recognizers. After S07.5b: `PointerEvent` carries `Option<PressureSample>` (mouse-class events default `None`; macOS Force Touch surfaces real values), a `provenance` enum distinguishes platform vs sanitizer-synthesised events, `timestamp` is split into `timestamp` + `source_timestamp` for resampler/semantics-aware velocity tracking, and `PointerPanZoomEvent` exists as a sibling type. Hit-test entries record an optional `Affine2` `target-local → window-local` transform (Flutter convention) pushed via the RAII `HitTestScope` guard, which the dispatcher inverts once per delivery to compute `DeliveredEvent.local_position`. Recognizers consume events via `DeliveredEvent<'_>` with explicit `local_position`, gate admission via the per-recognizer `AllowedButtonsFilter`, and share a unified per-pointer `set_arena_back_channel(pid, bc, idx)` lifecycle hook. The arena `hold_count: u32` counter replaces the previous boolean.
- Type-safe routing via `flui-navigator` (nested routes, transitions, guards, middleware).
- Procedural macros (`flui-macros`): `derive(Render)`, `derive(IntoElement)`, etc.
- Skeleton crates ready for incremental population: `flui-platform`, `flui-animate`, `flui-a11y`, `flui-theme`, `flui-material`, `flui-widgets`.

## Tech Stack

- **Programming language:** Rust (edition 2024, MSRV 1.85)
- **Workspace resolver:** Cargo resolver = "3"
- **Async runtime:** smol
- **GPU / graphics:** wgpu, naga, metal (macOS), Direct3D 11 via `windows` crate, ash (Vulkan loader), Wayland (`wayland-client`) and X11 (`x11rb`) on Linux
- **Layout engine:** Taffy
- **Text rendering:** cosmic-text, swash, fontdue
- **Geometry:** euclid
- **Build automation:** Cargo + Nix flake (`flake.nix`) for reproducible dev shell
- **Lint discipline:** workspace-wide Clippy config (`clippy.toml`) with `disallowed-methods` rules pushing async-safe variants from `smol::process` over `std::process`
- **Spell check:** `typos.toml`

## Architecture

See `.ai-factory/ARCHITECTURE.md` for detailed architecture guidelines (folder layout, dependency rules, layer communication, code examples, anti-patterns).

Pattern: **Layered + Cargo workspace** — five layers (platform → core → widgets/animate/a11y → navigator → app), each realized as a Cargo crate so layering is enforced mechanically by the dependency graph.

## Architecture Notes

The project intentionally avoids replicating Flutter's deep internal layering (an earlier v1 attempt with `flui-foundation`/`flui-engine`/`flui-rendering`/etc. was abandoned). Instead the architecture replicates Flutter's **feature surface** on top of GPUI's single-level engine. The 5-layer model documented in `README.md` reflects user-facing layering, not engine internals:

```
Layer 5: Application (your app, examples, demos)
Layer 4: flui-navigator (routing, transitions, guards, middleware)
Layer 3: flui-widgets / flui-animate / flui-a11y (widget library, animation, a11y)
Layer 2: flui-core (entity system, views, elements, layout, styling, input, executor)
Layer 1: Platform backends (Metal / DirectX / wgpu / Wayland / X11)
```

Per the active roadmap, the long-term direction is to peel Layer 1 out of `flui-core` into the dedicated `flui-platform` crate. Approximately 42,936 LoC across 80 files under `crates/flui-core/src/platform/**` are scheduled to migrate via specs S02 through S06. The new `flui-platform` skeleton already exists at `crates/flui-platform/`, populated incrementally per `docs/superpowers/specs/`.

Authoritative architectural and migration context lives in `docs/superpowers/specs/` and `docs/superpowers/plans/`. Specialized review subagents (`flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, `rust-api-migration-auditor`) are configured in `.claude/agents/` and should be used proactively on changes touching the runtime or platform code.

## Non-Functional Requirements

- **Logging:** standard `log` / `tracing` ecosystem (configured per-crate); no project-wide logger config yet.
- **Error handling:** structured Rust `Result`/`Error` types; explicit `unimplemented!()` / `unreachable!()` sites in platform code are tracked by the roadmap (S01a inventory) and must be classified, not casually replaced.
- **Async safety:** Clippy enforces `smol::process::Command::*` over `std::process::Command::*` to avoid blocking the executor thread.
- **Determinism:** GPU work targets deterministic offscreen rendering for golden tests; `lock-checks` tooling and `lock-coverage-gaps.md` track regressions.
- **Platform parity:** macOS (Metal), Windows (Direct3D 11), Linux (wgpu + Wayland + X11) are all first-class targets; iOS/Android/WASM are roadmap items.
- **MSRV:** Rust 1.85 (edition 2024).
- **Spelling discipline:** `typos.toml` is enforced.
