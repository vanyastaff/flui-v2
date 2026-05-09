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
- Skeleton crates ready for incremental population: `flui-platform`, `flui-a11y`, `flui-theme`, `flui-material`, `flui-widgets`. (The `flui-animate` skeleton was removed in S21 phase 5 — animation primitives live in `flui-core::animation`; widget-layer animation builders are deferred to the existing `flui-widgets` crate.)

## Tech Stack

- **Programming language:** Rust (edition 2024, MSRV 1.95 — bumped in K99)
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

See `.ai-factory/ARCHITECTURE.md` for detailed architecture guidelines (folder layout, dependency rules, tier communication, code examples, anti-patterns).

Pattern: **Three-Tier Layered + Cargo Workspace** — the project is organized into three strategic tiers, each realized as one or more Cargo crates so tiering is enforced mechanically by the dependency graph.

```
Tier C — Ecosystem    flui-widgets, flui-material, flui-cupertino, flui-theme,
                      flui-a11y, flui-navigator, third-party widget crates
Tier B — Framework    flui-framework (PLANNED — Phase II-F)
                      Widget + Key + State + BuildCx + Provider + reconciliation
Tier A — Engine       flui-core (single-crate runtime), flui-platform, flui-macros
```

The Engine tier (A) is the GPU/runtime substrate forked from `gpui-ce`. The Framework tier (B) is the Flutter-style developer-experience layer that does NOT yet exist — it is the central work item for the next strategic phase. The Ecosystem tier (C) is everything app authors and the community write on top.

## Architecture Notes

**flui-v2 is a hard fork of `gpui-ce`**, not a tracking fork. Upstream `gpui-ce` (and Zed's GPUI) became inactive on framework-level evolution; flui-v2 takes ownership of the trajectory and diverges as needed. Breaking changes from upstream are the entire point of the fork — there is no upstream-sync commitment, no semver compatibility with `gpui`, and no obligation to preserve GPUI's public API. Selected upstream fixes may be cherry-picked, but it is a one-way pull, not a two-way sync.

The project intentionally avoids replicating Flutter's deep internal layering (an earlier v1 attempt with `flui-foundation`/`flui-engine`/`flui-rendering`/etc. was abandoned). Instead the architecture replicates Flutter's **feature surface** on top of GPUI's single-level engine. The Engine tier deliberately stays single-crate (`flui-core`); architectural layering happens **on top of** the engine through the Framework tier, not inside it.

The Framework tier replaces Flutter's 4-tree internal model (Widget / Element / RenderObject / Layer) with **"2 structures + 1 cache"**: an immutable Widget config struct, the existing `flui-core::Element` runtime, and a flat `HashMap<ElementId, Box<dyn State>>` for surviving stateful data. Rust ownership lets us collapse Flutter's internal trees into a much smaller framework — this is the central justification for choosing Rust as the implementation language.

Per the current roadmap, Phase I (platform extraction, S01–S06) is FROZEN after S01 + S02a; S02b–S06 are deferred to Phase III when a real new-platform driver lands (iOS / Android / Web). Active work is split across two parallel tracks: Phase II — Engine completeness (S08 Semantics, S09 Canvas, S10 Filters, S12 Focus, S13 Text, S14 MediaQuery, S15 Assets), and Phase II-F — Framework layer (Widget / Key / State / BuildCx / Provider / reconciliation, spec series SF##). The new `flui-platform` skeleton exists at `crates/flui-platform/` but stays unpopulated until Phase III. The new `flui-framework` crate is to be created at the start of Phase II-F.

Authoritative architectural and migration context lives in `docs/superpowers/specs/` and `docs/superpowers/plans/`. Specialized review subagents (`flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, `rust-api-migration-auditor`) are configured in `.claude/agents/` and should be used proactively on changes touching the runtime or platform code.

## Non-Functional Requirements

- **Logging:** standard `log` / `tracing` ecosystem (configured per-crate); no project-wide logger config yet.
- **Error handling:** structured Rust `Result`/`Error` types; explicit `unimplemented!()` / `unreachable!()` sites in platform code are tracked by the roadmap (S01a inventory) and must be classified, not casually replaced.
- **Async safety:** Clippy enforces `smol::process::Command::*` over `std::process::Command::*` to avoid blocking the executor thread.
- **Determinism:** GPU work targets deterministic offscreen rendering for golden tests; `lock-checks` tooling and `lock-coverage-gaps.md` track regressions.
- **Platform parity:** macOS (Metal), Windows (Direct3D 11), Linux (wgpu + Wayland + X11) are all first-class targets; iOS/Android/WASM are roadmap items.
- **MSRV:** Rust 1.95 (edition 2024). Enforced via three synchronized locations: `Cargo.toml` `[workspace.package].rust-version`, `rust-toolchain.toml` `channel`, `clippy.toml` `msrv`. CI gate: standard jobs (check / clippy / test / format) honor the pin via `rust-toolchain.toml`; a non-blocking `forward-compat` job runs latest stable as an early-warning radar for upcoming Rust changes. flake.nix uses `fenix.latest` (intentionally divergent — Nix users get forward-compat radar; rustup users stay pinned to MSRV).
- **Spelling discipline:** `typos.toml` is enforced.
