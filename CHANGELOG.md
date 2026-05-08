# Changelog

All notable changes to this workspace are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project (pre-1.0) follows [Semantic Versioning](https://semver.org/)
intent — every breaking change ships with a migration note even though we
have not yet published a numbered release. Cross-references point to the
plan and design docs in `.ai-factory/plans/` and `docs/superpowers/specs/`.

## [Unreleased] — S21 Animation Flutter parity (in progress)

This entry captures the S21 milestone work, which brings `flui-core::animation`
to parity with Flutter's `dart:ui` / `package:flutter/animation.dart` surface.
Plan: `.ai-factory/plans/animation-flutter-parity.md`. Phases land
incrementally; only completed phases appear below.

### Added — `flui-core::animation`

- **S21 phase 0 foundation lands.** `crates/flui-core/src/animation/` gains
  the trait-shaped Flutter-parity surface — purely additive on top of the
  existing `AnimationController` / `Curve` / `Tween` / `Lerp` / simulation
  types, which keep working unchanged.
  * **`Animation<T>` trait** (`animation.rs`): `value`, `status`,
    `add_listener` / `remove_listener` / `add_status_listener` /
    `remove_status_listener`, plus default helpers
    `is_dismissed` / `is_completed` / `is_forward_or_completed`. Object-safe
    by construction (compile-time `_object_safe` pin); marker bound `'static`
    matches the single-threaded UI assumption.
  * **`AnimationStatus`** moved out of `controller.rs` into `status.rs` and
    marked `#[non_exhaustive]` (A8 progress; future phases may add muting /
    behaviour-driven states without a major bump).
  * **Listener mixins** (`listeners.rs`): `LocalListeners`,
    `LocalStatusListeners` (storage with snapshot-during-dispatch,
    Flutter-parity `contains` skip when a callback removes another mid-fire),
    plus `LazyListenable` + `EagerListenable` hook traits.
    `ListenerCallback` / `StatusListenerCallback` are `Rc<dyn Fn>` (cheap to
    clone into a dispatch snapshot); `ListenerId` is an opaque
    `NonZeroU64` (deviation from Flutter's callback-equality model —
    Rust closures have no equality).
  * **`Ticker` family** (`ticker.rs`): `Ticker`, `TickerFuture`,
    `TickerFutureState`, `TickerCanceled`, `TickerProvider`. Wraps an
    `Arc<dyn Clock>` from the active scheduler so both production
    (`RealClock`) and tests (`TestClock`) share the same elapsed-time
    contract — substrate for deterministic animation goldens (T6).
    `TickerFuture` ships as a synchronous status holder; the proper
    `Future` impl is deferred to a phase that has a concrete `await`
    consumer (route transitions).
  * **`AnimationController` wired**: takes its clock from
    `cx.background_executor().scheduler_executor().scheduler().clock()` at
    `attach(cx)` time, stores a `Ticker` for clock injection, implements
    `Animation<f64>` (value widens lossless `f32 -> f64` at the trait
    boundary; inherent `value() -> f32` preserved for backward compat by
    inherent-over-trait resolution). Every state transition fans out to
    raw listeners AND `cx.notify()` so existing `cx.observe` chains keep
    working alongside the new listener model.
  * **`ElementAnimationElement` (S21 phase 0.9)** also migrates off bare
    `Instant::now()` to the same scheduler-clock — element-level
    `with_animation` is now deterministic under `TestClock`.

### Changed — `flui-core` public surface

- **Replaced `pub use animation::*;` glob in `crates/flui-core/src/lib.rs`**
  with a curated explicit re-export list (S21 phase 0.3). Closes one of
  the ~30 globs tracked under roadmap item A2; the animation module's own
  `mod.rs` is the single curator.

### Breaking — `flui-core::animation::Curve`

- **`Curve` is now a TRAIT, not an enum** (S21 phase 1). Concrete types
  per former variant: `Linear`, `EaseIn`, `EaseOut`, `EaseInOut`,
  `EaseInCubic`, `EaseOutCubic`, `EaseInOutCubic`, `Decelerate`,
  `BounceIn`, `BounceOut`, `BounceInOut`, `Cubic{x1,y1,x2,y2}`,
  `ElasticIn{period}`, `ElasticOut{period}`, `ElasticInOut{period}`
  (the parameter-less `Curve::Elastic` is gone — Flutter-parity period is
  now explicit; default 0.4), `Interval{begin,end,curve}`,
  `Threshold(f32)`, `SawTooth(u32)`, `FlippedCurve<C>`, `Reversed<C>`,
  `Split<A,B>`, `CustomCurve(Arc<Fn>)`. The `Curve::Bounce` variant maps
  to `BounceInOut` for closest semantic match. **Migration:**
  `Curve::EaseInOut` → `Curves::EASE_IN_OUT` (catalogue) or `EaseInOut`
  (bare struct). `Curve::Custom(arc)` → `CustomCurve(arc)` or
  `CustomCurve::new(closure)`. No deprecation shim — the rename is
  intentionally clean per the S21 plan.

### Added — `flui-core::animation` (Phase 1)

- **`Curves` catalogue** — empty struct with associated `pub const`s
  mirroring Flutter's `Curves.linear`/`Curves.bounceOut`/etc. surface.
  Covers `LINEAR`, `EASE_IN`, `EASE_OUT`, `EASE_IN_OUT`, `EASE_IN_CUBIC`,
  `EASE_OUT_CUBIC`, `EASE_IN_OUT_CUBIC`, `DECELERATE`, `BOUNCE_IN`,
  `BOUNCE_OUT`, `BOUNCE_IN_OUT`, `ELASTIC_IN`, `ELASTIC_OUT`,
  `ELASTIC_IN_OUT`, `FAST_OUT_SLOW_IN`, `SLOW_MIDDLE`, `EASE` —
  18 entries (will grow as Phase 7 audits the Flutter surface).
  Naming is SCREAMING_SNAKE_CASE per Rust idiom (Flutter
  `Curves.fastOutSlowIn` → `Curves::FAST_OUT_SLOW_IN`).
- **`Curve` trait** — `Send + Sync + 'static`; required
  `transform_internal(t)`; default `transform(t)` clamps `t` to `[0, 1]`;
  optional `derivative_at(t) -> Option<f32>` (analytical for Linear,
  EaseIn, EaseOut, EaseInCubic, EaseOutCubic — used by
  `AnimationController::velocity()` in S21 phase 4 with
  numerical-differentiation fallback); `clone_box(&self)` plus
  `impl Clone for Box<dyn Curve>` make trait objects `Clone`.
- **`CurvedAnimation`** decorator (S21 phase 1.6) — applies a `Curve` to
  a parent `Animation<f64>`, exposes the result as another `Animation<f64>`.
  Forward / reverse curves selected by parent status; subscribes to
  parent's value+status listeners on construction; releases them in `Drop`
  (no dangling notifications). Establishes the listener-forwarding pattern
  Phase 3 combinators reuse.

### Added — `flui-core::animation` (Phase 2)

- **`Animatable<T>` trait** + **`AnimatableExt`** extension trait (S21 phase
  2). Object-safe parametric value-producer; `transform(t: f64) -> T`
  matches Flutter's f64 surface. `chain<P: Animatable<f64>>(self, parent)`
  composes animatables — typically used as
  `Tween::new(0, 100).chain(CurveTween::new(EaseInOut))`. The chained
  output is `ChainedAnimatable<P, C, T>` (no boxing required for
  monomorphized stacks).
- **Tween family**: `Tween<T: Lerp>` (now implements `Animatable<T>` in
  addition to its existing inherent `transform(t: f32)` for backward
  compat), `ConstantTween<T>`, `ReverseTween<T>` (lerp in reverse
  direction), `CurveTween<C>` (`Animatable<f64>` applying a curve to
  `t`), `IntTween` (round-to-nearest), `StepTween` (floor),
  `ColorTween` (`Animatable<Option<Hsla>>` with Flutter-parity
  null-aware lerp — `None` → same-hue transparent endpoint avoids the
  hue-flip artifact a naive lerp-to-zero-color would introduce),
  `SizeTween`, `RectTween`.
- **`TweenSequence<T>` + `TweenSequenceItem<T>`** — weighted segment
  sequence with normalized cumulative boundaries; `O(N)` lookup for
  typical small sequences. `FlippedTweenSequence<T>` runs the underlying
  sequence backward.
- **`Lerp for Bounds<Pixels>`** — composes the existing `Lerp` impls for
  `Point<Pixels>` and `Size<Pixels>`. Required by `RectTween`.

### Added — `flui-core::animation` (Phase 3 combinators)

- **`AlwaysStoppedAnimation<T>`** — constant value, status hard-coded
  to `Forward`, listener add/remove are no-ops (S21 phase 3).
- **`ProxyAnimation<T>`** — runtime parent-swap via `set_parent`.
  Unsubscribes from the old parent's listeners and re-subscribes to
  the new one; fires our own listeners + status listeners on swap so
  consumers re-render against the new value.
- **`ReverseAnimation`** (`Animation<f64>`-only — matches Flutter's
  `ReverseAnimation` shape) — `value()` returns `1.0 - parent.value()`;
  status flipped (`Forward↔Reverse`, `Dismissed↔Completed`); future
  `AnimationStatus` variants pass through unchanged.
- **`CompoundAnimation<F>`** + free constructors **`animation_min`**,
  **`animation_max`**, **`animation_mean`**. Combines two
  `Animation<f64>` parents via a closure `Fn(f64, f64) -> f64`. Status
  priority: `Forward > Reverse > Completed > Dismissed` (a simpler
  approximation of Flutter's `_lastStatus` cache that covers the
  common Min/Max/Mean cases).
- **`TrainHoppingAnimation`** — listens to two `Animation<f64>` parents;
  once their values cross (sign of `first - second` flips), disposes
  the first parent's subscription and behaves identically to the
  second from then on. One-shot hop semantics; constructor returns
  `Rc<Self>` because the value listener captures `Weak<Self>` to avoid
  a self-referential strong cycle. Used by route-transition swaps where
  the destination animation takes over from the source mid-flight.

### Added — `flui-core::animation` (Phase 4 controller polish)

- **`AnimationBehavior`** enum (`Normal` / `Preserve`,
  `#[non_exhaustive]`) — future hook for `MediaQueryData.disableAnimations`
  (S14). `AnimationController::with_behavior(behavior)` builder.
- **`AnimationStyle`** struct (all-`Option` override bag for `duration`,
  `reverse_duration`, `curve`, `reverse_curve`) with chained builders:
  `AnimationStyle::new().with_duration(...).with_curve(...)`.
  `AnimationController::with_style(style)` merges into defaults.
- **`AnimationController::animate_to(target, style, cx)`** — animates
  the controller from its current value to `target`, direction inferred
  (`Forward` if `target >= current`, else `Reverse`); per-call
  duration / curve overrides via `AnimationStyle`.
- **`AnimationController::animate_back(target, style, cx)`** — same
  shape with explicit `Reverse` status (matches Flutter's `animateBack`).
- **`AnimationController::fling(velocity, behavior, cx)`** — drives a
  default-drag `FrictionSimulation`. The behaviour parameter is stored
  for future S14 integration but does not currently affect the
  simulation choice.
- **`AnimationController::velocity()`** — three-branch implementation:
  (1) active simulation `dx`, (2) analytical
  `Curve::derivative_at(t) * span / duration_secs`, (3) numerical
  central-differences fallback with `EPS = 1e-3` (emits a `log::trace!`
  on fallback so profiling can see how often non-analytical curves
  trigger it).
- **`BoundedFrictionSimulation`** — `FrictionSimulation` clamped to a
  `[min, max]` range; `is_done` becomes true once a bound is hit
  (Flutter's `BoundedFrictionSimulation` parity).
- New internal `clear_segment_overrides` helper resets per-segment
  `target_value` / `style_duration` / `style_curve` so prior
  `animate_to` / `with_style` ad-hoc state does not leak into a
  subsequent `forward` / `reverse` / `repeat` / `stop` / `reset` /
  `animate_with` segment.

Deferred to follow-ups: `repeat()` overload with `min` / `max` /
`period` / `reverse` / `count` (existing infinite-repeat case
unchanged); `ClampedSimulation` (generic wrapper following the same
shape as `BoundedFrictionSimulation`).

### Deprecated — `flui-core::elements::animation::easing`

- Legacy free functions `easing::linear`, `easing::quadratic`,
  `easing::ease_in_out`, `easing::ease_out_quint`, `easing::bounce`,
  `easing::pulsating_between` (S21 phase 1.8b). Migrate to
  `Curves::*.transform(t)` where a direct catalogue equivalent exists;
  for `bounce` the closest match is `Curves::BOUNCE_IN_OUT`; for
  `ease_out_quint` and `pulsating_between` build a `CustomCurve`.
  `ElementAnimation::new`'s default `easing` field now uses an inline
  `|t| t` closure to avoid deprecation warnings on the default itself.

### Breaking — `flui-core::elements::animation`

- **`pub struct Animation` renamed to `ElementAnimation`** (S21 phase 0a).
  Frees the bare `Animation` symbol at the crate root for the new
  Flutter-parity `Animation<T>` trait that lands in
  `flui_core::animation` in phase 0. The `AnimationExt` trait keeps its
  name; only the struct it produces is renamed (also: `AnimationElement<E>`
  -> `ElementAnimationElement<E>`). **Migration:** replace
  `flui_core::Animation` with `flui_core::ElementAnimation` and
  `Animation::new(duration)` with `ElementAnimation::new(duration)`. No
  deprecated re-export shim — the rename is intentionally clean to keep
  `Animation` reserved for the trait-shaped API.

## [Unreleased] — S07.5b GestureArena pre-roster cleanup

This entry captures the breaking-friendly cleanup that lands ahead of
S07.6 (recognizer roster expansion). Plan: `.ai-factory/plans/feature-s07-5b-pre-roster-cleanup.md`.
Audit rationale: `docs/superpowers/specs/2026-05-08-flutter-gestures-architectural-audit.md`.

### Breaking — `flui-core::gesture`

- **`PointerEvent.pressure: f32` -> `PointerEvent.pressure:
  Option<PressureSample>`** (S07.5b T1, T2). Mouse-class events
  default to `None`. macOS Force Touch via `MousePressureEvent`
  surfaces `Some(PressureSample { value, min: 0.0, max: 1.0 })`.
  Recognizers must compare against `PressureSample::normalize()` for
  device-agnostic threshold semantics, never the raw `value`. **Migration:**
  any consumer reading `event.pressure` (an `f32`) now sees
  `Option<PressureSample>`; replace `event.pressure` with either
  `event.pressure.map(|p| p.normalize()).unwrap_or(0.0)` for a
  normalized scalar or `event.pressure.is_some()` for "pressure is
  available."
- **`PointerEvent.synthesized: bool` removed; `PointerEvent.provenance:
  PointerEventProvenance` introduced** (T5). The `#[non_exhaustive]`
  enum has variants `Platform` (default) and `SanitizerSynthesized`,
  with future `ResamplerSynthesized` (S07.7) and `SemanticsSynthesized`
  (S08) reserved. **Migration:** `event.synthesized` becomes
  `matches!(event.provenance, PointerEventProvenance::SanitizerSynthesized)`,
  but new code should branch on the enum directly so future variants
  are caught at the use site.
- **`PointerEvent.timestamp: Instant` splits into `timestamp` +
  `source_timestamp`** (T6). For non-synthesised events the two are
  equal. `VelocityTracker` consumers (drag recognizer at
  `drag.rs:231` and `drag.rs:249`) read `source_timestamp`. **Migration:**
  any code feeding pointer event timestamps into a velocity- or
  resampler-aware path should switch to `source_timestamp`; for
  ordinary "when did this fire on the wall clock" reads, keep
  `timestamp`.
- **`PointerKind` gains three variants** (T3): `Trackpad`,
  `InvertedStylus`, `Unknown`. `PointerKind` was already
  `#[non_exhaustive]`, so a `match` without a wildcard arm fails to
  compile until the new variants are handled.
- **New `PointerPanZoomEvent` sibling type** (T4) with
  `#[non_exhaustive] PanZoomPhase { Start, Update, End }`. **Not** a
  set of `PointerPhase` variants — pan-zoom carries a rich
  pan/scale/rotation tuple that does not fit on every PointerEvent
  without 3 always-empty `Option` fields. The platform layer is not
  yet wired to emit these (S20 work).
- **`HitTestEntry.transform: Option<Affine2>`** (T10) records the
  target-local-to-window-local affine for each hit-test entry
  (Flutter `local → window` convention). The dispatcher inverts and
  applies it once per delivery to recover the per-target
  `local_position`; recognizers consume the result through
  [`DeliveredEvent::local_position`] and never invert directly.
  `Affine2` is a bespoke 2x3 row-major primitive in
  `flui_core::geometry` (`IDENTITY`, `translation`, `rotation`,
  `composed`, `inverse`, `transform_point`); no `euclid` direct
  dependency.
- **`HitTestResult` push API now RAII-only** (T11). `push_transform(t)
  -> HitTestScope<'_>` and `push_offset(offset) -> HitTestScope<'_>`
  return guards; `Drop` pops. `HitTestResult::push` is no longer
  exposed (`pub(crate)` removal). Add entries via
  `HitTestScope::add(entry)`. Unbalanced push/pop is a borrow-check
  error; panic-safety follows from standard RAII.
- **`GestureRecognizer::add_pointer` and `handle_event` take
  `DeliveredEvent<'_>` instead of `&PointerEvent`** (T13). The
  wrapper exposes `event.local_position` (target-local) plus
  accessor methods (`event.global_position()`, `event.kind()`,
  `event.phase()`, `event.buttons()`, `event.timestamp()`,
  `event.source_timestamp()`, `event.provenance()`,
  `event.pressure()`, `event.modifiers()`, `event.delta()`). **Migration:**
  recognizer impls must be updated to take `DeliveredEvent<'_>`,
  read `event.local_position` for slop / distance / down_position,
  and replace any `event.position` access with
  `event.global_position()`.
- **`RecognizerLifecycle::set_arena_back_channel(bc, idx)` removed**;
  the only hook is `set_arena_back_channel(pointer_id, bc, idx)` —
  same name, three arguments (T14). `LongPressGestureRecognizer`
  migrates from `pointer_index: Option<usize>` to `pointer_indexes:
  SmallVec<[(PointerId, usize); 1]>` (T15). **Migration:** the only
  external impl is `LongPressGestureRecognizer`; downstream
  recognizers that overrode the previous shape must add the
  `pointer_id: PointerId` parameter and key any per-pointer storage
  on it.
- **`GestureArena.is_held: bool` -> `GestureArena.hold_count: u32`**
  (T18). `hold` increments, `release` decrements (saturating-sub),
  sweep gates on `hold_count == 0`. Consumers that read this private
  field via crate-internal access must switch to `hold_count > 0`
  for "is held"-style reads.
- **`AllowedButtonsFilter` newtype + per-recognizer `pub
  allowed_buttons_filter: Option<AllowedButtonsFilter>` field** on
  every shipping recognizer (T19). Construction via
  `AllowedButtonsFilter::new(closure)`, evaluation via `call(buttons,
  modifiers)`. Per-recognizer fluent builder
  `with_allowed_buttons_filter(closure)`. The trait method
  `GestureRecognizer::allowed_buttons_filter() ->
  Option<&AllowedButtonsFilter>` defaults to `None`.
- **`GestureBinding::register_recognizer` signature change** (T16):
  the function now takes `buttons: PointerButtons` and `modifiers:
  Modifiers` from the dispatcher and evaluates
  `recognizer.allowed_buttons_filter()` *before* `arena.add`
  (Decision D10). Rejecting recognizers never enter the arena.
  `register_recognizer` is `pub(crate)`; the only caller is the
  Window-side dispatcher.

### Added

- `flui_core::Affine2` — 2x3 row-major affine transform primitive.
- `flui_core::HitTestScope` — RAII guard returned by
  `HitTestResult::push_transform` / `push_offset`.
- `flui_core::DeliveredEvent` — wrapper for per-recognizer event
  delivery with `local_position`.
- `flui_core::AllowedButtonsFilter` — newtype gating predicate
  evaluated before arena admission.
- `flui_core::PointerPanZoomEvent`, `flui_core::PanZoomPhase` — sibling
  pan-zoom event family (definition only; platform plumbing is
  deferred to S20).
- `flui_core::PointerEventProvenance` — provenance enum.
- `flui_core::PressureSample` — pressure value with raw min/max range
  plus `normalize() -> f32`.
- New `with_allowed_buttons_filter` builder on every recognizer family
  (Tap, DoubleTap, LongPress, Pan, HorizontalDrag, VerticalDrag,
  Scale).

### Fixed

- `translate_mouse_pressure` (`dispatch.rs:419`) preserves macOS
  Force Touch pressure as `Some(PressureSample { value, min: 0.0,
  max: 1.0 })` — Decision MM (T2 / T7). Mouse-class events from
  non-pressure devices remain `None`.
- Hit-test transform stack invariant is structurally enforced by
  the `HitTestScope` guard rather than a debug-assert; unbalanced
  push/pop is now a borrow-check error.
- Arena hold counter prevents `release` from one recognizer
  silently clearing another's hold on the same pointer (motivates
  S07.6 MultiTap + DoubleTap coexistence).
- `LongPressGestureRecognizer::rejected` clears the pointer's entry
  slot in `pointer_indexes`, matching the per-pointer storage
  shape.

### Documentation

- `docs/superpowers/specs/2026-05-08-recognizer-extension.md`
  updated for the new `DeliveredEvent` signature, the unified
  `set_arena_back_channel(pid, bc, idx)` hook, the
  `allowed_buttons_filter` row, and the normalized-pressure
  threshold rule.
- `crates/flui-core/src/gesture/mod.rs` module-level rustdoc gains
  an "S07.5b — completed" subsection mirroring the S07.5 entry.
- `crates/flui-core/src/gesture/recognizers/long_press.rs` module
  comment now reflects the actual `BackgroundExecutor::timer` /
  per-pointer `pointer_indexes` shape (the previous comment claimed
  `smol::Timer::after`, contradicting the code).
