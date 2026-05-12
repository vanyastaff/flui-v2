# Research

Updated: 2026-05-11
Status: active

**K15 (2026-05-09)** — Re-entrancy contract published at `crates/flui-core/src/reentrancy.rs` (second spec in the Phase 0-K critical chain after K99). `ReentryError` (`#[non_exhaustive]`) names every same-target re-entry case; `ReentryMode { Strict, Loose }` selects log level (Strict = `error!`, Loose = `warn!`); test default is Strict. Behavior: same-window `update_window` returns `Err(anyhow{ ReentryError::NestedWindowUpdate })`; same-entity `update_entity` panics with `ReentryError::NestedEntityUpdate(_)` Display (trait signature `R` cannot widen); multi-entity cycles `A → B → A` ALSO use the unified Display via the rewritten `EntityMap::double_lease_panic`; `with_element_state` recursive panic uses `ReentryError::ElementStateInUse { global_element_id, type_id }`; `Window::prompt` widens to `Result<Receiver, ReentryError>` and `AsyncWindowContext::prompt` widens to `anyhow::Result<Receiver>` (was: silently swallowed errors via dead receivers). `cx.defer` / `Window::defer` are the documented queue escape hatches — no new `Effect` variant introduced. `PanicLikeUpstream` mode and `legacy-reentry-panics` feature DEFERRED to K07 per adversarial-review consensus (the hatch could not faithfully reproduce upstream entity-side panic, and the runtime field for compile-time-gated variant is dead weight). Four Known Limitations documented in design spec: 10+ remaining `AsyncApp::borrow_mut()` sites unstructured (K07), `AsyncApp::as_mut` panic out of class, `web` platform unverified, `AppBorrowed` carries no source location (nightly-only API). Tests: 344 lib tests (333 baseline + 11 new — 6 type-level + 5 behavioral via `TestApp`). Spec: `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md`.

**K07 (2026-05-10)** — AppCell removal landed as Candidate B: `flui_core::app::cell::AppCell` is now a hand-rolled `UnsafeCell<App>` + `BorrowState` primitive that returns `ReentryError` directly, while preserving the public doc-hidden `AppCell` / `AppRef` / `AppRefMut` spelling for compatibility. The 103 narrow AppCell-derived borrow callsites migrate onto the new cell; `impl From<std::cell::BorrowMutError> for ReentryError` and the `TRACK_THREAD_BORROWS` shim are gone; `AppCell` stays `#[doc(hidden)]` and `!Send + !Sync`. K15 Known Limitations discharged: #1 AsyncApp result paths now propagate `ReentryError` / panic paths use typed `panic_any`, #2 async/test/headless/visual `as_mut` panics use `ReentryError::AsyncContextAsMut`, and #6 panic-leak fields (`currently_updating_entity`, `window_update_stack`, `pending_updates`) are restored with raw-pointer field guards. Validation added: AppCell proptests, direct AppBorrowed replacement test, scoped Miri Stacked Borrows + non-blocking Tree Borrows, CI Miri jobs, hot-path audit, and acquire/release bench (`5 ns/op`, budget `1000 ns`). Spec: `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md`.

**K05 (2026-05-11)** — Element lifecycle context objects landed for the low-level engine `Element` API. `Element::request_layout`, `prepaint`, and `paint` now receive `LayoutCx<'_>`, `PrepaintCx<'_>`, and `PaintCx<'_>` instead of raw global-id / inspector-id / bounds / `Window` / `App` argument bundles. `AnyElement` traversal, `Drawable`, `Interactivity`, built-in elements, `ProviderElement`, root/deferred/inspector `Window` paths, test harness drawing, key-dispatch tests, and the legacy input example were migrated. The contexts expose documented identity/bounds/runtime accessors plus explicit nested-context helpers for adjusted id/bounds delegation. K05 deliberately does not introduce Framework `BuildCx`, Provider rewrite, panic recovery for lifecycle panics, or ownership sharding. Spec: `docs/superpowers/specs/2026-05-11-K05-element-context-object-design.md`. Migration guide: `docs/superpowers/migrations/K05-element-context-object.md`.

**K01 (2026-05-11)** — Provider rewrite landed for the low-level engine substrate. The old `provider/stack.rs` thread-local global is removed from production, and each `Window` now owns an `InheritedRegistry` with provider scope identity, phase-scoped activation, clone-returning scoped reads, subscribing `inherit<T>()`, `PartialEq`-based value-change invalidation, cached-view inherited dependency capture/replay, provider removal cleanup, and test-support inspection helpers. `Provider::new` uses source-location fallback identity; `Provider::new_keyed` accepts explicit K02 `Key` / `ElementId` values for data-stable provider identity. Public global reads (`read::<T>()` / `try_read::<T>()`) are removed from `flui-core` and `flui-widgets`; low-level lifecycle contexts expose `read_inherited::<T>()` and `inherit::<T>()`. Spec: `docs/superpowers/specs/2026-05-11-K01-provider-rewrite-design.md`. Migration guide: `docs/superpowers/migrations/K01-provider-rewrite.md`.

**K02 (2026-05-11)** — Element identity and Key landed for the Tier-A engine substrate. `ElementId` moved into an Element-owned identity module and remains re-exported; opaque `Key::{local, value, global}`, `ValueKey`, and `GlobalKey` model identity intent; `ElementId::CodeLocation` is normalized into `ElementId::Local(LocalElementId)` by an internal `ElementIdStack`. The stack tracks parent-scoped Local occurrence counters, debug duplicate explicit sibling-key diagnostics, lifecycle-pass resets, and deferred-draw resolver snapshots. `Window::use_state` now uses Local occurrence; `use_keyed_state`, `with_id`, `with_element_namespace`, `Provider::new_keyed`, and `Component::key` accept key/value identity for stable boundaries. `AnyView::cached` behavior is preserved and cache rerenders restore nested identity state; public stateless element cache wrappers and cross-tree GlobalKey moves are deferred to SF02/SF05. Spec: `docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md`. Migration guide: `docs/superpowers/migrations/K02-element-identity-key.md`.

**K03 (2026-05-11)** — Render to Build separation landed for the Tier-A engine/framework boundary. `Render` remains the mutable entity-backed view trait for roots and `AnyView`; `RenderOnce` and `Component<C>` remain source-compatible engine recipes; K03 adds the narrow immutable recipe substrate `ElementBuilder`, `ElementBuildCx`, `BuildElement`, and `build_element(...)` in `flui-core`. The design deliberately does not create `flui-framework`, final `Widget`, reconciliation, dirty-list scheduling, `setState`, object-safe heterogeneous widget storage, or pure-build roots. Provider reads through `ElementBuildCx`, keyed builder identity, cached-view provider replay, deferred-draw identity, macro compatibility, Tier C crates, and examples are covered. Validation run: `cargo fmt --check`, `cargo test -p flui-core`, `cargo test -p flui-macros`, `cargo check -p flui-widgets --all-targets`, `cargo check -p flui-material --all-targets`, `cargo check -p flui-navigator --all-targets`, example checks for `creating_components`, `nav_demo`, `material_demo`, `animation_demo`, and `cargo test --workspace`. Spec: `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md`. Migration guide: `docs/superpowers/migrations/K03-render-build-separation.md`.

## Active Summary (input for /aif-plan)
<!-- aif:active-summary:start -->

**K03 status (2026-05-11):** Render to Build separation is complete in `flui-core`. `Render` stays the mutable entity-backed engine view trait; `RenderOnce` / `Component<C>` stay compatible; `ElementBuilder` / `ElementBuildCx` / `BuildElement` provide a narrow immutable engine recipe path built from `&self`. K03 deliberately leaves final `Widget`, `State`, `BuildCx`, reconciliation, dirty lists, `setState`, object-safe widget erasure, and pure-build roots to Phase II-F. Workspace validation passed. Next critical-chain item is K04 (Effect / Frame contract).

**K04 status (2026-05-12):** Effect / Frame contract is complete — the eighth and final Phase 0-K critical-chain spec. The App scheduler is now a typed seven-phase state machine (`PreFrame → AnimationTick → Build (reserved no-op for SF05) → Layout → Prepaint → Paint → PostFrame`) entered via `App::run_frame(window_id) -> FrameOutcome`. `FrameClock` samples the underlying `Clock` exactly once per frame (axiom P3) and serves every consumer the same `Instant`. `Effect::Defer { placement: DeferPlacement, callback }` replaces the placement-less variant; `App::defer(f)` preserves pre-K04 observable behavior by routing through `DeferPlacement::EndOfUpdate`. Per-phase drains use `FlushScope::Phase` (pre-body, full placement admission) and `FlushScope::PhasePost` (post-body, only `EndOfUpdate`) so matching-placement defers always carry one frame to their target phase. `AnimationController::value` caches per-frame keyed on the tick's `Instant`. `Window::on_pre_frame` (renamed from `on_next_frame`) and `Window::on_post_frame` (new) bookend each frame; App-level `App::on_pre_frame` / `App::on_post_frame` cover cross-window callbacks. `Window::request_animation_frame` is idempotent via `Cell<bool>`. Per-window `next_frame_callbacks` migrated from `Rc<RefCell<Vec<_>>>` to `RefCell<SmallVec<[_; 4]>>` directly on `Window`. `TestApp::advance_frame` is the canonical test-mode frame driver. Panic-in-phase recovery via `App::abort_frame_after_panic` keeps the App usable. The K15 contract is unchanged — joint K15+K04 paragraph published in the `flui_core::reentrancy` module docs. Validation: 417 `cargo test -p flui-core --lib --features test-support` pass; `cargo fmt --check` clean; cross-crate `cargo check` clean. The Phase 0-K critical chain is now FULLY COMPLETE. Phase II-F (SF03 BuildCx ergonomics, SF04 InheritedWidget facade, SF05 setState + dirty list) is unblocked and can begin planning. K04+1 follow-ups: flip `auto_advance_frames_on_flush` default to `false` once Tier-C tests migrate; remove the deprecated `Window::on_next_frame` alias; integrate the platform `on_request_frame` callback to call `App::run_frame` directly (today's production path stays inline). Spec: `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md`. Migration: `docs/superpowers/migrations/K04-effect-frame-contract.md`.

**Topic:** Strategic alignment of flui-v2 toward "Flutter ecosystem on Rust" — reconciling vision with current architecture and prior abandoned attempts.

**Goal:** Build a full UI engine in Rust with a Flutter-equivalent ecosystem and developer experience, leveraging Rust 1.95 idioms (AFIT/RPITIT, edition-2024 lifetime captures, async closures, let-chains, `std::sync::{OnceLock, LazyLock}`, unsafe extern, `#[diagnostic::on_unimplemented]`), GPUI as the engine substrate, and community libraries (wgpu, taffy, cosmic-text). Performance and safety advantages over Flutter come from Rust ownership and zero-cost abstractions.

**Goal is "feature surface", not "internal layering":** the project intentionally does NOT replicate Flutter's 4-tree internal model (Widget/Element/RenderObject/Layer). The earlier `flui` v1 attempt (`C:\Users\vanya\RustroverProjects\flui`) tried that and was abandoned due to multi-tree complexity. Decision is final — we build on top of GPUI's single-tree engine.

**Three-layer strategic model (NEW — must be added to ARCHITECTURE/DESCRIPTION/ROADMAP):**

```
   ┌─────────────────────────────────────────────────────────┐
   │  C. ECOSYSTEM (community-writable)                      │
   │     flui-widgets, flui-material, flui-cupertino,        │
   │     flui-theme, flui-navigator, third-party crates      │
   │     KPI: stable public API → external widget crates     │
   ├─────────────────────────────────────────────────────────┤
   │  B. FRAMEWORK (Flutter-DX layer) ← CURRENTLY ABSENT     │
   │     Widget + Key + State + BuildCx + Provider           │
   │     Reconciliation + dirty-list                         │
   │     Theme.of() / MediaQuery.of() / Navigator.of()       │
   │     "Flutter feature surface" from DESCRIPTION.md       │
   ├─────────────────────────────────────────────────────────┤
   │  A. ENGINE (current flui-core)                          │
   │     App + Entity + Element + Scene + Window             │
   │     Layout (Taffy) + Text (cosmic) + Gesture + Anim     │
   │     Platform backends                                   │
   │     Stabilization = closing S08-S15 gaps                │
   └─────────────────────────────────────────────────────────┘
```

**Architectural decision: "2 structures + 1 cache", not 4 trees.**

```
   Widget          (immutable config struct, derive macro,
                    cheap clone, recreated each rebuild)
       │ build()
       ▼
   Element tree    (current GPUI Element — runtime,
                    layout/paint, hit-test — UNCHANGED)

   ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─

   StateMap        (HashMap<ElementId, Box<dyn State>>,
                    FLAT, not a tree — survives rebuilds,
                    keyed by ElementId or user-provided Key)
```

State is NOT a tree — it's a flat map keyed by `ElementId`. Reconciliation operates per-position within siblings via `(TypeId, Key)` matching, not globally. This is the central simplification vs Flutter, made possible by Rust ownership.

**Constraints:**
- Keep GPUI substrate. Do NOT return to v1 multi-tree implementation.
- **flui-v2 is a HARD FORK, not a tracking fork.** Project explicitly diverges from upstream `gpui-ce` and Zed. Both became inactive on framework-level evolution; flui-v2 picks up the work and goes its own way. No commitment to upstream-sync, no semver compatibility with `gpui`, no obligation to preserve GPUI public API. Breaking changes from upstream are the entire point of the fork.
- Phase I (platform extraction) is FROZEN per ROADMAP.md. S02b-S06 deferred to Phase III when a real platform driver lands.
- Breaking changes are allowed (no semver promise yet) — both internal and from upstream gpui-ce.
- 60 FPS is a structural property — Framework layer must not allocate on rebuild hot path; reconciliation must be O(siblings), not O(tree).
- `Rc<RefCell<...>>` forbidden on dispatch/tick/paint hot paths (per `docs/promt.md` §3.1).
- Edition 2024, MSRV 1.95 (bumped in K99, 2026-05-08).

**Fork positioning (NEW — critical context):**

```
   Upstream lineage:                       flui-v2 trajectory:
   ──────────────────                      ───────────────────
   GPUI (Zed proprietary)                  ┌─────────────────┐
       │                                   │  flui-v2        │
       ▼                                   │  • own roadmap  │
   gpui-ce (community fork)                │  • own API      │
       │  ← became inactive                │  • own DX       │
       │  ← framework gaps unfilled        │  • Flutter feat │
       ▼                                   │    surface      │
   ─── divergence point ───                │  • Rust 2024+   │
                                           │    idioms       │
                                           │  • can take     │
   flui-v2 (this project) ◄────────────────│    upstream PRs │
                                           │    selectively  │
                                           └─────────────────┘
```

Implications:
- ROADMAP item **R10 ("Migration guide from gpui-ce")** is REVERSED — it's a one-way migration, not a sync mechanism. The `extern crate flui_core as gpui;` pattern in `README.md` is a transitional aid for porting Zed-style code, not a permanent compatibility contract.
- We MAY cherry-pick PRs from gpui-ce / Zed when relevant (user has already done this for some core fixes), but it's a one-way pull, not a two-way sync.
- "GPUI" branding in code/docs (157 occurrences per E16 in `docs/promt.md`) should be rebranded to "flui" — this is not just cosmetic, it signals the divergence to anyone reading the code.
- API decisions are made for flui-v2's goals (Flutter DX, Rust idioms, 60fps as structural property), not for upstream compatibility.

**Decisions made in this session:**

1. **NO return to v1 flui** (multi-tree implementation). Decision encoded in `DESCRIPTION.md:41`, reaffirmed.
2. **NO 1:1 Flutter clone**. Replicate feature surface, not internals.
3. **YES new Framework layer (B)** between flui-core and flui-widgets — currently missing from ROADMAP.
4. **YES three-layer model A/B/C** replaces or augments the current 5-layer model in `DESCRIPTION.md` and `README.md`.
5. **YES "2 structures + 1 cache"** — Widget config + Element tree + flat StateMap. No RenderObject as separate tree. No Layer tree (Scene already exists).
6. **`Component<C: RenderOnce>` is NOT the Widget adapter** — it's a one-shot RenderOnce shim. New Framework layer needs its own `Component<W: Widget>` adapter or different mounting strategy.
7. **`docs/promt.md` is partially incorrect**: §4.5 references "existing `Component<Widget>` adapter" which does not exist. §3.3 hybrid widget tree is right in spirit but conflates Engine/Framework boundaries.
8. **HARD FORK posture**: flui-v2 is not a tracking fork of gpui-ce. Upstream became inactive on framework evolution; flui-v2 takes ownership of the trajectory. May cherry-pick upstream fixes selectively, but is not bound by upstream API or roadmap.
9. **Rebrand "GPUI" → "flui"** is strategic, not cosmetic — signals the fork's autonomy. Current 157 GPUI mentions across 25 files (per E16) should be cleaned up as part of Phase 0 hygiene.

**Gaps identified in current `.ai-factory/*` and AGENTS.md:**

| File | Issue |
|------|-------|
| `AGENTS.md` | Says "currently in Phase I" — STALE. Phase I frozen. S07/S07.5/S07.5b/S21 done. Phase II active. No mention of Framework layer. |
| `DESCRIPTION.md` | Correctly rejects v1 multi-tree (line 41), but does NOT describe where Widget+State layer lives in the target architecture. |
| `ROADMAP.md` | S08-S15 close ENGINE gaps. NO specs for Framework layer (Widget, Key, State, BuildCx, reconciliation). `flui-widgets` listed only as "planned". Framework track missing. |
| `ARCHITECTURE.md` | Documents 5-layer model. Does NOT mention StateMap, reconciliation, or Widget/Element duality. |
| `rules/base.md` | No rules covering Framework layer (no-allocation rebuild, Widget = immutable config, etc.) |
| `docs/promt.md` | §4.5 "Widget layer" is one paragraph for what is multi-spec phase work. References non-existent `Component<Widget>` adapter. Hybrid model description conflates Engine and Framework. |

**Open questions (for future specs):**

1. **Where does Framework layer live?** New crate `flui-framework`? Module inside `flui-core`? Or extend `flui-widgets` to include framework primitives?
2. **InheritedWidget analog?** Full Provider system (per-Window registry with subscriptions) vs simpler Entity-subscription pattern? `docs/promt.md` §4.4 proposes per-Window InheritedRegistry — keep that proposal.
3. **State<W> ownership model?** `Box<dyn State>` in flat HashMap vs `Entity<S>` reuse? Entity gives free reactivity but couples Framework to App lifetime.
4. **Mounting/disposal lifecycle?** Flutter's `initState`/`didUpdateWidget`/`dispose` — implement all three or simplify?
5. **`setState` semantics?** Mark element dirty + schedule rebuild? Through which channel — App effect queue, Window dirty list, or new mechanism?
6. **Key types?** Local (source-location hash + sibling index), Value (user `ValueKey<T>`), Global (cross-tree). `docs/promt.md` §4.5 sketches all three. Confirm.
7. **Reconciliation algorithm:** Flutter uses 4-pass (top match, bottom match, middle hash, leftover sweep). Match Flutter or simplify?

**Ecosystem KPI definition (NEW):**

"Flutter ecosystem parity" defined concretely as:
- Public API of flui-core + flui-framework is `cargo-semver-checks` clean
- A third-party crate can implement a custom widget against stable Framework API
- `flui-widgets` reaches 50+ widgets (Container, Row/Column/Stack, Text, Button, ListView, GridView, Scaffold, AppBar, etc.)
- One representative app implementable in ≤ 500 lines

This is the success metric for the "Phase II + Framework + Ecosystem" track.

**Success signals:**

- `AGENTS.md` no longer says "Phase I"
- `ROADMAP.md` has explicit Phase II-F (Framework) section with SF01-SF06+ specs
- `DESCRIPTION.md` describes A/B/C three-layer model
- `ARCHITECTURE.md` documents StateMap pattern and reconciliation
- First Framework spec (SF01 Widget+Key trait) lands and passes review
- `flui-arch-reviewer` agent updated to recognize Framework layer boundaries

**Next step (for the user):**

The previous SF01-first plan is REVERSED. Audit (see "Phase 0-K Kernel Cleanup audit" session below) identified 24+ structural issues in `flui-core` that block Framework tier work. Kernel Cleanup must precede Framework.

1. **K99, K15, K07, K05, K01, K02, and K03 are complete.**
2. **Next run `/aif-plan full K04-effect-frame-contract`** — define structured frame phases, drain order, and deadline placement.
3. **Then Phase II-F can begin planning once K04 lands** and the Phase 0-K critical chain is complete.
4. **Hygiene K90-K98 in parallel slots** — small independent PRs, can land any time.
5. **Internal-org K06, K08, K10, K11 are now unblocked by K05** — schedule them alongside the remaining critical-chain work only when they do not slow K01-K04.
6. **SF01 only AFTER K01-K04 lands** — Framework tier sits on the cleaned kernel.

**Phase 0-K Kernel Cleanup spec slate (replaces premature SF01 path):**

Critical chain (sequential):
- **K99 (done)** — MSRV bump to Rust 1.95+ (workspace-mechanical, prerequisite for all K-specs)
- **K15 (done)** — Re-entrancy contract (document + enforce, queue-not-panic for nested updates)
- **K07 (done)** — AppCell removal (token-based borrow model, replaces `RefCell<App>`)
- **K05 (done)** — Element trait → context object (`PaintCx` / `LayoutCx` / `PrepaintCx`)
- **K01** — Provider rewrite (per-Window InheritedRegistry, reactive subscriptions) — complete
- **K02 (done)** — Element identity & Key (Local/Value/Global)
- **K03 (done)** — Render → Build separation (`ElementBuilder` immutable engine recipe substrate; final `Widget` deferred to Framework)
- **K04** — Effect / Frame contract (preFrame / postFrame phases, deadlines)

Internal-org (parallel after K05):
- **K06** — Window decomposition + ownership split (BuildOwner/PipelineOwner/SemanticsOwner)
- **K08** — Action subtree dispatcher (per-subtree, replaces `inventory::collect!`)
- **K10** — Style decomposition (LayoutStyle/SpacingStyle/BoxDecoration/...)
- **K11** — Hit-test arena (Vec<HitTestEntry> indexed by HitboxId)

Independent (any order):
- **K12** — Drop order codification + Entity cycle detection
- **K13** — Arena allocator audit (custom unsafe vs bumpalo/typed_arena)
- **K14** — Subscription backpressure + bounds
- **K16** — Coordinate-space type-safety (sealed conversions, no bidirectional From<f32>)
- **K17** — Test harness simplification (`flui_core::testing::WidgetTester`)
- **K20** — Layout cache (Taffy results keyed by style+constraints hash)
- **K21** — Text-shape cache audit + LRU bound
- **K22** — Inspector intro API (read-only tree traversal substrate)

Hygiene (parallel slots, any time):
- **K90** — Rebrand "GPUI" → "flui" (157 mentions)
- **K91** — 29 globs → explicit re-exports
- **K92** — derive_more 0.99 → 2.x
- **K93** — TODO/FIXME/dead_code triage
- **K94** — Prelude expansion
- **K95** — `with_context().unwrap()` → expect helpers
- **K96** — `unwrap_or_else(|| panic!(…))` → expect
- **K97** — scene.rs missing_docs → real docs
- **K98** — `_ownership_and_data_flow.rs` rewrite (broken doctests)

**SF specs remain queued, but gated:**

- **SF01** — Widget + Key trait — needs K02, K03, K05
- **SF02** — Reconciliation — needs SF01, K15
- **SF03** — BuildCx + Provider ergonomics — needs SF01, K01
- **SF04** — State<W> + StateMap — needs SF01, SF02, K07
- **SF05** — setState + dirty-list — needs SF03, SF04, K04
- **SF06** — InheritedWidget analog — needs SF03
- **SF07** — Widget → Element adapter — needs SF01–SF05
- **SF08** — Async widgets — needs SF04, SF05

Engine completeness (S08, S09, S10, S12-S15) runs in parallel with K-track since most are additive (semantics tree, canvas facade, focus traversal, text parity, mediaquery completeness, asset bundle) — they don't conflict with kernel refactor.

<!-- aif:active-summary:end -->

## Sessions
<!-- aif:sessions:start -->

### 2026-05-08 — Strategic alignment session

**What changed:**

- Identified core strategic question: "Flutter ecosystem on Rust" via Engine substrate (GPUI) + new Framework layer + Ecosystem crates.
- Confirmed v1 `flui` (multi-tree implementation at `C:\Users\vanya\RustroverProjects\flui`) is dead — decision in `DESCRIPTION.md:41` is final.
- Diagnosed `docs/promt.md`: solid Engine plan, but Widget/Framework section underspecified and references non-existent `Component<Widget>` adapter.
- Surfaced gap: `ROADMAP.md` has no Framework-layer specs — only Engine gaps (S08-S15). This is the missing track.
- Identified stale text in `AGENTS.md` ("currently in Phase I") that pre-dates S07.5b and S21 completion.
- Defined three-layer model A (Engine) / B (Framework) / C (Ecosystem) as replacement for current implicit 5-layer description.
- Defined "2 structures + 1 cache" as Framework's central simplification vs Flutter's 4 trees, justified by Rust ownership.
- Defined ecosystem KPI: stable public API → community can write external widget crates.

**Key notes:**

- `Component<C: RenderOnce>` exists at `crates/flui-core/src/element.rs:182` but is `#[doc(hidden)]` and one-shot — NOT a Framework adapter. New SF07 spec needed for stateful Widget mounting.
- S01a roadmap, gesture S07/S07.5/S07.5b, animation S21 are complete and high-quality. Framework layer can build on top of them confidently.
- `flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, `rust-api-migration-auditor` agents should be invoked when Framework specs touch Engine surface.

**Links (paths):**

- `.ai-factory/DESCRIPTION.md:41` — "no v1 multi-tree" decision
- `.ai-factory/ROADMAP.md` — current spec slate (S07.5b, S21 done; S08-S15 pending)
- `docs/promt.md` — long-form strategic plan dropped by user; partially incorporated, partially needs revision per this RESEARCH
- `crates/flui-core/src/element.rs:182` — existing `Component<C: RenderOnce>` (NOT a Widget adapter)
- `AGENTS.md` — stale "Phase I" reference, needs update
- `C:\Users\vanya\RustroverProjects\flui` — abandoned v1 implementation; reference only, do not resurrect

### 2026-05-08 — Phase 0-K Kernel Cleanup audit (course correction)

**What changed:**

User raised a strategic course correction: Framework tier (Phase II-F / SF01-SF08) is **not the right next step**. `flui-core` itself has 24+ architectural issues that must be fixed first. Building Framework on the current kernel = construct on cracks = redo work later.

User produced a detailed `flui-core` architectural audit covering:
- 4 critical issues (Provider broken, no Widget identity / `Key`, `Render::&mut self`, undefined effect/frame ordering)
- 5 high-priority issues (Element trait param explosion, Window 222 methods, AppCell, action globals via `inventory`, missing Canvas facade)
- 6 medium-priority issues (Style 38 fields, frame caching limited to Entity, hit-test FxHashMap, drop ordering hand-controlled, 16 unsafe blocks, subscription no backpressure)
- 9 hygiene issues (157 GPUI mentions, 29 globs, derive_more 0.99, 47 TODOs, prelude minimal, with_context.unwrap, unwrap_or_else panic, scene.rs missing_docs, broken `_ownership_and_data_flow.rs` doctests)
- Mandatory MSRV bump to Rust 1.95+

Independent audit pass added 9 categories not in user's list:
1. Re-entrancy contract — undocumented, RefCell-driven, no tests for nested update_window/update_entity/setState. **Critical for Framework's `did_update_widget` + `setState` interactions.**
2. Coordinate-space type-safety leaky — bidirectional `From<f32> for Pixels` + `impl From<DevicePixels> for ScaledPixels` (geometry.rs:3012, 2772, 2784) without scale factor. Bug-magnet for HiDPI / retina.
3. No layout cache — Taffy recomputes every frame even for static UI. Wastes ~3ms × 60fps = 180ms/sec.
4. Cosmic-text shape cache scope unclear — needs audit; LRU bound likely missing.
5. Test harness too heavy — no `WidgetTester`-style lightweight harness; unit-testing widgets requires `Application::test()` + event loop. Will hurt Framework tier test coverage.
6. Inspector / DevTools surface absent — no read-only tree-traversal API. Cheap to reserve now, expensive to retrofit.
7. Drop order + Entity cycle detection — cross-Entity Weak refs can form cycles via `cx.observe`; no detection.
8. Hot-reload strategically absent — Flutter has it as load-bearing DX; not in roadmap. Should at minimum reserve as future R-track.
9. Window's monolithic borrow domain — splitting `window.rs` into files is necessary but not sufficient; Flutter splits ownership into `BuildOwner` / `PipelineOwner` / `SemanticsOwner` (independent borrow domains). K06 must address this, not just file decomposition.

**Sequencing decision:**

Phase 0-K (Kernel Cleanup) becomes the active phase. Critical chain is sequential:
`K99 (MSRV 1.95) → K15 (re-entrancy contract) → K07 (AppCell removal) → K05 (Element ctx-object) → K01 (Provider rewrite) → K02 (Key + identity) → K03 (Render/build separation) → K04 (Effect/Frame contract)`

As of 2026-05-11, K99, K15, K07, K05, K01, K02, and K03 are complete; K04 is next.

Internal-org (K06, K08, K10, K11) parallelizes after K05. Independent items (K12, K13, K14, K16, K17, K20, K21, K22) and hygiene (K90-K98) run in parallel slots throughout.

Phase II (Engine completeness — S08, S09, S10, S12-S15) runs in parallel with K-track since most specs are additive and don't conflict with kernel refactor.

Phase II-F (Framework — SF01-SF08) gated on K-track critical chain completion. Each SF spec gains explicit K-prereqs in ROADMAP.

**Rust 1.95+ specifics:**

- AFIT + RPITIT + edition-2024 lifetime captures — enables `Widget::build(&self) -> impl Widget` without `Box<dyn>`. Critical for "no allocation on rebuild hot path" invariant.
- async closures stable (1.85+) — callback API without `Box<dyn FnMut>`.
- `let-chains` stable (1.88, 2024 edition) — cleaner reconciliation code.
- `std::sync::OnceLock` (1.70+) and `std::sync::LazyLock` (1.80+) — drop `once_cell` crate dependency. Single-threaded variants `std::cell::OnceCell` / `std::cell::LazyCell` also stable.
- `unsafe extern` (1.82) — cleaner platform code.
- `#[diagnostic::on_unimplemented]` (1.78) — better Widget API errors.
- Cost: single-PR mechanical bump in `Cargo.toml` + `rust-toolchain.toml`. No downstream consumer constraints (hard fork, no upstream-sync commitment).

**Estimated effort for Phase 0-K:**

- Hygiene K90-K99: ~10 specs, 1-2 weeks
- Critical chain K15→K04: ~7 specs, 4-6 weeks (sequential)
- Internal org K06, K08, K10, K11: ~4 specs, 2-3 weeks parallel
- Independent K12-K22 (selective): ~5 specs, 2-3 weeks
- **Total Phase 0-K: 8-12 weeks of sequential work for one agent.**

After Phase 0-K, Framework tier (SF01-SF08) becomes substantially cheaper because the substrate is sound.

**Key notes:**

- K06 (Window decomposition) is bigger than first thought — needs Flutter-style ownership split (`BuildOwner`/`PipelineOwner`/`SemanticsOwner`), not just file split.
- K15 (re-entrancy contract) is the unsung critical fix — every callback in the Framework will hit this.
- K17 (test harness) is gating on Framework tier velocity — without it, SF specs ship with weak test coverage.
- K22 (inspector intro) is cheap insurance — read-only traversal trait now, real DevTools later.
- S09 Canvas facade currently in Phase II is effectively a Framework prereq for `CustomPainter` widgets — consider promoting into K-track.
- Hot-reload (audit-finding 8) is NOT a K-track concern; reserved as future Phase IV / R-track item.

**Links (paths):**

- `crates/flui-core/src/provider/stack.rs:7-9` — broken Provider thread-local global
- `crates/flui-core/src/element.rs:131-134` — `Render::render(&mut self)` semantic mismatch
- `crates/flui-core/src/element.rs:73-104` — Element trait param explosion
- `crates/flui-core/src/window.rs:1156, 1550` — Window two-impl-block, 222 pub methods, 6123 lines
- `crates/flui-core/src/app.rs:73-75` — `AppCell` "Strongly consider removing after stabilization"
- `crates/flui-core/src/app.rs:2488-2509, 1389-1424` — Effect system, no frame-phase guarantee
- `crates/flui-core/src/action.rs:282` — `inventory::collect!` global action statics
- `crates/flui-core/src/style.rs:180` — Style 38 flat fields
- `crates/flui-core/src/window.rs:966-982` — hit-test storage as FxHashMap
- `crates/flui-core/src/arena.rs:182` — runtime check for arena allocator validity
- `crates/flui-core/src/geometry.rs:3012, 2772, 2784, 2814` — bidirectional `From<f32>`, suspicious `From<DevicePixels> for ScaledPixels`

<!-- aif:sessions:end -->
