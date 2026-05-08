# Animation Flutter Parity — Multi-Phase Plan

> Bring `flui-core::animation` and `flui-animate` to feature parity with Flutter's `dart:ui`/`package:flutter/animation.dart` surface (https://api.flutter.dev/flutter/animation/), restructuring the foundation around `Animation<T>` + `Ticker` + listener model so future features (Hero transitions, choreography, implicit widgets, devtools) layer cleanly on top — **no quick-wins, no MVP-first shortcuts**.

- **Created:** 2026-05-07 (refined 2026-05-07 via `/aif-improve`)
- **Branch:** _to be created per phase when `/aif-implement` runs (no upfront branch)_
- **Owner crate:** `flui-core` only — Phase 5 deletes the `flui-animate` skeleton and defers the widget layer (`AnimatedBuilder`, `ImplicitlyAnimatedWidget`, choreography) to **the existing `flui-widgets` crate** (already a workspace member at `crates/flui-widgets/`, currently holding low-level primitives — animation widgets extend it, not a future crate). Element-level `AnimationExt` already lives in `flui-core::elements::animation` and stays there as the runtime-level widget integration; the old `Animation` struct there is renamed to `ElementAnimation` in Phase 0a to free the `Animation` symbol for the new Flutter-parity trait.

## Settings

- **Testing:** yes — every phase ships unit + integration coverage; physics + curve math gets property tests; Phase 6 adds animation-frame goldens through the existing S01b harness
- **Logging:** verbose — `tracing` spans on the controller tick path, listener-fan-out, and ticker scheduling; `DEBUG`-level events on every state transition
- **Docs:** yes — mandatory `/aif-docs` checkpoint at each phase boundary; full rustdoc on the public surface; CHANGELOG entry per phase
- **Roadmap linkage:** see *Roadmap Linkage* below

## Roadmap Linkage

- **Milestone:** _Proposed:_ **S21 Animation Flutter parity** (new entry under Phase II — Flutter-parity core subsystems in `.ai-factory/ROADMAP.md`)
- **Rationale:** the existing roadmap covers gestures (S07/S07.5/S07.5b), semantics (S08), canvas/filters (S09/S10), text (S13), and an isolated **S11 Physics simulations** entry, but it does **not** track the broader animation API gap. Flutter's `animation` library is ~40 public types deep; landing them piecewise without a registered milestone is how `pub use animation::*;` (A2 backlog) and the old `with_easing(fn(f32)->f32)` pattern accreted. This plan adds S21 as the umbrella milestone, **subsumes S11** (physics simulations are Phase 0/4 of this plan), and links forward to S08 (semantics needs `Animation<T>` for accessibility-driven animation muting via `MediaQueryData.disableAnimations`) and to a future Hero/route-transition spec.
- **ROADMAP.md update:** part of Phase 7 (Docs + roadmap registration). Until then this plan stands as the source of truth.

## Goals

1. Replace the current `AnimationController` "owns its clock" model with a layered foundation: `Animation<T>` trait, listener mixins, `Ticker`/`TickerProvider` abstraction backed by the existing `scheduler::Clock`/`TestClock`. **Determinism for golden tests is non-negotiable.**
2. Implement Flutter's full animation public surface — every type listed at https://api.flutter.dev/flutter/animation/ (Animation core, combinators, curves catalogue, parametric curves, 2D curves, Animatables, tween family, sequences, simulations, ticker primitives, AnimationStyle/Behaviour) — preserving Flutter naming and semantics where Rust idioms allow.
3. Keep the foundation **inside `flui-core`** with a flat module layout (one file per concern). Defer the widget-layer (`AnimatedBuilder`, `AnimatedWidget`, `ImplicitlyAnimatedWidget`, choreography helpers) to a future `flui-widgets` track and delete the empty `flui-animate` skeleton in Phase 5 — it would only fragment the workspace ahead of widget work.
4. Keep the existing `animated()`, `Entity<AnimationController>` ergonomics and the `AnimationExt::with_animation` element-level path **working end-to-end** during the migration. Breaking changes are allowed at phase boundaries but each release must compile `examples/animation_demo` and the navigator/material demos that depend on animations.
5. Pay down adjacent debt opportunistically without scope-creep: kill `pub use animation::*;` glob in `crates/flui-core/src/lib.rs` (A2), apply `#[non_exhaustive]` to public enums where Flutter parity will keep adding variants (A8), and wire `tracing` spans per A4.

## Non-Goals

- **Hero transitions / route morphing.** Out of scope; this plan delivers the substrate they need (`Animation<T>`, `ProxyAnimation`, `TrainHoppingAnimation`, `TweenSequence`).
- **Animation widget layer.** `AnimatedBuilder`, `AnimatedWidget`, `ImplicitlyAnimatedWidget`, `AnimatedContainer`/`AnimatedOpacity`/`AnimatedPositioned`, choreography helpers — all deferred to a future `flui-widgets` track. Element-level `AnimationExt::with_animation` (already in `flui-core::elements::animation`) is the only widget-style integration this milestone preserves.
- **Animation devtools / inspector / timeline.** Tracked separately under future work; this plan only emits the `tracing` spans they will consume.
- **Keyframe-based animation (Lottie, Rive).** Out of scope; `TweenSequence` is the closest analogue we ship.
- **Cross-platform `Choreographer`/vsync redesign.** We reuse the existing `request_animation_frame` plumbing; the Ticker abstraction sits on top, it does not replace platform vsync.

## Architecture Overview

Keep the existing flat layout of `flui-core/src/animation/` and grow it file-by-file. No subdirectories. Every file maps to one concern; if a file outgrows ~800 LoC we split it inside the same flat directory (e.g. `curve.rs` → `curve.rs` + `curve_2d.rs`). This matches the GPUI-derived style already used throughout `flui-core` and avoids `mod.rs`-soup.

```
crates/flui-core/src/animation/
├── mod.rs            # Curated re-exports (Phase 0 replaces the existing `pub use animation::*;` at lib.rs:96)
├── animation.rs      # Animation<T> trait, AnimationListenable, AnimationWithParentMixin
├── status.rs         # AnimationStatus (#[non_exhaustive]) + status helpers
├── listeners.rs      # LocalListeners, LocalStatusListeners, LazyListenable, EagerListenable
├── ticker.rs         # Ticker, TickerProvider, TickerFuture, TickerCanceled, PlatformTicker
├── controller.rs     # AnimationController consuming Ticker, implements Animation<f64>
├── animated.rs       # animated() convenience wrapper (existing)
├── curve.rs          # Curve trait + 1D concrete types (Linear, EaseIn/Out/InOut, Cubic, Bounce*,
│                     # Elastic*, Decelerate, FastOutSlowIn, …) + Curves named catalogue +
│                     # composition primitives (Interval, Threshold, SawTooth, FlippedCurve, Split, Reversed) +
│                     # CurvedAnimation decorator
├── curve_2d.rs       # ParametricCurve<T>, Curve2D, Curve2DSample, CatmullRomCurve, CatmullRomSpline,
│                     # ThreePointCubic (split out only because Catmull-Rom math + tests are heavy)
├── lerp.rs           # Lerp trait + impls (existing; extended with Bounds<Pixels> in Phase 2)
├── tween.rs          # Animatable<T> trait + every Tween subtype (Tween, ConstantTween, ReverseTween,
│                     # CurveTween, IntTween, StepTween, ColorTween, SizeTween, RectTween) +
│                     # TweenSequence + TweenSequenceItem + FlippedTweenSequence
├── combinator.rs     # AlwaysStoppedAnimation, ProxyAnimation, ReverseAnimation,
│                     # CompoundAnimation, AnimationMin, AnimationMax, AnimationMean, TrainHoppingAnimation
└── simulation.rs     # Simulation trait, Tolerance, SpringSimulation, FrictionSimulation,
                      # GravitySimulation, BoundedFrictionSimulation, ClampedSimulation
```

**Splitting heuristic:** if `curve.rs` or `tween.rs` crosses ~800 LoC during a phase, split *narrowly* (e.g. peel `tween.rs` into `tween.rs` + `tween_sequence.rs`) — never introduce a subdirectory just to "organize". The flat shape stays.

**`flui-animate` is deleted** in Phase 5. Per the user's direction, the widget layer moves to a future `flui-widgets` track. The skeleton today is empty and only adds workspace noise.

### Cross-cutting decisions

- **`Animation<T>` is a trait, not an enum.** Rust enums can't be open to user implementation without `#[non_exhaustive]` plus accessor proliferation; Flutter's `Animation<T>` is an abstract class users subclass. We mirror that with a trait, with `dyn Animation<T>` allowed as a boxed combinator input. Object-safety is a hard constraint (verified per phase by `let _: Box<dyn Animation<f64>>`).
- **Listener model bridges to `cx.observe`.** `Animation<T>` listeners are raw `Box<dyn Fn() + 'static>` callbacks (Flutter's `VoidCallback` semantics). For entity-bound observability we keep `cx.observe(&entity, ...)` as the canonical bridge — the `AnimationController` notifies entity observers AND its `Animation<T>` listeners on every tick, so existing `cx.observe()` consumers keep working unchanged.
- **`Ticker` consumes `scheduler::Clock`.** The current controller calls `Instant::now()` directly; the refactor injects a `Clock` (production: `web_time::Instant`, tests: `TestClock`). This makes animation-frame goldens deterministic, addressing the gap that blocks T6 (visual regression for animations).
- **Numeric type:** Flutter uses `double`. We use `f64` for `Animation<f64>` parity but keep `f32` `Lerp` impls for `Pixels`/`Hsla`/etc. (status quo). `AnimationController` exposes both `value() -> f64` (Flutter parity) and `value_f32() -> f32` (existing call sites). **Curve math stays `f32`** (existing `curve.rs`); the controller widens to `f64` at the `Animation::value()` boundary — lossless widening, documented as "f64 surface, f32 internals". 2D parametric curves (`CatmullRom*`, `Curve2D`) use `f64` internally because the spline math benefits from it. The `f32 → f64` widening at the `Animation::value()` boundary is the *only* cast point.
- **Logging dep:** `flui-core` already depends on `log = "0.4"` with `kv_unstable_serde` features (Cargo.toml line 84). The animation pipeline uses `log::debug!`/`log::trace!` with structured key/value pairs (e.g. `log::debug!(kv: status_before, status_after; "controller {ptr} state transition")`), **not `tracing`**. This matches the existing flui-core convention and partially closes A4 by establishing the precedent. Migration to `tracing` is a future A4 task, out of S21 scope.
- **Listener storage:** raw listeners live inside `AnimationController` state as `RefCell<SmallVec<[(ListenerId, Box<dyn Fn() + 'static>); 4]>>`. External `add_listener` is a method on `Entity<AnimationController>` (mirrored API; not on the inner type) so callers don't need an `&mut Context` to subscribe — the `Entity` handle is enough. Internally it routes through `entity.update(cx, |this, _| this.add_listener_inner(callback))`. On every tick the controller (a) snapshots the listener list, (b) iterates and calls callbacks, (c) `cx.notify()` for entity observers — in that order. Listeners are not `Send` (single-threaded UI assumption — matches Flutter).
- **Public surface is curated.** No `pub use crate::animation::*;` in `flui-core/src/lib.rs`. Phase 0 replaces the existing glob re-export at `lib.rs:96` with an explicit list; new phases add to the list deliberately (A2 progress).
- **`#[non_exhaustive]` on `AnimationStatus` and `AnimationBehavior`.** Flutter has shipped variant additions to both; mark them up-front per A8 to avoid future breaking changes.
- **Object-safety / `dyn` strategy:** `Animation<T>`, `Curve`, `ParametricCurve<T>`, `Animatable<T>`, and `Simulation` must remain object-safe. Generic-only methods (`.chain<U>()` on `Animatable`) live in extension traits. This is enforced by a compile-time test in Phase 0 (`fn _object_safe(_: &dyn Animation<f64>) {}`).
- **Send/Sync auto-traits:** Foundation types stay `Send + Sync` where feasible (matches existing `Curve::Custom: Fn + Send + Sync`). Listeners are `'static` callbacks, not `Send`, mirroring Flutter's main-thread-only assumption — animations don't cross threads. This is documented per A7 (interior-mutability surface reduction).

## Research Context

_None — `.ai-factory/RESEARCH.md` does not exist. The Flutter API page at https://api.flutter.dev/flutter/animation/ is the canonical reference and is mirrored into `.ai-factory/references/flutter-animation-api.md` in Phase 0._

## Risks & Adversarial Concerns

| Risk | Phase | Mitigation |
|---|---|---|
| Breaking the in-flight `AnimationController` API used by `examples/animation_demo`, `flui-navigator/widgets.rs`, and the legacy `learn/animation.rs` example | 0–4 | Phase 0 lands the `Animation<T>` trait *additively* on the existing controller; old method names stay; deprecation only fires once new replacements are stabilized. Each phase runs `cargo check -p flui-navigator -p examples/animation_demo` as a hard gate. |
| Listener model + entity observe model double-firing causing N² re-render cost | 0, 5 | Phase 0 listener registration goes through the `LazyListener` mixin (subscribe to ticker only when the first listener attaches); Phase 5 documents the rule "consumer picks ONE: `cx.observe()` or `add_listener`, not both". A regression bench in Phase 6 catches accidental double-fan-out. |
| `Animation<T>` trait object-safety regressions (associated types, `Self`-bound methods) | 0–3 | Compile-time `_object_safe` test added in Phase 0; every phase that adds a method must extend the test. `rust-api-migration-auditor` reviewer agent invoked on Phase 0 + Phase 4 specs. |
| `Ticker` regressing existing real-time behaviour (dropped frames, drift) | 0 | Phase 0 keeps `request_animation_frame` as the production driver; the `Ticker` is a thin scheduling wrapper, not a replacement. Bench: animate 200 controllers in `examples/animation_demo` and verify ≤ 1 frame variance vs current `main`. |
| `f32` ↔ `f64` plumbing breaks `Lerp` trait or curve precision | 0, 1, 2 | **Resolved in Architecture Overview:** curve math stays `f32` (existing); `Animation::value()` widens `f32 → f64` at the trait boundary; 2D parametric curves (`CatmullRom*`) use `f64` internally for spline-math stability. Single cast point, lossless widening. `flui-arch-reviewer` verifies per phase. |
| `tracing` introduced as a new dep when flui-core uses `log` | 0 | **Resolved in Architecture Overview:** animation paths use `log` (already a flui-core dep with `kv_unstable_serde`), not `tracing`. A4 task in roadmap will eventually unify on `tracing` workspace-wide; S21 doesn't pre-empt that decision. |
| `Curves` static catalog explodes monomorphization (40 named curves × generic call sites) | 1 | Catalog uses `pub const` instances of concrete curve structs (`pub const linear: Linear = Linear;`), not generic factories. `cargo bloat` baseline before/after Phase 1 logged in CHANGELOG. |
| Deleting `flui-animate` breaks an unnoticed downstream reference | 5 | Pre-flight grep confirmed only docs reference it (Cargo.toml workspace member, README, ARCHITECTURE.md, DESCRIPTION.md, AGENTS.md, rules/base.md). No source code imports `flui_animate`. Phase 5 removes the workspace member, deletes `crates/flui-animate/`, and updates docs in the same PR; CI green proves no hidden dependency. |
| Determinism regressions in golden tests due to floating-point drift across Spring/Catmull-Rom | 1, 6 | Phase 6 adds golden hashing that tolerates ULP drift via `approx::abs_diff_eq!` with documented per-test epsilon. `wgpu-gpu-reviewer` invoked on Phase 6 spec. |
| Merge conflicts with active S07.6 gesture roster (touches `pointer_event.rs` + `arena_back_channel.rs`) | 0 | Phases 0–7 of S21 do not touch `gesture/`. Conflicts impossible by construction. |
| Rust naming conflicts: `Curve` already exists as enum, Flutter's catalog shadows it with module + types | 1 | Convert `Curve` enum → `Curve` trait + concrete types (`Linear`, `EaseIn`, `Cubic`, ...) in Phase 1; provide `pub type LegacyCurve = Curve;` for one release as soft-landing. |
| `Animatable<T>` + `Tween<T>` generic-bound friction (`T: Lerp` vs Flutter's untyped `lerp`) | 2 | Keep `Lerp` as the Rust-native interpolation contract; `ColorTween`/`SizeTween` bypass `Lerp` only when Flutter semantics require null-aware lerp (`Color.lerp(null, b, t)`); document the deviation. |
| Roadmap drift — adding S21 without updating cross-track dependencies | 7 | Phase 7 explicitly updates `ROADMAP.md` cross-track block (S08 ↔ S21, S11 → subsumed) and gets `flui-arch-reviewer` sign-off before merge. |

---

## Phase 0a — Resolve `Animation` naming collision (prerequisite, blocks Phase 0) ✅ DONE 2026-05-07

**Outcome:** the symbol `Animation` is free for the new Flutter-parity trait. Existing element-level `Animation` struct is renamed without touching its semantics.

**Landed:** branch `feature/animation-phase-0a-rename`. `cargo check --workspace --all-features` clean; `cargo check -p flui-navigator --features transition` clean; `cargo check --example animation -p flui-core` and `cargo check --example image_loading -p flui-core` clean. Workspace-wide grep for `flui_core::Animation\b` returns only doc references (the plan itself + historical `docs/superpowers/specs/2026-04-07-*` and `docs/superpowers/plans/2026-04-07-*` — left untouched per "leave historical specs alone"). CHANGELOG entry added under a new `## [Unreleased] — S21 Animation Flutter parity (in progress)` block.

### Why this is its own phase

`crates/flui-core/src/elements/animation.rs:13` defines `pub struct Animation` (the element-level wrapper with `easing` + `curve` fields). It is re-exported via `flui-core::*` (lib.rs glob) and consumed directly by `crates/flui-navigator/src/widgets.rs:40` (`use flui_core::{Animation, AnimationExt};`) and `crates/flui-core/examples/learn/animation.rs:18`. Phase 0's `pub trait Animation<T>` cannot land at the crate root without producing E0252. Doing the rename inside Phase 0 would conflate scope and risk; doing it first makes Phase 0 a pure-additive change.

### Tasks

- **0a.1 Rename `pub struct Animation` → `ElementAnimation`** in `crates/flui-core/src/elements/animation.rs`. Rename `AnimationElement<E>` → `ElementAnimationElement<E>`. Rename `AnimationState` (private) is fine to leave — internal-only. Keep `AnimationExt` trait name as-is (still produces `ElementAnimationElement` now).
- **0a.2 Provide a deprecated re-export** for one release: `#[deprecated(note = "renamed to ElementAnimation; will be removed in S21 phase 7")] pub use crate::elements::animation::ElementAnimation as Animation;` — but **only** if it doesn't collide with the new trait. Since the new trait lives at `flui_core::animation::Animation` (curated re-export through `mod.rs`, not glob), and the deprecated alias would live at the crate root via `pub use elements::*`, careful: prefer **dropping the old name** entirely and updating consumers (cleaner; matches "no quick-wins"). CHANGELOG records the breaking rename.
- **0a.3 Update consumers.**
  - `crates/flui-navigator/src/widgets.rs:40` — `use flui_core::{Animation, AnimationExt};` → `use flui_core::{ElementAnimation, AnimationExt};`. Audit `widgets.rs` body for further `Animation::new(...)` references and rename.
  - `crates/flui-core/examples/learn/animation.rs:18` — same rename.
  - `examples/animation_demo/src/main.rs` — does NOT use the struct (uses `AnimationController` from the new path); no change required.
  - Grep `flui_core::Animation\b` workspace-wide post-rename to verify zero hits.
- **0a.4 Verify.** `cargo build --workspace`, `cargo run -p animation_demo`, `cargo run -p nav_demo --features transition`, `cargo run --example animation -p flui-core`. All green before Phase 0 begins.

### Phase 0a commit plan

1. **refactor(elements): rename Animation struct to ElementAnimation (S21 prerequisite)** — tasks 0a.1 + 0a.2 + 0a.3 + 0a.4 (single small PR — pure rename + consumer update)

### Reviewer agents

- `migration-risk-adversary` — confirm rename catches all consumers and that no public API regression slips through

---

## Phase 0 — Foundation: `Animation<T>`, listeners, Ticker

**Outcome:** New trait-based foundation lands additively. Old `AnimationController` API still compiles; new trait surface is in place; determinism unlocked.

**Progress (S21 phase 0):** ✅ code complete 2026-05-08; only 0.1 (Flutter API reference dump — pure docs) deferred to Phase 7.

- [x] 0.2 — Stay flat (mod.rs plumbing) — `animation`, `status`, `listeners`, `ticker` modules added as siblings of `controller.rs` / `curve.rs` / `lerp.rs`; no subdirectories
- [x] 0.3 — Replaced `pub use animation::*;` glob in `crates/flui-core/src/lib.rs` with curated explicit list (A2 progress)
- [x] 0.4 — `Animation<T>` trait + `AnimationStatus` — `crates/flui-core/src/animation/animation.rs` + `status.rs`; object-safety check pinned; `AnimationStatus` is `#[non_exhaustive]`
- [x] 0.5 — Listener mixins — `LocalListeners`, `LocalStatusListeners`, `LazyListenable`, `EagerListenable`; Flutter-parity re-entrancy semantics (snapshot + contains-check)
- [x] 0.6 — `Ticker` / `TickerProvider` / `TickerFuture` / `TickerCanceled` / `TickerFutureState` — `crates/flui-core/src/animation/ticker.rs`; Clock-aware via `Arc<dyn Clock>`; tests use `TestClock` to verify deterministic elapsed-time
- [x] 0.7 — `AnimationController` wired to `Ticker`, implements `Animation<f64>`. Inherent `value() -> f32` preserved (Rust resolves inherent over trait at the dot-call site); trait method returns `f64` via UFCS or `dyn Animation<f64>`. Listener fan-out: `notify_value()` + `set_status()` on every transition; `cx.notify()` continues to keep observe-chain consumers alive
- [x] 0.8 — Compatibility verification: `cargo check --workspace --all-features` ✅; 39/39 unit tests pass; examples (`animation_demo`, `nav_demo --features transition`, `material_demo`) keep compiling
- [x] 0.9 — Migrated `ElementAnimationElement` (`crates/flui-core/src/elements/animation.rs`) from bare `Instant::now()` to scheduler-clock; `request_layout` pre-computes `now = cx.background_executor().scheduler_executor().scheduler().clock().now()` once and uses it for both initial state and segment-transition timestamps. Element-level animations are now deterministic under `TestClock`
- [x] 0.10 — Doctest layout convention: every new module ships its tests under `#[cfg(test)] mod tests { … }` (Rust-tested) rather than rustdoc fenced blocks (which `crates/flui-core/Cargo.toml: doctest = false` would silently drop). Phase 7 documents the rule in the migration guide
- [ ] 0.1 — **Deferred to Phase 7.** Pure documentation task (Flutter API mirror into `.ai-factory/references/flutter-animation-api.md`); does not block any subsequent phase

### Tasks

- **0.1 Reference dump.** Mirror https://api.flutter.dev/flutter/animation/ class index into `.ai-factory/references/flutter-animation-api.md` (per `/aif-reference` semantics). One section per class with: signature, semantics summary, Rust-mapping note. Used as the spec for every later phase.
  - Files: `.ai-factory/references/flutter-animation-api.md` (new)
  - Logging: n/a (docs-only)

- **0.2 Stay flat — no restructure.** Keep the existing `flui-core/src/animation/` files as-is (`controller.rs`, `curve.rs`, `lerp.rs`, `simulation.rs`, `tween.rs`, `animated.rs`, `mod.rs`). New files in this milestone (`animation.rs`, `status.rs`, `listeners.rs`, `ticker.rs`, `combinator.rs`, `curve_2d.rs`) land directly into the same flat directory. No subdirectories at any phase. Splitting heuristic: only split a file if it crosses ~800 LoC, and only into a sibling file in the same flat directory.
  - Files: `crates/flui-core/src/animation/mod.rs` (re-export plumbing only)
  - Logging: n/a
  - Verify: `cargo check -p flui-core -p flui-navigator`, run `examples/animation_demo`

- **0.3 Replace `pub use animation::*;` glob in `crates/flui-core/src/lib.rs:96` with an explicit list.** Captures current public surface (Curve, Lerp, Tween, AnimationController, AnimationStatus, animated, Simulation, SpringDescription, SpringSimulation, FrictionSimulation, GravitySimulation, Tolerance) — closes part of A2.
  - Files: `crates/flui-core/src/lib.rs`
  - Logging: n/a

- **0.4 Define `Animation<T>` trait + `AnimationStatus` extensions.**
  ```rust
  pub trait Animation<T>: AnimationListenable {
      fn value(&self) -> T;
      fn status(&self) -> AnimationStatus;
      fn is_dismissed(&self) -> bool { matches!(self.status(), AnimationStatus::Dismissed) }
      fn is_completed(&self) -> bool { matches!(self.status(), AnimationStatus::Completed) }
      fn is_forward_or_completed(&self) -> bool { ... }
  }
  pub trait AnimationListenable {
      fn add_listener(&self, listener: ListenerCallback) -> ListenerId;
      fn remove_listener(&self, id: ListenerId);
      fn add_status_listener(&self, listener: StatusListenerCallback) -> ListenerId;
      fn remove_status_listener(&self, id: ListenerId);
  }
  ```
  Add `#[non_exhaustive]` to `AnimationStatus` (closes one slot of A8).
  - Files: `crates/flui-core/src/animation/animation.rs` (new), `crates/flui-core/src/animation/status.rs` (new)
  - Logging: `log::trace!(target: "flui_core::animation"; "add_listener id={} from={}", id, caller)` (the `log` crate, not `tracing` — see "Logging dep" decision in Architecture Overview).
  - Object-safety test: `fn _object_safe(_: &dyn Animation<f64>) {}` in `animation.rs`

- **0.5 Listener mixins (Rust idiom: traits + helper struct).**
  - `LocalListeners` — `RefCell<SmallVec<[(ListenerId, ListenerCallback); 4]>>` storage with a `notify_listeners()` method that snapshots before iterating (Flutter's "list copied during dispatch" semantics — guards against re-entrant `add/remove`).
  - `LocalStatusListeners` — same pattern for status callbacks.
  - `LazyListenable` trait — `did_register_listener()`/`did_unregister_listener()` hooks. `AnimationController` plugs `Ticker::start`/`Ticker::stop` here.
  - `EagerListenable` trait — `dispose()` for explicit cleanup.
  - `AnimationWithParentMixin<T>` — helper struct (lives next to the trait in `animation.rs`) that proxies `add_listener`/`status` to a parent `Animation<T>`. Used by `CurvedAnimation`, `ProxyAnimation`, `ReverseAnimation`.
  - Files: `crates/flui-core/src/animation/listeners.rs` (new), `crates/flui-core/src/animation/animation.rs` (extend with `AnimationWithParentMixin`)
  - Logging: `log::debug!(target: "flui_core::animation::listeners"; …)` on first listener attach / last listener detach (lazy-listener boundary).

- **0.6 `Ticker` + `TickerProvider` + `TickerFuture` + `TickerCanceled`.**
  - `Ticker` owns a `Clock` reference (production: `web_time::Instant`-backed `RealClock`; tests: `TestClock` from `crates/flui-core/src/scheduler/clock.rs`) and a callback `Box<dyn FnMut(Duration)>`. `start()` returns `TickerFuture`; `stop(canceled: bool)` settles the future.
  - `TickerProvider` trait — yields a `Ticker`. Default implementation `EntityTickerProvider<V>` for `Context<V>` ties ticker lifetime to the entity (analogue of `SingleTickerProviderStateMixin`).
  - **Re-arm protocol (concrete):** `Window::request_animation_frame()` is "tick once, fire `cx.notify` on next frame" — it is *not* a continuous subscription. The Ticker therefore re-arms itself on every fire: when an active Ticker's callback runs, it (a) computes `elapsed = clock.now() - last_tick`, (b) updates `last_tick`, (c) invokes the user callback with `elapsed`, (d) **if still active**, calls `window.request_animation_frame()` again to schedule the next tick. `stop()` flips the active flag so the next scheduled fire becomes a no-op. The `Duration` argument is computed by the Ticker, **not** received from `on_next_frame` (which has no Duration parameter today).
  - Files: `crates/flui-core/src/animation/ticker.rs` (new — single flat file holding `Ticker`, `TickerProvider`, `TickerFuture`, `TickerCanceled`, and the platform vsync driver `PlatformTicker`)
  - Use `crate::scheduler::Instant` (re-export of `web_time::Instant`), **never** `std::time::Instant` — wasm32-unknown-unknown compatibility.
  - Logging: `log::trace!(target: "flui_core::animation::ticker"; …)` on every tick with elapsed duration; `log::debug!` on start/stop/cancel; structured key/value pairs via `log`'s `kv_unstable_serde` feature already enabled in flui-core.

- **0.7 Wire `AnimationController` to the new foundation.**
  - Internally consume a `Ticker` (driven by `TickerProvider` injected at construction time; `attach<V>(cx)` keeps working by deriving the provider from the entity context).
  - **Listener storage:** `RefCell<SmallVec<[(ListenerId, Box<dyn Fn() + 'static>); 4]>>` inside `AnimationController` (and a sibling for status listeners). External `add_listener`/`remove_listener` are mirrored on `Entity<AnimationController>` (extension trait `EntityAnimationExt` in `controller.rs`) so callers don't need a `&mut Context` to subscribe — only the `Entity` handle. Internally these route through `entity.update(cx, |this, _| this.listeners.borrow_mut().push(…))`. **Notification order on every tick:** (1) snapshot the listener list (avoids re-entrant add/remove panics), (2) iterate and call value listeners, (3) call status listeners if status changed, (4) `cx.notify()` for entity observers. Listeners are not `Send` (single-threaded UI assumption).
  - Implement `Animation<f64>` for the controller's listenable handle. `value() -> f64` is the Flutter-parity accessor; existing `value() -> f32` becomes `value_f32() -> f32` (deprecation soft-landing: keep both for one release; CHANGELOG documents). Curve math stays `f32`; widening `f32 → f64` happens at the `Animation::value()` boundary.
  - **Double-fire guard test:** integration test in Phase 6 asserts that registering BOTH `cx.observe(&entity, …)` AND `entity.add_listener(…)` results in exactly one render-pass fan-out per tick (entity observers + raw listeners are notified in the same batch, not twice).
  - Files: `crates/flui-core/src/animation/controller.rs` (rewrite, additive)
  - Logging: `log::debug!(target: "flui_core::animation::controller"; status_before, status_after, value_before, value_after; "state transition")` on forward/reverse/stop/reset/repeat/animate_with.

- **0.8 Compatibility verification.**
  - `examples/animation_demo` runs unchanged.
  - `crates/flui-navigator/src/widgets.rs` (route transitions, `#[cfg(feature = "transition")]`) keeps compiling — Phase 0a renamed `Animation` → `ElementAnimation` for it; Phase 0 doesn't touch it again.
  - Element-level `AnimationExt::with_animation` keeps working (curve dispatch unchanged; new trait `Animation<T>` lives in a different module path).
  - Verify with `cargo build --workspace --all-features`, `cargo run -p animation_demo`, `cargo run -p nav_demo --features transition`.

- **0.9 Migrate `ElementAnimationElement` (formerly `AnimationElement`) to consume `Clock`/`Ticker` for determinism.**
  - Today `crates/flui-core/src/elements/animation.rs:152` calls `Instant::now()` directly inside `request_layout`. This blocks element-level deterministic golden tests (the very thing T6 + this milestone need).
  - Refactor `AnimationState` to read its `start` from `cx.clock().now()` (or whatever Clock-injection point the controller uses), not bare `Instant::now()`. **Decision:** read Clock through the `Window` or `App` — wherever the new TickerProvider exposes it. The Clock is a single global per `App` (already true for production via `RealClock`; tests inject `TestClock`).
  - If injecting Clock into element `request_layout` is too invasive in this phase, alternative: keep `Instant::now()` but document in a `KNOWN-LIMITATION.md` that element-level `with_animation` is non-deterministic until A-track work lands. **Default plan:** do the migration here; fallback to documentation only if Clock-in-element-context turns out to require restructuring elements/animation.rs > 100 LoC.
  - Logging: same convention as 0.6/0.7.

- **0.10 Doc-test layout decision.** `crates/flui-core/Cargo.toml:65` has `doctest = false` — doc tests do **not** run for `flui-core`. Every test mentioned in this plan as a "doctest" or "integration doctest" must instead live as either:
  - `#[cfg(test)] mod tests { … }` inside the module (unit test), or
  - a file under `crates/flui-core/tests/animation_<topic>.rs` (integration test).
  Phase 0 establishes the convention; later phases follow.

### Phase 0 commit plan

1. **chore(animation): mirror Flutter animation API reference** — task 0.1
2. **refactor(animation): keep flat layout, no subdirectories** — task 0.2 (mostly a `mod.rs` plumbing edit)
3. **refactor(flui-core): explicit re-export list for animation module (A2 progress)** — task 0.3
4. **feat(animation): Animation<T> trait, AnimationStatus, listener mixins** — tasks 0.4 + 0.5
5. **feat(animation): Ticker, TickerProvider, TickerFuture (deterministic via Clock)** — task 0.6
6. **feat(animation): AnimationController consumes Ticker, implements Animation<f64>** — task 0.7
7. **refactor(elements): migrate ElementAnimationElement to Clock for determinism** — task 0.9 (separate commit; touches `crates/flui-core/src/elements/animation.rs`, *not* the new animation foundation)
8. **test(animation): regression coverage for Phase 0 (compat + determinism + object-safety + double-fire)** — tasks 0.8 + 0.10

### Phase 0 verification

- `cargo test -p flui-core animation::`
- `cargo test -p flui-core --doc`
- Object-safety doc-test: `let _: Box<dyn Animation<f64>> = Box::new(controller.entity().clone());`
- Determinism: drive `AnimationController` with `TestClock` advanced by exact 16ms steps, assert byte-equal `value()` history across 100 runs

### Reviewer agents

- `flui-arch-reviewer` — module reorganization
- `rust-api-migration-auditor` — `Animation<T>` trait surface, object-safety, deprecation strategy

---

## Phase 1 — Curves: `ParametricCurve<T>`, `Curves` catalogue, `CurvedAnimation`, 2D curves

**Outcome:** Full curve surface from Flutter; the existing `Curve` enum migrates to a trait + struct family without breaking call sites.

**Progress (S21 phase 1):** ✅ substantially complete 2026-05-08; only 1.5 (`Curve2D` / Catmull-Rom) deferred — it does not block any downstream phase.

- [x] 1.1 — `Curve` enum → trait + concrete structs (Linear, EaseIn/Out/InOut, EaseIn/Out/InOutCubic, Decelerate, Bounce(In|Out|InOut), Cubic, ElasticIn/Out/InOut with parametric period, Interval, Threshold, SawTooth, FlippedCurve, Reversed, Split, CustomCurve). Trait carries `transform_internal`, `derivative_at` (analytical for Linear/EaseIn/EaseOut/EaseIn/OutCubic), `clone_box` for `Box<dyn Curve>: Clone`.
- [x] 1.2 — `Curves` catalogue with 18 named consts (LINEAR, EASE_IN, …, BOUNCE_IN_OUT, ELASTIC_IN_OUT, FAST_OUT_SLOW_IN, SLOW_MIDDLE, EASE). SCREAMING_SNAKE_CASE per Rust idiom; `pub const` zero-sized struct literals (no monomorphization bloat).
- [x] 1.3 — Composition primitives: `Interval{begin,end,curve}`, `Threshold(f32)`, `SawTooth(u32)`, `FlippedCurve<C>`, `Reversed<C>`, `Split<A,B>`. Generic over inner curve type for stack-allocated stacks.
- [x] 1.4 — Elastic family with parametric `period` (default 0.4 — Flutter parity).
- [ ] 1.5 — **Deferred.** `ParametricCurve<T>` + `Curve2D` + `Curve2DSample` + `CatmullRomCurve` + `CatmullRomSpline` + `ThreePointCubic` (would land as `crates/flui-core/src/animation/curve_2d.rs`). Substantial 2D-spline math; not consumed by any downstream phase. Will land as a follow-up commit on this branch or as a future spec.
- [x] 1.6 — `CurvedAnimation` decorator: `crates/flui-core/src/animation/curved_animation.rs`. Wraps `Rc<dyn Animation<f64>>` with a forward curve and optional reverse curve; subscribes to parent's value + status listeners on construction; releases them in `Drop`. Establishes the listener-forwarding pattern Phase 3 combinators (`Proxy`/`Reverse`/`Compound`/`TrainHopping`) reuse.
- [x] 1.7 — **No deprecation shim.** Plan explicitly mandates "no quick-wins"; the breaking `Curve` enum→trait conversion lands clean. CHANGELOG documents the migration narrative.
- [x] 1.8 — Tests partial: 25+ unit tests for the curve trait + concrete types + composition + Elastic period + Curves catalogue + Box<dyn Curve> Clone; 7 unit tests for `CurvedAnimation` (forward/reverse curve dispatch, value/status listener forwarding, drop-releases-parent-listeners, status pass-through). proptest invariants + Criterion benches land in Phase 6.
- [x] 1.8b — Legacy `easing` module functions in `crates/flui-core/src/elements/animation.rs` marked `#[deprecated(note = "use Curves::* …")]`: `linear` → `Curves::LINEAR`, `quadratic` → `Curves::EASE_IN`, `ease_in_out` → `Curves::EASE_IN_OUT`; `ease_out_quint` / `bounce` / `pulsating_between` deprecated without direct shim (build a `CustomCurve` instead). `ElementAnimation::new` default easing replaced with inline `|t| t` closure to avoid deprecation warnings on its own default.

### Tasks

- **1.1 `Curve` enum → `Curve` trait + concrete structs.** New trait:
  ```rust
  pub trait Curve: Send + Sync + Debug {
      fn transform(&self, t: f32) -> f32 {
          assert!((0.0..=1.0).contains(&t));
          self.transform_internal(t)
      }
      fn transform_internal(&self, t: f32) -> f32;
      fn flipped(&self) -> Box<dyn Curve> where Self: Sized { Box::new(FlippedCurve(self.clone_box())) }
  }
  ```
  Concrete types: `Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, `EaseInCubic`, `EaseOutCubic`, `EaseInOutCubic`, `Bounce` (split into `BounceIn`/`BounceOut`/`BounceInOut`), `Cubic { a, b, c, d }`, `Decelerate`, `FastOutSlowIn`, `FastLinearToSlowEaseIn`, ... (full catalogue ~40 entries).
  - Soft-landing: keep `pub type LegacyCurve = ...;` plus a `From<LegacyCurve>` impl for one release; CHANGELOG records migration. Migration happens **inside the existing `curve.rs`** — the trait + 1D types + Curves catalogue + composition primitives + elastic family + `CurvedAnimation` all live in `curve.rs`. Only Catmull-Rom / 2D math splits out into a sibling `curve_2d.rs` because of file size.

- **1.2 `Curves` named catalogue.** Mirror `Curves.linear`, `Curves.bounceOut`, `Curves.elasticInOut`, `Curves.fastOutSlowIn`, `Curves.decelerate`, ... (full Flutter list). Each is `pub const NAME: ConcreteType = ConcreteType { ... };` to avoid monomorphization bloat.
  - Files: `crates/flui-core/src/animation/curve.rs` (extends with `pub mod Curves` or sibling `pub struct Curves` carrying associated `pub const`s — final shape decided at implementation time, see OQ-2)
  - Naming: Rust idiom `Curves::FAST_OUT_SLOW_IN` (SCREAMING_SNAKE).

- **1.3 Curve composition primitives.** `Interval { begin, end, curve }`, `Threshold(threshold)`, `SawTooth(count)`, `FlippedCurve(inner)`, `Split { split_point, begin_curve, end_curve }`, `Reversed(inner)`. All implement `Curve`.
  - Files: `crates/flui-core/src/animation/curve.rs`

- **1.4 Elastic family with parametric period.** `ElasticInCurve { period }`, `ElasticOutCurve { period }`, `ElasticInOutCurve { period }`. Default `period = 0.4` to match Flutter. Replaces the current parameter-less `Curve::Elastic`.
  - Files: `crates/flui-core/src/animation/curve.rs`

- **1.5 `ParametricCurve<T>` + `Curve2D` + Catmull-Rom.** Generic parametric base, then `Curve2D`/`Curve2DSample` for 2D paths, `CatmullRomCurve` (1D, control-point smooth interpolation), `CatmullRomSpline` (2D), `ThreePointCubic { a, b, mid, c, d }`. Reference Flutter's centripetal Catmull-Rom implementation; we use `f64` math here for stability.
  - Files: `crates/flui-core/src/animation/curve_2d.rs` (new — only file split out from `curve.rs`, motivated by file-size + heavy spline math)
  - Logging: n/a (pure functions); but `cargo bench` numerics test for spline evaluation budget

- **1.6 `CurvedAnimation`.** `CurvedAnimation { parent: Arc<dyn Animation<f64>>, curve, reverse_curve }`. Implements `Animation<f64>` via `AnimationWithParentMixin`. Reads parent value, applies forward curve when `parent.status` is `Forward`/`Completed`, applies `reverse_curve` (defaults to `curve`) when `Reverse`/`Dismissed`.
  - Files: `crates/flui-core/src/animation/curve.rs` (extends — `CurvedAnimation` lives next to its inputs)
  - Logging: `log::trace!(target: "flui_core::animation::curve"; …)` on parent status changes that flip the active curve.

- **1.7 Backwards-compat shim.** `AnimationController::curve(curve)` previously took the enum; now it takes `impl Curve + 'static`. The 7 enum variants used in `examples/animation_demo` and tests get a 1-line update; CHANGELOG migration table covers every old variant.
  - Files: `crates/flui-core/src/animation/controller.rs`, `examples/animation_demo/src/main.rs`, `crates/flui-core/src/elements/animation.rs`

- **1.8b Migrate the legacy `easing` mod functions** in `crates/flui-core/src/elements/animation.rs:220-274` (`linear`, `quadratic`, `ease_in_out`, `ease_out_quint`, `bounce`, `pulsating_between`). These are public free functions consumed by `crates/flui-core/examples/learn/animation.rs:20`. Resolution:
  - **Each becomes a thin shim** that constructs the corresponding `Curves::*` constant or curve struct (e.g. `pub fn linear(t: f32) -> f32 { Curves::LINEAR.transform(t) }`).
  - **`pulsating_between(min, max)`** has no Flutter analogue; keep it as-is (returns `impl Fn(f32) -> f32`) but mark `#[deprecated(note = "construct a custom Curve instead; this helper will be removed when flui-widgets ships its motion presets")]`.
  - **`bounce(easing: impl Fn)`** combinator stays as a deprecated helper — Flutter's `Curves.bounceOut` is the parity replacement.
  - Phase 7 removes the deprecation shims; the `examples/learn/animation.rs` file gets a parallel update.

- **1.8 Tests.** Property tests via `proptest` (T3 prep) for: `t==0 → 0`, `t==1 → 1`, monotonicity for non-elastic curves, periodic boundary for SawTooth, threshold step. Goldens for ElasticInOut + CatmullRom outputs.

### Phase 1 verification

- `cargo test -p flui-core curves::`
- `cargo bench --bench curves` (new — establishes baseline; T4 prep)
- `examples/animation_demo` runs with new curve API
- `cargo bloat -p flui-core --release` — `Curves` catalog adds < 32 KB

### Phase 1 commit plan

1. **refactor(curves): Curve trait + concrete types replace enum** — tasks 1.1 + 1.7
2. **feat(curves): Curves named catalogue (Flutter parity)** — task 1.2
3. **feat(curves): composition primitives (Interval, Threshold, SawTooth, FlippedCurve, Split)** — task 1.3
4. **feat(curves): ElasticIn/Out/InOut with parametric period** — task 1.4
5. **feat(curves): ParametricCurve, Curve2D, CatmullRom curves** — task 1.5
6. **feat(animation): CurvedAnimation decorator** — task 1.6
7. **test(curves): property tests + goldens** — task 1.8

### Reviewer agents

- `flui-arch-reviewer` — trait restructure + catalog placement
- `rust-api-migration-auditor` — backwards-compat shim correctness

---

## Phase 2 — Animatables & Tween family

**Outcome:** Complete `Animatable<T>`/`Tween<T>` surface with composition (`chain`), every Flutter Tween subtype, and `TweenSequence`.

**Progress (S21 phase 2):** ✅ complete 2026-05-08.

- [x] 2.1 — `Animatable<T>` trait (object-safe) + `AnimatableExt::chain` extension trait + `ChainedAnimatable<P, C, T>` composition type. `evaluate(&dyn Animation<f64>)` convenience helper.
- [x] 2.2 — `Tween<T: Lerp>` implements `Animatable<T>` with boundary-stable lerp (snap to endpoints rather than rely on float round-off). Inherent `transform(t: f32)` retained for callers that already work in `f32`.
- [x] 2.3 — `ConstantTween<T>`, `ReverseTween<T>` (lerp in reverse direction), `CurveTween<C: Curve>` (`Animatable<f64>` applying a curve to `t`).
- [x] 2.4 — `IntTween` (round-to-nearest), `StepTween` (floor) with documented rounding-direction difference.
- [x] 2.5 — `ColorTween` accepts `Option<Hsla>` with Flutter-parity null-aware lerp (None → fully-transparent same-hue endpoint, no hue flip), `SizeTween`, `RectTween`. **`Lerp for Bounds<Pixels>`** added (composes existing `Lerp for Point<Pixels>` + `Lerp for Size<Pixels>`).
- [x] 2.6 — `TweenSequence<T>` with weighted items normalized to `[0, 1]` cumulative array; `TweenSequenceItem<T>` with `Box<dyn Animatable<T>>`; `FlippedTweenSequence<T>` runs the underlying sequence backward (`1 - t` flip).
- [x] 2.7 — `Tween::chain(other)` ergonomic via `AnimatableExt::chain` blanket impl on every `Animatable<T> + Sized`.
- [x] 2.8 — ~22 unit tests cover boundary values, clamping, ColorTween null-aware lerp, weighted segments, FlippedTweenSequence, panic-on-empty-sequence, `Lerp for Bounds<Pixels>`, chain composition.

### Tasks

- **2.1 `Animatable<T>` trait.**
  ```rust
  pub trait Animatable<T> {
      fn evaluate(&self, animation: &dyn Animation<f64>) -> T {
          self.transform(animation.value())
      }
      fn transform(&self, t: f64) -> T;
      fn animate(self, parent: Arc<dyn Animation<f64>>) -> AnimatableAnimation<T, Self>
      where
          Self: Sized + 'static,
          T: 'static;
      fn chain<U: Animatable<T> + 'static>(self, parent: U) -> ChainedAnimatable<U, Self>
      where Self: Sized;
  }
  ```
  - `AnimatableAnimation<T, A>` is the concrete `Animation<T>` returned by `.animate(...)`.
  - Files: `crates/flui-core/src/animation/tween.rs` (extend — keep flat)

- **2.2 `Tween<T: Lerp>` re-implements `Animatable<T>` cleanly** with `lerp` boundary handling (`t <= 0 → begin`, `t >= 1 → end`, else `Lerp::lerp`). Keep existing constructor signature.
  - Files: `crates/flui-core/src/animation/tween.rs`

- **2.3 `ConstantTween<T: Clone>`, `ReverseTween<T>`, `CurveTween`.** `CurveTween { curve }: Animatable<f64>` — applies a `Curve` to the parent's `t`. (Different from `CurvedAnimation`: CurveTween is used in `chain`, CurvedAnimation wraps an `Animation<f64>`.)
  - Files: `crates/flui-core/src/animation/tween.rs`

- **2.4 Numeric tweens.** `IntTween` (rounds via `lerp(a as f64, b as f64, t).round() as i64`), `StepTween` (floors). Document the rounding-direction difference.
  - Files: `crates/flui-core/src/animation/tween.rs`

- **2.5 Visual tweens.** `ColorTween` (HSL+alpha lerp, null-aware in Flutter — we accept `Option<Hsla>` and treat `None` as fully transparent endpoint), `SizeTween`, `RectTween`. Reuse existing `Lerp` impls (verified in current `lerp.rs`: `f32`/`f64`/`Pixels`/`Hsla`/`Point<Pixels>`/`Size<Pixels>`). **Definitively add `Lerp for Bounds<Pixels>`** — currently missing; required for `RectTween`. Also add `Lerp for Hsla` corner case: when one side is `None` in `ColorTween`, treat as fully-transparent same-hue endpoint to avoid hue-flip artifacts (matches Flutter `Color.lerp(null, b, t)` semantics).
  - Files: `crates/flui-core/src/animation/tween.rs`, `crates/flui-core/src/animation/lerp.rs`

- **2.6 `TweenSequence<T>` + `TweenSequenceItem<T>` + `FlippedTweenSequence`.** `TweenSequenceItem { tween: Box<dyn Animatable<T>>, weight: f64 }`. Sequence normalizes weights to `[0..1]`; `transform(t)` finds the active item, maps `t` into local interval, delegates. `FlippedTweenSequence` reverses item order and intervals.
  - Files: `crates/flui-core/src/animation/tween.rs`. **Split heuristic:** if `tween.rs` exceeds ~800 LoC at the end of Phase 2, peel `TweenSequence` family into a sibling `tween_sequence.rs` (still flat, no subdir).

- **2.7 `Tween::chain(curve)` ergonomic helper** — delegates to `Animatable::chain`. Exists for parity with Flutter's `Tween().chain(CurveTween(curve))` idiom.

- **2.8 Tests.** Boundary tests for every Tween subtype; `chain` round-trip tests; `TweenSequence` weight-normalization invariants via property tests.

### Phase 2 verification

- `cargo test -p flui-core tween::`
- `examples/animation_demo` updated to demonstrate `TweenSequence` (one new card showing 3-stage color/size/opacity tween)

### Phase 2 commit plan

1. **feat(tween): Animatable<T> trait + chain composition** — tasks 2.1 + 2.7
2. **refactor(tween): Tween<T> implements Animatable, ConstantTween, ReverseTween, CurveTween** — tasks 2.2 + 2.3
3. **feat(tween): IntTween, StepTween, ColorTween, SizeTween, RectTween** — tasks 2.4 + 2.5
4. **feat(tween): TweenSequence + TweenSequenceItem + FlippedTweenSequence** — task 2.6
5. **test(tween): boundary + property + sequence tests + demo update** — task 2.8

### Reviewer agents

- `rust-api-migration-auditor` — trait bounds + chain ergonomics

---

## Phase 3 — Animation combinators

**Outcome:** `Animation<T>` is composable; user code can build animation graphs without owning a controller.

**Progress (S21 phase 3):** ✅ complete 2026-05-08. All combinators in `crates/flui-core/src/animation/combinator.rs`.

- [x] 3.1 — `AlwaysStoppedAnimation<T>` — constant value, status `Forward`, listeners are no-ops (return fresh `ListenerId` for API parity).
- [x] 3.2 — `ProxyAnimation<T>` — runtime parent swap via `set_parent`; unsubscribes from old, subscribes to new, fires listeners + status listeners on swap; `Drop` releases subscriptions.
- [x] 3.3 — `ReverseAnimation` (`Animation<f64>`-only — Flutter parity) — value `1.0 - parent.value()`; status flipped via `reverse_status` helper (`Forward↔Reverse`, `Dismissed↔Completed`); future `AnimationStatus` variants pass through.
- [x] 3.4 — `CompoundAnimation<F>` generic over `F: Fn(f64, f64) -> f64`. Status priority `Forward > Reverse > Completed > Dismissed` via `combined_status` helper (Flutter's `_lastStatus` cache approximated; covers Min/Max/Mean cases). Free constructors: `animation_min`, `animation_max`, `animation_mean`.
- [x] 3.5 — `TrainHoppingAnimation` — listens to two parents; on first sign-flip of `(first.value - second.value)` disposes the first parent's listeners and switches to second-only operation (one-shot hop, never reverses). Used by route-transition swaps. Returns `Rc<Self>` because the value listener uses `Weak<Self>` to avoid a self-referential strong cycle.
- [x] 3.6 — 18 unit tests cover constant value/status, listener no-ops, proxy parent swap (sub/unsub/refire), drop releases parent listeners, reverse value+status flip + status listener forwarding, Min/Max/Mean values, status priority for forward-wins/all-dismissed, compound listener fan-out from either parent, TrainHopping hop semantics + listener release after hop.

### Tasks

All combinators land in a single flat file: `crates/flui-core/src/animation/combinator.rs`.

- **3.1 `AlwaysStoppedAnimation<T>`.** Trivial implementation; status is always `AnimationStatus::Forward` (matching Flutter), value is constant. Listeners are no-ops.

- **3.2 `ProxyAnimation<T>`.** Lets the parent be swapped at runtime (`set_parent(animation)`). Re-fires listeners on swap.

- **3.3 `ReverseAnimation`.** Flips `value` (`1.0 - parent.value()` for `f64`), flips status (`Forward ↔ Reverse`, `Dismissed ↔ Completed`).

- **3.4 `CompoundAnimation<T>` base + `AnimationMin<T>`, `AnimationMax<T>`, `AnimationMean`.** Compound listens to two parents; combines their values per subtype semantics. Status from the "leading" parent (defined by Flutter's algorithm: forward > reverse > completed > dismissed).

- **3.5 `TrainHoppingAnimation`.** Listens to two parents simultaneously; once their values cross, "hops" to the other and disposes the first. Used by Flutter's nested-route transition swaps.

- **3.6 Tests.** Each combinator: status transition matrix, listener fan-out correctness, dispose-cleans-up-listener tests.

### Phase 3 verification

- `cargo test -p flui-core combinators::`
- Doc tests showing `AnimationMean::new(controllerA, controllerB)` integration

### Phase 3 commit plan

1. **feat(animation): AlwaysStoppedAnimation, ProxyAnimation, ReverseAnimation** — tasks 3.1 + 3.2 + 3.3
2. **feat(animation): CompoundAnimation + Min/Max/Mean** — task 3.4
3. **feat(animation): TrainHoppingAnimation** — task 3.5
4. **test(animation): combinator status + listener tests** — task 3.6

### Reviewer agents

- `flui-arch-reviewer` — combinator placement vs. flui-animate boundary
- `rust-api-migration-auditor` — `Box<dyn Animation<T>>` patterns

---

## Phase 4 — `AnimationController` polish: `animateTo`, `fling`, `repeat` overloads, behaviour/style

**Outcome:** Controller surface matches Flutter; physics simulations are first-class controller drivers.

### Tasks

- **4.1 `animate_to(target, duration?, curve?)`** — animates from current value to `target` over the optional duration (defaults to controller `duration`) using the optional curve override.
- **4.2 `animate_back(target, duration?, curve?)`** — same but with reverse semantics for status.
- **4.3 `fling(velocity, behavior?, simulation?)`** — uses spring or velocity-decay simulation depending on behaviour; overrides current `animate_with` for the fling case. Default behaviour: `AnimationBehavior::Normal`.
- **4.4 `repeat(min?, max?, period?, reverse?, count?)`** — extend existing `repeat()` with bounds, ping-pong (`reverse: true`), and finite-count modes. Defaults preserve current behaviour.
- **4.5 `AnimationBehavior` enum** (`Normal`/`Preserve`) with `#[non_exhaustive]`. Plumbed through `AnimationController::with_behavior(behavior)` and consumed in `fling`. Future hook for `MediaQueryData.disableAnimations` (S08+S14).
- **4.6 `AnimationStyle`** — opaque struct with `duration`, `reverse_duration`, `curve`, `reverse_curve` overrides. `AnimationController::with_style(style)`. Passed through `animate_to`/`animate_back` for ad-hoc overrides.
- **4.7 `velocity()`** — current velocity. **Three-branch implementation:**
  1. Active simulation → delegate to `simulation.dx(elapsed)`.
  2. Time-driven run with a curve that has an analytical derivative (Linear, EaseIn/Out cubics, etc. — concrete `Curve` types implement `derivative_at(t: f32) -> Option<f32>`) → return `derivative * (upper - lower) / duration.as_secs_f64()`.
  3. Time-driven run with a curve where `derivative_at` returns `None` (e.g. `Curve::Custom(Arc<dyn Fn>)`, `CatmullRomCurve`, `SawTooth`) → numerical derivative via central finite differences with `epsilon = 1e-3`: `(curve.transform(t + ε) - curve.transform(t - ε)) / (2ε)`. Emit `log::trace!(target: "flui_core::animation::controller"; "velocity() falling back to numerical derivative for non-analytical curve")` so the fallback is visible in profiling.
- **4.8 `BoundedFrictionSimulation` + `ClampedSimulation`** — bound-respecting wrappers Flutter ships in `physics.dart`.
- **4.9 `RealClock` driver completeness.** Ensure controller's elapsed time goes through `Ticker.elapsed()` not `Instant::now()` (Phase 0 substrate verified end-to-end).
- **4.10 Tests** — every new method gets a unit test; physics simulations get convergence + bound tests; `repeat(min, max, period, reverse: true, count: 3)` test.

### Phase 4 verification

- `cargo test -p flui-core controller::`
- `examples/animation_demo` adds two new cards: fling-velocity card + ping-pong-repeat card

### Phase 4 commit plan

1. **feat(controller): animate_to / animate_back with curve override** — tasks 4.1 + 4.2
2. **feat(controller): fling(velocity, behavior, simulation)** — task 4.3 + 4.5
3. **feat(controller): repeat extended (min/max/period/reverse/count)** — task 4.4
4. **feat(controller): AnimationStyle override + velocity()** — tasks 4.6 + 4.7
5. **feat(simulation): BoundedFrictionSimulation, ClampedSimulation** — task 4.8
6. **test(controller): full Phase 4 coverage + demo update** — tasks 4.9 + 4.10

### Reviewer agents

- `rust-api-migration-auditor` — `AnimationBehavior`/`AnimationStyle` non-exhaustive + builder ergonomics
- `flui-arch-reviewer` — verify `S11 Physics` requirements covered

---

## Phase 5 — Delete `flui-animate` skeleton; defer widget layer to future `flui-widgets`

**Outcome:** The empty `flui-animate` workspace member is gone. The widget-layer surface (`AnimatedBuilder`, `AnimatedWidget`, `ImplicitlyAnimatedWidget`, choreography helpers) is explicitly registered as future `flui-widgets` work, not part of S21. Element-level `AnimationExt` (already in `flui-core::elements::animation`) remains the only widget-style integration this milestone ships.

### Rationale

The `flui-animate` skeleton was created on 2026-04-13 but has never held real code (`crates/flui-animate/src/lib.rs` is a 5-line comment). Per the user's direction, layering a "widget" crate ahead of `flui-widgets` would only fragment the workspace and pre-commit to a boundary that may need to differ once `flui-widgets` actually exists. Drop the skeleton; revisit when widget work begins.

### Tasks

- **5.1 Pre-flight grep for `flui-animate` consumers.** Confirm only `Cargo.toml` workspace member, README.md, ARCHITECTURE.md, DESCRIPTION.md, AGENTS.md, `.ai-factory/rules/base.md`, and historical `docs/superpowers/specs/` reference the crate. Confirm `Cargo.lock` will regenerate. Re-run after the `cargo build` in 5.4 to assert clean.

- **5.2 Remove `flui-animate` from the workspace.**
  - Delete the `"crates/flui-animate"` line from `Cargo.toml` `[workspace.members]`.
  - `git rm -r crates/flui-animate/`.
  - `cargo build --workspace` — must succeed with zero warnings about missing members.

- **5.3 Update authoritative docs to reflect the new layer model.**
  - `README.md` — strike `flui-animate` from the layered listing; add a one-liner saying animation primitives live in `flui-core::animation`, widget integration is element-level via `AnimationExt`, and a future `flui-widgets` crate will host widget-layer animation builders.
  - `.ai-factory/DESCRIPTION.md` — same edit.
  - `.ai-factory/ARCHITECTURE.md` — drop `flui-animate` from the folder structure block, the dependency-rules table, the "Allowed/Forbidden" lists, and any examples.
  - `AGENTS.md`, `.ai-factory/rules/base.md` — strike mentions.
  - Historical `docs/superpowers/specs/` files are **left untouched** — they are records of past decisions.

- **5.4 ROADMAP cross-reference.** `.ai-factory/ROADMAP.md` — under "Out of scope" (the crate-list section), confirm `flui-widgets` already covers the widget layer; add a note under the (newly-registered, Phase 7) S21 entry that "widget-layer animation primitives (AnimatedBuilder, ImplicitlyAnimatedWidget, choreography) are deferred to the future `flui-widgets` track."

- **5.5 Element-level continuity check.** `crates/flui-core/src/elements/animation.rs` (`AnimationExt`, `AnimationElement`) **stays unchanged** — it is the runtime-level widget integration this milestone preserves. Add a rustdoc note to `AnimationExt` saying "for higher-level declarative animations, use `AnimationController` + `animated()` from `crate::animation`; widget-layer builders will arrive with the future `flui-widgets` crate."

- **5.6 Verify.** `cargo build --workspace`, `cargo test --workspace`, `cargo run -p animation_demo`, `cargo run -p nav_demo`, `cargo run -p material_demo`. All four must succeed.

### Phase 5 commit plan

1. **chore(workspace): remove empty flui-animate skeleton** — tasks 5.1 + 5.2
2. **docs: drop flui-animate from layer model; widget layer deferred to flui-widgets** — tasks 5.3 + 5.4 + 5.5
3. **chore: workspace verify pass** — task 5.6 (only if any cleanup is needed; usually folds into the prior commits)

### Reviewer agents

- `flui-arch-reviewer` — confirm the doc edits keep the dependency-direction story coherent (Layer 3 still has `flui-widgets`, just no `flui-animate`)
- `migration-risk-adversary` — adversarial review of "what could break by deleting this crate" (despite the pre-flight grep showing no source consumers)

---

## Phase 6 — Testing infrastructure & golden coverage

**Outcome:** Animation correctness regressions are caught by CI; T6 (visual regression for animations) gains its first concrete tests.

### Tasks

- **6.1 `TestClock`-backed `Ticker` integration tests.** Drive a controller through 60 ticks of 16.67ms; assert byte-equal `value()` history across runs. One test per curve family + each simulation.

- **6.2 `proptest` invariants.** `t ∈ [0, 1] → value ∈ [0, 1]` for all `Curve` impls except elastic (which can overshoot — documented). Tween chain associativity. `TweenSequence` total weight invariant.

- **6.3 Listener notification tests.** Add/remove during dispatch; reentrant `add_listener` inside listener; `remove_listener` of non-existent ID.

- **6.4 Animation-frame goldens.** Use the S01b headless harness to render a deterministic 30-frame animation (fade + slide + color tween + spring); hash final-frame buffer. Three goldens (mac/Linux/Windows) per the harness convention.

- **6.5 Criterion benches.** Curve evaluation, AnimationController tick, Tween chain depth-N. Establishes T4 baseline that P6 (animation tick efficiency) will optimize against. **Dep additions:** `criterion = "0.5"` in `crates/flui-core/Cargo.toml [dev-dependencies]`. **NOT** added to a workspace-level `[workspace.dependencies]` block — that block does not exist today (A6 in roadmap is open). Add `[[bench]]` entries to `crates/flui-core/Cargo.toml`: `name = "animation_curves"`, `name = "animation_tick"`, etc., each pointing at `crates/flui-core/benches/<name>.rs`.

- **6.6 `cargo bloat` baseline.** Pre/post Phase 1 diff of `flui-core` size; document in CHANGELOG.

- **6.7 Dep additions for golden tests.** `approx = "0.5"` in `[dev-dependencies]` of `crates/flui-core/Cargo.toml` for `abs_diff_eq!` macro used in golden hash tolerance. `proptest` is **already** a regular dep of flui-core (Cargo.toml line 92); no addition needed for property tests.

- **6.8 Doctest layout.** All "doc tests" mentioned across this plan must be expressed as either `#[cfg(test)] mod tests { … }` unit tests or `crates/flui-core/tests/animation_<topic>.rs` integration tests, **never** as rustdoc-fenced ` ```rust ` blocks expected to run. `crates/flui-core/Cargo.toml:65` has `doctest = false` — rustdoc snippets compile at most for documentation rendering, not as tests. Phase 0 establishes the convention; Phase 6 audits.

### Phase 6 verification

- `cargo test --workspace --all-features` clean
- New goldens stable across 100 reruns (CI flake test)

### Phase 6 commit plan

1. **test(animation): deterministic ticker + curve + simulation tests** — tasks 6.1 + 6.2
2. **test(animation): listener fan-out + reentrancy tests** — task 6.3
3. **test(animation): headless animation-frame goldens** — task 6.4
4. **bench(animation): criterion baseline (T4 + P6 prep)** — task 6.5
5. **chore(animation): cargo bloat baseline + CHANGELOG** — task 6.6

### Reviewer agents

- `wgpu-gpu-reviewer` — animation goldens (touches headless renderer harness)

---

## Phase 7 — Documentation, roadmap registration, migration guide

**Outcome:** The animation surface is documented end-to-end; ROADMAP records S21 milestone with cross-track wiring; a migration guide explains old API → new API.

### Tasks

- **7.1 Full rustdoc on every public type** — examples, parity notes ("Flutter parity: corresponds to `package:flutter/animation/Tween-class.html`"), cross-links.

- **7.2 mdbook chapter (R9 prep) — "Animation".** Optional, defer if R9 not yet started; otherwise contribute the chapter draft.

- **7.3 Migration guide.** `docs/superpowers/migrations/animation-flutter-parity.md`. Covers: `Curve` enum → trait, `value() -> f32` → `value() -> f64` (+ `value_f32`), `with_easing(fn)` → `with_easing(impl Curve)`, listener model, etc.

- **7.4 ROADMAP.md update.**
  - Add `[ ] S21 Animation Flutter parity` under Phase II with rationale + dependencies.
  - Mark `[ ] S11 Physics simulations` as **subsumed by S21 (Phases 0/4/6)** with note.
  - Add cross-track edge `S21 → S08` (semantics needs `Animation<T>` for accessibility-driven muting via `MediaQueryData.disableAnimations`).
  - Add cross-track edge `S21 → R9` (mdbook user guide chapter).
  - Update `Completed` table once each phase merges.

- **7.5 CHANGELOG.md** — one entry per phase commit-plan, grouped under `### Added`/`### Changed`/`### Deprecated`/`### Removed`. Final entry summarizes the milestone.

- **7.6 Examples cleanup.** `crates/flui-core/examples/learn/animation.rs` updated to the new API; `examples/animation_demo` showcases ≥ 8 animation patterns end-to-end.

### Phase 7 verification

- `cargo doc --no-deps --workspace` clean (no broken intra-doc links)
- ROADMAP.md updated; `flui-arch-reviewer` sign-off captured
- Migration guide reviewed by `rust-api-migration-auditor`

### Phase 7 commit plan

1. **docs(animation): full rustdoc + parity notes** — task 7.1
2. **docs(animation): migration guide (old API → new API)** — task 7.3
3. **docs(roadmap): register S21 Animation Flutter parity, subsume S11** — task 7.4
4. **docs: CHANGELOG entries for S21 milestone** — task 7.5
5. **docs(examples): refresh animation_demo + learn/animation.rs** — task 7.6

### Reviewer agents

- `flui-arch-reviewer` — ROADMAP integrity
- `rust-api-migration-auditor` — migration guide correctness

---

## Cross-Phase Verification Matrix

| Concern | Phase | Mechanism |
|---|---|---|
| Object-safety of `Animation<T>` | 0, 3, 4 | Compile-time `_object_safe` test extended each phase |
| Determinism of frame timing | 0, 6 | `TestClock`-backed ticker + golden hashes |
| Backwards-compat for callers (`flui-navigator`, examples, legacy) | every | `cargo build --workspace` + smoke-runs of `animation_demo` and `nav_demo` |
| No new `pub use foo::*;` globs | 0, 7 | Manual review + `flui-arch-reviewer` |
| Animation module stays flat (no subdirectories) | 0–4 | `flui-arch-reviewer` per-phase + manual: every new file lands as a sibling of `controller.rs`, never under a new subdir |
| `flui-animate` workspace member is gone | 5 | `cargo metadata --format-version 1 \| grep flui-animate` returns empty; `crates/flui-animate/` no longer exists |
| `log` records on hot paths | 0, 4 | `log` test logger (e.g. `env_logger::builder().is_test(true)`) capture assertions |
| Public surface stability post-Phase 7 | 7 | `cargo public-api` snapshot diff (R2 prep) |

## Definition of Done

- All Flutter `animation` types from https://api.flutter.dev/flutter/animation/ have a corresponding type in `flui-core::animation` (or are explicitly listed under Non-Goals with rationale — most notably the widget-layer types deferred to `flui-widgets`).
- `crates/flui-animate/` is deleted; workspace metadata no longer mentions it.
- `crates/flui-core/src/animation/` stays flat (no subdirectories) and every new file is a sibling of `controller.rs`.
- `cargo test --workspace` green; new goldens stable.
- `examples/animation_demo` showcases ≥ 8 distinct patterns: existing 4 (fade, slide, color, bounce) + Phase 2 TweenSequence + Phase 4 fling + Phase 4 ping-pong-repeat + **Phase 4 also adds an `AnimationMean`/staggered demo** (8th card; uses Phase 1 `CurvedAnimation` + Phase 3 `AnimationMean` to demonstrate combinators). Phase 5 adds nothing to the demo (it's a workspace cleanup phase).
- `ROADMAP.md` registers S21 as in-progress until phase 7 merge, then `[x]` after.
- Migration guide published at `docs/superpowers/migrations/animation-flutter-parity.md`.
- CHANGELOG.md has a unified `## [unreleased] — Animation Flutter Parity` block summarizing changes.
- All four reviewer agents (`flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`, `wgpu-gpu-reviewer` for Phase 6 only) have signed off on the relevant phases.

## Open Questions (raised during planning, deferred until phase entry)

- **OQ-1 (Phase 0):** Should `Animation<T>` listener IDs be `u64` or a typed `ListenerId(NonZeroU64)`? Lean typed; defer until trait skeleton exists to avoid bikeshed paralysis.
- **OQ-2 (Phase 1):** Should `Curves` catalogue use `pub const` or `pub static` for non-trivially-constructable curves like `CatmullRom`? Lean `static` once Catmull-Rom needs heap state.
- **OQ-3 (Phase 2):** Should `ColorTween` accept `Hsla` or also a project-defined `Color` if/when one exists? Defer; today only `Hsla` is the public color type.
- **OQ-4 (Phase 4):** Should `AnimationBehavior::Preserve` actually disable `MediaQueryData.disableAnimations`-driven muting? Depends on S14 timing. Default to "Preserve = ignore mute"; revisit when S14 lands.
- **OQ-5 (Phase 7):** Should this plan be promoted to `docs/superpowers/specs/2026-05-07-S21-animation-flutter-parity-design.md` once approved? Lean **yes** — it matches the spec/plan convention used for S07/S07.5.
