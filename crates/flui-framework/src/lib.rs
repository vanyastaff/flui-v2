//! # flui-framework
//!
//! Tier B of flui-v2's three-tier architecture. This crate publishes the
//! Flutter-style developer-experience layer that ecosystem widget crates
//! (Tier C — `flui-widgets`, `flui-material`, `flui-cupertino`,
//! `flui-theme`, `flui-navigator`, third-party crates) build on top of.
//!
//! ## Tier model
//!
//! ```text
//!  Tier C — Ecosystem    flui-widgets, flui-material, …, third-party crates
//!                              │
//!                              ▼
//!  Tier B — Framework    flui-framework (this crate) — Widget + Key + State
//!                                                      + BuildCx + Provider
//!                                                      + reconciliation
//!                              │
//!                              ▼
//!  Tier A — Engine       flui-core, flui-platform, flui-macros
//! ```
//!
//! Dependencies flow strictly downward. `flui-framework` depends on
//! `flui-core` only; consumers in Tier C depend on `flui-framework`.
//!
//! ## "2 structures + 1 cache" model
//!
//! Where Flutter has four internal trees (Widget / Element / RenderObject /
//! Layer), `flui-framework` collapses to two structures plus one cache:
//!
//! - **Widget** — immutable owned config struct, recreated each rebuild,
//!   cheap to clone. Defines what the UI should look like.
//! - **Element tree** — the existing `flui_core::Element` runtime that
//!   owns layout, painting, hit-testing. Unchanged from Tier A.
//! - **StateMap** — flat `HashMap<ElementId, Box<dyn State>>` (lands in
//!   SF04) keyed by element identity. Survives rebuilds.
//!
//! Reconciliation matches old children to new by `(TypeId, Key)` per
//! sibling position — **O(siblings) per level, not O(tree)**. Lands in
//! SF02.
//!
//! ## SF01 scope
//!
//! This crate's initial release (SF01) ships only the trait surface:
//!
//! - [`Widget`] — the immutable widget trait.
//! - [`StatefulWidget`] — widget with mutable state across rebuilds.
//! - [`WidgetState`] — forward-declared marker for the per-element state
//!   companion (body lands in SF04).
//! - [`IntoWidget`] — conversion contract for `Widget::build` returns.
//! - [`Key`], [`ValueKey`], [`GlobalKey`] — re-exports of the K02 identity
//!   substrate from `flui_core`.
//! - [`prelude`] — opt-in `use flui_framework::prelude::*;` convenience.
//!
//! SF01 does **not** ship reconciliation, `BuildCx`, `setState`,
//! `WidgetState` body, or a mounting adapter. See the SF01 design spec at
//! `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`
//! for the precise frozen contract.
//!
//! ## Engine vs Framework — anti-pattern table
//!
//! `flui-core` contains several similarly-named traits that are
//! intentionally distinct from `flui-framework::Widget`:
//!
//! | Trait | Tier | When to use |
//! |---|---|---|
//! | [`Widget`] | B | Writing a widget for an app or another widget to compose. |
//! | [`flui_core::Render`] | A | Engine-internal mutable view trait for window roots. |
//! | [`flui_core::RenderOnce`] | A | Engine compatibility path for stateless engine recipes. |
//! | [`flui_core::ElementBuilder`] | A | K03 engine substrate; the eventual lowering target of `Widget::build` (SF07 adapter). |
//!
//! Tier C widget authors write [`Widget`] / [`StatefulWidget`]. Engine
//! traits stay engine-internal.

#![deny(missing_docs)]

pub mod key;
pub mod prelude;
pub mod widget;

pub use crate::key::{GlobalKey, Key, ValueKey};
pub use crate::widget::{Empty, IntoWidget, StatefulWidget, Widget, WidgetState};

// Re-export the `#[derive(Widget)]` proc-macro so consumers can write
// `use flui_framework::Widget;` for both the trait and the derive. The
// trait and the macro coexist in different namespaces (types vs
// macros) per Rust's namespace rules. See SF01 design spec
// §"derive(Widget) Macro Contract".
pub use flui_macros::Widget;
