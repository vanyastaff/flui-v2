# K04 Effect / Frame Contract Migration Guide

## Summary

K04 turns the App scheduler into a typed, observable **seven-phase state
machine** with placement-aware effect placement, an App-level `FrameClock`,
advisory per-phase deadlines, and panic-safe phase wind-down. The pre-K04
single-unbounded effect drain is replaced by phase-keyed FIFO drains; the
ad-hoc "next frame callback" list becomes a named pair of `on_pre_frame` /
`on_post_frame` hooks; animation ticking gets a real `AnimationTick` phase
backed by an active-controller set.

All pre-K04 callsites continue to compile and behave as before. The pre-K04
`App::defer(f)` routes through `DeferPlacement::EndOfUpdate` and preserves
observable behavior. `Window::on_next_frame` remains as a `#[deprecated]`
alias for the renamed `Window::on_pre_frame`.

This is **not** the Framework tier (`Widget`, `State<W>`, `setState`,
`InheritedWidget`). K04 ships only the kernel scheduling primitives those
specs need; Phase II-F (`SF03`-`SF08`) consumes them.

## The Seven Phases

```text
Idle → PreFrame → AnimationTick → Build (reserved) → Layout → Prepaint → Paint → PostFrame → Idle
```

- `Idle` — no frame in flight.
- `PreFrame` — `App::on_pre_frame` + `Window::on_pre_frame` callbacks drain
  here. K15 still applies: no `update_window` on the same window, no
  layout reads.
- `AnimationTick` — `App::active_animations` walked; every
  `TickTarget::tick(frame_clock.now())` fires; `Effect::Notify` emitted for
  controllers that returned `TickOutcome::Continue`.
- `Build` — **reserved no-op slot for SF05 (`BuildOwner::flush_dirty`).**
  Enters and exits immediately in K04. Reserving the discriminant now
  prevents an enum addition under `#[non_exhaustive]` later.
- `Layout` — Taffy resolve.
- `Prepaint` — bounds + hitbox + `Interactivity::paint`. Internal
  `Window::DrawPhase::Prepaint` is a strict sub-state.
- `Paint` — scene-primitive generation, `Window::present`, `complete_frame`.
  Internal `Window::DrawPhase::{Paint, Focus}` are strict sub-states.
- `PostFrame` — `App::on_post_frame` + `Window::on_post_frame` callbacks
  drain. Read-only for the in-flight scene (no element mutation; queue
  mutations via `cx.defer_to(DeferPlacement::NextFrameStart, …)`).

`EffectFlush` is **not** a phase. It is interleaved at every phase boundary:
before each phase enters, the phase-keyed drain consumes admissible
`Effect::Defer` callbacks; after the body, a `PhasePost` drain consumes only
`DeferPlacement::EndOfUpdate` — defers queued with the phase's matching
placement carry to the NEXT frame's matching phase entry.

## `cx.defer` vs `cx.defer_to(placement, ...)`

The pre-K04 `App::defer(f)` / `cx.defer(...)` keeps working unchanged. It
routes through `DeferPlacement::EndOfUpdate`, which drains at every phase
boundary — the same observable behavior as the pre-K04 single-loop drain.
Existing Tier-C callsites need zero migration.

When you want a specific later phase, use `cx.defer_to(placement, f)`:

```rust
use flui_core::frame::DeferPlacement;

// Wait until the NEXT frame's PreFrame fires.
cx.defer_to(DeferPlacement::NextFrameStart, |cx| {
    // Runs in frame N+1's PreFrame.
});

// Wait until the current frame's PostFrame.
cx.defer_to(DeferPlacement::PostFrame, |cx| {
    // Observes the resolved scene before the frame closes.
});

// Drain "eventually" — coalesce with future Idle phases.
cx.defer_to(DeferPlacement::Idle, |cx| {
    // Non-time-critical bookkeeping.
});
```

`Context::defer_to`, `Window::defer_to`, and `AsyncWindowContext::defer_to`
are placement-aware counterparts of the existing `defer` methods.

## `Window::on_next_frame` → `Window::on_pre_frame`

The pre-K04 `Window::on_next_frame` was misleading — callbacks fire BEFORE
the next frame's draw, not after. K04 renames it to `on_pre_frame`:

```rust
// Before (K03):
window.on_next_frame(|window, cx| { /* ... */ });

// After (K04):
window.on_pre_frame(|window, cx| { /* ... */ });
```

`Window::on_next_frame` continues to compile as a `#[deprecated]` alias that
forwards to `on_pre_frame`. The alias will be removed in K04+1.

The rename also lands on `Context::on_pre_frame` (for view-typed entities)
and `AsyncWindowContext::on_pre_frame`.

## New: `Window::on_post_frame`

A new `Window::on_post_frame` API anchors a callback at the `PostFrame`
phase — AFTER `window.draw` has produced the scene and `complete_frame` has
fired. Use it for telemetry export, inspector readout, or post-paint settle
work that needs the resolved scene state:

```rust
window.on_post_frame(|window, cx| {
    let scene_primitives = window.scene_primitive_count();
    metrics.record("frame.primitives", scene_primitives);
});
```

The same shape mirrors on `Context::on_post_frame` (view-typed) and
`AsyncWindowContext::on_post_frame`.

## New: App-level pre/post-frame hooks

For cross-window callbacks (input replay, telemetry export, focus moves
across windows), K04 adds `App::on_pre_frame` and `App::on_post_frame`:

```rust
cx.on_pre_frame(|cx| {
    // App-wide PreFrame work; fires AFTER per-window on_pre_frame callbacks.
});

cx.on_post_frame(|cx| {
    // App-wide PostFrame work; fires AFTER per-window on_post_frame.
});
```

## `FrameClock` for time-sensitive code

Per K04 axiom **P3** — *time is sampled once per logical frame; every
consumer in that frame sees the same `Instant`* — the App holds a
`FrameClock` accessible via `cx.frame_clock()`:

```rust
let now = cx.frame_clock().now();          // valid inside a frame
let frame_index = cx.frame_clock().frame_index();
let delta = cx.frame_clock().delta();
```

Outside a frame (`FramePhase::Idle`), `FrameClock::now()` triggers a debug
assertion in `cfg(debug_assertions)` and returns the last-sampled `Instant`
in release. Code paths that read non-frame time should either schedule via
`cx.defer_to(...)` or use a wall clock directly with an explicit comment.

`Window::frame_clock_view()` returns an opaque `FrameClockView` snapshot —
reserved so a future R-track / Wasm spec can introduce per-window epoch
divergence without a SemVer break.

## `AnimationController::value()` — per-frame cache

The pre-K04 `AnimationController::value()` re-read the clock on every call,
so two reads within one frame could see slightly different elapsed times.
K04 closes the TODO at `animation/controller.rs:233`:

- The `AnimationTick` walker seeds `last_tick_instant` once per frame.
- The first `value()` call in the frame computes against that `Instant` and
  caches the result.
- Subsequent reads return the cache.

The public signature is unchanged; downstream code that reads `value()`
multiple times per frame (e.g. an animated view's prepaint + paint passes)
gets stability for free.

## Deadline-overrun expectations

K04 ships advisory per-phase deadlines (per `docs/promt.md` §3.1):

| Phase            | Budget |
|------------------|--------|
| Animation tick   | ≤ 1 ms |
| Layout           | ≤ 3 ms |
| Prepaint         | ≤ 4 ms |
| Paint + present  | ≤ 1 ms |
| Gesture dispatch | ≤ 1 ms |
| Effect flush     | ≤ 2 ms |
| Slack            | ~4 ms  |

- **Non-effect phases** (`PreFrame`, `AnimationTick`, `Layout`, `Prepaint`,
  `Paint`, `PostFrame`): if a phase exceeds its budget, the engine records
  the overrun in `FrameProfile.overruns` and emits one rate-limited `WARN`
  per phase per frame. The phase runs to completion — aborting mid-paint
  would corrupt scene state.
- **`EffectFlush`** (interleaved): break-and-requeue. When the per-flush
  budget is exceeded, the remaining `Effect::Defer` entries stay in
  `pending_effects` for the next phase boundary; one rate-limited `WARN`
  fires. Atomic effect units make re-queue safe.

Consumers do not enforce deadlines themselves; the K04 substrate handles
it. Read `App::frame_profile()` to observe overruns.

## Panic-safety contract

A panic inside a phase body is caught by `App::run_frame`'s `catch_unwind`
boundary and routed to `App::abort_frame_after_panic(phase)`. After
recovery:

- `current_phase = FramePhase::Idle`
- `flushing_effects = false`
- `Window::next_frame` buffer cleared
- `frame_clock.in_frame() = false`, but `frame_index` and `last_sampled`
  preserved
- Active-animation set unchanged (controllers tick again next frame)
- Effect queue retains pending entries (drain next frame)

The next `App::run_frame` succeeds normally. `FrameOutcome.panicked_phase`
reports which phase panicked.

## `TestApp::advance_frame` for tests

K04 introduces an explicit test-mode frame driver:

```rust
let mut app = TestApp::new();
app.set_auto_advance_frames(false);                  // opt out of legacy auto-redraw
let mut window = app.open_window(MyView::new);

let outcome = window.advance_frame();                // drive one frame
assert!(outcome.panicked_phase.is_none());

let _outcomes = window.advance_frames(3);            // drive N frames

let profile = app.frame_profile();
assert!(profile.frame_index >= 1);
```

`TestApp::set_auto_advance_frames(false)` disables the pre-K04 auto-redraw
inside `flush_effects` so the phase pipeline is observable cleanly. K04+1
will flip the default to `false` once the broader test suite migrates;
existing tests that don't care about phase order keep working with the
default `true`.

`TestAppWindow::advance_frame()` is the per-window variant for tests that
own multiple windows.

### Panic conditions on `TestApp::advance_frame()`

`TestApp::advance_frame()` is a convenience wrapper that assumes the
TestApp owns exactly one open window. It panics in two scenarios:

- **No windows open** — `"TestApp::advance_frame called with no open
  windows; open a window first"`. Construct the test window via
  `app.open_window(...)` before calling `advance_frame()`.
- **Multiple windows open** — `"TestApp::advance_frame is ambiguous with
  N open windows; use TestAppWindow::advance_frame"`. With more than one
  window, the wrapper cannot pick which one to drive — call
  `window.advance_frame()` on the specific `TestAppWindow` instead.

The per-window `TestAppWindow::advance_frame()` does not panic on
window-count; it drives the window the handle owns.

## K15 Q&A

**Q: Did K04 add new re-entry escape paths?**

A: No. The K15 contract is unchanged. `cx.defer` (default
`DeferPlacement::EndOfUpdate`) remains the single sanctioned re-entry escape.
`cx.update_window` on the same window inside any forbidden context still
returns `ReentryError::NestedWindowUpdate`.

**Q: Where do I read "this is fine, K15 still applies"?**

A: The joint K15+K04 paragraph is prepended to the `reentrancy` module
docstring. It states: *re-entry from within any K04 phase callback follows
K15; the phase a callback runs in determines which `DeferPlacement` is sane
to defer to; `cx.defer` (default `EndOfUpdate`) remains the only K04 re-entry
escape; the phase a deferred callback eventually runs in is its placement,
not the queueing phase.*

**Q: What happens if I call `cx.defer_to(NextFrameStart, ...)` from inside an
`on_pre_frame` callback?**

A: The defer is queued and **carries one frame**. It does NOT fire same-frame
even though the `on_pre_frame` callback itself ran in `PreFrame`. The
post-body drain admits only `DeferPlacement::EndOfUpdate`, so `NextFrameStart`
placement is carried to the next frame's `PreFrame` pre-drain.

## See also

- Design spec: `docs/superpowers/specs/2026-05-11-K04-effect-frame-contract-design.md`
- K15 spec: `docs/superpowers/specs/2026-05-09-K15-reentrancy-contract-design.md`
- Phase budgets and hot-path rules: `docs/promt.md` §3.1, §4.6, §5
- Plan and tasks: `.ai-factory/plans/feature-K04-effect-frame-contract.md`
