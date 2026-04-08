# Spec B: Animation System

**Date:** 2026-04-07
**Status:** Approved
**Scope:** Flutter-level animation system in flui-core — Curve, Lerp, Tween, Simulations, AnimationController, animated() wrapper

---

## Goals

Add a two-level animation system to flui-core:
1. **Element-scoped** — extend existing `AnimationExt` with `Curve` enum (simple CSS-like animations)
2. **Entity-scoped** — `AnimationController` state container for complex forward/reverse/repeat/physics animations

## Non-Goals

- Animation groups / choreography helpers
- Hero transitions
- `flui-animate` crate (everything goes in flui-core)
- Animation devtools / inspector
- Keyframe animations

---

## 1. Curve Enum

Defines easing functions as a zero-allocation enum. Standard variants use `match` dispatch. Custom closures via `Arc<dyn Fn>`.

```rust
pub enum Curve {
    // Standard easing
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,

    // Dramatic
    Bounce,
    Elastic,

    // Parametric
    Spring { damping: f32, stiffness: f32 },
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },

    // Composition
    Interval { begin: f32, end: f32, curve: Box<Curve> },
    Reversed(Box<Curve>),

    // Custom
    Custom(Arc<dyn Fn(f32) -> f32 + Send + Sync>),
}

impl Curve {
    /// Transform t (0.0..1.0) through this curve.
    pub fn transform(&self, t: f32) -> f32;
}
```

### Staggered Animations via Interval

`Curve::Interval` maps a subset of the parent timeline to 0.0..1.0, enabling staggered animations with a single controller. This is the equivalent of Flutter's `Interval(0.2, 0.8, curve: Curves.easeIn)`.

Example — 3 items appearing in sequence:
```rust
let controller = AnimationController::new(Duration::from_millis(900)).attach(cx);

// Item 1: animates during 0%–33% of timeline
let curve1 = Curve::Interval { begin: 0.0, end: 0.33, curve: Box::new(Curve::EaseOut) };
// Item 2: animates during 33%–66%
let curve2 = Curve::Interval { begin: 0.33, end: 0.66, curve: Box::new(Curve::EaseOut) };
// Item 3: animates during 66%–100%
let curve3 = Curve::Interval { begin: 0.66, end: 1.0, curve: Box::new(Curve::EaseOut) };

// In render:
animated(&controller, window, cx, |t| {
    column()
        .child(div().opacity(curve1.transform(t)).child("Item 1"))
        .child(div().opacity(curve2.transform(t)).child("Item 2"))
        .child(div().opacity(curve3.transform(t)).child("Item 3"))
})
```

**File:** `src/animation/curve.rs`

---

## 2. Lerp Trait + Tween

### Lerp trait

```rust
pub trait Lerp: Clone {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}
```

Implementations for MVP: `f32`, `f64`, `Pixels`, `Hsla`, `Point<Pixels>`, `Size<Pixels>`.

Users can impl `Lerp` for their own types.

### Tween

```rust
pub struct Tween<T: Lerp> {
    pub begin: T,
    pub end: T,
}

impl<T: Lerp> Tween<T> {
    pub fn new(begin: T, end: T) -> Self;
    pub fn transform(&self, t: f32) -> T;
}
```

`Hsla` lerp interpolates h/s/l/a components linearly.

**Files:** `src/animation/lerp.rs`, `src/animation/tween.rs`

---

## 3. Physics Simulations

### Simulation trait

```rust
pub trait Simulation: Send + Sync {
    /// Position at time t (seconds).
    fn x(&self, t: f32) -> f32;
    /// Velocity at time t (seconds).
    fn dx(&self, t: f32) -> f32;
    /// Whether the simulation is complete at time t.
    fn is_done(&self, t: f32) -> bool;
}
```

### Tolerance

```rust
pub struct Tolerance {
    pub distance: f32,  // default: 0.001
    pub velocity: f32,  // default: 0.001
}

impl Default for Tolerance {
    fn default() -> Self {
        Self { distance: 0.001, velocity: 0.001 }
    }
}
```

Spring `is_done()` checks: `|x(t) - end| < tolerance.distance && |dx(t)| < tolerance.velocity`. This prevents infinite oscillation.

### SpringSimulation

Damped harmonic oscillator. Solves the ODE: `m*x'' + c*x' + k*x = 0`.

```rust
pub struct SpringDescription {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDescription {
    /// Create from damping ratio (1.0 = critically damped).
    pub fn with_damping_ratio(mass: f32, stiffness: f32, ratio: f32) -> Self;
}

pub struct SpringSimulation {
    start: f32,
    end: f32,
    velocity: f32,
    spring: SpringDescription,
    tolerance: Tolerance,
}
```

### FrictionSimulation

Exponential deceleration for fling momentum.

```rust
pub struct FrictionSimulation {
    drag: f32,
    position: f32,
    velocity: f32,
    tolerance: Tolerance,
}
```

### GravitySimulation

Constant acceleration (parabolic motion).

```rust
pub struct GravitySimulation {
    acceleration: f32,
    position: f32,
    velocity: f32,
    end: f32,
    tolerance: Tolerance,
}
```

**File:** `src/animation/simulation.rs`

---

## 4. AnimationController

Pure state container. Does NOT tick itself — parent view drives rendering via `animated()` wrapper.

```rust
pub struct AnimationController {
    value: f32,
    status: AnimationStatus,
    duration: Duration,
    reverse_duration: Option<Duration>,
    lower_bound: f32,        // default: 0.0
    upper_bound: f32,        // default: 1.0
    curve: Curve,
    start_time: Option<Instant>,
    start_value: f32,
    simulation: Option<Box<dyn Simulation>>,
}

pub enum AnimationStatus {
    Dismissed,  // at lower bound, idle
    Forward,    // animating toward upper
    Reverse,    // animating toward lower
    Completed,  // at upper bound, idle
}
```

### API

```rust
impl AnimationController {
    pub fn new(duration: Duration) -> Self;
    pub fn curve(mut self, curve: Curve) -> Self;
    pub fn lower_bound(mut self, v: f32) -> Self;
    pub fn upper_bound(mut self, v: f32) -> Self;
    pub fn reverse_duration(mut self, d: Duration) -> Self;

    /// Create Entity + auto-observe parent for re-render. Kills the boilerplate.
    /// Called from parent's Context — `cx` is `&mut Context<ParentView>`.
    pub fn attach<V: 'static>(self, cx: &mut Context<V>) -> Entity<Self>;

    // -- State reading (recalculates from elapsed time, no caching) --
    // TODO: consider per-frame caching if animated() is called multiple times
    pub fn value(&self) -> f32;
    pub fn is_animating(&self) -> bool;
    pub fn status(&self) -> AnimationStatus;

    // -- Control (each calls cx.notify()) --
    pub fn forward(&mut self, cx: &mut Context<Self>);
    pub fn reverse(&mut self, cx: &mut Context<Self>);
    pub fn toggle(&mut self, cx: &mut Context<Self>);
    pub fn repeat(&mut self, cx: &mut Context<Self>);
    pub fn stop(&mut self, cx: &mut Context<Self>);
    pub fn reset(&mut self, cx: &mut Context<Self>);

    /// Drive with physics simulation (spring, friction, gravity).
    pub fn animate_with(
        &mut self,
        simulation: impl Simulation + 'static,
        cx: &mut Context<Self>,
    );
}
```

### `.attach(cx)` helper

Eliminates boilerplate of `cx.new()` + `cx.observe()` + `.detach()`:

```rust
impl AnimationController {
    pub fn attach<V: 'static>(self, cx: &mut Context<V>) -> Entity<Self> {
        let entity = cx.new(|_| self);
        cx.observe(&entity, |_, _, cx| cx.notify()).detach();
        entity
    }
}
```

### How it ticks

AnimationController does NOT call `window.request_animation_frame()`. The `animated()` wrapper does that. Controller is a pure state machine:

1. `forward(cx)` → sets `start_time = now`, `status = Forward`, calls `cx.notify()`
2. `cx.notify()` → parent view re-renders (via observe chain from `.attach()`)
3. Parent calls `animated(&ctrl, window, cx, |v| ...)` → reads `value()`, schedules next frame if animating
4. Next frame → parent re-renders → reads updated `value()` → repeat until done

**File:** `src/animation/controller.rs`

---

## 5. `animated()` Wrapper

Convenience function that reads controller value and automatically schedules frame updates. Users never need to know about `window.request_animation_frame()`.

```rust
pub fn animated<E: IntoElement>(
    controller: &Entity<AnimationController>,
    window: &mut Window,
    cx: &mut App,
    builder: impl FnOnce(f32) -> E,
) -> impl IntoElement {
    let ctrl = controller.read(cx);
    let value = ctrl.value();
    let animating = ctrl.is_animating();
    drop(ctrl);

    if animating {
        window.request_animation_frame();
    }

    builder(value)
}
```

### Usage

```rust
struct FadeView {
    controller: Entity<AnimationController>,
}

impl FadeView {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            controller: AnimationController::new(Duration::from_millis(300))
                .curve(Curve::EaseInOut)
                .attach(cx),
        }
    }
}

impl Render for FadeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        animated(&self.controller, window, cx, |opacity| {
            div()
                .opacity(opacity)
                .child("Hello, animated world!")
        })
    }
}
```

**File:** `src/animation/animated.rs`

---

## 6. Extend Existing AnimationExt

The existing `Animation` struct in `src/elements/animation.rs` uses `easing: fn(f32) -> f32`. Extend to accept `Curve` enum:

```rust
// Existing field:
pub easing: fn(f32) -> f32,

// Add alternative:
pub fn curve(mut self, curve: Curve) -> Self {
    self.easing = move |t| curve.transform(t);
    // OR store Curve directly and use it in AnimationElement
    self
}
```

Backward compatible — existing `easing` API continues to work. New `.curve()` method provides the enum-based API.

**File:** `src/elements/animation.rs` (modify existing)

---

## 7. Files Summary

### New files:
| File | Contents |
|------|----------|
| `src/animation/mod.rs` | Module root, re-exports |
| `src/animation/curve.rs` | Curve enum (15+ variants) |
| `src/animation/lerp.rs` | Lerp trait + impls (f32, Pixels, Hsla, Point, Size) |
| `src/animation/tween.rs` | Tween\<T: Lerp\> |
| `src/animation/simulation.rs` | Simulation trait, Spring/Friction/Gravity, Tolerance |
| `src/animation/controller.rs` | AnimationController + .attach() + AnimationStatus |
| `src/animation/animated.rs` | animated() convenience wrapper |

### Modified files:
| File | Change |
|------|--------|
| `src/elements/animation.rs` | Add `.curve()` method to existing `Animation` struct |
| `src/lib.rs` | `pub mod animation;` + re-exports |

---

## 8. Testing

- Unit: Curve::transform() for all variants (Linear, EaseIn, Bounce, etc.) — verify 0→0, 1→1, monotonic where expected
- Unit: Curve::Interval — verify subsection mapping, stagger pattern
- Unit: Lerp impls — f32, Pixels, Hsla interpolation correctness
- Unit: Tween::transform() with various t values
- Unit: SpringSimulation — verify convergence, is_done with tolerance
- Unit: FrictionSimulation — verify deceleration, is_done
- Unit: AnimationController state machine — forward/reverse/stop/reset transitions
- Unit: AnimationController.value() — correct interpolation over time
- Integration: animated() wrapper schedules frames correctly
