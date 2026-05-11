# Architecture: Three-Tier Layered + Cargo Workspace

## Overview

flui-v2 is a **hard fork** of `gpui-ce` (the community edition of Zed's GPUI runtime). Upstream became inactive on framework-level evolution; flui-v2 takes ownership of the trajectory and diverges as needed. There is no upstream-sync commitment, no semver compatibility with `gpui`, and no obligation to preserve GPUI's public API. The fork exists precisely so that breaking changes can be made.

The project is structured as a **three-tier layered architecture** physically realized through a **Cargo workspace**. The three tiers — **Engine (A)**, **Framework (B)**, **Ecosystem (C)** — separate the GPU/runtime substrate, the Flutter-style developer-experience layer, and community-writable widget crates. Each tier is one or more Cargo crates; the dependency graph mechanically enforces the layering.

The model deliberately rejects deep DDD-style internal layering of the engine (an earlier v1 attempt with `flui-foundation`/`flui-engine`/`flui-rendering`/etc. was abandoned). The engine stays single-level (one crate, `flui-core`); architectural layering happens **on top of** the engine, not inside it. The architecture replicates **Flutter's user-visible feature surface** in Rust, not Flutter's 4-tree internal model — Rust ownership lets us collapse Widget+Element+RenderObject+Layer into "2 structures + 1 cache".

## Decision Rationale

- **Project type:** GPU-accelerated UI framework + ecosystem (libraries), hard fork of `gpui-ce`.
- **Tech stack:** Rust (edition 2024, MSRV 1.95 — bumped in K99), Cargo workspace, smol async runtime, wgpu/Metal/Direct3D 11, Taffy layout, cosmic-text/swash text.
- **Key factor — three tiers:** A UI ecosystem has three distinct concerns: (A) what the GPU and OS do, (B) what the framework does to make widgets ergonomic, (C) what app authors compose. Conflating them creates exactly the problems the project hit before — engine bleed into widgets (Zed-style `div`-based UI) or framework bleed into engine (no place for `Widget`/`Key`/`State` to live).
- **Key factor — Cargo crates as the boundary:** Each tier is enforced by Cargo's dependency graph. A forbidden dependency would form a cycle and `cargo build` refuses to compile.
- **Alternative considered (rejected):** Clean Architecture — there is no business logic to invert dependencies around; the framework's "domain" is the rendering pipeline itself.
- **Alternative considered (rejected):** Flat modular monolith — would lose the dependency-direction guarantees that the layered model gives for free.
- **Alternative considered (rejected):** v1's multi-crate engine split (`flui-foundation`/`flui-engine`/`flui-rendering`) — produced more confusion than separation; engine stays single-crate.
- **Alternative considered (rejected):** Flutter's 4-tree internal model (Widget/Element/RenderObject/Layer) — necessary in a GC language with ephemeral builds, unnecessary in Rust where ownership separates config from runtime naturally.
- **Sequencing decision: Kernel Cleanup precedes Framework.** An audit recorded in `.ai-factory/RESEARCH.md` and ROADMAP Phase 0-K identified 24+ structural issues in `flui-core` that block a healthy Framework tier — broken Provider, no Widget identity / `Key`, `Render::&mut self` semantics, undefined re-entrancy contract, AppCell, Element param explosion, action globals, leaky coordinate-space type-safety, no layout cache. K99, K15, K07, and K05 have landed; K01 Provider rewrite is next in the critical chain before K02, K03, and K04. The Framework tier (Phase II-F / SF##) waits for the Phase 0-K critical chain to land. Building Framework on the current kernel produces "construct on cracks" — a refactor that has to be redone after the kernel is fixed. The K-track exists precisely to avoid that double work.

## Three-Tier Strategic Model (A / B / C)

```
   ┌─────────────────────────────────────────────────────────┐
   │  C.  ECOSYSTEM tier (community-writable)                │
   │      flui-widgets, flui-material, flui-cupertino,       │
   │      flui-theme, flui-navigator, flui-a11y,             │
   │      third-party crates                                 │
   │      ───────────                                        │
   │      Success metric: a third-party widget crate         │
   │      can be written against stable Framework API        │
   └────────────────────┬────────────────────────────────────┘
                        ▲ depends on stable Framework API
   ┌────────────────────┴────────────────────────────────────┐
   │  B.  FRAMEWORK tier (Flutter developer experience)      │
   │      flui-framework (NEW — to be created in Phase II-F) │
   │      Widget + Key + State + BuildCx + Provider          │
   │      Reconciliation + dirty-list                        │
   │      Theme.of() / MediaQuery.of() / Navigator.of()      │
   │      ───────────                                        │
   │      "Flutter feature surface" — the API app authors    │
   │      see; what makes flui feel like Flutter             │
   └────────────────────┬────────────────────────────────────┘
                        ▲ uses Engine primitives
   ┌────────────────────┴────────────────────────────────────┐
   │  A.  ENGINE tier (substrate)                            │
   │      flui-core (App + Entity + Element + Scene +        │
   │                 Window + Layout + Text + Gesture +      │
   │                 Animation)                              │
   │      flui-platform (skeleton — Phase III)               │
   │      flui-macros (proc macros)                          │
   │      ───────────                                        │
   │      Stabilization = closing S08-S15 gaps; runtime      │
   │      stays single-level (one crate)                     │
   └─────────────────────────────────────────────────────────┘
```

### Framework tier as "2 structures + 1 cache"

The Framework tier (B) does NOT introduce 4 trees. It introduces:

```
   Widget          (immutable config struct, derive macro,
                    cheap clone, recreated each rebuild — like
                    Flutter Widget but enforced immutable by Rust)
       │ build()
       ▼
   Element tree    (current flui-core Element — runtime,
                    layout/paint, hit-test — UNCHANGED;
                    plays the role of Flutter Element +
                    RenderObject combined)

   ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─

   StateMap        (HashMap<ElementId, Box<dyn State>>,
                    FLAT — not a tree — survives rebuilds,
                    keyed by ElementId or user-provided Key.
                    Reconciliation is per-position within
                    siblings, not global tree-wide)
```

Rust ownership replaces Flutter's 4-tree separation:
- Widget = config (immutable by `&self`, no GC pressure since drop is deterministic)
- Element = runtime (already exists in flui-core)
- State = mutable owned data (one `Box<dyn State>` per stateful element id, not a tree)
- Layer = `Scene` (already in flui-core, retained GPU primitives)

This is the central simplification vs Flutter and the central justification for Rust as the implementation language.

## Folder Structure

```
flui-v2/
├── Cargo.toml                          # Workspace manifest (members, profiles, workspace lints)
├── crates/
│   │
│   │   ╔══ Engine tier (A) ═══════════════════════════════════════════╗
│   ├── flui-platform/                  # Engine — platform abstraction (skeleton, Phase III)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── flui-core/                      # Engine — runtime, rendering, layout, input, executor
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs / app/           # App + Entity + Context
│   │       ├── element.rs / elements/  # Element tree (substrate for Framework tier)
│   │       ├── scene.rs                # Scene primitives (GPU-reviewed)
│   │       ├── path_builder.rs
│   │       ├── animation/              # Animation<T> trait family (S21 — done)
│   │       ├── gesture/                # GestureArena (S07 + S07.5b — done)
│   │       ├── executor.rs             # Async executor on smol
│   │       ├── input.rs / interactive.rs / key_dispatch.rs / keymap/
│   │       ├── platform/               # ⚠️ frozen — new platform code → flui-platform
│   │       │   ├── mac/  windows/  linux/  wgpu/  web/  test/
│   │       └── ...
│   ├── flui-macros/                    # Engine — procedural macros (derive Render, IntoElement, …)
│   │
│   │   ╔══ Framework tier (B) ═══════════════════════════════════════╗
│   ├── flui-framework/                 # Framework — Widget/State/BuildCx/Provider over core Key (PLANNED — Phase II-F)
│   │   ├── Cargo.toml                  # depends on flui-core only
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── widget.rs               # Widget + StatefulWidget traits
│   │       ├── key.rs                  # Framework re-exports/wrappers over flui-core Key substrate
│   │       ├── state.rs                # State<W> + StateMap (flat, keyed by ElementId)
│   │       ├── build_cx.rs             # BuildCx: read/inherit/setState/depend
│   │       ├── reconcile.rs            # Sibling reconciliation by (TypeId, Key)
│   │       ├── provider.rs             # Framework Provider API over flui-core inherited registry
│   │       └── adapter.rs              # Widget → Element compilation (replaces Component<RenderOnce> path)
│   │
│   │   ╔══ Ecosystem tier (C) ═════════════════════════════════════════╗
│   ├── flui-widgets/                   # Ecosystem — widget library (planned, depends on flui-framework)
│   ├── flui-a11y/                      # Ecosystem — accessibility / semantic tree (planned, S08 spec)
│   ├── flui-theme/                     # Ecosystem — theming via Provider (planned)
│   ├── flui-material/                  # Ecosystem — Material 3 widgets (planned)
│   ├── flui-cupertino/                 # Ecosystem — iOS-style widgets (planned, future)
│   └── flui-navigator/                 # Ecosystem — type-safe routing
│
├── examples/                           # Application code (uses Framework + Ecosystem)
│   ├── nav_demo/
│   ├── material_demo/
│   └── animation_demo/
├── tooling/
│   └── lock-checks/                    # Tooling crate (not a tier member)
├── docs/superpowers/{specs,plans}/     # Design docs and plans
└── .ai-factory/                        # AI Factory project context
```

The workspace `[workspace.lints]` block in the root `Cargo.toml` is the single source of lint configuration; each member opts in via `[lints] workspace = true`. Workspace-wide Clippy denies `dbg_macro`, `redundant_clone`, `declare_interior_mutable_const`, and the `disallowed-methods` rules in `clippy.toml` push async-safe `smol::process` over `std::process`.

## Dependency Rules

The three tiers form a strict downward dependency graph. Each crate may only depend on crates at lower tiers (or sibling crates within the same tier when explicitly authorized).

| Tier | Crate(s) | May depend on |
|---|---|---|
| **C — Ecosystem (App)** | `examples/*` | Tiers A, B, C |
| **C — Ecosystem (Routing)** | `flui-navigator` | Tiers A, B; sibling Ecosystem crates by explicit authorization |
| **C — Ecosystem (Widgets)** | `flui-widgets`, `flui-a11y`, `flui-theme`, `flui-material`, `flui-cupertino` | Tiers A, B; **sibling Ecosystem crates only when authorized** (e.g., `flui-material` → `flui-widgets` is OK; reverse is NOT) |
| **B — Framework** | `flui-framework` | Tier A only |
| **A — Engine (Runtime)** | `flui-core` | Tier A only (depends on `flui-platform` + `flui-macros`) |
| **A — Engine (Macros)** | `flui-macros` | proc-macro toolchain only — no flui crates |
| **A — Engine (Platform)** | `flui-platform` | external crates only (wgpu, metal, windows, wayland-client, x11rb, ash, …) |
| — Tooling | `tooling/lock-checks` | not a tier member; isolated |

**Allowed:**
- ✅ `flui-navigator` → `flui-framework` → `flui-core` → `flui-platform`
- ✅ `flui-widgets` → `flui-framework` → `flui-core`
- ✅ `flui-material` → `flui-widgets` → `flui-framework` → `flui-core`
- ✅ `examples/nav_demo` → `flui-navigator` + `flui-framework` + `flui-core`
- ✅ `flui-core` → `flui-macros` (proc-macro consumer)
- ✅ Third-party widget crate → `flui-framework` (this is the ecosystem entry point)

**Forbidden:**
- ❌ `flui-core` → `flui-framework` (upward — Engine knows nothing about Framework)
- ❌ `flui-framework` → `flui-widgets` (Framework is API, Widgets are consumers)
- ❌ `flui-platform` → `flui-core` (would cycle the migration)
- ❌ `flui-macros` → any other `flui-*` crate (proc-macro crates must stay leaf)
- ❌ Adding new platform code under `crates/flui-core/src/platform/**` — that tree is frozen; new platform code goes to `crates/flui-platform/`.
- ❌ Sibling Ecosystem-tier dependencies without explicit authorization (e.g., `flui-widgets` → `flui-material` is forbidden — Material consumes Widgets, not the other way).
- ❌ Re-introducing v1's multi-crate engine split (`flui-foundation`/`flui-engine`/`flui-rendering`/…). Engine stays single-crate.
- ❌ Application code (`examples/*`) reaching into `flui-core` internals (e.g., `flui_core::platform::*`) for app-style work. Apps go through Framework + Ecosystem.

Cargo enforces these rules mechanically: a forbidden dependency would form a cycle and `cargo build` would refuse to compile.

## Tier Communication Patterns

- **Downward calls (allowed):** higher-tier crates use lower-tier types and traits directly through their public API. `flui-widgets` calls `flui-framework::Widget::build`, which calls into `flui-core::Element`.
- **Upward signaling (allowed via traits):** lower-tier crates expose traits and event types; higher tiers implement / observe them. `flui-core` exposes `Element`; `flui-framework` provides a `Widget`-to-`Element` adapter that implements it. Lower tiers never call up.
- **Cross-tier types:** primitive geometry / styling types live in `flui-core` and are re-exported through `flui-framework`. Do not duplicate them in the Ecosystem tier.
- **Public surface discipline:** `flui-core` re-exports are explicit (per S01a3). Framework and Ecosystem crates follow the same convention. No blanket `pub use crate::*`.
- **Macros:** procedural macros from `flui-macros` are consumed by Tier A+; macros must not reach into a specific user crate's types. New macro for `derive(Widget)` lives in `flui-macros` (or a sibling proc-macro crate) but operates on `flui-framework::Widget`.
- **Provider / inherited values:** `flui-core` owns the low-level per-Window inherited-value registry and dependency invalidation substrate. The Framework tier owns the ergonomic Widget/BuildCx/Provider API that builds on that substrate. Ecosystem crates (Theme, MediaQuery, Localizations) should consume the Framework API once Phase II-F lands rather than depending directly on Engine internals.

## Framework Tier Internals

This section documents the internal structure of `flui-framework` (Tier B), since it is the most novel and where the project diverges most from both upstream GPUI and from Flutter's internal model.

### Widget vs Element vs State

```
   Widget                    Element                   State<W>
   ──────                    ───────                   ────────
   Immutable config          Runtime substrate         Mutable owned data
   Implements Widget         Implements                Implements WidgetState<W>
                             flui_core::Element
   Recreated every rebuild   Survives rebuilds         Survives rebuilds
                             (until unmounted)         (one per stateful Element)
   Cheap to clone            Identity = ElementId      Identity = same ElementId
                                                       as the host Element
   No business logic         Layout + paint + hit      State machine + setState
                             test (existing)           triggers Element rebuild
```

### Reconciliation

```
   Old children (in StateMap):     New children (from Widget::build):
   ─────────────────────────       ─────────────────────────────────
   [(TypeId_A, Key_a1), id=7]      [(TypeId_A, Key_a1), pos=0]
   [(TypeId_B, Key_b1), id=8]      [(TypeId_C, Key_c1), pos=1]
   [(TypeId_A, Key_a2), id=9]      [(TypeId_A, Key_a1), pos=2]   ← duplicate Key collision
                                   [(TypeId_A, Key_a2), pos=3]

   Match strategy: (TypeId, Key) pair, position-as-fallback when Key absent.
   On match: State survives, did_update_widget() called.
   On mismatch: old State::dispose() runs; new State created.
   Duplicate Key collision: panic in debug, position-fallback in release.
```

The reconciliation pass is **O(siblings)** at each level, not O(tree). With `(TypeId, Key)` hashing it is amortized O(1) per child. This is the principal performance invariant of Tier B.

### Provider (InheritedRegistry)

K01 moved the low-level inherited-value substrate into `flui-core`, replacing the old `provider/stack.rs` thread-local global. This is intentionally an Engine substrate: it owns per-Window value storage, provider scope identity, dependency recording, and dirty-view invalidation. The planned Framework tier will wrap it with Flutter-style `Provider` / `BuildCx::inherit<T>()` ergonomics.

```
   Per-Window registry:
   FxHashMap<TypeId, InheritedEntry { value, version, dependents: SmallVec<[ElementId; 4]> }>

   read<T>()         — non-subscribing lookup
   inherit<T>()      — subscribing (adds caller's ElementId to dependents)
   provide<T>(v)     — pushes value, bumps version, marks dependents dirty
```

Window isolation is automatic. Theme, MediaQuery, DefaultTextStyle, Localizations all build on this single mechanism.

## Key Principles

1. **Cargo is the architecture.** A crate boundary is the architectural boundary. Don't bypass it with workspace-internal `path` shortcuts that hide a tier violation.
2. **The Engine stays single-level.** Do not re-introduce a v1-style multi-crate engine split (`-foundation`/`-engine`/`-rendering`/…). flui-v2 deliberately keeps a single-crate engine.
3. **The Framework is "2 structures + 1 cache", not 4 trees.** Widget config + Element runtime + flat StateMap. No RenderObject as a separate tree. No Layer tree (Scene already exists in Engine).
4. **Hard-fork posture.** flui-v2 is not a tracking fork of `gpui-ce`. May cherry-pick upstream fixes selectively, but is not bound by upstream API or roadmap. Breaking changes from upstream are the entire point of the fork.
5. **Platform code lives in `flui-platform`.** `crates/flui-core/src/platform/**` is frozen. New backend code, new platform features, and any non-trivial platform fix must land in `flui-platform`, not grow the in-tree module.
6. **Explicit re-exports.** No `pub use crate::*` anywhere — Engine, Framework, or Ecosystem.
7. **No allocation on the rebuild hot path.** Framework tier code in `Widget::build`, reconciliation, and `setState` propagation must not allocate. Use `SmallVec`, `FxHashMap`, capacity-preserving `clear()`. This is what makes Rust faster than Flutter for the same widget tree.
8. **No `Rc<RefCell<…>>` on dispatch / tick / paint hot paths.** Owning structures + index-based references; runtime borrow checks belong outside hot loops.
9. **Async-safe by default.** Use `smol`-based primitives. `clippy.toml` enforces `smol::process::Command::*` over `std::process::Command::*`.
10. **`unimplemented!()` and `unreachable!()` are tracked, not ornamental.** Inventoried by S01a; classify before touching.
11. **MSRV 1.95.** Edition 2024. Three-file synchronization invariant: `Cargo.toml` `[workspace.package].rust-version`, `rust-toolchain.toml` `channel`, and `clippy.toml` `msrv` must agree. Document MSRV bumps explicitly (see K99 design spec for the precedent). Modern idioms unlocked by 1.95 (AFIT, RPITIT, edition-2024 lifetime captures, async closures, let-chains, `std::sync::{OnceLock, LazyLock}`, `unsafe extern`, `#[diagnostic::on_unimplemented]`) are encouraged where they improve clarity.
12. **Determinism on the GPU path.** Offscreen / golden-test outputs must remain reproducible; coordinate with the `wgpu-gpu-reviewer` agent for any change in `crates/flui-core/src/platform/wgpu/**`, `scene.rs`, the Metal renderer, or the DirectX renderer.
13. **Ecosystem KPI.** Public API of `flui-framework` is `cargo-semver-checks` clean. A third-party widget crate must be implementable against stable Framework API. This is the success metric for "Flutter ecosystem parity".

## Code Examples

### Manifest tiering (Cargo as the boundary)

```toml
# crates/flui-framework/Cargo.toml — Tier B may depend on Tier A only.
[package]
name = "flui-framework"
edition.workspace = true
rust-version.workspace = true

[dependencies]
flui-core = { path = "../flui-core" }       # ✅ downward dependency
# ❌ flui-widgets = { ... }                  # would cycle (Widgets depend on Framework)

[lints]
workspace = true
```

```toml
# crates/flui-platform/Cargo.toml — Engine substrate must NOT depend on flui-core.
[package]
name = "flui-platform"

[dependencies]
# ✅ external platform crates only
# wgpu = { workspace = true }
# wayland-client = { workspace = true }
# ❌ flui-core = { path = "../flui-core" }   # would form a cycle and break tiering
```

### Explicit, curated re-exports (per S01a3)

```rust
// crates/flui-core/src/lib.rs
//
// ❌ DO NOT do this:
// pub use crate::*;
//
// ✅ Curate the public surface explicitly:
pub use crate::app::App;
pub use crate::element::Element;
pub use crate::scene::Scene;
pub use crate::animation::AnimationController;
// ... and so on, item by item.
```

### Async-safe process spawning (Clippy-enforced)

```rust
// ❌ Denied by clippy.toml — blocks the executor thread:
// let _ = std::process::Command::new("cargo").status()?;

// ✅ Use the smol equivalent:
use smol::process::Command;
let _ = Command::new("cargo").status().await?;
```

### Adding a new platform feature (post-migration shape)

```rust
// crates/flui-platform/src/lib.rs
//
// New platform features land HERE, not under crates/flui-core/src/platform/**.
// flui-core then consumes the abstraction:
//
//   use flui_platform::{Window, Display, Renderer};
//
// Cargo enforces the direction: flui-platform never imports flui_core.
```

### Framework: defining a Stateful Widget (target shape)

```rust
// In a third-party crate or flui-widgets:
use flui_framework::{StatefulWidget, WidgetState, BuildCx, Key, Widget};

#[derive(Widget)]
pub struct Counter {
    initial: i32,
    #[widget(key)] key: Option<Key>,
}

impl StatefulWidget for Counter {
    type State = CounterState;
    fn create_state(&self) -> CounterState {
        CounterState { value: self.initial }
    }
}

pub struct CounterState { value: i32 }

impl WidgetState<Counter> for CounterState {
    fn build(&mut self, cx: &mut BuildCx<'_>) -> impl Widget {
        // ✅ build is allocation-free at framework level — only widget construction
        Column::new()
            .child(Text::new(format!("Count: {}", self.value)))
            .child(Button::new("Increment").on_press(cx.handler(|s: &mut Self| {
                s.value += 1;
                // setState is implicit through `cx.handler`
            })))
    }

    fn did_update_widget(&mut self, _old: &Counter) {
        // Called when parent rebuild produces a new Counter widget
        // for the same ElementId. State survives, this hook lets us react.
    }

    fn dispose(&mut self) {
        // Called when this Element unmounts.
    }
}
```

### Framework: reading inherited values (target shape)

```rust
use flui_framework::{BuildCx, Widget};

impl WidgetState<MyWidget> for MyWidgetState {
    fn build(&mut self, cx: &mut BuildCx<'_>) -> impl Widget {
        // ✅ Subscribing read — this widget rebuilds when Theme changes
        let theme = cx.inherit::<Theme>().expect("Theme not provided");

        // ✅ Non-subscribing read — for one-shot lookups in event handlers
        let media = cx.read::<MediaQueryData>();

        Container::new()
            .color(theme.colors.surface)
            .child(Text::new("Hi"))
    }
}
```

## Anti-Patterns

- ❌ **Bypassing the Framework tier.** App code or Ecosystem widgets reaching directly into `flui-core::Element` API. The whole point of Tier B is to be the supported integration surface.
- ❌ **Allocating in `Widget::build` or `WidgetState::build`.** This is the hot path. Use `SmallVec`, capacity-preserving `clear()`, struct-of-arrays patterns.
- ❌ **Storing `Rc<RefCell<…>>` in `State<W>`.** State is owned; mutation goes through `&mut self` in `build` and event handlers. Runtime borrow checks belong outside the rebuild loop.
- ❌ **Using `Component<C: RenderOnce>` (existing) as the Widget mounting adapter.** It is a one-shot RenderOnce shim, not a stateful Widget bridge. Framework provides its own adapter.
- ❌ **Adding new code under `crates/flui-core/src/platform/**`.** That tree is frozen; new platform code goes into `crates/flui-platform/`.
- ❌ **Pulling `flui-core` into `flui-platform`.** The cycle the migration exists to prevent.
- ❌ **Re-creating v1's multi-crate engine split** (`flui-foundation` / `flui-engine` / `flui-rendering` / …). Engine stays single-crate.
- ❌ **Blanket `pub use crate::*`.** Public surfaces are curated explicitly across all three tiers.
- ❌ **Sibling-crate dependencies in the Ecosystem tier without an explicit decision.** `flui-widgets` does NOT depend on `flui-material` — Material consumes Widgets, not the reverse.
- ❌ **`std::process::Command` for spawning.** Denied by `clippy.toml`; use `smol::process::Command`.
- ❌ **Silently deleting `unimplemented!()` / `unreachable!()` sites in platform code.** Classify them per the S01a inventory first.
- ❌ **Tracking upstream `gpui-ce` API.** flui-v2 is a hard fork. May cherry-pick fixes, never preserves API for compatibility's sake.
- ❌ **Bypassing review subagents.** Use `flui-arch-reviewer`, `migration-risk-adversary`, `wgpu-gpu-reviewer`, and `rust-api-migration-auditor` proactively on the matching change types.
- ❌ **Conflating Engine and Framework concerns.** If something feels like "Flutter DX" (Widget, State, BuildCx, Provider ergonomics, setState, did_update_widget, dispose) it belongs in Tier B (`flui-framework`), not Tier A (`flui-core`). K02 is the exception for identity substrate: `flui-core::Key` / `GlobalKey` / `ValueKey` are low-level engine identity primitives that the Framework tier consumes or wraps; don't define a second incompatible Key model. Existing `Render` / `Component` / `AnyView` machinery in `flui-core` stays as the Engine substrate; don't grow it into Framework concerns.
