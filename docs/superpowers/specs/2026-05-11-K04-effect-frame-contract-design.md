# K04 - Effect / Frame Contract Design

**Date:** 2026-05-11
**Phase:** 0-K Kernel Cleanup (eighth and final spec in the critical chain after K99, K15, K07, K05, K01, K02, K03)
**Status:** Implemented (2026-05-12). Plan checkboxes 1-49 done; Task 40 dedicated logging-sink test and Task 50 pre-PR review gates remain as follow-ups tracked in the plan file.
**Plan:** `.ai-factory/plans/feature-K04-effect-frame-contract.md`

## Summary

K04 turns the implicit App effect loop and Window draw pipeline of `flui-core` into a typed,
observable seven-phase frame contract with placement-aware effect placement, an App-level
`FrameClock`, advisory per-phase deadlines, panic-safe phase wind-down, and a two-layer
telemetry surface (`FrameProfile` + `FrameProfileDetailed`).

The contract is designed for a 10-year horizon. Every load-bearing decision in this spec is
anchored to convergent invariants from Flutter `SchedulerBinding` (stable since 1.0), Compose
`MonotonicFrameClock` + Snapshot system, the HTML Living Standard event loop, SwiftUI
`CADisplayLink` + run-loop observer, React Scheduler lanes, Bevy ECS Schedule v3 (post the
v1→v3 stage-elimination migration), and Glenn Fiedler's injected-clock determinism canon. The
ten axioms (P1–P10) recorded in this spec are the cross-platform-convergent invariants those
designs all agreed on.

K04 explicitly does NOT introduce: `flui-framework`, `Widget`, `State<W>`, `StateMap`,
reconciliation, dirty lists, `setState`, `InheritedWidget` ergonomics, Theme/MediaQuery, async
widgets, or a widget catalogue. Those belong to Phase II-F (SF##) and are gated on K04.

This spec is API-shaping. The breaking changes are:

- `Window::on_next_frame` is renamed to `Window::on_pre_frame` (deprecated alias retained for
  one release cycle); a new `Window::on_post_frame` is added.
- `Effect::Defer` gains a `placement: DeferPlacement` field. The existing `App::defer(f)`
  preserves observable behavior by routing to `DeferPlacement::EndOfUpdate`.
- `Window::request_animation_frame` becomes idempotent within a frame.
- `AnimationController::value()` caches the sampled value per frame.
- `App::run_frame(window_id)` is the new seven-phase entry point.

## Motivation

The current effect loop and frame pipeline have four structural problems that block Framework
tier work (SF03/SF04/SF05) and the long-term ecosystem:

1. **Effect ordering is undefined.** `App::flush_effects` (`crates/flui-core/src/app.rs:1414`)
   drains `pending_effects` in a single unbounded loop with no phase label, no deadline, no
   placement awareness. SF05's `setState` cannot specify "run me after build but before
   layout"; SF06's inherited invalidation has no scheduling vocabulary; SF08's async
   resolution has no defer-to-idle option.

2. **`Window::on_next_frame` has misleading semantics.** Today it fires **before**
   `window.draw()` in the platform `on_request_frame` callback at `window.rs:1275-1284` —
   semantically a `preFrame` hook, not the `postFrame` its name suggests. The misleading
   comment at `animation/animated.rs:10` ("Users never need to call
   `window.request_animation_frame()` manually") is symptomatic. Documentation, tests, and
   migration paths inherit the confusion.

3. **Time is not a per-frame invariant.** `AnimationController::value()`
   (`animation/controller.rs:233` TODO) re-reads `Clock::now()` on every call. Multiple reads
   inside one frame can return different values — animation curves drift, golden tests are
   flaky, layout-cache hash keys false-miss. `docs/promt.md` §3.1 names this as item 1 on the
   60 FPS hit list.

4. **Re-entrancy and panic safety are scoped to `update_window` only.** `App::start_update` /
   `finish_update` (`app.rs:867-887`) and `App::abort_update_after_panic` (`app.rs:870-874`)
   cover the K15 contract but say nothing about frame phases. A panic inside `tick` / `layout`
   / `prepaint` / `paint` leaves `flushing_effects`, `next_frame` buffer, and `pending_effects`
   in undefined states — fatal for hot-reload (R-track) and headless CI fan-out.

K04 turns the implicit pipeline into an explicit, typed, observable contract that closes all
four problems and reserves forward-hooks for SF##, R-track, Wasm, multi-window, and the
inspector (K22).

## Non-Goals

- Do not introduce `flui-framework`, `Widget`, `BuildCx`, `State`, `Key`, reconciliation,
  dirty lists, `setState`, or `InheritedWidget`. (Phase II-F.)
- Do not shard `App` or `Window` ownership beyond what the seven-phase contract requires.
  K06 owns `BuildOwner` / `PipelineOwner` / `SemanticsOwner` decomposition.
- Do not introduce a new `Platform` trait method for frame pacing. Platform-driven frame
  pacing (existing `PlatformWindow::draw` + `on_request_frame` callback) is preserved.
- Do not add committed per-element or per-frame logging on the dispatch / tick / paint paths.
  Per `docs/promt.md` §3.1, only the deadline-overrun `WARN` and explicit `FrameProfile`
  instrumentation are allowed in committed code.
- Do not implement parallel scheduling, fixed-step timesteps, or multi-threaded phase
  execution. K04 stays single-threaded; the DAG-shape representation is reserved as a future
  hook only.
- Do not implement hot-reload, async widgets, or per-window frame-clock epochs in K04.
  Forward-hooks are reserved.
- Do not migrate the 36+ existing `cx.defer` callsites. K04 preserves observable behavior of
  `App::defer(f)` via routing to `DeferPlacement::EndOfUpdate`.

## Current Inventory

This section is the consolidated output of plan Tasks 1 and 2 (inventory of effect/frame
surfaces and `cx.defer` / `on_next_frame` / `request_animation_frame` callsites).

### Effect System (`crates/flui-core/src/app.rs`)

| Item | Location | Role |
|---|---:|---|
| `Effect` enum | `app.rs:2563-2599` | 6 variants — see breakdown below |
| `Effect::Notify` | `app.rs:2563` | Per-emitter notification; deduplicated |
| `Effect::Emit` | `app.rs:2563` | Type-filtered event dispatch |
| `Effect::RefreshWindows` | `app.rs:2563` | Sets `window.refreshing = true` on all windows |
| `Effect::NotifyGlobalObservers` | `app.rs:2563` | Global observer fan-out; deduplicated |
| `Effect::Defer` | `app.rs:2563` | K15-blessed re-entry escape hatch; queued, no dedup |
| `Effect::EntityCreated` | `app.rs:2563` | Dispatches new-entity observers |
| `App::push_effect` | `app.rs:1393-1409` | Dedupes Notify/NotifyGlobalObservers via `FxHashSet`; pushes to `VecDeque` |
| `App::flush_effects` | `app.rs:1414-1470` | Single unbounded loop; exits when queue empty; test-mode block at `1450-1462` auto-redraws dirty windows |
| `App::start_update` | `app.rs:866-868` | Increments `pending_updates` |
| `App::finish_update` | `app.rs:877-884` | Flushes effects only at `pending_updates == 1` (outermost) |
| `App::abort_update_after_panic` | `app.rs:870-874` | Restores `pending_updates`, clears `flushing_effects` |
| `App::defer` (public) | `app.rs:1704-1708` | Pushes `Effect::Defer { callback }`; K15 escape hatch |
| `pending_effects: VecDeque<Effect>` | `app.rs` (field on `App`) | The single drain queue |
| `pending_notifications: FxHashSet<EntityId>` | `app.rs` (field on `App`) | Notify dedup set |
| `pending_global_notifications: FxHashSet<TypeId>` | `app.rs` (field on `App`) | NotifyGlobalObservers dedup set |
| `flushing_effects: bool` | `app.rs:604` | Re-entry guard for `flush_effects` |
| `window_update_stack: Vec<WindowId>` | `app.rs:590` | K15: detects nested `update_window` on same window |
| `currently_updating_entity: Option<EntityId>` | `app.rs:598` | K15: detects nested `update_entity` |

### Frame Pipeline (`crates/flui-core/src/window.rs`)

| Item | Location | Role |
|---|---:|---|
| `Window::draw` | `window.rs:2379-2465` | Entry point for the per-window render pipeline |
| `Window::draw_roots` | `window.rs:2496+` | Iterates roots; prepaint + paint phases |
| `Window::present` | `window.rs:2490-2493` | Delegates to `PlatformWindow::draw(&scene)` |
| `Window::complete_frame` | `window.rs:2372-2374` | Calls `platform_window.completed_frame()` |
| `DrawPhase` enum | `window.rs:1078-1083` | Internal: `None / Prepaint / Paint / Focus` |
| `Window::invalidator` | `window.rs` | Per-window dirty flag; `set_dirty`, `is_dirty` |
| `Window::on_next_frame` | `window.rs:1911-1912` | Pushes to `next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>>` (`window.rs:960`) |
| `Window::request_animation_frame` | `window.rs:1921-1924` | Schedules `cx.notify(current_view())` for next draw |
| Platform `on_request_frame` callback | `window.rs:1257-1314` | Wraps next_frame_callbacks drain → draw → present → complete_frame |
| Thermal-state throttle | `window.rs:1262-1273` | Drops frames to 60 Hz when thermal_state is `Serious`/`Critical` |
| `input_rate_tracker` | `window.rs:1256, 1291` | Sustains presentation under high-rate input |
| `force_render` / `request_frame_options` | `window.rs:1289-1293` | Allows forced frame rendering |
| `measure("frame duration", ...)` | `window.rs:1294` | Wraps entire frame for telemetry |
| `Frame::scene` + `Frame::deferred_draws` | `window.rs:765+` | Two-buffer swap: `rendered_frame` and `next_frame` |
| `hit_test_behaviors`, `pending_recognizers` | `window.rs:2390-2398` | Per-frame maps cleared at start of `draw` |

### Animation Substrate (`crates/flui-core/src/animation/`)

| Item | Location | Role |
|---|---:|---|
| `Ticker` | `animation/ticker.rs:44-78` | Time source for animations; wraps `Arc<dyn Clock>` |
| `AnimationController` | `animation/controller.rs:69+` | Holds Ticker + value + status + duration + curve |
| `AnimationController::value()` | `animation/controller.rs:233` (TODO) | Re-reads `Clock::now()` on each call — K04 fix target |
| `AnimationController` callers | `animation/animated.rs:30`, `elements/animation.rs:210`, `elements/img.rs:371` | Call `window.request_animation_frame()` to drive rebuild |
| `assets.rs::ImageFrame::frame_index` | `assets.rs:39` | Animated-image data concept — **distinct** from scheduler frame index |

### Scheduler & Clock Substrate

| Item | Location | Role |
|---|---:|---|
| `Clock` trait | `scheduler/clock.rs` | `now() -> Instant`, `utc_now() -> DateTime<Utc>` |
| `TestClock` | `scheduler/test_scheduler.rs` | Deterministic test clock injection |
| `Scheduler` trait | `scheduler/mod.rs:83-120` | Foreground / background runnable scheduling |
| `BackgroundExecutor` / `ForegroundExecutor` | `executor.rs` | Thin wrappers over `Scheduler` for `cx.spawn` / `cx.background_spawn` |
| `PlatformScheduler` | `platform_scheduler.rs` | Wraps `PlatformDispatcher` with `Clock` |
| `TestScheduler` | `scheduler/test_scheduler.rs:36-100` | `block_on`, `run()`; drives frames manually in tests |
| `Platform::run` | `platform.rs:233` | Platform event-loop entry |
| `PlatformDispatcher` | `platform.rs:761-783` | OS-level dispatch; `dispatch_on_main_thread`, `dispatch_after`, etc. |

### Re-entrancy (K15) Substrate

| Item | Location | Role |
|---|---:|---|
| `ReentryError` | `reentrancy.rs` | `NestedWindowUpdate`, `NestedEntityUpdate`, `ElementStateInUse`, `AsyncContextAsMut` |
| `ReentryMode` | `reentrancy.rs` | `Strict` (panic) / `Loose` (warn) — test default is Strict |
| `EntityMap::double_lease_panic` | `reentrancy.rs` | Multi-entity cycle panic with unified Display |
| `cx_defer_avoids_reentry_panic` test | `reentrancy.rs:526-550` | Documented `cx.defer` escape hatch test |

### Callsite Inventory: `cx.defer` / `Window::defer` / `on_next_frame` / `request_animation_frame`

A precise regex (`cx\.defer\(`, `\.defer_to\(`, `Window::defer\(`, `on_next_frame\(`,
`request_animation_frame\(`) returns roughly 36 hits across 5 `flui-core` files plus Tier-C
consumers. Categorized by target placement:

| Category | Example callsites | Target `DeferPlacement` |
|---|---|---|
| Focus / window activation | `app/context.rs:621` (`focus_window`), `app.rs:956`, `app.rs:1005`, `app.rs:1092` | `EndOfUpdate` (preserve current behavior) |
| Action toggles | `app/context.rs:551, 575, 600, 715` (`toggle_action_status`, etc.) | `EndOfUpdate` |
| Subscribe / observe re-entry guards | `app/context.rs:188, 311, 742`, `window.rs:1651, 1747` | `EndOfUpdate` |
| Scroll-into-view | `window.rs:2299` (`request_autoscroll`) | `PostFrame` (logically; today routes through `EndOfUpdate` for compat) |
| Animation request | `animation/animated.rs:30`, `elements/animation.rs:210`, `elements/img.rs:371` | `NextFrameStart` (logically; today via `request_animation_frame`) |
| Image-cache lazy loading | `elements/image_cache.rs:275` | `NextFrameStart` |
| Window-closing flow | `app.rs:1803` | `EndOfUpdate` |
| Async-context wrappers | `app/async_context.rs:310-313` | `EndOfUpdate` |
| Context activation | `window.rs:4299, 4327, 5299` | `EndOfUpdate` |
| Native-window event examples | `examples/**/*.rs` | `EndOfUpdate` (rarely matter) |

**Migration impact:** none in K04. Every callsite continues to call `cx.defer(f)` which routes
to `DeferPlacement::EndOfUpdate` — observable behavior identical to today. Future SF05 / SF06
specs may migrate select callsites to `NextFrameStart` / `PostFrame` additively.

## 10-Year Contract Axioms (P1–P10)

These ten axioms are the cross-platform-convergent invariants. They MUST hold for K04 to
survive SF03–SF08, hot-reload (R-track), Wasm, and headless-CI for the 10-year horizon.

| # | Axiom | Cross-platform precedent | Consequence for K04 |
|---|---|---|---|
| **P1** | A frame is a typed, observable state machine, not an unfolded callback chain. | Flutter `SchedulerPhase` (stable since 1.0); Compose `Recomposer` lifecycle states. | `FramePhase` is `pub`, `#[non_exhaustive]`, ordered, queryable from `App` and every context. |
| **P2** | Frame ordering is a **logical** contract; the implementation may collapse code paths. | Bevy collapses `Update`/`PostUpdate`; React folds commit/passive effects. | Tests assert observable order via markers, not internal field inspection. Implementation may move work between phases without a SemVer break. |
| **P3** | Time is sampled once per logical frame; everything in that frame sees the same `Instant`. | Flutter `currentFrameTimeStamp`; Compose `withFrameNanos`; rAF `DOMHighResTimeStamp`; Fiedler injected clock; React `getCurrentTime`. | `FrameClock::now()` is the **only** sanctioned time source inside a frame. Direct `Instant::now()` in committed phase code is a lint. |
| **P4** | Deadlines describe **policy**, not scheduling guarantees. The OS, GPU, and thermal state decide when frames actually fire. | Flutter exposes no per-phase deadline; React per-lane expirations; Bevy reports per-phase times. | Budgets are advisory by default; only `EffectFlush` has "break and re-queue" semantics. `tick`/`layout`/`prepaint`/`paint`/`preFrame`/`postFrame` only **report** overruns. |
| **P5** | Re-entry is admissible only through the documented queue. No phase introduces a second escape. | K15 already published; Compose forbids re-entrant composition past a threshold; Flutter forbids `setState` during build. | All phases needing same-target mutation go through `cx.defer_to(...)`. |
| **P6** | Effects are typed by **placement**, not by **target**. | Compose distinguishes `LaunchedEffect`/`SideEffect`/`DisposableEffect` by capability axis; React lanes are when, not what. | `Effect::Defer { placement, callback }` — never `Effect::DeferForReconciliation`. SF05/SF06/SF08 reuse the placement enum additively. |
| **P7** | A frame is per-window in observable scope, even if some phases are App-wide. | Flutter `WidgetsBinding.drawFrame` per-window; SwiftUI multi-scene; Web one rAF / per-document layout. | `App::run_frame(window_id)` is the entry. App-global `AnimationTick`, `PreFrame`, `PostFrame` wrap per-window phases. |
| **P8** | Panic in any phase leaves the App recoverable. | Compose / Flutter recover and re-throw to test/inspector; without recovery hot-reload is impossible. | `abort_frame_after_panic` is mandatory, mirrors `abort_update_after_panic` (K07), is part of the published contract. |
| **P9** | The contract compiles and behaves identically under headless / single-threaded / Wasm. | Web supports rAF without OS vsync; iOS supports background scenes; CI fans out. | `FrameClock` is `!Send`; phase code never touches `std::time::Instant` directly (uses injected `Clock`). |
| **P10** | The public surface for non-engine consumers must be writable with **only the contract**, not the implementation. | Flutter `WidgetsBindingObserver`; Compose `compositionLocalOf`; React DevTools. | `App::current_phase()` and a reserved phase-subscription hook are part of the contract from day one. |

## Frozen Design Decisions

### D1. Phase Ordering and Observable Contract

`FramePhase` is `#[non_exhaustive]`, `Copy + Clone + Debug + PartialEq + Eq + Hash + Ord`. The
ordered variants are:

```rust
pub enum FramePhase {
    Idle,
    PreFrame,
    AnimationTick,
    Build,          // reserved no-op for SF05; see D2
    Layout,
    Prepaint,
    Paint,
    PostFrame,
}
```

`EffectFlush` is **not** a phase. It is interleaved at phase boundaries — see D5/D7.

#### Per-Phase Contract

| Phase | Predecessor | Successor | Allowed | Forbidden | K15 class | Deadline class | Allowed `cx.defer_to(...)` |
|---|---|---|---|---|---|---|---|
| `Idle` | startup or `PostFrame` | `PreFrame` | `cx.defer_to(Idle, …)` callbacks, K20 layout-cache eviction, profiler flush, drain `pending_effects` with placement `Idle` and `EndOfUpdate` | `Window::draw`, element mutation, `request_animation_frame` (queues for next `PreFrame`) | Outside `start_update` | Advisory 4 ms | All placements admissible |
| `PreFrame` | `Idle` | `AnimationTick` | App-level `on_pre_frame` callbacks, Window-level `on_pre_frame` callbacks, `FrameClock::begin_frame(now)` | Layout reads, paint, nested `update_window` for same window | Inside `start_update` | Advisory 1 ms | `PostFrame`, `NextFrameStart` |
| `AnimationTick` | `PreFrame` | `Build` | Walk `App::active_animations` set; call `TickTarget::tick(frame_clock.now())`; emit `Effect::Notify` for changed targets | `AnimationController::start`/`stop` (queue via defer); `setState` (queue via defer); no element mutation | Inside `start_update` | Advisory 1 ms | `PostFrame`, `NextFrameStart` |
| `Build` (reserved) | `AnimationTick` | `Layout` | Nothing in K04. SF05 fills with `BuildOwner::flush_dirty()` | Anything in K04 | Inside `start_update` | Reserved Hard 4 ms | `PostFrame`, `NextFrameStart` |
| `Layout` | `Build` | `Prepaint` | Taffy work, layout cache lookups, mediaquery materialization | Paint, scene primitives, GPU calls | Inside `start_update` | Advisory 3 ms | `NextFrameStart` |
| `Prepaint` | `Layout` | `Paint` | Bounds finalization, hitbox registration, `Interactivity::paint`, deferred-draw resolution | GPU encode | Inside `start_update` | Advisory 4 ms | `NextFrameStart` |
| `Paint` | `Prepaint` | `PostFrame` | Scene primitives, focus listener fan-out, `Window::present()`, `Window::complete_frame()` | Layout, effect push (queue via defer) | Inside `start_update` | Advisory 1 ms | `PostFrame`, `NextFrameStart` |
| `PostFrame` | `Paint` | `Idle` | Inspector readout, telemetry export, App-level `on_post_frame` callbacks, Window-level `on_post_frame` callbacks, SF08 future settle, drain `pending_effects` with placement `PostFrame` | Element mutation, layout, paint | Inside `start_update` | Advisory 2 ms | `NextFrameStart` |
| `EffectFlush` (interleaved at boundaries) | varies | varies | Drain `pending_effects` with placement matching boundary; FIFO; dedup-preserved | Phase entry | Inside `start_update` | **Break-and-requeue 2 ms** | Defer admissible to any future placement |

#### Interleaved `EffectFlush` schedule

```text
PreFrame  →  flush(EndOfUpdate)
AnimationTick  →  flush(EndOfUpdate)
Build  →  flush(EndOfUpdate)
Layout  →  flush(EndOfUpdate)
Prepaint  →  flush(EndOfUpdate)
Paint  →  flush(EndOfUpdate)
PostFrame  →  flush(EndOfUpdate) + flush(PostFrame)
Idle  →  flush(EndOfUpdate) + flush(Idle)
```

`EndOfUpdate` is the default placement and drains at every phase boundary, preserving current
observable behavior. `NextFrameStart` callbacks drain at the start of the next `PreFrame`.
`PostFrame` callbacks drain in the `PostFrame` phase only. `Idle` callbacks drain in `Idle`
only and may coalesce across multiple `Idle` entries.

### D2. `FramePhase::Build` Reservation

`Build` is a no-op phase in K04. The phase enters and exits immediately, drains no effects,
advances no state. The slot exists between `AnimationTick` and `Layout` so SF05 can fill it
with `BuildOwner::flush_dirty()` without adding a new enum variant — which would otherwise be
a SemVer break under `#[non_exhaustive]` if downstream code matched exhaustively on
`FramePhase`.

The deadline class for `Build` is "Reserved Hard" — it will become a hard deadline in SF05
because rebuild storms must be terminable. K04 does not implement the hard mode; the slot's
deadline behavior is "no work, no overrun".

### D3. `Window::on_next_frame` Rename + `Window::on_post_frame` Addition

**Decision:** rename + add (both, not either).

- `Window::on_next_frame(callback)` → renamed to `Window::on_pre_frame(callback)`. The old
  name is kept as `#[deprecated(since = "K04 release", note = "renamed to on_pre_frame")]`
  alias forwarding to the new name for one release cycle.
- New `Window::on_post_frame(callback)` is added, anchored at `Window::complete_frame()`
  (`window.rs:2372`). Drains in the `PostFrame` phase of `App::run_frame`.
- All three wrappers move together: `Window`, `Context` (`app/context.rs:292`),
  `AsyncWindowContext` (`app/async_context.rs:311`). Both pre and post variants on each.
- The misleading comment at `animation/animated.rs:10` ("Users never need to call
  `window.request_animation_frame()` manually") is rewritten to reflect actual callsite
  expectations.

**Rationale:** today's `Window::on_next_frame` runs in the platform `on_request_frame`
callback **before** `window.draw()` (`window.rs:1275-1284`). The name "next_frame" is
semantically a `preFrame` hook, but downstream code reads it as `postFrame` because the name
matches Flutter's `addPostFrameCallback` superficially. Flutter's `setState` in
`postFrameCallback` is one of the most-cited gotchas in its issue tracker (`flutter#147605`);
flui-v2 avoids the equivalent footgun at the source by naming the hooks for what they do.

### D4. App-Level Pre/Post-Frame Contract Surface

**Decision:** both — App level is the contract, Window level is the wrapper.

```rust
impl App {
    pub fn on_pre_frame(&mut self, callback: impl FnOnce(&mut App) + 'static);
    pub fn on_post_frame(&mut self, callback: impl FnOnce(&mut App) + 'static);
}

impl Window {
    pub fn on_pre_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static);
    pub fn on_post_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static);
}
```

**Rationale (P7):** multi-window apps need App-level for cross-window callbacks (input
replay, telemetry export). Flutter's `WidgetsBinding.instance.addPostFrameCallback` is
App-level; Element-level is sugar. Per-window callbacks operate only on that window's frame.

**Storage:** App-level uses `SmallVec<[FrameCallback; 4]>` directly on `App`. Window-level
uses `SmallVec<[FrameCallback; 4]>` directly on `Window` (replacing the existing
`Rc<RefCell<Vec<FrameCallback>>>` at `window.rs:960`). See D11 for the storage rationale.

### D5. `Effect::Defer` Placement Model — Single Variant with Field

**Decision:** extend the existing `Effect::Defer` variant with a `placement` field. Reject
the sibling `Effect::DeferTo` design.

```rust
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum DeferPlacement {
    EndOfUpdate,       // default — drains at next phase boundary (today's `cx.defer` behavior)
    NextFrameStart,    // drains at the start of the next PreFrame
    PostFrame,         // drains in the PostFrame phase
    Idle,              // drains in Idle; may coalesce across multiple Idle entries
}

pub(crate) enum Effect {
    // ... existing variants unchanged ...
    Defer {
        placement: DeferPlacement,
        callback: Box<dyn FnOnce(&mut App) + 'static>,
    },
}
```

**API:**

```rust
impl App {
    pub fn defer(&mut self, f: impl FnOnce(&mut App) + 'static);              // existing — routes to EndOfUpdate
    pub fn defer_to(&mut self, placement: DeferPlacement, f: impl FnOnce(&mut App) + 'static);  // new
}

// Mirror wrappers:
impl Context<'_, T> { pub fn defer_to(&mut self, placement: DeferPlacement, f: impl FnOnce(&mut App) + 'static); }
impl Window { pub fn defer_to(&self, placement: DeferPlacement, f: impl FnOnce(&mut Window, &mut App) + 'static); }
impl AsyncWindowContext { pub fn defer_to(&mut self, placement: DeferPlacement, f: impl FnOnce(&mut Window, &mut App) + 'static); }
```

**Rationale:** sibling `Effect::DeferTo` doubles match arms across `flush_effects` and the
dedup loop with zero benefit. Single-variant-with-field matches how Compose distinguishes
effects by capability rather than by enum variant. The cost of extending the variant is one
struct field; every match arm already pattern-matches `Defer { callback }`.

**Backward compat:** all 36+ existing `cx.defer(f)` callsites continue to work unchanged. The
default placement preserves today's observable "drain at next phase boundary" behavior.

**Forward compat (SF05/SF06/SF08):** under `#[non_exhaustive]`, SF05 can add
`DeferPlacement::BeforeBuild`, SF08 can add `DeferPlacement::AfterSettle`, etc. — without a
SemVer break.

### D6. `FrameClock` Ownership — App-Level with Opaque Window View

**Decision:** `App::frame_clock: FrameClock` is the single source of truth. `Window` exposes
an opaque `FrameClockView` for forward-compat with per-window epochs (Wasm tab pause, iOS
background scene).

```rust
pub struct FrameClock { /* private */ }

impl FrameClock {
    pub fn now(&self) -> Instant;
    pub fn frame_index(&self) -> u64;
    pub fn delta(&self) -> Duration;
    pub fn in_frame(&self) -> bool;
    pub(crate) fn begin_frame(&mut self, now: Instant);  // called by App::run_frame
}

#[derive(Copy, Clone)]
pub struct FrameClockView { /* private; today wraps a reference to App's FrameClock */ }

impl FrameClockView {
    pub fn now(&self) -> Instant;
    pub fn frame_index(&self) -> u64;
    pub fn delta(&self) -> Duration;
    pub fn in_frame(&self) -> bool;
}

impl App { pub fn frame_clock(&self) -> &FrameClock; }
impl Window { pub fn frame_clock_view(&self) -> FrameClockView; }
```

**Outside `in_frame()`:** `now()` panics in `cfg(debug_assertions)`, returns the last-sampled
`Instant` in release. This catches the bug-class where post-frame code reads `now()` thinking
it is wall-clock.

**`FrameClock` is `!Send`** — it lives on `App` (which is `!Send`). The underlying time
source is the existing `Arc<dyn Clock>` from `scheduler/clock.rs`. `TestClock` injection
works unchanged.

**Multi-window:** single App-wide clock (P7). Flutter has one `currentFrameTimeStamp` across
all scenes; Web has one `performance.now()` per document. Per-window epochs (where tab
visibility or background scene affects the clock) are reachable via `FrameClockView`'s opaque
type — today returns the App view; future R-track / Wasm work can swap the view to a
window-local epoch without an API break.

### D7. Deadline-Class Taxonomy — Advisory / Break-and-Requeue / Hard-Reserved

| Class | Behavior | Phases |
|---|---|---|
| **Advisory** | Record overrun in `FrameProfile.overruns: FramePhaseSet` (bitset, no allocation). Emit at most one rate-limited `WARN` per phase per frame. Phase runs to completion. | `PreFrame`, `AnimationTick`, `Layout`, `Prepaint`, `Paint`, `PostFrame` |
| **Break-and-requeue** | At each iteration of the drain, check deadline. On overrun, requeue remainder, emit `WARN`, exit phase. | `EffectFlush` only |
| **Hard** (reserved) | `panic!` in `cfg(debug_assertions)`, log+abort-phase in release. | Reserved for SF05 worst-case rebuild storms. Not active in K04. |

**Rationale (P4):** aborting layout mid-frame leaves the scene in an undefined state; we
cannot recover responsibly. Effect-flush is the unique phase where work units are atomic
(each effect is independent), so break-and-requeue is sound. Flutter exposes no deadline to
user code (only post-hoc `FrameTiming` via `addTimingsCallback`); React uses expiration-based
scheduling per lane; Bevy reports per-phase times. K04 lands the strictest design that fits
the platform constraint: enforce only what is safely interruptible.

**Budgets (per `docs/promt.md` §3.1):**

```text
PreFrame:      1 ms (advisory)
AnimationTick: 1 ms (advisory)
Build:         4 ms (reserved hard, SF05)
Layout:        3 ms (advisory)
Prepaint:      4 ms (advisory)
Paint+present: 1 ms (advisory)
PostFrame:     2 ms (advisory)
EffectFlush:   2 ms (break-and-requeue)
Slack:        ~4 ms
```

`FrameProfile.overruns` is a `FramePhaseSet` bitset (no heap allocation on the hot path).

### D8. Animation Tick — `App::active_animations` + Sealed `TickTarget`

**Decision:** App-level active-controller set keyed on a sealed trait.

```rust
mod sealed { pub trait Sealed {} }

pub trait TickTarget: sealed::Sealed {
    fn tick(&mut self, now: Instant) -> TickOutcome;
    fn id(&self) -> TickTargetId;
}

pub enum TickOutcome {
    Continue,
    Done,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TickTargetId(/* private opaque u64 */);

// App-level active set:
impl App {
    pub(crate) active_animations: FxHashSet<TickTargetId>,
}

impl AnimationController {
    pub fn start(&mut self, cx: &mut App) { cx.active_animations.insert(self.id()); }
    pub fn stop(&mut self, cx: &mut App)  { cx.active_animations.remove(&self.id()); }
}
```

The `AnimationTick` phase walks `App::active_animations`, dispatches `TickTarget::tick(now)`
for each, removes entries that returned `TickOutcome::Done`, and emits `Effect::Notify` for
`TickOutcome::Continue` entries that changed.

**Sealing rationale:** in K04, `AnimationController` is the only `TickTarget`. SF08 (async
widgets) and future audio / spring / particle controllers add `impl TickTarget` for their
types additively; the trait is sealed so Tier-C cannot add new impls outside `flui-core` in
K04. Future opening (removing the sealing supertrait) is an additive change.

**`AnimationController::value()` cache (D8.1):**

```rust
pub struct AnimationController {
    // ... existing fields ...
    cached_at_frame: Option<(u64, f32)>,  // (frame_index, sampled_value)
}

impl AnimationController {
    pub fn value(&self) -> f32 {
        let frame_index = /* read frame_clock.frame_index() */;
        if let Some((cached_idx, cached_val)) = self.cached_at_frame {
            if cached_idx == frame_index { return cached_val; }
        }
        let val = self.sample();  // existing computation
        self.cached_at_frame = Some((frame_index, val));
        val
    }
}
```

Public signature **unchanged**; only the body adds the cache. Closes the TODO at
`animation/controller.rs:233` and `docs/promt.md` §5 item 1.

**`Window::request_animation_frame` idempotence (D8.2):**

```rust
impl Window {
    pub(crate) request_next_frame: Cell<bool>,  // new

    pub fn request_animation_frame(&self) {
        self.request_next_frame.set(true);
    }
}
```

Multiple calls coalesce. The platform `on_request_frame` callback drains the flag at frame
entry. Existing closure-queue (`next_frame_callbacks`) is retained only for explicit
`on_pre_frame` callbacks.

### D9. Panic-Safety Contract

**Decision:** `App::abort_frame_after_panic(phase: FramePhase)` mirrors
`App::abort_update_after_panic` (`app.rs:870-874`).

```rust
impl App {
    pub(crate) fn abort_frame_after_panic(&mut self, phase: FramePhase) {
        // Restore phase state:
        self.current_phase = FramePhase::Idle;
        self.flushing_effects = false;
        if self.pending_updates > 0 { self.pending_updates -= 1; }

        // Clear in-flight frame buffer (if panic happened mid-paint):
        if let Some(window) = self.current_drawing_window() {
            window.next_frame.clear();
        }

        // "Stuck dirty" — left as-is:
        // - frame_clock.sampled stays at the panicked frame's `now()`
        // - active_animations stays unchanged (controllers tick again next frame)
        // - pending_effects stays unchanged (drains next frame)
    }
}
```

**What is restored:** `current_phase = Idle`, `flushing_effects = false`, `pending_updates`,
`window_update_stack`, `currently_updating_entity` (already covered by K07/K15),
`window.next_frame` buffer (cleared, not swapped).

**What is left "stuck dirty":** `frame_clock` (panicked frame's `now()` preserved), active
animation set (controllers tick again next frame), effect queue (drains next frame), window
`invalidator` (already dirty → forces redraw on next frame).

**Wiring:** the same `catch_unwind` / `Drop` guards used by K15 wrap each phase. On panic:
log + `abort_frame_after_panic(phase)` + re-raise. Subsequent `App::run_frame` recovers
cleanly.

### D10. Test-Mode Divergence Policy

**Decision:** add `TestApp::advance_frame()` as the canonical test entry. Keep the existing
`#[cfg(any(test, feature = "test-support"))]` auto-redraw block at `app.rs:1450-1462` behind
a flag with default-true in `cfg(test)`. Deprecate the flag in K04+1 after Tier-C tests
migrate.

```rust
impl App {
    pub auto_advance_frames_on_flush: bool,  // default: cfg(test) -> true, else -> false
}

// In flush_effects:
#[cfg(any(test, feature = "test-support"))]
if self.auto_advance_frames_on_flush {
    // existing auto-redraw block, gated by the flag
}
```

```rust
// In test-support crate / module:
impl TestApp {
    pub fn advance_frame(&mut self) -> FrameOutcome;
    pub fn advance_frames(&mut self, n: u32) -> Vec<FrameOutcome>;
    pub fn set_auto_advance_frames(&mut self, enabled: bool);
    pub fn frame_profile(&self) -> &FrameProfile;
}
```

**Rationale:** the existing auto-redraw is observably different from production drain order
and is a source of phase-order test flakiness. The flag preserves back-compat (today's tests
keep passing); `advance_frame` is the explicit, deterministic path for new phase-order tests
and Framework-tier tests. K04+1 will flip the default to `false` after Tier-C tests adopt
`advance_frame`.

### D11. Hot-Path Storage Cleanup

The existing `Window::next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>>` at
`window.rs:960` is on the per-frame hot path and uses both `Rc<RefCell<…>>` (forbidden by
`docs/promt.md` §3.1) and heap-allocated `Vec`. The `Rc<RefCell<…>>` was needed only because
the platform callback drained from a clone of the field; the new `App::run_frame` owns the
drain and does not need aliasing.

**Decision:** replace with `SmallVec<[FrameCallback; 4]>` directly on `Window` (no `Rc`, no
`RefCell`). Same change for the new `on_post_frame` storage and for the App-level pre/post
storage. The `SmallVec` inline capacity of 4 covers the typical case (animation request,
focus restore, scroll-into-view) without heap allocation; spillover is supported.

### D12. K15 + K04 Joint Re-Entrancy Contract

Published as a module-level docstring at the top of `crates/flui-core/src/reentrancy.rs`:

> **Re-entry from within any K04 phase callback follows K15.** The phase a callback runs in
> determines which `DeferPlacement` is sane to defer to:
>
> - During `PreFrame` / `AnimationTick` / `Build` / `Layout` / `Prepaint` / `Paint`: prefer
>   `DeferPlacement::PostFrame` or `DeferPlacement::NextFrameStart`. `EndOfUpdate` is
>   admissible but drains mid-frame at the next phase boundary.
> - During `PostFrame`: prefer `DeferPlacement::NextFrameStart`.
> - During `Idle`: prefer `DeferPlacement::NextFrameStart` (or do the work inline).
>
> The phase a deferred callback eventually **runs in** is its placement, not the queueing
> phase. `cx.defer(f)` (default placement `EndOfUpdate`) remains the **only** K04 re-entry
> escape, exactly as K15 published.

### D13. Platform-Throttle Coexistence

K04 deadlines apply **only inside frames that the platform allowed to fire**. The two are
orthogonal:

- **Thermal throttle** (`window.rs:1262-1273`) = "do we run a frame?" Platform decision.
- **`input_rate_tracker`** (`window.rs:1256, 1291`) = "do we sustain presentation?" Platform
  decision.
- **`force_render` / `request_frame_options`** (`window.rs:1289-1293`) = "do we render even
  when not dirty?" Platform decision.
- **K04 deadlines** = "did this frame's phases meet their budget?" Engine reporting.

No double-counting: K04 deadlines do not enforce frame frequency (except `EffectFlush`
break-and-requeue, which is per-phase, not per-frame). The `measure("frame duration", ...)`
macro wraps the entire `on_request_frame` callback today; K04's per-phase measurements nest
inside that.

## Public API Surface

| Symbol | Visibility | Prelude? | Rationale |
|---|---|---|---|
| `FramePhase` | `pub`, `#[non_exhaustive]` | ✅ Yes | Inspector, SF05, tests, application observability. |
| `FramePhase::Build` | `pub` variant | n/a | Reserved no-op; SF05 fills. |
| `DeferPlacement` | `pub`, `#[non_exhaustive]` | ❌ No | Application code rarely needs it; SF03+ does. Explicit import. |
| `FrameClock` | `pub` (struct) | ❌ No | Animation/timestamp consumers import explicitly. |
| `FrameClockView` | `pub` (struct) | ❌ No | Opaque window-local view; forward-hook for per-window epochs. |
| `FrameProfile` | `pub`, `#[non_exhaustive]` | ✅ Yes | Inspector + DevTools entry; always-on telemetry. |
| `FrameProfileDetailed` | `pub`, `#[non_exhaustive]` | ❌ No | Flag-gated; explicit import for tooling. |
| `TickTarget` (sealed trait) | `pub` (sealed) | ❌ No | Engine-internal impls only in K04. |
| `TickTargetId` | `pub` (newtype) | ❌ No | Opaque identity. |
| `TickOutcome` | `pub` | ❌ No | Companion to `TickTarget`. |
| `App::run_frame(window_id) -> FrameOutcome` | `pub` | n/a | Public; tests and headless CI need it. |
| `App::on_pre_frame` / `App::on_post_frame` | `pub` | n/a | New contract surface. |
| `App::defer_to(placement, f)` | `pub` | n/a | Placement-aware defer. |
| `App::current_phase() -> FramePhase` | `pub` | n/a | Cheap (one field read); inspector + Framework-tier tests. |
| `App::frame_clock() -> &FrameClock` | `pub` | n/a | Read access. |
| `App::frame_profile() -> &FrameProfile` | `pub` | n/a | Always-on read. |
| `App::frame_profile_detailed() -> Option<&FrameProfileDetailed>` | `pub` | n/a | Flag-gated read. |
| `App::set_profiling_enabled(bool)` | `pub` | n/a | Toggle detailed profile. |
| `App::auto_advance_frames_on_flush` field | `pub` | n/a | Test-mode flag (D10). |
| `Window::on_pre_frame(callback)` | `pub` | n/a | Renamed from `on_next_frame`. |
| `Window::on_next_frame(callback)` | `#[deprecated]` alias | n/a | Kept for one release cycle. |
| `Window::on_post_frame(callback)` | `pub` | n/a | New. |
| `Window::defer_to(placement, callback)` | `pub` | n/a | Placement-aware defer. |
| `Window::frame_clock_view() -> FrameClockView` | `pub` | n/a | Opaque view; forward-hook for per-window epochs. |
| `Window::request_animation_frame()` | `pub` (unchanged signature) | n/a | Now idempotent via `Cell<bool>` (D8.2). |
| `Context::on_pre_frame` / `on_post_frame` / `defer_to` | `pub` | n/a | Mirror wrappers. |
| `AsyncWindowContext::on_pre_frame` / `on_post_frame` / `defer_to` | `pub` | n/a | Mirror wrappers. |
| `Effect`, `Effect::Defer` | `pub(crate)` (unchanged) | n/a | Never been public; do not change. |
| `Window::DrawPhase` | `pub(crate)` (unchanged) | n/a | Strict sub-state of `FramePhase::{Prepaint, Paint}`. |
| `TestApp::advance_frame` / `advance_frames` / `set_auto_advance_frames` / `frame_profile` | `pub` (`feature = "test-support"`) | n/a | Required for Framework-tier tests. |
| `App::observe_phase(...)` | NOT SHIPPED (reserved) | n/a | Inspector (K22). Design-spec note only. |
| `Platform::request_animation_frame` verb | NOT SHIPPED (reserved) | n/a | R-track / Wasm. Design-spec note only. |
| `Effect::DeferAsync(async fn)` | NOT SHIPPED (reserved) | n/a | SF08. Design-spec note only. |

## Telemetry Shape

```rust
#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct FrameProfile {
    pub frame_index: u64,
    pub frame_duration_total: Duration,
    pub active_animations: usize,
    pub primitive_count: u32,
    pub overruns: FramePhaseSet,  // bitset
    pub dropped_frames: u32,       // reserved for future drift detection
}

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct FrameProfileDetailed {
    /// Heap-backed slice with `len() == FramePhase::count()`. Implemented
    /// as `Box<[Duration]>` rather than a fixed-size array so that
    /// `FramePhase` can grow under `#[non_exhaustive]` without changing
    /// the public field's type — see review-fix note in `frame/profile.rs`.
    pub per_phase: Box<[Duration]>,
    pub effect_drain_count: u32,
    pub effect_drain_requeued: u32,
    pub deadline_overrun: SmallVec<[(FramePhase, Duration); 4]>,
}
```

`FrameProfile` is ~32 bytes, always populated by `run_frame` (no `Duration` measurements in
release unless `profiling_enabled`). `FrameProfileDetailed` is `Option` on `App`, populated
only when `set_profiling_enabled(true)`. Default state: `profiling_enabled = cfg!(debug_assertions)`.

## Migration Plan

### Required migrations: zero

All 36+ existing `cx.defer` callsites continue to work unchanged via the
`DeferPlacement::EndOfUpdate` default.

### Recommended migrations (additive, can land any time)

- `Window::on_next_frame` callers can migrate to `Window::on_pre_frame` to clear the
  `#[deprecated]` warning. One release cycle to clean up Tier-C.
- Tier-C tests using the auto-redraw smell can migrate to explicit
  `TestApp::advance_frame()`. K04+1 will flip the `auto_advance_frames_on_flush` default to
  `false`.
- Animation code that read `Clock::now()` directly should switch to `FrameClock::now()`
  inside any phase callback.

### Breaking changes (intentional)

- `AnimationController::value()` is now per-frame stable (cached). Code that relied on
  observing intra-frame drift will see no drift.
- `Window::request_animation_frame()` is now idempotent within a frame. Code that previously
  queued one callback per `request_animation_frame` call will see those calls coalesce.

## Rejected Alternatives

| Rejected | Reason |
|---|---|
| `Effect::DeferTo { placement, callback }` as a new sibling variant | Doubles match arms across `flush_effects` and dedup logic with zero benefit. Single-variant-with-field is the Compose `LaunchedEffect`-family pattern. |
| `trait FrameClock` injected via DI | A second injection point on `App` for time. The existing `Arc<dyn Clock>` injection is sufficient; `FrameClock` is a struct layered on top. One injection point, one bug surface. |
| Per-window `FrameClock` (one per `Window`) | Fragments determinism for cross-window animations. Flutter / Web / SwiftUI all use a single time origin. `FrameClockView` opaque indirection reserves the per-window option for future R-track / Wasm work. |
| Hard-abort deadlines on all phases | Aborting layout / paint mid-frame leaves the scene in an undefined state. Only `EffectFlush` (atomic work units) is safely interruptible. |
| Open `FramePhase` registry (Bevy-style `ScheduleLabel`) | Flutter's closed `SchedulerPhase` enum has been stable for 10 years. `#[non_exhaustive]` gives Bevy-style additive evolution without the runtime label-registry cost; the closed shape is a feature, not a limitation, given K04's audience (Framework tier, not arbitrary plugin authors). |
| Removing the test-mode auto-redraw immediately | Would force ~50 Tier-C test migrations in K04. Behind-a-flag with deprecation in K04+1 is the safer two-step landing. |
| Bind `on_next_frame` semantics to "fire after draw" without rename | Today it fires before draw. Changing the semantics would silently break dozens of callsites. Renaming with `#[deprecated]` alias is the only safe move. |
| Open `TickTarget` trait | Tier-C should not be able to inject arbitrary tick targets in K04. Sealing the trait makes the active set engine-controlled; SF08 opens it additively when async widgets land. |
| `Platform::request_animation_frame` trait verb in K04 | Premature. Current platform-driven frame pacing (winit/wgpu on_request_frame) is sufficient. Reserved as design-spec note for R-track / Wasm. |
| Per-phase logs as opt-in | Every UI framework that allowed per-phase logging in committed code regretted it. `docs/promt.md` §3.1 forbids it. Only `WARN` overruns and `FrameProfile` instrumentation are allowed in committed code. |

## Forward Hooks Reserved (NOT implemented in K04)

These are documented intent so future specs can land additively under `#[non_exhaustive]`.
K04 implements none of them.

| Hook | Reserved for | Status |
|---|---|---|
| `FramePhase::HotReload` variant | R-track (hot-reload) | Design comment only |
| `Platform::request_animation_frame()` trait verb | R-track / Wasm / iOS scenes | Design comment only |
| `Effect::DeferAsync(async fn)` variant | SF08 (async widgets) | Design comment only |
| `FrameClockView` per-window epoch divergence | R-track (Wasm tab pause, iOS background scene) | API shape exists; today returns App view |
| `App::observe_phase(...)` subscription | K22 (inspector intro API) | Design comment only |
| `App::run_idle(deadline: Duration)` body | R-track / Bevy-style idle scheduling | API shape stubbed in K04, body is no-op |
| `FrameProfile::custom_metric(name, value)` typed registry | K22 (inspector typed metrics) | Design comment only |
| `DeferPlacement::BeforeBuild` variant | SF05 (`setState` + dirty list) | Reserved under `#[non_exhaustive]` |
| `DeferPlacement::AfterSettle` variant | SF08 (async future-settled) | Reserved under `#[non_exhaustive]` |
| Open `TickTarget` trait (remove sealing) | SF08 / future audio / spring / particle controllers | Reserved as additive change |
| Parallel phase execution (DAG-shaped) | Future P-track if profiling justifies | Out of scope; representation stays linear |
| Fixed-step inner loop (Glenn Fiedler accumulator) | Animation libraries that want spring physics | Out of scope; live inside the `tick` phase if implemented |

## Review Gates

Before PR merge:

- **`flui-arch-reviewer`** for the `App` / `Window` / scheduler / `Effect` boundary and the
  seven-phase contract.
- **`migration-risk-adversary`** because K04 alters effect ordering, `defer` semantics, and
  renames `on_next_frame`. All three changes are load-bearing for every Tier-C `cx.defer` and
  `on_next_frame` callsite.
- **`rust-api-migration-auditor`** for any public additions (`FrameClock`, `FrameClockView`,
  `DeferPlacement`, `FramePhase`, `FrameProfile`, `FrameProfileDetailed`, `TickTarget`,
  `TickTargetId`, `TickOutcome`), prelude / re-export changes, and trait-object decisions on
  `trait TickTarget`.
- **`wgpu-gpu-reviewer`** only if implementation unexpectedly touches `scene.rs`,
  `platform/wgpu`, Metal/DirectX renderers, or shader modules. K04 should not touch any of
  these.

## Known Limitations (intentional, deferred to follow-up specs)

1. **No hot mode** for non-effect phase deadlines. Reserved for SF05.
2. **No per-window `FrameClock` epoch divergence.** `FrameClockView` is an opaque indirection
   that today always returns the App view. Future R-track / Wasm work fills this.
3. **No async-effect placement.** `Effect::DeferAsync` is reserved for SF08.
4. **No hot-reload phase.** `FramePhase::HotReload` is reserved for R-track.
5. **Test-mode auto-redraw stays behind a flag in K04.** K04+1 will flip the default after
   Tier-C tests migrate to `TestApp::advance_frame`.
6. **`App::observe_phase` subscription** is reserved; K22 (inspector) will implement.
7. **`Platform::request_animation_frame` trait verb** is reserved; R-track will implement.
8. **`TickTarget` is sealed.** SF08 will open it additively when async widgets land.
9. **Parallel phase execution** is out of scope. The DAG-shape representation is reserved as
   a future hook if profiling justifies; K04 stays single-threaded.

## References

- `docs/promt.md` §3.1 (frame budget table), §4.6 (`FrameClock` + `FrameProfile` proposal),
  §5 (60 FPS hot-path hit list), §6 (phased execution plan).
- `.ai-factory/RESEARCH.md` Active Summary (K04 status), Sessions (Phase 0-K Kernel Cleanup
  audit).
- `.ai-factory/ROADMAP.md` line 63 (K04 critical-chain entry).
- `.ai-factory/ARCHITECTURE.md` (Tier A/B/C model, Phase 0-K context).
- K99/K15/K07/K05/K01/K02/K03 design specs (prior Phase 0-K critical-chain).
- Flutter `SchedulerBinding`:
  <https://api.flutter.dev/flutter/scheduler/SchedulerBinding-mixin.html>,
  <https://api.flutter.dev/flutter/scheduler/SchedulerPhase.html>.
- Compose `MonotonicFrameClock`:
  <https://developer.android.com/reference/kotlin/androidx/compose/runtime/MonotonicFrameClock>.
- HTML Living Standard event loop:
  <https://html.spec.whatwg.org/multipage/webappapis.html#event-loops>.
- SwiftUI render loop reverse-engineering: <https://rensbr.eu/blog/swiftui-render-loop/>.
- React Scheduler: <https://jser.dev/react/2022/03/16/how-react-scheduler-works/>.
- Bevy Schedule v3: <https://bevy-cheatbook.github.io/programming/schedules.html>; v1→v3
  migration PRs <https://github.com/bevyengine/bevy/pull/6587>,
  <https://github.com/bevyengine/bevy/pull/7267>.
- Glenn Fiedler "Fix Your Timestep!": <https://gafferongames.com/post/fix_your_timestep/>.
- Robert Nystrom "Game Loop": <https://gameprogrammingpatterns.com/game-loop.html>.
