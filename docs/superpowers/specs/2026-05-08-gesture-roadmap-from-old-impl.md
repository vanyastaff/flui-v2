# Gesture roadmap — lessons from the v1 `flui-interaction` impl

**Status:** identified follow-up work. Not scheduled, not implemented.
**Source comparison:** `C:\Users\vanya\RustroverProjects\flui\crates\flui-interaction\src` (v1, abandoned) vs `crates/flui-core/src/gesture/` (v2, current — S07 + S07.5 shipped).
**Date:** 2026-05-08.
**Audience:** future contributors picking up `S07.6+` follow-on work.

This document captures the actionable findings from a side-by-side review of the abandoned v1 gesture system against the v2 implementation that just landed. v1 had several ideas worth porting and several anti-patterns the v2 design successfully avoided. The point of this doc is to record both, so future work doesn't relitigate decisions and doesn't miss the opportunities.

---

## High-value additions (recommended, not yet scheduled)

### S07.6 — `MultiTapGestureRecognizer`

- **What it detects:** N-finger simultaneous tap (2-finger, 3-finger, etc.) within a configurable time + position window.
- **v1 reference:** `flui-interaction/src/recognizers/multi_tap.rs` (state machine: `Ready → Collecting → WaitingForUp → Completed`).
- **Why port:** accessibility (3-finger tap to trigger VoiceOver/TalkBack shortcuts), macOS/iPad trackpad gestures, Flutter parity.
- **Integration cost on v2:** **low.** v2's arena already supports multi-pointer entries; the recognizer just needs to coordinate per-finger state across the arena's `entries` vec. Implements `RecognizerLifecycle::configure_settings` to read `multi_tap_window` from `GestureSettings`. New file: `crates/flui-core/src/gesture/recognizers/multi_tap.rs` + builder in `gesture/mod.rs` + e2e test.
- **Risks:** None significant. The arena's `merge_by_pointer_id` already handles the per-pointer competition correctly.
- **Estimated effort:** 2-3 hours including tests.

### S07.6 — `ForcePressGestureRecognizer`

- **What it detects:** pressure-based gestures (3D Touch / Force Touch on iOS, pressure-capable digitizers on macOS / Wacom). Fires `on_force_press_start`, `on_force_press_peak`, `on_force_press_end` with normalized pressure.
- **v1 reference:** `flui-interaction/src/recognizers/force_press.rs` (state machine: `Ready → Possible → Started → Peaked → Ended`).
- **Why port:** Flutter parity for iOS first-party experiences. The `pressure: f32` field is already on `PointerEvent`, so signal extraction is free.
- **Integration cost on v2:** **medium.** State machine fits cleanly on `GestureRecognizer` + `GestureDisposition`. Configurable thresholds (`pressure_start`, `pressure_peak`) live on the recognizer. Need new fields on `GestureSettings` (`force_press_start_pressure`, `force_press_peak_pressure`).
- **Risks:** Pressure values are platform-dependent — macOS `MousePressureEvent` reports differently from iOS. Document the platform-truth table in rustdoc; recognizer normalises to `[0.0, 1.0]`.
- **Estimated effort:** 3-4 hours including platform docs + tests.

### S07.7 — Pointer event resampling (preprocessing)

- **What it does:** buffers raw pointer events and emits a smoothed stream synchronised to the display refresh rate. Mitigates input/display refresh mismatch (e.g. 30Hz tablet → 60Hz screen → jittery drag).
- **v1 reference:** `flui-interaction/src/processing/resampler.rs` (linear interpolation between buffered samples).
- **Why port:** **this is the biggest gap in v2.** Flutter does resampling by default. Without it, drag/pan gestures on devices with low input frequency or mismatched refresh rates feel stuttery — the difference is visible to end users.
- **Integration cost on v2:** **medium.** Lives outside the recognizer arena, in `GestureBinding` as a pre-arena pipeline phase. Probably a new `PointerResampler` field next to `PointerSanitizer` in the binding, with a configurable enable flag in `GestureSettings`. The resampled events are what flows into the existing `dispatch_split_mut` path.
- **Risks:** Resampling delays event delivery by one frame (typical implementation). For Tap recognisers this is invisible; for fast scroll wheel handling it could be noticeable. Mitigation: only apply to drag-class gestures (Pan/Scale), not to Tap/Click/PointerSignal.
- **Estimated effort:** 4-5 hours including the bench regression check (resampling adds work to the dispatch path; T22 budgets must still pass).

### S07.8 — Pointer prediction (future work, large)

- **What it does:** extrapolates future pointer positions using velocity + optional acceleration; renders content at the predicted position to reduce perceived latency by 8-16ms.
- **v1 reference:** `flui-interaction/src/processing/prediction.rs` (multiple strategies: polynomial, linear, two-sample, with confidence scoring).
- **Why port (long-term):** noticeable latency improvement for stylus-heavy apps (drawing, note-taking) and games. Modern competitors (Apple Pencil low-latency mode, Samsung S-Pen) ship with prediction.
- **Integration cost on v2:** **high.** Requires frame-timing instrumentation that GPUI does not currently expose to `flui-core`. Would also need a feedback loop with the renderer to invalidate predicted regions when actual events arrive.
- **Recommendation:** **defer.** This is a P-track perf milestone, not part of the gesture core. Schedule after S09 (canvas facade) and the P1 frame-budget instrumentation roadmap item — those land the substrate prediction needs.
- **Estimated effort:** 12-16 hours (large architectural change spanning gesture, scene, and renderer).

---

## Medium-value additions (consider for future spec)

### Raw input mode (game/drawing app bypass)

- **v1 reference:** `flui-interaction/src/processing/raw_input.rs`.
- **Goal:** let games and drawing apps intercept raw, unprocessed pointer events before the gesture arena competes for them.
- **v2 status:** no equivalent. The gesture arena is the only path.
- **Recommendation:** keep gesture-first defaults; add an opt-in `with_raw_pointer_input(callback)` on `Window` if a real consumer requests it. Premature abstraction otherwise.

### Gesture recording & replay

- **v1 reference:** `flui-interaction/src/testing/{recording.rs, input.rs}` — captures `RecordedEvent { time_offset, position, kind, pressure, tilt, rotation }` and replays at scaled-time.
- **Goal:** deterministic regression tests that replay real-world gesture streams. Useful for property-based testing of custom recognisers.
- **v2 status:** has unit + property + e2e tests, but no record/replay harness for arbitrary streams.
- **Recommendation:** worth adding as a `test-support`-feature-only module (`gesture/recording.rs`) when v2 has > 2 third-party recognisers in the wild. Until then, the existing test pyramid is sufficient.

---

## Anti-patterns from v1 that v2 successfully avoids

These are decisions v2 got right; do not regress them in future work.

| v1 pattern | v2 alternative | Why v2 wins |
|---|---|---|
| `Arc<Mutex<State>>` everywhere for shared recognizer state | `Rc<RefCell<…>>` per-window, main-thread-only | UI runtime is single-threaded; `Arc<Mutex>` adds lock contention with no benefit. v2's borrow-check failure points at the `Rc` directly — easier to debug. |
| `Arc<Callback>` cloned on every fire | `Box<dyn FnMut>` stored once, called by `&mut` ref | Zero allocations per gesture event; better cache behaviour. |
| Sealed `GestureRecognizer` trait + extension trait pattern | Unsealed `GestureRecognizer` + sibling `RecognizerLifecycle` | Custom recognisers in downstream crates compile without jumping through visibility hoops. |
| 3-level trait hierarchy (`GestureArenaMember` → `OneSequenceGestureRecognizer` → `PrimaryPointerGestureRecognizer`) | Flat `GestureRecognizer` trait, per-impl state | Less ceremony, easier to read, no premature abstraction. The helper traits in v1 were over-engineered for the actual recogniser variety. |
| Singleton `PointerRouter` + `EventRouter` layered above arena | `Window::dispatch_event` calls arena directly | One less indirection. The router was solving a problem GPUI's per-window model already solves. |

## What NOT to port (v2's design is better)

- **`focus_scope.rs` / `focus.rs`:** v1 layered a custom focus system on top of the gesture arena. v2 uses GPUI's `FocusHandle`, which integrates with keyboard navigation (S12), accessibility (S08), and platform IME. v1 was pre-GPUI and predated all of that.
- **`signal_resolver.rs`:** v1 ran scroll/pinch/rotate signals through a conflict resolver inside the arena. v2 routes these via `PointerSignalEvent` and bypasses the arena entirely (signals are non-competitive by design — see `gesture/mod.rs` rustdoc § "Common pitfalls"). v2's separation is cleaner.
- **`primary_pointer.rs` / `one_sequence.rs`:** v1's helper recogniser bases. v2 doesn't need them — concrete recognisers (Tap, Drag, Scale, etc.) implement the flat `GestureRecognizer` trait directly.
- **`mouse_tracker.rs`:** v1's custom hover/cursor tracker. v2's `PointerSanitizer::diff_hover` covers the same ground, integrated into the dispatch path.

---

## Suggested roadmap entries

When the next sprint planning happens, add these to `.ai-factory/ROADMAP.md` Phase II under existing S07.5:

```markdown
- [ ] **S07.6 GestureArena — recognizer roster expansion** — `MultiTapGestureRecognizer` (N-finger taps, accessibility) + `ForcePressGestureRecognizer` (3D Touch / Force Touch). Documented in `docs/superpowers/specs/2026-05-08-gesture-roadmap-from-old-impl.md`.
- [ ] **S07.7 GestureArena — pointer resampling** — pre-arena pipeline phase that smooths input/display refresh-rate mismatch. Closes the biggest UX gap vs Flutter. Same doc for context.
```

S07.8 (prediction) waits for the P1 frame-budget instrumentation roadmap entry to land.

---

## See also

- `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md` — original S07 design (the system this builds on).
- `docs/superpowers/specs/2026-05-08-recognizer-extension.md` — the contributor recipe for adding new recognisers using the S07.5 `RecognizerLifecycle` seam. Anyone implementing S07.6 starts there.
- `.ai-factory/plans/feature-gesture-s07-t15-followup.md` — the S07.5 plan that delivered the seam and closed the T15.5 backlog.
