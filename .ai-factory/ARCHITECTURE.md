# Architecture: Layered + Cargo Workspace

## Overview

flui-v2 is structured as a **layered architecture** physically realized through a **Cargo workspace**. Each architectural layer is its own crate, and Cargo's dependency graph mechanically enforces the layering: a crate cannot depend on a higher layer because that would form a cycle. The model deliberately rejects deep DDD-style internal layering (an earlier v1 attempt with `flui-foundation`/`flui-engine`/`flui-rendering`/etc. was abandoned). Instead, the architecture replicates **Flutter's user-visible feature surface** on top of GPUI's single-level engine.

The active migration (S01–S06 of the flui-core roadmap) is moving platform-specific code out of `flui-core` and into a dedicated `flui-platform` crate so that the runtime has a clean dependency on a single platform abstraction layer rather than an in-tree platform module. Until that migration completes, `flui-core` temporarily owns both the runtime and its platform backends — this is acknowledged technical debt, not a target state.

## Decision Rationale

- **Project type:** GPU-accelerated UI framework (library), built on `gpui-ce`.
- **Tech stack:** Rust (edition 2024, MSRV 1.85), Cargo workspace, smol async runtime, wgpu/Metal/Direct3D 11, Taffy layout, cosmic-text/swash text.
- **Key factor:** A UI framework has a natural vertical: pixels → primitives → widgets → routing → app. Cargo crates make the layers physical, so dependency violations fail at `cargo build`, not at code review.
- **Alternative considered:** Clean Architecture was rejected — there is no business logic to invert dependencies around; the framework's "domain" is the rendering pipeline itself.
- **Alternative considered:** A flat modular monolith was rejected — it would lose the dependency-direction guarantees that the layered model gives for free.

## Folder Structure

```
flui-v2/
├── Cargo.toml                          # Workspace manifest (members, profiles, workspace lints)
├── crates/
│   ├── flui-platform/                  # Layer 1 — platform abstraction (skeleton, populated by S02–S06)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── flui-core/                      # Layer 2 — runtime, rendering, layout, input, executor
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs / app/           # App + Entity + Context
│   │       ├── element.rs / elements/  # Element tree
│   │       ├── scene.rs                # Scene primitives (GPU-reviewed)
│   │       ├── path_builder.rs
│   │       ├── animation/              # AnimationController, curves, tweens
│   │       ├── executor.rs             # Async executor on smol
│   │       ├── input.rs / interactive.rs / key_dispatch.rs / keymap/
│   │       ├── platform/               # ⚠️ migrating out → flui-platform per S02–S06
│   │       │   ├── mac/                # Metal backend
│   │       │   ├── windows/            # Direct3D 11 backend
│   │       │   ├── linux/              # x11 + wayland
│   │       │   ├── wgpu/               # shared wgpu backend
│   │       │   ├── web/ test/ visual_test.rs
│   │       └── ...
│   ├── flui-macros/                    # Procedural macros (derive Render, IntoElement, …)
│   ├── flui-widgets/                   # Layer 3 — widget library (planned)
│   ├── flui-a11y/                      # Layer 3 — accessibility / semantic tree (planned)
│   ├── flui-theme/                     # Layer 3 companion — theming (planned)
│   ├── flui-material/                  # Layer 3 companion — Material widgets (planned)
│   └── flui-navigator/                 # Layer 4 — type-safe routing
├── examples/                           # Layer 5 — application code
│   ├── nav_demo/
│   ├── material_demo/
│   └── animation_demo/
├── tooling/
│   └── lock-checks/                    # Tooling crate (not a layer member)
├── docs/superpowers/{specs,plans}/     # Design docs and plans
└── .ai-factory/                        # AI Factory project context
```

The workspace `[workspace.lints]` block in the root `Cargo.toml` is the single source of lint configuration; each member opts in via `[lints] workspace = true`. Workspace-wide Clippy denies `dbg_macro`, `redundant_clone`, `declare_interior_mutable_const`, and the `disallowed-methods` rules in `clippy.toml` push async-safe `smol::process` over `std::process`.

## Dependency Rules

The five layers form a strict downward dependency graph. Each crate may only depend on crates at lower layers (or sibling crates within the same layer when explicitly authorized).

| Layer | Crate(s) | May depend on |
|---|---|---|
| 5 — Application | `examples/*` | Layers 1–4 |
| 4 — Routing | `flui-navigator` | Layers 1–3 |
| 3 — Widgets / A11y | `flui-widgets`, `flui-a11y`, `flui-theme`, `flui-material` | Layers 1–2 (and sibling crates in Layer 3 when explicitly authorized) |
| 2 — Core runtime | `flui-core` | Layer 1, plus `flui-macros` (proc-macro support) |
| 1 — Platform abstraction | `flui-platform` | external crates only (wgpu, metal, windows, wayland-client, x11rb, ash, …) |
| — Macros | `flui-macros` | proc-macro toolchain only — no flui crates |
| — Tooling | `tooling/lock-checks` | not a layer member; isolated |

Allowed:
- ✅ `flui-navigator` → `flui-core` → `flui-platform`
- ✅ `flui-widgets` → `flui-core`
- ✅ `examples/nav_demo` → `flui-navigator` + `flui-core`
- ✅ `flui-core` → `flui-macros` (proc-macro consumer)

Forbidden:
- ❌ `flui-core` → `flui-widgets` (upward dependency — breaks layering)
- ❌ `flui-platform` → `flui-core` (would create the very cycle the migration is removing)
- ❌ `flui-macros` → any other `flui-*` crate (proc-macro crates must stay leaf)
- ❌ Adding new platform code under `crates/flui-core/src/platform/**` instead of `crates/flui-platform/` — this contradicts the active migration roadmap (S02–S06).
- ❌ Sibling-crate dependencies within Layer 3 without an explicit roadmap entry (e.g., `flui-material` → `flui-widgets` is acceptable; `flui-widgets` → `flui-material` is not).

Cargo enforces these rules mechanically: a forbidden dependency would form a cycle and `cargo build` would refuse to compile.

## Layer / Module Communication

- **Downward calls (allowed):** higher-layer crates use lower-layer types and traits directly through their public API (`pub use`, `pub` items).
- **Upward signaling (allowed):** lower-layer crates expose traits and event types; higher layers implement / observe them. Never have a lower layer call a higher layer directly.
- **Cross-layer types:** primitive geometry / styling types live in `flui-core` and are re-exported. Do not duplicate them in higher layers.
- **Public surface discipline:** per S01a3, `flui-core` re-exports are explicit. Do not introduce blanket `pub use crate::*` re-exports. The same convention applies to `flui-platform` as it grows.
- **Macros:** procedural macros from `flui-macros` are consumed by Layer 2+; macros must not reach into a specific user crate's types.

## Key Principles

1. **Cargo is the architecture.** A crate boundary is the architectural boundary. Don't bypass it with workspace-internal `path` shortcuts that hide a layer violation.
2. **The runtime stays single-level.** Do not re-introduce a v1-style multi-crate engine split (`-foundation`/`-engine`/`-rendering`/…). flui-v2 deliberately keeps GPUI's single-level engine.
3. **Platform code lives in `flui-platform`.** `crates/flui-core/src/platform/**` is in the process of being emptied. New backend code, new platform features, and any non-trivial platform fix must land in `flui-platform`, not grow the in-tree module.
4. **Explicit re-exports.** No `pub use crate::*`. Public surface is curated.
5. **Async-safe by default.** Use `smol`-based primitives. `clippy.toml` enforces `smol::process::Command::*` over `std::process::Command::*`.
6. **`unimplemented!()` and `unreachable!()` are tracked, not ornamental.** They are inventoried by S01a; classify (legitimate invariant guard vs trap-in-waiting) before touching.
7. **MSRV 1.85.** Do not rely on features requiring a newer toolchain.
8. **Determinism on the GPU path.** Offscreen / golden-test outputs must remain reproducible; coordinate with the `wgpu-gpu-reviewer` agent for any change in `crates/flui-core/src/platform/wgpu/**`, `scene.rs`, the Metal renderer, or the DirectX renderer.

## Code Examples

### Manifest layering (Cargo as the boundary)

```toml
# crates/flui-navigator/Cargo.toml — Layer 4 may depend on Layers 1-3.
[package]
name = "flui-navigator"
edition.workspace = true
rust-version.workspace = true

[dependencies]
flui-core = { path = "../flui-core" }       # ✅ downward dependency

[lints]
workspace = true
```

```toml
# crates/flui-platform/Cargo.toml — Layer 1 must NOT depend on flui-core.
[package]
name = "flui-platform"
description = "Platform abstraction layer for flui (skeleton — populated incrementally by migration specs S02b through S06)"

[dependencies]
# ✅ external platform crates only
# wgpu = { workspace = true }
# wayland-client = { workspace = true }
# ❌ flui-core = { path = "../flui-core" }   // would form a cycle and break layering
```

### Explicit, curated re-exports (per S01a3)

```rust
// crates/flui-core/src/lib.rs
//
// ❌ DO NOT do this:
// pub use crate::*;
//
// ✅ Curate the public surface explicitly:
pub use crate::app::App;
pub use crate::element::Element;
pub use crate::scene::Scene;
pub use crate::animation::AnimationController;
// ... and so on, item by item.
```

### Async-safe process spawning (Clippy-enforced)

```rust
// ❌ Denied by clippy.toml — blocks the executor thread:
// let _ = std::process::Command::new("cargo").status()?;

// ✅ Use the smol equivalent:
use smol::process::Command;
let _ = Command::new("cargo").status().await?;
```

### Adding a new platform feature (post-migration shape)

```rust
// crates/flui-platform/src/lib.rs
//
// New platform features land HERE, not under crates/flui-core/src/platform/**.
// flui-core then consumes the abstraction:
//
//   use flui_platform::{Window, Display, Renderer};
//
// Cargo enforces the direction: flui-platform never imports flui_core.
```

## Anti-Patterns

- ❌ **Adding new code under `crates/flui-core/src/platform/**`.** That tree is being emptied; new platform code goes into `crates/flui-platform/`.
- ❌ **Pulling `flui-core` into `flui-platform`.** This is the cycle the migration exists to prevent.
- ❌ **Re-creating v1's multi-crate engine split** (`flui-foundation` / `flui-engine` / `flui-rendering` / …). Lessons learned in v1: too much internal layering on top of a single-level engine.
- ❌ **Blanket `pub use crate::*`.** Public surfaces are curated explicitly.
- ❌ **Skipping layers in examples** (e.g., an example reaching into `flui-core::platform` internals instead of going through `flui-navigator`/`flui-widgets`). Examples are user-facing demos and should consume the same surface end users will.
- ❌ **`std::process::Command` for spawning.** Denied by `clippy.toml`; use `smol::process::Command`.
- ❌ **Silently deleting `unimplemented!()` / `unreachable!()` sites in platform code.** Classify them per the S01a inventory first.
- ❌ **Sibling-crate dependencies in Layer 3 without an explicit decision.** For example, do not let `flui-widgets` depend on `flui-material` — `flui-material` is the consumer of `flui-widgets`, not the other way around.
- ❌ **Bypassing review subagents.** Use `flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, and `rust-api-migration-auditor` proactively on the matching change types.
