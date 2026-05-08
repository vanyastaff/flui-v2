# flui-v2 — Base Project Rules

> Auto-detected conventions from codebase analysis. Edit as needed.

## Naming Conventions

- **Crates:** `flui-<area>` kebab-case (`flui-core`, `flui-platform`, `flui-navigator`, `flui-widgets`, `flui-a11y`, `flui-theme`, `flui-material`, `flui-macros`).
- **Lib targets:** snake_case mirroring the crate (`flui_core`, `flui_platform`, `flui_navigator`, ...).
- **Modules / files:** snake_case (`metal_renderer.rs`, `path_builder.rs`, `key_dispatch.rs`).
- **Types / traits / enums:** UpperCamelCase (`App`, `Entity`, `Window`, `Element`, `AnimationController`).
- **Functions / methods / variables:** snake_case.
- **Constants / statics:** `SCREAMING_SNAKE_CASE`.
- **Examples:** snake_case folder names under `examples/` (`nav_demo`, `material_demo`, `animation_demo`).

## Module Structure

- Cargo workspace (`resolver = "3"`) with `crates/` for libraries, `examples/` for runnable demos, `tooling/` for repo tools (`tooling/lock-checks`).
- Layered model (per `README.md`): platform backends → `flui-core` → widgets/animate/a11y → `flui-navigator` → application.
- Active migration: platform backends are being extracted from `crates/flui-core/src/platform/**` into `crates/flui-platform/` per the S01–S06 roadmap. New platform code goes into `flui-platform`; do not grow `flui-core/src/platform/**` further without an explicit roadmap reason.
- `flui-core` re-exports are explicit (per S01a3). Do not introduce blanket `pub use crate::*` re-exports.
- Specs live in `docs/superpowers/specs/`, plans in `docs/superpowers/plans/`. New design docs follow the `YYYY-MM-DD-<id>-<slug>-design.md` filename pattern.

## Error Handling

- Use idiomatic Rust `Result<T, E>` with project-defined error enums; do not paper over runtime stubs.
- `unimplemented!()`, `todo!()`, and `unreachable!()` sites are tracked by the S01a inventory and must be classified (legitimate invariant guard vs trap-in-waiting) before being touched. Do not silently delete or replace them.
- New runtime stubs in `flui-core` or `flui-platform` require a tracked entry in the migration inventory.

## Async / Concurrency

- Async runtime is `smol`. Prefer `smol::process::Command` over `std::process::Command` — Clippy enforces this via `clippy.toml` (`disallowed-methods`).
- `dbg!` is denied workspace-wide (`workspace.lints.clippy.dbg_macro = "deny"`); `redundant_clone` and `declare_interior_mutable_const` are also denied.
- Do not introduce blocking calls on the executor thread without an explicit reason.

## Logging

- Use the `log` / `tracing` ecosystem at the crate level. There is no project-wide logger configuration yet — do not invent one without a roadmap entry.
- Avoid `println!` / `eprintln!` outside of examples and tooling.

## Testing

- Cargo's built-in test harness. Doctests are disabled on the platform skeleton crate (`flui-platform/Cargo.toml: doctest = false`); follow the same pattern for skeleton crates being populated incrementally.
- GPU-bearing tests should be deterministic and use the offscreen / golden-test path; coordinate with the `wgpu-gpu-reviewer` agent for changes under `crates/flui-core/src/platform/wgpu/**`, `crates/flui-core/src/scene.rs`, `crates/flui-core/src/platform/mac/metal_renderer.rs`, or `crates/flui-core/src/platform/windows/directx_renderer.rs`.
- `tooling/lock-checks` and `docs/lock-coverage-gaps.md` exist to detect regressions in lock behavior — keep them green.

## Spelling and Style

- `typos` is enforced via `typos.toml`.
- Workspace-wide Clippy config in root `Cargo.toml` (`[workspace.lints.clippy]`) sets the baseline; per-crate `[lints] workspace = true` opts in.
- MSRV: Rust **1.85** (edition **2024**). Do not use features that require a newer toolchain.

## Review Subagents

Use these read-only review agents proactively on relevant changes (defined in `.claude/agents/`):

- `flui-arch-reviewer` — for any spec in `docs/superpowers/specs/` or any change touching core runtime types (`App`, `Entity`, `Context`, `Window`, `Element`, `Scene`, `Platform` trait).
- `migration-risk-adversary` — for migration specs (S02–S06) and any code change moving/extracting/renaming/deleting >100 LoC.
- `wgpu-gpu-reviewer` — for any spec or code change touching wgpu/Metal/DirectX renderers, scene, shader modules, pipeline cache, or offscreen rendering.
- `rust-api-migration-auditor` — for any spec that promotes `pub(crate)` to `pub`, introduces new public types, extracts code into a new crate, or modifies Cargo feature flags.
