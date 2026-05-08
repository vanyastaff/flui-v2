# Migration Guide — S21 Animation Flutter Parity

**Date:** 2026-05-08
**Plan:** `.ai-factory/plans/animation-flutter-parity.md`
**Audience:** code that depends on `flui_core::animation` or
`flui_core::elements::animation` from before S21.

This guide collects the breaking changes introduced by S21 and the
mechanical migration steps. The full per-phase narrative lives in
`CHANGELOG.md` under the `## [Unreleased] — S21 Animation Flutter parity`
block.

## Quick reference

| Old (pre-S21)                              | New (post-S21)                                       |
| ------------------------------------------ | ---------------------------------------------------- |
| `flui_core::Animation` (struct)            | `flui_core::ElementAnimation` (struct)               |
| `flui_core::AnimationElement<E>`           | `flui_core::ElementAnimationElement<E>`              |
| `Animation::new(duration)`                 | `ElementAnimation::new(duration)`                    |
| `flui_core::animation::Curve` (enum)       | `flui_core::animation::Curve` (trait)                |
| `Curve::Linear`                            | `Curves::LINEAR` (catalogue) or `Linear` (struct)    |
| `Curve::EaseInOut`                         | `Curves::EASE_IN_OUT` or `EaseInOut`                 |
| `Curve::Bounce`                            | `Curves::BOUNCE_IN_OUT` or `BounceInOut`             |
| `Curve::Custom(arc)`                       | `CustomCurve(arc)` or `CustomCurve::new(closure)`    |
| `Curve::Elastic` (period 0.3 hardcoded)    | `Curves::ELASTIC_IN_OUT` (period 0.4) — **visual shape differs**, see Elastic period note |
| `controller.value()` (`f32`)               | `controller.value()` (`f32`, unchanged) — but the    |
|                                            | `Animation<f64>` trait method also exists; reach it  |
|                                            | via `Animation::value(&controller)` for `f64`        |
| `easing::linear(t)` (free fn)              | `Curves::LINEAR.transform(t)`                        |
| `easing::ease_in_out(t)`                   | `Curves::EASE_IN_OUT.transform(t)`                   |
| `easing::quadratic(t)`                     | `Curves::EASE_IN.transform(t)`                       |
| `easing::bounce(easing)` (combinator)      | `Curves::BOUNCE_IN_OUT` (closest case)               |
| `Tween::new(begin, end).transform(t: f32)` | unchanged, plus `Animatable<T>::transform(t: f64)`   |

## Breaking changes by phase

### Phase 0a — `Animation` struct renamed to `ElementAnimation`

**Why:** the new Flutter-parity `Animation<T>` trait owns the bare
`Animation` symbol at `flui_core::animation::Animation`. The old
element-level struct moved to `ElementAnimation` to free the name.

**What changed:**

- `pub struct Animation` → `pub struct ElementAnimation`
- `pub struct AnimationElement<E>` → `pub struct ElementAnimationElement<E>`
- `AnimationExt` trait keeps its name; only the struct it produces
  is renamed.

**Migration:**

```rust
// Before
use flui_core::{Animation, AnimationExt};
let anim = Animation::new(Duration::from_millis(300));

// After
use flui_core::{ElementAnimation, AnimationExt};
let anim = ElementAnimation::new(Duration::from_millis(300));
```

No deprecation shim ships — the rename is intentionally clean.

### Phase 1 — `Curve` enum → `Curve` trait

**Why:** Flutter's `Curve` is an open class hierarchy. An enum cannot
absorb new variants without breaking matches in user code; a trait
lets `flui-widgets` ship its own curves and lets users define custom
ones via `CustomCurve` or any struct implementing `Curve`.

**What changed:**

- `pub enum Curve { Linear, EaseIn, ..., Custom(Arc<Fn>), ... }`
  is gone.
- `pub trait Curve: Send + Sync + 'static` is the new shape, with
  required `transform_internal(t)`, optional analytical `derivative_at(t)`,
  and `clone_box(&self)` for `Box<dyn Curve>: Clone`.
- Concrete struct types per former variant: `Linear`, `EaseIn`,
  `EaseOut`, `EaseInOut`, `EaseInCubic`, `EaseOutCubic`,
  `EaseInOutCubic`, `Decelerate`, `BounceIn`/`Out`/`InOut`,
  `Cubic{x1,y1,x2,y2}`, `ElasticIn`/`Out`/`InOut` with explicit
  `period: f32` (default `0.4` matching Flutter), `Interval`,
  `Threshold`, `SawTooth`, `FlippedCurve`, `Reversed`, `Split`,
  `CustomCurve(Arc<Fn>)`.
- `Curves` catalogue (empty struct + assoc consts) mirrors Flutter's
  `Curves.linear` / `Curves.bounceOut` / etc. surface. SCREAMING_SNAKE
  per Rust idiom: `Curves::FAST_OUT_SLOW_IN`, `Curves::BOUNCE_IN_OUT`,
  `Curves::ELASTIC_IN_OUT`, `Curves::EASE`, …
- `AnimationController.curve` field: `Curve` (enum) →
  `Box<dyn Curve>`. Builder `controller.curve(C: Curve)` takes a
  generic + boxes internally.
- `ElementAnimation.curve` field: `Option<Curve>` →
  `Option<Box<dyn Curve>>`.

**Migration:**

```rust
// Before
controller.curve(Curve::EaseInOut);
controller.curve(Curve::Bounce);
controller.curve(Curve::Custom(Arc::new(|t| t * t * t)));

// After (catalogue style — recommended)
use flui_core::Curves;
controller.curve(Curves::EASE_IN_OUT);
controller.curve(Curves::BOUNCE_IN_OUT);

// After (bare struct — also works)
use flui_core::{EaseInOut, BounceInOut, CustomCurve};
controller.curve(EaseInOut);
controller.curve(BounceInOut);
controller.curve(CustomCurve::new(|t| t * t * t));
```

### Elastic period note (visual-shape change)

Pre-S21 `Curve::Elastic` was parameter-less and used a **hard-coded
`period = 0.3`**. Post-S21 `ElasticIn` / `ElasticOut` / `ElasticInOut`
take an explicit `period` field that defaults to `0.4` (Flutter parity).
The catalogue constants `Curves::ELASTIC_IN`, `Curves::ELASTIC_OUT`,
`Curves::ELASTIC_IN_OUT` use the `0.4` default. If you depended on the
exact pre-S21 oscillation pattern, construct the curve explicitly with
`ElasticIn { period: 0.3 }` (or the variant you used).

### `Curve::Spring` restoration

The pre-S21 `Curve::Spring { damping, stiffness }` enum variant was
inadvertently dropped during the enum→trait migration and restored in
the S21 review-fix pass. **Migration:** replace
`Curve::Spring { damping, stiffness }` with `Spring::new(damping, stiffness)`.
For default tuning use `Curves::SPRING` (damping=10, stiffness=100;
critically-damped fast settle).

For richer parametrisation (initial velocity, custom mass) use
`SpringSimulation` via `AnimationController::animate_with`.

### `Cubic` field privacy

`Cubic`'s `x1, y1, x2, y2` fields are now private. **Migration:**
replace `Cubic { x1, y1, x2, y2 }` literal construction with
`Cubic::new(x1, y1, x2, y2)` — the constructor `assert!`s
`x1, x2 ∈ [0, 1]` to catch invalid CSS-cubic-bezier inputs that would
otherwise misbehave in the Newton-Raphson solver.

### Phase 1.8b — Legacy `easing::*` free functions deprecated

The `pub fn linear / quadratic / ease_in_out / ease_out_quint /
bounce / pulsating_between` free functions in
`crates/flui-core/src/elements/animation.rs` are marked
`#[deprecated]`. Each carries a note pointing at the `Curves::*`
replacement (or `CustomCurve` when no direct catalogue equivalent
exists). Implementation bodies are preserved (do NOT clamp input
to `[0, 1]` — original behaviour) so existing call sites compile
with a deprecation warning until they are migrated.

**Migration:**

```rust
// Before
use flui_core::ease_in_out;
ElementAnimation::new(d).with_easing(ease_in_out);

// After
use flui_core::Curves;
ElementAnimation::new(d).curve(Curves::EASE_IN_OUT);
```

`bounce` (the combinator wrapper that mirrors a curve around 0.5),
`ease_out_quint`, and `pulsating_between` have no direct catalogue
equivalent. For `bounce`, `Curves::BOUNCE_IN_OUT` is the closest
shape. For `ease_out_quint` / `pulsating_between`, build a
`CustomCurve`.

## Non-breaking additions worth knowing

These are additive in S21 but you may want to adopt them.

### `Animation<T>` trait

`AnimationController` now implements `Animation<f64>`. The trait
gives you `.value() -> f64`, `.status()`,
`.add_listener` / `.remove_listener` / `.add_status_listener` /
`.remove_status_listener`. The inherent `controller.value() -> f32`
still exists and is preferred for f32 callers (Rust resolves inherent
methods over trait methods at the dot-call site).

For an f64 read, use UFCS: `Animation::value(&controller)`.

For raw listener subscription (separate from the existing
`cx.observe(&entity, ...)` pattern):

```rust
use flui_core::{Animation, ListenerCallback};

let id = controller.read(cx).add_listener(ListenerCallback::new(|| {
    // fired on every controller transition
}));
controller.read(cx).remove_listener(id);
```

`ListenerCallback` wraps an `Rc<dyn Fn()>` — the API takes the wrapper,
not a bare closure or `Rc`, so listener identity is opaque
(`ListenerId` rather than callback equality).

**Both subscription paths fan out from the same controller, so a
single transition fires both `cx.observe(&entity, ...)` listeners and
any raw `add_listener` listeners exactly once each.** They are
independent channels: `cx.observe` is the entity-aware path that
re-renders the host widget, while `add_listener` is the lower-level
path used inside combinators (`CurvedAnimation`, `ProxyAnimation`,
…). If you subscribe via *both* on the same controller you will see
two callbacks per transition (one per channel) — that is the same
shape as Flutter's `addListener` + `Listenable` overlap, not a bug.

### `Animatable<T>` + Tween family

`Tween<T: Lerp>` now implements `Animatable<T>` in addition to
its existing inherent `transform(t: f32)`. Use the trait for
composition:

```rust
use flui_core::{AnimatableExt, CurveTween, Curves, Tween};

let with_curve = Tween::new(0.0, 100.0)
    .chain(CurveTween::new(Curves::EASE_IN_OUT));
```

New tween types: `ConstantTween`, `ReverseTween`, `CurveTween`,
`IntTween`, `StepTween`, `ColorTween` (null-aware on
`Option<Hsla>`), `SizeTween`, `RectTween`, `TweenSequence`,
`FlippedTweenSequence`. See the rustdoc on each.

### Animation combinators

`AlwaysStoppedAnimation`, `ProxyAnimation`, `ReverseAnimation`,
`CompoundAnimation` (+ `animation_min` / `animation_max` /
`animation_mean`), `TrainHoppingAnimation`. All listener-forwarding
combinators clean up parent subscriptions in `Drop`.

### Controller polish

`with_behavior` / `with_style` builders, `animate_to(target, style)`,
`animate_back(target, style)`, `fling(velocity, behavior)`,
`velocity()` (three-branch with analytical / numerical fallback),
`BoundedFrictionSimulation`. See `flui_core::animation::AnimationStyle`
for the override bag.

## Verification commands

After migrating, run:

```
cargo check --workspace --all-features
cargo test -p flui-core --lib animation::
cargo run -p animation_demo
cargo run -p nav_demo --features transition
```

## Where to ask

- The full plan with rationales: `.ai-factory/plans/animation-flutter-parity.md`
- Per-phase change details: `CHANGELOG.md` under
  `## [Unreleased] — S21 Animation Flutter parity (in progress)`
- Roadmap entry: `.ai-factory/ROADMAP.md` (search for "S21")
