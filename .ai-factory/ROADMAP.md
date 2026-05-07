# Project Roadmap

> Flutter-inspired, GPU-accelerated UI framework for Rust on top of `gpui-ce`. Phase I (platform extraction) is FROZEN after S01 + S02a; the active strategic direction is Phase II (Flutter-parity core subsystems), with parallel cross-cutting tracks for performance, architecture hygiene, testing infrastructure, and release readiness.

Authoritative spec lives in `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`. This file is the high-level milestone tracker — keep it in sync with the spec, with `git log`, and with new specs as they land.

Numbering convention:
- **S##** — feature specs (already in `docs/superpowers/specs/`)
- **P#** — performance & GPU optimizations
- **A#** — architecture & API hygiene
- **T#** — testing & quality infrastructure
- **R#** — release readiness & developer experience

## Milestones

### Phase I — Platform extraction (FROZEN after S01 + S02a)

- [x] **S01a.1 lock infrastructure** — xtask `check-stubs` / `check-platform-imports` subcommands, `.gitattributes`, test-support benchmark, lavapipe on Linux CI
- [x] **S01a.2 delete dead screen-capture code** — feature was referenced but never declared
- [x] **S01a.3 explicit re-export list for platform module** — replaces `pub use platform::*;` glob with curated list (~95–100 symbols)
- [x] **S01a.4 repair debug-mode Windows build** — 257 errors → 0; missing `Win32_Media` feature + glob imports cleaned up
- [x] **S01b wgpu headless renderer + golden infrastructure** — `WgpuContext::new_headless`, pipeline cache lift, `Bgra8Unorm` lock, golden harness, mac + Linux suites
- [x] **S01c behaviour pinning (non-rendering)** — event dispatch per input variant, focus/tab-stop, keyboard layout, clipboard, window lifecycle, real example smoke
- [x] **S01d extraction facades** — `WebWindowInner` `#[doc(hidden)]` facade, `PlatformScreenCaptureFrame` opaque newtype, submodule visibility strategy
- [x] **S02a flui-platform crate skeleton** — empty workspace member with minimal `Cargo.toml` + doc-only `lib.rs`; reserved slot for future Phase III work
- [ ] **S02b–S06 platform migration (DEFERRED)** — `Platform` trait flip and per-platform code moves (wgpu/Linux, macOS, Windows, Web). Re-opened only when a concrete Phase III driver (iOS / Android / Web) forces a real platform-abstraction boundary.

### Phase II — Flutter-parity core subsystems (active strategic direction)

- [x] **S07 GestureArena** — competing recognizers (tap, double-tap, long-press, drag, scale, horizontal/vertical drag), hit-test protocol [Gap B]
- [ ] **S08 Semantics protocol** — `SemanticsNode` tree, `SemanticsOwner`, actions, roles/hints/labels, hooks for `flui-a11y` [Gap F]
- [ ] **S09 Canvas facade** — unified `Canvas` API over `scene` + `path_builder`; `saveLayer`, clips, transforms, blend modes [Gap C]
- [ ] **S10 Image filters** — `ImageFilter` (blur, matrix), `ColorFilter`, `BackdropFilter`, `MaskFilter`. Depends on S09 [Gap C]
- [ ] **S11 Physics simulations** — `Spring`, `Friction`, `Gravity`, `ScrollPhysics` integrated with `AnimationController` [Gap E]
- [ ] **S12 Focus traversal** — directional traversal, `FocusTraversalPolicy`, `FocusScope` groups [Gap B]
- [ ] **S13 Text parity** — `StrutStyle`, `TextDecoration`, `FontFeatures`, `FontVariations`, selection rendering, IME composition preview [Gap D]
- [ ] **S14 MediaQuery completeness** — accessibility flags (highContrast, disableAnimations, accessibleNavigation), gestureSettings, SystemChrome [Gap H]
- [ ] **S15 Asset bundle** — resolution-aware variants, locale variants, structured manifest format [Gap I]

### Phase III — New platform embeddings (future)

- [ ] **S16 Headless renderer (cross-platform)** — wgpu-offscreen backend, reusable golden-test infrastructure
- [ ] **S17 iOS embedding** — UIKit + Metal + IMKit + UIAccessibility
- [ ] **S18 Android embedding** — JNI/NDK Surface + Choreographer + InputMethod + AccessibilityNodeProvider
- [ ] **S19 Web rendering** — wgpu → WebGPU/WebGL2, canvas integration, IME, clipboard API, fetch-based assets
- [ ] **S20 Desktop platform-gaps cleanup** — close remaining TODOs on Windows/Linux/macOS (IME edges, fractional scaling, wayland session lock); cross-check against S01 inventory

### Performance & GPU optimizations (cross-cutting)

- [ ] **P1 Frame-budget instrumentation** — `tracing` spans on paint / layout / animation hot paths, recorded budget assertions, optional `tracing-tracy` flamegraph integration
- [ ] **P2 Atlas eviction policy review** — measure occupancy + thrash on `mac/metal_atlas.rs` and `wgpu/wgpu_atlas.rs`; document the eviction strategy and the one acknowledged `unimplemented!()` in `metal_atlas.rs` (rare unsupported texture format)
- [ ] **P3 Path rasterization perf** — Criterion benchmark on `path_builder`, evaluate caching strategy and SIMD tessellation
- [ ] **P4 Text shaping cache profiling** — cold vs warm hit rates for cosmic-text + swash; document a per-frame budget and an LRU bound
- [ ] **P5 Pipeline cache metrics** — hit-rate counters around the S01b lift (pipeline cache moved into `WgpuContext`); export via `tracing` for headless and surface paths
- [ ] **P6 Animation tick efficiency** — eliminate per-frame allocations in `AnimationController`, profile with `dhat` heap profiler
- [ ] **P7 Async executor profiling** — `smol` executor under simultaneous animation + IO load; document task-queue depth and wake latency
- [ ] **P8 Build-time optimization** — sccache for CI, `mold`/`lld` linker on Linux, evaluate `cargo-chef` for Docker, consider per-crate `codegen-units` tuning beyond root profiles
- [ ] **P9 Binary-size audit** — `cargo-bloat` baseline on `examples/nav_demo`, identify monomorphization-bloat hot spots, decide on `dyn`-erasure trade-offs

### Architecture & API hygiene (cross-cutting)

- [x] **A1 Explicit `platform::*` re-exports** — done as part of S01a.3
- [ ] **A2 Audit remaining ~29 globs in `flui-core/src/lib.rs`** — explicitly out of scope of S01a.3 ("the ~29 other globs at `lib.rs` stay") and need their own pass before any future API stabilization
- [ ] **A3 Error-type unification** — define a project-wide error policy (per-crate `Error` enum vs `anyhow` boundary), consolidate ad-hoc `Box<dyn Error>` sites
- [ ] **A4 Tracing standardization** — choose `log` vs `tracing` per crate, define standard spans/fields/levels, add a workspace-level guideline doc
- [ ] **A5 Feature flag matrix discipline** — run `cargo hack check --feature-powerset` in CI; document required combinations for `screen-capture`-class features (avoid the S01a.2 class of landmines)
- [ ] **A6 `[workspace.dependencies]` migration** — consolidate version pinning at the workspace root (currently per-crate); enables single-PR upgrades for `wgpu`, `naga`, `windows`, `wayland-client`, etc.
- [ ] **A7 Interior-mutability surface reduction** — audit public APIs that expose `Rc<RefCell<…>>` (per the S01d auto-trait concern); prefer opaque newtypes when the auto-trait set must not be part of semver
- [ ] **A8 `#[non_exhaustive]` audit** — extend the S01a treatment of `PrimitiveBatch` to all public enums whose variants may grow (`CursorStyle`, input variants, scene primitive families)
- [ ] **A9 Crate-boundary review for `flui-core`** — identify files that belong in `flui-platform` once S02b unfreezes, and files (text system, media query) that may eventually become their own crate

### Testing & Quality infrastructure (cross-cutting)

- [ ] **T1 Code coverage in CI** — `cargo-llvm-cov` job, publish HTML report as artifact; later: integrate with Codecov / Coveralls
- [ ] **T2 `cargo-fuzz` targets** — fuzz `path_builder`, `keymap` parser, scene primitive iteration; add to CI as a scheduled job (not per-PR)
- [ ] **T3 Property-based tests with `proptest`** — layout invariants at the Taffy integration boundary, geometry round-trips, color-space conversions
- [ ] **T4 Criterion benchmark suite** — paint / layout / text shaping / animation tick; track regressions with `bencher.dev` or comparable
- [ ] **T5 Mutation testing pilot** — `cargo-mutants` on a focused module (`path_builder` is a good first target); decide whether to scale up
- [ ] **T6 Expand visual regression suite** — beyond S01b: input-dispatch goldens, animation-frame goldens, font-rendering goldens across all three desktop platforms

### Release readiness & DX (cross-cutting)

- [ ] **R1 crates.io publishing strategy** — decide which crates publish (`flui-core`, `flui-navigator`, `flui-macros`, ...), publish order, ownership, and whether `flui-core` is published as `flui-core` or remains git-only until Phase II completes
- [ ] **R2 `cargo-semver-checks` in CI** — gate `flui-core` and `flui-platform` public surface; ties into A2 and A8
- [ ] **R3 CHANGELOG.md** — adopt Keep a Changelog format; backfill from existing `git log` since project inception
- [ ] **R4 Release tooling** — `release-plz` or `cargo-release` for tag/version automation
- [ ] **R5 MSRV policy + CI job** — current MSRV is 1.85 (root `Cargo.toml`); add a CI job that pins to MSRV toolchain to catch drift
- [ ] **R6 `cargo-deny` workflow** — advisories, licenses, sources, bans; add `deny.toml` and a CI job
- [ ] **R7 CI matrix expansion** — add Windows debug build (per S01a.4 repair), macOS aarch64, scheduled (nightly) full-matrix runs; current CI is per-OS check/clippy/test/fmt only
- [ ] **R8 CONTRIBUTING.md** — workflow expectations, when to invoke each review subagent (`flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, `rust-api-migration-auditor`), commit message style, PR checklist
- [ ] **R9 mdbook user guide** — hosted on GitHub Pages: getting started, widget catalogue, navigator routing, theming, examples gallery
- [ ] **R10 Migration guide from `gpui-ce`** — formalize the `extern crate flui_core as gpui;` pattern shown in `README.md`; document expected upstream-sync cadence

### Out of scope (gated on roadmap completion)

- Higher-level widget crates (`flui-widgets`, `flui-material`, `flui-theme`)
- `flui-cli`, `flui-build`, `flui-test`, `flui-golden`, `flui-devtools`
- Dart VM / platform channels (we are native-only)
- Replicating Flutter's internal layer tree (GPUI's scene already solves this)
- DevTools / inspector / performance overlay (P1 instrumentation is a prerequisite, not a substitute)

## Completed

| Milestone | Date |
|---|---|
| S01a.1 lock infrastructure | 2026-04-13 |
| S01a.2 delete dead screen-capture code | 2026-04-13 |
| S01a.3 explicit re-export list for platform module (also closes A1) | 2026-04-13 |
| S01a.4 repair debug-mode Windows build | 2026-04-13 |
| S01b wgpu headless renderer + golden infrastructure | 2026-04-13 |
| S01c behaviour pinning (non-rendering) | 2026-04-13 |
| S01d extraction facades | 2026-04-13 |
| S02a flui-platform crate skeleton | 2026-04-13 |
| S07 GestureArena (competing recognizers, hit-test protocol, arena binding, settings, velocity tracker, demo, bench, properties) | 2026-05-07 |

## Cross-track dependencies

- **A2 → R2 → R1**: stabilizing the public surface (kill remaining globs) is a prerequisite for `cargo-semver-checks`, which is itself a prerequisite for confident crates.io publishing.
- **A6 → R7**: workspace-level dependency consolidation makes CI matrix expansion (especially feature-powerset jobs) tractable.
- **P1 → P2..P9**: frame-budget instrumentation gives the measurement substrate that the other perf items act on. Land P1 first.
- **T4 → P3..P7**: Criterion benchmark suite is the baseline that lets perf work show measurable wins.
- **S08 → S17, S18**: semantics protocol must land before mobile platforms can plug accessibility into it.
- **A4 + A3 → R8**: tracing + error-handling guidance feed CONTRIBUTING.md.
- **S01b lock infrastructure → T6**: visual regression expansion reuses the S01b harness; do not build a parallel one.

## Anti-goals for cross-cutting tracks

- ❌ Do not let perf milestones (P#) drive premature pessimization of API ergonomics.
- ❌ Do not treat A2 / A8 as license to rewrite the public surface in one big PR; each is a curated, reviewable change.
- ❌ Do not introduce R-track tooling (semver-checks, deny, release-plz) before the first publishable surface exists — premature gating is friction without value.
- ❌ Do not start S17/S18 (mobile) without S08 (semantics protocol) and S16 (headless renderer baseline).
