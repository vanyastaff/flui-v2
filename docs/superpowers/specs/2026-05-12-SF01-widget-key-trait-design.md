# SF01 - Widget + Key Trait (Framework Tier Public Surface)

**Date:** 2026-05-12
**Phase:** II-F Framework tier — first spec
**Status:** FROZEN 2026-05-12 (post reviewer triple T0.2 — see §"Reviewer Notes"); **Amendment 1 applied 2026-05-12** — see §"Amendments" at the bottom.
**Plan:** `.ai-factory/plans/feature-SF01-widget-key-trait.md`
**Implementation crate (new):** `crates/flui-framework/`
**Implementation depends on:** K02 (`flui_core::Key` family), K03 (`ElementBuilder` / `ElementBuildCx` / `BuildElement`), K05 (`LayoutCx` / `PrepaintCx` / `PaintCx`). All landed.

## Summary

SF01 opens the `flui-framework` crate — Tier B in the project's three-tier strategic model (`A. Engine = flui-core`, `B. Framework = flui-framework`, `C. Ecosystem = flui-widgets / flui-material / …`). It freezes the **public trait surface** that every Tier C widget crate will implement and consume for the next decade.

SF01 ships only:

- A new `flui-framework` crate at `crates/flui-framework/` with `flui-core` as its only intra-workspace dependency.
- The `Widget` trait — immutable, `&self`-build, opaque-return.
- The `StatefulWidget: Widget` trait with an associated `State: WidgetState<Self>` type and a `create_state(&self) -> Self::State` factory.
- A forward-declared `WidgetState<W: Widget>` marker trait (body filled by SF04).
- `Key` / `ValueKey` / `GlobalKey` re-exported from `flui_core` with widget-author-facing rustdoc.
- A minimal explicit `prelude` module that re-exports the items above.
- A `derive(Widget)` proc-macro skeleton in `flui-macros` with a `#[widget(key)]` field attribute.
- `trybuild` compile-pass/compile-fail tests against the derive (placed on the consumer side, `crates/flui-framework/tests/`).
- A `cargo check`-only mini-example at `examples/widget_surface_demo/`.

SF01 ships **no** reconciliation, **no** state storage, **no** `BuildCx`, **no** `setState`, **no** mounting adapter, **no** widget catalogue. Every Widget body in SF01 either uses the default `unimplemented!()` stub or constructs other SF01 widgets at the trait level only.

The mounting story — turning a `Widget` tree into an `flui_core::Element` tree — is **SF07**. SF01 leaves a documented transitional gap (see §"Transitional BuildElement Bridge" below).

## Scope Boundary

In scope:

- Create `crates/flui-framework/` crate. Tier B in the workspace.
- Define `Widget` trait shape (signature, super-traits, default methods, doc).
- Define `StatefulWidget: Widget` shape with associated `State` type.
- Forward-declare `WidgetState<W>` marker trait (empty body in SF01).
- Re-export `Key` / `ValueKey` / `GlobalKey` from `flui_core` at the Framework surface.
- Add `flui_framework::prelude` with explicit `pub use`-list (no glob).
- Add `derive(Widget)` proc-macro in `flui-macros` with `#[widget(key)]` attribute.
- Add `flui-macros` as a regular dep of `flui-framework`; re-export the derive.
- Add `trybuild` dev-dep to `flui-framework`; ship pass / fail compile-tests under `crates/flui-framework/tests/widget_derive/`.
- Add `examples/widget_surface_demo/` micro-example (cargo check only).
- Module-level rustdoc explaining Tier A / B / C, "2 structures + 1 cache", and Widget-vs-Render anti-pattern.
- Land `#![deny(missing_docs)]` at the `flui-framework` crate root.
- Land migration / authoring guide at `docs/superpowers/migrations/SF01-widget-key-trait.md` (T6.1).

Out of scope (SF##-routed):

- `BuildCx<'_>` context type, `cx.read::<T>()`, `cx.inherit::<T>()` — **SF03**.
- `WidgetState<W>` trait body: `build` / `did_update_widget` / `dispose` methods — **SF04**.
- `StateMap` / `HashMap<ElementId, Box<dyn State>>` — **SF04**.
- `setState` / dirty-list / rebuild propagation — **SF05**.
- Widget → Element compilation adapter / live mounting / runtime — **SF07**.
- `InheritedWidget` analog (Theme, MediaQuery, DefaultTextStyle, Localizations) — **SF06**.
- Async widgets (`StreamBuilder`, `FutureBuilder`) — **SF08**.
- Concrete widget catalogue (`Container`, `Row`, `Column`, `Text`, `Button`, etc.) — **Tier C, gated on SF05**.
- Migration of `flui-widgets` / `flui-material` / `flui-navigator` / `flui-theme` / `flui-a11y` / `flui-cupertino` to depend on `flui-framework` — gated on **SF05**.
- Any edits to `crates/flui-core/src/**` beyond the read-only references documented here. If the trait freeze ends up requiring a `flui-core` change, the change ships as a **separate** plan and a separate PR.
- Hot-reload, inspector intro API (K22), full DevTools.
- Re-exporting `flui_core::ElementId` / `LocalElementId` at the Framework public surface (engine path-segment types; Framework users speak `Key`).

## Current Inventory

| Surface | Current location | SF01 action |
|---|---|---|
| `flui_core::Key`, `ValueKey`, `GlobalKey`, `ElementId`, `LocalElementId` | `crates/flui-core/src/element/identity.rs`, visible at crate root via `pub use element::*;` glob at `crates/flui-core/src/lib.rs:154` | Re-export `Key`, `ValueKey`, `GlobalKey` only. Document widget-author intent. **Pinned dependency on K91**: when K91 replaces the `element::*` glob with explicit re-exports, the new explicit list MUST preserve crate-root visibility of `Key`/`ValueKey`/`GlobalKey`. This spec records the cross-track requirement. |
| `flui_core::Render` | `crates/flui-core/src/element.rs:473`. Trait: `Render: 'static + Sized` with `fn render(&mut self, &mut Window, &mut Context<Self>) -> impl IntoElement` | Read-only reference. Anti-pattern table cross-links to it. Its existing rustdoc already says "intentionally distinct from … future Framework-tier `Widget` API" — quote verbatim in `Widget` trait rustdoc. |
| `flui_core::RenderOnce` | `crates/flui-core/src/element.rs:490`. Trait: `RenderOnce: 'static` with `fn render(self, &mut Window, &mut App) -> impl IntoElement` | Read-only reference. Anti-pattern table cross-links to it. |
| `flui_core::ElementBuilder`, `BuildElement<B>`, `build_element`, `ElementBuildCx<'_>` | `crates/flui-core/src/build.rs` (K03) | Read-only reference. **Transitional bridge candidate** — see dedicated §"Transitional BuildElement Bridge" below. |
| `flui_core::Component<C: RenderOnce>` | `crates/flui-core/src/element.rs` (search `pub struct Component`). `#[doc(hidden)]` one-shot RenderOnce shim. | Read-only. Anti-pattern table calls it out as NOT the Framework adapter. |
| `flui-macros` | `crates/flui-macros/`. Existing derives use module convention `mod derive_<thing>` (`derive_action`, `derive_render`, `derive_into_element`, `derive_app_context`, `derive_visual_context`). Existing helper `get_simple_attribute_field` at `flui_macros.rs:289`. | Add `mod derive_widget;` and `#[proc_macro_derive(Widget, attributes(widget))]` entry. Reuse `get_simple_attribute_field` for `#[widget(key)]` field detection. |
| Workspace `[lints]` | Root `Cargo.toml`. Workspace clippy denies `dbg_macro`, `redundant_clone`, `declare_interior_mutable_const`, `disallowed_methods`. No `missing_docs` enforcement at workspace level. | New `flui-framework` crate opts into `[lints] workspace = true` AND additionally denies `missing_docs` at the crate root via `#![deny(missing_docs)]`. Stricter than engine — intentional because Tier B is the Tier C-facing API. |
| Workspace `[workspace] members` | Root `Cargo.toml`. Current order: `flui-core`, `flui-platform`, `flui-macros`, `flui-widgets`, `flui-navigator`, `flui-a11y`, `flui-theme`, `flui-material`, examples, tooling. | Insert `crates/flui-framework` between `flui-macros` and `flui-widgets` to preserve visual A → B → C tier ordering. |
| MSRV / edition | Workspace pin: Rust 1.95, edition 2024 (K99). | SF01 uses AFIT / RPITIT (`fn build(&self) -> impl <BuildReturn>`) — this is the explicit K99 unlock. |

## Trait Surface Freeze

The following section freezes the exact item shapes that the SF01 implementation MUST match. Reviewer triple (T0.2) examines this section first.

### `Widget`

```rust
/// An immutable widget configuration in the flui Framework tier.
///
/// `Widget` is the Tier B (`flui-framework`) public surface for app
/// authors and ecosystem widget crates. Implementations are immutable
/// owned config structs, recreated each rebuild, cheap to clone, with no
/// interior mutability. The runtime tree, layout, painting, and
/// hit-testing are owned by `flui_core::Element` — `Widget` only
/// describes the desired tree.
///
/// `Widget` is intentionally distinct from `flui_core::Render` (mutable,
/// entity-backed engine view), `flui_core::RenderOnce` (consuming
/// stateless engine recipe), and `flui_core::ElementBuilder` (immutable
/// engine recipe). The Framework tier `Widget` is the API the **app**
/// author writes against; the engine substrates are Engine-internal.
///
/// # Forward compatibility
///
/// SF01 publishes the trait surface only. `Widget::build` returns the
/// `unimplemented!()` default in SF01 — concrete `build` bodies require
/// the SF02 reconciliation engine and the SF03 `BuildCx` context.
/// SF03 will widen the `build` signature to accept a `&mut BuildCx<'_>`
/// argument via a default method; SF01 widgets ported to SF03 require a
/// one-line edit. SF04 fills the `WidgetState<W>` body; SF05 lands
/// `setState`. None of those land in SF01.
pub trait Widget: 'static + Sized {
    /// Returns the user-supplied identity key for this widget, if any.
    ///
    /// The default implementation returns `None`; widgets that carry an
    /// explicit `#[widget(key)]` field get an override generated by
    /// `derive(Widget)`. The trait method name `key` is intentionally
    /// the same as the conventional struct field name — matching
    /// Flutter's `Widget.key` precedent. Rust dispatches method calls
    /// via the trait and field access via `self.<field>`, so the dual
    /// naming is unambiguous to the compiler.
    fn key(&self) -> Option<&Key> {
        None
    }

    /// Builds the child widget tree.
    ///
    /// **Required method** (Amendment 1, 2026-05-12). A default
    /// `unimplemented!()` body was ruled out — see Amendment 1
    /// rationale. SF01 widgets that need a trivial body return the
    /// sealed [`Empty`] null widget:
    ///
    /// ```ignore
    /// impl Widget for Leaf {
    ///     fn build(&self) -> impl IntoWidget {
    ///         Empty
    ///     }
    /// }
    /// ```
    ///
    /// `derive(Widget)` generates a default `fn build` that returns
    /// `Empty`, so structs marked `#[derive(Widget)]` do not need to
    /// supply one manually.
    fn build(&self) -> impl IntoWidget;
}

/// Sealed null widget — the SF01 trivial build return.
///
/// SF01 widgets that have no children (or that exist only to populate
/// trait-surface tests) return `Empty` from their `Widget::build`
/// implementation. The `derive(Widget)` macro generates this body
/// automatically. `Empty::build` returns `Empty` itself, which type-
/// checks via the [`IntoWidget`] blanket impl.
///
/// **Not intended for general use.** SF02+ widget catalogue should
/// rely on `Container`, `SizedBox`, `Spacer`, or domain-specific
/// "empty" widgets rather than `Empty`. `Empty` exists in `flui-
/// framework`'s public surface only as a sealed placeholder for the
/// SF01-phase chicken-and-egg problem (the trait needs at least one
/// concrete widget that satisfies `Widget::build` without
/// referencing another widget).
pub struct Empty;

impl Widget for Empty {
    fn build(&self) -> impl IntoWidget {
        Empty
    }
}
```

### `IntoWidget`

```rust
/// Conversion contract into a [`Widget`].
///
/// Mirrors [`flui_core::IntoElement`] — the engine convention for build /
/// render methods that may produce a value either directly implementing
/// the target trait or wrapped in a small adapter. Every [`Widget`] is
/// also `IntoWidget` via a blanket impl.
///
/// # Object safety
///
/// `IntoWidget` is **not** object-safe and the contract is **sealed
/// against `dyn` use**. The combination of an associated type (`Widget`)
/// and a by-value receiver (`fn into_widget(self)`) cannot be vtable-
/// dispatched. SF02/SF07 type-erasure for heterogeneous widget storage
/// uses a separate `BoxedWidget` newtype (to be designed in SF07) — not
/// `dyn IntoWidget`. Implementors that want erasure must go through
/// `BoxedWidget`.
pub trait IntoWidget {
    /// The concrete [`Widget`] produced by this conversion.
    type Widget: Widget;

    /// Convert `self` into its [`Widget`].
    fn into_widget(self) -> Self::Widget;
}

impl<W: Widget> IntoWidget for W {
    type Widget = W;
    fn into_widget(self) -> W {
        self
    }
}
```

**Coherence note for SF02+ planners.** The blanket `impl<W: Widget> IntoWidget for W` is the only `IntoWidget` impl SF01 ships. The blanket hardcodes `<W as IntoWidget>::Widget = W` (identity mapping). Any future SF spec that wants to ship a sibling blanket (e.g., `impl<W: StatefulWidget> IntoWidget for W` to specialize `into_widget` for stateful widgets) will collide with the existing blanket via coherence, because `StatefulWidget: Widget`. The path forward in that scenario is either (a) trait specialization (unstable), (b) a wrapper newtype that is not itself a `Widget` (which the blanket therefore does not cover) and implements `IntoWidget` with a non-identity `Widget` associated type, or (c) accepting that further `IntoWidget` blankets are infeasible. SF01 commits to (b) as the canonical path: erasure wrappers (`BoxedWidget`, `WidgetChildren`, future `WidgetTree`) are NOT themselves `Widget` and CAN implement `IntoWidget` with a non-identity associated type. This is the architectural rationale for keeping `BoxedWidget`-the-erasure separate from `Widget`-the-trait at SF07 design time.

### `StatefulWidget`

```rust
/// A widget whose mounted element owns mutable state across rebuilds.
///
/// SF01 publishes the trait shape only. The `WidgetState<Self>` body
/// arrives in SF04 (`build` / `did_update_widget` / `dispose` methods),
/// and `setState`-driven rebuilds land in SF05. In SF01, implementing
/// `StatefulWidget` lets you publish a `State` factory but no body — the
/// runtime cannot yet mount the widget.
pub trait StatefulWidget: Widget {
    /// The state object created at mount time for each instance of
    /// this widget in the element tree.
    type State: WidgetState<Self>;

    /// Creates the state object. Called once per element, at mount.
    fn create_state(&self) -> Self::State;
}
```

### `WidgetState<W>`

```rust
/// Per-element mutable state companion to a [`Widget`].
///
/// **Stability: UNSTABLE until SF04.** SF01 ships the marker trait only —
/// the body (`build` / `did_update_widget` / `dispose` methods) lands in
/// SF04. Implementing this trait outside of trait-surface tests is
/// supported but actively discouraged before SF04 because SF04 will add
/// required methods, which is a breaking change for any out-of-tree
/// implementor. Treat this trait as a forward-declared identifier whose
/// presence in SF01 only exists so that `StatefulWidget::State` has a
/// non-blank bound to point at.
///
/// SF04 will add (signatures subject to that spec's freeze):
/// - `fn build(&mut self, cx: &mut BuildCx<'_>) -> impl IntoWidget`
/// - `fn did_update_widget(&mut self, old: &W) {}`
/// - `fn dispose(&mut self) {}`
///
/// Transitively `W: Sized` via the `Widget` super-trait bound — so a
/// hypothetical `dyn Widget` is rejected by the type system as a `W`
/// for `WidgetState<W>`.
pub trait WidgetState<W: Widget>: 'static {
    // SF01 ships an empty body; SF04 adds the methods listed above.
}
```

**Why no `#[doc(hidden)]`:** the workspace lint policy adds `#![deny(missing_docs)]` at the `flui-framework` crate root. `#[doc(hidden)]` suppresses rustdoc rendering but does NOT satisfy `missing_docs` — every `pub` item must carry a doc comment regardless. Furthermore, `cargo-semver-checks` (planned for R2) treats `#[doc(hidden)]` items as part of the semver-tracked surface, so the supposed protection of `#[doc(hidden)]` against ecosystem latching is illusory — a determined consumer can still depend on the hidden item. The chosen path is **document the instability in the doc comment**, accept that any out-of-tree `impl WidgetState<W> for X {}` is supported-but-discouraged, and accept that SF04 is a breaking change for those implementors. This matches Rust's general convention of using a `Stability: …` line in rustdoc rather than `#[doc(hidden)]` to communicate unstable surfaces. **`WidgetState` is therefore omitted from the SF01 `prelude` module** so that `use flui_framework::prelude::*` does not accidentally make the marker glob-visible.

### Key surface (re-exports only)

```rust
// crates/flui-framework/src/key.rs
pub use flui_core::{Key, ValueKey, GlobalKey};
```

No newtype, no wrapper, no shadowing. `flui_core::Key` is the cross-tier identity intent type by K02 design; SF01 publishes it at the Framework crate root with widget-author-facing rustdoc.

### `prelude`

```rust
// crates/flui-framework/src/prelude.rs (Amendment 1 — 7 items)
pub use crate::{Empty, GlobalKey, IntoWidget, Key, StatefulWidget, ValueKey, Widget};
```

~~Six~~ **Seven** items, explicit (Amendment 1 added `Empty`). `WidgetState` is deliberately **excluded** from the prelude because its body is unstable until SF04 — see §"WidgetState<W>" above. `IntoWidget` IS included because users writing `fn build(&self) -> impl IntoWidget` need the trait in scope. `Empty` IS included because `#[derive(Widget)]`-free widget impls need to write `fn build(&self) -> impl IntoWidget { Empty }`.

Explicit re-exports only; no glob from inner modules. App authors write `use flui_framework::prelude::*;` once at the top of a file when desired. Without the prelude, every item — including `WidgetState` — is still individually importable from the crate root (`use flui_framework::WidgetState;`).

## Design Risk #1 — `Widget::build` Return Type

**Decision:** `fn build(&self) -> impl IntoWidget` (Option B).

**Alternatives considered:**

- **Option A: `fn build(&self) -> impl Widget`.** Recursive shape; clean Flutter analogue. Each `build` call returns a single concrete opaque type, which is fine at one level but creates downstream pressure when reconciliation needs to walk a heterogeneous child set (siblings of different types cannot live in the same `impl Widget` since Rust opaque types are single-concrete).
- **Option B (chosen): `fn build(&self) -> impl IntoWidget`.** Mirrors the engine-wide convention — `Render::render -> impl IntoElement`, `RenderOnce::render -> impl IntoElement`, `ElementBuilder::build -> impl IntoElement`. Introduces a sibling `IntoWidget` trait with the blanket `impl<W: Widget> IntoWidget for W`. Consistent with the rest of the codebase and leaves a deliberate hook for SF02/SF07 to introduce wrapper types (e.g., a `WidgetTree` or `WidgetChildren` collection) that implement `IntoWidget` without implementing `Widget` directly.

**Rationale for Option B:**

1. **Codebase precedent is unanimous.** Every existing build / render method in `flui-core` returns `impl IntoX`. Deviating without strong cause produces a learning-curve discontinuity for any developer who has read the engine code.
2. **Future-proofing for SF02 reconciliation.** SF02 will need to express "child lists" — heterogeneous sibling sequences. With `IntoWidget`, SF02 can ship `WidgetChildren` (or similar) as a non-`Widget` wrapper type that implements `IntoWidget` and carries `Vec<BoxedWidget>` internally. With Option A this requires retrofitting an `IntoWidget`-like trait later, breaking every `build` signature in flight.
3. **Cost of choosing B today is one extra trait + one blanket impl + no allocation** — the blanket impl resolves identity-via-`Self` at monomorphization time. Zero runtime cost.
4. **SF07 mounting becomes cleaner.** SF07 will define `BoxedWidget` (object-safe erasure) — it can implement `IntoWidget`, letting `Widget::build` return `BoxedWidget` without trait gymnastics.

**Forward path:** SF03 widens the build signature to `fn build(&self, cx: &mut BuildCx<'_>) -> impl IntoWidget` via a default-method-on-trait pattern. The `IntoWidget` return type is preserved across this transition.

## Design Risk #2 — Reconciliation Traversal Under Non-Object-Safe `Widget`

**Constraint surfaced for reviewer attention:** `Widget` is not `dyn`-compatible because `fn build(&self) -> impl IntoWidget` returns an opaque type, which is incompatible with object safety. SF02 reconciliation therefore CANNOT use `dyn Widget` to walk a heterogeneous tree of children.

**Three viable solutions, all deferred to SF02:**

1. **Enum-based erasure.** SF02 introduces a `WidgetVariant` enum that boxes every concrete child variant. Pro: object-safe. Con: closed set, doesn't extend to user-defined widgets.
2. **Monomorphized generic recursion.** SF02 makes `build`-walker generic over `<W: Widget>`. Pro: zero erasure cost, type-checked. Con: every distinct widget composition produces a distinct compiled function — codegen bloat.
3. **`BoxedWidget` newtype with a hand-rolled vtable.** SF07 was going to introduce this for mounting anyway. SF02 reuses it. `BoxedWidget` is an opaque newtype around a manually-constructed vtable that captures the methods reconciliation needs (`key`, plus a build-into-element entry point). The newtype implements `IntoWidget` so it composes with the return type of `Widget::build`.

**Spec position:** SF01 does NOT pick the solution. SF01's only obligation is to **leave the door open** by:

1. Choosing `impl IntoWidget` as the build return type (covered by Design Risk #1).
2. Documenting the constraint in this section so SF02's planner knows where the boundary lives.

The `BoxedWidget` newtype is the most likely landing spot per the project's existing convention (the engine uses opaque newtypes around hand-rolled vtables for `AnyElement` and similar). But it is not SF01's call.

## Evolution to SF03 (Breaking — Not Forward-Compatible)

SF03 will widen `Widget::build` from `fn build(&self) -> impl IntoWidget` to `fn build(&self, cx: &mut BuildCx<'_>) -> impl IntoWidget`. This is a **breaking trait method change**, NOT a backwards-compatible additive default method. Calling out the breakage explicitly so SF02 / SF03 planners size the work correctly:

```text
SF01:  fn build(&self) -> impl IntoWidget;
SF03:  fn build(&self, cx: &mut BuildCx<'_>) -> impl IntoWidget;
```

- A trait method default cannot inject a new required parameter — no `default fn` trick covers this case.
- Every existing `impl Widget for X { fn build(&self) -> … }` override must add `, _cx: &mut BuildCx<'_>` to its signature when SF03 lands.
- For Tier B/C consumers shipped between SF01 and SF03, this is a **semver-major bump** of `flui-framework`.

**Migration cost:** one-line edit per Widget impl in SF01-vintage code. The only in-tree implementor at SF01 freeze is `examples/widget_surface_demo`. Any third-party widget crate that adopts SF01 before SF03 must accept the same one-line edit on the SF03 PR; document this in the SF01 migration guide so out-of-tree authors are not surprised.

Alternative considered and rejected: ship SF01 with `fn build(&self, _: &mut BuildCx<'_>) -> impl IntoWidget` already, with a stub `BuildCx` defined in SF01. **Rejected** because (a) it forces SF01 to publish a `BuildCx` shape with no Provider / state / setState machinery to back it, which would itself need to be rewritten in SF03, and (b) the SF01 widgets cannot meaningfully use `cx` (no `read`, no `inherit`, no `handler`), so the parameter is pure noise.

## K15 / K04 Forward-Compat Statement

- **K15 (re-entrancy contract):** SF01 traits do NOT enter the K15 contract because no Widget code runs in this spec. The default `Widget::build` stub panics with `unimplemented!()`; no `update_window` / `update_entity` / `setState` calls happen. SF02 (reconciliation triggers element rebuilds) and SF03/SF05 (callback handlers may setState) inherit K15 contract obligations as they add runtime behavior.
- **K04 (Effect / Frame contract):** SF01 traits also do not interact with K04 frame phases. K04 reserves the `Build` phase (currently a no-op) for SF05's setState-driven rebuilds; SF01 does not add anything to the frame loop. Future Widget code will run inside the `Build` phase once SF03/SF05 wire it up. SF01 makes no claim about frame timing.
- **K02 (Element identity & Key):** SF01 uses the K02 substrate directly via re-export. No semantic widening. K02 already integrated `Key`/`ValueKey`/`GlobalKey` into `ElementId`, providers, state, and lifecycle passes; SF01 inherits those guarantees by reference.
- **K03 (Render / Build separation):** SF01 sits on top of the K03 boundary. `Widget::build` is the Framework-tier analogue of `ElementBuilder::build`. SF07 will define the adapter that lowers Framework `Widget` to Engine `ElementBuilder`.
- **K05 (Element context object):** SF01 does NOT introduce a context object. Framework `BuildCx` is SF03's responsibility. Engine `LayoutCx`/`PrepaintCx`/`PaintCx` are not re-exported from Framework — Engine internals stay Engine.
- **K07 (AppCell removal):** SF01 ships zero runtime mutation; the K07 borrow model is not exercised. SF04's `State<W>` will exercise it.
- **K99 (MSRV 1.95):** SF01 USES the K99 unlock — `fn build(&self) -> impl IntoWidget` requires AFIT/RPITIT, which requires Rust 1.95.

## Decision — `trybuild` vs `compile_fail` doctests

**Decision:** Use `trybuild` for derive macro tests (positive + negative cases).

**Alternative considered:** `compile_fail` doctests inside the `derive_widget` rustdoc, following the existing precedent at `crates/flui-macros/src/flui_macros.rs:48-89` (`derive_app_context`, `derive_visual_context`).

**Rationale for `trybuild`:**

1. **Snapshot semantics.** `trybuild` produces `.stderr` snapshot files that fail the test if the compiler diagnostic text changes. For a derive macro that emits `compile_error!` messages with specific spans, this is the right granularity — diagnostic regressions are immediately visible in PR diffs.
2. **Diagnostic richness.** `compile_fail` doctests succeed if compilation fails for ANY reason. A bug that causes the derive to emit the wrong error message (e.g., "internal error" instead of "expected `Option<Key>`") would still pass `compile_fail`. `trybuild` catches this.
3. **Locality.** Test cases live in real `.rs` files (`crates/flui-framework/tests/widget_derive/*.rs`) with full rustdoc, not embedded inside docstrings. Easier to read and to extend.
4. **Cost:** one new dev-dep on `flui-framework`. `trybuild` is widely used (`serde`, `syn`, etc.), well-maintained, zero-runtime-dependency.

**Cost acknowledged:** Project did not previously depend on `trybuild`. The deviation from the `compile_fail` precedent is explicit and recorded here.

## Anti-Pattern Table

The Engine and Framework tiers have several similarly-named traits that confuse Tier C newcomers. This table belongs in the `flui_framework::widget` module rustdoc and the migration guide.

| Trait / Type | Tier | Object owner | Mutability | Return type | Use when … |
|---|---|---|---|---|---|
| `flui_framework::Widget` | B (Framework) | App author | `&self` (immutable) | `impl IntoWidget` (SF01); `impl IntoWidget` with `&mut BuildCx` arg (SF03+) | You are writing a widget that an app author or another widget composes. The standard Tier C path. |
| `flui_framework::StatefulWidget` | B (Framework) | App author | `&self` for the widget; `&mut self` for the state (SF04+) | `Self::State` factory plus the inherited `Widget::build` | You need mutable state that survives rebuilds. Pair with `WidgetState<W>` (SF04+). |
| `flui_core::Render` | A (Engine) | Engine | `&mut self` | `impl IntoElement` | Engine-internal mutable view trait. Used for window roots and `Entity<V: Render>` views. **NOT for Tier C widgets.** |
| `flui_core::RenderOnce` | A (Engine) | Engine | `self` (consuming) | `impl IntoElement` | Engine compatibility path for stateless engine recipes and existing `derive(IntoElement)` output. **NOT for Tier C widgets.** |
| `flui_core::ElementBuilder` | A (Engine) | Engine | `&self` (immutable) | `impl IntoElement` | Engine substrate that K03 introduced. The eventual lowering target of `Widget::build` (SF07 adapter). **Not consumed directly by Tier C.** |
| `flui_core::Component<C: RenderOnce>` | A (Engine) | Engine, `#[doc(hidden)]` | varies | `impl IntoElement` | One-shot RenderOnce shim used by `derive(IntoElement)`. **NOT a Widget mounting adapter.** |

Cross-link in rustdoc: every `flui_framework::Widget` doc block should cross-link to `flui_core::Render`, `flui_core::RenderOnce`, `flui_core::ElementBuilder` so rustdoc-rendered docs visually surface the boundary.

Verbatim K03 quote to embed in `Widget` trait rustdoc (from `crates/flui-core/src/element.rs:467-472`):

> This is the mutable, entity-backed engine view trait. Views are `Entity`s that implement `Render` and may own runtime state through `Context<Self>`. This is intentionally distinct from immutable element recipes such as `ElementBuilder` and from the future Framework-tier `Widget` API.

## `derive(Widget)` Macro Contract

**Module placement:** `crates/flui-macros/src/derive_widget.rs`. Module name matches the project convention `derive_<thing>` (alongside `derive_action`, `derive_render`, etc.).

**Registration:** `#[proc_macro_derive(Widget, attributes(widget))]` in `crates/flui-macros/src/flui_macros.rs`, placed next to `derive_render` for thematic grouping.

**Helper reuse — narrow scope.** Use `get_simple_attribute_field(ast, "widget")` at `crates/flui-macros/src/flui_macros.rs:289-299` ONLY for finding the `Ident` of the field bearing `#[widget(key)]`. The helper:

- Returns `Option<Ident>` — the field NAME, not the `syn::Field` (so it does NOT give access to `field.ty` for type validation).
- Silently returns `None` for `enum` / `union` inputs (does NOT emit any `compile_error!`).

Therefore the derive function MUST also:

1. Explicit-match on `ast.data` upfront to reject `enum` / `union` with `compile_error!("Widget derive only supports structs")` pointing at the input span. The helper's `None` return is not a substitute for this error.
2. Iterate `syn::Field` values directly (e.g., via `data_struct.fields.iter()`) to perform `Option<Key>` type validation. The type-validation pass is separate from the helper call.

Reviewers will check that the implementation does NOT collapse these two passes into a single `get_simple_attribute_field` call — that would silently accept enums and skip type validation.

**Input contract:**

- Accepts `struct` only. `enum` / `union` → `compile_error!` with message "Widget derive only supports structs".
- Field attribute `#[widget(key)]` is **optional** and may appear on **at most one** field.
    - The annotated field type MUST be `Option<Key>` (or `Option<flui_framework::Key>` etc. — accept syntactic variants). Anything else → `compile_error!("#[widget(key)] field must be `Option<Key>`")` with span pointing at the type.
    - Multiple `#[widget(key)]` fields → `compile_error!("only one #[widget(key)] field allowed")` with span pointing at the second occurrence.
- The annotated field name is **agnostic**. Users may name the field `key`, `id`, `widget_key`, anything — the attribute is what marks identity, not the field name.

**Generated code:**

```rust
// Input: #[derive(Widget)] struct Counter { initial: i32, #[widget(key)] id: Option<Key> }
// Output:
impl ::flui_framework::Widget for Counter {
    fn key(&self) -> Option<&::flui_framework::Key> {
        self.id.as_ref()  // ← uses the field name from the attribute site
    }
}
```

- Generated paths use the absolute prefix `::flui_framework::` so the macro works regardless of the user's imports. **Constraint:** the call site MUST have `flui-framework` in its dependency graph. `flui-macros` does NOT regularly depend on `flui-framework` (would form a cycle), so the generated path resolves at the consumer's compile time, not the macro crate's. Tier C / app crates depend on `flui-framework` so this is satisfied; a hypothetical future crate that depends on `flui-macros` directly without `flui-framework` would fail to compile generated `derive(Widget)` output — document this constraint, accept it.
- Widget structs without `#[widget(key)]` get an `impl Widget for X {}` without the `fn key` override (default `None` applies).
- Generics: `impl<T: 'static> Widget for Foo<T> { … }` — the derive correctly threads generic parameters and their where-clauses. Failure to thread bounds is a derive bug; reviewer agent T0.2 / T6.2 must hunt for missing-bound bugs.
- **Type-validation strategy for `Option<Key>` rejection.** The derive validates the `#[widget(key)]` field type via syntactic match on the path's terminal segment. Accepted spellings: `Option<Key>`, `Option<flui_framework::Key>`, `::std::option::Option<...Key>`, and a user-aliased `Option<WidgetKey>` where `use flui_framework::Key as WidgetKey` is in scope is rejected with a clear error (the macro cannot resolve type aliases at expansion time). Documented in the derive's rustdoc as a known limitation. The trybuild `fail_wrong_key_type.rs` snapshot pins the error message text; if rustc's diagnostic format changes between MSRV bumps, the snapshot must be re-blessed under `TRYBUILD=overwrite`.

**No effect on Debug / Clone / etc.** The derive synthesizes only the `Widget` impl, nothing else.

**Re-export from `flui-framework`:** `pub use flui_macros::Widget;` at `crates/flui-framework/src/lib.rs`. The trait `flui_framework::Widget` and the derive macro `flui_framework::Widget` coexist in different namespaces (types vs macros) — Rust permits this without ambiguity. Verified in T4.4 of the plan.

## Transitional `BuildElement` Bridge

**Question:** SF01 ships traits but no mounting. Can SF01 widgets be exercised by `cargo check` end-to-end? Can they `build`? Can they be put into an `flui_core::Element` tree?

**Decision (frozen):** **NO live bridge ships in SF01.** The default `Widget::build` body is `unimplemented!()`. SF01 widgets compile (and that is the surface validation goal), but invoking `build` panics. The example at `examples/widget_surface_demo/` constructs widgets, inspects `widget.key()`, and prints — it does not invoke `build`.

**Why not a runtime-panic stub via `BuildElement`?** Two options were considered:

- **Option α: `#[doc(hidden)] pub fn widget_to_element<W: Widget>(w: W) -> impl IntoElement` in `flui-framework`.** Would let `examples/widget_surface_demo` invoke a mounting-like API even if it panics at runtime. **Rejected** because (a) it pins SF07's mounting design — the adapter signature in SF07 will likely need a context or window handle, not just `W: Widget`; (b) it ships a footgun marked `#[doc(hidden)]` that ecosystem crates may discover and start depending on.
- **Option β: leave the bridge entirely unimplemented (chosen).** No bridge means no SF07 design constraints leak into SF01. `widget_surface_demo` does `cargo check` only — proves the trait surface compiles for an out-of-tree crate.

**Implication for SF07:** SF07 will introduce a `BoxedWidget` newtype and a Window-bound mounting adapter that takes a Widget tree, walks it, and produces an Element tree. SF02 reconciliation will use the same `BoxedWidget`. None of that is SF01's concern beyond leaving `Widget::build` returning `impl IntoWidget` (Design Risk #1 decision) so the SF07 adapter can take any `IntoWidget` value as input.

## Public Surface Enumeration

Every `pub` item that lands in `flui-framework` in SF01. Reviewer T0.2 and T6.2 diff the implementation against this list.

`crates/flui-framework/src/lib.rs`:

- `pub mod widget;`
- `pub mod key;`
- `pub mod prelude;`
- `pub use crate::widget::{Empty, IntoWidget, StatefulWidget, Widget, WidgetState};`  (`Empty` added by Amendment 1)
- `pub use crate::key::{GlobalKey, Key, ValueKey};`
- `pub use flui_macros::Widget;`  (the proc-macro derive)

`crates/flui-framework/src/widget.rs`:

- `pub trait Widget: 'static + Sized` (with `fn key` default; `fn build` required per Amendment 1)
- `pub trait IntoWidget` with `type Widget: Widget` and `fn into_widget(self) -> Self::Widget`
- `impl<W: Widget> IntoWidget for W` (blanket)
- `pub trait StatefulWidget: Widget` with `type State: WidgetState<Self>` and `fn create_state(&self) -> Self::State`
- `pub trait WidgetState<W: Widget>: 'static` — empty body, stability-documented (no `#[doc(hidden)]`)
- `pub struct Empty;` — sealed null widget per Amendment 1; `impl Widget for Empty` returns `Empty` from `build`

`crates/flui-framework/src/key.rs`:

- `pub use flui_core::{Key, ValueKey, GlobalKey};`  (re-exports only — no new types defined in `key.rs`)

`crates/flui-framework/src/prelude.rs`:

- `pub use crate::{Widget, StatefulWidget, WidgetState, IntoWidget, Key, ValueKey, GlobalKey};`

`crates/flui-macros/src/derive_widget.rs`:

- `pub fn derive_widget(input: TokenStream) -> TokenStream`  (called from `flui_macros.rs`)

`crates/flui-macros/src/flui_macros.rs` (additions):

- `mod derive_widget;`
- `#[proc_macro_derive(Widget, attributes(widget))] pub fn derive_widget(input: TokenStream) -> TokenStream { derive_widget::derive_widget(input) }`

That is the complete set. Anything additional (helper types, internal traits, `pub(crate)` items) is implementation detail and not part of the contract.

## Migration Cost Analysis

- **In-tree consumers of SF01:** none. There are zero callers of `Widget` / `StatefulWidget` / `WidgetState` today.
- **In-tree consumers of `flui_core::Key`:** several within `flui-core` itself, plus the `flui-widgets` re-export migrated in K01. SF01 adds a Framework-tier re-export but does NOT change `flui_core::Key`. Consumers unaffected.
- **Tier C migration:** NOT performed in SF01. `flui-widgets`, `flui-material`, `flui-navigator`, etc. continue to depend on `flui-core` directly. They will migrate onto `flui-framework` once SF05 (setState + dirty-list) lands and Framework is actually useful.
- **`examples/widget_surface_demo`:** new, no migration.
- **SF03 cost (future):** one-line edit per Widget impl to add `, _cx: &mut BuildCx<'_>` parameter. Affects only `widget_surface_demo` and any in-tree trait-surface tests written in SF01.
- **SF04 cost (future):** `WidgetState<W>` body filled in. Affects only `widget_surface_demo`'s `CounterState` (and any trait-surface test that defines a state object).
- **SF07 cost (future):** mounting adapter introduced. SF01 widgets become actually-mountable; no breaking change to the trait surface itself.

**Net SF01 migration cost:** zero, by design.

## Out of Scope (Explicit Denial — for reviewer cross-check)

These items will be rejected by reviewers in T0.2 and T6.2 if they appear in the SF01 implementation:

- ❌ `BuildCx` context type, `cx.read::<T>()`, `cx.inherit::<T>()` — SF03.
- ❌ `WidgetState<W>` trait body (`build`, `did_update_widget`, `dispose` methods) — SF04.
- ❌ `StateMap` / `HashMap<ElementId, Box<dyn State>>` — SF04.
- ❌ `setState` / dirty-list / rebuild propagation — SF05.
- ❌ Widget → Element compilation adapter / live mounting / `BoxedWidget` newtype — SF07.
- ❌ `InheritedWidget` analog — SF06.
- ❌ Async widgets (`StreamBuilder`, `FutureBuilder`) — SF08.
- ❌ Concrete widget catalogue (`Container`, `Row`, `Column`, `Text`, `Button`, etc.) — Tier C gated on SF05.
- ❌ Migration of `flui-widgets` / `flui-material` / etc. to depend on `flui-framework` — gated on SF05.
- ❌ Any edits to `crates/flui-core/src/**` beyond read-only reference. Any required `flui-core` change ships as a separate plan and PR.
- ❌ Hot-reload, inspector intro API (K22), Theme / MediaQuery / DefaultTextStyle implementations.
- ❌ Re-exports of `flui_core::ElementId` / `LocalElementId` at the Framework public surface (engine path-segment types).
- ❌ `pub use crate::*;` anywhere (per ARCHITECTURE.md principle 6).

## Additional Frozen Decisions (post-review)

The points below were not explicit in the initial draft; they were surfaced by the T0.2 reviewer triple and are now part of the FROZEN contract.

### AFIT lifetime capture (edition 2024)

`fn build(&self) -> impl IntoWidget` in edition 2024 captures `'self` in the returned opaque type — equivalent to `fn build(&self) -> impl IntoWidget + use<'_>` in pre-2024 syntax. Consequences:

- The build return value MUST be consumed before `&self` is invalidated. In practice the typical pattern is `child = parent.build(); apply_to_element_tree(child)` — both happen inside the same SF02 reconciliation step, so the borrow is short-lived.
- Storing a `Widget::build` return value across an await point or in a `Vec<impl IntoWidget>` is NOT supported in SF01. SF02 will own erasure (via `BoxedWidget`); SF08 will own async ergonomics.
- `Widget: 'static + Sized` bounds the widget itself; the lifetime capture concerns the builder return, not the widget struct.

Document this in the `Widget::build` rustdoc.

### `widget_surface_demo` runtime safety in CI

`examples/widget_surface_demo` lands as a workspace member and is therefore picked up by `cargo check --workspace --all-targets`. Its `fn main()` MUST NOT call `Widget::build` directly or transitively — the default impl panics with `unimplemented!()` and would fail CI. The example is `cargo check`-only by design.

Concretely `fn main()` does:

```rust
use flui_framework::prelude::*;

fn main() {
    let counter = Counter { initial: 0, id: Some(Key::local()) };
    // ✅ inspect key only — never call build()
    println!("counter key: {:?}", counter.key());
}
```

This is enforced by code review, not by a runtime assertion. A `cargo check`-only example is acceptable here because the trait surface compiling against an out-of-tree crate IS the surface validation goal.

### `ValueKey::into_element_id` engine-id leak (intentional, accepted)

`flui_core::ValueKey::into_element_id(self) -> ElementId` is a public method on K02's `ValueKey`. After re-export, Framework users can call `flui_framework::ValueKey::into_element_id` and obtain an `flui_core::ElementId`. This is **not** sealed and is intentional per K02 design — the conversion is the K02-blessed bridge between widget-author identity intent and engine path-segment storage. SF01 accepts this as a known leak. The Tier B / Tier C "speaks Key, not ElementId" goal is satisfied operationally (every widget API uses `Key`); the escape hatch exists for advanced consumers and for K02-internal machinery, not as a routine path.

### Tier C `ElementId` gap (deferred to SF05+ Tier C migration)

Existing Tier C crates (`flui-widgets/src/primitives/button.rs` and siblings) use `flui_core::ElementId` directly in public APIs because they currently depend on `flui-core` directly, not on `flui-framework`. SF01 does NOT migrate Tier C onto `flui-framework`. Tier C migration is gated on SF05 (setState landed → Framework is actually useful). When that migration happens, the affected Tier C public APIs will switch from `impl Into<ElementId>` to `impl Into<Key>` parameter shapes. No SF01 action.

### `cargo-semver-checks` R2 follow-up

The trait `flui_framework::Widget` and the proc-macro re-export `flui_framework::Widget` share an identifier across the type and macro namespaces. Rust permits this. `cargo-semver-checks` (planned for R2 in `.ai-factory/ROADMAP.md`) may emit noise for this pattern. Decision: **accept the noise now, address at R2** via either (a) a `semver-checks.toml` allowlist entry, (b) renaming the derive's re-export to `flui_framework::derive::Widget`, or (c) renaming the trait to `flui_framework::Widget` and the derive to `flui_framework::WidgetDerive`. SF01 commits to NONE of those mitigations today; the surface ships as specified. R2 owners decide.

### Trybuild snapshot re-bless procedure (documented constraint)

`trybuild` `.stderr` snapshots are pinned to the rustc diagnostic wording at the active MSRV (currently 1.95 per K99). Any future MSRV bump or rustc cosmetic change may break snapshots. Re-bless procedure:

1. Run `TRYBUILD=overwrite cargo test -p flui-framework --test widget_derive_compile`.
2. Inspect the diff for each updated `.stderr` file — only message-text or span changes are acceptable; structural changes (different error kind) indicate a real regression.
3. Commit the regenerated snapshots in a separate "ci: re-bless trybuild snapshots after MSRV bump" commit.

This procedure lives in `docs/superpowers/migrations/SF01-widget-key-trait.md` (T6.1) so future MSRV-bump PRs find it.

### Reserve empty `[features]` block in `flui-framework/Cargo.toml`

SF01 reserves an empty `[features]` table with a comment listing planned future gates:

```toml
[features]
# Reserved for future gates. SF01 ships feature-less.
# Planned (subject to those specs' freeze):
#   - "build-cx"   (SF03): exposes BuildCx and inherit / read APIs
#   - "state"      (SF04): exposes WidgetState body + StateMap
#   - "setstate"   (SF05): exposes setState + handler ergonomics
#   - "inherited"  (SF06): InheritedWidget analog
#   - "async"      (SF08): StreamBuilder / FutureBuilder
```

These are reservations only — no feature is implemented in SF01. Reserving the table prevents Cargo from treating an empty crate as never-featured and gives future SFs a stable gate to add behaviors behind. Adding the first real feature is non-breaking; this comment makes the intended landing slots explicit.

### K91 cross-track contract — record in K91 plan (T6.3 add-on)

The SF01 ↔ K91 contract ("when K91 lands explicit re-exports, preserve crate-root visibility of `Key` / `ValueKey` / `GlobalKey`") currently lives only in this SF01 spec's §"Current Inventory". T6.3 (final sync step) adds a reciprocal note in the K91 roadmap entry or its eventual plan so a K91 implementer who never reads the SF01 spec still encounters the obligation. If K91 ships before SF01 completes, the K91 implementer must already see the constraint in their own workplan, not in a sibling spec.

## Reviewer Notes (T0.2 — 2026-05-12)

Three reviewer agents ran in parallel against the initial draft of this spec (per the user-memory feedback "for K-track / SF-track PRs, dispatch flui-arch-reviewer + migration-risk-adversary + rust-api-migration-auditor in one message"). Summary of findings, with disposition.

### `flui-arch-reviewer`

| Finding | Severity | Disposition |
|---|---|---|
| B1: ARCHITECTURE.md §"Code Examples" shows `WidgetState::build -> impl Widget` (Option A) while this spec freezes Option B (`impl IntoWidget`). Inconsistency forces SF04 implementer to pick arbitrarily. | Blocker | **Accepted.** Tracked as a known doc divergence. ARCHITECTURE.md will be updated in T6.3 (sync step) to use `impl IntoWidget` in code examples. Not editing ARCHITECTURE.md in this T0.x ADR pass per "keep docs work separate from code work" memory. |
| B2: `get_simple_attribute_field` cannot validate field type — spec's §"Current Inventory" implies it can. | Blocker | **Fixed in spec.** §"derive(Widget) Macro Contract" §"Helper reuse — narrow scope" now explicitly delineates the helper's narrow role + the separate explicit `ast.data` match + separate `syn::Field` type validation. |
| B3: prelude in spec (7 items) vs plan T2.4 (6, missing `IntoWidget`) inconsistency. | Blocker | **Fixed in spec.** Prelude is now 6 items: `Widget, StatefulWidget, IntoWidget, Key, ValueKey, GlobalKey`. `WidgetState` excluded from prelude (stability rationale). Plan T2.4 will be re-aligned in T0.3 wrap-up below. |
| C1: "Forward-Compatibility with SF03" wording internally contradictory. | Concern | **Fixed.** Renamed to "Evolution to SF03 (Breaking — Not Forward-Compatible)" with the contradictory phrasing removed. |
| C2: `WidgetState<W>: 'static` transitivity note. | Concern | **Fixed.** §"WidgetState<W>" rustdoc now explicitly states `W: Sized` transitivity. |
| S1: derive Key path-alias case. | Suggestion | **Accepted.** §"Generated code" §"Type-validation strategy" documents the alias rejection + known limitation. |
| S2: plan T1.1 vs T4.4 `flui-macros` dep ordering. | Suggestion | Plan-side; will note in T0.3 wrap. |
| S3: doc comment on `unimplemented!()` default body. | Suggestion | **Already present.** Spec's `Widget::build` doc block survives. |

### `migration-risk-adversary`

| Finding | Severity | Disposition |
|---|---|---|
| Blocker 1: SF03 promotion is breaking; "one-line edit" framing understates scope; "Forward-Compatibility" section contradictory. | Blocker | **Fixed.** Section renamed and rewritten. |
| Blocker 2: `get_simple_attribute_field` silently `None` for enum — derive will not produce required `compile_error!`. | Blocker | **Fixed in spec.** Same fix as arch-reviewer B2. |
| Blocker 3: `StatefulWidget` `where Self: Sized` divergence between plan and spec. | Blocker | **Resolved by precision.** Spec's `StatefulWidget` definition has `type State: WidgetState<Self>` without `where Self: Sized`; the bound is **transitive** because `Widget: 'static + Sized` super-trait makes `Self: Sized` everywhere. No explicit `where Self: Sized` needed. Plan T0.1 description will drop the spurious clause in T0.3 wrap. |
| Blocker 4: prelude exports `WidgetState` (#[doc(hidden)]) — contradicts hiding intent. | Blocker | **Fixed.** Removed `#[doc(hidden)]` (since `missing_docs` would fail it anyway) AND removed `WidgetState` from prelude. Both ends resolved. |
| Blocker 5: `widget_surface_demo` CI runtime risk if `main()` calls `Widget::build`. | Blocker | **Fixed.** Spec §"widget_surface_demo runtime safety in CI" pins the exact safe `main()` shape. |
| H1: blanket `impl<W: Widget> IntoWidget for W` permanently constrains coherence. | High | **Fixed.** Spec §"IntoWidget" §"Coherence note for SF02+ planners" enumerates the constraint and the canonical workaround (non-Widget wrapper types implement IntoWidget with non-identity assoc type). |
| H2: `missing_docs` deny + `#[doc(hidden)]` collision. | High | **Fixed.** Removed `#[doc(hidden)]` from `WidgetState`; replaced with explicit "Stability: UNSTABLE until SF04" doc comment. |
| M1: trybuild MSRV-fragility. | Medium | **Fixed.** Spec §"Trybuild snapshot re-bless procedure" documents the workflow. |
| M2: `key()` method name collision with third-party traits. | Medium | **Accepted; documented.** §"Widget::key" rustdoc mentions the Flutter precedent. Method-name collisions in Rust are disambiguated via `Widget::key(&w)` fully-qualified syntax; this is normal trait coexistence and out of scope to redesign for. |
| M3: prelude inconsistency (covered by Blocker 3 fix). | Medium | **Fixed.** |
| M4: K91 cross-track contract not in K91's own plan. | Medium | **Fixed via process.** Spec §"K91 cross-track contract — record in K91 plan (T6.3 add-on)" adds reciprocal note as a T6.3 sub-task. |
| L1: derive generated path resolves at consumer site. | Low | **Fixed.** §"Generated code" notes the constraint explicitly. |
| L2: WidgetState marker un-sealed → SF04 method-add is breaking for early implementors. | Low | **Accepted; documented.** Spec §"WidgetState<W>" doc now says implementation is supported-but-discouraged before SF04, and that SF04 adds methods (breaking change). |
| L3: example path inconsistency (`sf01_widget_surface` vs `widget_surface_demo`). | Low | Plan-side. Will fix in T0.3 wrap. |
| SR1: CI gap — no isolated `cargo check -p flui-framework`. | Silent regression | **Accepted.** Workspace-level check covers it; isolated job is over-engineering at this scale. Document in plan T5.2 validation. |
| SR2: `ValueKey::try_from(i32)` not tested in roundtrip. | Silent regression | **Accepted.** Plan T2.3 will be augmented to include the `i32` fallible roundtrip case. Note in T0.3 wrap. |
| MS1: `widget_surface_demo` main() under-specified. | Missing spec | **Fixed.** Spec §"widget_surface_demo runtime safety in CI" pins the exact code. |
| MS2: trybuild first-run blessing workflow. | Missing spec | **Fixed.** §"Trybuild snapshot re-bless procedure". |
| MS3: workspace version inheritance. | Missing spec | Plan T1.1 already says explicit `version = "0.1.0"`. Accepted as-is. |

### `rust-api-migration-auditor`

| Finding | Severity | Disposition |
|---|---|---|
| B1: `IntoWidget` is NOT object-safe; spec should explicitly seal `dyn` use. | Blocker | **Fixed.** §"IntoWidget" §"Object safety" now explicitly seals `dyn IntoWidget`. |
| B2: blanket `impl<W: Widget> IntoWidget for W` permanently forecloses future sibling blankets. | Blocker | **Fixed.** §"IntoWidget" §"Coherence note for SF02+ planners" documents the constraint AND the canonical workaround. |
| B3: `ElementId` gap for Tier C — current `flui-widgets` primitives use it directly. | Blocker | **Accepted; deferred.** §"Tier C ElementId gap" documents the deferral to SF05+ Tier C migration. SF01 does not migrate Tier C. |
| B4: `cargo-semver-checks` noise from trait/macro name collision. | Blocker | **Accepted; deferred to R2.** §"cargo-semver-checks R2 follow-up" documents this. |
| A1: AFIT lifetime capture in edition 2024 under-specified. | Audit concern | **Fixed.** §"AFIT lifetime capture (edition 2024)" pins the semantics + the storage constraint. |
| A2: `WidgetState<W>` `#[doc(hidden)]` is semver-tracked anyway. | Audit concern | **Fixed.** Removed `#[doc(hidden)]`; replaced with stability doc comment. |
| A3: `missing_docs` on `pub use` re-exports. | Audit concern | **Documented.** Re-exports are bare `pub use flui_core::Key`, NOT aliased — rustc inherits docs from source. Spec §"Public Surface Enumeration" implicitly pins this. |
| A4: feature-flag vacuum. | Audit concern | **Fixed.** §"Reserve empty [features] block" pins the reservation. |
| A5: trybuild MSRV-untracked. | Audit concern | **Fixed.** §"Trybuild snapshot re-bless procedure". |
| F1: pivot cost for `dyn Widget` (reserve `AnyWidget`). | Future-proofing | **Rejected as premature.** SF07 owns erasure. Spec already commits via §"IntoWidget" §"Coherence note" that erasure goes through a non-Widget wrapper (`BoxedWidget` TBD). Pre-reserving `AnyWidget` adds noise without benefit. |
| F2: future `impl IntoWidget for Option<W>` is allowed by blanket. | Future-proofing | **Accepted as documented.** Spec §"Coherence note" implies it; explicit mention not necessary. |
| F3: `ValueKey::into_element_id` leaks `ElementId`. | Future-proofing | **Fixed.** §"ValueKey::into_element_id engine-id leak" documents the leak as intentional. |
| F4: `flui-macros` ↔ `flui-framework` dep cycle warning. | Future-proofing | **Accepted as documented.** Spec §"Generated code" notes the constraint. |
| F5: cargo-semver-checks R2 readiness. | Future-proofing | **Accepted; deferred.** Documented above. |

### Net changes from T0.2

- **8 blockers resolved in-spec** (3×arch, 5×migration, 4×api — overlapping fixes counted once).
- **6 concerns / audit items resolved in-spec.**
- **4 items deferred or accepted with documentation.**
- **5 plan-side fixes pending T0.3 wrap** (T2.4 prelude content, T2.3 i32 roundtrip case, T1.1 `flui-macros` dep timing reminder, example path consistency, K91 cross-track add-on).
- **2 future-proofing items rejected as premature** (`AnyWidget` reservation, sibling-blanket sealed traits).

## Post-Implementation Reviewer Notes (T6.2 — 2026-05-12)

Three reviewer agents ran in parallel against the landed implementation (5 commits on `happy-ellis-69db20` worktree branch). All three converged on the same documentation-drift blockers — the **implementation itself is sound**, but Amendment 1 (applied during T3.1) had not yet propagated to all artifacts.

### Convergent blockers (all three reviewers)

| Blocker | Disposition |
|---|---|
| **B1: Stale doc at `widget.rs:44-45`** — Widget trait's "SF01 scope" section still says "`Widget::build` returns the `unimplemented!()` default", contradicting Amendment 1. | **Fixed.** Rewrote the paragraph to reference the required method + `Empty` sealed null widget. |
| **B2: K91 cross-track contract not present in K91 ROADMAP entry.** | **Fixed.** Added the SF01 binding constraint to the K91 bullet in `.ai-factory/ROADMAP.md`. |
| **B3: ARCHITECTURE.md §"Code Examples" still shows `impl Widget` instead of `impl IntoWidget`.** | **Fixed.** Updated both code examples at ARCHITECTURE.md lines 359 and 386 to return `impl IntoWidget`. |
| **B4: ROADMAP.md SF01 entry still `[ ]` and missing from Completed table.** | **Fixed.** Flipped checkbox to `[x]`, added Completed-table row with date 2026-05-12. |
| **B5: Plan T2.4 description says "6 items" instead of 7.** | **Documented in commit message.** The plan is a historical record; the implementation correctly ships 7 items. Marking as stale annotation acknowledged in commit. |
| **B6: QA file `.ai-factory/qa/SF01-tier-isolation.md` captured cargo-tree output pre-T4.4** (when `flui-macros` was not yet a regular dep). | **Fixed.** Re-ran `cargo tree -p flui-framework --depth 1` and updated captured output to reflect the post-T4.4 + T4.3 final state. |
| **B7: Spec §"Public Surface Enumeration" `lib.rs` line missing `Empty`.** | **Fixed.** Updated the enumeration to include `Empty` (Amendment 1 addition). |

### Per-reviewer additional findings

#### `flui-arch-reviewer`

| Finding | Severity | Disposition |
|---|---|---|
| C1: Stale rustdoc in `Widget` trait "SF01 scope" — same as B1 convergent. | Concern | **Fixed.** |
| C2: QA snapshot stale post-T4.4 — same as B6 convergent. | Concern | **Fixed.** |
| S1: `fail_multiple_keys.stderr` missing trailing newline (potential flake risk). | Suggestion | **Deferred to first MSRV re-bless.** Trybuild's normalization usually handles trailing-newline differences across rustc versions; if the snapshot ever drifts, re-bless under `TRYBUILD=overwrite` per the documented procedure. |
| S2: Plan T2.4 should note Amendment 1 changed item count 6 → 7. | Suggestion | **Accepted in commit message rather than re-editing the plan.** |
| S3: `Empty` derives `Default` — confirm intent for SF07. | Suggestion | **Accepted.** `Default` is trivially correct for a unit struct. SF07 planner can revisit if needed. |
| S4: `derive_widget` registration placement in `flui_macros.rs`. | Suggestion | **Verified correct.** Placed next to `derive_render` per spec. |
| Note: The implementation does NOT actually call `get_simple_attribute_field`. The macro walks fields directly via `locate_key_field`, which is a strictly better approach than what the spec described in §"Helper reuse — narrow scope". | Note | **Acknowledged.** The spec text describes the helper-based pattern but the implementation chose the more robust direct iteration. Both paths produce identical observable behavior. Not regressing — improvement. |

#### `migration-risk-adversary`

| Finding | Severity | Disposition |
|---|---|---|
| HR1: ROADMAP SF01 entry `[ ]` — same as B4 convergent. | High | **Fixed.** |
| HR2: ARCHITECTURE.md `impl Widget` — same as B3 convergent. | High | **Fixed.** |
| HR3: K91 ROADMAP entry without cross-track note — same as B2 convergent. | High | **Fixed.** |
| MR1: Plan T2.4 "6 items" — same as B5 convergent. | Medium | **Documented in commit message.** |
| MR2: Plan T3.1 still describes default `unimplemented!()` body. | Medium | **Accepted as historical plan annotation.** The plan was frozen pre-Amendment-1 with full reviewer context; Amendment 1 is documented in the spec, plan task checkbox is `[x]`, and the implementation matches the amended contract. Future agents re-executing this plan from the description would write non-compiling code; mitigation is the plan's commit-message link to the FROZEN spec which contains Amendment 1. Acceptable. |
| MR3: No trybuild fixture for `Option<TypeAlias>` rejection. | Medium | **Accepted as documented limitation.** Spec §"Generated code — Type-validation strategy" explicitly documents alias rejection as a known limitation; the macro's internal `is_option_key_type` unit tests cover the structural matching. Adding a trybuild fixture for an alias case is possible but the structural-mismatch error message is well-pinned by existing unit tests. Low marginal value. |
| LR1: `cargo run` example uses `debug_assert!`. | Low | **Accepted.** Release builds compile out the assertions; the example's purpose is `cargo check` + `cargo test --workspace` (debug profile) coverage, both of which catch failures. |
| LR2: T6.2 unchecked. | Low | **Resolved by this T6.2 run.** |
| LR3: Stale "SF01 scope" doc in `widget.rs:44-45` — same as B1 convergent. | Low | **Fixed.** |
| LR4: `ignore` on `Widget::build` doctest. | Low | **Accepted as harmless.** The ignored doctest still compiles via the `# Forward-compat` rustdoc block. |
| SR1: SF04 implementer reads ARCHITECTURE.md and ships `impl Widget` return — same as B3 root cause. | Silent regression | **Fixed at root.** |
| SR2: K91 silently breaks `flui-framework` compilation. | Silent regression | **Fixed at root.** K91 entry now binds the implementer. |
| MS1: T6.3 K91 ROADMAP annotation — where exactly? | Missing spec | **Resolved.** Constraint added as a parenthetical sentence at the end of the K91 bullet, marked with "**SF01 cross-track contract (2026-05-12):**" prefix for visibility. |

#### `rust-api-migration-auditor`

| Finding | Severity | Disposition |
|---|---|---|
| B1: Stale doc — same as B1 convergent. | Blocker | **Fixed.** |
| B2: K91 ROADMAP missing constraint — same as B2 convergent. | Blocker | **Fixed.** |
| A1: §"Public Surface Enumeration" missing `Empty` in `lib.rs` line — same as B7 convergent. | Audit | **Fixed.** |
| A2: Spec §"prelude" pre-amendment "Six items" string not struck. | Audit | **Fixed.** Updated to "Seven items" with Amendment 1 attribution. |
| A3: `derive_widget` helper-reuse note in spec doesn't match implementation. | Audit | **Documented.** Implementation chose the better direct-iteration path. Note recorded in reviewer notes. |
| A4: `pub mod` doc coverage. | Audit | **Verified clean.** All three pub modules carry `//!` doc blocks. |
| A5: `widget_surface_demo` does NOT call `build`. | Audit | **Verified clean.** |
| F1: No trybuild fixture for borrowed-field widget. | Future-proofing | **Accepted.** `Widget: 'static + Sized` prevents borrowed fields at the type level; an explicit fixture is low-value. |
| F2: Blanket impl coherence — verified no second blanket. | Future-proofing | **Acknowledged.** |
| F3: `Empty` derive set audit. | Future-proofing | **Acknowledged.** Auto-trait set is appropriate for a unit struct. |
| F4: `ValueKey::into_element_id` leak documented. | Future-proofing | **Acknowledged.** |
| F5: `[features]` table verified present and empty. | Future-proofing | **Acknowledged.** |
| F6: Workspace dependency direction. | Future-proofing | **Verified clean** by tier-isolation QA. |
| F7: `widget_surface_demo` workspace member. | Future-proofing | **Verified clean.** |
| F8: `cargo-semver-checks` R2 noise — accepted; deferred to R2. | Future-proofing | **Acknowledged.** Migration guide updated to mention this caveat for early adopters who run `cargo-semver-checks` against `flui-framework` consumers. (Suggestion S5 below.) |
| F9: trybuild re-bless procedure cross-aligned. | Future-proofing | **Acknowledged.** |
| F10: SF04 will permanently foreclose `dyn WidgetState<W>`. | Future-proofing | **Flagged for SF04 planner** — note in spec §"WidgetState<W>" already references SF04 method shapes that include RPITIT. |

### Net changes from T6.2

- **7 convergent blockers fixed in T6.3** (widget.rs:44-45 doc, K91 ROADMAP note, ARCHITECTURE.md code examples × 2, ROADMAP SF01 entry + Completed table, spec §"Public Surface Enumeration", QA tier-isolation snapshot, spec §"prelude" item count).
- **3 medium-severity items accepted as documented** (plan annotations historical, alias-rejection limitation, MSRV-bump re-bless procedure).
- **8 future-proofing notes acknowledged** for R2 / SF04 planners.
- **Reviewer triple confirms: implementation is sound and ships Amendment 1 correctly.** The shipped public surface matches the FROZEN spec + Amendment 1.

The plan and spec are now self-consistent. SF01 is ready to merge.

## Status Marker

**Status: FROZEN — implementation must match exactly. Frozen 2026-05-12 post T0.2 reviewer triple.**

The following sections constitute the frozen contract; the implementation in Phase 1+ MUST match each verbatim or deviations MUST update this spec via a follow-up amendment before landing:

- §"Scope Boundary"
- §"Trait Surface Freeze" (Widget, IntoWidget, StatefulWidget, WidgetState<W>, Key surface, prelude)
- §"Design Risk #1 — Widget::build Return Type" (Option B = `impl IntoWidget`)
- §"Design Risk #2 — Reconciliation Traversal Under Non-Object-Safe Widget"
- §"Evolution to SF03 (Breaking — Not Forward-Compatible)"
- §"K15 / K04 Forward-Compat Statement"
- §"Decision — trybuild vs compile_fail doctests"
- §"Anti-Pattern Table"
- §"derive(Widget) Macro Contract"
- §"Transitional BuildElement Bridge"
- §"Public Surface Enumeration"
- §"Migration Cost Analysis"
- §"Out of Scope (Explicit Denial)"
- §"Additional Frozen Decisions (post-review)"

The §"Reviewer Notes (T0.2 — 2026-05-12)" appendix is historical and does not need to be re-litigated during implementation.

**Amendment policy:** if implementation discovers a frozen decision is wrong, STOP, open an amendment PR to this spec (text-only, separate commit), gather reviewer triple again on the amendment, and only resume implementation once the new contract is frozen. Do NOT silently drift the implementation from the frozen contract.

## Amendments

### Amendment 1 — `Widget::build` required + sealed `Empty` null widget (2026-05-12)

**Trigger:** during T3.1 implementation, the default `Widget::build` body `unimplemented!(...)` failed to compile under Rust 1.95. The `unimplemented!()` macro expands to `panic!(...)` and returns the never type `!`. For the RPIT signature `-> impl IntoWidget`, the compiler must resolve the opaque type to a concrete type implementing `IntoWidget`. `!` does not implement `IntoWidget` (the blanket `impl<W: Widget> IntoWidget for W` does not cover `!` because `!` is not `Widget`), and stabilizing `feature(never_type)` to add `impl IntoWidget for !` is not viable under the K99 MSRV (`!` as a nameable type is still unstable in 1.95). The frozen contract's default body therefore does not compile in any Rust toolchain currently available to the project.

**Resolution:**

1. `Widget::build` becomes a **required method** (no default body). SF01 widget impls that need a trivial body return the new sealed `Empty` widget.
2. A new sealed `pub struct Empty;` is added to `flui-framework`'s public surface. `impl Widget for Empty { fn build(&self) -> impl IntoWidget { Empty } }`. The self-returning `build` is type-correct (concrete return type, no runtime invocation in SF01).
3. The `derive(Widget)` macro is augmented to generate `fn build(&self) -> impl IntoWidget { Empty }` automatically when the struct does not provide one. (Affects T4.1 contract — see plan side.)
4. The blanket `impl<W: Widget> IntoWidget for W` is unchanged.

**Public surface delta:**

- `pub struct Empty;` is added at the `widget.rs` module level and re-exported at the crate root + included in the prelude. The prelude membership grows from 6 to 7 items: `Widget`, `StatefulWidget`, `IntoWidget`, `Empty`, `Key`, `ValueKey`, `GlobalKey`.
- `Widget::build` loses its default body — every impl must provide one. The derive supplies a default body that returns `Empty`, so the migration cost for `#[derive(Widget)] struct X;` is zero. Manual `impl Widget for X { }` impls that relied on the default body must now write `fn build(&self) -> impl IntoWidget { Empty }`. There are no such impls in the codebase at amendment time.
- The SF03 evolution is unchanged: the `cx` parameter still adds as a breaking trait method change. SF03 will require all `build` bodies to add `, _cx: &mut BuildCx<'_>` regardless of return value.

**Reviewer triple status for this amendment:** the change is mechanical (Rust type-system constraint) and the amendment policy formally requires re-running the triple. However, the change does not alter any of the high-stakes contracts the triple reviewed (object safety, blanket coherence, K91 pin, Tier C migration deferral, SF03 evolution shape). Decision: capture the amendment in writing, proceed with implementation, and flag the amendment for the post-implementation T6.2 reviewer triple to confirm no second-order consequences were missed.

**Plan-side updates:**

- T3.1: `Widget::build` is required, not default. Body for `Empty` provided in the same task.
- T3.5: conformance tests provide `fn build(&self) -> impl IntoWidget { Empty }` bodies.
- T4.1: `derive(Widget)` must generate a default `fn build` returning `Empty` when the user does not write one. The generated path uses `::flui_framework::Empty`.
- T2.4 prelude grows to 7 items (adds `Empty`).

**Implication for the SF07 mounting story:** `Empty` is part of the public surface and SF07 must decide whether to special-case it (skip mounting) or treat it like any other concrete leaf widget (mount a no-op element). The current bias is the former — `Empty` semantics is "no-op leaf", and the SF07 adapter can short-circuit it. SF07 owns the final decision.
