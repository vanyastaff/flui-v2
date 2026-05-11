# AGENTS.md

> Structural map for AI agents working in this repository. Update whenever the project layout changes significantly.

## Project Overview

flui-v2 is a Flutter-inspired, GPU-accelerated UI framework for Rust. It is a **hard fork** of `gpui-ce` (the community edition of Zed's GPUI runtime); upstream became inactive on framework-level evolution and flui-v2 owns the trajectory now, diverging from upstream as needed. The strategic goal is a full UI ecosystem in Rust with Flutter-equivalent developer experience and ecosystem reach.

The architecture is organized into **three tiers**:

- **Tier A — Engine:** `flui-core` (single-crate runtime, forked from gpui-ce), `flui-platform` (skeleton, Phase III), `flui-macros` (proc macros).
- **Tier B — Framework:** `flui-framework` (PLANNED, Phase II-F) — Widget, Key, State, BuildCx, Provider, reconciliation. The Flutter-DX layer.
- **Tier C — Ecosystem:** `flui-widgets`, `flui-material`, `flui-cupertino`, `flui-theme`, `flui-a11y`, `flui-navigator`, third-party widget crates.

**Current status:** Phase I (platform extraction) is FROZEN after S01 + S02a — S02b–S06 are deferred to Phase III. **Active work is Phase 0-K Kernel Cleanup** (K-track) — repaying remaining architectural debt in `flui-core` (action globals, leaky coordinate-space type-safety, no layout cache, effect/frame ordering, etc.) that blocks a healthy Framework tier. K99, K15, K07, K05, K01, K02, and K03 are complete; the next critical-chain item is K04. Remaining chain: `K04`. Phase II (Engine completeness — S08 Semantics, S09 Canvas, S10 Filters, S12 Focus, S13 Text, S14 MediaQuery, S15 Assets) runs in parallel with K-track since most specs are additive. Phase II-F (Framework tier — spec series SF##) is **gated on K-track critical chain completion**. Done: S07 Gesture, S07.5b PointerEvent surface, S21 Animation, K99 MSRV, K15 Re-entrancy, K07 app ownership primitive, K05 Element lifecycle context objects, K01 Provider rewrite, K02 Element identity and Key, K03 Render to Build separation. See `.ai-factory/DESCRIPTION.md`, `.ai-factory/ROADMAP.md`, and `.ai-factory/RESEARCH.md` for full details.

## Tech Stack

- **Programming language:** Rust (edition 2024, MSRV 1.95, workspace resolver = "3")
- **Async runtime:** smol
- **GPU / graphics:** wgpu, naga, metal (macOS), Direct3D 11 (Windows), Wayland + X11 (Linux)
- **Layout:** Taffy
- **Text:** cosmic-text, swash, fontdue
- **Build:** Cargo + Nix flake (`flake.nix`)
- **Lint:** workspace Clippy config (`clippy.toml`, `Cargo.toml`)

## Project Structure

```
flui-v2/
├── crates/
│   ├── flui-core/        # GPU rendering, entity/element/view runtime, layout, platform backends
│   ├── flui-platform/    # Platform abstraction crate (skeleton — populated by S02–S06)
│   ├── flui-macros/      # Procedural macros (derive Render, IntoElement, etc.)
│   ├── flui-navigator/   # Type-safe routing: nested routes, transitions, guards, middleware
│   ├── flui-widgets/     # Widget library (planned)
│   ├── flui-a11y/        # Accessibility / semantic tree (planned)
│   ├── flui-theme/       # Theming (planned)
│   └── flui-material/    # Material widgets (planned)
├── examples/
│   ├── nav_demo/         # Navigation routing demo
│   ├── material_demo/    # Material widget demo
│   └── animation_demo/   # Animation system demo
├── tooling/
│   └── lock-checks/      # Repo tooling for lock-coverage regression checks
├── docs/
│   ├── superpowers/
│   │   ├── specs/        # Design docs (YYYY-MM-DD-<id>-<slug>-design.md)
│   │   ├── audits/       # Focused implementation/review audit artifacts
│   │   ├── migrations/   # User-facing migration guides for breaking changes
│   │   └── plans/        # Implementation plans paired with specs
│   ├── reports/          # Generated reports
│   ├── fixtures/         # Test fixtures
│   └── lock-coverage-gaps.md
├── .agents/              # Project-scoped skills installed for this repository
├── .ai-factory/          # AI Factory project context (config, description, rules, etc.)
├── .claude/              # Claude Code skills + agents installed for this project
├── .codex/               # Codex skills installed for this project
├── .github/              # GitHub workflows
├── .cargo/               # Cargo config
├── .mcp.json             # Project-level MCP server configuration
├── target/               # Build artifacts (ignored)
├── Cargo.toml            # Workspace manifest
├── Cargo.lock            # Locked dependency graph
├── clippy.toml           # Workspace Clippy config (disallowed-methods, etc.)
├── typos.toml            # Spelling check config
├── flake.nix             # Nix dev shell
├── README.md             # Project overview and quick start
└── LICENSE.md            # Apache-2.0
```

## Key Entry Points

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace manifest — members, profiles, workspace lints |
| `crates/flui-core/src/lib.rs` | Public surface of `flui-core` (entity system, runtime, platform backends) |
| `crates/flui-platform/src/lib.rs` | Public surface of the new `flui-platform` skeleton crate |
| `crates/flui-navigator/src/lib.rs` | Public surface of the routing crate |
| `crates/flui-macros/src/lib.rs` | Procedural macros entry point |
| `examples/nav_demo/src/main.rs` | Reference application demonstrating navigator + core |
| `clippy.toml` | Workspace-level Clippy config (disallowed-methods enforces `smol::process` over `std::process`) |
| `flake.nix` | Reproducible dev shell |

## Documentation

| Document | Path | Description |
|---|---|---|
| README | `README.md` | Project overview, three-tier architecture, quick start, build instructions, project status |
| LICENSE | `LICENSE.md` | Apache-2.0 license |
| MCP config | `.mcp.json` | Project-local MCP server configuration for filesystem and GitHub access |
| flui-core roadmap | `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` | Master roadmap for Phase I (platform extraction + Flutter-parity gaps) |
| K99 — MSRV bump to Rust 1.95 | `docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md` | First Phase 0-K spec; workspace MSRV pinned to 1.95 + clippy.toml + CI gate |
| K15 — Re-entrancy contract | `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md` | Second Phase 0-K spec; `flui_core::reentrancy` module with `ReentryError` + `ReentryMode`; `cx.defer` / `Window::defer` named as escape hatches |
| K07 — App ownership primitive | `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` | Third Phase 0-K spec; custom app borrow cell + structured re-entry errors |
| K05 — Element lifecycle context objects | `docs/superpowers/specs/2026-05-11-K05-element-context-object-design.md` | Fourth Phase 0-K spec; `LayoutCx`, `PrepaintCx`, and `PaintCx` replace raw lifecycle argument bundles |
| K01 — Provider rewrite | `docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md` | Fifth Phase 0-K spec; per-`Window` inherited registry, scoped lifecycle reads, provider invalidation, cached dependency replay |
| K02 — Element identity and Key | `docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md` | Sixth Phase 0-K spec; `Key`, normalized Local identity, value/global key substrate, identity stack manager |
| K03 — Render to Build separation | `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md` | Seventh Phase 0-K spec; `ElementBuilder`, `ElementBuildCx`, `BuildElement`, and `build_element` immutable engine recipe substrate |
| K07 migration guide | `docs/superpowers/migrations/K07-appcell-removal.md` | Breaking-change guide for K07 callers |
| K01 migration guide | `docs/superpowers/migrations/K01-provider-rewrite.md` | Breaking-change guide for migrating global provider reads to scoped inherited reads |
| K02 migration guide | `docs/superpowers/migrations/K02-element-identity-key.md` | Breaking-change guide for Local/Value/Global key identity and state/provider migration |
| K03 migration guide | `docs/superpowers/migrations/K03-render-build-separation.md` | Guide for `Render`, `RenderOnce`, `ElementBuilder`, keying build boundaries, and deferred Framework scope |
| Design specs | `docs/superpowers/specs/` | Per-task design documents (date-stamped) |
| Migration guides | `docs/superpowers/migrations/` | User-facing guides for breaking changes |
| Implementation plans | `.ai-factory/plans/` | Per-task implementation plans paired with specs (resolved via `paths.plans`) |
| Lock coverage gaps | `docs/lock-coverage-gaps.md` | Tracking of lock-behavior regression coverage |

## AI Context Files

| File | Purpose |
|---|---|
| `AGENTS.md` | This file — structural map for AI agents |
| `.agents/skills/` | Project-scoped skills directory; includes AI Factory skills plus external `github-actions-docs` and `rust-testing` |
| `.ai-factory/DESCRIPTION.md` | Full project description (overview, features, stack, architecture, NFRs) |
| `.ai-factory/ARCHITECTURE.md` | Architecture guidelines (generated by `/aif-architecture`) |
| `.ai-factory/config.yaml` | AI Factory configuration (language, paths, workflow, git, rules) |
| `.ai-factory/rules/base.md` | Project base rules (naming, modules, error handling, async, testing, review subagents) |
| `.mcp.json` | Project-local MCP server definitions (`filesystem`, `github`) |

## Specialized Review Subagents

These read-only review agents are installed in `.claude/agents/` and should be invoked proactively on the matching change types:

| Agent | When to use |
|---|---|
| `flui-arch-reviewer` | Specs in `docs/superpowers/specs/` or changes touching core runtime types (`App`, `Entity`, `Context`, `Window`, `Element`, `Scene`, `Platform` trait) |
| `migration-risk-adversary` | Migration specs (S02–S06) and any change moving/extracting/renaming/deleting >100 LoC |
| `wgpu-gpu-reviewer` | Changes under `crates/flui-core/src/platform/wgpu/**`, `crates/flui-core/src/scene.rs`, `crates/flui-core/src/platform/mac/metal_renderer.rs`, or `crates/flui-core/src/platform/windows/directx_renderer.rs`; shader/pipeline/offscreen rendering changes |
| `rust-api-migration-auditor` | Specs that promote `pub(crate)` → `pub`, add new public types, extract code into new crates, or modify Cargo feature flags |

## Agent Rules

- **Decompose chained shell commands.** Run each step separately so the user can audit and approve each one.
  - Incorrect (combined): `git checkout main && git pull`
  - Correct (decomposed): first `git checkout main`, then `git pull origin main`
- **Respect MSRV.** Do not use Rust language features that require a toolchain newer than 1.95. The MSRV is enforced via three places that must stay in sync: `Cargo.toml` `[workspace.package].rust-version`, `rust-toolchain.toml` `channel`, and `clippy.toml` `msrv`. Drift between them is a bug.
- **Prefer modern Rust idioms unlocked by MSRV 1.95** (these ARE allowed and recommended):
  - `async fn` in traits (AFIT) and `-> impl Trait` in traits (RPITIT) — use these instead of `Box<dyn Future>` / `Box<dyn Trait>` in trait return types when possible.
  - Edition-2024 lifetime captures (`-> impl Trait + use<'_>`) — express precise lifetime relationships in async/iterator return types without manual elision.
  - Async closures (`async |...| { ... }`) — for callback-heavy APIs that need to await.
  - `let-chains` (`if let Some(x) = ... && cond && let Some(y) = ...`) — collapse nested matches in reconciliation / lookup code.
  - `std::sync::OnceLock` (1.70+) and `std::sync::LazyLock` (1.80+) — use instead of pulling in the `once_cell` crate. For single-threaded contexts: `std::cell::OnceCell` (1.70+) and `std::cell::LazyCell` (1.80+).
  - `unsafe extern "C"` blocks (1.82+) — required syntax in edition 2024 for FFI.
  - `#[diagnostic::on_unimplemented]` — author better trait-bound error messages on Framework-tier traits.
- **Do not silently delete `unimplemented!()` / `unreachable!()` sites in platform code.** They are tracked by the S01a roadmap inventory and must be classified before being touched.
- **Do not grow `crates/flui-core/src/platform/**`.** New platform code goes into `crates/flui-platform/` per the active migration roadmap.
- **Use `smol::process::Command`, not `std::process::Command`.** Enforced by `clippy.toml`.
- **No `dbg!`, no `redundant_clone`.** Workspace Clippy config denies these.
