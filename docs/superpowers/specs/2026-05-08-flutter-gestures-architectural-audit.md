# Flutter `gestures` package — architectural audit for flui-v2

**Status:** completed audit; identified follow-up work. Not scheduled, not implemented.
**Source:** Flutter API documentation at https://api.flutter.dev/flutter/gestures/ (catalog + sub-pages: GestureArenaManager, GestureArenaTeam, OneSequenceGestureRecognizer, PrimaryPointerGestureRecognizer, MultiTapGestureRecognizer, MultiDragGestureRecognizer, TapAndPanGestureRecognizer, ForcePressGestureRecognizer, EagerGestureRecognizer, PointerEventResampler, VelocityTracker, MediaQueryData.gestureSettings, DeviceGestureSettings, dart:ui GestureSettings) plus engine PRs #27836 / #60558 / #66745 / framework PR #161549, breaking-change docs for trackpad gestures and default scroll behaviour.
**Date:** 2026-05-08.
**Audience:** future contributors planning S07.7+, S08, S09+, S11, S13, S14, A8.

This document records architectural nuances of Flutter's gesture system that flui-v2 either does not yet cover or covers in a way that may prove costly later. The goal is to lock the long-term direction *before* the public API is published to crates.io — most recommendations are breaking-friendly only because we have not committed to backwards-compatibility yet.

This audit complements:
- `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md` — the original S07 design.
- `docs/superpowers/specs/2026-05-08-recognizer-extension.md` — the contributor recipe for new recognisers.
- `docs/superpowers/specs/2026-05-08-gesture-roadmap-from-old-impl.md` — the v1 → v2 comparative review (which identifies S07.6 / S07.7 / S07.8 follow-up).
- `.ai-factory/plans/feature-s07-6-recognizer-roster.md` — the in-progress S07.6 plan (MultiTap + ForcePress).

---

## 1. Catalog of Flutter `gestures` (compact, by group)

**Binding & dispatch**
- `GestureBinding` (mixin on `BindingBase`) — singleton hub, owns `pointerRouter`, `gestureArena`, `samplingClock`, `resamplingEnabled`, drives `handlePointerEvent` → `hitTest` → `dispatchEvent`.
- `PointerRouter` — global routing table, `addRoute`/`addGlobalRoute`/`route` with optional `Matrix4` per route.
- `HitTestable`, `HitTestDispatcher`, `HitTestTarget`, `HitTestEntry`, `HitTestResult` (with transform stack: `pushTransform`/`pushOffset`/`popTransform`).
- `HitTestBehavior` (`opaque` | `translucent` | `deferToChild`).

**Pointer events**
- Base `PointerEvent` + subtypes: `PointerAddedEvent`, `PointerRemovedEvent`, `PointerHoverEvent`, `PointerEnterEvent`, `PointerExitEvent`, `PointerDownEvent`, `PointerMoveEvent`, `PointerUpEvent`, `PointerCancelEvent`.
- `PointerSignalEvent` family (parent class): `PointerScrollEvent`, `PointerScrollInertiaCancelEvent`, `PointerScaleEvent`.
- `PointerPanZoom*Event` family (macOS trackpad): `PointerPanZoomStartEvent`, `PointerPanZoomUpdateEvent`, `PointerPanZoomEndEvent`.
- `PointerDeviceKind` (`touch | mouse | stylus | invertedStylus | trackpad | unknown`).
- `PointerEventResampler` — frame-aligned sampling with `samplingOffset` (negative = look-back).

**Arena**
- `GestureArenaManager` (`add` / `close` / `sweep` / `hold` / `release`).
- `GestureArenaEntry` (`resolve(GestureDisposition)`).
- `GestureArenaMember` (interface with `acceptGesture(pointer)` / `rejectGesture(pointer)`).
- `GestureArenaTeam` (captain-deferred resolution; team has optional captain `GestureArenaMember`).
- `GestureDisposition` (`accepted | rejected`).

**Recognizer base hierarchy**
- `GestureRecognizer` (abstract; `addPointer`, `addAllowedPointer`, `addAllowedPointerPanZoom`, `isPointerAllowed`, `getKindForPointer`, `dispose`, `debugOwner`, `supportedDevices`, `allowedButtonsFilter`).
- `OneSequenceGestureRecognizer` (track-pointer + arena-resolution + `team`).
- `PrimaryPointerGestureRecognizer` (single primary pointer + `deadline` + `preAcceptSlopTolerance` + `postAcceptSlopTolerance`).
- `EagerGestureRecognizer` (accept-on-down, blocks siblings).
- `GestureRecognizerFactory<T>` + `GestureRecognizerFactoryWithHandlers<T>` (initializer pattern for `RawGestureDetector`).

**Tap family**
- `BaseTapGestureRecognizer`, `TapGestureRecognizer`, `MultiTapGestureRecognizer`, `DoubleTapGestureRecognizer`, `SerialTapGestureRecognizer`.

**Long-press**
- `LongPressGestureRecognizer` — primary/secondary/tertiary callback sets, drag-after-press via `LongPressMoveUpdateDetails`, end with velocity via `LongPressEndDetails.velocity`.

**Drag family**
- `DragGestureRecognizer` (sealed abstract), `MonoDragGestureRecognizer`, `HorizontalDragGestureRecognizer`, `VerticalDragGestureRecognizer`, `PanGestureRecognizer`.
- `BaseTapAndDragGestureRecognizer`, `TapAndDragGestureRecognizer`, `TapAndPanGestureRecognizer`, `TapAndHorizontalDragGestureRecognizer` (consecutive-tap counter, used by text selection).
- `DragStartBehavior` (`down | start`).
- `DragStartDetails`, `DragUpdateDetails`, `DragEndDetails` (carry `sourceTimeStamp`, `kind`, `globalPosition`, `localPosition`, `primaryDelta`, `velocity`, `primaryVelocity`).

**Multi-drag family**
- `MultiDragGestureRecognizer` (abstract), `ImmediateMultiDragGestureRecognizer`, `DelayedMultiDragGestureRecognizer`, `HorizontalMultiDragGestureRecognizer`, `VerticalMultiDragGestureRecognizer`.
- `MultiDragPointerState` (per-pointer drag state; subclass-customizable).
- `Drag` interface (returned from `onStart` to receive `update`/`end`/`cancel`).

**Scale & force**
- `ScaleGestureRecognizer` + `ScaleStart/Update/EndDetails`.
- `ForcePressGestureRecognizer` + `ForcePressDetails` (interpolates raw `pressureMin..pressureMax` via custom `interpolation` callback).

**Velocity & physics support**
- `VelocityTracker`, `VelocityTracker.withKind`.
- `IOSScrollViewFlingVelocityTracker`, `MacOSScrollViewFlingVelocityTracker` (platform-tuned LSQ smoothing).
- `Velocity`, `VelocityEstimate`.

**Settings & semantics**
- `dart:ui GestureSettings { physicalTouchSlop, physicalDoubleTapSlop }` (engine-supplied platform default).
- `DeviceGestureSettings` (gestures-layer wrapper; `MediaQueryData.gestureSettings` carries it).
- `SemanticsGestureDelegate` + `RawGestureDetectorState.replaceGestureRecognizers` — accessibility plumbing.

---

## 2. Gap analysis vs flui-v2

| Flutter entity | v2 equivalent | Status |
|---|---|---|
| `GestureBinding` (singleton) | `GestureBinding` (per-window) | Have, intentional divergence |
| `PointerRouter` (global) | direct dispatch via `gpui` per-window event loop | Intentionally rejected |
| `GestureArenaManager` (`hold`/`release`/`sweep`) | `GestureArenaManager` with `merge_by_pointer_id`, hold/release | Have |
| `GestureArenaEntry.resolve` | implicit via `ArenaBackChannel(idx)` | Have, surface differs |
| `GestureArenaTeam` (captain) | `GestureArenaTeam` (struct exists, not public, captain unused) | Partial — needs S11/S13 surface |
| `GestureArenaMember` interface | merged into `GestureRecognizer` trait | Have |
| `GestureDisposition` | `GestureDisposition::{Possible, Accepted, Rejected}` | Have (extra `Possible`) |
| `OneSequenceGestureRecognizer` (base) | no analog — every recognizer is direct `dyn GestureRecognizer` | Missing (deliberate?) |
| `PrimaryPointerGestureRecognizer` (base) | no analog — Tap/LongPress duplicate primary-pointer logic | Missing |
| `EagerGestureRecognizer` | none | Missing — needed for native-view embedding |
| `GestureRecognizerFactory` | fluent builder via `__internal_on_*` | Different model (see §3.15) |
| `TapGestureRecognizer` | `Tap` recognizer | Have |
| `DoubleTapGestureRecognizer` | `DoubleTap` recognizer | Have |
| `MultiTapGestureRecognizer` | — | S07.6 planned |
| `SerialTapGestureRecognizer` | — | Missing |
| `LongPressGestureRecognizer` | `LongPress` | Have, but no secondary/tertiary, no end-velocity |
| `DragGestureRecognizer` family | `Pan`/`HorizontalDrag`/`VerticalDrag` | Have |
| `MonoDragGestureRecognizer` (base) | no analog | Missing |
| `MultiDragGestureRecognizer` family | — | Deferred (no S07.x slot yet) |
| `MultiDragPointerState` + `Drag` interface | — | Missing |
| `TapAndDrag*` family | — | Deferred (S13 text selection seam) |
| `ScaleGestureRecognizer` | `Scale` | Have |
| `ForcePressGestureRecognizer` | — | S07.6 planned |
| `PointerEventResampler` | — | S07.7 deferred |
| `VelocityTracker` | `VelocityTracker` (Flutter LSQ port) | Have, single variant |
| iOS/macOS fling velocity trackers | — | Missing |
| `PointerSignalEvent` (Scroll/Scale) | `PointerSignalEvent::{Scroll, Magnify}` | Have (subset) |
| `PointerPanZoom*Event` (trackpad) | — | Missing — only `Magnify`, no PanZoom |
| `PointerDeviceKind::trackpad` | — | Missing |
| `PointerDeviceKind::invertedStylus` | — | Missing |
| `PointerDeviceKind::unknown` | — | Missing |
| `pressureMin`/`pressureMax` (raw range) | normalized `pressure: f32` | Lossy (see §3.13) |
| `radiusMajor`/`radiusMinor`/`size`/`distance`/`distanceMax` | — | Missing |
| `viewId`/`embedderId`/`platformData`/`obscured`/`synthesized`/`original` | — | Mostly missing (per-window model covers `viewId`) |
| `PointerEvent.transform` (Matrix4 + `original`) | — | Missing — no transform-aware hit-test |
| `HitTestResult` transform stack (`pushTransform`/`pushOffset`) | flat `SmallVec<HitTestEntry>` | Missing — see §3.4 (critical for rotated targets) |
| `HitTestEntry.transform` for local-coord mapping | only `position` (window-local) | Missing |
| `DragStartBehavior` (start \| down) | — | Missing (see §3.7) |
| `AllowedButtonsFilter` (functional filter) | `pub button: PointerButtons` | Surface mismatch (see §3.8) |
| `dart:ui GestureSettings.physicalTouchSlop` | logical `Pixels` only | Missing — no physical-pixel path |
| `DeviceGestureSettings` (override) | mutable `GestureSettings` on Window | Different model |
| `SemanticsGestureDelegate`, `replaceGestureRecognizers` | `SemanticAction` enum (S08 stub) | Stub only |
| `LongPressMoveUpdateDetails`, `LongPressEndDetails.velocity` | `LongPressDetails` (no velocity?) | Likely partial |
| `consecutiveTapCount` (tap-and-drag) | — | Missing (S13 prereq) |

---

## 3. Architectural nuances (what v2 risks missing)

### 3.1 Recognizer roster after S07.6
Even after S07.6 ships `MultiTap` and `ForcePress`, v2 will be missing:
- The whole **MultiDrag** family (`Immediate`, `Delayed`, `Horizontal`, `Vertical`MultiDrag) plus `MultiDragPointerState` + the `Drag` interface returned from `onStart`. This powers Reorderable, drag-handles, and any "n fingers each dragging an independent target" use case.
- **`EagerGestureRecognizer`** — required for native-view embedding (S20 desktop-gaps / future webview embed). Trivial to add; should not wait for a milestone.
- **`SerialTapGestureRecognizer`** — emits `onSerialTap` with consecutive count without committing to "double-tap-only" semantics (the building block under `TapAndDrag`).
- **`TapAndDrag*` family** — *the* text-selection primitive in modern Flutter (replaces the old `TextSelectionGestureDetector` recognizer split). Without it, S13 text parity will reinvent it.
- **`MonoDragGestureRecognizer`** as a shared base — the place where the axis-locked drag math lives so `HorizontalDrag`/`VerticalDrag` don't duplicate.

Open question: should v2 expose `OneSequenceGestureRecognizer` / `PrimaryPointerGestureRecognizer` as crate-internal helper traits, or keep recognizers monolithic? Today the five built-ins each carry their own arena-tracking + slop-tolerance code; adding force-press/multi-tap/multi-drag will quadruple that duplication. Recommendation: extract a `PrimaryPointerState` helper struct (not a trait — Rust trait inheritance is the v1 anti-pattern we already rejected), parameterized by deadline + slop, used internally by Tap/LongPress/ForcePress.

### 3.2 Arena nuances
Flutter's resolution rule is exact: *"the first member to accept or the last member to not reject wins"*. Three subtleties v2 needs to honour:

- **`hold`/`release` is a counter, not a boolean.** Multiple holds stack. v2's `schedule_arena_release(timeout)` must compose: a recognizer holding for double-tap and a sibling holding for force-press both contribute, and sweep waits for both releases.
- **`sweep` only fires on `close` after no member accepted *and* hold-count is zero.** v2's per-pointer arena-hold timer must call `release` (decrement) — never directly `sweep`. The current S07.5 surface looks right; just make sure `ArenaBackChannel` exposes only `accept`/`reject`/`hold`/`release`, never `sweep` — sweep is the manager's prerogative.
- **Team captain resolution rule.** Without captain, the **first added member** auto-wins when external competitors clear; with captain, the captain wins on either condition (any member claims OR all externals out). The captain itself is allowed to be a no-op recognizer that "never explicitly claims" (AndroidView pattern). v2's `GestureArenaTeam` struct should keep the optional-captain field and surface this exact two-mode behaviour — do not simplify to "always has captain".
- v2's extra `GestureDisposition::Possible` is fine but should be treated as *internal to the recognizer state machine*, never a value `ArenaBackChannel` can route to the manager — Flutter's manager only knows `accepted | rejected`. Keep the wire-protocol two-valued.

### 3.3 PointerRouter / GestureBinding ownership
Flutter's `PointerRouter` is a global table because Flutter's `PlatformDispatcher` delivers a flat event stream and gestures need pre-arena per-pointer subscription (`addRoute(pointer, callback)`). It exists to let any recognizer say "I want every event for pointer N from now on, regardless of hit-test result" — `OneSequenceGestureRecognizer.startTrackingPointer` is built on it.

v2's per-window `gpui` model already gives every recognizer the per-window event stream, and recognizers decide per-event whether to act. **What v2 risks losing**: the *post-hit-test, pre-arena* "I'm following this pointer even after it leaves my hitbox" subscription. Today our recognisers are kept alive by being attached to a hit-test target; if the pointer leaves that target mid-gesture, do we still get `Move`/`Up`? Verify in `dispatch.rs` — once a pointer is captured by a recognizer, subsequent events for that `pointer_id` should bypass hit-testing and route directly. If they don't, we have a Flutter-router-shaped hole.

Cross-window gesture handoff (drag from one window into another) is **not** something Flutter solves — its singleton router is bound to the engine's single `FlutterView` history. v2's per-window model loses nothing real here.

### 3.4 HitTestBehavior + transform stack
This is the **single biggest hidden gap**. Flutter's `HitTestResult` is not a flat list — it's a stack with `pushTransform(Matrix4)` / `pushOffset(Offset)` / `popTransform()`. Each `HitTestEntry` carries a transform that lets recognisers map global pointer positions back into **local target coordinates** for arbitrary rotated/scaled/skewed parents. The relevant invariant: `entry.localPosition = entry.transform.invert() * event.position`.

v2's `HitTestEntry` carries only window-local `position: Point<Pixels>`. Today this works because our hitboxes are axis-aligned rectangles. The moment a `Transform` widget (S09 layers/canvas territory) interposes a rotation between a recogniser and the pointer, recognisers will see *global* coordinates while the user expects *target-local* — drag deltas, slop checks, and hit-testing of nested children all break.

This is breaking-friendly to fix now: extend `HitTestEntry` with `transform: Option<Mat4>` (or per-entry `local_position`), and have `Window::hit_test` walk the transform stack the way Flutter does. Flutter's `Matrix4` inversions are cached in `MatrixUtils.transformPoint`; v2 should do the same to keep slop checks O(1).

### 3.5 Pointer event lifecycle
v2 covers the basics but is missing several event kinds that *will* matter:
- **`PointerPanZoom{Start,Update,End}Event`** — macOS trackpad two-finger pan/scale (distinct from `Magnify`/scroll). The Flutter trackpad-gestures breaking-change doc is explicit: trackpad pan-zoom now drives `ScaleGestureRecognizer` directly, not a bypass channel. Without these events, scale-on-trackpad works only via wheel-scaling, not native pinch.
- **`PointerScrollInertiaCancelEvent`** — fired when momentum scrolling is interrupted. iOS scroll physics needs this to cancel a fling.
- **`PointerCancelEvent`** distinct from `Up` — v2 has `PointerPhase::Cancel` so we cover this in shape, but verify recognisers actually treat Cancel as "drop the gesture, do not fire `onTap`/`onLongPress`" rather than "treat as Up".
- **`PointerDeviceKind::invertedStylus`** — pen flipped to eraser side. Tablet-app users notice. Add as a `PointerKind` variant.
- **`PointerDeviceKind::trackpad`** — kind for the synthetic device that emits PanZoom events. Different from `Mouse` because pressure/wheel semantics differ.
- **`PointerDeviceKind::unknown`** — Flutter's escape hatch when the platform doesn't tell us. Mapping unknown → `Mouse` (today's implicit default) means recognisers can't refuse unknown devices. Add it.

Per-pointer **`viewId`/`embedderId`/`obscured`/`synthesized`/`original`** are mostly Flutter-engine bookkeeping; v2's per-window model owns the equivalent of `viewId`, the rest is internal to its sanitizer.

### 3.6 Drag details niceties
Flutter's `DragStartDetails` carries `sourceTimeStamp` (when the underlying `PointerDownEvent` happened, **not** when the recogniser fired — this matters for `VelocityTracker` re-seeding). Verify v2's `DragStartDetails` has the equivalent: a `Instant` representing the originating pointer event, not `Instant::now()` at callback dispatch.

`primaryDelta` is `Option<Pixels>` (None for `PanGestureRecognizer`, Some for axis-locked variants). v2 should expose the same — exporting full `delta: Point<Pixels>` and a separate `primary_delta: Option<Pixels>` keeps surface explicit.

### 3.7 DragStartBehavior
Flutter's `DragStartBehavior` (`down | start`) toggles whether `DragStartDetails.globalPosition` is the initial Down position or the position at slop-threshold-crossing. iOS-style rubber-banding scrollers need `down` (so the surface tracks the finger from contact, not from when drag is recognised). Default in Flutter is `start`; iOS scrollables override to `down`.

v2 ships neither today. This is a one-field-on-builder addition. Without it, S11 scroll physics will fight the recogniser.

### 3.8 AllowedButtonsFilter
Flutter's `allowedButtonsFilter: bool Function(int buttons)` is a *function*, not a bitmask, because the test isn't always "is bit X set" — middle-click-pan filters on `kMiddleMouseButton == buttons` (exact equality, no other buttons), but two-finger drag-with-modifier filters on `(buttons & kPrimary) != 0 && shiftPressed`.

v2's `pub button: PointerButtons` field is fine for the common case but loses the closure form. Recommendation: keep `button` as the simple builder API, but add an opt-in `allowed_buttons_filter: Option<Box<dyn Fn(PointerButtons, Modifiers) -> bool>>` for advanced cases. This ships *with* S07.6 force-press because force-press needs to reject mouse explicitly.

### 3.9 Per-platform velocity trackers
`IOSScrollViewFlingVelocityTracker` and `MacOSScrollViewFlingVelocityTracker` exist because Flutter's iOS/macOS scroll physics demand specific smoothing coefficients to match UIKit/AppKit fling behaviour. The base `VelocityTracker` is a generic LSQ; the iOS variant adds asymmetric weighting on the most recent samples to match `_UIScrollViewFlingMomentum`. Without it, scroll-fling on iOS *feels wrong* in side-by-side comparison with native apps.

v2 has one `VelocityTracker`. For S11 scroll physics parity, plan a `VelocityTracker` *trait* (with `add_position`/`get_velocity`/`get_velocity_estimate`) plus three implementations selected per-platform. Don't make it a giant enum — the LSQ math diverges enough that polymorphism is cleaner. Keep `with_kind(PointerKind)` as the Flutter ergonomic.

### 3.10 Text selection
S13 text parity should ride **`TapAndDrag*`** + a small `consecutiveTapCount` state machine. Flutter no longer exposes a separate `TextSelectionGestureRecognizer` — text widgets compose `TapAndPanGestureRecognizer` + `LongPressGestureRecognizer` in a `RawGestureDetector`, with a `GestureArenaTeam` (no captain) so drag wins immediately once tap loses. Plan S13 with this exact composition; do not invent a v2-specific monolithic `TextSelectionRecognizer`.

### 3.11 LongPress details
Flutter's `LongPressEndDetails` carries `velocity: Velocity` — needed for "swipe-after-long-press" interactions (drag-to-rearrange in lists). Verify v2's `LongPressDetails` includes velocity *at end* (move-update can also expose `offsetFromOrigin` which v2 should mirror). Also: secondary/tertiary callback sets (`onSecondaryLongPress*`, `onTertiaryLongPress*`) are real-world requirements for desktop right-click-and-hold. v2 currently single-button; add `button: PointerButtons` parameterisation.

### 3.12 GestureDisposition lifecycle
Flutter is intentionally minimal: `accepted | rejected`. The only "in-between" state is the implicit "still in the arena" — represented by the absence of a resolution. v2's `GestureDisposition::{Possible, Accepted, Rejected}` is fine **iff** `Possible` is recogniser-internal and the wire to `ArenaBackChannel` is two-valued.

Open question: do we need `Accepted-but-deferred` for the captain-deferred case? Answer: no — that's what `hold` is for. Captain accept-on-team-behalf is just `member.entry.resolve(Accepted)` routed through the team's captain-aware logic. Don't grow the enum.

### 3.13 Force/pressure semantics
Flutter's `PointerEvent.pressure` is **raw platform pressure**, with `pressureMin`/`pressureMax` exposing the platform's range. `ForcePressGestureRecognizer.startPressure` (default 0.4) is a normalised threshold against `(pressure - pressureMin) / (pressureMax - pressureMin)`, *not* against `pressure` directly. v2's normalised-on-platform-side `pressure: f32` collapses the range and means: a Wacom pen with 8192 raw levels and a Force Touch trackpad with `[0.0, 1.0]` raw range produce identical normalised values, but the *meaning* of "0.4" differs (a Wacom pen's 40% is a light press; a Force Touch's 40% is the deep-press threshold). The S07.6 force-press recogniser using a fixed `0.4` constant will misfire on stylus.

Two fixes, both breaking-friendly:
1. Replace `pressure: f32` with `pressure: Option<PressureSample>` where `PressureSample { value: f32, min: f32, max: f32 }` (carry the platform range; mouse-class events return `None`). Recognisers normalise in their own thresholds.
2. Or split: keep `pressure: f32` (always 0..=1, always available) but add `pressure_range: Option<(f32, f32)>` for stylus/touch-with-force. Recognisers check the range to decide whether `pressure` is meaningful.

(1) is cleaner; (2) is more compatible with current call sites. Do (1) before S07.6 ships.

### 3.14 GestureRecognizer.dispose
Flutter's mandatory `dispose()` exists because Dart has no destructors. Rust's `Drop` covers it. v2 is fine. **Caveat**: any recogniser that owns a `BackgroundExecutor::timer` task handle must `Drop`-cancel it (verify LongPress and DoubleTap do this). If a recogniser is dropped mid-gesture and its timer keeps holding the arena, we leak hold-counts. This is testable and should have a unit test.

### 3.15 GestureRecognizerFactory
Flutter's factory pattern `GestureRecognizerFactoryWithHandlers<T>(_constructor, _initializer)` exists so `RawGestureDetector` can *re-instantiate* recognisers when the widget rebuilds with new callback closures, without losing the underlying recogniser's per-instance state. The initialiser mutates the existing instance with new callbacks rather than constructing a fresh one.

v2's fluent builder `__internal_on_*` builds a recogniser at compose-time and rebuilds-from-scratch on widget rebuild. Today this works because recognisers are per-event-loop-tick and have no cross-frame state. **Risk**: when S08 adds semantics-driven synthetic events or S07.7 adds resampling queues, recogniser state outlives a single frame. At that point, the "rebuild fresh" model loses queued events.

Recommendation: defer this until S07.7. If resampling forces stateful recognisers, introduce a `RecognizerKey` (hash of recogniser-type + debug-owner) and let the binding reuse a recogniser across frames keyed on that. Don't prematurely import Flutter's factory pattern.

### 3.16 Multi-pointer drag
The `MultiDragGestureRecognizer` family is **not in S07.6**. It's needed for: (a) reorderable list widgets where each finger drags an independent row, (b) split-pane drag handles where two fingers resize two splitters simultaneously, (c) drawing apps with N-finger independent strokes. Today v2's `Pan` is single-pointer.

Plan: S07.9 (after S07.7 resampling and S07.8 prediction). The model is `MultiDragPointerState` per pointer + a `Drag` trait returned from `onStart` that the recogniser routes per-pointer events to. This *requires* per-pointer arena entries (which we have via `merge_by_pointer_id`), but also requires the `Drag` interface — a tiny trait with `update(DragUpdateDetails)`, `end(DragEndDetails)`, `cancel()`. Add the trait now (empty crate, no implementations) so its type identity is committed, then fill in S07.9.

### 3.17 Pointer event resampling
Flutter's `PointerEventResampler.sample(sampleTime, nextSampleTime, callback)` runs **inside `GestureBinding.handlePointerEvent`** — not at scheduler tick. The algorithm is **linear interpolation** between the two queued samples bracketing `sampleTime` (no Hermite). `samplingOffset` is *negative* (typically `-Duration(milliseconds: 5)` for 60Hz) so we sample "in the past" with both bracketing samples already known — non-negative offsets disable resampling. The resampler synthesises intermediate `PointerMoveEvent`s only when position changed; `Down`/`Up` events pass through but are emitted aligned to the sample boundary.

v2's S07.7 plan should mirror this exactly: live in `binding.rs::handle_pointer_event` between sanitiser and dispatch, single-buffer per pointer, linear interp, negative offset relative to display refresh. Don't put it in a scheduler tick — Flutter explicitly moved it out of there because frame-aligned dispatch is wrong (you want sample-time-aligned, which is *before* frame begin).

Open question: gpui's `BackgroundExecutor` doesn't expose a `samplingClock` directly. v2 will need to track "frame begin time" itself — a small `FramePhaseTracker` shim. Note this in the S07.7 plan.

### 3.18 Semantics protocol (S08)
Flutter's `SemanticsGestureDelegate` overrides `RawGestureDetectorState.replaceGestureRecognizers` so that when the semantics tree is built, the gesture recogniser set is *replaced* with semantics-aware variants that respond to `SemanticsAction` events from accessibility services (TalkBack, VoiceOver, switch control) by synthesising a `PointerDownEvent`+`PointerUpEvent` pair routed through the normal arena.

For S08, the seam is: a recogniser must opt into "I respond to semantics action X" (Tap, LongPress, ScrollLeft/Right/Up/Down, Increase, Decrease). The binding receives a `SemanticAction`, walks the recogniser tree, finds opt-in recognisers, and synthesises pointer events. Critically, the synthesised events go through the normal arena — accessibility never bypasses gesture competition. v2's current `SemanticAction { Tap, DoubleTap, LongPress }` enum is the right shape; add `ScrollDirection` + `Adjust(f32)` + `Activate` for parity. Make it `#[non_exhaustive]`.

### 3.19 Gesture settings overrides (S14)
Flutter's two-layer model:
- `dart:ui GestureSettings` (engine, *physical* pixels) — Android `ViewConfiguration.getScaledTouchSlop()` per-device.
- `gestures DeviceGestureSettings` (framework, wraps engine values, accessible via `MediaQuery.gestureSettingsOf(context)`) — *logical* pixels after DPI conversion.

Only `PrimaryPointerGestureRecognizer.preAcceptSlopTolerance` actually consults `gestureSettings.touchSlop` in current Flutter — drag/scale recognisers still use compile-time `kTouchSlop`. So MediaQuery-override is *partially wired* even in Flutter today.

v2 plan for S14: keep `GestureSettings` per-window (already done), add a `MediaQuery::gesture_settings_of()` lookup, plumb only the slop fields initially (touch_slop, pan_slop, long_press_slop). Do **not** plumb every threshold — that creates a 12-knob matrix downstream users won't tune. Mirror Flutter's "physical → logical" path by carrying a `physical_touch_slop_px: u32` raw value at the binding edge and converting to `Pixels` once.

### 3.20 Scrollable / scroll physics integration (S11)
Flutter's `Scrollable` widget builds its drag recogniser as the **captain** of a `GestureArenaTeam`, with all child draggables as members. This is *the* canonical use of teams: a list with a button inside both wants to scroll-drag (Scrollable's drag recogniser) AND fire the button's tap. The team-without-captain pattern lets the button's tap recogniser win on `Up` if no drag-slop was crossed, without the drag recogniser needing to wait for tap timeout.

For S11 physics: `Scrollable` hosts a `GestureArenaTeam` (no captain). Its inner `VerticalDragGestureRecognizer` is the first member. Children that opt into "I want to compete with scroll" register their own recogniser as a team member. This gives you proper "tap-inside-scroll-list" behaviour. v2's `GestureArenaTeam` struct is ready; what's missing is (a) public surface, (b) builder API on a future `Scrollable` widget to attach team members from descendants, (c) plumbing through hit-test so the team's pointer-add logic runs.

---

## 4. Recommendations by milestone

**S07.7 (resampling)**
- Algorithm: linear interpolation, negative `samplingOffset`, in-binding (not scheduler).
- Prereq: a `FramePhaseTracker` exposing "current frame begin time" — gpui doesn't have this off the shelf.
- Add `PointerEvent.synthesized: bool` field now (cheap, breaking-friendly), so resampler-emitted events are distinguishable.
- Test harness: virtual-clock `samplingClock` injected via `BackgroundExecutor`-style trait.

**S07.8 (prediction)**
- Hold until P1 frame-budget instrumentation lands. Without per-frame jitter measurement, prediction is noise.
- Algorithm choice: 1-Euro filter > Kalman > Hermite for touch; Flutter doesn't ship prediction in core (just resampling). v2 can innovate here but must keep it opt-in.

**S07.9 (multi-drag)**
- Add `Drag` trait + `MultiDragPointerState` helper now (empty surface, no recogniser).
- Implement `ImmediateMultiDragGestureRecognizer` first (simplest, no delay).

**S08 (semantics)**
- Make `SemanticAction` `#[non_exhaustive]`. Add `Activate`, `ScrollUp/Down/Left/Right`, `Increase`, `Decrease`, `DismissModal`.
- Recognisers register interest via a `SemanticActionInterest` bitfield — binding routes synthetic pointer events through the arena, never bypassing.
- Do not introduce `SemanticsGestureDelegate` (Flutter pattern is heavy). Instead: each recogniser optionally returns its semantic interests during construction.

**S09+ (canvas/layers)**
- The transform-stack hit-test (§3.4) **must** land before `Transform`/`RotatedBox` widgets ship. Add `HitTestEntry.transform: Option<Mat4>` and `local_position: Point<Pixels>` *now*, even before the layer system uses them — better to have the field unused than to retrofit it after public API freeze.

**S11 (physics + scroll)**
- Public-ise `GestureArenaTeam`. Document the captain/no-captain semantics with the Slider (no captain) and Scrollable (no captain) examples.
- Add per-platform `VelocityTracker` trait + iOS/macOS variants.
- Implement `DragStartBehavior` field on Pan/HorizontalDrag/VerticalDrag — Scrollable will need `down` for iOS rubber-banding.

**S13 (text selection)**
- Build on `TapAndDrag*` family + `consecutiveTapCount` + a no-captain `GestureArenaTeam`. Do not invent a v2-specific monolith.
- Add `SerialTapGestureRecognizer` first (the building block).

**S14 (MediaQuery gesture settings)**
- Plumb only slop fields initially.
- Carry a raw `physical_touch_slop` from the platform layer; convert once at binding edge.
- Use `MediaQuery::gesture_settings_of(context)` lookup pattern, listened-to (not full-rebuild-trigger).

**A8 (non_exhaustive sweep)**
- Already done for `PointerKind`, `PointerPhase`, `HitTestBehavior`, `GestureSettings`, `PointerEvent`, `HitTestEntry`. Confirm `GestureDisposition`, `SemanticAction`, `PointerSignalEvent`, future `PressureSample`, future `Drag` trait return-type all carry it.

**Future drag seam**
- `Drag` trait identity (not implementation) ships with S07.9 type stub.
- Multi-drag implementations follow the order: Immediate → Horizontal/Vertical → Delayed.

---

## 5. Breaking-friendly proposals (do these before public API freeze)

1. **Replace `pressure: f32` with `pressure: Option<PressureSample { value, min, max }>`.** Today's normalised-on-platform-side approach loses the platform range and breaks force-press threshold semantics for stylus. Cheap to fix now; expensive after S07.6 ships ForcePress with a hard-coded `0.4`.

2. **Add `PointerKind::Trackpad` + `PointerKind::InvertedStylus` + `PointerKind::Unknown`.** All three are real Flutter device kinds. `Trackpad` is the kind for the synthetic device that emits PanZoom-class events; without it, we cannot route PanZoom events. Adding now while `PointerKind` is `#[non_exhaustive]` is a no-op for downstream code.

3. **Add `PointerPhase::PanZoomStart | PanZoomUpdate | PanZoomEnd`** (or a separate `PointerPanZoomEvent` family alongside `PointerSignalEvent`). Required for trackpad pinch parity on macOS — currently we only have `Magnify` which is a coarse approximation.

4. **Skip the S07.5 single `set_arena_back_channel(bc, idx)` legacy hook entirely; ship `set_arena_back_channel_for_pointer(pid, bc, idx)` directly.** The S07.6 plan already wants this. Backwards compat does not exist yet (no crates.io). Ship the per-pointer form and delete the per-recogniser form before the first cargo publish.

5. **`HitTestEntry` gains `transform: Option<Mat4>` + `local_position: Point<Pixels>`.** Pre-populate `local_position = position` and `transform = None` until the layer system uses them. Adding fields after `Transform` widgets ship is a real breakage.

6. **`GestureDisposition` becomes `#[non_exhaustive]`.** Even if we never add variants, the attribute lets us add observability hooks (e.g. `Disposition::AcceptedDeferred`) without breaking consumers. Cost: zero.

7. **`PointerEvent.synthesized: bool` field.** Pay-now-or-pay-later: resampler (S07.7) and semantics (S08) both need to mark events as synthesised for debugging and gesture-detector filters.

8. **`PointerEvent.source_timestamp: Instant` separate from `timestamp: Instant`.** Today `timestamp` is "platform produced this event"; resampler will need both "platform produced" and "we synthesised at sample time". Separate them now.

9. **`AllowedButtonsFilter` as an opt-in `Box<dyn Fn(...) -> bool>` next to the current `button: PointerButtons` simple field.** Don't replace the simple form — augment it. ForcePress/MultiTap need it.

10. **`DragStartBehavior` enum + field on Pan/HorizontalDrag/VerticalDrag builders.** Minimal API addition; iOS scroll parity (S11) requires it.

11. **`Drag` trait stub + `MultiDragPointerState` helper struct stub** in the gesture crate, even before any recogniser implements them. Just so the type identity is committed before we publish.

12. **Public-ise `GestureArenaTeam` with documented captain/no-captain semantics.** Today it's a private struct. S11 will need it; surface it now with a doc-test mirroring the Slider example.

13. **`LongPressDetails::end` carries `velocity: Velocity`** if not already; secondary/tertiary callback parameterisation via a `button: PointerButtons` builder field on `LongPress`.

14. **`SemanticAction` becomes `#[non_exhaustive]` + adds `Activate`, `ScrollUp/Down/Left/Right`, `Increase`, `Decrease`.** S08 prereq.

---

## 6. Anti-recommendations (do NOT copy from Flutter)

- **Global `PointerRouter` singleton** — gpui's per-window event loop subsumes it. Resist the temptation to add one for "consistency".
- **`GestureBinding` as a global mixin on a `BindingBase`** — v2's per-window binding is strictly better. Singletons in Rust mean `Arc<Mutex>` everywhere and we already rejected that.
- **Dart `Stream<PointerEvent>` API for events** — direct dispatch via `dyn FnMut` callbacks is cheaper, has no allocation per-event, and matches the single-thread per-window model.
- **Three-way hit-test interface split (`HitTestable` / `HitTestDispatcher` / `HitTestTarget`)** — v2's collapsed model is simpler and complete. The transform stack is the part to copy; the interface split is not.
- **Mandatory `dispose()` everywhere** — Rust's `Drop` covers it. Don't introduce a manual `dispose()` method on `GestureRecognizer`.
- **`GestureRecognizerFactory<T>` factory-with-initialiser pattern** — fluent builders fit Rust better; only adopt the factory pattern if S07.7 resampling forces stateful recognisers across frames, and even then prefer a `RecognizerKey`-based reuse mechanism.
- **Sealed `DragGestureRecognizer` abstract class** — v1 already proved supertrait sealing hurts downstream extension. Keep recognisers as concrete structs implementing a non-sealed `GestureRecognizer` trait.
- **`Pointer{Scroll,Scale}Event` as `PointerSignalEvent` *and* an unrelated `PointerScaleEvent`** — Flutter has both for historical reasons. v2's single `PointerSignalEvent::{Scroll, Magnify}` enum is cleaner; preserve it. PanZoom is a *different* family (it carries pan + zoom + rotation tuples, distinct from a scalar magnify).
- **`postAcceptSlopTolerance` defaulting to `null` (no limit)** — Flutter's default lets long-press drift unboundedly post-accept. v2 should default to a real value (e.g. unlimited but document it; or `Some(40px)` to match user expectations). Open question — pick a default deliberately, don't inherit Flutter's.
- **`SemanticsGestureDelegate` + `replaceGestureRecognizers` swap mechanism** — too heavy. Use opt-in semantic-action interests on each recogniser instead.

---

## Open questions

- Does v2's current dispatch keep recognisers receiving events for a captured `pointer_id` after the pointer leaves the original hitbox? Verify in `dispatch.rs` before assuming the per-window model fully replaces `PointerRouter`.
- Should `OneSequenceGestureRecognizer` / `PrimaryPointerGestureRecognizer` materialise as crate-internal helper structs (not traits) to deduplicate slop/timer logic across Tap/LongPress/ForcePress?
- For S07.7 resampling: where does "current frame begin time" come from in gpui? Need a `FramePhaseTracker` shim or a gpui upstream addition.
- `postAcceptSlopTolerance` default: unlimited (Flutter parity) or bounded (better UX)?

---

## See also

- `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md` — the original S07 design.
- `docs/superpowers/specs/2026-05-08-recognizer-extension.md` — recognizer extensibility recipe.
- `docs/superpowers/specs/2026-05-08-gesture-roadmap-from-old-impl.md` — v1 → v2 comparative review.
- `.ai-factory/plans/feature-s07-6-recognizer-roster.md` — current S07.6 plan (consumes recommendations 1, 4, 9 directly).
