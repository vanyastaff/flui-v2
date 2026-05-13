# SF01 — Widget + Key Trait Authoring Guide

**SF01 status:** landed 2026-05-12 on the Phase II-F kickoff PR. Implements the trait surface frozen by `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md` (plus Amendment 1 applied during T3.1 implementation).

## What landed

SF01 introduces the `flui-framework` Tier B crate. The crate publishes the trait surface every Tier C widget crate and app author writes against:

- `flui_framework::Widget` — immutable widget trait, `'static + Sized`, required `fn build(&self) -> impl IntoWidget`.
- `flui_framework::StatefulWidget` — widget with mutable state, associated `State: WidgetState<Self>` type and `fn create_state` factory.
- `flui_framework::WidgetState<W>` — forward-declared marker (body lands in SF04).
- `flui_framework::IntoWidget` — sealed conversion contract (no `dyn IntoWidget`).
- `flui_framework::Empty` — sealed null widget for trivial `build` bodies (SF01 Amendment 1).
- `flui_framework::Key` / `ValueKey` / `GlobalKey` — re-exports of the K02 identity substrate from `flui-core`.
- `flui_framework::prelude` — opt-in `use flui_framework::prelude::*;` (7 items; `WidgetState` excluded for stability).
- `#[derive(Widget)]` proc-macro in `flui-macros`, re-exported at `flui_framework::Widget` (macro namespace).

## How to write a widget today

### Stateless leaf

```rust
use flui_framework::Widget;

#[derive(Widget)]
struct MyButton;
```

The derive generates a `Widget` impl with `fn key` returning `None` (default) and `fn build` returning the sealed `Empty` placeholder. The widget compiles cleanly but cannot be mounted until SF07 ships the adapter.

### Widget with an identity key

```rust
use flui_framework::{Key, Widget};

#[derive(Widget)]
struct ListItem {
    text: String,
    #[widget(key)]
    key: Option<Key>,
}
```

The `#[widget(key)]` attribute marks one optional field that the derive uses to implement `Widget::key`. Field type MUST be `Option<Key>`. Field name is agnostic — `key`, `id`, `widget_key`, anything works:

```rust
#[derive(Widget)]
struct ListItem {
    text: String,
    #[widget(key)]
    id: Option<Key>,  // arbitrary field name
}
```

### Stateful widget (SF04-pending)

```rust
use flui_framework::prelude::*;
use flui_framework::WidgetState;  // not in prelude — stability rationale

#[derive(Widget)]
struct Counter {
    initial: i32,
    #[widget(key)]
    key: Option<Key>,
}

impl StatefulWidget for Counter {
    type State = CounterState;

    fn create_state(&self) -> CounterState {
        CounterState { value: self.initial }
    }
}

struct CounterState {
    value: i32,
}

// WidgetState body is empty in SF01. SF04 will add:
//   fn build(&mut self, cx: &mut BuildCx<'_>) -> impl IntoWidget
//   fn did_update_widget(&mut self, old: &Counter) {}
//   fn dispose(&mut self) {}
impl WidgetState<Counter> for CounterState {}
```

This compiles in SF01, but `Counter::create_state()` produces a state object with no behavior until SF04 fills the body. The widget cannot be mounted until SF07.

## Feature matrix: SF01 vs the rest of Phase II-F

| Capability | SF01 | SF02 | SF03 | SF04 | SF05 | SF06 | SF07 | SF08 |
|---|---|---|---|---|---|---|---|---|
| Define a `Widget` | ✅ | — | — | — | — | — | — | — |
| Define a `StatefulWidget` | ✅ shape only | — | — | full | — | — | — | — |
| `#[derive(Widget)]` macro | ✅ | — | — | — | — | — | — | — |
| `#[widget(key)]` field attribute | ✅ | — | — | — | — | — | — | — |
| Reconciliation (`(TypeId, Key)`) | — | ✅ | — | — | — | — | — | — |
| `BuildCx<'_>` with `read` / `inherit` | — | — | ✅ | — | — | — | — | — |
| `WidgetState::build` / `did_update_widget` / `dispose` | — | — | — | ✅ | — | — | — | — |
| `setState` / dirty-list / rebuild | — | — | — | — | ✅ | — | — | — |
| `Theme.of()` / `MediaQuery.of()` etc. | — | — | — | — | — | ✅ | — | — |
| Mount widgets onto Element tree | — | — | — | — | — | — | ✅ | — |
| `StreamBuilder` / `FutureBuilder` | — | — | — | — | — | — | — | ✅ |

## Anti-patterns

The Engine tier (`flui-core`) has several traits with similar names that are **intentionally distinct** from `flui_framework::Widget`:

| Trait / Type | Tier | Use when |
|---|---|---|
| `flui_framework::Widget` | B | Writing a widget for an app or another widget to compose. The standard Tier C path. |
| `flui_framework::StatefulWidget` | B | Need mutable state across rebuilds. Pair with `WidgetState<W>` (SF04+). |
| `flui_core::Render` | A | **Engine-internal** mutable view trait for window roots and `Entity<V: Render>`. **NOT for Tier C widgets.** |
| `flui_core::RenderOnce` | A | **Engine** compatibility path for stateless engine recipes. **NOT for Tier C widgets.** |
| `flui_core::ElementBuilder` | A | K03 **engine substrate**; the eventual lowering target of `Widget::build` (SF07 adapter). Not consumed directly by Tier C. |
| `flui_core::Component<C: RenderOnce>` | A | `#[doc(hidden)]` one-shot RenderOnce shim. **NOT a Widget mounting adapter.** |

**If in doubt:** write `Widget`. The other traits are engine internals reserved for the rendering substrate and the SF07 mounting adapter.

## Forward compatibility — what changes when later SFs land

### SF03 — `BuildCx<'_>` parameter (breaking)

SF03 will widen `Widget::build`:

```rust
// SF01:
fn build(&self) -> impl IntoWidget { Empty }
// SF03:
fn build(&self, cx: &mut BuildCx<'_>) -> impl IntoWidget { /* … use cx.read/cx.inherit … */ }
```

This is a **breaking trait method change** — every SF01 impl needs a one-line edit to add the `_cx: &mut BuildCx<'_>` parameter. For `#[derive(Widget)]` users, SF03 will update the generated body; no manual change needed for derived impls.

### SF04 — `WidgetState<W>` body (additive)

SF04 will add three required methods to `WidgetState<W>`:

```rust
pub trait WidgetState<W: Widget>: 'static {
    fn build(&mut self, cx: &mut BuildCx<'_>) -> impl IntoWidget;
    fn did_update_widget(&mut self, _old: &W) {}
    fn dispose(&mut self) {}
}
```

This is a **breaking change** for any out-of-tree `impl WidgetState<W> for X {}` written before SF04 lands. SF01 documented the trait as `Stability: UNSTABLE` precisely for this reason. The `did_update_widget` and `dispose` methods land with default empty bodies; only `build` is required.

### SF05 — `setState` integration (additive)

SF05 adds `cx.handler(...)` and explicit `setState` APIs on `BuildCx`. Existing SF03 / SF04 widgets continue to compile; using `setState` becomes possible.

### SF07 — Mounting (additive)

SF07 introduces the Widget → Element adapter. After SF07, widgets actually mount onto the engine `Element` tree. Until then, SF01 widgets are `cargo check`-only — useful for verifying the trait surface compiles, not for running a real app.

## Re-blessing trybuild snapshots (MSRV bumps)

The `#[derive(Widget)]` macro is tested via `trybuild` compile-fail fixtures. `.stderr` snapshots are pinned to rustc diagnostic wording at the current MSRV (1.95 per K99). When the MSRV changes:

1. Run `TRYBUILD=overwrite cargo test -p flui-framework --test widget_derive_compile`.
2. Inspect the diff for each updated `.stderr` file — only message-text or span changes are acceptable; structural changes (different error kind) indicate a real regression that must be fixed before re-blessing.
3. Commit the regenerated snapshots in a dedicated `ci: re-bless trybuild snapshots after MSRV bump` commit.

## Known limitations recorded by the SF01 design spec

- `dyn Widget` is permanently unsupported (RPIT in `fn build` makes the trait non-object-safe). Erasure for heterogeneous storage will arrive in SF02 / SF07 via the planned `BoxedWidget` newtype.
- `dyn IntoWidget` is permanently unsupported (associated type + by-value `self`).
- `#[widget(key)]` field type must be a syntactic `Option<…Key>`. Type aliases (`type WidgetKey = Key; … #[widget(key)] k: Option<WidgetKey>`) are NOT supported — proc-macros run before name resolution.
- `Empty` is a sealed SF01-internal placeholder. Tier C widget catalogue (SF02+) should use `Container`, `SizedBox`, `Spacer`, or domain-specific empty widgets instead.
- `cargo-semver-checks` may emit noise for the trait/derive name collision at `flui_framework::Widget` (the trait and the proc-macro re-export share the identifier across the type and macro namespaces — Rust permits this, but the tool may not model the namespace distinction). The R2 roadmap item will decide whether to add a `semver-checks.toml` allowlist or rename one side. Tier C crates that run `cargo-semver-checks` against an SF01-vintage `flui-framework` dep should expect this warning to remain until R2 lands.

## Cross-references

- Design spec: [`docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`](../specs/2026-05-12-SF01-widget-key-trait-design.md)
- Plan: [`.ai-factory/plans/feature-SF01-widget-key-trait.md`](../../../.ai-factory/plans/feature-SF01-widget-key-trait.md)
- Tier isolation QA: [`.ai-factory/qa/SF01-tier-isolation.md`](../../../.ai-factory/qa/SF01-tier-isolation.md)
- ARCHITECTURE.md §"Framework Tier Internals" — the "2 structures + 1 cache" model SF01 implements.
- K91 cross-track contract: when K91 replaces the `flui_core::lib.rs:154` `pub use element::*;` glob with explicit re-exports, the new list MUST preserve crate-root visibility of `Key`, `ValueKey`, `GlobalKey`.
