# Plan: S07.5b GestureArena — pre-roster cleanup (breaking changes before S07.6)

- **Branch:** `feature/s07-5b-pre-roster-cleanup`
- **Created:** 2026-05-08
- **Revision:** 2026-05-08 (revised to integrate three review-agent passes — flui-arch-reviewer, rust-api-migration-auditor, migration-risk-adversary)
- **Mode:** full
- **Predecessor PRs:** #1 (S07 GestureArena) and #2 (S07.5 follow-up).
- **Source design rationale:** `docs/superpowers/specs/2026-05-08-flutter-gestures-architectural-audit.md` — the Flutter gestures architectural audit; this plan executes its breaking-friendly proposals **#1, #2, #3, #4, #5, #6, #7, #8, #9** as one consolidated cleanup before S07.6, plus several review-driven additions that closed open architectural questions in the audit.
- **Working set:** the gesture-layer surface still has nine breaking-friendly mistakes plus three structural bugs in the existing arena/dispatch code that became visible during review. We have not published to crates.io, so backwards compatibility is hypothetical. Fixing them now costs one PR; fixing them after S07.6 ships ForcePress with hardcoded thresholds and after S09 lands `Transform` widgets costs many PRs and one user-visible regression each.

This plan is **prerequisite** for `feature-s07-6-recognizer-roster.md`. S07.6 consumes the renamed lifecycle hook (single-pointer hook deleted, only per-pointer hook ships), the new `Option<PressureSample>` pressure surface (replaces the `kind != Mouse` guard for ForcePress with proper `Some`-checking + AllowedButtonsFilter), and the new `AllowedButtonsFilter` type. Land S07.5b first, then S07.6.

## Settings

- **Testing:** yes — every surface change ships with a regression-lock test. Several phases include compile-time canaries that the public API stays the intended shape.
- **Logging:** verbose — `log` + `kv_unstable_serde`. New `kv` fields: `pressure_min`, `pressure_max`, `pressure_value`, `provenance`, `transform_present` for hit-test path.
- **Docs:** yes — mandatory `/aif-docs` checkpoint at completion. **Doc discipline rule:** every commit checkpoint (A/B/C/D) must compile clean against `#![warn(missing_docs)]`. New `pub` items get rustdoc *in the same commit they are introduced* — do not batch all rustdoc work into Phase D.

## Roadmap Linkage

- **Milestone:** new entry `S07.5b GestureArena — pre-roster cleanup (breaking changes)` under Phase II, between `S07.5 GestureArena T15 follow-up` and the planned `S07.6 GestureArena recognizer roster expansion`.
- **Rationale:** the audit identified nine breaking-friendly proposals plus three latent bugs surfaced by the review-agent pass (`is_held: bool` instead of counter, `dispatch.rs` 12 PointerEvent literals not enumerated by the original T7, `translate_mouse_pressure` losing macOS Force Touch values). Doing them in one sweep gives reviewers one architectural pass instead of 12 drips.

## Goals

1. **Faithful pressure semantics.** Replace `pressure: f32` with `pressure: Option<PressureSample>` carrying `value: f32` plus the platform's `min` / `max` raw range. Mouse-class events return `None` *unless* the platform is actually emitting real pressure for that mouse-class device (macOS `MousePressureEvent` for Force Touch — see Decision MM in the table). Recognizers normalize inside their own threshold checks via `PressureSample::normalize()`.
2. **Provenance enum, not a boolean.** Replace the originally-planned `synthesized: bool` with `provenance: PointerEventProvenance` (`#[non_exhaustive]` enum: `Platform`, `SanitizerSynthesized`, future `ResamplerSynthesized`, future `SemanticsSynthesized`). A `bool` cannot capture the resampler/semantics distinction without a parallel field; once published, `bool` would force a major bump.
3. **Complete `PointerKind` surface.** Add `Trackpad`, `InvertedStylus`, `Unknown`. `Trackpad` is the kind for the synthetic device emitting pan-zoom events on macOS.
4. **Pan-zoom as a sibling event family, not `PointerPhase` variants.** Add `PointerPanZoomEvent { kind: PointerKind, pointer_id: PointerId, timestamp: Instant, source_timestamp: Instant, pan: Point<Pixels>, scale: f32, rotation: f32, phase: PanZoomPhase }` as a new sibling type alongside `PointerSignalEvent`. Putting `PanZoomStart/Update/End` on `PointerPhase` would force the rich payload (pan/scale/rotation tuple) to live on every `PointerEvent` as `Option`-typed dead weight 99% of the time. Flutter split this for the same reason.
5. **Single lifecycle hook for back-channel.** Remove `RecognizerLifecycle::set_arena_back_channel(bc, idx)` entirely. The only hook is `set_arena_back_channel(pid, bc, idx)` — same name, three arguments. `LongPressGestureRecognizer` migrates to a `SmallVec<[(PointerId, usize); 1]>` storage (single-shot keeps inline storage; multi-pointer recognisers use `HashMap`).
6. **Transform-stack hit-test substrate.** Custom `Affine2 { rows: [[f32; 3]; 2] }` primitive (not `euclid::Transform2D` — `euclid` is NOT a direct `flui-core` dep). `HitTestEntry` gains `transform: Option<Affine2>`. The dispatcher passes `local_position` to recognisers via a new `DeliveredEvent<'a>` wrapper, **not** via a mutable field on `PointerEvent`.
7. **`HitTestScope<'_>` RAII transform stack.** `HitTestResult::push_transform(t) -> HitTestScope<'_>` returns an RAII guard whose `Drop` impl pops. Unbalanced push/pop becomes a borrow-checker error, not a debug-assert.
8. **`DeliveredEvent<'a>` wrapper for handle_event.** `GestureRecognizer::handle_event` signature changes from `(event: &PointerEvent, ...)` to `(event: DeliveredEvent<'_>, ...)`. The wrapper carries `event: &PointerEvent` plus per-recogniser `local_position: Point<Pixels>`. Avoids a per-delivery mutation on a shared `&PointerEvent` and makes the "different recognisers see different local_positions" semantics explicit at the type level.
9. **`GestureDisposition` `#[non_exhaustive]`** — verified already true at `arena.rs:26`. T17 collapses to a verification-only task (no code change).
10. **`AllowedButtonsFilter` as a newtype struct.** `pub struct AllowedButtonsFilter(Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>)` with `::new(closure)` and `::call(buttons, mods)` methods. Avoids the `pub type X = dyn Trait` alias footgun (not nameable in return position, ugly in error messages, can't be impl'd).
11. **`is_held` becomes a counter, not a boolean.** `arena.rs:65 is_held: bool` → `hold_count: u32`. Increment on `hold(pid)`, decrement on `release(pid)`, sweep gated on `hold_count == 0`. Fixes the latent bug where DoubleTap+MultiTap on the same pointer would have one's `release` clear both.
12. **Split `timestamp` into `timestamp` + `source_timestamp`.** For non-synthesised events: equal. For resampler/semantics: distinct. `VelocityTracker` callers (incl. drag.rs:231, drag.rs:249) switch to `source_timestamp`.

## Non-goals

- **Recommendations #10-#14 from the audit** (DragStartBehavior, Drag trait stub, MultiDragPointerState stub, public GestureArenaTeam, LongPressDetails.velocity, secondary/tertiary long-press, SemanticAction extension). Ship with their relevant milestones (S07.9, S11, S13, S08).
- **Helper-trait extraction (PrimaryPointerState).** Audit §3.1 recommendation; lands in S07.6 as part of MultiTap/ForcePress when the duplication actually shows up.
- **Per-platform velocity trackers** (audit §3.9). S11 territory.
- **Real platform pressure plumbing for non-macOS desktops.** `PressureSample.min/max` honestly populated by every platform is S20 work. For S07.5b: every desktop platform except macOS-Force-Touch path emits `None` for mouse-class events; macOS keeps emitting real `MousePressureEvent` pressure (see Decision MM).
- **Hit-test transform actually populated by paint.** S09 territory. For S07.5b: `transform = None` always until S09.
- **Public GestureArenaTeam captain-deferred resolution.** S11 territory; the existing struct's required-captain shape stays unchanged here.

## Architectural Decisions

| ID | Decision | Choice | Rationale |
|---|---|---|---|
| **D1** | Pressure shape | `Option<PressureSample { value: f32, min: f32, max: f32 }>` | Audit §3.13. Carries platform range; mouse returns `None`. `PressureSample::normalize() -> f32` returns `(value - min) / (max - min)` clamped. |
| **D2** | `PointerKind` extras | Add `Trackpad`, `InvertedStylus`, `Unknown` (4→7 variants). | Audit §3.5 + §5#2. `#[non_exhaustive]` already on the enum. |
| **D3** | Pan-zoom event shape | New sibling type `PointerPanZoomEvent` in `gesture/pan_zoom_event.rs` (or extending `pointer_signal.rs`), **NOT** `PointerPhase` variants. | flui-arch-reviewer: pan/scale/rotation payload doesn't fit on `PointerEvent` without 3 `Option` fields ~99%-empty. Flutter split for this reason. |
| **D4** | Lifecycle hook unification | Delete two-arg `set_arena_back_channel(bc, idx)`. Keep only `set_arena_back_channel(pid, bc, idx)` — same name, three args. | Audit §5#4. The legacy hook is dead weight pre-publish. LongPress carries `SmallVec<[(PointerId, usize); 1]>` after migration. |
| **D5** | Hit-test transform primitive | **Custom** `pub struct Affine2 { rows: [[f32; 3]; 2] }` with `identity()`, `composed(other)`, `inverse() -> Option<Self>`, `transform_point(p)`. **NOT** `euclid::Transform2D`. | rust-api-migration-auditor + flui-arch-reviewer: `euclid` is NOT a direct `flui-core` dep (only transitive via `etagere`). Direct dep on `euclid` either pollutes the crate's public surface (since `Affine2` is `pub`) or risks silent breakage on `etagere` major bumps. ~15 lines of bespoke code is cheaper than a transitive-dep stability risk. |
| **D6** | Transform-stack ergonomics | `HitTestResult::push_transform(t) -> HitTestScope<'_>` RAII guard. `HitTestScope::add(entry)` for adding entries inside the scope. `Drop` pops the stack. **NOT** explicit push_transform/pop_transform with debug-assert. | rust-api-migration-auditor + flui-arch-reviewer: imperative push/pop with debug-assert silently corrupts on production unbalanced state. RAII makes it a borrow-check error. Cannot become RAII post-publish without breaking change. |
| **D7** | Per-delivery local position | `DeliveredEvent<'a> { event: &'a PointerEvent, local_position: Point<Pixels> }` wrapper passed to `GestureRecognizer::handle_event(event: DeliveredEvent<'_>, ...)`. **NOT** a mutable field on `PointerEvent`. | rust-api-migration-auditor: a per-delivery mutation on shared `&PointerEvent` is a semantic surprise; adding `local_position` to `PointerEvent` makes the field stale on any cloned/stashed event. The wrapper makes per-delivery semantics explicit at the type level. Trait signature change is a breaking change we'd need to make eventually anyway — pre-publish is free. |
| **D8** | Provenance shape | `pub provenance: PointerEventProvenance` (`#[non_exhaustive]` enum: `Platform`, `SanitizerSynthesized`, future variants). **NOT** `synthesized: bool`. | rust-api-migration-auditor: bool can't distinguish resampler-synthesized (S07.7) from semantics-synthesized (S08) from sanitizer-synthesized (now). Once published, bool→enum is a major bump. Ship enum from day one. |
| **D9** | AllowedButtonsFilter shape | `pub struct AllowedButtonsFilter(Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>)` with `new()` constructor + `call()` evaluator. **NOT** a `pub type X = dyn Fn(...)` alias. | rust-api-migration-auditor: `dyn Trait` aliases have well-known Rust footguns. Newtype names cleanly in errors, is extensible (add methods later), and unit-testable. |
| **D10** | AllowedButtonsFilter check site | Filter check moves to `GestureBinding::register_recognizer` **before** `arena.add(pid, recognizer)` (not in `add_pointer`). On filter rejection, `register_recognizer` short-circuits and the recognizer never enters the arena. | migration-risk-adversary: filter in `add_pointer` leaves the recogniser in the arena with `pointer == None`, returning `Possible` forever — permanent zombie slot. Move the gate one level up to fix the state-machine corruption at the source. |
| **D11** | Arena hold counter | `arena.rs` `is_held: bool` → `hold_count: u32`. `hold(pid)` increments; `release(pid)` decrements; sweep gated on `hold_count == 0`. | flui-arch-reviewer: audit §3.2 already warned. With S07.6 MultiTap (also wanting hold) and DoubleTap (also wanting hold), a boolean conflates two independent holders. Counter is the only correct shape. Land here so S07.6 can rely on it. |
| **D12** | Timestamp split | `PointerEvent { timestamp: Instant, source_timestamp: Instant }`. For non-synthesised: equal. `VelocityTracker` and the drag recogniser at `drag.rs:231, drag.rs:249` use `source_timestamp`. | Audit §5#8. Resampler (S07.7) sets `timestamp = sample_boundary_time` and `source_timestamp = nearest_input_event.timestamp`. |
| **MM** | macOS Force Touch path | `translate_mouse_pressure` (`dispatch.rs:419`) maps `MousePressureEvent.pressure` → `Some(PressureSample { value: e.pressure, min: 0.0, max: 1.0 })`. Mouse-class events from non-pressure devices remain `None`. | migration-risk-adversary contradiction resolution: macOS Force Touch is the **only** real pressure path that exists today and flows through mouse-class events. Mapping it to `None` would break Force Touch silently. Mapping it to `Some(PressureSample {...})` preserves the existing behavior and aligns with future trackpad pressure work. |

## Cross-cutting Roadmap Interactions

| Cross-cutting | This plan's contract |
|---|---|
| **A2 — Audit remaining ~29 globs** | New types (`PressureSample`, `PointerEventProvenance`, `Affine2`, `DeliveredEvent`, `AllowedButtonsFilter`, `HitTestScope`) re-exported per the explicit list pattern. **T1 must add explicit `pub use` lines to both `gesture/mod.rs` AND `lib.rs`** — A2 hygiene applies to every new public item, not as an afterthought. |
| **A3 — Error-type unification** | No new error types. `Option<PressureSample>` makes "no pressure available" a value, not an error. |
| **A4 — Tracing standardization** | Stay on `log` + `kv`. New `kv` fields `pressure_min`/`pressure_max`/`pressure_value` (numeric) and `provenance` (string). |
| **A5 — Feature flag matrix discipline** | No new feature combos. **`--features test-support` smoke check:** verify that `simulate_*` helpers in `TestAppContext` (which may construct `PointerEvent` literals directly) compile after Phase A. |
| **A7 — Interior-mutability surface reduction** | `AllowedButtonsFilter(Box<dyn Fn>)` newtype adds no interior mutability — `Fn` (not FnMut) is the bound. |
| **A8 — `#[non_exhaustive]` audit** | `PressureSample`, `PointerEventProvenance`, `PointerPanZoomEvent`, `PanZoomPhase` carry `#[non_exhaustive]` from day one. New `PointerKind` variants ride existing `#[non_exhaustive]`. `GestureDisposition` is **already** `#[non_exhaustive]` (`arena.rs:26`); T17 verifies but does not change. |
| **R3 — CHANGELOG.md** | T22 introduces `CHANGELOG.md`. Not optional any more — this PR has many breaking changes. |
| **S07.6** | This plan is its prerequisite. S07.6's lifecycle-extension task collapses; S07.6 ForcePress moves from `kind != Mouse` to `pressure.is_some() && allowed_buttons_filter.map_or(true, \|f\| f.call(...))`. |
| **S07.7** | Resampler's two-timestamp model + `provenance: ResamplerSynthesized` get the substrate. |
| **S07.9** | Multi-drag's per-pointer entries reuse the unified back-channel hook. |
| **S08** | Semantics `provenance: SemanticsSynthesized` for synthetic Down/Up pairs. |
| **S09** | `HitTestEntry.transform` and `HitTestScope` ergonomics ready. **Constraint:** when S09 starts pushing real transforms, it must also drive `local_position` on every entry — not just populate `transform` and leave `local_position` unset. Rustdoc on T10 documents this. |
| **S11** | Scroll physics' fling integrators consume `source_timestamp` from VelocityTracker samples. |
| **S20** | Real platform pressure values (`min`, `max` populated honestly per device) is S20's job; `MousePressureEvent` (macOS) already populated correctly per **Decision MM**. |

## Performance Budgets

The S07 bench stays the contract:

| Sub-bench | Budget | S07.5 measured | Target after this PR |
|---|---|---|---|
| `hit_test_8deep` | < 2 µs | ~0 ns | unchanged |
| `arena_tick` | < 1.25 µs | ~272 ns | unchanged or slightly faster (LSQ uses `source_timestamp` with no Instant arithmetic on each call) |
| `full_frame_120hz` | < 8 ms p99 | ~1.6 µs | unchanged |

**Memory accounting:**
- `PointerEvent` grows by `Option<PressureSample>` (24 bytes vs 4 today's `f32` = +20 bytes worst case), `provenance: PointerEventProvenance` (1 byte enum), `source_timestamp: Instant` (8-16 bytes). Total: +29-37 bytes. Today's PointerEvent ≈ 80 bytes; new ≈ 110-120 bytes.
- `HitTestEntry` grows by `Option<Affine2>` (≈25 bytes incl. discriminant) + `local_position` was deferred to `DeliveredEvent` per D7 (no growth on `HitTestEntry` for that part).
- `SmallVec<[HitTestEntry; 8]>` inline budget: 8 × ~64 bytes ≈ 512 bytes stack — well within budget.

Check at T23 bench that the larger `PointerEvent` doesn't push any allocation off-stack.

## Tasks

### Phase A — Pointer event surface (D1, D2, D3, D8, D12, MM)

- [ ] **T1:** Define `PressureSample` in `gesture/pointer_event.rs`. `#[non_exhaustive]` struct, `Copy + Clone + Debug + PartialEq` derive (NOT `Eq`/`Hash` — `f32` blocks them). Method `pub fn normalize(self) -> f32` returns `((value - min) / (max - min)).clamp(0.0, 1.0)` if `max > min` else `0.0`. **Add explicit `pub use` line** to `gesture/mod.rs:251` re-export block AND to `lib.rs` re-export block (A2 hygiene). Naming uses American spelling (`normalize`) for consistency with Rust ecosystem conventions.

- [ ] **T2:** Replace `pub pressure: f32` with `pub pressure: Option<PressureSample>` on `PointerEvent`. **Also migrate `WindowPointerState::last_pressure: f32` (`dispatch.rs:47`) to `last_pressure: Option<PressureSample>`** — its setters at `dispatch.rs:348, 375, 426` need the new shape. Update `PointerEvent`'s rustdoc to describe the platform-truth matrix:
  - Mouse-class events (most desktops): `None`.
  - Mouse-class events on macOS via `MousePressureEvent` (Force Touch): `Some(PressureSample { value, min: 0.0, max: 1.0 })` per Decision MM.
  - Touch / Stylus / Trackpad with real pressure sensors: `Some(...)` with the device's actual range.
  - Touch / Stylus / Trackpad on platforms without sensors: `None`.

- [ ] **T3:** Add `PointerKind::Trackpad`, `PointerKind::InvertedStylus`, `PointerKind::Unknown`. Update rustdoc table. **Document explicitly:** Windows emits `Mouse` for normal trackpad cursor movement; `Trackpad` only for the dedicated pan-zoom synthetic device path (Decision D3).

- [ ] **T4:** Define `PointerPanZoomEvent` in `gesture/pan_zoom_event.rs` (new file) as a sibling type to `PointerSignalEvent`:

  ```rust
  /// macOS-trackpad-style pan-zoom-rotate gesture event.
  ///
  /// Distinct from `PointerSignalEvent::Magnify` (which is scalar-only)
  /// because pan-zoom carries pan + scale + rotation tuples.
  #[non_exhaustive]
  pub struct PointerPanZoomEvent {
      pub kind: PointerKind,            // typically Trackpad
      pub pointer_id: PointerId,
      pub timestamp: Instant,
      pub source_timestamp: Instant,
      pub provenance: PointerEventProvenance,
      pub position: Point<Pixels>,
      pub pan: Point<Pixels>,
      pub scale: f32,                   // 1.0 = no zoom
      pub rotation: f32,                // radians
      pub phase: PanZoomPhase,          // Start | Update | End
  }

  #[non_exhaustive]
  pub enum PanZoomPhase { Start, Update, End }
  ```

  Re-export from `gesture/mod.rs` and `lib.rs`. The platform layer is not yet wired to emit these on any platform; T4 defines the type identity only. Future S20 / native macOS pinch lands in `crates/flui-platform/`. **DO NOT** add `PanZoomStart/Update/End` variants to `PointerPhase` — that's the rejected alternative.

- [ ] **T5:** Replace the originally-planned `synthesized: bool` with `pub provenance: PointerEventProvenance` field on `PointerEvent`. Define the enum:

  ```rust
  #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
  #[non_exhaustive]
  pub enum PointerEventProvenance {
      /// Emitted directly by the platform layer.
      #[default]
      Platform,
      /// Synthesized by the pointer sanitizer (hover Enter/Exit).
      SanitizerSynthesized,
      // Future: ResamplerSynthesized (S07.7), SemanticsSynthesized (S08).
  }
  ```

  Update `PointerSanitizer::diff_hover` AND the orphan-Cancel synthesis path at `dispatch.rs:162-174` to set `provenance: PointerEventProvenance::SanitizerSynthesized`. Both synthesis sites must be enumerated; T5 explicitly addresses both (the original audit only mentioned `diff_hover`).

- [ ] **T6:** Split `timestamp` into two fields. For platform-emitted events both equal `event_time`. For sanitizer-synthesised hover Enter/Exit, `source_timestamp = triggering_event.timestamp`. **Switch all `VelocityTracker::add_position` callers to `source_timestamp`** — this includes:
  - `drag.rs:231`: `PositionSample::new(event.position, event.timestamp)` → `event.source_timestamp`.
  - `drag.rs:249`: same migration.
  - Any other site found by grep `event\.timestamp` inside `crates/flui-core/src/gesture/recognizers/` — current grep confirms only the two drag sites.

- [ ] **T7:** Update **every** dispatch path to populate `pressure`, `provenance`, `source_timestamp`. **Explicit enumeration of `PointerEvent` struct literals to migrate:**
  1. `dispatch.rs:162-174` — orphan-Cancel synthesis (sanitizer)
  2. `dispatch.rs:241-257` — diff_hover Exit synthesis
  3. `dispatch.rs:260-277` — diff_hover Enter synthesis
  4. `dispatch.rs:translate_mouse_down` — Decision MM applies (mouse-class default `None`, except Force Touch path)
  5. `dispatch.rs:translate_mouse_up` — same
  6. `dispatch.rs:translate_mouse_move` — same
  7. `dispatch.rs:translate_mouse_pressure` (line 419) — Decision MM: `Some(PressureSample { value: e.pressure, min: 0.0, max: 1.0 })`
  8. `dispatch.rs:translate_mouse_exit` — `pressure: None`
  9. `gesture/recognizers/tap.rs::tests::pe()` — synthetic helper (T8 covers)
  10. `gesture/recognizers/double_tap.rs::tests::pe()` — synthetic helper
  11. `gesture/recognizers/long_press.rs::tests::pe()` — synthetic helper
  12. `gesture/recognizers/drag.rs::tests::pe()` — synthetic helper
  13. `gesture/recognizers/scale.rs::tests::pe()` — synthetic helper
  14. **`gesture/arena.rs::tests::pointer_event()` (line 591)** — sixth synthetic helper, used by P-T15.5-A/B/C property tests
  15. Test-support `simulate_*` helpers in `TestAppContext` if they construct PointerEvent literals (verify during T7).
  16. `examples/gesture_arena_bench` — verify after Phase A that the bench compiles.

  **Verification gate at the end of T7:** `cargo check -p flui-core --all-features` must pass.

- [ ] **T8:** Update tests. **Helpers must set `source_timestamp: timestamp` (re-use the same binding, not call `Instant::now()` twice — microseconds may differ).** Bump `pe()` builders in all six locations from T7. Add a unit test `pressure_sample_normalize_correct_for_wacom_range`: `PressureSample { value: 4096.0, min: 0.0, max: 8192.0 }.normalize() == 0.5`.

> **Commit checkpoint A — after T1–T8:** `feat(flui-core)!: PointerEvent surface upgrade — pressure/kind/PanZoomEvent/provenance/timestamp split (S07.5b A)`

### Phase B — Hit-test surface (D5, D6, D7)

- [ ] **T9:** Define `Affine2` in `crates/flui-core/src/geometry.rs` as a custom struct (not `euclid::Transform2D`):

  ```rust
  /// 2D affine transform stored as a 2×3 row-major matrix.
  ///
  /// `[[a, b, tx], [c, d, ty]]` representing
  /// `[x', y'] = [a*x + b*y + tx, c*x + d*y + ty]`.
  #[derive(Copy, Clone, Debug, PartialEq)]
  pub struct Affine2 { pub rows: [[f32; 3]; 2] }

  impl Affine2 {
      pub const IDENTITY: Self = Self { rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] };
      pub fn translation(d: Point<Pixels>) -> Self { /* ... */ }
      pub fn rotation(angle: f32) -> Self { /* ... */ }
      pub fn composed(self, other: Self) -> Self { /* ... */ }
      pub fn inverse(self) -> Option<Self> { /* ... */ }
      pub fn transform_point(self, p: Point<Pixels>) -> Point<Pixels> { /* ... */ }
  }
  ```

  ~50 lines incl. tests. Add explicit `pub use` line to `lib.rs`. **DO NOT** add `euclid` to `[dependencies]` — the workspace's transitive `euclid` via `etagere` is unsuitable as a direct dep without explicit pinning.

- [ ] **T10:** Extend `HitTestEntry` with `transform: Option<Affine2>` (default `None`, identity). Document in rustdoc that `transform` is the affine from window-local to entry-local; `local_position = transform.unwrap_or(IDENTITY).inverse().unwrap().transform_point(position)`. **Constraint for S09:** when paint starts populating real transforms, it must drive `local_position` consistently — recognisers read `local_position`, never `position`, for in-target geometry.

- [ ] **T11:** RAII transform-stack API on `HitTestResult`:

  ```rust
  pub struct HitTestScope<'r> { result: &'r mut HitTestResult }

  impl HitTestResult {
      pub fn push_transform(&mut self, t: Affine2) -> HitTestScope<'_>;
      pub fn push_offset(&mut self, offset: Point<Pixels>) -> HitTestScope<'_>;
  }

  impl<'r> HitTestScope<'r> {
      pub fn add(&mut self, entry: HitTestEntry); // composes top-of-stack into entry.transform
      pub fn push_transform(&mut self, t: Affine2) -> HitTestScope<'_>; // nested
  }

  impl Drop for HitTestScope<'_> { fn drop(&mut self) { /* pops the stack */ } }
  ```

  Internally the result keeps a `SmallVec<[Affine2; 4]>` stack. Unbalanced state is unrepresentable: every push returns a guard, dropping the guard pops. Nested scopes work via `HitTestScope::push_transform` returning a nested guard. **Panic-safety:** unwinding through a scope drops the guard, which pops correctly — no corruption. T19 includes a test that exercises a panic path through a nested scope.

- [ ] **T12:** Wire `Window::hit_test` to use the transform stack. For S07.5b, only `IDENTITY` is pushed (no real transforms yet). One unit test: synthetic `push_transform(rotate_90)`, Down event, manually-added entry has `transform = Some(rotate_90)` and `local_position` correctly inverse-rotated.

- [ ] **T13:** Define `DeliveredEvent<'a>`:

  ```rust
  /// A `PointerEvent` as delivered to a specific recognizer, augmented
  /// with the target's local coordinate for this delivery.
  ///
  /// `event.position` is window-local (constant across recognizers).
  /// `local_position` is hitbox-local (set by dispatcher per recognizer).
  pub struct DeliveredEvent<'a> {
      pub event: &'a PointerEvent,
      pub local_position: Point<Pixels>,
  }
  ```

  **Change `GestureRecognizer::handle_event` signature:**

  ```rust
  fn handle_event(
      &mut self,
      event: DeliveredEvent<'_>,
      window: &mut Window,
      cx: &mut App,
  ) -> GestureDisposition;
  ```

  This is a breaking change to the `GestureRecognizer` trait. Migrate all five recognisers (Tap, DoubleTap, LongPress, Drag-family, Scale) to:
  - Read `event.event.<field>` for everything except slop/distance.
  - Read `event.local_position` for slop/distance/down_position storage.
  - Re-export `DeliveredEvent` from `gesture/mod.rs` and `lib.rs`.

  **Verification gate:** `grep "event\.position" crates/flui-core/src/gesture/recognizers/` must return zero hits after T13. Every reference to `event.position` in a recogniser is a bug.

> **Commit checkpoint B — after T9–T13:** `feat(flui-core)!: Affine2 + HitTestEntry transform + HitTestScope RAII + DeliveredEvent (S07.5b B)`

### Phase C — Recognizer + arena surface (D4, D9, D10, D11)

**Phase C atomicity rule:** T14, T15, T16, and T17 must land as **one atomic commit**. T14 changes the trait method signature; T15 migrates the only impl (LongPress); T16 updates the only call site (`binding.rs:227`); T17 is a no-op verification of `GestureDisposition #[non_exhaustive]`. Between T14 and T16 the tree does not compile. Phase C's commit checkpoint encompasses all four.

- [ ] **T14:** Replace `RecognizerLifecycle::set_arena_back_channel(&mut self, _back_channel: ArenaBackChannel, _entry_index: usize)` with `set_arena_back_channel(&mut self, _pointer_id: PointerId, _back_channel: ArenaBackChannel, _entry_index: usize)`. Same name, three args, default no-op body. Object-safety verified: `PointerId` is `Copy + 'static`, no generics. Update the trait's rustdoc to mention the new pointer_id semantics.

- [ ] **T15:** Migrate `LongPressGestureRecognizer` (`long_press.rs`):
  - `pointer_index: Option<usize>` → `pointer_indexes: SmallVec<[(PointerId, usize); 1]>` (single-shot inline storage; no allocation in the common case).
  - `set_arena_back_channel(pid, bc, idx)` impl pushes `(pid, idx)` and stores `bc`.
  - Timer closure (currently captures `entry_index = self.pointer_index` at `long_press.rs:168`) now captures `pointer_id` (already in scope at line 164) and looks up `(pid, idx)` in `pointer_indexes`. Drop on rejected/cancel clears the entry.
  - Verify: `cargo check -p flui-core` passes after T15 (against T14's new trait shape).

- [ ] **T16:** Update `GestureBinding::register_recognizer` (`binding.rs:227`):

  ```rust
  if lifecycle.needs_back_channel() {
      let back_channel = GestureArenaManager::make_back_channel_from(&self.arena);
      lifecycle.set_arena_back_channel(pointer_id, back_channel, entry_index);
  }
  ```

  Same call site, signature now matches T14. **Also implement Decision D10 here:** check `recognizer.allowed_buttons_filter` (per-recogniser-specific; via a new trait method `GestureRecognizer::allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter>`) **before** `arena.add(pointer_id, recognizer)`. On filter rejection, `register_recognizer` returns early without adding to the arena. This prevents the zombie-arena bug (recogniser added but never enters `add_pointer`).

- [ ] **T17:** Verify `GestureDisposition` is already `#[non_exhaustive]` (`arena.rs:26`). No code change. Rustdoc note added documenting the future-extension contract.

- [ ] **T18 (renamed from earlier T18):** Convert `is_held: bool` to `hold_count: u32` on `GestureArena` (`arena.rs:65`). Update:
  - `GestureArenaManager::hold(pid)` increments.
  - `GestureArenaManager::release(pid)` decrements; saturating-sub to avoid underflow.
  - Sweep gated on `hold_count == 0`, not `!is_held`.
  - All log fields `arena_state = "needs_hold"` etc. switch to `hold_count = N` numeric.
  - The S07.5 P-T15.5-C property test `prop_hold_release_symmetry` extends to verify `hold_count` round-trips correctly under any sequence of `hold`/`release`.

- [ ] **T19:** Define `AllowedButtonsFilter` newtype in `gesture/mod.rs`:

  ```rust
  pub struct AllowedButtonsFilter(Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>);

  impl AllowedButtonsFilter {
      pub fn new(f: impl Fn(PointerButtons, Modifiers) -> bool + 'static) -> Self {
          Self(Box::new(f))
      }
      pub fn call(&self, buttons: PointerButtons, modifiers: Modifiers) -> bool {
          (self.0)(buttons, modifiers)
      }
  }
  ```

  Add `pub allowed_buttons_filter: Option<AllowedButtonsFilter>` field to every existing recogniser (Tap, DoubleTap, LongPress, Pan, HorizontalDrag, VerticalDrag, Scale). Add a fluent builder `with_allowed_buttons_filter(f: impl Fn(...) + 'static)` per recogniser. **The filter check moves to `register_recognizer` per Decision D10** — recognisers no longer check it in `add_pointer`. `add_pointer` continues to gate on `event.buttons.contains(self.button)` only.

  Add a trait method to `GestureRecognizer`:
  ```rust
  fn allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter> { None }
  ```
  Default `None`; each recogniser overrides to return `self.allowed_buttons_filter.as_ref()`.

> **Commit checkpoint C — after T14–T19 atomically:** `feat(flui-core)!: unify back-channel hook + GestureDisposition non_exhaustive + hold_count + AllowedButtonsFilter (S07.5b C)`

### Phase D — Tests, docs, bench, CHANGELOG, ROADMAP

- [ ] **T20:** Update unit tests for every changed surface:
  - `gesture/dispatch.rs::tests` — verify mouse-class events produce `pressure: None` (or `Some(...)` for Force Touch), hover-synthesised events have `provenance: SanitizerSynthesized`, `source_timestamp` propagated.
  - `gesture/recognizers/*::tests` — bump `pe()` helpers; add `*_allowed_buttons_filter_overrides_button` canary per recogniser.
  - `gesture/arena.rs::tests` — bump `pointer_event()` helper; add `prop_hold_count_balance` property test (every hold balanced by exactly one release; sweep iff `hold_count == 0`).
  - Add `prop_long_press_back_channel_round_trips_pointer_id` property test (verifies the `(pid, idx)` lookup in T15's migration).
  - Add transform-stack panic-safety test (T11): unwinding through a `HitTestScope` drops the guard, popping the stack — no corruption.
  - Add `force_touch_macos_path` test: synthetic `MousePressureEvent` translates to `PointerEvent` with `Some(PressureSample { value: 0.5, min: 0.0, max: 1.0 })` (Decision MM lock).

- [ ] **T21:** Sweep rustdoc on every changed type. Files: `gesture/pointer_event.rs`, `gesture/pan_zoom_event.rs` (new), `gesture/recognizer.rs`, `gesture/arena.rs`, `gesture/hit_test.rs`, `gesture/recognizers/*.rs`, `gesture/mod.rs`, `geometry.rs`. **Also fix the documentation lie at `long_press.rs:3`** — module comment says `smol::Timer::after`, body uses `BackgroundExecutor::timer`. **Module-level "S07.5b — completed" subsection** in `gesture/mod.rs` mirrors the S07.5 pattern.

- [ ] **T22:** Update `docs/superpowers/specs/2026-05-08-recognizer-extension.md`:
  - "Adding a new recognizer step-by-step" — `handle_event(event: DeliveredEvent<'_>, ...)` is the new signature; reference `event.local_position` for slop/distance, `event.event.<field>` for everything else.
  - "When to use `RecognizerLifecycle`" — table row for `set_arena_back_channel(pid, bc, idx)` updates the signature.
  - New row: `allowed_buttons_filter` — closure-based extension point for advanced gating.
  - "Threshold-field conventions" — `pressure` thresholds operate on `PressureSample::normalize()`, never raw `value`.
  - LongPress worked example updated to use `SmallVec<[(PointerId, usize); 1]>` storage.

- [ ] **T23:** Introduce `CHANGELOG.md` at workspace root. Keep-a-Changelog format. First entry: "Unreleased — S07.5b breaking changes". One line per breaking change with migration note; cross-references the audit + this plan. Addresses R3 prematurely-but-cheaply.

- [ ] **T24:** Bench regression. `cargo run -p flui-core --release --example gesture_arena_bench`. All three sub-bench budgets pass: `hit_test_8deep < 2 µs`, `arena_tick < 1.25 µs`, `full_frame_120hz < 8 ms p99`. Confirm `PointerEvent`'s growth doesn't push allocation off-stack.

- [ ] **T25:** Update `.ai-factory/ROADMAP.md`: add `S07.5b GestureArena — pre-roster cleanup` between S07.5 and S07.6 entries; add to Completed table on merge. Update `DESCRIPTION.md` Input pipeline bullet.

> **Commit checkpoint D — after T20–T25:** `test(flui-core): S07.5b regression locks + rustdoc + recognizer-extension doc + CHANGELOG + ROADMAP`

## Commit Plan

| Checkpoint | Tasks | Suggested message |
|---|---|---|
| A | T1–T8 | `feat(flui-core)!: PointerEvent surface upgrade — pressure/kind/PanZoomEvent/provenance/timestamp split (S07.5b A)` |
| B | T9–T13 | `feat(flui-core)!: Affine2 + HitTestEntry transform + HitTestScope RAII + DeliveredEvent (S07.5b B)` |
| C | T14–T19 (atomic) | `feat(flui-core)!: unify back-channel hook + GestureDisposition non_exhaustive + hold_count + AllowedButtonsFilter (S07.5b C)` |
| D | T20–T25 | `test(flui-core): S07.5b regression locks + rustdoc + recognizer-extension doc + CHANGELOG + ROADMAP` |

The `!` markers are conventional-commits "breaking change" indicators.

## Review Subagents

Re-invoke after substantial edits:

- **`flui-arch-reviewer`** — before T9 lands (custom Affine2 design), after T11 (HitTestScope ergonomics), after T18 (hold_count rollout), after T19 (allowed_buttons_filter trait method).
- **`rust-api-migration-auditor`** — on every Phase A task (each is a public surface change), T13 (DeliveredEvent trait signature change), T19 (AllowedButtonsFilter struct + trait method).
- **`migration-risk-adversary`** — on Phase A T7 (dispatch.rs enumeration completeness), T15 (LongPress migration), T18 (hold_count migration), T19 (AllowedButtonsFilter zombie-arena fix verification).
- **`wgpu-gpu-reviewer`** — not applicable.

## Acceptance Criteria

1. **`PointerEvent.pressure: Option<PressureSample>` ships.** Mouse-class events return `None` except macOS Force Touch (Decision MM). `PressureSample::normalize()` returns `[0.0, 1.0]` for any valid range. Tests cover Wacom-range, Force-Touch-range, and `MousePressureEvent` migration cases.
2. **`PointerKind::{Trackpad, InvertedStylus, Unknown}` exist** with rustdoc clarifying the Trackpad-vs-Mouse distinction on Windows.
3. **`PointerPanZoomEvent` exists as a sibling type to `PointerSignalEvent`.** `PointerPhase` does NOT have `PanZoom*` variants.
4. **`PointerEvent.provenance: PointerEventProvenance` exists.** Sanitizer-synthesised events (both diff_hover Enter/Exit AND orphan-Cancel synthesis) marked `SanitizerSynthesized`. Future S07.7/S08 consumers find the enum pre-positioned.
5. **`PointerEvent.timestamp` and `PointerEvent.source_timestamp` are distinct fields.** For non-synthesised: equal. `VelocityTracker` consumers (drag.rs:231, drag.rs:249) use `source_timestamp`.
6. **`HitTestEntry.transform: Option<Affine2>` exists.** `Affine2` is a custom struct, no `euclid` direct dep. `HitTestResult` exposes only the `push_transform(t) -> HitTestScope<'_>` RAII API; no exposed `pop_transform`.
7. **`DeliveredEvent<'a>` wrapper is the parameter type for `GestureRecognizer::handle_event`.** Every recogniser reads `event.local_position` for in-target geometry. Verification: zero `event.position` references inside `recognizers/`.
8. **`RecognizerLifecycle::set_arena_back_channel(pid, bc, idx)` is the only back-channel hook.** LongPress migrated. Property test covers round-trip.
9. **`GestureArena.hold_count: u32`** replaces `is_held: bool`. Property test locks balance.
10. **`AllowedButtonsFilter` is a `pub struct` newtype.** Filter check happens in `register_recognizer` BEFORE arena add (Decision D10) — no zombie-arena slot on rejection.
11. **`GestureDisposition` is `#[non_exhaustive]`** (verified, no change required).
12. **`translate_mouse_pressure` (`dispatch.rs:419`)** maps to `Some(PressureSample { value: e.pressure, min: 0.0, max: 1.0 })` — Force Touch on macOS continues to flow correctly.
13. **Phase C lands as one atomic commit** (T14–T19). No intermediate commit has compile errors.
14. **`missing_docs` discipline:** every commit checkpoint compiles cleanly under `#![warn(missing_docs)]`. Doc work is per-task, not batched.
15. **`CHANGELOG.md` exists** with a "Unreleased — S07.5b" entry.
16. **`docs/superpowers/specs/2026-05-08-recognizer-extension.md` updated.** New `DeliveredEvent` signature, unified hook, AllowedButtonsFilter row, normalized-pressure note.
17. **Performance budgets unchanged.** T24 bench confirms.
18. **Verification grep gates pass:**
    - `grep "event\.position" crates/flui-core/src/gesture/recognizers/` → 0 hits.
    - `grep "event\.timestamp" crates/flui-core/src/gesture/recognizers/` → 0 hits (all migrated to `source_timestamp`).
    - `grep "impl RecognizerLifecycle" crates/` → 1 hit (long_press.rs only).
    - `grep "PointerEvent {" crates/flui-core/src/gesture/` → all sites enumerated in T7.

## Risks

- **Pressure-shape change has the widest blast radius.** 12+ `PointerEvent` literal sites, plus `WindowPointerState::last_pressure`. Mitigation: T7 enumerates every site explicitly; verification gate at end of T7 is `cargo check --all-features`. The original audit's T7 was insufficient — this revision lists each literal by file:line.
- **Decision MM (Force Touch via mouse-class) creates a per-platform asymmetry in the pressure-truth contract.** macOS mouse-class can carry real pressure; other platforms cannot. Recognizers that gate on `pressure.is_some()` will activate on macOS Force Touch but not on a Linux/Windows mouse — that's the intent. Mitigation: rustdoc on `PointerEvent.pressure` documents the platform-truth matrix.
- **`HitTestScope` panic-safety.** Unwinding through a scope must drop the guard correctly. Standard Rust RAII guarantee — but worth a panic-safety test (T20).
- **Recogniser `event.local_position` migration risk.** S07.5b sets `local_position == position` everywhere via dispatcher's `transform.unwrap_or(IDENTITY)`. Behaviour unchanged. **But** if a recogniser keeps reading `event.event.position` after the migration, slop checks will be subtly wrong when S09 lands. Mitigation: T13 explicit verification grep `event\.position` returns zero hits in `recognizers/`; T22 doc updates make `local_position` the canonical reference.
- **`Affine2` is a new public type.** Custom math is bug-prone (inversion in particular). Mitigation: T9 includes unit tests for inverse round-trip; flui-arch-reviewer is invoked before T9 lands; if implementation feels error-prone, fall back to `euclid::Transform2D` with explicit `euclid = "0.22"` direct dep — but the custom path is the default.
- **`PanZoomEvent` is a new event-family type.** Currently emitted by no platform; defined for forward-compatibility with macOS native pan-zoom (S20). Risk: gathering moss for a long time before being used. Acceptable — type identity is cheap, public-API stability matters, and adding the type later breaks consumers.
- **`hold_count` migration is invasive in `arena.rs`.** Every site that today reads `is_held` becomes a read of `hold_count == 0` (with inverse). Mitigation: T18 explicitly enumerates the migration sites in `arena.rs`; property test `prop_hold_release_symmetry` extended.
- **Phase C atomicity is a process risk, not a design risk.** Implementer must land T14, T15, T16, T17 (no-op), T18 (hold_count), T19 in one PR commit, not four reviewable ones. Mitigation: checkpoint C explicitly says "atomic"; CI runs against the full checkpoint, not intermediate states.
- **`source_timestamp` propagation through every dispatch path.** Easy to forget. Mitigation: T7 systematically updates every conversion site; T20 tests verify both fields are populated correctly.
- **CHANGELOG.md introduction may collide with future R3 work.** Mitigation: T23 explicitly notes "addresses R3 prematurely-but-cheaply"; future R3 inherits the file.
- **Big PR (~25 tasks).** Reviewers may time out. Mitigation: 4 commit checkpoints split it into reviewable chunks. If reviewers want, split into four sequential PRs (A → B → C → D), but the working assumption is one PR with ordered commits.
- **`source_timestamp`-vs-`timestamp` confusion in tests.** `pe()` helpers must set `source_timestamp: timestamp` (same binding), not `Instant::now()` twice. Mitigation: T8 explicit; ergonomic fix would be a `Default` impl on `PointerEvent` (not added here — `#[non_exhaustive]` precludes external Default; in-crate test helpers handle it).
- **Dependency on `flui-arch-reviewer` for Affine2 design.** T9 says "custom struct" definitively per Decision D5. If the arch-reviewer pivots, the field type on `HitTestEntry` changes — a breaking change inside the same PR, which is acceptable.
- **`gesture_arena_bench` example must compile after Phase A.** Verify before merging A.
