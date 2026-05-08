---
title: flui-core — Master Architecture Plan & Agent Prompt
status: draft
audience: ai-agent + human-architect
purpose: drive the long-term architectural rework of flui-core toward Flutter-parity, hybrid widget tree, and 60 FPS as a design property
last-updated: 2026-05-08
---

# flui-core — Master Architecture Plan

> **How to use this document.**
>
> This is a self-contained brief for an AI coding agent (or human architect) tasked with
> evolving `crates/flui-core/` into a Flutter-parity, Rust-idiomatic UI framework that
> hits 60 FPS as a structural property — not as an after-the-fact optimization.
>
> The document combines: (1) project context, (2) hard constraints, (3) the target
> architecture, (4) a prioritized issue catalog with `file:line` references, (5) a
> phased execution plan, and (6) explicit done criteria.
>
> **Operating mode for the agent:**
> 1. Read sections 1–5 fully **before** proposing or writing any code.
> 2. When picking a task, cite the exact issue ID (e.g. `A3`, `G7`, `E5`) so reviewers
>    can trace each commit back to a documented problem.
> 3. Each commit must respect the constraints in §2. If a constraint blocks the work,
>    surface it as a question rather than silently bypassing.
> 4. New design docs go to `docs/superpowers/specs/` following the format already used
>    by `2026-04-13-S01a1-…-design.md`.
> 5. No code without a spec for any change with the labels **HIGH-RISK** or
>    **API-BREAKING** in §8.

---

## 1. Project context

### 1.1 What `flui-core` is

`flui-core` is the runtime kernel of `flui-v2`, a Flutter-inspired GPU-accelerated UI
framework for Rust. It is a fork of `gpui-ce` (the community edition of Zed's GPUI)
and inherits its core abstractions:

- **Entity-based state** (`Entity<T>` + `App` + `Context<T>`)
- **Element tree** (`Element` trait, `Render` trait, `AnyView`)
- **GPU scene** (`Scene` + paths/quads/sprites + atlas)
- **Platform layer** (Metal on macOS, Direct3D 11 on Windows, wgpu on Linux)
- **Taffy-based layout**, cosmic-text text system, custom arena allocator

### 1.2 Where we are

- **184 source files, ~31 600 LoC** in `crates/flui-core/src/`.
- Phase I migration (extracting platform code to `flui-platform`) is **frozen** after
  S01 lock + S02a skeleton — see `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`.
- S07 (gesture arena) is complete and well-documented — it is the **gold standard**
  inside the crate for module organization and design rigor.
- Animation subsystem (`animation/`) is partially complete: `Tween`, `Curve`,
  `Lerp`, `AnimationController`, `Spring/Friction/Gravity` simulations exist.
- Phase II Flutter-parity work (S08 Semantics, S09 Canvas, S10 Filters, S12 Focus,
  S13 Text, S14 MediaQuery, S15 Assets) is **not started**.

### 1.3 Workspace shape

```
crates/
├── flui-core         ← THIS DOCUMENT'S TARGET
├── flui-platform     ← skeleton; reserved for future Phase III
├── flui-macros
├── flui-animate      ← planned (additional animations on top of core)
├── flui-navigator    ← routing
├── flui-a11y         ← planned (semantics)
├── flui-theme        ← planned
├── flui-material     ← planned
└── flui-widgets      ← planned (gated on roadmap completion)
```

### 1.4 North-star vision

- A Flutter-parity feature surface implemented natively in Rust — same user-visible
  concepts, blazing-fast Rust execution.
- A **hybrid widget model**: GPUI's existing `Render`/`Entity` API stays as the
  low-level escape hatch (Zed-style imperative UI for editor-like workloads), and a
  Flutter-style `Widget`+`State<T>`+`Key` reconciliation layer sits on top of it for
  app-style declarative UI.
- 60 FPS / 16.67 ms frame budget is a structural invariant — no design that requires
  unbounded work per frame is admitted.

---

## 2. Constraints

### 2.1 What is allowed

- Break the public API of `flui-core`. There is no semver promise yet.
- Re-organize the module structure (within `flui-core/src/`).
- Re-design subsystems if the result is materially cleaner.
- Add new dependencies if they earn their compile-time and binary-size cost.
- Raise MSRV if it unlocks idioms (current workspace MSRV: 1.85, edition 2024).
- Modify lock-tests / golden tests when their pinned API changes — but rewrite them
  to lock the new shape.

### 2.2 What is forbidden

- Hacky workarounds (Russian: «костыли»). The whole point of this rework is to
  remove existing kludges, not add new ones. If a fix feels hacky, write a spec
  instead.
- Silent behavior changes. Renderable output must be regression-tested via the
  golden suite (S01b harness).
- Unbounded work per frame. Any new code path must justify its frame budget.
- Pre-emptively-built API surface ("this method will be useful when T15 lands"). Add
  it when the consumer lands.
- Growing `crates/flui-core/src/platform/**`. New platform code goes to
  `crates/flui-platform/`.
- Skipping clippy/format/test gates with `--no-verify`.

### 2.3 What requires a design spec first

Any change that:

- Modifies the `Element` / `Render` / `Widget` / `Entity` / `App` / `Context` /
  `Window` / `Scene` traits or struct surfaces.
- Adds a new public type to `flui-core::*`.
- Crosses a sub-system boundary (animation ↔ gesture ↔ paint ↔ layout).
- Has the label **HIGH-RISK** or **API-BREAKING** in §8.

Spec format follows
`docs/superpowers/specs/2026-04-13-S01a3-explicit-re-export-list-design.md` exactly
— `Context`, `Goals`, `Non-goals`, `Current state`, `Design`, `API surface`,
`Migration / Compatibility`, `Testing strategy`, `Open questions`, `Done criteria`.

---

## 3. Architectural principles

These are the load-bearing rules. Every design choice must justify itself against
them.

### 3.1 60 FPS is a structural property, not an optimization

16.67 ms / frame; realistic app-code budget ~10 ms; explicit per-phase sub-budgets:

| Phase            | Target budget |
|------------------|---------------|
| Animation tick   | ≤ 1 ms        |
| Layout           | ≤ 3 ms        |
| Prepaint         | ≤ 4 ms        |
| Paint + present  | ≤ 1 ms        |
| Gesture dispatch | ≤ 1 ms        |
| Effect flush     | ≤ 2 ms        |
| Slack            | ~4 ms         |

Implications for design:

- **Per-frame work scales with `O(active)`, not `O(total)`.** 1000 controllers, 5
  active = 5 ticks of work.
- **No allocations on dispatch / tick / paint hot paths.** `SmallVec`, `[T; N]`
  inline arrays, capacity-preserving `clear()`.
- **Time fixes once per frame**, cached on `FrameClock`. Multiple reads in one frame
  return the same value (also makes animations deterministic).
- **No runtime borrow check (`RefCell`) inside paint or dispatch loops.** Owning
  structures + index-based references.
- **Each phase has a hard budget**; effect-loop deadlines exist.
- **Static-content shortcut.** Style hash + bounds match → reuse layout, prepaint,
  scene primitives.

### 3.2 No costyl' rule

If a fix feels hacky:

- It probably is.
- Open a design doc instead of merging the hack.
- A 200-line spec saves a 2-week archaeology session later.

Marker patterns that flag a kludge:

- `mem::take(...)` / restore dance to escape borrow checker.
- `Rc<RefCell<Box<dyn ...>>>` triple indirection on a hot path.
- `__internal_` / `__assert_` prefixes leaking into autocomplete.
- `#[allow(dead_code, reason = "future T<N> consumer")]` on speculative API.
- `unwrap_or_else(|| panic!(...))` instead of `expect`.
- Silent `Option::default()` fallback that should panic in release.
- Comments saying "Temporary(?)" or "remove after stabilization" older than 3 months.

### 3.3 Hybrid widget tree

The architecture has **two layers** that coexist:

```
┌─────────────────────────────────────────────────────────────┐
│  Flutter-style declarative widgets (Widget / StatefulWidget)│  ← flui-widget
│  - Key-based reconciliation                                 │
│  - State<T> survives rebuild                                │
│  - BuildCx with Provider / inherit<T>()                     │
└─────────────────────────────────────────────────────────────┘
                              │ compiles down to
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Existing Element / Render / Entity / AnyElement layer      │  ← flui-core
│  - Used directly for editor-style imperative UI (Zed)       │
│  - Used as the runtime substrate for the widget layer above │
└─────────────────────────────────────────────────────────────┘
```

Implication: do **not** rip out `Render` / `Entity` / `Component`. Build the widget
layer on top of them. Both APIs ship.

### 3.4 Rust 2024 idiomatic

- Edition 2024; raise MSRV to latest stable when needed and document it.
- Prefer `let-else`, `let-chains`, `if-let` chains over nested matches.
- Use `&[T]` + slice patterns instead of `Vec<T>` where the data is read-only.
- `#[must_use]` on builder methods returning `Self`.
- `#[non_exhaustive]` on every `pub` struct/enum that is likely to grow.
- `#[doc(hidden)]` (without `__` prefix) for items that must be `pub` for macro
  reasons but are not user-facing.
- `Cell<T>` over `RefCell<T>` for `Copy` types.
- `&'static [T]` constants over `vec![…]` allocations.
- Sealed traits (`crate::seal::Sealed`) for marker traits with controlled impl set.

### 3.5 Test-coverage discipline

- Every new subsystem ships with: (a) unit tests, (b) integration test, (c) one
  example, (d) at least one entry in the bench harness.
- Frame-budget regression test: `cargo bench` output is captured per commit; PR
  fails if any tracked metric regresses > 5% without explicit justification.

---

## 4. Target architecture (the "after" picture)

### 4.1 Animation subsystem — `Animation<T>` hierarchy

Replace the current single-purpose `AnimationController` with a Flutter-shaped
trait hierarchy:

```rust
pub trait Listenable {
    fn add_listener(&self, cb: ListenerCallback) -> ListenerSubscription;
    // remove via dropping subscription
}

pub trait Animation<T>: Listenable {
    fn value(&self) -> T;
    fn status(&self) -> AnimationStatus;
}

// concrete:
pub struct AnimationController { ... }                   impl Animation<f32>
pub struct CurvedAnimation<A: Animation<f32>> { ... }    impl Animation<f32>
pub struct ReverseAnimation<A: Animation<f32>> { ... }   impl Animation<f32>
pub struct ProxyAnimation<T> { ... }                     impl Animation<T>
pub struct ChainedAnimation<U, T, A, F: Fn(T) -> U> { } impl Animation<U>

// data-flow:
pub struct Tween<T: Lerp> { begin: T, end: T }
impl<T: Lerp> Tween<T> {
    pub fn animate<A: Animation<f32>>(self, parent: A) -> impl Animation<T>;
    pub fn chain(self, next: Tween<T>) -> ChainedTween<T>;
}
```

Goals:

- Animations compose. `tween.animate(controller).curve(EaseInOut).reverse()` is one
  expression.
- Status listener separate from value listener. `addStatusListener` triggers only on
  `Status` transitions (Forward → Completed, etc.).
- `AnimationController` is a `Listenable`-implementing concrete driver; it is **not**
  bound to `Entity` at the trait level. An `EntityAnimationController` or trivial
  Entity wrapper provides the Entity binding.
- `Lerp` covers all primitive UI types: `Pixels`, `DevicePixels`, `ScaledPixels`,
  `Rems`, `f32`, `f64`, `Hsla` (with shortest-path hue), `Rgba`, `Point<T>`,
  `Size<T>`, `Bounds<T>`, `Edges<T>`, `Corners<T>`, `BoxShadow`, `BorderRadius`.
- `Curve` and `SpringSimulation` share the **same physics engine**. `Curve::Spring`
  delegates to `SpringSimulation`, not a duplicate approximation.
- `AnimationController` precomputes simulation coefficients in `new()`; tick reads
  cached time from `FrameClock`.

### 4.2 Gesture subsystem — owned arena, no `Rc<RefCell>`

Replace the current `Rc<RefCell<Box<dyn GestureRecognizer>>>` triple indirection
with owned storage:

```rust
pub struct GestureArenaManager {
    arenas: SmallVec<[(PointerId, GestureArena); 4]>,
}

pub(crate) struct GestureArena {
    entries: SmallVec<[Box<dyn GestureRecognizer>; 4]>,
    winner: Option<usize>,
    is_open: bool,
    hold_count: u32,
}
```

Async timer back-channel (LongPress) uses **message-passing through the effect
queue**, not direct `Weak<RefCell<...>>` access:

```rust
// Recognizer stores opaque handle:
struct LongPressGestureRecognizer {
    binding_handle: GestureBindingHandle,  // copyable token
    pointer_indexes: Option<(PointerId, usize)>,  // single-shot
    ...
}

// Timer fires:
async fn timer_fired(handle: GestureBindingHandle, pointer: PointerId, idx: usize) {
    handle.post_message(ArenaMessage::DeclareWinner { pointer, idx });
}
```

`Window::dispatch_event` drains the message queue together with the effect queue
each tick. Window-teardown invalidates the `GestureBindingHandle`; pending messages
are dropped.

Recognizer state shared base:

```rust
pub(crate) struct RecognizerCore {
    pointer: Option<PointerId>,
    down_position: Point<Pixels>,
    last_position: Point<Pixels>,
    last_kind: PointerKind,
    state: RecognizerState,  // small enum
}
```

Concrete recognizers compose `RecognizerCore` + per-recognizer fields.

`RecognizerLifecycle` merged into `GestureRecognizer` with default no-op methods.
The `Option<&mut dyn RecognizerLifecycle>` opt-in is removed.

VelocityTracker uses inline arrays (no heap allocations on `estimate()`).

`GestureSettings` extends to full Flutter parity (kMinFlingVelocity,
kMaxFlingVelocity, kHorizontalDragSlopFactor, kJumpTapTimeout, etc.).

### 4.3 Element trait — context object pattern

Replace 6–7-arg method signatures with `&mut PaintCx<'_>` / `&mut LayoutCx<'_>` /
`&mut PrepaintCx<'_>`:

```rust
pub trait Element: 'static {
    type Layout: 'static;
    type Prepaint: 'static;

    fn id(&self) -> Option<ElementId>;
    fn layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::Layout);
    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>, layout: &mut Self::Layout)
        -> Self::Prepaint;
    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        layout: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
    );
}

pub struct PaintCx<'a> {
    pub bounds: Bounds<Pixels>,
    pub id: Option<&'a GlobalElementId>,
    pub inspector_id: Option<&'a InspectorElementId>,
    window: &'a mut Window,
    app: &'a mut App,
}

impl PaintCx<'_> {
    pub fn paint_quad(&mut self, q: PaintQuad) { self.window.paint_quad(q) }
    pub fn paint_path(&mut self, path: Path<Pixels>, fill: impl Into<Background>) { … }
    pub fn with_offset<R>(&mut self, off: Point<Pixels>, f: impl FnOnce(&mut Self) -> R) -> R;
    pub fn window(&mut self) -> &mut Window;
    pub fn app(&mut self) -> &mut App;
}
```

Result: custom Element implementations write `cx.paint_quad(q)` instead of plumbing
six arguments.

### 4.4 Reactive Provider (replaces `provider/stack.rs`)

Replace thread-local global `HashMap<TypeId, Vec<Box<dyn Any>>>` with per-Window
registry that supports subscriptions:

```rust
pub trait Inherited: Clone + Any {
    fn should_notify(&self, old: &Self) -> bool { true }
}

pub struct InheritedRegistry {
    by_type: FxHashMap<TypeId, InheritedEntry>,
}

struct InheritedEntry {
    value: Box<dyn Any>,
    version: u64,
    dependents: SmallVec<[ElementId; 4]>,
}

impl BuildCx<'_> {
    /// Read value AND subscribe — element rebuilds when value changes.
    pub fn inherit<T: Inherited>(&mut self) -> Option<T>;

    /// Read value WITHOUT subscribing.
    pub fn read<T: Inherited>(&self) -> Option<&T>;
}
```

Provider element pushes/pops cleanly via RAII (no panic-corrupt windows). Window
isolation is automatic — registry lives on `Window`.

This is the foundation for Theme / MediaQuery / DefaultTextStyle / Localizations.

### 4.5 Widget layer with `Key` + reconciliation

New `flui-widget` crate (or module inside `flui-core` if simpler):

```rust
pub trait Widget: 'static {
    fn key(&self) -> Option<Key> { None }
    fn build(&self, ctx: &mut BuildCx<'_>) -> AnyWidget;
}

pub trait StatefulWidget: 'static {
    type State: WidgetState;
    fn key(&self) -> Option<Key> { None }
    fn create_state(&self) -> Self::State;
}

pub trait WidgetState: 'static {
    type Widget: StatefulWidget;
    fn build(&mut self, ctx: &mut BuildCx<'_>) -> AnyWidget;
    fn did_update_widget(&mut self, _old: &Self::Widget) {}
    fn dispose(&mut self) {}
}

pub struct Key(KeyKind);
enum KeyKind {
    Local(u64),     // hash of source location + sibling index
    Value(u64),     // user-provided ValueKey
    Global(GlobalKey),
}
```

Reconciliation algorithm: for each rebuild, match old children to new children by
`(TypeId, Key)`. On match, `State` survives + `did_update_widget` is called. On
mismatch, old `State::dispose` runs, new `State` is created.

Widget tree compiles down to existing `Element` tree — `Component<Widget>` adapter.

### 4.6 Frame budget enforcement

```rust
pub struct FrameClock {
    frame_start: Instant,
    delta: Duration,
    frame_index: u64,
}

pub struct FrameProfile {
    pub layout: Duration,
    pub prepaint: Duration,
    pub paint: Duration,
    pub gesture_dispatch: Duration,
    pub animation_tick: Duration,
    pub effect_flush: Duration,
    pub primitive_count: usize,
    pub active_animations: usize,
}

impl App {
    fn flush_effects(&mut self) {
        let deadline = self.frame_clock.frame_start
            + Duration::from_millis(EFFECT_BUDGET_MS);
        while let Some(effect) = self.pending_effects.pop_front() {
            if Instant::now() > deadline {
                log::warn!("effect-flush exceeded budget; deferring");
                break;
            }
            ...
        }
    }
}
```

`FrameProfile` surfaces through the inspector (debug builds). Per-phase regression
gate: bench harness fails CI if any phase exceeds its target budget on the
reference scene.

### 4.7 Style decomposition

Replace 38-flat-field `Style` with composition:

```rust
pub struct ElementStyle {
    pub layout: LayoutStyle,           // flex, grid, position, gap, sizes
    pub spacing: SpacingStyle,         // padding, margin, border_widths
    pub decoration: BoxDecoration,     // background, border, corner_radii, shadows
    pub text: TextStyle,
    pub effects: EffectsStyle,         // opacity, filters, transforms
    pub interaction: InteractionStyle, // cursor, mouse_through, debug
}
```

Cache key for layout = `hash(LayoutStyle + SpacingStyle + Constraints)`. Style
changes that touch only `decoration` or `text` skip Taffy entirely.

### 4.8 Workspace split (long-term)

Once subsystem boundaries stabilize:

```
crates/
├── flui-foundation     # geometry, color, sharedstring, units (no_std-friendly)
├── flui-runtime        # App, Entity, Effect, scheduler, executor
├── flui-painting       # Scene, paths, atlas, primitives, lyon
├── flui-text           # cosmic-text, line layout, IME plumbing
├── flui-element        # Element trait, AnyElement, Drawable, layout
├── flui-widget         # Widget, StatefulWidget, Key, BuildCx, reconciliation
├── flui-input          # gestures, focus, keymap, actions
├── flui-platform       # platform backends
├── flui-widgets        # Container/Row/Column/Stack/Text/Button/...
├── flui-material       # Material design widgets
├── flui-cupertino      # iOS-style widgets
├── flui-theme          # Theme + Provider integration
├── flui-navigator      # routing
├── flui-a11y           # semantics
└── flui-macros         # proc macros
```

Bonus: parallel compilation. Edits to `style.rs` no longer rebuild the entire core.

---

## 5. Hot-path 60-FPS hit list

Items that, in current code, work against the 60 FPS target. Each appears in §8 with
concrete details.

| # | Hot path | Current cost | Fix |
|---|---|---|---|
| 1 | `AnimationController::value()` syscall on every read | ~50–100 ns × 4 reads × 100 controllers = ~40 µs/frame | Cache via `FrameClock` (A3) |
| 2 | `Scene::insert_primitive` clones each primitive | mem-copy per primitive, thousands per frame | Take by-value (E1) |
| 3 | `Rc<RefCell<Box<dyn GestureRecognizer>>>` triple indirect | runtime borrow check + 3 ptr hops | Owned `Vec<Box<dyn>>` (G1) |
| 4 | VelocityTracker: 4× `Vec::with_capacity` per estimate() | ≥ 240 heap allocs/sec on continuous drag | Inline arrays (G9) |
| 5 | Hit-test storage as `FxHashMap<HitboxId, …>` | hash + branch + indirect ~ 80 ns / lookup | `Vec` indexed by `HitboxId(u32)` (E4) |
| 6 | `Arc<[ElementId]>` allocated per `element_id_stack.push` | 1 atomic + heap per element | `SmallVec` + lazy `Arc` only on lookup (E5) |
| 7 | `pending_effects: VecDeque<Effect>` allocations | heap on every push | `SmallVec<[Effect; 16]>` + spillover (E6) |
| 8 | Layout: every frame Style → Taffy | full Taffy work for static UI | Layout hash cache (E7) |
| 9 | `App` has 60+ fields hot/cold mixed | poor cache locality | Split hot/cold (E8) |
| 10 | `SubscriberSet` clones `Vec<Callback>` for retain | heap per fire | In-place retain + swap_remove (E9) |
| 11 | Heavy `TextStyle` `PartialEq` in `AnyView::cached` cache key | ~100–200 ns per cache check | Pre-hash key (E10) |
| 12 | Animation tick walks all controllers via `cx.observe` | O(total) | Active set (A12) |
| 13 | `mem::take` arena dance in `binding.rs` | swap of two VecDeque every Up | Owned arena (G5) |
| 14 | `Hsla → Rgba` per-frame on CPU | repeated for static colors | Pre-bake or shader-side (E12) |
| 15 | `Style` 38 fields hashed/compared as a unit | unrelated changes invalidate everything | Decompose `Style` (E11) |

---

## 6. Phased execution plan

```
Phase 0 — Cleanup (1–2 weeks)            ← non-architectural; unblocks everything
   │
   ▼
Phase 1 — Foundations (4–8 weeks)        ← Element ctx, Animation<T>, Provider, Style
   │
   ▼
Phase 2 — Widget layer (8–16 weeks)      ← Widget+Key+reconciliation, hybrid
   │
   ├── Phase 3 — Flutter parity (parallel, 6–12 weeks)
   │      ← S08, S09, S10, S11, S12, S13, S14, S15
   │
   ▼
Phase 4 — Workspace split (1–2 months)
   │
   ▼
Phase 5 — Widgets library (3–6 months)
       ← flui-widgets + flui-material + flui-cupertino
```

### 6.1 Phase 0 — Cleanup

**Goal:** establish a clean baseline. No architectural changes. Each item is a
single small commit.

| Task | Issue refs |
|------|-----------|
| Rebrand "GPUI" → "flui" in public docstrings (~157 instances, 25 files) | E16 |
| Fix `_ownership_and_data_flow.rs` doctests (`gpui_platform::application()` → `flui_core::application()`) | E16 |
| Convert remaining 29 `pub use mod::*;` globs in `lib.rs` to explicit lists (continuation of S01a.3) | E17 |
| Upgrade `derive_more = "0.99.17"` → `2.x`; switch trivial `Deref/DerefMut` to manual impls in `shared_string.rs`, `shared_uri.rs` | E18 |
| Expand `prelude.rs` to include `Pixels`, `px`, `point`, `size`, `Hsla`, `rgb`, `rgba`, `SharedString`, plus existing traits | E20 |
| Replace `unwrap_or_else(|| panic!(...))` with `expect` everywhere | E22 |
| Replace `.with_context().unwrap()` anti-pattern with `.expect_with` (custom helper) or proper error propagation | E21 |
| Triage 47 `// TODO` / `// FIXME` markers — convert to GitHub issues or fix immediately | E19 |
| Triage 13 `#[allow(dead_code|unused)]` attrs — remove or wire up | E19 |
| Document `#[expect(missing_docs)]` sites in `scene.rs` — add real docs | E23 |

### 6.2 Phase 1 — Foundations

**Goal:** make the substrate clean and Flutter-shaped before adding new features.

| Spec ID (suggested) | Title | Issues addressed |
|---|---|---|
| **S21** | Element context object refactor | E5, E6, A4 (in element trait) |
| **S22** | `Animation<T>` trait hierarchy + `Listenable` | A1, A2, A5, A6, A8, A10 |
| **S23** | Reactive Provider (replaces `provider/stack.rs`) | E1 (provider) |
| **S24** | Style decomposition (`ElementStyle` composition) | E11 |
| **S25** | Frame budget architecture (`FrameClock`, `FrameProfile`, deadlines) | All hot-path items |
| **S26** | Gesture arena ownership refactor (no `Rc<RefCell<Box<dyn>>>`) | G1, G3, G5, G6, G7, G9, G10, G11, G12 |
| **S27** | `GestureRecognizer` + `RecognizerLifecycle` merge | G2 |
| **S28** | `RecognizerCore` shared state | G3 |
| **S29** | Animation: ticker / active-set / cached `value()` | A3, A11, A12 |
| **S30** | Lerp completion (all UI types, shortest-path Hsla) | A4 |

S22, S23 and S24 should land before S25 because the budget profiler exercises them.
S26–S29 can happen in parallel with S22–S24.

### 6.3 Phase 2 — Widget layer

**Goal:** implement hybrid widget tree on top of cleaned-up Element layer.

| Spec ID | Title | Notes |
|---|---|---|
| **S31** | Widget / StatefulWidget / Key trait set | depends on S21, S23 |
| **S32** | Reconciliation algorithm | depends on S31 |
| **S33** | BuildCx (Provider integration, Theme access) | depends on S23, S31 |
| **S34** | Widget → Element compilation | depends on S31, S32 |
| **S35** | StreamBuilder / FutureBuilder async widgets | depends on S31 |
| **S36** | `AppCell` → token-based borrow refactor | E2 |
| **S37** | Action system: per-subtree Actions + Intent | E3 |

### 6.4 Phase 3 — Flutter parity (parallel with Phase 2)

These specs are mostly orthogonal. Order by dependency:

```
S08 Semantics ──┐
                ├─ enables Material / Cupertino properly
S12 Focus ──────┘

S09 Canvas ────► S10 Filters

S11 Physics (already partially in animation/) — finish Spring/Friction integration

S13 Text parity (StrutStyle, FontFeatures, IME)

S14 MediaQuery (a11y flags, gestureSettings, SystemChrome)

S15 Asset bundle (resolution-aware, locale variants)
```

### 6.5 Phase 4 — Workspace split

Only **after** §4.8 boundaries have stabilized. Each crate extraction is one
reviewable commit.

### 6.6 Phase 5 — Widgets library

Build `flui-widgets`, `flui-material`, `flui-cupertino` on top of stable core.

---

## 7. Per-issue catalog

Issue IDs:

- **A** = animation subsystem
- **G** = gesture subsystem
- **E** = core / runtime / cross-cutting

Format: `Id` | `severity` | `file:line` | `summary` | `proposed fix`

### 7.1 Animation issues

#### A1 — No `Animation<T>` abstraction (HIGH-RISK, API-BREAKING)
- **Where:** absent.
- **Cost:** Animations don't compose. Every `render()` does `controller.value() →
  curve.transform() → tween.transform()` by hand.
- **Fix:** §4.1. Trait hierarchy `Listenable → Animation<T>`, with concrete
  composers `CurvedAnimation`, `ReverseAnimation`, `ProxyAnimation`,
  `ChainedAnimation`. **Spec:** S22.

#### A2 — `Curve::Spring` duplicates `SpringSimulation` physics
- **Where:** `crates/flui-core/src/animation/curve.rs:103-115`
  vs `crates/flui-core/src/animation/simulation.rs:102-118`.
- **Fix:** `Curve::Spring(SpringDescription)` delegates to
  `SpringSimulation::x(t)`. One source of truth for the physics. **Spec:** S22.

#### A3 — `value()` syscall on every read
- **Where:** `crates/flui-core/src/animation/controller.rs:127, 165`. TODO comment
  on line 127.
- **Cost:** 50–100 ns per `Instant::elapsed()`; multiple reads per frame per
  controller.
- **Fix:** `FrameClock` provides cached `frame_start: Instant`. Controllers compute
  elapsed against it. Optionally cache last computed value with `cached_at_frame`
  marker. **Spec:** S25 + S29.

#### A4 — `Lerp` incomplete; `Hsla` lerp wrong over hue wrap
- **Where:** `crates/flui-core/src/animation/lerp.rs`.
- **Missing:** `Rgba`, `DevicePixels`, `ScaledPixels`, `Rems`, `Edges<T>`,
  `Corners<T>`, `Bounds<T>`, `BoxShadow`, `BorderRadius`.
- **Bug:** `Lerp for Hsla` interpolates `h` linearly — fails on red↔blue (goes
  through green/yellow). Use shortest-path on the hue circle.
- **Fix:** systematic blanket impls for `Lerp` over geometric types; reimplement
  Hsla with `lerp_hue_shortest_path`. **Spec:** S30.

#### A5 — `AnimationController` has 6 mutually-exclusive fields
- **Where:** `crates/flui-core/src/animation/controller.rs:60-65`.
- **Smell:** `start_time` and `sim_start_time` interlocked; `simulation` zeros out
  `start_time`; every method clears 2–3 fields at once.
- **Fix:** `enum Driver { Idle, Tween { … }, Simulation { … } }`. One field,
  exhaustive match in `value()` and `is_animating()`. **Spec:** S22.

#### A6 — No `addStatusListener` analogue
- **Where:** absent.
- **Cost:** Chaining animations (fade-in done → slide-in start) requires manual
  status comparison in `cx.observe` callback.
- **Fix:** `Listenable::add_status_listener(cb)` separate from value listener.
  **Spec:** S22.

#### A7 — `Simulation: Send + Sync` unnecessary
- **Where:** `crates/flui-core/src/animation/simulation.rs:22`.
- **Cost:** Forces `Send`/`Sync` on every implementor; animations are
  main-thread-only.
- **Fix:** drop the bounds. **Spec:** S22.

#### A8 — `Curve::Custom(Arc<dyn Fn>)` over-allocated
- **Where:** `crates/flui-core/src/animation/curve.rs:49`.
- **Fix:** `Box<dyn Fn>` if not shared, or accept that `Curve: Clone` requires
  `Arc`. Document the trade-off. **Spec:** S22.

#### A9 — 4 files with `#![allow(missing_docs)]`
- **Where:** `controller.rs:3`, `curve.rs:3`, `tween.rs:3`, `simulation.rs:3`.
- **Fix:** write the docs as part of S22.

#### A10 — `AnimationController::attach(self, cx)` couples to `Entity`
- **Where:** `crates/flui-core/src/animation/controller.rs:115-119`.
- **Fix:** `AnimationController` is a pure data driver with `Listenable` impl.
  Entity binding via separate adapter or trivial wrapper. **Spec:** S22.

#### A11 — Doctests marked `ignore`
- **Where:** `controller.rs:37`, `animated.rs:13`, `tween.rs:11`.
- **Fix:** convert to `no_run` or actual compilable examples. **Spec:** S22.

#### A12 — No active-animation set; tick walks all controllers
- **Where:** Animation tick currently rides on `cx.observe` notifications, which
  fire for any change.
- **Fix:** `AnimationDriver` tracks `active: SmallVec<[EntityId; 32]>`. Idle
  controllers (`Dismissed` / `Completed`) are not in `active`. **Spec:** S29.

### 7.2 Gesture issues

#### G1 — `Rc<RefCell<Box<dyn GestureRecognizer>>>` triple indirection (HIGH-RISK)
- **Where:** `crates/flui-core/src/gesture/arena.rs:48-50`,
  `crates/flui-core/src/window.rs:977-980`.
- **Cost:** runtime borrow check + 3 pointer hops per dispatch event.
- **Fix:** owned `Vec<Box<dyn GestureRecognizer>>`, indices instead of refs. Async
  back-channel via message passing through effect queue. **Spec:** S26.

#### G2 — `RecognizerLifecycle` as opt-in sibling trait (API-BREAKING)
- **Where:** `crates/flui-core/src/gesture/recognizer.rs:131-133, 159-220`.
- **Smell:** `Option<&mut dyn RecognizerLifecycle>` opt-in for forward-compat. The
  methods belong on the main trait.
- **Fix:** merge `RecognizerLifecycle` into `GestureRecognizer` with default no-op
  methods. **Spec:** S27.

#### G3 — Recognizer state pattern duplicated 5 times
- **Where:** `tap.rs`, `drag.rs`, `long_press.rs`, `double_tap.rs`, `scale.rs`.
- **Fields repeated:** `state`, `pointer`, `down_position`, `last_position`,
  `last_kind`.
- **Fix:** shared `RecognizerCore` struct, recognizer-specific data composed on
  top. **Spec:** S28.

#### G4 — 14 `Box<dyn FnMut>` callback fields across recognizers
- **Where:** `tap.rs:80-87`, `drag.rs:88-90`, `long_press.rs:53-60`,
  `double_tap.rs`, `scale.rs`.
- **Cost:** 16–24 bytes vtable + heap allocation per callback per recognizer
  instance.
- **Fix:** consider single typed event channel
  (`Box<dyn FnMut(RecognizerEvent<T>, …)>`). Trade-off: simpler fluent API vs
  fewer allocations. Decision in S26.

#### G5 — `arena_take` / `arena_restore` `mem::take` dance
- **Where:** `crates/flui-core/src/gesture/binding.rs:152-159`.
- **Fix:** killed by G1 — owned arena means no swap dance. **Spec:** S26.

#### G6 — `__assert_object_safe` `__` prefix in autocomplete
- **Where:** `crates/flui-core/src/gesture/recognizer.rs:341-353`.
- **Fix:** `const _: fn(Box<dyn GestureRecognizer>) = |_| {};` inside a `#[cfg(test)]`
  module. **Spec:** can land standalone.

#### G7 — `arena_back_channel: ArenaBackChannel::empty()` silent fallback
- **Where:** `crates/flui-core/src/gesture/recognizers/long_press.rs:96`,
  `crates/flui-core/src/gesture/arena.rs:115-117`.
- **Smell:** unbound recognizer silently no-ops timer fire.
- **Fix:** `Option<ArenaBackChannel>` + `expect()` in production paths;
  `MockArenaBackChannel` for tests. **Spec:** S26.

#### G8 — `pointer_indexes: SmallVec<[(PointerId, usize); 1]>` for single-pointer
- **Where:** `crates/flui-core/src/gesture/recognizers/long_press.rs:104`.
- **Fix:** `Option<(PointerId, usize)>`; future MultiTap can refactor when it lands.
  **Spec:** S26.

#### G9 — `VelocityTracker::estimate()` allocates 4 `Vec`s
- **Where:** `crates/flui-core/src/gesture/velocity_tracker.rs:119-122`.
- **Cost:** 4 heap allocs per estimate; ~240 allocs/sec on continuous drag.
- **Fix:** `[f32; MAX_SAMPLES]` stack arrays (MAX_SAMPLES = 20 by config). **Spec:**
  S26.

#### G10 — `PointerSanitizer` is a unit-struct
- **Where:** `crates/flui-core/src/gesture/binding.rs:113`,
  `crates/flui-core/src/gesture/dispatch.rs`.
- **Fix:** convert methods to free functions on `WindowPointerState`. **Spec:** S26.

#### G11 — 4 `#[allow(dead_code)]` "future T15" markers
- **Where:** `crates/flui-core/src/gesture/binding.rs:107, 164, 178, 191`.
- **Fix:** wire up or delete. Pre-baked API is over-engineering. **Spec:** S26.

#### G12 — `Default` via `new()` via `#[allow(dead_code)]`
- **Where:** `crates/flui-core/src/gesture/binding.rs:96-107`.
- **Fix:** keep one entry point. **Spec:** S26.

#### G13 — `SemanticAction` enum lives in gesture/
- **Where:** `crates/flui-core/src/gesture/recognizer.rs:227-236`.
- **Fix:** move to `flui-a11y` (or `semantics/`) when S08 lands; until then
  `pub(crate)` in gesture. **Spec:** S08.

#### G14 — `GestureSettings` incomplete vs Flutter
- **Where:** `crates/flui-core/src/gesture/gesture_settings.rs:25-50`.
- **Missing:** `kMinFlingVelocity`, `kMaxFlingVelocity`,
  `kHorizontalDragSlopFactor`, `kJumpTapTimeout`, `kZoomControlsTimeout`,
  `kPagingTouchSlop`.
- **Fix:** add fields with Flutter-parity defaults; recognizers stop hardcoding
  factors (`drag.rs` uses `2.0 * dy.abs()`). **Spec:** S26 or follow-up.

#### G15 — `semantic_actions` and `on_focus_request` hooks pre-baked in trait
- **Where:** `crates/flui-core/src/gesture/recognizer.rs:93-103`.
- **Fix:** keep for now (S08, S12 seams) but design alternative — separate
  `RecognizerSemantics` / `RecognizerFocus` traits — and decide before merging
  G2's RecognizerLifecycle. **Spec:** decided in S27.

### 7.3 Core / runtime / cross-cutting issues

#### E1 — Provider system is thread-local global push/pop stack (HIGH-RISK)
- **Where:** `crates/flui-core/src/provider/stack.rs:7-9`.
- **Smell:** `thread_local! { PROVIDER_STACKS: RefCell<HashMap<TypeId, Vec<Box<dyn
  Any>>>> }`. No reactivity, fragile push/pop, no per-Window isolation.
- **Fix:** §4.4. Per-Window `InheritedRegistry` with subscriptions. **Spec:** S23.

#### E2 — No widget identity / `Key` (HIGH-RISK, API-BREAKING)
- **Where:** absent.
- **Cost:** widget reconciliation impossible — list reorder loses state.
- **Fix:** §4.5. `Key` enum + reconciliation algorithm. **Spec:** S31.

#### E3 — `AppCell = RefCell<App>` with TODO "remove after stabilization" (HIGH-RISK)
- **Where:** `crates/flui-core/src/app.rs:73-75`.
- **Fix:** token-based borrow model; runtime-borrow-check elimination. **Spec:** S36.

#### E4 — Hit-test storage in `FxHashMap<HitboxId, …>`
- **Where:** `crates/flui-core/src/window.rs:966-982`.
- **Fix:** `Vec<HitTestBehavior>` indexed by `HitboxId(u32).0`. O(1) without hash.
  **Spec:** S25 or S26.

#### E5 — `Arc<[ElementId]>` allocated per `element_id_stack` push
- **Where:** `crates/flui-core/src/element.rs:289`,
  `crates/flui-core/src/window.rs` element_id_stack manipulation sites.
- **Fix:** `SmallVec<[ElementId; 16]>` stack-allocated; lazily upgrade to `Arc` only
  when the global id leaves the call stack. **Spec:** S21.

#### E6 — `Element` trait has 6–7 args per method (API-BREAKING)
- **Where:** `crates/flui-core/src/element.rs:73-104`.
- **Fix:** §4.3 context object pattern. **Spec:** S21.

#### E7 — Layout has no caching layer
- **Where:** `crates/flui-core/src/window.rs:3832` and Taffy integration sites.
- **Fix:** layout cache by `hash(LayoutStyle + SpacingStyle + Constraints)`; static
  elements skip Taffy. **Spec:** S25.

#### E8 — `App` struct has 60+ fields, hot/cold mixed
- **Where:** `crates/flui-core/src/app.rs:584-664`.
- **Fix:** split `AppHot` (entities, windows, effects, focus) + `AppCold` (settings,
  observers, registries). Improves cache locality. **Spec:** S25 or follow-up.

#### E9 — `SubscriberSet` clones `Vec<Callback>` for retain
- **Where:** `crates/flui-core/src/subscription.rs`.
- **Fix:** in-place retain + swap_remove. **Spec:** small standalone improvement.

#### E10 — `AnyView::cached` cache key includes heavy `TextStyle`
- **Where:** `crates/flui-core/src/view.rs:22-26`.
- **Fix:** pre-hash on insert, compare hashes first. **Spec:** small standalone
  improvement.

#### E11 — `Style` has 38 flat fields (API-BREAKING)
- **Where:** `crates/flui-core/src/style.rs:180`.
- **Fix:** §4.7 decompose. Cache key per sub-struct. **Spec:** S24.

#### E12 — `Hsla → Rgba` per-frame on CPU
- **Where:** color pipeline in scene primitives.
- **Fix:** pre-convert on insert; long-term — shader-side conversion.

#### E13 — `Window` has 222 public methods in one 6 123-line file
- **Where:** `crates/flui-core/src/window.rs`, two `impl Window` blocks (lines
  1156, 1550).
- **Fix:** split into `window/{lifecycle, layout, paint, hit_test, dispatch, focus,
  state, frame, actions}.rs`. Internal reorganization, no public API change.
  **Spec:** S25 or follow-up.

#### E14 — `geometry.rs` 4 149 lines, 32 public types
- **Where:** `crates/flui-core/src/geometry.rs`.
- **Fix:** split into `geometry/{point, size, bounds, edges, corners, pixels,
  length, grid, affine}.rs`. Internal reorganization. **Spec:** follow-up to E13.

#### E15 — `app.rs` has multiple responsibilities mixed (2 718 lines)
- **Where:** `crates/flui-core/src/app.rs`.
- **Fix:** extract `SystemWindowTabController` (~290 lines), `Application` builder
  to submodules. **Spec:** follow-up.

#### E16 — 157 "GPUI" / "gpui_" mentions in core sources
- **Where:** 25 files, including public docstrings (`lib.rs:90, 269`,
  `prelude.rs:1`) and `_ownership_and_data_flow.rs` doctests using
  `gpui_platform::application()` (which doesn't exist).
- **Fix:** rebrand to "flui". Phase 0 work.

#### E17 — 29 `pub use mod::*;` glob re-exports in `lib.rs`
- **Where:** `crates/flui-core/src/lib.rs`.
- **Fix:** convert to explicit lists per module (continuation of S01a.3 prec
  edent). Phase 0 work.

#### E18 — `derive_more = "0.99.17"` (2021) outdated
- **Where:** `crates/flui-core/Cargo.toml:74`; used in 10 files.
- **Fix:** upgrade to 2.x; switch trivial `Deref/DerefMut` to manual impls.
  Phase 0 work.

#### E19 — 47 `// TODO` / `// FIXME` markers; 13 `#[allow(dead_code|unused)]`
- **Fix:** triage in Phase 0. Convert to issues or fix.

#### E20 — `prelude.rs` is 9 lines
- **Where:** `crates/flui-core/src/prelude.rs`.
- **Fix:** add `Pixels`, `px`, `point`, `size`, `Hsla`, `rgb`, `rgba`,
  `SharedString` to existing trait re-exports. Phase 0.

#### E21 — `with_context().unwrap()` anti-pattern
- **Where:** `crates/flui-core/src/app.rs:1701-1702, 1764-1765`.
- **Fix:** proper error propagation or `.expect_with` helper. Phase 0.

#### E22 — `unwrap_or_else(|| panic!(...))` instead of `expect`
- **Where:** `crates/flui-core/src/app.rs:1737`,
  `crates/flui-core/src/local_util.rs:188`.
- **Fix:** replace with `expect`. Phase 0.

#### E23 — 4 `#[expect(missing_docs)]` in `scene.rs`
- **Where:** `crates/flui-core/src/scene.rs:18, 22, 26, 41`.
- **Fix:** write docs. Phase 0.

---

## 8. Quality gates / Done criteria

### 8.1 Per-spec done criteria

A spec is done when:

1. Public API documented with rustdoc; no `#[allow(missing_docs)]`.
2. At least one runnable example exists in `examples/learn/`.
3. Unit tests cover the primary control flow.
4. Integration test covers cross-subsystem interaction.
5. Bench harness has at least one entry tracking the hot-path metric.
6. Frame-budget assertion passes on the reference scene.
7. S01 lock tests remain green.
8. Issue catalog (§7) is updated — entries listed under "addressed by" are marked
   ✅.

### 8.2 Per-phase done criteria

**Phase 0** done when:

- Workspace builds clean with no `derive_more 0.99` references.
- `lib.rs` has zero `pub use module::*;` globs (or each remaining one is documented
  with rationale).
- Public docstrings have zero "GPUI" / "gpui_" references.
- `prelude` is comprehensive enough that downstream code does not need to import
  `Pixels`, `px`, `point`, `size`, etc. individually.
- TODO count is < 20 (down from 47).

**Phase 1** done when:

- `Animation<T>` trait hierarchy lands; existing `AnimationController` is a
  concrete implementor.
- `provider/stack.rs` is gone; reactive Provider works.
- `Element` trait uses context objects (`PaintCx`, `LayoutCx`, `PrepaintCx`).
- `Style` decomposed into composite struct.
- `FrameClock` exists; per-phase profiler surfaces in inspector.
- `Rc<RefCell<Box<dyn>>>` removed from gesture arena hot path.
- 60-FPS bench harness baseline captured.

**Phase 2** done when:

- `Widget` / `StatefulWidget` / `Key` traits land.
- Reconciliation algorithm passes a stress test (1000-element list with
  re-orderings).
- `BuildCx` integrates with Provider for Theme / MediaQuery.
- One non-trivial example app uses the widget layer.

**Phase 3** done when:

- Each S08–S15 spec is complete and ✅ in `2026-04-13-flui-core-roadmap.md`.

**Phase 4** done when:

- `flui-foundation`, `flui-runtime`, `flui-painting`, `flui-text`, `flui-element`,
  `flui-widget`, `flui-input` exist as separate crates.
- Each builds independently.
- Edits to a single crate trigger sub-second incremental rebuild of dependents.

**Phase 5** done when:

- `flui-widgets` has ≥ 50 widgets covering layout, container, display,
  interactive, form, scrollable, navigation categories.
- `flui-material` has Material 3 button / card / app bar / scaffold.
- One representative app (a notes app, a chat client, etc.) is implementable in
  ≤ 500 lines of widget code.

### 8.3 60-FPS quality gate

A bench scene with the following must hit ≤ 16.67 ms per frame on the
reference hardware (M2-class Mac, mid-range Linux box, mid-range Windows box):

- 100 active animations
- 200 hitboxes with at least 50 active recognizers
- 1000-element scrollable list
- 2 stacked dialogs with backdrop blur
- One running video / animated SVG element

Per-phase budgets per §3.1.

---

## 9. Anti-patterns to refuse

When proposing or reviewing a change, refuse if it does any of the following:

1. Adds `Rc<RefCell<...>>` on a paint or dispatch hot path.
2. Allocates inside dispatch / tick / paint loops.
3. Calls `Instant::now()` more than once per frame from animation code.
4. Adds a `pub` item without a rustdoc.
5. Adds a public type that is not `#[non_exhaustive]` if it has any chance of
   growing.
6. Introduces a `__internal_` / `__assert_` prefix to escape doc-hidden constraints.
7. Adds a `#[allow(dead_code, reason = "future consumer")]` instead of an issue
   entry + commit-when-needed plan.
8. Silently `Option::default()`-s or `unwrap_or_default()`-s where the absence is
   actually a bug.
9. Returns `Result` when `Option` is appropriate, or vice versa.
10. Imports `std::collections::HashMap` instead of `collections::FxHashMap`
    (workspace clippy disallowed-types).
11. Calls `std::process::Command` (workspace clippy disallowed-methods —
    use `smol::process::Command`).
12. Skips `--no-verify` git hooks.
13. Renames an `unimplemented!()` site without updating
    `docs/platform-expected-stubs.md`.
14. Touches `crates/flui-core/src/platform/**` (frozen; new platform code goes to
    `crates/flui-platform/`).

---

## 10. Pre-flight checks before any change

Before writing or merging, the agent (or human) verifies:

1. Issue ID is cited in the commit message / spec.
2. Constraints (§2) are not violated.
3. Anti-patterns (§9) are not introduced.
4. The change has a test.
5. If the change touches a hot path, a bench entry exists.
6. If the change is API-breaking, a spec exists in `docs/superpowers/specs/`.
7. `cargo fmt` + `cargo clippy --workspace --all-targets` + `cargo test --workspace`
   pass locally.
8. `cargo bench` does not regress > 5% on tracked metrics without justification.

---

## 11. Glossary

| Term | Meaning |
|------|---------|
| Element | Low-level GPUI-derived primitive in `flui-core/src/element.rs`. The runtime substrate. |
| Render | Trait implemented by views; produces an Element tree on each frame. |
| Entity | `Entity<T>` — type-safe handle to an instance of `T` owned by `App`. |
| Widget | (Future) Flutter-style declarative UI component, sits on top of Element. |
| Key | (Future) Stable identity for a Widget across rebuilds. |
| Lerp | Linear interpolation trait; foundation of animations. |
| Tween | Begin-end pair, transformable via t ∈ [0, 1]. |
| Curve | Easing function. |
| Animation<T> | (Future) Composable observable producing a T over time. |
| Listenable | (Future) Anything that can notify status / value listeners. |
| GestureRecognizer | One competitor in the gesture arena. |
| GestureArena | Per-pointer competition resolver. |
| Provider | (Future) Reactive InheritedWidget analogue. |
| BuildCx | (Future) Build-time context with inherit / read access. |
| FrameClock | (Future) Per-frame cached time + delta. |
| FrameProfile | (Future) Per-phase budget tracker. |
| Costyl' (костыль) | Russian colloquial for "kludge" or "hack workaround". |

---

## 12. Source-of-truth pointers

- Existing roadmap: `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`
- Existing specs (S01a–S07.5): `docs/superpowers/specs/`
- Workspace AGENTS map: `AGENTS.md`
- AI Factory project context: `.ai-factory/DESCRIPTION.md`,
  `.ai-factory/ARCHITECTURE.md`, `.ai-factory/rules/base.md`
- Workspace lints: `clippy.toml`, root `Cargo.toml [workspace.lints]`
- Subagents (used proactively):
  - `flui-arch-reviewer` — for any change touching `App`, `Entity`, `Context`,
    `Window`, `Element`, `Scene`, `Platform` trait
  - `migration-risk-adversary` — for moves / extracts > 100 LoC
  - `wgpu-gpu-reviewer` — for scene / shader / pipeline changes
  - `rust-api-migration-auditor` — for `pub(crate) → pub` promotions, new public
    types, crate extractions

---

## 13. Final agent directives

When acting on this document:

1. **Pick the highest-priority unaddressed issue** from §7 that fits the current
   work scope.
2. **Open a spec** in `docs/superpowers/specs/<date>-S<N>-<slug>-design.md` if
   the issue is HIGH-RISK or API-BREAKING.
3. **Implement** within the spec's scope, citing the issue ID in every commit.
4. **Add tests + bench** per §8.1.
5. **Update §7** in this document — mark the issue ✅ when done.
6. **Surface decisions** that aren't covered here as "Open questions" in the spec
   or as new entries in §7.
7. **Do not refactor adjacent code** that is not in the issue's scope, even if it
   looks tempting. Open a separate issue.

The cumulative effect of executing this plan is a Rust-idiomatic, Flutter-parity
UI framework that hits 60 FPS as a structural property and stays maintainable as
the widget surface grows.
