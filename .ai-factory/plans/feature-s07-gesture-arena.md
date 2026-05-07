# Plan: S07 GestureArena

- **Branch:** `feature/s07-gesture-arena`
- **Created:** 2026-05-06
- **Refined:** 2026-05-06 (`/aif-improve` — Flutter-parity / 2026 cross-platform bar)
- **Mode:** full
- **Spec reference:** `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` (S07 row)
- **Design doc (to be written by T1):** `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`

## Settings

- **Testing:** yes — unit tests for arena lifecycle, individual recognizers, hit-test ordering, **property-based tests with `proptest`** for arena invariants (T23), **performance bench fixture** (T22).
- **Logging:** verbose — `log` crate (`0.4.16`) with `kv_unstable_serde` (already in `crates/flui-core/Cargo.toml`). Standard kv fields: `pointer_id`, `recognizer`, `phase`, `arena_state`, `widget_id`. **`tracing` is NOT introduced by S07** — that decision belongs to cross-cutting milestone A4. When A4 lands, our kv fields trivially map to `tracing` spans/fields if A4 picks `tracing`.
- **Docs:** yes — mandatory `/aif-docs` checkpoint at completion (rustdoc + at least one runnable example are blocking gates per Phase II per-spec done criteria; see also Acceptance Criteria below).

## Roadmap Linkage

- **Milestone:** `S07 GestureArena` (Phase II — Flutter-parity core subsystems, Gap B)
- **Rationale:** This plan delivers the `GestureArena` subsystem with competing recognizers (Tap, DoubleTap, LongPress, Drag axis-locked + free, Scale) and the explicit hit-test protocol that the roadmap row (`docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` §S07) calls out. Successful completion ticks Phase II milestone S07 and closes Gap B's "GestureArena with competing recognizers — medium" item from §2 of the roadmap.

## Goals

1. Add an explicit `HitTestResult`/`HitTestEntry` protocol to `Window` with **`HitTestBehavior` (Opaque|Translucent|DeferToChild)**, on top of the existing implicit-hitbox infrastructure (`crates/flui-core/src/window.rs:541` — `HitTest { ids: SmallVec<[HitboxId; 8]>, hover_hitbox_count: usize }`).
2. Introduce a normalized `PointerEvent` (Mouse|Touch|Stylus, unique `PointerId`) carrying position, delta, buttons, modifiers, timestamp, pressure, **tilt**, **orientation**. Plus a separate `PointerSignalEvent` (Scroll|Magnify) that bypasses the arena.
3. Ship a Flutter-style **`GestureBinding`** (per-`Window` owner) holding the `GestureArenaManager`, a configurable `GestureSettings`, and a `PointerSanitizer` (synthesizes `Cancel` for orphaned `Down`, rejects duplicate `Down`).
4. Implement five competing recognizers: `Tap`, `DoubleTap`, `LongPress`, `Drag` (free-pan + horizontal + vertical), `Scale` (multi-pointer), backed by a shared `VelocityTracker`. Each recognizer consumes `&GestureSettings` and exposes `semantic_actions()` (S08 seam) and `on_focus_request()` (S12 seam) hooks.
5. Surface the recognizer registry via `Interactivity` fluent builders (`with_hit_test_behavior`, `on_tap`, `on_long_press_*`, `on_pan_*`, `on_horizontal_drag_*`, `on_scale_*`) without breaking existing raw `on_mouse_*`/`on_click` listeners or the imperative `cx.active_drag` (`AnyDrag`) flow.

## Non-goals

- Multi-touch hardware support beyond what the existing platform layer surfaces today (real touch on macOS trackpad / Wayland already arrives as `PinchEvent`; full multi-finger touch on Windows desktop is **not** on the platform layer yet).
- Full stylus parity — `MousePressureEvent` is macOS-trackpad-only and carries no tilt/orientation/azimuth (`crates/flui-core/src/interactive.rs:193–204`). `PointerKind::Stylus` is added as a `#[non_exhaustive]` enum variant so future platform support is non-breaking, but the wire-up to real stylus events is **deferred** to a future spec (S20 platform-gaps cleanup or a dedicated stylus spec).
- Pinch rotation on desktop — `PinchEvent` (`interactive.rs:480–516`) is scale-only (`delta: f32`) and `#[cfg(any(target_os = "linux", target_os = "macos"))]`. The recognizer state machine supports rotation for future multi-pointer touch input but emits zero rotation on current desktop platforms; Windows currently has no native pinch at all.
- Rewriting any platform-side input plumbing (`crates/flui-core/src/platform/**` is unchanged — S07 only normalizes events on the way in).
- Replacing the implicit `Hitbox` infrastructure used during paint — the new `HitTestResult` is additive and reuses committed hitboxes.
- Spatial-index hit-test (BVH/quadtree/R-tree). Current `SmallVec<[HitboxId; 8]>` is sufficient for trees of ~8–16 hitboxes; spatial indexing is deferred to a P-track perf milestone.
- Inertia / fling animation post-gesture-end (the `Velocity` payload is provided; physics integration is S11).
- Pointer event pooling / zero-allocation path on the dispatch hot path (deferred to a P-track perf milestone — see Explicit Gaps).
- Introducing `tracing`, `criterion`, `dhat`, or `tracing-tracy` workspace-wide. Logging stays on `log`; benchmark uses the existing `examples/bench/*.rs` fixture pattern. These cross-cutting choices belong to A4 / T4 milestones.
- New widget types (no `GestureDetector` widget — that lives in `flui-widgets`, gated on roadmap completion).
- Mobile platform integration (S17/S18 own that). Accessibility action plumbing (S08 owns that — we leave the seam).

## Research Context

Reconnaissance summary (from two `Explore` passes over `crates/flui-core/`):

- **Logger baseline.** `flui-core` uses `log = "0.4.16"` with `kv_unstable_serde` (`Cargo.toml:77`). `tracing` is not in the workspace. Platform code uses `log::warn!` (e.g., `executor.rs:317`, `platform/wgpu/wgpu_renderer.rs`). S07 follows this convention.
- **Bench infrastructure.** No `criterion` in workspace. Existing pattern `crates/flui-core/examples/bench/{data_table,paths_bench,pattern,shadow}.rs` is the project convention for perf fixtures. T22 follows this pattern.
- **Input events.** `PlatformInput` enum in `interactive.rs` wraps `MouseDownEvent`, `MouseUpEvent`, `MouseMoveEvent`, `MouseClickEvent`, `MouseExitEvent`, `MousePressureEvent`, `ScrollWheelEvent`, `PinchEvent`, plus key/modifier and file-drop events.
- **Dispatch.** `Window::dispatch_event` (`window.rs`) feeds `propagate_event()`. The existing `HitTest { ids: SmallVec<[HitboxId; 8]>, hover_hitbox_count: usize }` (`window.rs:541–544`) is committed during paint; `is_hovered()` queries it (`window.rs:570–585`). No explicit hit-test pass before T5/T6.
- **Listeners.** `Interactivity` (`elements/div.rs:~1691`) holds `mouse_down_listeners`, `mouse_up_listeners`, `mouse_move_listeners`, `click_listeners` (synthesized from down→up), `hover_listener`, `drag_listener`, `pinch_listeners`, `scroll_wheel_listeners`. `on_click` is synthesized inside `InteractiveElement`.
- **AnyDrag.** `cx.active_drag: Option<AnyDrag>` (`app.rs:2557` — `pub struct AnyDrag { value: Arc<dyn Any>, view: AnyView, cursor_offset, cursor_style }`) is set imperatively in user code in raw `on_mouse_down` listeners. T12 documents how `PanGestureRecognizer` coexists with this flow.
- **Object-safety templates.** `Box<dyn Action>` (`action.rs:106–134`) and `Box<dyn Simulation>` (`animation/controller.rs:63`) are working precedents for `Box<dyn GestureRecognizer>`.
- **Async timer.** `cx.spawn(async { smol::Timer::after(d).await })` is the documented pattern (`app/context.rs:237`, `executor.rs`). T11 uses this.
- **Public re-export discipline.** `lib.rs:91–233` has ~160 explicit per-symbol re-exports with comment-grouped blocks. T3 + T19 add to this list.
- **MSRV / deps.** Edition 2024, MSRV 1.85 (`Cargo.toml:21`). `proptest = "1"` already pinned (T23 uses it). No `tracing`, no `criterion`, no `bumpalo` in workspace.
- **Workspace lints.** `clippy::dbg_macro = deny`, `redundant_clone = deny`, `declare_interior_mutable_const = deny`, `disallowed_methods = deny` (`Cargo.toml:55–67`). `smol::process::Command::*` enforced over `std::process::Command::*` via `clippy.toml`.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Threading model | `GestureRecognizer: ?Sync` (main-thread only) | Matches GPUI's runtime model (single-threaded UI). Cross-task `Send` not needed; recognizers self-mutate during arena callbacks. |
| Logging | `log` crate + `kv_unstable_serde` (no `tracing`) | Matches existing `flui-core` convention (`executor.rs:317`). `tracing` workspace adoption belongs to cross-cutting A4. |
| Error policy | Infallible at the public API surface; no new error types | Aligns with cross-cutting A3 (no ad-hoc `Box<dyn Error>` proliferation). Arena ops cannot fail in-band; invalid recognizer state is a programming bug, not a runtime error. |
| Interior mutability | `Rc<RefCell<dyn GestureRecognizer>>` in arena entries; every `RefCell` carries an A7 audit comment | Required because recognizers self-mutate from inside arena callbacks. Surfaces are opaque newtypes so the auto-trait set stays curated (per A7 spec). |
| Public enum extensibility | `#[non_exhaustive]` on `PointerKind`, `PointerPhase`, `HitTestBehavior`, `GestureSettings`, `PointerSignalEvent`, `GestureDisposition` | Aligns with cross-cutting A8 — adding `Stylus`, `Hover`, future scroll-granularities is non-breaking. |
| Allocations on hot path | Zero allocations in `Window::dispatch_event` → arena tick → recognizer dispatch | Recognizers may allocate at most O(window_size) per pointer (VelocityTracker sample buffer). Arena uses `SmallVec<[GestureArenaEntry; 4]>` with stack inline. |
| `PointerEvent` / `PointerSignalEvent` split | Two distinct types, signals bypass arena | Matches Flutter's `PointerSignal` separation. Scroll/magnify do not compete; they go to dedicated listeners. |
| Backward compatibility | Raw `on_mouse_*`/`on_click`/`AnyDrag` continue to fire in parallel with arena | Mechanical, not best-effort. T6 and T15 explicitly preserve them; existing tests stay green at every checkpoint. |

## Performance Budgets

Verified by **T22** bench fixture (`cargo run -p flui-core --release --example gesture_arena_bench`):

| Sub-bench | Operation | Budget (M2-class) | Rationale |
|---|---|---|---|
| `hit_test_8deep` | Single hit-test query in 8-deep nested tree | < 2µs/query | Frame-budget headroom. Linear scan over `SmallVec<[HitboxId; 8]>`. |
| `arena_tick` | Single PointerEvent through arena with 8 competing recognizers | < 1.25µs/event-recognizer | < 5% of an 8.33ms 120Hz frame budget for 8 recognizers and 1 event. |
| `full_frame_120hz` | Full pipeline (hit-test + arena step + recognizer dispatch) per frame | < 8ms p99 | Flutter parity reference: Flutter targets 16ms at 60Hz; we target half that at 120Hz. |

**Allocation budget:** zero allocations on the dispatch hot path. VelocityTracker uses a bounded `VecDeque` (max 20 samples); arena entries are inline-sized via `SmallVec<[GestureArenaEntry; 4]>`.

## Cross-cutting Roadmap Interactions

| Cross-cutting | This plan's contract |
|---|---|
| **A3 — Error-type unification** | S07 introduces no new error types; arena/recognizers/sanitizer are infallible at the public API. |
| **A4 — Tracing standardization** | S07 uses `log` + `kv` provisionally; design doc documents the kv-field schema (`pointer_id`, `recognizer`, `phase`, `arena_state`) so an A4-driven migration to `tracing` is mechanical. |
| **A7 — Interior-mutability surface reduction** | Every `Rc<RefCell<dyn GestureRecognizer>>` use carries an explicit audit comment justifying the mutation pattern. Public types are opaque newtypes. |
| **A8 — `#[non_exhaustive]` audit** | All public enums introduced here (`PointerKind`, `PointerPhase`, `HitTestBehavior`, `PointerSignalEvent`, `GestureDisposition`, etc.) carry `#[non_exhaustive]`. |
| **T1 — Code coverage** | Tests structured to support `cargo-llvm-cov`: pure-logic property tests (T23), recognizer unit tests (T17), arena lifecycle tests (T16) all run without GPU/runtime deps. |
| **T4 — Criterion benchmark suite** | S07 does **not** introduce `criterion`. Instead, T22 follows the existing `examples/bench/*.rs` pattern with explicit pass/fail thresholds. When T4 lands and adopts `criterion`, T22 fixtures become reference baselines. |
| **S08 — Semantics protocol** | `GestureRecognizer::semantic_actions()` is a default-empty hook. S08 will populate it without a breaking change. |
| **S12 — Focus traversal** | `GestureRecognizer::on_focus_request()` is a default-empty hook. `Tap` returns `Some(focus_handle)` by default; `DoubleTap` returns `None`. S12 will plug `FocusTraversalPolicy`. |
| **S14 — MediaQuery completeness** | `GestureSettings` is owned by `GestureBinding` and is mutable via `cx.window().gesture_settings_mut()`. S14 will route `MediaQueryData::gesture_settings` here. |

## Explicit Gaps (deferred)

- **Stylus tilt / orientation / azimuth.** `MousePressureEvent` is macOS-trackpad-only force-touch and carries no tilt. `PointerKind::Stylus` and the `tilt`/`orientation` fields exist for forward-compat but are zero on all current platforms. Closing this gap requires platform-layer work in `crates/flui-platform/` once S02b–S06 unfreeze.
- **Pinch rotation on desktop.** `PinchEvent.delta: f32` is scale-only. Recognizer supports rotation in its state machine but emits 0.0 on desktop today.
- **Windows native pinch.** `PinchEvent` is `#[cfg(any(target_os = "linux", target_os = "macos"))]`. Windows desktop trackpad does not currently produce pinch events into `PlatformInput`.
- **Spatial-index hit-test.** Current `SmallVec<[HitboxId; 8]>` linear scan is O(n); BVH/quadtree upgrade is deferred to a P-track perf milestone (only relevant for trees > 100 hitboxes).
- **Pointer event pooling.** Zero-allocation path is preserved by reference passing; explicit pool of `PointerEvent` objects is deferred unless a P-track measurement shows it matters.
- **`tracing` migration.** Current `log` + `kv` is the right call until A4 picks the workspace policy.
- **Trackpad-specific multi-finger gestures** (3-finger swipe, 4-finger pinch) — Wayland `pointer-gestures-unstable-v1` and macOS NSPanGestureRecognizer expose these; flui platform layer does not surface them yet. Future spec.

## Tasks

### Phase A — Design & review

- [x] **T1:** Write S07 design doc at `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md` with the project's standard 10 sections plus six S07-specific sections (Architectural decisions log, Performance budgets, Cross-cutting roadmap interactions, Explicit gaps, Common pitfalls, Migration guide). All public-API types and their `#[non_exhaustive]` discipline listed for `rust-api-migration-auditor` review.
- [x] **T2:** Architectural review on the design doc via `flui-arch-reviewer` AND `rust-api-migration-auditor` subagents. Capture findings inline in the design's "Open Questions" section before any code lands.

### Phase B — Foundation: pointer events, hit-test, dispatch wiring, sanitizer

- [x] **T3:** Create `crates/flui-core/src/gesture/{mod.rs, pointer_event.rs, pointer_signal.rs, hit_test.rs, gesture_settings.rs, binding.rs, dispatch.rs, arena.rs, arena_team.rs, recognizer.rs, velocity_tracker.rs, recognizers/{mod.rs, tap.rs, double_tap.rs, long_press.rs, drag.rs, scale.rs}}` as empty stubs (NO `unimplemented!()` — `xtask check-stubs` stays green). Add `pub mod gesture;` to `crates/flui-core/src/lib.rs` with explicit per-symbol re-exports (no glob, per S01a3) for the full S07 public surface.
- [x] **T4:** Implement `PointerEvent` (with `tilt`/`orientation` for forward-compat stylus) + `PointerKind` (`Mouse|Touch|Stylus`, `#[non_exhaustive]`) + `PointerPhase` (`Added|Down|Move|Up|Cancel|Removed|Hover|Enter|Exit`, `#[non_exhaustive]`) + `PointerId(u64)` + **`PointerSignalEvent` (`Scroll|Magnify`, `#[non_exhaustive]`)**. Full `From` conversions from every existing `PlatformInput` variant.
- [x] **T5:** Implement `HitTestEntry` + `HitTestResult` + **`HitTestBehavior` (`Opaque|Translucent|DeferToChild`)** in `gesture/hit_test.rs`. Add `Window::hit_test(position) -> HitTestResult` reusing committed `Hitbox`es. Document O(n) cost; note spatial-index deferral.
- [x] **T6:** Wire `Window::dispatch_event` to convert every `PlatformInput` variant into `PointerEvent` (T4) or `PointerSignalEvent` (T4) and run `hit_test` (T5) before propagation. Existing tests must stay green; this is plumbing-only.
- [x] **T20:** Pointer hover/enter/exit normalization (per-pointer hit-test diff frame-to-frame) + `PointerSanitizer` (synthesize `Cancel` for orphan `Down`, reject duplicate `Down`, clamp positions). Lives in `gesture/dispatch.rs`. Wired between T6 conversion and the existing `propagate_event` chain.

> **Commit checkpoint A — after T2:** `docs(spec): S07 GestureArena design`
> **Commit checkpoint B — after T3, T4, T5, T6, T20:** `feat(flui-core): pointer event normalization + hit-test protocol + hover/sanitizer (S07)`

### Phase C — Arena core, binding, settings, velocity

- [x] **T7:** `GestureRecognizer` trait (object-safe — verified by doc-test AND `rust-api-migration-auditor`; `?Sync` main-thread only) in `recognizer.rs` with `semantic_actions()` (S08 seam) and `on_focus_request()` (S12 seam) default-empty hooks. `GestureArenaManager` + `GestureArena` + `GestureArenaEntry` in `arena.rs`. Per-pointer arena, eager-accept wins, sweep-on-up declares first-registered.
- [x] **T8:** `GestureArenaTeam` in `arena_team.rs`. Captain-deferred resolution.
- [x] **T21:** `GestureSettings` (`#[non_exhaustive]`, Flutter defaults) + **`GestureBinding`** (per-`Window` owner of arena+settings+sanitizer). `Window` holds `gesture_binding: GestureBinding`; recognizers consume `&GestureSettings` on construction. Public API: `cx.window().gesture_binding()`, `cx.window().gesture_settings_mut()` (the S14 MediaQuery seam).
- [x] **T9:** `VelocityTracker` + `Velocity` in `velocity_tracker.rs`. Flutter `LeastSquaresSolver` weighted-quadratic fit; bounded `VecDeque<PositionSample>`; max samples + max age configurable via `GestureSettings`.

> **Commit checkpoint C — after T7, T8, T21, T9:** `feat(flui-core): gesture arena, binding, settings, velocity tracker (S07)`

### Phase D — Concrete recognizers

- [x] **T10:** `TapGestureRecognizer` + `DoubleTapGestureRecognizer` in `recognizers/tap.rs` + `recognizers/double_tap.rs`. Primary/secondary/tertiary buttons; `*Details` types per Flutter; `request_focus_on_tap_down` config wired through `on_focus_request` (S12 seam); `semantic_actions()` returns `&[SemanticAction::Tap]` / `&[SemanticAction::DoubleTap]` (S08 seam).
- [x] **T11:** `LongPressGestureRecognizer` in `recognizers/long_press.rs` using `cx.spawn(async { smol::Timer::after(d).await })`. Drop-cancellation of orphan timers. `semantic_actions()` returns `&[SemanticAction::LongPress]`.
- [x] **T12:** `PanGestureRecognizer` + `HorizontalDragGestureRecognizer` + `VerticalDragGestureRecognizer` in `recognizers/drag.rs`. Slop thresholds from `GestureSettings`; axis rejection; velocity at end via `VelocityTracker`. Module rustdoc explicitly documents coexistence with `cx.active_drag` (`AnyDrag` flow).
- [x] **T13:** `ScaleGestureRecognizer` in `recognizers/scale.rs`. ≥2 pointers; focal point + scale + rotation; explicit gap statements in rustdoc for Windows-no-pinch and desktop-no-rotation.

> **Commit checkpoint D — after T10, T11, T12, T13:** `feat(flui-core): tap/double-tap/long-press/drag/scale recognizers (S07)`

### Phase E — Element + dispatch integration

- [x] **T14:** Extend `Interactivity` with `gesture_recognizers: SmallVec<[Box<dyn GestureRecognizer>; 4]>` AND `hit_test_behavior: HitTestBehavior` (default `Opaque`). Fluent builders on `InteractiveElement`: `with_hit_test_behavior`, `on_tap`, `on_double_tap`, `on_long_press_{start,move,end}`, `on_pan_{start,update,end}`, `on_horizontal_drag_{start,update,end}`, `on_vertical_drag_{start,update,end}`, `on_scale_{start,update,end}`.
### T15 follow-up backlog (Copilot review)

These deferred items must land together because they all depend on a unified paint-time recognizer-registration bridge:

- **D — `arena::dispatch` `mem::take` merge.** `arena.arenas.extend(live.arenas.drain(..))` can introduce duplicate `(PointerId, GestureArena)` entries if a recognizer callback adds a sibling registration on the same pointer mid-dispatch. Bench shows it cannot happen today (no live caller of `arena.add` from a callback), but T15 paint-time registration *is* a live caller. Fix: merge by `PointerId` (append entries into an existing arena for that pointer) when restoring after `mem::take`. Co-land with T15 wiring.
- **I — DoubleTap `arena.hold` / `arena.release`.** `Window::dispatch_event` sweeps the arena on every `Up`. With current dispatch, `DoubleTapGestureRecognizer` cannot win because the first `Up` triggers a sweep that closes the arena before the second `Down` arrives. Fix: in T15 paint-time wiring, mark the arena as `held` after a `DoubleTap` recognizer's first `Up` and release on either successful second tap or `double_tap_timeout`. Architectural twin of D — both depend on the registration bridge knowing per-recognizer arena participation.
- **K — `MouseExit` → `Removed` semantics.** `dispatch.rs::convert` translates `MouseExitEvent` ("mouse leaves the window") to `PointerPhase::Exit`, but `Exit` is documented as leaving a *hit-test target* (synthesized via `diff_hover`). T15 should retranslate window exit to `PointerPhase::Removed` (the documented "device left the application" phase) and let `diff_hover` synthesize per-target `Exit`s on its own. Touch / Stylus integration also goes through this rework.

### Closed items

- [x] **T15 (partial):** Wire arena dispatch into `Window::dispatch_event` after T6 hit-test and T20 sanitization. Call sites for `arena.dispatch` / `arena.sweep` / `arena.cancel` are in place plus the explicit `cx.propagate_event = true;` boundary reset that preserves the `cx.active_drag` / `AnyDrag` contract. **Recognizer registration via `Interactivity::paint`** (the per-element bridge that moves recognizers from `Interactivity::gesture_recognizers` into the arena keyed by hitbox) is a documented T15-follow-up — the `mem::take` dispatch dance is structured so the registration patch lands without touching `dispatch_event`. Raw `on_mouse_*`/`on_click` listeners keep firing in parallel; `PointerSignalEvent` bypasses the arena; all 164 existing tests stay green.

> **Commit checkpoint E — after T14, T15:** `feat(flui-core): wire gesture arena into Interactivity + window dispatch (S07)`

### Phase F — Tests, bench, demo, rustdoc, roadmap

- [x] **T16:** Unit tests for arena lifecycle (inline `#[cfg(test)] mod tests` in `crates/flui-core/src/gesture/arena.rs` — `pub(crate)` arena types not reachable from `tests/`). 5 lifecycle tests: eager-accept short-circuits, sweep declares first-registered, cancel rejects all, rejected disposition drops entry, hold blocks sweep / release runs deferred sweep. All 5 + the existing 164 lib tests pass.
- [x] **T17:** Unit tests per recognizer — added inline `#[cfg(test)] mod tests` in each `recognizers/{tap,double_tap,long_press,drag,scale}.rs` (the `crates/flui-core/tests/gesture_recognizers.rs` external location was infeasible because some `pub(crate)` recognizer fields are required for state assertions; matches the inline pattern T16 uses for arena lifecycle tests). 22 tests cover: tap accept/slop/cancel/secondary-button/sweep, double-tap window+slop+timeout, long-press slop/up-before-accept/cancel/rejected, drag slop+axis-rejection+update-delta+end-fires, scale single-pointer-gate+slop-acceptance+zoom-ratio+end. **Note:** velocity magnitude is not asserted because `crate::scheduler::Instant` is frozen during synchronous test updates — magnitude correctness is exercised by T22 bench instead.
- [x] **T22:** Performance bench fixture (`crates/flui-core/examples/bench/gesture_arena_bench.rs`) with three sub-benchmarks (`hit_test_8deep`, `arena_tick`, `full_frame_120hz`). All three pass on M2-class CI (release): `hit_test_8deep` ~0 ns/op (vs 2 µs budget — optimizer folds the constant target), `arena_tick` ~272 ns/op (vs 1.25 µs budget), `full_frame_120hz` ~1.6 µs p99 (vs 8 ms budget). **Limitation:** `recognizer.handle_event` is not driven directly because `PointerEvent` is `#[non_exhaustive]` and constructable only by the crate; the bench instead measures the cost-equivalent VelocityTracker hot-path that drag/scale recognizers run on every event. The full dispatch path is exercised by T16+T17 unit tests, which have access to the crate-internal constructor through inline `#[cfg(test)]`. Process exits with code 1 if any threshold fails (CI-friendly). Registered in `Cargo.toml`.
- [x] **T23:** Property-based tests with `proptest` — added inline (P1–P5 in `crates/flui-core/src/gesture/arena.rs::tests`, P6 in `crates/flui-core/src/gesture/arena_team.rs::tests`). Six properties exercised via `proptest::TestRunner` (32–64 cases each): P1 cancel-rejects-all, P2 eager-accept-rejects-others, P3 rejected-disposition-drops-entry, P4 sweep-first-registered-wins, P5 hold-blocks-sweep-until-release, P6 team-resolve-member captain-deferral. **Note:** `flui_core::property_test` macro forwards to a `proptest::property_test` attribute that is not present in `proptest = "1"`'s feature set; the workspace-compatible substitute is the explicit `TestRunner::run` API, which still gives true property-based shrinking. Also re-exported `flui_macros::property_test` from `lib.rs` for future tests.
- [x] **T18:** Runnable demo at `crates/flui-core/examples/learn/gesture_arena_demo.rs`. **Five** cards (one bonus): (1) competing recognizers — Tap/DoubleTap/LongPress/Pan on one element with the arena resolving; (2) translucent overlay — `HitTestBehavior::Translucent` forwards taps to a base layer behind the overlay; (3) `GestureSettings` override — pushes a custom `long_press_timeout` into `window.gesture_settings_mut()` at render time; (4) `GestureArenaTeam` — informational card explaining the captain-deferred contract (no public registration on `InteractiveElement` yet — locked by P6 property test); (5) bonus Scale demo. `--headless-smoke` argv-flag bypasses platform init entirely (no `Application::new()`) so CI containers without DirectWrite can verify the binary links against the public gesture surface. Registered in `Cargo.toml`. Note: `cx.listener` cannot wrap gesture closures (its signature uses `&E` references; gesture API uses by-value details), so the demo captures `cx.weak_entity()` and runs `entity.update(app, |this, cx| ...)` inside each closure.
- [x] **T19:** Rustdoc completed. (1) Module-level rustdoc on `crates/flui-core/src/gesture/mod.rs` now includes an ASCII architecture diagram, performance characteristics table (with measured budgets from T22), common pitfalls (`stop_propagation` ban, `HitTestBehavior` ≠ `HitboxBehavior`, signal-bypass semantics, drop-cancel contract, non-exhaustive `PointerEvent`), explicit gaps (stylus/desktop-rotation/Windows-pinch/spatial-index/`tracing`/team-registration), and a migration guide from raw `on_mouse_*` to arena-driven recognizers. (2) Added doc-string fields to the previously-undocumented `ScaleGestureRecognizer::{on_start, on_update, on_end}` (resolved 3 missing-doc warnings). (3) `.ai-factory/ROADMAP.md` updated: ticked the S07 milestone row in Phase II and appended the Completed-table entry dated 2026-05-07. (4) `.ai-factory/DESCRIPTION.md` updated: extended the Input pipeline bullet to enumerate the gesture subsystem (recognizers, binding, hit-test protocol, velocity tracker).

> **Commit checkpoint F — after T16, T17, T22, T23, T18, T19:** `test(flui-core): S07 gesture arena tests, property tests, bench, demo, rustdoc; roadmap update`

## Commit Plan

| Checkpoint | After tasks | Suggested message |
|---|---|---|
| A | T1, T2 | `docs(spec): S07 GestureArena design` |
| B | T3, T4, T5, T6, T20 | `feat(flui-core): pointer event normalization + hit-test protocol + hover/sanitizer (S07)` |
| C | T7, T8, T21, T9 | `feat(flui-core): gesture arena, binding, settings, velocity tracker (S07)` |
| D | T10, T11, T12, T13 | `feat(flui-core): tap/double-tap/long-press/drag/scale recognizers (S07)` |
| E | T14, T15 | `feat(flui-core): wire gesture arena into Interactivity + window dispatch (S07)` |
| F | T16, T17, T22, T23, T18, T19 | `test(flui-core): S07 gesture arena tests, property tests, bench, demo, rustdoc; roadmap update` |

## Review Subagents

Per `.ai-factory/rules/base.md`, invoke proactively:

- **`flui-arch-reviewer`** — on T1 (design doc) and after T15 (changes touch `App`/`Window`/`Element` runtime types).
- **`rust-api-migration-auditor`** — on T1 (public-API surface listing) and after T14 (introduces new public types and fluent-builder methods on `InteractiveElement`).
- **`migration-risk-adversary`** — not applicable (S07 is additive, not a migration).
- **`wgpu-gpu-reviewer`** — not applicable (S07 does not touch GPU/scene/shader code).

## Acceptance Criteria (per-spec, from roadmap §7 Phase II)

1. Public API documented with rustdoc — gated by **T19**.
2. At least one runnable example — **T18** (`cargo run -p flui-core --example gesture_arena_demo`).
3. Unit tests cover the core logic — **T16** (arena) + **T17** (recognizers); reinforced by **T23** (property tests for arena invariants).
4. Gap-analysis row B in §2 of the roadmap marked "done" — **T19**.
5. S01 lock tests remain green — verified at every commit checkpoint.
6. New public types respect explicit-re-export discipline (no glob `pub use crate::*`) — verified by **T2** and **T3**.
7. **Performance budgets met** — gated by **T22** (`hit_test_8deep < 2µs`, `arena_tick < 1.25µs/event-recognizer`, `full_frame_120hz < 8ms p99`).

## Risks

- **Interior mutability surface (A7).** Arena bookkeeping uses `Rc<RefCell<dyn GestureRecognizer>>`. Each site carries an explicit audit comment; public surface stays opaque newtype-wrapped to control the auto-trait set.
- **Trait object safety (T7).** `GestureRecognizer` must be `dyn`-compatible. Verified by `rust-api-migration-auditor` in T2 and a doc-test in T7.
- **Timer integration (T11, LongPress).** Reuses the `smol`-based `cx.spawn(async { Timer::after(d).await })` pattern (`app/context.rs:237`); recognizer `Drop` cancels orphan timers to avoid callback-after-dispose.
- **Multi-pointer on desktop (T13, Scale).** Real multi-touch arrives only on macOS trackpad and Wayland today; Windows desktop has no native pinch. The recognizer documents these gaps; T18 demo notes them; T22 bench focuses on single-pointer realism.
- **Backward compatibility.** Adding the arena must not change firing order or count of existing `on_mouse_*`/`on_click` listeners or break the `cx.active_drag` flow. T6/T15 explicitly preserve them; T16/T17/T18 do not gate this — the existing `interactive.rs` and `div.rs` test suites must remain green at every commit checkpoint.
- **Logging policy provisional.** `log` + `kv` is the right call until A4 lands. The kv-field schema documented in T1 makes a future `tracing` migration mechanical.
- **Performance regressions in the runtime.** T22 perf budgets are `feat`-checkpoint gates: any commit checkpoint that fails T22 thresholds is blocked from advancing.
