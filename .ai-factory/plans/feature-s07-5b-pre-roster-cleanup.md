# Plan: S07.5b GestureArena — pre-roster cleanup (breaking changes before S07.6)

- **Branch:** `feature/s07-5b-pre-roster-cleanup`
- **Created:** 2026-05-08
- **Mode:** full
- **Predecessor PRs:** #1 (S07 GestureArena) and #2 (S07.5 follow-up).
- **Source design rationale:** `docs/superpowers/specs/2026-05-08-flutter-gestures-architectural-audit.md` — the Flutter gestures architectural audit; this plan executes its breaking-friendly proposals **#1, #2, #3, #4, #5, #6, #7, #8, #9** as one consolidated cleanup before S07.6.
- **Working set:** the gesture-layer surface still has nine breaking-friendly mistakes. We have not published to crates.io, so backwards compatibility is hypothetical. Fixing them now costs one PR; fixing them after S07.6 ships ForcePress with hardcoded thresholds and after S09 lands `Transform` widgets costs many PRs and one user-visible regression each.

This plan is **prerequisite** for `feature-s07-6-recognizer-roster.md`. S07.6 consumes the renamed lifecycle hook (single-pointer hook deleted, only per-pointer hook ships), the new `Option<PressureSample>` pressure surface (replaces the `kind != Mouse` guard for ForcePress with proper `Some`-checking + AllowedButtonsFilter), and the new `AllowedButtonsFilter` opt-in. Land S07.5b first, then S07.6.

## Settings

- **Testing:** yes — every surface change ships with a regression-lock test. Several phases include compile-time canaries that the public API stays the intended shape (the `*_threshold_fields_are_settable` pattern from S07.5 generalised).
- **Logging:** verbose — `log` + `kv_unstable_serde`. New `kv` fields: `pressure_min`, `pressure_max`, `pressure_value`, `synthesized`, `transform_present` for hit-test path. Existing `pointer_id` / `recognizer` / `phase` / `arena_state` / `lifecycle` schema preserved.
- **Docs:** yes — mandatory `/aif-docs` checkpoint at completion. T20 sweeps rustdoc on every type that grew or changed shape; T21 updates `docs/superpowers/specs/2026-05-08-recognizer-extension.md` because `RecognizerLifecycle::set_arena_back_channel` is gone and `set_arena_back_channel_for_pointer` is the only hook; T22 logs the breaking change list in `CHANGELOG.md` (or sets up `CHANGELOG.md` if absent — addresses R3 prematurely-but-cheaply).

## Roadmap Linkage

- **Milestone:** new entry `S07.5b GestureArena — pre-roster cleanup (breaking changes)` under Phase II, between `S07.5 GestureArena T15 follow-up` and the planned `S07.6 GestureArena recognizer roster expansion`.
- **Rationale:** the audit at `docs/superpowers/specs/2026-05-08-flutter-gestures-architectural-audit.md` identified nine breaking-friendly proposals (§5 of that document) which are cheap to do now and expensive to do later. Three of them (#1 `pressure: Option<PressureSample>`, #4 lifecycle-hook deletion, #9 `AllowedButtonsFilter`) directly enable a cleaner S07.6 surface — without them, S07.6 will hardcode `0.4` against a normalised-on-platform pressure, ship a doomed legacy single-pointer hook, and rely on `kind != Mouse` as a force-press gate (which falsely rejects future trackpad/stylus pressure paths). Six more (#2 `PointerKind` extras, #3 `PointerPhase::PanZoom*`, #5 transform-stack hit-test, #6 `GestureDisposition` `#[non_exhaustive]`, #7 `synthesized` flag, #8 `source_timestamp` split) prepare the substrate for S07.7 resampling, S07.9 multi-drag, S08 semantics, S09 layers, and S11 scroll-physics. Doing them in one sweep gives reviewers a single architectural pass instead of nine drips.

## Goals

1. **Faithful pressure semantics.** Replace `pressure: f32` (normalised on platform side, lossy) with `pressure: Option<PressureSample>` carrying `value: f32` plus the platform's `min` / `max` raw range. Mouse-class events return `None` (no real pressure information). Stylus and Force Touch return `Some` with their device range. Recognizers normalise inside their own threshold checks.
2. **Complete `PointerKind` surface.** Add `Trackpad`, `InvertedStylus`, `Unknown` variants. `Trackpad` is the kind for the synthetic device emitting pan-zoom events on macOS; `InvertedStylus` covers eraser-side pen flips; `Unknown` is the platform-doesn't-tell-us escape hatch (today implicitly mapped to `Mouse`, which is a lie).
3. **Complete `PointerPhase` surface.** Add `PanZoomStart`, `PanZoomUpdate`, `PanZoomEnd` for native macOS trackpad gestures. They are distinct from `Magnify` (which is scalar) — pan-zoom carries pan + zoom + rotation tuples. `ScaleGestureRecognizer` consumes them on platforms that emit them.
4. **Single lifecycle hook for back-channel.** Remove `RecognizerLifecycle::set_arena_back_channel(bc, idx)` entirely. The only hook is `set_arena_back_channel_for_pointer(pid, bc, idx)`. `LongPressGestureRecognizer` migrates to the per-pointer hook (the pointer_id is just stashed alongside the entry index in a `HashMap` of size 1 — no behaviour change). This removes the dual-hook contract S07.6 originally proposed and makes the multi-pointer pattern the only pattern.
5. **Transform-stack hit-test substrate.** Extend `HitTestEntry` with `transform: Option<Affine2>` and `local_position: Point<Pixels>`. `HitTestResult` gains a transform stack (push/pop) so `Window::hit_test` walks transformed children correctly. Recognizers begin reading `entry.local_position` for slop and distance checks instead of `event.position` — which is global. **The single most important fix in this plan.** Without it, `Transform` widgets in S09 will silently break drag/tap/long-press recognition on rotated subtrees.
6. **`GestureDisposition` `#[non_exhaustive]`.** Costs zero today; opens the door to observability variants (`AcceptedDeferred`, `RejectedBySweep`) without a future breaking change.
7. **`PointerEvent.synthesized: bool` field.** S07.7 resampling will emit synthesised intermediate `Move` events, and S08 semantics will synthesise `Down` + `Up` pairs from `SemanticAction`. Both need debug-distinguishability and a recognizer-side filter knob.
8. **Split `timestamp` into `timestamp` + `source_timestamp`.** Today `timestamp` is "platform produced this event"; resampler will need both "we emitted at sample boundary" (`timestamp`) and "platform original event time" (`source_timestamp`). For non-synthesised events, the two are identical.
9. **`AllowedButtonsFilter` opt-in.** Augment the simple `pub button: PointerButtons` field with an optional closure-based filter `Option<Box<dyn Fn(PointerButtons, Modifiers) -> bool>>`. Required by ForcePress (rejects mouse-class events except where the user explicitly allows them) and by future middle-click-pan / shift-drag interactions. Don't replace the simple field — augment it; the closure runs only when set.

## Non-goals

- **Recommendations #10-#14 from the audit** (`DragStartBehavior`, `Drag` trait stub, `MultiDragPointerState` stub, public `GestureArenaTeam`, `LongPressDetails.velocity`, secondary/tertiary long-press, `SemanticAction` extension). These ship with their relevant milestones (S07.9, S11, S13, S08), not here. Including them would balloon S07.5b past one PR's worth of review.
- **Helper-trait extraction (`PrimaryPointerState`).** Audit §3.1 recommends extracting the primary-pointer slop/timer logic to deduplicate Tap/LongPress/ForcePress. That's worth doing — but it's an internal refactor without surface-level breaking changes, so it can land in S07.6 as part of MultiTap/ForcePress implementation when the duplication actually shows up.
- **Per-platform velocity trackers** (audit §3.9). S11 territory; not driven by S07.5b.
- **Pressure-range platform plumbing.** This plan changes the `PointerEvent` shape to carry `Option<PressureSample>`; the platform layer's job to populate `min`/`max` honestly is **future** S20 work (see Risks). For S07.5b, every desktop platform emits `Some(PressureSample { value: 1.0, min: 0.0, max: 1.0 })` for `Down` and `None` elsewhere — semantically equivalent to today but in the new shape.
- **Public `Affine2` / `Mat4` design for transforms.** T9 introduces *some* affine-transform primitive (`euclid::Transform2D<f32, _, _>` is the leading candidate; workspace already depends on `euclid`). Picking the exact type is a sub-decision; the plan locks the *contract* (`HitTestEntry.transform: Option<Affine2>` where `Affine2` is whatever we pick), not the implementation. `flui-arch-reviewer` weighs in on T9.
- **Hit-test transform actually populated by paint.** Paint-side transforms get populated when S09 lands. For S07.5b: `transform = None` and `local_position = position` always (until S09 layers wire actual transforms). This makes the breaking surface change cheap and lets recognisers begin reading `local_position` immediately.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Pressure shape | `Option<PressureSample { value: f32, min: f32, max: f32 }>` | Carries platform range; mouse returns `None`. Audit §3.13. Recognisers normalise via `(value - min) / (max - min)`. Wacom 8192-level pen and Force Touch trackpad become semantically distinguishable. |
| `PointerKind` extras | Add `Trackpad`, `InvertedStylus`, `Unknown` (4 → 7 variants total) | Audit §3.5 + §5#2. `#[non_exhaustive]` already on the enum so adding variants is mechanical. `Trackpad` distinct from `Mouse` because pressure/wheel semantics differ. |
| `PointerPhase` extras | Add `PanZoomStart`, `PanZoomUpdate`, `PanZoomEnd` (10 → 13 variants total) | Audit §3.5 + §5#3. Inline with existing `#[non_exhaustive]`. macOS native pinch-pan-rotate is the immediate driver. |
| Lifecycle hook unification | Delete `set_arena_back_channel(bc, idx)`. Keep only `set_arena_back_channel_for_pointer(pid, bc, idx)`. | Audit §5#4. The single-pointer hook is dead weight that S07.6 was going to keep alive only for backwards compat we don't owe anyone. LongPress carries a single-entry `HashMap<PointerId, usize>` after the migration — trivially small. |
| Hit-test transform | `HitTestEntry { transform: Option<Affine2>, local_position: Point<Pixels>, ... }` plus a `HitTestResult::push_transform(Affine2)` / `push_offset(Point<Pixels>)` / `pop_transform()` builder API used by `Window::hit_test`. | Audit §3.4 + §5#5. The single biggest hidden gap: today's flat `position` breaks under rotation. We add the field now, populate it `None` until S09 wires real transforms, and migrate recognisers to read `local_position` so the migration path lands continuously. |
| Affine2 primitive | `euclid::Transform2D<f32, WindowSpace, LocalSpace>` (workspace already depends on `euclid`) | T9 introduces a thin alias. Open question if a custom `Affine2 { matrix: [[f32; 3]; 3] }` is preferable for cache layout — the architectural decision is "pick a 2D affine", not "pick one specific impl"; `flui-arch-reviewer` decides. |
| GestureDisposition non_exhaustive | `#[non_exhaustive]` | Audit §5#6. Future variants (`AcceptedDeferred` for captain-deferred, `RejectedBySweep` for tracing) become non-breaking additions. Cost zero today. |
| Synthesized field | `PointerEvent.synthesized: bool` (default `false`) | Audit §5#7. Resampler (S07.7) marks emitted intermediate events `true`; semantics (S08) marks synthesised `Down`/`Up` pairs `true`. Recognisers that want to skip synthesised events check the flag. |
| Timestamp split | `PointerEvent { timestamp: Instant, source_timestamp: Instant, ... }` | Audit §5#8. For non-synthesised: `source_timestamp == timestamp` (populated on construction). Resampler sets `timestamp = sample_boundary_time` and `source_timestamp = nearest_sample.timestamp`. VelocityTracker uses `source_timestamp` for its LSQ samples. |
| AllowedButtonsFilter shape | Opt-in `pub allowed_buttons_filter: Option<Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>>` field on each recogniser, alongside the existing simple `pub button: PointerButtons`. The simple field is the default fast-path; the closure overrides when set. | Audit §3.8 + §5#9. ForcePress needs to reject mouse-class events without a hard `kind != Mouse` guard (which would block future Mouse-class pressure paths). Middle-click-pan also wants this. |
| Existing-recogniser migration | Tap, DoubleTap, LongPress, Pan/HorizontalDrag/VerticalDrag, Scale all gain `allowed_buttons_filter: Option<...>` field, use `entry.local_position` instead of `event.position` for slop/distance, and (LongPress only) migrate to the per-pointer back-channel hook. No behavioural change for existing call sites. | The migration is mechanical; the test suite catches regressions. |
| Test-support helpers | Add `PointerEventBuilder::with_pressure_sample(min, max, value)` and `with_synthesized(bool)` for synthetic event construction; bump existing helpers to populate `source_timestamp == timestamp` by default. | Existing tests construct `PointerEvent` directly via the builder; the new fields need ergonomic synthetic construction. |

## Cross-cutting Roadmap Interactions

| Cross-cutting | This plan's contract |
|---|---|
| **A2 — Audit remaining ~29 globs in `flui-core/src/lib.rs`** | New types (`PressureSample`, `Affine2`-alias) re-exported per the explicit list pattern. No new globs. |
| **A3 — Error-type unification** | No new error types. `Option<PressureSample>` makes "no pressure available" a value, not an error. |
| **A4 — Tracing standardization** | Stay on `log` + `kv`. New `kv` fields `pressure_min` / `pressure_max` / `pressure_value` (numeric) and `synthesized` (bool) on per-event log lines where relevant. |
| **A5 — Feature flag matrix discipline** | No new feature combos. |
| **A7 — Interior-mutability surface reduction** | `allowed_buttons_filter: Option<Box<dyn Fn>>` adds no interior mutability — `Fn` (not `FnMut`) is the chosen bound; the closure is read-only. |
| **A8 — `#[non_exhaustive]` audit** | `GestureDisposition` gains `#[non_exhaustive]`. `PressureSample` carries it from day one. New `PointerKind` and `PointerPhase` variants ride existing `#[non_exhaustive]`. |
| **R3 — CHANGELOG.md** | T22 introduces a `CHANGELOG.md` — addresses R3 prematurely-but-cheaply, since this is the first plan with breaking changes worth documenting up-front. |
| **S07.6** | This plan is its prerequisite. S07.6 plan's "T2 — extend `RecognizerLifecycle` with new `set_arena_back_channel_for_pointer`" task collapses (already done here). S07.6 ForcePress moves from `kind != Mouse` to `pressure.is_some() && allowed_buttons_filter.map_or(true, \|f\| f(e.buttons, e.modifiers))`. |
| **S07.7** | Resampler's two-timestamp model gets the substrate (`timestamp` vs `source_timestamp`). Synthesised flag pre-positioned. |
| **S07.9** | Multi-drag's per-pointer entries reuse the per-pointer back-channel hook this plan finalises. |
| **S08** | Semantics `synthesized = true` for synthetic `Down`/`Up` pairs. `SemanticAction` enum still S08's job to extend. |
| **S09** | `HitTestEntry.transform` becomes meaningful when paint actually populates it; recognisers already read `local_position` so no recogniser-side change at S09 time. |
| **S11** | Scroll physics' fling integrators consume `source_timestamp` from VelocityTracker samples. `DragStartBehavior` is **not** in this plan — S11 territory. |
| **S20** | Real platform pressure values (`PressureSample.min`, `.max` populated honestly) is S20's job; this plan only changes the wire shape. |

## Performance Budgets

The S07 bench (`cargo run -p flui-core --release --example gesture_arena_bench`) stays the contract:

| Sub-bench | Operation | Budget | S07.5 measured | Target after this PR |
|---|---|---|---|---|
| `hit_test_8deep` | Linear scan | < 2 µs | ~0 ns (optimizer-folded) | unchanged |
| `arena_tick` | VelocityTracker.add+estimate | < 1.25 µs | ~272 ns | unchanged or slightly faster (LSQ now uses `source_timestamp` directly, no Instant arithmetic on each call) |
| `full_frame_120hz` | Combined p99 | < 8 ms | ~1.6 µs | unchanged |

`HitTestEntry` grows by `Option<Affine2>` (16 bytes for `None` discriminant + 6 × f32 for `Some` = 40 bytes total) plus `Point<Pixels>` (8 bytes). The hit-test path's `SmallVec<[HitTestEntry; 8]>` inline budget grows from N × 16 bytes (PointerId + position) to N × ~64 bytes — well within stack budget for N ≤ 8. T22 verifies via the bench.

The closure-based `allowed_buttons_filter` adds an indirect call **only when set** — when `None`, the simple `button.contains(...)` fast-path runs unchanged. Zero overhead for the default case.

## Tasks

### Phase A — Pointer event surface (changes #1, #2, #3, #7, #8)

- [ ] **T1:** Define `PressureSample` in `gesture/pointer_event.rs`:

  ```rust
  /// Platform-reported pressure value with its raw device range.
  ///
  /// Use `normalised()` to get a 0..=1 value relative to the device's
  /// own range. Two devices reporting different `value` may represent
  /// the same physical effort — always normalise before thresholding.
  #[derive(Copy, Clone, Debug, PartialEq)]
  #[non_exhaustive]
  pub struct PressureSample {
      /// Raw platform-reported pressure value.
      pub value: f32,
      /// Minimum value the platform can report (often 0.0; not always).
      pub min: f32,
      /// Maximum value the platform can report (often 1.0; Wacom etc. differ).
      pub max: f32,
  }

  impl PressureSample {
      pub fn normalised(self) -> f32 {
          let range = self.max - self.min;
          if range > 0.0 { ((self.value - self.min) / range).clamp(0.0, 1.0) } else { 0.0 }
      }
  }
  ```

  Add to `gesture/mod.rs` re-export list. Lock with a unit test: `pressure_sample_normalises_correctly_for_wacom_range()` (range 0..=8192, value 4096 → 0.5).

- [ ] **T2:** Replace `pressure: f32` with `pressure: Option<PressureSample>` on `PointerEvent`. Update `PointerEvent`'s rustdoc to describe the platform-truth matrix:
  - Mouse-class: `None`.
  - Touch / Stylus / Trackpad on platforms with real pressure sensors: `Some(PressureSample { value, min, max })` with the device's actual range.
  - Touch / Stylus / Trackpad on platforms without sensors (today: every desktop): `None`.

  Update the dispatch-side conversions in `gesture/dispatch.rs` to emit `None` for mouse-class events (current `pressure = 1.0 on Down else 0.0` logic dies) and `None` for synthetic test events that don't specify pressure.

- [ ] **T3:** Add `PointerKind::Trackpad`, `PointerKind::InvertedStylus`, `PointerKind::Unknown`. Update the rustdoc table on `PointerKind`. Document that `Trackpad` is the synthetic device behind `PointerPhase::PanZoom*` events (not the same as a `Mouse` cursor that happens to live on a trackpad).

- [ ] **T4:** Add `PointerPhase::PanZoomStart`, `PointerPhase::PanZoomUpdate`, `PointerPhase::PanZoomEnd`. Update `PointerPhase` rustdoc with the pan-zoom event lifecycle. Note that `Magnify` (in `PointerSignalEvent`) and PanZoom are **not** the same: Magnify is scalar; PanZoom carries pan + scale + rotation tuples.

  **Open question for `flui-arch-reviewer`:** should pan-zoom live as `PointerPhase` variants on the existing `PointerEvent`, or as a sibling `PointerPanZoomEvent` family alongside `PointerSignalEvent`? Flutter does the latter; v2's wire format is leaner. T4 adopts `PointerPhase` variants by default; the arch-reviewer pass ratifies or pivots.

- [ ] **T5:** Add `synthesized: bool` field to `PointerEvent` (default `false` for platform-emitted, `true` for resampler/semantics-emitted). Mention in module docs that:
  - Sanitiser-synthesised hover-Enter/Exit events are `synthesized = true`.
  - Future S07.7 resampler outputs `true` for interpolated Move events.
  - Future S08 semantics outputs `true` for synthetic Down/Up pairs.

  Update `PointerSanitizer::diff_hover` to mark its synthesised Enter/Exit events `synthesized = true`. This *was* an undocumented invariant; now it's enforced.

- [ ] **T6:** Split `timestamp` into two fields:
  - `timestamp: Instant` — when this event was *delivered* (for synthesised events: sample-boundary time).
  - `source_timestamp: Instant` — when the *originating* platform event happened (for non-synthesised: equal to `timestamp`).

  For platform-emitted events both equal `event_time`. For sanitizer-synthesised hover Enter/Exit, `source_timestamp = triggering_event.timestamp`. Update `VelocityTracker::add_position` to use `source_timestamp` (the time the user's finger was actually at that position, not the time we got around to noticing).

- [ ] **T7:** Update every dispatch path to populate `pressure`, `synthesized`, `source_timestamp`. Files touched: `gesture/dispatch.rs`, `gesture/binding.rs` (sanitizer call sites). For platform-side conversions (in `crates/flui-core/src/platform/**`), keep the wire-shape change centralized: the conversion functions (`convert_mouse_event` / etc.) populate `Option<PressureSample>`; the platform code itself stays `f32` until S20 plumbs real ranges.

- [ ] **T8:** Update existing recognisers and tests to consume the new shape:
  - Tap/DoubleTap/LongPress/Drag-family/Scale: none reference `event.pressure`. Compile-only impact.
  - Tests in `gesture/recognizers/*::tests`: bump synthetic `pe()` builders to set `pressure: None, synthesized: false, source_timestamp: timestamp`.
  - Integration tests in `crates/flui-core/tests/gesture_dispatch_integration.rs`: same.

> **Commit checkpoint A — after T1, T2, T3, T4, T5, T6, T7, T8:** `feat(flui-core)!: PointerEvent pressure/kind/phase/synthesized/timestamp surface upgrade (S07.5b A)`

### Phase B — Hit-test surface (change #5)

- [ ] **T9:** Pick the affine-transform primitive. Workspace already depends on `euclid`; the leading candidate is a thin alias `pub type Affine2 = euclid::Transform2D<f32, WindowSpace, LocalSpace>;` with marker structs for the source/target spaces. Alternative: a custom `pub struct Affine2 { rows: [[f32; 3]; 2] }` for cache-friendly inline storage. **Decision deferred to `flui-arch-reviewer`.** Whichever wins must support: `identity()`, `composed(other)`, `inverse() -> Option<Self>`, `transform_point(Point<Pixels>) -> Point<Pixels>`. Add to `crates/flui-core/src/geometry.rs` re-exports.

- [ ] **T10:** Extend `HitTestEntry` with two new fields:
  - `transform: Option<Affine2>` — `None` means identity (no transform between window and target). `Some(t)` means `local = t.inverse().transform_point(window_local)`.
  - `local_position: Point<Pixels>` — pre-computed from `transform.unwrap_or(identity).inverse().transform_point(event.position)`. Pre-computing once per hit-test entry avoids re-computing on every `Move` event in the recogniser hot loop.

  Default for both: `transform = None`, `local_position = position`. After T10 lands but before paint plumbs real transforms, every `HitTestEntry` is shape-compliant but semantically unchanged.

- [ ] **T11:** Add `HitTestResult` builder API:

  ```rust
  impl HitTestResult {
      pub fn push_transform(&mut self, t: Affine2);
      pub fn push_offset(&mut self, offset: Point<Pixels>); // shorthand for translation-only
      pub fn pop_transform(&mut self);
      pub fn add_with_transform(&mut self, entry: HitTestEntry); // composes current stack into entry.transform
  }
  ```

  Internally the result keeps a `SmallVec<[Affine2; 4]>` stack; each `add` snapshots the composed top-of-stack into the entry being added. `pop_transform` is the only way to leave a pushed scope; document the push/pop balance contract (any unbalanced push is a bug — assert in debug builds).

- [ ] **T12:** Wire `Window::hit_test` to use the transform stack. For S07.5b, the only transform pushed by paint is the implicit `identity` — so `transform` stays `None` for every entry. The point of T12 is to verify the *path* is correct end-to-end: a unit test where we manually `push_transform(rotate_90)` before adding an entry, drive a Down event, and confirm the recogniser receives `entry.local_position` matching the rotation-corrected coordinates.

- [ ] **T13:** Migrate every recogniser's slop/distance check from `event.position` to `entry.local_position`. For S07.5b this is a no-op behaviourally (`local_position == position` until S09). The migration unblocks S09: when paint starts pushing real transforms, recognisers immediately see local coords.

  **Where the migration happens:** the recogniser `handle_event` body has the position; today it just reads `event.position`. We need to plumb the *entry's* `local_position` to the recogniser somehow. Two paths:
  - **A) Add a parameter** to `GestureRecognizer::handle_event`: `entry: &HitTestEntry`. **Breaking** for the trait, fixes every recogniser cleanly.
  - **B) Decorate `PointerEvent`** with the local-position from the dispatcher's perspective. `PointerEvent.position` becomes window-local; add `event.local_position: Point<Pixels>` (set by dispatcher at deliver time).

  **Decision:** **B**. The trait stays stable; `PointerEvent` was already growing. The dispatcher sets `event.local_position = entry.local_position` per-recogniser-delivery. Document the asymmetry: `event.position` is window-local (constant across recognizers); `event.local_position` is hitbox-target-local (varies per recogniser).

> **Commit checkpoint B — after T9, T10, T11, T12, T13:** `feat(flui-core)!: HitTestEntry transform stack + per-recogniser local_position (S07.5b B)`

### Phase C — Recognizer + arena surface (changes #4, #6, #9)

- [ ] **T14:** Delete `RecognizerLifecycle::set_arena_back_channel(bc, idx)`. Rename the planned-for-S07.6 `set_arena_back_channel_for_pointer(pid, bc, idx)` (which was going to ship in S07.6) to land **here** as the only hook. The trait method becomes the canonical default-no-op:

  ```rust
  pub trait RecognizerLifecycle {
      fn needs_back_channel(&self) -> bool { false }
      fn set_arena_back_channel(
          &mut self,
          _pointer_id: PointerId,
          _back_channel: ArenaBackChannel,
          _entry_index: usize,
      ) {}
      fn needs_arena_hold(&self) -> bool { false }
      fn configure_settings(&mut self, _settings: &GestureSettings) {}
  }
  ```

  Note: re-using the name `set_arena_back_channel` (now with three args including `pointer_id`) is the cleanest naming — the planned-for-S07.6 `_for_pointer` suffix was only there to disambiguate from a legacy hook we just deleted.

- [ ] **T15:** Migrate `LongPressGestureRecognizer` to the per-pointer hook. The recogniser today stores `pointer_index: Option<usize>` (single-shot) and `arena_back_channel: ArenaBackChannel`. After this task: `pointer_index: HashMap<PointerId, usize>` (or `SmallVec<[(PointerId, usize); 1]>` for inline storage — single-shot guarantees one entry). `set_arena_back_channel(pid, bc, idx)` inserts; the timer-fire path looks up by the recogniser's known `pointer` field. Behaviour-equivalent.

- [ ] **T16:** Update `GestureBinding::register_recognizer` to call the renamed hook with `pointer_id` baked in (it already has `pointer_id` as the first argument; just pass it through):

  ```rust
  if lifecycle.needs_back_channel() {
      let back_channel = GestureArenaManager::make_back_channel_from(&self.arena);
      lifecycle.set_arena_back_channel(pointer_id, back_channel, entry_index);
  }
  ```

  No more dual-call path.

- [ ] **T17:** Add `#[non_exhaustive]` to `GestureDisposition`. Document that future variants (potentially `AcceptedDeferred` for captain-deferred resolution, `RejectedBySweep` for tracing) become non-breaking additions.

- [ ] **T18:** Define `AllowedButtonsFilter` and add `pub allowed_buttons_filter: Option<Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>>` to every existing recogniser (Tap, DoubleTap, LongPress, Pan, HorizontalDrag, VerticalDrag, Scale). The semantic is: in `add_pointer`, if the recogniser would normally accept based on `event.buttons.contains(self.button)`, additionally check `self.allowed_buttons_filter.as_ref().map_or(true, |f| f(event.buttons, event.modifiers))`. When the filter is `None`, behaviour matches today exactly. When set, the filter overrides. Add a fluent builder `with_allowed_buttons_filter(f)` per recogniser.

  ```rust
  pub type AllowedButtonsFilter = dyn Fn(PointerButtons, Modifiers) -> bool + 'static;
  ```

  (free type alias; no struct needed). Re-export from `gesture/mod.rs`.

> **Commit checkpoint C — after T14, T15, T16, T17, T18:** `feat(flui-core)!: unify back-channel lifecycle hook + GestureDisposition non_exhaustive + AllowedButtonsFilter (S07.5b C)`

### Phase D — Tests, docs, bench

- [ ] **T19:** Update unit tests for every changed recogniser + every dispatch path:
  - `gesture/dispatch.rs::tests` — verify mouse-class events produce `pressure: None`, hover-synthesised events are `synthesized: true`, source_timestamp is propagated.
  - `gesture/recognizers/*::tests` — bump `pe()` helpers; add a `*_allowed_buttons_filter_overrides_button_field` canary per recogniser.
  - Property tests in `gesture/arena.rs::tests` — confirm the per-pointer back-channel migration preserves the S07.5 P-T15.5-A/B/C invariants.
  - Add a new property test `prop_long_press_back_channel_round_trips_pointer_id` — registers via `set_arena_back_channel(pid, bc, idx)`, fires the timer, asserts `declare_winner` was called with the same pid + idx.
  - Add a transform-stack hit-test test: synthetic `push_transform(rotate_90)`, Down event, recogniser sees rotated `local_position`.

- [ ] **T20:** Sweep rustdoc on every type that grew or changed:
  - `gesture/pointer_event.rs` — `PointerEvent`, `PointerKind`, `PointerPhase`, `PressureSample`.
  - `gesture/recognizer.rs` — `RecognizerLifecycle::set_arena_back_channel`.
  - `gesture/arena.rs` — `GestureDisposition` `#[non_exhaustive]` note.
  - `gesture/hit_test.rs` — `HitTestEntry`, `HitTestResult`.
  - `gesture/recognizers/*.rs` — `allowed_buttons_filter` field doc.
  - `gesture/mod.rs` — module-level "S07.5b — completed" subsection mirroring the S07.5 pattern.

- [ ] **T21:** Update `docs/superpowers/specs/2026-05-08-recognizer-extension.md`:
  - "Adding a new recognizer step-by-step" — step 4 now references the unified `set_arena_back_channel(pid, bc, idx)` hook (drop the legacy hook from the worked LongPress example).
  - "When to use `RecognizerLifecycle`" — table row for `set_arena_back_channel` updates the signature.
  - Add a row noting `allowed_buttons_filter` as the canonical extension point for advanced button/modifier gating.
  - "Threshold-field conventions" — add `pressure` thresholds operate on `PressureSample::normalised()`, never raw `value`.

- [ ] **T22:** Introduce `CHANGELOG.md` at the workspace root. Adopt Keep-a-Changelog format. First entry: "Unreleased — S07.5b breaking changes". Lists each Phase A/B/C surface change with a one-line migration note. Cross-references the audit doc as design rationale. (Addresses R3 of the roadmap prematurely-but-cheaply.)

- [ ] **T23:** Bench regression verification. Run `cargo run -p flui-core --release --example gesture_arena_bench` after Phase C lands. All three sub-bench budgets must still pass: `hit_test_8deep < 2 µs`, `arena_tick < 1.25 µs`, `full_frame_120hz < 8 ms p99`. If any threshold regresses, isolate via git bisect within Phases A/B/C. The expected impact is zero — every change is shape, not algorithm — but the bench is the lock.

- [ ] **T24:** Update `.ai-factory/ROADMAP.md`: add `S07.5b GestureArena — pre-roster cleanup` under Phase II between S07.5 and S07.6 entries; add to Completed table on merge. Update `DESCRIPTION.md` Input pipeline bullet to mention the new pressure surface and the `allowed_buttons_filter` extension point.

> **Commit checkpoint D — after T19, T20, T21, T22, T23, T24:** `test(flui-core): S07.5b regression locks + rustdoc + recognizer-extension doc + CHANGELOG + ROADMAP (S07.5b D)`

## Commit Plan

| Checkpoint | After tasks | Suggested message |
|---|---|---|
| A | T1–T8 | `feat(flui-core)!: PointerEvent pressure/kind/phase/synthesized/timestamp surface upgrade (S07.5b A)` |
| B | T9–T13 | `feat(flui-core)!: HitTestEntry transform stack + per-recogniser local_position (S07.5b B)` |
| C | T14–T18 | `feat(flui-core)!: unify back-channel lifecycle hook + GestureDisposition non_exhaustive + AllowedButtonsFilter (S07.5b C)` |
| D | T19–T24 | `test(flui-core): S07.5b regression locks + rustdoc + recognizer-extension doc + CHANGELOG + ROADMAP` |

The `!` markers are conventional-commits "breaking change" indicators. Phase D does not break — only locks.

## Review Subagents

Per `.ai-factory/rules/base.md`:

- **`flui-arch-reviewer`** — proactively before T4 lands (PointerPhase vs sibling event family decision), before T9 (Affine2 primitive choice), after T11 (HitTestResult builder API design), after T14 (lifecycle-hook unification — long-lived seam).
- **`rust-api-migration-auditor`** — on every Phase A task (each is a `pub` field/variant change → semver impact under any future crates.io publish). On T17 (`GestureDisposition` `#[non_exhaustive]` is a forward-compat lock). On T18 (`AllowedButtonsFilter` type alias and per-recogniser builder).
- **`migration-risk-adversary`** — on T2 (pressure-shape change has the widest blast radius), T13 (every recogniser's slop check moves to `local_position`; subtle off-by-one risk), T14 (deletes a public-ish trait method).
- **`wgpu-gpu-reviewer`** — not applicable.

## Acceptance Criteria

1. **`PointerEvent.pressure: Option<PressureSample>` ships.** Mouse-class events return `None`; synthetic test events specify or omit pressure explicitly. `PressureSample::normalised()` returns `[0.0, 1.0]` for any valid range. Tests cover Wacom-range and Force-Touch-range cases.
2. **`PointerKind::{Trackpad, InvertedStylus, Unknown}` exist** and are documented in the rustdoc table.
3. **`PointerPhase::{PanZoomStart, PanZoomUpdate, PanZoomEnd}` exist** and are distinct from `PointerSignalEvent::Magnify`. (Or, if `flui-arch-reviewer` pivots T4, a sibling event family with the same coverage.)
4. **`PointerEvent.synthesized: bool` field exists.** Sanitizer-synthesised hover Enter/Exit events are marked `true`; platform-emitted events are `false` by default. Future S07.7 / S08 consumers find the flag pre-positioned.
5. **`PointerEvent.timestamp` and `PointerEvent.source_timestamp` are distinct.** For non-synthesised: equal. `VelocityTracker` uses `source_timestamp`.
6. **`HitTestEntry.transform: Option<Affine2>` and `HitTestEntry.local_position: Point<Pixels>` exist.** Default `None` and `position` respectively. `HitTestResult::{push_transform, push_offset, pop_transform, add_with_transform}` API is documented and unit-tested.
7. **Every recogniser reads `entry.local_position` (via `event.local_position`) for slop and distance checks.** Existing tests pass unchanged; one new transform-stack test locks the rotation case.
8. **`RecognizerLifecycle::set_arena_back_channel(pid, bc, idx)` is the only back-channel hook.** No legacy single-pointer hook remains. `LongPressGestureRecognizer` is migrated. Property test covers the round-trip.
9. **`GestureDisposition` is `#[non_exhaustive]`.**
10. **Every recogniser exposes `allowed_buttons_filter: Option<Box<dyn Fn(...)>>` field + `with_allowed_buttons_filter(f)` builder.** Default `None`; default behaviour unchanged.
11. **Performance budgets unchanged.** Bench (T23) confirms.
12. **`CHANGELOG.md` exists** with a "Unreleased — S07.5b" entry listing every breaking change with a migration note.
13. **`docs/superpowers/specs/2026-05-08-recognizer-extension.md` updated.** Lifecycle table row reflects the unified hook signature; new row for `allowed_buttons_filter`; `pressure` thresholds normalisation note.
14. **`ROADMAP.md` and `DESCRIPTION.md` updated** to document the new surface and mark S07.5b complete on merge.

## Risks

- **Pressure-shape change has the widest blast radius.** Every test that constructs a synthetic `PointerEvent` touches `pressure`. Mitigation: `PointerEventBuilder` test helper centralizes the construction; one bulk update.
- **Hit-test transform stack is a new public API.** Pushing/popping unbalanced is a bug class we're inviting. Mitigation: assert-balance in debug builds; explicit pop-all-on-frame-end discipline in `Window::hit_test`. Document the contract in rustdoc.
- **Recogniser `event.local_position` migration risk.** S07.5b sets `local_position == position` everywhere, so behaviour is unchanged. **But** if a recogniser keeps reading `event.position` after the migration, slop checks will be subtly wrong when S09 lands. Mitigation: T13 changes every reference; T20 rustdoc on `event.position` clarifies "use `local_position` for in-target geometry"; a future arch-review on S09 should grep for `event\.position` in recogniser code.
- **`Affine2` primitive choice may be wrong.** Picking `euclid::Transform2D` may force ergonomic awkwardness; picking a custom struct may force re-implementation of inversion math. Mitigation: T9 is gated on `flui-arch-reviewer`. If we pick wrong, it's an internal type — swap is non-breaking.
- **`PanZoomStart/Update/End` as `PointerPhase` variants vs sibling event family.** T4 chooses `PointerPhase` variants; arch-reviewer may pivot to a sibling family. The plan's contract is "the substrate exists" — naming/shape can adjust.
- **`AllowedButtonsFilter` is a `Box<dyn Fn>` per recogniser.** Adds 16 bytes per recogniser instance whether or not anyone uses it. Mitigation: `Option<Box<...>>` makes the empty case 8 bytes (null pointer) and zero allocation. Acceptable.
- **Lifecycle hook unification is a real public-ish breaking change.** Any downstream code that implemented `RecognizerLifecycle::set_arena_back_channel(bc, idx)` will not compile. Mitigation: there is no downstream code. The trait is brand new (S07.5, May 2026) and the crate is unpublished. The migration is internal to LongPress only.
- **Synthesized flag on synthesised events is silent today.** Recognisers don't inspect it yet. Mitigation: that's the point — the flag is pre-positioned for S07.7 / S08; until then it's metadata. The cost of pre-positioning is one bool per event.
- **`source_timestamp` propagation through every dispatch path.** Easy to forget. Mitigation: T7 systematically updates every conversion site; the sanitizer's hover-synthesis path is the trickiest case (uses `triggering_event.timestamp`); tests cover both.
- **CHANGELOG.md introduction may collide with future R3 work.** Mitigation: T22 explicitly notes "addresses R3 prematurely-but-cheaply"; future R3 inherits the file rather than competes with it.
- **Big PR.** S07.5b is ~24 tasks across 4 phases. Reviewers may time out. Mitigation: 4 commit checkpoints split it into reviewable chunks (~6 tasks each); each checkpoint is mergeable as its own conventional-commit-tagged commit. If reviewers want, we can split into four PRs (A → B → C → D), but the working assumption is one PR with ordered commits.
