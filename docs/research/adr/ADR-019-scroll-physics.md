# ADR-019: Scroll physics — scoping document for the future `Scrollable` widget

**Date:** 2026-05-12
**Status:** Draft — scoping ADR. Defines what a future scroll widget
must carry; does not pick the implementation. No code changes land
with this ADR.
**Scope:** `flui-core/src/elements/uniform_list.rs` (imperative
scroll-to-item), `flui-core/src/animation/simulation.rs` (spring +
friction simulators), and a future `Scrollable` widget yet to be
written.
**Drivers:** Flutter `f: scrolling` issue cluster (no single one
dominates — the cluster as a whole),
[zed-industries/zed#40623](https://github.com/zed-industries/zed/issues/40623)
(macOS trackpad horizontal scroll does not lock out vertical).

## Context

Flutter has the largest `f: scrolling` open issue cluster of any
label — hundreds of issues spanning Android jank, iOS rubber-band
edge cases, web wheel-event misrouting, and platform-specific fling
curves. They all share one observation: **scrolling is not just
mapping pointer-delta to offset**. It is a physics simulation that
must match the host platform's feel and stay deterministic under
input flurries, layout changes, and animation interruptions.

flui-v2 today has the read/write half: `ScrollHandle`,
`UniformListScrollHandle`, `ScrollStrategy::{Top,Center,Bottom,Nearest}`,
imperative `scroll_to_item`. It does not have any physics — no
momentum, no fling, no bounce, no overscroll lock-axis. We do not
have a generic `Scrollable` widget. Anything scrollable today
(`uniform_list`, `list`) is bespoke.

This ADR fixes the **contract** a future generic scroll widget must
carry, so that when somebody builds it, the contract is not a
research project on top of an implementation project.

## Current behaviour (verified)

References cite the commit this ADR is written against.

### Imperative scroll surface

[`crates/flui-core/src/elements/uniform_list.rs:79`](../../../crates/flui-core/src/elements/uniform_list.rs#L79):

```rust
pub struct UniformListScrollHandle(pub Rc<RefCell<UniformListScrollState>>);
pub enum ScrollStrategy { Top, Center, Bottom, Nearest }
```

Programmatic scroll: pick an item index and a strategy. The list
element re-runs its prepaint at the chosen offset; no animation,
no physics.

[`crates/flui-core/src/elements/uniform_list.rs:751`](../../../crates/flui-core/src/elements/uniform_list.rs#L751)
also has hover-tracker autoscroll near edges — driven by a tick
inside the element, not by a generic physics engine.

### Animation simulation primitives

[`crates/flui-core/src/animation/simulation.rs`](../../../crates/flui-core/src/animation/simulation.rs)
contains spring and friction simulators (visible in the file
listing). These exist but are not wired to anything scroll-related;
they are general physics primitives the future scroll widget can
consume.

### What is *not* present

- No `Scrollable` widget — a re-usable container that takes any
  child and gives it pannable, fling-able overflow.
- No `ScrollPhysics` trait — the strategy object that converts
  gestures and momentum into offset trajectories.
- No platform-aware default physics (`BouncingScrollPhysics` on
  iOS / macOS, `ClampingScrollPhysics` on Android / Windows /
  Linux, web-native passive wheel handling).
- No axis-lock logic in pointer dispatch — `gesture/recognizers/`
  has tap, double-tap, drag, scale, long-press, but no
  pan-with-axis-lock specific to wheel/trackpad scrolling.
  That is the GPUI #40623 gap.

## Findings vs upstream

| Issue | 👍 | Why it maps |
|---|---|---|
| Flutter `f: scrolling` cluster | many | Every Flutter scrolling bug ultimately routes through `ScrollPhysics`; the cluster is the evidence that "scroll feel" needs a typed contract. |
| [zed-industries/zed#40623](https://github.com/zed-industries/zed/issues/40623) | n/a | macOS horizontal trackpad scroll does not block vertical — axis-lock missing from gesture dispatch. |
| [flutter/flutter#46070](https://github.com/flutter/flutter/issues/46070) | 2 | `Stack` relayout pessimistic w.r.t. `Positioned` — also a scroll-perf case via nested scrollable. |
| [flutter/flutter#182085](https://github.com/flutter/flutter/issues/182085) | 1 | Nested scroll inside `Stack` escapes — clipping interaction. |

## Decision (contract)

This is a **scoping** ADR — the constraints, not the implementation.

1. **`ScrollPhysics` is a trait, not an enum.** Two reasons:
   - Platform-specific physics (iOS bouncing, Android overscroll
     glow + clamp, web passive) cannot be encoded as a closed enum
     without future breakage.
   - Tests need a deterministic mock physics; a trait gives us
     that for free.

   The trait surface is approximately:

   ```rust
   pub trait ScrollPhysics {
       /// Apply the physics to a pending pointer delta. May reject
       /// the delta entirely (axis lock), modify it (rubber-band
       /// resistance at edges), or pass it through.
       fn apply_delta(&self, state: &ScrollState, delta: Offset) -> Offset;

       /// Build the simulator that will run after the user releases
       /// the gesture. Returns `None` if the physics has no fling.
       fn fling(&self, state: &ScrollState, velocity: Velocity)
           -> Option<Box<dyn Simulation>>;

       /// Should this physics allow overscroll past the edges
       /// during an active gesture?
       fn allows_overscroll(&self) -> bool;
   }
   ```

2. **Two reference implementations ship together: `BouncingPhysics`
   and `ClampingPhysics`.** The platform default is selected at
   widget creation time from a `Theme::scroll_physics_default`
   field. App code overrides per `Scrollable`.

3. **Axis lock lives in `ScrollPhysics::apply_delta`.** When the
   trackpad gesture has a dominant axis early on, `apply_delta`
   zeroes the orthogonal component for the duration of the
   gesture. This closes GPUI #40623 by making the rule data, not
   wired into the gesture recogniser.

4. **`Scrollable` is a single widget, not a family.** No
   `SingleChildScrollView` / `ListView` / `GridView` split at the
   engine level. Higher-level widgets compose `Scrollable` with a
   specific child layout. The engine knows about scroll *physics
   and offsets*; layout is the child's concern.

5. **Programmatic scroll integrates with physics.** Today
   `scroll_to_item(ix, strategy)` jumps immediately. The contract:
   `scroll_to_item(ix, strategy, animated: bool)`. When `animated`,
   the same simulation pipeline as fling runs; when not, the jump
   is one tick. This avoids two divergent animation paths.

6. **Scroll position is reactive.** A `ScrollHandle::position()`
   is observable via the same notify path as other state; widgets
   that depend on the scroll position rebuild via ADR-001 contract
   (per-view, not full-window).

7. **The animation budget composes with ADR-014 (software
   rendering).** Fling simulations run at the platform-permitted
   frame rate; on software fall-back the simulation step is larger
   per tick. The visual result degrades gracefully.

8. **Nested scrollables resolve by priority + axis.** When two
   scrollables claim the same gesture, the inner one wins **if** it
   can still consume the delta; otherwise the gesture bubbles. This
   is the same model as web `overscroll-behavior` and matches every
   modern toolkit. The decision is for the gesture-arena (see
   `crates/flui-core/src/gesture/arena.rs`) — `ScrollPhysics` only
   speaks for one scrollable at a time.

## Consequences

- A future `Scrollable` widget has a target shape — no
  research-during-implementation.
- The existing `UniformListScrollHandle` continues to work for
  imperative use. Adding physics-driven animation is additive;
  the imperative jump remains as `animated: false`.
- Axis-lock for trackpad pointers becomes a one-line override in
  `BouncingPhysics::apply_delta`; we close GPUI #40623 with a
  data change rather than a new gesture recogniser.
- Tests for scroll feel use a `MockPhysics` that records what is
  asked of it; visual correctness verified through deterministic
  simulation steps.
- The Flutter `f: scrolling` cluster becomes the evidence corpus
  for `BouncingPhysics` / `ClampingPhysics` corner cases as we
  implement them.

## Out of scope (own ADRs / future work)

- **The `Scrollable` widget implementation itself.** Belongs in a
  K-series spec or a flui-widgets task.
- **`SingleChildScrollView` / `CustomScrollView` / `Slivers`.**
  Higher-level widgets composed on top of `Scrollable`.
- **`ScrollNotification`-style event bubbling.** A flui-framework
  concern, not engine.
- **Page-snap physics** (`PageView`, `PagedListView`). Composed
  on top via a custom `ScrollPhysics::fling`.
- **Pull-to-refresh.** Built on top of overscroll + custom
  widget; not a physics primitive.
- **Smooth wheel-scroll on web** (passive vs active wheel
  listeners). Web-platform concern that crosses into ADR-016.

## Action items (tracked; no code lands with this ADR)

1. Add the `ScrollPhysics` trait, `ScrollState`, `BouncingPhysics`,
   and `ClampingPhysics` to `flui-core` as types (no consumer
   yet). The fact that they exist as published types pre-empts
   API divergence when the widget arrives.
2. Wire `Theme::scroll_physics_default()` selection in
   `flui-theme`. Default selection is platform-conditional; the
   `flui-theme` crate is the right home.
3. Extend `UniformListScrollHandle::scroll_to_item` with an
   `animated: bool` parameter; the existing call sites pass
   `false`. The animated path delegates to a fresh
   `Simulation` from the current physics.
4. Audit `gesture/recognizers/drag.rs` for axis-lock semantics on
   wheel/trackpad inputs; document where `ScrollPhysics::apply_delta`
   takes over.
5. Open a separate spec for the `Scrollable` widget itself once
   somebody picks it up.

## References

### Upstream
- [zed-industries/zed#40623](https://github.com/zed-industries/zed/issues/40623) — macOS trackpad horizontal scroll.
- [flutter/flutter#46070](https://github.com/flutter/flutter/issues/46070) — `Stack` relayout pessimistic.
- [flutter/flutter#182085](https://github.com/flutter/flutter/issues/182085) — nested scroll inside `Stack` escapes.
- Flutter `f: scrolling` label — too many issues to list; consult
  the snapshot.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md) — per-view notify path applies.
- [docs/research/adr/ADR-014-software-rendering-fallback.md](ADR-014-software-rendering-fallback.md) — frame budget interaction.
- [docs/research/adr/ADR-018-modal-overlay-layering.md](ADR-018-modal-overlay-layering.md) — hit-test interplay when scrollables overlap.
- [docs/research/flutter-cross-walk.md](../flutter-cross-walk.md) — "themes without an ADR" entry for scroll physics, now this ADR.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — open-ended; this ADR adds a new theme not in the original list.
