# K04 - Effect / Frame Contract

**Branch:** `epic-hopper-8dacbf` (planning lives in the active worktree; the implementation PR should land on a dedicated `feature/k04-effect-frame-contract` branch when execution starts)
**Created:** 2026-05-11 (refined after Flutter / Web / Compose / SwiftUI / React / Bevy cross-platform research)
**Phase:** 0-K Kernel Cleanup - eighth and final spec in the critical chain after K99, K15, K07, K05, K01, K02, and K03.
**Type:** API-shaping engine refactor in `flui-core` that formalizes frame phases, deadlines, and effect-drain placement. HIGH-RISK: touches the App scheduler. This is the "10-year contract" version — five sharper commitments distinguish it from a first-pass implementation guide (§ "10-Year Contract Axioms" below).
**Tasks:** 50 checkbox tasks.

> **Design-first spec.** K04 introduces structured frame phases and per-phase deadlines into the existing App effect loop and Window draw pipeline. The design spec must freeze the phase contract, the `Effect::Defer` placement model, the `FrameClock` time-source policy, and the deadline-overrun behavior **before** the App scheduler is rewritten. Phase II-F (`SF03`, `SF04`, `SF05`) is gated on this contract; getting it wrong forces a second pass on every Framework spec that schedules work.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | K04 redefines drain order and adds deadline-aware effect handling. Behavior must be proven by deterministic tests against `TestScheduler` / `TestClock`, not by manual visual inspection. |
| Logging | verbose during implementation; only `WARN` deadline-overrun logs may stay committed | Phase enters/exits, deadline checks, and effect placement decisions need traces while wiring. Per `docs/promt.md` §3.1 the committed dispatch/tick/paint paths must not log per element or per frame; only the deadline-overrun warning (`effect-flush exceeded budget; deferring`) and explicit `FrameProfile` instrumentation are allowed in committed code. |
| Docs | yes (mandatory checkpoint) | K04 alters a public engine contract (effect ordering, `defer` semantics, animation tick), needs a migration guide, rustdoc updates, examples cleanup, and roadmap/research status updates. |
| Roadmap linkage | linked | K04 is the next Phase 0-K critical-chain item and the last one before Phase II-F starts planning. SF03/SF04/SF05 explicitly depend on it. |

## Roadmap Linkage

**Milestone:** K04 Effect / Frame contract — formalize the App-scheduler frame phases, per-phase deadlines, `Effect::Defer` placement, and drain-order tests (Phase 0-K critical chain, final item).

**Rationale:** `.ai-factory/ROADMAP.md:63` names K04 as the next critical-chain item after K03. The kernel audit recorded in `.ai-factory/RESEARCH.md:269-276` lists "effect/frame ordering undefined" as a critical blocker for Framework work. `docs/promt.md` §3.1, §4.6, §5 already define the target phase ordering and per-phase budgets — K04 turns that target into a typed, tested contract inside `flui-core` so SF03/SF04/SF05 can schedule against it.

K04 must not implement the Framework tier itself. In particular, it must not add `Widget`, `State<W>`, `StateMap`, reconciliation, dirty lists, `setState`, `InheritedWidget` ergonomics, Theme/MediaQuery, async widgets, or a widget catalogue. K04 may add the minimal scheduling primitives those specs need (e.g., placement-aware `cx.defer`, post-frame callback hook, `FrameClock`), but their public surface stays narrow and engine-owned.

## 10-Year Contract Axioms

These ten axioms are the cross-platform-convergent invariants. They are derived from Flutter's `SchedulerBinding` (stable since 1.0), Compose's `MonotonicFrameClock` + Snapshot system, the HTML Living Standard event-loop / `requestAnimationFrame` model, SwiftUI's `CADisplayLink` + run-loop-observer pipeline, React Scheduler lanes, Bevy ECS Schedule v3, and Glenn Fiedler's injected-clock determinism canon. They MUST hold for the K04 contract to survive SF03–SF08, hot-reload (R-track), Wasm, and headless-CI for the 10-year horizon.

| # | Axiom | Justification (cross-platform precedent) | Consequence for K04 API |
|---|---|---|---|
| **P1** | A frame is a typed, observable state machine, not an unfolded callback chain. | Flutter `SchedulerPhase`, Compose `Recomposer` lifecycle. | `FramePhase` is `pub`, `#[non_exhaustive]`, ordered, and queryable from `App` and every context. |
| **P2** | Frame ordering is a **logical** contract; the implementation may collapse code paths. | Bevy collapses `Update`/`PostUpdate` in one scheduler; React folds commit/passive effects. | Tests assert observable order via markers, not internal field inspection. Implementation may move work between phases without a SemVer break. |
| **P3** | Time is sampled once per logical frame; everything in that frame sees the same `Instant`. | Flutter `currentFrameTimeStamp`, Compose `withFrameNanos`, rAF `DOMHighResTimeStamp`, Fiedler injected clock. | `FrameClock::now()` is the **only** sanctioned time source inside a frame. Direct `Instant::now()` calls in committed phase code are a lint. |
| **P4** | Deadlines describe **policy**, not scheduling guarantees. The OS, GPU, and thermal state decide when frames actually fire. | Flutter exposes no deadline; React per-lane expirations; Bevy reports per-phase times. | Budgets are advisory by default; only `EffectFlush` has "break and re-queue" semantics. `tick`/`layout`/`prepaint`/`paint`/`preFrame`/`postFrame` only **report** overruns. |
| **P5** | Re-entry is admissible only through the documented queue. No phase introduces a second escape. | K15 published; Compose forbids re-entrant composition past a threshold; Flutter forbids `setState` during build. | All phases needing same-target mutation go through `cx.defer_to(...)`. |
| **P6** | Effects are typed by **placement**, not by **target**. | Compose distinguishes `LaunchedEffect`/`SideEffect`/`DisposableEffect` by capability axis. React's lanes are when, not what. | `Effect::Defer { placement, callback }` — never `Effect::DeferForReconciliation`. SF05/SF06/SF08 reuse the placement enum additively. |
| **P7** | A frame is per-window in observable scope, even if some phases are App-wide. | Flutter `WidgetsBinding.drawFrame` per-window; SwiftUI multi-scene; Web one rAF / per-document layout. | `App::run_frame(window_id)` is the entry. App-global `tick` wraps per-window phases, not vice versa. |
| **P8** | Panic in any phase leaves the App recoverable. | Compose/Flutter recover and re-throw to test/inspector; without recovery hot-reload is impossible. | `abort_frame_after_panic` is mandatory, mirrors `abort_update_after_panic` (K07), is part of the published contract. |
| **P9** | The contract compiles and behaves identically under headless / single-threaded / Wasm. | Web supports rAF without OS vsync; iOS supports background scenes; CI fans out. | `FrameClock` is `!Send`; phase code never touches `std::time::Instant` directly (use injected `Clock`). |
| **P10** | The public surface for non-engine consumers must be writable with **only the contract**, not the implementation. | Flutter `WidgetsBindingObserver`, Compose `compositionLocalOf`, React DevTools. | `App::current_phase()` and a reserved phase-subscription hook are part of the contract from day one. |

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `.ai-factory/ARCHITECTURE.md`, `docs/promt.md` §3.1 / §4.6 / §5 / §6, K15/K07/K05/K01/K02/K03 specs, and current `app.rs`, `window.rs`, `reentrancy.rs`, `animation/`, `scheduler/` code. Cross-platform synthesis from Flutter `SchedulerBinding`, Compose `Recomposer` + `MonotonicFrameClock`, HTML rAF + event loop, SwiftUI `CADisplayLink` + run-loop observer, React Scheduler, Bevy ECS Schedule v3, Glenn Fiedler's fixed-timestep canon.

- K99, K15, K07, K05, K01, K02, and K03 are complete. K04 is the last gate before Phase II-F can begin planning.
- The current `Effect` enum has 6 variants (`Notify`, `Emit`, `RefreshWindows`, `NotifyGlobalObservers`, `Defer`, `EntityCreated`) and lives at `crates/flui-core/src/app.rs:2563-2599`. `Notify` and `NotifyGlobalObservers` are deduplicated on insert; the rest are not. `Defer` always queues and is the K15-blessed re-entry escape hatch.
- `App::flush_effects` (`app.rs:1414-1470`) drains `pending_effects: VecDeque<Effect>` in a single unbounded loop and only exits when the queue is empty. There is no deadline, no phase label, no placement awareness. New effects pushed by handlers extend the same drain.
- `App::start_update` / `finish_update` (`app.rs:867-887`) flush effects only at the outermost update (`pending_updates == 1`). K15 re-entrancy guarantees come from this gate plus `window_update_stack` and `currently_updating_entity`.
- `Window::draw` (`window.rs:2379-2465`) advances a private `DrawPhase` enum (`None` → `Prepaint` → `Paint` → `Focus` → `None`) and swaps `next_frame` ↔ `rendered_frame`. There is no `preFrame` / `tick` / `postFrame` / `idle` notion in the engine yet.
- `Window::on_next_frame` (`window.rs:1911-1912`) drains via the platform `on_request_frame` callback at `window.rs:1275-1284` **before** `window.draw()`. Semantically this is a `preFrame` hook today. The misleading comment at `animation/animated.rs:10` ("Users never need to call `window.request_animation_frame()` manually") is symptomatic of the unclear name.
- `Window::request_animation_frame` (`window.rs:1921-1924`) currently schedules a notify on the next draw. Animation ticking is pull-based via `AnimationController` (`crates/flui-core/src/animation/controller.rs:69+`) and the `Ticker` time source (`animation/ticker.rs:44-78`). Per `docs/promt.md` §5 item 1, `AnimationController::value()` re-reads the clock on each call (`animation/controller.rs:233` TODO) — that bug is part of the K04 surface because `FrameClock` is the fix.
- `Platform` trait (`crates/flui-core/src/platform.rs:233-375`) has no `request_animation_frame` / `set_display_link` verb. `PlatformDispatcher` (`platform.rs:761-783`) and `PlatformScheduler` (`platform_scheduler.rs`) abstract OS dispatch but do not own frame pacing. K04 keeps platform-driven frame pacing (status quo) and wraps the existing `on_request_frame` callback.
- `Clock` trait and `TestClock` (`scheduler/clock.rs`, `scheduler/test_scheduler.rs`) already make time injectable. K04 layers `FrameClock` on top — does not introduce a parallel `trait FrameClock`.
- `cx.defer` / `Window::defer` already function as the K15 escape hatch. A precise `cx\.defer\(` callsite count is ~36 across 5 `flui-core` files plus Tier-C consumers. K04 preserves every existing caller's intent — same admission rules, no new failure modes.
- `docs/promt.md` §3.1 frame-budget table is the source of truth for per-phase deadlines:

  | Phase            | Target budget |
  |------------------|---------------|
  | Animation tick   | ≤ 1 ms        |
  | Layout           | ≤ 3 ms        |
  | Prepaint         | ≤ 4 ms        |
  | Paint + present  | ≤ 1 ms        |
  | Gesture dispatch | ≤ 1 ms        |
  | Effect flush     | ≤ 2 ms        |
  | Slack            | ~4 ms         |

- `docs/promt.md` §4.6 sketches a `FrameClock` + `FrameProfile` + budget-aware `flush_effects` shape. K04 ships an evolved version: split `FrameProfile` (always-on, cheap) and `FrameProfileDetailed` (flag-gated, full per-phase Durations).
- Flutter's `setState`-in-`postFrameCallback` footgun (`#147605`) and SwiftUI's `.default` run-loop-mode mistake are explicit 10-year regrets. K04 avoids both by (a) declaring `postFrame` callbacks cannot mutate elements without `defer_to(NextFrameStart, …)`, and (b) treating thermal/input-rate throttles as platform-side concerns that K04 deadlines must not double-throttle.

## Current State

| Area | Current shape | K04 concern / decision |
|---|---|---|
| Effect enum | 6 variants in `Effect` (`app.rs:2563-2599`), `Defer` is `Box<dyn FnOnce(&mut App)>` | **Decision (P6):** extend the existing `Defer` variant with a `placement: DeferPlacement` field. Single variant, not `Effect::DeferTo` sibling. |
| Effect flush | Single unbounded loop in `flush_effects` (`app.rs:1414-1470`) | **Decision (P4):** per-phase, FIFO-per-phase, deadline-aware drain. Effect-flush phase uses break-and-requeue; other phases only record overruns. |
| Dedup | `pending_notifications: FxHashSet<EntityId>` + `pending_global_notifications: FxHashSet<TypeId>` | Must remain first-insert-wins under placement splits. Placement does NOT introduce a new dedup dimension. |
| Drain gating | `App::start_update`/`finish_update` (`app.rs:867-887`), `flushing_effects` bool | Phase entry/exit integrates with this gate; `flushing_effects` is preserved or replaced with a phase-keyed equivalent. |
| Draw phases | `DrawPhase { None, Prepaint, Paint, Focus }` in `window.rs:1078-1083` | **Decision:** strict sub-state of `FramePhase::{Prepaint, Paint}`. `Focus` folds into tail of `Paint`. |
| "Next-frame" hook (today's semantics) | `on_next_frame(callback)` runs **before** `window.draw()` (`window.rs:1275-1284`); `Rc<RefCell<Vec<FrameCallback>>>` at `window.rs:960`; three wrappers (`Window`, `Context`, `AsyncWindowContext`). | **Decision:** rename `Window::on_next_frame` → `Window::on_pre_frame` (deprecated alias kept). Add new `Window::on_post_frame` anchored at `Window::complete_frame` (`window.rs:2372`). Both lifted to App level (`App::on_pre_frame` / `App::on_post_frame`). Storage replaced with `SmallVec<[FrameCallback; 4]>` directly on `Window` (the `Rc<RefCell<…>>` was needed only because the platform callback drained a clone; the new `run_frame` path obviates that). |
| Platform-side frame pacing | `on_request_frame` callback (`window.rs:1257-1314`) wraps `next_frame_callbacks` drain, `thermal_state` 60 Hz throttle (`window.rs:1262-1273`), `input_rate_tracker` high-rate detection (`window.rs:1256, 1291`), `force_render` / `request_frame_options` (`window.rs:1289-1293`), `measure("frame duration", ...)` wrapper, and `complete_frame` (`window.rs:2372`). | **Decision:** `App::run_frame(window_id)` wraps (does not replace) this callback. Thermal / input-rate / force-render all preserved; K04 deadlines layer **inside** the platform-fired frame, never double-throttle. |
| Test-mode flush divergence | Inside `flush_effects`, a `#[cfg(any(test, feature = "test-support"))]` block at `app.rs:1450-1462` re-draws dirty windows before declaring the drain complete. | **Decision:** add `TestApp::advance_frame()` / `advance_frames(n)` as the canonical path. Keep the auto-redraw behind `App::auto_advance_frames_on_flush` flag (default `true` in `cfg(test)` for back-compat). Deprecate the flag in K04+1; legacy tests keep working. |
| Panic recovery | `App::abort_update_after_panic` (`app.rs:870-874`) restores `pending_updates` and clears `flushing_effects`. | **Decision (P8):** `App::abort_frame_after_panic(phase)` restores `current_phase = Idle`, resets `flushing_effects`, clears `next_frame` buffer, but leaves `frame_clock`, active-animation set, and remaining effects "stuck dirty" — they tick again next frame. |
| Animation tick | Pull-based: `AnimationController::value()` re-reads clock on each call (`animation/controller.rs:233`) | **Decision:** `App::active_animations` set abstracted as `trait TickTarget` (sealed); `value()` caches `(frame_index, sampled_value)` per controller. |
| Animation request | `Window::request_animation_frame` schedules a notify-on-next-frame (`window.rs:1921-1924`) | **Decision:** `Window::request_next_frame: Cell<bool>` flag — multiple calls coalesce idempotently. Closure-queue retained only for explicit `on_pre_frame` callbacks. |
| Re-entrancy (K15) | `ReentryError::NestedWindowUpdate`, `EntityMap::double_lease_panic`, `cx.defer` escape hatch (`reentrancy.rs`) | **Decision (P5):** K15 + K04 publish a single joint contract paragraph at the top of `reentrancy.rs`. `cx.defer` remains the only escape; placement defaults to `EndOfUpdate` for back-compat. |
| Platform / vsync | `PlatformWindow::draw(&scene)` (`window.rs:2490-2493`), no `request_animation_frame` verb on `Platform` | **Decision:** no new `Platform` verb in K04. Reserved as design-spec note for R-track (Wasm/iOS scenes). |
| Scheduler API | `Scheduler`, `BackgroundExecutor`, `ForegroundExecutor`, `TestScheduler`, `Clock` already exist | K04 reuses them; no new executor trait. `FrameClock` is a **struct** layered on `Arc<dyn Clock>`. |
| Tests | No explicit effect-order / drain / phase tests; some `cx_defer_avoids_reentry_panic` coverage (`reentrancy.rs:526-550`) | New focused test module asserts ordering, dedup, deadline behavior, K15 escape integrity, panic-in-phase recovery, animation-tick determinism. Uses `TestApp::advance_frame`. |
| Telemetry | None | **Decision:** two-layer. `FrameProfile` (always-on, ~32 bytes, in prelude) + `FrameProfileDetailed` (flag-gated, full per-phase `[Duration; FramePhase::COUNT]`). |

## Target Design Direction

The exact public names and structs are frozen by `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md` before code. Per the 10-Year Contract Axioms (§ above) and cross-platform synthesis, the direction is:

1. **Phase model — 7 logical phases plus reserved `Build` slot:**
   `Idle → PreFrame → AnimationTick → Build (reserved, no-op in K04) → Layout → Prepaint → Paint → PostFrame → Idle`.
   `Build` is reserved between `AnimationTick` and `Layout` for SF05's `BuildOwner::flush_dirty()`. Reserving costs nothing today and prevents an enum addition in SF05. (Mirrors Flutter `transientCallbacks` → `buildOwner.buildScope` separation.) `EffectFlush` is **interleaved** at phase boundaries — not a phase itself.
2. **Frame ownership — `App::run_frame(window_id) -> FrameOutcome`** as the single seven-phase entry. Wraps (does not replace) the platform `on_request_frame` callback. `Window::draw` becomes its `Prepaint` + `Paint` body. `AnimationTick`, `PreFrame`, `PostFrame` become App-owned wrappers.
3. **FrameClock — App-level, `!Send`, opaque type.** `App::frame_clock: FrameClock` samples `Clock::now()` once per `run_frame`. `Window::frame_clock_view() -> FrameClockView` is an opaque indirection that returns the App view today; per-window epochs (Wasm tab pause, iOS background scene) can layer on top without an API break.
4. **Placement-aware `Effect::Defer` — single variant with a placement field:**
   ```rust
   Defer { placement: DeferPlacement, callback: Box<dyn FnOnce(&mut App) + 'static> }
   ```
   `DeferPlacement = { EndOfUpdate, NextFrameStart, PostFrame, Idle }`, `#[non_exhaustive]`. The default for the existing `App::defer(f)` keeps current observable behavior (`EndOfUpdate`); no Tier-C callsite breaks. SF05/SF06/SF08 add variants additively.
5. **Per-phase deadline classes (P4):**
   - **Advisory** for `PreFrame`, `AnimationTick`, `Layout`, `Prepaint`, `Paint`, `PostFrame`: record overrun → emit one rate-limited `warn!` per phase per frame → phase still runs to completion. Aborting mid-paint corrupts scene state irrecoverably.
   - **Break-and-requeue** for `EffectFlush` only: each effect is atomic, so re-queue remainder onto next frame's effect flush.
   - **Hard** mode: reserved for SF05 worst-case rebuild storms. Not active in K04.
6. **Drain order:** existing dedup invariants (first-insert-wins for `Notify`/`NotifyGlobalObservers`) preserved verbatim. Per-phase drains are FIFO, deterministic, identical between `TestScheduler` and production.
7. **Animation tick as a real phase:** `App::active_animations: FxHashSet<TickTargetId>` (sealed `trait TickTarget`) walks once per frame, advances each controller via `FrameClock::now()`, emits `Effect::Notify`. `AnimationController::value()` caches `(frame_index, sampled_value)` and exposes the same public signature — TODO at `animation/controller.rs:233` closed.
8. **Pre/post-frame split:** `Window::on_next_frame` **renamed** to `Window::on_pre_frame` (deprecated alias retained); new `Window::on_post_frame` anchored at `Window::complete_frame`. App-level `App::on_pre_frame` / `App::on_post_frame` is the contract surface; Window-level is convenience. Storage replaced with `SmallVec<[FrameCallback; 4]>` directly on `Window` — no `Rc<RefCell<…>>` on hot path.
9. **`FrameProfile` — two-layer telemetry:** `FrameProfile` (always-on, ~32 bytes, `pub` in prelude, `#[non_exhaustive]`) + `FrameProfileDetailed` (flag-gated via `App::profiling_enabled`, full per-phase `Duration` array). `App::frame_profile()` accessor is public read-only.
10. **Platform contract:** no new `Platform` verb in K04. `Platform::request_animation_frame` reserved as design-spec note for R-track / Wasm. Frame entry stays driven by existing `PlatformWindow::draw` callback chain.

## Key Design Decisions (frozen)

Cross-platform synthesis produced explicit answers for every question that was open in the first-pass plan. The design spec must record each with rationale; the table below is the binding shortlist.

| Decision | Frozen value | Cross-platform precedent |
|---|---|---|
| Phase count | 7 + reserved `Build` | Flutter 5-phase stable since 1.0; Compose folds via `Recomposer`; reserving Build avoids SF05 SemVer break |
| Effect partitioning | Single queue with placement discriminator | Bevy `apply_deferred` model; doubles match arms otherwise |
| `Defer` default placement | `EndOfUpdate` — back-compat for all 36 callsites | React's "low priority by default"; Flutter `scheduleMicrotask` analogous |
| `Effect::Defer` enum shape | Extend variant with `placement` field, not sibling | Compose's `LaunchedEffect` family with internal phase routing |
| FrameClock ownership | `App`-level, opaque type, `Window::frame_clock_view()` indirection | Flutter `WidgetsBinding.currentFrameTimeStamp`; Compose `MonotonicFrameClock` |
| Multi-window | Single App-wide clock; `FrameClockView` allows future per-window epochs | Flutter (one clock across scenes); Web (one rAF / per-document layout) |
| Animation tick scheduling | `App::active_animations` set, abstracted as sealed `trait TickTarget` | Compose `Recomposer` awaiter set; Unity custom-update-manager (opt-in tick) |
| Deadline overrun (non-effect) | Advisory: log + record in `FrameProfile.overruns`; never abort phase | Flutter no deadline exposed; React expiration-based scheduling |
| Deadline overrun (effect) | Break-and-requeue with one rate-limited `warn!` | React's `shouldYield()` model; per `docs/promt.md` §4.6 |
| Telemetry surface | Two-layer: `FrameProfile` (always) + `FrameProfileDetailed` (flag) | Flutter `FrameTiming` from `addTimingsCallback`; Compose nothing built-in |
| Test harness | Add `TestApp::advance_frame()` / `advance_frames(n)`; auto-redraw retained behind `auto_advance_frames_on_flush` flag (default true in cfg(test)), deprecated in K04+1 | Compose `TestMonotonicFrameClock`; React `act()` |
| K15 contract | Joint paragraph at top of `reentrancy.rs`; `cx.defer` remains only escape | K15 already published; SwiftUI `withMutation` analogue |
| `on_next_frame` semantics | Rename to `on_pre_frame` with `#[deprecated]` alias; add new `on_post_frame` | Flutter `addPostFrameCallback` is distinct from `scheduleFrameCallback`; misleading name was a Flutter footgun avoided here |
| `postFrame` API location | Both: App-level contract, Window-level wrapper | Flutter `WidgetsBinding.instance.addPostFrameCallback` (global); Element-level is sugar |
| Test-mode flush draw | `advance_frame` canonical; auto-redraw behind flag, deprecated K04+1 | None — this is a flui-specific divergence cleanup |
| Panic safety | `abort_frame_after_panic` restores phase + `flushing_effects` + `next_frame` clear; active-set and effect queue left "stuck dirty" | K07/K15 precedent; required for hot-reload (R-track) |
| Platform throttles | K04 deadlines never double-throttle thermal / input-rate / force-render | All platforms: scheduler decides if frame fires, framework decides what runs in it |
| `request_animation_frame` idempotence | `Window::request_next_frame: Cell<bool>` flag; coalesce | Web rAF debouncing; Flutter `_hasScheduledFrame` debouncer |
| Public API surface | `FramePhase` + `FrameProfile` in prelude; `DeferPlacement`, `FrameClock` explicit-import | Flutter `SchedulerPhase` exported; React lanes internal |
| Reserved-for-future | `FramePhase::HotReload`, `Platform::request_animation_frame`, `Effect::DeferAsync`, per-window `FrameClockView` epochs | Design-spec notes only; not code |

## Review Gates

Before PR merge:

- `flui-arch-reviewer` for the `App` / `Window` / scheduler / `Effect` boundary and the seven-phase contract.
- `migration-risk-adversary` because K04 alters effect ordering and `defer` semantics — both load-bearing for every Tier-C `cx.defer` callsite, plus the `on_next_frame` rename touches Tier-C examples.
- `rust-api-migration-auditor` for any public additions (`FrameClock`, `FrameClockView`, `DeferPlacement`, `FramePhase`, `FrameProfile`, `FrameProfileDetailed`), prelude/re-export changes, and trait-object decisions on `trait TickTarget`.
- `wgpu-gpu-reviewer` only if implementation unexpectedly touches `scene.rs`, `platform/wgpu`, Metal/DirectX renderers, or shader modules.

## Commit Plan

- **Commit 1** (after Tasks 1-14): `docs: specify k04 effect/frame contract (axioms + decisions)`
- **Commit 2** (after Tasks 15-28): `feat(core): introduce frame phases, frame clock, placement-aware defer, panic safety`
- **Commit 3** (after Tasks 29-37): `feat(core): wire animation tick, pre/post-frame, telemetry`
- **Commit 4** (after Tasks 38-43): `test(core): cover k04 phase order, deadlines, reentrancy, panic`
- **Commit 5** (after Tasks 44-50): `docs: document k04 migration, rename on_next_frame, status updates`

## Tasks

### Phase 1: Design, Inventory, and Scope Freeze

- [x] Task 1: Inventory current effect and frame surfaces.
  - Deliverable: tabular inventory covering `Effect` enum + variants, `push_effect`, `flush_effects`, `App::defer`, `App::start_update`/`finish_update`, `Window::draw`, `Window::on_next_frame`, `Window::request_animation_frame`, `DrawPhase`, `Window::next_frame_callbacks`, animation-controller tick path, scheduler/executor types, K15 fields, and the platform `on_request_frame` callback wrapper.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/reentrancy.rs`, `crates/flui-core/src/animation/controller.rs`, `crates/flui-core/src/animation/ticker.rs`, `crates/flui-core/src/scheduler/mod.rs`, `crates/flui-core/src/scheduler/clock.rs`, `crates/flui-core/src/scheduler/test_scheduler.rs`, `crates/flui-core/src/platform.rs`, `crates/flui-core/src/platform_scheduler.rs`, `crates/flui-core/src/executor.rs`.
  - Logging requirements: no runtime logs. Inventory evidence belongs in the design spec.

- [x] Task 2: Inventory all `cx.defer` / `Window::defer` / `on_next_frame` / `request_animation_frame` callsites.
  - Deliverable: callsite table with file:line and category (focus, action toggle, scroll, window activation, observer re-entry guard, animation request, pre/post-frame side effect, other). Tag each with target placement (`EndOfUpdate`, `NextFrameStart`, `PostFrame`, `Idle`). Mark which callsites the rename of `on_next_frame` → `on_pre_frame` will touch.
  - Method: precise regex (`cx\.defer\(`, `\.defer_to\(`, `Window::defer\(`, `on_next_frame\(`, `request_animation_frame\(`) to avoid `flushing_effects` false positives. A workspace-wide scan returns ~36 hits across 5 files in `flui-core` alone; Tier-C adds more.
  - Files: workspace-wide grep across `crates/flui-core/src/**/*.rs`, `crates/flui-widgets/src/**/*.rs`, `crates/flui-material/src/**/*.rs`, `crates/flui-navigator/src/**/*.rs`, `crates/flui-core/examples/**/*.rs`, `examples/**/*.rs`.
  - Logging requirements: none.

- [x] Task 3: Author the K04 design spec.
  - Deliverable: `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md` covering: the 10-Year Contract Axioms (P1–P10), the seven-phase contract (with reserved `Build` slot), the joint K15+K04 re-entrancy paragraph, every Key Design Decision (frozen table above) with rationale, rejected alternatives, telemetry strategy, migration plan, and the forward-hook reservations for hot-reload / Wasm / multi-window.
  - Logging requirements: spec must document the no-per-element / no-per-frame log invariant and explicitly list which committed log statements are allowed (effect-flush overruns, profile-enabled per-frame info).

- [x] Task 4: Freeze the phase ordering and observable contract.
  - Deliverable: spec section enumerating `Idle → PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame → Idle` with: predecessor/successor invariants, allowed effect placements per phase, K15 re-entrancy class (inside or outside `start_update`), allowed `Window` mutations, deadline class (advisory / break-and-requeue / reserved-hard), and one-line examples of work that belongs there.
  - Logging requirements: none; observable order proven via tests in Phase 4.

- [x] Task 5: Reserve `FramePhase::Build` as a no-op slot for SF05.
  - Deliverable: spec section documenting `Build` as runtime-no-op in K04 — phase enters and exits immediately, drains no effects, advances no state. SF05 (`setState` + dirty-list) fills it with `BuildOwner::flush_dirty()`. Reserving avoids an enum addition in SF05 that would otherwise be a SemVer break under `#[non_exhaustive]`.
  - Logging requirements: none.

- [x] Task 6: Resolve `on_next_frame` pre-draw vs post-draw semantics — RENAME + ADD.
  - Deliverable: spec decision: (a) rename `Window::on_next_frame` → `Window::on_pre_frame` with `#[deprecated]` alias kept for one release cycle; (b) add new `Window::on_post_frame` anchored at `Window::complete_frame` (`window.rs:2372`). All three wrappers (`Window`, `Context`, `AsyncWindowContext`) move together. Misleading comment at `animation/animated.rs:10` updated.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/app/async_context.rs`, `crates/flui-core/src/animation/animated.rs`.
  - Logging requirements: none.

- [x] Task 7: Add App-level `on_pre_frame` / `on_post_frame` contract surface.
  - Deliverable: spec section establishing `App::on_pre_frame(FnOnce(&mut App))` and `App::on_post_frame(FnOnce(&mut App))` as the canonical contract; Window-level wrappers are sugar. Multi-window apps need App-level for cross-window post-frame work (input replay, telemetry export). Storage strategy (per-App `SmallVec`) specified.
  - Files: `crates/flui-core/src/app.rs`.
  - Logging requirements: none.

- [x] Task 8: Freeze `Effect::Defer` placement model — single variant with placement field.
  - Deliverable: spec section: `Effect::Defer { placement: DeferPlacement, callback: Box<dyn FnOnce(&mut App) + 'static> }`. `DeferPlacement` enum is `#[non_exhaustive]`, initial values `{ EndOfUpdate, NextFrameStart, PostFrame, Idle }`. `App::defer(f)` keeps current observable behavior by routing to `EndOfUpdate`. New API `App::defer_to(placement, f)`. Justify against the Task 2 callsite inventory: zero forced migrations.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: none.

- [x] Task 9: Freeze `FrameClock` ownership — App-level, opaque, with `Window::frame_clock_view()` indirection.
  - Deliverable: `App::frame_clock: FrameClock` is `!Send`, wraps `Arc<dyn Clock>`, samples once per `run_frame`. Public methods: `now() -> Instant`, `frame_index() -> u64`, `delta() -> Duration`, `in_frame() -> bool`. Outside `in_frame()`, `now()` panics in debug, returns last-sampled in release. `Window::frame_clock_view() -> FrameClockView` is opaque; today returns App's view; reserved for future per-window epochs (Wasm tab pause, iOS background scene).
  - Files: candidate new module `crates/flui-core/src/frame/clock.rs`, glue in `crates/flui-core/src/app.rs` and `crates/flui-core/src/window.rs`.
  - Logging requirements: none.

- [x] Task 10: Freeze deadline-class taxonomy — advisory / break-and-requeue / hard-reserved.
  - Deliverable: spec section with the deadline-class table:
    - **Advisory**: `PreFrame`, `AnimationTick`, `Layout`, `Prepaint`, `Paint`, `PostFrame` — record overrun in `FrameProfile.overruns: FramePhaseSet`, emit one rate-limited `warn!`, phase runs to completion.
    - **Break-and-requeue**: `EffectFlush` only — atomic work units, safe to interrupt and re-queue remainder.
    - **Hard** (reserved): SF05 worst-case rebuild storms. Not active in K04.
  - Justify against `docs/promt.md` §3.1 budgets; cite cross-platform precedent (Flutter no deadline, React expiration-based, Bevy reports times).
  - Logging requirements: spec must say overrun log is at most one `WARN` per phase per frame, never per element.

- [x] Task 11: Freeze animation-tick scheduling — `App::active_animations` + sealed `trait TickTarget`.
  - Deliverable: spec for `App::active_animations: FxHashSet<TickTargetId>`, populated by `AnimationController::start`/`stop`. `trait TickTarget` is sealed (closed to extension outside flui-core) so `AnimationController` is the only K04 implementor; SF08 (async widgets) and future audio/spring controllers add impls additively. `tick` phase walks the set once per frame, calls `target.tick(frame_clock.now())`, emits `Effect::Notify` for changed targets. `AnimationController::value()` caches `(frame_index, sampled_value)`; public signature unchanged.
  - Files: `crates/flui-core/src/animation/controller.rs`, `crates/flui-core/src/animation/ticker.rs`, `crates/flui-core/src/app.rs`, candidate `crates/flui-core/src/frame/tick.rs`.
  - Logging requirements: no per-tick logs.

- [x] Task 12: Freeze panic-safety contract.
  - Deliverable: spec section for `App::abort_frame_after_panic(phase: FramePhase)`. Restores: `current_phase = Idle`, `flushing_effects = false`, `next_frame` buffer cleared. Leaves "stuck dirty": active-animation set (controllers tick again next frame), remaining effect queue (drains next frame), `frame_clock` (does not roll forward). Mirrors `abort_update_after_panic` (`app.rs:870-874`).
  - Files: `crates/flui-core/src/app.rs`, candidate `crates/flui-core/src/frame/mod.rs`.
  - Logging requirements: none in committed code beyond existing K15/panic-hook output.

- [x] Task 13: Freeze test-mode divergence policy — `advance_frame` canonical, auto-redraw flag-gated.
  - Deliverable: spec decision: add `TestApp::advance_frame()` / `TestApp::advance_frames(n)` as canonical test entry. Add `App::auto_advance_frames_on_flush: bool` (default `true` in `cfg(test)`, `false` elsewhere). The `#[cfg(any(test, feature = "test-support"))]` block at `app.rs:1450-1462` checks the flag. Flag is `pub` so phase-order tests can opt out. Deprecation timeline: K04+1 flips default to `false` after Tier-C tests migrate.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/test_app.rs`.
  - Logging requirements: none.

- [x] Task 14: Freeze review gates, K15 joint contract, platform-throttle coexistence.
  - Deliverable: spec section with: (a) review checklist (`flui-arch-reviewer` / `migration-risk-adversary` / `rust-api-migration-auditor`); (b) the K15+K04 joint contract paragraph for `reentrancy.rs` — `cx.defer` is the only re-entry escape, placement determines run-phase of deferred callbacks; (c) explicit coexistence with `thermal_state` 60 Hz throttle (`window.rs:1262-1273`), `input_rate_tracker` (`window.rs:1256, 1291`), `force_render` / `request_frame_options` (`window.rs:1289-1293`) — K04 deadlines apply only inside frames that platform allowed to fire; never double-throttle.
  - Files: design spec; cross-references to `crates/flui-core/src/app.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/reentrancy.rs`.
  - Logging requirements: none.

### Phase 2: Core Frame Pipeline, Effect Placement, Panic Safety

- [x] Task 15: Introduce the K04 `frame` module skeleton.
  - Deliverable: new `crates/flui-core/src/frame/mod.rs` exporting `FramePhase` (`#[non_exhaustive]`), `DeferPlacement` (`#[non_exhaustive]`), `FrameClock`, `FrameClockView`, `FrameProfile`, `FrameProfileDetailed`, sealed `trait TickTarget`. Submodules: `frame/clock.rs`, `frame/profile.rs`, `frame/tick.rs`. Public surface curated via `crates/flui-core/src/lib.rs`.
  - Files: candidate new modules, `crates/flui-core/src/lib.rs`.
  - Logging requirements: module-level rustdoc must restate the no-per-element / no-per-frame log invariant.

- [x] Task 16: Implement `FramePhase` and `DeferPlacement` enums.
  - Deliverable: both enums `pub`, `#[non_exhaustive]`, `Copy + Clone + Debug + PartialEq + Eq + Hash`. `FramePhase::COUNT` constant for array sizing. `FramePhase` includes `Idle, PreFrame, AnimationTick, Build, Layout, Prepaint, Paint, PostFrame`. `DeferPlacement` includes `EndOfUpdate, NextFrameStart, PostFrame, Idle`. Both implement `Ord` for stable iteration.
  - Files: `crates/flui-core/src/frame/mod.rs`.
  - Logging requirements: none.

- [x] Task 17: Implement `FrameClock` + `FrameClockView`.
  - Deliverable: `FrameClock { clock: Arc<dyn Clock>, sampled: Option<Instant>, frame_index: u64, last_delta: Duration }`. Methods: `begin_frame(now)`, `now()`, `frame_index()`, `delta()`, `in_frame()`. `FrameClockView` is `#[derive(Copy, Clone)]` newtype with the same accessors, returned by `Window::frame_clock_view()`. Layered on top of, not replacing, `measure("frame duration", ...)` (`window.rs:1294`) and `#[profiling::function]` (`window.rs:2378`).
  - Files: `crates/flui-core/src/frame/clock.rs`, glue in `crates/flui-core/src/app.rs` and `crates/flui-core/src/window.rs`.
  - Logging requirements: none.

- [x] Task 18: Add placement-aware deferred-effect API across all four contexts.
  - Deliverable: `App::defer_to(placement, f)`, `Context::defer_to(placement, f)`, `Window::defer_to(placement, f)`, `AsyncWindowContext::defer_to(placement, f)` in lockstep. Existing `App::defer(f)` / `cx.defer(...)` keeps current observable behavior by routing to `Effect::Defer { placement: EndOfUpdate, callback: Box::new(f) }`.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/async_context.rs`.
  - Logging requirements: none.

- [x] Task 19: Modify the `Effect` enum to carry placement on `Defer`.
  - Deliverable: `Effect::Defer { placement: DeferPlacement, callback: Box<dyn FnOnce(&mut App) + 'static> }`. Preserve `Notify`/`NotifyGlobalObservers` dedup and all other variants. Update every `match effect` callsite to the new field; no behavior change for non-`Defer` variants.
  - Files: `crates/flui-core/src/app.rs`, all `match effect` callsites (including `flush_effects` and the test-mode block).
  - Logging requirements: none.

- [x] Task 20: Refactor `flush_effects` into a per-phase, deadline-aware drain.
  - Deliverable: phase-keyed FIFO drain that preserves dedup, applies break-and-requeue when `EffectFlush` budget exceeded, never violates K15 re-entrancy. Existing `flushing_effects` guard preserved or replaced with phase-keyed equivalent. Coordinates with Task 41 (K15 coexistence tests). Single rate-limited `WARN` per overrun.
  - Files: `crates/flui-core/src/app.rs`.
  - Logging requirements: committed code may emit one `log::warn!` per phase per frame when budget exceeded; no per-effect logs.

- [x] Task 21: Specify and migrate the test-mode flush-time draw policy.
  - Deliverable: implement `App::auto_advance_frames_on_flush: bool` per Task 13. The `#[cfg(any(test, feature = "test-support"))]` block at `app.rs:1450-1462` checks the flag; default `true` in `cfg(test)`, `false` elsewhere. Existing test suite passes unchanged. Document the deprecation timeline (K04+1 flips default).
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/test_app.rs`.
  - Logging requirements: none in committed runtime code.

- [x] Task 22: Add `TestApp::advance_frame()` / `advance_frames(n)` / `set_auto_advance_frames` / `frame_profile()`.
  - Deliverable: `TestApp::advance_frame()` calls `App::run_frame(window_id)` for the test window, returns `FrameOutcome`. `advance_frames(n)` iterates. `set_auto_advance_frames(bool)` toggles the flag. `frame_profile() -> &FrameProfile` reads always-on telemetry. All gated behind `feature = "test-support"`.
  - Files: `crates/flui-core/src/app/test_app.rs`.
  - Logging requirements: none.

- [x] Task 23: Introduce `App::run_frame` as the seven-phase entry point.
  - Deliverable: `App::run_frame(window_id) -> FrameOutcome` that walks `PreFrame → AnimationTick → Build (no-op) → Layout → Prepaint → Paint → PostFrame`, drives `FrameClock`, populates `FrameProfile`. Wraps (not replaces) the existing platform `on_request_frame` callback (`window.rs:1257-1314`), preserving `thermal_state`, `input_rate_tracker`, `force_render` / `request_frame_options`, `measure("frame duration", ...)`, and `complete_frame`. The legacy `Window::draw` body migrates to `Prepaint` + `Paint` phases.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: no per-phase logs beyond overruns.

- [x] Task 24: Reconcile `Window::DrawPhase` with K04 `FramePhase`.
  - Deliverable: keep the internal `DrawPhase` enum as strict sub-state of `FramePhase::{Prepaint, Paint}`; fold `Focus` into the tail of `Paint`. `Window::draw` callers (root render, inspector, deferred draws) see no observable behavior change.
  - Files: `crates/flui-core/src/window.rs`.
  - Logging requirements: none.
  - **K04 staged rollout note:** as of Task 23 the minimal `App::run_frame` calls `window.draw()` inside `FramePhase::Paint`. `Window::DrawPhase::{Prepaint, Paint, Focus}` continue to advance internally as before — they are already strict sub-states of K04 `FramePhase::{Prepaint, Paint}`. No observable behavior change. A follow-up refactor will split `window.draw()` into a `Prepaint`-phase pass (bounds/hitbox/interactivity paint) and a `Paint`-phase pass (scene primitives + present) once the layout cache (K20) gates the prepaint output; that split is out of K04 scope.

- [x] Task 25: Land panic-safe phase wind-down.
  - Deliverable: `App::abort_frame_after_panic(phase)` per Task 12. Wired into the same `catch_unwind` / `Drop` guards used by K15. Restores phase / `flushing_effects` / `next_frame`; leaves active-set and effect queue stuck-dirty.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/frame/mod.rs`.
  - Logging requirements: no per-panic logs beyond existing K15 / panic-hook output.

- [x] Task 26: Add `App::current_phase()` accessor.
  - Deliverable: `App::current_phase() -> FramePhase` returns the current phase (or `Idle` outside `run_frame`). `pub`. Cheap (one field read). K22 inspector and Framework-tier tests need this. Reserve `App::observe_phase(...)` as a design-spec note (not implemented in K04).
  - Files: `crates/flui-core/src/app.rs`.
  - Logging requirements: none.

- [x] Task 27: Add `App::frame_profile()` accessor and `set_profiling_enabled`.
  - Deliverable: `App::frame_profile() -> &FrameProfile` returns the always-on profile (cheap). `App::frame_profile_detailed() -> Option<&FrameProfileDetailed>` returns the flag-gated detailed view. `App::set_profiling_enabled(bool)` toggles the detailed flag (default `cfg!(debug_assertions)`).
  - Files: `crates/flui-core/src/app.rs`.
  - Logging requirements: none.

- [ ] Task 28: Update prelude, `lib.rs` re-exports, and public docstrings.
  - Deliverable: prelude additions: `FramePhase`, `FrameProfile`. NOT in prelude: `DeferPlacement`, `FrameClock`, `FrameClockView`, `FrameProfileDetailed`, `TickTarget` (explicit import when needed). Public rustdoc on `App::defer*`, `Window::on_pre_frame` (renamed), `Window::on_post_frame` (new), `Window::request_animation_frame`, and `AnimationController::value()` aligns with the new contract.
  - Files: `crates/flui-core/src/lib.rs`, `crates/flui-core/src/prelude.rs`, `crates/flui-core/src/app.rs`, `crates/flui-core/src/window.rs`.
  - Logging requirements: docstrings must mention the no-per-element / no-per-frame log invariant where the new APIs are documented.
  - **K04 staged rollout note:** Prelude additions (`FramePhase`, `FrameProfile`) landed alongside Task 27 (`crates/flui-core/src/prelude.rs`). `lib.rs` exposes the `frame` module as `pub mod frame`. The remaining rustdoc updates on `on_pre_frame` / `on_post_frame` / `request_animation_frame` wait for Tasks 33-35 to land the corresponding APIs (the rename + new `on_post_frame` callbacks). Keep unchecked until those tasks finish.

### Phase 3: Animation Tick, Pre/Post-Frame, Telemetry

- [ ] Task 29: Implement sealed `trait TickTarget`.
  - Deliverable: `pub trait TickTarget: sealed::Sealed { fn tick(&mut self, now: Instant) -> TickOutcome; fn id(&self) -> TickTargetId; }`. `TickOutcome { Continue, Done }` signals whether the target stays in the active set after this tick. `AnimationController` implements it. Sealed via private supertrait so Tier-C cannot add impls in K04 (future opening is additive).
  - Files: `crates/flui-core/src/frame/tick.rs`, `crates/flui-core/src/animation/controller.rs`.
  - Logging requirements: none.

- [ ] Task 30: Land the active-animation-controller set.
  - Deliverable: `App::active_animations: FxHashSet<TickTargetId>` populated by `AnimationController::start` / `stop`. The `AnimationTick` phase walks the set, calls `TickTarget::tick(frame_clock.now())`, removes `Done` entries, emits `Effect::Notify` for `Continue` entries that changed. Sequential dependency on Task 17 (`FrameClock`) and Task 29 (`TickTarget`). References existing callers at `animation/animated.rs:30`, `elements/animation.rs:210`, `elements/img.rs:371` for compat verification. `assets.rs::ImageFrame::frame_index` is an animated-image data concept, distinct from the scheduler frame index.
  - Files: `crates/flui-core/src/animation/controller.rs`, `crates/flui-core/src/animation/ticker.rs`, `crates/flui-core/src/animation/animated.rs`, `crates/flui-core/src/elements/animation.rs`, `crates/flui-core/src/app.rs`.
  - Logging requirements: no per-tick logs.

- [ ] Task 31: Make `AnimationController::value()` `FrameClock`-aware via per-frame cache.
  - Deliverable: `AnimationController` adds `cached_at_frame: Option<(u64, f32)>`. First `value()` call in frame N computes and caches; subsequent reads in frame N return the cache. Public signature unchanged. Closes the TODO at `animation/controller.rs:233` and `docs/promt.md` §5 item 1.
  - Files: `crates/flui-core/src/animation/controller.rs`.
  - Logging requirements: none.

- [ ] Task 32: Make `Window::request_animation_frame` idempotent via `Cell<bool>`.
  - Deliverable: `Window::request_next_frame: Cell<bool>` replaces the per-call closure push. `request_animation_frame()` sets the flag; the platform `on_request_frame` callback drains it. Multiple calls coalesce. Coexists with `request_frame_options.force_render` (`window.rs:1289-1293`). Existing callers at `elements/animation.rs:210`, `elements/img.rs:371`, `animation/animated.rs:30` keep working.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/app.rs`.
  - Logging requirements: none.

- [ ] Task 33: Rename `Window::on_next_frame` → `Window::on_pre_frame` with deprecated alias.
  - Deliverable: rename across all three wrappers — `Window::on_pre_frame` (`window.rs:1911`), `Context::on_pre_frame` (`app/context.rs:292`), `AsyncWindowContext::on_pre_frame` (`app/async_context.rs:311`). Keep `Window::on_next_frame` as `#[deprecated(since = "K04 release", note = "renamed to on_pre_frame")]` alias forwarding to the new name. Update the misleading comment at `animation/animated.rs:10`.
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/app/async_context.rs`, `crates/flui-core/src/animation/animated.rs`.
  - Logging requirements: none.

- [ ] Task 34: Add `Window::on_post_frame` anchored at `Window::complete_frame`.
  - Deliverable: new `Window::on_post_frame(callback)` (and mirror wrappers on `Context`, `AsyncWindowContext`). Stored in a per-Window `SmallVec<[FrameCallback; 4]>`. Drained in the `PostFrame` phase of `run_frame`, after `window.complete_frame()` (`window.rs:2372`).
  - Files: `crates/flui-core/src/window.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/app/async_context.rs`.
  - Logging requirements: none.

- [ ] Task 35: Add App-level `App::on_pre_frame` / `App::on_post_frame`.
  - Deliverable: `App::on_pre_frame(FnOnce(&mut App))` and `App::on_post_frame(FnOnce(&mut App))`. Fire at the top / bottom of any `run_frame` (across all windows). Storage per Task 7 design. Multi-window apps use these for cross-window callbacks.
  - Files: `crates/flui-core/src/app.rs`.
  - Logging requirements: none.

- [ ] Task 36: Replace `next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>>` with `SmallVec<[FrameCallback; 4]>`.
  - Deliverable: on `Window`, replace the `Rc<RefCell<Vec<...>>>` (`window.rs:960`) with `SmallVec<[FrameCallback; 4]>`. The `Rc<RefCell<…>>` was needed only because the platform callback drained from a clone; the new `run_frame` path owns the drain. Satisfies `docs/promt.md` §3.1 / §5 item 7 hot-path rule.
  - Files: `crates/flui-core/src/window.rs`.
  - Logging requirements: none.

- [ ] Task 37: Land `FrameProfile` + `FrameProfileDetailed` telemetry.
  - Deliverable:
    ```rust
    #[non_exhaustive]
    pub struct FrameProfile {
        pub frame_index: u64,
        pub frame_duration_total: Duration,
        pub active_animations: usize,
        pub primitive_count: u32,
        pub overruns: FramePhaseSet,
        pub dropped_frames: u32,  // reserved for future drift detection
    }

    #[non_exhaustive]
    pub struct FrameProfileDetailed {
        pub per_phase: [Duration; FramePhase::COUNT],
        pub effect_drain_count: u32,
        pub effect_drain_requeued: u32,
        pub deadline_overrun: SmallVec<[(FramePhase, Duration); 4]>,
    }
    ```
    Always-on `FrameProfile` populated by `run_frame` (no `Duration` measurements in release unless `profiling_enabled`). `FrameProfileDetailed` populated only when flag enabled. Layered on top of `measure("frame duration", ...)` (`window.rs:1294`); does not replace `#[profiling::function]` annotations.
  - Files: `crates/flui-core/src/frame/profile.rs`, `crates/flui-core/src/app.rs`.
  - Logging requirements: when profile enabled, one `info!` per frame is acceptable; in default release builds the profile path stays cold and silent.

### Phase 4: Tests and Compatibility Coverage

- [ ] Task 38: Add phase-order tests.
  - Deliverable: deterministic tests via `TestApp::advance_frame` that observe `PreFrame → AnimationTick → Build → Layout → Prepaint → Paint → PostFrame` ordering across one frame and across N≥3 frames. Assertions use `App::current_phase()` markers captured by a test-only phase-recorder, not internal field inspection. Tests call `TestApp::set_auto_advance_frames(false)` to opt out of the auto-redraw path.
  - Files: new tests under `crates/flui-core/src/frame/` or `crates/flui-core/tests/`.
  - Logging requirements: tests may use local counters/markers; no committed runtime logs.

- [ ] Task 39: Add placement-aware `defer` tests.
  - Deliverable: tests prove that each `DeferPlacement` value runs in the expected phase, that the legacy `cx.defer` continues to behave as today (no Tier-C callsite breaks), and that `Notify`/`NotifyGlobalObservers` dedup is preserved under placement splits.
  - Files: focused tests in `crates/flui-core/src/app.rs` or a sibling test module.
  - Logging requirements: none.

- [ ] Task 40: Add deadline-overrun tests.
  - Deliverable: tests for the effect-flush budget — a hostile effect that overshoots forces break-and-requeue, the next frame drains the remainder, and one `WARN` is emitted per overrun (verified via a test logging sink or counter, not by string-matching log output). Separate tests verify non-effect phases (`Layout`, `Paint`) emit at most one `WARN` per overrun and run to completion.
  - Files: focused tests in `crates/flui-core/src/app.rs` or `frame/`.
  - Logging requirements: tests may install a logging sink for assertion purposes.

- [ ] Task 41: Add K15 coexistence tests.
  - Deliverable: tests confirming (a) `cx.defer` remains the only re-entry escape after K04, (b) `cx.update_window` inside a forbidden phase still returns `ReentryError::NestedWindowUpdate`, (c) `EntityMap::double_lease_panic` is unchanged, (d) `cx.defer_to(NextFrameStart, …)` from inside any phase queues correctly. Reuse `cx_defer_avoids_reentry_panic` (`reentrancy.rs:526-550`) as a baseline.
  - Files: `crates/flui-core/src/reentrancy.rs` test module, new tests in `frame/`.
  - Logging requirements: none.

- [ ] Task 42: Add panic-in-phase recovery tests.
  - Deliverable: tests that panic inside each phase (`PreFrame`, `AnimationTick`, `Layout`, `Prepaint`, `Paint`, `PostFrame`) and assert: `abort_frame_after_panic` was invoked, `current_phase == Idle`, `flushing_effects == false`, `next_frame` is empty, but `frame_clock`'s last sample is preserved, active-animation set is unchanged, and effect queue retains pending entries. The subsequent `advance_frame` recovers cleanly. Mirrors `abort_update_after_panic` test coverage.
  - Files: new tests in `crates/flui-core/src/frame/` or `crates/flui-core/tests/`.
  - Logging requirements: none.

- [ ] Task 43: Add animation-tick and `FrameClock` determinism tests.
  - Deliverable: with `TestClock` injected, animations advance only on `AnimationTick`; multiple `AnimationController::value()` reads within one frame return the same result; `request_animation_frame` is idempotent within a frame (multiple calls coalesce via `Cell<bool>`); inactive controllers are not visited; `FrameClock::now()` is stable for all consumers within one `run_frame`.
  - Files: `crates/flui-core/src/animation/controller.rs`, new tests.
  - Logging requirements: none.

### Phase 5: Documentation, Migration, and Status Updates

- [ ] Task 44: Write the K04 migration guide.
  - Deliverable: `docs/superpowers/migrations/K04-effect-frame-contract.md` covering: the seven phases, when to use `cx.defer` vs `cx.defer_to(placement, ...)`, the `Window::on_next_frame` → `Window::on_pre_frame` rename (with old/new code snippets), the new `Window::on_post_frame` API, `FrameClock` usage for time-sensitive code, `AnimationController::value()` behavioral change (per-frame cache), deadline-overrun expectations, panic-safety contract, `TestApp::advance_frame()` for tests, and a Q&A on K15 escape semantics.
  - Logging requirements: migration guide must restate the no-per-element / no-per-frame log invariant.

- [ ] Task 45: Update affected rustdoc and examples.
  - Deliverable: docstrings on `App::defer*`, `App::defer_to`, `App::on_pre_frame`, `App::on_post_frame`, `Window::on_pre_frame` (with deprecation note on the old alias), `Window::on_post_frame`, `Context::on_pre_frame`, `AsyncWindowContext::on_pre_frame`, `Window::request_animation_frame`, `AnimationController::value()`, `App::current_phase`, `App::frame_profile` reflect the new contract. The misleading comment at `animation/animated.rs:10` ("Users never need to call `window.request_animation_frame()` manually") is rewritten to reflect actual callsite expectations. Examples in `crates/flui-core/examples/learn/` and `examples/` updated.
  - Files: `crates/flui-core/src/app.rs`, `crates/flui-core/src/app/context.rs`, `crates/flui-core/src/app/async_context.rs`, `crates/flui-core/src/window.rs`, `crates/flui-core/src/animation/controller.rs`, `crates/flui-core/src/animation/animated.rs`, `crates/flui-core/examples/learn/**/*.rs`, any animation example under `examples/`.
  - Logging requirements: example code must not add committed per-frame logs.

- [ ] Task 46: Publish the joint K15+K04 re-entrancy paragraph.
  - Deliverable: prepend a module-level docstring at the top of `crates/flui-core/src/reentrancy.rs` stating: "Re-entry from within any K04 phase callback follows K15. The phase a callback runs in determines which `DeferPlacement` is sane to defer to. `cx.defer` (default `EndOfUpdate`) remains the only K04 re-entry escape; the phase a deferred callback eventually runs in is its placement, not the queueing phase." Cross-link to design spec.
  - Files: `crates/flui-core/src/reentrancy.rs`.
  - Logging requirements: none.

- [ ] Task 47: Reserve forward-hooks in the design spec.
  - Deliverable: spec section "Forward Hooks Reserved for Future Specs" explicitly listing: `FramePhase::HotReload` (R-track), `Platform::request_animation_frame` verb (R-track / Wasm), `Effect::DeferAsync(async fn)` (SF08), per-window `FrameClockView` epoch divergence (Wasm tab pause / iOS background scene), `App::observe_phase(...)` subscription (K22), `App::run_idle(deadline)` body (currently no-op stub), `FrameProfile::custom_metric` registry (K22 typed). None of these are implemented in K04 — they are documented intent so future specs can land additively under `#[non_exhaustive]`.
  - Files: `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md`.
  - Logging requirements: none.

- [ ] Task 48: Run focused and workspace validation.
  - Deliverable: `cargo fmt --check`, `cargo test -p flui-core`, `cargo test -p flui-macros`, `cargo check -p flui-widgets --all-targets`, `cargo check -p flui-material --all-targets`, `cargo check -p flui-navigator --all-targets`, example checks for `creating_components`, `nav_demo`, `material_demo`, `animation_demo`, and finally `cargo test --workspace`. Verify `#[deprecated]` warnings on legacy `on_next_frame` callsites are non-blocking (Tier-C migration is K04+1). Capture results in PR description.
  - Files: workspace-wide.
  - Logging requirements: command output is verification evidence only.

- [ ] Task 49: Update roadmap, research, AGENTS, and changelog status.
  - Deliverable: mark K04 complete in `.ai-factory/ROADMAP.md`; update `.ai-factory/RESEARCH.md` Active Summary to record K04's resolution and name Phase II-F planning (with SF01 gated on K01/K02/K03/K05 — K04 unblocks SF05) as the next milestone; update `AGENTS.md` and `CHANGELOG.md` if appropriate. Note the K04+1 follow-up (flip `auto_advance_frames_on_flush` default after Tier-C tests migrate; eventually remove the deprecated `on_next_frame` alias).
  - Files: `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md`, `CHANGELOG.md`.
  - Logging requirements: status updates cite validation results, deferred work, and remaining known limitations.

- [ ] Task 50: Complete review gates and final API audit.
  - Deliverable: `flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor` findings addressed or explicitly accepted. Public re-exports curated: `FramePhase` and `FrameProfile` in prelude; `DeferPlacement`, `FrameClock`, `FrameClockView`, `FrameProfileDetailed`, `TickTarget` `pub` but explicit-import. No accidental Phase II-F (`SF##`) scope creep landed. `cargo-semver-checks` clean on `flui-core` public surface. Per-phase logs confirmed bounded to documented `WARN` overrun paths only.
  - Files: changed files from Tasks 15-49, plus `Cargo.toml` if member/feature changes occur.
  - Logging requirements: review evidence belongs in PR notes.

## Done Criteria

- K04 design spec exists at `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md` and resolves every Key Design Decision in this plan with rationale.
- The seven-phase contract (`PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame`, plus `Idle` and interleaved `EffectFlush`) is implemented in `flui-core` with deterministic order under `TestScheduler`.
- `FramePhase::Build` is reserved as a no-op for SF05; reserving costs zero runtime and zero API churn under `#[non_exhaustive]`.
- `FrameClock` exists at the App level, samples `Clock::now()` exactly once per `run_frame`, exposes `now() / frame_index() / delta() / in_frame()`, and is read by the animation tick. `Window::frame_clock_view()` returns an opaque `FrameClockView` reserved for future per-window epochs.
- `Effect::Defer { placement: DeferPlacement, callback }` is the single placement-aware effect variant. The existing `App::defer(f)` preserves observable behavior; `App::defer_to(placement, f)` and its three wrappers (`Context`, `Window`, `AsyncWindowContext`) are the new placement-aware API.
- Per-phase deadline classes are documented and enforced: advisory for non-effect phases (record + one rate-limited `WARN` per phase per frame, never abort); break-and-requeue for `EffectFlush`; hard reserved for SF05.
- `Window::on_next_frame` is renamed to `Window::on_pre_frame` with a `#[deprecated]` alias kept for one release cycle. `Window::on_post_frame` is added, anchored at `Window::complete_frame`. App-level `App::on_pre_frame` / `App::on_post_frame` are the canonical contract surface.
- The `Rc<RefCell<Vec<FrameCallback>>>` storage at `window.rs:960` is replaced with `SmallVec<[FrameCallback; 4]>` directly on `Window`, satisfying `docs/promt.md` §3.1 hot-path rule.
- `AnimationController::value()` no longer re-reads `Clock::now()` per call; per-frame cache `(frame_index, sampled_value)` makes multiple reads in one frame stable. Public signature unchanged.
- `Window::request_animation_frame` is idempotent within a frame via `Cell<bool>` flag; coalesces multiple calls without bypassing placement-aware effects.
- The `#[cfg(any(test, feature = "test-support"))]` auto-redraw is gated by `App::auto_advance_frames_on_flush` flag; `TestApp::advance_frame()` is the new canonical test entry. K04+1 will flip the default.
- Panic-in-phase recovery via `App::abort_frame_after_panic` restores `current_phase`, `flushing_effects`, and `next_frame` buffer; leaves `frame_clock`, active-animation set, and effect queue "stuck dirty" until next frame. Mirrors `abort_update_after_panic`.
- K04 deadlines never double-throttle platform-side thermal / input-rate / force-render throttles.
- `FrameProfile` (always-on, ~32 bytes, in prelude) and `FrameProfileDetailed` (flag-gated, full per-phase Durations) are public; `App::frame_profile()` and `App::frame_profile_detailed()` accessors expose them; `App::set_profiling_enabled(bool)` toggles the detailed view.
- `App::current_phase()` is the cheap public accessor for inspector and Framework-tier tests; `App::observe_phase(...)` reserved as design-spec note.
- K15 re-entrancy guarantees are unchanged; `cx.defer` remains the only K15 escape hatch; the joint K15+K04 contract paragraph is published at the top of `reentrancy.rs`.
- Forward hooks for `FramePhase::HotReload`, `Platform::request_animation_frame`, `Effect::DeferAsync`, per-window `FrameClockView` epochs, `App::observe_phase`, `App::run_idle`, and `FrameProfile::custom_metric` are documented in the design spec but NOT implemented in K04.
- Tests cover phase order, placement-aware effects, deadline overruns (both effect and non-effect), K15 coexistence, panic-in-phase recovery, and `FrameClock` determinism. All tests use `TestApp::advance_frame()` with `set_auto_advance_frames(false)` where phase order matters.
- `cargo fmt --check`, `cargo test -p flui-core`, `cargo test -p flui-macros`, Tier-C compile checks, example checks, and `cargo test --workspace` pass. `#[deprecated]` warnings on legacy `on_next_frame` callsites are non-blocking.
- Migration guide, updated rustdoc, and joint K15+K04 contract explain how to consume the new contract; ROADMAP / RESEARCH / AGENTS / CHANGELOG reflect K04 completion, K04+1 follow-up, and Phase II-F as the next planning target.
