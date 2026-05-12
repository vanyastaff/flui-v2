# Project Roadmap

> Flutter-inspired, GPU-accelerated UI framework for Rust. **Hard fork** of `gpui-ce` — upstream became inactive on framework-level work; flui-v2 owns the trajectory now and diverges as needed.
>
> Architecture is organized into three tiers (see `.ai-factory/ARCHITECTURE.md`):
> - **Tier A — Engine:** `flui-core`, `flui-platform`, `flui-macros`.
> - **Tier B — Framework:** `flui-framework` (PLANNED, Phase II-F) — Widget / Key / State / BuildCx / Provider / reconciliation.
> - **Tier C — Ecosystem:** `flui-widgets`, `flui-material`, `flui-cupertino`, `flui-theme`, `flui-a11y`, `flui-navigator`, third-party crates.
>
> Phase I (platform extraction) is FROZEN after S01 + S02a. **Active strategic work begins with Phase 0-K (Kernel Cleanup)** — fixing foundational issues in `flui-core` (broken Provider, no Widget Identity, `Render::&mut self` semantics, effect/frame contract, Element param explosion, action globals, coordinate-space type-safety, layout cache). K99, K15, K07, K05, K01, K02, K03, and K04 are all complete; the Phase 0-K critical chain is fully landed. Phase II-F (SF03/SF04/SF05) can now begin planning. The Phase 0-K critical chain is now complete. The Framework tier (Phase II-F) and remaining Engine completeness (Phase II) are unblocked. Cross-cutting tracks (performance, architecture hygiene, testing infrastructure, release readiness) run continuously.

Authoritative spec for Phase II (Engine track) lives in `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`. The Phase 0-K and Phase II-F (Framework) spec series will live alongside in `docs/superpowers/specs/` as they land. This file is the high-level milestone tracker — keep it in sync with the specs, with `git log`, and with new specs as they land.

Numbering convention:
- **K##** — Kernel cleanup specs (Phase 0-K — flui-core architectural debt repaid before Framework tier work)
- **S##** — Engine feature specs (already in `docs/superpowers/specs/`)
- **SF##** — Framework tier specs (Phase II-F — Widget/Key/State/BuildCx/etc., gated on K-track critical chain)
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

### Phase 0-K — Kernel Cleanup (gates Phase II-F; runs in parallel with Phase II additive work)

> **Why this phase exists:** an architectural audit of `flui-core` (recorded in `.ai-factory/RESEARCH.md`) identified 24+ issues spanning critical/high/medium priority that block a healthy Framework tier. The Provider system is fundamentally broken (thread-local global, no reactivity, fragile push/pop). There is no Widget identity / `Key` mechanism. `Render::&mut self` makes widgets mutable owners of state, contradicting Flutter's pure-build model. Effect/frame ordering is undefined. The `Element` trait has 6-7 args per method. `AppCell = RefCell<App>` is marked "remove after stabilization". Action system is global statics via `inventory`. Re-entrancy contract is undocumented. `Rc<RefCell<…>>` lurks on hot paths. Coordinate-space type-safety is leaky.
>
> Building Framework on top of these is constructing on cracks. Phase 0-K repays the debt first.
>
> **Critical chain (sequential — gates Phase II-F start):**
> `K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04`
>
> **Internal-org track (parallel after K05):** K06, K08, K10, K11
>
> **Independent track (any order):** K12, K13, K14, K16, K17, K20, K21, K22
>
> **Hygiene track (continuous, parallel slots):** K90 — K98
>
> **Done criteria for Phase 0-K:** all critical-chain specs land; `cargo test --workspace` green; bench harness shows no regression > 5% on tracked metrics; `flui-arch-reviewer` audit pass on the cleaned `flui-core`.

#### K-track: critical chain (sequential)

- [x] **K99 MSRV bump to Rust 1.95+** — workspace Cargo.toml, rust-toolchain.toml, CI matrix update. Unlocks: AFIT + RPITIT + edition-2024 lifetime captures (allow `Widget::build(&self) -> impl Widget` without `Box<dyn>`), async closures stable, `let-chains` stable, `std::sync::{OnceLock, LazyLock}` stable. Single-PR mechanical change. Spec: `docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md`. **Prerequisite for all subsequent K-specs.**
- [x] **K15 Re-entrancy contract** — document and enforce semantics for `update_window` inside `update_window`, `update_entity` inside callback, `setState` inside `did_update_widget`. `ReentryError` (`#[non_exhaustive]`) + `ReentryMode { Strict, Loose }` published in `flui_core::reentrancy`. Same-window re-entry returns `Err`; same-entity re-entry panics with structured Display (trait signature can't widen); `EntityMap::double_lease_panic` unified to use the same Display so multi-entity cycles `A → B → A` produce one message. `Window::prompt` widens to `Result<Receiver, ReentryError>`; `AsyncWindowContext::prompt` widens to `anyhow::Result<Receiver>` (was silently swallowed). `cx.defer` / `Window::defer` are the documented queue escape hatches — no new `Effect` variant. `PanicLikeUpstream` deferred to K07. 11 new tests (6 type-level, 5 behavioral via `TestApp`); 344 lib tests pass. Spec: `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md`. Plan: `.ai-factory/plans/feature-K15-reentrancy-contract.md`. Second spec in critical chain. Unblocks K07.
- [x] **K07 AppCell removal — token-based borrow model** — replaces `AppCell = RefCell<App>` (marked "remove after stabilization") with `flui_core::app::cell::AppCell`, a hand-rolled `UnsafeCell<App>` + `BorrowState` primitive returning `ReentryError` directly. Preserves doc-hidden `AppCell` / `AppRef` / `AppRefMut` spelling; removes `BorrowMutError` conversion and `TRACK_THREAD_BORROWS`; adds panic-restoration guards, property tests, scoped Miri, CI Miri jobs, and acquire/release bench. Closes E3 from `docs/promt.md`. Third Phase 0-K spec; unblocks K05.
- [x] **K05 Element trait → context object** — `&mut LayoutCx<'_>` / `&mut PrepaintCx<'_>` / `&mut PaintCx<'_>` replace 6-7-arg method signatures. Adds documented lifecycle context accessors, derived/nested context helpers, `AnyElement` context traversal, built-in element migration, focused lifecycle tests, and a migration guide. Closes E5, E6 from `docs/promt.md`. **API-BREAKING** for every custom Element. Unblocks K01-K04 by giving them clean borrow surfaces. Spec: `docs/superpowers/specs/2026-05-11-K05-element-context-object-design.md`. Plan: `.ai-factory/plans/feature-K05-element-context-object.md`. Migration: `docs/superpowers/migrations/K05-element-context-object.md`.
- [x] **K01 Provider rewrite — per-Window InheritedRegistry, reactive** — replaces `provider/stack.rs` thread-local global with a per-`Window` `InheritedRegistry`, scoped lifecycle reads (`read_inherited` / `inherit`), provider scope identity, value-change invalidation, cached-view dependency replay, removal cleanup, migration guide, and provider-focused tests. Closes E1 from `docs/promt.md`. **API-BREAKING.** Spec: `docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md`. Plan: `.ai-factory/plans/feature-K01-provider-rewrite.md`. Migration: `docs/superpowers/migrations/K01-provider-rewrite.md`.
- [x] **K02 Element identity & Key** — stable Tier-A identity substrate with `Key::{local,value,global}`, `ValueKey`, `GlobalKey`, normalized `ElementId::Local(LocalElementId)`, internal `ElementIdStack` occurrence tracking, debug duplicate sibling-key diagnostics, deferred-draw stack snapshots, state/provider key convergence, focused identity/state/provider tests, and migration guide. Public stateless element cache wrappers and cross-tree GlobalKey moves are deferred to SF02/SF05. Spec: `docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md`. Plan: `.ai-factory/plans/feature-K02-element-identity-key.md`. Migration: `docs/superpowers/migrations/K02-element-identity-key.md`. **API-BREAKING.** Unblocks K03 and SF01/SF02 substrate work.
- [x] **K03 Render → Build separation** — keeps existing `Render::render(&mut self)` as the mutable entity-backed Engine view trait, preserves `RenderOnce` / `Component<C>` compatibility, and adds the narrow `ElementBuilder` / `ElementBuildCx` / `BuildElement` substrate for immutable engine recipes built from `&self`. No `flui-framework` crate, final `Widget`, reconciliation, dirty-list, `setState`, or pure-build roots land in K03. Spec: `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md`. Plan: `.ai-factory/plans/feature-k03-render-build-separation.md`. Migration: `docs/superpowers/migrations/K03-render-build-separation.md`. Validation: `cargo fmt --check`, `cargo test -p flui-core`, `cargo test -p flui-macros`, Tier C compile checks, example checks, and `cargo test --workspace`. Seventh Phase 0-K spec; unblocks K04 and gives SF01/SF07 a clean render/build boundary.
- [x] **K04 Effect / Frame contract** — typed seven-phase pipeline (`PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame`) with `App::run_frame` entry, App-level `FrameClock`, placement-aware `Effect::Defer { placement, callback }`, advisory per-phase deadlines + `EffectFlush` break-and-requeue, panic-safe `abort_frame_after_panic`, `AnimationController::value` per-frame caching, `Window::on_pre_frame` (renamed) / `Window::on_post_frame` (new) + App-level mirrors, idempotent `Window::request_animation_frame` via `Cell<bool>`, `SmallVec`-on-Window callback storage, and `TestApp::advance_frame` test driver. Closes the kernel-audit "effect/frame ordering undefined" blocker. K15 contract unchanged: `cx.defer` remains the only sanctioned re-entry escape. Eighth and final Phase 0-K critical-chain spec; unblocks Phase II-F (SF03/SF04/SF05) planning. Spec: `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md`. Plan: `.ai-factory/plans/feature-K04-effect-frame-contract.md`. Migration: `docs/superpowers/migrations/K04-effect-frame-contract.md`. Validation: 417 `cargo test -p flui-core --lib --features test-support` pass, `cargo fmt --check` clean, cross-crate `cargo check` clean.

#### K-track: internal organization (parallel after K05)

- [ ] **K06 Window decomposition + ownership split** — split `window.rs` (6123 lines, 222 pub methods) into `window/{lifecycle,layout,paint,hit_test,dispatch,focus,state,frame,actions}.rs`. Beyond cosmetic — split Window's monolithic borrow domain into `BuildOwner` / `PipelineOwner` / `SemanticsOwner` (Flutter-style independent owners). Closes E13 from `docs/promt.md`. **API-BREAKING** for any code depending on Window field access patterns.
- [ ] **K08 Action subtree dispatcher** — replace global `inventory::collect!` action registry with per-subtree `Actions(actions: {...}, child: ...)` model. Per-Window and per-Entity actions become possible. Plugin extensibility. Closes E3 partially. **API-BREAKING** for action API consumers.
- [ ] **K10 Style decomposition** — replace 38-flat-field `Style` with composition (`LayoutStyle`, `SpacingStyle`, `BoxDecoration`, `TextStyle`, `EffectsStyle`, `InteractionStyle`). Cache key per sub-struct enables fine-grained diffing. Closes E11 from `docs/promt.md`. **API-BREAKING** for style consumers.
- [ ] **K11 Hit-test arena** — replace `FxHashMap<HitboxId, …>` with `Vec<HitTestEntry>` indexed by `HitboxId(u32).0`. O(1) without hash. Closes E4 from `docs/promt.md`.

#### K-track: independent items (any order)

- [ ] **K12 Drop order codification + entity cycle detection** — codify `App` field ordering with rationale; debug-build assertion for `Entity<T>` cycles via cross-Entity Weak refs (audit-driven). Closes E13's drop-ordering hand-control concern.
- [ ] **K13 Arena allocator audit** — assess 16 `unsafe` blocks (12 in `arena.rs`, 2 in `window.rs`, others). Decision: keep custom unsafe (current trade-off) vs migrate to `bumpalo`/`typed_arena` (fewer unsafe, similar perf). Document the chosen path. Run `wgpu-gpu-reviewer` if scene-arena interaction touched.
- [ ] **K14 Subscription backpressure + bounds** — bound `SubscriberSet` by max-subscribers; add metrics for high-frequency events (mouse_move). Low priority.
- [ ] **K16 Coordinate-space type-safety** — replace bidirectional `From<f32> for Pixels` with `Pixels::new(f32)` ctor + `into_f32()`; remove `impl From<DevicePixels> for ScaledPixels` (requires scale factor); add sealed conversion traits. Closes audit-finding B (geometry.rs:3012, 2772, 2784).
- [ ] **K17 Test harness simplification** — `flui_core::testing` module exposing `WidgetTester`-style API: spin up minimal App, mount widget, query layout tree, simulate events, assert. Reduces unit-test cost from "spin up Application::test()" to ~5 lines. Closes audit-finding E.
- [ ] **K20 Layout cache** — cache `Taffy` results keyed by `hash(LayoutStyle + SpacingStyle + Constraints)`. Skip Taffy for unchanged subtrees. Closes E7 from `docs/promt.md` and audit-finding C.
- [ ] **K21 Text-shape cache audit + LRU** — verify what cosmic-text shaping cache covers; add LRU bound; per-frame budget assertion. Closes audit-finding D.
- [ ] **K22 Inspector intro API** — read-only tree traversal trait (`InspectableElement`). No UI yet, just the substrate that future inspector / DevTools will hook into. Cheap now, expensive to retrofit later. Closes audit-finding F.

#### K-track: hygiene (parallel slots, continuous)

- [ ] **K90 Rebrand "GPUI" → "flui"** — 157 mentions across 25 files including public docstrings (`lib.rs:90, 269`, `prelude.rs:1`) and `_ownership_and_data_flow.rs` doctests using `gpui_platform::application()` (which doesn't exist). Closes E16 from `docs/promt.md`. Strategic, not cosmetic — signals fork autonomy.
- [ ] **K91 29 globs → explicit re-exports** — continuation of S01a.3 precedent. Closes E17 from `docs/promt.md` and roadmap A2.
- [ ] **K92 derive_more 0.99 → 2.x** — outdated dependency (2021); used in 10 files. Closes E18 from `docs/promt.md`.
- [ ] **K93 TODO/FIXME/dead_code triage** — 47 TODO/FIXME markers + 13 `#[allow(dead_code|unused)]`. Convert to GitHub issues or fix immediately. Closes E19 from `docs/promt.md`.
- [ ] **K94 Prelude expansion** — add `Pixels`, `px`, `point`, `size`, `Hsla`, `rgb`, `rgba`, `SharedString` to existing trait re-exports. Closes E20 from `docs/promt.md`.
- [ ] **K95 with_context().unwrap() → expect helpers** — closes E21 from `docs/promt.md`.
- [ ] **K96 unwrap_or_else(|| panic!(...)) → expect** — closes E22 from `docs/promt.md`.
- [ ] **K97 scene.rs missing_docs → real docs** — closes E23 from `docs/promt.md`.
- [ ] **K98 `_ownership_and_data_flow.rs` rewrite** — fix broken doctests using non-existent `gpui_platform::application()`. Document is rendered in rustdoc (`#[cfg(doc)] pub mod _ownership_and_data_flow`) — currently embarrassing. Closes audit-finding #24.

### Phase II — Flutter-parity core subsystems (Engine completeness; runs alongside Phase 0-K)

- [x] **S07 GestureArena** — competing recognizers (tap, double-tap, long-press, drag, scale, horizontal/vertical drag), hit-test protocol [Gap B]
- [x] **S07.5 GestureArena — T15 wiring follow-up** — `RecognizerLifecycle` extensibility seam, LongPress arena back-channel, DoubleTap arena hold/release, per-window settings flow, `MouseExit → Removed` semantics, gesture-state consolidation, `test-support` decoupling, end-to-end integration test, recognizer-extension contributor doc
- [x] **S07.5b GestureArena — pre-roster cleanup (breaking changes)** — `PointerEvent` surface upgrade (`pressure: Option<PressureSample>`, `provenance` enum, `timestamp` / `source_timestamp` split, three new `PointerKind` variants, sibling `PointerPanZoomEvent`), hit-test transform substrate (`Affine2`, `HitTestEntry.transform`, `HitTestScope` RAII), `DeliveredEvent<'_>` recognizer-side wrapper with `local_position`, unified `set_arena_back_channel(pid, bc, idx)` hook + per-pointer LongPress storage, `hold_count: u32` arena counter, `AllowedButtonsFilter` newtype + per-recognizer fields with `register_recognizer`-time gating (Decision D10), `CHANGELOG.md` introduction. Prerequisite for S07.6 recognizer roster expansion.
- [ ] **S08 Semantics protocol** — `SemanticsNode` tree, `SemanticsOwner`, actions, roles/hints/labels, hooks for `flui-a11y` [Gap F]
- [ ] **S09 Canvas facade** — unified `Canvas` API over `scene` + `path_builder`; `saveLayer`, clips, transforms, blend modes [Gap C]
- [ ] **S10 Image filters** — `ImageFilter` (blur, matrix), `ColorFilter`, `BackdropFilter`, `MaskFilter`. Depends on S09 [Gap C]
- [ ] ~~**S11 Physics simulations**~~ — **subsumed by S21** (Phases 0/4/6 cover `Spring`/`Friction`/`Gravity`/`BoundedFriction` integration with `AnimationController`; `ScrollPhysics` deferred to a future scrollable-views spec) [Gap E]
- [x] **S21 Animation Flutter parity** — Trait-shaped `Animation<T>` + listener mixins + `Ticker`/`Clock` injection + full `Curve` trait family + `Curves` catalogue + `Animatable<T>` + complete Tween family + `TweenSequence` + combinators (`AlwaysStopped`, `Proxy`, `Reverse`, `Compound`/`Min`/`Max`/`Mean`, `TrainHopping`) + `CurvedAnimation` + controller polish (`animate_to`, `animate_back`, `fling`, `velocity` 3-branch, `AnimationBehavior`, `AnimationStyle`, `BoundedFrictionSimulation`). `Animation` struct renamed to `ElementAnimation` (S21 phase 0a, breaking). `flui-animate` skeleton deleted (S21 phase 5); widget-layer animation primitives deferred to the existing `flui-widgets` crate as a future S21-followup. Plan: `.ai-factory/plans/animation-flutter-parity.md`. Partially closes A2 (one re-export glob removed in phase 0.3) and A8 (`AnimationStatus` + `AnimationBehavior` are `#[non_exhaustive]`). Cross-track: feeds S08 (semantics needs `Animation<T>` for accessibility-driven muting via `MediaQueryData.disableAnimations`) and S14 (the `AnimationBehavior::Preserve` integration lands when MediaQuery's `disableAnimations` flag is wired). Deferred follow-ups: 2D/Catmull-Rom curves (phase 1.5), `repeat()` overload extension + `ClampedSimulation` (phase 4.4/4.8b), criterion benches + animation-frame goldens + proptest sweep (phase 6), Flutter API reference dump + mdbook chapter (phase 1 / R9).
- [ ] **S12 Focus traversal** — directional traversal, `FocusTraversalPolicy`, `FocusScope` groups [Gap B]
- [ ] **S13 Text parity** — `StrutStyle`, `TextDecoration`, `FontFeatures`, `FontVariations`, selection rendering, IME composition preview [Gap D]
- [ ] **S14 MediaQuery completeness** — accessibility flags (highContrast, disableAnimations, accessibleNavigation), gestureSettings, SystemChrome [Gap H]
- [ ] **S15 Asset bundle** — resolution-aware variants, locale variants, structured manifest format [Gap I]

### Phase II-F — Framework tier (Flutter developer experience)

> **GATING:** Phase II-F starts only after the Phase 0-K critical chain (K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04) lands. SF01 in particular depends on K01 (Provider), K02 (Key + identity), K03 (Render/build separation), K05 (Element ctx-object). Starting Framework before kernel cleanup means rebuilding it after — wasted work.
>
> **Goal:** Build the `flui-framework` crate (Tier B) that makes flui feel like Flutter for app authors. Currently the project has Engine (Tier A) and skeleton Ecosystem crates (Tier C), but no Framework tier — app code reaches into engine primitives directly. This track creates the missing layer.
>
> Success criterion: a third-party widget crate can be implemented against the stable `flui-framework` public API without touching `flui-core` internals. This is the operational definition of "Flutter ecosystem parity" for flui-v2.
>
> Architectural invariant: "**2 structures + 1 cache, not 4 trees**" (see `.ai-factory/ARCHITECTURE.md` §"Framework Tier Internals"). Widget = immutable config; Element = existing flui-core runtime; State = flat `HashMap<ElementId, Box<dyn State>>`. No RenderObject as separate tree, no Layer tree (Scene already in Engine).

- [ ] **SF01 Widget + Key trait** — `Widget` trait (immutable, `&self` build), `StatefulWidget` trait, Framework re-exports/wrappers over the K02 `flui_core::Key` substrate, `derive(Widget)` macro skeleton in `flui-macros`. Establishes the public surface of `flui-framework`. Depends on: K02 (identity & Key), K03 (Render/build separation), K05 (Element ctx-object).
- [ ] **SF02 Reconciliation algorithm** — sibling matching by `(TypeId, Key)`, `did_update_widget` / `dispose` lifecycle hooks, position-fallback when keys absent, duplicate-key collision handling (debug panic / release fallback). O(siblings) per level invariant. Depends on: SF01, K15 (re-entrancy contract — reconcile may trigger setState).
- [ ] **SF03 BuildCx + Provider** — `BuildCx<'_>` context object, `read<T>()` / `inherit<T>()` distinction, per-Window `InheritedRegistry`. Provider mechanism itself is built in K01; SF03 wraps it with `BuildCx` ergonomics. Depends on: SF01, K01 (Provider rewrite).
- [ ] **SF04 State<W> + StateMap** — `WidgetState<W>` trait with `build` / `did_update_widget` / `dispose`, flat `HashMap<ElementId, Box<dyn State>>` with reuse semantics, `Entity<S>` interop for state that wants engine-level reactivity. Depends on: SF01, SF02, K07 (AppCell removal).
- [ ] **SF05 setState + dirty-list + rebuild scheduling** — `cx.handler(...)` / explicit `setState` API, dirty propagation through the App effect queue, rebuild ordering invariants, allocation-free hot path. Depends on: SF03, SF04, K04 (Effect/Frame contract).
- [ ] **SF06 InheritedWidget analog** — `Theme`, `MediaQuery`, `DefaultTextStyle`, `Localizations` patterns built on SF03's Provider. Cross-track: feeds S14 (MediaQuery completeness) and Tier C theme work. Depends on: SF03.
- [ ] **SF07 Widget → Element compilation adapter** — Framework-tier adapter mounting Widget tree onto Engine's Element tree. Distinct from the existing `Component<C: RenderOnce>` (Engine substrate, one-shot). The adapter lives in `flui-framework`, not `flui-core`. Depends on: SF01–SF05.
- [ ] **SF08 Async widgets** — `StreamBuilder<T>`, `FutureBuilder<T>`, with cancellation tied to `dispose`. Depends on: SF04, SF05.

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
- [ ] **R10 One-way port guide from `gpui-ce`** — formalize the `extern crate flui_core as gpui;` pattern shown in `README.md` as a **one-way migration aid** for porting Zed-style code to flui, NOT a sync mechanism. Document the divergence: API differences, removed APIs, the hard-fork posture. Selected upstream fixes may be cherry-picked but flui-v2 makes no upstream-sync commitment.

### Out of scope (gated on roadmap completion)

- **Tier C ecosystem populating** (`flui-widgets`, `flui-material`, `flui-cupertino`, `flui-theme` build-out beyond skeletons) — gated on Phase II-F (Framework tier) reaching SF05 minimum, since widgets need Widget+State+setState to be implementable.
- `flui-cli`, `flui-build`, `flui-test`, `flui-golden`, `flui-devtools`
- Dart VM / platform channels (we are native-only)
- Replicating Flutter's internal 4-tree model (Widget/Element/RenderObject/Layer) — Framework tier uses "2 structures + 1 cache" instead. See `.ai-factory/ARCHITECTURE.md`.
- Tracking-fork relationship with `gpui-ce` (we are a hard fork)
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
| S07.5 GestureArena T15 follow-up (RecognizerLifecycle, back-channel, hold/release, per-window settings, end-to-end test, contributor doc) | 2026-05-07 |
| S07.5b GestureArena pre-roster cleanup (PointerEvent surface upgrade, Affine2 + HitTestScope RAII, DeliveredEvent, unified back-channel hook + per-pointer LongPress storage, hold_count counter, AllowedButtonsFilter, CHANGELOG.md) | 2026-05-07 |
| S21 Animation Flutter parity (Animation<T> trait + listeners + Ticker, Curve trait family + Curves catalogue, Animatable + Tween family + TweenSequence, combinators, CurvedAnimation, controller polish, flui-animate skeleton removed) | 2026-05-08 |
| K99 MSRV bump to Rust 1.95 (workspace Cargo.toml + rust-toolchain.toml + clippy.toml msrv field; per-member rust-version inheritance for all 12 workspace members; CI converted to MSRV-enforced via rust-toolchain.toml + new non-blocking forward-compat job; AGENTS/DESCRIPTION/ARCHITECTURE/RESEARCH/rules docs aligned; FREEZE Cargo.lock policy; flake.nix divergence documented). First Phase 0-K spec. Unblocks K15 → K07 → K05 → K01 → K02 → K03 → K04 critical chain. | 2026-05-08 |
| K15 Re-entrancy contract (`flui_core::reentrancy` module with `ReentryError` `#[non_exhaustive]` enum + `ReentryMode { Strict, Loose }`; same-window `update_window` returns `Err(anyhow{ NestedWindowUpdate })`; same-entity `update_entity` panics with `NestedEntityUpdate(_)` Display; `EntityMap::double_lease_panic` unified Display so multi-entity cycles produce same message; `Window::prompt` widens to `Result<Receiver, ReentryError>`; `AsyncWindowContext::prompt` widens to `anyhow::Result<Receiver>`; `with_element_state` structured panic via `ElementStateInUse { global_element_id, type_id }`; three platform deferral comments updated; `cx.defer` / `Window::defer` documented as escape hatches; `PanicLikeUpstream` deferred to K07; 11 new tests). Second Phase 0-K spec; unblocks K07. | 2026-05-09 |
| K07 AppCell removal (`flui_core::app::cell::AppCell` hand-rolled `UnsafeCell<App>` + `BorrowState`; doc-hidden compatibility surface preserved; `BorrowMutError` conversion and `TRACK_THREAD_BORROWS` removed; AsyncApp structured `ReentryError` paths; typed `AsyncContextAsMut`/`AppGoneAway` panics; panic-restoration guards for entity/window/pending-update state; hot-path audit; proptests; scoped Miri Stacked Borrows + Tree Borrows; CI Miri jobs; acquire/release bench). Third Phase 0-K spec; unblocks K05. | 2026-05-10 |
| K05 Element trait context object (`LayoutCx`, `PrepaintCx`, `PaintCx` lifecycle contexts; public custom `Element` signatures migrated off raw id/inspector/bounds/window/app bundles; `AnyElement` traversal and `Interactivity` helper layer migrated; `Window` root/deferred/inspector/test draw paths updated; focused context/focus tests; migration guide). Fourth Phase 0-K spec; unblocks K01. | 2026-05-11 |
| K01 Provider rewrite (per-`Window` inherited registry; `Provider<T>` scope identity with source-location fallback plus `new_keyed`; scoped `Window`/`LayoutCx`/`PrepaintCx`/`PaintCx` inherited reads; value-change invalidation via existing view invalidator; cached-view inherited dependency capture/replay; provider removal cleanup; old thread-local `provider/stack.rs` and free reads removed; `flui-widgets` re-export migrated; migration guide and focused provider tests). Fifth Phase 0-K spec; unblocks K02. | 2026-05-11 |
| K02 Element identity and Key (`Key::{local,value,global}`, `ValueKey`, `GlobalKey`, Element-owned `ElementId`, normalized Local occurrence segments, internal identity stack resolver, debug duplicate sibling-key diagnostics, lifecycle/cache/deferred pass resets, provider/state convergence, focused identity/state/provider tests, migration guide). Sixth Phase 0-K spec; unblocks K03 and provides SF01/SF02 identity substrate. | 2026-05-11 |
| K03 Render to Build separation (`ElementBuilder`, `ElementBuildCx`, `BuildElement`, and `build_element` immutable engine recipe substrate; `Render`, `RenderOnce`, `Component<C>`, root mounting, `AnyView`, provider/cache/deferred behavior, macros, Tier C crates, and examples preserved; migration/docs updated). Seventh Phase 0-K spec; unblocks K04 and provides SF01/SF07 render/build boundary. | 2026-05-11 |
| K04 Effect / Frame contract (typed seven-phase pipeline `PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame` driven by `App::run_frame`; App-level `FrameClock` + `FrameProfile` + flag-gated `FrameProfileDetailed`; placement-aware `Effect::Defer { placement, callback }` with `DeferPlacement { EndOfUpdate, NextFrameStart, PostFrame, Idle }` + `App::defer_to` mirrors; advisory per-phase deadlines + `EffectFlush` break-and-requeue; `App::abort_frame_after_panic` panic safety; sealed `TickTarget` + `App::active_animations` walker; `AnimationController::value` per-frame cache; `Window::on_pre_frame` rename with deprecated `on_next_frame` alias + new `Window::on_post_frame` + App-level mirrors; idempotent `Window::request_animation_frame` via `Cell<bool>`; `SmallVec`-on-Window callback storage; `TestApp::advance_frame` test driver; joint K15+K04 paragraph in `reentrancy` module; migration guide. Eighth and final Phase 0-K critical-chain spec; unblocks Phase II-F SF03/SF04/SF05 planning. K04+1 follow-up: flip `auto_advance_frames_on_flush` default to `false` after Tier-C tests migrate; remove the deprecated `on_next_frame` alias. Validation: 417 `cargo test -p flui-core --lib --features test-support` pass; `cargo fmt --check` clean; cross-crate `cargo check` clean. | 2026-05-12 |

## Cross-track dependencies

- **A2 → R2 → R1**: stabilizing the public surface (kill remaining globs) is a prerequisite for `cargo-semver-checks`, which is itself a prerequisite for confident crates.io publishing.
- **A6 → R7**: workspace-level dependency consolidation makes CI matrix expansion (especially feature-powerset jobs) tractable.
- **P1 → P2..P9**: frame-budget instrumentation gives the measurement substrate that the other perf items act on. Land P1 first.
- **T4 → P3..P7**: Criterion benchmark suite is the baseline that lets perf work show measurable wins.
- **S08 → S17, S18**: semantics protocol must land before mobile platforms can plug accessibility into it.
- **A4 + A3 → R8**: tracing + error-handling guidance feed CONTRIBUTING.md.
- **S01b lock infrastructure → T6**: visual regression expansion reuses the S01b harness; do not build a parallel one.
- **K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04**: Phase 0-K critical chain. Sequential. K99, K15, K07, K05, K01, K02, K03, and K04 are all complete; the chain is fully landed. Phase II-F (SF03/SF04/SF05) is unblocked.
- **K05 → K06, K08, K10, K11**: internal-org K-specs unlock after Element ctx-object lands.
- **K01 → SF03**: Provider mechanism (K01) is the actual implementation; SF03 is the BuildCx ergonomics wrapper.
- **K02 → SF01, SF02**: Element identity & Key are the substrate; SF01 defines the trait, SF02 uses it for reconciliation.
- **K03 → SF01/SF07**: Render/build separation establishes the type-level distinction Framework widget traits and mounting rely on.
- **K04 → SF05**: Effect/Frame contract underpins setState scheduling.
- **K07 → SF04, SF05**: AppCell removal allows clean ownership for State<W> mutation.
- **SF01 → SF02 → SF04 → SF05**: Framework tier core path. SF05 (`setState` + dirty-list) is the gate that unblocks Tier C ecosystem build-out.
- **SF03 → SF06 → S14**: Provider mechanism unblocks `MediaQuery.of()`, which closes the developer-facing half of S14 (MediaQuery completeness). The accessibility-flag plumbing in S14 still needs Engine work.
- **SF03 → existing E1 closure**: SF03's per-Window `InheritedRegistry` replaces `flui-core::provider::stack`'s thread-local global, closing the E1 issue from `docs/promt.md`.
- **SF05 → Tier C populating**: widget crates (`flui-widgets`, `flui-material`) cannot be meaningfully implemented until `setState` lands. Tier C build-out is gated on SF05.
- **S21 (done) → SF04**: `Animation<T>` trait family is consumed by Framework's `AnimatedBuilder` / animation-aware widgets. Available now.
- **S07.5b (done) → SF05**: `GestureBinding` is consumed by Framework gesture detector widgets. Available now.

## Anti-goals for cross-cutting tracks

- ❌ Do not let perf milestones (P#) drive premature pessimization of API ergonomics.
- ❌ Do not treat A2 / A8 as license to rewrite the public surface in one big PR; each is a curated, reviewable change.
- ❌ Do not introduce R-track tooling (semver-checks, deny, release-plz) before the first publishable surface exists — premature gating is friction without value.
- ❌ Do not start S17/S18 (mobile) without S08 (semantics protocol) and S16 (headless renderer baseline).
- ❌ Do not implement Framework tier (Phase II-F) inside `flui-core`. It lives in `flui-framework` (new crate). Engine and Framework are separate crates by design — see `.ai-factory/ARCHITECTURE.md`.
- ❌ Do not start populating Tier C widget crates (Container, Row, Stack, Material widgets) before SF05 (`setState` + dirty-list) lands. Widgets without rebuild semantics are dead-ends.
- ❌ Do not preserve `gpui-ce` API for backwards compatibility. flui-v2 is a hard fork — divergence is the design goal, not a regression.
- ❌ Do not re-introduce v1's multi-crate engine split (`flui-foundation` / `flui-engine` / `flui-rendering` / …). Engine stays single-crate (`flui-core`).
