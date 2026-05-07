# Plan: S07.6 GestureArena — recognizer roster expansion (MultiTap + ForcePress)

- **Branch:** `feature/s07-6-recognizer-roster`
- **Created:** 2026-05-08
- **Mode:** full
- **Predecessor PRs:** #1 (S07 GestureArena), #2 (S07.5 follow-up), and **S07.5b pre-roster cleanup** (`feature-s07-5b-pre-roster-cleanup.md` — must land before this plan starts).
- **Blocked by:** S07.5b. This plan reads from the post-S07.5b surface:
  - Single back-channel hook `set_arena_back_channel(pid, bc, idx)` (legacy two-arg form deleted; one trait method only).
  - `PointerEvent.pressure: Option<PressureSample>` instead of `f32`; `PressureSample::normalize()` returns `[0.0, 1.0]` clamped against the device's reported range.
  - `AllowedButtonsFilter` newtype struct (NOT `dyn Trait` alias) available; filter check happens in `register_recognizer` BEFORE arena add (post-S07.5b D10) — no zombie-arena slots on rejection.
  - `PointerEvent.provenance: PointerEventProvenance` enum (NOT bool) — recognisers can filter synthesised events.
  - `GestureRecognizer::handle_event(event: DeliveredEvent<'_>, ...)` signature (NOT `&PointerEvent`); slop checks read `event.local_position`, everything else reads `event.event.<field>`.
  - `GestureArena.hold_count: u32` (NOT bool) — holds compose. MultiTap and DoubleTap holding the same pointer increment+decrement independently.
  - Custom `Affine2` struct for `HitTestEntry.transform`; `HitTestResult` uses `HitTestScope<'_>` RAII for transform stack.
  - `PointerKind::{Trackpad, InvertedStylus, Unknown}` and `PointerPanZoomEvent` sibling type defined.
- **Source spec:** `docs/superpowers/specs/2026-05-08-gesture-roadmap-from-old-impl.md` — the v1 → v2 comparative review that surfaced two recognizer-roster gaps (`MultiTap`, `ForcePress`) and two pre-arena-pipeline gaps (resampling S07.7, prediction S07.8 — both deferred).
- **Audit context:** `docs/superpowers/specs/2026-05-08-flutter-gestures-architectural-audit.md` — the Flutter gestures architectural audit informs S07.5b prerequisites; only the specific items #1, #4, #9 from that audit are visible to this plan (everything else lives in S07.5b or in later milestones).
- **Working set:** the two recognizers grouped under "Suggested roadmap entries → S07.6" in the source spec. S07.7 (pointer resampling) and S07.8 (pointer prediction) are explicitly out of scope here — separate plans.

## Settings

- **Testing:** yes — every recognizer ships with unit tests (state-machine arms + threshold canary), property tests where the recognizer touches arena invariants (multi-pointer hold/release symmetry), and an end-to-end integration test through `Window::dispatch_event` for the user-facing builders. The end-to-end `multi_tap` test depends on a multi-pointer `simulate_*` helper; if none exists today, T18 introduces one as part of `test-support` (synthetic `PointerEvent` with explicit `pointer_id`/`kind`). The end-to-end `force_press` test synthesises a `PointerKind::Touch` stream because Mouse-class events are explicitly rejected by the recognizer (rationale in T12).
- **Logging:** verbose — `log` crate with `kv_unstable_serde` (matches the S07/S07.5 convention). New events on the MultiTap collection lifecycle and the ForcePress phase transitions use `kv` fields (`pointer_id`, `recognizer`, `phase`, `arena_state`, `lifecycle`, plus `pressure` / `pointer_count` where applicable). MultiTap logs `phase = "collect"|"complete"|"timeout"|"slop_reject"|"too_many_pointers"`. ForcePress logs `phase = "possible_to_started"|"started_to_peaked"|"end"` plus `pressure` as a `kv` float.
- **Docs:** yes — mandatory `/aif-docs` checkpoint at completion. T21 extends `2026-05-08-recognizer-extension.md` with two new worked examples (multi-pointer recognizer; pressure-based recognizer with `kind` guard), T22 sweeps rustdoc + ROADMAP + DESCRIPTION, T20 updates `gesture_arena_demo` where realistic.

## Roadmap Linkage

- **Milestone:** add a new entry `S07.6 GestureArena — recognizer roster expansion (MultiTap + ForcePress)` under Phase II (sibling to `S07.5 GestureArena T15 follow-up`, distinct from S07.7 resampling which stays unscheduled).
- **Rationale:** the source spec identifies two Flutter-parity recognisers v2 lacks. `MultiTapGestureRecognizer` covers accessibility (3-finger tap to invoke screen reader shortcuts) and macOS/iPad trackpad gestures. `ForcePressGestureRecognizer` covers iOS-class first-party experiences and unlocks the existing `PointerEvent.pressure` channel (already on the wire format, currently unused by any recogniser). Both fit cleanly on the `RecognizerLifecycle` seam introduced by S07.5 — the canonical extensibility recipe in `docs/superpowers/specs/2026-05-08-recognizer-extension.md` is the working artefact this plan exercises end-to-end for the first time. Landing both in one milestone keeps the lifecycle-trait extension (T2) and the `schedule_arena_release` generalization (T3) under a single architectural review pass.

## Goals

1. **Ship `MultiTapGestureRecognizer`** — N-finger simultaneous tap (configurable `required_pointer_count`, default 2). Multi-pointer arena coordination: each finger registers in its own per-pointer arena, the recognizer accumulates entry indices via the new `RecognizerLifecycle::set_arena_back_channel_for_pointer` hook (T2), and on the all-up transition declares itself winner of every arena it joined via the back-channel.
2. **Ship `ForcePressGestureRecognizer`** — pressure-based recogniser firing `on_force_press_start` / `on_force_press_update` / `on_force_press_end` with normalised pressure. Eager-accept on the `Possible → Started` transition (pressure crosses `force_press_start_pressure`). One-shot peak callback when pressure crosses `force_press_peak_pressure`. Activation requires `event.pressure.is_some()` (i.e. the device reports real pressure) plus the optional `AllowedButtonsFilter` predicate; mouse-class events that report `pressure: None` after S07.5b are filtered automatically without a hard-coded `kind` guard. Thresholds operate on `PressureSample::normalised()` so a Wacom pen's 0.4 and a Force Touch trackpad's 0.4 are semantically equivalent (both 40% of the device's reported range).
3. **Use the unified `RecognizerLifecycle::set_arena_back_channel(pointer_id, bc, entry_index)` hook** — multi-pointer recognisers (MultiTap) accumulate `(pointer_id, entry_index)` mappings in a `HashMap<PointerId, usize>` across the per-pointer registration calls. After S07.5b there is only one back-channel hook; LongPress (single-shot) and MultiTap (multi-pointer) consume the same shape. No multi-vs-single dual-hook contract.
4. **Generalize `GestureBinding::schedule_arena_release`** — accept a custom `Duration` (or add a sibling `schedule_arena_release_with` method) so MultiTap can schedule its `arena.release` based on `GestureSettings::multi_tap_window` (default 100 ms) instead of the DoubleTap timeout (default 300 ms). DoubleTap continues to use the existing call site with `settings.double_tap_timeout`.
5. **Expand `GestureSettings`** — three new `#[non_exhaustive]`-protected fields (`multi_tap_window: Duration`, `force_press_start_pressure: f32`, `force_press_peak_pressure: f32`) plus an optional `force_press_slop: Pixels` for slop-rejection on press-and-drag. Defaults match Flutter/v1 (100 ms, 0.4, 0.85, 18 px). Mutable via `window.gesture_settings_mut()` exactly like the existing thresholds; reach the recognisers through `RecognizerLifecycle::configure_settings`.
6. **Establish the multi-pointer recogniser pattern** — beyond MultiTap, future multi-pointer recognisers (rotation, pinch-extension, two-finger-pan with simultaneous tap-cancel) can copy the per-pointer entry-index accumulator pattern. T21 codifies the pattern in the contributor doc.

## Non-goals

- **Pointer resampling (S07.7).** Pre-arena pipeline phase that smooths input/display refresh-rate mismatch. Identified by the source spec as the biggest UX gap, but lands outside the recogniser layer (in `GestureBinding`'s pre-arena pipeline) and has its own bench-regression story. Separate plan after S07.6 lands.
- **Pointer prediction (S07.8).** Latency-reduction via velocity extrapolation. Source spec defers this until P1 (frame-budget instrumentation) lands. Not in this plan.
- **Real desktop touch / stylus support.** `PointerKind::Touch` and `PointerKind::Stylus` are reserved in the `PointerEvent` wire format but no current desktop platform emits them through the gesture pipeline. ForcePress is therefore architecturally complete but **dormant on desktop today** — it activates only when a future S20 desktop-gaps-cleanup spec wires macOS-trackpad pressure events into `PointerKind::Touch`. Documented as an explicit gap; not in this plan.
- **Force-press-on-mouse.** After S07.5b, mouse-class events report `pressure: None`. ForcePress's `event.pressure.is_some()` guard automatically rejects them — no hard-coded `kind` guard needed. A future Mouse-with-real-pressure path (some Wacom mice) lands without recogniser-side changes; users who explicitly want to allow it can pass an `AllowedButtonsFilter` that overrides.
- **Pressure normalisation across platforms.** S07.5b's `PressureSample { value, min, max }` carries the platform's raw range. The recogniser uses `PressureSample::normalised()` for threshold checks. Honest platform-side population of `min`/`max` is S20 territory; for S07.5b every desktop platform reports `Some(PressureSample { value: 1.0, min: 0.0, max: 1.0 })` for `Down` and `None` elsewhere — semantically identical to today, just in the new shape.
- **Multi-pointer drag / rotation / two-finger-pan.** This plan only delivers MultiTap (the simplest multi-pointer case). The `set_arena_back_channel_for_pointer` hook lays the groundwork for those future recognisers, but they are out of scope here.
- **Recognizer record/replay test harness.** Source spec lists this under "Medium-value additions". The existing `simulate_*` helpers are sufficient for S07.6's tests; the record/replay machinery only pays off once v2 has third-party recognisers in the wild. Future spec.
- **Public `Pinch` / `Rotation` recogniser surface.** PinchEvent already exists for the desktop platforms that emit it (macOS, Linux), but this plan does not introduce new arena-driven recognisers for them.

## Research Context

Inputs that informed this plan:

- **Source spec** (`docs/superpowers/specs/2026-05-08-gesture-roadmap-from-old-impl.md`) — the v1 → v2 comparative review. Identifies S07.6 as low-to-medium effort (the two recognisers fit cleanly on the existing `RecognizerLifecycle` seam) and assigns S07.7/S07.8 to separate, larger plans.
- **v1 reference impls** (`flui/crates/flui-interaction/src/recognizers/multi_tap.rs`, `force_press.rs`) — read for state-machine semantics. v1's Arc<Mutex>/Arc<Callback> patterns are explicitly **not** portable; v2 uses `Rc<RefCell>` and `Box<dyn FnMut>`. v1's hard-coded `max_time_window = 100ms` becomes a `GestureSettings::multi_tap_window` field. v1's `start_pressure = 0.4` / `peak_pressure = 0.85` defaults port directly.
- **v2 baseline** (`crates/flui-core/src/gesture/`) — read in full for S07.5 compliance. `RecognizerLifecycle` already supports `configure_settings` + `needs_back_channel` + `set_arena_back_channel(bc, idx)` + `needs_arena_hold`. The new `set_arena_back_channel_for_pointer(pid, bc, idx)` is purely additive (default no-op).
- **Recognizer-extension contributor doc** (`docs/superpowers/specs/2026-05-08-recognizer-extension.md`) — followed step-by-step for both new recognisers. Two of the document's invariants apply:
  - Threshold fields stay `pub` and are read from `GestureSettings` at construction; `configure_settings` re-reads them at registration to honour `window.gesture_settings_mut()` overrides.
  - Async timer paths use `BackgroundExecutor::timer`, never `smol::Timer::after` (so the test-scheduler's virtual clock wakes them).
- **paint → pending_recognizers chain** (S07.5 T15 wiring, in `Interactivity::paint` + `Window::dispatch_event`) — needs verification for multi-pointer reuse semantics (T4 below). One Box per element travels through `Interactivity::gesture_recognizers`; the open question is whether parking transfers ownership to **one** `pending_recognizers[hitbox_id]` slot (in which case multi-pointer arenas all reuse the same Box via `Rc::clone` on register) or replicates per-Down (in which case MultiTap can never aggregate state across pointers). T4 is explicitly the investigation step.

Pre-S07.6 state (from squash commits `1c63346f44` + #2 follow-up):

- Five recognisers landed (`Tap`, `DoubleTap`, `LongPress`, `Pan` / `HorizontalDrag` / `VerticalDrag` family, `Scale`).
- `RecognizerLifecycle` shipped with single-pointer back-channel only. No multi-pointer entry-index accumulator.
- `GestureSettings` carries Flutter-parity defaults for the existing recognisers; no fields for multi-tap / force-press yet.
- `GestureBinding::schedule_arena_release` is hard-wired to `settings.double_tap_timeout`.
- `PointerEvent.pressure` exists on the wire format and is populated by the dispatch layer. `MousePressureEvent` exists in the platform layer but does not flow into `PointerEvent.pressure` for non-Mouse events on any current platform.
- `__internal_on_*` builders cover Tap/DoubleTap/LongPress/Pan/HorizontalDrag/VerticalDrag/Scale. No multi-tap or force-press builders.

Open after S07.5 (this plan):

- `MultiTapGestureRecognizer` does not exist.
- `ForcePressGestureRecognizer` does not exist.
- `RecognizerLifecycle` has no multi-pointer hook.
- `GestureBinding::schedule_arena_release` is single-purpose.
- `GestureSettings` lacks `multi_tap_window`, `force_press_start_pressure`, `force_press_peak_pressure`, `force_press_slop`.
- `paint → pending_recognizers` flow for the same recogniser receiving N Downs in the same hitbox is undocumented and might or might not work; T4 verifies and adjusts.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Multi-pointer back-channel | Use the unified `RecognizerLifecycle::set_arena_back_channel(pointer_id, back_channel, entry_index)` (post-S07.5b — there is only one hook; the legacy single-pointer form was deleted). MultiTap implements it to accumulate `HashMap<PointerId, usize>` across the per-pointer registration calls. | S07.5b unified the hook signature so multi-pointer and single-pointer recognisers share one shape. LongPress stores a single-entry map; MultiTap stores N entries. No dual-hook contract. |
| Multi-pointer arena resolution | MultiTap eager-accepts via the back-channel **on the all-up transition**, declaring itself winner of every arena it joined. Sweep on individual arenas before the all-up transition is a no-op. | Multi-pointer recognisers must not let per-arena sweep declare them winners prematurely (e.g. on the first finger's Up before the second finger lifts). Eager-accept via back-channel is the only path that resolves all participating arenas atomically. |
| Multi-pointer arena hold | MultiTap returns `needs_arena_hold = true` for **every** arena it joins. The dispatcher honours this by calling `arena.hold(pointer_id)` after each registration. | Without hold, the first finger's Up would sweep arena[pid_1] and resolve against MultiTap before the second finger's events arrive. Hold defers per-arena sweep until the recogniser explicitly releases (via back-channel `declare_winner`) or the multi-tap-window timer fires `arena.release`. |
| Multi-tap timeout source | New `GestureSettings::multi_tap_window` (default 100 ms) drives the per-pointer `arena.release` timer through a generalised `GestureBinding::schedule_arena_release_with(pid, handle, cx, timeout)`. DoubleTap continues to use the existing `schedule_arena_release` call site (which now delegates to `schedule_arena_release_with(pid, handle, cx, settings.double_tap_timeout)`). | Avoids hard-coding two recognisers into one shared timer constant. Generalisation is mechanical (one new parameter) and lets future multi-pointer recognisers pick their own window. **Post-S07.5b note:** `hold_count: u32` makes co-existence of DoubleTap (300ms) and MultiTap (100ms) on the same pointer correct without a `min(timeouts)` workaround — each recogniser owns its own hold/release pair, both timers fire independently, sweep waits for `hold_count == 0`. |
| ForcePress activation guard | Activation requires `event.pressure.is_some()` (the device reports real pressure) and `allowed_buttons_filter.map_or(true, \|f\| f(buttons, modifiers))`. No hard-coded `kind != Mouse` check. Thresholds compare against `pressure.unwrap().normalised()`. | After S07.5b, mouse-class events report `pressure: None`, which the `is_some()` guard auto-rejects. Future Mouse-with-real-pressure paths (some Wacom hardware) work without recogniser changes. `AllowedButtonsFilter` lets users explicitly allow specific button combinations if they want. The `PressureSample::normalised()` call ensures `0.4` means the same thing on Wacom and Force Touch. |
| ForcePress state machine | `Idle → Possible → Started → Peaked → Ended`, eager-accept on `Possible → Started` (the press is committed once it crosses the start threshold). Peaked is a one-shot transition (the `peak_triggered` flag survives until `rejected`/`Up`/`Cancel`). | Matches Flutter and v1 semantics. Eager-accept on `Started` means a competing tap recogniser cannot win once force-press commits — desirable since force-press takes priority over single-tap on iOS. |
| ForcePress callback shape | Three external callbacks (`on_force_press_start`, `on_force_press_update`, `on_force_press_end`) plus a one-shot peak fired internally during `update`. v1 surfaces `on_peak` as a separate callback; we collapse it into `update` with a `is_peak: bool` field on `ForcePressDetails` to match the v2 callback austerity (one event = one callback type). | Reduces public surface from 4 callbacks to 3 without losing information. Downstream code that wants the peak-only path can filter `if details.is_peak { … }` inside `on_force_press_update`. Reversible — adding `on_force_press_peak` later is non-breaking via `#[non_exhaustive]`. |
| Recogniser file layout | One file per recogniser under `gesture/recognizers/` matching the existing convention. `mod.rs` re-exports both. `lib.rs` adds the canonical flat path (`flui_core::MultiTapGestureRecognizer`, `flui_core::ForcePressGestureRecognizer`). | No deviation from S07.5 conventions; nothing new to learn for readers. |
| Test discipline | Unit tests in each recogniser module (state-machine arms + threshold canary). Property tests for arena invariants only if MultiTap's hold/release symmetry adds new edge cases (likely yes — three-finger hold/release symmetry generalizes the S07.5 P-T15.5-C). End-to-end integration tests in `gesture_dispatch_integration.rs` for both, gated on `simulate_*` helpers. | Mirrors S07.5's three-tier test strategy. The integration tests are the critical regression lock — they exercise the full `paint → pending_recognizers → register_recognizer → arena.dispatch → callback` chain. |

## Cross-cutting Roadmap Interactions

| Cross-cutting | This plan's contract |
|---|---|
| **A3 — Error-type unification** | No new error types. Construction is infallible at the public API; multi-pointer registration is a binding-internal `bool` return today and does not surface errors. |
| **A4 — Tracing standardization** | Stay on `log` + `kv`. New events use the established `pointer_id` / `recognizer` / `phase` / `arena_state` / `lifecycle` schema. MultiTap adds `pointer_count` and `expected_pointers` `kv` fields; ForcePress adds `pressure` and `peak` fields. |
| **A5 — Feature flag matrix discipline** | No new feature combos. The `cargo hack check --feature-powerset --depth 2` smoke entry from S07.5 stays unchanged. |
| **A7 — Interior-mutability surface reduction** | New recognisers use the existing `Rc<RefCell<dyn Recognizer>>` pattern. No new interior-mutability surface introduced. |
| **A8 — `#[non_exhaustive]` audit** | New `MultiTapDetails` and `ForcePressDetails` carry `#[non_exhaustive]` from day one. New `GestureSettings` fields are added before the existing `#[non_exhaustive]` boundary so the type stays forward-compatible. |
| **T1 — Code coverage** | New unit + property + integration tests hit the new state machines through public APIs; `cargo-llvm-cov` picks them up without extra deps. |
| **T3 — Property-based tests** | One new property test set: `prop_multi_tap_n_pointer_hold_release_symmetry` (generalises S07.5 P-T15.5-C to N pointers). |
| **S08 — Semantics protocol** | Untouched — `GestureRecognizer::semantic_actions()` remains the seam. MultiTap returns `&[]` (no current Flutter semantic for "N-finger tap"; SemanticAction enum is `#[non_exhaustive]` so a future `MultiTap(usize)` variant lands in S08). ForcePress returns `&[]` for the same reason. |
| **S12 — Focus traversal** | Untouched — neither recogniser claims focus. `on_focus_request()` returns `None` for both. |
| **S14 — MediaQuery completeness** | New `GestureSettings` fields ride the existing `window.gesture_settings_mut()` seam. S14's MediaQuery-driven settings flow gains three more controllable thresholds for free. |
| **S20 — Desktop platform-gaps cleanup** | ForcePress is dormant on desktop until S20 wires macOS-trackpad pressure events through `PointerKind::Touch` (or until iOS S17 ships). T22 docs explicitly cross-reference S20 from `force_press.rs` rustdoc and the contributor doc. |

## Performance Budgets

The S07 bench (`cargo run -p flui-core --release --example gesture_arena_bench`) stays the contract:

| Sub-bench | Operation | Budget | S07.5 measured | Target after this PR |
|---|---|---|---|---|
| `hit_test_8deep` | Linear scan | < 2 µs | ~0 ns (optimizer-folded) | unchanged |
| `arena_tick` | VelocityTracker.add+estimate | < 1.25 µs | ~272 ns | unchanged |
| `full_frame_120hz` | Combined p99 | < 8 ms | ~1.6 µs | unchanged |

T2 (`schedule_arena_release` generalisation) is a parameter rename; same cold-path code. T6 (MultiTap recogniser) and T13 (ForcePress recogniser) add per-recogniser instances behind opt-in fluent builders; recognisers that aren't installed cost zero. T18 / T19 integration tests are CI-only. The lifecycle-hook plumbing was already paid for by S07.5b — this plan inherits the unified `set_arena_back_channel(pid, bc, idx)` shape.

The bench measures the dispatch hot path for the existing five recognisers; adding MultiTap / ForcePress to the bench fixture (T23 stretch) is non-blocking — they only run in apps that opt in.

## Explicit Gaps (still deferred after this PR)

- **Pointer resampling (S07.7).** The source spec's "biggest UX gap vs Flutter". Lands as a pre-arena pipeline phase in a separate plan.
- **Pointer prediction (S07.8).** Defers until P1 frame-budget instrumentation provides the substrate (per source spec).
- **Real desktop Touch / Stylus emission.** Platform-layer work, S20 / future spec.
- **`tracing` migration.** Pending A4.
- **`SemanticAction::MultiTap(usize)` and `SemanticAction::ForcePress` enum variants.** Pending S08.
- **Pinch rotation on desktop.** Pending platform-layer pinch-rotation emission (S20).
- **Public `GestureArenaTeam` registration on `InteractiveElement`.** Pending future `GestureDetector` widget.
- **Force-press peak as a separate callback.** Collapsed into `on_force_press_update` via `details.is_peak: bool` per the architectural decision above. Adding a separate `on_force_press_peak` later is non-breaking; deferred until a real downstream consumer asks.

## Tasks

### Phase A — Foundation (settings + timeout generalisation)

S07.5b already shipped the `RecognizerLifecycle` hook unification — there is no trait extension to do here. Phase A is just settings + the timeout-parameter generalisation.

- [ ] **T1:** Augment `GestureSettings` with four new fields:
  - `multi_tap_window: Duration` — max time window for all N pointers to land. Flutter / v1 default: 100 ms.
  - `force_press_start_pressure: f32` — *normalised* pressure threshold (relative to `PressureSample::normalised()`) at which `Possible → Started` fires. Flutter / v1 default: 0.4.
  - `force_press_peak_pressure: f32` — *normalised* pressure threshold for the one-shot peak event. Flutter / v1 default: 0.85.
  - `force_press_slop: Pixels` — max movement during force-press before slop-rejection. Default: 18 px (matches `touch_slop`).

  All four added before the `#[non_exhaustive]` boundary at the bottom of the struct so existing field order stays stable. Update the rustdoc constants block. Document explicitly that the two pressure thresholds are normalised values (compared against `PressureSample::normalised()`), not raw `value` — this is what makes them platform-agnostic. Lock with a `gesture_settings_default_values_match_flutter` test.

- [ ] **T2:** Generalize `GestureBinding::schedule_arena_release` to accept a custom `Duration`. Add a new method `schedule_arena_release_with(pointer_id, handle, cx, timeout)` and reduce the existing `schedule_arena_release(pid, handle, cx)` to a thin wrapper that calls `…_with(pid, handle, cx, settings.double_tap_timeout)`. MultiTap calls `…_with(pid, handle, cx, settings.multi_tap_window)`. The `arena_hold_timers: FxHashMap<PointerId, Task<()>>` storage stays unchanged; only the timer's wake duration is new. Update the binding's rustdoc to mention both timeouts.

> **Commit checkpoint A — after T1, T2:** `chore(flui-core): GestureSettings additions + schedule_arena_release timeout generalisation (S07.6 prep)`

### Phase B — `MultiTapGestureRecognizer`

- [ ] **T4:** **Investigation step.** Verify the `Interactivity::paint → pending_recognizers[hitbox_id] → Window::dispatch_event` chain correctly hands the **same recogniser instance** to N Downs landing on the same hitbox. The chain currently routes one `Box<dyn GestureRecognizer>` from `Interactivity::gesture_recognizers` through paint into `pending_recognizers[hitbox_id]`. Read `interactive.rs` / `dispatch.rs` / the relevant paint code; produce a one-paragraph finding documenting whether:

  - **Case A (works as-is):** paint parks once per hitbox, and dispatcher reuses the parked Box across multiple Downs in the same hitbox. MultiTap accumulates per-pointer state via `add_pointer(pid, event)` calls, one per Down.
  - **Case B (needs adjustment):** paint clones / consumes the Box per Down, fragmenting MultiTap's state across N instances. Adjustment options:
    1. Convert `Interactivity::gesture_recognizers` to `Vec<Rc<RefCell<Box<dyn GestureRecognizer>>>>` so paint can `Rc::clone` into `pending_recognizers` without copying state.
    2. Have paint park once per hitbox and dispatcher pull the parked Rc on each Down.

  If Case A applies — proceed to T5. If Case B applies — execute the chosen adjustment as a sub-task **T4.1** before T5, with its own arch-reviewer pass. Either outcome is acceptable; T4 forces the question and locks the decision.

- [ ] **T5:** Create `crates/flui-core/src/gesture/recognizers/multi_tap.rs`:

  ```rust
  #[non_exhaustive]
  pub struct MultiTapDetails {
      pub pointer_count: usize,
      pub positions: SmallVec<[Point<Pixels>; 4]>,
      pub center: Point<Pixels>,
      pub kind: PointerKind,
  }

  #[non_exhaustive]
  pub struct MultiTapGestureRecognizer {
      pub on_multi_tap: Option<Box<dyn FnMut(MultiTapDetails, &mut Window, &mut App)>>,
      pub on_multi_tap_cancel: Option<Box<dyn FnMut(&mut Window, &mut App)>>,
      pub required_pointer_count: usize,
      pub touch_slop: Pixels,
      pub multi_tap_window: Duration,
      pub button: PointerButtons,
      // private state:
      state: MultiTapState,                              // Idle | Collecting | WaitingForUp
      pointers: FxHashMap<PointerId, PointerInfo>,
      first_down_time: Option<Instant>,
      pointer_indexes: FxHashMap<PointerId, usize>,      // populated via set_arena_back_channel_for_pointer
      arena_back_channel: ArenaBackChannel,
      last_kind: PointerKind,
  }

  struct PointerInfo {
      initial_position: Point<Pixels>,
      current_position: Point<Pixels>,
      down_time: Instant,
      is_down: bool,
  }
  ```

  Construction: `Self::new(settings: &GestureSettings)` reads `touch_slop`, `multi_tap_window`. Default `required_pointer_count = 2`; override via `with_pointer_count(n)` builder (panics if `n < 2`).

- [ ] **T6:** Implement `GestureRecognizer` for `MultiTapGestureRecognizer`:
  - `add_pointer(pid, event)` — tracks the pointer if `state ∈ {Idle, Collecting}` and `event.buttons.contains(self.button)`. Updates `first_down_time` on the first pointer; rejects (state→Cancelled, fire `on_multi_tap_cancel`) if subsequent pointers arrive past `multi_tap_window`. Transitions to `WaitingForUp` once `pointers.len() == required_pointer_count`. If `pointers.len() > required_pointer_count` (extra finger), self-rejects via `on_multi_tap_cancel`.
  - `handle_event(event, window, cx)` — on `Move > slop` for any tracked pointer: rejects all. On `Up`: marks the pointer `is_down = false`. When all pointers have lifted in `WaitingForUp` state: computes `MultiTapDetails`, fires `on_multi_tap`, declares winner via back-channel for **every** `(pid, entry_index)` in `pointer_indexes`, returns `GestureDisposition::Accepted` for the current pointer's arena (the others resolve via the back-channel calls). On `Cancel` / `Removed` for any tracked pointer: full reset, fires `on_multi_tap_cancel`, returns `Rejected`.
  - `sweep_accepted(pid, window, cx)` — no-op. MultiTap's resolution path is the back-channel `declare_winner`; sweep means the arena resolved via a different path (e.g. the multi-tap window timer expired).
  - `rejected(pid, window, cx)` — full reset, fires `on_multi_tap_cancel` if state was past `Idle`.
  - `name()` — `"multi_tap"`. `as_any_mut()`, `lifecycle()` follow the canonical pattern.

- [ ] **T7:** Implement `RecognizerLifecycle` for `MultiTapGestureRecognizer`. After S07.5b, there is exactly one back-channel hook (three-arg form):
  - `needs_back_channel() → true`.
  - `needs_arena_hold() → true` — each pointer's arena holds. Post-S07.5b's `hold_count: u32` makes this compose correctly: MultiTap+DoubleTap on the same pointer would each increment, each release decrements; sweep waits for both.
  - `configure_settings(settings)` — copies `touch_slop` and `multi_tap_window` from `settings`.
  - `set_arena_back_channel(pid, bc, idx)` — inserts `(pid, idx)` into `pointer_indexes` (typically `HashMap<PointerId, usize>` or `SmallVec<[(PointerId, usize); 4]>` for inline storage of the common 2-4 finger case). Stores `bc` into `arena_back_channel` if not yet set; idempotent across pointers (all back-channels for the same arena resolve to the same `Rc`).
  - On the all-up transition, the recogniser iterates `pointer_indexes` and calls `arena_back_channel.declare_winner(pid, idx, ...)` for every `(pid, idx)` pair — atomically resolving every arena it joined.

- [ ] **T8:** Update `Window::dispatch_event` (or wherever the dispatcher schedules arena releases) to schedule a per-recogniser-per-pointer release timer when a recogniser's `lifecycle().needs_arena_hold()` returns true. **Post-S07.5b's `hold_count: u32`** makes the policy clean: each `needs_arena_hold` recogniser increments the counter via its own `arena.hold(pid)` and schedules its own release timer with its own timeout (DoubleTap → `double_tap_timeout`, MultiTap → `multi_tap_window`). When both fire, they decrement independently. Sweep waits for `hold_count == 0`. **Storage change to `GestureBinding::arena_hold_timers`:** key changes from `FxHashMap<PointerId, Task<()>>` to `FxHashMap<(PointerId, RecognizerKey), Task<()>>` where `RecognizerKey` is something like `(TypeId, *const dyn GestureRecognizer)` — needed so two recognisers' release timers don't trample each other on the same pid. Update the timer-storage rustdoc.

- [ ] **T9:** Add the fluent builder in `crates/flui-core/src/gesture/mod.rs`:

  ```rust
  #[doc(hidden)]
  pub fn __internal_on_multi_tap(
      iv: &mut crate::elements::Interactivity,
      pointer_count: usize,
      f: impl FnMut(recognizers::MultiTapDetails, &mut Window, &mut App) + 'static,
  ) {
      let r = find_or_push(__recognizers_mut(iv), || {
          recognizers::MultiTapGestureRecognizer::new(&GestureSettings::default())
              .with_pointer_count(pointer_count)
      });
      r.on_multi_tap = Some(Box::new(f));
      // The pointer_count override is sticky — the helper rebuilds
      // the recogniser if find_or_push produced a fresh instance.
  }
  ```

  Surface on `InteractiveElement` in `crates/flui-core/src/elements/div.rs` as `on_multi_tap(pointer_count: usize, f: …)`.

- [ ] **T10:** Re-export from `gesture/mod.rs` (`pub use recognizers::{MultiTapGestureRecognizer, MultiTapDetails};`) and from `lib.rs` for the canonical flat path.

- [ ] **T11:** Unit tests in `multi_tap.rs::tests`:
  - `multi_tap_two_finger_eagerly_accepts` — two `add_pointer` + two `Up` → `on_multi_tap` fires once, returns `Accepted`.
  - `multi_tap_three_finger_works_with_required_pointer_count_3` — recogniser configured for 3, three pointers, all up.
  - `multi_tap_too_many_pointers_cancels` — 3 pointers when `required_pointer_count = 2` → `on_multi_tap_cancel` fires, returns `Rejected`.
  - `multi_tap_slop_rejects` — first pointer moves past slop → all rejected, `on_multi_tap_cancel` fires.
  - `multi_tap_window_timeout_rejects` — second pointer arrives past `multi_tap_window` → reject, fresh sequence starts.
  - `multi_tap_threshold_fields_are_settable` — compile-time canary, identical pattern to `tap_threshold_fields_are_settable`.

> **Commit checkpoint B — after T4 (and T4.1 if needed) through T11:** `feat(flui-core): MultiTapGestureRecognizer with N-pointer arena coordination (S07.6)`

### Phase C — `ForcePressGestureRecognizer`

- [ ] **T12:** Create `crates/flui-core/src/gesture/recognizers/force_press.rs`:

  ```rust
  #[non_exhaustive]
  pub struct ForcePressDetails {
      pub global_position: Point<Pixels>,
      pub local_position: Point<Pixels>,
      pub pressure: f32,            // 0.0..=1.0 normalised
      pub kind: PointerKind,
      pub is_peak: bool,            // true on the one-shot peak event
  }

  #[non_exhaustive]
  pub struct ForcePressGestureRecognizer {
      pub on_force_press_start: Option<Box<dyn FnMut(ForcePressDetails, &mut Window, &mut App)>>,
      pub on_force_press_update: Option<Box<dyn FnMut(ForcePressDetails, &mut Window, &mut App)>>,
      pub on_force_press_end: Option<Box<dyn FnMut(ForcePressDetails, &mut Window, &mut App)>>,
      pub start_pressure: f32,
      pub peak_pressure: f32,
      pub slop: Pixels,
      pub button: PointerButtons,
      // private state:
      state: ForcePressState,             // Idle | Possible | Started | Peaked
      pointer: Option<PointerId>,
      down_position: Point<Pixels>,
      last_kind: PointerKind,
      peak_triggered: bool,
  }
  ```

  Construction: `Self::new(settings: &GestureSettings)` reads `force_press_start_pressure`, `force_press_peak_pressure`, `force_press_slop`.

- [ ] **T13:** Implement `GestureRecognizer` for `ForcePressGestureRecognizer`. Note `handle_event` signature post-S07.5b is `handle_event(event: DeliveredEvent<'_>, window, cx)`:
  - `add_pointer(pid, event: &PointerEvent)` — **rejects events with no real pressure** via `if event.pressure.is_none() { return; }`. After S07.5b, mouse-class events have `pressure: None` (except macOS Force Touch via Decision MM, which produces `Some(...)`), so this is the canonical mouse-rejection path; there is no hard `kind` check. The `allowed_buttons_filter` check is performed by `register_recognizer` BEFORE `add_pointer` (S07.5b D10) — so by the time `add_pointer` runs, the filter already passed. Check `event.buttons.contains(self.button)`. On pass: `state = Possible`, `pointer = Some(pid)`, `down_position = event.position` (window-local; we don't have a `DeliveredEvent` here yet — `add_pointer` runs at registration, `local_position` only enters via `handle_event`), `last_kind = event.kind`, `peak_triggered = false`.
  - `handle_event(event: DeliveredEvent<'_>, window, cx)`:
    - All slop checks use `event.local_position` (post-S07.5b convention).
    - All pressure checks use `event.event.pressure.map(|p| p.normalize()).unwrap_or(0.0)` — a `None` mid-stream pressure is treated as zero (gesture should reject; can happen on weird platform-layer transitions). Note: `event.event.pressure` reads through the `DeliveredEvent` wrapper; `event.local_position` is the per-recogniser local coordinate.
    - `Move > slop`: state→Idle, return `Rejected`.
    - `Move` (within slop) — branch on state:
      - `Possible`: if `normalised_pressure >= start_pressure` → state→`Started`, fire `on_force_press_start`, return **`Accepted` (eager-accept)**. Otherwise stay `Possible`, return `Possible`.
      - `Started`: if `!peak_triggered && normalised_pressure >= peak_pressure` → set `peak_triggered = true`, fire `on_force_press_update` with `is_peak = true`. Always fire `on_force_press_update` (the update event is the catch-all). If pressure drops below `start_pressure` post-peak (release-without-up): state→Idle, fire `on_force_press_end`, return `Rejected`. Otherwise stay `Started`/`Peaked`, return `Possible` (we already eager-accepted earlier).
    - `Up`: if state ≥ `Started`: fire `on_force_press_end`, state→Idle, return `Accepted`. If state == `Possible` (pressure never crossed start): state→Idle, return `Rejected`.
    - `Cancel` / `Removed`: if state ≥ `Started`: fire `on_force_press_end` for tear-down symmetry, state→Idle, return `Rejected`.
    - `Down`: ignore (state is already set by `add_pointer`).
  - `sweep_accepted(pid, window, cx)` — no-op. ForcePress wins via eager-accept; sweep means the recogniser was the last competitor without committing, which is a no-fire path.
  - `rejected(pid, window, cx)` — fires `on_force_press_end` if state was past `Possible` (to honour the symmetry contract: every `start` callback gets paired with an `end`). State→Idle, `peak_triggered = false`.
  - `name()` — `"force_press"`. `as_any_mut()`, `lifecycle()` follow the canonical pattern.

- [ ] **T14:** Implement `RecognizerLifecycle` for `ForcePressGestureRecognizer`:
  - No back-channel needed — `needs_back_channel() → false` (defaults to `false`, nothing to override).
  - No arena hold — `needs_arena_hold() → false` (defaults to `false`).
  - `configure_settings(settings)` — copies `start_pressure`, `peak_pressure`, `slop` from `settings`.

- [ ] **T15:** Add the fluent builders in `crates/flui-core/src/gesture/mod.rs`: `__internal_on_force_press_start`, `__internal_on_force_press_update`, `__internal_on_force_press_end`. Pattern matches `__internal_on_long_press_*`. Surface on `InteractiveElement` as `on_force_press_start(f)`, `on_force_press_update(f)`, `on_force_press_end(f)`.

- [ ] **T16:** Re-export from `gesture/mod.rs` (`pub use recognizers::{ForcePressGestureRecognizer, ForcePressDetails};`) and from `lib.rs` for the canonical flat path.

- [ ] **T17:** Unit tests in `force_press.rs::tests`:
  - `force_press_no_pressure_event_is_rejected` — `add_pointer` with `pressure: None` (mouse-class) → state stays `Idle`, `pointer = None`.
  - `force_press_with_pressure_sample_pressure_below_start_stays_possible` — `Down` with `pressure: Some(PressureSample { value: 0.3, min: 0.0, max: 1.0 })` (normalised 0.3 < 0.4 default) → state stays `Possible`, no callback fires.
  - `force_press_with_pressure_sample_pressure_crosses_start_eager_accepts` — `Down` then `Move` with `pressure: Some(PressureSample { value: 0.5, ... })` → `on_force_press_start` fires, returns `Accepted`.
  - `force_press_normalisation_is_platform_agnostic` — same gesture against Wacom-range (`min: 0, max: 8192, value: 3500`) and Force-Touch-range (`min: 0, max: 1, value: 0.42`) both fire `on_force_press_start` (both normalise above 0.4). Locks the platform-agnostic threshold semantics.
  - `force_press_peak_one_shot` — `pressure` rising past 0.85 normalised → `on_force_press_update` fires once with `is_peak = true`, then again with `is_peak = false` for further updates.
  - `force_press_up_after_start_fires_end` — start, then `Up` → `on_force_press_end` fires, returns `Accepted`.
  - `force_press_slop_rejects` — start, then `Move > slop` → `Rejected`, `on_force_press_end` fires for symmetry.
  - `force_press_allowed_buttons_filter_overrides_pressure_check` — `pressure: Some(...)` event but custom `allowed_buttons_filter` returns `false` → recogniser rejects (`Possible` never entered).
  - `force_press_threshold_fields_are_settable` — compile-time canary.

> **Commit checkpoint C — after T12 through T17:** `feat(flui-core): ForcePressGestureRecognizer with kind-guarded eager-accept (S07.6)`

### Phase D — Integration tests, demo, docs, bench

- [ ] **T18:** End-to-end integration test `gesture_dispatch_integration::multi_tap_two_finger_through_dispatch`. Paint `div().on_multi_tap(2, callback)`. Drive synthetic Touch `Down` (pid_1) + `Down` (pid_2) + `Up` (pid_1) + `Up` (pid_2) through `Window::dispatch_event` via a new `simulate_multi_finger_tap` helper in `test-support`. Assert `on_multi_tap` fires exactly once with `pointer_count = 2`. Cover the `simulate_*` gap if needed — most likely a thin wrapper around `simulate_*` with explicit `PointerId` allocation. **Cfg-gated** on `test-support` so workspace builds without it stay clean.

- [ ] **T19:** End-to-end integration test `gesture_dispatch_integration::force_press_touch_through_dispatch`. Paint `div().on_force_press_start(...).on_force_press_update(...).on_force_press_end(...)`. Drive synthetic Touch events with `pressure: Some(PressureSample { value, min: 0, max: 1 })` carrying the crescendo: `Down(0.0)` + `Move(0.3)` + `Move(0.5)` (crosses start) + `Move(0.9)` (crosses peak) + `Up(0.0)`. Assert `start` fires once, `update` fires twice with `is_peak=true` then `is_peak=false`, `end` fires once. Also cover the negative case: a `Mouse Down` with `pressure: None` must not fire any callback (the `is_some()` guard rejects it). Cover one more negative: a `Touch Down` with `pressure: Some(...)` but custom `allowed_buttons_filter` returning `false` must not fire either.

- [ ] **T20:** Update `examples/gesture_arena_demo` (or whatever the demo name is) with optional MultiTap + ForcePress scenarios. **Conditional:** if no Touch device is available on the runtime platform, the scenario displays a "requires Touch input — currently dormant on desktop" hint and continues. The demo should not panic or error out; it's a demonstration, not a CI gate. Skip-without-failing if running on a desktop Mouse-only path.

- [ ] **T21:** Extend `docs/superpowers/specs/2026-05-08-recognizer-extension.md` with two new worked examples:
  - "MultiTap (multi-pointer — accumulator pattern)": shows `set_arena_back_channel_for_pointer` usage, the `pointer_indexes` HashMap, and the all-up resolution flow.
  - "ForcePress (pressure + kind guard)": shows the `kind != Mouse` filter, the `Possible → Started` eager-accept transition, and the pressure-driven peak one-shot.
  - Add a third row to the existing "When to use `RecognizerLifecycle`" table for `set_arena_back_channel_for_pointer`.
  - Add a "Threshold-field conventions" note specific to ForcePress: pressure thresholds clamp to `[0.0, 1.0]` post-construction; out-of-range values silently saturate.

- [ ] **T22:** Update rustdoc + ROADMAP + DESCRIPTION:
  - Module-level rustdoc in `gesture/mod.rs` gets a "S07.6 — completed" subsection listing both new recognisers, their builder methods, and the platform-support caveat for ForcePress.
  - `recognizer.rs` documents the new `set_arena_back_channel_for_pointer` hook.
  - `binding.rs` documents `schedule_arena_release_with`.
  - `gesture_settings.rs` documents the four new fields.
  - `.ai-factory/ROADMAP.md` adds `S07.6 GestureArena — recognizer roster expansion (MultiTap + ForcePress)` under Phase II + Completed table on merge.
  - `DESCRIPTION.md` Input pipeline bullet enumerates `MultiTap` and `ForcePress` as additions, with the platform-support caveat.

- [ ] **T23:** Bench regression verification. Run `cargo run -p flui-core --release --example gesture_arena_bench` after Phase C lands. All three sub-bench budgets must still pass: `hit_test_8deep < 2 µs`, `arena_tick < 1.25 µs`, `full_frame_120hz < 8 ms p99`. T2 (lifecycle hook) and T3 (timeout generalisation) are cold-path additions; they should not affect the hot path. T6/T13 recognisers cost zero when not installed. If any threshold regresses, isolate the source via git bisect within Phase B/C commits before continuing. **Stretch (non-blocking):** add `multi_tap_2finger_dispatch` and `force_press_touch_dispatch` micro-benches to the bench fixture; lock budgets at the same `< 8 ms p99` overall.

> **Commit checkpoint D — after T18 through T23:** `test(flui-core): S07.6 integration tests + multi-pointer property locks + bench verification + demo + extensibility doc + roadmap update`

## Commit Plan

| Checkpoint | After tasks | Suggested message |
|---|---|---|
| A | T1, T2 | `chore(flui-core): GestureSettings additions + schedule_arena_release timeout generalisation (S07.6 prep)` |
| B | T4 (+T4.1 if needed), T5–T11 | `feat(flui-core): MultiTapGestureRecognizer with N-pointer arena coordination (S07.6)` |
| C | T12–T17 | `feat(flui-core): ForcePressGestureRecognizer with kind-guarded eager-accept (S07.6)` |
| D | T18–T23 | `test(flui-core): S07.6 integration tests + multi-pointer property locks + bench verification + demo + extensibility doc + roadmap update` |

## Review Subagents

Per `.ai-factory/rules/base.md`, invoke proactively:

- **`flui-arch-reviewer`** — on the design decisions in this plan **before T2 lands** (the `schedule_arena_release_with` generalisation is a long-lived seam), again **after T4 / T4.1** (resolution of the multi-pointer paint/dispatch question is architectural), and **after T8** (multi-pointer hold-timeout resolution policy `min(double_tap_timeout, multi_tap_window)` deserves a sanity check).
- **`rust-api-migration-auditor`** — on **T1** (new public `GestureSettings` fields → semver-relevant), **T9** (fluent builder accepts a `usize` parameter — semver shape lock), **T10/T16** (new public `pub use` re-exports — explicit exports list maintenance per A1/A2 hygiene).
- **`migration-risk-adversary`** — on **T4.1** (if Case B applies — `Interactivity::gesture_recognizers` shape change has wide blast radius), **T8** (touches the dispatcher's hold/release scheduling — the same code path S07.5 T6 was adversarial about). Skip on T2 alone since it's additive.
- **`wgpu-gpu-reviewer`** — not applicable.

## Acceptance Criteria

1. **`MultiTapGestureRecognizer` ships.** Public type re-exported via `flui_core::MultiTapGestureRecognizer`. Fluent builder `on_multi_tap(pointer_count, callback)` on `InteractiveElement`. End-to-end integration test (T18) covers the 2-finger case through `Window::dispatch_event` and is in CI. Three-finger case covered by unit test.
2. **`ForcePressGestureRecognizer` ships.** Public type re-exported via `flui_core::ForcePressGestureRecognizer`. Three fluent builders (`on_force_press_start` / `_update` / `_end`) on `InteractiveElement`. End-to-end integration test (T19) covers the Touch crescendo path. Mouse-class events with `pressure: None` are auto-rejected via `is_some()` guard; rustdoc cross-references the audit's pressure-truth rationale. Thresholds operate on `PressureSample::normalised()` so Wacom and Force Touch behave equivalently.
3. **Lifecycle hook unchanged from S07.5b.** The `RecognizerLifecycle::set_arena_back_channel(pid, bc, idx)` shape (already unified by S07.5b) is the only back-channel hook; MultiTap consumes it as-is. No new trait methods.
4. **`GestureSettings` adds four new fields.** `multi_tap_window`, `force_press_start_pressure`, `force_press_peak_pressure`, `force_press_slop`. All Flutter-parity defaults. `#[non_exhaustive]` boundary preserved.
5. **`GestureBinding::schedule_arena_release_with` exists.** Old `schedule_arena_release` is a thin wrapper; both DoubleTap and MultiTap drive their respective timeouts through the new method.
6. **Property test for N-pointer hold/release symmetry exists** (extends S07.5 P-T15.5-C). Locks the invariant that every `arena.hold(pid)` for a multi-pointer recogniser is balanced by exactly one `arena.release(pid)` (either explicit or via `cancel`).
7. **Performance budgets unchanged.** Bench verification (T23) confirms `hit_test_8deep`, `arena_tick`, `full_frame_120hz` budgets all hold.
8. **Public-API stability.** No breaking change to existing `flui_core` public exports. Three new types added (`MultiTapGestureRecognizer`, `MultiTapDetails`, `ForcePressGestureRecognizer`, `ForcePressDetails`) — purely additive.
9. **Contributor doc T21 done.** `2026-05-08-recognizer-extension.md` carries two new worked examples and a third row in the lifecycle-method table.
10. **Logging discipline.** New `kv` events in MultiTap and ForcePress follow the existing schema (`pointer_id`, `recognizer`, `phase`, `lifecycle`) plus recogniser-specific fields (`pointer_count`, `expected_pointers`, `pressure`, `peak`).

## Risks

- **T4 outcome unknown ahead of investigation.** If the `paint → pending_recognizers` chain fragments per Down (Case B), Phase B grows by one task (T4.1 — convert `Interactivity::gesture_recognizers` to `Vec<Rc<RefCell<...>>>`). That's a workspace-wide change with downstream blast radius (every recogniser-touching test).
  Mitigation: T4 happens **before** any user-visible MultiTap code. Bail out and rescope as needed before commit checkpoint B.

- **Multi-pointer hold under DoubleTap + MultiTap on the same element.** Post-S07.5b's `hold_count: u32` makes this composable correctly — each recogniser owns its own hold/release pair, both timers fire independently, sweep waits for `hold_count == 0`. T8's storage change (`(PointerId, RecognizerKey)` keyed timer map) is the implementation detail that lets two recognisers schedule independent release timers on the same pointer.
  Mitigation: T11 includes a unit test exercising both recognisers on the same element with different timeouts (DoubleTap 300ms, MultiTap 100ms), confirming MultiTap's release fires first without disturbing DoubleTap's pending hold.

- **ForcePress dormant on desktop today.** After S07.5b, mouse-class events report `pressure: None`, so ForcePress's `is_some()` guard auto-rejects them. Until S20 wires real pressure values from macOS trackpad / iOS / Android, the recogniser is functionally idle on desktop. Users wiring `on_force_press_*` may expect immediate feedback and not get any.
  Mitigation: T22 rustdoc on `ForcePressGestureRecognizer` opens with **"Currently active only on platforms emitting `pressure: Some(PressureSample)` from real pressure-sensing hardware (macOS Force Touch trackpad after S20; iOS/Android after their respective platform spec lands). Dormant on desktop today; mouse-class events report `None`."** The contributor doc and DESCRIPTION mirror this. Discoverable upfront; not a silent footgun.

- **`AllowedButtonsFilter` overrides may be misused** to allow ForcePress to fire on mouse-class events with `pressure: None`. The recogniser still requires `pressure.is_some()` regardless of the filter — the filter only narrows, never widens. T13 makes the order explicit: `is_some()` check first, filter second.
  Mitigation: T13 rustdoc explicitly: "the `is_some()` guard is non-negotiable; the filter only further restricts."

- **Pressure threshold semantics depend on S07.5b having shipped honest `min`/`max`.** If the platform layer reports `Some(PressureSample { value: 1.0, min: 0.0, max: 1.0 })` for every event regardless of device, normalisation is a no-op and Wacom/Force Touch parity is illusory. This is S20's job to fix.
  Mitigation: T22 docs note the dependency. T17 unit tests verify the recogniser-side math is correct; honest platform-side population is verified separately when S20 lands.

- **Test-helper gap for `simulate_multi_finger_tap`.** If `test-support` does not currently expose multi-pointer event injection, T18 grows by a sub-task to add it. Any change to `test-support` is a workspace-wide concern (S07.5 T1 split it into `test-support` + `test-support-with-platform`).
  Mitigation: scope the helper as **synthetic events only** — explicit `PointerId` allocation + no platform-layer plumbing. Stays inside the pure `test-support` feature, no `test-support-with-platform` regression.

- **Bench regression risk from per-pointer arena-hold timers under MultiTap.** A 3-finger MultiTap on a 60Hz frame creates 3 `Task<()>` allocations per gesture (one per pointer). For non-test workloads this is fine; for stress tests of N-finger gestures the allocation count grows linearly with N.
  Mitigation: T23 bench verification flags this if it shows up. If a real workload demonstrates allocation pressure, a future P-track perf milestone can introduce a `SmallVec`-backed timer pool — out of scope here.

- **Recogniser-extension contributor doc drift.** Adding two more worked examples + one more lifecycle hook stretches the doc; the existing structure may not absorb the additions cleanly.
  Mitigation: T21 explicitly restructures (not just appends). Reviewed alongside the code via `flui-arch-reviewer`.
