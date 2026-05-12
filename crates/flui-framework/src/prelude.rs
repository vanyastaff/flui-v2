//! Opt-in convenience re-exports.
//!
//! `use flui_framework::prelude::*;` imports the seven items widget
//! authors reach for in almost every file:
//!
//! ```text
//! Widget          — the immutable widget trait
//! StatefulWidget  — widget with mutable state across rebuilds
//! IntoWidget      — conversion contract for Widget::build returns
//! Empty           — sealed null widget (SF01 Amendment 1)
//! Key             — widget identity intent type
//! ValueKey        — reorder-stable list-item identity
//! GlobalKey       — cross-tree global identity handle
//! ```
//!
//! `WidgetState` is **intentionally omitted** from the prelude because
//! its body is unstable until SF04. Import it individually from the
//! crate root (`use flui_framework::WidgetState;`) if you really need
//! to implement it in SF01-vintage code — see the trait's own
//! documentation for the stability caveat.
//!
//! The prelude is opt-in. Every item is also importable individually
//! from the crate root, so consumers who prefer explicit imports can
//! skip the glob altogether.
//!
//! # Example
//!
//! ```ignore
//! use flui_framework::prelude::*;
//!
//! #[derive(Widget)]
//! struct Counter {
//!     initial: i32,
//!     #[widget(key)]
//!     id: Option<Key>,
//! }
//! ```
//!
//! (The example is `ignore`d in SF01 because `derive(Widget)` lands in
//! T4.1 of the SF01 plan — currently a future commit.)

pub use crate::{Empty, GlobalKey, IntoWidget, Key, StatefulWidget, ValueKey, Widget};
